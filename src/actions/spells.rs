use std::collections::HashSet;
use std::sync::LazyLock;

use crate::{
    actions::action_template::{
        action_and_slot, action_only, actor_lacks_condition, bonus_action_and_slot,
        bonus_action_only, first_ally_target_id, first_target_id, first_target_location, Action,
        TargetingSchema,
    },
    actors::actor_template::ConcentrationData,
    conditions::{Condition, ConditionTimer},
    engine::{
        action_overrides::ActionOverride,
        dice::Dice,
        encounter::EncounterInstance,
        saves::SaveDamagePolicy,
        side_effects::{
            ApplicableSideEffect, ApplyCondition, DealDamage, GainTempHp, Heal, Resource,
            StartConcentration, install_condition_with_link,
        },
        types::{AbilityScoreType, Coordinate, DamageType, SpellSchool},
    },
};

/// Resolve a spell attack roll vs `target_id`'s AC. Logs the breakdown,
/// rolls damage on hit (with crit-doubled dice on a nat-20), returns the
/// queued `DealDamage` (or nothing on a miss). Centralized so single-target
/// damaging spells (Guiding Bolt, Inflict Wounds, future spells) don't
/// each re-implement attack-roll + crit + log glue.
///
/// The default flavor takes no flat damage bonus; for spells that add an
/// ability modifier to damage (Spiritual Weapon, etc.) call
/// `spell_attack_with_bonus` instead. For spells that need the resolved
/// damage value (Vampiric Touch's half-as-heal rider), use
/// `spell_attack_outcome` directly.
#[allow(clippy::too_many_arguments)]
fn spell_attack(
    encounter: &mut EncounterInstance,
    caster_id: usize,
    target_id: usize,
    action_name: &str,
    attack_bonus: i32,
    damage_dice: Dice,
    damage_type: DamageType,
    is_melee: bool,
) -> Vec<Box<dyn ApplicableSideEffect>> {
    spell_attack_outcome(
        encounter,
        caster_id,
        target_id,
        action_name,
        attack_bonus,
        damage_dice,
        0,
        damage_type,
        is_melee,
    )
    .0
}

/// Same as `spell_attack` but adds a flat `damage_bonus` (e.g. caster's
/// WIS modifier for Spiritual Weapon) to the rolled damage. Crit doubles
/// only the dice — flat bonuses are added once per RAW.
#[allow(clippy::too_many_arguments)]
fn spell_attack_with_bonus(
    encounter: &mut EncounterInstance,
    caster_id: usize,
    target_id: usize,
    action_name: &str,
    attack_bonus: i32,
    damage_dice: Dice,
    damage_bonus: i32,
    damage_type: DamageType,
    is_melee: bool,
) -> Vec<Box<dyn ApplicableSideEffect>> {
    spell_attack_outcome(
        encounter,
        caster_id,
        target_id,
        action_name,
        attack_bonus,
        damage_dice,
        damage_bonus,
        damage_type,
        is_melee,
    )
    .0
}

/// Lower-level spell-attack resolver. Returns both the queued side-effects
/// (DealDamage on hit, empty on miss) and the post-crit damage value that
/// will land — `0` on a miss. Useful for spells that need to chain off
/// the dealt damage value (e.g. Vampiric Touch's half-as-heal rider)
/// without re-rolling the damage dice and double-consuming the RNG.
#[allow(clippy::too_many_arguments)]
/// Outcome of a spell attack roll: whether it connected, and whether it
/// was a critical hit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpellAttackRoll {
    /// `false` on a miss, on a natural 1, on a Sanctuary fizzle, and on
    /// an intercepted swing — in every case the caller applies nothing.
    pub hit: bool,
    pub is_crit: bool,
}

/// Roll a spell attack against `target_id` and report whether it landed,
/// applying every rule a spell attack is subject to along the way: cover
/// and Multiattack Defense on the AC, Sanctuary, the advantage rider
/// stack (and its clearing), the defender-side reactive taxes, Bless /
/// Bane, the caster's attack buffs, Lucky, the target-scoped crit range,
/// the natural-1 auto-miss, Seeking Spell, Bend Luck, the
/// Paralyzed/Unconscious auto-crit clause, the interception cohort, and
/// the hit-this-turn mark. Logs the breakdown.
///
/// Split out of `spell_attack_outcome` so that a spell attack whose
/// effect *isn't* damage can still be a first-class spell attack. Ray of
/// Enfeeblement is the case: it rolls to hit and then installs a
/// condition, so the damage-rolling resolver was no use to it and it
/// open-coded its own roll instead — which silently cost it cover,
/// Sanctuary, Bless, the reactive taxes, Multiattack Defense, the
/// interception cohort and the hit mark. A future "attack roll, then a
/// rider" spell gets all of it by calling this.
#[allow(clippy::too_many_arguments)]
pub fn spell_attack_roll(
    encounter: &mut EncounterInstance,
    caster_id: usize,
    target_id: usize,
    action_name: &str,
    attack_bonus: i32,
    is_melee: bool,
) -> SpellAttackRoll {
    let target_ac = encounter
        .actors
        .get(&target_id)
        .map(|a| a.armor_class() as i32)
        .unwrap_or(10);
    // 5e Cover: intervening creatures bump the target's effective AC,
    // same as for weapon swings. Spell attacks (Fire Bolt, Guiding Bolt,
    // Scorching Ray, etc.) honor the rule identically.
    let cover_bonus = encounter.cover_ac_bonus(caster_id, target_id);
    // 5e Hunter Ranger Multiattack Defense (Defensive Tactics, lv7):
    // spell attacks honor the same +4 AC envelope as weapon swings —
    // RAW says "when a creature hits you with an attack" without a
    // weapon-only qualifier. Shared with the weapon-attack path via
    // `EncounterInstance::multiattack_defense_ac_bonus`.
    let multiattack_defense_bonus =
        encounter.multiattack_defense_ac_bonus(caster_id, target_id);
    let target_ac = target_ac + cover_bonus + multiattack_defense_bonus;
    // 5e Sanctuary: gate spell attacks the same way weapon attacks are
    // gated — attacker rolls a WIS save vs the ward's DC. On fail, the
    // spell silently fizzles against the warded target.
    if encounter.sanctuary_save_blocks(caster_id, target_id) {
        return SpellAttackRoll { hit: false, is_crit: false };
    }
    encounter.break_sanctuary_on_hostile(caster_id);
    // Spell attacks are attack rolls per 5e RAW, so the full rider stack
    // applies: Help, Hidden, Bless, Mocked, etc. Route through
    // attack_mode_with_riders so a one-shot Help grant on the caster is
    // consumed exactly once (matching weapon-attack semantics in
    // resolve_attack).
    let mode = encounter.attack_mode_with_riders(caster_id, target_id, is_melee);
    // Defender-side reactive taxes on the attack roll — Fighting Style:
    // Protection and the per-rest `REACTIVE_ATTACK_DISADVANTAGE_SOURCES`
    // cohort (Light Cleric Warding Flare, Great Old One Warlock Entropic
    // Ward). Shared with the weapon path in `engine::attack` through one
    // engine chokepoint, since RAW's triggers ("when a creature you can
    // see attacks a target other than you" / "when a creature attacks
    // you") say nothing about weapons. Layered here, after
    // `attack_mode_with_riders`, so advantage the spell picked up from
    // Help / Bless / Hidden still combines cleanly via `RollMode::combine`.
    let mode = encounter.apply_reactive_attack_taxes(caster_id, target_id, mode);
    // Pull through the same caster-side flat buffs (Bless / Bane d4,
    // attack_bonus_buff, condition_attack_bonus) that weapon attacks
    // get via `resolve_attack`. This keeps spell-attack rolls
    // consistent with weapon swings — Sacred Weapon's +CHA fires on
    // spell attacks too (e.g. a Sacred-Weapon paladin casting Guiding
    // Bolt as a multiclass with cleric / divine soul). Shared with
    // weapon attacks via `EncounterInstance::caster_attack_buffs`.
    //
    // Read these BEFORE the rider clear below so the Inspired die's
    // +3 (and any other `condition_attack_bonus` contribution) is
    // still active when we sum the bonus. Mirrors the same ordering
    // fix in `resolve_attack`.
    let (buff, cond_attack_bonus) = encounter.caster_attack_buffs(caster_id);
    let (bless_die, bless_note) = encounter.bless_bane_attack_die(caster_id);
    // Burn through the one-shot rider stack (Helped, Hidden, per-target
    // help grant, Invisibility concentration, Inspired). Same hook as
    // weapon attacks — kept identical so a Helped wizard firing Fire
    // Bolt consumes their help-grant exactly like a Helped fighter
    // swinging a longsword. Unfailing Inspiration is captured across
    // the same clear for the same reason as on the weapon path — the
    // back-link it reads goes away with the flag.
    let unfailing = encounter.unfailing_inspiration_granter(caster_id);
    let was_inspired = encounter
        .actors
        .get(&caster_id)
        .is_some_and(|a| a.has_condition(Condition::Inspired));
    encounter.clear_attack_advantage_riders(caster_id, target_id);
    // 5e Lucky: reroll a nat-1 if the caster has the trait. Mirrors the
    // identical hook on the weapon-attack path.
    let mut raw = encounter.roll_d20_lucky(caster_id, mode) as i32;
    let mut total = raw + attack_bonus + buff + cond_attack_bonus + bless_die;
    // 5e Improved Critical: template-driven crit threshold. Spell attacks
    // honor the lower threshold too — a Champion fighter multiclassed
    // into Eldritch Knight crits Fire Bolt on 19s. Read via the
    // target-aware engine helper so the default (20) folds in for
    // non-Champion casters, and so a Hexblade's Curse on *this* target
    // widens the range for the hexblade who cast it.
    let mut nat_crit = raw >= encounter.crit_threshold_against(caster_id, target_id);
    // 5e: "If the d20 roll for an attack is a 1, the attack misses
    // regardless of any modifiers or the target's AC." The rule is
    // written about attack rolls, not about weapons, so a spell attack
    // auto-misses on a natural 1 exactly like a longsword swing.
    let mut is_nat_one = raw == 1;
    let mut hit = !is_nat_one && (nat_crit || total >= target_ac);
    // 5e Tasha's Sorcerer Seeking Spell metamagic: on a miss, if the
    // caster has the prime up, reroll the d20 and use the new result
    // (RAW: "you must use the new roll"). Routed through the same
    // mode + Lucky chain so a primed sorcerer keeps their other riders.
    if !hit {
        let new_raw = encounter.reroll_seeking_spell(caster_id, raw as u32, mode) as i32;
        if new_raw != raw {
            raw = new_raw;
            total = raw + attack_bonus + buff + cond_attack_bonus + bless_die;
            nat_crit = raw >= encounter.crit_threshold_against(caster_id, target_id);
            is_nat_one = raw == 1;
            hit = !is_nat_one && (nat_crit || total >= target_ac);
        }
    }
    // 5e Wild Magic Sorcerer Bend Luck (lv6): the target may burn 2 SP +
    // reaction to subtract a 1d4 from the attacker's roll. Symmetric with
    // the weapon-attack hook in `resolve_attack_outcome` — both lanes
    // share the same `apply_bend_luck_penalty` site so a sorcerer's
    // bend fires whether the incoming attack is a longsword or a Fire
    // Bolt. Bypassed on nat-crit (the d20 face stands) and nat-1 (already
    // a miss).
    let bend_penalty = if hit && !nat_crit && !is_nat_one {
        encounter.apply_bend_luck_penalty(target_id, caster_id) as i32
    } else {
        0
    };
    let bend_note = if bend_penalty > 0 {
        total -= bend_penalty;
        hit = total >= target_ac;
        format!(" -d4({})", bend_penalty)
    } else {
        String::new()
    };
    // 5e Paralyzed / Unconscious clause — touch spell attacks honor the
    // "any hit within 5ft becomes a crit" rider too. Mirrors the gate in
    // `resolve_attack_outcome`: only `is_melee` spells trigger.
    let is_crit = nat_crit
        || (hit && encounter.target_grants_melee_auto_crit(caster_id, target_id, is_melee));
    let outcome = if is_nat_one {
        "miss (nat 1)"
    } else if is_crit {
        "CRIT!"
    } else if hit {
        "hit"
    } else {
        "miss"
    };
    let cover_note = EncounterInstance::cover_log_suffix(cover_bonus);
    encounter.log(format!(
        "  {}: 1d20({}){:+}{}{} = {} vs AC {}{}{} \u{2014} {}",
        action_name,
        raw,
        attack_bonus + buff + cond_attack_bonus,
        bless_note,
        bend_note,
        total,
        target_ac,
        cover_note,
        mode.log_suffix(),
        outcome,
    ));
    if !hit {
        // 5e Unfailing Inspiration — same refund as the weapon path.
        if was_inspired && let Some(granter) = unfailing {
            encounter.refund_unfailing_inspiration(caster_id, granter);
        }
        return SpellAttackRoll { hit: false, is_crit: false };
    }
    // Post-hit interception (Mirror Image decoys, Illusory Self): spell
    // attack rolls trigger every row of the cohort too (RAW, uniformly:
    // "any attack roll against you"). Shared with weapon swings via the
    // engine's `attack_intercepted` helper.
    if encounter.attack_intercepted(target_id, caster_id, is_crit) {
        return SpellAttackRoll { hit: false, is_crit: false };
    }
    // 5e Hunter Ranger Multiattack Defense (Defensive Tactics, lv7):
    // record the connecting spell hit so subsequent spell / weapon
    // attacks from this caster against the same target this turn eat
    // the +4 AC penalty above. Written after the interception cohort so
    // a swing that landed on a decoy or an illusory duplicate doesn't
    // count as a "hit on you" per RAW. Shared with
    // the weapon-attack path in `engine::attack::resolve_attack_outcome`.
    if let Some(attacker) = encounter.actors.get_mut(&caster_id) {
        attacker.mark_hit_target_this_turn(target_id);
    }
    SpellAttackRoll { hit: true, is_crit }
}

#[allow(clippy::too_many_arguments)]
fn spell_attack_outcome(
    encounter: &mut EncounterInstance,
    caster_id: usize,
    target_id: usize,
    action_name: &str,
    attack_bonus: i32,
    damage_dice: Dice,
    damage_bonus: i32,
    damage_type: DamageType,
    is_melee: bool,
) -> (Vec<Box<dyn ApplicableSideEffect>>, u32) {
    let roll = spell_attack_roll(
        encounter,
        caster_id,
        target_id,
        action_name,
        attack_bonus,
        is_melee,
    );
    if !roll.hit {
        return (Vec::new(), 0);
    }
    let is_crit = roll.is_crit;
    // Route the base damage roll through the shared caster-aware
    // chokepoint, the same one every save-based spell uses. Spell
    // *attacks* were the last damage lane in the file rolling outside
    // it, which meant the Sorcerer's Empowered Spell reroll, the
    // Evocation Wizard's Empowered Evocation and the cleric's Potent
    // Spellcasting all silently skipped every attack-roll spell — Fire
    // Bolt, Guiding Bolt, Scorching Ray, Chill Touch, Ray of Frost,
    // Eldritch Blast, Thorn Whip, Vampiric Touch and the rest. RAW
    // names Fire Bolt's own school in Empowered Evocation's text, so
    // this was the lane the feature most obviously meant to cover.
    //
    // Multi-ray spells (Scorching Ray, Eldritch Blast) call this helper
    // once per beam; the once-per-cast latch on the cast frame is what
    // keeps the flat bonuses from being paid per ray.
    let dmg = encounter.roll_empowered_sum(caster_id, damage_dice.count, damage_dice.faces) as i32;
    // The crit dice stay a plain roll. RAW doubles the *dice* on a
    // crit and adds flat bonuses once, so folding this through the
    // chokepoint would be asking for a second payout the latch would
    // refuse anyway — and would consume nothing, since Empowered
    // Spell's prime is already spent on the roll above.
    let crit_extra = if is_crit { encounter.roll(&damage_dice) as i32 } else { 0 };
    // Fold in the caster-side flat damage bonuses (item-passive +
    // spell-installed buff) so spell attacks see the same `+N weapon`
    // damage half that weapon swings get via `engine::attack`. Read
    // through the shared encounter helper.
    //
    // `curse_damage_bonus` is the target-scoped half: a hexblade adds
    // their proficiency bonus against the one creature they cursed, and
    // nobody else's beam picks it up. Added here rather than to
    // `caster_damage_buffs` because that helper takes no target — the
    // whole point of this bonus is that it depends on who is being hit.
    let caster_damage_buff = encounter.caster_damage_buffs(caster_id)
        + encounter.curse_damage_bonus(caster_id, target_id)
        + crate::engine::attack::attack_damage_penalty(encounter, caster_id);
    let total_damage_bonus = damage_bonus + caster_damage_buff;
    let total_dmg = (dmg + crit_extra + total_damage_bonus).max(0) as u32;
    encounter.log(format!(
        "  {}: {}({}){} = {} {:?}{}",
        action_name,
        damage_dice,
        dmg,
        if total_damage_bonus != 0 {
            format!("{:+}", total_damage_bonus)
        } else {
            String::new()
        },
        total_dmg,
        damage_type,
        if is_crit {
            format!(" (+{} crit)", crit_extra)
        } else {
            String::new()
        }
    ));
    // Attacker-scoped reductions (Ancestral Protectors) — before the
    // reactive clamps, matching the weapon path.
    let total_dmg = crate::engine::attack::attacker_scoped_damage_reduction(
        encounter, caster_id, target_id, total_dmg,
    );
    // Reactive damage clamps — Uncanny Dodge, Deflect Missiles, Parry,
    // Fighting Style: Interception. Shared with the weapon path in
    // `engine::attack` via the `REACTIVE_DAMAGE_CLAMPS` cohort walker,
    // so RAW's per-feature trigger wording ("hits you with an attack"
    // vs "melee attack" vs "ranged weapon attack") decides which rows
    // fire here rather than which chokepoint happens to have the block
    // pasted into it. `is_spell: true` keeps Deflect Missiles out — RAW
    // gates it on a ranged *weapon* attack — while Uncanny Dodge and
    // Interception, whose RAW wording covers any attack, now cover
    // spell attacks too.
    let total_dmg = crate::engine::attack::apply_reactive_damage_clamps(
        encounter,
        caster_id,
        target_id,
        total_dmg,
        is_melee,
        true,
        damage_type,
    );
    let mut effects: Vec<Box<dyn ApplicableSideEffect>> = vec![Box::new(DealDamage {
        actor_id: target_id,
        amount: total_dmg,
        damage_type,
    })];
    // Caster-side on-hit riders whose RAW trigger is "hit a creature
    // with an attack" rather than "with a weapon attack". The shared
    // table is walked here with `is_spell: true`, so only
    // `RiderLane::AnyAttack` rows fire — the Smites and the weapon-
    // scoped buffs stay out. Before the lane existed the table was
    // reachable only from the weapon chokepoint, which silently
    // narrowed every rider to weapons whether RAW said so or not.
    let total_dmg = total_dmg.saturating_add(crate::engine::attack::push_on_hit_riders(
        encounter,
        &mut effects,
        caster_id,
        target_id,
        crate::engine::attack::RiderSwing {
            is_melee,
            is_spell: true,
            is_crit,
            damage_so_far: total_dmg,
        },
    ));
    // 5e Hex rider on spell attacks. The Hex spell RAW says "you deal
    // extra 1d6 necrotic damage to the target whenever you hit it with
    // an attack" — both weapon and spell attacks trigger the rider.
    // Hunter's Mark RAW is weapon-only, so it's not duplicated here.
    if encounter.is_hex_target(caster_id, target_id) {
        crate::engine::attack::push_die_rider(
            encounter,
            &mut effects,
            target_id,
            Dice::new(1, 6),
            is_crit,
            DamageType::Necrotic,
            "hex",
        );
    }
    // Melee retaliation against the caster. Every reflect source — Fire
    // Shield, Armor of Agathys, Investiture of Flame, and the creature-
    // intrinsic ones — triggers on RAW's "hits you with a melee attack",
    // and a touch spell is a melee attack. Shared with the weapon path so
    // a wizard who reaches out to Shocking Grasp a Fire Shielded target
    // eats the same 2d8 a fighter would.
    if is_melee {
        crate::engine::attack::push_melee_reflect_riders(
            encounter,
            &mut effects,
            caster_id,
            target_id,
        );
    }
    // Reflect sources whose RAW trigger is "hits you with an attack",
    // full stop — Scornful Rebuke. A Conquest Paladin answers a Fire
    // Bolt the same way they answer a longsword, so this lane sits
    // outside the melee gate above.
    crate::engine::attack::push_any_attack_reflect_riders(
        encounter,
        &mut effects,
        caster_id,
        target_id,
    );
    (effects, total_dmg)
}

/// Who's caught in a save-burst: enemies only (allies on the safe side
/// of a directed effect — Fireball, Burning Hands, the wall spells) or
/// everyone in radius (non-discriminating shrapnel — Ice Knife's
/// shatter, Sword Burst). Selects the target-set helper on the encounter
/// inside the shared resolver.
#[derive(Clone, Copy)]
enum BurstTargets {
    Enemy,
    Neutral,
}

impl BurstTargets {
    /// Pull the target id list from the encounter using the right
    /// helper. Both helpers share the caster-exclusion / combat-active
    /// / footprint-in-radius filter so the resolver doesn't have to
    /// re-inline either loop body.
    fn ids(
        self,
        encounter: &EncounterInstance,
        caster_id: usize,
        point: Coordinate,
        radius: isize,
    ) -> Vec<usize> {
        match self {
            BurstTargets::Enemy => encounter.enemy_burst_targets(caster_id, point, radius),
            BurstTargets::Neutral => encounter.neutral_burst_targets(caster_id, point, radius),
        }
    }
}

// `SaveDamagePolicy` (formerly the private `SaveOutcome` enum) lives in
// `engine::saves` so the same post-save / evasion damage logic is shared
// with the item-side `SingleSaveDamageItem` factor and the AoE-burst
// helper in `actions::action_template`. Imported at the top of the file.

/// Roll one shared damage value for a burst, log it, and hand the target
/// set to the engine's single per-target save loop.
///
/// Returns `(damage_effects, per_target_save_results)` — the saves
/// vector is `(target_id, passed)` for every actor that took the save so
/// callers can attach per-target failure riders (Tidal Wave's Prone,
/// Mental Prison's Restrained, Earth Tremor's Prone) without re-walking
/// the burst.
///
/// The loop itself is `action_template::resolve_burst_targets`, which is
/// also what the class-feature and item bursts resolve through. What
/// stays here is the two things that are genuinely spell-side: the
/// caster-aware damage roll (so the Sorcerer's Empowered Spell and the
/// Evocation Wizard's Empowered Evocation apply to burst spells) and the
/// one "  {name}: {dice}({roll}) shared {damage_type}" line above the
/// per-target save lines that `roll_save` emits.
///
/// This function used to carry its own copy of the loop, and the copy is
/// how the Heightened Spell and Empowered Spell bugs in the comments
/// above got to exist in the first place: a rule added to one loop and
/// not the other is invisible at both call sites. The wrappers
/// (`enemy_burst_save_for_half` / `neutral_burst_save_for_half` /
/// `neutral_burst_save_only`) pre-pick the two enum dimensions so call
/// sites stay one-liner-readable. The concentration-anchored variant
/// (Wall of Light / Black Tentacles / Caustic Brew) routes through
/// `concentration_burst_with_rider` instead.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn burst_save_damage(
    encounter: &mut EncounterInstance,
    caster_id: usize,
    point: Coordinate,
    radius: isize,
    save_ability: AbilityScoreType,
    dc: i32,
    dice: Dice,
    damage_type: DamageType,
    action_name: &str,
    targets: BurstTargets,
    outcome: SaveDamagePolicy,
) -> (Vec<Box<dyn ApplicableSideEffect>>, Vec<(usize, bool)>) {
    // Caster-aware damage roll: routes through the shared chokepoint so
    // the Sorcerer's Empowered Spell metamagic (reroll low dice) and the
    // Evocation Wizard's Empowered Evocation (+INT mod) both apply to
    // burst spells.
    let raw = encounter.roll_empowered_sum(caster_id, dice.count, dice.faces);
    encounter.log(format!(
        "  {}: {}({}) shared {:?}",
        action_name, dice, raw, damage_type
    ));
    // Pre-compute the shielded ally set (Sorcerer Careful Spell,
    // Evocation Wizard Sculpt Spells). Only meaningful for Neutral
    // bursts — Enemy bursts already exclude allies at the target-list
    // step, so there is nobody left for it to spare.
    let shielded = match targets {
        BurstTargets::Enemy => std::collections::HashSet::new(),
        BurstTargets::Neutral => encounter.auto_pass_shielded_allies(caster_id, point, radius),
    };
    let target_ids = targets.ids(encounter, caster_id, point, radius);
    crate::actions::action_template::resolve_burst_targets(
        encounter,
        caster_id,
        &target_ids,
        save_ability,
        dc,
        raw,
        damage_type,
        outcome,
        &shielded,
    )
}

/// Resolve an enemy-only AoE burst where each victim makes a save for
/// half damage off a *shared* damage roll. Matches 5e's standard AoE
/// semantics (Fireball / Cone of Cold / Aganazzar's Scorcher / Dawn /
/// Fire Storm / Tidal Wave / Mental Prison's burst-variant). Allies
/// inside the radius are spared via `enemy_burst_targets`.
///
/// Thin wrapper that picks `BurstTargets::Enemy` + `SaveDamagePolicy::HalfOnSave`
/// over the shared `burst_save_damage` resolver. See that function for
/// the load-bearing loop body and logging shape.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn enemy_burst_save_for_half(
    encounter: &mut EncounterInstance,
    caster_id: usize,
    point: Coordinate,
    radius: isize,
    save_ability: AbilityScoreType,
    dc: i32,
    dice: Dice,
    damage_type: DamageType,
    action_name: &str,
) -> (Vec<Box<dyn ApplicableSideEffect>>, Vec<(usize, bool)>) {
    burst_save_damage(
        encounter,
        caster_id,
        point,
        radius,
        save_ability,
        dc,
        dice,
        damage_type,
        action_name,
        BurstTargets::Enemy,
        SaveDamagePolicy::HalfOnSave,
    )
}

/// Friend-or-foe variant of `enemy_burst_save_for_half`. Routes through
/// `neutral_burst_targets` (every combat-active actor in the burst
/// except the caster) instead of `enemy_burst_targets`, so allies
/// inside the radius take the save and the damage just like enemies.
/// Used by spells whose damage is non-discriminating shrapnel — Ice
/// Knife's shatter, Circle of Death, Incendiary Cloud, Tsunami.
///
/// Thin wrapper that picks `BurstTargets::Neutral` + `SaveDamagePolicy::HalfOnSave`
/// over the shared `burst_save_damage` resolver.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn neutral_burst_save_for_half(
    encounter: &mut EncounterInstance,
    caster_id: usize,
    point: Coordinate,
    radius: isize,
    save_ability: AbilityScoreType,
    dc: i32,
    dice: Dice,
    damage_type: DamageType,
    action_name: &str,
) -> (Vec<Box<dyn ApplicableSideEffect>>, Vec<(usize, bool)>) {
    burst_save_damage(
        encounter,
        caster_id,
        point,
        radius,
        save_ability,
        dc,
        dice,
        damage_type,
        action_name,
        BurstTargets::Neutral,
        SaveDamagePolicy::HalfOnSave,
    )
}

/// Cantrip-flavored neutral burst: every combat-active actor in the
/// burst (caster excluded) makes a save against `dc` using `save_ability`.
/// Failed save = full damage from a *shared* roll, success = no damage.
/// Used by Thunderclap, Acid Splash, Sword Burst, Earth Tremor, etc.
///
/// Thin wrapper that picks `BurstTargets::Neutral` + `SaveDamagePolicy::NoneOnSave`
/// over the shared `burst_save_damage` resolver — distinct from
/// `neutral_burst_save_for_half` because cantrips canonically don't
/// half-on-save (a passed save is a clean miss).
/// Enemy-only sibling of `neutral_burst_save_only`: every combat-active
/// *hostile* in the burst saves against `dc` using `save_ability`, and
/// a failed save takes full damage from a shared roll while a passed
/// one takes none.
///
/// The fourth corner of the burst-resolver grid — the other three are
/// `enemy_burst_save_for_half`, `neutral_burst_save_for_half` and
/// `neutral_burst_save_only`. This is the one a cantrip whose RAW text
/// says "each creature *of your choice*" wants: friend-or-foe is the
/// wrong target set (the caster picks), and half-on-save is the wrong
/// fold (cantrips miss clean).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn enemy_burst_save_only(
    encounter: &mut EncounterInstance,
    caster_id: usize,
    point: Coordinate,
    radius: isize,
    save_ability: AbilityScoreType,
    dc: i32,
    dice: Dice,
    damage_type: DamageType,
    action_name: &str,
) -> (Vec<Box<dyn ApplicableSideEffect>>, Vec<(usize, bool)>) {
    burst_save_damage(
        encounter,
        caster_id,
        point,
        radius,
        save_ability,
        dc,
        dice,
        damage_type,
        action_name,
        BurstTargets::Enemy,
        SaveDamagePolicy::NoneOnSave,
    )
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn neutral_burst_save_only(
    encounter: &mut EncounterInstance,
    caster_id: usize,
    point: Coordinate,
    radius: isize,
    save_ability: AbilityScoreType,
    dc: i32,
    dice: Dice,
    damage_type: DamageType,
    action_name: &str,
) -> (Vec<Box<dyn ApplicableSideEffect>>, Vec<(usize, bool)>) {
    burst_save_damage(
        encounter,
        caster_id,
        point,
        radius,
        save_ability,
        dc,
        dice,
        damage_type,
        action_name,
        BurstTargets::Neutral,
        SaveDamagePolicy::NoneOnSave,
    )
}

/// Walk the `(target_id, save_passed)` vector returned by any of the
/// `*_burst_save_*` helpers and append an `ApplyCondition` side-effect
/// for every failed-save target. Centralizes the recurring shape:
///
/// ```ignore
/// for (tid, passed) in saves {
///     if !passed {
///         effects.push(Box::new(ApplyCondition { actor_id: tid, condition, timer }));
///     }
/// }
/// ```
///
/// Used by ~9 burst spells that layer a per-target failed-save rider
/// on top of the shared save-for-half damage roll (Tidal Wave Prone,
/// Wall of Stone Prone, Mental Prison Restrained, Vitriolic Sphere
/// drip, etc.). Returning the appended effects directly lets the
/// caller fold them into the main effect vec with a single `.extend()`
/// call, or — for cases that also need to record the (target, condition)
/// pair for concentration tracking — the `_with_conditions` variant
/// below.
fn push_condition_on_failed_save(
    effects: &mut Vec<Box<dyn ApplicableSideEffect>>,
    saves: &[(usize, bool)],
    condition: Condition,
    timer: ConditionTimer,
) {
    // Delegate to the concentration variant and discard the collected
    // pair vec — keeps both helpers driving off a single chokepoint so
    // adding (e.g.) a per-target log line or a new install side-effect
    // lands in one place.
    let _ = push_condition_on_failed_save_for_concentration(
        effects, saves, condition, timer,
    );
}

/// Concentration-tracking sibling of `push_condition_on_failed_save`.
/// Same loop body — append an `ApplyCondition` for each failed-save
/// target — but also collect the `(target_id, condition)` pairs into
/// a vec the caller can hand to `ConcentrationData::with_conditions`
/// so dropping concentration strips every install cleanly.
///
/// Used by burst spells where the failed-save rider is concentration-
/// bound (Wall of Light's Blinded cohort, Black Tentacles' Restrained
/// cohort, Caustic Brew's CausticBrewed DoT, etc.). Without this
/// helper each call site re-inlined the same loop body twice — once
/// to push the `ApplyCondition` effect and once to collect the
/// concentration pair — which the helper folds into one pass.
fn push_condition_on_failed_save_for_concentration(
    effects: &mut Vec<Box<dyn ApplicableSideEffect>>,
    saves: &[(usize, bool)],
    condition: Condition,
    timer: ConditionTimer,
) -> Vec<(usize, Condition)> {
    let mut conditions: Vec<(usize, Condition)> = Vec::new();
    for &(tid, passed) in saves {
        if !passed {
            effects.push(Box::new(ApplyCondition {
                actor_id: tid,
                condition,
                timer,
            }));
            conditions.push((tid, condition));
        }
    }
    conditions
}

/// Enemy-only AoE burst whose failed-save targets pick up a
/// concentration-anchored rider. Folds the recurring three-step recipe
/// used by Black Tentacles, Wall of Light, Caustic Brew, and Psychic
/// Scream into a single call:
///
///   1. resolve `enemy_burst_save_for_half` (shared damage roll, sorted
///      ids, allies spared, Heightened-aware saves);
///   2. layer `push_condition_on_failed_save_for_concentration` to tag
///      every failed-save target with `rider` for `rider_timer`;
///   3. push a `StartConcentration` anchored to the failed-save cohort
///      so dropping concentration strips every rider cleanly.
///
/// `outcome` picks between `HalfOnSave` (Wall of Light / Psychic Scream
/// — half damage on a passed save) and `NoneOnSave` (Caustic Brew —
/// zero damage on a passed save). The variant-pick used to live at the
/// call site as a separate `enemy_burst_save_for_half` /
/// `enemy_burst_save_only` choice; folding it here keeps the
/// concentration-burst pattern to a single chokepoint.
///
/// Returns the resolved effect list ready to fold into the action's
/// `side_effects` return — no further bookkeeping needed at the call
/// site.
#[allow(clippy::too_many_arguments)]
fn concentration_burst_with_rider(
    encounter: &mut EncounterInstance,
    caster_id: usize,
    point: Coordinate,
    radius: isize,
    save_ability: AbilityScoreType,
    dc: i32,
    dice: Dice,
    damage_type: DamageType,
    action_name: &str,
    spell_name: &'static str,
    rider: Condition,
    rider_timer: ConditionTimer,
    outcome: SaveDamagePolicy,
) -> Vec<Box<dyn ApplicableSideEffect>> {
    let (mut effects, saves) = burst_save_damage(
        encounter,
        caster_id,
        point,
        radius,
        save_ability,
        dc,
        dice,
        damage_type,
        action_name,
        BurstTargets::Enemy,
        outcome,
    );
    let conditions = push_condition_on_failed_save_for_concentration(
        &mut effects,
        &saves,
        rider,
        rider_timer,
    );
    effects.push(Box::new(StartConcentration {
        caster_id,
        data: ConcentrationData::with_conditions(spell_name, conditions),
    }));
    effects
}

/// Single-target save-or-nothing cantrip resolver — the cantrip sibling
/// of `save_for_half_damage`. Rolls the caster-aware save, then resolves
/// the damage through the shared post-save chokepoint, so the four
/// cantrips on this shape pick up two things they previously couldn't:
///
///   - **Potent Cantrip** (Evocation Wizard lv6), which upgrades the
///     `NoneOnSave` policy so a successful save still leaves half
///     standing. The old hand-rolled shape (`if save.passed() { return
///     Vec::new(); }`) had no room to express that — it discarded the
///     save outcome before any damage existed to halve.
///   - **Evasion** on the DEX-save cantrips, which only matters once
///     Potent Cantrip has lifted the effect into the half-on-save class.
///
/// Returns the queued `DealDamage` (empty on a fully-saved hit) plus the
/// save outcome, so callers that attach a rider on a failed save
/// (Vicious Mockery's `Mocked`) can branch without re-rolling.
///
/// Routes the save through `roll_save_against_caster` so the Sorcerer
/// Heightened Spell metamagic prime forces disadvantage (RAW), and the
/// damage through `roll_empowered_sum` so Empowered Spell and Empowered
/// Evocation both apply — matching every other damage-roll site in this
/// file.
#[allow(clippy::too_many_arguments)]
fn cantrip_save_damage(
    encounter: &mut EncounterInstance,
    caster_id: usize,
    target_id: usize,
    save_ability: AbilityScoreType,
    dc: i32,
    dice: Dice,
    damage_type: DamageType,
    action_name: &str,
) -> (Vec<Box<dyn ApplicableSideEffect>>, bool) {
    let save = encounter.roll_save_against_caster(target_id, save_ability, dc, caster_id);
    let passed = save.passed();
    let raw = encounter.roll_empowered_sum(caster_id, dice.count, dice.faces);
    let dmg = encounter.resolve_post_save_damage(
        caster_id,
        target_id,
        save_ability,
        SaveDamagePolicy::NoneOnSave,
        raw,
        passed,
    );
    if dmg == 0 {
        return (Vec::new(), passed);
    }
    encounter.log(format!(
        "  {}: {}({}) = {} {:?}",
        action_name, dice, raw, dmg, damage_type
    ));
    (
        vec![Box::new(DealDamage {
            actor_id: target_id,
            amount: dmg,
            damage_type,
        })],
        passed,
    )
}

/// Roll a damage burst against a target's saving throw, halving on
/// success. Returns `(damage, save_passed)` so callers can branch on
/// the save (e.g. attach a rider only on fail). The roll + save log
/// line is emitted with `action_name`; callers don't need to re-log
/// the breakdown. Folds the recurring `let raw = roll(); let dmg = if
/// save.passed() { raw / 2 } else { raw };` shape used by ~8 single-
/// target save-or-half spells (Hellish Rebuke, Mind Whip, Mind Spike,
/// Synaptic Static, Dawn, Disintegrate, Blight, Delayed Blast Fireball,
/// etc.) into one chokepoint.
///
/// Routes the save through `roll_save_against_caster` so the Sorcerer
/// Heightened Spell metamagic prime forces disadvantage on the first
/// save (RAW: "the target has disadvantage on the saving throw"). The
/// helper consumes the prime on its first call — subsequent saves in
/// the same cast fall through to the normal save path.
#[allow(clippy::too_many_arguments)]
pub(crate) fn save_for_half_damage(
    encounter: &mut EncounterInstance,
    caster_id: usize,
    target_id: usize,
    save_ability: AbilityScoreType,
    dc: i32,
    dice: Dice,
    damage_type: DamageType,
    action_name: &str,
) -> (u32, bool) {
    // Same caster-aware roll as the burst helper — see `burst_save_damage`.
    let raw = encounter.roll_empowered_sum(caster_id, dice.count, dice.faces);
    let save = encounter.roll_save_against_caster(target_id, save_ability, dc, caster_id);
    let dmg = if save.passed() { raw / 2 } else { raw };
    encounter.log(format!(
        "  {}: {}({}) = {} {:?} ({})",
        action_name,
        dice,
        raw,
        dmg,
        damage_type,
        if save.passed() { "save (half)" } else { "fail (full)" },
    ));
    (dmg, save.passed())
}

/// Build the two-step "apply a self-only condition + start concentration
/// tracking that condition" effect chain shared by every single-condition
/// self-buff concentration spell (Blur, Globe of Invulnerability, Crusader's
/// Mantle, Spirit Shroud, Bigby's Hand, Investiture of Flame, Wind Wall,
/// Warding Bond's caster-side leg, etc.). Each call site previously hand-
/// rolled the same two `Box::new(...)` entries with the condition repeated
/// across both — keeps the spell impls one logical line per cast.
///
/// Returns a `Vec<Box<dyn ApplicableSideEffect>>` ready to push into the
/// caller's effect list, or to use directly as the side_effects return.
fn self_concentration_buff_effects(
    caster_id: usize,
    spell_name: &'static str,
    condition: Condition,
    timer: ConditionTimer,
) -> Vec<Box<dyn ApplicableSideEffect>> {
    vec![
        Box::new(ApplyCondition {
            actor_id: caster_id,
            condition,
            timer,
        }),
        Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData::with_conditions(
                spell_name,
                vec![(caster_id, condition)],
            ),
        }),
    ]
}

/// Ally-aura concentration buff: every ally inside `radius` of the caster
/// (including the caster, who always sits at the center) picks up
/// `condition` with `timer`, and the caster takes concentration tracking
/// the full ally cohort so dropping concentration strips the flag from
/// every recipient cleanly.
///
/// Used by paladin auras (Aura of Life's DeathWarded, Aura of Purity's
/// Purified) — same shape as `self_concentration_buff_effects` but with
/// the radius-walk + multi-target concentration list. Caster missing →
/// empty effect list (cast is a no-op rather than panicking).
fn ally_aura_concentration_effects(
    encounter: &EncounterInstance,
    caster_id: usize,
    radius: isize,
    spell_name: &'static str,
    condition: Condition,
    timer: ConditionTimer,
) -> Vec<Box<dyn ApplicableSideEffect>> {
    let Some(caster) = encounter.actors.get(&caster_id) else {
        return Vec::new();
    };
    let center = caster.location();
    let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
    let mut conditions: Vec<(usize, Condition)> = Vec::new();
    for tid in encounter.ally_burst_targets(caster_id, center, radius) {
        effects.push(Box::new(ApplyCondition {
            actor_id: tid,
            condition,
            timer,
        }));
        conditions.push((tid, condition));
    }
    effects.push(Box::new(StartConcentration {
        caster_id,
        data: ConcentrationData::with_conditions(spell_name, conditions),
    }));
    effects
}

/// Spawn up to `max_count` instances of `template` on free anchors
/// adjacent to the caster's footprint, joining the caster's team. Each
/// successful spawn is logged through the standard "<spell>: a <name>
/// appears at <coord> (actor #<id>)" line. Returns the list of new
/// actor ids in spawn order — empty when the caster has vanished or the
/// arena has no legal adjacent slot.
///
/// Factor for the recurring summon-adjacent pattern: previously each
/// summoning spell (Animate Dead, Conjure Animals) re-inlined the
/// team-lookup → find_adjacent_spawn → instantiate_creature → log
/// chain. The shared helper keeps the spawn cap, search radius, and
/// failure-mode logging consistent across every summon spell — and
/// gives Conjure Elemental (and any future "summon N adjacent
/// creatures of size S") a one-line entry point.
///
/// `search_radius` widens the find_adjacent_spawn ring after the first
/// successful spawn (each new minion occupies its anchor, so later
/// spawns need a slightly wider search to find an open slot). The
/// caller passes the same value used by their RAW envelope — Conjure
/// Animals uses radius 3 for the 30 ft RAW range; Animate Dead uses
/// radius 2 for the touch-range envelope.
#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_adjacent_summons(
    encounter: &mut EncounterInstance,
    caster_id: usize,
    template: &'static crate::actors::actor_template::CreatureTemplate,
    size: crate::engine::types::Size,
    max_count: usize,
    search_radius: isize,
    base_instance_id: usize,
    spell_label: &str,
) -> Vec<usize> {
    let team = match encounter.actors.get(&caster_id) {
        Some(c) => c.team(),
        None => return Vec::new(),
    };
    let mut spawned: Vec<usize> = Vec::new();
    for _ in 0..max_count {
        let Some(anchor) = encounter.find_adjacent_spawn(caster_id, size, search_radius) else {
            break;
        };
        match encounter.instantiate_creature(template, anchor, team, base_instance_id + spawned.len())
        {
            Ok(new_id) => {
                encounter.log(format!(
                    "  {}: a {} appears at {} (actor #{})",
                    spell_label,
                    template.name.to_lowercase(),
                    anchor,
                    new_id
                ));
                spawned.push(new_id);
            }
            Err(e) => {
                encounter.log(format!("  {} failed: {}", spell_label, e));
                break;
            }
        }
    }
    // Mark the caster as having called for reinforcements. Read only by
    // the AI's summon rung, which declines while it is up — see
    // `Condition::Summoner` for why the marker lives on the caster and
    // why the concentration check the rung already does isn't enough
    // (Animate Dead holds no concentration and has no charge).
    if !spawned.is_empty()
        && let Some(caster) = encounter.actors.get_mut(&caster_id)
    {
        caster.add_condition(
            crate::conditions::Condition::Summoner,
            crate::conditions::ConditionTimer::Rounds(100),
        );
    }
    spawned
}

/// Build the concentration-tag effects for a freshly spawned summon
/// cohort. Each id picks up the `Conjured` marker (long timer — the
/// concentration anchor is what actually controls duration), and the
/// caster takes a single `StartConcentration` anchored to the full
/// list. Dropping concentration runs through the engine's
/// `Condition::Conjured` cleanup path, which despawns every conjured
/// minion via `despawn_actor` rather than just stripping the flag.
///
/// Caller is expected to skip this when `spawned` is empty — a no-op
/// summon shouldn't burn the caster's concentration slot.
fn conjured_summon_concentration_effects(
    caster_id: usize,
    spawned: &[usize],
    spell_name: &'static str,
) -> Vec<Box<dyn ApplicableSideEffect>> {
    let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
    let mut tags = Vec::new();
    for id in spawned {
        effects.push(Box::new(ApplyCondition {
            actor_id: *id,
            condition: Condition::Conjured,
            timer: ConditionTimer::Rounds(100),
        }));
        tags.push((*id, Condition::Conjured));
    }
    effects.push(Box::new(StartConcentration {
        caster_id,
        data: ConcentrationData::with_conditions(spell_name, tags),
    }));
    effects
}

/// Single-target "save-or-condition with concentration" install. Routes the
/// save through `roll_save_against_caster` so Heightened Spell metamagic
/// can force disadvantage; on pass returns an empty effect list (the
/// caller's `Action::execute` still pays the cost, matching RAW "spell
/// fizzles on save"). On fail returns the canonical two-entry list:
/// `ApplyCondition` on the target plus `StartConcentration` on the caster
/// anchored to the same (target, condition) pair, so dropping concentration
/// strips the condition cleanly via the existing cleanup hook.
///
/// Captures the recurring shape used by Phantasmal Force, Watery Sphere,
/// Eyebite, Flesh to Stone, etc. — every "make a save or pick up one
/// condition the caster sustains" lane collapses to a single helper call
/// instead of re-inlining the same `if save.passed() { ... } else { ... }`
/// block at every site. Empty `pass_log` / `fail_log` strings suppress the
/// matching log line so silent-flavor spells (Hold Person) can opt out.
#[allow(clippy::too_many_arguments)]
fn save_or_concentration_condition(
    encounter: &mut EncounterInstance,
    caster_id: usize,
    target_id: usize,
    save_ability: AbilityScoreType,
    dc: i32,
    spell_name: &'static str,
    condition: Condition,
    timer: ConditionTimer,
    pass_log: &str,
    fail_log: &str,
) -> Vec<Box<dyn ApplicableSideEffect>> {
    let save = encounter.roll_save_against_caster(target_id, save_ability, dc, caster_id);
    if save.passed() {
        if !pass_log.is_empty() {
            encounter.log(pass_log.to_string());
        }
        return Vec::new();
    }
    if !fail_log.is_empty() {
        encounter.log(fail_log.to_string());
    }
    vec![
        Box::new(ApplyCondition {
            actor_id: target_id,
            condition,
            timer,
        }),
        Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData::with_conditions(spell_name, vec![(target_id, condition)]),
        }),
    ]
}

/// Install the canonical "target is Charmed by the caster" pair: an
/// `ApplyCondition(Charmed, timer)` on the target plus a
/// `SetConditionLink(Condition::Charmed)` that anchors the engine's "can't attack your charmer"
/// gate in `action_template::validate_input`. Shared by Charm Person,
/// Charm Monster, and Geas.
///
/// A named wrapper over the general
/// `side_effects::install_condition_with_link`, which owns the
/// condition-to-link dispatch for all six linked conditions. Kept for the
/// call sites' legibility — `install_charmed_by(t, c, timer)` reads better
/// at a charm spell than the generic form.
///
/// Timer varies per spell (Charm Person / Charm Monster: `Rounds(10)`
/// ≈ 1 hour RAW capped to encounter-scale; Geas: `Rounds(100)` ≈ 10
/// minutes capped from 30 days). Centralizing the install lane keeps
/// the `Charmed` back-link / `Charmed` flag in lockstep — if a future
/// change adds e.g. a "charm aura" flag, it lands here once instead
/// of three times.
fn install_charmed_by(
    target_id: usize,
    caster_id: usize,
    timer: ConditionTimer,
) -> Vec<Box<dyn ApplicableSideEffect>> {
    crate::engine::side_effects::install_condition_with_link(
        Condition::Charmed,
        target_id,
        caster_id,
        timer,
    )
}

/// Install the canonical Dominate spell payload: Charmed by the caster
/// (via `install_charmed_by`) plus Dominated (the
/// `imposes_attacker_disadvantage` clause) for `Rounds(10)`, anchored
/// on the caster's concentration so dropping concentration strips both
/// marks in lockstep via the standard concentration-cleanup hook.
///
/// Replaces the hand-copied 4-Box trio (install_charmed_by + Dominated
/// ApplyCondition + StartConcentration with the two-condition cleanup
/// list) that lived inline in **Dominate Person**, **Dominate Beast**,
/// and **Dominate Monster**. Each of those spells differs only in their
/// slot level (5 / 4 / 8), reach gate, and target type filter — the
/// payload itself is identical RAW. A future Dominate-style spell drops
/// to a single call.
///
/// `spell_name` is the human-readable label used by the concentration
/// log and the dispel sweep — pass the spell's display name verbatim
/// ("Dominate Person", "Dominate Beast", "Dominate Monster") so the log
/// reads cleanly.
fn install_dominated_by(
    target_id: usize,
    caster_id: usize,
    spell_name: &'static str,
) -> Vec<Box<dyn ApplicableSideEffect>> {
    let mut effects =
        install_charmed_by(target_id, caster_id, ConditionTimer::Rounds(10));
    effects.push(Box::new(ApplyCondition {
        actor_id: target_id,
        condition: Condition::Dominated,
        timer: ConditionTimer::Rounds(10),
    }));
    effects.push(Box::new(StartConcentration {
        caster_id,
        data: ConcentrationData::with_conditions(
            spell_name,
            vec![
                (target_id, Condition::Charmed),
                (target_id, Condition::Dominated),
            ],
        ),
    }));
    effects
}

/// Enemy-only AoE burst whose only effect is "save or pick up a
/// concentration-anchored condition" — no damage. Mirror of
/// `concentration_burst_with_rider` but without the shared damage roll:
/// each enemy in the radius rolls the save up-front (via the Heightened-
/// aware `roll_save_against_caster` helper), and failures pick up `rider`
/// for `rider_timer`. The caster anchors concentration on the failed-save
/// cohort so dropping concentration strips every install at once via the
/// standard concentration-cleanup hook.
///
/// Used by save-or-condition burst spells where the load-bearing effect
/// is the rider, not damage (Wall of Sand's Restrained-on-fail, Compulsion's
/// Charmed-on-fail). Keeps the recurring shape — burst-loop, per-target
/// save, condition-on-fail, anchor concentration — to a single chokepoint
/// shared with the damage-burst variant.
#[allow(clippy::too_many_arguments)]
fn concentration_burst_condition_only(
    encounter: &mut EncounterInstance,
    caster_id: usize,
    point: Coordinate,
    radius: isize,
    save_ability: AbilityScoreType,
    dc: i32,
    action_name: &str,
    spell_name: &'static str,
    rider: Condition,
    rider_timer: ConditionTimer,
) -> Vec<Box<dyn ApplicableSideEffect>> {
    let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
    let mut conditions: Vec<(usize, Condition)> = Vec::new();
    for tid in encounter.enemy_burst_targets(caster_id, point, radius) {
        let save = encounter.roll_save_against_caster(tid, save_ability, dc, caster_id);
        if save.passed() {
            encounter.log(format!("  {}: target shrugs off the effect.", action_name));
            continue;
        }
        effects.push(Box::new(ApplyCondition {
            actor_id: tid,
            condition: rider,
            timer: rider_timer,
        }));
        conditions.push((tid, rider));
    }
    effects.push(Box::new(StartConcentration {
        caster_id,
        data: ConcentrationData::with_conditions(spell_name, conditions),
    }));
    effects
}

/// Sacred Flame — cleric cantrip. Range 60ft (24 tiles), DEX save vs the
/// caster's WIS-based spell save DC. On fail: 1d8 radiant. On success:
/// nothing (cantrips don't half-on-save). No spell slot consumed.
pub struct SacredFlame {}

impl Action for SacredFlame {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Evocation)
    }
    fn name(&self) -> &str {
        "sacred flame"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["sf", "flame"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }

    fn reach_tiles(&self) -> Option<isize> {
        Some(24)
    }

    fn requires_los(&self) -> bool {
        true
    }

    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Radiant]
    }

    // Cantrip — uses the default `cost()` (single Action, no spell slot).

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spellcasting_save_dc();
        let n = crate::engine::util::cantrip_dice_count(caster.level());

        // Shared save-or-nothing cantrip resolver: caster-aware save
        // (Heightened Spell), caster-aware damage roll (Empowered Spell
        // / Empowered Evocation), and the post-save chokepoint that
        // applies Potent Cantrip and Evasion.
        cantrip_save_damage(
            encounter,
            caster_id,
            target_id,
            AbilityScoreType::Dexterity,
            dc,
            Dice::new(n, 8),
            DamageType::Radiant,
            "sacred flame",
        )
        .0
    }
}

pub static SACRED_FLAME: LazyLock<SacredFlame> = LazyLock::new(|| SacredFlame {});

/// Single-target healing spell driven by configuration. Replaces the
/// per-spell impls of Healing Word and Cure Wounds — they only differ
/// in name, reach, action-economy slot, and dice. Heal amount = roll +
/// caster's `ability` modifier.
pub struct HealSpell {
    pub display_name: &'static str,
    pub aliases: &'static [&'static str],
    pub reach: isize,
    pub requires_los: bool,
    /// Action / BonusAction. Plus a level-`spell_slot_lvl` slot.
    pub action_cost: Resource,
    pub spell_slot_lvl: u32,
    pub heal_dice: Dice,
    /// Spellcasting ability whose modifier is added to the heal roll.
    pub ability: AbilityScoreType,
}

impl Action for HealSpell {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Evocation)
    }
    fn name(&self) -> &str {
        self.display_name
    }
    fn aliases(&self) -> Vec<&str> {
        self.aliases.to_vec()
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(self.reach)
    }
    fn requires_los(&self) -> bool {
        self.requires_los
    }
    fn is_harmful(&self) -> bool {
        false
    }

    fn is_heal(&self) -> bool {
        true
    }

    fn cost(
        &self,
        _encounter: &EncounterInstance,
        _caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        let lvl = crate::engine::action_overrides::cast_level(overrides, self.spell_slot_lvl);
        vec![self.action_cost, Resource::SpellSlot(lvl)]
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let ability_mod = caster.ability_modifier(self.ability);
        // Upcasting: +1 die per level above the spell's base slot level.
        let lvl = crate::engine::action_overrides::cast_level(overrides, self.spell_slot_lvl);
        let extra_dice = lvl - self.spell_slot_lvl;
        let dice = Dice::new(self.heal_dice.count + extra_dice, self.heal_dice.faces);
        // 5e Life Domain Cleric **Disciple of Life** — the leveled-heal
        // amplifier. Adds `2 + slot_level` on top of the roll for any
        // spell of level 1+; returns 0 for a non-Life caster so the
        // stock heal formula lands unchanged. Snapshot before the
        // (mutable) roll so the borrow checker is happy.
        let bonus = crate::actions::class_features::disciple_of_life_bonus(caster, lvl);
        // 5e Grave Domain Cleric **Circle of Mortality** — swap the
        // rolled dice for the max face-value when the target is at 0 HP.
        // Snapshot before the (mutable) roll so both immutable borrows
        // (`caster` from `.get(&caster_id)`, target from `.get(&target_id)`)
        // are dropped before `encounter.roll(&dice)` takes `&mut self`.
        let use_max = encounter
            .actors
            .get(&target_id)
            .is_some_and(|target| {
                crate::actions::class_features::should_use_max_heal_dice(caster, target)
            });
        let raw = if use_max {
            dice.max_roll() as i32
        } else {
            encounter.roll(&dice) as i32
        };
        let base = (raw + ability_mod).max(1) as u32;
        let amount = base + bonus;
        encounter.log(format!(
            "  {}: {}({}){}{:+}{} = {} HP",
            self.display_name,
            dice,
            raw,
            crate::actions::class_features::circle_of_mortality_log_suffix(use_max),
            ability_mod,
            crate::actions::class_features::disciple_of_life_log_suffix(bonus),
            amount
        ));
        vec![Box::new(Heal {
            actor_id: target_id,
            amount,
        })]
    }
}

/// Healing Word — bonus-action level-1 heal at 60ft (24 tiles).
pub static HEALING_WORD: HealSpell = HealSpell {
    display_name: "healing word",
    aliases: &["hw", "heal"],
    reach: 24,
    requires_los: true,
    action_cost: Resource::BonusAction,
    spell_slot_lvl: 1,
    heal_dice: Dice::new(1, 4),
    ability: AbilityScoreType::Wisdom,
};

/// Sacred Burst — generic cleric AoE cantrip. Pick a tile within 30ft;
/// every actor (friend or foe) whose footprint touches the burst takes
/// 2d6 radiant on a failed DEX save vs the caster's WIS-based DC, half
/// on success. Damage is rolled once and shared. Requires LOS to the
/// burst origin (not to each target).
pub struct SacredBurst {}

impl Action for SacredBurst {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Evocation)
    }
    fn name(&self) -> &str {
        "sacred burst"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["sb", "burst"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 3 }
    }

    fn reach_tiles(&self) -> Option<isize> {
        Some(12)
    }

    fn requires_los(&self) -> bool {
        true
    }

    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Radiant]
    }

    // Cantrip — uses the default `cost()` (single Action, no spell slot).

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spell_save_dc(AbilityScoreType::Wisdom);
        let radius: isize = match self.targeting_schema() {
            TargetingSchema::Burst { radius } => radius,
            _ => return Vec::new(),
        };

        let n = crate::engine::util::cantrip_dice_count(caster.level());
        let die = Dice::new(n + 1, 6);
        // Roll through the shared caster-aware chokepoint rather than
        // `roll` directly — Empowered Spell, Empowered Evocation and
        // Potent Spellcasting all live there, and a bare `roll` skips
        // all three silently.
        let raw = encounter.roll_empowered_sum(caster_id, die.count, die.faces);
        encounter.log(format!(
            "  sacred burst: {}({}) = {} radiant area",
            die, raw, raw
        ));
        crate::actions::action_template::resolve_burst_save_damage(
            encounter,
            caster_id,
            point,
            radius,
            AbilityScoreType::Dexterity,
            dc,
            raw,
            DamageType::Radiant,
        )
    }
}

pub static SACRED_BURST: LazyLock<SacredBurst> = LazyLock::new(|| SacredBurst {});

/// Hold Person — 5e-flavored single-target paralysis. WIS save vs the
/// caster's WIS-based DC; on fail, target is Stunned for 10 rounds.
/// Stunned models paralysis: blocks actions/movement, auto-fails STR/DEX
/// saves, and attackers roll with advantage. Missing: the 5-foot crit rule
/// (melee hits auto-crit vs paralyzed targets). Concentration: when the
/// caster takes damage and fails a CON save (or hits 0 HP), the spell
/// drops and Stunned clears immediately.
pub struct HoldPerson {}

impl Action for HoldPerson {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Enchantment)
    }
    fn name(&self) -> &str {
        "hold person"
    }

    fn deals_damage(&self) -> bool {
        // Control, not damage. The AI's focus-fire lane scores by who
        // drops soonest, so an action that claims damage and deals none
        // gets picked over the attack that would have.
        false
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["hp", "hold"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }

    fn reach_tiles(&self) -> Option<isize> {
        Some(24)
    }

    fn requires_los(&self) -> bool {
        true
    }

    fn cost(
        &self,
        _encounter: &EncounterInstance,
        _caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        // Level-2 leveled spell — Action + a level-2 (or upcast) spell slot.
        action_and_slot(crate::engine::action_overrides::cast_level(overrides, 2))
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.best_spell_save_dc([
            // Every full-caster list carries Hold Person, so the DC has
            // to follow the caster rather than a single class's stat:
            // WIS for the cleric / druid / Four Elements monk, INT for
            // the wizard, CHA for the bard / sorcerer / warlock. WIS
            // leads the candidate order so the divine carriers this
            // spell shipped for keep their anchor on a tie.
            AbilityScoreType::Wisdom,
            AbilityScoreType::Intelligence,
            AbilityScoreType::Charisma,
        ]);
        // Apply Stunned for up to 10 rounds, plus install concentration
        // tracking so the round-end WIS save (shared `ROUND_END_SAVES`
        // table) can release the target. Helper routes the cast through
        // `roll_save_against_caster` so Heightened Spell metamagic still
        // fires on this single save-or-suck roll. Empty logs match the
        // legacy silent flavor.
        save_or_concentration_condition(
            encounter,
            caster_id,
            target_id,
            AbilityScoreType::Wisdom,
            dc,
            "Hold Person",
            Condition::Stunned,
            ConditionTimer::Rounds(10),
            "",
            "",
        )
    }
}

pub static HOLD_PERSON: LazyLock<HoldPerson> = LazyLock::new(|| HoldPerson {});

/// Cure Wounds — 5e level-1 cleric/druid/bard spell. Touch range, no
/// save: target regains 1d8 + caster's WIS modifier HP (+1d8 per
/// upcast level via the shared HealSpell upcasting rule). Compared to
/// Healing Word: Cure Wounds is a full Action (not bonus action) but
/// heals more on average. Both consume a level-1 slot.
///
/// Implemented via the shared `HealSpell` chassis — same code path as
/// Healing Word, differing only in the (reach, action_cost, dice)
/// tuple. Disciple of Life amplification, upcasting scaling, and log
/// formatting all live on the `HealSpell::side_effects` path so a
/// future healing-lane change (e.g. RAW-tightening the Disciple bonus
/// to prepared Life-domain spells only) lands in one place.
///
/// `LazyLock<HealSpell>` (rather than `pub static ... = HealSpell {}`)
/// so every existing `&*CURE_WOUNDS` call site — creature templates
/// registering CURE_WOUNDS onto their action pool, engine tests
/// building an ActionExecutionInfo against it — continues to compile
/// without a mass rename.
pub static CURE_WOUNDS: LazyLock<HealSpell> = LazyLock::new(|| HealSpell {
    display_name: "cure wounds",
    aliases: &["cw", "cure"],
    reach: 1,
    requires_los: true,
    action_cost: Resource::Action,
    spell_slot_lvl: 1,
    heal_dice: Dice::new(1, 8),
    ability: AbilityScoreType::Wisdom,
});

/// Fire Bolt — 5e wizard cantrip. Ranged spell attack: d20 + caster's
/// INT modifier vs target AC. On hit: 1d10 fire damage. No save (it's
/// an attack roll, not a save spell). Crits double the damage dice.
pub struct FireBolt {}

impl Action for FireBolt {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Evocation)
    }
    fn name(&self) -> &str {
        "fire bolt"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["fb", "bolt"]
    }

    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Fire]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }

    fn reach_tiles(&self) -> Option<isize> {
        // 120 ft range — well past any current map.
        Some(48)
    }

    fn requires_los(&self) -> bool {
        true
    }

    // Cantrip — uses the default `cost()` (single Action, no spell slot).

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::attack::{AttackParams, resolve_attack};

        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let attack_bonus = caster.spellcasting_attack_modifier();
        let dice_count = crate::engine::util::cantrip_dice_count(caster.level());
        resolve_attack(
            encounter,
            AttackParams {
                caster_id,
                target_id,
                action_name: self.name(),
                attack_bonus,
                damage_dice: Dice::new(dice_count, 10),
                damage_bonus: 0,
                damage_type: DamageType::Fire,
                is_melee: false,
                long_range: None,
                is_spell: true,
            },
        )
    }
}

pub static FIRE_BOLT: LazyLock<FireBolt> = LazyLock::new(|| FireBolt {});

/// Bless — 5e level-1 concentration spell. For up to 3 targets, each
/// gets +1d4 to attack rolls and saving throws while the spell lasts
/// (we approximate the d4 as a flat +2 — average roll on a d4 = 2.5,
/// rounded to keep math integer). Concentration; drops cleanly via the
/// existing concentration cleanup hook.
///
/// Schema is SingleActor for simplicity — the AI / picker can cast it
/// once per ally per round. Models the multi-target version on top of
/// the single-target schema by buffing the target chosen.
pub struct Bless {}

impl Action for Bless {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Enchantment)
    }
    fn name(&self) -> &str {
        "bless"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["bl"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        // Custom: accepts no args (auto-target all nearby allies) or
        // a single SingleActor (just bless that one ally + caster).
        TargetingSchema::Custom
    }

    fn is_harmful(&self) -> bool {
        false
    }

    fn cost(
        &self,
        _encounter: &EncounterInstance,
        _caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(1)
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::side_effects::{AdjustAttackBuff, AdjustSaveBuff};
        use crate::engine::util::{footprint_chebyshev, get_tiles_from_size};

        // 5e Bless: pick targets in range. If `target_ids` was supplied,
        // bless those (typical UI flow). Otherwise auto-pick the caster
        // and every combat-active ally within 12 tiles.
        let mut targets: Vec<usize> = match target_ids {
            Some(v) if !v.is_empty() => v.clone(),
            _ => {
                let Some(caster) = encounter.actors.get(&caster_id) else {
                    return Vec::new();
                };
                let caster_team = caster.team();
                let caster_loc = caster.location();
                let caster_size = get_tiles_from_size(caster.size());
                const REACH: isize = 12;
                let mut out = Vec::new();
                let mut ids: Vec<usize> = encounter.actors.keys().copied().collect();
                ids.sort_unstable();
                for tid in ids {
                    let Some(target) = encounter.actors.get(&tid) else {
                        continue;
                    };
                    if target.team() != caster_team || !target.is_combat_active() {
                        continue;
                    }
                    let dist = footprint_chebyshev(
                        target.location(),
                        get_tiles_from_size(target.size()),
                        caster_loc,
                        caster_size,
                    );
                    if dist > REACH {
                        continue;
                    }
                    out.push(tid);
                }
                out
            }
        };
        // Always include the caster — Bless can target the caster too.
        if !targets.contains(&caster_id) {
            targets.insert(0, caster_id);
        }
        if targets.is_empty() {
            return Vec::new();
        }
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        let mut conditions = Vec::new();
        let mut attack_buffs = Vec::new();
        let mut save_buffs = Vec::new();
        for tid in &targets {
            effects.push(Box::new(AdjustAttackBuff {
                actor_id: *tid,
                delta: 2,
            }));
            effects.push(Box::new(AdjustSaveBuff {
                actor_id: *tid,
                delta: 2,
            }));
            effects.push(Box::new(ApplyCondition {
                actor_id: *tid,
                condition: Condition::Blessed,
                timer: ConditionTimer::Rounds(10),
            }));
            conditions.push((*tid, Condition::Blessed));
            attack_buffs.push((*tid, 2));
            save_buffs.push((*tid, 2));
        }
        effects.push(Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData::with_conditions("Bless", conditions)
                .with_attack_buffs(attack_buffs)
                .with_save_buffs(save_buffs),
        }));
        effects
    }
}

pub static BLESS: LazyLock<Bless> = LazyLock::new(|| Bless {});

/// Burning Hands — 5e level-1 evocation. 15-foot cone (we approximate as
/// a 3-tile burst centered on the target tile, since cones aren't yet
/// modeled). Every actor in the burst takes 3d6 fire on a failed DEX
/// save, half on success. Consumes a level-1 slot.
pub struct BurningHands {}

impl Action for BurningHands {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Evocation)
    }
    fn name(&self) -> &str {
        "burning hands"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["bh", "hands"]
    }

    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Fire]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 2 }
    }

    fn reach_tiles(&self) -> Option<isize> {
        // 15 ft cone — short range. We treat the burst origin as the
        // far edge of the cone.
        Some(6)
    }

    fn requires_los(&self) -> bool {
        true
    }

    fn cost(
        &self,
        _encounter: &EncounterInstance,
        _caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(1)
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.best_spell_save_dc([
            // Arcane / primal / pact list all reach this spell, and the
            // 5e save DC is the *caster's* spellcasting ability — the
            // wizard's INT, the sorcerer's and warlock's CHA, the
            // druid's and the Four Elements monk's WIS. Anchoring on
            // INT alone quietly handed every non-wizard carrier a DC
            // computed off a stat their class never invests in.
            AbilityScoreType::Intelligence,
            AbilityScoreType::Charisma,
            AbilityScoreType::Wisdom,
        ]);
        let radius: isize = match self.targeting_schema() {
            TargetingSchema::Burst { radius } => radius,
            _ => return Vec::new(),
        };

        // Empowered Spell metamagic — sorcerer can reroll low dice on
        // the burning-hands pool. Same hook as Fireball / Lightning Bolt.
        let raw = encounter.roll_empowered_sum(caster_id, 3, 6);
        encounter.log(format!("  burning hands: 3d6({}) = {} fire area", raw, raw));
        crate::actions::action_template::resolve_burst_save_damage(
            encounter,
            caster_id,
            point,
            radius,
            AbilityScoreType::Dexterity,
            dc,
            raw,
            DamageType::Fire,
        )
    }
}

pub static BURNING_HANDS: LazyLock<BurningHands> = LazyLock::new(|| BurningHands {});

/// Magic Missile — 5e level-1 evocation. Three darts, each auto-hitting
/// (no attack roll, no save) for 1d4+1 force damage. We model it as a
/// single-target spell that fires all three darts at the chosen target;
/// the multi-target split-fire variant is an extension.
pub struct MagicMissile {}

impl Action for MagicMissile {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Evocation)
    }
    fn name(&self) -> &str {
        "magic missile"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["mm", "missile"]
    }

    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Force]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }

    fn reach_tiles(&self) -> Option<isize> {
        Some(48)
    }

    fn requires_los(&self) -> bool {
        true
    }

    fn cost(
        &self,
        _encounter: &EncounterInstance,
        _caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(crate::engine::action_overrides::cast_level(overrides, 1))
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        // 5e upcasting: 3 darts at level 1, +1 dart per level above 1.
        let lvl = crate::engine::action_overrides::cast_level(overrides, 1);
        let n_darts = 3 + (lvl - 1);
        // Roll all dart dice in one call so Empowered Spell metamagic
        // can reroll across the whole spell's pool (RAW: the spell is
        // the unit, not each dart). Non-empowered casters get an
        // identical roll sequence to per-dart `roll(&Dice::new(1, 4))`.
        let dart_rolls = encounter.roll_empowered(caster_id, n_darts, 4);
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> =
            Vec::with_capacity(n_darts as usize);
        let mut dart_values: Vec<u32> = Vec::with_capacity(n_darts as usize);
        for raw in dart_rolls {
            let dmg = raw + 1;
            dart_values.push(dmg);
            effects.push(Box::new(DealDamage {
                actor_id: target_id,
                amount: dmg,
                damage_type: DamageType::Force,
            }));
        }
        let dart_str: Vec<String> = dart_values.iter().map(|v| v.to_string()).collect();
        encounter.log(format!(
            "  magic missile: {} darts [{}] force",
            n_darts,
            dart_str.join(", ")
        ));
        effects
    }
}

pub static MAGIC_MISSILE: LazyLock<MagicMissile> = LazyLock::new(|| MagicMissile {});

/// Shield of Faith — concentration buff that grants +2 AC to a willing
/// target for the spell's duration (10 rounds). Costs an Action and a
/// level-1 spell slot. Tracked as the `ShieldOfFaith` condition; the
/// engine reads it from `armor_class()` at attack-resolution time.
pub struct ShieldOfFaith {}

impl Action for ShieldOfFaith {
    fn name(&self) -> &str {
        "shield of faith"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["sof", "shield-of-faith"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(1)
    }
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        vec![
            Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::ShieldOfFaith,
                timer: ConditionTimer::Rounds(10),
            }),
            Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions(
                    "Shield of Faith",
                    vec![(target_id, Condition::ShieldOfFaith)],
                ),
            }),
        ]
    }
}

pub static SHIELD_OF_FAITH: LazyLock<ShieldOfFaith> = LazyLock::new(|| ShieldOfFaith {});

/// Cause Fear — single-target WIS save vs spell DC; on fail, target is
/// Frightened for up to 3 rounds (concentration). Costs an Action and a
/// level-1 spell slot.
pub struct CauseFear {}

impl Action for CauseFear {
    fn name(&self) -> &str {
        "cause fear"
    }

    fn deals_damage(&self) -> bool {
        // Control, not damage. The AI's focus-fire lane scores by who
        // drops soonest, so an action that claims damage and deals none
        // gets picked over the attack that would have.
        false
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["cf", "fear"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(1)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spellcasting_save_dc();
        // Caster-aware save so Heightened Spell can force disadvantage
        // on this single save-or-suck roll. RAW: the prime affects the
        // first save against the spell, which here is the only save.
        let save =
            encounter.roll_save_against_caster(target_id, AbilityScoreType::Wisdom, dc, caster_id);
        if save.passed() {
            return Vec::new();
        }
        vec![
            Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Frightened,
                timer: ConditionTimer::Rounds(3),
            }),
            Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions(
                    "Cause Fear",
                    vec![(target_id, Condition::Frightened)],
                ),
            }),
        ]
    }
}

pub static CAUSE_FEAR: LazyLock<CauseFear> = LazyLock::new(|| CauseFear {});

/// Guiding Bolt — level-1 ranged spell attack. 4d6 radiant on hit; the
/// next attack against the target before the end of the caster's next
/// turn has advantage. Demonstrates timed-rider conditions.
pub struct GuidingBolt {}

impl Action for GuidingBolt {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Evocation)
    }
    fn name(&self) -> &str {
        "guiding bolt"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["gb", "bolt"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(48)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Radiant]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(crate::engine::action_overrides::cast_level(overrides, 1))
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let attack_mod = caster.spellcasting_attack_modifier();
        let lvl = crate::engine::action_overrides::cast_level(overrides, 1);
        let dice = 4 + (lvl - 1);
        let mut effects = spell_attack(
            encounter,
            caster_id,
            target_id,
            self.name(),
            attack_mod,
            Dice::new(dice, 6),
            DamageType::Radiant,
            false,
        );
        if !effects.is_empty() {
            effects.push(Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::GuidingBoltLit,
                timer: ConditionTimer::Rounds(1),
            }));
        }
        effects
    }
}

pub static GUIDING_BOLT: LazyLock<GuidingBolt> = LazyLock::new(|| GuidingBolt {});

/// Web — level-2 conjuration. AoE 4-tile burst, 1-minute concentration.
/// Targets in the burst make a DEX save; fail = Restrained, success =
/// no effect. We don't model the "difficult terrain" clause yet (no
/// terrain-mod system); the Restrained condition does the heavy lifting.
pub struct Web {}

impl Action for Web {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Conjuration)
    }
    fn name(&self) -> &str {
        "web"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["wb"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 4 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(2)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spellcasting_save_dc();
        let radius = match self.targeting_schema() {
            TargetingSchema::Burst { radius } => radius,
            _ => return Vec::new(),
        };
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        let mut conditions = Vec::new();
        // Web is friend-or-foe agnostic — every creature in the burst
        // (except the caster) makes a DEX save. `neutral_burst_targets`
        // captures that policy in one chokepoint instead of an ad-hoc
        // loop over `actors.keys()`.
        for tid in encounter.neutral_burst_targets(caster_id, point, radius) {
            let save = encounter.roll_save_against_caster(tid, AbilityScoreType::Dexterity, dc, caster_id);
            if !save.passed() {
                effects.push(Box::new(ApplyCondition {
                    actor_id: tid,
                    condition: Condition::Restrained,
                    timer: ConditionTimer::Rounds(10),
                }));
                conditions.push((tid, Condition::Restrained));
            }
        }
        if !conditions.is_empty() {
            effects.push(Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions("Web", conditions),
            }));
        }
        effects
    }
}

pub static WEB: LazyLock<Web> = LazyLock::new(|| Web {});

/// False Life — level-1 necromancy. Self-target; gain 1d4+4 temp HP.
/// Doesn't require concentration (it's a flat buff). Cleared by long
/// rest with the rest of temp HP.
pub struct FalseLife {}

impl Action for FalseLife {
    fn name(&self) -> &str {
        "false life"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["fl", "false-life"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(1)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let raw = encounter.roll(&Dice::new(1, 4));
        let amount = raw + 4;
        encounter.log(format!(
            "  false life: 1d4({})+4 = {} temp HP",
            raw, amount
        ));
        vec![Box::new(GainTempHp {
            actor_id: caster_id,
            amount,
        })]
    }
}

pub static FALSE_LIFE: LazyLock<FalseLife> = LazyLock::new(|| FalseLife {});

/// Blindness/Deafness — level-2 necromancy. CON save vs spell DC; on
/// fail target is Blinded for 10 rounds (1 minute). Doesn't require
/// concentration in 5e — the duration runs without sustain.
pub struct Blindness {}

impl Action for Blindness {
    fn name(&self) -> &str {
        "blindness"
    }

    fn deals_damage(&self) -> bool {
        // Control, not damage. The AI's focus-fire lane scores by who
        // drops soonest, so an action that claims damage and deals none
        // gets picked over the attack that would have.
        false
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["blind"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(2)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spellcasting_save_dc();
        // Route through the caster-aware save helper so Heightened Spell
        // metamagic forces disadvantage on the save (RAW). The prime is
        // consumed on the first save resolved through this site.
        let save = encounter
            .roll_save_against_caster(target_id, AbilityScoreType::Constitution, dc, caster_id);
        if save.passed() {
            return Vec::new();
        }
        vec![Box::new(ApplyCondition {
            actor_id: target_id,
            condition: Condition::Blinded,
            timer: ConditionTimer::Rounds(10),
        })]
    }
}

pub static BLINDNESS: LazyLock<Blindness> = LazyLock::new(|| Blindness {});

/// Shield — level-1 abjuration reaction (we model as a normal Action
/// for pipeline simplicity since Reaction-cost actions are also slotted
/// through the Resource enum). Adds the Shielded condition (+5 AC)
/// until the start of your next turn.
pub struct Shield {}

impl Action for Shield {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Abjuration)
    }
    fn name(&self) -> &str {
        "shield"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["sh-spell"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        vec![Resource::Reaction, Resource::SpellSlot(1)]
    }
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        vec![Box::new(ApplyCondition {
            actor_id: caster_id,
            condition: Condition::Shielded,
            timer: ConditionTimer::UntilStartOfNextTurn,
        })]
    }
}

pub static SHIELD: LazyLock<Shield> = LazyLock::new(|| Shield {});

/// Faerie Fire — level-1 evocation, concentration. Pick a tile; every
/// actor in a 4-tile burst makes a DEX save. On fail, the target is
/// Outlined: attacks against them have advantage and they can't benefit
/// from invisibility. No damage. Helpful for breaking up clumped enemies
/// or marking a tough single target. Caster takes the WIS-based DC.
pub struct FaerieFire {}

impl Action for FaerieFire {
    fn name(&self) -> &str {
        "faerie fire"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ff", "faerie"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 4 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(1)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spellcasting_save_dc();
        let radius = match self.targeting_schema() {
            TargetingSchema::Burst { radius } => radius,
            _ => return Vec::new(),
        };
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        let mut conditions = Vec::new();
        for tid in encounter.neutral_burst_targets(caster_id, point, radius) {
            let save = encounter.roll_save_against_caster(tid, AbilityScoreType::Dexterity, dc, caster_id);
            if !save.passed() {
                effects.push(Box::new(ApplyCondition {
                    actor_id: tid,
                    condition: Condition::Outlined,
                    timer: ConditionTimer::Rounds(10),
                }));
                conditions.push((tid, Condition::Outlined));
            }
        }
        if !conditions.is_empty() {
            effects.push(Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions("Faerie Fire", conditions),
            }));
        }
        effects
    }
}

pub static FAERIE_FIRE: LazyLock<FaerieFire> = LazyLock::new(|| FaerieFire {});

/// Ray of Frost — wizard cantrip. Ranged spell attack: d20 + INT vs AC.
/// On hit: 1d8 cold damage AND the target's speed is reduced by 10ft
/// until the start of the caster's next turn (we approximate with a
/// 1-round timer on a save-buff penalty rather than a full speed
/// override; the effect is small enough that the simpler model is fine).
pub struct RayOfFrost {}

impl Action for RayOfFrost {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Evocation)
    }
    fn name(&self) -> &str {
        "ray of frost"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["rof", "frost"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60 ft range.
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Cold]
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::attack::{AttackParams, resolve_attack};
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let attack_bonus = caster.spellcasting_attack_modifier();
        let n = crate::engine::util::cantrip_dice_count(caster.level());
        resolve_attack(
            encounter,
            AttackParams {
                caster_id,
                target_id,
                action_name: self.name(),
                attack_bonus,
                damage_dice: Dice::new(n, 8),
                damage_bonus: 0,
                damage_type: DamageType::Cold,
                is_melee: false,
                long_range: None,
                is_spell: true,
            },
        )
    }
}

pub static RAY_OF_FROST: LazyLock<RayOfFrost> = LazyLock::new(|| RayOfFrost {});

/// Thunderwave — level-1 evocation. 15-ft cube around the caster (we
/// approximate with a 2-tile burst centered on the caster's tile). Each
/// creature in the burst makes a CON save vs the caster's INT-DC; on
/// fail, takes 2d8 thunder and is pushed 10 ft (4 tiles) away from the
/// caster. On success, half damage and no push. The push routes through
/// the standard `PushActor` side-effect so wall / occupancy blocking is
/// honored — a creature pinned to a wall takes the damage but doesn't
/// budge.
pub struct Thunderwave {}

impl Action for Thunderwave {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Evocation)
    }
    fn name(&self) -> &str {
        "thunderwave"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["tw", "thunder"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Thunder]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(1)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::side_effects::PushActor;

        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.best_spell_save_dc([
            // Arcane / primal / pact list all reach this spell, and the
            // 5e save DC is the *caster's* spellcasting ability — the
            // wizard's INT, the sorcerer's and warlock's CHA, the
            // druid's and the Four Elements monk's WIS. Anchoring on
            // INT alone quietly handed every non-wizard carrier a DC
            // computed off a stat their class never invests in.
            AbilityScoreType::Intelligence,
            AbilityScoreType::Charisma,
            AbilityScoreType::Wisdom,
        ]);
        let center = caster.location();
        const RADIUS: isize = 2;
        // 10 ft = 4 tiles on this 2.5ft grid. RAW Thunderwave push.
        const PUSH_TILES: u32 = 4;
        // Friend-or-foe burst — Thunderwave's cube doesn't discriminate.
        // The save-for-half helper handles the per-target CON save +
        // shared roll log; we layer the push rider on top of the
        // returned save outcomes.
        let (mut effects, saves) = neutral_burst_save_for_half(
            encounter,
            caster_id,
            center,
            RADIUS,
            AbilityScoreType::Constitution,
            dc,
            Dice::new(2, 8),
            DamageType::Thunder,
            "thunderwave",
        );
        // 5e RAW: push only fires on a failed save. The PushActor
        // helper handles wall / occupancy blocking — a target pinned
        // to a wall just doesn't move.
        for (tid, passed) in saves {
            if !passed {
                effects.push(Box::new(PushActor {
                    actor_id: tid,
                    from: center,
                    max_tiles: PUSH_TILES,
                }));
            }
        }
        effects
    }
}

pub static THUNDERWAVE: LazyLock<Thunderwave> = LazyLock::new(|| Thunderwave {});

/// Misty Step — level-2 conjuration. Bonus action; teleport the caster
/// up to 30 ft (12 tiles) to an unoccupied spot you can see. No save,
/// no concentration, no damage. Pure repositioning.
pub struct MistyStep {}

impl Action for MistyStep {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Conjuration)
    }
    fn name(&self) -> &str {
        "misty step"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ms", "step"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SinglePoint
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 30 ft = 12 tiles.
        Some(12)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        bonus_action_and_slot(2)
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // Destination must be a legal landing spot for this caster's full
        // footprint — same constraint as Move's pathing, minus the budget
        // check (Misty Step bypasses movement entirely).
        let Some(point) = first_target_location(target_locations) else {
            return false;
        };
        encounter.can_move_to(caster_id, point)
    }
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        // Misty Step is RAW explicitly OA-free since the caster doesn't
        // traverse intervening tiles. TeleportActor bypasses the per-step
        // OA dispatch that MoveActor uses.
        vec![Box::new(crate::engine::side_effects::TeleportActor {
            actor_id: caster_id,
            dest: point,
        })]
    }
}

pub static MISTY_STEP: LazyLock<MistyStep> = LazyLock::new(|| MistyStep {});

/// Bane — level-1 enchantment, concentration. Symmetric counterpart to
/// Bless: enemy targets in range each make a CHA save vs the caster's
/// spell save DC. On fail, they're Baned for 10 rounds: -1d4 to attacks
/// and saves (we model as -2 flat via the existing condition pipeline).
/// Concentration; all stacked debuffs drop when the caster's
/// concentration drops.
pub struct Bane {}

impl Action for Bane {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Enchantment)
    }
    fn name(&self) -> &str {
        "bane"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["bn"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        // Single-target form for simplicity. RAW lets the caster pick up
        // to 3 creatures, but the picker UI doesn't have a multi-actor
        // schema yet — start with one.
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 30 ft = 12 tiles.
        Some(12)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(1)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spell_save_dc(AbilityScoreType::Wisdom);
        let save = encounter.roll_save_against_caster(target_id, AbilityScoreType::Charisma, dc, caster_id);
        if save.passed() {
            return Vec::new();
        }
        vec![
            Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Baned,
                timer: ConditionTimer::Rounds(10),
            }),
            Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions(
                    "Bane",
                    vec![(target_id, Condition::Baned)],
                ),
            }),
        ]
    }
}

pub static BANE: LazyLock<Bane> = LazyLock::new(|| Bane {});

/// Mage Armor — level-1 abjuration. Self-only; while active, the caster's
/// AC becomes 13 + DEX modifier (we model as a floor; existing AC wins
/// if higher). 8-hour duration; we use a generous 100-round timer so it
/// sticks for the whole encounter. No concentration.
pub struct MageArmor {}

impl Action for MageArmor {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Abjuration)
    }
    fn name(&self) -> &str {
        "mage armor"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ma", "mage-armor"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(1)
    }
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        vec![Box::new(ApplyCondition {
            actor_id: caster_id,
            condition: Condition::MageArmored,
            // 8 hours = effectively permanent for any single encounter.
            timer: ConditionTimer::Rounds(100),
        })]
    }
}

pub static MAGE_ARMOR: LazyLock<MageArmor> = LazyLock::new(|| MageArmor {});

/// Aid — level-2 abjuration. Bumps each target's max HP by 5 and
/// restores 5 HP to each. We collapse the multi-target form into a
/// single ally pick for now (the picker UI doesn't yet support multi-
/// actor selection). The HP boost is permanent for the encounter
/// (8-hour 5e duration, longer than any combat).
pub struct Aid {}

impl Action for Aid {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Abjuration)
    }
    fn name(&self) -> &str {
        "aid"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["a"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 30 ft = 12 tiles.
        Some(12)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn is_heal(&self) -> bool {
        true
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(2)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        // Aid only affects allies — reject hostile targets at side-effect
        // time as a safety net (the harmful=false flag should already
        // steer the picker UI here). Routes through the shared
        // `first_ally_target_id` helper so the "extract id + ally check"
        // gate is shared with Longstrider / Enhance Ability.
        let Some(target_id) = first_ally_target_id(encounter, caster_id, target_ids) else {
            return Vec::new();
        };
        // RAW Aid: "the target's hit point maximum and current hit
        // points increase by 5." Our `bump_max_hp` raises the base by
        // `delta` and the current HP by the same amount, capped at the
        // new max — no separate Heal needed.
        if let Some(target) = encounter.actors.get_mut(&target_id) {
            target.bump_max_hp(5);
        }
        encounter.log("  aid: +5 max HP, +5 HP".to_string());
        Vec::new()
    }
}

pub static AID: LazyLock<Aid> = LazyLock::new(|| Aid {});

/// Acid Splash — wizard cantrip. Pick a target; that creature (and one
/// adjacent creature) makes a DEX save vs spell DC. On fail: 1d6 acid.
/// Cantrips don't half-on-save. We use a tiny burst (radius 1) at the
/// target's tile to model the splash to one neighbor.
pub struct AcidSplash {}

impl Action for AcidSplash {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Conjuration)
    }
    fn name(&self) -> &str {
        "acid splash"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["as-spell", "splash"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        // Single-tile burst — simulates the "pick a creature; an
        // adjacent creature is also affected" wording with a 1-tile
        // splash radius.
        TargetingSchema::Burst { radius: 1 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60 ft = 24 tiles.
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Acid]
    }
    // Cantrip — uses the default `cost()` (single Action, no spell slot).
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spellcasting_save_dc();
        let n = crate::engine::util::cantrip_dice_count(caster.level());
        let (effects, _saves) = neutral_burst_save_only(
            encounter,
            caster_id,
            point,
            1,
            AbilityScoreType::Dexterity,
            dc,
            Dice::new(n, 6),
            DamageType::Acid,
            "acid splash",
        );
        effects
    }
}

pub static ACID_SPLASH: LazyLock<AcidSplash> = LazyLock::new(|| AcidSplash {});

/// Chill Touch — wizard cantrip. Ranged spell attack: d20 + INT vs AC.
/// On hit: 1d8 necrotic. Auxiliary RAW rider (target can't regain HP
/// until the start of caster's next turn) is omitted for now — the
/// engine doesn't yet model "no-heal" gates. Crit doubles the dice.
pub struct ChillTouch {}

impl Action for ChillTouch {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Necromancy)
    }
    fn name(&self) -> &str {
        "chill touch"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ct", "chill"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 120 ft = 48 tiles.
        Some(48)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Necrotic]
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let attack_bonus = caster.spellcasting_attack_modifier();
        let n = crate::engine::util::cantrip_dice_count(caster.level());
        spell_attack(
            encounter,
            caster_id,
            target_id,
            "chill touch",
            attack_bonus,
            Dice::new(n, 8),
            DamageType::Necrotic,
            false,
        )
    }
}

pub static CHILL_TOUCH: LazyLock<ChillTouch> = LazyLock::new(|| ChillTouch {});

/// Spiritual Weapon — level-2 evocation. Bonus action; the caster makes
/// a melee spell attack (using WIS modifier + proficiency, no STR) against
/// a target within reach (we collapse the floating-weapon range to
/// melee reach since we don't yet model summoned terrain). On hit:
/// 1d8 + WIS mod force damage. Reach 1 tile (5 ft). No concentration.
pub struct SpiritualWeapon {}

impl Action for SpiritualWeapon {
    fn name(&self) -> &str {
        "spiritual weapon"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["sw-spell", "spirit"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // We model the floating weapon as caster-melee for now.
        Some(crate::actions::action_template::MELEE_REACH)
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Force]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        bonus_action_and_slot(2)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        // RAW: "a melee spell attack ... 1d8 force damage plus your
        // spellcasting ability modifier." Both halves read the *same*
        // ability, so they resolve it once and share it — attacking off
        // one stat and adding damage off another would be a caster with
        // two spellcasting abilities, which is not a thing.
        let ability = caster.best_spellcasting_ability(
            crate::actors::actor_template::ActorInstance::SPELLCASTING_ABILITIES,
        );
        let attack_bonus = caster.spell_attack_modifier(ability);
        let damage_bonus = caster.ability_modifier(ability);
        spell_attack_with_bonus(
            encounter,
            caster_id,
            target_id,
            "spiritual weapon",
            attack_bonus,
            Dice::new(1, 8),
            damage_bonus,
            DamageType::Force,
            true,
        )
    }
}

pub static SPIRITUAL_WEAPON: LazyLock<SpiritualWeapon> = LazyLock::new(|| SpiritualWeapon {});

/// Hunter's Mark — level-1 divination, concentration. Mark a target;
/// while marked, the caster's weapon attacks against them deal an
/// extra 1d6 of weapon damage (handled by `resolve_attack`). Bonus
/// action to cast.
pub struct HuntersMark {}

impl Action for HuntersMark {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Divination)
    }
    fn name(&self) -> &str {
        "hunters mark"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["hm", "mark"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 90 ft = 36 tiles.
        Some(36)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        bonus_action_and_slot(1)
    }
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        vec![
            Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::HuntersMarked,
                timer: ConditionTimer::Rounds(10),
            }),
            Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions(
                    "Hunter's Mark",
                    vec![(target_id, Condition::HuntersMarked)],
                ),
            }),
        ]
    }
}

pub static HUNTERS_MARK: LazyLock<HuntersMark> = LazyLock::new(|| HuntersMark {});

/// Poison Spray — wizard / druid / sorcerer / warlock cantrip. The caster
/// extends a hand toward an adjacent creature; the target must make a
/// CON save against the caster's spell save DC or take 1d12 poison. No
/// damage on a successful save (cantrip saves are binary). Touch range
/// (1 tile) per 5e RAW.
pub struct PoisonSpray {}

impl Action for PoisonSpray {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Conjuration)
    }
    fn name(&self) -> &str {
        "poison spray"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ps", "poison"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(crate::actions::action_template::MELEE_REACH)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Poison]
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spellcasting_save_dc();
        let n = crate::engine::util::cantrip_dice_count(caster.level());
        cantrip_save_damage(
            encounter,
            caster_id,
            target_id,
            AbilityScoreType::Constitution,
            dc,
            Dice::new(n, 12),
            DamageType::Poison,
            "poison spray",
        )
        .0
    }
}

pub static POISON_SPRAY: LazyLock<PoisonSpray> = LazyLock::new(|| PoisonSpray {});

/// Inflict Wounds — level-1 necromancy. Melee spell attack; on hit, 3d10
/// necrotic damage. The dark mirror of Cure Wounds — used by warlocks,
/// evil clerics, and necromancers. Crit doubles the dice per RAW. Uses
/// the caster's WIS spell-attack mod by default (cleric flavor); we pick
/// WIS because Inflict Wounds is the cleric domain spell.
pub struct InflictWounds {}

impl Action for InflictWounds {
    fn name(&self) -> &str {
        "inflict wounds"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["iw", "inflict"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(crate::actions::action_template::MELEE_REACH)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Necrotic]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(crate::engine::action_overrides::cast_level(overrides, 1))
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let attack_bonus = caster.spellcasting_attack_modifier();
        let lvl = crate::engine::action_overrides::cast_level(overrides, 1);
        let dice = 3 + (lvl - 1);
        spell_attack(
            encounter,
            caster_id,
            target_id,
            "inflict wounds",
            attack_bonus,
            Dice::new(dice, 10),
            DamageType::Necrotic,
            true,
        )
    }
}

pub static INFLICT_WOUNDS: LazyLock<InflictWounds> = LazyLock::new(|| InflictWounds {});

/// Ray of Sickness — level-1 necromancy. Ranged spell attack vs target's
/// AC; on hit, 2d8 poison. Then the target rolls a CON save vs the
/// caster's spell save DC; on fail, gains the Poisoned condition until
/// the end of the caster's next turn (we use a 1-round timer for
/// approximation). No effect on a passed save (other than the damage).
pub struct RayOfSickness {}

impl Action for RayOfSickness {
    fn name(&self) -> &str {
        "ray of sickness"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ros", "sickness"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60 ft = 24 tiles.
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Poison]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(1)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let attack_bonus = caster.spell_attack_modifier(AbilityScoreType::Intelligence);
        let dc = caster.spell_save_dc(AbilityScoreType::Intelligence);
        let mut effects = spell_attack(
            encounter,
            caster_id,
            target_id,
            "ray of sickness",
            attack_bonus,
            Dice::new(2, 8),
            DamageType::Poison,
            false,
        );
        // Only run the rider save when the ray landed — `spell_attack`
        // returns an empty Vec on a miss, so checking emptiness keeps the
        // RAW gate ("on a hit, also...") honest.
        if effects.is_empty() {
            return effects;
        }
        // Caster-aware save so Heightened Spell metamagic can force
        // disadvantage on the rider save. The prime is consumed on the
        // first save resolved through this site.
        let save = encounter
            .roll_save_against_caster(target_id, AbilityScoreType::Constitution, dc, caster_id);
        if !save.passed() {
            effects.push(Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Poisoned,
                timer: ConditionTimer::Rounds(1),
            }));
        }
        effects
    }
}

pub static RAY_OF_SICKNESS: LazyLock<RayOfSickness> = LazyLock::new(|| RayOfSickness {});

/// Lesser Restoration — level-2 abjuration. Touch range; remove one of
/// Poisoned / Blinded / Deafened / Paralyzed from an ally. We don't
/// expose the choice to the caller — the engine pops the first present
/// condition in that priority order (poison-first matches the spell's
/// most common 5e use case).
pub struct LesserRestoration {}

impl Action for LesserRestoration {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Abjuration)
    }
    fn name(&self) -> &str {
        "lesser restoration"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["lr", "restore"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(crate::actions::action_template::MELEE_REACH)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(2)
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        _caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // RAW: the spell ends "one disease or condition" — there has to
        // be something to end. Without this gate, the AI's heal-search
        // would happily fire Lesser Restoration on a clean ally and
        // burn a level-2 slot for nothing.
        let Some(target_id) = first_target_id(target_ids) else {
            return false;
        };
        let Some(target) = encounter.actors.get(&target_id) else {
            return false;
        };
        Self::CANDIDATES.iter().any(|c| target.has_condition(*c))
    }
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        _caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        vec![Box::new(crate::engine::side_effects::RemoveOneOfConditions {
            actor_id: target_id,
            candidates: Self::CANDIDATES.to_vec(),
        })]
    }
}

impl LesserRestoration {
    /// Conditions Lesser Restoration is allowed to lift, in priority
    /// order (the cleanse pops the first match).
    const CANDIDATES: [Condition; 4] = [
        Condition::Poisoned,
        Condition::Blinded,
        Condition::Deafened,
        Condition::Paralyzed,
    ];
}

pub static LESSER_RESTORATION: LazyLock<LesserRestoration> =
    LazyLock::new(|| LesserRestoration {});

/// Thorn Whip — druid cantrip. Ranged spell attack at 30 ft (12 tiles);
/// on hit, 1d6 piercing and the target is pulled up to 10 ft (2 tiles)
/// toward the caster (RAW: Large or smaller; we apply the cap as a size
/// gate). Crit doubles the dice; the pull is unaffected by crit.
pub struct ThornWhip {}

impl Action for ThornWhip {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Transmutation)
    }
    fn name(&self) -> &str {
        "thorn whip"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["tw-spell", "thorn"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 30 ft = 12 tiles.
        Some(12)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Piercing]
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::side_effects::PullActor;
        use crate::engine::types::Size;

        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let caster_loc = caster.location();
        let attack_bonus = caster.spellcasting_attack_modifier();
        let mut effects = spell_attack(
            encounter,
            caster_id,
            target_id,
            "thorn whip",
            attack_bonus,
            Dice::new(1, 6),
            DamageType::Piercing,
            false,
        );
        // Pull only fires on a hit; on a miss `spell_attack` returns
        // an empty effect list so we'd skip silently anyway.
        if effects.is_empty() {
            return effects;
        }
        // 5e: Large or smaller — Huge / Gargantuan targets ignore the pull.
        let too_big = encounter
            .actors
            .get(&target_id)
            .map(|t| matches!(t.size(), Size::Huge | Size::Gargantuan))
            .unwrap_or(true);
        if !too_big {
            effects.push(Box::new(PullActor {
                actor_id: target_id,
                toward: caster_loc,
                max_tiles: 2,
            }));
        }
        effects
    }
}

pub static THORN_WHIP: LazyLock<ThornWhip> = LazyLock::new(|| ThornWhip {});

/// Spare the Dying — cleric cantrip. Stabilize a dying ally at touch
/// range. No spell slot, no save, no damage. Only valid if the target
/// has 0 HP and is rolling death saves (Dying). Stabilization stops
/// the death-save cycle without restoring HP — the target sits at 0
/// HP / Stable / Unconscious until healed.
pub struct SpareTheDying {}

impl Action for SpareTheDying {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Necromancy)
    }
    fn name(&self) -> &str {
        "spare the dying"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["std", "spare"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(crate::actions::action_template::MELEE_REACH)
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    // Stabilize is a heal in spirit — it pulls the target off the death-
    // save treadmill. The AI's "find someone to help" pipeline keys off
    // is_heal so this gets considered the same way Cure Wounds does.
    fn is_heal(&self) -> bool {
        true
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        _caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        let Some(target_id) = first_target_id(target_ids) else {
            return false;
        };
        encounter
            .actors
            .get(&target_id)
            .is_some_and(|a| a.is_dying())
    }
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        _caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        vec![Box::new(crate::engine::side_effects::StabilizeActor {
            actor_id: target_id,
        })]
    }
}

pub static SPARE_THE_DYING: LazyLock<SpareTheDying> = LazyLock::new(|| SpareTheDying {});

/// Toll the Dead — cleric / warlock cantrip. Range 60ft (24 tiles).
/// Target makes a WIS save vs caster's spell save DC; on fail, takes
/// Nd8 necrotic (or Nd12 if wounded) where N scales with caster level.
pub struct TollTheDead {}

impl Action for TollTheDead {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Necromancy)
    }
    fn name(&self) -> &str {
        "toll the dead"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ttd", "toll"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Necrotic]
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spellcasting_save_dc();
        let n = crate::engine::util::cantrip_dice_count(caster.level());
        // The die upgrade is resolved before the save so the shared
        // cantrip resolver gets the right pool; RAW keys it on the
        // target's wounded state, which the save can't change.
        let wounded = encounter
            .actors
            .get(&target_id)
            .is_some_and(|a| a.is_wounded());
        let die = if wounded { Dice::new(n, 12) } else { Dice::new(n, 8) };
        cantrip_save_damage(
            encounter,
            caster_id,
            target_id,
            AbilityScoreType::Wisdom,
            dc,
            die,
            DamageType::Necrotic,
            if wounded {
                "toll the dead (wounded)"
            } else {
                "toll the dead"
            },
        )
        .0
    }
}

pub static TOLL_THE_DEAD: LazyLock<TollTheDead> = LazyLock::new(|| TollTheDead {});

/// Vicious Mockery — bard cantrip. Range 60ft (24 tiles). Target WIS
/// save vs caster's CHA-based DC. On fail: 1d4 psychic AND disadvantage
/// on its next attack roll (we tag with Condition::Mocked, which the
/// engine reads in `compute_attack_mode`). On pass, nothing.
pub struct ViciousMockery {}

impl Action for ViciousMockery {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Enchantment)
    }
    fn name(&self) -> &str {
        "vicious mockery"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["vm", "mock"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Psychic]
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spell_save_dc(AbilityScoreType::Charisma);
        let n = crate::engine::util::cantrip_dice_count(caster.level());
        let (mut effects, passed) = cantrip_save_damage(
            encounter,
            caster_id,
            target_id,
            AbilityScoreType::Wisdom,
            dc,
            Dice::new(n, 4),
            DamageType::Psychic,
            "vicious mockery",
        );
        // RAW's disadvantage rider lands only on a failed save. Potent
        // Cantrip's "takes half the damage but suffers no additional
        // effect" is exactly this split — a saved target that still eats
        // half the psychic damage does not pick up `Mocked` — which is
        // why the resolver hands back the save outcome rather than just
        // the damage effects.
        if !passed {
            effects.push(Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Mocked,
                timer: ConditionTimer::UntilStartOfNextTurn,
            }));
        }
        effects
    }
}

pub static VICIOUS_MOCKERY: LazyLock<ViciousMockery> = LazyLock::new(|| ViciousMockery {});

/// Heroism — bard / paladin level-1, concentration. Target ally gains
/// temp HP equal to caster's spellcasting modifier (we use CHA) at the
/// start of each of their turns, and immunity to Frightened while the
/// spell is up. We model the "temp HP each turn" via an immediate
/// grant on cast and rely on concentration cleanup to drop the
/// Heroic condition; ticking the regrant each turn would require a
/// per-actor concentration tick we don't have today.
pub struct Heroism {}

impl Action for Heroism {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Enchantment)
    }
    fn name(&self) -> &str {
        "heroism"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["hr", "hero"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(crate::actions::action_template::MELEE_REACH)
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn is_heal(&self) -> bool {
        true
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        bonus_action_and_slot(1)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let amt = caster.ability_modifier(AbilityScoreType::Charisma).max(1) as u32;
        vec![
            Box::new(GainTempHp {
                actor_id: target_id,
                amount: amt,
            }),
            Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Heroic,
                timer: ConditionTimer::Rounds(10),
            }),
            Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions(
                    "Heroism",
                    vec![(target_id, Condition::Heroic)],
                ),
            }),
        ]
    }
}

pub static HEROISM: LazyLock<Heroism> = LazyLock::new(|| Heroism {});

/// Mass Healing Word — cleric level-3 bonus-action heal. Up to six
/// creatures within range, each within line-of-sight of the caster,
/// regain `1d4 + WIS` HP. We implement it with a per-actor radius
/// (60ft = 24 tile gap) and an LOS check; the AI's heal-search
/// pipeline can ignore it for now (it picks single-target).
pub struct MassHealingWord {}

impl Action for MassHealingWord {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Evocation)
    }
    fn name(&self) -> &str {
        "mass healing word"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["mhw", "mass-heal"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn is_heal(&self) -> bool {
        true
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        bonus_action_and_slot(3)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::util::{footprint_chebyshev, get_tiles_from_size};
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let caster_team = caster.team();
        let caster_loc = caster.location();
        let caster_size = get_tiles_from_size(caster.size());
        let wis_mod = caster.ability_modifier(AbilityScoreType::Wisdom);
        // Mass Healing Word is a fixed level-3 slot in this engine (no
        // upcasting exposed). Feed the fixed level to the Disciple of
        // Life bonus so a Life Cleric adds +5 per target on top of the
        // shared roll. Snapshot before the (mutable) roll.
        let bonus = crate::actions::class_features::disciple_of_life_bonus(caster, 3);
        // RAW: pick up to 6 creatures. We snap to the closest 6 eligible
        // allies (combat-active OR dying — heals revive both).
        const RANGE_TILES: isize = 24;
        const MAX_TARGETS: usize = 6;
        let mut candidates: Vec<(isize, usize)> = encounter
            .actors
            .iter()
            .filter_map(|(id, a)| {
                if a.team() != caster_team {
                    return None;
                }
                if !a.is_combat_active() && !a.is_dying() {
                    return None;
                }
                let dist = footprint_chebyshev(
                    caster_loc,
                    caster_size,
                    a.location(),
                    get_tiles_from_size(a.size()),
                );
                if dist > RANGE_TILES {
                    return None;
                }
                Some((dist, *id))
            })
            .collect();
        candidates.sort_unstable();
        candidates.truncate(MAX_TARGETS);
        // 5e Grave Domain Cleric **Circle of Mortality** — swap the
        // shared 1d4 roll for its max face-value (4) if the caster
        // holds the tag AND any picked target is at 0 HP. The RAW
        // clause is per-die not per-target; on a shared-roll mass
        // heal the coherent read is "if any die is being applied to
        // a 0-HP target, that die maxes — and since all dice are
        // shared, all dice max". A downed-ally-included Mass Healing
        // Word burst floors the whole burst at max; a fully-healthy
        // burst rolls normally. Snapshot before the (mutable) roll.
        let dice = Dice::new(1, 4);
        let use_max = caster
            .has_passive_feature(crate::actions::class_features::CIRCLE_OF_MORTALITY_TAG)
            && candidates
                .iter()
                .any(|(_, id)| encounter.actors.get(id).is_some_and(|a| a.hitpoints() == 0));
        let raw = if use_max {
            dice.max_roll() as i32
        } else {
            encounter.roll(&dice) as i32
        };
        let base = (raw + wis_mod).max(1) as u32;
        let amount = base + bonus;
        encounter.log(format!(
            "  mass healing word: {}({}){}{:+}{} = {} HP each",
            dice,
            raw,
            crate::actions::class_features::circle_of_mortality_log_suffix(use_max),
            wis_mod,
            crate::actions::class_features::disciple_of_life_log_suffix(bonus),
            amount
        ));
        candidates
            .into_iter()
            .map(|(_, id)| {
                Box::new(Heal {
                    actor_id: id,
                    amount,
                }) as Box<dyn ApplicableSideEffect>
            })
            .collect()
    }
}

pub static MASS_HEALING_WORD: LazyLock<MassHealingWord> = LazyLock::new(|| MassHealingWord {});

/// Shocking Grasp — wizard / sorcerer cantrip. Melee spell attack; on
/// hit, 1d8 lightning AND the target loses its reactions until the
/// start of its next turn (we install Condition::NoReaction with the
/// `UntilStartOfNextTurn` timer). Has advantage on the attack roll if
/// the target is wearing metal armor — we don't model armor types, so
/// we skip that rider.
pub struct ShockingGrasp {}

impl Action for ShockingGrasp {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Evocation)
    }
    fn name(&self) -> &str {
        "shocking grasp"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["sg", "shock"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(crate::actions::action_template::MELEE_REACH)
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Lightning]
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let attack_bonus = caster.spellcasting_attack_modifier();
        let n = crate::engine::util::cantrip_dice_count(caster.level());
        let mut effects = spell_attack(
            encounter,
            caster_id,
            target_id,
            "shocking grasp",
            attack_bonus,
            Dice::new(n, 8),
            DamageType::Lightning,
            true,
        );
        if !effects.is_empty() {
            effects.push(Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::NoReaction,
                timer: ConditionTimer::UntilStartOfNextTurn,
            }));
        }
        effects
    }
}

pub static SHOCKING_GRASP: LazyLock<ShockingGrasp> = LazyLock::new(|| ShockingGrasp {});

/// Shatter — level-2 evocation. 10-ft-radius burst centered on a point
/// within 60 ft (24 tiles). Every creature in the burst makes a CON save
/// vs the caster's spell save DC: pass = half, fail = full. 3d8 thunder
/// damage. We share-roll once and route through the burst-save helper —
/// identical pattern to Burning Hands but spherical instead of a cone.
pub struct Shatter {}

impl Action for Shatter {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Evocation)
    }
    fn name(&self) -> &str {
        "shatter"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["sh-spell", "shat"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        // 10ft radius = 2-tile burst on the 2.5ft grid.
        TargetingSchema::Burst { radius: 2 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Thunder]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(crate::engine::action_overrides::cast_level(overrides, 2))
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.best_spell_save_dc([
            // Arcane / primal / pact list all reach this spell, and the
            // 5e save DC is the *caster's* spellcasting ability — the
            // wizard's INT, the sorcerer's and warlock's CHA, the
            // druid's and the Four Elements monk's WIS. Anchoring on
            // INT alone quietly handed every non-wizard carrier a DC
            // computed off a stat their class never invests in.
            AbilityScoreType::Intelligence,
            AbilityScoreType::Charisma,
            AbilityScoreType::Wisdom,
        ]);
        let lvl = crate::engine::action_overrides::cast_level(overrides, 2);
        let dice = 3 + (lvl - 2);
        // Roll through the shared caster-aware chokepoint rather than
        // `roll` directly — Empowered Spell, Empowered Evocation and
        // Potent Spellcasting all live there, and a bare `roll` skips
        // all three silently.
        let raw = encounter.roll_empowered_sum(caster_id, dice, 8);
        encounter.log(format!("  shatter: {}d8({}) = {} thunder area", dice, raw, raw));
        crate::actions::action_template::resolve_burst_save_damage(
            encounter,
            caster_id,
            point,
            2,
            AbilityScoreType::Constitution,
            dc,
            raw,
            DamageType::Thunder,
        )
    }
}

pub static SHATTER: LazyLock<Shatter> = LazyLock::new(|| Shatter {});

/// Sleep — level-1 enchantment. Roll 5d8; the total is a "HP pool".
/// Sweep enemy creatures within range in ascending current-HP order and
/// put each to Asleep until their pool of current HP is fully consumed
/// (each target consumes `current_hp` from the pool). Undead and creatures
/// immune to the Charmed condition (most are) are unaffected. Asleep is
/// stripped by any damage.
pub struct Sleep {}

impl Action for Sleep {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Enchantment)
    }
    fn name(&self) -> &str {
        "sleep"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["sleep-spell", "slumber"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SinglePoint
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 90ft = 36 tiles.
        Some(36)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(1)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        const BURST_RADIUS: isize = 8; // 20ft radius
        let pool_roll = encounter.roll(&Dice::new(5, 8));
        encounter.log(format!("  sleep: 5d8({}) = {} HP pool", pool_roll, pool_roll));

        // Sort eligible targets by ascending current HP (5e RAW). Undead
        // and Charmed-immune creatures are skipped — they don't dream.
        // The Charmed gate doubles up: in our pool, Charm immunity is
        // the cleanest "no mind-affecting" proxy.
        let hit = crate::actions::action_template::pool_sweep_targets(
            encounter,
            caster_id,
            point,
            BURST_RADIUS,
            pool_roll,
            Condition::Charmed,
        );
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        for id in hit {
            effects.push(Box::new(ApplyCondition {
                actor_id: id,
                condition: Condition::Asleep,
                timer: ConditionTimer::Rounds(10),
            }));
            // 5e: Sleep also drops the target prone (unconscious clause).
            effects.push(Box::new(ApplyCondition {
                actor_id: id,
                condition: Condition::Prone,
                timer: ConditionTimer::Permanent,
            }));
        }
        effects
    }
}

pub static SLEEP: LazyLock<Sleep> = LazyLock::new(|| Sleep {});

/// Charm Person — level-1 enchantment. Target makes a WIS save vs the
/// caster's spell save DC; on fail, the target is Charmed for an hour
/// (we use 10 rounds). The charmed creature can't attack their charmer
/// (enforced in `validate_input`). On a save, the spell fizzles.
pub struct CharmPerson {}

impl Action for CharmPerson {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Enchantment)
    }
    fn name(&self) -> &str {
        "charm person"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["cp", "charm"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(12)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn deals_damage(&self) -> bool {
        false
    }
    // Charm is harmful in 5e (it's a hostile mind-affecting spell), so we
    // leave is_harmful at the default true. The validate_input charm-vs-
    // charmer block still works because a freshly-charmed actor can't
    // retaliate against the original charmer.
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(1)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spellcasting_save_dc();
        // Caster-aware save so Heightened Spell can force disadvantage.
        let save =
            encounter.roll_save_against_caster(target_id, AbilityScoreType::Wisdom, dc, caster_id);
        if save.passed() {
            return Vec::new();
        }
        // 1 hour RAW capped to encounter-scale via the shared install
        // helper — keeps the Charmed flag + `Charmed` back-link in
        // lockstep with Charm Monster / Geas.
        install_charmed_by(target_id, caster_id, ConditionTimer::Rounds(10))
    }
}

pub static CHARM_PERSON: LazyLock<CharmPerson> = LazyLock::new(|| CharmPerson {});

/// Mirror Image — level-2 illusion. No save, no concentration, no
/// targeting. The caster gains three duplicates that absorb incoming
/// attacks: a hit may instead pop a decoy. Pool count is tracked on the
/// actor and read by `resolve_attack` (engine/attack.rs).
pub struct MirrorImage {}

impl Action for MirrorImage {
    fn name(&self) -> &str {
        "mirror image"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["mi", "mirror"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(2)
    }
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::side_effects::SetMirrorImages;
        vec![
            Box::new(SetMirrorImages {
                actor_id: caster_id,
                count: 3,
            }),
            Box::new(ApplyCondition {
                actor_id: caster_id,
                condition: Condition::MirroredImages,
                timer: ConditionTimer::Rounds(10),
            }),
        ]
    }
}

pub static MIRROR_IMAGE: LazyLock<MirrorImage> = LazyLock::new(|| MirrorImage {});

/// Eldritch Blast — warlock cantrip. Ranged spell attack: d20 + CHA vs
/// AC. Fires N separate beams (1 at level 1, 2 at level 5, 3 at level
/// 11, 4 at level 17) — each beam makes its own attack roll for 1d10
/// force damage. Per 5e RAW each beam is an independent attack, so they
/// can individually hit or miss and each triggers on-hit riders (Hex,
/// etc.) separately.
pub struct EldritchBlast {}

impl Action for EldritchBlast {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Evocation)
    }
    fn name(&self) -> &str {
        "eldritch blast"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["eb", "blast"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(48)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Force]
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::actions::class_features::{AGONIZING_BLAST_TAG, REPELLING_BLAST_TAG};
        use crate::engine::side_effects::PushActor;
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let attack_bonus = caster.spellcasting_attack_modifier();
        let beam_count = crate::engine::util::cantrip_dice_count(caster.level());
        // 5e Warlock Eldritch Invocations: read both invocations up front
        // from the caster's permanent feature tags. Agonizing Blast adds
        // CHA modifier to each beam's damage; Repelling Blast appends a
        // PushActor on hit (4-tile push, gated to non-Huge/non-Gargantuan
        // targets per RAW's "Large or smaller" clause).
        let agonizing = caster.feature_available(AGONIZING_BLAST_TAG);
        let repelling = caster.feature_available(REPELLING_BLAST_TAG);
        let damage_bonus = if agonizing {
            // RAW: add CHA mod, floor at 0 — a negative CHA modifier
            // doesn't reduce beam damage (the invocation only buffs).
            caster.ability_modifier(AbilityScoreType::Charisma).max(0)
        } else {
            0
        };
        let caster_loc = caster.location();
        // RAW "Large or smaller" gate: skip the push on Huge / Gargantuan
        // targets — they're too massive for the cantrip's recoil. Snapshot
        // the gate result up front so the per-beam loop doesn't re-query
        // the actor map (the target can change HP / die but not size).
        let push_target = repelling
            && encounter
                .actors
                .get(&target_id)
                .is_some_and(|t| t.size().ordinal() <= crate::engine::types::Size::Large.ordinal());
        // Fire each beam as an independent attack roll (1d10 force each).
        // This matches 5e RAW: each beam can hit or miss individually and
        // triggers on-hit riders (Hex, Hunter's Mark, etc.) per beam.
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        for i in 0..beam_count {
            let label = if beam_count > 1 {
                format!("eldritch blast (beam {})", i + 1)
            } else {
                "eldritch blast".to_string()
            };
            let (beam_effects, damage) = spell_attack_outcome(
                encounter,
                caster_id,
                target_id,
                &label,
                attack_bonus,
                Dice::new(1, 10),
                damage_bonus,
                DamageType::Force,
                false,
            );
            effects.extend(beam_effects);
            // Repelling Blast: push on every beam that landed (damage > 0).
            // 5e RAW lets the warlock choose which beam(s) push; in our
            // model the AI doesn't make that choice so we apply per-beam
            // unconditionally — the worst case is the target gets shoved
            // farther than necessary, which still matches the cantrip's
            // intent.
            if push_target && damage > 0 {
                effects.push(Box::new(PushActor {
                    actor_id: target_id,
                    from: caster_loc,
                    max_tiles: 4,
                }));
            }
        }
        effects
    }
}

pub static ELDRITCH_BLAST: LazyLock<EldritchBlast> = LazyLock::new(|| EldritchBlast {});

/// Protection from Evil and Good — level-1 abjuration, concentration.
/// Target gains the Warded condition: aberrations, celestials, elementals,
/// fey, fiends, and undead have disadvantage on attacks against them. We
/// approximate the creature-type gate via the target's necrotic/poison
/// immunity profile (a rough but reliable proxy for undead / fiend status
/// in our pool). The condition is read by `compute_attack_mode`.
pub struct ProtectionFromEvilAndGood {}

impl Action for ProtectionFromEvilAndGood {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Abjuration)
    }
    fn name(&self) -> &str {
        "protection from evil and good"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["pfeg", "protection"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(crate::actions::action_template::MELEE_REACH)
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(1)
    }
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        vec![
            Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Warded,
                timer: ConditionTimer::Rounds(10),
            }),
            Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions(
                    "Protection from Evil and Good",
                    vec![(target_id, Condition::Warded)],
                ),
            }),
        ]
    }
}

pub static PROTECTION_FROM_EVIL_AND_GOOD: LazyLock<ProtectionFromEvilAndGood> =
    LazyLock::new(|| ProtectionFromEvilAndGood {});

/// Color Spray — level-1 illusion. Roll a 6d10 HP pool; sweep enemies in
/// a 15ft cone (we approximate with a 2-tile burst centered on the target
/// point) in ascending current-HP order, blinding each one until the end
/// of the caster's next turn until the pool is exhausted. Targets immune
/// to Blinded (constructs, oozes that don't have eyes) are skipped, and a
/// target with more current HP than the remaining pool stops the sweep.
/// Distinct from Sleep: shorter timer, blinds instead of asleep, and
/// undead are *not* exempt — only literal blind-immune creatures are.
pub struct ColorSpray {}

impl Action for ColorSpray {
    fn name(&self) -> &str {
        "color spray"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["cs-spell", "color"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SinglePoint
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 15ft cone — short range, treat the burst origin as the cone's
        // far edge much like Burning Hands.
        Some(6)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(1)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        const BURST_RADIUS: isize = 2;
        let pool = encounter.roll(&Dice::new(6, 10));
        encounter.log(format!(
            "  color spray: 6d10({}) = {} HP pool",
            pool, pool
        ));
        crate::actions::action_template::pool_sweep_targets(
            encounter,
            caster_id,
            point,
            BURST_RADIUS,
            pool,
            Condition::Blinded,
        )
        .into_iter()
        .map(|id| {
            Box::new(ApplyCondition {
                actor_id: id,
                condition: Condition::Blinded,
                timer: ConditionTimer::UntilStartOfNextTurn,
            }) as Box<dyn ApplicableSideEffect>
        })
        .collect()
    }
}

pub static COLOR_SPRAY: LazyLock<ColorSpray> = LazyLock::new(|| ColorSpray {});

/// Command — level-1 enchantment. The caster barks a one-word command at
/// a target within 60ft; on a failed WIS save the target spends their
/// next turn complying (we model the "Halt" / "Grovel" variants as a
/// simple Stunned for one turn). Save-immune creatures (charm-immune in
/// our pool: undead, constructs) are unaffected. No concentration; the
/// effect is short and self-clearing via the UntilStartOfNextTurn timer.
pub struct Command {}

impl Action for Command {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Enchantment)
    }
    fn name(&self) -> &str {
        "command"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["cmd", "halt"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60ft = 24 tiles.
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(1)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        // 5e Command: undead and creatures that don't understand the
        // caster's language are immune. Charm immunity is the closest
        // proxy for "won't be cowed" in our pool — honor dynamic
        // immunities too (Fey Ancestry / MindBlanked).
        if let Some(target) = encounter.actors.get(&target_id)
            && target.effectively_immune_to_condition(Condition::Charmed)
        {
            encounter.log("  command: target is immune".to_string());
            return Vec::new();
        }
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spellcasting_save_dc();
        // Caster-aware save so Heightened Spell metamagic can force
        // disadvantage on the save. The prime is consumed on the first
        // save resolved through this site.
        let save = encounter
            .roll_save_against_caster(target_id, AbilityScoreType::Wisdom, dc, caster_id);
        if save.passed() {
            return Vec::new();
        }
        vec![Box::new(ApplyCondition {
            actor_id: target_id,
            condition: Condition::Stunned,
            timer: ConditionTimer::UntilStartOfNextTurn,
        })]
    }
}

pub static COMMAND: LazyLock<Command> = LazyLock::new(|| Command {});

/// Fireball — iconic 5e level-3 evocation. 150ft range, 20ft-radius
/// sphere (radius 4 on the 2.5ft grid). DEX save against the caster's
/// INT-based DC: failed save takes 8d6 fire, success takes half. Single
/// shared damage roll for the whole burst (5e shared-roll AoE). Damage
/// scales by +1d6 per spell level above 3 — our Resource::SpellSlot
/// model only reports the base level, so the scaling lane is preserved
/// for the future leveled-cast UI but defaults to 8d6 for now.
pub struct Fireball {}

impl Action for Fireball {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Evocation)
    }
    fn name(&self) -> &str {
        "fireball"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["fb-spell", "fire-ball"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 4 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 150ft = 60 tiles.
        Some(60)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Fire]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(crate::engine::action_overrides::cast_level(overrides, 3))
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.best_spell_save_dc([
            // Arcane / primal / pact list all reach this spell, and the
            // 5e save DC is the *caster's* spellcasting ability — the
            // wizard's INT, the sorcerer's and warlock's CHA, the
            // druid's and the Four Elements monk's WIS. Anchoring on
            // INT alone quietly handed every non-wizard carrier a DC
            // computed off a stat their class never invests in.
            AbilityScoreType::Intelligence,
            AbilityScoreType::Charisma,
            AbilityScoreType::Wisdom,
        ]);
        let lvl = crate::engine::action_overrides::cast_level(overrides, 3);
        let dice = 8 + (lvl - 3);
        // Route through `roll_empowered_sum` so Empowered Spell metamagic
        // primes (Sorcerer) reroll low dice on the shared Fireball roll.
        // For non-empowered casters the helper falls through to plain
        // rolls — identical RNG consumption to a flat `roll(&Dice::new(...))`.
        let raw = encounter.roll_empowered_sum(caster_id, dice, 6);
        encounter.log(format!("  fireball: {}d6({}) = {} fire area", dice, raw, raw));
        crate::actions::action_template::resolve_burst_save_damage(
            encounter,
            caster_id,
            point,
            4,
            AbilityScoreType::Dexterity,
            dc,
            raw,
            DamageType::Fire,
        )
    }
}

pub static FIREBALL: LazyLock<Fireball> = LazyLock::new(|| Fireball {});

/// Magic Weapon — level-2 transmutation, concentration up to 1 hour.
/// Touch a single weapon-wielding ally; their attack rolls and damage
/// gain a flat +1 bonus for the duration. We track each half on its own
/// concentration-managed ledger: `attack_bonus_buff` for the to-hit
/// half, `damage_bonus_buff` for the damage half. Both drop cleanly
/// when concentration ends. Targeting is touch (1 tile reach).
pub struct MagicWeapon {}

impl Action for MagicWeapon {
    fn name(&self) -> &str {
        "magic weapon"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["mw", "magic-weapon"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(crate::actions::action_template::MELEE_REACH)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        bonus_action_and_slot(2)
    }
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::side_effects::{AdjustAttackBuff, AdjustDamageBuff};
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        // Install +1 attack AND +1 damage buffs, registering both on the
        // concentration so dropping the spell rolls back each delta on
        // the right actor.
        vec![
            Box::new(AdjustAttackBuff {
                actor_id: target_id,
                delta: 1,
            }),
            Box::new(AdjustDamageBuff {
                actor_id: target_id,
                delta: 1,
            }),
            Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::new("Magic Weapon")
                    .with_attack_buffs(vec![(target_id, 1)])
                    .with_damage_buffs(vec![(target_id, 1)]),
            }),
        ]
    }
}

pub static MAGIC_WEAPON: LazyLock<MagicWeapon> = LazyLock::new(|| MagicWeapon {});

/// Scorching Ray — level-2 evocation. Three independent ranged spell
/// attack rolls against the same target (or, in 5e RAW, different
/// targets — we don't yet model multi-target selection so they all
/// converge on the chosen actor). Each ray deals 2d6 fire on hit. No
/// save: standard spell attack vs AC per ray. The triple-attack lane
/// rewards a high spell attack mod and gives wizards a reliable
/// concentration-free single-target nuke.
pub struct ScorchingRay {}

impl Action for ScorchingRay {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Evocation)
    }
    fn name(&self) -> &str {
        "scorching ray"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["sr", "scorch"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 120ft = 48 tiles.
        Some(48)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Fire]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(2)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let attack_bonus = caster.spellcasting_attack_modifier();
        let mut all: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        // Three independent rays. Each is its own attack roll → its own
        // hit/miss/crit. If the target falls between rays the later rays
        // still queue DealDamage, which no-ops on a dead actor.
        for i in 0..3 {
            let label = format!("scorching ray (ray {})", i + 1);
            let effs = spell_attack(
                encounter,
                caster_id,
                target_id,
                &label,
                attack_bonus,
                Dice::new(2, 6),
                DamageType::Fire,
                false,
            );
            all.extend(effs);
        }
        all
    }
}

pub static SCORCHING_RAY: LazyLock<ScorchingRay> = LazyLock::new(|| ScorchingRay {});

/// Lightning Bolt — level-3 evocation. A 100ft line / 5ft wide (RAW); we
/// approximate as a burst at the target point: every creature in a
/// 4-tile radius makes a DEX save vs caster's spell save DC for 8d6
/// lightning. Pass = half, fail = full. Shared damage roll across all
/// targets. Distinct from Fireball: lightning damage type and same
/// resource cost — picking between the two is a function of enemy
/// resistances and party positioning (lightning more linear-flavored
/// even if our grid approximation is a sphere).
pub struct LightningBolt {}

impl Action for LightningBolt {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Evocation)
    }
    fn name(&self) -> &str {
        "lightning bolt"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["lb", "lightning"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 4 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 100ft = 40 tiles.
        Some(40)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Lightning]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(crate::engine::action_overrides::cast_level(overrides, 3))
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spellcasting_save_dc();
        let lvl = crate::engine::action_overrides::cast_level(overrides, 3);
        let dice = 8 + (lvl - 3);
        // Route through `roll_empowered_sum` — Sorcerer Empowered Spell
        // primes apply to the lightning bolt's shared damage pool.
        let raw = encounter.roll_empowered_sum(caster_id, dice, 6);
        encounter.log(format!(
            "  lightning bolt: {}d6({}) = {} lightning area",
            dice, raw, raw
        ));
        crate::actions::action_template::resolve_burst_save_damage(
            encounter,
            caster_id,
            point,
            4,
            AbilityScoreType::Dexterity,
            dc,
            raw,
            DamageType::Lightning,
        )
    }
}

pub static LIGHTNING_BOLT: LazyLock<LightningBolt> = LazyLock::new(|| LightningBolt {});

/// Vampiric Touch — level-3 necromancy, concentration. Melee spell
/// attack; on hit, target takes 3d6 necrotic and the caster heals half
/// (rounded down). Concentration is installed so re-cast within an hour
/// drops the prior buff cleanly. Distinct from Inflict Wounds (1-action
/// burst nuke, no slot scaling); Vampiric Touch trades single-hit damage
/// for sustained self-sustain on a beefy caster.
pub struct VampiricTouch {}

impl Action for VampiricTouch {
    fn name(&self) -> &str {
        "vampiric touch"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["vt", "vamp"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(crate::actions::action_template::MELEE_REACH)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Necrotic]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(3)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let attack_bonus = caster.spellcasting_attack_modifier();
        // spell_attack rolls the d20 vs AC and (on hit) the damage dice,
        // returning a DealDamage we'll merge with the self-heal rider.
        // `dealt` is the post-crit damage queued onto the target; we use
        // it to drive the half-as-heal rider without re-rolling.
        let (mut effs, dealt) = spell_attack_outcome(
            encounter,
            caster_id,
            target_id,
            "vampiric touch",
            attack_bonus,
            Dice::new(3, 6),
            0,
            DamageType::Necrotic,
            true,
        );
        // Install concentration regardless of hit/miss — RAW: the spell
        // is active for its full duration once cast, even if the first
        // strike misses. Drop on re-cast keeps memory tight.
        effs.push(Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData::new("Vampiric Touch"),
        }));
        if dealt > 0 {
            let heal = (dealt / 2).max(1);
            encounter.log(format!(
                "  vampiric touch: caster regains {} HP",
                heal
            ));
            effs.push(Box::new(Heal {
                actor_id: caster_id,
                amount: heal,
            }));
        }
        effs
    }
}

pub static VAMPIRIC_TOUCH: LazyLock<VampiricTouch> = LazyLock::new(|| VampiricTouch {});

/// Hypnotic Pattern — level-3 illusion. Burst (we use 4-tile radius for
/// a 30ft cube) of incapacitating fascination. Every creature in the
/// burst makes a WIS save vs caster's spell save DC. On fail, the
/// target is Incapacitated for several rounds (we use Rounds(5)) and
/// concentration tracks the spell so dropping it clears the condition.
/// Charm-immune creatures (undead, constructs, etc.) save automatically.
/// No damage — pure crowd control.
pub struct HypnoticPattern {}

impl Action for HypnoticPattern {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Enchantment)
    }
    fn name(&self) -> &str {
        "hypnotic pattern"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["hyp", "hypnotic"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        // 30ft cube ≈ 4-tile radius burst on the 2.5ft grid.
        TargetingSchema::Burst { radius: 4 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 120ft = 48 tiles.
        Some(48)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(3)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spellcasting_save_dc();
        let mut conditions_tracked: Vec<(usize, Condition)> = Vec::new();
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        for target_id in encounter.neutral_burst_targets(caster_id, point, 4) {
            // Charm-immune creatures shrug off the pattern (template or
            // dynamic — Fey Ancestry / MindBlanked). We don't log per-
            // target immunity for AoE — would be noisy.
            if encounter.actor_immune_to_condition(target_id, Condition::Charmed) {
                continue;
            }
            // Caster-aware save so the Sorcerer Heightened Spell prime
            // can force disadvantage on the *first* save in the burst
            // (RAW). The helper consumes the prime on its first call —
            // subsequent targets fall through to the normal save path.
            let save = encounter.roll_save_against_caster(
                target_id,
                AbilityScoreType::Wisdom,
                dc,
                caster_id,
            );
            if save.passed() {
                continue;
            }
            effects.push(Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Incapacitated,
                timer: ConditionTimer::Rounds(5),
            }));
            conditions_tracked.push((target_id, Condition::Incapacitated));
        }
        if !conditions_tracked.is_empty() {
            effects.push(Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions(
                    "Hypnotic Pattern",
                    conditions_tracked,
                ),
            }));
        }
        effects
    }
}

pub static HYPNOTIC_PATTERN: LazyLock<HypnoticPattern> = LazyLock::new(|| HypnoticPattern {});

/// Divine Favor — level-1 evocation, concentration. Self-buff: weapon
/// attacks deal +1d4 radiant for the duration. We approximate the +1d4
/// damage rider as a flat +2 attack-buff (the engine doesn't have a
/// per-attack-extra-damage lane for self-buffs yet). Installed via
/// concentration so re-casting another concentration drops it cleanly.
/// Distinct from Bless (allies-only, +1d4 to attack rolls / saves):
/// Divine Favor is self-only and stacks freely with Bless.
pub struct DivineFavor {}

impl Action for DivineFavor {
    fn name(&self) -> &str {
        "divine favor"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["df", "favor"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        bonus_action_and_slot(1)
    }
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::side_effects::AdjustAttackBuff;
        // +2 attack buff approximates "+1d4 radiant per hit". The buff
        // lives on the concentration so it rolls back automatically.
        vec![
            Box::new(AdjustAttackBuff {
                actor_id: caster_id,
                delta: 2,
            }),
            Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::new("Divine Favor")
                    .with_attack_buffs(vec![(caster_id, 2)]),
            }),
        ]
    }
}

pub static DIVINE_FAVOR: LazyLock<DivineFavor> = LazyLock::new(|| DivineFavor {});

/// Spirit Guardians — level-3 conjuration, concentration. Caster is
/// surrounded by a 15ft-radius aura of spectral guardians; each enemy
/// that starts its turn in the aura makes a WIS save vs the caster's
/// spell save DC. Pass = half, fail = full of 3d8 radiant. We model the
/// aura via a one-shot burst centered on the caster at cast time
/// (immediate damage on cast); the per-turn re-pulse requires
/// per-actor concentration tick we don't have today, so the spell's
/// flavor is collapsed to a powerful single-cast radiant burst that
/// matches a typical first-round application. Concentration tracks the
/// cast so re-casting drops cleanly.
/// Spirit Guardians — level-3 conjuration, concentration. On cast, deals
/// 3d8 radiant (WIS save for half) to nearby enemies. Then installs the
/// SpiritGuarding condition on the caster: at every round-end, the aura
/// repeats the damage to every hostile creature within 6 tiles. The
/// recurring damage is processed by `apply_spirit_guardians_aura` in
/// the round-end loop. Concentration-bound — dropping it ends the aura.
pub struct SpiritGuardians {}

impl Action for SpiritGuardians {
    fn name(&self) -> &str {
        "spirit guardians"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["sg", "spirit", "guards"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn requires_los(&self) -> bool {
        false
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Radiant]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(3)
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        encounter
            .actors
            .get(&caster_id)
            .is_some_and(|a| !a.is_concentrating() && a.is_combat_active())
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let caster_loc = caster.location();
        let dc = caster.spell_save_dc(AbilityScoreType::Wisdom);
        // Roll through the shared caster-aware chokepoint rather than
        // `roll` directly — Empowered Spell, Empowered Evocation and
        // Potent Spellcasting all live there, and a bare `roll` skips
        // all three silently.
        let raw = encounter.roll_empowered_sum(caster_id, 3, 8);
        encounter.log(format!(
            "  spirit guardians: 3d8({}) = {} radiant area",
            raw, raw
        ));
        let mut effs = crate::actions::action_template::resolve_burst_save_damage(
            encounter,
            caster_id,
            caster_loc,
            6,
            AbilityScoreType::Wisdom,
            dc,
            raw,
            DamageType::Radiant,
        );
        effs.push(Box::new(ApplyCondition {
            actor_id: caster_id,
            condition: Condition::SpiritGuarding,
            timer: ConditionTimer::Rounds(100),
        }));
        effs.push(Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData::with_conditions(
                "Spirit Guardians",
                vec![(caster_id, Condition::SpiritGuarding)],
            ),
        }));
        effs
    }
}

pub static SPIRIT_GUARDIANS: LazyLock<SpiritGuardians> = LazyLock::new(|| SpiritGuardians {});

/// Hex — level-1 enchantment, concentration. Bonus action to mark a target;
/// the caster's weapon attacks against the hexed target deal an extra 1d6
/// necrotic (handled by `resolve_attack` via `is_hex_target`). Distinct
/// from Hunter's Mark: same on-hit rider but necrotic-typed (so resistant
/// undead shrug it off) and tied to CHA-based warlock casting flavor.
pub struct Hex {}

impl Action for Hex {
    fn name(&self) -> &str {
        "hex"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["hex-mark"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 90 ft = 36 tiles (same as Hunter's Mark).
        Some(36)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Necrotic]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        bonus_action_and_slot(1)
    }
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        vec![
            Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Hexed,
                timer: ConditionTimer::Rounds(10),
            }),
            Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions(
                    "Hex",
                    vec![(target_id, Condition::Hexed)],
                ),
            }),
        ]
    }
}

pub static HEX: LazyLock<Hex> = LazyLock::new(|| Hex {});

/// Hold Monster — level-5 enchantment, concentration. Identical mechanic
/// to Hold Person (WIS save vs spell DC, on fail target is Stunned for up
/// to 10 rounds, concentration tracks the lock), but consumes a level-5
/// slot in exchange for working on creatures that would normally shrug
/// off the humanoid-only Hold Person. Charm-immune creatures (undead,
/// constructs) still resist via condition immunity.
pub struct HoldMonster {}

impl Action for HoldMonster {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Enchantment)
    }
    fn name(&self) -> &str {
        "hold monster"
    }

    fn deals_damage(&self) -> bool {
        // Control, not damage. The AI's focus-fire lane scores by who
        // drops soonest, so an action that claims damage and deals none
        // gets picked over the attack that would have.
        false
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["hm-spell", "holdm"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 90 ft = 36 tiles.
        Some(36)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(5)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spellcasting_save_dc();
        // Caster-aware save so the sorcerer Heightened Spell prime can
        // force disadvantage on this single save-or-suck roll. The
        // shared helper routes through `roll_save_against_caster`
        // automatically; empty logs match the legacy silent behavior.
        save_or_concentration_condition(
            encounter,
            caster_id,
            target_id,
            AbilityScoreType::Wisdom,
            dc,
            "Hold Monster",
            Condition::Stunned,
            ConditionTimer::Rounds(10),
            "",
            "",
        )
    }
}

pub static HOLD_MONSTER: LazyLock<HoldMonster> = LazyLock::new(|| HoldMonster {});

/// Invisibility — level-2 illusion, concentration. Target becomes Invisible
/// until concentration ends or the target makes an attack / casts a spell.
/// The "drop on attack" rider is enforced by `resolve_attack`: when the
/// caster (or whoever they targeted) attacks while concentrating on
/// Invisibility, the spell's concentration ends and the Invisible condition
/// clears with it.
pub struct Invisibility {}

impl Action for Invisibility {
    fn name(&self) -> &str {
        "invisibility"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["invis"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // Touch — 5 ft = 1 tile.
        Some(crate::actions::action_template::MELEE_REACH)
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(2)
    }
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        vec![
            Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Invisible,
                timer: ConditionTimer::Rounds(10),
            }),
            Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions(
                    "Invisibility",
                    vec![(target_id, Condition::Invisible)],
                )
                .breaking_on_attack(),
            }),
        ]
    }
}

pub static INVISIBILITY: LazyLock<Invisibility> = LazyLock::new(|| Invisibility {});

/// Bestow Curse — level-3 necromancy, concentration. WIS save vs the
/// caster's spell save DC; on fail, the target has disadvantage on
/// attack rolls and saving throws (we model via the Baned condition,
/// which already implements the symmetric -2 to both lanes — close
/// enough to RAW's "disadvantage on saves vs this caster's spells"
/// without spinning a per-source debuff lane). Concentration tracks
/// the curse so dropping it lifts the debuff cleanly.
pub struct BestowCurse {}

impl Action for BestowCurse {
    fn name(&self) -> &str {
        "bestow curse"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["bc", "curse"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // Touch — 5 ft = 1 tile.
        Some(crate::actions::action_template::MELEE_REACH)
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(3)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spellcasting_save_dc();
        let save = encounter.roll_save_against_caster(target_id, AbilityScoreType::Wisdom, dc, caster_id);
        if save.passed() {
            return Vec::new();
        }
        vec![
            Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Baned,
                timer: ConditionTimer::Rounds(10),
            }),
            Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions(
                    "Bestow Curse",
                    vec![(target_id, Condition::Baned)],
                ),
            }),
        ]
    }
}

pub static BESTOW_CURSE: LazyLock<BestowCurse> = LazyLock::new(|| BestowCurse {});

/// Mind Sliver — enchantment cantrip. INT save vs caster's spell save DC;
/// on fail, target takes 1d6 psychic AND has a -1d4 penalty (modeled via
/// the Baned condition, which is -2 to saves / attacks; close enough for
/// the single-round window). The save-debuff rider lasts one round per
/// RAW. No damage on a save (cantrip binary).
pub struct MindSliver {}

impl Action for MindSliver {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Enchantment)
    }
    fn name(&self) -> &str {
        "mind sliver"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ms", "sliver"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60 ft = 24 tiles.
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Psychic]
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spellcasting_save_dc();
        // Caster-aware save so Heightened Spell metamagic can force
        // disadvantage on the save. The prime is consumed on the first
        // save resolved through this site.
        let save = encounter
            .roll_save_against_caster(target_id, AbilityScoreType::Intelligence, dc, caster_id);
        if save.passed() {
            return Vec::new();
        }
        let dmg = encounter.roll_empowered_sum(caster_id, 1, 6);
        encounter.log(format!("  mind sliver: 1d6({}) = {} psychic", dmg, dmg));
        vec![
            Box::new(DealDamage {
                actor_id: target_id,
                amount: dmg,
                damage_type: DamageType::Psychic,
            }),
            Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Baned,
                timer: ConditionTimer::Rounds(1),
            }),
        ]
    }
}

pub static MIND_SLIVER: LazyLock<MindSliver> = LazyLock::new(|| MindSliver {});

/// Blur — level-2 illusion, self-buff, concentration. Attacks against the
/// caster have disadvantage while the spell is up (handled by
/// `compute_attack_mode` via the Blurred condition). Drops on the usual
/// concentration triggers; cleanup clears the Blurred flag.
pub struct Blur {}

impl Action for Blur {
    fn name(&self) -> &str {
        "blur"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["blur-spell"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(2)
    }
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        self_concentration_buff_effects(
            caster_id,
            "Blur",
            Condition::Blurred,
            ConditionTimer::Rounds(10),
        )
    }
}

pub static BLUR: LazyLock<Blur> = LazyLock::new(|| Blur {});

/// Haste — level-3 transmutation, concentration. Target one willing
/// ally (or self with no args): they gain +2 AC, advantage on DEX saves,
/// and double walking speed for up to 10 rounds. We don't model the
/// extra-Action rider (would require a second Action slot the engine
/// doesn't currently track); the AC + DEX save + speed half is the
/// load-bearing part for repositioning support casters.
///
/// Targeting is `SingleActor` (the AI typically buffs the lead melee
/// attacker). Cleared cleanly when concentration drops.
pub struct Haste {}

impl Action for Haste {
    fn name(&self) -> &str {
        "haste"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ha"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 30 ft = 12 tile gap.
        Some(12)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(3)
    }
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        vec![
            Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Hasted,
                timer: ConditionTimer::Rounds(10),
            }),
            Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions(
                    "Haste",
                    vec![(target_id, Condition::Hasted)],
                ),
            }),
        ]
    }
}

pub static HASTE: LazyLock<Haste> = LazyLock::new(|| Haste {});

/// Slow — level-3 transmutation, concentration. Target a 40-ft cube;
/// every enemy within takes a WIS save vs the caster's spell DC. On
/// fail: Slowed for up to 10 rounds (−2 AC, halved speed, disadvantage
/// on DEX saves; we skip the action-economy half of the 5e effect to
/// keep AI behavior predictable). Hostile-only — allies in the cube
/// are spared by the caster-team filter.
///
/// Like Faerie Fire, this picks the burst origin via SinglePoint and
/// iterates the radius itself so the friendly-fire filter can run.
pub struct Slow {}

impl Action for Slow {
    fn name(&self) -> &str {
        "slow"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["sl"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 4 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 120 ft = 48 tiles.
        Some(48)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(3)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::util::{footprint_chebyshev, get_tiles_from_size};

        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let caster_team = caster.team();
        let dc = caster.spell_save_dc(AbilityScoreType::Intelligence);
        const RADIUS: isize = 4;
        const MAX_TARGETS: usize = 6;

        // Enumerate enemy actors in the burst, sort by id for determinism,
        // cap at 6 targets per RAW.
        let mut victims: Vec<usize> = encounter
            .sorted_actor_ids()
            .into_iter()
            .filter(|id| {
                let Some(t) = encounter.actors.get(id) else {
                    return false;
                };
                if !t.is_combat_active() || t.team() == caster_team {
                    return false;
                }
                let dist = footprint_chebyshev(
                    t.location(),
                    get_tiles_from_size(t.size()),
                    point,
                    1,
                );
                dist <= RADIUS
            })
            .collect();
        victims.truncate(MAX_TARGETS);

        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        let mut applied: Vec<(usize, Condition)> = Vec::new();
        for tid in victims {
            let save = encounter.roll_save_against_caster(tid, AbilityScoreType::Wisdom, dc, caster_id);
            if save.passed() {
                continue;
            }
            effects.push(Box::new(ApplyCondition {
                actor_id: tid,
                condition: Condition::Slowed,
                timer: ConditionTimer::Rounds(10),
            }));
            applied.push((tid, Condition::Slowed));
        }
        if !applied.is_empty() {
            effects.push(Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions("Slow", applied),
            }));
        }
        effects
    }
}

pub static SLOW: LazyLock<Slow> = LazyLock::new(|| Slow {});

/// Cone of Cold — level-5 evocation. A 60-ft cone of frigid air from
/// the caster: 8d8 cold damage, CON save for half. We approximate the
/// cone with a burst of radius 6 centered on the target tile (RAW is a
/// 60-ft cone — the engine doesn't yet model directional cones, so a
/// generous radius approximates the area). Damage is rolled once and
/// shared via `resolve_burst_save_damage`.
pub struct ConeOfCold {}

impl Action for ConeOfCold {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Evocation)
    }
    fn name(&self) -> &str {
        "cone of cold"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["coc", "cone"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 6 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // Self-cone, but we cap range at the cone reach (60 ft = 24 tiles)
        // so the picker doesn't drop pins on the far side of the map.
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Cold]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(5)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.best_spell_save_dc([
            // Arcane / primal / pact list all reach this spell, and the
            // 5e save DC is the *caster's* spellcasting ability — the
            // wizard's INT, the sorcerer's and warlock's CHA, the
            // druid's and the Four Elements monk's WIS. Anchoring on
            // INT alone quietly handed every non-wizard carrier a DC
            // computed off a stat their class never invests in.
            AbilityScoreType::Intelligence,
            AbilityScoreType::Charisma,
            AbilityScoreType::Wisdom,
        ]);
        // Empowered Spell metamagic routes through the same chokepoint
        // as Fireball / Lightning Bolt — the 8d8 cone is the single
        // pool the sorcerer's CHA-mod reroll applies to.
        let raw = encounter.roll_empowered_sum(caster_id, 8, 8);
        encounter.log(format!(
            "  cone of cold: 8d8({}) = {} cold area",
            raw, raw
        ));
        crate::actions::action_template::resolve_burst_save_damage(
            encounter,
            caster_id,
            point,
            6,
            AbilityScoreType::Constitution,
            dc,
            raw,
            DamageType::Cold,
        )
    }
}

pub static CONE_OF_COLD: LazyLock<ConeOfCold> = LazyLock::new(|| ConeOfCold {});

/// Mass Cure Wounds — cleric level-5 action heal. Up to six creatures
/// in a 30-ft (12-tile-gap) sphere around a target point regain
/// `3d8 + WIS` HP. Unlike Mass Healing Word (bonus action, level-3,
/// 1d4 die), this is the cleric's emergency-button heal: bigger dice,
/// bigger slot, full Action.
pub struct MassCureWounds {}

impl Action for MassCureWounds {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Evocation)
    }
    fn name(&self) -> &str {
        "mass cure wounds"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["mcw", "mass-cure"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        // Pick a tile; we sweep that point's burst for allies.
        TargetingSchema::SinglePoint
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60 ft = 24 tiles.
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn is_heal(&self) -> bool {
        true
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(5)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::util::{footprint_chebyshev, get_tiles_from_size};

        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let caster_team = caster.team();
        let wis_mod = caster.ability_modifier(AbilityScoreType::Wisdom);
        // Mass Cure Wounds is fixed at the level-5 slot cost. Route the
        // level through the Disciple of Life bonus for a Life Cleric's
        // +7 per target on top of the shared roll. Snapshot before the
        // (mutable) roll.
        let bonus = crate::actions::class_features::disciple_of_life_bonus(caster, 5);
        const RADIUS: isize = 3;
        const MAX_TARGETS: usize = 6;
        // Pick allies inside the burst, sorted by current HP ascending so
        // the lowest-HP allies get healed first if we exceed the cap.
        let mut candidates: Vec<(u32, usize)> = encounter
            .actors
            .iter()
            .filter_map(|(id, a)| {
                if a.team() != caster_team {
                    return None;
                }
                if !a.is_combat_active() && !a.is_dying() {
                    return None;
                }
                let dist = footprint_chebyshev(
                    a.location(),
                    get_tiles_from_size(a.size()),
                    point,
                    1,
                );
                if dist > RADIUS {
                    return None;
                }
                Some((a.hitpoints(), *id))
            })
            .collect();
        candidates.sort_unstable();
        candidates.truncate(MAX_TARGETS);
        // 5e Grave Domain Cleric **Circle of Mortality** — swap the
        // shared 3d8 roll for its max face-value (24) if the caster
        // holds the tag AND any picked target is at 0 HP. Same
        // shared-roll-max coalescing shape the sibling Mass Healing
        // Word site uses — see that call site for the per-die
        // interpretation on a shared-dice mass heal. Snapshot before
        // the (mutable) roll so the burst-selection borrow drops.
        let dice = Dice::new(3, 8);
        let use_max = caster
            .has_passive_feature(crate::actions::class_features::CIRCLE_OF_MORTALITY_TAG)
            && candidates
                .iter()
                .any(|(hp, _)| *hp == 0);
        let raw = if use_max {
            dice.max_roll() as i32
        } else {
            encounter.roll(&dice) as i32
        };
        let base = (raw + wis_mod).max(1) as u32;
        let amount = base + bonus;
        encounter.log(format!(
            "  mass cure wounds: {}({}){}{:+}{} = {} HP each",
            dice,
            raw,
            crate::actions::class_features::circle_of_mortality_log_suffix(use_max),
            wis_mod,
            crate::actions::class_features::disciple_of_life_log_suffix(bonus),
            amount
        ));
        candidates
            .into_iter()
            .map(|(_, id)| {
                Box::new(Heal {
                    actor_id: id,
                    amount,
                }) as Box<dyn ApplicableSideEffect>
            })
            .collect()
    }
}

pub static MASS_CURE_WOUNDS: LazyLock<MassCureWounds> = LazyLock::new(|| MassCureWounds {});

/// Stinking Cloud — level-3 conjuration, concentration. A 20-ft sphere
/// of yellow vapor at a point; every creature inside makes a CON save
/// vs the caster's spell DC or becomes Incapacitated until the start
/// of their next turn (RAW: lose your action and your bonus action).
/// Re-rolls happen each round as the cloud lingers; we model that as
/// an `UntilStartOfNextTurn` timer, which gives the failed save a
/// one-round impact and lets the spell hit again next round if the
/// caster sustains concentration.
///
/// Unlike Fireball / Cone of Cold this is non-damaging — it doesn't
/// trigger concentration saves, it just shuts down enemy turns.
pub struct StinkingCloud {}

impl Action for StinkingCloud {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Conjuration)
    }
    fn name(&self) -> &str {
        "stinking cloud"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["sc-spell", "stink"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 2 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 90 ft = 36 tiles.
        Some(36)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(3)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spell_save_dc(AbilityScoreType::Intelligence);
        const RADIUS: isize = 2;

        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        let mut applied: Vec<(usize, Condition)> = Vec::new();
        for tid in encounter.enemy_burst_targets(caster_id, point, RADIUS) {
            let save = encounter.roll_save_against_caster(tid, AbilityScoreType::Constitution, dc, caster_id);
            if save.passed() {
                continue;
            }
            effects.push(Box::new(ApplyCondition {
                actor_id: tid,
                condition: Condition::Incapacitated,
                timer: ConditionTimer::UntilStartOfNextTurn,
            }));
            applied.push((tid, Condition::Incapacitated));
        }
        // Even with no failed saves, we still start concentration so the
        // cloud lingers — but only when at least one enemy is inside the
        // burst (otherwise the cast was wasted). Without applied targets
        // there's nothing to clear on concentration drop, but we'd want
        // re-rolls each round if we modeled cloud persistence. Today the
        // engine doesn't tick area effects across rounds; install
        // concentration only when at least one target was caught so
        // dropping is a clean no-op when the wind blows over.
        if !applied.is_empty() {
            effects.push(Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions("Stinking Cloud", applied),
            }));
        }
        effects
    }
}

pub static STINKING_CLOUD: LazyLock<StinkingCloud> = LazyLock::new(|| StinkingCloud {});

/// True Strike — divination cantrip. Targets a creature within 30 ft;
/// the caster gains advantage on their next attack roll against that
/// target before the end of their next turn. We model the rider by
/// applying the existing `Helped` condition to the caster — that's the
/// same one-shot advantage hook the Help action installs (consumed on
/// next attack, cleared after one swing). Single-target only.
pub struct TrueStrike {}

impl Action for TrueStrike {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Divination)
    }
    fn name(&self) -> &str {
        "true strike"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ts", "true"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 30 ft = 12 tiles.
        Some(12)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn deals_damage(&self) -> bool {
        false
    }
    // Cantrip — uses the default `cost()` (single Action, no spell slot).
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        // The Helped condition gives one-shot advantage on the next
        // attack made by the holder. RAW True Strike confers advantage
        // only against the targeted creature; we approximate by giving
        // a generic advantage flag since the engine's Helped condition
        // is consumed on the first attack regardless of target.
        vec![Box::new(ApplyCondition {
            actor_id: caster_id,
            condition: Condition::Helped,
            timer: ConditionTimer::UntilStartOfNextTurn,
        })]
    }
}

pub static TRUE_STRIKE: LazyLock<TrueStrike> = LazyLock::new(|| TrueStrike {});

/// Dispel Magic — level-3 abjuration, action. Touch range in 5e RAW is
/// 120 ft; we use 24 tiles. Drops the target's concentration outright;
/// if the target wasn't concentrating, strips one beneficial buff
/// instead (the engine picks deterministically — see `DispelMagicOn`).
/// No save: the spell auto-succeeds against effects from a slot of level
/// ≤ the cast level (which is the only level we track today). No damage.
pub struct DispelMagic {}

impl Action for DispelMagic {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Abjuration)
    }
    fn name(&self) -> &str {
        "dispel magic"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["dm", "dispel"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 120 ft = 48 tiles.
        Some(48)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn is_harmful(&self) -> bool {
        // Dispel against a friendly buffed creature is technically harmful
        // (it removes their buff), but the AI's harmful-action picker uses
        // this as "should I target an enemy?" — Dispel is enemy-facing
        // when targeting concentrators, so true matches the intended use.
        true
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(3)
    }
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        _caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::side_effects::DispelMagicOn;
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        vec![Box::new(DispelMagicOn { target_id })]
    }
}

pub static DISPEL_MAGIC: LazyLock<DispelMagic> = LazyLock::new(|| DispelMagic {});

/// Greater Invisibility — level-4 illusion, concentration. Functionally
/// the same as Invisibility (target gains the Invisible condition and
/// attacks against them have disadvantage, while their attacks have
/// advantage), but **does NOT drop on attack**. The plain Invisibility
/// spell's break-on-attack rider lives in
/// `EncounterInstance::clear_attack_advantage_riders`, which compares
/// the active concentration name against `"Invisibility"`; Greater
/// Invisibility uses a distinct concentration name so that hook leaves
/// it alone — the target stays invisible until concentration drops.
pub struct GreaterInvisibility {}

impl Action for GreaterInvisibility {
    fn name(&self) -> &str {
        "greater invisibility"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ginv", "greater-invis"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // Touch — 1 tile.
        Some(1)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(4)
    }
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        // 10-round concentration — same as the rest of our buff-spells.
        vec![
            Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Invisible,
                timer: ConditionTimer::Rounds(10),
            }),
            Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions(
                    "Greater Invisibility",
                    vec![(target_id, Condition::Invisible)],
                ),
            }),
        ]
    }
}

pub static GREATER_INVISIBILITY: LazyLock<GreaterInvisibility> =
    LazyLock::new(|| GreaterInvisibility {});

/// Ice Storm — level-4 evocation. 20-ft radius cylinder (we use a tile
/// burst, radius 4). Every creature in the area makes a DEX save vs the
/// caster's INT-based DC: 2d8 bludgeoning + 4d6 cold on a fail, half on a
/// success. Two damage types means resistance / immunity has to apply
/// twice to halve / null the full hit; the dual lane is what makes the
/// spell distinct from Fireball / Lightning Bolt at the same level slot.
pub struct IceStorm {}

impl Action for IceStorm {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Evocation)
    }
    fn name(&self) -> &str {
        "ice storm"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["is", "icestorm"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 4 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 300 ft RAW; we cap to a map-realistic 48 tiles (120 ft).
        Some(48)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Bludgeoning, DamageType::Cold]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(4)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spellcasting_save_dc();
        let bludg = encounter.roll(&Dice::new(2, 8));
        let cold = encounter.roll(&Dice::new(4, 6));
        encounter.log(format!(
            "  ice storm: 2d8({}) bludgeoning + 4d6({}) cold area",
            bludg, cold
        ));
        // Two passes through resolve_burst_save_damage so each damage
        // type interacts with target resistance / immunity independently.
        // The save is rolled once per pass — we accept the small RNG
        // cost (two save rolls per target) in exchange for keeping the
        // helper signature simple.
        let mut all = crate::actions::action_template::resolve_burst_save_damage(
            encounter,
            caster_id,
            point,
            4,
            AbilityScoreType::Dexterity,
            dc,
            bludg,
            DamageType::Bludgeoning,
        );
        all.extend(crate::actions::action_template::resolve_burst_save_damage(
            encounter,
            caster_id,
            point,
            4,
            AbilityScoreType::Dexterity,
            dc,
            cold,
            DamageType::Cold,
        ));
        all
    }
}

pub static ICE_STORM: LazyLock<IceStorm> = LazyLock::new(|| IceStorm {});

/// Death Ward — level-4 abjuration, action, touch. Applies the
/// `DeathWarded` condition to one ally. The first time the holder
/// would drop to 0 HP, they instead drop to 1 HP and the ward clears
/// (engine hook lives in `ActorInstance::take_damage`). No
/// concentration — it's a fire-and-forget hard save.
pub struct DeathWard {}

impl Action for DeathWard {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Abjuration)
    }
    fn name(&self) -> &str {
        "death ward"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["dw", "ward"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // Touch.
        Some(1)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(4)
    }
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        _caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        // 5e RAW: 8 hour duration. We cap at a long Rounds timer so the
        // condition has a definite expiry even if combat drags on.
        vec![Box::new(ApplyCondition {
            actor_id: target_id,
            condition: Condition::DeathWarded,
            timer: ConditionTimer::Rounds(100),
        })]
    }
}

pub static DEATH_WARD: LazyLock<DeathWard> = LazyLock::new(|| DeathWard {});

/// Revivify — level-3 necromancy, action, touch. Brings a Dying actor
/// (rolling death saves at 0 HP) back at 1 HP. Stabilized actors are
/// already alive at 0 HP and don't need this; vanilla Heal can pick
/// them back up. Dead actors are gone from the table by the time the
/// spell could resolve, so we don't try to chase them.
pub struct Revivify {}

impl Action for Revivify {
    fn name(&self) -> &str {
        "revivify"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["rev", "revive"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // Touch.
        Some(1)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn is_heal(&self) -> bool {
        true
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        _caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // Only valid against a Dying ally — the spell shouldn't be
        // wasted on healthy targets or actors whose status doesn't need
        // a revive (Stable / Active actors heal via normal spells).
        let Some(target_id) = first_target_id(target_ids) else {
            return false;
        };
        encounter
            .actors
            .get(&target_id)
            .is_some_and(|a| a.is_dying())
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(3)
    }
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        _caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::side_effects::ReviveDying;
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        vec![Box::new(ReviveDying {
            actor_id: target_id,
        })]
    }
}

pub static REVIVIFY: LazyLock<Revivify> = LazyLock::new(|| Revivify {});

/// Stoneskin — level-4 abjuration, concentration. Touch. Until the spell
/// ends, the target has resistance to bludgeoning, piercing, and slashing
/// damage. We use the existing `DamageResistant` condition which gives a
/// generic damage-halving effect — close enough to RAW's physical-only
/// resistance for our engine, and the buff drops cleanly when the caster
/// loses concentration. Doesn't stack with creature-template resistance
/// (halving is multiplicative, but we apply DamageResistant once at the
/// take-damage path so re-halving doesn't happen).
pub struct Stoneskin {}

impl Action for Stoneskin {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Abjuration)
    }
    fn name(&self) -> &str {
        "stoneskin"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ss", "stone"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // Touch — 1 tile.
        Some(crate::actions::action_template::MELEE_REACH)
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(4)
    }
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        vec![
            Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::DamageResistant,
                timer: ConditionTimer::Rounds(10),
            }),
            Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions(
                    "Stoneskin",
                    vec![(target_id, Condition::DamageResistant)],
                ),
            }),
        ]
    }
}

pub static STONESKIN: LazyLock<Stoneskin> = LazyLock::new(|| Stoneskin {});

/// Beacon of Hope — level-3 abjuration, concentration. 30-ft radius cube;
/// up to 6 creatures of the caster's choice gain advantage on Wisdom
/// saves + death saves and regain the maximum from any healing for the
/// duration. We model the lasting buff with the existing `Heroic`
/// condition (already grants Frightened immunity; the additional save /
/// max-heal clauses are not yet engine-modeled but the buff icon is
/// useful flavor and stacks cleanly with concentration drop logic).
///
/// Targeting: AoE around a tile; affected = friendly combat-active actors
/// within radius 6 of the point (≈ 30 ft cube).
pub struct BeaconOfHope {}

impl Action for BeaconOfHope {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Abjuration)
    }
    fn name(&self) -> &str {
        "beacon of hope"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["boh", "beacon"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 6 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // Self/30-ft origin. We pick a tile within ~30 ft to anchor the
        // cube — generous reach keeps the spell usable from the back row.
        Some(12)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(3)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        const RADIUS: isize = 6;
        const MAX_TARGETS: usize = 6;

        let mut buffed = encounter.ally_burst_targets(caster_id, point, RADIUS);
        buffed.truncate(MAX_TARGETS);
        if buffed.is_empty() {
            return Vec::new();
        }

        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        let mut applied: Vec<(usize, Condition)> = Vec::new();
        for tid in &buffed {
            effects.push(Box::new(ApplyCondition {
                actor_id: *tid,
                condition: Condition::Heroic,
                timer: ConditionTimer::Rounds(10),
            }));
            applied.push((*tid, Condition::Heroic));
        }
        effects.push(Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData::with_conditions("Beacon of Hope", applied),
        }));
        effects
    }
}

pub static BEACON_OF_HOPE: LazyLock<BeaconOfHope> = LazyLock::new(|| BeaconOfHope {});

/// Cloud of Daggers — level-2 conjuration, concentration. 5-ft cube of
/// whirling daggers; any creature that enters or starts its turn in the
/// area takes 4d4 slashing. We model the instantaneous on-cast hit as a
/// guaranteed 4d4 to every enemy currently inside the burst (radius 1 in
/// our tile-gap math); persistent ticks aren't yet modeled, but the
/// concentration is started so a follow-up cast or drop behaves cleanly.
/// No save — RAW autohits creatures in the area.
pub struct CloudOfDaggers {}

impl Action for CloudOfDaggers {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Conjuration)
    }
    fn name(&self) -> &str {
        "cloud of daggers"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["cod", "daggers"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 1 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60 ft = 24 tiles.
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Slashing]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(2)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        const RADIUS: isize = 1;
        let damage = encounter.roll_empowered_sum(caster_id, 4, 4);
        encounter.log(format!(
            "  cloud of daggers: 4d4({}) = {} slashing",
            damage, damage
        ));

        let targets = encounter.enemy_burst_targets(caster_id, point, RADIUS);
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        let mut conc_conditions: Vec<(usize, Condition)> = Vec::new();
        for tid in targets {
            effects.push(Box::new(DealDamage {
                actor_id: tid,
                amount: damage,
                damage_type: DamageType::Slashing,
            }));
            effects.push(Box::new(ApplyCondition {
                actor_id: tid,
                condition: Condition::CloudOfDaggered,
                timer: ConditionTimer::Rounds(10),
            }));
            conc_conditions.push((tid, Condition::CloudOfDaggered));
        }
        effects.push(Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData::with_conditions("Cloud of Daggers", conc_conditions),
        }));
        effects
    }
}

pub static CLOUD_OF_DAGGERS: LazyLock<CloudOfDaggers> = LazyLock::new(|| CloudOfDaggers {});

/// Witch Bolt — level-1 evocation, concentration. A spell-attack against a
/// single target deals 1d12 lightning on the initial hit. The 5e RAW
/// sustained-damage clause (a free 1d12 each subsequent turn) lands via
/// the `WitchBolted` entry in the central `ROUND_END_DOTS` table — the
/// caster's concentration anchors the condition, so dropping it severs
/// the bolt cleanly.
pub struct WitchBolt {}

impl Action for WitchBolt {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Evocation)
    }
    fn name(&self) -> &str {
        "witch bolt"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["wb", "witch"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 30 ft = 12 tiles.
        Some(12)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Lightning]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(1)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        // Wizards (INT), clerics (WIS) and sorcerers / warlocks (CHA)
        // all carry this spell, so the modifier follows the caster —
        // see `spellcasting_attack_modifier`. Previously an open-coded
        // max of INT and WIS, which left every CHA caster on the list
        // shooting off a stat they never invested in.
        let attack_bonus = caster.spellcasting_attack_modifier();
        let mut effects = spell_attack(
            encounter,
            caster_id,
            target_id,
            "witch bolt",
            attack_bonus,
            Dice::new(1, 12),
            DamageType::Lightning,
            false,
        );
        if !effects.is_empty() {
            effects.push(Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::WitchBolted,
                timer: ConditionTimer::Rounds(10),
            }));
            effects.push(Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions(
                    "Witch Bolt",
                    vec![(target_id, Condition::WitchBolted)],
                ),
            }));
        }
        effects
    }
}

pub static WITCH_BOLT: LazyLock<WitchBolt> = LazyLock::new(|| WitchBolt {});

/// Phantasmal Killer — level-4 illusion, concentration. Target makes a
/// WIS save. On fail: 4d10 psychic and Frightened. On save: nothing
/// (we skip the RAW repeat-save-each-round mechanic; the damage and
/// fright on the first failure carry the encounter weight). No damage
/// on success per RAW (the illusion never lands).
pub struct PhantasmalKiller {}

impl Action for PhantasmalKiller {
    fn name(&self) -> &str {
        "phantasmal killer"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["pk", "phantasm"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 120 ft = 48 tiles.
        Some(48)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Psychic]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(4)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spell_save_dc(AbilityScoreType::Intelligence);
        let save = encounter.roll_save_against_caster(target_id, AbilityScoreType::Wisdom, dc, caster_id);
        if save.passed() {
            return Vec::new();
        }
        let dmg = encounter.roll_empowered_sum(caster_id, 4, 10);
        encounter.log(format!(
            "  phantasmal killer: 4d10({}) = {} psychic",
            dmg, dmg
        ));
        vec![
            Box::new(DealDamage {
                actor_id: target_id,
                amount: dmg,
                damage_type: DamageType::Psychic,
            }),
            Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Frightened,
                timer: ConditionTimer::Rounds(10),
            }),
            Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions(
                    "Phantasmal Killer",
                    vec![(target_id, Condition::Frightened)],
                ),
            }),
        ]
    }
}

pub static PHANTASMAL_KILLER: LazyLock<PhantasmalKiller> =
    LazyLock::new(|| PhantasmalKiller {});

/// Banishment — level-4 abjuration, concentration. Target makes a CHA
/// save. On fail, banished to a harmless demiplane for the duration —
/// we model as Incapacitated (cannot take actions or reactions) for
/// the duration since the engine doesn't yet model off-board status.
/// Concentration tracks the lock so dropping it ends the banishment.
pub struct Banishment {}

impl Action for Banishment {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Abjuration)
    }
    fn name(&self) -> &str {
        "banishment"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["banish", "banishspell"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60 ft = 24 tiles.
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(4)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        // Use the caster's own spellcasting ability (INT for wizards,
        // CHA for warlocks) so the DC scales with whoever is firing it.
        let dc = caster.spellcasting_save_dc();
        // Caster-aware save so Heightened Spell can force disadvantage.
        let save =
            encounter.roll_save_against_caster(target_id, AbilityScoreType::Charisma, dc, caster_id);
        if save.passed() {
            return Vec::new();
        }
        encounter.log("  banishment: target is banished from the field");
        vec![
            Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Incapacitated,
                timer: ConditionTimer::Rounds(10),
            }),
            Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions(
                    "Banishment",
                    vec![(target_id, Condition::Incapacitated)],
                ),
            }),
        ]
    }
}

pub static BANISHMENT: LazyLock<Banishment> = LazyLock::new(|| Banishment {});

/// Tasha's Hideous Laughter — level-1 enchantment, concentration. WIS
/// save vs DC. On fail, target falls Prone in fits of laughter and is
/// Incapacitated for the duration. Creatures with Intelligence ≤ 4 are
/// immune (we don't gate this — most enemies in our pool meet the
/// threshold, and the few low-INT ones are usually charm-immune anyway
/// via their template). Concentration tracks both conditions for clean
/// teardown.
pub struct TashasHideousLaughter {}

impl Action for TashasHideousLaughter {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Enchantment)
    }
    fn name(&self) -> &str {
        "tasha's hideous laughter"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["thl", "laughter", "hideous laughter"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 30 ft = 12 tiles.
        Some(12)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(1)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spellcasting_save_dc();
        let save = encounter.roll_save_against_caster(target_id, AbilityScoreType::Wisdom, dc, caster_id);
        if save.passed() {
            return Vec::new();
        }
        encounter.log("  hideous laughter: target collapses in fits");
        vec![
            Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Prone,
                timer: ConditionTimer::Rounds(10),
            }),
            Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Incapacitated,
                timer: ConditionTimer::Rounds(10),
            }),
            Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions(
                    "Tasha's Hideous Laughter",
                    vec![
                        (target_id, Condition::Prone),
                        (target_id, Condition::Incapacitated),
                    ],
                ),
            }),
        ]
    }
}

pub static TASHAS_HIDEOUS_LAUGHTER: LazyLock<TashasHideousLaughter> =
    LazyLock::new(|| TashasHideousLaughter {});

/// Heal — level-6 evocation, action, 60 ft range. Restores 70 HP to a
/// single creature and ends Blinded, Deafened, Poisoned (5e RAW), and
/// clears any one Frightened / Charmed via condition cleanse. We pull
/// out a few key debuffs after the heal so it's not just a giant HP
/// patch — the spell is supposed to be a swiss-army-knife emergency
/// button.
pub struct HealSpellHigh {}

impl Action for HealSpellHigh {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Evocation)
    }
    fn name(&self) -> &str {
        "heal"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["heal6", "high-heal"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60 ft = 24 tiles.
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn is_heal(&self) -> bool {
        true
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(6)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::side_effects::RemoveCondition;
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        // Baseline heal is a flat 70 HP; a Life Cleric adds `2 + 6 = 8`
        // via Disciple of Life. Snapshot the caster before the log site
        // so the mutable borrow on `encounter` for `log(...)` doesn't
        // collide with the caster read.
        let bonus = encounter
            .actors
            .get(&caster_id)
            .map(|a| crate::actions::class_features::disciple_of_life_bonus(a, 6))
            .unwrap_or(0);
        let amount = 70 + bonus;
        // Only log if the bonus fired — the base 70 HP heal is already
        // implicit in the Heal side-effect's own "heals N HP" line.
        if bonus > 0 {
            encounter.log(format!(
                "  heal: 70{} = {} HP",
                crate::actions::class_features::disciple_of_life_log_suffix(bonus),
                amount
            ));
        }
        vec![
            Box::new(Heal {
                actor_id: target_id,
                amount,
            }),
            Box::new(RemoveCondition {
                actor_id: target_id,
                condition: Condition::Blinded,
            }),
            Box::new(RemoveCondition {
                actor_id: target_id,
                condition: Condition::Deafened,
            }),
            Box::new(RemoveCondition {
                actor_id: target_id,
                condition: Condition::Poisoned,
            }),
        ]
    }
}

pub static HEAL_SPELL_HIGH: LazyLock<HealSpellHigh> = LazyLock::new(|| HealSpellHigh {});

/// Disintegrate — level-6 transmutation. DEX save vs the caster's spell
/// save DC: on fail 10d6+40 force; on save, nothing. The damage type is
/// Force (rarely resisted in our pool), so a hit is essentially
/// guaranteed to register as raw damage. No save-half.
pub struct Disintegrate {}

impl Action for Disintegrate {
    fn name(&self) -> &str {
        "disintegrate"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["dsg", "disint"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60 ft = 24 tiles.
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Force]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(6)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spellcasting_save_dc();
        let save = encounter.roll_save_against_caster(target_id, AbilityScoreType::Dexterity, dc, caster_id);
        if save.passed() {
            return Vec::new();
        }
        // Empowered Spell metamagic routes through the same chokepoint
        // as the AoE blasters — Disintegrate is the sorcerer's apex
        // single-target nuke and benefits most from rerolled 1s/2s.
        let dmg = encounter.roll_empowered_sum(caster_id, 10, 6) + 40;
        encounter.log(format!(
            "  disintegrate: 10d6+40({}) = {} force",
            dmg, dmg
        ));
        vec![Box::new(DealDamage {
            actor_id: target_id,
            amount: dmg,
            damage_type: DamageType::Force,
        })]
    }
}

pub static DISINTEGRATE: LazyLock<Disintegrate> = LazyLock::new(|| Disintegrate {});

/// Finger of Death — level-7 necromancy. CON save vs the caster's DC.
/// 7d8+30 necrotic on fail; half on success. We follow the half-on-save
/// pattern that big single-target damage spells use (Disintegrate-style
/// no-save-half would be too lethal at this level). The slain-target
/// raising-as-zombie clause from RAW isn't modeled.
pub struct FingerOfDeath {}

impl Action for FingerOfDeath {
    fn name(&self) -> &str {
        "finger of death"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["fod", "fingerdeath"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60 ft = 24 tiles.
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Necrotic]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(7)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spellcasting_save_dc();
        let save = encounter.roll_save_against_caster(target_id, AbilityScoreType::Constitution, dc, caster_id);
        let dmg_full = encounter.roll_empowered_sum(caster_id, 7, 8) + 30;
        let dmg = if save.passed() { dmg_full / 2 } else { dmg_full };
        encounter.log(format!(
            "  finger of death: 7d8+30({}) = {} necrotic",
            dmg_full, dmg
        ));
        if dmg == 0 {
            return Vec::new();
        }
        vec![Box::new(DealDamage {
            actor_id: target_id,
            amount: dmg,
            damage_type: DamageType::Necrotic,
        })]
    }
}

pub static FINGER_OF_DEATH: LazyLock<FingerOfDeath> = LazyLock::new(|| FingerOfDeath {});

/// Power Word Stun — level-8 enchantment. If the target has 150 HP or
/// fewer, they are Stunned (no save) for 10 rounds. If they have more,
/// nothing happens. Hard-cap means the spell is a clean executioner
/// against weakened bosses. No damage.
pub struct PowerWordStun {}

impl Action for PowerWordStun {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Enchantment)
    }
    fn name(&self) -> &str {
        "power word stun"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["pws", "powerstun"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60 ft = 24 tiles.
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(8)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        _caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(target) = encounter.actors.get(&target_id) else {
            return Vec::new();
        };
        if target.hitpoints() > 150 {
            encounter.log("  power word stun: target too healthy — no effect");
            return Vec::new();
        }
        encounter.log("  power word stun: target locks up");
        vec![Box::new(ApplyCondition {
            actor_id: target_id,
            condition: Condition::Stunned,
            timer: ConditionTimer::Rounds(10),
        })]
    }
}

pub static POWER_WORD_STUN: LazyLock<PowerWordStun> = LazyLock::new(|| PowerWordStun {});

/// Synaptic Static — level-5 enchantment. 20-ft radius burst (radius 4).
/// Every creature in area makes an INT save against the caster's spell
/// save DC: 8d6 psychic on fail, half on save. Fails also leave the
/// target Baned (–2 to attacks and saves for 1 round) — the load-bearing
/// rider that justifies it as a control spell, not just damage.
pub struct SynapticStatic {}

impl Action for SynapticStatic {
    fn name(&self) -> &str {
        "synaptic static"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["synaptic", "static"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 4 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 120 ft = 48 tiles.
        Some(48)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Psychic]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(5)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::util::{footprint_chebyshev, get_tiles_from_size};

        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spellcasting_save_dc();
        const RADIUS: isize = 4;
        let full = encounter.roll_empowered_sum(caster_id, 8, 6);
        encounter.log(format!(
            "  synaptic static: 8d6({}) = {} psychic (each)",
            full, full
        ));

        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        for tid in encounter.sorted_actor_ids() {
            let Some(t) = encounter.actors.get(&tid) else {
                continue;
            };
            if tid == caster_id || !t.is_combat_active() {
                continue;
            }
            let dist = footprint_chebyshev(
                t.location(),
                get_tiles_from_size(t.size()),
                point,
                1,
            );
            if dist > RADIUS {
                continue;
            }
            let save = encounter.roll_save_against_caster(tid, AbilityScoreType::Intelligence, dc, caster_id);
            let dmg = if save.passed() { full / 2 } else { full };
            if dmg > 0 {
                effects.push(Box::new(DealDamage {
                    actor_id: tid,
                    amount: dmg,
                    damage_type: DamageType::Psychic,
                }));
            }
            // Baned only on a fail — the muddled rider that lasts one
            // round per the spell description.
            if !save.passed() {
                effects.push(Box::new(ApplyCondition {
                    actor_id: tid,
                    condition: Condition::Baned,
                    timer: ConditionTimer::Rounds(1),
                }));
            }
        }
        effects
    }
}

pub static SYNAPTIC_STATIC: LazyLock<SynapticStatic> = LazyLock::new(|| SynapticStatic {});

/// Crown of Madness — level-2 enchantment, concentration. WIS save vs
/// the caster's spell DC. On fail, target is Charmed (mechanically: their
/// own actions become less useful through the Charmed flag, and the
/// charmer-target hostile-action gate prevents them from attacking the
/// caster). Humanoids only in 5e RAW; we don't enforce the type gate
/// since condition immunities already cover the immune-to-charm cases.
pub struct CrownOfMadness {}

impl Action for CrownOfMadness {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Enchantment)
    }
    fn name(&self) -> &str {
        "crown of madness"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["com", "crown"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 120 ft = 48 tiles.
        Some(48)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(2)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spell_save_dc(AbilityScoreType::Intelligence);
        let save = encounter.roll_save_against_caster(target_id, AbilityScoreType::Wisdom, dc, caster_id);
        if save.passed() {
            return Vec::new();
        }
        encounter.log("  crown of madness: target falls under the caster's sway");
        let mut out = install_charmed_by(target_id, caster_id, ConditionTimer::Rounds(10));
        out.push(Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData::with_conditions(
                "Crown of Madness",
                vec![(target_id, Condition::Charmed)],
            ),
        }));
        out
    }
}

pub static CROWN_OF_MADNESS: LazyLock<CrownOfMadness> = LazyLock::new(|| CrownOfMadness {});

/// Word of Radiance — cleric cantrip. 5-ft burst centered on the caster
/// (radius 1 in tile-gap). Every creature in the area except the caster
/// makes a CON save vs the caster's WIS-based spell DC: 1d6 radiant on
/// fail, nothing on success. Pure cantrip — no spell slot.
pub struct WordOfRadiance {}

impl Action for WordOfRadiance {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Evocation)
    }
    fn name(&self) -> &str {
        "word of radiance"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["wor", "radiance"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn requires_los(&self) -> bool {
        false
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Radiant]
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spell_save_dc(AbilityScoreType::Wisdom);
        let n = crate::engine::util::cantrip_dice_count(caster.level());
        let center = caster.location();
        // RAW: "each creature of your choice that you can see within 5
        // feet of you must succeed on a Constitution saving throw or
        // take 1d6 radiant damage." Two clauses the previous
        // implementation got wrong, both fixed by routing through the
        // shared resolver instead of hand-rolling the burst:
        //
        //   - **"of your choice"** makes this enemy-only. It was going
        //     through the neutral resolver, so a cleric standing in
        //     their own front line irradiated the party alongside the
        //     enemy — the one cleric cantrip that punished being where
        //     a cleric is supposed to stand.
        //   - **"or take"** is save-for-nothing, not save-for-half. It
        //     was folding through `HalfOnSave`, which is the levelled-
        //     AoE convention; cantrips canonically miss clean, which is
        //     exactly the distinction `enemy_burst_save_only` exists to
        //     draw.
        //
        // The shared resolver also rolls through `roll_empowered_sum`,
        // so Empowered Spell / Empowered Evocation / Potent Spellcasting
        // reach this cantrip for the first time.
        let (effects, _saves) = enemy_burst_save_only(
            encounter,
            caster_id,
            center,
            1,
            AbilityScoreType::Constitution,
            dc,
            Dice::new(n, 6),
            DamageType::Radiant,
            "word of radiance",
        );
        effects
    }
}

pub static WORD_OF_RADIANCE: LazyLock<WordOfRadiance> = LazyLock::new(|| WordOfRadiance {});

/// Calm Emotions — level-2 enchantment. 60-ft range, 20-ft radius sphere
/// (radius 4 in tile-gap). Every humanoid in the area makes a CHA save
/// against the caster's spell DC; on fail, the target is suppressed of
/// Charmed and Frightened (5e RAW: "suppress" — we model by removing
/// the conditions, which is the practical equivalent for our model).
/// Concentration in 5e for re-arming the dispelled conditions if it
/// drops — we apply once, no concentration needed for the simple cleanse.
///
/// The spell affects friend and foe alike by RAW; we approximate the
/// caster's intent by aiming the cleanse at every actor in radius,
/// which gives the cleric a tool against enemy fear/charm spells.
pub struct CalmEmotions {}

impl Action for CalmEmotions {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Enchantment)
    }
    fn name(&self) -> &str {
        "calm emotions"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ce", "calm"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 4 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60 ft = 24 tiles.
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(2)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::side_effects::RemoveCondition;
        use crate::engine::util::{footprint_chebyshev, get_tiles_from_size};

        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spell_save_dc(AbilityScoreType::Wisdom);
        const RADIUS: isize = 4;

        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        for tid in encounter.sorted_actor_ids() {
            let Some(t) = encounter.actors.get(&tid) else {
                continue;
            };
            if !t.is_combat_active() {
                continue;
            }
            let dist = footprint_chebyshev(
                t.location(),
                get_tiles_from_size(t.size()),
                point,
                1,
            );
            if dist > RADIUS {
                continue;
            }
            // 5e: no save = no effect. The save is *against* the cleanse
            // (a fey trying to keep its charm). On fail, the charm /
            // frighten is stripped.
            let save = encounter.roll_save_against_caster(tid, AbilityScoreType::Charisma, dc, caster_id);
            if save.passed() {
                continue;
            }
            effects.push(Box::new(RemoveCondition {
                actor_id: tid,
                condition: Condition::Charmed,
            }));
            effects.push(Box::new(RemoveCondition {
                actor_id: tid,
                condition: Condition::Frightened,
            }));
        }
        effects
    }
}

pub static CALM_EMOTIONS: LazyLock<CalmEmotions> = LazyLock::new(|| CalmEmotions {});

/// Suggestion — level-2 enchantment, concentration. 30-ft range, single
/// target. Target makes a WIS save vs the caster's spell DC; on fail, it
/// is Charmed by the caster for up to 8 hours (we use 10 rounds). The
/// charm enforces the "cannot attack the charmer" gate from action
/// validation, mirroring Charm Person / Crown of Madness.
pub struct Suggestion {}

impl Action for Suggestion {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Enchantment)
    }
    fn name(&self) -> &str {
        "suggestion"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["sug", "suggest"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 30 ft = 12 tiles.
        Some(12)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(2)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spellcasting_save_dc();
        let save = encounter.roll_save_against_caster(target_id, AbilityScoreType::Wisdom, dc, caster_id);
        if save.passed() {
            return Vec::new();
        }
        encounter.log("  suggestion: target's mind is bent to the caster's words");
        let mut out = install_charmed_by(target_id, caster_id, ConditionTimer::Rounds(10));
        out.push(Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData::with_conditions(
                "Suggestion",
                vec![(target_id, Condition::Charmed)],
            ),
        }));
        out
    }
}

pub static SUGGESTION: LazyLock<Suggestion> = LazyLock::new(|| Suggestion {});

/// Mass Suggestion — level-6 enchantment. 60-ft range, 30-ft radius
/// sphere (radius 6). Up to 12 creatures of the caster's choice in the
/// area each make a WIS save vs the caster's spell DC; failures are
/// Charmed by the caster for an extended duration (we use 10 rounds).
/// Unlike Suggestion this does NOT require concentration (RAW: "for up
/// to 24 hours" — no concentration line), so the caster keeps their
/// concentration slot free for other rope-a-dope effects.
pub struct MassSuggestion {}

impl Action for MassSuggestion {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Enchantment)
    }
    fn name(&self) -> &str {
        "mass suggestion"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["msug", "mass-suggest"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 6 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60 ft = 24 tiles.
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(6)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spell_save_dc(AbilityScoreType::Charisma);
        const RADIUS: isize = 6;
        const MAX_TARGETS: usize = 12;

        // RAW lets the caster pick targets; the burst-targets helper
        // already filters to enemies (the only useful charm victims).
        let candidates = encounter.enemy_burst_targets(caster_id, point, RADIUS);
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        for tid in candidates.into_iter().take(MAX_TARGETS) {
            let save = encounter.roll_save_against_caster(tid, AbilityScoreType::Wisdom, dc, caster_id);
            if save.passed() {
                continue;
            }
            effects.extend(install_condition_with_link(
                Condition::Charmed,
                tid,
                caster_id,
                ConditionTimer::Rounds(10),
            ));
        }
        effects
    }
}

pub static MASS_SUGGESTION: LazyLock<MassSuggestion> = LazyLock::new(|| MassSuggestion {});

/// Sunburst — level-8 evocation. 60-ft range, 60-ft radius sphere of
/// brilliant sunlight (we cap the radius at 12 tile-gap for engine
/// sanity). Every creature in the area makes a CON save vs the caster's
/// spell DC: 12d6 radiant on fail, half on success. Failures are also
/// Blinded for 1 minute (10 rounds). Undead and oozes take the burst as
/// normal; the spell's "bright sunlight" tag isn't engine-modeled.
pub struct Sunburst {}

impl Action for Sunburst {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Evocation)
    }
    fn name(&self) -> &str {
        "sunburst"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["sun", "sunb"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 12 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 150 ft per RAW = 60 tiles. We cap at 40 since the map is
        // typically that wide.
        Some(40)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Radiant]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(8)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::util::{footprint_chebyshev, get_tiles_from_size};

        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spellcasting_save_dc();
        const RADIUS: isize = 12;
        // Empowered Spell metamagic — sorcerer can reroll low dice on
        // the shared sunburst pool. Same hook as Fireball / Cone of Cold.
        let full = encounter.roll_empowered_sum(caster_id, 12, 6);
        encounter.log(format!(
            "  sunburst: 12d6({}) = {} radiant (each)",
            full, full
        ));

        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        for tid in encounter.sorted_actor_ids() {
            let Some(t) = encounter.actors.get(&tid) else {
                continue;
            };
            if tid == caster_id || !t.is_combat_active() {
                continue;
            }
            let dist = footprint_chebyshev(
                t.location(),
                get_tiles_from_size(t.size()),
                point,
                1,
            );
            if dist > RADIUS {
                continue;
            }
            let save = encounter.roll_save_against_caster(tid, AbilityScoreType::Constitution, dc, caster_id);
            let dmg = if save.passed() { full / 2 } else { full };
            if dmg > 0 {
                effects.push(Box::new(DealDamage {
                    actor_id: tid,
                    amount: dmg,
                    damage_type: DamageType::Radiant,
                }));
            }
            if !save.passed() {
                effects.push(Box::new(ApplyCondition {
                    actor_id: tid,
                    condition: Condition::Blinded,
                    timer: ConditionTimer::Rounds(10),
                }));
            }
        }
        effects
    }
}

pub static SUNBURST: LazyLock<Sunburst> = LazyLock::new(|| Sunburst {});

/// Mass Heal — level-9 conjuration, action. A pool of 700 HP is divided
/// among any number of allies within range; each chosen ally regains HP
/// up to the pool. We model this by sorting allies by missing HP (most
/// hurt first) and pouring the pool until it's empty or every ally is
/// topped off. Also ends Blinded, Deafened, Poisoned on each target —
/// mirroring the Heal spell's status cleanse.
pub struct MassHeal {}

impl Action for MassHeal {
    fn name(&self) -> &str {
        "mass heal"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["mh", "mass-heal"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        // Self-targeted: the pool sweeps every ally in line-of-sight.
        TargetingSchema::NoArgs
    }
    fn requires_los(&self) -> bool {
        false
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn is_heal(&self) -> bool {
        true
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(9)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::side_effects::{RemoveCondition, ReviveDying};
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let team = caster.team();
        // Allies who are alive (or merely dying — Mass Heal lifts them
        // out) and not already at max HP.
        let mut candidates: Vec<(i64, usize)> = encounter
            .actors
            .iter()
            .filter_map(|(id, a)| {
                if a.team() != team {
                    return None;
                }
                if !a.is_combat_active() && !a.is_dying() {
                    return None;
                }
                let missing = a.max_hitpoints() as i64 - a.hitpoints() as i64;
                if missing <= 0 {
                    return None;
                }
                // Sort key: most missing HP first (negate so .sort is ascending).
                Some((-missing, *id))
            })
            .collect();
        candidates.sort_unstable();

        encounter.log("  mass heal: a 700-HP pool washes over the caster's allies");
        let mut pool: u32 = 700;
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        for (neg_missing, id) in candidates {
            if pool == 0 {
                break;
            }
            let need = (-neg_missing) as u32;
            let give = need.min(pool);
            pool -= give;
            // Revive first so dying allies are lifted out of Dying (which
            // also clears their auto-Prone) before the bulk heal tops
            // them up. ReviveDying is a no-op for live allies, so the
            // unconditional queue is safe.
            effects.push(Box::new(ReviveDying { actor_id: id }));
            effects.push(Box::new(Heal {
                actor_id: id,
                amount: give,
            }));
            for c in [Condition::Blinded, Condition::Deafened, Condition::Poisoned] {
                effects.push(Box::new(RemoveCondition {
                    actor_id: id,
                    condition: c,
                }));
            }
        }
        effects
    }
}

pub static MASS_HEAL: LazyLock<MassHeal> = LazyLock::new(|| MassHeal {});

/// Power Word Kill — level-9 enchantment. Single target with 100 HP or
/// fewer is killed outright (no save, no attack roll). Targets above
/// 100 HP are unaffected. We model "killed outright" as a direct
/// damage hit of `current_hp` necrotic so the standard death path runs
/// (death save start for PCs that die outright per RAW; instant Dead
/// for monsters). Range 60 ft.
pub struct PowerWordKill {}

impl Action for PowerWordKill {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Enchantment)
    }
    fn name(&self) -> &str {
        "power word kill"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["pwk", "kill"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60 ft = 24 tiles.
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Necrotic]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(9)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        _caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(target) = encounter.actors.get(&target_id) else {
            return Vec::new();
        };
        let hp = target.hitpoints();
        if hp > 100 {
            encounter.log(format!(
                "  power word kill: target has {} HP (>100) \u{2014} unaffected",
                hp
            ));
            return Vec::new();
        }
        encounter.log(format!(
            "  power word kill: target has {} HP \u{2264} 100 \u{2014} struck down",
            hp
        ));
        vec![Box::new(DealDamage {
            actor_id: target_id,
            amount: hp,
            damage_type: DamageType::Necrotic,
        })]
    }
}

pub static POWER_WORD_KILL: LazyLock<PowerWordKill> = LazyLock::new(|| PowerWordKill {});

/// Meteor Swarm — level-9 evocation. 20-ft radius burst (radius 4 in
/// tile-gap; RAW it's four 40-ft spheres, we collapse to one big sphere
/// for engine simplicity). Every creature in the area makes a DEX save
/// vs the caster's spell DC: 20d6 fire + 20d6 bludgeoning on fail, half
/// on save. The two damage rolls share a single save outcome (RAW: one
/// save vs both packets), but they apply independently so resistance to
/// one type (a fire-resistant elemental) still eats the other half.
pub struct MeteorSwarm {}

impl Action for MeteorSwarm {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Evocation)
    }
    fn name(&self) -> &str {
        "meteor swarm"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ms", "meteor"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 4 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 1 mile per RAW; we cap at the map edge (40 tiles).
        Some(40)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Fire, DamageType::Bludgeoning]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(9)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::util::{footprint_chebyshev, get_tiles_from_size};

        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spellcasting_save_dc();
        const RADIUS: isize = 4;
        // Empowered Spell metamagic — RAW says "When you roll damage for
        // a spell, you can reroll a number of the damage dice." Meteor
        // Swarm rolls fire then bludgeoning; our `roll_empowered`
        // consumes the prime on the first call, so the fire pool gets
        // the rerolls and the bludgeoning rolls fall through normally.
        // Consistent with the "once per spell" cap RAW imposes.
        let fire = encounter.roll_empowered_sum(caster_id, 20, 6);
        let bludge = encounter.roll(&Dice::new(20, 6));
        encounter.log(format!(
            "  meteor swarm: 20d6({}) fire + 20d6({}) bludgeoning (each)",
            fire, bludge
        ));

        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        for tid in encounter.sorted_actor_ids() {
            let Some(t) = encounter.actors.get(&tid) else {
                continue;
            };
            if tid == caster_id || !t.is_combat_active() {
                continue;
            }
            let dist = footprint_chebyshev(
                t.location(),
                get_tiles_from_size(t.size()),
                point,
                1,
            );
            if dist > RADIUS {
                continue;
            }
            let save = encounter.roll_save_against_caster(tid, AbilityScoreType::Dexterity, dc, caster_id);
            let (f_dmg, b_dmg) = if save.passed() {
                (fire / 2, bludge / 2)
            } else {
                (fire, bludge)
            };
            if f_dmg > 0 {
                effects.push(Box::new(DealDamage {
                    actor_id: tid,
                    amount: f_dmg,
                    damage_type: DamageType::Fire,
                }));
            }
            if b_dmg > 0 {
                effects.push(Box::new(DealDamage {
                    actor_id: tid,
                    amount: b_dmg,
                    damage_type: DamageType::Bludgeoning,
                }));
            }
        }
        effects
    }
}

pub static METEOR_SWARM: LazyLock<MeteorSwarm> = LazyLock::new(|| MeteorSwarm {});

/// Prayer of Healing — level-2 evocation. Pick up to six allies within
/// 30 ft (12 tiles) of the caster; each regains 2d8 + WIS HP. RAW has a
/// 10-minute cast time (so it's strictly out-of-combat per book); we keep
/// it as a one-action in-combat heal because (a) our encounter loop has
/// no out-of-combat phase and (b) it slots cleanly into the cleric's
/// level-2 healing toolkit between Cure Wounds and Mass Cure Wounds.
/// Same dying-allies-included logic as Mass Cure Wounds — a dying ally
/// would just regain HP from the heal and exit the dying state on the
/// next HP roll, but allowing them as targets lets the cleric stabilize
/// a downed party member in bulk with a single 2nd-level slot.
pub struct PrayerOfHealing {}

impl Action for PrayerOfHealing {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Evocation)
    }
    fn name(&self) -> &str {
        "prayer of healing"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["poh", "prayer"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn is_heal(&self) -> bool {
        true
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(2)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let caster_loc = caster.location();
        let wis_mod = caster.ability_modifier(AbilityScoreType::Wisdom);
        // 30-ft range centered on the caster — reuse the burst helper
        // with the caster's own footprint as the anchor so distance math
        // matches every other ally-burst spell.
        const RADIUS: isize = 6;
        const MAX_TARGETS: usize = 6;
        let raw = encounter.roll(&Dice::new(2, 8)) as i32;
        let amount = (raw + wis_mod).max(1) as u32;
        encounter.log(format!(
            "  prayer of healing: 2d8({}){:+} = {} HP each",
            raw, wis_mod, amount
        ));
        let mut targets = encounter.ally_burst_targets(caster_id, caster_loc, RADIUS);
        targets.truncate(MAX_TARGETS);
        targets
            .into_iter()
            .map(|id| {
                Box::new(Heal {
                    actor_id: id,
                    amount,
                }) as Box<dyn ApplicableSideEffect>
            })
            .collect()
    }
}

pub static PRAYER_OF_HEALING: LazyLock<PrayerOfHealing> = LazyLock::new(|| PrayerOfHealing {});

/// Sunbeam — level-6 evocation, concentration. RAW is a 60-ft line that
/// blinds + damages each creature inside on a failed CON save (6d8
/// radiant on fail, half on save). Modelled as a single-target ranged
/// spell attack for engine simplicity (line targeting isn't yet a schema)
/// — 6d8 radiant on hit, and the target is Blinded for one round.
/// Concentration lets the caster sustain the spell to fire it on
/// subsequent turns (we don't yet model the action-per-turn repeat
/// rider, but the conc slot prevents stacking with other conc spells
/// and clears on damage like every other concentration effect).
pub struct Sunbeam {}

impl Action for Sunbeam {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Evocation)
    }
    fn name(&self) -> &str {
        "sunbeam"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["sun", "beam"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60-ft line ≈ 24 tiles.
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Radiant]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(6)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        // The modifier follows the caster across all three casting
        // abilities — see `spellcasting_attack_modifier`. Previously an
        // open-coded max of INT and WIS only.
        let attack_bonus = caster.spellcasting_attack_modifier();
        let (effs, dealt) = spell_attack_outcome(
            encounter,
            caster_id,
            target_id,
            "sunbeam",
            attack_bonus,
            Dice::new(6, 8),
            0,
            DamageType::Radiant,
            false,
        );
        let mut effects = effs;
        // On hit, target is also Blinded for one round (until start of
        // their next turn) — the sun-flare clause from RAW.
        if dealt > 0 {
            effects.push(Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Blinded,
                timer: ConditionTimer::UntilStartOfNextTurn,
            }));
        }
        effects.push(Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData::new("Sunbeam"),
        }));
        effects
    }
}

pub static SUNBEAM: LazyLock<Sunbeam> = LazyLock::new(|| Sunbeam {});

/// Resurrection — level-7 necromancy. Touch a creature that has been
/// dead no more than a century (in our engine: a Dying actor — Dead
/// actors get removed from `actors` so they can't be targeted). Restores
/// the target to full HP, cures Blinded / Deafened / Poisoned, and
/// strips all conditions that came with the Dying state (Unconscious,
/// Prone). One step beyond Revivify (which restores them to 1 HP) — the
/// cleric pays a 7th-level slot for a full top-up.
pub struct Resurrection {}

impl Action for Resurrection {
    fn name(&self) -> &str {
        "resurrection"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["res", "resurrect"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(crate::actions::action_template::MELEE_REACH)
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn is_heal(&self) -> bool {
        true
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        _caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // Same dying-only gate as Revivify — the spell shouldn't be wasted
        // on healthy targets.
        let Some(target_id) = first_target_id(target_ids) else {
            return false;
        };
        encounter
            .actors
            .get(&target_id)
            .is_some_and(|a| a.is_dying())
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(7)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        _caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::side_effects::{RemoveCondition, ReviveDying};
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let max = encounter
            .actors
            .get(&target_id)
            .map(|a| a.max_hitpoints())
            .unwrap_or(0);
        // Revive first (lifts the Dying state to 1 HP, cleans Prone /
        // Unconscious) and then top up to max with a regular heal —
        // ReviveDying is a no-op for non-Dying actors, so the chained
        // queue stays safe even if the gate above passes a borderline
        // case.
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = vec![
            Box::new(ReviveDying { actor_id: target_id }),
            Box::new(Heal {
                actor_id: target_id,
                amount: max,
            }),
        ];
        for c in [Condition::Blinded, Condition::Deafened, Condition::Poisoned] {
            effects.push(Box::new(RemoveCondition {
                actor_id: target_id,
                condition: c,
            }));
        }
        effects
    }
}

pub static RESURRECTION: LazyLock<Resurrection> = LazyLock::new(|| Resurrection {});

/// Power Word Heal — level-9 evocation. Single touch target regains all
/// HP, then the spell cleanses every captivating / control condition
/// (Charmed, Frightened, Paralyzed, Stunned) and stands them up from
/// Prone. The 5e RAW also lets the target use a reaction to stand and
/// remove the charmed/frightened/paralyzed/stunned riders; we collapse
/// the reaction step into the heal's side effects since we don't yet
/// have a "trigger reaction on heal" hook. Symmetric counterpart to
/// Power Word Kill — top of the heal tree at the same slot cost.
pub struct PowerWordHeal {}

impl Action for PowerWordHeal {
    fn name(&self) -> &str {
        "power word heal"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["pwh", "wordheal"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // Touch.
        Some(crate::actions::action_template::MELEE_REACH)
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn is_heal(&self) -> bool {
        true
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(9)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        _caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::side_effects::{RemoveCondition, ReviveDying};
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let max = encounter
            .actors
            .get(&target_id)
            .map(|a| a.max_hitpoints())
            .unwrap_or(0);
        // ReviveDying first (idempotent for non-Dying actors); then heal
        // to full; then strip the captivating conditions plus Prone (the
        // RAW reaction lets the target stand up).
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = vec![
            Box::new(ReviveDying { actor_id: target_id }),
            Box::new(Heal {
                actor_id: target_id,
                amount: max,
            }),
        ];
        for c in [
            Condition::Charmed,
            Condition::Frightened,
            Condition::Paralyzed,
            Condition::Stunned,
            Condition::Prone,
        ] {
            effects.push(Box::new(RemoveCondition {
                actor_id: target_id,
                condition: c,
            }));
        }
        effects
    }
}

pub static POWER_WORD_HEAL: LazyLock<PowerWordHeal> = LazyLock::new(|| PowerWordHeal {});

/// Dimension Door — level-4 conjuration. The caster teleports up to 500 ft
/// (we cap to 120 tiles / 300 ft for map-realism) to a tile they can see.
/// No opportunity attacks (5e teleports bypass per-step OAs — handled by
/// `TeleportActor`). Distinct from Misty Step's bonus-action / 30-ft form:
/// long range, full Action cost, level-4 slot. RAW lets the caster bring
/// one willing creature along; we model the single-caster variant since
/// the picker UI doesn't have a "two-actor teleport" schema yet.
pub struct DimensionDoor {}

impl Action for DimensionDoor {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Conjuration)
    }
    fn name(&self) -> &str {
        "dimension door"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["dd", "dimensiondoor"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SinglePoint
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 500 ft RAW; capped to 120 tiles for map-scale.
        Some(120)
    }
    fn requires_los(&self) -> bool {
        // RAW: a location you can see, or a location you've visited /
        // describable distance. We require LOS for the simple case.
        true
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(4)
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        let Some(point) = first_target_location(target_locations) else {
            return false;
        };
        encounter.can_move_to(caster_id, point)
    }
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        vec![Box::new(crate::engine::side_effects::TeleportActor {
            actor_id: caster_id,
            dest: point,
        })]
    }
}

pub static DIMENSION_DOOR: LazyLock<DimensionDoor> = LazyLock::new(|| DimensionDoor {});

/// Wall of Fire — level-4 evocation, concentration. The caster picks a
/// tile within 120 ft; every enemy whose footprint touches the chosen
/// point (radius 2 — approximates the 20-ft wall length) takes 5d8 fire
/// damage with no save and gains the Burning condition (DOT: 1d4 fire
/// per round-end until expiry). Friendly creatures inside the burst are
/// skipped — Wall of Fire RAW lets the caster pick which side of the
/// wall burns, so we model the "caster's allies face the cool side"
/// clause by using `enemy_burst_targets`. Concentration: dropping it
/// before the timer expires clears the Burning ride immediately.
pub struct WallOfFire {}

impl Action for WallOfFire {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Evocation)
    }
    fn name(&self) -> &str {
        "wall of fire"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["wof", "firewall"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 2 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 120 ft = 48 tiles.
        Some(48)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Fire]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(4)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let raw = encounter.roll_empowered_sum(caster_id, 5, 8);
        encounter.log(format!(
            "  wall of fire: 5d8({}) = {} fire (enemies only)",
            raw, raw
        ));
        // Enemy-only AoE: friendly walkers don't get caught. Each enemy
        // takes the rolled damage and starts Burning for 3 rounds —
        // matches RAW's "spend a turn near the wall = sustained DOT" feel.
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        let mut tagged: Vec<(usize, Condition)> = Vec::new();
        for id in encounter.enemy_burst_targets(caster_id, point, 2) {
            effects.push(Box::new(DealDamage {
                actor_id: id,
                amount: raw,
                damage_type: DamageType::Fire,
            }));
            effects.push(Box::new(ApplyCondition {
                actor_id: id,
                condition: Condition::Burning,
                timer: ConditionTimer::Rounds(3),
            }));
            tagged.push((id, Condition::Burning));
        }
        effects.push(Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData::with_conditions("Wall of Fire", tagged),
        }));
        effects
    }
}

pub static WALL_OF_FIRE: LazyLock<WallOfFire> = LazyLock::new(|| WallOfFire {});

/// Cloudkill — level-5 conjuration, concentration. A 20-ft radius (radius
/// 4 on the 2.5 ft grid) cloud of yellow-green fog drifts where the
/// caster points. Every creature whose footprint touches the burst makes
/// a CON save vs the caster's INT-based DC: 5d8 poison on a fail, half on
/// a success. RAW the cloud also persists and re-damages over time — we
/// model the immediate hit but skip the per-turn re-damage to keep the
/// concentration plumbing simple. Poison immunity zeros the damage.
pub struct Cloudkill {}

impl Action for Cloudkill {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Conjuration)
    }
    fn name(&self) -> &str {
        "cloudkill"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ck", "poisoncloud"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 4 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 120 ft = 48 tiles.
        Some(48)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Poison]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(5)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spellcasting_save_dc();
        let raw = encounter.roll(&Dice::new(5, 8));
        encounter.log(format!("  cloudkill: 5d8({}) = {} poison area", raw, raw));
        let mut effects = crate::actions::action_template::resolve_burst_save_damage(
            encounter,
            caster_id,
            point,
            4,
            AbilityScoreType::Constitution,
            dc,
            raw,
            DamageType::Poison,
        );
        effects.push(Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData::new("Cloudkill"),
        }));
        effects
    }
}

pub static CLOUDKILL: LazyLock<Cloudkill> = LazyLock::new(|| Cloudkill {});

/// Insect Plague — level-5 conjuration, concentration. A 20-ft radius
/// (radius 4) cloud of biting locusts. Every creature in the cloud makes
/// a CON save vs the caster's WIS-based DC: 4d10 piercing on a fail,
/// half on a success. Pierces resistance for most undead/oozes — but
/// since we route through normal damage modifiers, immunity / resistance
/// applies as usual. Concentration: drop ends the swarm.
pub struct InsectPlague {}

impl Action for InsectPlague {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Conjuration)
    }
    fn name(&self) -> &str {
        "insect plague"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ip", "locusts"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 4 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 300 ft RAW; cap to 96 tiles (240 ft).
        Some(96)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Piercing]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(5)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spell_save_dc(AbilityScoreType::Wisdom);
        let raw = encounter.roll(&Dice::new(4, 10));
        encounter.log(format!(
            "  insect plague: 4d10({}) = {} piercing area",
            raw, raw
        ));
        let mut effects = crate::actions::action_template::resolve_burst_save_damage(
            encounter,
            caster_id,
            point,
            4,
            AbilityScoreType::Constitution,
            dc,
            raw,
            DamageType::Piercing,
        );
        effects.push(Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData::new("Insect Plague"),
        }));
        effects
    }
}

pub static INSECT_PLAGUE: LazyLock<InsectPlague> = LazyLock::new(|| InsectPlague {});

/// Daylight — level-3 evocation. Anchors a 60-ft sphere of bright sunlight
/// to a tile within 120 ft. Every ally inside the radius gets the Daylit
/// condition, which imposes disadvantage on incoming attacks from
/// undead / fiend-flavored enemies (proxied by Necrotic / Poison
/// immunity, like the Warded clause). RAW the spell also dispels magical
/// darkness in the area; we don't model darkness terrain, so the
/// dispel-darkness clause is a no-op today. No concentration, but lasts
/// only ~10 rounds before the timer ticks the condition off each ally.
pub struct Daylight {}

impl Action for Daylight {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Evocation)
    }
    fn name(&self) -> &str {
        "daylight"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["day", "sunlight"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        // Sphere of radius 6 (≈ 30 ft); anchored to a tile within range.
        TargetingSchema::Burst { radius: 6 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60 ft = 24 tiles.
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(3)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        encounter
            .ally_burst_targets(caster_id, point, 6)
            .into_iter()
            .map(|id| {
                Box::new(ApplyCondition {
                    actor_id: id,
                    condition: Condition::Daylit,
                    timer: ConditionTimer::Rounds(10),
                }) as Box<dyn ApplicableSideEffect>
            })
            .collect()
    }
}

pub static DAYLIGHT: LazyLock<Daylight> = LazyLock::new(|| Daylight {});

/// Fire Shield — level-4 evocation. The caster ignites in protective flame
/// for 10 rounds: they gain resistance to cold damage and any creature
/// that hits them with a melee attack within reach takes 2d8 fire damage
/// in retaliation. We model this with the FireShielded condition;
/// resolve_attack reads the condition to fire the reflective damage on
/// melee hits, and the holder also gains the generic DamageResistant
/// flag (which halves cold and most other damage — close enough for our
/// purposes). Self-only by default — RAW limits it to the caster.
pub struct FireShield {}

impl Action for FireShield {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Evocation)
    }
    fn name(&self) -> &str {
        "fire shield"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["fs", "flameshield"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(4)
    }
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        vec![Box::new(ApplyCondition {
            actor_id: caster_id,
            condition: Condition::FireShielded,
            timer: ConditionTimer::Rounds(10),
        })]
    }
}

pub static FIRE_SHIELD: LazyLock<FireShield> = LazyLock::new(|| FireShield {});

/// Sanctuary — level-1 abjuration, bonus action. Wards one ally so any
/// attacker targeting them must succeed on a WIS save vs the caster's
/// spell DC or pick a different target / lose the attack. We model this
/// via the engine-level `Sanctuary` condition: at attack-resolution
/// time, `resolve_attack` / `spell_attack_outcome` short-circuit on a
/// failed save (the attacker's swing whiffs). Hostile actions by the
/// warded actor end the spell — `Action::execute` clears the buff when
/// the holder casts a harmful spell or attacks.
pub struct Sanctuary {}

impl Action for Sanctuary {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Abjuration)
    }
    fn name(&self) -> &str {
        "sanctuary"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["sanc"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // Touch.
        Some(crate::actions::action_template::MELEE_REACH)
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        bonus_action_and_slot(1)
    }
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        _caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        // 10-round duration approximates 1 minute. The ward drops as soon
        // as the holder takes a hostile action (engine hook below).
        vec![Box::new(ApplyCondition {
            actor_id: target_id,
            condition: Condition::Sanctuary,
            timer: ConditionTimer::Rounds(10),
        })]
    }
}

pub static SANCTUARY: LazyLock<Sanctuary> = LazyLock::new(|| Sanctuary {});

/// True Resurrection — level-9 necromancy. Restores a Dying creature to
/// full HP and strips every captivating / mind-affecting / wound rider
/// the body might be carrying. Functionally a Resurrection that also
/// drops Charmed / Frightened / Stunned / Paralyzed and stands the
/// target back up (Prone clear) — top-of-the-line cleric panic button.
/// Costs a 9th-level slot, single-target, touch.
pub struct TrueResurrection {}

impl Action for TrueResurrection {
    fn name(&self) -> &str {
        "true resurrection"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["trueres", "tres"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(crate::actions::action_template::MELEE_REACH)
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn is_heal(&self) -> bool {
        true
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        _caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        let Some(target_id) = first_target_id(target_ids) else {
            return false;
        };
        encounter
            .actors
            .get(&target_id)
            .is_some_and(|a| a.is_dying())
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(9)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        _caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::side_effects::{RemoveCondition, ReviveDying};
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let max = encounter
            .actors
            .get(&target_id)
            .map(|a| a.max_hitpoints())
            .unwrap_or(0);
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = vec![
            Box::new(ReviveDying { actor_id: target_id }),
            Box::new(Heal {
                actor_id: target_id,
                amount: max,
            }),
        ];
        // Top-of-line cleanse: every condition you'd want gone after dying.
        for c in [
            Condition::Blinded,
            Condition::Deafened,
            Condition::Poisoned,
            Condition::Charmed,
            Condition::Frightened,
            Condition::Stunned,
            Condition::Paralyzed,
            Condition::Petrified,
            Condition::Prone,
        ] {
            effects.push(Box::new(RemoveCondition {
                actor_id: target_id,
                condition: c,
            }));
        }
        effects
    }
}

pub static TRUE_RESURRECTION: LazyLock<TrueResurrection> = LazyLock::new(|| TrueResurrection {});

/// Healing Spirit — level-2 conjuration, bonus action, concentration. A
/// shimmering spirit anchors to a tile; every ally whose footprint
/// touches the burst regains 1d6 HP. RAW the spirit moves and pulses
/// each round; we collapse to a single instant heal-burst at cast time
/// to keep the concentration plumbing simple — the bonus-action cost
/// and the AoE pattern are the load-bearing parts of the spell anyway.
pub struct HealingSpirit {}

impl Action for HealingSpirit {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Conjuration)
    }
    fn name(&self) -> &str {
        "healing spirit"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["hs", "spirit"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 1 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60 ft = 24 tiles.
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn is_heal(&self) -> bool {
        true
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        bonus_action_and_slot(2)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let raw = encounter.roll(&Dice::new(1, 6));
        encounter.log(format!(
            "  healing spirit: 1d6({}) = {} HP to each ally in area",
            raw, raw
        ));
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = encounter
            .ally_burst_targets(caster_id, point, 1)
            .into_iter()
            .map(|id| {
                Box::new(Heal {
                    actor_id: id,
                    amount: raw,
                }) as Box<dyn ApplicableSideEffect>
            })
            .collect();
        effects.push(Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData::new("Healing Spirit"),
        }));
        effects
    }
}

pub static HEALING_SPIRIT: LazyLock<HealingSpirit> = LazyLock::new(|| HealingSpirit {});

/// Aid — level-2 abjuration, action. Boosts up to three creatures' max
/// HP by 5 (level-2 baseline) for 8 hours. We already have the simpler
/// single-target Aid; this aliased version is a no-op stub kept off the
/// spell list for now. (Engine note: see the existing AID for the impl.)
/// Aura of Vitality — level-3 evocation, concentration, bonus action.
/// Anchors a 30-ft radius aura that lets the caster spend a bonus action
/// each round to heal one ally inside the aura for 2d6 HP. We model the
/// instant cast as a single 2d6 heal-burst on every ally inside a radius-
/// 6 sphere at the caster's tile — collapses the per-round bonus-action
/// retrigger into one strong upfront heal that mirrors Mass Healing Word
/// at a slightly lower slot cost.
pub struct AuraOfVitality {}

impl Action for AuraOfVitality {
    fn name(&self) -> &str {
        "aura of vitality"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["aov", "vitality"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn is_heal(&self) -> bool {
        true
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(3)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(caster_loc) = encounter
            .actors
            .get(&caster_id)
            .map(|a| a.location())
        else {
            return Vec::new();
        };
        let raw = encounter.roll(&Dice::new(2, 6));
        encounter.log(format!(
            "  aura of vitality: 2d6({}) = {} HP to allies in aura",
            raw, raw
        ));
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = encounter
            .ally_burst_targets(caster_id, caster_loc, 6)
            .into_iter()
            .map(|id| {
                Box::new(Heal {
                    actor_id: id,
                    amount: raw,
                }) as Box<dyn ApplicableSideEffect>
            })
            .collect();
        effects.push(Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData::new("Aura of Vitality"),
        }));
        effects
    }
}

pub static AURA_OF_VITALITY: LazyLock<AuraOfVitality> = LazyLock::new(|| AuraOfVitality {});

/// Wall of Force — level-5 evocation, concentration. Drops a panel of
/// invisible force at a tile within 120 ft. Anyone footprint-adjacent to
/// the panel at cast time is shoved one tile away (we approximate with a
/// `PullActor` *away from* the wall via negative max_tiles — no, just
/// pick a direction and use TeleportActor). Concrete effect today:
/// everyone in the burst takes 0 damage but is moved one tile away from
/// the anchor. Lasts 10 rounds. Concentration: dropping it doesn't
/// recall the moved actors.
pub struct WallOfForce {}

impl Action for WallOfForce {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Evocation)
    }
    fn name(&self) -> &str {
        "wall of force"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["woforce", "force-wall"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 1 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(48)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(5)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        // Approximate the panel by knocking enemies adjacent to the anchor
        // prone (no save) — a stand-in for "blocked by an invisible wall."
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = encounter
            .enemy_burst_targets(caster_id, point, 1)
            .into_iter()
            .map(|id| {
                Box::new(ApplyCondition {
                    actor_id: id,
                    condition: Condition::Prone,
                    timer: ConditionTimer::Permanent,
                }) as Box<dyn ApplicableSideEffect>
            })
            .collect();
        effects.push(Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData::new("Wall of Force"),
        }));
        effects
    }
}

pub static WALL_OF_FORCE: LazyLock<WallOfForce> = LazyLock::new(|| WallOfForce {});

/// Spike Growth — level-2 transmutation, concentration, action. Plants
/// spikes across a 20-ft radius (we use radius-3 in our 2.5ft grid).
/// Enemies in the area get the `Spiked` condition; whenever they move,
/// the `MoveActor` side-effect rolls 2d4 piercing damage per step
/// against them (the rider lives in side_effects.rs to keep the damage
/// roll consistent across every motion source — walks, Pulls, etc.).
/// Concentration: ending the spell clears Spiked from every target.
pub struct SpikeGrowth {}

impl Action for SpikeGrowth {
    fn name(&self) -> &str {
        "spike growth"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["spikes", "spike"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 3 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 150 ft = 60 tiles.
        Some(60)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn deals_damage(&self) -> bool {
        // Damage is movement-triggered, not on-cast. Surfacing
        // deals_damage=false keeps the AI's focus-fire pipeline from
        // picking this as a "first-strike" damage spell.
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(2)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        const RADIUS: isize = 3;
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        let mut applied: Vec<(usize, Condition)> = Vec::new();
        for tid in encounter.enemy_burst_targets(caster_id, point, RADIUS) {
            effects.push(Box::new(ApplyCondition {
                actor_id: tid,
                condition: Condition::Spiked,
                timer: ConditionTimer::Permanent,
            }));
            applied.push((tid, Condition::Spiked));
        }
        // Always start concentration: even with no current targets the
        // spike field exists and could catch a creature that walks in
        // later. We don't model "actors entering the area get Spiked"
        // (would need a positional re-check tick), but the concentration
        // marker keeps the slot in use.
        effects.push(Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData::with_conditions("Spike Growth", applied),
        }));
        effects
    }
}

pub static SPIKE_GROWTH: LazyLock<SpikeGrowth> = LazyLock::new(|| SpikeGrowth {});

/// Telekinesis — level-5 transmutation, concentration, action. Targets a
/// single creature within 60 ft; on a failed STR save vs the caster's
/// spell DC, the target is hoisted into the air — we apply the `Lifted`
/// condition (zeros movement) and pull them one tile toward the caster.
/// Concentration: dropping the spell clears Lifted. Each round the
/// caster could re-hoist a fresh target, but we collapse that into the
/// initial cast for now.
pub struct Telekinesis {}

impl Action for Telekinesis {
    fn name(&self) -> &str {
        "telekinesis"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["tk", "lift"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60 ft = 24 tiles.
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(5)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::side_effects::PullActor;
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spell_save_dc(AbilityScoreType::Intelligence);
        let caster_loc = caster.location();
        // Caster-aware save so Heightened Spell can force disadvantage
        // on the single save-or-suck STR roll.
        let save =
            encounter.roll_save_against_caster(target_id, AbilityScoreType::Strength, dc, caster_id);
        if save.passed() {
            return Vec::new();
        }
        vec![
            Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Lifted,
                timer: ConditionTimer::Permanent,
            }) as Box<dyn ApplicableSideEffect>,
            // Drag them one tile toward the caster — telekinetic grab.
            Box::new(PullActor {
                actor_id: target_id,
                toward: caster_loc,
                max_tiles: 1,
            }),
            Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions(
                    "Telekinesis",
                    vec![(target_id, Condition::Lifted)],
                ),
            }),
        ]
    }
}

pub static TELEKINESIS: LazyLock<Telekinesis> = LazyLock::new(|| Telekinesis {});

/// Polymorph — level-4 transmutation, concentration, action. Targets a
/// creature within 60 ft; on a failed WIS save the target is transformed
/// into a beast form. We model the load-bearing mechanical changes:
/// (1) apply the `Polymorphed` condition (cosmetic; tracked for narrative)
/// and (2) grant a fixed `30` temp HP pool representing the beast form's
/// HP — damage drains the beast pool first, dropping it ends the
/// transformation when concentration ends. Allies targeted with the
/// spell get a +30 temp HP buff (5e RAW: caster picks the beast); we
/// apply uniformly regardless of allegiance for simplicity.
///
/// Concentration: ending the spell strips `Polymorphed` from the target;
/// the temp HP pool is left to natural attrition (we don't reset it on
/// drop — matches 5e where the target reverts to their previous form
/// with their own HP, but the beast pool isn't refunded).
pub struct Polymorph {}

impl Action for Polymorph {
    fn name(&self) -> &str {
        "polymorph"
    }

    fn deals_damage(&self) -> bool {
        // Control, not damage. The AI's focus-fire lane scores by who
        // drops soonest, so an action that claims damage and deals none
        // gets picked over the attack that would have.
        false
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["poly", "morph"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60 ft = 24 tiles.
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(4)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let caster_team = caster.team();
        let target_team = encounter
            .actors
            .get(&target_id)
            .map(|a| a.team())
            .unwrap_or(caster_team);
        // Enemies get a WIS save; willing allies auto-fail (5e RAW).
        if target_team != caster_team {
            let dc = caster.spellcasting_save_dc();
            // Caster-aware save so the sorcerer Heightened Spell prime
            // can force disadvantage on this single save-or-suck roll.
            let save = encounter.roll_save_against_caster(
                target_id,
                AbilityScoreType::Wisdom,
                dc,
                caster_id,
            );
            if save.passed() {
                return Vec::new();
            }
        }
        vec![
            Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Polymorphed,
                timer: ConditionTimer::Permanent,
            }),
            Box::new(GainTempHp {
                actor_id: target_id,
                amount: 30,
            }),
            Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions(
                    "Polymorph",
                    vec![(target_id, Condition::Polymorphed)],
                ),
            }),
        ]
    }
}

pub static POLYMORPH: LazyLock<Polymorph> = LazyLock::new(|| Polymorph {});

/// Globe of Invulnerability — level-6 abjuration, concentration, action.
/// Self-buff: while the caster concentrates, they have generic damage
/// resistance (we approximate the 5e "immune to spells of level 5 or
/// lower" clause with a flat half-damage rider via the `Globed` condition
/// — the condition's `effective_damage` hook halves all incoming damage).
/// Pure self-defense — drops on concentration loss.
pub struct GlobeOfInvulnerability {}

impl Action for GlobeOfInvulnerability {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Abjuration)
    }
    fn name(&self) -> &str {
        "globe of invulnerability"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["globe", "invuln"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(6)
    }
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        self_concentration_buff_effects(
            caster_id,
            "Globe of Invulnerability",
            Condition::Globed,
            ConditionTimer::Permanent,
        )
    }
}

pub static GLOBE_OF_INVULNERABILITY: LazyLock<GlobeOfInvulnerability> =
    LazyLock::new(|| GlobeOfInvulnerability {});

/// Counterspell — level-3 abjuration, reaction. When another creature
/// casts a spell of level 3 or lower within 60 ft, the counterspeller
/// interrupts the cast and the spell fails. We approximate the reaction
/// timing by exposing Counterspell as an Action (not a Reaction trigger)
/// that targets a *currently concentrating* enemy — on cast, the target
/// loses their concentration (the most common "I'm running an active
/// spell" handle in our model). This collapses Counterspell's
/// interrupt-on-cast clause into a "rip the buff" effect since we don't
/// have spell-cast triggers wired into the reaction bus. Slot cost is
/// the level-3 default; targeting an unconcentrating enemy fizzles the
/// cast (validation gate).
pub struct Counterspell {}

impl Action for Counterspell {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Abjuration)
    }
    fn name(&self) -> &str {
        "counterspell"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["cs", "counter"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60 ft = 24 tiles.
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        // Spell slot only — Counterspell is RAW a reaction, but we don't
        // have a spell-cast trigger to fire from yet, so it gates on
        // Action availability and the level-3 slot to keep the gating
        // symmetric with other slotted spells.
        action_and_slot(3)
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        _caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // Valid against any target that is either concentrating or
        // holding a dispellable buff — otherwise the cast does nothing
        // useful and we'd be wasting the slot.
        let Some(target_id) = first_target_id(target_ids) else {
            return false;
        };
        let Some(target) = encounter.actors.get(&target_id) else {
            return false;
        };
        // 5e Sorcerer **Subtle Spell** rider: a sorcerer who's primed
        // their next cast with Subtle Spell ignores Counterspell (RAW: no
        // somatic or verbal components → nothing for the counterspeller
        // to perceive). Block the cast at validate-time so the slot
        // doesn't drain on a fizzle. We don't consume the prime here —
        // the validator runs during the AI's target-picker probe loop,
        // and consuming on probe would burn the prime against a target
        // we never actually counterspell. The prime expires on the
        // sorcerer's next turn via the `UntilStartOfNextTurn` tick-down,
        // which gives the sorcerer one full round of counterspell
        // immunity (sorcerer's turn N → opponent's reply turn → tick
        // down at sorcerer's turn N+1) — the load-bearing window for
        // the metamagic.
        if target.has_condition(Condition::SubtleSpelling) {
            return false;
        }
        target.is_concentrating()
            || target
                .conditions()
                .keys()
                .any(|c| c.is_dispellable_buff())
    }
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        _caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::side_effects::DispelMagicOn;
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        vec![Box::new(DispelMagicOn {
            target_id,
        })]
    }
}

pub static COUNTERSPELL: LazyLock<Counterspell> = LazyLock::new(|| Counterspell {});

/// Booming Blade — cantrip. Make a melee attack against a target within 5ft;
/// on hit, weapon damage as normal plus the target is marked with
/// `BoomingBladeMarked` — they take an extra 1d8 thunder the *next* time
/// they move voluntarily before the start of the caster's next turn. We
/// reuse the spell-attack pipeline with a 0d0 damage roll for the cantrip
/// itself (the weapon-attack half is folded in via the rider — at cantrip
/// scaling, the headline is the thunder rider, not the swing's main
/// damage). On hit the mark applies with a 1-round timer so it ticks off
/// the holder's turn cleanly. Misses do nothing.
pub struct BoomingBlade {}

impl Action for BoomingBlade {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Evocation)
    }
    fn name(&self) -> &str {
        "booming blade"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["bb", "boom"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(crate::actions::action_template::MELEE_REACH)
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Thunder]
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let attack_bonus = caster.spellcasting_attack_modifier();
        let n = crate::engine::util::cantrip_dice_count(caster.level());
        let mut effects = spell_attack(
            encounter,
            caster_id,
            target_id,
            "booming blade",
            attack_bonus,
            Dice::new(n, 8),
            DamageType::Thunder,
            true,
        );
        // Mark on hit only (empty effect list = miss).
        if !effects.is_empty() {
            effects.push(Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::BoomingBladeMarked,
                timer: ConditionTimer::Rounds(1),
            }));
        }
        effects
    }
}

pub static BOOMING_BLADE: LazyLock<BoomingBlade> = LazyLock::new(|| BoomingBlade {});

/// Tasha's Mind Whip — level-2 enchantment. Target within 90ft makes an INT
/// save vs the caster's spell DC: pass = half, fail = full 3d6 psychic and
/// the target loses one of action / bonus action / reaction on their next
/// turn. We model the reaction-loss via the `NoReaction` rider and the
/// action-loss via the `MindWhipped` condition (consumed at the start of
/// the next turn by `reset_for_new_round`, zeroing the action slot).
pub struct MindWhip {}

impl Action for MindWhip {
    fn name(&self) -> &str {
        "mind whip"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["mw", "whip"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 90 ft = 36 tiles.
        Some(36)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Psychic]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(2)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spell_save_dc(AbilityScoreType::Intelligence);
        let raw = encounter.roll_empowered_sum(caster_id, 3, 6);
        let save = encounter.roll_save_against_caster(target_id, AbilityScoreType::Intelligence, dc, caster_id);
        let dmg = if save.passed() { raw / 2 } else { raw };
        encounter.log(format!(
            "  mind whip: 3d6({}) = {} psychic",
            raw, dmg
        ));
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        if dmg > 0 {
            effects.push(Box::new(DealDamage {
                actor_id: target_id,
                amount: dmg,
                damage_type: DamageType::Psychic,
            }));
        }
        // Action-economy debuff lands only on a fail per RAW.
        if !save.passed() {
            effects.push(Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::MindWhipped,
                timer: ConditionTimer::Permanent,
            }));
            effects.push(Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::NoReaction,
                timer: ConditionTimer::Rounds(1),
            }));
        }
        effects
    }
}

pub static MIND_WHIP: LazyLock<MindWhip> = LazyLock::new(|| MindWhip {});

/// Crusader's Mantle — level-3 evocation, concentration. Self-buff aura that
/// makes every weapon hit by the caster (and, RAW, allies within 30ft) deal
/// +1d4 radiant. We model the load-bearing self-cast version: the caster
/// gains the `CrusadersMantled` condition for the duration. The +1d4
/// radiant rider lives on the `resolve_attack` path. We skip the aura-
/// extend-to-allies clause because the aura-tick infrastructure isn't in
/// place; for simplicity any willing caster gets the buff and concentration
/// holds the spell.
pub struct CrusadersMantle {}

impl Action for CrusadersMantle {
    fn name(&self) -> &str {
        "crusader's mantle"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["cm", "mantle", "crusader"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(3)
    }
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        self_concentration_buff_effects(
            caster_id,
            "Crusader's Mantle",
            Condition::CrusadersMantled,
            ConditionTimer::Permanent,
        )
    }
}

pub static CRUSADERS_MANTLE: LazyLock<CrusadersMantle> = LazyLock::new(|| CrusadersMantle {});

/// Earthquake — level-8 evocation, concentration. Burst at a point within
/// 500ft; every enemy in a 20ft radius (= 4-tile gap) makes a STR save vs
/// the caster's spell DC: fail = knocked Prone and takes 5d6 bludgeoning,
/// pass = no damage / no prone. Allies are spared (caster picks the safe
/// arc, per the spell's RAW "ground rupture" flavor). Damage is rolled
/// once and shared across all victims (matches 5e shared-roll AoE
/// semantics). Concentration so re-casting drops cleanly.
pub struct Earthquake {}

impl Action for Earthquake {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Evocation)
    }
    fn name(&self) -> &str {
        "earthquake"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["eq", "quake"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 4 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 500 ft RAW, capped to 120 tiles for our map scale.
        Some(120)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Bludgeoning]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(8)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        const RADIUS: isize = 4;
        let raw = encounter.roll_empowered_sum(caster_id, 5, 6);
        encounter.log(format!(
            "  earthquake: 5d6({}) shared bludgeoning",
            raw
        ));
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spellcasting_save_dc();
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        for tid in encounter.enemy_burst_targets(caster_id, point, RADIUS) {
            let save = encounter.roll_save_against_caster(tid, AbilityScoreType::Strength, dc, caster_id);
            if save.passed() {
                continue;
            }
            effects.push(Box::new(DealDamage {
                actor_id: tid,
                amount: raw,
                damage_type: DamageType::Bludgeoning,
            }));
            effects.push(Box::new(ApplyCondition {
                actor_id: tid,
                condition: Condition::Prone,
                timer: ConditionTimer::Permanent,
            }));
        }
        effects.push(Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData::new("Earthquake"),
        }));
        effects
    }
}

pub static EARTHQUAKE: LazyLock<Earthquake> = LazyLock::new(|| Earthquake {});

/// Time Stop — level-9 transmutation. Caster gets an extra Action and an
/// extra Bonus Action *immediately* (the 5e "1d4+1 turns of solo activity"
/// is collapsed to a one-turn burst of action economy). The TimeStopped
/// condition is a flag for the dispel pipeline / UI; the action-economy
/// boost is the load-bearing mechanical effect, delivered via two
/// `GiveResource` side-effects. Self-only; no save / no targeting.
pub struct TimeStop {}

impl Action for TimeStop {
    fn name(&self) -> &str {
        "time stop"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ts", "timestop"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(9)
    }
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::side_effects::GiveResource;
        vec![
            Box::new(ApplyCondition {
                actor_id: caster_id,
                condition: Condition::TimeStopped,
                timer: ConditionTimer::UntilStartOfNextTurn,
            }),
            Box::new(GiveResource {
                actor_id: caster_id,
                resource: Resource::Action,
            }),
            Box::new(GiveResource {
                actor_id: caster_id,
                resource: Resource::BonusAction,
            }),
        ]
    }
}

pub static TIME_STOP: LazyLock<TimeStop> = LazyLock::new(|| TimeStop {});

/// Wish — level-9 conjuration. The 5e RAW spell can mimic any sub-9 spell or
/// produce one of a small set of canonical effects. We model the "restore
/// up to twenty creatures to full HP" wish: every ally within 60ft is
/// healed to full HP. Self-only cast, no save, no targeting beyond the
/// implicit ally radius.
pub struct Wish {}

impl Action for Wish {
    fn name(&self) -> &str {
        "wish"
    }
    fn aliases(&self) -> Vec<&str> {
        vec![]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn is_heal(&self) -> bool {
        true
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(9)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let center = caster.location();
        // 60ft = 24 tiles. Includes the caster.
        let allies = encounter.ally_burst_targets(caster_id, center, 24);
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        for tid in allies {
            let Some(ally) = encounter.actors.get(&tid) else {
                continue;
            };
            // Full heal: the "missing HP" delta seeded as the Heal value.
            let missing = ally.max_hitpoints().saturating_sub(ally.hitpoints());
            if missing == 0 {
                continue;
            }
            effects.push(Box::new(Heal {
                actor_id: tid,
                amount: missing,
            }));
        }
        encounter.log(format!(
            "  wish: blessing {} ally(ies) to full HP",
            effects.len()
        ));
        effects
    }
}

pub static WISH: LazyLock<Wish> = LazyLock::new(|| Wish {});

/// Forcecage — level-7 evocation. Target within 100ft makes a CHA save vs
/// the caster's spell DC (RAW: no save if the cage is set up as the
/// "solid cage" variant, but we keep one save for symmetry with other
/// imprisonment spells). On fail the target gains the `Caged` condition
/// for 10 rounds (≈1 minute RAW). Caged zeros movement and blocks
/// reactions via the existing condition wiring.
pub struct Forcecage {}

impl Action for Forcecage {
    fn name(&self) -> &str {
        "forcecage"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["fc", "cage"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 100 ft = 40 tiles.
        Some(40)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(7)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spell_save_dc(AbilityScoreType::Intelligence);
        let save = encounter.roll_save_against_caster(target_id, AbilityScoreType::Charisma, dc, caster_id);
        if save.passed() {
            return Vec::new();
        }
        vec![Box::new(ApplyCondition {
            actor_id: target_id,
            condition: Condition::Caged,
            timer: ConditionTimer::Rounds(10),
        })]
    }
}

pub static FORCECAGE: LazyLock<Forcecage> = LazyLock::new(|| Forcecage {});

/// Crown of Stars — level-7 evocation. Self-cast that grants the caster a
/// halo of seven motes for the duration. Each weapon hit by the caster
/// rolls +1d8 radiant — we model the per-mote charge clause as a flat
/// per-hit rider via `resolve_attack`'s `CrownOfStars` lookup. Lasts
/// 10 rounds (≈1 hour RAW, capped here to a long Rounds timer). Doesn't
/// require concentration.
pub struct CrownOfStarsSpell {}

impl Action for CrownOfStarsSpell {
    fn name(&self) -> &str {
        "crown of stars"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["cos", "crown"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(7)
    }
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        vec![Box::new(ApplyCondition {
            actor_id: caster_id,
            condition: Condition::CrownOfStars,
            timer: ConditionTimer::Rounds(10),
        })]
    }
}

pub static CROWN_OF_STARS: LazyLock<CrownOfStarsSpell> = LazyLock::new(|| CrownOfStarsSpell {});

/// Fear — level-3 illusion, concentration. 30-foot cone of dread (4-tile
/// burst). Each enemy in the burst makes a WIS save vs the caster's DC:
/// fail = Frightened for the spell's duration; pass = no effect. We use
/// the shared `enemy_burst_targets` partition so allies in the blast are
/// spared. Concentration so a re-cast / damage drop cleans up the entire
/// Frightened pool in one shot.
pub struct Fear {}

impl Action for Fear {
    fn name(&self) -> &str {
        "fear"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["fr", "terror"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 4 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // Self-origin cone; the burst point sits right in front of the
        // caster. Cap the targeting tile to the caster's footprint so
        // the cone always engulfs them as the cone's origin.
        Some(6)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(3)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spellcasting_save_dc();
        const RADIUS: isize = 4;
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        let mut applied = Vec::new();
        for tid in encounter.enemy_burst_targets(caster_id, point, RADIUS) {
            let save = encounter.roll_save_against_caster(tid, AbilityScoreType::Wisdom, dc, caster_id);
            if save.passed() {
                continue;
            }
            effects.push(Box::new(ApplyCondition {
                actor_id: tid,
                condition: Condition::Frightened,
                timer: ConditionTimer::Rounds(10),
            }));
            applied.push((tid, Condition::Frightened));
        }
        if !applied.is_empty() {
            effects.push(Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions("Fear", applied),
            }));
        }
        effects
    }
}

pub static FEAR: LazyLock<Fear> = LazyLock::new(|| Fear {});

/// Greater Restoration — level-5 abjuration. Touch-range cleanse + heal.
/// Removes one of: Charmed / Petrified / Paralyzed / Stunned / one
/// exhaustion level (we don't model exhaustion). Then heals 4d8 + caster's
/// spellcasting modifier. Distinct from Lesser Restoration: GR can lift
/// the heavyweight lockdown conditions LR can't touch, and pairs the
/// cleanse with a real heal — paired action-economy efficiency. RAW
/// requires a 100gp diamond as material; we don't model components.
pub struct GreaterRestoration {}

impl Action for GreaterRestoration {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Abjuration)
    }
    fn name(&self) -> &str {
        "greater restoration"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["gr", "grestore"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(crate::actions::action_template::MELEE_REACH)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn is_heal(&self) -> bool {
        true
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(5)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let mod_bonus = caster.ability_modifier(AbilityScoreType::Wisdom);
        let raw = encounter.roll(&Dice::new(4, 8));
        let amount = (raw as i32 + mod_bonus).max(0) as u32;
        encounter.log(format!(
            "  greater restoration: 4d8({}){:+} = {} HP",
            raw, mod_bonus, amount
        ));
        vec![
            Box::new(crate::engine::side_effects::RemoveOneOfConditions {
                actor_id: target_id,
                candidates: Self::CANDIDATES.to_vec(),
            }),
            Box::new(Heal {
                actor_id: target_id,
                amount,
            }),
        ]
    }
}

impl GreaterRestoration {
    /// Heavyweight conditions Greater Restoration is allowed to lift, in
    /// priority order. Distinct from the Lesser Restoration list — GR
    /// targets the lockdown set (Paralyzed, Stunned, Petrified, Charmed)
    /// that LR can't touch. Includes Lesser-Restoration's targets too so
    /// a stuck-with-only-GR caster can still cleanse Poisoned / etc.
    /// Also lifts Exhausted (RAW: GR removes one level of exhaustion;
    /// we model the simplified single-tier flag so cleansing it ends
    /// the condition outright) and Feebled (RAW: GR explicitly removes
    /// Feeblemind's mind-shattering effect — high-priority since it
    /// otherwise locks down INT/WIS/CHA saves indefinitely).
    const CANDIDATES: [Condition; 10] = [
        Condition::Petrified,
        Condition::Paralyzed,
        Condition::Stunned,
        Condition::Feebled,
        Condition::Charmed,
        Condition::Frightened,
        Condition::Exhausted,
        Condition::Poisoned,
        Condition::Blinded,
        Condition::Deafened,
    ];
}

pub static GREATER_RESTORATION: LazyLock<GreaterRestoration> =
    LazyLock::new(|| GreaterRestoration {});

/// Compelled Duel — level-1 enchantment, concentration. The paladin
/// challenges a target to a duel: the target makes a WIS save vs the
/// caster's spell DC. Fail = target is `Dueled` (attacks against anyone
/// other than the caster are at disadvantage — see `compute_attack_mode`).
/// Pass = no effect. The duel is tracked via `Dueled` back-link so the engine
/// knows the anchor. Caster picks the toughest enemy in melee range so
/// the paladin uses themselves as a tank.
///
/// We use the existing Charm save-immunity gate (undead / constructs)
/// here: a creature that can't be enchanted shrugs off the duel.
pub struct CompelledDuel {}

impl Action for CompelledDuel {
    fn name(&self) -> &str {
        "compelled duel"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["cd-spell", "duel"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 30ft RAW = 12 tiles.
        Some(12)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        // Bonus action + level-1 slot — RAW Compelled Duel is bonus-action
        // economy so the paladin can still swing their greatsword on the
        // same turn they open the challenge.
        bonus_action_and_slot(1)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        // Charm-immune creatures (undead / constructs in our pool, plus
        // Fey Ancestry / MindBlanked dynamic immunities) shrug off the
        // enchantment by RAW — no save needed.
        if let Some(target) = encounter.actors.get(&target_id)
            && target.effectively_immune_to_condition(Condition::Charmed)
        {
            encounter.log("  compelled duel: target resists enchantment".to_string());
            return Vec::new();
        }
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spell_save_dc(AbilityScoreType::Charisma);
        let save = encounter.roll_save_against_caster(target_id, AbilityScoreType::Wisdom, dc, caster_id);
        if save.passed() {
            return Vec::new();
        }
        let mut out: Vec<Box<dyn ApplicableSideEffect>> = vec![
            Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Dueled,
                timer: ConditionTimer::Rounds(10),
            }),
        ];
        // Pull the SetConditionLink(Dueled) install from the central
        // `condition_link_side_effect` dispatch — same source of truth
        // the weapon on-hit rider chain uses, so a single match arm
        // there serves both spell-cast and rider-trigger code paths.
        if let Some(link) = crate::engine::side_effects::condition_link_side_effect(
            Condition::Dueled,
            target_id,
            caster_id,
        ) {
            out.push(link);
        }
        out.push(Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData::with_conditions(
                "Compelled Duel",
                vec![(target_id, Condition::Dueled)],
            ),
        }));
        out
    }
}

pub static COMPELLED_DUEL: LazyLock<CompelledDuel> = LazyLock::new(|| CompelledDuel {});

/// Config-driven Smite spell. Every Smite (Searing / Wrathful /
/// Thunderous / Branding / Blinding / Staggering / Banishing) shares
/// the same shape: bonus-action cast, level-N slot, concentration,
/// applies a one-shot "primed" condition to the caster that the on-hit
/// rider table in `engine::attack` consumes on the next melee weapon
/// hit. The spells differ only in slot level, log name, and which prime
/// they apply — collapsed into one impl so adding another smite is a
/// one-entry table addition (plus the matching rider in attack.rs).
pub struct SmiteSpell {
    pub display_name: &'static str,
    pub aliases: &'static [&'static str],
    pub spell_slot_lvl: u32,
    /// Caster-side condition the smite primes — read by the on-hit
    /// rider table to apply the bonus damage and follow-up effect.
    pub prime: Condition,
    /// Spell name string used for concentration tracking. Matches the
    /// 5e RAW spell name so concentration logs read cleanly.
    pub concentration_name: &'static str,
}

impl Action for SmiteSpell {
    fn name(&self) -> &str {
        self.display_name
    }
    fn aliases(&self) -> Vec<&str> {
        self.aliases.to_vec()
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        // Like Divine Smite — the rider damage lands on the *next* hit,
        // not on this action's resolution. False keeps the AI's
        // focus-fire pipeline from picking the prime over an actual
        // attack.
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        bonus_action_and_slot(self.spell_slot_lvl)
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // Don't double-prime: re-casting the same smite while already
        // primed wastes a slot. The AI's pipeline doesn't deeply model
        // this; the gate is here for symmetry with Divine Smite's
        // `!has_condition(Smiting)` guard.
        encounter
            .actors
            .get(&caster_id)
            .is_some_and(|a| a.is_combat_active() && !a.has_condition(self.prime))
    }
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        // 10-round prime window — long enough that a primed paladin who
        // can't connect on the cast turn still has the better part of a
        // minute (in 5e time) to land it. Concentration anchors the
        // spell so taking damage can break the prime via the CON-save
        // path, matching RAW.
        vec![
            Box::new(ApplyCondition {
                actor_id: caster_id,
                condition: self.prime,
                timer: ConditionTimer::Rounds(10),
            }),
            Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions(
                    self.concentration_name,
                    vec![(caster_id, self.prime)],
                ),
            }),
        ]
    }
}

/// Searing Smite — 1st-level paladin evocation, bonus action,
/// concentration. Primes the next melee hit with +1d6 fire and ignites
/// the target (Burning, 3 rounds).
pub static SEARING_SMITE: SmiteSpell = SmiteSpell {
    display_name: "searing smite",
    aliases: &["searing", "smite-fire"],
    spell_slot_lvl: 1,
    prime: Condition::SearingSmiting,
    concentration_name: "Searing Smite",
};

/// Wrathful Smite — 1st-level paladin enchantment, bonus action,
/// concentration. Primes the next melee hit with +1d6 psychic and a
/// WIS save (vs caster CHA-DC) gates Frightened (10 rounds) on fail.
pub static WRATHFUL_SMITE: SmiteSpell = SmiteSpell {
    display_name: "wrathful smite",
    aliases: &["wrathful", "smite-fear"],
    spell_slot_lvl: 1,
    prime: Condition::WrathfulSmiting,
    concentration_name: "Wrathful Smite",
};

/// Branding Smite — 2nd-level paladin evocation, bonus action,
/// concentration. Primes the next melee hit with +2d6 radiant and
/// brands the target (Outlined, 10 rounds — attackers get advantage).
pub static BRANDING_SMITE: SmiteSpell = SmiteSpell {
    display_name: "branding smite",
    aliases: &["branding", "smite-brand"],
    spell_slot_lvl: 2,
    prime: Condition::BrandingSmiting,
    concentration_name: "Branding Smite",
};

/// Blinding Smite — 3rd-level paladin evocation, bonus action,
/// concentration. Primes the next melee hit with +3d8 radiant and a
/// CON save gates Blinded (10 rounds) on fail.
pub static BLINDING_SMITE: SmiteSpell = SmiteSpell {
    display_name: "blinding smite",
    aliases: &["blinding", "smite-blind"],
    spell_slot_lvl: 3,
    prime: Condition::BlindingSmiting,
    concentration_name: "Blinding Smite",
};

/// Staggering Smite — 4th-level paladin enchantment, bonus action,
/// concentration. Primes the next melee hit with +4d6 psychic damage
/// and a WIS save (vs the paladin's CHA-based DC) gates Stunned-for-1-
/// round on fail. The lv4 slot tier slots cleanly between Blinding
/// Smite (lv3 radiant + blind) and Banishing Smite (lv5 force + banish)
/// on the smite spell ladder.
pub static STAGGERING_SMITE: SmiteSpell = SmiteSpell {
    display_name: "staggering smite",
    aliases: &["staggering", "smite-stagger", "smite-stun"],
    spell_slot_lvl: 4,
    prime: Condition::StaggeringSmiting,
    concentration_name: "Staggering Smite",
};

/// Banishing Smite — 5th-level paladin abjuration, bonus action,
/// concentration. Primes the next melee hit with +5d10 force damage;
/// if the hit reduces the target to 50 HP or fewer the target is
/// banished (we collapse the demi-plane mechanic to a 10-round inert
/// envelope via `Condition::Mazed` — same end-state, distinct log
/// line). The HP threshold gate is evaluated at the on-hit rider site
/// via the `hp_threshold: Some(50)` field on the rider's follow-up.
pub static BANISHING_SMITE: SmiteSpell = SmiteSpell {
    display_name: "banishing smite",
    aliases: &["banishing", "smite-banish"],
    spell_slot_lvl: 5,
    prime: Condition::BanishingSmiting,
    concentration_name: "Banishing Smite",
};

/// Thunderous Smite — 1st-level paladin evocation, bonus action,
/// concentration. Primes the next melee hit with +2d6 thunder; target
/// makes a STR save vs the paladin's CHA-based DC or is knocked Prone
/// (RAW also pushes 10 ft — we collapse the push to just the prone
/// follow-up since the load-bearing crowd-control effect is the prone
/// tag; future use of `PushActor` here would slot in via a custom
/// rider, but the smite follow-up table currently only carries one
/// condition apply).
pub static THUNDEROUS_SMITE: SmiteSpell = SmiteSpell {
    display_name: "thunderous smite",
    aliases: &["thunderous", "smite-thunder"],
    spell_slot_lvl: 1,
    prime: Condition::ThunderousSmiting,
    concentration_name: "Thunderous Smite",
};

/// Central registry of every Smite spell, ordered cheapest-slot first.
/// Single source of truth: the paladin loadout, the AI's slot-cheapest-
/// first smite picker, and (over time) the on-hit rider table can all
/// pull from this slice instead of restating the list inline. Keeping
/// the order slot-cheap → slot-expensive matches the AI's "preserve
/// higher slots for emergencies" heuristic.
pub static ALL_SMITE_SPELLS: &[&SmiteSpell] = &[
    &SEARING_SMITE,
    &WRATHFUL_SMITE,
    &THUNDEROUS_SMITE,
    &BRANDING_SMITE,
    &BLINDING_SMITE,
    &STAGGERING_SMITE,
    &BANISHING_SMITE,
];

/// Ranger-flavored smite spells (Ensnaring Strike + Zephyr Strike at
/// lv1, Lightning Arrow at lv3 — ordered cheapest-slot first, matching
/// the AI's "preserve higher slots for emergencies" heuristic on
/// `ALL_SMITE_SPELLS`). Kept distinct from the paladin-flavored
/// `ALL_SMITE_SPELLS` registry because the AI's melee-smite pipeline
/// gates on adjacent-enemy-present, which is wrong for a ranger's
/// bow-range prime; the ranged registry pairs with the AI's ranger-
/// flavored `try_ranged_smite_spell` heuristic which gates on
/// enemy-in-bow-range (24 tiles). Same SmiteSpell chassis — the gate
/// is the only difference.
///
/// Note: **Ensnaring Strike** and **Zephyr Strike**'s on-hit riders
/// fire on either the scimitar swing or the longbow shot (RAW:
/// Ensnaring's "the next time you hit a creature with a weapon
/// attack" broad envelope and Zephyr's "the next attack you make on
/// this turn" turn-scoped envelope — both unrestricted by weapon
/// lane), while **Lightning Arrow**'s rider is ranged-only (RAW:
/// "with this weapon" refers to the bow swing the prime installs on).
/// The AI heuristic still gates on bow-range for all three since the
/// ranger's action-economy loop typically closes on the longbow — the
/// melee-fallback lane on Ensnaring Strike / Zephyr Strike is a bonus,
/// not the primary consumption path.
///
/// Both lv1 primes share a slot cost but split the tactical role:
/// Ensnaring trades away raw damage for a Restrained lock (STR-save
/// follow-up), while Zephyr trades away the lock for +1d8 Force damage
/// (one of the rarest-resisted damage types in the engine, punching
/// through nearly every typed-defense lane). The registry orders
/// Ensnaring first so a ranger facing a target with poor STR saves
/// preferentially burns the lock-flavored slot before falling through
/// to Zephyr's raw-damage rider on the next round's cast.
pub static ALL_RANGED_SMITE_SPELLS: &[&SmiteSpell] = &[&ENSNARING_STRIKE, &ZEPHYR_STRIKE, &LIGHTNING_ARROW];

/// Flame Strike — 5th-level evocation. A column of divine fire descends
/// on a tile within 60ft (24 tiles); every creature whose footprint is
/// within a 2-tile (10ft) radius of the point makes a DEX save vs the
/// caster's WIS-based DC. On fail: 4d6 fire + 4d6 radiant. On success:
/// half. The mixed damage type is the spell's signature — it slips past
/// fire-resistant fiends (radiant lands) and undead with radiant
/// resistance (fire lands), making it the cleric's go-to AoE.
pub struct FlameStrike {}

impl Action for FlameStrike {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Evocation)
    }
    fn name(&self) -> &str {
        "flame strike"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["fs", "fstrike"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 2 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60ft RAW = 24 tiles.
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Fire, DamageType::Radiant]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(5)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spellcasting_save_dc();
        // Roll the two damage halves separately so the per-actor
        // resistance / immunity lookup applies independently — a fire-
        // immune efreet still eats the radiant half, and a radiant-
        // resistant celestial still takes full fire.
        let fire_raw = encounter.roll(&Dice::new(4, 6));
        let rad_raw = encounter.roll(&Dice::new(4, 6));
        encounter.log(format!(
            "  flame strike: 4d6({}) fire + 4d6({}) radiant",
            fire_raw, rad_raw
        ));
        let mut effects = crate::actions::action_template::resolve_burst_save_damage(
            encounter,
            caster_id,
            point,
            2,
            AbilityScoreType::Dexterity,
            dc,
            fire_raw,
            DamageType::Fire,
        );
        effects.extend(crate::actions::action_template::resolve_burst_save_damage(
            encounter,
            caster_id,
            point,
            2,
            AbilityScoreType::Dexterity,
            dc,
            rad_raw,
            DamageType::Radiant,
        ));
        effects
    }
}

pub static FLAME_STRIKE: LazyLock<FlameStrike> = LazyLock::new(|| FlameStrike {});

/// Heat Metal — 2nd-level transmutation, concentration, bonus action
/// (RAW: Action on cast, bonus action to repeat the damage each round;
/// we collapse to a one-tap concentration mark that ticks the damage on
/// the holder's turn-start). The target's metal armor / weapon glows
/// red-hot: they take 2d8 fire on cast, and an additional 2d8 fire at
/// the start of each of their turns while concentration holds. They
/// also have disadvantage on attack rolls and ability checks (the
/// HeatMetaled condition feeds `compute_attack_mode`'s disadvantage
/// clause). No save — RAW gives a CON save each turn to drop the gear
/// but we keep the simulation crisp by skipping it.
pub struct HeatMetal {}

impl Action for HeatMetal {
    fn name(&self) -> &str {
        "heat metal"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["hm-fire", "heat"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60ft RAW = 24 tiles.
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Fire]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(2)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let raw = encounter.roll_empowered_sum(caster_id, 2, 8);
        encounter.log(format!("  heat metal: 2d8({}) fire on cast", raw));
        vec![
            Box::new(DealDamage {
                actor_id: target_id,
                amount: raw,
                damage_type: DamageType::Fire,
            }),
            Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::HeatMetaled,
                timer: ConditionTimer::Rounds(10),
            }),
            Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions(
                    "Heat Metal",
                    vec![(target_id, Condition::HeatMetaled)],
                ),
            }),
        ]
    }
}

pub static HEAT_METAL: LazyLock<HeatMetal> = LazyLock::new(|| HeatMetal {});

/// Chain Lightning — 6th-level evocation. A bolt of lightning leaps
/// from the caster to a primary target (DEX save for half, 10d8
/// lightning), then forks to up to 3 additional creatures within
/// 5 tiles (25ft RAW) of the primary — each rolling its own DEX save
/// for half. Selection of secondary targets is deterministic: the 3
/// nearest combat-active actors (other than the primary), excluding
/// the caster. Mixed-team — fork hits allies as well as enemies, so
/// the AI's friendly-fire heuristic gates casting through
/// `try_attack_aoe`'s pool check.
pub struct ChainLightning {}

impl Action for ChainLightning {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Evocation)
    }
    fn name(&self) -> &str {
        "chain lightning"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["chain", "cl"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 150 ft RAW = 60 tiles, but we cap to the engine's standard
        // long-range cantrip reach to keep the targeting picker honest.
        Some(60)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Lightning]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(6)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::util::{footprint_chebyshev, get_tiles_from_size};

        let Some(primary_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spellcasting_save_dc();
        let raw = encounter.roll_empowered_sum(caster_id, 10, 8);
        encounter.log(format!(
            "  chain lightning: 10d8({}) lightning (primary + forks)",
            raw
        ));

        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        // Primary target — full DEX save for half. Routes through the
        // caster-aware save helper so Heightened Spell metamagic can
        // force disadvantage on the first save (RAW applies it to the
        // primary, not the chained forks); the prime is consumed here
        // so subsequent fork saves fall through to the normal path.
        let save = encounter
            .roll_save_against_caster(primary_id, AbilityScoreType::Dexterity, dc, caster_id);
        let dmg = if save.passed() { raw / 2 } else { raw };
        if dmg > 0 {
            effects.push(Box::new(DealDamage {
                actor_id: primary_id,
                amount: dmg,
                damage_type: DamageType::Lightning,
            }));
        }

        // Find the 3 nearest combat-active actors within 5 tiles of the
        // primary — caster excluded so the bolt doesn't bite its source.
        let Some(primary) = encounter.actors.get(&primary_id) else {
            return effects;
        };
        let primary_loc = primary.location();
        let primary_size = get_tiles_from_size(primary.size());
        let mut forks: Vec<(isize, usize)> = encounter
            .actors
            .iter()
            .filter_map(|(id, a)| {
                if *id == caster_id || *id == primary_id || !a.is_combat_active() {
                    return None;
                }
                let dist = footprint_chebyshev(
                    a.location(),
                    get_tiles_from_size(a.size()),
                    primary_loc,
                    primary_size,
                );
                if dist > 5 {
                    return None;
                }
                Some((dist, *id))
            })
            .collect();
        // Deterministic by (distance, id) so seeded tests are stable.
        forks.sort_unstable();
        for (_, fork_id) in forks.into_iter().take(3) {
            // Fork saves route through the caster-aware helper too —
            // the prime is consumed on the primary's save above, so
            // these fall through to the normal save path. Keeping the
            // call site uniform with the primary keeps a future
            // "metamagic affects all chain forks" tweak landing in one
            // place.
            let save = encounter
                .roll_save_against_caster(fork_id, AbilityScoreType::Dexterity, dc, caster_id);
            let dmg = if save.passed() { raw / 2 } else { raw };
            if dmg > 0 {
                effects.push(Box::new(DealDamage {
                    actor_id: fork_id,
                    amount: dmg,
                    damage_type: DamageType::Lightning,
                }));
            }
        }
        effects
    }
}

pub static CHAIN_LIGHTNING: LazyLock<ChainLightning> = LazyLock::new(|| ChainLightning {});

/// Goodberry — 5e druid level-1 transmutation. Conjures up to 10 magical
/// berries; eating one restores 1 HP. We collapse the "10 berries over an
/// hour" RAW into a single in-combat heal of 10 HP on a touch-range ally
/// — the caster's WIS modifier isn't added (RAW: berries are a flat 1 HP
/// each). Behaves like a low-cost emergency top-up: cheap level-1 slot,
/// touch range, schemes nicely with Healing Word for ranged backup.
pub struct Goodberry {}

impl Action for Goodberry {
    fn name(&self) -> &str {
        "goodberry"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["gb", "berry"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(crate::actions::action_template::MELEE_REACH)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn is_heal(&self) -> bool {
        true
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(1)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        _caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        encounter.log("  goodberry: 10 HP restored from magical berries".to_string());
        vec![Box::new(Heal {
            actor_id: target_id,
            amount: 10,
        })]
    }
}

pub static GOODBERRY: LazyLock<Goodberry> = LazyLock::new(|| Goodberry {});

/// Moonbeam — 5e druid level-2 evocation, concentration. A 5ft-radius
/// beam of silvery light strikes the targeted point. Every creature in
/// the beam makes a CON save; fail = 2d10 radiant, pass = half. We treat
/// the cast as a single burst (Spirit Guardians shape) since the engine
/// doesn't yet model "lingering area, re-rolled each round" AoEs. Targets
/// allies and enemies alike (it's an indiscriminate beam) and starts
/// concentration so the AI knows it's holding it.
pub struct Moonbeam {}

impl Action for Moonbeam {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Evocation)
    }
    fn name(&self) -> &str {
        "moonbeam"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["mb", "moon"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 1 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 120 ft = 48 tiles.
        Some(48)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Radiant]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(2)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spell_save_dc(AbilityScoreType::Wisdom);
        let raw = encounter.roll_empowered_sum(caster_id, 2, 10);
        encounter.log(format!("  moonbeam: 2d10({}) radiant beam", raw));
        let targets = encounter.enemy_burst_targets(caster_id, point, 1);
        let mut effs: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        let mut conc_conditions: Vec<(usize, Condition)> = Vec::new();
        for tid in targets {
            let save = encounter.roll_save_against_caster(tid, AbilityScoreType::Constitution, dc, caster_id);
            let dmg = if save.passed() { raw / 2 } else { raw };
            if dmg > 0 {
                effs.push(Box::new(DealDamage {
                    actor_id: tid,
                    amount: dmg,
                    damage_type: DamageType::Radiant,
                }));
            }
            if !save.passed() {
                effs.push(Box::new(ApplyCondition {
                    actor_id: tid,
                    condition: Condition::Moonbeamed,
                    timer: ConditionTimer::Rounds(10),
                }));
                conc_conditions.push((tid, Condition::Moonbeamed));
            }
        }
        effs.push(Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData::with_conditions("Moonbeam", conc_conditions),
        }));
        effs
    }
}

pub static MOONBEAM: LazyLock<Moonbeam> = LazyLock::new(|| Moonbeam {});

/// Call Lightning — 5e druid level-3 conjuration, concentration. Calls a
/// storm cloud overhead; on cast and on each subsequent Action this turn,
/// a lightning bolt strikes a chosen point dealing 3d10 lightning (DEX
/// save, half on success) to every creature within 5ft of the strike.
/// We model the cast as a single 3d10 burst at the point + concentration
/// install — re-casts of the same spell while concentrating proc the
/// bolt anew (the action picker handles that path since the slot is gone
/// after the initial cast, RAW's "without spending a spell slot" repeat
/// fires the standard concentration channel). Single-tile burst (5ft).
pub struct CallLightning {}

impl Action for CallLightning {
    fn name(&self) -> &str {
        "call lightning"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["cl-spell", "lightning", "callbolt"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        // 5ft burst — single-tile strike. Targets in the same tile as
        // the strike point catch the full radius.
        TargetingSchema::Burst { radius: 1 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 120 ft strike radius.
        Some(48)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Lightning]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(3)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spell_save_dc(AbilityScoreType::Wisdom);
        let raw = encounter.roll(&Dice::new(3, 10));
        encounter.log(format!("  call lightning: 3d10({}) lightning bolt", raw));
        let mut effs = crate::actions::action_template::resolve_burst_save_damage(
            encounter,
            caster_id,
            point,
            1,
            AbilityScoreType::Dexterity,
            dc,
            raw,
            DamageType::Lightning,
        );
        effs.push(Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData::new("Call Lightning"),
        }));
        effs
    }
}

pub static CALL_LIGHTNING: LazyLock<CallLightning> = LazyLock::new(|| CallLightning {});

/// Sleet Storm — 5e druid level-3 conjuration, concentration. Freezing
/// rain coats a 20-ft cylinder; creatures inside make a DEX save or be
/// knocked Prone, and concentrating spellcasters in the area must save
/// on a CON check or drop concentration. We model: enemy-only burst
/// (caster + allies stay vertical), DEX save vs Prone on fail, plus an
/// optional concentration-break for any enemy holding concentration. No
/// damage — the storm is pure crowd-control. 20ft radius = 4 tiles.
pub struct SleetStorm {}

impl Action for SleetStorm {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Conjuration)
    }
    fn name(&self) -> &str {
        "sleet storm"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["sleet", "ss-spell"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 4 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 150 ft = 60 tiles.
        Some(60)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(3)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spell_save_dc(AbilityScoreType::Wisdom);
        const RADIUS: isize = 4;

        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        let mut applied: Vec<(usize, Condition)> = Vec::new();
        for tid in encounter.enemy_burst_targets(caster_id, point, RADIUS) {
            let save = encounter.roll_save_against_caster(tid, AbilityScoreType::Dexterity, dc, caster_id);
            if save.passed() {
                continue;
            }
            effects.push(Box::new(ApplyCondition {
                actor_id: tid,
                condition: Condition::Prone,
                timer: ConditionTimer::Permanent,
            }));
            applied.push((tid, Condition::Prone));
            // Concentration break: any enemy holding concentration must
            // succeed on a CON save or drop. We piggyback on the engine's
            // drop_concentration helper rather than re-rolling here — the
            // CON save uses the same DC as the DEX save (5e RAW: "DC
            // equal to your spell save DC").
            if encounter
                .actors
                .get(&tid)
                .is_some_and(|a| a.is_concentrating())
            {
                let conc_save =
                    encounter.roll_save_against_caster(tid, AbilityScoreType::Constitution, dc, caster_id);
                if !conc_save.passed() {
                    encounter.drop_concentration(tid);
                }
            }
        }
        if !applied.is_empty() {
            effects.push(Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions("Sleet Storm", applied),
            }));
        }
        effects
    }
}

pub static SLEET_STORM: LazyLock<SleetStorm> = LazyLock::new(|| SleetStorm {});

/// Reverse Gravity — 5e level-7 transmutation, concentration. Gravity
/// reverses in a wide column; creatures inside fall *up*, then crash
/// back down when concentration drops. We model the cast's load-bearing
/// half: a STR save (failure = thrown around, Prone) plus 8d6 bludgeoning
/// to fallen creatures (the fall damage). Allies in the column are
/// included — RAW makes no friend/foe distinction. Concentration is
/// installed so dispel can lift the gravity column.
pub struct ReverseGravity {}

impl Action for ReverseGravity {
    fn name(&self) -> &str {
        "reverse gravity"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["rg", "reverse"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 10 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 100 ft = 40 tiles.
        Some(40)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Bludgeoning]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(7)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        // Druid (WIS) or wizard (INT) — pick the caster's better DC.
        let dc = caster
            .spell_save_dc(AbilityScoreType::Wisdom)
            .max(caster.spell_save_dc(AbilityScoreType::Intelligence));
        let raw = encounter.roll_empowered_sum(caster_id, 8, 6);
        encounter.log(format!(
            "  reverse gravity: 8d6({}) bludgeoning fall damage",
            raw
        ));
        const RADIUS: isize = 10;

        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        let mut applied: Vec<(usize, Condition)> = Vec::new();
        // RAW makes no ally/enemy distinction — every creature in the
        // column rolls a save. Caster is excluded (they cast it; they
        // brace themselves).
        for tid in encounter.neutral_burst_targets(caster_id, point, RADIUS) {
            let save = encounter.roll_save_against_caster(tid, AbilityScoreType::Strength, dc, caster_id);
            if save.passed() {
                continue;
            }
            // On a fail: 8d6 bludgeoning + Prone (the crash landing).
            effects.push(Box::new(DealDamage {
                actor_id: tid,
                amount: raw,
                damage_type: DamageType::Bludgeoning,
            }));
            effects.push(Box::new(ApplyCondition {
                actor_id: tid,
                condition: Condition::Prone,
                timer: ConditionTimer::Permanent,
            }));
            applied.push((tid, Condition::Prone));
        }
        effects.push(Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData::with_conditions("Reverse Gravity", applied),
        }));
        effects
    }
}

pub static REVERSE_GRAVITY: LazyLock<ReverseGravity> = LazyLock::new(|| ReverseGravity {});

/// Storm of Vengeance — 5e druid level-9 conjuration, concentration. A
/// massive storm churns above the targeted point. The apex druid spell:
/// huge radius, mixed thunder + lightning damage, plus a CON save vs
/// Deafened. We collapse the multi-round RAW (acid → wind → hail) into
/// a single big burst on cast: 2d6 thunder + 4d6 lightning on every
/// enemy in the 30ft sphere; CON save halves and dodges the Deafened
/// rider. Caster + allies are spared by the enemy_burst_targets filter.
pub struct StormOfVengeance {}

impl Action for StormOfVengeance {
    fn name(&self) -> &str {
        "storm of vengeance"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["sov", "stormv"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 6 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // "Sight" range RAW — we cap to the engine's long-range bracket
        // so the picker stays sane.
        Some(60)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Thunder, DamageType::Lightning]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(9)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spell_save_dc(AbilityScoreType::Wisdom);
        const RADIUS: isize = 6;

        let thunder = encounter.roll_empowered_sum(caster_id, 2, 6);
        let lightning = encounter.roll_empowered_sum(caster_id, 4, 6);
        encounter.log(format!(
            "  storm of vengeance: 2d6({}) thunder + 4d6({}) lightning",
            thunder, lightning
        ));

        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        let mut deafened: Vec<(usize, Condition)> = Vec::new();
        for tid in encounter.enemy_burst_targets(caster_id, point, RADIUS) {
            let save = encounter.roll_save_against_caster(tid, AbilityScoreType::Constitution, dc, caster_id);
            let t_dmg = if save.passed() { thunder / 2 } else { thunder };
            let l_dmg = if save.passed() { lightning / 2 } else { lightning };
            if t_dmg > 0 {
                effects.push(Box::new(DealDamage {
                    actor_id: tid,
                    amount: t_dmg,
                    damage_type: DamageType::Thunder,
                }));
            }
            if l_dmg > 0 {
                effects.push(Box::new(DealDamage {
                    actor_id: tid,
                    amount: l_dmg,
                    damage_type: DamageType::Lightning,
                }));
            }
            // Deafened rider only on failed saves — the storm's roar
            // ruptures eardrums on a fail.
            if !save.passed() {
                effects.push(Box::new(ApplyCondition {
                    actor_id: tid,
                    condition: Condition::Deafened,
                    timer: ConditionTimer::Rounds(10),
                }));
                deafened.push((tid, Condition::Deafened));
            }
        }
        effects.push(Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData::with_conditions("Storm of Vengeance", deafened),
        }));
        effects
    }
}

pub static STORM_OF_VENGEANCE: LazyLock<StormOfVengeance> =
    LazyLock::new(|| StormOfVengeance {});

/// Hellish Rebuke — 5e level-1 evocation (warlock signature). Reaction
/// spell: when you take damage, deal fire damage to the attacker. Target
/// makes a DEX save vs the caster's CHA-based DC. On fail: 2d10 fire;
/// on save: half. Upcasting: +1d10 per slot level above 1. Cost is a
/// Reaction + spell slot (RAW: "1 reaction, which you take in response
/// to being damaged by a creature within 60 feet").
pub struct HellishRebuke {}

impl Action for HellishRebuke {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Evocation)
    }
    fn name(&self) -> &str {
        "hellish rebuke"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["hr", "rebuke"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Fire]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        // Hellish Rebuke is a reaction spell per RAW.
        let lvl = crate::engine::action_overrides::cast_level(overrides, 1);
        vec![Resource::Reaction, Resource::SpellSlot(lvl)]
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spell_save_dc(AbilityScoreType::Charisma);
        // Upcasting: 2d10 at level 1, +1d10 per slot level above 1.
        let lvl = crate::engine::action_overrides::cast_level(overrides, 1);
        let dice_count = 1 + lvl; // 2d10 at lv1, 3d10 at lv2, etc.
        let (dmg, _) = save_for_half_damage(
            encounter,
            caster_id,
            target_id,
            AbilityScoreType::Dexterity,
            dc,
            Dice::new(dice_count, 10),
            DamageType::Fire,
            "hellish rebuke",
        );
        if dmg == 0 {
            return Vec::new();
        }
        vec![Box::new(DealDamage {
            actor_id: target_id,
            amount: dmg,
            damage_type: DamageType::Fire,
        })]
    }
}

pub static HELLISH_REBUKE: LazyLock<HellishRebuke> = LazyLock::new(|| HellishRebuke {});

/// Spirit Shroud — 5e level-3 necromancy / abjuration, concentration.
/// Self-buff that wreathes the caster in deathly mist: their next 10
/// rounds of melee weapon attacks deal +1d8 cold rider per hit (per the
/// OnHitRider table entry). No save, no target — purely a self-prime.
/// The actual rider lives in `attack::on_hit_riders()`.
pub struct SpiritShroud {}

impl Action for SpiritShroud {
    fn name(&self) -> &str {
        "spirit shroud"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["shroud", "spirits"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        bonus_action_and_slot(3)
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // Already wreathed → don't re-cast and burn another slot.
        actor_lacks_condition(encounter, caster_id, Condition::SpiritShrouded)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        encounter.log("  spirit shroud: ghostly mist coils around you.".to_string());
        self_concentration_buff_effects(
            caster_id,
            "Spirit Shroud",
            Condition::SpiritShrouded,
            ConditionTimer::Rounds(10),
        )
    }
}

pub static SPIRIT_SHROUD: LazyLock<SpiritShroud> = LazyLock::new(|| SpiritShroud {});

/// Holy Aura — 5e level-8 abjuration cleric, concentration. The caster
/// and every ally inside a 30ft sphere centered on the caster receive a
/// huge defensive buff: advantage on saves + attackers vs them have
/// disadvantage. Single-shot install: every ally in range at cast time
/// picks up the condition. No re-scan per round (cheap approximation —
/// allies who walk in after the cast miss out, but the high-impact
/// half-blast-radius "everyone in the room" cleanse is preserved).
pub struct HolyAura {}

impl Action for HolyAura {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Abjuration)
    }
    fn name(&self) -> &str {
        "holy aura"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["aura"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(8)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let center = match encounter.actors.get(&caster_id) {
            Some(c) => c.location(),
            None => return Vec::new(),
        };
        // 30ft = 12 tiles. Pick up every ally (including caster) inside.
        const RADIUS: isize = 12;
        let allies = encounter.ally_burst_targets(caster_id, center, RADIUS);
        encounter.log(format!(
            "  holy aura: {} ally{} bathed in light",
            allies.len(),
            if allies.len() == 1 { "" } else { "ies" }
        ));
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        let mut applied: Vec<(usize, Condition)> = Vec::new();
        for id in allies {
            effects.push(Box::new(ApplyCondition {
                actor_id: id,
                condition: Condition::HolyAuraed,
                timer: ConditionTimer::Rounds(10),
            }));
            applied.push((id, Condition::HolyAuraed));
        }
        effects.push(Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData::with_conditions("Holy Aura", applied),
        }));
        effects
    }
}

pub static HOLY_AURA: LazyLock<HolyAura> = LazyLock::new(|| HolyAura {});

/// Foresight — 5e level-9 divination, concentration. Target ally gets
/// the mightiest single-target buff in the SRD: advantage on every
/// attack roll, save, and ability check; attackers vs them have
/// disadvantage. 10-round timer (8 hours RAW). Concentration-bound.
pub struct Foresight {}

impl Action for Foresight {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Divination)
    }
    fn name(&self) -> &str {
        "foresight"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["fs"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // Touch — the caster lays hands on the recipient.
        Some(1)
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(9)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        encounter.log("  foresight: glimpse of the future settles over them.".to_string());
        vec![
            Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Foreseen,
                timer: ConditionTimer::Rounds(10),
            }),
            Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions(
                    "Foresight",
                    vec![(target_id, Condition::Foreseen)],
                ),
            }),
        ]
    }
}

pub static FORESIGHT: LazyLock<Foresight> = LazyLock::new(|| Foresight {});

/// Hail of Thorns — 5e level-1 ranger conjuration. A single-target ranged
/// attack that on hit erupts in a 5ft burst of thorns around the target,
/// dealing 1d10 piercing on a failed DEX save (half on save) to every
/// other creature within reach of the target. We approximate as: pick
/// a target tile, every actor within 1-tile gap of that tile (excluding
/// the caster) makes a save against the caster's WIS-based DC.
pub struct HailOfThorns {}

impl Action for HailOfThorns {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Conjuration)
    }
    fn name(&self) -> &str {
        "hail of thorns"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["hot", "thorns"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 1 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // Long bow range — 600 ft RAW; we cap to 60 tiles for the map.
        Some(60)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Piercing]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(1)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spell_save_dc(AbilityScoreType::Wisdom);
        let raw = encounter.roll(&Dice::new(1, 10));
        encounter.log(format!(
            "  hail of thorns: 1d10({}) piercing around target tile",
            raw
        ));
        crate::actions::action_template::resolve_burst_save_damage(
            encounter,
            caster_id,
            point,
            1,
            AbilityScoreType::Dexterity,
            dc,
            raw,
            DamageType::Piercing,
        )
    }
}

pub static HAIL_OF_THORNS: LazyLock<HailOfThorns> = LazyLock::new(|| HailOfThorns {});

/// Animate Dead — 5e level-3 necromancy. The caster raises an undead
/// minion adjacent to themselves: a Skeleton joins the caster's team
/// and acts on its own initiative for the rest of the encounter. We
/// approximate "raise from a corpse pile" by spawning a fresh
/// SKELETON_TEMPLATE instance at a footprint-free tile next to the
/// caster (closest spawnable diagonal / orthogonal neighbor). No
/// concentration; the minion is permanent for the encounter.
pub struct AnimateDead {}

impl Action for AnimateDead {
    fn summons_allies(&self) -> bool {
        true
    }
    fn name(&self) -> &str {
        "animate dead"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["raise"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(3)
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // Need a free adjacent slot to spawn the Medium skeleton.
        encounter
            .find_adjacent_spawn(caster_id, crate::engine::types::Size::Medium, 2)
            .is_some()
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::actors::creatures::skeletons::SKELETON_TEMPLATE;

        // Single skeleton minion at touch range. Routes through the
        // shared `spawn_adjacent_summons` helper so the team-lookup,
        // find_adjacent_spawn search, instantiate + log chain stays
        // consistent with Conjure Animals / Conjure Elemental. Animate
        // Dead doesn't use concentration so the minion is permanent for
        // the encounter and gets no `Conjured` tag.
        spawn_adjacent_summons(
            encounter,
            caster_id,
            &SKELETON_TEMPLATE,
            crate::engine::types::Size::Medium,
            1,
            2,
            99,
            "animate dead",
        );
        Vec::new()
    }
}

pub static ANIMATE_DEAD: LazyLock<AnimateDead> = LazyLock::new(|| AnimateDead {});

/// Confusion — 5e level-4 enchantment, concentration, action. Targets a
/// 20ft burst (radius 4) at a point within 90ft (36 tiles). Every
/// creature in the area makes a WIS save vs the caster's spell DC; on
/// fail, they're Confused for up to 10 rounds (1 minute RAW).
///
/// Confused (collapsed from RAW's per-turn chaos table): disadvantage on
/// attack rolls (via `Condition::imposes_attacker_disadvantage`) plus the
/// holder cannot take Reactions (via `Condition::blocks_reactions`).
/// Concentration-bound on the caster — dropping the spell strips Confused
/// from every target it landed on.
pub struct Confusion {}

impl Action for Confusion {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Enchantment)
    }
    fn name(&self) -> &str {
        "confusion"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["conf", "scramble"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 4 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 90 ft = 36 tiles.
        Some(36)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(4)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spellcasting_save_dc();
        const RADIUS: isize = 4;
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        let mut applied: Vec<(usize, Condition)> = Vec::new();
        for tid in encounter.enemy_burst_targets(caster_id, point, RADIUS) {
            let save = encounter.roll_save_against_caster(tid, AbilityScoreType::Wisdom, dc, caster_id);
            if save.passed() {
                continue;
            }
            effects.push(Box::new(ApplyCondition {
                actor_id: tid,
                condition: Condition::Confused,
                timer: ConditionTimer::Rounds(10),
            }));
            applied.push((tid, Condition::Confused));
        }
        // Concentration installs even on a no-effect cast — RAW the chaos
        // mist hangs around for the duration regardless of saves.
        effects.push(Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData::with_conditions("Confusion", applied),
        }));
        effects
    }
}

pub static CONFUSION: LazyLock<Confusion> = LazyLock::new(|| Confusion {});

/// Fly — 5e level-3 transmutation, concentration, action. Targets one
/// willing creature within 5ft (1 tile reach). The target gains a flying
/// speed bonus (we model as +60ft, applied additively in `speed()`).
/// Concentration-bound; dropping concentration strips the `Flying`
/// condition (and the speed boost).
pub struct Fly {}

impl Action for Fly {
    fn name(&self) -> &str {
        "fly"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["levitate-flight", "wing"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // Touch range — 5ft = 1 tile.
        Some(1)
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(3)
    }
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        vec![
            Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Flying,
                timer: ConditionTimer::Rounds(10),
            }) as Box<dyn ApplicableSideEffect>,
            Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions(
                    "Fly",
                    vec![(target_id, Condition::Flying)],
                ),
            }),
        ]
    }
}

pub static FLY: LazyLock<Fly> = LazyLock::new(|| Fly {});

/// Levitate — 5e level-2 transmutation, concentration, action. Targets
/// one creature or object within 60ft. Target makes a CON save vs the
/// caster's spell DC; on fail, hoisted 20ft into the air and unable to
/// move horizontally. Mechanically we apply the existing `Lifted`
/// condition (zeros movement). Concentration-bound; lighter-weight than
/// Telekinesis (level-5, includes the forced-pull rider).
pub struct Levitate {}

impl Action for Levitate {
    fn name(&self) -> &str {
        "levitate"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["lev", "hoist"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60 ft = 24 tiles.
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(2)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spellcasting_save_dc();
        let save = encounter.roll_save_against_caster(target_id, AbilityScoreType::Constitution, dc, caster_id);
        if save.passed() {
            return Vec::new();
        }
        vec![
            Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Lifted,
                timer: ConditionTimer::Permanent,
            }) as Box<dyn ApplicableSideEffect>,
            Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions(
                    "Levitate",
                    vec![(target_id, Condition::Lifted)],
                ),
            }),
        ]
    }
}

pub static LEVITATE: LazyLock<Levitate> = LazyLock::new(|| Levitate {});

/// Plant Growth — 5e level-3 transmutation, action. Targets a 20ft burst
/// (radius 4) at a point within 150ft. Every enemy creature in the area
/// is Entangled for 10 rounds — vines snare them in place, zeroing
/// movement for the duration. No save; no concentration. Allies are
/// untouched via the `enemy_burst_targets` partition.
pub struct PlantGrowth {}

impl Action for PlantGrowth {
    fn name(&self) -> &str {
        "plant growth"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["pg", "vines", "entangle"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 4 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 150 ft = 60 tiles.
        Some(60)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(3)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        const RADIUS: isize = 4;
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        for tid in encounter.enemy_burst_targets(caster_id, point, RADIUS) {
            effects.push(Box::new(ApplyCondition {
                actor_id: tid,
                condition: Condition::Entangled,
                timer: ConditionTimer::Rounds(10),
            }));
        }
        effects
    }
}

pub static PLANT_GROWTH: LazyLock<PlantGrowth> = LazyLock::new(|| PlantGrowth {});

/// Dominate Person — 5e level-5 enchantment, concentration, action. Targets
/// one creature within 60ft; target makes a WIS save vs the caster's spell
/// DC. On fail, target is Charmed by the caster AND Dominated (disadvantage
/// on all attacks — they hesitate, fight the compulsion) for 10 rounds.
/// The `Charmed` half blocks the target from attacking the dominator (via
/// `Charmed` back-link); the `Dominated` half folds into
/// `Condition::imposes_attacker_disadvantage`. Concentration-bound on the
/// caster — drop concentration to free the target.
pub struct DominatePerson {}

impl Action for DominatePerson {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Enchantment)
    }
    fn name(&self) -> &str {
        "dominate person"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["dom", "dominate"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60 ft = 24 tiles.
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(5)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spell_save_dc(AbilityScoreType::Charisma);
        // Caster-aware save so Heightened Spell can force disadvantage.
        let save =
            encounter.roll_save_against_caster(target_id, AbilityScoreType::Wisdom, dc, caster_id);
        if save.passed() {
            return Vec::new();
        }
        // Layer Dominated on top of the standard Charmed + its back-link
        // install lane via the shared `install_dominated_by` helper.
        install_dominated_by(target_id, caster_id, "Dominate Person")
    }
}

pub static DOMINATE_PERSON: LazyLock<DominatePerson> = LazyLock::new(|| DominatePerson {});

/// Dominate Beast — 5e level-4 enchantment, concentration, action. Slots
/// between Charm Person (lv1) / Charm Monster (lv4) on the low-end and
/// Dominate Person (lv5) / Dominate Monster (lv8) on the high-end of the
/// dominate ladder. Targets one **Beast** within 60 ft; the target makes
/// a WIS save vs the caster's spell DC. On fail, the target is Charmed
/// AND Dominated for 10 rounds (1 minute RAW). The `Charmed` half blocks
/// the target from attacking the dominator (via `Charmed` back-link); the
/// `Dominated` half folds into `Condition::imposes_attacker_disadvantage`.
/// Concentration-bound on the caster — drop concentration to free the
/// target.
///
/// The headline gate vs Dominate Person / Monster: a Beast-only filter
/// via `custom_validate_input`. RAW: "You attempt to beguile a beast that
/// you can see within range." The engine's first spell that reads the
/// target's `creature_type()`; the install payload itself routes through
/// the shared `install_dominated_by` helper alongside Dominate Person /
/// Dominate Monster so only the slot level, name, and target-type gate
/// differ between the three spells.
pub struct DominateBeast {}

impl Action for DominateBeast {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Enchantment)
    }
    fn name(&self) -> &str {
        "dominate beast"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["domb", "dom-beast"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60 ft = 24 tiles.
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(4)
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        _caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // RAW Beast-only gate — distinguishes lv4 Dominate Beast from
        // lv5 Dominate Person (Humanoid-coded, though our engine doesn't
        // enforce that side) and lv8 Dominate Monster (any creature
        // type). Fail-closed on missing target / actor — same convention
        // as the recharge gates on the dao / mammoth / dragon side.
        let Some(target_id) = first_target_id(target_ids) else {
            return false;
        };
        encounter
            .actors
            .get(&target_id)
            .is_some_and(|a| {
                a.creature_type() == crate::engine::types::CreatureType::Beast
            })
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spell_save_dc(AbilityScoreType::Charisma);
        // Caster-aware save so Heightened Spell can force disadvantage.
        let save =
            encounter.roll_save_against_caster(target_id, AbilityScoreType::Wisdom, dc, caster_id);
        if save.passed() {
            return Vec::new();
        }
        // Same Charmed + its back-link + Dominated install lane as
        // Dominate Person / Dominate Monster via the shared helper.
        install_dominated_by(target_id, caster_id, "Dominate Beast")
    }
}

pub static DOMINATE_BEAST: LazyLock<DominateBeast> = LazyLock::new(|| DominateBeast {});

/// Magic Stone — 5e druid / artificer cantrip. The caster blesses up to
/// three pebbles; flinging one is a ranged spell attack (60ft) with the
/// caster's spellcasting mod, 1d6 + mod bludgeoning on hit. We collapse
/// the "three charges over the day" RAW into a per-cast 1-pebble attack
/// — at cantrip cadence the action-economy gate matters more than the
/// charge pool, and the 60ft range + spell-attack treatment is the
/// load-bearing piece (it bypasses ranged-in-melee disadvantage RAW,
/// which we don't yet model). Druids/artificers cast off WIS in 5e; we
/// reuse the caster's `spell_attack_modifier(Wisdom)` so a sorcerer
/// multiclass via Druidic Warrior would still get a sensible swing
/// (CHA-flavored casters fall back via spell_attack_modifier itself).
pub struct MagicStone {}

impl Action for MagicStone {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Transmutation)
    }
    fn name(&self) -> &str {
        "magic stone"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ms", "stone"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60ft RAW = 24 tiles.
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Bludgeoning]
    }
    // Cantrip — uses the default `cost()` (single Action, no spell slot).
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let wis_mod = caster.ability_modifier(AbilityScoreType::Wisdom);
        let attack_bonus = caster.spell_attack_modifier(AbilityScoreType::Wisdom);
        // 1d6 + WIS modifier on hit (RAW). Damage type is bludgeoning —
        // a stone, not a magical force projectile — so per-target
        // resistance / immunity to BPS applies normally.
        spell_attack_with_bonus(
            encounter,
            caster_id,
            target_id,
            "magic stone",
            attack_bonus,
            Dice::new(1, 6),
            wis_mod,
            DamageType::Bludgeoning,
            false,
        )
    }
}

pub static MAGIC_STONE: LazyLock<MagicStone> = LazyLock::new(|| MagicStone {});

/// Heroes' Feast — 5e level-6 conjuration. A magical feast appears; every
/// ally that partakes (we model: every ally in the caster's burst at
/// cast time) gains 2d10 + 10 temp HP (we collapse the "2d10 max HP for
/// 24 hours" RAW into a flat temp-HP grant for combat duration), gains
/// immunity to Frightened (we approximate via Heroic, which already
/// includes Frightened immunity), and is healed for 2d10 HP. Caster is
/// included if they're in the burst. Slot-6 = once-per-day apex pre-
/// fight buff for the cleric / druid / paladin loadout.
pub struct HeroesFeast {}

impl Action for HeroesFeast {
    fn name(&self) -> &str {
        "heroes' feast"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["hf", "feast"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 6 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 30ft to the burst origin = 12 tiles.
        Some(12)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn is_heal(&self) -> bool {
        true
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(6)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        // Roll the temp-HP grant and heal once and share across all
        // partakers per RAW's shared-feast semantics.
        let temp_pool = encounter.roll(&Dice::new(2, 10)) + 10;
        let heal_amount = encounter.roll(&Dice::new(2, 10));
        encounter.log(format!(
            "  heroes' feast: {} temp HP + {} HP heal + Heroic buff to allies in 30ft",
            temp_pool, heal_amount
        ));
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        // Ally-only burst — friendly fire feasts make no sense.
        let allies = encounter.ally_burst_targets(caster_id, point, 6);
        // Include the caster explicitly if they sit at the table — the
        // ally_burst_targets helper covers the caster naturally via the
        // same-team filter, but we double-include if the caster wasn't
        // in the 30ft window (they cast it on a distant pile of allies).
        let mut targets = allies.clone();
        if !targets.contains(&caster_id) {
            targets.push(caster_id);
        }
        targets.sort_unstable();
        targets.dedup();
        for id in targets {
            effects.push(Box::new(GainTempHp {
                actor_id: id,
                amount: temp_pool,
            }));
            effects.push(Box::new(Heal {
                actor_id: id,
                amount: heal_amount,
            }));
            // Heroic carries the Frightened-immunity clause already via
            // the AdjustSaveBuff lane in the existing Heroism spell. We
            // reuse the condition for symmetry; 10-round timer.
            effects.push(Box::new(ApplyCondition {
                actor_id: id,
                condition: Condition::Heroic,
                timer: ConditionTimer::Rounds(10),
            }));
        }
        effects
    }
}

pub static HEROES_FEAST: LazyLock<HeroesFeast> = LazyLock::new(|| HeroesFeast {});

/// Spike Stones — 5e level-3 druid transmutation, concentration. Stones
/// in a 20ft square sprout sharp spikes. Every enemy whose footprint
/// touches the burst is tagged with `Spiked` for 10 rounds — the per-
/// step piercing damage rider lives on `MoveActor::apply` and reads the
/// condition. Acts like a slower, larger-area Spike Growth, traded for
/// the higher slot cost. Concentration: dropping it pulls the tags.
pub struct SpikeStones {}

impl Action for SpikeStones {
    fn name(&self) -> &str {
        "spike stones"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ss", "spike-stones"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        // 20ft square ≈ 4-tile radius (we use Chebyshev burst so this is
        // a 9×9 region; close enough to the 4-square RAW footprint).
        TargetingSchema::Burst { radius: 4 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60ft to the burst origin = 24 tiles.
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn deals_damage(&self) -> bool {
        // Damage lands via the per-step Spiked rider, not at cast time.
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(4)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        // Enemy-only burst — we don't want allies stepping into the
        // spike field to also bleed (RAW: it's terrain that hits
        // anyone, but the AI's targeting works better as enemy-only).
        let enemies = encounter.enemy_burst_targets(caster_id, point, 4);
        if enemies.is_empty() {
            encounter.log("  spike stones: no enemies in the area".to_string());
        } else {
            encounter.log(format!(
                "  spike stones: {} enemies tagged with spiked",
                enemies.len()
            ));
        }
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        let mut concentration_targets: Vec<(usize, Condition)> = Vec::new();
        for id in enemies {
            effects.push(Box::new(ApplyCondition {
                actor_id: id,
                condition: Condition::Spiked,
                timer: ConditionTimer::Rounds(10),
            }));
            concentration_targets.push((id, Condition::Spiked));
        }
        effects.push(Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData::with_conditions(
                "Spike Stones",
                concentration_targets,
            ),
        }));
        effects
    }
}

pub static SPIKE_STONES: LazyLock<SpikeStones> = LazyLock::new(|| SpikeStones {});

/// Holy Word — 5e level-7 cleric evocation, 30ft burst centered on the
/// caster. Every enemy in the area makes a CHA save against the cleric's
/// WIS-based DC: on fail, take 5d10 radiant. The save outcome also
/// determines a rider keyed to the target's *remaining* HP after the
/// damage lands (we approximate by reading the pre-damage HP — the engine
/// queues all side effects in one batch so a "post-damage" read would
/// require splitting into two queues, which the rest of the codebase
/// avoids):
/// - HP ≤ 50  → Stunned for 1 round  (the lockdown rider)
/// - HP ≤ 75  → Blinded for 1 round
/// - HP ≤ 100 → Deafened for 1 round
/// - HP > 100 → damage only
///
/// Allies are spared per RAW (the spell explicitly targets enemies).
/// Concentration-free.
pub struct HolyWord {}

impl Action for HolyWord {
    fn name(&self) -> &str {
        "holy word"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["hwd", "holy"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Radiant]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(7)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let caster_loc = caster.location();
        let dc = caster.spell_save_dc(AbilityScoreType::Wisdom);
        let damage = encounter.roll_empowered_sum(caster_id, 5, 10);
        encounter.log(format!(
            "  holy word: 5d10({}) = {} radiant to enemies in 30ft",
            damage, damage
        ));
        // Enemy-only burst centered on the caster (radius 6 ≈ 30ft).
        let enemies = encounter.enemy_burst_targets(caster_id, caster_loc, 6);
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        for tid in enemies {
            let save = encounter.roll_save_against_caster(tid, AbilityScoreType::Charisma, dc, caster_id);
            if save.passed() {
                continue;
            }
            // Capture HP *before* the damage queues — used to pick the rider
            // tier per RAW's "if it has X or fewer HP" gate.
            let hp = encounter
                .actors
                .get(&tid)
                .map(|a| a.hitpoints())
                .unwrap_or(0);
            effects.push(Box::new(DealDamage {
                actor_id: tid,
                amount: damage,
                damage_type: DamageType::Radiant,
            }));
            let (cond, label) = if hp <= 50 {
                (Condition::Stunned, "stunned")
            } else if hp <= 75 {
                (Condition::Blinded, "blinded")
            } else if hp <= 100 {
                (Condition::Deafened, "deafened")
            } else {
                continue;
            };
            encounter.log(format!(
                "  holy word: target at {} HP is {}",
                hp, label
            ));
            effects.push(Box::new(ApplyCondition {
                actor_id: tid,
                condition: cond,
                timer: ConditionTimer::Rounds(1),
            }));
        }
        effects
    }
}

pub static HOLY_WORD: LazyLock<HolyWord> = LazyLock::new(|| HolyWord {});

/// Prismatic Spray — 5e level-7 evocation. 60ft cone (radius-6 burst on
/// the target point). Each enemy in the area rolls 1d8 to determine
/// which colored ray strikes them; the ray's damage type is fixed by the
/// roll. Then they make a DEX save against the caster's INT-based DC:
/// on fail, take 10d6 of the rolled type; on save, half. The 8th color
/// (white/multi) deals all rolled types together — we collapse the rare
/// "force + blinded" rider into the 7-roll table and skip the reroll
/// branch for simplicity.
///   roll → damage type:
///     1 → Fire, 2 → Acid, 3 → Lightning, 4 → Poison,
///     5 → Cold, 6 → Force, 7 → Radiant (also Blinded for 1 round),
///     8 → Necrotic (the indigo ray)
/// Each target gets its own ray roll — RAW lets each pick a different
/// color, and this matches the chaos of the spell. Enemy-only filter:
/// the caster controls the cone aim, so allies in the burst are spared.
pub struct PrismaticSpray {}

impl Action for PrismaticSpray {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Evocation)
    }
    fn name(&self) -> &str {
        "prismatic spray"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ps", "prismatic"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 6 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60ft cone — the burst origin sits at the cone's far edge.
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        // Random per target — list the full envelope so resistance hints
        // in the UI surface every possibility.
        vec![
            DamageType::Fire,
            DamageType::Acid,
            DamageType::Lightning,
            DamageType::Poison,
            DamageType::Cold,
            DamageType::Force,
            DamageType::Radiant,
            DamageType::Necrotic,
        ]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(7)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spell_save_dc(AbilityScoreType::Intelligence);
        encounter.log("  prismatic spray: a rainbow burst erupts");
        let enemies = encounter.enemy_burst_targets(caster_id, point, 6);
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        for tid in enemies {
            let ray = encounter.roll(&Dice::new(1, 8));
            let (dtype, name, with_blind) = match ray {
                1 => (DamageType::Fire, "red", false),
                2 => (DamageType::Acid, "orange", false),
                3 => (DamageType::Lightning, "yellow", false),
                4 => (DamageType::Poison, "green", false),
                5 => (DamageType::Cold, "blue", false),
                6 => (DamageType::Force, "violet", false),
                7 => (DamageType::Radiant, "white", true),
                _ => (DamageType::Necrotic, "indigo", false),
            };
            // Bare `roll`, deliberately, for the same reason
            // Sickening Radiance keeps one: RAW gives each target its
            // own ray *and* its own 10d6, so this is a per-target roll
            // rather than a shared one. `roll_empowered_sum` carries
            // per-cast bonuses (Empowered Evocation, Potent
            // Spellcasting) and a per-cast metamagic reroll, all of
            // which would be paid once per victim from inside this
            // loop. The two per-target bursts are the file's only two
            // exceptions and both say so here.
            let raw = encounter.roll(&Dice::new(10, 6));
            let save = encounter.roll_save_against_caster(tid, AbilityScoreType::Dexterity, dc, caster_id);
            let dmg = if save.passed() { raw / 2 } else { raw };
            encounter.log(format!(
                "  prismatic spray: 1d8({}) {} ray \u{2014} 10d6({}) {:?} ({}{})",
                ray,
                name,
                raw,
                dtype,
                dmg,
                if save.passed() { ", saved" } else { "" }
            ));
            if dmg > 0 {
                effects.push(Box::new(DealDamage {
                    actor_id: tid,
                    amount: dmg,
                    damage_type: dtype,
                }));
            }
            // The white ray also leaves the target Blinded for 1 round on
            // a failed save (the radiant flash sears their eyes).
            if with_blind && !save.passed() {
                effects.push(Box::new(ApplyCondition {
                    actor_id: tid,
                    condition: Condition::Blinded,
                    timer: ConditionTimer::Rounds(1),
                }));
            }
        }
        effects
    }
}

pub static PRISMATIC_SPRAY: LazyLock<PrismaticSpray> = LazyLock::new(|| PrismaticSpray {});

/// Feeblemind — 5e level-8 enchantment. Single-target INT save against
/// the caster's INT-based spell DC. On a failed save, the target takes
/// 4d6 psychic damage and is Feebled (our new condition): their INT and
/// CHA effectively drop to 1, modeled as blanket disadvantage on attack
/// rolls (the target can barely focus), disadvantage on INT/WIS/CHA
/// saves, and they can't cast spells. The condition lasts 10 rounds
/// (RAW: permanent until Greater Restoration / Heal / etc; we cap to a
/// duration the engine can resolve before the encounter ends). Greater
/// Restoration explicitly removes Feebled.
pub struct Feeblemind {}

impl Action for Feeblemind {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Enchantment)
    }
    fn name(&self) -> &str {
        "feeblemind"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["fm", "feeble"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 150 ft = 60 tiles RAW — capped at 40 to fit common map widths.
        Some(40)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Psychic]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(8)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spell_save_dc(AbilityScoreType::Intelligence);
        let damage = encounter.roll_empowered_sum(caster_id, 4, 6);
        // Per RAW the damage lands regardless of the save; only the
        // mind-shatter rider gates on the save outcome.
        encounter.log(format!(
            "  feeblemind: 4d6({}) = {} psychic",
            damage, damage
        ));
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = vec![Box::new(DealDamage {
            actor_id: target_id,
            amount: damage,
            damage_type: DamageType::Psychic,
        })];
        let save = encounter.roll_save_against_caster(target_id, AbilityScoreType::Intelligence, dc, caster_id);
        if !save.passed() {
            encounter.log("  feeblemind: target's mind shatters");
            effects.push(Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Feebled,
                timer: ConditionTimer::Rounds(10),
            }));
        }
        effects
    }
}

pub static FEEBLEMIND: LazyLock<Feeblemind> = LazyLock::new(|| Feeblemind {});

/// Otto's Irresistible Dance — 5e level-6 enchantment, concentration,
/// action. Single-target WIS save against the caster's spell DC. On a
/// failed save, the target dances helplessly: zero movement, attack
/// disadvantage, auto-fail DEX saves, attackers get advantage. RAW
/// allows the target to spend an Action each turn to reattempt the
/// save; we collapse to a duration-bound install (10 rounds). Cleared
/// on concentration drop. No damage — pure control.
pub struct OttosIrresistibleDance {}

impl Action for OttosIrresistibleDance {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Enchantment)
    }
    fn name(&self) -> &str {
        "otto's irresistible dance"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["dance", "ottos", "irresistible dance"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 30 ft RAW = 12 tiles.
        Some(12)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(6)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        // Bard / wizard / sorcerer-list spell — pick the higher of the
        // caster's mental abilities for the DC.
        let dc = caster.best_spell_save_dc([
            AbilityScoreType::Charisma,
            AbilityScoreType::Intelligence,
        ]);
        let save = encounter.roll_save_against_caster(target_id, AbilityScoreType::Wisdom, dc, caster_id);
        if save.passed() {
            encounter.log("  otto's dance: target resists the compulsion");
            return Vec::new();
        }
        encounter.log("  otto's dance: target capers helplessly");
        vec![
            Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Dancing,
                timer: ConditionTimer::Rounds(10),
            }),
            Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions(
                    "Otto's Irresistible Dance",
                    vec![(target_id, Condition::Dancing)],
                ),
            }),
        ]
    }
}

pub static OTTOS_IRRESISTIBLE_DANCE: LazyLock<OttosIrresistibleDance> =
    LazyLock::new(|| OttosIrresistibleDance {});

/// Maze — 5e level-8 conjuration, concentration, action. Single-target
/// banishment with no save (RAW gives the target an INT check each turn
/// to escape — we collapse to a duration-bound install). The target is
/// removed from the encounter for up to 10 rounds (1 minute RAW) — we
/// model with the `Mazed` condition which blocks all action economy +
/// movement, leaving the actor inert on the map. Concentration-bound.
pub struct Maze {}

impl Action for Maze {
    fn name(&self) -> &str {
        "maze"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["banish"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60 ft RAW = 24 tiles.
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(8)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        encounter.log("  maze: target vanishes into a labyrinthine demiplane");
        vec![
            Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Mazed,
                timer: ConditionTimer::Rounds(10),
            }),
            Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions(
                    "Maze",
                    vec![(target_id, Condition::Mazed)],
                ),
            }),
        ]
    }
}

pub static MAZE: LazyLock<Maze> = LazyLock::new(|| Maze {});

/// Fire Storm — 5e level-7 evocation, action. 20ft-radius sphere
/// centered on a point within 150ft (40 tiles). Every creature in the
/// burst makes a DEX save against the caster's spell DC (WIS for
/// druid/cleric, INT for wizard — we pick the caster's higher one);
/// on fail they take 7d10 fire, half on save. Allies in the radius are
/// spared (caster picks the silhouette of the storm per RAW) — we use
/// the standard `enemy_burst_targets` partition.
pub struct FireStorm {}

impl Action for FireStorm {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Evocation)
    }
    fn name(&self) -> &str {
        "fire storm"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["firestorm", "storm"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 4 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(40)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Fire]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(7)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        // Cleric / Druid → WIS, Wizard → INT. Pick the larger.
        let dc = caster.best_spell_save_dc([
            AbilityScoreType::Wisdom,
            AbilityScoreType::Intelligence,
        ]);
        // 5e RAW lets the caster shape the storm as ten contiguous 10ft
        // cubes — players use it to skirt allies. We approximate with
        // the enemy-only burst partition so allies in the radius are
        // spared (matches the load-bearing "caster chooses the
        // silhouette" intent of the spell).
        let (effects, _) = enemy_burst_save_for_half(
            encounter,
            caster_id,
            point,
            4,
            AbilityScoreType::Dexterity,
            dc,
            Dice::new(7, 10),
            DamageType::Fire,
            "fire storm",
        );
        effects
    }
}

pub static FIRE_STORM: LazyLock<FireStorm> = LazyLock::new(|| FireStorm {});

/// Eyebite — 5e level-6 necromancy, concentration, action. Single-target
/// WIS save against the caster's spell DC. RAW offers three eye options
/// (asleep / panicked / sickened); we pick `asleep` as the load-bearing
/// flavor since it's the strongest control. On fail the target falls
/// Asleep for up to 10 rounds (1 minute RAW). Sleep is woken by damage
/// per the engine's existing damage-on-Asleep hook, so the spell still
/// gives the target an escape. Concentration-bound on the caster.
pub struct Eyebite {}

impl Action for Eyebite {
    fn name(&self) -> &str {
        "eyebite"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["evil eye"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60 ft RAW = 24 tiles.
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(6)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        // Eyebite is on the bard / sorcerer / warlock / wizard list — pick
        // the caster's higher mental ability for the DC.
        let dc = caster.best_spell_save_dc([
            AbilityScoreType::Charisma,
            AbilityScoreType::Intelligence,
        ]);
        let save = encounter.roll_save_against_caster(target_id, AbilityScoreType::Wisdom, dc, caster_id);
        if save.passed() {
            encounter.log("  eyebite: target shakes off the evil eye");
            return Vec::new();
        }
        encounter.log("  eyebite: target collapses into a magical slumber");
        // Two parallel conditions: Asleep (load-bearing mechanics —
        // action-economy block + melee-crit-on-hit) and EyebittenSick
        // (concentration mark; only used to link the spell to the
        // target for cleanup on drop).
        vec![
            Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Asleep,
                timer: ConditionTimer::Rounds(10),
            }),
            Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::EyebittenSick,
                timer: ConditionTimer::Rounds(10),
            }),
            Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions(
                    "Eyebite",
                    vec![(target_id, Condition::Asleep), (target_id, Condition::EyebittenSick)],
                ),
            }),
        ]
    }
}

pub static EYEBITE: LazyLock<Eyebite> = LazyLock::new(|| Eyebite {});

/// Conjure Animals — 5e level-3 conjuration, concentration, action.
/// Summons two CR-1/4 wolves on adjacent tiles to the caster, joining
/// the caster's team. We collapse the RAW "1 CR-2 / 2 CR-1 / 4 CR-1/2
/// / 8 CR-1/4" option table to the 2-wolf branch since it's the load-
/// bearing flavor for a level-3 cast and our wolf is already on the
/// books. Each conjured wolf gets the Conjured condition so dropping
/// concentration prunes them via the engine's cleanup hook.
/// Concentration-bound on the caster.
pub struct ConjureAnimals {}

impl Action for ConjureAnimals {
    fn summons_allies(&self) -> bool {
        true
    }
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Conjuration)
    }
    fn name(&self) -> &str {
        "conjure animals"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["conjure", "summon"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(3)
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // Need at least one free Medium slot adjacent to the caster —
        // the second wolf is best-effort (the spell still resolves with
        // one conjured ally if only one slot is available).
        encounter
            .find_adjacent_spawn(caster_id, crate::engine::types::Size::Medium, 2)
            .is_some()
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::actors::creatures::wolves::WOLF_TEMPLATE;

        // Two spectral wolves on free adjacent slots (radius 3 — RAW
        // 30 ft envelope). Shared summon helper handles team lookup,
        // adjacent-spawn search, instantiate + log chain.
        let spawned = spawn_adjacent_summons(
            encounter,
            caster_id,
            &WOLF_TEMPLATE,
            crate::engine::types::Size::Medium,
            2,
            3,
            90,
            "conjure animals",
        );
        if spawned.is_empty() {
            return Vec::new();
        }
        conjured_summon_concentration_effects(caster_id, &spawned, "Conjure Animals")
    }
}

pub static CONJURE_ANIMALS: LazyLock<ConjureAnimals> = LazyLock::new(|| ConjureAnimals {});

/// Conjure Elemental — 5e level-5 conjuration, concentration, action.
/// The caster summons a single Large elemental ally. RAW lets the caster
/// pick an element matching nearby terrain (water, fire, air, earth); the
/// engine collapses this to a single Fire Elemental conjuration since
/// the FIRE_ELEMENTAL_TEMPLATE already sits in the live creature pool
/// and its melee touch + Burning rider matches the spell's "destructive
/// elemental ally" feel.
///
/// Sibling to Conjure Animals (lv3, 2× Medium wolves) on the summon
/// lane — Conjure Elemental costs a higher slot for a single
/// CR-5 Large minion with fire immunity / poison immunity and
/// resistance to non-magical physical damage. The Conjured-on-drop
/// despawn path (added in `drop_concentration`) cleans up the
/// elemental when the caster's concentration ends. Touch range RAW —
/// the elemental appears in an unoccupied space within 90 ft RAW; we
/// reuse the same adjacent-spawn search the rest of the summon family
/// uses, search radius widened to 4 to accommodate the Large
/// footprint.
pub struct ConjureElemental {}

impl Action for ConjureElemental {
    fn summons_allies(&self) -> bool {
        true
    }
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Conjuration)
    }
    fn name(&self) -> &str {
        "conjure elemental"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["conjure-elem", "ce"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(5)
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // Need a free Large (4-tile) slot adjacent to the caster.
        encounter
            .find_adjacent_spawn(caster_id, crate::engine::types::Size::Large, 4)
            .is_some()
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::actors::creatures::fire_elementals::FIRE_ELEMENTAL_TEMPLATE;

        // Single Large elemental ally. Shared summon helper handles
        // team lookup, adjacent-spawn search, instantiate + log.
        let spawned = spawn_adjacent_summons(
            encounter,
            caster_id,
            &FIRE_ELEMENTAL_TEMPLATE,
            crate::engine::types::Size::Large,
            1,
            4,
            80,
            "conjure elemental",
        );
        if spawned.is_empty() {
            return Vec::new();
        }
        conjured_summon_concentration_effects(caster_id, &spawned, "Conjure Elemental")
    }
}

pub static CONJURE_ELEMENTAL: LazyLock<ConjureElemental> = LazyLock::new(|| ConjureElemental {});

/// Power Word Pain — 5e level-7 necromancy (XGtE), action,
/// concentration. Single target within 24 tiles (60 ft); no save, no
/// attack roll — but the spell only takes effect if the target has
/// 100 HP or fewer at cast time. RAW: target is racked with excruciating
/// pain — attack rolls suffer disadvantage and walking speed halves;
/// at the end of each of the target's turns the target attempts a CON
/// save vs the caster's spell DC to shake the pain off.
///
/// Applies the dedicated `PowerWordPained` condition rather than
/// reusing `Slowed`: the pain rides the pure attack-disadvantage +
/// half-speed lane (no unrelated -2 AC / -2 DEX-save penalty from
/// Slowed's compound envelope), matching RAW. The per-turn CON-save-
/// to-break loop rides the shared `ROUND_END_SAVES` table next to
/// Hold Person's WIS-vs-Stunned save and Flesh to Stone's CON-vs-
/// Petrified save — same save-then-clear-and-drop-concentration
/// semantics on a fresh save/condition axis. The half-speed multiplier
/// folds into the shared `CONDITION_SPEED_MULTIPLIERS` table (a
/// sibling row to Hasted ×2 and Slowed ×½) so cross-cast stacking
/// composes correctly.
///
/// HP-threshold spells are rare in the engine — most spells gate on
/// save or HP-percent rather than absolute HP. Power Word Pain is one
/// of the canonical Power Word family (Stun ≤150, Kill ≤100, Pain
/// ≤100) so we keep the threshold literal and log the gate explicitly
/// so a play-through can see why a Tarrasque shrugs it off and a
/// level-3 fighter doesn't.
pub struct PowerWordPain {}

impl Action for PowerWordPain {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Enchantment)
    }
    fn name(&self) -> &str {
        "power word pain"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["pwp", "pain"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60ft = 24 tiles.
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(7)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(target) = encounter.actors.get(&target_id) else {
            return Vec::new();
        };
        let hp = target.hitpoints();
        if hp > 100 {
            encounter.log(format!(
                "  power word pain: target has {} HP (>100), shrugged off",
                hp
            ));
            return Vec::new();
        }
        encounter.log(format!(
            "  power word pain: target has {} HP, racked with pain",
            hp
        ));
        // 100-round timer serves as the RAW "up to 1 minute" cap; the
        // per-turn CON save via `ROUND_END_SAVES` is the primary
        // break-free path, and concentration drop cleans up the tail
        // (`StartConcentration` with the (target, PowerWordPained)
        // pair wires both).
        vec![
            Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::PowerWordPained,
                timer: ConditionTimer::Rounds(100),
            }),
            Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions(
                    "Power Word Pain",
                    vec![(target_id, Condition::PowerWordPained)],
                ),
            }),
        ]
    }
}

pub static POWER_WORD_PAIN: LazyLock<PowerWordPain> = LazyLock::new(|| PowerWordPain {});

/// Mass Polymorph — 5e level-9 transmutation, concentration, action.
/// Burst variant of Polymorph: every enemy whose footprint touches a
/// 30ft radius around the chosen point makes a WIS save against the
/// caster's spell save DC. On fail, the target is Polymorphed (loses
/// its action economy, AC/speed default, etc., per the condition's
/// existing wiring) and gains 30 temp HP (the beast pool). Allies in
/// the radius are spared — friendly polymorphs are RAW willing, but
/// the AI would auto-fail-save them which makes the burst variant
/// strictly hostile in practice. Concentration drops all polymorphs at
/// once.
pub struct MassPolymorph {}

impl Action for MassPolymorph {
    fn name(&self) -> &str {
        "mass polymorph"
    }

    fn deals_damage(&self) -> bool {
        // Control, not damage. The AI's focus-fire lane scores by who
        // drops soonest, so an action that claims damage and deals none
        // gets picked over the attack that would have.
        false
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["mpoly", "mass morph"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 6 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 120 ft to burst origin.
        Some(48)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(9)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(origin) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spellcasting_save_dc();
        let targets = encounter.enemy_burst_targets(caster_id, origin, 6);
        if targets.is_empty() {
            return Vec::new();
        }
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        let mut tagged: Vec<(usize, Condition)> = Vec::new();
        for target_id in targets {
            let save = encounter.roll_save_against_caster(target_id, AbilityScoreType::Wisdom, dc, caster_id);
            if save.passed() {
                encounter.log(format!(
                    "  mass polymorph: actor #{} resists the transformation",
                    target_id
                ));
                continue;
            }
            encounter.log(format!(
                "  mass polymorph: actor #{} morphs into a beast",
                target_id
            ));
            effects.push(Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Polymorphed,
                timer: ConditionTimer::Permanent,
            }));
            effects.push(Box::new(GainTempHp {
                actor_id: target_id,
                amount: 30,
            }));
            tagged.push((target_id, Condition::Polymorphed));
        }
        if tagged.is_empty() {
            return effects;
        }
        effects.push(Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData::with_conditions("Mass Polymorph", tagged),
        }));
        effects
    }
}

pub static MASS_POLYMORPH: LazyLock<MassPolymorph> = LazyLock::new(|| MassPolymorph {});

/// Mordenkainen's Sword — 5e level-7 evocation, concentration, action.
/// RAW: a sword of force appears within range; on cast and then each
/// subsequent turn as a bonus action you can swing it for 3d10 force
/// damage on hit. The "bonus-action recurring attack" pattern is
/// awkward in this engine's one-action-per-spell model, so we collapse
/// to a heavier on-cast hit (5d10 force, melee spell attack) plus a
/// concentration mark that the AI can drop and re-cast — close enough
/// in damage budget to one cast + ~3-4 sustained sword swings RAW.
/// The mark also primes the Slowed condition on a hit (force is
/// gravitically dense in 5e flavor — RAW Mordenkainen's "sword" cuts
/// motion as well as flesh). Single-target.
pub struct MordenkainensSword {}

impl Action for MordenkainensSword {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Evocation)
    }
    fn name(&self) -> &str {
        "mordenkainen's sword"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["mord", "force sword", "sword"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60ft to the conjure point + 5ft reach for the sword itself —
        // we collapse to a flat 24-tile spell range.
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Force]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(7)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        // Spell uses the caster's best mental ability — wizards (INT),
        // sorcerers (CHA), and warlocks (CHA) all get Mordenkainen's
        // Sword on their published lists.
        let attack_bonus = caster.spellcasting_attack_modifier();
        // 5d10 force on hit — a melee spell attack, so reach + footprint
        // adjacency apply via resolve_attack's melee path.
        let mut effects = spell_attack_with_bonus(
            encounter,
            caster_id,
            target_id,
            "mordenkainen's sword",
            attack_bonus,
            Dice::new(5, 10),
            0,
            DamageType::Force,
            true,
        );
        // Concentration mark — drops on damage / next concentration cast.
        // We always install regardless of hit/miss (RAW: the sword
        // persists for the duration even if the first swing whiffs).
        effects.push(Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData::new("Mordenkainen's Sword"),
        }));
        effects
    }
}

pub static MORDENKAINENS_SWORD: LazyLock<MordenkainensSword> =
    LazyLock::new(|| MordenkainensSword {});

/// Frostbite — 5e cantrip, evocation. Single target, CON save vs the
/// caster's spell save DC. On fail: 1d6 cold damage and the target has
/// disadvantage on its next weapon attack (we approximate via Slowed
/// for a 1-round timer — Slowed already wires up DEX-save disadvantage
/// and the AC penalty, close enough flavor for the cantrip). On
/// success: nothing. Caster's best of INT / WIS / CHA save DC, since
/// druid / sorcerer / wizard / warlock all share the cantrip.
pub struct Frostbite {}

impl Action for Frostbite {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Evocation)
    }
    fn name(&self) -> &str {
        "frostbite"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["frost", "fb"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60ft = 24 tiles.
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Cold]
    }
    // Cantrip — uses the default `cost()` (single Action, no spell slot).
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.best_spell_save_dc([
            AbilityScoreType::Intelligence,
            AbilityScoreType::Wisdom,
            AbilityScoreType::Charisma,
        ]);
        let n = crate::engine::util::cantrip_dice_count(
            encounter.actors.get(&caster_id).map(|a| a.level()).unwrap_or(1),
        );
        let save = encounter.roll_save_against_caster(target_id, AbilityScoreType::Constitution, dc, caster_id);
        if save.passed() {
            encounter.log("  frostbite: target shrugs off the chill");
            return Vec::new();
        }
        let die = Dice::new(n, 6);
        // Route the roll through the shared caster-aware chokepoint
        // rather than `roll` directly: that is where the Sorcerer's
        // Empowered Spell reroll, the Evocation Wizard's Empowered
        // Evocation bonus and the cleric's Potent Spellcasting bonus
        // all live. A bare `roll` here meant every one of them
        // silently skipped this cantrip.
        let raw = encounter.roll_empowered_sum(caster_id, die.count, die.faces);
        encounter.log(format!("  frostbite: {}({}) = {} cold", die, raw, raw));
        vec![
            Box::new(DealDamage {
                actor_id: target_id,
                amount: raw,
                damage_type: DamageType::Cold,
            }),
            // Single-round disadvantage on the next attack — Slowed
            // already encodes the AC penalty + DEX save disadvantage.
            Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Slowed,
                timer: ConditionTimer::Rounds(1),
            }),
        ]
    }
}

pub static FROSTBITE: LazyLock<Frostbite> = LazyLock::new(|| Frostbite {});

/// Negative Energy Flood — 5e level-5 necromancy, action. Single target;
/// CON save vs the caster's spell save DC. On fail: 5d12 necrotic
/// damage. On success: half. The signature flavor is the "rises as a
/// zombie if the target drops" clause — we don't model post-death
/// raise here (the engine's Animate Dead spell covers that path), but
/// the headline 5d12 burst lands either way. Force-resistant /
/// necrotic-immune actors (vampires, wights, etc.) shrug damage off
/// per the engine's resistance table.
pub struct NegativeEnergyFlood {}

impl Action for NegativeEnergyFlood {
    fn name(&self) -> &str {
        "negative energy flood"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["nef", "negflood"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60ft = 24 tiles.
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Necrotic]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(5)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.best_spell_save_dc([
            AbilityScoreType::Intelligence,
            AbilityScoreType::Charisma,
        ]);
        let (dmg, _) = save_for_half_damage(
            encounter,
            caster_id,
            target_id,
            AbilityScoreType::Constitution,
            dc,
            Dice::new(5, 12),
            DamageType::Necrotic,
            "negative energy flood",
        );
        if dmg == 0 {
            return Vec::new();
        }
        vec![Box::new(DealDamage {
            actor_id: target_id,
            amount: dmg,
            damage_type: DamageType::Necrotic,
        })]
    }
}

pub static NEGATIVE_ENERGY_FLOOD: LazyLock<NegativeEnergyFlood> =
    LazyLock::new(|| NegativeEnergyFlood {});

/// Armor of Agathys — 5e level-1 abjuration (Warlock signature). Self-only
/// buff: caster gains 5 temp HP per spell level and any creature that hits
/// them with a melee attack takes cold damage in retaliation. The temp HP
/// IS the shield — once the pool is drained, the retaliation rider drops
/// with it (handled in `DealDamage::apply` — when temp HP is exhausted and
/// `AgathysShielded` is up, the condition is stripped so subsequent melee
/// hits don't free-trigger off a depleted shield).
///
/// Upcasting: +5 temp HP per slot level above 1 (5 at lv1, 10 at lv2,
/// 15 at lv3, etc.). Concentration-free; flat Rounds timer.
pub struct ArmorOfAgathys {}

impl Action for ArmorOfAgathys {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Abjuration)
    }
    fn name(&self) -> &str {
        "armor of agathys"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["agathys", "aoa"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(crate::engine::action_overrides::cast_level(overrides, 1))
    }
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        // Upcasting: 5 temp HP per spell level (5 at lv1, 10 at lv2, etc.)
        let lvl = crate::engine::action_overrides::cast_level(overrides, 1);
        let temp_hp = 5 * lvl;
        vec![
            Box::new(GainTempHp {
                actor_id: caster_id,
                amount: temp_hp,
            }),
            Box::new(ApplyCondition {
                actor_id: caster_id,
                condition: Condition::AgathysShielded,
                timer: ConditionTimer::Rounds(10),
            }),
        ]
    }
}

pub static ARMOR_OF_AGATHYS: LazyLock<ArmorOfAgathys> = LazyLock::new(|| ArmorOfAgathys {});

/// Sickening Radiance — 5e level-4 evocation, concentration. RAW: a 30ft
/// sphere of dim radiant light persists for the spell's duration; every
/// creature inside that fails a CON save each round takes 4d10 radiant
/// and gains a level of exhaustion. We collapse the sustained zone into
/// a one-shot burst at cast time: every enemy in the 30ft radius rolls
/// CON; on fail they eat the full 4d10 radiant AND gain `Exhausted`
/// (engine's single-tier exhaustion). The concentration mark holds so
/// dropping it can prune the exhaustion later if the AI swaps focus.
/// Excludes allies (typical 5e gotcha — RAW hits everyone in the zone,
/// but enemy-only is the load-bearing tactical use). Damage and save are
/// rolled per-target (independent CON saves per RAW); the `Exhausted`
/// install is paired with the `SickeningRadiated` marker for the
/// concentration cleanup hook.
pub struct SickeningRadiance {}

impl Action for SickeningRadiance {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Evocation)
    }
    fn name(&self) -> &str {
        "sickening radiance"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["sr", "sickening"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 6 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 120ft range to the burst origin.
        Some(48)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Radiant]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(4)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(center) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.best_spell_save_dc([
            AbilityScoreType::Intelligence,
            AbilityScoreType::Wisdom,
            AbilityScoreType::Charisma,
        ]);
        // 30ft = 6 tile gap. Enemy-only partition matches the burst's
        // tactical use; allies caught in the zone are spared per
        // standard engine convention.
        let targets = encounter.enemy_burst_targets(caster_id, center, 6);
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        let mut conditions: Vec<(usize, Condition)> = Vec::new();
        for tid in targets {
            // Deliberately a bare `roll` and the only damage roll in
            // this file that stays one. `roll_empowered_sum` carries
            // flat per-*cast* bonuses (Empowered Evocation's +INT,
            // Potent Spellcasting's +WIS) and a per-*cast* metamagic
            // reroll, and this is the one burst that rolls fresh dice
            // inside the per-target loop rather than sharing a single
            // roll — so routing it through the chokepoint would pay the
            // flat bonus once per victim and burn the Empowered Spell
            // prime on whichever target happened to be first in the
            // sorted order. RAW's unit for all three features is the
            // spell, not the target.
            let raw = encounter.roll(&Dice::new(4, 10));
            let save = encounter.roll_save_against_caster(tid, AbilityScoreType::Constitution, dc, caster_id);
            encounter.log(format!(
                "  sickening radiance: 4d10({}) radiant ({})",
                raw,
                if save.passed() { "save" } else { "fail" }
            ));
            if save.passed() {
                continue;
            }
            effects.push(Box::new(DealDamage {
                actor_id: tid,
                amount: raw,
                damage_type: DamageType::Radiant,
            }));
            effects.push(Box::new(ApplyCondition {
                actor_id: tid,
                condition: Condition::Exhausted,
                timer: ConditionTimer::Rounds(10),
            }));
            effects.push(Box::new(ApplyCondition {
                actor_id: tid,
                condition: Condition::SickeningRadiated,
                timer: ConditionTimer::Rounds(10),
            }));
            conditions.push((tid, Condition::Exhausted));
            conditions.push((tid, Condition::SickeningRadiated));
        }
        effects.push(Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData::with_conditions("Sickening Radiance", conditions),
        }));
        effects
    }
}

pub static SICKENING_RADIANCE: LazyLock<SickeningRadiance> =
    LazyLock::new(|| SickeningRadiance {});

/// Bigby's Hand — level-5 evocation, concentration. The caster summons a
/// fist-sized spectral force-hand that follows them around mauling
/// targets. RAW exposes four activation modes (clenched fist, grasping
/// hand, forceful hand, interposing hand); we collapse to the headline
/// "+force damage on every attack" envelope — a persistent +1d10 force
/// rider on every weapon swing the caster lands (slots into the
/// OnHitRider table next to Crown of Stars). Self-buff, no targeting.
/// Concentration so re-casting drops cleanly.
pub struct BigbysHand {}

impl Action for BigbysHand {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Evocation)
    }
    fn name(&self) -> &str {
        "bigby's hand"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["bh", "bigby", "hand"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        // Damage lands via the per-hit rider, not directly on cast.
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(5)
    }
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        self_concentration_buff_effects(
            caster_id,
            "Bigby's Hand",
            Condition::BigbysHanded,
            ConditionTimer::Permanent,
        )
    }
}

pub static BIGBYS_HAND: LazyLock<BigbysHand> = LazyLock::new(|| BigbysHand {});

/// Tenser's Transformation — level-6 transmutation, concentration. The
/// caster channels arcane force into their body: they gain 50 temp HP
/// (the explicit RAW "temporary hit points" lane) AND advantage on
/// weapon attack rolls for the duration (the `Transformed` condition
/// joins `grants_self_attack_advantage`). RAW additionally gives +2d12
/// force damage on weapon hits and proficiency in all weapons / CON
/// saves; we skip those clauses since they'd need per-class weapon
/// bookkeeping. The 50 temp HP + advantage envelope is load-bearing
/// enough to make the spell shine. Self-only.
pub struct TensersTransformation {}

impl Action for TensersTransformation {
    fn name(&self) -> &str {
        "tenser's transformation"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["tt", "tenser", "transformation"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(6)
    }
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = vec![Box::new(GainTempHp {
            actor_id: caster_id,
            amount: 50,
        })];
        effects.extend(self_concentration_buff_effects(
            caster_id,
            "Tenser's Transformation",
            Condition::Transformed,
            ConditionTimer::Permanent,
        ));
        effects
    }
}

pub static TENSERS_TRANSFORMATION: LazyLock<TensersTransformation> =
    LazyLock::new(|| TensersTransformation {});

/// Aura of Life — level-4 paladin abjuration, concentration. The paladin
/// emits a 30-foot aura (6-tile radius); the caster and every ally
/// inside the aura gain the DeathWarded condition for the duration —
/// the next hit that would drop them to 0 HP instead leaves them at 1.
/// We collapse the RAW "max-HP restoration if reduced to 0" half into
/// the existing DeathWard mechanic so the aura plays well with the
/// engine's killing-blow interception path. Concentration-bound on the
/// caster; the aura's ally-only partition uses `ally_burst_targets`.
pub struct AuraOfLife {}

impl Action for AuraOfLife {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Abjuration)
    }
    fn name(&self) -> &str {
        "aura of life"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["aol", "aura-life"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn is_heal(&self) -> bool {
        // The aura grants death-ward protection — not strictly a heal,
        // but the AI's support pipeline should consider it alongside
        // healing actions when the party is low.
        true
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(4)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        // 30ft aura ≈ 6-tile radius; allies (including the caster) in the
        // burst receive DeathWarded for the spell's 10-round duration.
        // Concentration tracks every recipient so a single drop clears
        // the aura cleanly from the whole party.
        ally_aura_concentration_effects(
            encounter,
            caster_id,
            6,
            "Aura of Life",
            Condition::DeathWarded,
            ConditionTimer::Rounds(10),
        )
    }
}

pub static AURA_OF_LIFE: LazyLock<AuraOfLife> = LazyLock::new(|| AuraOfLife {});

/// Aura of Purity — level-4 paladin abjuration, concentration. The paladin
/// emits a 30ft protective aura; the caster and every ally inside pick up
/// the `Purified` condition for 10 rounds. While Purified, the holder is
/// dynamically immune to Charmed / Frightened / Poisoned installs (the
/// three "social/biological" debuffs) and gains resistance to poison
/// damage — folded into `dynamic_immunity_to` and `effective_damage`
/// respectively, same chokepoints as Heroic / MindBlanked / Globed.
///
/// Slots between Aura of Life (death-ward, lv4) and Holy Aura (lv8 fiend-
/// rebuke disadvantage): the broader immunity profile makes it the
/// paladin's go-to opener vs charm-heavy / poison-heavy encounters
/// (hags, drow, spiders), while Aura of Life remains the lethality
/// emergency button. Both share the same `ally_aura_concentration_effects`
/// shape so the engine treats them identically for concentration drops.
pub struct AuraOfPurity {}

impl Action for AuraOfPurity {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Abjuration)
    }
    fn name(&self) -> &str {
        "aura of purity"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["aop", "aura-purity", "purity"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(4)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        // Same envelope as Aura of Life: 30ft (6-tile) ally-only burst,
        // 10-round (1-minute RAW) install, concentration tracks every
        // recipient.
        ally_aura_concentration_effects(
            encounter,
            caster_id,
            6,
            "Aura of Purity",
            Condition::Purified,
            ConditionTimer::Rounds(10),
        )
    }
}

pub static AURA_OF_PURITY: LazyLock<AuraOfPurity> = LazyLock::new(|| AuraOfPurity {});

/// Aganazzar's Scorcher — level-2 evocation. Roaring flames erupt in a
/// 30ft line (RAW); we approximate as a 3-tile burst at the target
/// point. Every creature in the area makes a DEX save vs the caster's
/// spell DC: fail = full 3d8 fire damage, pass = half. Shared damage
/// roll across all victims. Enemy-only partition keeps allies safe
/// inside the line — same simplification as Burning Hands / Cone of
/// Cold use. No concentration.
pub struct AganazzarsScorcher {}

impl Action for AganazzarsScorcher {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Evocation)
    }
    fn name(&self) -> &str {
        "aganazzar's scorcher"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["scorcher", "aganazzar"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 3 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 30ft RAW line — origin must be close to caster.
        Some(12)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Fire]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(2)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spell_save_dc(AbilityScoreType::Intelligence);
        let (effects, _) = enemy_burst_save_for_half(
            encounter,
            caster_id,
            point,
            3,
            AbilityScoreType::Dexterity,
            dc,
            Dice::new(3, 8),
            DamageType::Fire,
            "aganazzar's scorcher",
        );
        effects
    }
}

pub static AGANAZZARS_SCORCHER: LazyLock<AganazzarsScorcher> =
    LazyLock::new(|| AganazzarsScorcher {});

/// Acid Arrow — level-2 evocation. Ranged spell attack against one target:
/// 4d4 acid on hit, half damage on miss. Per RAW the spell also splashes
/// 2d4 acid "at the end of its next turn" on hit — we collapse that into
/// a single combined damage roll at cast time (4d4 immediate + 2d4
/// follow-up) so the engine doesn't need a per-actor deferred-damage
/// queue. Miss still drops the splash (consistent with the engine's
/// "miss does nothing but core damage" model).
///
/// Spellcasting ability defaults to INT (wizard primary); sorcerer
/// multiclass would CHA, but acid arrow lives on the wizard/sorcerer
/// list and INT is the safe default for both.
pub struct AcidArrow {}

impl Action for AcidArrow {
    fn name(&self) -> &str {
        "acid arrow"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["arrow", "aa"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 90ft RAW = 36 tiles.
        Some(36)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Acid]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(2)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let attack_bonus = caster.spellcasting_attack_modifier();
        // Combined 4d4 immediate + 2d4 splash → 6d4 on hit (5e RAW: full
        // immediate + full splash). Half damage on miss only covers the
        // splash dice per RAW: "half damage on a miss and no splash" →
        // 2d4 on miss. We model the miss-half as a manual fallback below
        // because `spell_attack` discards the miss path entirely.
        let (effects, dmg) = spell_attack_outcome(
            encounter,
            caster_id,
            target_id,
            "acid arrow",
            attack_bonus,
            Dice::new(4, 4),
            0,
            DamageType::Acid,
            false,
        );
        if dmg > 0 {
            // Hit: append the 2d4 splash. Logged separately so the
            // breakdown stays legible.
            let splash = encounter.roll_empowered_sum(caster_id, 2, 4);
            encounter.log(format!("  acid arrow splash: 2d4({}) acid", splash));
            let mut all = effects;
            all.push(Box::new(DealDamage {
                actor_id: target_id,
                amount: splash,
                damage_type: DamageType::Acid,
            }));
            return all;
        }
        // Miss: the splash still drips for half damage per RAW.
        let half_splash = encounter.roll_empowered_sum(caster_id, 2, 4) / 2;
        if half_splash == 0 {
            return effects;
        }
        encounter.log(format!(
            "  acid arrow miss splash: 2d4/2 = {} acid",
            half_splash
        ));
        let mut all = effects;
        all.push(Box::new(DealDamage {
            actor_id: target_id,
            amount: half_splash,
            damage_type: DamageType::Acid,
        }));
        all
    }
}

pub static ACID_ARROW: LazyLock<AcidArrow> = LazyLock::new(|| AcidArrow {});

/// Tidal Wave — level-3 conjuration. A 30-foot cube of water crashes
/// down: every creature in the area makes a DEX save vs the caster's
/// spell DC. Fail: 4d8 bludgeoning + Prone (the wave sweeps them off
/// their feet). Pass: half damage, no prone. Allies are spared via the
/// enemy_burst_targets partition (the AI-friendliest simplification of
/// RAW's "everyone in the cube"). No concentration.
pub struct TidalWave {}

impl Action for TidalWave {
    fn name(&self) -> &str {
        "tidal wave"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["wave", "tw"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        // 30ft cube ≈ 3-tile radius (Chebyshev).
        TargetingSchema::Burst { radius: 3 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 120ft to the burst origin = 48 tiles.
        Some(48)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Bludgeoning]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(3)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.best_spell_save_dc([
            AbilityScoreType::Intelligence,
            AbilityScoreType::Wisdom,
            AbilityScoreType::Charisma,
        ]);
        let (mut effects, saves) = enemy_burst_save_for_half(
            encounter,
            caster_id,
            point,
            3,
            AbilityScoreType::Dexterity,
            dc,
            Dice::new(4, 8),
            DamageType::Bludgeoning,
            "tidal wave",
        );
        // Failed-save targets are knocked Prone by the breaker wave.
        push_condition_on_failed_save(
            &mut effects,
            &saves,
            Condition::Prone,
            ConditionTimer::Permanent,
        );
        effects
    }
}

pub static TIDAL_WAVE: LazyLock<TidalWave> = LazyLock::new(|| TidalWave {});

/// Dawn — level-5 evocation, concentration. The caster summons a 30ft-
/// radius cylinder of sunlight. Every enemy in the area makes a CON
/// save: fail = full 4d10 radiant, pass = half. We collapse the
/// per-round sustained-cylinder RAW into a one-shot install at cast
/// time (matches our Sickening Radiance simplification). Concentration-
/// bound on the caster; dropping concentration ends the dawn.
pub struct Dawn {}

impl Action for Dawn {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Evocation)
    }
    fn name(&self) -> &str {
        "dawn"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["sunlight"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        // 30ft radius ≈ 6-tile Chebyshev burst.
        TargetingSchema::Burst { radius: 6 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60ft to the burst origin = 24 tiles.
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Radiant]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(5)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.best_spell_save_dc([
            AbilityScoreType::Wisdom,
            AbilityScoreType::Intelligence,
            AbilityScoreType::Charisma,
        ]);
        let (mut effects, _) = enemy_burst_save_for_half(
            encounter,
            caster_id,
            point,
            6,
            AbilityScoreType::Constitution,
            dc,
            Dice::new(4, 10),
            DamageType::Radiant,
            "dawn",
        );
        // Concentration mark — dropping cleans up the dawn marker.
        effects.push(Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData::new("Dawn"),
        }));
        effects
    }
}

pub static DAWN: LazyLock<Dawn> = LazyLock::new(|| Dawn {});

/// Mental Prison — level-6 illusion, concentration. Single target makes
/// an INT save vs the caster's spell DC. Fail: 5d10 psychic + the target
/// is `MentallyImprisoned` for the duration (movement zero, attacks with
/// disadvantage, attackers gain advantage — the full Restrained envelope
/// plus the illusory-prison flavor). Pass: half damage, no prison.
/// Concentration-bound on the caster. A target who fails the save can
/// try again at the end of each of their turns RAW — we collapse to a
/// duration-bound install for simplicity. The 5d10 hits psychic, so
/// psychic immunity (Mind Flayer / Death Knight) cleanly zero-ifies the
/// damage without breaking the imprison effect.
pub struct MentalPrison {}

impl Action for MentalPrison {
    fn name(&self) -> &str {
        "mental prison"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["mp", "prison"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60ft RAW = 24 tiles.
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Psychic]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(6)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.best_spell_save_dc([
            AbilityScoreType::Intelligence,
            AbilityScoreType::Charisma,
            AbilityScoreType::Wisdom,
        ]);
        let (dmg, passed) = save_for_half_damage(
            encounter,
            caster_id,
            target_id,
            AbilityScoreType::Intelligence,
            dc,
            Dice::new(5, 10),
            DamageType::Psychic,
            "mental prison",
        );
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        if dmg > 0 {
            effects.push(Box::new(DealDamage {
                actor_id: target_id,
                amount: dmg,
                damage_type: DamageType::Psychic,
            }));
        }
        let mut conditions: Vec<(usize, Condition)> = Vec::new();
        if !passed {
            effects.push(Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::MentallyImprisoned,
                timer: ConditionTimer::Rounds(10),
            }));
            conditions.push((target_id, Condition::MentallyImprisoned));
        }
        effects.push(Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData::with_conditions("Mental Prison", conditions),
        }));
        effects
    }
}

pub static MENTAL_PRISON: LazyLock<MentalPrison> = LazyLock::new(|| MentalPrison {});

/// Investiture of Flame — level-6 transmutation, concentration. The
/// caster wreathes themselves in flames: they gain resistance to fire
/// (read by `effective_damage`'s Invested-in-Flame branch), and every
/// melee attacker takes 1d10 fire damage in retaliation (handled by the
/// `resolve_attack` reflect alongside Fire Shield / Armor of Agathys).
/// Self-only, concentration-bound; no save / no target. The 4d8 fire
/// emanation rider in RAW is omitted — the load-bearing buff is the
/// resistance + melee retaliation envelope.
pub struct InvestitureOfFlame {}

impl Action for InvestitureOfFlame {
    fn name(&self) -> &str {
        "investiture of flame"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["iof", "flameinvest"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(6)
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // Already invested → don't re-cast and burn a level-6 slot.
        actor_lacks_condition(encounter, caster_id, Condition::InvestedInFlame)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        encounter.log("  investiture of flame: your body erupts in flame.".to_string());
        self_concentration_buff_effects(
            caster_id,
            "Investiture of Flame",
            Condition::InvestedInFlame,
            ConditionTimer::Rounds(10),
        )
    }
}

pub static INVESTITURE_OF_FLAME: LazyLock<InvestitureOfFlame> =
    LazyLock::new(|| InvestitureOfFlame {});

/// Grease — level-1 conjuration. A 10-foot square of slick grease coats
/// the ground at a point within 60 ft. Every enemy whose footprint
/// touches the burst makes a DEX save vs the caster's spell DC; failure
/// knocks them Prone (the difficult-terrain half of RAW is omitted — the
/// load-bearing penalty is the prone). Allies are spared via the
/// enemy_burst_targets partition (the spell is centered by the caster,
/// not a friendly-fire AoE in our model). No concentration; the slick
/// surface lasts a flat 10-round Rounds timer (1 minute RAW). The
/// caster doesn't *need* to do anything else — the prone is the entire
/// payload, matching the spell's reputation as a cheap lv1 disabler.
pub struct Grease {}

impl Action for Grease {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Conjuration)
    }
    fn name(&self) -> &str {
        "grease"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["slick", "slip"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        // 10ft square ≈ 2-tile Chebyshev burst (the grease covers a
        // 2x2-tile patch in 2.5ft squares).
        TargetingSchema::Burst { radius: 2 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60 ft = 24 tiles.
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(1)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.best_spell_save_dc([
            AbilityScoreType::Intelligence,
            AbilityScoreType::Wisdom,
            AbilityScoreType::Charisma,
        ]);
        const RADIUS: isize = 2;
        encounter.log(format!("  grease: slick patch at {} (DC {})", point, dc));
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        for tid in encounter.enemy_burst_targets(caster_id, point, RADIUS) {
            let save = encounter.roll_save_against_caster(tid, AbilityScoreType::Dexterity, dc, caster_id);
            if save.passed() {
                continue;
            }
            effects.push(Box::new(ApplyCondition {
                actor_id: tid,
                condition: Condition::Prone,
                timer: ConditionTimer::Permanent,
            }));
        }
        effects
    }
}

pub static GREASE: LazyLock<Grease> = LazyLock::new(|| Grease {});

/// Flaming Sphere — level-2 conjuration, concentration. The caster
/// conjures a 5-ft-radius ball of flame at a tile within 60 ft. Every
/// enemy whose footprint touches the burst makes a DEX save vs the
/// caster's spell DC: fail = full 2d6 fire, pass = half. RAW lets the
/// sphere be re-positioned each turn as a bonus action; we collapse the
/// per-round re-roll to the cast-time install (matches our Sickening
/// Radiance / Dawn simplification). Concentration-bound so the slot is
/// committed; dropping concentration ends the sphere cleanly.
pub struct FlamingSphere {}

impl Action for FlamingSphere {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Conjuration)
    }
    fn name(&self) -> &str {
        "flaming sphere"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["sphere", "fs-spell", "flameball"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        // 5ft-radius sphere ≈ 1-tile burst.
        TargetingSchema::Burst { radius: 1 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60 ft = 24 tiles.
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Fire]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(2)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.best_spell_save_dc([
            AbilityScoreType::Intelligence,
            AbilityScoreType::Wisdom,
            AbilityScoreType::Charisma,
        ]);
        let (mut effects, _) = enemy_burst_save_for_half(
            encounter,
            caster_id,
            point,
            1,
            AbilityScoreType::Dexterity,
            dc,
            Dice::new(2, 6),
            DamageType::Fire,
            "flaming sphere",
        );
        // Concentration mark — dropping cleans up the sphere marker.
        effects.push(Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData::new("Flaming Sphere"),
        }));
        effects
    }
}

pub static FLAMING_SPHERE: LazyLock<FlamingSphere> = LazyLock::new(|| FlamingSphere {});

/// Guardian of Faith — level-4 conjuration. A spectral Large guardian
/// appears at a tile within 30 ft; any enemy that enters its 10-foot
/// reach takes 20 radiant damage (RAW: fiends / undead take 20, others
/// take 10 — we collapse to the high tier since our pool is
/// fiend / undead -heavy and the load-bearing flavor is "the guardian
/// punishes intruders"). DEX save vs the caster's spell DC halves the
/// damage. The guardian has a 60-HP / 8-hour budget in RAW; we model the
/// cast as a one-shot burst rather than a sustained presence. No
/// concentration in RAW.
pub struct GuardianOfFaith {}

impl Action for GuardianOfFaith {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Conjuration)
    }
    fn name(&self) -> &str {
        "guardian of faith"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["guardian", "gof"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        // 10ft reach ≈ 2-tile Chebyshev burst.
        TargetingSchema::Burst { radius: 2 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 30 ft = 12 tiles.
        Some(12)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Radiant]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(4)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.best_spell_save_dc([
            AbilityScoreType::Wisdom,
            AbilityScoreType::Charisma,
        ]);
        // Flat 20 radiant on fail, 10 on save — no dice roll per RAW.
        encounter.log(format!(
            "  guardian of faith: 20 radiant at {} (DC {})",
            point, dc
        ));
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        for tid in encounter.enemy_burst_targets(caster_id, point, 2) {
            let save = encounter.roll_save_against_caster(tid, AbilityScoreType::Dexterity, dc, caster_id);
            let dmg = if save.passed() { 10 } else { 20 };
            effects.push(Box::new(DealDamage {
                actor_id: tid,
                amount: dmg,
                damage_type: DamageType::Radiant,
            }));
        }
        effects
    }
}

pub static GUARDIAN_OF_FAITH: LazyLock<GuardianOfFaith> = LazyLock::new(|| GuardianOfFaith {});

/// Blade Barrier — level-6 evocation, concentration. A vertical wall of
/// whirling, razor-sharp blades springs into existence at a tile within
/// 90 ft. Every enemy whose footprint touches the burst makes a DEX save
/// vs the caster's spell DC: fail = full 6d10 slashing, pass = half. The
/// wall lingers (10 minutes RAW) — we collapse to the cast-time install
/// and use concentration as the sustainment anchor. Allies are spared via
/// the enemy_burst_targets partition (the wall is a vertical surface; in
/// RAW the caster chooses its orientation so allies stand on the safe
/// side).
pub struct BladeBarrier {}

impl Action for BladeBarrier {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Evocation)
    }
    fn name(&self) -> &str {
        "blade barrier"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["bb", "blades", "barrier"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        // 100ft long, 20ft high wall ≈ 4-tile Chebyshev burst (we treat
        // the wall as a wide damage zone rather than a literal line so
        // the cast picker has a single tile to aim at).
        TargetingSchema::Burst { radius: 4 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 90 ft = 36 tiles.
        Some(36)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Slashing]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(6)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.best_spell_save_dc([
            AbilityScoreType::Wisdom,
            AbilityScoreType::Charisma,
        ]);
        let (mut effects, _) = enemy_burst_save_for_half(
            encounter,
            caster_id,
            point,
            4,
            AbilityScoreType::Dexterity,
            dc,
            Dice::new(6, 10),
            DamageType::Slashing,
            "blade barrier",
        );
        effects.push(Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData::new("Blade Barrier"),
        }));
        effects
    }
}

pub static BLADE_BARRIER: LazyLock<BladeBarrier> = LazyLock::new(|| BladeBarrier {});

/// Wind Wall — level-3 evocation, concentration. The caster conjures a
/// vertical curtain of strong wind on themselves. Ranged attacks against
/// the warded caster have disadvantage (RAW: arrows / bolts deflect,
/// gases dissipate) but melee swings are unaffected — the wall blocks
/// the air, not the blade. We model as a self-buff (`WindWalled`
/// condition) tied to concentration so dropping concentration ends the
/// wall cleanly. The is-melee gate lives on the engine's
/// `imposes_disadvantage_to_ranged_attackers` cohort.
pub struct WindWall {}

impl Action for WindWall {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Evocation)
    }
    fn name(&self) -> &str {
        "wind wall"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ww", "wind"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(3)
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // Don't burn a slot re-casting when the wall is already up.
        encounter
            .actors
            .get(&caster_id)
            .is_some_and(|a| a.is_combat_active() && !a.has_condition(Condition::WindWalled))
    }
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        // 10 rounds = 1 minute RAW.
        self_concentration_buff_effects(
            caster_id,
            "Wind Wall",
            Condition::WindWalled,
            ConditionTimer::Rounds(10),
        )
    }
}

pub static WIND_WALL: LazyLock<WindWall> = LazyLock::new(|| WindWall {});

/// Evard's Black Tentacles — level-4 conjuration, concentration. A
/// writhing mass of tentacles fills a 20-foot square (we model as a
/// 2-tile-radius burst centered on the caster's chosen tile). Every
/// enemy whose footprint touches the burst makes a DEX save vs the
/// caster's spell DC; on fail, they take 3d6 bludgeoning AND are
/// Restrained for the spell's duration. On save, they take half and
/// avoid the Restrained rider. Concentration-bound on the caster;
/// dropping concentration releases every restrained victim.
pub struct EvardsBlackTentacles {}

impl Action for EvardsBlackTentacles {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Conjuration)
    }
    fn name(&self) -> &str {
        "black tentacles"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["tentacles", "evards", "ebt"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        // 20ft square ≈ 2-tile Chebyshev burst (≈10ft radius).
        TargetingSchema::Burst { radius: 2 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 90 ft = 36 tiles.
        Some(36)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Bludgeoning]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(4)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.best_spell_save_dc([
            AbilityScoreType::Intelligence,
            AbilityScoreType::Wisdom,
            AbilityScoreType::Charisma,
        ]);
        // Failed-save targets take 3d6 bludgeoning AND pick up Restrained
        // for the duration; pass = half damage and no rider. The
        // concentration mark captures the restrained ids so dropping
        // concentration releases them cleanly. 10 rounds = 1 minute RAW.
        // Same recipe Caustic Brew / Wall of Light / Psychic Scream
        // route through — concentration_burst_with_rider keeps the
        // three-step burst/rider/anchor shape in one chokepoint.
        concentration_burst_with_rider(
            encounter,
            caster_id,
            point,
            2,
            AbilityScoreType::Dexterity,
            dc,
            Dice::new(3, 6),
            DamageType::Bludgeoning,
            "black tentacles",
            "Black Tentacles",
            Condition::Restrained,
            ConditionTimer::Rounds(10),
            SaveDamagePolicy::HalfOnSave,
        )
    }
}

pub static EVARDS_BLACK_TENTACLES: LazyLock<EvardsBlackTentacles> =
    LazyLock::new(|| EvardsBlackTentacles {});

/// Otiluke's Resilient Sphere — level-4 evocation, concentration. The
/// caster encases a single target in a sphere of force. The target
/// makes a DEX save vs the caster's spell DC; on fail, they're Sphered
/// (a full incapacitation envelope: zero movement, blocked action
/// economy, attacks against have advantage, attacks they make have
/// disadvantage). On save, the spell fizzles. Concentration-bound so
/// re-cast / drop concentration shatters the sphere cleanly.
pub struct OtilukesResilientSphere {}

impl Action for OtilukesResilientSphere {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Abjuration)
    }
    fn name(&self) -> &str {
        "resilient sphere"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["sphere", "ors", "otilukes"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 30 ft = 12 tiles.
        Some(12)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(4)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.best_spell_save_dc([
            AbilityScoreType::Intelligence,
            AbilityScoreType::Wisdom,
            AbilityScoreType::Charisma,
        ]);
        let save = encounter.roll_save_against_caster(target_id, AbilityScoreType::Dexterity, dc, caster_id);
        if save.passed() {
            return Vec::new();
        }
        vec![
            Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Sphered,
                // 10 rounds = 1 minute RAW.
                timer: ConditionTimer::Rounds(10),
            }),
            Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions(
                    "Resilient Sphere",
                    vec![(target_id, Condition::Sphered)],
                ),
            }),
        ]
    }
}

pub static OTILUKES_RESILIENT_SPHERE: LazyLock<OtilukesResilientSphere> =
    LazyLock::new(|| OtilukesResilientSphere {});

/// Lightning Lure — sorcerer / warlock / wizard cantrip. Range 15 ft (6
/// tiles); the target makes a STR save vs the caster's spell DC. On
/// fail, the target is pulled up to 10 ft (4 tiles) in a straight line
/// toward the caster; if they end the pull within 5 ft (footprint-
/// adjacent), they take Nd8 lightning (scaling with caster level).
pub struct LightningLure {}

impl Action for LightningLure {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Evocation)
    }
    fn name(&self) -> &str {
        "lightning lure"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ll", "lure"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 15 ft = 6 tiles.
        Some(6)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Lightning]
    }
    // Cantrip — uses the default `cost()` (single Action, no slot).
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::side_effects::PullActor;
        use crate::engine::util::{footprint_chebyshev, get_tiles_from_size};

        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let caster_loc = caster.location();
        let dc = caster.best_spell_save_dc([
            AbilityScoreType::Intelligence,
            AbilityScoreType::Charisma,
        ]);
        let save = encounter.roll_save_against_caster(target_id, AbilityScoreType::Strength, dc, caster_id);
        if save.passed() {
            encounter.log("  lightning lure: target saves".to_string());
            return Vec::new();
        }
        // 5e RAW: pull up to 10 ft (4 tiles) toward the caster, then —
        // only if the target ends within 5 ft of the caster — deal the
        // damage. We resolve the pull eagerly here (mutating encounter
        // state) so the adjacency check uses the post-pull position;
        // the returned effect list carries only the damage step. The
        // PullActor helper honors wall / occupancy blocking — a target
        // pinned to an obstacle just doesn't move and the adjacency
        // gate skips the damage.
        PullActor {
            actor_id: target_id,
            toward: caster_loc,
            max_tiles: 4,
        }
        .apply(encounter);
        let adjacent = encounter
            .actors
            .get(&target_id)
            .map(|t| {
                footprint_chebyshev(
                    t.location(),
                    get_tiles_from_size(t.size()),
                    caster_loc,
                    1,
                ) <= 1
            })
            .unwrap_or(false);
        if !adjacent {
            encounter.log("  lightning lure: target pulled but stays out of reach".to_string());
            return Vec::new();
        }
        let n = crate::engine::util::cantrip_dice_count(
            encounter.actors.get(&caster_id).map(|a| a.level()).unwrap_or(1),
        );
        let die = Dice::new(n, 8);
        // Route the roll through the shared caster-aware chokepoint
        // rather than `roll` directly: that is where the Sorcerer's
        // Empowered Spell reroll, the Evocation Wizard's Empowered
        // Evocation bonus and the cleric's Potent Spellcasting bonus
        // all live. A bare `roll` here meant every one of them
        // silently skipped this cantrip.
        let raw = encounter.roll_empowered_sum(caster_id, die.count, die.faces);
        encounter.log(format!("  lightning lure: {}({}) lightning", die, raw));
        vec![Box::new(DealDamage {
            actor_id: target_id,
            amount: raw,
            damage_type: DamageType::Lightning,
        })]
    }
}

pub static LIGHTNING_LURE: LazyLock<LightningLure> = LazyLock::new(|| LightningLure {});

/// Shillelagh — druid cantrip (transmutation). The caster imbues their
/// melee weapon (RAW: club or quarterstaff) with sylvan magic, priming
/// the next melee weapon hit with +1d8 force damage. RAW also lets the
/// swing use WIS instead of STR for the attack / damage roll; we skip
/// the stat-swap (the rider damage is the load-bearing portion of the
/// buff). One-shot — the on-hit rider table strips the prime the moment
/// a melee swing lands. Concentration-free per RAW (the spell has a
/// 1-minute duration). Pairs with Thorn Whip on the druid's at-will
/// melee lane: free action + bonus action route.
pub struct Shillelagh {}

impl Action for Shillelagh {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Transmutation)
    }
    fn name(&self) -> &str {
        "shillelagh"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["shil", "club"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        // Like Divine Smite — the rider damage lands on the *next* hit,
        // not on this action's resolution. False keeps the AI's
        // focus-fire pipeline from picking the prime over an actual
        // attack.
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        // RAW: bonus action (cantrip). No slot consumed.
        bonus_action_only()
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // Don't burn a bonus action re-priming a still-active prime.
        encounter
            .actors
            .get(&caster_id)
            .is_some_and(|a| a.is_combat_active() && !a.has_condition(Condition::Shillelaghed))
    }
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        // 10-round prime window — RAW: 1 minute. Tick-down timer caps a
        // swing-less prime so it can't dangle across rests.
        vec![Box::new(ApplyCondition {
            actor_id: caster_id,
            condition: Condition::Shillelaghed,
            timer: ConditionTimer::Rounds(10),
        })]
    }
}

pub static SHILLELAGH: LazyLock<Shillelagh> = LazyLock::new(|| Shillelagh {});

/// Maximilian's Earthen Grasp — 2nd-level transmutation, concentration.
/// A man-sized fist of magical earth erupts under the target. They make
/// a STR save vs the caster's spell DC. On fail: 2d6 bludgeoning damage
/// and the target is grasped (`EarthenGrasped` envelope — Restrained
/// shape: zero movement, attack disadvantage, attacks against have
/// advantage). On save: the spell fizzles. While the spell holds
/// (concentration-bound on the caster), the fist crushes the target for
/// 2d6 bludgeoning at every round-end via the `ROUND_END_DOTS` table.
///
/// Dropping concentration releases the grip cleanly. Pairs well with
/// the druid / wizard's lv2 lane: a sticky single-target control that
/// trickles damage on every round-end the fist holds.
pub struct MaximiliansEarthenGrasp {}

impl Action for MaximiliansEarthenGrasp {
    fn name(&self) -> &str {
        "earthen grasp"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["grasp", "earth", "meg"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 30ft RAW = 12 tiles.
        Some(12)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Bludgeoning]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(2)
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // Don't burn a slot to replace our own concentration. The
        // AI's focus_fire pipeline tests every harmful single-target
        // spell against `validate_input`; the gate keeps Earthen Grasp
        // out of the picker when a more valuable buff already holds
        // the concentration slot.
        encounter.caster_can_concentrate(caster_id)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        // RAW: wizard / druid / sorcerer pick the spell up — INT / WIS /
        // CHA all viable. Best-of routes through the existing helper so
        // multi-class casters anchor on the right stat.
        let dc = caster.best_spell_save_dc([
            AbilityScoreType::Intelligence,
            AbilityScoreType::Wisdom,
            AbilityScoreType::Charisma,
        ]);
        let save = encounter.roll_save_against_caster(target_id, AbilityScoreType::Strength, dc, caster_id);
        if save.passed() {
            encounter.log("  earthen grasp: target saves, fist crumbles".to_string());
            return Vec::new();
        }
        let dmg = encounter.roll_empowered_sum(caster_id, 2, 6);
        encounter.log(format!(
            "  earthen grasp: 2d6({}) bludgeoning + Restrained",
            dmg
        ));
        vec![
            Box::new(DealDamage {
                actor_id: target_id,
                amount: dmg,
                damage_type: DamageType::Bludgeoning,
            }),
            Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::EarthenGrasped,
                // 10 rounds = 1 minute RAW. Concentration anchors the
                // real lifetime — dropping concentration releases the
                // target before the timer expires.
                timer: ConditionTimer::Rounds(10),
            }),
            Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions(
                    "Earthen Grasp",
                    vec![(target_id, Condition::EarthenGrasped)],
                ),
            }),
        ]
    }
}

pub static MAXIMILIANS_EARTHEN_GRASP: LazyLock<MaximiliansEarthenGrasp> =
    LazyLock::new(|| MaximiliansEarthenGrasp {});

/// Vitriolic Sphere — 4th-level evocation (sorcerer / wizard). A
/// brilliant green ball of acid bursts in a 20-foot sphere (4-tile
/// radius). Every enemy in the area makes a DEX save vs the caster's
/// spell DC. On fail: 10d4 acid damage immediately, plus 5d4 acid at the
/// next round-end (the `VitriolicAcidCoated` condition with a
/// `Rounds(1)` timer; the central `ROUND_END_DOTS` table handles the
/// drip). On save: half the immediate damage and no residual drip.
///
/// The delayed-drip rider makes the spell punish failed saves
/// significantly harder than a flat AoE, slotting cleanly between
/// Fireball (lv3, 8d6 immediate, no drip) and Cone of Cold (lv5, 8d8
/// immediate, no drip).
pub struct VitriolicSphere {}

impl Action for VitriolicSphere {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Evocation)
    }
    fn name(&self) -> &str {
        "vitriolic sphere"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["vitriol", "vs", "acid sphere"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        // 20ft RAW radius ≈ 4-tile Chebyshev burst.
        TargetingSchema::Burst { radius: 4 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 150ft RAW = 60 tiles — caps at our typical map size.
        Some(60)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Acid]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(4)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.best_spell_save_dc([
            AbilityScoreType::Intelligence,
            AbilityScoreType::Charisma,
        ]);
        // Shared immediate damage with save-for-half semantics. The
        // saves vector tells us which targets failed → those get the
        // VitriolicAcidCoated residual drip rider.
        let (mut effects, saves) = enemy_burst_save_for_half(
            encounter,
            caster_id,
            point,
            4,
            AbilityScoreType::Dexterity,
            dc,
            Dice::new(10, 4),
            DamageType::Acid,
            "vitriolic sphere",
        );
        // One-shot drip — the central round-end DoT rolls the 5d4
        // damage at round-end, then the Rounds(1) timer expires the flag.
        push_condition_on_failed_save(
            &mut effects,
            &saves,
            Condition::VitriolicAcidCoated,
            ConditionTimer::Rounds(1),
        );
        effects
    }
}

pub static VITRIOLIC_SPHERE: LazyLock<VitriolicSphere> =
    LazyLock::new(|| VitriolicSphere {});

/// Pick the damage type, from `candidates`, that lands the most raw HP on
/// `target_id`. Vulnerability beats nothing beats resistance beats immunity.
/// Ties prefer the earlier entry in `candidates` for deterministic logs.
/// Returns the first listed type if the target isn't found (silent fallback).
///
/// Used by Chromatic Orb to pick from its six-element damage menu, but
/// shaped generically so future "caster picks a damage type" spells
/// (Elemental Bane, Elemental Affinity sorcerer) can reuse it.
fn pick_damage_type_against_target(
    encounter: &EncounterInstance,
    target_id: usize,
    candidates: &[DamageType],
) -> DamageType {
    use crate::engine::types::DamageModifier;
    let Some(target) = encounter.actors.get(&target_id) else {
        return candidates[0];
    };
    // Score: vuln = 2, none = 1, resist = 0, immune = -1. Higher wins.
    let score = |dt: &&DamageType| -> i32 {
        match target.damage_modifier(**dt) {
            Some(DamageModifier::Vulnerability) => 2,
            None => 1,
            Some(DamageModifier::Resistance) => 0,
            Some(DamageModifier::Immunity) => -1,
        }
    };
    candidates
        .iter()
        .max_by_key(score)
        .copied()
        .unwrap_or(candidates[0])
}

/// Chromatic Orb — level-1 evocation (sorcerer / wizard). The caster hurls
/// a sphere of energy at a single target within 90 ft (36 tiles) for 3d8
/// damage of their choice from acid, cold, fire, lightning, poison, or
/// thunder. RAW: ranged spell attack (d20 + INT / CHA vs AC). On hit:
/// damage; on miss: nothing. We pick the type that maximizes effective
/// damage against the target (`pick_damage_type_against_target`) — the
/// caster's signature flexibility is exactly its strength.
pub struct ChromaticOrb {}

impl Action for ChromaticOrb {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Evocation)
    }
    fn name(&self) -> &str {
        "chromatic orb"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["co", "orb"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 90ft = 36 tiles.
        Some(36)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![
            DamageType::Acid,
            DamageType::Cold,
            DamageType::Fire,
            DamageType::Lightning,
            DamageType::Poison,
            DamageType::Thunder,
        ]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(1)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        // Pick caster spellcasting ability: prefer the higher of INT / CHA
        // so a sorcerer (CHA) and a wizard (INT) both benefit. WIS is not
        // in the list since RAW restricts Chromatic Orb to INT / CHA classes.
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let attack_bonus = caster.best_spell_attack_modifier([
            AbilityScoreType::Intelligence,
            AbilityScoreType::Charisma,
        ]);
        // Pick the damage type that maximizes effective damage on the
        // target. Acid first (most creatures resist nothing; some oozes
        // are immune which the picker handles).
        let dt = pick_damage_type_against_target(
            encounter,
            target_id,
            &[
                DamageType::Acid,
                DamageType::Cold,
                DamageType::Fire,
                DamageType::Lightning,
                DamageType::Poison,
                DamageType::Thunder,
            ],
        );
        let label = format!("chromatic orb ({:?})", dt);
        spell_attack(
            encounter,
            caster_id,
            target_id,
            &label,
            attack_bonus,
            Dice::new(3, 8),
            dt,
            false,
        )
    }
}

pub static CHROMATIC_ORB: LazyLock<ChromaticOrb> = LazyLock::new(|| ChromaticOrb {});

/// Snilloc's Snowball Swarm — level-2 evocation (sorcerer / wizard). A
/// flurry of magic snowballs explodes from a target point within 90 ft
/// (36 tiles). Every creature in a 5-foot-radius (1-tile) burst makes a
/// DEX save vs the caster's spell DC: pass = half, fail = full. 3d6 cold
/// damage. Shares the burst-save-for-half shape with Shatter / Burning
/// Hands — distinct from those by its cold typing and the small burst
/// radius (closer to Acid Splash's footprint than to Fireball's reach).
pub struct SnillocsSnowballSwarm {}

impl Action for SnillocsSnowballSwarm {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Evocation)
    }
    fn name(&self) -> &str {
        "snowball swarm"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["snowball", "snilloc", "sss"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        // 5ft radius = 1-tile burst on this grid.
        TargetingSchema::Burst { radius: 1 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 90ft = 36 tiles.
        Some(36)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Cold]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(2)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.best_spell_save_dc([
            AbilityScoreType::Intelligence,
            AbilityScoreType::Charisma,
        ]);
        let (effects, _saves) = enemy_burst_save_for_half(
            encounter,
            caster_id,
            point,
            1,
            AbilityScoreType::Dexterity,
            dc,
            Dice::new(3, 6),
            DamageType::Cold,
            "snowball swarm",
        );
        effects
    }
}

pub static SNILLOCS_SNOWBALL_SWARM: LazyLock<SnillocsSnowballSwarm> =
    LazyLock::new(|| SnillocsSnowballSwarm {});

/// Mind Spike — level-2 divination (sorcerer / warlock / wizard). The
/// caster drives a spike of psychic energy into a creature's mind. The
/// target makes a WIS save vs the caster's spell DC: pass = half, fail
/// = full. 3d8 psychic damage. RAW also lets the caster sense the
/// target's location for 1 hour; we skip the tracking rider since the
/// engine doesn't model fog-of-war. The single-target save-for-half
/// shape slots cleanly next to Mind Sliver (cantrip, save-for-flat-
/// debuff) and Psychic Lance (lv4, save-for-half + Incapacitated rider).
pub struct MindSpike {}

impl Action for MindSpike {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Divination)
    }
    fn name(&self) -> &str {
        "mind spike"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["mspike", "ms-spike"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60ft = 24 tiles.
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Psychic]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(2)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.best_spell_save_dc([
            AbilityScoreType::Intelligence,
            AbilityScoreType::Charisma,
        ]);
        let (dmg, _passed) = save_for_half_damage(
            encounter,
            caster_id,
            target_id,
            AbilityScoreType::Wisdom,
            dc,
            Dice::new(3, 8),
            DamageType::Psychic,
            "mind spike",
        );
        if dmg == 0 {
            return Vec::new();
        }
        vec![Box::new(DealDamage {
            actor_id: target_id,
            amount: dmg,
            damage_type: DamageType::Psychic,
        })]
    }
}

pub static MIND_SPIKE: LazyLock<MindSpike> = LazyLock::new(|| MindSpike {});

/// Psychic Lance — level-4 enchantment (bard / sorcerer / warlock /
/// wizard). The caster drives a beam of psychic energy into a single
/// creature within 120 ft (48 tiles). The target makes an INT save vs
/// the caster's spell DC: pass = half damage, fail = full damage AND
/// Incapacitated until the end of the caster's next turn. 7d6 psychic.
/// One of the few enchantments that lands hard control on a single
/// target with no concentration tax — pairs well with the existing
/// Maze / Otto's Dance lock-down lane but at a cheaper slot.
pub struct PsychicLance {}

impl Action for PsychicLance {
    fn name(&self) -> &str {
        "psychic lance"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["plance", "lance"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 120ft = 48 tiles.
        Some(48)
    }
    fn requires_los(&self) -> bool {
        // RAW: "a creature you can see, OR a creature you name or describe".
        // We require LOS to stay consistent with the rest of the targeted
        // spell list — the name/describe clause needs party-knowledge state
        // we don't model.
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Psychic]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(4)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.best_spell_save_dc([
            AbilityScoreType::Intelligence,
            AbilityScoreType::Charisma,
        ]);
        let (dmg, passed) = save_for_half_damage(
            encounter,
            caster_id,
            target_id,
            AbilityScoreType::Intelligence,
            dc,
            Dice::new(7, 6),
            DamageType::Psychic,
            "psychic lance",
        );
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        if dmg > 0 {
            effects.push(Box::new(DealDamage {
                actor_id: target_id,
                amount: dmg,
                damage_type: DamageType::Psychic,
            }));
        }
        // Incapacitated rider on a failed save — short timer so the
        // crowd-control runs out by the caster's next turn (RAW: ends at
        // the start of the caster's next turn). UntilStartOfNextTurn is
        // the holder's timer, not the caster's, but it's the closest
        // single-tick approximation; the AI doesn't lean on the precise
        // expiry tick.
        if !passed {
            effects.push(Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Incapacitated,
                timer: ConditionTimer::UntilStartOfNextTurn,
            }));
        }
        effects
    }
}

pub static PSYCHIC_LANCE: LazyLock<PsychicLance> = LazyLock::new(|| PsychicLance {});

/// Thunderclap — sorcerer / warlock / wizard cantrip. The caster claps
/// their hands; every creature within 5 ft (1-tile burst centered on
/// the caster, excluding the caster themselves) makes a CON save vs the
/// caster's spell DC. On fail: 1d6 thunder. On success: nothing
/// (cantrips don't half-on-save). Self-centered AoE — distinct from
/// Sacred Burst (targets a tile) and Acid Splash (also targets a tile,
/// hits one creature + adjacent). Doesn't require line-of-sight since
/// the wave radiates outward from the caster.
pub struct Thunderclap {}

impl Action for Thunderclap {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Evocation)
    }
    fn name(&self) -> &str {
        "thunderclap"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["tc", "clap"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Thunder]
    }
    // Cantrip — uses the default `cost()` (single Action, no spell slot).
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let center = caster.location();
        let dc = caster.best_spell_save_dc([
            AbilityScoreType::Intelligence,
            AbilityScoreType::Charisma,
            AbilityScoreType::Wisdom,
        ]);
        let n = crate::engine::util::cantrip_dice_count(caster.level());
        let (effects, _saves) = neutral_burst_save_only(
            encounter,
            caster_id,
            center,
            1,
            AbilityScoreType::Constitution,
            dc,
            Dice::new(n, 6),
            DamageType::Thunder,
            "thunderclap",
        );
        effects
    }
}

pub static THUNDERCLAP: LazyLock<Thunderclap> = LazyLock::new(|| Thunderclap {});

/// Guidance — cleric / druid cantrip (divination). Touch range; the
/// target gains the `Inspired` buff, adding +3 (the d4 / d6 average) to
/// their next attack roll or save. RAW gives +1d4 to one ability check
/// of the holder's choice — we approximate with the existing `Inspired`
/// flat-buff lane since the engine collapses checks / attacks / saves
/// into the same buff slot. Concentration-free per RAW (we model it as
/// a short Rounds timer so it can't dangle across the entire dungeon).
pub struct Guidance {}

impl Action for Guidance {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Divination)
    }
    fn name(&self) -> &str {
        "guidance"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["gd", "guide"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(crate::actions::action_template::MELEE_REACH)
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        _caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // Don't burn the action re-priming an already-inspired ally.
        let Some(target_id) = first_target_id(target_ids) else {
            return false;
        };
        actor_lacks_condition(encounter, target_id, Condition::Inspired)
    }
    // Cantrip — uses the default `cost()` (single Action, no slot).
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        _caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        vec![Box::new(ApplyCondition {
            actor_id: target_id,
            condition: Condition::Inspired,
            // RAW: 1 minute (we cap at 10 rounds — the holder consumes
            // the buff at their next attack / save anyway, so the timer
            // is mostly defensive).
            timer: ConditionTimer::Rounds(10),
        })]
    }
}

pub static GUIDANCE: LazyLock<Guidance> = LazyLock::new(|| Guidance {});

/// Dissonant Whispers — level-1 enchantment (bard). The caster whispers
/// a discordant melody at a single creature within 60 ft (24 tiles): the
/// target makes a WIS save vs the caster's CHA-based DC. Pass = half;
/// fail = full 3d6 psychic + the target uses its reaction (if available)
/// to flee away from the caster at full walking speed without provoking
/// opportunity attacks. We collapse the reaction-driven flee to the
/// shared `PushActor` helper — `speed / 5` tiles is the target's full
/// walking-speed budget, and the forced-move semantics skip opportunity
/// attacks per RAW.
///
/// Note: the spell's "uses its reaction" clause is approximated as
/// always-on; we don't track per-turn reaction availability for the
/// target side. The disadvantage on the save for deaf creatures is also
/// skipped (the engine doesn't gate spells on sound).
pub struct DissonantWhispers {}

impl Action for DissonantWhispers {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Enchantment)
    }
    fn name(&self) -> &str {
        "dissonant whispers"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["dw", "whispers"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60 ft = 24 tiles.
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Psychic]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(1)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::side_effects::PushActor;
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        // Bards anchor on CHA; sorcerers / wizards aren't on the spell
        // list per RAW, but we offer the INT / CHA pick for forward-
        // compatibility with future multiclass picks.
        let dc = caster.best_spell_save_dc([
            AbilityScoreType::Charisma,
            AbilityScoreType::Intelligence,
        ]);
        let caster_loc = caster.location();
        let (dmg, passed) = save_for_half_damage(
            encounter,
            caster_id,
            target_id,
            AbilityScoreType::Wisdom,
            dc,
            Dice::new(3, 6),
            DamageType::Psychic,
            "dissonant whispers",
        );
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        if dmg > 0 {
            effects.push(Box::new(DealDamage {
                actor_id: target_id,
                amount: dmg,
                damage_type: DamageType::Psychic,
            }));
        }
        // RAW: on a failed save, the target uses its reaction to flee
        // its full walking speed away from the caster, without provoking
        // opportunity attacks. We approximate "full walking speed" as
        // `speed_ft / 5` tiles (each tile is 5 ft on this grid).
        if !passed {
            let max_tiles =
                encounter
                    .actors
                    .get(&target_id)
                    .map(|a| (a.speed() / 5.0) as u32)
                    .unwrap_or(0);
            if max_tiles > 0 {
                effects.push(Box::new(PushActor {
                    actor_id: target_id,
                    from: caster_loc,
                    max_tiles,
                }));
            }
        }
        effects
    }
}

pub static DISSONANT_WHISPERS: LazyLock<DissonantWhispers> =
    LazyLock::new(|| DissonantWhispers {});

/// Ice Knife — level-1 conjuration (druid / sorcerer / wizard). The
/// caster hurls a shard of magical ice at a creature within 60 ft (24
/// tiles): a ranged spell attack lands 1d10 piercing on hit. Hit OR
/// miss, the shard shatters — every creature within 5 ft (1-tile
/// burst) of the target makes a DEX save vs the caster's spell DC for
/// 2d6 cold or half. The split — single-target attack roll plus an
/// always-applied burst at the target's point — is the spell's
/// signature: even a missed throw still threatens the impact area.
pub struct IceKnife {}

impl Action for IceKnife {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Conjuration)
    }
    fn name(&self) -> &str {
        "ice knife"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ik", "iceknife"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60 ft = 24 tiles.
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Piercing, DamageType::Cold]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(1)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        // Caster ability: best of INT / CHA / WIS so the spell works
        // for wizard (INT), sorcerer (CHA), and druid (WIS) without
        // each loadout having to special-case the modifier pick.
        let cast_ability = [
            AbilityScoreType::Intelligence,
            AbilityScoreType::Charisma,
            AbilityScoreType::Wisdom,
        ]
        .into_iter()
        .max_by_key(|a| caster.ability_score(*a))
        .unwrap_or(AbilityScoreType::Intelligence);
        let attack_bonus = caster.spell_attack_modifier(cast_ability);
        let dc = caster.spell_save_dc(cast_ability);
        // Pull the target's tile up front — the burst centers there
        // regardless of whether the attack roll hits.
        let target_loc = encounter
            .actors
            .get(&target_id)
            .map(|a| a.location())
            .unwrap_or(Coordinate::new(0, 0));
        // Part 1: ranged spell attack for 1d10 piercing.
        let mut effects = spell_attack(
            encounter,
            caster_id,
            target_id,
            "ice knife (shard)",
            attack_bonus,
            Dice::new(1, 10),
            DamageType::Piercing,
            false,
        );
        // Part 2: shatter burst at the target's tile — fires hit OR
        // miss per RAW. Friend-or-foe save-for-half (no enemy filter —
        // the shrapnel doesn't discriminate); shared roll across
        // affected actors.
        let (burst_effects, _saves) = neutral_burst_save_for_half(
            encounter,
            caster_id,
            target_loc,
            1,
            AbilityScoreType::Dexterity,
            dc,
            Dice::new(2, 6),
            DamageType::Cold,
            "ice knife (shatter)",
        );
        effects.extend(burst_effects);
        effects
    }
}

pub static ICE_KNIFE: LazyLock<IceKnife> = LazyLock::new(|| IceKnife {});

/// Enlarge / Reduce — level-2 transmutation, concentration (sorcerer /
/// wizard). One spell with two halves, and this struct is both of them:
/// the caster touches a creature and moves it one category along the size
/// ladder, up or down.
///
/// Enlarge is the buff: +1 size, +1d4 on every weapon hit, advantage on
/// STR saves, cast on a willing ally with no save. Reduce is its mirror:
/// -1 size, -1d4 on every weapon hit, disadvantage on STR saves, cast on
/// an enemy who gets a CON save to shrug it off.
///
/// The spell itself installs a condition and an anchor; everything the
/// condition *does* lives on shared tables — `RESIZING_CONDITIONS` for
/// the size move, `ON_HIT_RIDERS` and `WEAPON_DAMAGE_PENALTY_DICE` for
/// the damage, `STRENGTH_SAVE_MODE_CONDITIONS` for the save. That is why
/// two directions cost one struct: the halves differ in which condition
/// they name and whether a save can refuse it, and in nothing else.
pub struct SizeShiftSpell {
    /// Canonical name for the prompt parser and the action list.
    name: &'static str,
    /// Alias set for the prompt parser.
    aliases: &'static [&'static str],
    /// Which end of the ladder this half installs.
    condition: Condition,
    /// Concentration anchor label, shown when the caster's focus breaks.
    anchor_label: &'static str,
    /// The save that refuses the effect, or `None` for the willing-target
    /// half. RAW gives Reduce a CON save and Enlarge none, because you do
    /// not resist a spell you asked for.
    save_ability: Option<AbilityScoreType>,
}

impl SizeShiftSpell {
    /// True when this half is the debuff — derived from whether RAW lets
    /// the target refuse it, which is the same question.
    fn is_debuff(&self) -> bool {
        self.save_ability.is_some()
    }
}

impl Action for SizeShiftSpell {
    fn name(&self) -> &str {
        self.name
    }
    fn aliases(&self) -> Vec<&str> {
        self.aliases.to_vec()
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(crate::actions::action_template::MELEE_REACH)
    }
    fn is_harmful(&self) -> bool {
        self.is_debuff()
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Transmutation)
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(2)
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // Don't re-prime if the caster is already holding concentration
        // (the effect anchors on the caster's concentration slot — if
        // a higher-value spell already holds it, skip).
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return false;
        };
        if caster.is_concentrating() {
            return false;
        }
        let Some(target_id) = first_target_id(target_ids) else {
            return false;
        };
        // Sides must match the half being cast: the buff goes on an ally,
        // the debuff on an enemy. Without the gate the AI's support
        // pipeline would happily Reduce its own front line.
        if encounter.actors_allied(caster_id, target_id) == self.is_debuff() {
            return false;
        }
        // Don't burn a slot on a target already at this end of the
        // ladder.
        encounter
            .actors
            .get(&target_id)
            .is_some_and(|a| a.is_combat_active() && !a.has_condition(self.condition))
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        // The debuff half offers a save; a pass spends the slot and
        // nothing else, which is what RAW's "on a successful save, the
        // spell has no effect" costs.
        if let Some(ability) = self.save_ability {
            let Some(dc) = encounter
                .actors
                .get(&caster_id)
                .map(|a| a.spellcasting_save_dc())
            else {
                return Vec::new();
            };
            if encounter
                .roll_save_against_caster(target_id, ability, dc, caster_id)
                .passed()
            {
                return Vec::new();
            }
        }
        vec![
            Box::new(ApplyCondition {
                actor_id: target_id,
                condition: self.condition,
                // 10 rounds = 1 minute RAW (concentration cap).
                timer: ConditionTimer::Rounds(10),
            }),
            Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions(
                    self.anchor_label,
                    vec![(target_id, self.condition)],
                ),
            }),
        ]
    }
}

pub static ENLARGE_REDUCE: LazyLock<SizeShiftSpell> = LazyLock::new(|| SizeShiftSpell {
    name: "enlarge",
    aliases: &["enlarge-reduce", "er"],
    condition: Condition::Enlarged,
    anchor_label: "Enlarge",
    save_ability: None,
});

/// The Reduce half of Enlarge / Reduce, as its own entry in the action
/// list. RAW is one spell with a choice made at cast time; the engine's
/// prompt takes a name and a target and nothing else, so the choice has
/// to live in the name — the alternative would be an override flag the
/// player has no way to type.
pub static REDUCE: LazyLock<SizeShiftSpell> = LazyLock::new(|| SizeShiftSpell {
    name: "reduce",
    aliases: &["shrink"],
    condition: Condition::Reduced,
    anchor_label: "Reduce",
    save_ability: Some(AbilityScoreType::Constitution),
});

/// Destructive Wave — level-5 evocation (paladin). The caster slams the
/// ground; every enemy within 30 ft (6-tile burst centered on the
/// caster) makes a CON save vs the caster's CHA-based DC. Pass = half;
/// fail = full 5d6 thunder + 5d6 radiant + knocked Prone. The mixed
/// thunder + radiant damage slips past single-type resistance the same
/// way Flame Strike's fire + radiant does — and the prone-on-fail
/// crowd-control rider gives the paladin a true mass disable at the
/// lv5 slot tier. Self-centered burst, no concentration, allies spared
/// via `enemy_burst_targets`.
pub struct DestructiveWave {}

impl Action for DestructiveWave {
    fn name(&self) -> &str {
        "destructive wave"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["dwave", "dw5"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Thunder, DamageType::Radiant]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(5)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let center = caster.location();
        let dc = caster.spell_save_dc(AbilityScoreType::Charisma);
        // Two halves rolled separately so per-actor resistance lookups
        // apply independently (Flame Strike-style). The save vector is
        // produced by the thunder pass; the radiant pass shares the same
        // save outcome on each target (5e treats both halves as a single
        // save), so we re-walk the same target list against the second
        // damage roll without re-rolling saves.
        let (mut effects, saves) = enemy_burst_save_for_half(
            encounter,
            caster_id,
            center,
            6,
            AbilityScoreType::Constitution,
            dc,
            Dice::new(5, 6),
            DamageType::Thunder,
            "destructive wave (thunder)",
        );
        // Radiant half: re-walk the same save list rather than re-rolling.
        let rad_raw = encounter.roll_empowered_sum(caster_id, 5, 6);
        encounter.log(format!(
            "  destructive wave (radiant): 5d6({}) shared Radiant",
            rad_raw
        ));
        for &(tid, passed) in &saves {
            let dmg = if passed { rad_raw / 2 } else { rad_raw };
            if dmg > 0 {
                effects.push(Box::new(DealDamage {
                    actor_id: tid,
                    amount: dmg,
                    damage_type: DamageType::Radiant,
                }));
            }
        }
        // Prone-on-fail rider — fires only on a failed save. Allies
        // were already filtered out by the enemy_burst pass, so this
        // is enemy-only by construction.
        for &(tid, passed) in &saves {
            if !passed {
                effects.push(Box::new(ApplyCondition {
                    actor_id: tid,
                    condition: Condition::Prone,
                    timer: ConditionTimer::Permanent,
                }));
            }
        }
        effects
    }
}

pub static DESTRUCTIVE_WAVE: LazyLock<DestructiveWave> =
    LazyLock::new(|| DestructiveWave {});

/// Sword Burst — sorcerer / warlock / wizard cantrip (conjuration). The
/// caster brandishes a half-circle of spectral blades around themselves:
/// every creature within 5 ft (1-tile self-centered burst, caster
/// excluded) makes a DEX save vs the caster's spell DC. On fail: 1d6
/// force. On success: nothing (cantrip — no half-on-save).
///
/// Mechanically symmetric to Thunderclap, swapping CON → DEX and
/// thunder → force. Friend-or-foe burst (your duplicates and the bandit
/// next to you are both fair game), routed through the shared
/// `neutral_burst_save_only` helper.
pub struct SwordBurst {}

impl Action for SwordBurst {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Conjuration)
    }
    fn name(&self) -> &str {
        "sword burst"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["sword-burst", "sweep", "spirit-blades"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Force]
    }
    // Cantrip — uses the default `cost()` (single Action, no spell slot).
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let center = caster.location();
        // Pick the best of INT / CHA / WIS so wizard / sorcerer / warlock
        // / bard (any future pickup) all get their DC anchored on the
        // class's primary stat without per-loadout special-casing.
        let dc = caster.best_spell_save_dc([
            AbilityScoreType::Intelligence,
            AbilityScoreType::Charisma,
            AbilityScoreType::Wisdom,
        ]);
        let n = crate::engine::util::cantrip_dice_count(caster.level());
        let (effects, _saves) = neutral_burst_save_only(
            encounter,
            caster_id,
            center,
            1,
            AbilityScoreType::Dexterity,
            dc,
            Dice::new(n, 6),
            DamageType::Force,
            "sword burst",
        );
        effects
    }
}

pub static SWORD_BURST: LazyLock<SwordBurst> = LazyLock::new(|| SwordBurst {});

/// Blade Ward — bard / sorcerer / warlock / wizard cantrip (abjuration).
/// The caster traces a sigil of warding; until the start of their next
/// turn they have resistance to bludgeoning, piercing, and slashing
/// damage from weapon attacks. We model the BPS resistance via the
/// generic `DamageResistant` condition (halves all incoming typed
/// damage, not just BPS — close enough for the engine's resistance
/// granularity) with the `UntilStartOfNextTurn` timer that the engine
/// already ticks down at turn-start.
///
/// Self-only, no concentration, no slot. Strictly defensive — `is_harmful`
/// false keeps the AI's focus-fire pipeline from picking it as an
/// attack option.
pub struct BladeWard {}

impl Action for BladeWard {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Abjuration)
    }
    fn name(&self) -> &str {
        "blade ward"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ward", "bw"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // Don't burn the action re-warding an already-warded caster —
        // the buff doesn't stack and an unspent Action is more valuable
        // than refreshing the timer (which is already short).
        actor_lacks_condition(encounter, caster_id, Condition::DamageResistant)
    }
    // Cantrip — uses the default `cost()` (single Action, no spell slot).
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        vec![Box::new(ApplyCondition {
            actor_id: caster_id,
            condition: Condition::DamageResistant,
            timer: ConditionTimer::UntilStartOfNextTurn,
        })]
    }
}

pub static BLADE_WARD: LazyLock<BladeWard> = LazyLock::new(|| BladeWard {});

/// Catapult — level-1 transmutation (sorcerer / wizard). The caster
/// hurls an unattended object weighing 1-5 lb in a 90-ft line at a
/// single creature within range (we model as `SingleActor` at 36 tiles
/// reach). The target makes a DEX save vs the caster's spell DC: on
/// fail the object strikes for 3d8 bludgeoning, on save the object
/// misses (no damage). Distinct from cantrip save-or-nothing spells in
/// being a leveled slot — the higher dice (3d8) at lv1 makes it a
/// punchier alternative to Magic Missile when you need a single big
/// hit and don't want to roll attack-vs-AC.
pub struct Catapult {}

impl Action for Catapult {
    fn name(&self) -> &str {
        "catapult"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["cat", "hurl"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 90 ft RAW; we cap at the engine's 60-ft equivalent (24 tiles)
        // since the spell's targeting falls off practical ranges past
        // line-of-sight on the typical encounter map.
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Bludgeoning]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(1)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        // Pick best of INT / CHA so sorcerer (CHA) and wizard (INT)
        // both anchor on their primary spellcasting ability.
        let dc = caster.best_spell_save_dc([
            AbilityScoreType::Intelligence,
            AbilityScoreType::Charisma,
        ]);
        let raw = encounter.roll_empowered_sum(caster_id, 3, 8);
        let save = encounter.roll_save_against_caster(target_id, AbilityScoreType::Dexterity, dc, caster_id);
        encounter.log(format!(
            "  catapult: 3d8({}) bludgeoning ({})",
            raw,
            if save.passed() {
                "save (no damage)"
            } else {
                "fail (full)"
            }
        ));
        if save.passed() || raw == 0 {
            return Vec::new();
        }
        vec![Box::new(DealDamage {
            actor_id: target_id,
            amount: raw,
            damage_type: DamageType::Bludgeoning,
        })]
    }
}

pub static CATAPULT: LazyLock<Catapult> = LazyLock::new(|| Catapult {});

/// Earth Tremor — level-1 evocation (bard / druid / sorcerer / wizard).
/// The caster causes a tremor in the ground in a 10-ft radius around
/// themselves (2-tile self-centered burst). Every creature in the area
/// makes a DEX save vs the caster's spell DC: on fail they take 1d6
/// bludgeoning and are knocked Prone, on save nothing. RAW also turns
/// the ground into difficult terrain — we skip that since the engine
/// doesn't yet model per-tile terrain-modification spells.
///
/// Self-centered burst, no concentration. Friend-or-foe (allies inside
/// the area take the save and damage same as enemies). Routes through
/// the shared `neutral_burst_save_only` cantrip helper plus a
/// prone-on-fail rider; mirrors Tidal Wave's pattern at the lv1
/// slot tier.
pub struct EarthTremor {}

impl Action for EarthTremor {
    fn name(&self) -> &str {
        "earth tremor"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["tremor", "quake-1"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Bludgeoning]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(1)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let center = caster.location();
        let dc = caster.best_spell_save_dc([
            AbilityScoreType::Intelligence,
            AbilityScoreType::Charisma,
            AbilityScoreType::Wisdom,
        ]);
        let (mut effects, saves) = neutral_burst_save_only(
            encounter,
            caster_id,
            center,
            2,
            AbilityScoreType::Dexterity,
            dc,
            Dice::new(1, 6),
            DamageType::Bludgeoning,
            "earth tremor",
        );
        // Prone-on-fail rider — the earth-shake topples failed-save
        // targets regardless of damage type. Permanent prone (the target
        // pays a movement to stand up); mirrors Tidal Wave / Destructive
        // Wave's prone follow-up.
        push_condition_on_failed_save(
            &mut effects,
            &saves,
            Condition::Prone,
            ConditionTimer::Permanent,
        );
        effects
    }
}

pub static EARTH_TREMOR: LazyLock<EarthTremor> = LazyLock::new(|| EarthTremor {});

/// Fog Cloud — level-1 conjuration, concentration (druid / ranger /
/// sorcerer / wizard). The caster creates a 20-ft-radius sphere of fog
/// centered on a point within 120 ft. The area is heavily obscured —
/// per 5e RAW, every creature inside is effectively Blinded (auto-fail
/// vision checks, attacks against have advantage, attacks from have
/// disadvantage).
///
/// We model the heavy obscurement by installing the existing `Blinded`
/// condition on every combat-active actor inside the burst at cast
/// time. This is a friend-or-foe install: allies caught in the cloud
/// suffer the same penalty as enemies (matches RAW, and rewards
/// thoughtful AoE placement). Concentration-bound on the caster, so
/// dropping concentration (taking damage, casting another concentration
/// spell, the spell timer running out) clears the Blinded mark on every
/// affected actor automatically via the engine's concentration cleanup
/// pipeline.
///
/// Simplification vs RAW: we install at cast time only. A creature that
/// walks into the cloud later doesn't pick up the Blinded mark, and a
/// creature that leaves the cloud keeps it until concentration drops.
/// In practice this is close enough: the cloud's tactical value is the
/// burst install + sustained denial of the area, and the engine has no
/// "is this tile fog-covered" terrain layer to query for moves yet.
pub struct FogCloud {}

impl Action for FogCloud {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Conjuration)
    }
    fn name(&self) -> &str {
        "fog cloud"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["fog", "cloud-spell"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        // 20-ft sphere = 4-tile radius on the 2.5ft grid (8 tiles
        // diameter ≈ 20 ft).
        TargetingSchema::Burst { radius: 4 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 120 ft = 48 tiles.
        Some(48)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn is_harmful(&self) -> bool {
        // Friend-or-foe debuff — the AI's heal-target pipeline shouldn't
        // pick this as a support cast (`is_heal` is false too), but the
        // hostile gate matters more for whether the spell hits charm-
        // immune targets. Mark hostile since the AoE primarily disables
        // enemies relative to the caster's intent.
        true
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(1)
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // Don't replace our own concentration on a less-valuable spell;
        // the AI's concentration pipeline tests every concentration-
        // bound spell against `validate_input` first.
        encounter.caster_can_concentrate(caster_id)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        const RADIUS: isize = 4;
        encounter.log(format!("  fog cloud: heavy obscurement at {}", point));
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        let mut conditions: Vec<(usize, Condition)> = Vec::new();
        for tid in encounter.neutral_burst_targets(caster_id, point, RADIUS) {
            // Skip targets that are immune to Blinded (treat the
            // condition table as the source of truth for "can this
            // actor be obscured?"). Honors dynamic immunities too even
            // though no current source dynamically immunes Blinded —
            // future-proof against feature additions.
            let immune = encounter
                .actors
                .get(&tid)
                .is_some_and(|a| a.effectively_immune_to_condition(Condition::Blinded));
            if immune {
                continue;
            }
            effects.push(Box::new(ApplyCondition {
                actor_id: tid,
                condition: Condition::Blinded,
                // RAW: 1 hour. Concentration caps the practical duration
                // long before the round timer; 10 rounds keeps the timer
                // honest even if the caster dies and the cleanup hook
                // misses a target somehow.
                timer: ConditionTimer::Rounds(10),
            }));
            conditions.push((tid, Condition::Blinded));
        }
        // Always start concentration even if no targets caught the burst
        // — the slot is spent and the fog is on the map; a future patch
        // that walks creatures into the cloud should pick up the spell
        // via this concentration mark.
        effects.push(Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData::with_conditions("Fog Cloud", conditions),
        }));
        effects
    }
}

pub static FOG_CLOUD: LazyLock<FogCloud> = LazyLock::new(|| FogCloud {});

/// Gust of Wind — level-2 evocation (druid / sorcerer / wizard). The
/// caster summons a 60-ft-long, 10-ft-wide line of strong wind. Every
/// creature in the line makes a STR save vs the caster's spell DC: on
/// fail they are pushed 15 ft (6 tiles) away from the caster in the
/// wind's direction. Save success = no movement, no damage either way
/// (the spell is pure crowd control / repositioning).
///
/// We collapse the line targeting to a `SinglePoint` schema: the picker
/// picks a tile that defines the wind's direction. Every combat-active
/// actor whose footprint is within a 3-tile gap of the line between the
/// caster and the target point (approximate as a 3-tile-radius burst
/// centered on the *midpoint* between caster and target) makes the save.
/// The push anchor is the caster's location — failed saves are pushed
/// away from the caster along the line, which matches RAW intent (the
/// wind blows outward from the caster).
///
/// Concentration-bound on the caster (RAW: 1 minute, re-blown each turn
/// as a bonus action). We collapse the per-turn re-blow to the cast-time
/// install: the push fires once, and the concentration anchor holds the
/// slot until the caster drops it (matches our Sickening Radiance /
/// Dawn simplification).
pub struct GustOfWind {}

impl Action for GustOfWind {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Evocation)
    }
    fn name(&self) -> &str {
        "gust of wind"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["gust", "gow"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SinglePoint
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60-ft line = 24 tiles. The target tile defines the far end
        // of the wind path.
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(2)
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        encounter.caster_can_concentrate(caster_id)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::side_effects::PushActor;

        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.best_spell_save_dc([
            AbilityScoreType::Intelligence,
            AbilityScoreType::Charisma,
            AbilityScoreType::Wisdom,
        ]);
        let caster_loc = caster.location();
        // Approximate the 60-ft line as a 3-tile burst centered on the
        // midpoint between caster and target. Picks up everyone roughly
        // in the wind's path without needing a true line-targeting
        // schema. The push anchor stays at the caster so failed-save
        // targets are blown outward along the wind direction.
        let midpoint = Coordinate::new(
            (caster_loc.x + point.x) / 2,
            (caster_loc.y + point.y) / 2,
        );
        const RADIUS: isize = 3;
        // 15 ft = 6 tiles on the 2.5ft grid.
        const PUSH_TILES: u32 = 6;
        encounter.log(format!(
            "  gust of wind: line toward {} (DC {})",
            point, dc
        ));
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        // Gust of Wind installs no per-target conditions — the push is
        // the entire effect — so the concentration data's conditions
        // vec stays empty. We keep the explicit binding so the
        // StartConcentration call reads symmetric with Fog Cloud and
        // future single-cast concentration spells.
        let conditions: Vec<(usize, Condition)> = Vec::new();
        for tid in encounter.neutral_burst_targets(caster_id, midpoint, RADIUS) {
            let save = encounter.roll_save_against_caster(tid, AbilityScoreType::Strength, dc, caster_id);
            if save.passed() {
                continue;
            }
            effects.push(Box::new(PushActor {
                actor_id: tid,
                from: caster_loc,
                max_tiles: PUSH_TILES,
            }));
        }
        // Anchor the concentration even if no actor was caught — the
        // slot was spent and the wind keeps blowing. No per-target
        // condition install to clean up; the data carries an empty
        // condition vec.
        effects.push(Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData::with_conditions("Gust of Wind", conditions),
        }));
        effects
    }
}

pub static GUST_OF_WIND: LazyLock<GustOfWind> = LazyLock::new(|| GustOfWind {});

/// Chaos Bolt — level-1 evocation (sorcerer). Ranged spell attack vs a
/// single target within 120 ft (48 tiles) for 2d8 + 1d6 damage of a
/// random elemental type rolled per cast. The RAW "matching d8s chain
/// to a new target" clause is the spell's signature — we model it by
/// detecting a doubled d8 result and, on a chain, replaying the bolt
/// against the nearest other enemy within 30 ft (12 tiles) of the
/// primary target with the same damage roll. One chain max per cast
/// (we cap at one hop instead of the RAW "until you roll non-matching
/// d8s" recursion to keep the dispatch deterministic).
///
/// The eight damage types mirror RAW's table (Acid / Cold / Fire / Force
/// / Lightning / Poison / Psychic / Thunder). The type is picked by
/// rolling 1d8, with the index 1..=8 mapped to the entry in declared
/// order — same shape as the RAW chart.
pub struct ChaosBolt {}

impl Action for ChaosBolt {
    fn name(&self) -> &str {
        "chaos bolt"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["chaos", "cb-bolt"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(48)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![
            DamageType::Acid,
            DamageType::Cold,
            DamageType::Fire,
            DamageType::Force,
            DamageType::Lightning,
            DamageType::Poison,
            DamageType::Psychic,
            DamageType::Thunder,
        ]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(1)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::util::{footprint_chebyshev, get_tiles_from_size};

        let Some(primary_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let attack_bonus = caster.best_spell_attack_modifier([
            AbilityScoreType::Charisma,
            AbilityScoreType::Intelligence,
        ]);

        // Roll a single d8 to pick the damage type AND drive the chain
        // check (the second d8's value is irrelevant to type / chain
        // detection — we just need to know if it matches the first).
        // The actual damage dice (2d8 + 1d6) re-roll inside `spell_attack`
        // — a small RAW divergence (RAW: the type-picking d8s ARE the
        // damage dice) that keeps the on-hit rider stack (Hex,
        // Hunter's Mark, smites, reflects) firing through the standard
        // `spell_attack` pipeline. The chain check still gates on the
        // d8 match, so the spell's signature behavior (matching d8s →
        // bounce to nearest enemy) is preserved.
        let type_d8 = encounter.roll(&Dice::new(1, 8));
        let chain_d8 = encounter.roll(&Dice::new(1, 8));
        let chained = type_d8 == chain_d8;
        let dt = chaos_damage_type(type_d8 as u8);
        encounter.log(format!(
            "  chaos bolt: type d8({}) → {:?}{}",
            type_d8,
            dt,
            if chained { " (CHAIN!)" } else { "" }
        ));

        // Ranged spell attack vs the primary for 2d8 + 1d6 of the
        // picked type, routing through `spell_attack` so the full
        // engine rider stack (Hex, Hunter's Mark, smite primes, etc.)
        // applies normally.
        let mut effects = spell_attack(
            encounter,
            caster_id,
            primary_id,
            "chaos bolt",
            attack_bonus,
            Dice::new(2, 8),
            dt,
            false,
        );
        // Add the +1d6 chaos damage as a separate same-typed payload
        // (RAW lumps it into the same damage type — separate side-effect
        // lets the target's typed resistance / immunity / vulnerability
        // apply to the +d6 too).
        let d6 = encounter.roll(&Dice::new(1, 6));
        if d6 > 0 && !effects.is_empty() {
            effects.push(Box::new(DealDamage {
                actor_id: primary_id,
                amount: d6,
                damage_type: dt,
            }));
        }

        // Chain to the nearest other enemy within 30 ft (12 tiles) of
        // the primary on a doubled d8. RAW's recursive chain caps at
        // one extra hop in our model — keeps the dispatch deterministic
        // and avoids the engine pondering through a chain of self-
        // referential rolls.
        if chained
            && let Some(primary) = encounter.actors.get(&primary_id)
        {
            let primary_loc = primary.location();
            let primary_size = get_tiles_from_size(primary.size());
            let caster_team = encounter.actors.get(&caster_id).map(|a| a.team());
            let mut forks: Vec<(isize, usize)> = encounter
                .actors
                .iter()
                .filter_map(|(id, a)| {
                    if *id == caster_id || *id == primary_id || !a.is_combat_active() {
                        return None;
                    }
                    if caster_team.is_some_and(|t| t == a.team()) {
                        return None;
                    }
                    let dist = footprint_chebyshev(
                        a.location(),
                        get_tiles_from_size(a.size()),
                        primary_loc,
                        primary_size,
                    );
                    if dist > 12 { None } else { Some((dist, *id)) }
                })
                .collect();
            forks.sort_unstable();
            if let Some((_, fork_id)) = forks.first().copied() {
                let chain_effects = spell_attack(
                    encounter,
                    caster_id,
                    fork_id,
                    "chaos bolt (chain)",
                    attack_bonus,
                    Dice::new(2, 8),
                    dt,
                    false,
                );
                let chain_d6 = encounter.roll(&Dice::new(1, 6));
                let chain_hit = !chain_effects.is_empty();
                effects.extend(chain_effects);
                if chain_hit && chain_d6 > 0 {
                    effects.push(Box::new(DealDamage {
                        actor_id: fork_id,
                        amount: chain_d6,
                        damage_type: dt,
                    }));
                }
            }
        }
        effects
    }
}

/// Map the RAW Chaos Bolt damage-type d8 (1..=8) to a `DamageType`. Any
/// value outside the 1..=8 band falls back to Force (defensive default —
/// the helper is only called with d8 results so the fall-through is dead
/// code in practice).
fn chaos_damage_type(roll: u8) -> DamageType {
    match roll {
        1 => DamageType::Acid,
        2 => DamageType::Cold,
        3 => DamageType::Fire,
        4 => DamageType::Force,
        5 => DamageType::Lightning,
        6 => DamageType::Poison,
        7 => DamageType::Psychic,
        8 => DamageType::Thunder,
        _ => DamageType::Force,
    }
}

pub static CHAOS_BOLT: LazyLock<ChaosBolt> = LazyLock::new(|| ChaosBolt {});

/// Arms of Hadar — level-1 conjuration (warlock). The caster slaps the
/// ground; black tentacles erupt around them in a 10-foot radius (1-tile
/// gap on this 2.5ft grid). Every enemy in the burst makes a STR save vs
/// the caster's CHA-based spell DC: pass = half damage (2d6 → 1d6),
/// fail = full damage (2d6 necrotic) plus they cannot take reactions
/// until the start of their next turn (the existing `NoReaction`
/// condition with the `UntilStartOfNextTurn` timer).
///
/// Self-centered burst — the picker is `NoArgs` and the origin is the
/// caster's location. Distinct from Thunderclap (cantrip, smaller burst,
/// no reaction lockout) — Arms of Hadar's level-1 slot buys the bigger
/// dice and the no-reactions rider on fail.
pub struct ArmsOfHadar {}

impl Action for ArmsOfHadar {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Conjuration)
    }
    fn name(&self) -> &str {
        "arms of hadar"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["hadar", "aoh"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Necrotic]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(1)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let center = caster.location();
        let dc = caster.best_spell_save_dc([
            AbilityScoreType::Charisma,
            AbilityScoreType::Intelligence,
        ]);
        let (mut effects, saves) = enemy_burst_save_for_half(
            encounter,
            caster_id,
            center,
            1,
            AbilityScoreType::Strength,
            dc,
            Dice::new(2, 6),
            DamageType::Necrotic,
            "arms of hadar",
        );
        // RAW: failed-save targets also "can't take reactions until the
        // start of their next turn." We tack a NoReaction install on
        // every failed save here; passed saves take just the half damage
        // (queued by the helper).
        push_condition_on_failed_save(
            &mut effects,
            &saves,
            Condition::NoReaction,
            ConditionTimer::UntilStartOfNextTurn,
        );
        effects
    }
}

pub static ARMS_OF_HADAR: LazyLock<ArmsOfHadar> = LazyLock::new(|| ArmsOfHadar {});

/// Dragon's Breath — level-2 transmutation (sorcerer / wizard). The caster
/// imbues a willing creature with breath-weapon magic; on cast the target
/// can immediately exhale a 15-ft cone of acid / cold / fire / lightning /
/// poison damage. We collapse the "buff an ally to breathe later" RAW into
/// the immediate self-burst at cast time — the caster exhales directly,
/// since the spell's load-bearing portion is the burst itself.
///
/// Self-centered 15-ft cone modeled as a 2-tile burst from the caster.
/// Every enemy in the area makes a DEX save vs the caster's INT/CHA-based
/// spell DC: pass = half, fail = full. Damage type is picked by the
/// caster's best-vs-target pick (acid by default if the target table is
/// empty); we route through the existing `pick_damage_type_against_target`
/// helper using the closest enemy as the reference target.
pub struct DragonsBreath {}

impl Action for DragonsBreath {
    fn name(&self) -> &str {
        "dragon's breath"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["dragon-breath", "db", "breath"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![
            DamageType::Acid,
            DamageType::Cold,
            DamageType::Fire,
            DamageType::Lightning,
            DamageType::Poison,
        ]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(2)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let center = caster.location();
        let dc = caster.best_spell_save_dc([
            AbilityScoreType::Intelligence,
            AbilityScoreType::Charisma,
        ]);
        // Pick the damage type that maximizes effective damage on the
        // nearest enemy in the burst — falls back to Fire when the
        // burst is empty (no reference target to optimize against, so
        // a deterministic default keeps the log line stable).
        let menu = [
            DamageType::Acid,
            DamageType::Cold,
            DamageType::Fire,
            DamageType::Lightning,
            DamageType::Poison,
        ];
        let dt = encounter
            .enemy_burst_targets(caster_id, center, 2)
            .first()
            .copied()
            .map(|ref_tid| pick_damage_type_against_target(encounter, ref_tid, &menu))
            .unwrap_or(DamageType::Fire);
        let label = format!("dragon's breath ({:?})", dt);
        let (effects, _saves) = enemy_burst_save_for_half(
            encounter,
            caster_id,
            center,
            2,
            AbilityScoreType::Dexterity,
            dc,
            Dice::new(3, 6),
            dt,
            &label,
        );
        effects
    }
}

pub static DRAGONS_BREATH: LazyLock<DragonsBreath> = LazyLock::new(|| DragonsBreath {});

/// Conjure Barrage — level-3 conjuration (ranger). The caster hurls a
/// barrage of nonmagical ammunition or thrown weapons in a 60-ft cone.
/// We model the cone as a 2-tile burst centered at a target tile within
/// 12 tiles (line-of-sight required); every enemy in the burst makes a
/// DEX save vs the caster's WIS-based spell DC: pass = half, fail = full
/// damage. 3d8 damage; the type matches the ammunition swung — we pick
/// Piercing as the RAW default (the spell's flavor leans toward arrows).
pub struct ConjureBarrage {}

impl Action for ConjureBarrage {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Conjuration)
    }
    fn name(&self) -> &str {
        "conjure barrage"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["barrage", "cb-vol"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 2 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60ft cone — we collapse to a 2-tile burst placed up to 12 tiles
        // (30ft) out; the picker can still place the burst at the cone's
        // far edge.
        Some(12)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Piercing]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(3)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spell_save_dc(AbilityScoreType::Wisdom);
        let (effects, _saves) = enemy_burst_save_for_half(
            encounter,
            caster_id,
            point,
            2,
            AbilityScoreType::Dexterity,
            dc,
            Dice::new(3, 8),
            DamageType::Piercing,
            "conjure barrage",
        );
        effects
    }
}

pub static CONJURE_BARRAGE: LazyLock<ConjureBarrage> = LazyLock::new(|| ConjureBarrage {});

/// Steel Wind Strike — level-5 conjuration (ranger / wizard, XGE). The
/// caster brandishes a melee weapon and teleports into a flurry of strikes
/// against up to five visible creatures within 30 ft. Each picks a target
/// from the spell's range; we model the spell as a `SingleActor` primary
/// target plus up to four auto-picked extras (the nearest other enemies
/// within 30 ft of the caster's post-teleport landing). Each victim takes
/// 6d10 force damage — RAW: no attack roll, no save (the strikes auto-
/// connect on the cast). After resolving the strikes, the caster teleports
/// to a tile adjacent to one of the struck creatures (we pick the primary
/// target's adjacent tile if walkable, falling back to the caster's
/// original position).
pub struct SteelWindStrike {}

impl Action for SteelWindStrike {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Conjuration)
    }
    fn name(&self) -> &str {
        "steel wind strike"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["sws", "steel-wind"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 30ft = 12 tiles. Targeting picker lets the caster reach any
        // visible enemy within that range.
        Some(12)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Force]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(5)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::side_effects::TeleportActor;
        use crate::engine::util::{footprint_chebyshev, get_tiles_from_size};

        let Some(primary_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let caster_team = caster.team();
        let caster_loc = caster.location();
        let caster_size = get_tiles_from_size(caster.size());

        // Auto-pick up to four additional nearest enemies within 12 tiles
        // of the caster (excluding the primary). Sorted by (distance, id)
        // for deterministic test seeds.
        let mut extras: Vec<(isize, usize)> = encounter
            .actors
            .iter()
            .filter_map(|(id, a)| {
                if *id == caster_id || *id == primary_id || !a.is_combat_active() {
                    return None;
                }
                if a.team() == caster_team {
                    return None;
                }
                let dist = footprint_chebyshev(
                    a.location(),
                    get_tiles_from_size(a.size()),
                    caster_loc,
                    caster_size,
                );
                if dist > 12 { None } else { Some((dist, *id)) }
            })
            .collect();
        extras.sort_unstable();
        let extras_ids: Vec<usize> = extras.into_iter().take(4).map(|(_, id)| id).collect();

        // Shared 6d10 force damage roll — no attack roll, no save. RAW
        // is per-target rolls, but a single shared roll keeps the log
        // line concise and the dice pool predictable (matches the AoE
        // shared-roll semantics used by Fireball / Cone of Cold).
        let raw = encounter.roll_empowered_sum(caster_id, 6, 10);
        encounter.log(format!(
            "  steel wind strike: 6d10({}) force (auto-hit, up to 5 targets)",
            raw
        ));

        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        let mut all_targets = vec![primary_id];
        all_targets.extend(extras_ids);
        for tid in &all_targets {
            effects.push(Box::new(DealDamage {
                actor_id: *tid,
                amount: raw,
                damage_type: DamageType::Force,
            }));
        }

        // Teleport the caster to a tile adjacent to the primary's
        // footprint (footprint-chebyshev gap ≤ 1) where the caster's own
        // footprint fits cleanly. We sweep candidate anchor offsets out
        // to twice the caster's footprint width so Medium/Large/Huge
        // casters all find a non-overlapping landing slot when one
        // exists. Deterministic offset order (row-major, top-left
        // first) keeps the landing pick stable across seeded tests.
        if let Some(primary) = encounter.actors.get(&primary_id) {
            let primary_loc = primary.location();
            let primary_size = get_tiles_from_size(primary.size()) as isize;
            let landing = adjacent_landing_for(
                encounter,
                caster_id,
                primary_loc,
                primary_size,
            );
            if let Some(loc) = landing
                && loc != caster_loc
            {
                effects.push(Box::new(TeleportActor {
                    actor_id: caster_id,
                    dest: loc,
                }));
            }
        }
        effects
    }
}

/// Find an anchor tile for `mover_id`'s footprint that sits adjacent to
/// the `target_loc` / `target_size` footprint (footprint-chebyshev gap
/// ≤ 1). Returns `None` if no walkable, non-overlapping landing exists
/// within a 2× footprint-width sweep. Used by Steel Wind Strike to
/// teleport the caster next to a struck target; shaped generically so
/// future "land next to enemy" effects (Misty Step retargeting,
/// teleport-strike riders) can reuse it.
fn adjacent_landing_for(
    encounter: &EncounterInstance,
    mover_id: usize,
    target_loc: Coordinate,
    target_size: isize,
) -> Option<Coordinate> {
    use crate::engine::util::{footprint_chebyshev, get_tiles_from_size};

    let mover = encounter.actors.get(&mover_id)?;
    let mover_size = get_tiles_from_size(mover.size()) as isize;
    // Sweep range: enough to step around the target's full footprint
    // for the mover's own footprint (mover_size + target_size). Capped
    // at a small constant so the search stays O(1) per cast.
    let sweep = (mover_size + target_size).max(2);
    for dy in -sweep..=sweep {
        for dx in -sweep..=sweep {
            let cand = Coordinate::new(target_loc.x + dx, target_loc.y + dy);
            if !encounter.can_move_to(mover_id, cand) {
                continue;
            }
            let gap = footprint_chebyshev(cand, mover_size as usize, target_loc, target_size as usize);
            if gap <= 1 {
                return Some(cand);
            }
        }
    }
    None
}

pub static STEEL_WIND_STRIKE: LazyLock<SteelWindStrike> = LazyLock::new(|| SteelWindStrike {});

/// Wall of Ice — level-6 evocation (wizard), concentration. The caster
/// summons a wall of ice up to 60 ft long, 10 ft high, and 1 ft thick.
/// Mechanically we collapse the panel to the load-bearing combat hook:
/// every enemy whose footprint touches the burst at cast time takes
/// 10d6 cold damage on a failed DEX save (half on success) and is
/// knocked Prone (the wall fractures around them). Concentration anchors
/// on the caster — dropping concentration ends the wall. Distinct from
/// Wall of Force (no damage, just blocking) and Sleet Storm (no damage,
/// movement debuff). Wall of Ice's defining feature is the damage burst
/// at install plus the prone slip.
pub struct WallOfIce {}

impl Action for WallOfIce {
    fn name(&self) -> &str {
        "wall of ice"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["woice", "ice-wall"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 1 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 120 ft RAW = 48 tiles.
        Some(48)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Cold]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(6)
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        encounter.caster_can_concentrate(caster_id)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.best_spell_save_dc([
            AbilityScoreType::Intelligence,
            AbilityScoreType::Charisma,
        ]);
        let (mut effects, saves) = enemy_burst_save_for_half(
            encounter,
            caster_id,
            point,
            1,
            AbilityScoreType::Dexterity,
            dc,
            Dice::new(10, 6),
            DamageType::Cold,
            "wall of ice",
        );
        // RAW: failed-save targets are forced out of the wall's space —
        // the engine has no "shove out" primitive, so we approximate
        // by knocking them Prone (the wall fractures around them and
        // they slip on the ice). Passed-save targets dodge clear.
        push_condition_on_failed_save(
            &mut effects,
            &saves,
            Condition::Prone,
            ConditionTimer::Permanent,
        );
        effects.push(Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData::new("Wall of Ice"),
        }));
        effects
    }
}

pub static WALL_OF_ICE: LazyLock<WallOfIce> = LazyLock::new(|| WallOfIce {});

/// Warding Bond — level-2 abjuration (cleric / paladin). The caster
/// touches a willing ally and links the two for the spell's duration.
/// The bonded ally:
/// - Gains +1 AC and +1 saving throws (via `condition_ac_bonus` /
///   `condition_save_bonus`).
/// - Has resistance to all damage (via `effective_damage`'s WardingBonded
///   clause).
/// - Every point of damage they take is mirrored onto the caster.
///
/// RAW: 1-hour duration, no concentration. We install with a Rounds(60)
/// timer (~10 minutes of combat) — long enough to outlast any encounter
/// but short enough not to bleed across long rests. The bond breaks
/// when the timer expires or Dispel Magic strips the WardingBonded
/// condition; the `WardingBonded` back-link on the bonded actor is cleared
/// automatically alongside the condition via `remove_condition`.
///
/// Targeting is touch-range (1 tile) ally-only — `is_harmful = false`
/// keeps the AI's offensive pipeline from picking it. The caller picks
/// a `SingleActor` target; `custom_validate_input` gates against
/// re-binding an already-bonded ally and against self-targeting (the
/// caster can't share damage with themselves — the partner pointer
/// requires a distinct actor).
pub struct WardingBond {}

impl Action for WardingBond {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Abjuration)
    }
    fn name(&self) -> &str {
        "warding bond"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["wb", "ward"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // Touch range — must be footprint-adjacent.
        Some(1)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn is_heal(&self) -> bool {
        // The bond grants resistance + AC + save bonuses — slot it
        // alongside the AI's support pipeline so a wounded ally is
        // a valid pick for the bond just like for a heal target.
        true
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(2)
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        let Some(target_id) = first_target_id(target_ids) else {
            return false;
        };
        // Self-bond is a no-op (caster mirroring damage to themselves)
        // — gate it out so the AI / picker doesn't burn the slot.
        if target_id == caster_id {
            return false;
        }
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return false;
        };
        let Some(target) = encounter.actors.get(&target_id) else {
            return false;
        };
        // Ally-only.
        if target.team() != caster.team() {
            return false;
        }
        if !target.is_combat_active() {
            return false;
        }
        // Don't re-bond an already-bonded ally — the second cast would
        // overwrite the partner link, leaving the first caster dangling.
        if target.has_condition(Condition::WardingBonded) {
            return false;
        }
        true
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        encounter.log(
            "  warding bond: caster and ally are linked — damage will be shared.".to_string(),
        );
        // The bond is a flag-plus-link install like every charm and
        // mark: `WardingBonded` says the ally is bonded, the link says
        // to whom. Routed through the shared helper so the pair travels
        // together — the damage-mirror site reads them back as a pair
        // too, via `linked_by(WardingBonded)`.
        crate::engine::side_effects::install_condition_with_link(
            Condition::WardingBonded,
            target_id,
            caster_id,
            ConditionTimer::Rounds(60),
        )
    }
}

pub static WARDING_BOND: LazyLock<WardingBond> = LazyLock::new(|| WardingBond {});

/// Telekinetic — cantrip (transmutation; sorcerer / warlock / wizard).
/// Bonus action; pick a creature within 60ft and shove it 5 ft toward
/// the caster on a failed STR save. The 5e cantrip lets the caster pick
/// push or pull each cast — we collapse to "pull" since the AI's
/// repositioning lane already has push tools (Thunderwave) and Lightning
/// Lure's pull-then-zap pattern proves the framework. No damage; no slot;
/// no concentration.
///
/// Save DC = caster's spell save DC against the highest of their INT /
/// CHA / WIS — so wizard / sorcerer / warlock all get clean scaling
/// without per-loadout special-casing.
pub struct Telekinetic {}

impl Action for Telekinetic {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Transmutation)
    }
    fn name(&self) -> &str {
        "telekinetic"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["tk", "shove"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60 ft = 24 tiles.
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        // Bonus action cantrip — no spell slot.
        bonus_action_only()
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::side_effects::PullActor;

        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let caster_loc = caster.location();
        let dc = caster.best_spell_save_dc([
            AbilityScoreType::Intelligence,
            AbilityScoreType::Charisma,
            AbilityScoreType::Wisdom,
        ]);
        let save = encounter.roll_save_against_caster(target_id, AbilityScoreType::Strength, dc, caster_id);
        if save.passed() {
            encounter.log("  telekinetic: target resists the pull".to_string());
            return Vec::new();
        }
        // Pull 1 tile (5 ft) toward the caster. The PullActor helper
        // honors wall / occupancy blocking — a pinned target just
        // doesn't move and we log the no-op via the standard forced-move
        // log line (or absence thereof).
        vec![Box::new(PullActor {
            actor_id: target_id,
            toward: caster_loc,
            max_tiles: 1,
        })]
    }
}

pub static TELEKINETIC: LazyLock<Telekinetic> = LazyLock::new(|| Telekinetic {});

/// Green-Flame Blade — cantrip (evocation; sorcerer / warlock / wizard).
/// Companion to Booming Blade: make a melee spell attack against a target
/// within 5 ft; on hit deal 1d8 fire to the primary target AND a leap of
/// green flame burns one creature within 5 ft of the primary target for
/// `INT/CHA-mod` fire damage (caster's spellcasting modifier, minimum 0).
/// We pick the "leap" target as the lowest-HP enemy adjacent to the
/// primary — matches the AI's focus-fire heuristic so the cantrip isn't
/// wasted poking a fresh tank when there's a wounded neighbor.
///
/// Misses do nothing (no leap, no primary damage). The leap requires LOS
/// from the caster to the secondary per RAW; we approximate by simply
/// requiring footprint-adjacency to the primary, which is the only
/// geometric prerequisite the spell actually depends on.
pub struct GreenFlameBlade {}

impl Action for GreenFlameBlade {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Evocation)
    }
    fn name(&self) -> &str {
        "green-flame blade"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["gfb", "greenflame"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(crate::actions::action_template::MELEE_REACH)
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Fire]
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        // Spell list spans INT (wizard) / CHA (sorcerer, warlock) — pick
        // the best so any of the three classes scales cleanly without
        // per-loadout special-casing.
        let ability = caster.best_spellcasting_ability([
            AbilityScoreType::Intelligence,
            AbilityScoreType::Charisma,
        ]);
        let attack_bonus = caster.spell_attack_modifier(ability);
        let leap_bonus = caster.ability_modifier(ability).max(0);
        // 1d8 fire on the touch itself — cantrip "weapon" damage at base
        // scaling, matching Booming Blade's headline die.
        let primary_loc = encounter.actors.get(&target_id).map(|a| a.location());
        let mut effects = spell_attack(
            encounter,
            caster_id,
            target_id,
            "green-flame blade",
            attack_bonus,
            Dice::new(1, 8),
            DamageType::Fire,
            true,
        );
        // Leap only on a hit (empty effect list = miss).
        if effects.is_empty() || leap_bonus == 0 {
            return effects;
        }
        // Find a secondary target: enemy of the caster, adjacent to the
        // primary's footprint, not the primary itself, not the caster.
        // Picks the lowest-HP candidate so the leap finishes wounded
        // foes rather than poking healthy ones.
        let Some(primary_loc) = primary_loc else {
            return effects;
        };
        // 1-tile leap radius = the 8 tiles around the primary; we route
        // through the team-filtered burst helper so caster-exclusion and
        // friend-or-foe filtering are shared with the rest of the engine
        // (no per-spell team-id lookup). We still drop the primary by id
        // since it sits inside the burst footprint.
        let mut best: Option<(u32, usize)> = None;
        for tid in encounter.enemy_burst_targets(caster_id, primary_loc, 1) {
            if tid == target_id {
                continue;
            }
            let Some(a) = encounter.actors.get(&tid) else {
                continue;
            };
            let hp = a.hitpoints();
            if best.map(|(bhp, _)| hp < bhp).unwrap_or(true) {
                best = Some((hp, tid));
            }
        }
        let Some((_, leap_id)) = best else {
            return effects;
        };
        encounter.log(format!(
            "  green-flame blade: flame leaps for {} Fire",
            leap_bonus
        ));
        effects.push(Box::new(DealDamage {
            actor_id: leap_id,
            amount: leap_bonus as u32,
            damage_type: DamageType::Fire,
        }));
        effects
    }
}

pub static GREEN_FLAME_BLADE: LazyLock<GreenFlameBlade> = LazyLock::new(|| GreenFlameBlade {});

/// Primal Savagery — cantrip (transmutation; druid). Melee spell attack
/// for 1d10 acid damage. The druid bites/claws the target with a brief
/// surge of bestial form. Uses WIS for the attack modifier — the druid
/// is the only class with this on its list per RAW. No save, no slot,
/// no concentration. Crit-doubles the dice via the shared spell-attack
/// pipeline.
pub struct PrimalSavagery {}

impl Action for PrimalSavagery {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Transmutation)
    }
    fn name(&self) -> &str {
        "primal savagery"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ps", "savagery"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(crate::actions::action_template::MELEE_REACH)
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Acid]
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let attack_bonus = caster.spell_attack_modifier(AbilityScoreType::Wisdom);
        let n = crate::engine::util::cantrip_dice_count(caster.level());
        spell_attack(
            encounter,
            caster_id,
            target_id,
            "primal savagery",
            attack_bonus,
            Dice::new(n, 10),
            DamageType::Acid,
            true,
        )
    }
}

pub static PRIMAL_SAVAGERY: LazyLock<PrimalSavagery> = LazyLock::new(|| PrimalSavagery {});

/// Sapping Sting — cantrip (necromancy; sorcerer / wizard, TCoE).
/// Range 30 ft. Target makes a CON save vs the caster's INT/CHA-based
/// spell save DC: on fail, 1d4 necrotic AND knocked Prone. On success,
/// nothing (cantrips don't half-on-save).
///
/// Slots between Toll the Dead (necrotic save-or-suck) and Word of
/// Radiance (radiant cleric AoE) as a low-cost CC cantrip: the prone
/// rider gives front-line allies advantage on their next melee swing
/// at the target — a chunkier payoff than the headline 1d4 die.
pub struct SappingSting {}

impl Action for SappingSting {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Necromancy)
    }
    fn name(&self) -> &str {
        "sapping sting"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ss", "sap"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 30 ft = 12 tiles.
        Some(12)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Necrotic]
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.best_spell_save_dc([
            AbilityScoreType::Intelligence,
            AbilityScoreType::Charisma,
        ]);
        let n = crate::engine::util::cantrip_dice_count(caster.level());
        let save = encounter.roll_save_against_caster(target_id, AbilityScoreType::Constitution, dc, caster_id);
        if save.passed() {
            return Vec::new();
        }
        let die = Dice::new(n, 4);
        // Route the roll through the shared caster-aware chokepoint
        // rather than `roll` directly: that is where the Sorcerer's
        // Empowered Spell reroll, the Evocation Wizard's Empowered
        // Evocation bonus and the cleric's Potent Spellcasting bonus
        // all live. A bare `roll` here meant every one of them
        // silently skipped this cantrip.
        let raw = encounter.roll_empowered_sum(caster_id, die.count, die.faces);
        encounter.log(format!(
            "  sapping sting: {}({}) = {} necrotic + prone",
            die, raw, raw
        ));
        vec![
            Box::new(DealDamage {
                actor_id: target_id,
                amount: raw,
                damage_type: DamageType::Necrotic,
            }),
            Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Prone,
                timer: ConditionTimer::Permanent,
            }),
        ]
    }
}

pub static SAPPING_STING: LazyLock<SappingSting> = LazyLock::new(|| SappingSting {});

/// Erupting Earth — level-3 transmutation (sorcerer / wizard / druid). A
/// 20-ft (4-tile) cube of earth erupts at a point within 120 ft (48 tiles).
/// Every creature in the area makes a DEX save vs the caster's spell save
/// DC: fail = 3d12 bludgeoning, success = half. Slots cleanly between
/// Fireball (lv3 AoE evocation) and Lightning Bolt (lv3 line) as a 3rd-
/// level AoE that bypasses fire resistance via the bludgeoning typing —
/// good against fire-immune fiends and dragons.
pub struct EruptingEarth {}

impl Action for EruptingEarth {
    fn name(&self) -> &str {
        "erupting earth"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["erupt", "ee"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 2 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 120 ft RAW = 48 tiles.
        Some(48)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Bludgeoning]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(3)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.best_spell_save_dc([
            AbilityScoreType::Intelligence,
            AbilityScoreType::Charisma,
            AbilityScoreType::Wisdom,
        ]);
        let (effects, _) = enemy_burst_save_for_half(
            encounter,
            caster_id,
            point,
            2,
            AbilityScoreType::Dexterity,
            dc,
            Dice::new(3, 12),
            DamageType::Bludgeoning,
            "erupting earth",
        );
        effects
    }
}

pub static ERUPTING_EARTH: LazyLock<EruptingEarth> = LazyLock::new(|| EruptingEarth {});

/// Blight — level-4 necromancy (warlock / sorcerer / wizard). A single
/// target within 30 ft (12 tiles) makes a CON save vs the caster's spell
/// save DC: fail = 8d8 necrotic, success = half. Undead and constructs
/// shrug it off via the engine's necrotic immunity / resistance tables.
/// Plants take maximum damage by RAW (we don't model creature type at
/// that granularity — the standard damage roll applies). Slots between
/// Vitriolic Sphere (lv4 acid AoE) and Sickening Radiance (lv4 burst) as
/// the wizard's single-target lv4 necromancy nuke.
pub struct Blight {}

impl Action for Blight {
    fn name(&self) -> &str {
        "blight"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["bl", "wither"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 30 ft RAW = 12 tiles.
        Some(12)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Necrotic]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(4)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.best_spell_save_dc([
            AbilityScoreType::Intelligence,
            AbilityScoreType::Charisma,
            AbilityScoreType::Wisdom,
        ]);
        let (dmg, _) = save_for_half_damage(
            encounter,
            caster_id,
            target_id,
            AbilityScoreType::Constitution,
            dc,
            Dice::new(8, 8),
            DamageType::Necrotic,
            "blight",
        );
        if dmg == 0 {
            return Vec::new();
        }
        vec![Box::new(DealDamage {
            actor_id: target_id,
            amount: dmg,
            damage_type: DamageType::Necrotic,
        })]
    }
}

pub static BLIGHT: LazyLock<Blight> = LazyLock::new(|| Blight {});

/// Circle of Death — level-6 necromancy. Wave of negative energy bursts
/// from a point within 150 ft (60 tiles). Every creature in the 30-ft
/// (6-tile) radius makes a CON save vs the caster's spell save DC: fail
/// = 8d6 necrotic, success = half. Friend-or-foe agnostic by RAW; we
/// route through the neutral-burst helper so allies caught in the wave
/// take the hit too — encourages careful placement. The signature 6th-
/// level necromancy AoE; slots between Chain Lightning (lv6 force) and
/// Sunbeam (lv6 radiant).
pub struct CircleOfDeath {}

impl Action for CircleOfDeath {
    fn name(&self) -> &str {
        "circle of death"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["cod", "circle"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 6 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 150 ft RAW = 60 tiles.
        Some(60)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Necrotic]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(6)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.best_spell_save_dc([
            AbilityScoreType::Intelligence,
            AbilityScoreType::Charisma,
            AbilityScoreType::Wisdom,
        ]);
        let (effects, _) = neutral_burst_save_for_half(
            encounter,
            caster_id,
            point,
            6,
            AbilityScoreType::Constitution,
            dc,
            Dice::new(8, 6),
            DamageType::Necrotic,
            "circle of death",
        );
        effects
    }
}

pub static CIRCLE_OF_DEATH: LazyLock<CircleOfDeath> = LazyLock::new(|| CircleOfDeath {});

/// Harm — level-6 necromancy (cleric). Single-target CON save vs the
/// caster's spell save DC. On fail: 14d6 necrotic AND the target's max
/// HP drops by the damage dealt for the next 10 rounds (1 hour RAW — we
/// don't tick max-HP back up, so the drop is effectively permanent in
/// combat). On success: half damage and no max-HP drain. The cleric's
/// signature single-target nuke, mirroring Heal but on the offensive
/// side. Mechanically pairs the load-bearing necrotic burst with a
/// `AdjustMaxHp` rider so a survivor still feels the hit on their HP
/// ceiling.
pub struct Harm {}

impl Action for Harm {
    fn name(&self) -> &str {
        "harm"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["hrm"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60 ft RAW = 24 tiles.
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Necrotic]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(6)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::side_effects::AdjustMaxHp;
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spell_save_dc(AbilityScoreType::Wisdom);
        let (dmg, passed) = save_for_half_damage(
            encounter,
            caster_id,
            target_id,
            AbilityScoreType::Constitution,
            dc,
            Dice::new(14, 6),
            DamageType::Necrotic,
            "harm",
        );
        if dmg == 0 {
            return Vec::new();
        }
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = vec![Box::new(DealDamage {
            actor_id: target_id,
            amount: dmg,
            damage_type: DamageType::Necrotic,
        })];
        // Max-HP drain only fires on a failed save (RAW: "If the target
        // fails the saving throw, its hit point maximum is reduced...").
        // Drain equals the damage dealt — we use the post-roll value so
        // the cap drops by the same number the target actually took.
        if !passed {
            effects.push(Box::new(AdjustMaxHp {
                actor_id: target_id,
                delta: -(dmg as i32),
            }));
        }
        effects
    }
}

pub static HARM: LazyLock<Harm> = LazyLock::new(|| Harm {});

/// Delayed Blast Fireball — level-7 evocation. A bead of fire is hurled
/// to a tile within 150 ft (60 tiles), where it detonates immediately —
/// our engine doesn't model the multi-round "delay" RAW, so we collapse
/// the delay window to a single-action burst at full base damage. Every
/// creature in the 20-ft (4-tile) radius makes a DEX save vs the
/// caster's spell save DC: fail = 12d6 fire, success = half. Friend-or-
/// foe agnostic — neutral_burst routes catch caster's allies too. Sits
/// between Fire Storm (lv7 enemy-only fire) and Meteor Swarm (lv9
/// multi-burst) as the wizard's high-tier fire AoE.
pub struct DelayedBlastFireball {}

impl Action for DelayedBlastFireball {
    fn name(&self) -> &str {
        "delayed blast fireball"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["dbf", "delayed", "dblast"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 4 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 150 ft RAW = 60 tiles.
        Some(60)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Fire]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(7)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.best_spell_save_dc([
            AbilityScoreType::Intelligence,
            AbilityScoreType::Charisma,
            AbilityScoreType::Wisdom,
        ]);
        let (effects, _) = neutral_burst_save_for_half(
            encounter,
            caster_id,
            point,
            4,
            AbilityScoreType::Dexterity,
            dc,
            Dice::new(12, 6),
            DamageType::Fire,
            "delayed blast fireball",
        );
        effects
    }
}

pub static DELAYED_BLAST_FIREBALL: LazyLock<DelayedBlastFireball> =
    LazyLock::new(|| DelayedBlastFireball {});

/// Incendiary Cloud — level-8 conjuration. A churning cloud of smoke
/// and embers fills a 20-ft (4-tile) radius sphere at a point within
/// 150 ft (60 tiles). Every creature in the area makes a DEX save vs
/// the caster's spell save DC: fail = 10d8 fire, success = half. RAW
/// lasts 1 minute with the cloud drifting 10 ft per round; we collapse
/// to a one-shot burst on cast (consistent with Fire Storm / DBF) since
/// the engine doesn't model moving cloud zones. Friend-or-foe agnostic
/// via the neutral-burst route — the cloud doesn't discriminate.
pub struct IncendiaryCloud {}

impl Action for IncendiaryCloud {
    fn name(&self) -> &str {
        "incendiary cloud"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ic", "icloud", "embers"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 4 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 150 ft RAW = 60 tiles.
        Some(60)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Fire]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(8)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.best_spell_save_dc([
            AbilityScoreType::Intelligence,
            AbilityScoreType::Charisma,
            AbilityScoreType::Wisdom,
        ]);
        let (effects, _) = neutral_burst_save_for_half(
            encounter,
            caster_id,
            point,
            4,
            AbilityScoreType::Dexterity,
            dc,
            Dice::new(10, 8),
            DamageType::Fire,
            "incendiary cloud",
        );
        effects
    }
}

pub static INCENDIARY_CLOUD: LazyLock<IncendiaryCloud> = LazyLock::new(|| IncendiaryCloud {});

/// Weird — level-9 illusion (wizard). Each enemy in a 30-ft (6-tile)
/// burst centered on a point within 120 ft (48 tiles) sees their worst
/// fear and makes a WIS save vs the caster's spell save DC. On fail:
/// 10d10 psychic damage AND Frightened for 10 rounds. On success: no
/// damage, no fear. RAW makes the target take 1d10 psychic on each of
/// its turns while Frightened by Weird; we collapse the iterated DoT
/// into the on-cast nuke for engine simplicity (the Frightened tag is
/// the load-bearing combat penalty — disadvantage on attacks). The
/// frightened lock is independent of concentration in RAW; we mirror
/// that with a flat Rounds(10) timer.
pub struct Weird {}

impl Action for Weird {
    fn name(&self) -> &str {
        "weird"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["wd", "fear9"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 6 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 120 ft RAW = 48 tiles.
        Some(48)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Psychic]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(9)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spellcasting_save_dc();
        // Shared 10d10 psychic roll. Fail = full damage + Frightened;
        // success = nothing. No half-on-save by RAW for Weird.
        let raw = encounter.roll_empowered_sum(caster_id, 10, 10);
        encounter.log(format!(
            "  weird: 10d10({}) shared {:?}",
            raw,
            DamageType::Psychic
        ));
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        for tid in encounter.enemy_burst_targets(caster_id, point, 6) {
            let save = encounter.roll_save_against_caster(tid, AbilityScoreType::Wisdom, dc, caster_id);
            if save.passed() {
                continue;
            }
            effects.push(Box::new(DealDamage {
                actor_id: tid,
                amount: raw,
                damage_type: DamageType::Psychic,
            }));
            effects.push(Box::new(ApplyCondition {
                actor_id: tid,
                condition: Condition::Frightened,
                timer: ConditionTimer::Rounds(10),
            }));
        }
        effects
    }
}

pub static WEIRD: LazyLock<Weird> = LazyLock::new(|| Weird {});

/// Regenerate — level-7 transmutation (cleric / druid / bard). Touch
/// (1 tile reach). The target regains 4d8+15 HP immediately. Distinct
/// from the high-tier Heal (lv6 flat 70 HP, single-creature, several
/// condition cleanses) — Regenerate's headline is the 4d8+15 burst plus
/// the regrowth flavor (RAW also regrows severed limbs over the next
/// hour and grants 1 HP/round for 1 hour; we don't model limbs or
/// hour-scale ticks, so the on-cast HP burst is the load-bearing
/// effect). Slots the lv7 single-target heal lane alongside Power
/// Word Heal (lv9 cap heal).
pub struct Regenerate {}

impl Action for Regenerate {
    fn name(&self) -> &str {
        "regenerate"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["regen", "rg"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(crate::actions::action_template::MELEE_REACH)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn is_heal(&self) -> bool {
        true
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(7)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        // Snapshot the Disciple of Life bonus (+9 at slot 7) BEFORE the
        // (mutable) roll — the borrow checker won't let the caster read
        // outlive `encounter.roll`.
        let bonus = encounter
            .actors
            .get(&caster_id)
            .map(|a| crate::actions::class_features::disciple_of_life_bonus(a, 7))
            .unwrap_or(0);
        let raw = encounter.roll(&Dice::new(4, 8));
        let total = raw + 15 + bonus;
        encounter.log(format!(
            "  regenerate: 4d8+15({}){} = {} HP",
            raw,
            crate::actions::class_features::disciple_of_life_log_suffix(bonus),
            total
        ));
        vec![Box::new(Heal {
            actor_id: target_id,
            amount: total,
        })]
    }
}

pub static REGENERATE: LazyLock<Regenerate> = LazyLock::new(|| Regenerate {});

/// Charm Monster — level-4 enchantment (bard / druid / sorcerer / warlock /
/// wizard). Single-target WIS save vs the caster's CHA-based DC; on fail,
/// the target is Charmed for 10 rounds (1 hour RAW) and gains a
/// `Charmed` back-link to the caster so they can't take hostile actions
/// against them (gated in `validate_input`). Mechanically identical to
/// Charm Person but works against any creature type — RAW differs by
/// pulling the "humanoid only" restriction. Slots cleanly at lv4 between
/// Charm Person (lv1) and the lv5 Dominate Person on the enchantment
/// ladder. No concentration in RAW — install with a flat 10-round timer.
pub struct CharmMonster {}

impl Action for CharmMonster {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Enchantment)
    }
    fn name(&self) -> &str {
        "charm monster"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["cm", "charm-monster"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 30 ft RAW = 12 tiles.
        Some(12)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(4)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.best_spell_save_dc([
            AbilityScoreType::Charisma,
            AbilityScoreType::Wisdom,
            AbilityScoreType::Intelligence,
        ]);
        // Heightened-aware save so a Sorcerer's first-target-burns
        // disadvantage prime fires on the charm. Previously this
        // bypassed Heightened via `roll_save` — fixed in the same
        // sweep that factored `install_charmed_by`.
        let save = encounter.roll_save_against_caster(
            target_id,
            AbilityScoreType::Wisdom,
            dc,
            caster_id,
        );
        if save.passed() {
            return Vec::new();
        }
        // 1 hour RAW capped to encounter-scale via the shared install
        // helper — keeps the Charmed flag + `Charmed` back-link in
        // lockstep with Charm Person / Geas.
        install_charmed_by(target_id, caster_id, ConditionTimer::Rounds(10))
    }
}

pub static CHARM_MONSTER: LazyLock<CharmMonster> = LazyLock::new(|| CharmMonster {});

/// Mind Blank — level-8 abjuration (bard / wizard). Self-cast or touch
/// (we model the single-target touch flavor); for the duration the
/// target is immune to psychic damage and to the Charmed condition (any
/// charm-style enchantment fizzles). RAW also grants immunity to mind-
/// reading and divination — neither is modeled in this engine, so the
/// load-bearing buff is the psychic / charm immunity envelope. No
/// concentration in RAW (lasts 24 hours); we install with a long
/// Rounds(100) timer so it covers any plausible encounter without being
/// permanently durable. Joins the dispellable-buff cohort so Dispel
/// Magic / Counterspell can rip it. Implementation is a single
/// `ApplyCondition` of `MindBlanked` — the actor-side hooks
/// (`effective_damage` for psychic-zero, `add_condition` for charm
/// block) live in `actor_template.rs`.
pub struct MindBlank {}

impl Action for MindBlank {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Abjuration)
    }
    fn name(&self) -> &str {
        "mind blank"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["mb", "blank"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // Touch range — 1-tile reach. RAW: "A willing creature you touch."
        Some(crate::actions::action_template::MELEE_REACH)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(8)
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        _caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // Don't re-cast on an already-blanked ally — wastes the lv8 slot.
        let Some(tid) = first_target_id(target_ids) else {
            return false;
        };
        actor_lacks_condition(encounter, tid, Condition::MindBlanked)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        _caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        encounter.log("  mind blank: psychic + charm immunity installed");
        vec![Box::new(ApplyCondition {
            actor_id: target_id,
            condition: Condition::MindBlanked,
            timer: ConditionTimer::Rounds(100),
        })]
    }
}

pub static MIND_BLANK: LazyLock<MindBlank> = LazyLock::new(|| MindBlank {});

/// Lightning Arrow — level-3 ranger evocation, bonus action,
/// concentration. The next ranged weapon attack the ranger makes deals
/// an extra 4d8 lightning damage to the target — modeled via the
/// `LightningArrowPrimed` rider in the on-hit table (ranged_only=true,
/// consume_on_trigger=true). The 10-ft splash clause is omitted from
/// the engine here (the rider table doesn't carry a burst follow-up);
/// the load-bearing buff is the +4d8 prime on the consuming hit, which
/// is what the AI's focus-fire pipeline leverages. Reuses the
/// `SmiteSpell` chassis since the per-cast shape — bonus action +
/// level-3 slot, concentration, prime-the-caster — matches every other
/// Smite spell in the engine.
pub static LIGHTNING_ARROW: SmiteSpell = SmiteSpell {
    display_name: "lightning arrow",
    aliases: &["la", "lightning-arrow"],
    spell_slot_lvl: 3,
    prime: Condition::LightningArrowPrimed,
    concentration_name: "Lightning Arrow",
};

/// Ensnaring Strike — level-1 ranger conjuration, bonus action,
/// concentration. Primes the ranger's next weapon attack (either
/// melee or ranged) with +1d6 piercing and a STR save (vs the
/// ranger's WIS-based DC) gates Restrained (10 rounds) on fail.
///
/// Signature ranger-flavored lockdown at the lv1 slot tier. RAW's
/// ongoing 1d6-per-turn drip while restrained collapses to a single
/// on-hit 1d6 rider — the load-bearing tactical effect is the
/// Restrained install, and the smite-follow-up shape doesn't carry a
/// per-round DoT so the drip is left to future work if
/// `ConditionTemplate::HeatMetaled`-style round-tick damage lands as
/// a generalized surface. Sibling to the paladin's Wrathful Smite on
/// the lv1 "bonus-action prime + save-vs-condition follow-up" lane:
/// same slot cost, same die (1d6), same STR-save vs a self-DC, but
/// distinct on four axes:
///   1. **Damage type** — Piercing (thorny vines) vs Psychic.
///   2. **Condition** — Restrained vs Frightened.
///   3. **DC anchor** — WIS (ranger's spellcasting mod) vs CHA.
///   4. **Weapon lane** — either melee or ranged (RAW's "the next
///      time you hit a creature with a weapon attack" broad envelope)
///      vs melee-only.
///
/// The either-lane routing makes Ensnaring Strike the first entry on
/// the `ALL_RANGED_SMITE_SPELLS` registry whose rider fires on melee
/// swings too — the AI's `try_ranged_smite_spell` gate uses bow-range
/// as the "enemy in engagement window" heuristic, but the primed hit
/// itself can consume on the scimitar fallback when a melee threat
/// closes through the kite.
pub static ENSNARING_STRIKE: SmiteSpell = SmiteSpell {
    display_name: "ensnaring strike",
    aliases: &["ensnaring", "smite-ensnare", "vines"],
    spell_slot_lvl: 1,
    prime: Condition::EnsnaringStriking,
    concentration_name: "Ensnaring Strike",
};

/// Zephyr Strike — level-1 ranger transmutation (XGtE), bonus action,
/// concentration. Primes the ranger's next weapon attack (either melee
/// or ranged — RAW's "the next attack you make on this turn" broad
/// envelope, unrestricted by weapon lane) with +1d8 Force damage via
/// the on-hit rider table.
///
/// Signature ranger-flavored raw-damage prime at the lv1 slot tier.
/// Sibling to Ensnaring Strike on the "either-lane bonus-action prime"
/// corner but distinct on the tactical trade:
///   1. **Damage type** — Force (rare-resisted, punches through
///      nearly every typed-defense lane) vs Piercing (broadly halved
///      by heavy-armor / DamageResistant / typed rows).
///   2. **Die size** — 1d8 vs 1d6 — raw-damage lane.
///   3. **Follow-up** — none (pure damage) vs STR-save-vs-Restrained.
///   4. **Rider consume** — same one-shot semantics as Ensnaring
///      Strike and Lightning Arrow: the first weapon hit consumes
///      the prime.
///
/// The either-lane routing places Zephyr Strike alongside Ensnaring
/// Strike as an entry on `ALL_RANGED_SMITE_SPELLS` whose rider fires
/// on melee swings too — the AI's `try_ranged_smite_spell` picker
/// uses bow-range as the "enemy in engagement window" heuristic, but
/// the primed hit itself can consume on the scimitar fallback when a
/// melee threat closes through the kite.
///
/// RAW's companion clauses (advantage on the primed attack, +30 ft
/// walking speed for the turn, and no opportunity attacks provoked
/// while the spell is up) are left as future work — a per-hit
/// caster-side attack-mode advantage rider needs a chokepoint the
/// engine doesn't yet expose, and the movement + OA-immunity clauses
/// fold into the same turn-scoped kite envelope the ranger's baseline
/// Fighting Style: Archery + Vanish already lean on. The +1d8 Force
/// damage prime is the load-bearing tactical clause and rides here
/// alone, matching the way Ensnaring Strike's per-round 1d6 drip
/// while Restrained and Lightning Arrow's 10-ft splash are similarly
/// left to future work in favor of the smite-follow-up chassis's
/// core one-shot rider shape.
pub static ZEPHYR_STRIKE: SmiteSpell = SmiteSpell {
    display_name: "zephyr strike",
    aliases: &["zephyr", "smite-zephyr", "wind-strike"],
    spell_slot_lvl: 1,
    prime: Condition::ZephyrStriking,
    concentration_name: "Zephyr Strike",
};

/// Conjure Volley — level-5 ranger conjuration. The ranger fires a
/// volley of arrows into the air; they rain down on a 40-ft (16-tile)
/// radius cylinder at a tile within 150 ft (60 tiles). Every creature
/// in the burst makes a DEX save vs the caster's WIS-based DC: fail =
/// 8d8 piercing, success = half. Friend-or-foe agnostic by RAW —
/// route through the neutral-burst helper so allies caught in the
/// volley take the hit too (encourages careful targeting). No
/// concentration in RAW.
pub struct ConjureVolley {}

impl Action for ConjureVolley {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Conjuration)
    }
    fn name(&self) -> &str {
        "conjure volley"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["volley", "cv", "arrow-rain"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        // 40 ft cylinder ≈ 16-tile burst (RAW radius is the cylinder
        // footprint; we approximate as a Chebyshev burst).
        TargetingSchema::Burst { radius: 16 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 150 ft RAW = 60 tiles.
        Some(60)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Piercing]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(5)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.best_spell_save_dc([
            AbilityScoreType::Wisdom,
            AbilityScoreType::Charisma,
            AbilityScoreType::Intelligence,
        ]);
        let (effects, _) = neutral_burst_save_for_half(
            encounter,
            caster_id,
            point,
            16,
            AbilityScoreType::Dexterity,
            dc,
            Dice::new(8, 8),
            DamageType::Piercing,
            "conjure volley",
        );
        effects
    }
}

pub static CONJURE_VOLLEY: LazyLock<ConjureVolley> = LazyLock::new(|| ConjureVolley {});

/// Tsunami — level-8 druid conjuration, concentration. A massive wall of
/// water crashes through the area: every creature in a 30-ft (6-tile)
/// burst centered on a tile within 120 ft (48 tiles) makes a STR save
/// vs the caster's WIS-based DC. On fail: 6d10 bludgeoning AND Prone.
/// On success: half damage, no prone. RAW the wall persists and re-
/// damages for several rounds as it sweeps the battlefield; we collapse
/// the iterated sweep into the on-cast burst (consistent with Fire Storm
/// / Incendiary Cloud) since the engine doesn't model moving damage
/// zones. Friend-or-foe agnostic via the neutral-burst route — the wave
/// doesn't discriminate. Concentration-bound on the caster so re-casts
/// drop the prior install cleanly.
pub struct Tsunami {}

impl Action for Tsunami {
    fn name(&self) -> &str {
        "tsunami"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ts", "wave", "tidal"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 6 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 120 ft RAW = 48 tiles.
        Some(48)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Bludgeoning]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(8)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spell_save_dc(AbilityScoreType::Wisdom);
        let (mut effects, saves) = neutral_burst_save_for_half(
            encounter,
            caster_id,
            point,
            6,
            AbilityScoreType::Strength,
            dc,
            Dice::new(6, 10),
            DamageType::Bludgeoning,
            "tsunami",
        );
        // Prone rider on every actor that failed their STR save —
        // mirrors Tidal Wave's prone-on-fail clause but with the bigger
        // burst footprint.
        push_condition_on_failed_save(
            &mut effects,
            &saves,
            Condition::Prone,
            ConditionTimer::Permanent,
        );
        // Concentration mark so a re-cast drops the prior install
        // cleanly. The burst already landed at cast time; the
        // concentration is just the engine's bookkeeping anchor.
        effects.push(Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData::new("Tsunami"),
        }));
        effects
    }
}

pub static TSUNAMI: LazyLock<Tsunami> = LazyLock::new(|| Tsunami {});

/// Wall of Thorns — level-6 druid conjuration, concentration. The druid
/// conjures a wall of bristling thorns at a tile within 120 ft (48
/// tiles). Every enemy whose footprint touches the 3-tile (15-ft)
/// burst takes 7d8 piercing on a failed DEX save (half on success). RAW
/// the wall persists and damages any creature that ends a turn within
/// 10 ft of it; we collapse the sustained damage zone into the on-cast
/// burst (consistent with the rest of the engine's wall / sphere
/// spells) since the engine doesn't model persistent damage terrain
/// outside `Spiked`. Concentration-bound on the caster — re-casts drop
/// the prior install cleanly. Enemy-only burst since RAW lets the
/// druid choose the wall's orientation so allies stand on the safe
/// side.
pub struct WallOfThorns {}

impl Action for WallOfThorns {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Conjuration)
    }
    fn name(&self) -> &str {
        "wall of thorns"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["wot", "thorns", "wall-thorns"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        // 60ft long, 10ft thick wall ≈ 3-tile Chebyshev burst (treat the
        // wall as a damage zone since the engine doesn't model linear
        // walls as terrain modifications).
        TargetingSchema::Burst { radius: 3 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 120 ft RAW = 48 tiles.
        Some(48)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Piercing]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(6)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spell_save_dc(AbilityScoreType::Wisdom);
        let (mut effects, _) = enemy_burst_save_for_half(
            encounter,
            caster_id,
            point,
            3,
            AbilityScoreType::Dexterity,
            dc,
            Dice::new(7, 8),
            DamageType::Piercing,
            "wall of thorns",
        );
        effects.push(Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData::new("Wall of Thorns"),
        }));
        effects
    }
}

pub static WALL_OF_THORNS: LazyLock<WallOfThorns> = LazyLock::new(|| WallOfThorns {});

/// Barkskin — 5e level-2 transmutation, concentration. Touch range; the
/// target's skin hardens to bark, setting their AC to 16 unless their
/// natural / worn-armor AC is already higher. We model the floor via
/// the existing `ac_floor()` accessor on `ActorInstance` (which now
/// reads both `MageArmored` and `Barkskinned`), mirroring how Mage
/// Armor plugs into `armor_class()`. Concentration-bound on the caster.
///
/// custom_validate gates against re-priming an already-barkskinned ally
/// so the AI's heal/buff pipeline doesn't burn the slot on a no-op.
pub struct Barkskin {}

impl Action for Barkskin {
    fn name(&self) -> &str {
        "barkskin"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["bark", "bs-skin"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(crate::actions::action_template::MELEE_REACH)
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(2)
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // Skip if the caster already concentrates on something else, or
        // the target already wears the buff.
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return false;
        };
        if caster.is_concentrating() {
            return false;
        }
        let Some(target_id) = first_target_id(target_ids) else {
            return false;
        };
        actor_lacks_condition(encounter, target_id, Condition::Barkskinned)
    }
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        vec![
            Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Barkskinned,
                // RAW: 1 hour. Capped to 10 rounds in line with other
                // concentration buffs — concentration drop is the load-
                // bearing termination path anyway.
                timer: ConditionTimer::Rounds(10),
            }),
            Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions(
                    "Barkskin",
                    vec![(target_id, Condition::Barkskinned)],
                ),
            }),
        ]
    }
}

pub static BARKSKIN: LazyLock<Barkskin> = LazyLock::new(|| Barkskin {});

/// Pass Without Trace — 5e level-2 abjuration, concentration. The caster
/// and every ally inside a 30-ft sphere of the caster receives the
/// `Untracked` condition — attackers have disadvantage on attack rolls
/// against them for the duration. RAW grants +10 to Stealth checks; the
/// engine's stealth lane is collapsed into the existing attack-mode
/// disadvantage cohort so the buff lands as "harder to target."
///
/// custom_validate gates against re-casting while concentrating or with
/// no allies in the aura — the latter blocks burning the slot on a
/// solo-caster picker.
pub struct PassWithoutTrace {}

impl Action for PassWithoutTrace {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Abjuration)
    }
    fn name(&self) -> &str {
        "pass without trace"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["pwt", "trace", "pass"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(2)
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return false;
        };
        if caster.is_concentrating() {
            return false;
        }
        // Skip a no-op re-prime on the caster.
        !caster.has_condition(Condition::Untracked)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let center = caster.location();
        // 30 ft sphere = 12 tiles. ally_burst_targets gives every
        // combat-active teammate within radius (caster's team only);
        // the caster's at gap 0 from `center` so they're already
        // included in the returned list — no manual append needed.
        let targets: Vec<usize> = encounter.ally_burst_targets(caster_id, center, 12);
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        let mut tagged: Vec<(usize, Condition)> = Vec::new();
        for target_id in &targets {
            effects.push(Box::new(ApplyCondition {
                actor_id: *target_id,
                condition: Condition::Untracked,
                timer: ConditionTimer::Rounds(10),
            }));
            tagged.push((*target_id, Condition::Untracked));
        }
        effects.push(Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData::with_conditions("Pass Without Trace", tagged),
        }));
        encounter.log(format!(
            "  pass without trace: {} allies cloaked",
            targets.len()
        ));
        effects
    }
}

pub static PASS_WITHOUT_TRACE: LazyLock<PassWithoutTrace> =
    LazyLock::new(|| PassWithoutTrace {});

/// Holy Weapon — 5e level-5 paladin evocation, concentration. The caster
/// channels divine light into their weapon: every weapon attack hit deals
/// an extra 2d8 radiant damage via the on_hit_riders table. Persistent
/// (not consumed on trigger) and self-only — mirrors Crusader's Mantle /
/// Spirit Shroud in the rider table but with bigger dice. Concentration-
/// bound; the slot drops the buff cleanly on concentration end.
///
/// custom_validate folds the standard "not already buffed + not already
/// concentrating" gates so the AI doesn't waste the lv5 slot on a no-op.
pub struct HolyWeapon {}

impl Action for HolyWeapon {
    fn name(&self) -> &str {
        "holy weapon"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["hw", "holy", "blessed-weapon"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        // Indirect: the rider lands on the next hit, not on cast.
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(5)
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return false;
        };
        !caster.is_concentrating() && !caster.has_condition(Condition::HolyWeaponed)
    }
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        self_concentration_buff_effects(
            caster_id,
            "Holy Weapon",
            Condition::HolyWeaponed,
            ConditionTimer::Rounds(10),
        )
    }
}

pub static HOLY_WEAPON: LazyLock<HolyWeapon> = LazyLock::new(|| HolyWeapon {});

/// Thunder Step — level-3 conjuration, action. Teleport up to 90ft (36
/// tiles) and deal 3d10 thunder damage (CON save for half) to all
/// creatures within 10ft (4 tiles) of the origin. Combines mobility with
/// area denial — the warlock/sorcerer/wizard escape tool.
pub struct ThunderStep {}

impl Action for ThunderStep {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Conjuration)
    }
    fn name(&self) -> &str {
        "thunder step"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["tstep", "thunder-step"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SinglePoint
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(36)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Thunder]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(3)
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        let Some(point) = first_target_location(target_locations) else {
            return false;
        };
        encounter.can_move_to(caster_id, point)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(dest) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let origin = caster.location();
        let dc = caster.best_spell_save_dc([
            AbilityScoreType::Intelligence,
            AbilityScoreType::Charisma,
            AbilityScoreType::Wisdom,
        ]);
        let (mut effects, _saves) = neutral_burst_save_for_half(
            encounter,
            caster_id,
            origin,
            4,
            AbilityScoreType::Constitution,
            dc,
            Dice::new(3, 10),
            DamageType::Thunder,
            "thunder step",
        );
        effects.push(Box::new(crate::engine::side_effects::TeleportActor {
            actor_id: caster_id,
            dest,
        }));
        effects
    }
}

pub static THUNDER_STEP: LazyLock<ThunderStep> = LazyLock::new(|| ThunderStep {});

/// Absorb Elements — level-1 abjuration, reaction. When you take acid,
/// cold, fire, lightning, or thunder damage, grant yourself resistance
/// (via DamageResistant) until start of next turn and store the energy
/// for a +1d6 melee damage rider on your next hit. We model the
/// resistance as the general DamageResistant condition and skip the
/// per-type modeling for simplicity.
pub struct AbsorbElements {}

impl Action for AbsorbElements {
    fn name(&self) -> &str {
        "absorb elements"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["absorb", "ae"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        vec![Resource::Reaction, Resource::SpellSlot(1)]
    }
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        vec![
            Box::new(ApplyCondition {
                actor_id: caster_id,
                condition: Condition::DamageResistant,
                timer: ConditionTimer::UntilStartOfNextTurn,
            }),
            Box::new(ApplyCondition {
                actor_id: caster_id,
                condition: Condition::AbsorbedElements,
                timer: ConditionTimer::Rounds(2),
            }),
        ]
    }
}

pub static ABSORB_ELEMENTS: LazyLock<AbsorbElements> = LazyLock::new(|| AbsorbElements {});

/// Warding Wind — level-2 evocation, concentration. Creates a 10ft
/// radius of strong wind around the caster: ranged attacks into and
/// out of the area have disadvantage. Modeled as self-buff with the
/// existing Untracked condition (disadvantage to attackers) to
/// approximate the defensive layer.
pub struct WardingWind {}

impl Action for WardingWind {
    fn name(&self) -> &str {
        "warding wind"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["wwind"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(2)
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return false;
        };
        !caster.is_concentrating()
    }
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        self_concentration_buff_effects(
            caster_id,
            "Warding Wind",
            Condition::Untracked,
            ConditionTimer::Rounds(10),
        )
    }
}

pub static WARDING_WIND: LazyLock<WardingWind> = LazyLock::new(|| WardingWind {});

/// Shadow Blade — level-2 illusion, concentration (bonus action). Creates
/// a magical blade of solidified shadow: the caster gains advantage on
/// attacks (Invisible analogue via the Transformed condition) and their
/// melee attacks deal extra psychic damage (+2d8 via the SpiritShrouded
/// condition's on-hit rider lane, typed as psychic). Concentration-bound.
pub struct ShadowBlade {}

impl Action for ShadowBlade {
    fn name(&self) -> &str {
        "shadow blade"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["sblade", "shadow"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        bonus_action_and_slot(2)
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return false;
        };
        !caster.is_concentrating() && !caster.has_condition(Condition::SpiritShrouded)
    }
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        self_concentration_buff_effects(
            caster_id,
            "Shadow Blade",
            Condition::SpiritShrouded,
            ConditionTimer::Rounds(10),
        )
    }
}

pub static SHADOW_BLADE: LazyLock<ShadowBlade> = LazyLock::new(|| ShadowBlade {});

/// Entangle — level-1 conjuration, concentration. A 20ft burst of grasping
/// vines sprouts from a point within 90ft. Every creature in the burst
/// makes a STR save; on fail, they're Restrained for up to 10 rounds.
/// Concentration-bound.
pub struct Entangle {}

impl Action for Entangle {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Conjuration)
    }
    fn name(&self) -> &str {
        "entangle"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ent", "vines"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 4 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(36)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn is_harmful(&self) -> bool {
        true
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(1)
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        encounter.caster_can_concentrate(caster_id)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spell_save_dc(AbilityScoreType::Wisdom);
        encounter.log("  entangle: grasping vines erupt!");
        let mut effs: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        let mut restrained: Vec<(usize, Condition)> = Vec::new();
        for target_id in encounter.neutral_burst_targets(caster_id, point, 4) {
            let save = encounter.roll_save_against_caster(target_id, AbilityScoreType::Strength, dc, caster_id);
            if !save.passed() {
                restrained.push((target_id, Condition::Restrained));
                effs.push(Box::new(ApplyCondition {
                    actor_id: target_id,
                    condition: Condition::Restrained,
                    timer: ConditionTimer::Rounds(10),
                }));
            }
        }
        effs.push(Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData::with_conditions("Entangle", restrained),
        }));
        effs
    }
}

pub static ENTANGLE: LazyLock<Entangle> = LazyLock::new(|| Entangle {});

/// Produce Flame — druid cantrip. The caster conjures a small flame in
/// their hand and hurls it at a target within 30ft. Ranged spell attack;
/// on hit, 1d8 fire damage. No spell slot cost.
pub struct ProduceFlame {}

impl Action for ProduceFlame {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Conjuration)
    }
    fn name(&self) -> &str {
        "produce flame"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["pf", "pflame"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(12)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Fire]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_only()
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let attack_mod = caster.spell_attack_modifier(AbilityScoreType::Wisdom);
        let n = crate::engine::util::cantrip_dice_count(caster.level());
        spell_attack(
            encounter,
            caster_id,
            target_id,
            "produce flame",
            attack_mod,
            Dice::new(n, 8),
            DamageType::Fire,
            false,
        )
    }
}

pub static PRODUCE_FLAME: LazyLock<ProduceFlame> = LazyLock::new(|| ProduceFlame {});

/// Create Bonfire — cantrip (XGtE). A 5ft bonfire appears at a point;
/// every creature in the tile makes a DEX save or takes 1d8 fire. The
/// bonfire persists as concentration — on each subsequent round-end, any
/// creature still standing on the point must save again (we approximate
/// with a single burst at cast time + concentration install).
pub struct CreateBonfire {}

impl Action for CreateBonfire {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Conjuration)
    }
    fn name(&self) -> &str {
        "create bonfire"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["bonfire", "cb"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 0 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Fire]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_only()
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spell_save_dc(AbilityScoreType::Intelligence);
        let n = crate::engine::util::cantrip_dice_count(caster.level());
        let die = Dice::new(n, 8);
        let raw = encounter.roll(&die);
        encounter.log(format!("  create bonfire: {}({}) fire", die, raw));
        let mut effs = crate::actions::action_template::resolve_burst_save_damage(
            encounter,
            caster_id,
            point,
            0,
            AbilityScoreType::Dexterity,
            dc,
            raw,
            DamageType::Fire,
        );
        effs.push(Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData::new("Create Bonfire"),
        }));
        effs
    }
}

pub static CREATE_BONFIRE: LazyLock<CreateBonfire> = LazyLock::new(|| CreateBonfire {});

/// Flame Blade — level-2 evocation, concentration. The caster conjures a
/// fiery blade: melee spell attack, 3d6 fire, using the caster's
/// spellcasting modifier. Lasts up to 10 rounds (concentration). We model
/// as a self-buff that applies the SpiritShrouded condition (reusing the
/// melee-only +1d8 rider lane) typed as fire.
pub struct FlameBlade {}

impl Action for FlameBlade {
    fn name(&self) -> &str {
        "flame blade"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["fblade", "flblade"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        bonus_action_and_slot(2)
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        encounter.caster_can_concentrate(caster_id)
    }
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        self_concentration_buff_effects(
            caster_id,
            "Flame Blade",
            Condition::SpiritShrouded,
            ConditionTimer::Rounds(10),
        )
    }
}

pub static FLAME_BLADE: LazyLock<FlameBlade> = LazyLock::new(|| FlameBlade {});

/// Chill Touch (ranged, necrotic cantrip) variant — ranged spell attack
/// that also prevents healing for 1 round. We already have Chill Touch
/// defined above. This is Infestation — a WIS-save cantrip that deals
/// 1d6 poison on a failed save. Simple save-or-damage cantrip pattern.
pub struct Infestation {}

impl Action for Infestation {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Conjuration)
    }
    fn name(&self) -> &str {
        "infestation"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["infest", "bugs"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(12)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Poison]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_only()
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.best_spell_save_dc([AbilityScoreType::Wisdom, AbilityScoreType::Intelligence]);
        let n = crate::engine::util::cantrip_dice_count(caster.level());
        let save = encounter.roll_save_against_caster(target_id, AbilityScoreType::Constitution, dc, caster_id);
        if save.passed() {
            return Vec::new();
        }
        let die = Dice::new(n, 6);
        // Route the roll through the shared caster-aware chokepoint
        // rather than `roll` directly: that is where the Sorcerer's
        // Empowered Spell reroll, the Evocation Wizard's Empowered
        // Evocation bonus and the cleric's Potent Spellcasting bonus
        // all live. A bare `roll` here meant every one of them
        // silently skipped this cantrip.
        let dmg = encounter.roll_empowered_sum(caster_id, die.count, die.faces);
        encounter.log(format!("  infestation: {}({}) poison", die, dmg));
        vec![Box::new(DealDamage {
            actor_id: target_id,
            amount: dmg,
            damage_type: DamageType::Poison,
        })]
    }
}

pub static INFESTATION: LazyLock<Infestation> = LazyLock::new(|| Infestation {});

/// Ray of Enfeeblement — level-2 necromancy, concentration. Ranged spell
/// attack; on hit, the target deals half damage with weapon attacks that
/// use Strength for the duration. We approximate with the Poisoned
/// condition (disadvantage on attacks + ability checks) for 10 rounds.
pub struct RayOfEnfeeblement {}

impl Action for RayOfEnfeeblement {
    fn name(&self) -> &str {
        "ray of enfeeblement"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["roe", "enfeeble"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(2)
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        encounter.caster_can_concentrate(caster_id)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let attack_mod = caster.best_spell_attack_modifier([
            AbilityScoreType::Intelligence,
            AbilityScoreType::Wisdom,
        ]);
        // Ray of Enfeeblement is a spell attack whose payload is a
        // condition rather than damage, so the damage-rolling
        // `spell_attack_outcome` is no use to it — but the attack roll
        // itself is the same attack roll, and this spell used to
        // open-code it. That cost it cover, Sanctuary, the defender-side
        // reactive taxes, Bless and Bane, the caster's attack buffs,
        // Multiattack Defense, the interception cohort and the
        // hit-this-turn mark, none of which anyone chose to skip.
        // `spell_attack_roll` is the attack-roll half on its own.
        if !spell_attack_roll(
            encounter,
            caster_id,
            target_id,
            "ray of enfeeblement",
            attack_mod,
            false,
        )
        .hit
        {
            return Vec::new();
        }
        let cond = Condition::Poisoned;
        let timer = ConditionTimer::Rounds(10);
        vec![
            Box::new(ApplyCondition {
                actor_id: target_id,
                condition: cond,
                timer,
            }),
            Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions(
                    "Ray of Enfeeblement",
                    vec![(target_id, cond)],
                ),
            }),
        ]
    }
}

pub static RAY_OF_ENFEEBLEMENT: LazyLock<RayOfEnfeeblement> =
    LazyLock::new(|| RayOfEnfeeblement {});

/// Wither and Bloom — level-2 necromancy (Strixhaven). A 10ft burst deals
/// 2d6 necrotic to enemies (CON save, half); one ally in the burst heals
/// for the amount rolled on the damage dice. We approximate by bursting
/// 2d6 necrotic, then healing the caster for the average (6 HP).
pub struct WitherAndBloom {}

impl Action for WitherAndBloom {
    fn name(&self) -> &str {
        "wither and bloom"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["wab", "wither"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 2 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Necrotic]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(2)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.best_spell_save_dc([AbilityScoreType::Intelligence, AbilityScoreType::Wisdom]);
        let raw = encounter.roll(&Dice::new(2, 6));
        encounter.log(format!("  wither and bloom: 2d6({}) necrotic burst", raw));
        let mut effs = crate::actions::action_template::resolve_burst_save_damage(
            encounter,
            caster_id,
            point,
            2,
            AbilityScoreType::Constitution,
            dc,
            raw,
            DamageType::Necrotic,
        );
        let heal_amount = raw.min(6);
        encounter.log(format!("  bloom: caster heals {} HP", heal_amount));
        effs.push(Box::new(Heal {
            actor_id: caster_id,
            amount: heal_amount,
        }));
        effs
    }
}

pub static WITHER_AND_BLOOM: LazyLock<WitherAndBloom> = LazyLock::new(|| WitherAndBloom {});

/// Hunger of Hadar — level-3 conjuration (warlock-exclusive, concentration).
/// A 20ft sphere of frigid blackness: enemies inside the sphere take 2d6
/// cold (no save) + 2d6 acid (DEX save for none) at the start of their turns
/// RAW. We collapse the sustained-zone mechanic to a one-shot burst at cast
/// time: 2d6 cold + DEX-save 2d6 acid to every enemy in a radius-4 sphere.
/// The burst runs through `enemy_burst_save_for_half` for the acid half,
/// then flat cold damage for the cold half. Concentration-bound on the caster.
pub struct HungerOfHadar {}

impl Action for HungerOfHadar {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Conjuration)
    }
    fn name(&self) -> &str {
        "hunger of hadar"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["hoh", "hadar"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 4 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(60)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Cold, DamageType::Acid]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(3)
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        encounter.caster_can_concentrate(caster_id)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.best_spell_save_dc([AbilityScoreType::Charisma]);
        encounter.log("  hunger of hadar: the void opens...".to_string());
        let cold_raw = encounter.roll_empowered_sum(caster_id, 2, 6);
        let targets = encounter.enemy_burst_targets(caster_id, point, 4);
        let mut effs: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        for tid in &targets {
            encounter.log(format!(
                "  cold lash: 2d6({}) cold on {}",
                cold_raw,
                encounter.actors.get(tid).map(|a| a.name()).unwrap_or("?")
            ));
            effs.push(Box::new(DealDamage {
                actor_id: *tid,
                amount: cold_raw,
                damage_type: DamageType::Cold,
            }));
        }
        let (acid_effs, _) = enemy_burst_save_for_half(
            encounter,
            caster_id,
            point,
            4,
            AbilityScoreType::Dexterity,
            dc,
            Dice::new(2, 6),
            DamageType::Acid,
            "hadar acid",
        );
        effs.extend(acid_effs);
        effs.push(Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData::new("Hunger of Hadar"),
        }));
        effs
    }
}

pub static HUNGER_OF_HADAR: LazyLock<HungerOfHadar> = LazyLock::new(|| HungerOfHadar {});

/// Cloud of Locusts — level-3 conjuration (druid / ranger). Swarm of
/// biting insects fills a 20ft sphere. Enemies in the burst take 4d10
/// piercing (CON save, half). Concentration-free instant burst that fits
/// the druid's "nature damage" niche between Moonbeam and Insect Plague.
/// We reuse the enemy_burst_save_for_half template.
pub struct SwarmOfLocusts {}

impl Action for SwarmOfLocusts {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Conjuration)
    }
    fn name(&self) -> &str {
        "conjure locusts"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["locusts", "swarm"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 4 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Piercing]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(3)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.best_spell_save_dc([AbilityScoreType::Wisdom, AbilityScoreType::Intelligence]);
        let (effs, _) = enemy_burst_save_for_half(
            encounter,
            caster_id,
            point,
            4,
            AbilityScoreType::Constitution,
            dc,
            Dice::new(4, 10),
            DamageType::Piercing,
            "conjure locusts",
        );
        effs
    }
}

pub static CONJURE_LOCUSTS: LazyLock<SwarmOfLocusts> = LazyLock::new(|| SwarmOfLocusts {});

/// Toll the Dead cantrip scaling — this spell already exists but let's
/// add Eldritch Smite as a lv5 warlock ability. The Eldritch Smite is
/// modeled as a single-target force damage attack plus a prone rider
/// on a failed STR save. Level 5 spell, warlock-only.
pub struct EldritchSmite {}

impl Action for EldritchSmite {
    fn name(&self) -> &str {
        "eldritch smite"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["esmite"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(1)
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Force]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(5)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(tid) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.best_spell_save_dc([AbilityScoreType::Charisma]);
        let raw = encounter.roll_empowered_sum(caster_id, 4, 8);
        encounter.log(format!("  eldritch smite: 4d8({}) force", raw));
        let mut effs: Vec<Box<dyn ApplicableSideEffect>> = vec![Box::new(DealDamage {
            actor_id: tid,
            amount: raw,
            damage_type: DamageType::Force,
        })];
        let save = encounter.roll_save_against_caster(tid, AbilityScoreType::Strength, dc, caster_id);
        if !save.passed() {
            encounter.log("  the target is knocked prone!".to_string());
            effs.push(Box::new(ApplyCondition {
                actor_id: tid,
                condition: Condition::Prone,
                timer: ConditionTimer::Permanent,
            }));
        }
        effs
    }
}

pub static ELDRITCH_SMITE: LazyLock<EldritchSmite> = LazyLock::new(|| EldritchSmite {});

/// Crown of Thorns — a homebrew-adjacent lv2 druid concentration spell.
/// An enemy-only 10ft burst of thorny vines deals 2d8 piercing and
/// applies Restrained on a failed STR save. Fits the druid's
/// "control-through-nature" niche between Spike Growth and Plant Growth.
pub struct CrownOfThorns {}

impl Action for CrownOfThorns {
    fn name(&self) -> &str {
        "crown of thorns"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["thorns", "cot"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 2 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Piercing]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(2)
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        encounter.caster_can_concentrate(caster_id)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.best_spell_save_dc([AbilityScoreType::Wisdom]);
        let (mut effs, saves) = enemy_burst_save_for_half(
            encounter,
            caster_id,
            point,
            2,
            AbilityScoreType::Strength,
            dc,
            Dice::new(2, 8),
            DamageType::Piercing,
            "crown of thorns",
        );
        let mut conc_conditions: Vec<(usize, Condition)> = Vec::new();
        for (tid, passed) in &saves {
            if !passed {
                effs.push(Box::new(ApplyCondition {
                    actor_id: *tid,
                    condition: Condition::Restrained,
                    timer: ConditionTimer::Rounds(10),
                }));
                conc_conditions.push((*tid, Condition::Restrained));
            }
        }
        effs.push(Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData::with_conditions("Crown of Thorns", conc_conditions),
        }));
        effs
    }
}

pub static CROWN_OF_THORNS: LazyLock<CrownOfThorns> = LazyLock::new(|| CrownOfThorns {});

/// Silvery Barbs — level-1 enchantment, reaction (Strixhaven). When a
/// creature you can see within 60 feet succeeds on an attack roll, ability
/// check, or saving throw, you magically distract them: they must reroll
/// and use the lower result. Additionally, you can choose one creature
/// you can see (including yourself) to gain advantage on their next
/// attack roll, ability check, or saving throw.
///
/// We model the load-bearing half: the target gets Mocked (disadvantage on
/// their next attack roll) and an ally gets Inspired (advantage on their
/// next roll). The "force a reroll" part is too tightly coupled with the
/// event pipeline to model retroactively — Mocked is the RAW-closest
/// approximation.
pub struct SilveryBarbs {}

impl Action for SilveryBarbs {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Enchantment)
    }
    fn name(&self) -> &str {
        "silvery barbs"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["barbs", "sb"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn is_harmful(&self) -> bool {
        true
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(24)
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        vec![Resource::Reaction, Resource::SpellSlot(1)]
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let mut effs: Vec<Box<dyn ApplicableSideEffect>> = vec![
            Box::new(ApplyCondition {
                actor_id: target,
                condition: Condition::Mocked,
                timer: ConditionTimer::UntilStartOfNextTurn,
            }),
        ];
        if !encounter.actors.get(&caster_id).is_some_and(|a| a.has_condition(Condition::Inspired)) {
            effs.push(Box::new(ApplyCondition {
                actor_id: caster_id,
                condition: Condition::Inspired,
                timer: ConditionTimer::Rounds(10),
            }));
        }
        effs
    }
}

pub static SILVERY_BARBS: LazyLock<SilveryBarbs> = LazyLock::new(|| SilveryBarbs {});

/// Protection from Energy — level-3 abjuration, concentration, touch range.
/// Grant one creature resistance to one damage type (acid, cold, fire,
/// lightning, or thunder) for the spell's duration. We model as a
/// DamageResistant condition install with concentration. The generic
/// DamageResistant flag halves all incoming damage — close enough for
/// the load-bearing defensive half.
pub struct ProtectionFromEnergy {}

impl Action for ProtectionFromEnergy {
    fn name(&self) -> &str {
        "protection from energy"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["pfe", "prot energy"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn is_heal(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(0)
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(3)
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        let actor = match encounter.actors.get(&caster_id) {
            Some(a) => a,
            None => return false,
        };
        if actor.is_concentrating() {
            return false;
        }
        let Some(tid) = first_target_id(target_ids) else {
            return false;
        };
        let caster_team = actor.team();
        encounter.actors.get(&tid).is_some_and(|t| {
            t.team() == caster_team
                && t.is_combat_active()
                && !t.has_condition(Condition::DamageResistant)
        })
    }
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(tid) = first_target_id(target_ids) else {
            return Vec::new();
        };
        vec![
            Box::new(ApplyCondition {
                actor_id: tid,
                condition: Condition::DamageResistant,
                timer: ConditionTimer::Rounds(100),
            }),
            Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions(
                    "Protection from Energy",
                    vec![(tid, Condition::DamageResistant)],
                ),
            }),
        ]
    }
}

pub static PROTECTION_FROM_ENERGY: LazyLock<ProtectionFromEnergy> =
    LazyLock::new(|| ProtectionFromEnergy {});

/// Remove Curse — level-3 abjuration, touch range. Remove all curses
/// from one creature. We model by stripping the most impactful debuff
/// conditions: Hexed, Bestow Curse variants (modeled as Poisoned in our
/// engine), and Frightened (Wrathful Smite). No concentration.
pub struct RemoveCurse {}

impl Action for RemoveCurse {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Abjuration)
    }
    fn name(&self) -> &str {
        "remove curse"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["rc"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn is_heal(&self) -> bool {
        true
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(0)
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(3)
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        let Some(tid) = first_target_id(target_ids) else {
            return false;
        };
        let actor = match encounter.actors.get(&caster_id) {
            Some(a) => a,
            None => return false,
        };
        let caster_team = actor.team();
        encounter.actors.get(&tid).is_some_and(|t| {
            t.team() == caster_team
                && t.is_combat_active()
                && (t.has_condition(Condition::Hexed)
                    || t.has_condition(Condition::Frightened)
                    || t.has_condition(Condition::Poisoned))
        })
    }
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        _caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::side_effects::RemoveCondition;
        let Some(tid) = first_target_id(target_ids) else {
            return Vec::new();
        };
        vec![
            Box::new(RemoveCondition { actor_id: tid, condition: Condition::Hexed }),
            Box::new(RemoveCondition { actor_id: tid, condition: Condition::Frightened }),
            Box::new(RemoveCondition { actor_id: tid, condition: Condition::Poisoned }),
        ]
    }
}

pub static REMOVE_CURSE: LazyLock<RemoveCurse> = LazyLock::new(|| RemoveCurse {});

/// Dominate Monster — 5e level-8 enchantment, concentration, action.
/// Upgraded version of Dominate Person that works on any creature type
/// (including constructs, undead, elementals, etc.). Target within 60ft
/// makes a WIS save vs the caster's spell DC. On fail, target is
/// Charmed AND Dominated for 10 rounds. The `Charmed` half blocks the
/// target from attacking the dominator (via `Charmed` back-link); the
/// `Dominated` half imposes disadvantage on all attacks.
/// Concentration-bound — dropping concentration frees the target.
pub struct DominateMonster {}

impl Action for DominateMonster {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Enchantment)
    }
    fn name(&self) -> &str {
        "dominate monster"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["dommon", "dom-monster"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60 ft = 24 tiles.
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(8)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spell_save_dc(AbilityScoreType::Charisma);
        // Caster-aware save so Heightened Spell can force disadvantage.
        let save =
            encounter.roll_save_against_caster(target_id, AbilityScoreType::Wisdom, dc, caster_id);
        if save.passed() {
            return Vec::new();
        }
        // Layer Dominated on top of the standard Charmed + its back-link
        // install lane via the shared `install_dominated_by` helper.
        install_dominated_by(target_id, caster_id, "Dominate Monster")
    }
}

pub static DOMINATE_MONSTER: LazyLock<DominateMonster> = LazyLock::new(|| DominateMonster {});

/// Antilife Shell — 5e level-5 abjuration, concentration, action.
/// Self-cast that creates a shimmering barrier preventing non-undead /
/// non-construct creatures from approaching within melee range. We
/// model the load-bearing half: on cast, all adjacent enemies are
/// pushed 4 tiles away from the caster, and the caster gains the
/// `Warded` condition for 10 rounds (disadvantage on incoming attacks
/// — the closest proxy for "can't approach in melee"). Concentration-
/// bound — dropping it removes the ward.
pub struct AntilifeShell {}

impl Action for AntilifeShell {
    fn name(&self) -> &str {
        "antilife shell"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["als", "antilife"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn requires_los(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(5)
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        encounter
            .actors
            .get(&caster_id)
            .is_some_and(|a| !a.is_concentrating() && a.is_combat_active())
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::side_effects::PushActor;
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let center = caster.location();
        encounter.log("  antilife shell: a shimmering barrier repels nearby creatures");
        // Push all adjacent enemies 4 tiles (10 ft) away.
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = encounter
            .enemy_burst_targets(caster_id, center, 1)
            .into_iter()
            .map(|id| {
                Box::new(PushActor {
                    actor_id: id,
                    from: center,
                    max_tiles: 4,
                }) as Box<dyn ApplicableSideEffect>
            })
            .collect();
        // Apply Warded to the caster as a melee-deterrence marker.
        effects.push(Box::new(ApplyCondition {
            actor_id: caster_id,
            condition: Condition::Warded,
            timer: ConditionTimer::Rounds(10),
        }));
        effects.push(Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData::with_conditions(
                "Antilife Shell",
                vec![(caster_id, Condition::Warded)],
            ),
        }));
        effects
    }
}

pub static ANTILIFE_SHELL: LazyLock<AntilifeShell> = LazyLock::new(|| AntilifeShell {});

/// Plane Shift — 5e level-7 conjuration, action. Touch range (1 tile).
/// The caster forces a single target to make a CHA save vs the caster's
/// spell DC. On fail the target is banished to another plane — we model
/// this with the `Mazed` condition for 100 rounds (effectively removed
/// from combat for the rest of the encounter). No concentration — once
/// the target is gone, they stay gone (RAW there is no concentration
/// requirement on the offensive use of Plane Shift).
pub struct PlaneShift {}

impl Action for PlaneShift {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Conjuration)
    }
    fn name(&self) -> &str {
        "plane shift"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["planeshift", "ps7"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // Touch = 1 tile.
        Some(1)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(7)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spell_save_dc(AbilityScoreType::Charisma);
        let save = encounter.roll_save_against_caster(target_id, AbilityScoreType::Charisma, dc, caster_id);
        if save.passed() {
            return Vec::new();
        }
        encounter.log("  plane shift: target is hurled to another plane of existence");
        vec![Box::new(ApplyCondition {
            actor_id: target_id,
            condition: Condition::Mazed,
            timer: ConditionTimer::Rounds(100),
        })]
    }
}

pub static PLANE_SHIFT: LazyLock<PlaneShift> = LazyLock::new(|| PlaneShift {});

/// Wall of Stone — level-5 evocation (wizard / sorcerer / druid),
/// concentration. RAW: the caster summons up to ten 10-ft panels of
/// stone at a point within 120 ft, forming an impassable barrier. We
/// collapse the panel geometry to the load-bearing combat hook: every
/// creature whose footprint touches the 10-ft-radius (2-tile) burst at
/// cast time makes a DEX save vs the caster's spell save DC. On fail,
/// they're caught in the rising stone — `Restrained` for 10 rounds
/// (movement zero, attack disadvantage, advantage to attackers, DEX-save
/// disadvantage). On pass, they slip clear with no penalty. The
/// restraint is anchored to the caster's concentration so dropping it
/// dissolves the wall and frees everyone caught.
///
/// Distinct from Wall of Force (no damage, no save, just shoves
/// adjacent enemies prone) and Wall of Ice (instant cold burst + prone).
/// Wall of Stone's defining feature is the lockdown — a single high-CR
/// enemy caught in the rising stone loses an entire turn while the
/// caster's allies focus-fire.
pub struct WallOfStone {}

impl Action for WallOfStone {
    fn name(&self) -> &str {
        "wall of stone"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["wostone", "stone-wall"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 2 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 120 ft RAW = 48 tiles.
        Some(48)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(5)
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // Concentration-gated install — don't re-cast while already
        // concentrating on another spell.
        encounter.caster_can_concentrate(caster_id)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.best_spell_save_dc([
            AbilityScoreType::Intelligence,
            AbilityScoreType::Wisdom,
            AbilityScoreType::Charisma,
        ]);
        encounter.log(format!(
            "  wall of stone: panels rise from the ground (DC {})",
            dc
        ));
        let targets = encounter.enemy_burst_targets(caster_id, point, 2);
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        let mut restrained: Vec<(usize, Condition)> = Vec::new();
        for tid in targets {
            // Route through the caster-aware save helper so Heightened
            // Spell metamagic forces disadvantage on the first save in
            // the burst (RAW). Subsequent targets fall through normally.
            let save = encounter.roll_save_against_caster(
                tid,
                AbilityScoreType::Dexterity,
                dc,
                caster_id,
            );
            if save.passed() {
                continue;
            }
            effects.push(Box::new(ApplyCondition {
                actor_id: tid,
                condition: Condition::Restrained,
                timer: ConditionTimer::Rounds(10),
            }));
            restrained.push((tid, Condition::Restrained));
        }
        // Anchor concentration so dropping it dissolves the wall and
        // frees everyone caught in it — mirrors the Hold Person /
        // Plant Growth concentration-prune shape.
        effects.push(Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData::with_conditions("Wall of Stone", restrained),
        }));
        effects
    }
}

pub static WALL_OF_STONE: LazyLock<WallOfStone> = LazyLock::new(|| WallOfStone {});

/// Investiture of Ice — level-6 transmutation, concentration. The caster's
/// body is sheathed in shards of ice: they gain resistance to cold damage
/// (read by `effective_damage`'s InvestedInIce branch), and every melee
/// attacker takes 1d10 cold damage in retaliation (handled by
/// `resolve_attack`'s `MELEE_REFLECT_RIDERS` table next to the
/// Investiture of Flame entry). Symmetric to Investiture of Flame: same
/// shape, swapped element. The 4d6 cold emanation rider in RAW is omitted
/// — the load-bearing buff is the resistance + melee retaliation envelope.
pub struct InvestitureOfIce {}

impl Action for InvestitureOfIce {
    fn name(&self) -> &str {
        "investiture of ice"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ioi", "iceinvest"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(6)
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // Already invested → don't re-cast and burn a level-6 slot.
        actor_lacks_condition(encounter, caster_id, Condition::InvestedInIce)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        encounter.log("  investiture of ice: your body is sheathed in shards of ice.".to_string());
        self_concentration_buff_effects(
            caster_id,
            "Investiture of Ice",
            Condition::InvestedInIce,
            ConditionTimer::Rounds(10),
        )
    }
}

pub static INVESTITURE_OF_ICE: LazyLock<InvestitureOfIce> =
    LazyLock::new(|| InvestitureOfIce {});

/// Investiture of Stone — level-6 transmutation, concentration. The caster's
/// body hardens to living rock: they gain resistance to bludgeoning,
/// piercing, and slashing damage (the three physical weapon types, read by
/// `effective_damage`'s InvestedInStone branch of `TYPED_RESISTANCE_CONDITIONS`),
/// and every melee attacker takes 1d10 force damage in retaliation (the
/// stone shell crackles with telekinetic recoil — handled by
/// `resolve_attack`'s `MELEE_REFLECT_RIDERS` table next to the
/// Investiture of Flame / Ice entries).
///
/// Distinct from `InvestedInFlame` / `InvestedInIce` in two ways:
///   - Resistance covers the *physical* trio rather than a single element,
///     so the buff is broader against martial enemies (a melee swarm) but
///     blank against any caster slinging elemental damage.
///   - Retaliation is force-typed — the rarest damage type to resist, so
///     the reflect chips through almost any creature's defenses.
///
/// The earth-tremor emanation and difficult-terrain creation in RAW are
/// omitted — the load-bearing buff is the resistance + melee-reflect
/// envelope, mirroring the Flame / Ice install shape.
pub struct InvestitureOfStone {}

impl Action for InvestitureOfStone {
    fn name(&self) -> &str {
        "investiture of stone"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ios", "stoneinvest"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(6)
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // Already invested → don't re-cast and burn a level-6 slot.
        actor_lacks_condition(encounter, caster_id, Condition::InvestedInStone)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        encounter.log("  investiture of stone: your body hardens to living rock.".to_string());
        self_concentration_buff_effects(
            caster_id,
            "Investiture of Stone",
            Condition::InvestedInStone,
            ConditionTimer::Rounds(10),
        )
    }
}

pub static INVESTITURE_OF_STONE: LazyLock<InvestitureOfStone> =
    LazyLock::new(|| InvestitureOfStone {});

/// Spider Climb — level-2 transmutation, concentration. Touches a willing
/// creature (5ft / 1 tile reach); they gain a climbing speed equal to
/// their walking speed for the duration. RAW also lets the holder traverse
/// vertical / inverted surfaces without an Athletics check.
///
/// We model the load-bearing half — the climb-speed bump — as a flat
/// +30ft via the `SpiderClimbing` condition read by `speed()`. The
/// engine doesn't model 3D terrain so the "walls and ceilings" rider
/// would be inert anyway; surfacing the bump as raw kiting speed gives
/// the spell tangible tactical value (a wounded ally can reposition
/// further than a baseline 30ft creature could).
///
/// Slots one spell-slot below `Fly` (level-3 transmutation, +60ft) — the
/// stack is intentional: a doubly-buffed creature (Spider Climb + Fly)
/// picks up both bumps for an extreme kiting envelope. Mirrors the Fly
/// install shape (single concentration-anchored condition install with
/// `Rounds(10)` timer ≈ 1 hour RAW capped to encounter horizon).
pub struct SpiderClimb {}

impl Action for SpiderClimb {
    fn name(&self) -> &str {
        "spider climb"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["scl", "climb"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // Touch range — 5ft = 1 tile.
        Some(1)
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(2)
    }
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        vec![
            Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::SpiderClimbing,
                timer: ConditionTimer::Rounds(10),
            }) as Box<dyn ApplicableSideEffect>,
            Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions(
                    "Spider Climb",
                    vec![(target_id, Condition::SpiderClimbing)],
                ),
            }),
        ]
    }
}

pub static SPIDER_CLIMB: LazyLock<SpiderClimb> = LazyLock::new(|| SpiderClimb {});

/// Tasha's Caustic Brew — level-1 evocation, concentration, action. The
/// caster splashes magical acid in a 30-ft line, 5 ft wide (RAW); we
/// collapse to a 3-tile-radius burst at a chosen point per the engine's
/// shared line→burst convention (matches Aganazzar's Scorcher's
/// line→burst shape at the same range tier).
///
/// Mechanically:
/// - Every actor in the burst makes a DEX save vs the caster's spell DC.
/// - Failed save: takes 2d4 acid immediately AND gets the `CausticBrewed`
///   condition — 2d4 acid at the end of each of their turns via the
///   shared `ROUND_END_DOTS` registry until they (a) wipe it off with an
///   action (the `WipeAcid` cleanse), (b) the caster drops concentration,
///   or (c) the spell's `Rounds(10)` ≈ 1-minute timer expires.
/// - Passed save: no damage, no condition (cantrips don't half-on-save
///   here either since the spell's RAW is save-or-nothing).
///
/// Concentration-bound on the caster RAW. The DOT lives on the standard
/// drip table so resistance / immunity / temp HP / death saves all flow
/// through the normal damage pipeline. Slots between Burning Hands (lv1
/// fire cone, no DOT) and Acid Arrow (lv2 single-target acid with splash)
/// — the sustained-DoT is the load-bearing differentiator at the lv1 tier.
pub struct TashasCausticBrew {}

impl Action for TashasCausticBrew {
    fn name(&self) -> &str {
        "tasha's caustic brew"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["tcb", "brew", "caustic"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 3 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 30 ft RAW line ≈ 12 tiles. Matches Aganazzar's Scorcher.
        Some(12)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Acid]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(1)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spellcasting_save_dc();
        // Failed save = 2d4 acid AND the target picks up the
        // `CausticBrewed` flag (DoT lives on `ROUND_END_DOTS` so the
        // per-turn drip rolls through the shared pipeline). Passed save
        // = 0 damage AND no DoT (cantrip-flavored — no half-on-save:
        // the DoT is what survivors carry forward, not a flat-damage
        // payload). Enemy-only — the caster aims the line to spare
        // allies, matching the engine's convention for line spells
        // (cf. Aganazzar's Scorcher, Burning Hands). Concentration
        // tracks every drip target so dropping concentration strips
        // every flag at once.
        concentration_burst_with_rider(
            encounter,
            caster_id,
            point,
            3,
            AbilityScoreType::Dexterity,
            dc,
            Dice::new(2, 4),
            DamageType::Acid,
            "tasha's caustic brew",
            "Tasha's Caustic Brew",
            Condition::CausticBrewed,
            ConditionTimer::Rounds(10),
            SaveDamagePolicy::NoneOnSave,
        )
    }
}

pub static TASHAS_CAUSTIC_BREW: LazyLock<TashasCausticBrew> =
    LazyLock::new(|| TashasCausticBrew {});

/// Vortex Warp — level-2 conjuration (Tasha's). The caster opens a rift
/// of swirling magic that snatches a creature within 90 ft (36 tiles) and
/// drops them at an unoccupied space the caster picks within the same
/// range. RAW: the target is automatically teleported if willing
/// (ally-cast); otherwise it makes a CON save vs the caster's spell DC,
/// and on a pass the spell fizzles for that creature.
///
/// We model the spell as a single-target teleport with the destination
/// resolved by the engine rather than the caster picking a tile:
///   - **Ally target** → snatched to a tile adjacent to the caster (the
///     classic "yank an injured ally out of a melee" play). No save.
///   - **Enemy target** → CON save vs the caster's spell DC. On fail,
///     the target is yanked next to the caster (which is *hostile*
///     ground: the caster's allies typically surround them, and the
///     target loses ground gained). On pass, the spell fizzles silently.
///
/// The "adjacent to caster" destination keeps the schema simple
/// (SingleActor — no separate destination picker needed) while
/// preserving the spell's load-bearing tactical use case: forced
/// repositioning at a 90-ft range. RAW lets the caster aim the
/// destination anywhere within 90 ft of themselves; we collapse to
/// adjacent-to-caster since that's the highest-value spot for either
/// allegiance and avoids surfacing a second targeting axis.
///
/// Slots between Misty Step (lv2, self-teleport 30 ft bonus action) and
/// Thunder Step (lv3, self + 1 willing creature, 90 ft Action) — Vortex
/// Warp is the "displace someone else" niche at the lv2 tier.
pub struct VortexWarp {}

impl Action for VortexWarp {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Conjuration)
    }
    fn name(&self) -> &str {
        "vortex warp"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["vw", "warp", "vortex"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 90 ft RAW = 36 tiles in the 2.5ft grid.
        Some(36)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn is_harmful(&self) -> bool {
        // RAW: a willing target auto-passes; an unwilling target makes a
        // save. The AI's enemy-target heuristics treat this as harmful
        // (unwilling-target use is the common case), but a friendly
        // surface in the UI mirrors how Banishment is flagged: harmful
        // for the focus-fire picker, but the spell's apply path handles
        // ally vs enemy cleanly.
        true
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(2)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::side_effects::TeleportActor;
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        if target_id == caster_id {
            // 5e RAW: the spell can't target the caster (range is "a
            // creature you can see", which excludes self by convention
            // for ally-style teleport spells).
            return Vec::new();
        }
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let caster_team = caster.team();
        let dc = caster.best_spell_save_dc([
            AbilityScoreType::Intelligence,
            AbilityScoreType::Wisdom,
            AbilityScoreType::Charisma,
        ]);
        let Some(target) = encounter.actors.get(&target_id) else {
            return Vec::new();
        };
        let target_team = target.team();
        let target_name = target.name().to_string();
        let willing = target_team == caster_team;
        // Unwilling target → CON save. On pass, the spell fizzles for
        // that creature (no teleport, no further effects). Routed through
        // the caster-aware save lane so Heightened Spell forces
        // disadvantage on the first save in the cast RAW.
        if !willing {
            let save = encounter.roll_save_against_caster(
                target_id,
                AbilityScoreType::Constitution,
                dc,
                caster_id,
            );
            if save.passed() {
                encounter.log(format!(
                    "  vortex warp: {} resists the pull.",
                    target_name
                ));
                return Vec::new();
            }
        }
        // Pick an unoccupied anchor tile next to the caster that the
        // target's full footprint can occupy. Routed through the engine
        // helper so the ring-walk + footprint-overlap math lives in one
        // place (shared with any future "yank target next to host"
        // teleport spell).
        let Some(dest) = encounter.find_adjacent_teleport_anchor(caster_id, target_id) else {
            // No legal landing tile around the caster — the spell still
            // resolves but the target stays put. RAW: "an unoccupied
            // space" — if there isn't one, the spell fails for that
            // target. Burn the slot anyway (the cast happened) but log
            // the no-op so the player understands why.
            encounter.log(format!(
                "  vortex warp: no open tile next to the caster — {} stays put.",
                target_name
            ));
            return Vec::new();
        };
        encounter.log(format!(
            "  vortex warp: {} is yanked through the rift.",
            target_name
        ));
        vec![Box::new(TeleportActor {
            actor_id: target_id,
            dest,
        })]
    }
}

pub static VORTEX_WARP: LazyLock<VortexWarp> = LazyLock::new(|| VortexWarp {});

/// Phantasmal Force — level-2 illusion (bard / sorcerer / warlock /
/// wizard), concentration. The caster crafts an illusion in the target's
/// mind; the target makes an INT save vs the caster's spell DC. On
/// fail, the illusion lands: the target picks up the `PhantasmalForced`
/// condition, taking 1d6 psychic damage at the end of each of their
/// turns (the central `ROUND_END_DOTS` registry handles the drip — see
/// the `PhantasmalForced` entry there). On save: the illusion fails
/// outright and the spell fizzles. Concentration-bound on the caster;
/// dropping concentration dispels the illusion cleanly.
///
/// Differentiated from `MindSliver` (cantrip, one-shot psychic),
/// `MindSpike` (lv2 save-for-half psychic burst), and `PhantasmalKiller`
/// (lv4 WIS save plus Frightened) by the sustained DoT: at low slot
/// tiers the per-round drip outpaces the cantrip burst over multi-round
/// fights, and the INT save (vs the more common WIS / CHA paths) hits
/// low-INT brutes harder.
pub struct PhantasmalForce {}

impl Action for PhantasmalForce {
    fn name(&self) -> &str {
        "phantasmal force"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["pf", "phantasm-force", "phantasmal"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60 ft = 24 tiles.
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Psychic]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(2)
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // Don't burn a slot to replace our own concentration. The AI's
        // focus_fire pipeline tests every harmful single-target spell
        // against `validate_input`; this gate keeps Phantasmal Force out
        // of the picker when a more valuable concentration buff is up.
        encounter.caster_can_concentrate(caster_id)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        // INT-primary casters (wizard) and CHA-primary casters (bard /
        // sorcerer / warlock) all reach for Phantasmal Force RAW. The
        // best-of resolver picks the strongest DC so a multi-class
        // caster anchors on the right stat.
        let dc = caster.best_spell_save_dc([
            AbilityScoreType::Intelligence,
            AbilityScoreType::Charisma,
        ]);
        save_or_concentration_condition(
            encounter,
            caster_id,
            target_id,
            AbilityScoreType::Intelligence,
            dc,
            "Phantasmal Force",
            Condition::PhantasmalForced,
            // 10 rounds = 1 minute RAW. Concentration anchors the
            // real lifetime — dropping concentration ends the
            // illusion before the timer expires.
            ConditionTimer::Rounds(10),
            "  phantasmal force: target sees through the illusion.",
            "  phantasmal force: the illusion takes hold.",
        )
    }
}

pub static PHANTASMAL_FORCE: LazyLock<PhantasmalForce> = LazyLock::new(|| PhantasmalForce {});

/// Wall of Light — level-5 evocation (sorcerer / warlock / wizard),
/// concentration. The caster summons a wall of radiant light. We collapse
/// the 60ft line / 10ft tall wall to the load-bearing combat hook: every
/// enemy whose footprint sits within `radius` of the burst point takes
/// 4d8 radiant on a failed CON save (half on success). Failed-save
/// targets are also Blinded for the duration as the brilliance sears
/// their eyes — the load-bearing crowd-control rider. Concentration
/// anchors the Blinded marks so dropping concentration ends every blind
/// at once via the standard concentration-cleanup path.
///
/// Differentiated from `Sunbeam` (lv6 evocation, line of sun) by the
/// blind rider; differentiated from `Dawn` (lv5 cleric, neutral burst)
/// by the enemy-only partition and the blind-on-fail clause.
pub struct WallOfLight {}

impl Action for WallOfLight {
    fn name(&self) -> &str {
        "wall of light"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["wol", "light-wall"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 2 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 120 ft RAW = 48 tiles.
        Some(48)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Radiant]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(5)
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        encounter.caster_can_concentrate(caster_id)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.best_spell_save_dc([
            AbilityScoreType::Intelligence,
            AbilityScoreType::Charisma,
        ]);
        // RAW: 4d8 radiant CON-save half + Blinded-on-fail
        // (concentration). The recipe — burst save for half, then
        // anchor the rider on the caster's concentration — is shared
        // with Caustic Brew / Black Tentacles / Psychic Scream, so we
        // route through the canonical `concentration_burst_with_rider`
        // chokepoint. Blinded is read until the wall ends OR until the
        // target spends an action to wipe their eyes — we collapse to
        // "blinded for the duration" since the engine has no per-target
        // action-economy cleanse outside the StillnessOfMind cohort.
        concentration_burst_with_rider(
            encounter,
            caster_id,
            point,
            2,
            AbilityScoreType::Constitution,
            dc,
            Dice::new(4, 8),
            DamageType::Radiant,
            "wall of light",
            "Wall of Light",
            Condition::Blinded,
            ConditionTimer::Rounds(10),
            SaveDamagePolicy::HalfOnSave,
        )
    }
}

pub static WALL_OF_LIGHT: LazyLock<WallOfLight> = LazyLock::new(|| WallOfLight {});

/// Watery Sphere — level-4 conjuration (XGtE, druid / sorcerer / warlock /
/// wizard), concentration. The caster conjures a 5ft sphere of water and
/// catches a single creature inside. Target makes a STR save vs the
/// caster's spell DC. On fail, they're encased in the sphere — the
/// `WaterSphered` condition collapses the Restrained envelope (zero
/// movement, attack disadvantage, attacks against have advantage) with
/// the Lifted clause (suspended above the ground). On save: the spell
/// fizzles. Concentration-bound on the caster; dropping concentration
/// bursts the sphere cleanly via the standard concentration-cleanup path.
///
/// Distinct from `Levitate` (Lifted only, CON save) and `OtilukesResilientSphere`
/// (DEX save, full incapacitation): Watery Sphere splits the difference —
/// a STR-save trap with the Restrained envelope rather than the full
/// action-economy lockout, slotting cleanly between the two on the
/// single-target control ladder.
pub struct WaterySphere {}

impl Action for WaterySphere {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Conjuration)
    }
    fn name(&self) -> &str {
        "watery sphere"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ws", "water-sphere"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 90 ft RAW = 36 tiles.
        Some(36)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(4)
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        encounter.caster_can_concentrate(caster_id)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        // Druid (WIS), sorcerer / warlock (CHA), wizard (INT) all reach
        // for Watery Sphere RAW. Best-of routes through the helper so a
        // multi-class caster anchors on the right stat.
        let dc = caster.best_spell_save_dc([
            AbilityScoreType::Intelligence,
            AbilityScoreType::Wisdom,
            AbilityScoreType::Charisma,
        ]);
        save_or_concentration_condition(
            encounter,
            caster_id,
            target_id,
            AbilityScoreType::Strength,
            dc,
            "Watery Sphere",
            Condition::WaterSphered,
            // 10 rounds = 1 minute RAW. Concentration anchors the
            // real lifetime.
            ConditionTimer::Rounds(10),
            "  watery sphere: target tears free of the water.",
            "  watery sphere: target is caught inside the sphere.",
        )
    }
}

pub static WATERY_SPHERE: LazyLock<WaterySphere> = LazyLock::new(|| WaterySphere {});

/// Investiture of Wind — level-6 transmutation (XGtE; sorcerer / warlock /
/// wizard / druid), concentration. The caster is wrapped in a swirling
/// vortex of air: ranged attacks against them have disadvantage (the
/// `InvestedInWind` condition joins `imposes_disadvantage_to_ranged_attackers`
/// next to Wind Wall) and they hover above the ground (joins the Flying
/// cohort via `remaining_speed`'s `InvestedInWind` branch).
///
/// Symmetric to Investiture of Flame / Ice / Stone — same self-only
/// concentration-bound install shape, but a different defensive envelope:
/// ranged deflection plus flight instead of damage resistance + melee
/// retaliation. The RAW Gust-of-Wind action and the per-turn 3d10
/// bludgeoning radial burst are omitted — the load-bearing combat clauses
/// are the ranged deflection and the +60ft flying speed.
pub struct InvestitureOfWind {}

impl Action for InvestitureOfWind {
    fn name(&self) -> &str {
        "investiture of wind"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["iow", "windinvest"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(6)
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // Already invested → don't re-cast and burn a level-6 slot.
        actor_lacks_condition(encounter, caster_id, Condition::InvestedInWind)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        encounter
            .log("  investiture of wind: a vortex of air whips around you.".to_string());
        self_concentration_buff_effects(
            caster_id,
            "Investiture of Wind",
            Condition::InvestedInWind,
            ConditionTimer::Rounds(10),
        )
    }
}

pub static INVESTITURE_OF_WIND: LazyLock<InvestitureOfWind> =
    LazyLock::new(|| InvestitureOfWind {});

/// Otiluke's Freezing Sphere — level-6 evocation (PHB, sorcerer / wizard).
/// A globe of frigid energy explodes at a target point within 300 ft.
/// Every creature in a 60-ft-radius sphere makes a CON save vs the
/// caster's spell DC: pass = half, fail = full. 10d6 cold damage.
///
/// Slots between Cone of Cold (lv5, 8d8) and Sunburst (lv8, 12d6) on
/// the AoE blasting ladder — same shared-roll save-for-half shape, but
/// with a cold typing that pairs cleanly with Investiture of Ice and
/// the Cold Sorcerer's elemental affinity. Empowered Spell metamagic
/// applies to the shared roll via the same `roll_empowered_sum` hook
/// that Cone of Cold / Fireball / Sunburst use.
pub struct OtilukesFreezingSphere {}

impl Action for OtilukesFreezingSphere {
    fn name(&self) -> &str {
        "freezing sphere"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ofs", "otiluke", "sphere"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        // 60ft-radius sphere → burst radius 6 on this 2.5ft grid.
        // Matches Cone of Cold's footprint at the same blast tier.
        TargetingSchema::Burst { radius: 6 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 300 ft RAW = 120 tiles, but the map is far smaller. Cap at
        // 40 (matches Sunburst's pragmatic cap) so the picker still
        // covers the full board without dangling pins off the map.
        Some(40)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Cold]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(6)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        // Sorcerer (CHA) + wizard (INT) — best-of so a multi-class caster
        // anchors on the right stat. Same shape as Chromatic Orb's
        // INT/CHA gate.
        let dc = caster.best_spell_save_dc([
            AbilityScoreType::Intelligence,
            AbilityScoreType::Charisma,
        ]);
        // Empowered Spell metamagic — the 10d6 shared pool is the single
        // roll the sorcerer's CHA-mod reroll applies to. Mirrors the
        // exact hook used by Cone of Cold / Fireball / Sunburst.
        let raw = encounter.roll_empowered_sum(caster_id, 10, 6);
        encounter.log(format!(
            "  freezing sphere: 10d6({}) = {} cold area",
            raw, raw
        ));
        crate::actions::action_template::resolve_burst_save_damage(
            encounter,
            caster_id,
            point,
            6,
            AbilityScoreType::Constitution,
            dc,
            raw,
            DamageType::Cold,
        )
    }
}

pub static OTILUKES_FREEZING_SPHERE: LazyLock<OtilukesFreezingSphere> =
    LazyLock::new(|| OtilukesFreezingSphere {});

/// Maelstrom — level-5 evocation (XGtE, druid / sorcerer / wizard /
/// warlock). A swirling vortex of water roars at a target point within
/// 120 ft. Every creature in a 30-ft (3-tile) burst makes a STR save vs
/// the caster's spell DC: pass = half, fail = full. 6d6 bludgeoning
/// damage. Failed-save targets are *pulled* 10 ft (4 tiles) toward the
/// center of the vortex — same `PullActor` hook that Thorn Whip's catch
/// uses, so wall / occupancy blocking is honored cleanly.
///
/// Distinct from Tidal Wave (lv3, 4d8 DEX-save bludgeoning + Prone) at a
/// higher slot tier: STR save instead of DEX (heavy hitters fare worse),
/// pull rider replaces the prone rider so the AoE bunches enemies for a
/// follow-up Fireball / Cone of Cold instead of laying them flat.
pub struct Maelstrom {}

impl Action for Maelstrom {
    fn name(&self) -> &str {
        "maelstrom"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["mael", "vortex"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        // 30 ft radius circle → 3-tile burst on this grid (matches Tidal Wave).
        TargetingSchema::Burst { radius: 3 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 120 ft RAW = 48 tiles.
        Some(48)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Bludgeoning]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(5)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::side_effects::PullActor;

        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        // Druid (WIS), sorcerer / warlock (CHA), wizard (INT) all reach
        // for Maelstrom RAW — same best-of triplet as Watery Sphere.
        let dc = caster.best_spell_save_dc([
            AbilityScoreType::Intelligence,
            AbilityScoreType::Wisdom,
            AbilityScoreType::Charisma,
        ]);
        let (mut effects, saves) = enemy_burst_save_for_half(
            encounter,
            caster_id,
            point,
            3,
            AbilityScoreType::Strength,
            dc,
            Dice::new(6, 6),
            DamageType::Bludgeoning,
            "maelstrom",
        );
        // 5e RAW: failed-save targets are dragged 10 ft toward the
        // center of the vortex. PullActor stops cleanly on walls /
        // occupied tiles — a target already pinned doesn't budge.
        const PULL_TILES: u32 = 4;
        for (tid, passed) in saves {
            if !passed {
                effects.push(Box::new(PullActor {
                    actor_id: tid,
                    toward: point,
                    max_tiles: PULL_TILES,
                }));
            }
        }
        effects
    }
}

pub static MAELSTROM: LazyLock<Maelstrom> = LazyLock::new(|| Maelstrom {});

/// Dust Devil — level-2 conjuration (XGtE, druid / sorcerer / wizard /
/// warlock). The caster summons a 5-ft cube of swirling air at a target
/// tile within 60 ft. Every creature within 10 ft (1 tile) of the cube
/// makes a STR save vs the caster's spell DC: pass = half, fail = full.
/// 1d8 bludgeoning damage. Failed-save targets are *pushed* 10 ft (4
/// tiles) away from the dust devil — same `PushActor` hook Thunderwave's
/// push uses, so wall / occupancy blocking is honored cleanly.
///
/// RAW lets the caster sustain the dust devil for up to 1 minute with
/// concentration, moving it as a bonus action so it can buffet additional
/// creatures on subsequent turns. We collapse the sustained sweep into a
/// one-shot burst at cast time (matches our Dawn / Sickening Radiance
/// simplification): the lv2 slot lands a single small-burst stagger and
/// the persistent-field upkeep falls out cleanly. The push lane fills
/// the lv2 displacement niche between Thunderwave (lv1 caster-centered
/// 4-tile push, CON save) and Maelstrom (lv5 pull-into-center, STR save).
pub struct DustDevil {}

impl Action for DustDevil {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Conjuration)
    }
    fn name(&self) -> &str {
        "dust devil"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["dd", "devil", "dust"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        // 10ft sweep → 1-tile burst around the summon point. Mirrors
        // Snilloc's Snowball Swarm's small footprint at the lv2 tier.
        TargetingSchema::Burst { radius: 1 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60 ft RAW = 24 tiles.
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Bludgeoning]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(2)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::side_effects::PushActor;

        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        // Druid (WIS), sorcerer / warlock (CHA), wizard (INT) all reach
        // for Dust Devil RAW — same best-of triplet as Watery Sphere /
        // Maelstrom for consistent multi-class handling.
        let dc = caster.best_spell_save_dc([
            AbilityScoreType::Intelligence,
            AbilityScoreType::Wisdom,
            AbilityScoreType::Charisma,
        ]);
        let (mut effects, saves) = enemy_burst_save_for_half(
            encounter,
            caster_id,
            point,
            1,
            AbilityScoreType::Strength,
            dc,
            Dice::new(1, 8),
            DamageType::Bludgeoning,
            "dust devil",
        );
        // 5e RAW push: 10 ft away from the dust devil's tile on a failed
        // save. PushActor halts cleanly on walls / occupied tiles so a
        // target pinned to the wall takes the damage but doesn't budge.
        const PUSH_TILES: u32 = 4;
        for (tid, passed) in saves {
            if !passed {
                effects.push(Box::new(PushActor {
                    actor_id: tid,
                    from: point,
                    max_tiles: PUSH_TILES,
                }));
            }
        }
        effects
    }
}

pub static DUST_DEVIL: LazyLock<DustDevil> = LazyLock::new(|| DustDevil {});

/// Flesh to Stone — level-6 transmutation (warlock / wizard), concentration.
/// The target makes a CON save vs the caster's spell DC. On pass: nothing.
/// On fail: target is Petrified for `Rounds(10)` (~1 minute RAW) and the
/// caster takes concentration anchored to the petrify mark. The target
/// makes a fresh CON save at the end of each of their turns via the
/// shared `ROUND_END_SAVES` table — on a pass the curse breaks and the
/// caster's concentration drops, mirroring how Hold Person / Hold Monster
/// route through the same hook.
///
/// RAW's graduated "3 saves vs 3 fails" ladder is collapsed to a single
/// break-free path; the load-bearing combat clause is the Petrified
/// envelope (zero movement, action-economy block, attacks have advantage,
/// auto-fail STR/DEX saves, broad damage resistance / poison immunity).
/// Slots between Hold Monster (lv5, WIS save, Paralyzed) and Eyebite (lv6,
/// WIS save, Asleep) on the single-target lockdown ladder — the CON save
/// hits low-CON casters and brutes that shrug off WIS effects.
pub struct FleshToStone {}

impl Action for FleshToStone {
    fn name(&self) -> &str {
        "flesh to stone"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["fts", "petrify", "stone"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60 ft RAW = 24 tiles.
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(6)
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        encounter.caster_can_concentrate(caster_id)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        // Warlock (CHA) + wizard (INT) — best-of so a multi-class caster
        // anchors on the right stat. Mirrors the Hold Monster / Watery
        // Sphere best-of pattern.
        let dc = caster.best_spell_save_dc([
            AbilityScoreType::Intelligence,
            AbilityScoreType::Charisma,
        ]);
        save_or_concentration_condition(
            encounter,
            caster_id,
            target_id,
            AbilityScoreType::Constitution,
            dc,
            "Flesh to Stone",
            Condition::Petrified,
            // 10 rounds = 1 minute RAW. Concentration anchors the real
            // lifetime; the round-end save can also break it early.
            ConditionTimer::Rounds(10),
            "  flesh to stone: target shrugs off the curse.",
            "  flesh to stone: target's flesh hardens to stone.",
        )
    }
}

pub static FLESH_TO_STONE: LazyLock<FleshToStone> = LazyLock::new(|| FleshToStone {});

/// Psychic Scream — level-9 enchantment (bard / sorcerer / warlock /
/// wizard), action. The caster unleashes a telepathic howl from their
/// location: every combat-active enemy whose footprint sits within
/// `radius` of the caster's tile makes an INT save vs the caster's
/// spell DC. Pass: 7d6 psychic (half damage RAW). Fail: full 14d6
/// psychic plus Stunned for `Rounds(10)`. The Stunned target gets a
/// WIS save at the end of each of their turns via the shared
/// `ROUND_END_SAVES` table (Psychic Scream RAW uses INT save at end
/// of turn; we collapse to the Hold Monster WIS-save lane since the
/// engine's repeated-save table doesn't fork per-source).
///
/// Self-centered NoArgs target (the caster broadcasts the scream from
/// their tile, no aim). Distinct from `PowerWordStun` (lv8, single-target
/// HP-gated stun) by the mass burst clause and the damage; distinct from
/// `Weird` (lv9, single-point WIS-save burst with Frightened) by the
/// damage type, save ability, and Stunned vs Frightened rider. The
/// flagship lv9 mind-spike for casters who want to crush the entire
/// hostile back rank in one tap. Concentration-free RAW — burst-and-done.
pub struct PsychicScream {}

impl Action for PsychicScream {
    fn name(&self) -> &str {
        "psychic scream"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ps", "scream", "psy-scream"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        // Self-centered burst — no target tile, no actor pick. The
        // caster's own location is the center. Same shape as
        // Thunderclap / Sword Burst at the cantrip tier, but scaled
        // up to lv9 radius / damage.
        TargetingSchema::NoArgs
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Psychic]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(9)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::actors::actor_template::ConcentrationData;
        // 90 ft radius RAW = 8 tiles on this 2.5ft grid (rounded down
        // from 9; the encounter map's diagonal is ~30 tiles so 8 still
        // sweeps most of a clustered enemy back rank).
        const RADIUS: isize = 8;
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let center = caster.location();
        // Bard / sorcerer / warlock (CHA) and wizard (INT) all reach for
        // Psychic Scream RAW — best-of so a multi-class caster anchors on
        // the right stat.
        let dc = caster.best_spell_save_dc([
            AbilityScoreType::Intelligence,
            AbilityScoreType::Charisma,
        ]);
        let (mut effects, saves) = enemy_burst_save_for_half(
            encounter,
            caster_id,
            center,
            RADIUS,
            AbilityScoreType::Intelligence,
            dc,
            // 14d6 psychic shared roll; halved on save via the standard
            // SaveDamagePolicy::HalfOnSave lane that the helper picks.
            Dice::new(14, 6),
            DamageType::Psychic,
            "psychic scream",
        );
        // RAW: failed-save targets are also Stunned for the duration.
        // The condition is concentration-FREE (no anchor caster) but we
        // still want the round-end WIS-save break-free hook, which the
        // shared `ROUND_END_SAVES` table fires only when a concentration
        // owner exists. To get the break-free hook we anchor a dummy
        // concentration so the target can shake out — keeps the cast
        // self-mitigating without needing per-source forking in the
        // round-end table. The concentration is dropped naturally when
        // every Stunned target breaks free or the timer expires; if the
        // caster was already concentrating on something else, that
        // concentration is replaced (5e RAW: only one concentration at
        // a time).
        let conditions = push_condition_on_failed_save_for_concentration(
            &mut effects,
            &saves,
            Condition::Stunned,
            ConditionTimer::Rounds(10),
        );
        if !conditions.is_empty() {
            effects.push(Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions("Psychic Scream", conditions),
            }));
        }
        effects
    }
}

pub static PSYCHIC_SCREAM: LazyLock<PsychicScream> = LazyLock::new(|| PsychicScream {});

/// Bones of the Earth — level-6 transmutation (druid, XGtE), action.
/// Six 5-ft-thick pillars of stone erupt from the ground in a 2-tile
/// burst at a target point within 120ft. Every creature in the burst
/// makes a DEX save vs the caster's WIS-based spell DC: pass = half,
/// fail = full. 6d6 bludgeoning damage. The pillars themselves are
/// not modeled (the engine has no terrain-mutation lane for
/// per-tile pillars), but the load-bearing combat clause — the
/// erupting damage and the "pinned between pillars" prone rider on
/// failed saves — lands cleanly through the existing burst + Prone
/// shape (Tidal Wave / Wall of Stone use the same install).
///
/// Slots between Sleet Storm (lv3, control / cover) and Earthquake
/// (lv8, AoE Prone + difficult terrain) on the druid's earth-themed
/// control ladder. The Prone rider on fail mirrors Wall of Stone — a
/// big enemy line gets laid flat for a Spike Growth / Spirit Guardians
/// follow-up. Concentration-free RAW (the pillars are physical objects
/// that stand on their own); we model as a one-shot burst.
pub struct BonesOfTheEarth {}

impl Action for BonesOfTheEarth {
    fn name(&self) -> &str {
        "bones of the earth"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["bones", "pillars", "bote"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        // 5ft pillars in a 2-tile gap → small burst footprint. Same
        // size as Maximilian's Earthen Grasp / Catapult.
        TargetingSchema::Burst { radius: 2 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 120 ft RAW = 48 tiles.
        Some(48)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Bludgeoning]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(6)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        // Druid (WIS) — single stat for the spell DC; mirrors Sleet
        // Storm / Spike Growth on the druid's earth-themed control lane.
        let dc = caster.spell_save_dc(AbilityScoreType::Wisdom);
        let (mut effects, saves) = enemy_burst_save_for_half(
            encounter,
            caster_id,
            point,
            2,
            AbilityScoreType::Dexterity,
            dc,
            Dice::new(6, 6),
            DamageType::Bludgeoning,
            "bones of the earth",
        );
        // RAW rider: failed-save targets are pinned between rising
        // pillars — we collapse to Prone via the existing AoE Prone
        // rider table (same shape Wall of Stone / Tidal Wave use).
        push_condition_on_failed_save(
            &mut effects,
            &saves,
            Condition::Prone,
            // Prone is removed by spending half movement to stand up;
            // the timer is a safety net so the rider doesn't dangle.
            ConditionTimer::Rounds(10),
        );
        effects
    }
}

pub static BONES_OF_THE_EARTH: LazyLock<BonesOfTheEarth> =
    LazyLock::new(|| BonesOfTheEarth {});

/// Storm Sphere — level-4 evocation (sorcerer / wizard, XGtE), action,
/// concentration. The caster summons a sphere of crackling storm clouds
/// at a target point within 150ft. Every creature whose footprint sits
/// in the 4-tile (20ft) burst makes a STR save vs the caster's spell
/// DC: pass = no damage, fail = 2d6 bludgeoning from the lashing winds
/// AND the target picks up the `WindBlasted` rider for the duration —
/// the storm's residual gusts impose disadvantage on the target's
/// ranged attacks. Concentration tracks the failed-save cohort so
/// dropping concentration strips every install at once.
///
/// RAW's "every creature that enters the sphere or starts its turn
/// there" sustained-zone clause is collapsed to a one-shot install at
/// cast time (matches our Dawn / Sickening Radiance simplification);
/// the bonus-action lightning-strike clause RAW grants is omitted (the
/// engine has no separate action-economy lane for a concentration-
/// driven follow-up attack). Slots between Ice Storm (lv4, DEX-save
/// half damage, no rider) and Wall of Light (lv5, blind on fail) on
/// the AoE-control ladder — distinct from Ice Storm by the rider,
/// distinct from Wall of Light by the save ability (STR vs CON) and
/// the smaller burst size.
pub struct StormSphere {}

impl Action for StormSphere {
    fn name(&self) -> &str {
        "storm sphere"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["sphere", "storm", "ss"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        // 20ft radius sphere → 4-tile burst on this grid.
        TargetingSchema::Burst { radius: 4 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 150 ft RAW = 60 tiles; cap at 48 (matches Bones of the Earth's
        // pragmatic cap) so the picker still covers the full board
        // without dangling pins off the map edge.
        Some(48)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Bludgeoning]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(4)
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        encounter.caster_can_concentrate(caster_id)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        // Sorcerer (CHA) + wizard (INT) — best-of so a multi-class caster
        // anchors on the right stat. Same shape as Otiluke's Freezing
        // Sphere's INT/CHA gate at the same lv4 evocation tier.
        let dc = caster.best_spell_save_dc([
            AbilityScoreType::Intelligence,
            AbilityScoreType::Charisma,
        ]);
        // Cantrip-flavored "no damage on save" outcome — RAW: a passed
        // STR save lets the target dodge out of the sphere entirely.
        // The `WindBlasted` rider only lands on failed saves; the same
        // concentration mark anchors the cohort.
        concentration_burst_with_rider(
            encounter,
            caster_id,
            point,
            4,
            AbilityScoreType::Strength,
            dc,
            Dice::new(2, 6),
            DamageType::Bludgeoning,
            "storm sphere",
            "Storm Sphere",
            Condition::WindBlasted,
            // 10 rounds = 1 minute RAW. Concentration anchors the real
            // lifetime — dropping concentration ends the storm before
            // the timer expires.
            ConditionTimer::Rounds(10),
            SaveDamagePolicy::NoneOnSave,
        )
    }
}

pub static STORM_SPHERE: LazyLock<StormSphere> = LazyLock::new(|| StormSphere {});

/// Maddening Darkness — level-8 evocation (warlock / wizard, XGtE),
/// action, concentration. The caster conjures a 60-ft sphere of magical
/// darkness at a target point within 150 ft. Every enemy whose
/// footprint sits in the 6-tile burst makes a WIS save vs the caster's
/// spell DC: pass = half damage (RAW), fail = full. 8d8 psychic damage
/// from the gibbering whispers in the dark. Concentration-bound — RAW
/// the sphere lingers up to 10 minutes, and any creature entering /
/// starting its turn in the area re-rolls the save. We collapse the
/// sustained zone to a one-shot install at cast time, matching the
/// Sickening Radiance / Sleet Storm / Dawn shape used elsewhere.
///
/// Slots between Sunburst (lv8, radiant DEX-save AoE) and Psychic
/// Scream (lv9, INT-save psychic burst + Stunned) on the lv8+
/// caster-burst ladder. Distinct from Power Word Stun (lv8, HP-gated
/// single-target stun) by being a multi-target AoE; distinct from
/// Sunburst by the damage type and save ability (WIS vs DEX). The
/// flagship lv8 mind-spike for warlocks who can't reach Psychic
/// Scream's lv9 slot.
pub struct MaddeningDarkness {}

impl Action for MaddeningDarkness {
    fn name(&self) -> &str {
        "maddening darkness"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["maddening", "darkness", "md"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        // 60ft radius sphere → 6-tile burst on this grid (matches
        // Otiluke's Freezing Sphere's footprint at a higher tier).
        TargetingSchema::Burst { radius: 6 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 150 ft RAW = 60 tiles; cap at 48 (mirrors Bones of the Earth)
        // so the picker covers the full board.
        Some(48)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Psychic]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(8)
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        encounter.caster_can_concentrate(caster_id)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::actors::actor_template::ConcentrationData;

        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        // Warlock (CHA) + wizard (INT) — best-of so a multi-class caster
        // anchors on the right stat. Mirrors Flesh to Stone / Otiluke's
        // Freezing Sphere on the wizard / warlock list.
        let dc = caster.best_spell_save_dc([
            AbilityScoreType::Intelligence,
            AbilityScoreType::Charisma,
        ]);
        let (mut effects, _saves) = enemy_burst_save_for_half(
            encounter,
            caster_id,
            point,
            6,
            AbilityScoreType::Wisdom,
            dc,
            // 8d8 psychic shared roll; halved on save via the standard
            // SaveDamagePolicy::HalfOnSave lane the helper picks.
            Dice::new(8, 8),
            DamageType::Psychic,
            "maddening darkness",
        );
        // RAW: the sphere lingers under concentration; while no per-
        // target rider rides into `effects` (no condition tag), the
        // concentration anchor is still useful — re-cast / drop
        // concentration ends the darkness cleanly and the metamagic
        // / dispel paths can prune the bare mark via the existing
        // concentration cleanup hook. Bare mark (no conditions to
        // strip) — matches the Crusader's Mantle / Mordenkainen's
        // Sword shape.
        effects.push(Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData::new("Maddening Darkness"),
        }));
        effects
    }
}

pub static MADDENING_DARKNESS: LazyLock<MaddeningDarkness> =
    LazyLock::new(|| MaddeningDarkness {});

/// Geas — level-5 enchantment (bard / cleric / druid / paladin /
/// wizard), action. The caster levels a magical command at a target
/// within 60 ft. The target makes a WIS save vs the caster's spell DC:
/// pass = nothing, fail = the target is Charmed by the caster for the
/// duration (and the engine's existing Charmed-on-actor enforcement
/// blocks them from making hostile actions against the caster — RAW's
/// "must obey the spoken command" clause).
///
/// RAW's 30-day duration is capped to `Rounds(100)` (~10 minutes of
/// combat) since the engine has no rest mechanic to amortize the
/// month-long timer; concentration-FREE per RAW. Routes through the
/// `save_or_concentration_condition` shape but *without* the
/// concentration anchor — Geas is the rare long-duration Charmed
/// install that doesn't burn the caster's concentration. We open-code
/// the two-step `if save.passed() { ... } else { ApplyCondition }`
/// branch instead of pulling the concentration helper.
///
/// Slots between Charm Person (lv1, short-duration WIS-save Charmed)
/// and Charm Monster (lv4, lifted CR cap) on the single-target
/// charm ladder — Geas trades the long duration for the requirement
/// that the target understand the caster's language (we omit the
/// language gate since the engine doesn't enforce per-target language
/// awareness). Distinct from Dominate Person / Monster which add the
/// blanket attack disadvantage clause.
pub struct Geas {}

impl Action for Geas {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Enchantment)
    }
    fn name(&self) -> &str {
        "geas"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["compel", "command", "g"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60 ft RAW = 24 tiles.
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(5)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        // Multi-class casters (bard / cleric / druid / paladin / wizard)
        // anchor on different stats — best-of picks the right one.
        let dc = caster.best_spell_save_dc([
            AbilityScoreType::Intelligence,
            AbilityScoreType::Wisdom,
            AbilityScoreType::Charisma,
        ]);
        let save = encounter.roll_save_against_caster(
            target_id,
            AbilityScoreType::Wisdom,
            dc,
            caster_id,
        );
        if save.passed() {
            encounter.log("  geas: target shrugs off the command.".to_string());
            return Vec::new();
        }
        encounter.log("  geas: target is bound to obey the caster.".to_string());
        // ~10 minutes of combat — long enough to cover any realistic
        // encounter span without sitting truly permanent. RAW's 30-day
        // timer would be effectively permanent in any combat session.
        // Shares the install helper with Charm Person / Charm Monster
        // so the `Charmed` flag + `Charmed` back-link stay in lockstep.
        install_charmed_by(target_id, caster_id, ConditionTimer::Rounds(100))
    }
}

pub static GEAS: LazyLock<Geas> = LazyLock::new(|| Geas {});

/// Wall of Sand — level-3 evocation (wizard, XGtE), action, concentration.
/// The caster summons a 30ft-wide, 10ft-tall wall of swirling sand at a
/// target point within 90ft. Every enemy whose footprint sits in the
/// 3-tile burst makes a STR save vs the caster's INT-based spell DC:
/// pass = no effect, fail = the target is `Restrained` for the duration
/// (the sand pins them in place). Concentration anchors the Restrained
/// cohort so dropping concentration strips every install at once via the
/// standard concentration-cleanup path.
///
/// RAW's "wall is otherwise impenetrable to vision, including darkvision"
/// clause is not modeled (the engine has no per-wall LOS gate for spell-
/// summoned terrain) — the load-bearing combat clause is the on-cast
/// Restrained install. Slots between Web (lv2 DEX-save Restrained burst,
/// concentration) and Black Tentacles (lv4 DEX-save 3d6 + Restrained burst,
/// concentration) on the wizard's restraint ladder — distinct from Web
/// by the STR-save lane (resists STR-heavy enemies less effectively but
/// punishes DEX builds), distinct from Black Tentacles by the lack of
/// damage rider and the smaller slot cost.
pub struct WallOfSand {}

impl Action for WallOfSand {
    fn name(&self) -> &str {
        "wall of sand"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["sand", "wos", "sand-wall"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        // 30ft wide wall → 3-tile burst on this grid. Sits between Wall
        // of Light's 2-tile and Maddening Darkness's 6-tile bursts.
        TargetingSchema::Burst { radius: 3 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 90 ft RAW = 36 tiles.
        Some(36)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(3)
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        encounter.caster_can_concentrate(caster_id)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        // Wizard (INT) — single stat for the spell DC; the spell is on
        // the wizard list RAW so the multi-class best-of isn't needed.
        let dc = caster.spell_save_dc(AbilityScoreType::Intelligence);
        concentration_burst_condition_only(
            encounter,
            caster_id,
            point,
            3,
            AbilityScoreType::Strength,
            dc,
            "wall of sand",
            "Wall of Sand",
            Condition::Restrained,
            // 10 rounds = 1 minute RAW. Concentration anchors the real
            // lifetime — dropping concentration ends the wall before the
            // timer expires.
            ConditionTimer::Rounds(10),
        )
    }
}

pub static WALL_OF_SAND: LazyLock<WallOfSand> = LazyLock::new(|| WallOfSand {});

/// Wall of Water — level-3 evocation (druid / sorcerer / wizard, XGtE),
/// action, concentration. The caster conjures a 30ft-long, 10ft-tall wall
/// of rushing water at a target point within 60ft. Every enemy whose
/// footprint sits in the 3-tile burst picks up the `WindWalled` rider for
/// the duration — ranged attacks against them have disadvantage as arrows
/// and bolts splash off the water curtain. No save (RAW: the wall just
/// appears — the protective effect is automatic for anyone behind it).
/// Concentration anchors the cohort so dropping concentration ends the
/// wall and strips every flag at once via the standard concentration-
/// cleanup path.
///
/// RAW's "fire damage halved through the wall" and "cold damage can freeze
/// it into Wall of Ice" clauses are not modeled — the engine has no per-
/// wall damage filter and the freeze interaction would need cross-spell
/// bookkeeping. The load-bearing combat clause is the ranged-disadvantage
/// rider, which lets the caster's frontliners shrug off arrow / bolt
/// barrages while the wall holds. Slots alongside Wind Wall (lv3
/// transmutation, concentration) on the ranged-deflection ladder —
/// distinct from Wind Wall by the burst footprint (multi-target install
/// vs self-only on Wind Wall) and the lack of any push / breath-weapon
/// clause.
pub struct WallOfWater {}

impl Action for WallOfWater {
    fn name(&self) -> &str {
        "wall of water"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["water", "wow", "water-wall"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        // 30ft wide wall → 3-tile burst on this grid (matches Wall of
        // Sand's footprint at the same lv3 tier).
        TargetingSchema::Burst { radius: 3 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60 ft RAW = 24 tiles.
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn is_harmful(&self) -> bool {
        // Wall of Water is defensively flavored — the wall imposes
        // disadvantage on ranged attacks made *through* it, protecting
        // whoever sits behind. The AI's heuristic for harmful spells
        // picks targets via enemy proximity; for a wall that protects
        // its own targets (allies in the burst), the support pipeline
        // is the right lane, so we mark it non-harmful.
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(3)
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        encounter.caster_can_concentrate(caster_id)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        // No save / no damage roll — the wall's ranged-disadvantage rider
        // is automatic for any creature caught in its footprint. We use
        // `neutral_burst_targets` so the wall protects both teams equally
        // (RAW: the wall doesn't discriminate — anyone behind it benefits
        // from ranged disadvantage). The caster picks the burst point to
        // anchor the wall between their front line and the enemy ranged
        // back rank; allies caught in the area pick up `WindWalled` as a
        // protective buff, enemies caught in the area pick it up as
        // collateral (the same wall protects them when allies shoot back).
        // Open-coded loop (rather than the concentration-burst helper)
        // because there's no save / no damage — every target picks up the
        // condition unconditionally.
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        let mut conditions: Vec<(usize, Condition)> = Vec::new();
        for tid in encounter.neutral_burst_targets(caster_id, point, 3) {
            effects.push(Box::new(ApplyCondition {
                actor_id: tid,
                condition: Condition::WindWalled,
                timer: ConditionTimer::Rounds(10),
            }));
            conditions.push((tid, Condition::WindWalled));
        }
        effects.push(Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData::with_conditions("Wall of Water", conditions),
        }));
        effects
    }
}

pub static WALL_OF_WATER: LazyLock<WallOfWater> = LazyLock::new(|| WallOfWater {});

/// Compulsion — level-4 enchantment (bard, PHB), action, concentration.
/// The caster radiates a magical compulsion in a 30ft burst centered on
/// themselves. Every enemy whose footprint sits in the 12-tile burst makes
/// a WIS save vs the caster's CHA-based spell DC: pass = no effect, fail =
/// the target is `Charmed` by the caster for the duration (and the
/// engine's existing Charmed-on-actor gate blocks them from making hostile
/// actions against the caster). Concentration anchors the cohort so
/// dropping concentration strips every Charmed flag (and its `Charmed` back-link
/// link) at once via the standard concentration-cleanup path.
///
/// RAW's "use a Bonus Action on subsequent turns to designate a direction"
/// forced-movement clause is not modeled (the engine has no per-turn
/// forced-movement lane outside the existing push helper) — the load-
/// bearing combat clause is the Charmed install with `Charmed` back-link, which
/// turns affected enemies into "won't attack the bard" while the spell
/// holds. Slots between Charm Monster (lv4 single-target Charmed) and
/// Mass Suggestion (lv6 multi-target enchantment) on the bard's crowd-
/// control ladder — distinct from Charm Monster by the burst footprint
/// and self-centered targeting, distinct from Hypnotic Pattern (lv3 WIS-
/// save Incapacitated burst) by the duration anchor (Charmed lingers
/// throughout the encounter, while Hypnotic Pattern breaks on any damage).
pub struct Compulsion {}

impl Action for Compulsion {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Enchantment)
    }
    fn name(&self) -> &str {
        "compulsion"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["compel", "compulse", "cmp"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        // Self-centered 30ft burst — the caster broadcasts the compulsion
        // from their own tile. Same shape as Psychic Scream's
        // self-centered NoArgs lane at a smaller tier.
        TargetingSchema::NoArgs
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(4)
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        encounter.caster_can_concentrate(caster_id)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        // 30 ft RAW = 12 tiles. The self-centered burst targets every
        // enemy within range — the bard's whole front rank in a typical
        // clustered encounter.
        const RADIUS: isize = 12;
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let center = caster.location();
        // Bard (CHA) — single stat for the spell DC; the spell is on the
        // bard list RAW so the multi-class best-of isn't needed.
        let dc = caster.spell_save_dc(AbilityScoreType::Charisma);
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        let mut conditions: Vec<(usize, Condition)> = Vec::new();
        for tid in encounter.enemy_burst_targets(caster_id, center, RADIUS) {
            let save = encounter.roll_save_against_caster(
                tid,
                AbilityScoreType::Wisdom,
                dc,
                caster_id,
            );
            if save.passed() {
                encounter.log("  compulsion: target resists the compulsion.".to_string());
                continue;
            }
            encounter.log("  compulsion: target is compelled.".to_string());
            // Mirror the Charm Person / Geas install lane — Charmed flag
            // plus Charmed back-link, so the "can't attack your charmer"
            // gate in `action_template::validate_input` fires correctly.
            effects.extend(install_condition_with_link(
                Condition::Charmed,
                tid,
                caster_id,
                ConditionTimer::Rounds(10),
            ));
            conditions.push((tid, Condition::Charmed));
        }
        effects.push(Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData::with_conditions("Compulsion", conditions),
        }));
        effects
    }
}

pub static COMPULSION: LazyLock<Compulsion> = LazyLock::new(|| Compulsion {});

/// Longstrider — level-1 transmutation. Touch a willing creature; their
/// walking speed increases by 10 ft for 1 hour. No concentration —
/// fire-and-forget buff that sits durably across multiple encounters in
/// the same long rest. Routes through the central `Longstriding`
/// condition + `condition_speed_bonus` lane so the +10 ft composes
/// cleanly with Fly / Spider Climb / Expeditious Retreat.
///
/// Engine model: install `Longstriding` for `Rounds(100)` (≈ 10 minutes
/// engine time, more than enough for any encounter). SingleActor target
/// — the picker UI lets the caster aim it at any ally; the spell is
/// `is_harmful = false` so the AI's support pipeline considers it
/// alongside Bless / Heroism.
pub struct Longstrider {}

impl Action for Longstrider {
    fn name(&self) -> &str {
        "longstrider"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ls", "longstride"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // Touch range = melee reach in our grid.
        Some(crate::actions::action_template::MELEE_REACH)
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(1)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        // Buff-only — reject hostile targets at side-effect time as a
        // safety net (the `is_harmful = false` flag already steers the
        // picker UI to allies). Shared `first_ally_target_id` helper
        // collapses the id-extract + ally-check into one early-return.
        let Some(target_id) = first_ally_target_id(encounter, caster_id, target_ids) else {
            return Vec::new();
        };
        vec![Box::new(ApplyCondition {
            actor_id: target_id,
            condition: Condition::Longstriding,
            // 1 hour RAW = effectively permanent for any single encounter.
            timer: ConditionTimer::Rounds(100),
        })]
    }
}

pub static LONGSTRIDER: LazyLock<Longstrider> = LazyLock::new(|| Longstrider {});

/// Expeditious Retreat — level-1 transmutation, bonus action, concentration.
/// The caster can Dash as a bonus action on each turn for up to 10 minutes.
/// We collapse the action-economy half of the RAW spell into a flat
/// `+30 ft` speed bump (matching one Dash's worth of bonus movement) so
/// the kiting payoff fires cleanly without re-modeling the Dash-as-bonus
/// mechanic. Routed through the `ExpeditiouslyRetreating` condition +
/// `condition_speed_bonus` lane so the boost composes with Longstrider
/// / Fly / Spider Climb.
///
/// Engine model: install `ExpeditiouslyRetreating` for `Rounds(10)` (1
/// minute, capped well below the RAW 10-minute duration so the
/// concentration uptime matches the engine's other 1-min concentration
/// buffs like Mage Armor / Bless). Self-target (NoArgs); concentration-
/// bound on the caster.
pub struct ExpeditiousRetreat {}

impl Action for ExpeditiousRetreat {
    fn name(&self) -> &str {
        "expeditious retreat"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["er", "expeditious", "retreat"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        // RAW: cast as a bonus action; the spell consumes a level-1 slot.
        bonus_action_and_slot(1)
    }
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        vec![
            Box::new(ApplyCondition {
                actor_id: caster_id,
                condition: Condition::ExpeditiouslyRetreating,
                timer: ConditionTimer::Rounds(10),
            }) as Box<dyn ApplicableSideEffect>,
            Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions(
                    "Expeditious Retreat",
                    vec![(caster_id, Condition::ExpeditiouslyRetreating)],
                ),
            }),
        ]
    }
}

pub static EXPEDITIOUS_RETREAT: LazyLock<ExpeditiousRetreat> =
    LazyLock::new(|| ExpeditiousRetreat {});

/// Earthbind — level-2 transmutation (XGtE), action, concentration. The
/// caster grounds a flying target with a yellow strip of magical energy.
/// RAW: target makes a STR save vs the caster's spell DC; on fail the
/// target's flying speed (if any) becomes 0 for the spell's duration
/// (concentration, up to 1 minute) and they fall safely to the ground.
///
/// Engine model: since our engine collapses flight into a binary
/// `Flying` / `InvestedInWind` condition (each contributing +60 ft to
/// `condition_speed_bonus`), Earthbind's load-bearing effect is the
/// strip of those flags from a failed-save target. We use the existing
/// `RemoveCondition` side-effect to clear both flight sources; no new
/// condition needed. If the target wasn't flying at cast time the spell
/// still consumes the slot but produces no observable effect — matching
/// RAW's "if the target isn't flying, nothing happens" clause.
///
/// We don't model the concentration-bound *re-apply-on-flight* behavior
/// (RAW: the target stays grounded for the duration even if they regain
/// flight) — the engine's flight conditions don't get reapplied mid-spell
/// in any current flow, so the simpler "strip on cast" matches observed
/// behavior. Concentration is still tracked so dropping it logs cleanly.
pub struct Earthbind {}

impl Action for Earthbind {
    fn name(&self) -> &str {
        "earthbind"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["eb", "earth-bind", "ground"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 300 ft RAW — effectively unbounded at engine scale. We cap at
        // 48 tiles (≈ 120 ft) so the picker UI still gates by range.
        Some(48)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(2)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::side_effects::RemoveCondition;
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spellcasting_save_dc();
        // Caster-aware save so Heightened Spell metamagic can force
        // disadvantage on the single save-or-suck STR roll.
        let save = encounter.roll_save_against_caster(
            target_id,
            AbilityScoreType::Strength,
            dc,
            caster_id,
        );
        if save.passed() {
            return Vec::new();
        }
        // Strip both flight sources on a failed save. RemoveCondition is
        // a no-op if the target wasn't holding the flag, so casting on a
        // grounded target consumes the slot but produces no visible
        // change — matching RAW's "if the target isn't flying, nothing
        // happens" clause.
        vec![
            Box::new(RemoveCondition {
                actor_id: target_id,
                condition: Condition::Flying,
            }) as Box<dyn ApplicableSideEffect>,
            Box::new(RemoveCondition {
                actor_id: target_id,
                condition: Condition::InvestedInWind,
            }),
        ]
    }
}

pub static EARTHBIND: LazyLock<Earthbind> = LazyLock::new(|| Earthbind {});

/// Enhance Ability — level-2 transmutation (bard / cleric / druid /
/// sorcerer / wizard), action, concentration. The caster touches one
/// willing creature and bestows a magical enhancement chosen from a list
/// of six animal-themed boons (Bear's Endurance / Bull's Strength /
/// Cat's Grace / Eagle's Splendor / Fox's Cunning / Owl's Wisdom). The
/// universal load-bearing clause is "advantage on ability checks for the
/// chosen ability score" — which the engine doesn't model directly (we
/// don't track ability-check rolls outside saves).
///
/// Engine model: collapse to the *combat-relevant* twin (Bear's
/// Endurance). On cast, the target gains 2d6 temporary HP (an immediate
/// buffer that pairs with low-AC casters and dying frontliners) and a
/// flat +2 to every saving throw via the standard `AdjustSaveBuff` lane.
/// Both halves clear on concentration drop — the temp HP via natural
/// consumption / long rest, the save buff via the standard concentration
/// cleanup hook. Distinct from `Bless` (the burst counterpart) by the
/// single-target focus, the temp HP rider, and no attack-roll buff.
///
/// SingleActor target, touch range, concentration-bound on the caster.
/// `is_harmful = false` so the AI's support pipeline considers it
/// alongside Bless / Heroism / Aid.
pub struct EnhanceAbility {}

impl Action for EnhanceAbility {
    fn name(&self) -> &str {
        "enhance ability"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ea", "enhance"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(crate::actions::action_template::MELEE_REACH)
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn is_heal(&self) -> bool {
        // Temp HP install — surfaces to the AI's heal-search lane so an
        // enhance-ability cast on a wounded ally registers as support.
        true
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(2)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::side_effects::AdjustSaveBuff;
        // Buff-only — reject hostile aim at the side-effect site as a
        // safety net (the `is_harmful = false` flag already steers the
        // picker UI to allies). Shared `first_ally_target_id` helper
        // collapses the id-extract + ally-check into one early-return,
        // mirroring Longstrider / Aid.
        let Some(target_id) = first_ally_target_id(encounter, caster_id, target_ids) else {
            return Vec::new();
        };
        // 2d6 temp HP (Bear's Endurance flavor) + flat +2 saves.
        let raw = encounter.roll(&Dice::new(2, 6));
        encounter.log(format!(
            "  enhance ability: 2d6({}) temp HP, +2 saves",
            raw
        ));
        vec![
            Box::new(GainTempHp {
                actor_id: target_id,
                amount: raw,
            }) as Box<dyn ApplicableSideEffect>,
            Box::new(AdjustSaveBuff {
                actor_id: target_id,
                delta: 2,
            }),
            Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Heroic,
                timer: ConditionTimer::Rounds(10),
            }),
            Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions(
                    "Enhance Ability",
                    vec![(target_id, Condition::Heroic)],
                )
                .with_save_buffs(vec![(target_id, 2)]),
            }),
        ]
    }
}

pub static ENHANCE_ABILITY: LazyLock<EnhanceAbility> = LazyLock::new(|| EnhanceAbility {});

/// Blink — level-3 transmutation (sorcerer / wizard), action, NO
/// concentration. The caster phases between the Material and Ethereal
/// Planes — RAW: at the end of each of their turns, roll 1d20; on 11+
/// they vanish to the Ethereal Plane until the start of their next turn,
/// during which time attacks against them are at disadvantage and they
/// can pass through obstacles.
///
/// Engine model: collapse the per-turn coin-flip into a flat install of
/// the existing `Displaced` condition on the caster. Displaced imposes
/// disadvantage on attackers (matching the "vanishes when attacked"
/// half) and breaks the first time the caster takes damage (matching
/// RAW's "if you're hit, you snap back into phase" intuition — close
/// enough; in RAW the blink-out is the explicit roll, but the engine's
/// damage-break envelope conveys the same defensive flavor cleanly).
/// Lasts up to 10 rounds (1 minute RAW) via the standard tick-down
/// timer. No concentration — fire-and-forget defensive buff.
///
/// Self-target (NoArgs); slots in alongside Mirror Image / Blur on the
/// caster's defensive lane. Distinct from Mirror Image (which uses the
/// mirror_images count) and Blur (which is concentration); Blink's niche
/// is "non-concentration, single-hit-break attacker disadvantage" — a
/// clean fit for a wizard already concentrating on Hold Person / Web.
pub struct Blink {}

impl Action for Blink {
    fn name(&self) -> &str {
        "blink"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["bl", "phase"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(3)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        encounter.log("  blink: phasing between planes".to_string());
        vec![Box::new(ApplyCondition {
            actor_id: caster_id,
            condition: Condition::Displaced,
            // 1 minute RAW; cap at 10 rounds so a swing-less blink
            // doesn't dangle across encounters.
            timer: ConditionTimer::Rounds(10),
        })]
    }
}

pub static BLINK: LazyLock<Blink> = LazyLock::new(|| Blink {});

/// Contagion — level-5 necromancy (cleric / druid), action, touch. RAW:
/// the caster makes a melee spell attack against one creature within
/// reach; on hit the target makes three CON saves at the end of each of
/// their turns over the next three turns; after three failed saves the
/// target contracts one of seven foul diseases (each a distinct mechanical
/// debuff: Blinding Sickness blinds, Filth Fever drops STR, etc.).
///
/// Engine model: collapse the three-save chain to a single CON save vs
/// the caster's spell DC, resolved on the cast site. The melee touch
/// attack itself is auto-hit (we drop the to-hit roll — the slot-5
/// resource is the gate, and a contested touch attack + save chain
/// would be one of the heaviest action-economy bills in the engine).
/// On a failed save the target picks up `Poisoned` for 10 rounds (1
/// minute) — the RAW disease variants collapse to the common debuff
/// envelope (disadvantage on attacks + ability checks).
///
/// SingleActor target, touch range; harmful single-target install. Slots
/// alongside Hold Person / Bestow Curse on the single-target lockdown
/// lane — distinct by save ability (CON, not WIS) so a different stat
/// profile gets bitten.
pub struct Contagion {}

impl Action for Contagion {
    fn name(&self) -> &str {
        "contagion"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["cg", "infect"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(crate::actions::action_template::MELEE_REACH)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(5)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        // Use WIS as the default spell ability (cleric/druid primary).
        // For SRD purity we'd pick per-caster; the cleric/druid pool
        // both lean WIS so a single ability suffices.
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spellcasting_save_dc();
        // Skip the save against a Poisoned-immune target — the install
        // can't land, and skipping preserves the Heightened Spell prime.
        if encounter.actor_immune_to_condition(target_id, Condition::Poisoned) {
            encounter.log("  contagion: target is immune to Poisoned".to_string());
            return Vec::new();
        }
        let save = encounter.roll_save_against_caster(
            target_id,
            AbilityScoreType::Constitution,
            dc,
            caster_id,
        );
        if save.passed() {
            return Vec::new();
        }
        encounter.log("  contagion: a foul disease takes hold".to_string());
        vec![Box::new(ApplyCondition {
            actor_id: target_id,
            condition: Condition::Poisoned,
            timer: ConditionTimer::Rounds(10),
        })]
    }
}

pub static CONTAGION: LazyLock<Contagion> = LazyLock::new(|| Contagion {});

/// Pyrotechnics — XGE level-2 transmutation, action, no concentration.
/// RAW: choose either Fireworks (bright burst → blinded) or Smoke (heavy
/// obscurement). We model the Fireworks half (the load-bearing combat
/// clause): a small burst at the target point deals 1d8 fire damage and
/// blinds enemies that fail a CON save. The smoke variant has no in-
/// engine clause (we don't model obscurement zones).
///
/// SinglePoint target, burst radius 2 (10ft), 60 ft (24-tile) range,
/// INT-based DC. Enemies in the area roll a CON save: half damage on
/// pass (Reflex Half via `enemy_burst_save_for_half`); on fail, also pick
/// up `Blinded` for `Rounds(2)` (the short flash-blind envelope). Slots
/// alongside Aganazzar's Scorcher / Snilloc's Snowball Swarm on the
/// entry-tier elemental-burst lane.
pub struct Pyrotechnics {}

impl Action for Pyrotechnics {
    fn name(&self) -> &str {
        "pyrotechnics"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["pyro", "flash"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 2 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Fire]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(2)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.best_spell_save_dc([
            AbilityScoreType::Intelligence,
            AbilityScoreType::Wisdom,
            AbilityScoreType::Charisma,
        ]);
        let (mut effects, saves) = enemy_burst_save_for_half(
            encounter,
            caster_id,
            point,
            2,
            AbilityScoreType::Constitution,
            dc,
            Dice::new(1, 8),
            DamageType::Fire,
            "pyrotechnics",
        );
        // Failed-save enemies are flash-blinded for a brief window. RAW
        // gates the blind on a CON save and lasts 1 minute — we collapse
        // the duration to 2 rounds since the spell's "flash" flavor is
        // a brief sear rather than a long install. Mirrors Tidal Wave's
        // Prone follow-up shape.
        push_condition_on_failed_save(
            &mut effects,
            &saves,
            Condition::Blinded,
            ConditionTimer::Rounds(2),
        );
        effects
    }
}

pub static PYROTECHNICS: LazyLock<Pyrotechnics> = LazyLock::new(|| Pyrotechnics {});

/// Flame Arrows — XGE level-3 transmutation, action, concentration.
/// RAW: the caster touches a quiver of arrows / crossbow bolts; up to 12
/// pieces of ammunition deal +1d6 fire on a hit until the spell ends or
/// 12 hits land. We collapse the per-ammunition counter to a flat
/// concentration self-buff: every ranged weapon hit the caster lands
/// deals +1d6 fire via the on_hit_riders table (gated by `ranged_only`
/// so a melee swing can't burn the buff). Drops cleanly on concentration
/// end. Mirrors Spirit Shroud's shape but ranged-only.
///
/// Self-target (NoArgs); slots alongside Spirit Shroud / Crusader's
/// Mantle on the per-hit weapon-buff lane. The `is_harmful = false` flag
/// + `deals_damage = false` keeps the AI's heal / harm pipelines clean.
pub struct FlameArrows {}

impl Action for FlameArrows {
    fn name(&self) -> &str {
        "flame arrows"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["fire arrows", "fa"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(3)
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // Already imbued → don't re-cast and burn another slot.
        actor_lacks_condition(encounter, caster_id, Condition::FlamingArrowed)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        encounter.log("  flame arrows: ammunition ignites with magical fire".to_string());
        self_concentration_buff_effects(
            caster_id,
            "Flame Arrows",
            Condition::FlamingArrowed,
            ConditionTimer::Rounds(10),
        )
    }
}

pub static FLAME_ARROWS: LazyLock<FlameArrows> = LazyLock::new(|| FlameArrows {});

/// Ashardalon's Stride — TCE level-3 transmutation, bonus action,
/// concentration. RAW: the caster's body crackles with elemental fire,
/// gaining +20 ft of speed; moving doesn't provoke opportunity attacks
/// and any creature within 5 ft of the caster's path takes 1d6 fire
/// damage from the blazing wake. We model the load-bearing combat
/// clauses: the +20 ft speed bump (routed through
/// `condition_speed_bonus`) and the per-step adjacent-enemy fire
/// damage (hooked in `MoveActor`'s apply path next to Spike Growth /
/// Booming Blade). The OA-exemption clause is *not* modeled (the
/// engine doesn't have a per-action OA-exempt gate); the spell still
/// reads well in combat without it.
///
/// Self-target (NoArgs); slots alongside Longstrider / Expeditious
/// Retreat on the mobility-buff lane, but combat-flavored (the trail
/// damage IS the spell's hook). Concentration-bound on the caster.
pub struct AshardalonsStride {}

impl Action for AshardalonsStride {
    fn name(&self) -> &str {
        "ashardalon's stride"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["stride", "ashardalon"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        bonus_action_and_slot(3)
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // Already striding → don't re-cast and burn another slot.
        actor_lacks_condition(encounter, caster_id, Condition::AshardalonStriding)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        encounter.log("  ashardalon's stride: a blazing wake erupts behind you".to_string());
        self_concentration_buff_effects(
            caster_id,
            "Ashardalon's Stride",
            Condition::AshardalonStriding,
            ConditionTimer::Rounds(10),
        )
    }
}

pub static ASHARDALONS_STRIDE: LazyLock<AshardalonsStride> =
    LazyLock::new(|| AshardalonsStride {});

/// Tasha's Otherworldly Guise — TCE level-6 transmutation, bonus action,
/// concentration. RAW: the caster transforms into a celestial or fiendish
/// form: AC becomes 16 + CHA mod (we model as a flat +2 AC via
/// `condition_ac_bonus`), they gain a fly speed of 40 ft, resistance to
/// chosen damage types (we pick radiant + poison for the celestial
/// flavor), immunity to Charmed and Frightened, and their melee weapon
/// hits deal an extra 2d6 radiant damage via the on_hit_riders table.
///
/// Engine model — all four load-bearing clauses are wired through the
/// existing chokepoints:
/// - +2 AC: `condition_ac_bonus` reads the flag.
/// - +60 ft fly speed: joins the `Flying` / `InvestedInWind` cohort in
///   `condition_speed_bonus`.
/// - Radiant + Poison resistance: `TYPED_RESISTANCE_CONDITIONS` row.
/// - Charmed / Frightened / Poisoned dynamic immunity:
///   `dynamic_immunity_to` chokepoint.
/// - +2d6 radiant melee rider: `ON_HIT_RIDERS` entry with `melee_only: true`.
///
/// Self-target (NoArgs); slots at the top of the self-buff
/// concentration ladder alongside Tenser's Transformation / Globe of
/// Invulnerability on the legendary tier. Concentration-bound on the
/// caster.
pub struct OtherworldlyGuise {}

impl Action for OtherworldlyGuise {
    fn name(&self) -> &str {
        "otherworldly guise"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["og", "guise"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        bonus_action_and_slot(6)
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // Already guised → don't re-cast and burn another slot.
        actor_lacks_condition(encounter, caster_id, Condition::OtherworldlyGuised)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        encounter.log("  otherworldly guise: form shifts into a celestial avatar".to_string());
        self_concentration_buff_effects(
            caster_id,
            "Otherworldly Guise",
            Condition::OtherworldlyGuised,
            ConditionTimer::Rounds(10),
        )
    }
}

pub static OTHERWORLDLY_GUISE: LazyLock<OtherworldlyGuise> =
    LazyLock::new(|| OtherworldlyGuise {});

/// Silence — level-2 illusion, no concentration. Pick a tile within range;
/// a 20ft (8-tile) sphere of magical silence covers the area. Every actor
/// caught in the burst (friend or foe — silence is non-discriminating)
/// gains the `Silenced` condition for the duration. Mechanically:
/// - Can't cast leveled spells (verbal-component proxy — gated in
///   `can_consume_resource`'s SpellSlot lane via `blocks_spell_slots`).
/// - Immune to thunder damage (the magical hush absorbs sonic effects —
///   folded into `effective_damage`'s condition-driven immunity lane).
///
/// RAW also Deafens holders, but the engine's `Deafened` condition is a
/// cosmetic marker today, so the silence install skips it to avoid
/// piling a no-op flag onto every burst victim. Distinct from
/// `Counterspell` (which fizzles a specific cast) — Silence is a
/// persistent zone debuff that locks down spellcasters in a radius.
pub struct Silence {}

impl Action for Silence {
    fn name(&self) -> &str {
        "silence"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["sil", "hush"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        // 20 ft radius = 8 tile-gaps.
        TargetingSchema::Burst { radius: 8 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 120 ft = 48 tiles.
        Some(48)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn is_harmful(&self) -> bool {
        // Non-discriminating zone — the picker UI doesn't apply the
        // hostile-only filter, so a caster can drop it over their own
        // melee allies if the tactic calls for it.
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn damage_types(&self) -> Vec<DamageType> {
        Vec::new()
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(2)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let radius = match self.targeting_schema() {
            TargetingSchema::Burst { radius } => radius,
            _ => return Vec::new(),
        };
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        // Non-discriminating burst — Silence covers allies and enemies
        // alike (RAW: "any creature or object entirely inside the sphere").
        // We route through `neutral_burst_targets` so the standard
        // caster-exclusion + combat-active filter applies.
        for tid in encounter.neutral_burst_targets(caster_id, point, radius) {
            effects.push(Box::new(ApplyCondition {
                actor_id: tid,
                condition: Condition::Silenced,
                // 10 rounds ~ 1 minute RAW; no concentration so the buff
                // can outlast a concentration drop on another spell.
                timer: ConditionTimer::Rounds(10),
            }));
        }
        effects
    }
}

pub static SILENCE: LazyLock<Silence> = LazyLock::new(|| Silence {});

/// Freedom of Movement — level-4 abjuration, no concentration. Touch a
/// willing ally; for the duration they ignore difficult terrain and are
/// immune to any magical effect that would Paralyze, Restrain, or
/// Grapple them. We model the load-bearing combat half:
/// - Install `Footloose` on the target, which `dynamic_immunity_to`
///   reads to block future installs of those three conditions.
/// - Strip any currently-active Paralyzed / Restrained / Grappled install
///   so the target shrugs off the existing restraint at cast time too
///   (RAW: "the target can use 5 feet of movement to automatically
///   escape from nonmagical bonds" — we collapse to instant removal).
///
/// RAW is 1 hour without concentration; we install with `Rounds(100)`
/// (~10 minutes engine time) which covers any plausible encounter.
/// Joins `is_dispellable_buff` so Dispel Magic / Counterspell can
/// strip the protection cleanly. Distinct from `Purified` (Aura of
/// Purity) which blocks Charmed / Frightened / Poisoned — Freedom of
/// Movement covers the physical-restraint cohort instead.
pub struct FreedomOfMovement {}

impl Action for FreedomOfMovement {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Abjuration)
    }
    fn name(&self) -> &str {
        "freedom of movement"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["fom", "freedom"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // Touch.
        Some(crate::actions::action_template::MELEE_REACH)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(4)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::side_effects::RemoveCondition;
        // Ally-only target (the buff makes no sense to fling onto a
        // hostile creature). Same gate as Aid / Longstrider / Enhance
        // Ability — route through the shared helper for one chokepoint.
        let Some(target_id) = first_ally_target_id(encounter, caster_id, target_ids) else {
            return Vec::new();
        };
        // Install the Footloose buff (dynamic-immunity gate fires from
        // here for the rest of the duration) and strip any active
        // movement-restraint install so the target frees themselves
        // at cast time too.
        vec![
            Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Footloose,
                timer: ConditionTimer::Rounds(100),
            }),
            Box::new(RemoveCondition {
                actor_id: target_id,
                condition: Condition::Paralyzed,
            }),
            Box::new(RemoveCondition {
                actor_id: target_id,
                condition: Condition::Restrained,
            }),
            Box::new(RemoveCondition {
                actor_id: target_id,
                condition: Condition::Grappled,
            }),
        ]
    }
}

pub static FREEDOM_OF_MOVEMENT: LazyLock<FreedomOfMovement> =
    LazyLock::new(|| FreedomOfMovement {});

/// Raise Dead — level-5 necromancy. Touch a Dying ally; restore them to
/// HP-floor (1 HP) and clear the death-save tally. Mirrors `Revivify`
/// (level-3, ~10 minute window) and `Resurrection` (level-7, full-HP
/// restore) — Raise Dead slots in the middle: same "touch a dying ally"
/// envelope as Revivify, but at the level-5 slot it carries an
/// implicit window stretch (RAW: 10 days vs Revivify's 1 minute). We
/// don't model elapsed-death timers in combat, so the load-bearing
/// difference vs Revivify is the higher slot cost — Raise Dead is
/// what a cleric reaches for when their level-3 slots are spent.
///
/// Doesn't restore conditions / cleanses — pair with Lesser
/// Restoration for the full revive-and-cleanse loop, or use
/// Resurrection / True Resurrection (level-7/9) for the cleanse-bundled
/// versions. Single-target, touch, action cost + level-5 slot.
pub struct RaiseDead {}

impl Action for RaiseDead {
    fn name(&self) -> &str {
        "raise dead"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["rd", "raise"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(crate::actions::action_template::MELEE_REACH)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn is_heal(&self) -> bool {
        true
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        _caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        let Some(target_id) = first_target_id(target_ids) else {
            return false;
        };
        // Only meaningful on a dying / stable target — pruning here
        // keeps the AI's heal-pipeline from queueing the slot-burning
        // cast onto a healthy ally.
        encounter
            .actors
            .get(&target_id)
            .is_some_and(|a| a.is_dying())
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(5)
    }
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        _caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::side_effects::ReviveDying;
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        // `ReviveDying` itself heals 1 HP and clears Unconscious / Prone
        // — that's the Revivify-style 1-HP floor RAW. No additional Heal
        // entry needed here (Resurrection / True Resurrection layer an
        // additional max-HP heal on top; Raise Dead does not).
        vec![Box::new(ReviveDying { actor_id: target_id })]
    }
}

pub static RAISE_DEAD: LazyLock<RaiseDead> = LazyLock::new(|| RaiseDead {});

/// Darkness — level-2 evocation, concentration. Pick a tile within range;
/// a 15ft (6-tile) sphere of magical darkness drops over the area. Every
/// actor caught in the burst — friend or foe — gains the `Darkened`
/// condition: they swing blind (attacker disadvantage on their attacks
/// via `imposes_attacker_disadvantage`) and attackers targeting them
/// swing blind too (target-side disadvantage via
/// `imposes_disadvantage_to_attackers`). RAW: the darkness blocks
/// sight even for darkvision — we collapse to a symmetric attack-mode
/// penalty mirroring how `Blinded` handles single-target sight loss.
///
/// Concentration-bound on the caster so re-cast / damage-drop cleanly
/// strips the install via the standard concentration cleanup. Distinct
/// from `Blinded` so cleanse pickers and dispel sweeps can target just
/// the Darkness install. Non-discriminating zone — the caster's own
/// allies caught in the burst eat the same blind / blind-on-attackers
/// envelope, so the tactical use is shielding a melee-heavy frontline
/// against ranged casters or blanket-blinding a tight enemy cluster.
pub struct Darkness {}

impl Action for Darkness {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Evocation)
    }
    fn name(&self) -> &str {
        "darkness"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["dark", "shroud"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        // 15 ft radius = 6 tile-gaps.
        TargetingSchema::Burst { radius: 6 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60 ft = 24 tiles.
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn is_harmful(&self) -> bool {
        // Non-discriminating zone — same hint as Silence so the picker
        // UI doesn't apply hostile-only filtering.
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(2)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let radius = match self.targeting_schema() {
            TargetingSchema::Burst { radius } => radius,
            _ => return Vec::new(),
        };
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        let mut conditions: Vec<(usize, Condition)> = Vec::new();
        for tid in encounter.neutral_burst_targets(caster_id, point, radius) {
            effects.push(Box::new(ApplyCondition {
                actor_id: tid,
                condition: Condition::Darkened,
                timer: ConditionTimer::Rounds(10),
            }));
            conditions.push((tid, Condition::Darkened));
        }
        // Concentration-bound — registering every install on the
        // concentration so dropping it (damage / re-cast) strips the
        // darkness from every caught target in one sweep.
        if !conditions.is_empty() {
            effects.push(Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions("Darkness", conditions),
            }));
        }
        effects
    }
}

pub static DARKNESS: LazyLock<Darkness> = LazyLock::new(|| Darkness {});

/// Shadow of Moil — XGE level-4 warlock evocation, concentration, self-only.
/// The caster wraps themselves in clinging shadow: any creature that hits
/// them with a melee attack takes 2d8 necrotic damage in retaliation, AND
/// attacks against them have disadvantage (the holder is hard to see).
/// We model the load-bearing pieces via the `MoilShrouded` condition:
/// - The melee-reflect rider lives in `MELEE_REFLECT_RIDERS` next to the
///   Fire Shield / Investiture entries — symmetric shape, swapped damage
///   type (necrotic). Fires on every melee hit, no save.
/// - The attacker-disadvantage half rides
///   `imposes_disadvantage_to_attackers` so every swing against the holder
///   eats the standard mode penalty (single source of truth alongside
///   Blur / Displaced / Holy Aura).
/// - Concentration-bound on the caster; dropping concentration strips
///   the install via the standard cleanup hook.
///
/// Sibling to Fire Shield (2d8 fire melee reflect) on the self-shield
/// lane — distinct by the necrotic damage type AND the bundled attacker-
/// disadvantage clause (Fire Shield doesn't carry the dimness rider).
/// RAW also gives the holder dim-light illumination + 10 ft of
/// surrounding darkness; we collapse the lighting clause since the
/// engine has no sight system.
pub struct ShadowOfMoil {}

impl Action for ShadowOfMoil {
    fn name(&self) -> &str {
        "shadow of moil"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["moil", "shadow-moil"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(4)
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // Don't double-cast: refreshing while the buff is already up
        // wastes a level-4 slot. Mirrors the FireShielded / Investiture
        // / Otherworldly Guise self-buff gate.
        actor_lacks_condition(encounter, caster_id, Condition::MoilShrouded)
    }
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        // Concentration-bound self-buff. Mirrors Blur / Globe of
        // Invulnerability / Crusader's Mantle via `self_concentration_buff_effects`.
        self_concentration_buff_effects(
            caster_id,
            "Shadow of Moil",
            Condition::MoilShrouded,
            ConditionTimer::Rounds(10),
        )
    }
}

pub static SHADOW_OF_MOIL: LazyLock<ShadowOfMoil> = LazyLock::new(|| ShadowOfMoil {});

/// Animate Objects — level-5 transmutation, action, concentration. The
/// caster gives life to up to ten Tiny inanimate objects within range;
/// each becomes a Construct minion under the caster's control. We
/// collapse the RAW per-size object table (10× Tiny, 5× Small, 2×
/// Medium, 1× Large, 1× Huge — each with its own AC / HP / slam dice)
/// down to the most-numerous Tiny tier since it's the load-bearing
/// flavor: a swarm of ten construct minions chipping the frontline
/// is the spell's iconic moment.
///
/// Sibling to Conjure Animals (lv3, 2× Medium wolves) and Conjure
/// Elemental (lv5, 1× Large elemental) on the summon lane —
/// distinguished by the burst-count: ten Tiny minions trade per-target
/// damage for action-economy pressure (each one swings independently
/// every round). Routed through the shared `spawn_adjacent_summons` +
/// `conjured_summon_concentration_effects` helpers so the team-lookup,
/// adjacent-spawn search, Conjured-on-drop despawn, and concentration-
/// anchoring all behave identically to the other summon spells.
///
/// The 5e RAW range is 120 ft with each object dropping into an
/// unoccupied space within range; we reuse the spawn helper's
/// adjacent-anchor search (radius 4 — enough room for ten Tiny
/// footprints around the caster) so the placement stays inside the
/// existing summon envelope.
pub struct AnimateObjects {}

impl Action for AnimateObjects {
    fn summons_allies(&self) -> bool {
        true
    }
    fn name(&self) -> &str {
        "animate objects"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ao", "animate-obj", "objects"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(5)
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // Need at least one free Tiny anchor adjacent to the caster —
        // a tightly-walled caster (no free adjacent tile inside the
        // search radius) shouldn't burn the level-5 slot on a no-op
        // cast. Mirrors the Conjure Animals / Conjure Elemental gate.
        encounter
            .find_adjacent_spawn(caster_id, crate::engine::types::Size::Tiny, 4)
            .is_some()
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::actors::creatures::tiny_animated_objects::TINY_ANIMATED_OBJECT_TEMPLATE;

        // Up to ten Tiny construct minions on free adjacent slots.
        // Shared summon helper handles team lookup, adjacent-spawn
        // search, instantiate + log chain.
        let spawned = spawn_adjacent_summons(
            encounter,
            caster_id,
            &TINY_ANIMATED_OBJECT_TEMPLATE,
            crate::engine::types::Size::Tiny,
            10,
            4,
            500,
            "animate objects",
        );
        if spawned.is_empty() {
            return Vec::new();
        }
        conjured_summon_concentration_effects(caster_id, &spawned, "Animate Objects")
    }
}

pub static ANIMATE_OBJECTS: LazyLock<AnimateObjects> = LazyLock::new(|| AnimateObjects {});

/// Magnify Gravity — level-1 evocation (TCE / SAiS / EGtW spell list).
/// A 5ft-radius (1-tile burst) crushing gravity well snaps down at a point
/// within range: every creature in the burst makes a STR save vs the
/// caster's spell DC. On fail they take 2d8 force damage AND their speed
/// is halved until the end of the caster's next turn (we model the
/// movement-halve via the existing `Slowed` condition with `Rounds(1)`).
/// On a passed save they take half damage and shrug off the slow rider.
///
/// Friend-or-foe agnostic (the gravity well doesn't discriminate) —
/// routes through the neutral-burst helper alongside Ice Knife / Circle
/// of Death / Thunderclap. Single-action cast, no concentration — RAW
/// the spell is a single-target instant burst rather than a sustained
/// zone. The Slowed rider piggybacks on the heavier Slow-spell condition
/// (RAW Slow imposes -2 AC and -2 DEX saves alongside the speed halving;
/// we accept the small over-modeling at the lv1 tier since the engine's
/// condition cohort prefers a single Slowed flag over a dedicated
/// "gravity-slow" variant — the 1-round timer keeps the over-bake brief).
///
/// Sibling to Earth Tremor (lv1, self-centered 1d6 + prone burst) on the
/// lv1 force / bludgeoning lane: Magnify Gravity's targeted-point reach
/// (60 ft RAW = 24 tiles) trades the self-centered convenience for
/// pinpoint placement and the slow follow-up rather than the prone tag.
pub struct MagnifyGravity {}

impl Action for MagnifyGravity {
    fn name(&self) -> &str {
        "magnify gravity"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["mg", "magnify", "gravity"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 1 }
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 60 ft RAW = 24 tiles.
        Some(24)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Force]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(1)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(point) = first_target_location(target_locations) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        // RAW: STR save vs the caster's spell save DC. We pick the best of
        // the standard caster ability scores so the spell reads cleanly on
        // any class with the TCE list (sorcerer / wizard / artificer).
        let dc = caster.best_spell_save_dc([
            AbilityScoreType::Intelligence,
            AbilityScoreType::Wisdom,
            AbilityScoreType::Charisma,
        ]);
        let (mut effects, saves) = neutral_burst_save_for_half(
            encounter,
            caster_id,
            point,
            1,
            AbilityScoreType::Strength,
            dc,
            Dice::new(2, 8),
            DamageType::Force,
            "magnify gravity",
        );
        // Failed-save targets pick up Slowed for 1 round (= until end of
        // caster's next turn RAW). Routes through the shared helper so any
        // future change to the "burst → save → rider" plumbing lands in
        // one place instead of per-spell.
        push_condition_on_failed_save(
            &mut effects,
            &saves,
            Condition::Slowed,
            ConditionTimer::Rounds(1),
        );
        effects
    }
}

pub static MAGNIFY_GRAVITY: LazyLock<MagnifyGravity> = LazyLock::new(|| MagnifyGravity {});

/// Elemental Weapon — level-3 transmutation (paladin / artificer / ranger
/// / artificer-style multiclass), action, concentration. The caster
/// touches a single weapon-wielding ally; their weapon glows with
/// elemental energy for the spell's duration. RAW: +1 attack rolls AND
/// +1d4 extra damage of the caster's chosen element (acid / cold / fire /
/// lightning / thunder) on every hit.
///
/// Engine model: install two parallel buff lanes via the shared
/// concentration ledger so dropping concentration rolls back both halves
/// cleanly:
///   1. **+1 attack** — routes through `AdjustAttackBuff` and the
///      `with_attack_buffs` concentration vec (mirrors Magic Weapon's
///      attack buff lane).
///   2. **+1d4 fire on hit** — installs `ElementallyWeaponed` on the
///      target; the `ON_HIT_RIDERS` entry in `attack.rs` reads the flag
///      and adds +1d4 fire to every melee weapon hit the holder lands.
///      Persistent (non-consumed), melee-only. RAW lets the caster pick
///      the element at cast time; we collapse to fire as the signature
///      flavor since the engine's per-cast picker UI doesn't yet surface
///      a damage-type selection — the lv3 slot cost still differentiates
///      cleanly from the lv2 Magic Weapon (+1 attack +1 damage, no
///      typed rider).
///
/// Touch range, SingleActor target, `is_harmful = false` so the AI's
/// support pipeline considers it alongside Bless / Magic Weapon. The
/// `actor_lacks_condition` gate stops double-cast refreshes (same shape
/// as Spirit Shroud / Investiture of Flame / Holy Weapon's self-buff
/// dodge — adapted for the ally-target case so a paladin can't burn the
/// slot re-priming an already-buffed fighter).
pub struct ElementalWeapon {}

impl Action for ElementalWeapon {
    fn name(&self) -> &str {
        "elemental weapon"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ew", "elemental-weapon"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(crate::actions::action_template::MELEE_REACH)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(3)
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        _caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // Don't burn the slot refreshing a target that already has the
        // buff up. Mirrors the self-buff `actor_lacks_condition` gate in
        // Shadow of Moil / Investiture of Flame / Otherworldly Guise,
        // adapted for the ally-target case (gates on the target id
        // rather than the caster).
        let Some(target_id) = first_target_id(target_ids) else {
            return false;
        };
        actor_lacks_condition(encounter, target_id, Condition::ElementallyWeaponed)
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::side_effects::AdjustAttackBuff;
        // Buff-only — reject hostile targets at side-effect time as a
        // safety net (the `is_harmful = false` flag should already steer
        // the picker UI to allies). Shared `first_ally_target_id` helper
        // collapses the id-extract + ally-check into one early-return,
        // mirroring Aid / Longstrider / Enhance Ability.
        let Some(target_id) = first_ally_target_id(encounter, caster_id, target_ids) else {
            return Vec::new();
        };
        // +1 attack via AdjustAttackBuff (rolled back by concentration
        // drop), plus the ElementallyWeaponed condition (read by the
        // OnHitRider table for +1d4 fire per melee hit). Concentration
        // anchored to both halves so dropping the spell cleanly removes
        // the condition AND negates the attack-buff delta.
        vec![
            Box::new(AdjustAttackBuff {
                actor_id: target_id,
                delta: 1,
            }) as Box<dyn ApplicableSideEffect>,
            Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::ElementallyWeaponed,
                // 1 hour RAW; capped here at 100 rounds (≈ 10 minutes
                // engine time) to match the other long-duration buffs
                // (Mage Armor, Mind Blank, Longstrider).
                timer: ConditionTimer::Rounds(100),
            }),
            Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions(
                    "Elemental Weapon",
                    vec![(target_id, Condition::ElementallyWeaponed)],
                )
                .with_attack_buffs(vec![(target_id, 1)]),
            }),
        ]
    }
}

pub static ELEMENTAL_WEAPON: LazyLock<ElementalWeapon> = LazyLock::new(|| ElementalWeapon {});

/// Protection from Poison — level-2 abjuration (cleric / druid / paladin /
/// ranger spell list). Touch range, 1 hour, no concentration. Cleanses
/// any active Poisoned condition off the target, gives them resistance
/// to poison damage, and (RAW) advantage on saves vs being poisoned.
///
/// Modeled by installing the existing `Purified` condition for ~100
/// rounds (1 hour engine time): it covers the poison-damage resistance
/// half (via `TYPED_RESISTANCE_CONDITIONS`) and the saves-vs-poison half
/// (collapsed to full immunity via the `dynamic_immunity_to(Poisoned)`
/// chokepoint). RAW Protection from Poison doesn't also block Charmed /
/// Frightened, but `Purified`'s broader dynamic-immunity scope catches
/// those too — a minor flavor overshoot accepted in exchange for not
/// adding a near-duplicate buff condition. The cure-on-cast strips any
/// existing `Poisoned` install first so casting on a poisoned ally has
/// immediate value.
///
/// Sibling to `LESSER_RESTORATION` (lv2, removes one of four conditions)
/// and `AURA_OF_PURITY` (lv4, paladin self-aura that installs `Purified`
/// on every nearby ally) on the cleanse/buff lane — Protection from
/// Poison is the single-target poison-focused niche at the lv2 tier.
pub struct ProtectionFromPoison {}

impl Action for ProtectionFromPoison {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Abjuration)
    }
    fn name(&self) -> &str {
        "protection from poison"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["pfp", "prot poison", "antitoxin spell"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(crate::actions::action_template::MELEE_REACH)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn is_heal(&self) -> bool {
        // The cleanse lane (strips Poisoned) parallels Lesser Restoration's
        // is_heal flag so the AI's support pipeline considers this spell
        // when an ally is actively poisoned, not just when full HP.
        true
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(2)
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // Ally-only single-target buff. Reuses the shared `first_ally_target_id`
        // helper for the ally / live-target gate; also short-circuits if the
        // target is already Purified (no point burning a slot on a no-op
        // refresh — `add_condition` keeps the longer timer either way).
        let Some(target_id) = first_ally_target_id(encounter, caster_id, target_ids) else {
            return false;
        };
        encounter
            .actors
            .get(&target_id)
            .is_some_and(|t| t.is_combat_active() && !t.has_condition(Condition::Purified))
    }
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        _caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::side_effects::RemoveCondition;
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        vec![
            // Cleanse any active poison first — the RAW: "neutralize any
            // poison afflicting it" half.
            Box::new(RemoveCondition {
                actor_id: target_id,
                condition: Condition::Poisoned,
            }) as Box<dyn ApplicableSideEffect>,
            // 1 hour RAW (~100 rounds engine time). No concentration —
            // Protection from Poison sits durably across multi-encounter
            // long rests.
            Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Purified,
                timer: ConditionTimer::Rounds(100),
            }),
        ]
    }
}

pub static PROTECTION_FROM_POISON: LazyLock<ProtectionFromPoison> =
    LazyLock::new(|| ProtectionFromPoison {});

/// True Seeing — level-6 divination (bard / cleric / sorcerer / warlock /
/// wizard). Touch range, 1 hour, no concentration. The target gains
/// truesight 120 ft: they see through invisibility, magical concealment,
/// and illusory blur / displacement.
///
/// Modeled as the `TrueSighted` condition installed for ~100 rounds (1
/// hour engine time). The condition is read by `compute_attack_mode` via
/// the `countered_by_truesight` cohort: when the holder is the *attacker*,
/// they ignore the disadvantage that the target's `Invisible` / `Blurred`
/// / `Displaced` would impose; when the holder is the *target*, attackers
/// can't ride the matching advantage from their own `Invisible`. Joins
/// `is_dispellable_buff` so Dispel Magic can rip it. Distinct from the
/// template-level senses (Truesight as a creature stat block trait) —
/// True Seeing is a spell-side buff that's portable to any ally.
///
/// At lv6 the spell sits next to `GLOBE_OF_INVULNERABILITY` / `MASS_SUGGESTION`
/// on the lv6 utility-buff tier — single-target hour-long enabler for
/// the rest of the party against an illusionist / invisible-stalker
/// opponent.
pub struct TrueSeeing {}

impl Action for TrueSeeing {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Divination)
    }
    fn name(&self) -> &str {
        "true seeing"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ts", "true-sight", "truesight"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(crate::actions::action_template::MELEE_REACH)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(6)
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // Ally-only buff + don't double-cast on an already-buffed target
        // (the lv6 slot is precious). Routes through the shared
        // ally-target + actor_lacks_condition helpers used by Aid /
        // Longstrider / Enhance Ability.
        let Some(target_id) = first_ally_target_id(encounter, caster_id, target_ids) else {
            return false;
        };
        actor_lacks_condition(encounter, target_id, Condition::TrueSighted)
    }
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        _caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        vec![Box::new(ApplyCondition {
            actor_id: target_id,
            condition: Condition::TrueSighted,
            // 1 hour RAW; capped here at 100 rounds (~10 minutes engine
            // time) to match the other long-duration utility buffs
            // (Mage Armor, Mind Blank, Longstrider, Foresight).
            timer: ConditionTimer::Rounds(100),
        })]
    }
}

pub static TRUE_SEEING: LazyLock<TrueSeeing> = LazyLock::new(|| TrueSeeing {});

/// See Invisibility — level-2 divination (bard / sorcerer / wizard).
/// **Self**-buff, 1 hour, no concentration: "for the duration, you see
/// invisible creatures and objects as if they were visible."
///
/// The strictly weaker, four-slot-levels cheaper sibling of
/// `TRUE_SEEING`, and the pair is what the engine's tiered
/// `ConcealmentPiercing` exists to tell apart. True Seeing pierces the
/// whole illusion cohort (`Invisible` / `Blurred` / `Displaced`); See
/// Invisibility pierces `Invisible` alone, so a Blurred or Displaced
/// opponent is exactly as hard to hit as before. Cast against the wrong
/// defense it does nothing at all, which is the trade the lv2 price
/// buys.
///
/// Three differences from `TRUE_SEEING` beyond the tier:
///   - **Self-only.** RAW's target line is "Self", where True Seeing
///     touches a willing creature. That drops the ally-targeting
///     helpers and the reach / LOS gates with them.
///   - **Level 2 rather than 6.** It lands on the wizard's loadout at a
///     tier where the slot is genuinely spendable mid-fight, which is
///     the point: a party that meets an invisible stalker at level 5
///     has this and does not have True Seeing.
///   - **No ally coverage.** Only the caster sees through the
///     invisibility, so it answers "I can't hit the thing" and not
///     "the party can't hit the thing".
///
/// Duration capped at 100 rounds like the other long utility buffs, and
/// on `is_dispellable_buff` via `SeeingInvisible` so Dispel Magic can
/// rip it.
pub struct SeeInvisibility {}

impl Action for SeeInvisibility {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Divination)
    }
    fn name(&self) -> &str {
        "see invisibility"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["si", "see-inv", "seeinvis"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }
    fn is_harmful(&self) -> bool {
        false
    }
    fn deals_damage(&self) -> bool {
        false
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(2)
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // Don't re-cast over an existing sight buff. True Seeing is a
        // strict superset, so a target already carrying it has nothing
        // to gain and the slot would be thrown away.
        actor_lacks_condition(encounter, caster_id, Condition::SeeingInvisible)
            && actor_lacks_condition(encounter, caster_id, Condition::TrueSighted)
    }
    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        vec![Box::new(ApplyCondition {
            actor_id: caster_id,
            condition: Condition::SeeingInvisible,
            timer: ConditionTimer::Rounds(100),
        })]
    }
}

pub static SEE_INVISIBILITY: LazyLock<SeeInvisibility> = LazyLock::new(|| SeeInvisibility {});

/// Immolation — level-5 transmutation (sorcerer / wizard spell list),
/// concentration. 90-ft single-target. The target makes a DEX save vs
/// the caster's spell save DC; on a fail they take 8d6 fire damage AND
/// catch fire — taking 4d6 fire damage at the end of each of their turns
/// (via the central `ROUND_END_DOTS` registry entry on `Immolated`) until
/// the spell ends. On a pass they take half damage and shrug off the
/// ongoing burn.
///
/// The end-of-turn DEX save to put the flames out lives in the matching
/// `ROUND_END_SAVES` entry — a passed save strips `Immolated` and drops
/// the caster's concentration. Concentration-bound on the caster;
/// dropping concentration also extinguishes the flames cleanly. The
/// `Rounds(10)` timer caps the burn at ~1 minute engine time so the
/// spell expires naturally even if the target never saves out and the
/// caster never drops concentration.
///
/// Sibling to `BURNING_HANDS` (lv1 burst) / `FIREBALL` (lv3 AoE) /
/// `WALL_OF_FIRE` (lv4 zone) on the fire-damage lane — Immolation
/// trades the burst-shape for sustained single-target pressure: 8d6
/// up-front + 4d6/round is a real threat curve that scales past a
/// single-cast `FIREBALL`-equivalent.
pub struct Immolation {}

impl Action for Immolation {
    fn school(&self) -> Option<SpellSchool> {
        Some(SpellSchool::Evocation)
    }
    fn name(&self) -> &str {
        "immolation"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["immo", "immolate"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        // 90 ft = 36 tiles in the 2.5ft grid.
        Some(36)
    }
    fn requires_los(&self) -> bool {
        true
    }
    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Fire]
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        action_and_slot(5)
    }
    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // Concentration spell — don't burn the lv5 slot if the caster is
        // already concentrating on something more valuable (the new cast
        // would drop the prior concentration RAW, but the AI's heuristics
        // expect the gate to short-circuit).
        encounter
            .actors
            .get(&caster_id)
            .is_some_and(|a| !a.is_concentrating())
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.best_spell_save_dc([
            AbilityScoreType::Intelligence,
            AbilityScoreType::Wisdom,
            AbilityScoreType::Charisma,
        ]);
        let save =
            encounter.roll_save_against_caster(target_id, AbilityScoreType::Dexterity, dc, caster_id);
        // Shared roll for full + half: the dice are rolled once and the
        // save outcome decides whether the target keeps the full lash or
        // walks off with half. Matches the canonical
        // `SaveDamagePolicy::HalfOnSave` shape used by single-target save
        // spells (Sacred Burst single, Inflict Wounds variant, etc.).
        let raw = encounter.roll_empowered_sum(caster_id, 8, 6);
        let dmg = SaveDamagePolicy::HalfOnSave.apply(raw, save.passed());
        encounter.log(format!(
            "  immolation: 8d6({}) fire ({})",
            raw,
            if save.passed() {
                "half on save"
            } else {
                "full on fail"
            },
        ));
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        if dmg > 0 {
            effects.push(Box::new(DealDamage {
                actor_id: target_id,
                amount: dmg,
                damage_type: DamageType::Fire,
            }));
        }
        if !save.passed() {
            // Ongoing burn rider: ~1 minute cap. Anchored to the caster's
            // concentration so dropping concentration extinguishes the
            // flames cleanly via the standard cleanup path.
            effects.push(Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Immolated,
                timer: ConditionTimer::Rounds(10),
            }));
            effects.push(Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData::with_conditions(
                    "Immolation",
                    vec![(target_id, Condition::Immolated)],
                ),
            }));
        }
        effects
    }
}

pub static IMMOLATION: LazyLock<Immolation> = LazyLock::new(|| Immolation {});
