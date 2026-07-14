use crate::conditions::{Condition, ConditionTimer};
use crate::engine::dice::Dice;
use crate::engine::encounter::EncounterInstance;
use crate::engine::side_effects::{
    ApplicableSideEffect, ApplyCondition, DealDamage, PushActor,
};
use crate::engine::types::{AbilityScoreType, DamageType};

/// Inputs to a single attack roll. Lets callers describe attacks without
/// repeating the d20 / crit / damage / log dance for every weapon and
/// damage cantrip.
pub struct AttackParams<'a> {
    pub caster_id: usize,
    pub target_id: usize,
    pub action_name: &'a str,
    /// Static attack-roll modifier (STR/DEX for weapons, INT/WIS/CHA for
    /// spells). Caster-side flat buffs (Bless, etc.) are folded in by
    /// `resolve_attack` itself — pass only the action's own bonus here.
    pub attack_bonus: i32,
    pub damage_dice: Dice,
    /// Static damage-roll modifier (STR/DEX/spellcasting mod). Added once
    /// per swing even on a crit.
    pub damage_bonus: i32,
    pub damage_type: DamageType,
    /// `true` for melee weapons / touch spells, `false` for ranged
    /// attacks. Drives the prone-target advantage / disadvantage clause
    /// in `compute_attack_mode`.
    pub is_melee: bool,
    /// 5e long-range threshold (in tiles). Ranged attacks beyond this
    /// distance impose disadvantage. `None` means no long-range penalty
    /// (melee weapons, spells). Set to the weapon's "normal range"
    /// converted to tiles — attacks between `long_range` and `reach`
    /// roll with disadvantage per RAW.
    pub long_range: Option<isize>,
    /// `true` for spell-attack rolls (Fire Bolt, Eldritch Blast, Magic
    /// Stone, ...), `false` for weapon swings. Drives the 5e Tasha's
    /// Sorcerer Seeking Spell rider: on a miss, a spell-attack roll can
    /// burn a sorcery-point prime to reroll the d20. Weapon attacks must
    /// stay opted-out — the metamagic is RAW spell-attacks only. The
    /// flag also gates any future "this is a spell" sites that the
    /// engine grows (e.g. counterspell triggers, anti-magic field).
    pub is_spell: bool,
}

/// Caster-side flat melee-only damage bumps read at
/// `resolve_attack_outcome` after the base damage roll lands. Each
/// entry is a (label, amount_fn) tuple: the amount_fn reads the
/// caster's features/conditions and returns the flat bonus (`0`
/// skips the log line). All entries stack additively on holders that
/// carry multiple flags.
///
/// Entries:
///   - **Rage (+2)**: Barbarian's Raging condition.
///   - **Dueling (+2)**: Fighting Style flag; RAW's "one-handed and
///     no other weapon" clause collapses to "melee weapon attack" in
///     this engine.
///   - **Two-Weapon Fighting (+STR mod)**: Fighting Style flag; RAW's
///     "second attack" clause collapses to "every melee swing on the
///     holder" since we don't distinguish off-hand swings at the
///     action-list level.
///   - **Aura of Hate (+CHA mod, min +1)**: Oathbreaker Paladin lv7
///     subclass feature. RAW's ally-side aura on adjacent fiends /
///     undead is dropped since the engine doesn't tag those as an
///     aura-eligible cohort — the self-side +CHA damage is the
///     mechanical core.
///
/// A new melee-side bump (Ancestral Guardians retribution, Rage
/// tier-scaling to +3/+4, a Warlock's Lifedrinker) drops in here as
/// a new tuple.
const MELEE_CASTER_BUMPS: &[(&str, fn(&crate::actors::actor_template::ActorInstance) -> u32)] = &[
    ("rage", |a| {
        if a.has_condition(Condition::Raging) { 2 } else { 0 }
    }),
    ("dueling", |a| {
        if a.has_dueling_style() { 2 } else { 0 }
    }),
    ("two-weapon fighting", |a| {
        if !a.has_two_weapon_fighting_style() {
            return 0;
        }
        a.ability_modifier(AbilityScoreType::Strength).max(0) as u32
    }),
    ("aura of hate", |a| {
        if !a.has_aura_of_hate() {
            return 0;
        }
        // RAW: minimum +1 even if the paladin's CHA modifier is
        // zero or negative. Matches the Aura of Protection floor
        // shape (`aura_of_protection_bonus` clamps to `max(1)`).
        a.ability_modifier(AbilityScoreType::Charisma).max(1) as u32
    }),
];

/// Caster-side crit-only melee extra-dice sources read at
/// `resolve_attack_outcome` when a critical hit lands with a melee
/// weapon. Each entry is a (label, count_fn) tuple: the count_fn
/// reads the caster's template flag / dice-count field and returns
/// the number of extra weapon-face dice to roll (`0` skips the log
/// line and the roll).
///
/// Entries stack additively on holders that carry multiple sources —
/// a level-17 half-orc barbarian reads Brutal Critical's 3 dice AND
/// Savage Attacks' 1 die for a +4 dice crit bump.
///
/// Entries:
///   - **Brutal Critical**: Barbarian level 9 / 13 / 17 template-
///     driven dice count. `0` for non-barbarians (the default).
///   - **Savage Attacks**: Half-Orc racial flag; one flat extra die.
///
/// A new crit-extra-dice source (Piercer feat's +1 die, a hypothetical
/// Champion "Superior Critical" bonus die) drops in as a new tuple.
const CRIT_MELEE_EXTRA_DICE_SOURCES: &[(&str, fn(&crate::actors::actor_template::ActorInstance) -> u32)] = &[
    ("brutal critical", |a| a.brutal_critical_dice()),
    ("savage attacks", |a| if a.has_savage_attacks() { 1 } else { 0 }),
];

/// Shared eligibility gate for target-side reactive self-clamp damage
/// reducers — Uncanny Dodge (Rogue lv5), Deflect Missiles (Monk lv3),
/// and Parry (Fighter Battle Master maneuver). All three fire on an
/// incoming attack against the target and share the same gate shape:
///
/// 1. Target exists and is combat-active.
/// 2. Target holds the passive flag (`passive_ok(target)` returns true).
/// 3. Target has an unspent reaction slot.
/// 4. Target has an unspent charge of `feature_tag` (skipped when the
///    reducer is un-gated, i.e. Uncanny Dodge / Deflect Missiles).
/// 5. Target can perceive the attacker — routes through
///    `viewer_can_see` so an Invisible / Blurred / Displaced attacker
///    (or a Blinded / Sphered target) can't provoke the reaction.
///
/// Returns `true` iff all gates pass. Kept as a `&self`-only read so
/// callers can borrow the target for stat lookups (DEX / level) before
/// swapping to `&mut encounter` for the die roll.
///
/// Centralizes the five-clause chain that each of the three reducers
/// previously open-coded so that adding a future reactive reducer
/// (Shield Master's shove-on-hit, a hypothetical Deflect Missiles
/// analog for spell attacks) drops in as a one-line `is_eligible` call
/// plus the reduction body.
fn reactive_reducer_eligible(
    encounter: &EncounterInstance,
    target_id: usize,
    attacker_id: usize,
    passive_ok: fn(&crate::actors::actor_template::ActorInstance) -> bool,
    feature_tag: Option<&'static str>,
) -> bool {
    let Some(target) = encounter.actors.get(&target_id) else {
        return false;
    };
    if !target.is_combat_active() || !passive_ok(target) || !target.has_reaction() {
        return false;
    }
    if feature_tag.is_some_and(|t| !target.feature_available(t)) {
        return false;
    }
    encounter.viewer_can_see(target_id, attacker_id)
}

/// Consume the shared side of a reactive self-clamp damage reducer —
/// spend the target's reaction slot and (optionally) the associated
/// per-rest feature charge. Called after Uncanny Dodge / Deflect
/// Missiles / Parry actually fire and reduce the swing's damage.
///
/// Pairs with `reactive_reducer_eligible` (the read-side gate) so the
/// two ends of the reactive-reducer lifecycle share one chokepoint per
/// side. Silent no-op if the target has already been cleaned up
/// mid-swing (matches the defensive pattern the sibling reducers used
/// before the refactor).
fn spend_reactive_reducer(
    encounter: &mut EncounterInstance,
    target_id: usize,
    feature_tag: Option<&'static str>,
) {
    if let Some(t) = encounter.actors.get_mut(&target_id) {
        t.consume_resource(crate::engine::side_effects::Resource::Reaction);
        if let Some(tag) = feature_tag {
            t.spend_feature(tag);
        }
    }
}

/// 5e Fighter Battle Master **Riposte** maneuver — reactive melee
/// counter-attack that fires when a melee attack MISSES the target and
/// the target holds the `has_riposte` flag with an unspent `RIPOSTE_TAG`
/// charge, an available reaction, and a suitable melee weapon action on
/// their action list. Also gated on `viewer_can_see` so an Invisible /
/// Blurred / Displaced attacker (or a Blinded fighter) can't be
/// counter-struck (mirrors the Uncanny Dodge / Deflect Missiles sight
/// gate). No-op on any missing prerequisite. The counter-attack fires
/// via the same "run the underlying attack's side_effects" chokepoint
/// the opportunity-attack dispatcher uses in `EncounterInstance`, so
/// weapon-side riders (Bless, on-hit smite primes, etc.) fold in
/// cleanly without a bespoke roll pipeline here.
///
/// Kept public so `spells.rs` can drive it from `spell_attack_outcome`
/// if we later extend RAW to trigger Riposte on missed spell attacks
/// too — the current RAW clause is melee-only so the caller in
/// `resolve_attack_outcome` gates on `p.is_melee` at the call site.
pub fn try_fire_riposte(
    encounter: &mut EncounterInstance,
    target_id: usize,
    attacker_id: usize,
) {
    use crate::actions::action_template::MELEE_REACH;

    // Shared eligibility gate (passive flag + reaction slot + per-rest
    // charge + sight) — same helper the self-clamp reducers (Uncanny
    // Dodge / Deflect Missiles / Parry) use. Riposte is a counter-attack
    // rather than a damage clamp, but its five-clause gate is identical
    // so both lanes route through the shared chokepoint.
    if !reactive_reducer_eligible(
        encounter,
        target_id,
        attacker_id,
        |a| a.has_riposte(),
        Some(crate::actions::class_features::RIPOSTE_TAG),
    ) {
        return;
    }
    // Find the target's first melee weapon action via the shared
    // `first_melee_weapon_action` predicate — same helper that the
    // opportunity-attack dispatcher uses on the reactor side. Filters
    // out touch-range buffs / heals (Cure Wounds is `SingleActor` with
    // reach 1 but harmless), ranged actions, and AoE bursts.
    let Some(attack) = encounter
        .actors
        .get(&target_id)
        .and_then(|target| target.first_melee_weapon_action())
    else {
        return;
    };
    // Confirm the attacker is still in range of the target — RAW: the
    // riposte is a melee weapon attack, so the standard reach check
    // must pass. Uses the encounter helper so multi-tile footprints
    // resolve correctly.
    let reach = attack.reach_tiles().unwrap_or(MELEE_REACH);
    let Some(dist) = encounter.footprint_distance(target_id, attacker_id) else {
        return;
    };
    if dist > reach {
        return;
    }

    let target_name = encounter.actor_name(target_id);
    let attacker_name = encounter.actor_name(attacker_id);
    encounter.log(format!(
        "[reaction] {} ripostes {} after the miss",
        target_name, attacker_name
    ));

    // Fire the underlying attack's side_effects directly (matches the
    // opportunity-attack dispatch shape). Cost consumption is bypassed
    // — Riposte is a reaction, not a regular action, so we spend the
    // reaction + feature charge below instead of the action's normal
    // Action-slot cost.
    let target_vec = vec![attacker_id];
    let effects = attack.side_effects(encounter, target_id, Some(&target_vec), None, None);
    for e in effects {
        e.apply(encounter);
    }
    spend_reactive_reducer(
        encounter,
        target_id,
        Some(crate::actions::class_features::RIPOSTE_TAG),
    );
    encounter.cleanup_dead_actors();
}

/// 5e Fighting Style: **Interception** — shared reduction helper for
/// weapon and spell attacks. Finds the first eligible adjacent ally with
/// the flag + reaction, rolls `1d10 + prof`, spends the ally's reaction,
/// and returns the post-clamp damage. Damage of 0 is a no-op (skip the
/// scan). No-op when no ally qualifies.
///
/// Shared between `resolve_attack_outcome` here and
/// `spell_attack_outcome` in `spells.rs` so a single site owns the
/// 1d10 + prof reduction envelope for every attack roll — RAW's
/// "weapon or spell attack" is honored by having both call sites
/// funnel through this helper.
pub fn apply_interception_reduction(
    encounter: &mut EncounterInstance,
    attacker_id: usize,
    target_id: usize,
    damage: u32,
) -> u32 {
    if damage == 0 {
        return damage;
    }
    let Some(interceptor_id) = encounter.first_eligible_interceptor(attacker_id, target_id)
    else {
        return damage;
    };
    let Some(interceptor) = encounter.actors.get(&interceptor_id) else {
        return damage;
    };
    let prof = interceptor.proficiency_bonus();
    let interceptor_name = interceptor.name().to_string();
    let raw = encounter.roll(&Dice::new(1, 10)) as i32;
    let reduction = (raw + prof).max(0) as u32;
    let reduced = damage.saturating_sub(reduction);
    encounter.log(format!(
        "  interception: {} clamps 1d10({}){:+} = -{} damage ({} \u{2192} {})",
        interceptor_name, raw, prof, reduction, damage, reduced
    ));
    if let Some(a) = encounter.actors.get_mut(&interceptor_id) {
        a.consume_resource(crate::engine::side_effects::Resource::Reaction);
    }
    reduced
}

/// Resolve a 5e d20 attack roll against a single target's AC. On a hit,
/// returns a `DealDamage` side-effect for the rolled damage; on a miss,
/// returns an empty vec. The d20 result, hit/miss outcome, and damage
/// breakdown are logged in the engine's standard shape.
///
/// Critical hits (natural 20 after advantage/disadvantage) auto-hit and
/// double the damage dice (the modifier is added once). Caster's flat
/// `attack_bonus_buff` (Bless, etc.) is added at roll time so a buff that
/// landed mid-multiattack still picks up later swings correctly.
pub fn resolve_attack(
    encounter: &mut EncounterInstance,
    p: AttackParams,
) -> Vec<Box<dyn ApplicableSideEffect>> {
    resolve_attack_outcome(encounter, p).0
}

/// Result of an attack roll. `damage_dealt` is the post-crit, pre-target-
/// resistance damage value that will hit the queue — `0` on a miss or when
/// a Mirror Image absorbed the swing. Use this variant when the caller
/// needs to chain off the rolled damage (e.g. a self-heal rider equal to
/// half the damage, or a max-HP drain equal to the damage on a failed
/// save) without re-rolling and double-consuming the RNG.
pub fn resolve_attack_outcome(
    encounter: &mut EncounterInstance,
    p: AttackParams,
) -> (Vec<Box<dyn ApplicableSideEffect>>, u32) {
    let Some(target_ac) = encounter
        .actors
        .get(&p.target_id)
        .map(|a| a.armor_class() as i32)
    else {
        return (Vec::new(), 0);
    };
    // 5e Swashbuckler Rogue Fancy Footwork (subclass level 3): every
    // melee attack made by the attacker writes the target id onto
    // their per-turn `melee_attack_targets_this_turn` ledger. Cleared
    // at the attacker's own turn-start reset. The write is
    // unconditional (no `has_fancy_footwork` gate) so the mark stays
    // cheap; the OA-suppression read in `dispatch_opportunity_attacks`
    // is where the flag gates the suppression. RAW's trigger is on
    // "making a melee attack" — we place the mark BEFORE the Sanctuary
    // save check below, so even a swing that bounces off a sanctified
    // target still counts as an attempted attack (mirrors RAW's "if
    // you make a melee attack" wording — the swing was attempted even
    // if it was warded off). Written for weapon melee only via the
    // `p.is_melee` gate — a spell attack routed through this chokepoint
    // (currently only weapon-attack-shaped spells like Booming Blade
    // and Green Flame Blade) picks up the mark too since they're
    // engine-tagged `is_melee: true`, matching the RAW "melee attack
    // roll" trigger.
    if p.is_melee
        && let Some(attacker) = encounter.actors.get_mut(&p.caster_id)
    {
        attacker.mark_melee_attacked_this_turn(p.target_id);
    }
    // 5e Cover: intervening combat-active creatures bump the target's
    // effective AC (+2 for half cover, +5 for three-quarters). Adjacent
    // melee swings are exempt (the cover routine returns 0 at gap ≤ 1).
    let cover_bonus = encounter.cover_ac_bonus(p.caster_id, p.target_id);
    // 5e Hunter Ranger Multiattack Defense (Defensive Tactics option,
    // lv7): if the target holds `MULTIATTACK_DEFENSE_TAG` AND this
    // attacker has already landed a connecting swing on the target
    // this turn, add +4 to the target's effective AC. The +4 lasts
    // for "the rest of the turn" per RAW — anchored to the attacker's
    // own turn-start reset in `reset_for_new_round`. Shared read
    // between weapon and spell attacks via the encounter helper.
    let multiattack_defense_bonus =
        encounter.multiattack_defense_ac_bonus(p.caster_id, p.target_id);
    let target_ac = target_ac + cover_bonus + multiattack_defense_bonus;

    // 5e Sanctuary: if the target is sanctified, the attacker first makes
    // a WIS save vs the warding caster's DC. On fail, the attack silently
    // misses (no AC roll, no rider consumption). Pass-through behavior
    // matches RAW's "the attacker must choose a new target or lose the
    // attack" clause — we choose the latter to avoid auto-retargeting.
    if encounter.sanctuary_save_blocks(p.caster_id, p.target_id) {
        return (Vec::new(), 0);
    }
    // Casting a harmful action revokes the holder's own Sanctuary buff.
    // We tag the clear after we know the attack is going through (the
    // sanctuary_save_blocks branch returns early on fail).
    encounter.break_sanctuary_on_hostile(p.caster_id);

    let mut mode = encounter.compute_attack_mode(p.caster_id, p.target_id, p.is_melee);
    // 5e Fighting Style: **Protection** — a target-adjacent ally (NOT
    // the target itself) with the Protection flag and an unspent
    // reaction may burn their reaction to impose disadvantage on THIS
    // attack. RAW: "when a creature you can see attacks a target other
    // than you that is within 5 feet of you". The eligibility scan
    // lives on the encounter (`first_eligible_protector`) so the ally-
    // sweep chokepoint stays shared with other ally-adjacency features.
    // Resolved HERE (rather than inside `compute_attack_mode`) because
    // the reaction spend needs `&mut encounter`, and `compute_attack_mode`
    // is a `&self` read chokepoint. Ordering-wise: Protection fires
    // before the help / bless one-shot rider consumption below, so a
    // Protection-tax swing STILL burns the attacker's Help / Hidden /
    // Inspired priming — RAW: those primes are consumed on the roll,
    // regardless of disadvantage.
    if let Some(protector_id) = encounter.first_eligible_protector(p.caster_id, p.target_id) {
        mode = mode.combine(crate::engine::dice::RollMode::Disadvantage);
        if let Some(protector) = encounter.actors.get_mut(&protector_id) {
            protector.consume_resource(crate::engine::side_effects::Resource::Reaction);
        }
        encounter.log(
            "  protection: attack against target imposed disadvantage (protector's reaction spent)",
        );
    }
    // 5e target-side reactive per-rest disadvantage-imposing features
    // (Light Domain Cleric Warding Flare lv1, Great Old One Warlock
    // Entropic Ward lv6, and any future sibling). Wired here rather
    // than inside `compute_attack_mode` because the reaction spend +
    // log needs `&mut encounter`. RAW is "any attack roll" — no
    // weapon-only qualifier — so the same helper fires on spell
    // attacks via `spell_attack_outcome`. Layered AFTER Protection so
    // a target with both an adjacent Protection ally AND a
    // self-carried Warding Flare / Entropic Ward doesn't waste the
    // per-rest charge when Protection already handled the tax. Routes
    // through the shared `REACTIVE_ATTACK_DISADVANTAGE_SOURCES`
    // cohort so a hypothetical Light Cleric / Great Old One Warlock
    // multiclass burns at most one per-rest charge per incoming
    // attack — iteration stops on first firing.
    if mode != crate::engine::dice::RollMode::Disadvantage
        && encounter.apply_reactive_attack_disadvantage(p.target_id, p.caster_id)
    {
        mode = mode.combine(crate::engine::dice::RollMode::Disadvantage);
    }
    // 5e long-range disadvantage: ranged weapon attacks beyond normal
    // range but within max range impose disadvantage. The `long_range`
    // threshold (in tiles) is set by the weapon definition — melee
    // weapons and spells leave it `None`.
    if let Some(nr) = p.long_range
        && !p.is_melee
        && let Some(dist) = encounter.footprint_distance(p.caster_id, p.target_id)
        && dist > nr
    {
        mode = mode.combine(crate::engine::dice::RollMode::Disadvantage);
    }
    // Caster-side flat bonuses. `attack_bonus_buff` is the install-side
    // ledger (Bless's AdjustAttackBuff(+2), etc.). `condition_attack_bonus`
    // is the read-side flag table — Sacred Weapon's +CHA modifier and
    // Bardic Inspiration's +3 ride here. Keeping the two lanes separate
    // makes Bless's "install once, drop on concentration" pattern reuse
    // cleanly with the read-only condition lane. Shared with spell
    // attacks via `EncounterInstance::caster_attack_buffs`.
    //
    // Read these BEFORE clearing the one-shot riders so the Inspired
    // condition (and any other condition that contributes to
    // `condition_attack_bonus`) is still active when we sum the bonus.
    // The rider clear below removes Inspired alongside Helped/Hidden,
    // so swapping the order would zero out the +3.
    let (buff, cond_attack_bonus) = encounter.caster_attack_buffs(p.caster_id);
    // 5e Fighting Style: **Archery** — +2 to attack rolls made with ranged
    // weapons. RAW carves out spell attack rolls ("ranged weapon attacks"
    // specifically), so the bonus is gated on `!p.is_melee && !p.is_spell`.
    // Read on the caster side so a ranger's longbow shot picks up the bonus
    // regardless of the target — mirrors how the item / condition attack
    // bonus lanes fold in above.
    let archery_bonus = if !p.is_melee
        && !p.is_spell
        && encounter
            .actors
            .get(&p.caster_id)
            .is_some_and(|a| a.has_archery_style())
    {
        2
    } else {
        0
    };
    // Bless/Bane: roll an actual 1d4 once per attack and add (Bless) or
    // subtract (Bane) from the total. Both: they cancel and no die is
    // rolled. We log the d4 separately so the player can see why the
    // d20 alone doesn't account for the swing's hit.
    let (bless_die, bless_note) = encounter.bless_bane_attack_die(p.caster_id);
    // Burn through the one-shot rider stack (Helped, Hidden,
    // per-target help grant, Invisibility concentration, Inspired)
    // before the d20 lands so a second swing this turn doesn't double-
    // dip. The advantage / disadvantage flags were already picked up
    // into `mode` by `compute_attack_mode` above; the flat bonuses
    // were just read into `cond_attack_bonus` immediately above. Both
    // are safe to clear here without losing this swing's modifiers.
    encounter.clear_attack_advantage_riders(p.caster_id, p.target_id);
    // 5e Lucky: if the holder rolls a nat-1, they may re-roll once. The
    // helper folds the reroll into the same seedable RNG so determinism
    // by seed holds — and falls back to the raw roll for actors without
    // the trait.
    let mut raw_attack = encounter.roll_d20_lucky(p.caster_id, mode) as i32;
    // 5e Improved Critical: the d20 face that promotes to a crit is
    // template-driven (Champion fighter: 19+; Superior Critical: 18+).
    // The engine-level `crit_threshold` accessor folds in the default of
    // 20 for missing actors / non-Champion builds.
    let mut nat_crit = raw_attack >= encounter.crit_threshold(p.caster_id);
    let mut attack_total =
        raw_attack + p.attack_bonus + buff + cond_attack_bonus + bless_die + archery_bonus;
    let mut is_nat_one = raw_attack == 1;
    let mut hit = !is_nat_one && (nat_crit || attack_total >= target_ac);
    // 5e Tasha's Sorcerer Seeking Spell metamagic: on a missed spell
    // attack, reroll the d20 and use the new face (RAW: "you must use
    // the new roll"). Gated on `is_spell` so weapon swings don't pick
    // up the rider. Mirrors the identical hook on the spell-attack
    // chokepoint in spells.rs — both attack paths go through this same
    // `EncounterInstance::reroll_seeking_spell` helper.
    if !hit && p.is_spell {
        let new_raw = encounter.reroll_seeking_spell(p.caster_id, raw_attack as u32, mode) as i32;
        if new_raw != raw_attack {
            raw_attack = new_raw;
            nat_crit = raw_attack >= encounter.crit_threshold(p.caster_id);
            attack_total = raw_attack
                + p.attack_bonus
                + buff
                + cond_attack_bonus
                + bless_die
                + archery_bonus;
            is_nat_one = raw_attack == 1;
            hit = !is_nat_one && (nat_crit || attack_total >= target_ac);
        }
    }
    // 5e Wild Magic Sorcerer **Bend Luck** (lv6 reaction): the target may
    // burn 2 SP + their reaction to subtract a 1d4 from the attacker's
    // total. Only worth firing when the swing would otherwise hit AND
    // isn't a natural crit (the d4 can't undo a 20-face); a nat-1 already
    // misses. The penalty is folded into `attack_total` so the hit check
    // and the log line both reflect the bent total.
    let bend_penalty = if hit && !nat_crit && !is_nat_one {
        encounter.apply_bend_luck_penalty(p.target_id, p.caster_id) as i32
    } else {
        0
    };
    let bend_note = if bend_penalty > 0 {
        attack_total -= bend_penalty;
        hit = attack_total >= target_ac;
        format!(" -d4({})", bend_penalty)
    } else {
        String::new()
    };
    // 5e Paralyzed / Unconscious clause: any hit from within 5ft is a
    // crit. The promotion happens after we've decided the swing connected
    // so a flat miss still misses — the rider only upgrades a regular
    // hit to a crit (mirrors the RAW "any attack that hits the creature
    // is a critical hit" wording).
    let is_crit = nat_crit
        || (hit
            && encounter.target_grants_melee_auto_crit(p.caster_id, p.target_id, p.is_melee));
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
        p.action_name,
        raw_attack,
        p.attack_bonus + buff + cond_attack_bonus + archery_bonus,
        bless_note,
        bend_note,
        attack_total,
        target_ac,
        cover_note,
        mode.log_suffix(),
        outcome,
    ));
    if !hit {
        // 5e Fighter Battle Master **Riposte** maneuver: on a melee
        // miss against a fighter with the passive `has_riposte` flag and
        // an unspent `RIPOSTE_TAG` charge, spend the reaction + charge
        // to fire a melee weapon attack against the attacker. RAW: "when
        // a creature misses you with a melee attack, you can use your
        // reaction and expend one superiority die to make a melee weapon
        // attack against the creature." Gated on `is_melee` (RAW: melee
        // miss only), sight (routes through `viewer_can_see` so an
        // Invisible / Blurred / Displaced attacker can't be counter-
        // struck), and the target holding a suitable melee action on
        // their action list. Skipped on nat-1s AND non-melee misses so a
        // whiffed longbow shot or spell attack doesn't burn the charge.
        if p.is_melee {
            try_fire_riposte(encounter, p.target_id, p.caster_id);
        }
        return (Vec::new(), 0);
    }
    // 5e Mirror Image: a hit may instead strike a decoy. Shared with
    // spell attacks via `EncounterInstance::mirror_image_deflect` so
    // the deflection rule applies to any attack roll, not just weapon
    // swings (RAW: "any attack roll against you").
    if encounter.mirror_image_deflect(p.target_id, is_crit) {
        return (Vec::new(), 0);
    }
    // 5e Hunter Ranger Multiattack Defense (Defensive Tactics, lv7):
    // record that this attacker has now landed a connecting swing on
    // the target's own body — RAW's trigger is "when a creature hits
    // you" so a Mirror Image redirect (which lands on a decoy, not
    // the target) doesn't count and the mark is written after the
    // deflect check. Uncanny Dodge and Deflect Missiles run below;
    // both are reactive damage-reducers that don't cancel the hit
    // itself, so the mark IS written even if damage lands as zero.
    // Written even if the target doesn't currently hold the tag; the
    // tag is read on the penalty side (cheap set-insert vs. the
    // passive-feature lookup makes this the simpler ordering).
    if let Some(attacker) = encounter.actors.get_mut(&p.caster_id) {
        attacker.mark_hit_target_this_turn(p.target_id);
    }
    // 5e Fighting Style: **Great Weapon Fighting** — reroll any 1 / 2 on
    // a melee weapon damage die once, taking the new value even if it
    // comes up 1 or 2 again per RAW. Gated on `p.is_melee` so a longbow
    // shot (or a spell attack routed through this chokepoint) doesn't
    // pick up the reroll — the RAW "two-handed melee weapon" gate
    // collapses to "melee weapon attack" since the engine doesn't track
    // weapon-hand-usage (same shape as Dueling's gate collapse). Applied
    // to BOTH the base damage roll AND the crit's doubled dice so the
    // per-die reroll fires uniformly across the swing's dice pool. The
    // helper's non-GWF fast path is a single delegated `roll(&dice)` so
    // the vast majority of swings pay no extra cost.
    let apply_gwf = p.is_melee
        && encounter
            .actors
            .get(&p.caster_id)
            .is_some_and(|a| a.has_great_weapon_fighting());
    let raw_damage = encounter.roll_weapon_damage_dice(p.damage_dice, apply_gwf) as i32;
    let crit_extra = if is_crit {
        encounter.roll_weapon_damage_dice(p.damage_dice, apply_gwf) as i32
    } else {
        0
    };
    // 5e Brutal Critical (Barbarian level 9 / 13 / 17) + Half-Orc
    // Savage Attacks: both add extra weapon damage dice on a critical
    // melee hit. Spell attacks don't qualify — gated on `is_melee` +
    // `is_crit`. Read from the shared `CRIT_MELEE_EXTRA_DICE_SOURCES`
    // table — each entry is a (label, dice_count_fn) pair; the fn
    // reads the caster's template flag / dice-count field and returns
    // the number of extra weapon-face dice to roll (0 = skip). A
    // level-17 half-orc barbarian rolls 3 (Brutal Critical) + 1
    // (Savage Attacks) = 4 extra dice without touching this site.
    // Each rider logs separately so the source of the extra dice is
    // legible in the combat log. A new crit-extra-dice source (Piercer
    // feat's +1 die, a hypothetical Champion "Superior Critical"
    // bonus die) drops in as a new tuple rather than another
    // if-block copy.
    let brutal_extra = if is_crit && p.is_melee {
        let mut total = 0;
        for (label, count_fn) in CRIT_MELEE_EXTRA_DICE_SOURCES {
            let Some(a) = encounter.actors.get(&p.caster_id) else { break; };
            let count = count_fn(a);
            if count == 0 {
                continue;
            }
            let extra_dice = Dice::new(count, p.damage_dice.faces);
            let rolled = encounter.roll(&extra_dice) as i32;
            encounter.log(format!(
                "  {}: +{}({}) = +{} {:?}",
                label, extra_dice, rolled, rolled, p.damage_type
            ));
            total += rolled;
        }
        total
    } else {
        0
    };
    // Fold in the caster-side flat damage bonuses: item-passive
    // (`+1 Weapon`, Bracers of Archery) and spell-installed
    // (Magic Weapon, Elemental Weapon). Added once per swing — crits
    // already double the dice but the modifier (action's
    // `damage_bonus` + caster damage buff) is added once per RAW.
    // Picked up at this site so weapon AND spell attacks both see
    // the bonus through the same chokepoint.
    let caster_damage_buff = encounter.caster_damage_buffs(p.caster_id);
    let total_damage_bonus = p.damage_bonus + caster_damage_buff;
    let mut damage = (raw_damage + crit_extra + brutal_extra + total_damage_bonus).max(0) as u32;
    if is_crit {
        encounter.log(format!(
            "  {}: {}({})+{}({}){:+} = {} {:?} damage (crit)",
            p.action_name,
            p.damage_dice,
            raw_damage,
            p.damage_dice,
            crit_extra,
            total_damage_bonus,
            damage,
            p.damage_type,
        ));
    } else {
        encounter.log(format!(
            "  {}: {}({}){:+} = {} {:?} damage",
            p.action_name, p.damage_dice, raw_damage, total_damage_bonus, damage, p.damage_type,
        ));
    }
    // Caster-side flat melee-only bumps. Read from the
    // `MELEE_CASTER_BUMPS` table — each entry is a (label,
    // amount_fn) pair; the amount_fn reads the caster's
    // features/conditions and returns the flat bonus (0 = skip). A
    // new melee-side bump (Ancestral Guardians retribution, Rage
    // tier-scaling to +3/+4, etc.) drops in as a new tuple rather
    // than another `if p.is_melee && …` block. Order matters only
    // for legibility (all entries stack additively on holders that
    // carry multiple flags).
    if p.is_melee {
        for (label, amount_fn) in MELEE_CASTER_BUMPS {
            let Some(a) = encounter.actors.get(&p.caster_id) else { break; };
            let bump = amount_fn(a);
            if bump == 0 {
                continue;
            }
            damage = damage.saturating_add(bump);
            encounter.log(format!("  {}: +{} melee damage", label, bump));
        }
    }
    // Hunter's Mark rider: attacker concentrating on Hunter's Mark with
    // this target marked deals +1d6 (weapon-typed). Crits double the
    // mark die per RAW — the rider folds into the weapon's damage type.
    if encounter.is_hunters_mark_target(p.caster_id, p.target_id) {
        let hm_total = roll_rider(encounter, Dice::new(1, 6), is_crit);
        damage = damage.saturating_add(hm_total);
        encounter.log(format!(
            "  hunter's mark: +{} extra {:?}",
            hm_total, p.damage_type
        ));
    }
    // 5e Uncanny Dodge (Rogue 5): when hit by an attack, spend reaction
    // to halve the damage. Only fires if the target has the feature, a
    // reaction available, and can see the attacker. Eligibility gate
    // (passive + reaction + sight; no per-rest feature charge here)
    // routes through the shared `reactive_reducer_eligible` helper —
    // covers Blinded on the target AND the attacker being illusion-
    // concealed (Invisible / Blurred / Displaced) without the target
    // holding a piercing sense. Consumption (reaction spend) routes
    // through `spend_reactive_reducer`. Same gate shape now shared with
    // Deflect Missiles / Parry below.
    if reactive_reducer_eligible(
        encounter,
        p.target_id,
        p.caster_id,
        |a| a.has_uncanny_dodge(),
        None,
    ) {
        damage /= 2;
        encounter.log(format!("  uncanny dodge: damage halved to {}", damage));
        spend_reactive_reducer(encounter, p.target_id, None);
    }
    // 5e Monk Deflect Missiles (level 3): when hit by a ranged weapon
    // attack, the monk can spend their reaction to reduce damage by
    // `1d10 + DEX modifier + monk level`. Only fires on ranged weapon
    // swings (gated on `!is_melee` AND the rider chokepoint sees only
    // weapon attacks — spell attacks resolve through a separate path,
    // so the gate here covers the RAW "ranged weapon attack" clause).
    // Reaction is consumed iff the rider actually fires; an unprimed
    // monk eats the full damage instead. Layered AFTER Uncanny Dodge so
    // a rare rogue/monk multiclass benefits from both halves cleanly
    // (RAW order doesn't matter since both are independent reactions).
    // Eligibility + consume route through the shared reactive-reducer
    // helpers — the sight gate covers Blinded on the target AND
    // Invisible attackers uniformly with Uncanny Dodge / Parry.
    if !p.is_melee
        && damage > 0
        && reactive_reducer_eligible(
            encounter,
            p.target_id,
            p.caster_id,
            |a| a.has_deflect_missiles(),
            None,
        )
    {
        let target = &encounter.actors[&p.target_id];
        let dex_mod = target.ability_modifier(crate::engine::types::AbilityScoreType::Dexterity);
        let level = target.level() as i32;
        let raw = encounter.roll(&Dice::new(1, 10)) as i32;
        let reduction = (raw + dex_mod + level).max(0) as u32;
        let reduced = damage.saturating_sub(reduction);
        encounter.log(format!(
            "  deflect missiles: 1d10({}){:+}{:+} = -{} damage ({} → {})",
            raw, dex_mod, level, reduction, damage, reduced
        ));
        damage = reduced;
        spend_reactive_reducer(encounter, p.target_id, None);
    }
    // 5e Fighter Battle Master **Parry** maneuver: when a melee attack
    // hits, spend a reaction + one `PARRY_TAG` charge to reduce damage
    // by `1d8 + DEX modifier` (the RAW 5e superiority-die reducer).
    // Gated on the same shape as Uncanny Dodge / Deflect Missiles via
    // `reactive_reducer_eligible`, plus the per-rest feature charge:
    //   - `is_melee` — RAW: "when a creature damages you with a melee
    //     attack" (RAW's 2014 wording; 2024's phrasing is broader but
    //     we keep the melee-only gate so the maneuver stays distinct
    //     from Deflect Missiles' ranged-only lane),
    //   - `damage > 0` — no work to clamp on a zero-damage hit,
    //   - shared reactive-reducer eligibility handles the passive
    //     flag, reaction slot, sight, and per-rest charge in one call.
    // Layered after Deflect Missiles so the ordering stays "target-
    // side self-clamps first, then ally-side clamps via Interception
    // below" — a Battle Master fighter with a hypothetical monk
    // multiclass would still eat the parry charge on a melee hit even
    // after Deflect Missiles handled a ranged one earlier (RAW:
    // reactions are per-round, and each reactive feature is
    // independent).
    if p.is_melee
        && damage > 0
        && reactive_reducer_eligible(
            encounter,
            p.target_id,
            p.caster_id,
            |a| a.has_parry(),
            Some(crate::actions::class_features::PARRY_TAG),
        )
    {
        let target = &encounter.actors[&p.target_id];
        let dex_mod = target.ability_modifier(crate::engine::types::AbilityScoreType::Dexterity);
        let raw = encounter.roll(&Dice::new(1, 8)) as i32;
        let reduction = (raw + dex_mod).max(0) as u32;
        let reduced = damage.saturating_sub(reduction);
        encounter.log(format!(
            "  parry: 1d8({}){:+} = -{} damage ({} → {})",
            raw, dex_mod, reduction, damage, reduced
        ));
        damage = reduced;
        spend_reactive_reducer(
            encounter,
            p.target_id,
            Some(crate::actions::class_features::PARRY_TAG),
        );
    }
    // 5e Fighting Style: **Interception** (XGtE). An adjacent ally
    // (within 5 ft of the target) with the style flag and an unspent
    // reaction may burn their reaction to reduce the incoming damage
    // by `1d10 + prof`. Layered AFTER Deflect Missiles / Uncanny Dodge
    // so the ally's clamp fires on whatever damage remains — a monk
    // deflecting first and a paladin intercepting second is the RAW
    // "each reaction is independent" stacking. Skipped on zero damage
    // (no work to shield). Same helper feeds `spell_attack_outcome`
    // so an intercepting ally covers spell attacks too per RAW.
    if damage > 0 {
        damage = apply_interception_reduction(encounter, p.caster_id, p.target_id, damage);
    }
    let mut effects: Vec<Box<dyn ApplicableSideEffect>> = vec![Box::new(DealDamage {
        actor_id: p.target_id,
        amount: damage,
        damage_type: p.damage_type,
    })];
    if encounter.is_hex_target(p.caster_id, p.target_id) {
        push_die_rider(
            encounter,
            &mut effects,
            p.target_id,
            Dice::new(1, 6),
            is_crit,
            DamageType::Necrotic,
            "hex",
        );
    }
    // Caster-side per-hit damage riders. Generalized so any condition
    // that grants "+Xdy damage of type T on hit" plugs in here without
    // re-implementing the attack-roll-to-damage glue. Gates:
    //   - `melee_only`: skip on ranged attacks (5e Smite spells / Divine
    //     Smite RAW: melee weapon only).
    //   - `consume_on_trigger`: strip the condition after the rider
    //     lands (one-shot primes like Divine Smite / the four Smite
    //     spells; persistent aura-style riders like Crusader's Mantle
    //     and Crown of Stars leave their condition in place for the
    //     full spell duration).
    //   - `follow_up`: optional secondary clause that fires only on the
    //     swing that *consumed* the rider — used by Blinding Smite (CON
    //     save or Blinded) and Wrathful Smite (WIS save or Frightened).
    for rider in ON_HIT_RIDERS.iter().copied() {
        if rider.melee_only && !p.is_melee {
            continue;
        }
        if rider.ranged_only && p.is_melee {
            continue;
        }
        if !encounter
            .actors
            .get(&p.caster_id)
            .is_some_and(|a| a.has_condition(rider.condition))
        {
            continue;
        }
        // Roll the rider damage only if the rider actually has dice —
        // primes whose entire effect is the follow-up (Stunning Strike:
        // no damage, just a stun save) declare 0 dice so the damage
        // line and the DealDamage push are skipped. Capture the rolled
        // total so the follow-up's optional `hp_threshold` gate can
        // predict the target's post-damage HP without re-rolling.
        let rider_total = if rider.dice.count > 0 {
            let total = roll_rider(encounter, rider.dice, is_crit);
            encounter.log(format!(
                "  {}: +{} {:?}",
                rider.label, total, rider.damage_type
            ));
            effects.push(Box::new(DealDamage {
                actor_id: p.target_id,
                amount: total,
                damage_type: rider.damage_type,
            }));
            total
        } else {
            0
        };
        if rider.consume_on_trigger
            && let Some(caster) = encounter.actors.get_mut(&p.caster_id)
        {
            caster.remove_condition(rider.condition);
        }
        if let Some(follow) = rider.follow_up {
            apply_smite_follow_up(
                encounter,
                &mut effects,
                p.caster_id,
                p.target_id,
                follow,
                rider_total + damage,
            );
        }
    }
    // 5e Paladin Improved Divine Smite (level 11+) — passive feature.
    // Every melee weapon hit lays +1d8 radiant on the target, independent
    // of any Smite-prime burn. Lives outside the ON_HIT_RIDERS table
    // because it's keyed off a passive feature flag rather than a
    // transient condition, and we don't want a sentinel "always-on"
    // condition cluttering the conditions enum just to gate this one
    // rider. Crits double the die per `roll_rider` RAW.
    if p.is_melee
        && encounter
            .actors
            .get(&p.caster_id)
            .is_some_and(|a| {
                a.has_passive_feature(
                    crate::actions::class_features::IMPROVED_DIVINE_SMITE_TAG,
                )
            })
    {
        push_die_rider(
            encounter,
            &mut effects,
            p.target_id,
            Dice::new(1, 8),
            is_crit,
            DamageType::Radiant,
            "improved divine smite",
        );
    }
    // 5e Hunter Ranger **Colossus Slayer** (level 3) — passive once-per-
    // turn rider. On a weapon hit against a wounded target, lay an extra
    // 1d8 of the weapon's damage type. Three gates:
    //   1. Caster has the COLOSSUS_SLAYER_TAG passive feature flag.
    //   2. Caster hasn't already fired Colossus Slayer this turn
    //      (`colossus_slayer_used` — cleared at turn-start by
    //      `reset_for_new_round`).
    //   3. Target is wounded (`is_wounded()` — current HP below max).
    // No melee gate (RAW: "When you hit a creature with a weapon attack"
    // — covers ranger longbow shots too). Crit doubles the die via
    // `roll_rider`. Damage type matches the weapon so a fire-imbued bow
    // shot still reads as fire on the Colossus Slayer line.
    if !p.is_spell
        && encounter
            .actors
            .get(&p.caster_id)
            .is_some_and(|a| {
                a.has_passive_feature(
                    crate::actions::class_features::COLOSSUS_SLAYER_TAG,
                ) && !a.colossus_slayer_used()
            })
        && encounter
            .actors
            .get(&p.target_id)
            .is_some_and(|t| t.is_wounded())
    {
        push_die_rider(
            encounter,
            &mut effects,
            p.target_id,
            Dice::new(1, 8),
            is_crit,
            p.damage_type,
            "colossus slayer",
        );
        if let Some(caster) = encounter.actors.get_mut(&p.caster_id) {
            caster.mark_colossus_slayer_used();
        }
    }
    // 5e Ranger **Foe Slayer** (level 20 capstone) — passive once-per-
    // turn rider. On any weapon hit, add the ranger's Wisdom modifier
    // as flat damage of the weapon's damage type. Two gates:
    //   1. Caster has the FOE_SLAYER_TAG passive feature flag.
    //   2. Caster hasn't already fired Foe Slayer this turn
    //      (`foe_slayer_used` — cleared at turn-start by
    //      `reset_for_new_round`).
    // No melee gate (RAW: "an attack you make" — covers longbow shots).
    // The rider is a flat modifier, not a die, so it doesn't double on
    // a crit per RAW (crit-doubling applies to dice, not flat mods).
    // Fires only when the WIS modifier is positive — a WIS-dump ranger
    // (rare) reads no bonus rather than adding a penalty.
    if !p.is_spell
        && encounter
            .actors
            .get(&p.caster_id)
            .is_some_and(|a| {
                a.has_passive_feature(
                    crate::actions::class_features::FOE_SLAYER_TAG,
                ) && !a.foe_slayer_used()
            })
    {
        let wis_mod = encounter
            .actors
            .get(&p.caster_id)
            .map(|a| {
                crate::engine::util::modifier_from_score(
                    a.ability_score(AbilityScoreType::Wisdom),
                )
            })
            .unwrap_or(0);
        if wis_mod > 0 {
            let extra = wis_mod as u32;
            encounter.log(format!(
                "  foe slayer: +{} {:?}",
                extra, p.damage_type
            ));
            effects.push(Box::new(DealDamage {
                actor_id: p.target_id,
                amount: extra,
                damage_type: p.damage_type,
            }));
            if let Some(caster) = encounter.actors.get_mut(&p.caster_id) {
                caster.mark_foe_slayer_used();
            }
        }
    }
    // 5e Barbarian Path of the Zealot **Divine Fury** (level 3) — passive
    // once-per-turn rider. The first weapon hit each turn while raging
    // lays +1d6 + half barbarian level (min +1) radiant damage. Three
    // gates:
    //   1. Not a spell attack (RAW: "with a weapon attack").
    //   2. Caster has the DIVINE_FURY_TAG passive feature flag AND is
    //      currently Raging (RAW: "while you're raging").
    //   3. Caster hasn't already fired Divine Fury this turn
    //      (`divine_fury_used` — cleared at turn-start by
    //      `reset_for_new_round`).
    // Crits double the die per `roll_rider` RAW; the +level/2 flat mod
    // doesn't double. Damage is typed Radiant — RAW gives the zealot a
    // choice of radiant or necrotic, we lock to radiant so the "holy
    // warrior" tell stays visible on the log line.
    if !p.is_spell
        && encounter
            .actors
            .get(&p.caster_id)
            .is_some_and(|a| {
                a.has_passive_feature(
                    crate::actions::class_features::DIVINE_FURY_TAG,
                ) && a.has_condition(Condition::Raging)
                    && !a.divine_fury_used()
            })
    {
        let half_level = encounter
            .actors
            .get(&p.caster_id)
            .map(|a| (a.level() / 2).max(1))
            .unwrap_or(1);
        let die = roll_rider(encounter, Dice::new(1, 6), is_crit);
        let total = die + half_level;
        encounter.log(format!(
            "  divine fury: +{} (1d6({})+{}) Radiant",
            total, die, half_level
        ));
        effects.push(Box::new(DealDamage {
            actor_id: p.target_id,
            amount: total,
            damage_type: DamageType::Radiant,
        }));
        if let Some(caster) = encounter.actors.get_mut(&p.caster_id) {
            caster.mark_divine_fury_used();
        }
    }
    // Melee-only retaliation table: any condition the *target* holds that
    // bounces damage back at a melee attacker (Fire Shield 2d8 fire,
    // Armor of Agathys 5 cold, Investiture of Flame 1d10 fire). Each
    // entry plugs in here without re-implementing the "target has cond?
    // → roll → log → push DealDamage(attacker)" dance. The reflected
    // damage resolves through the standard damage pipeline so the
    // attacker's typed immunity / resistance / vulnerability is honored.
    if p.is_melee {
        for rider in MELEE_REFLECT_RIDERS.iter().copied() {
            if !encounter
                .actors
                .get(&p.target_id)
                .is_some_and(|a| a.has_condition(rider.condition))
            {
                continue;
            }
            push_reflect_damage(encounter, &mut effects, p.caster_id, rider.damage,
                rider.damage_type, rider.label);
        }
        // Creature-intrinsic natural reflect (Black Pudding Corrosive
        // Form, Salamander Heated Body). Composes additively with the
        // condition-keyed table — a salamander wearing Fire Shield rolls
        // both reflects on the same incoming swing.
        if let Some(natural) = encounter
            .actors
            .get(&p.target_id)
            .and_then(|a| a.natural_melee_reflect())
        {
            push_reflect_damage(encounter, &mut effects, p.caster_id, natural.damage,
                natural.damage_type, natural.label);
        }
    }
    (effects, damage)
}

/// Roll (or read flat) the reflect amount, log the reflection, and queue
/// the `DealDamage` payload against the original attacker. Shared
/// chokepoint for the condition-keyed reflect table and the creature-
/// intrinsic natural reflect lane so the log shape stays uniform across
/// both sources.
fn push_reflect_damage(
    encounter: &mut EncounterInstance,
    effects: &mut Vec<Box<dyn ApplicableSideEffect>>,
    attacker_id: usize,
    damage: ReflectDamage,
    damage_type: DamageType,
    label: &str,
) {
    let amount = match damage {
        ReflectDamage::Flat(n) => {
            encounter.log(format!("  {}: {} {:?} reflected", label, n, damage_type));
            n
        }
        ReflectDamage::Dice(dice) => {
            let rolled = encounter.roll(&dice);
            encounter.log(format!(
                "  {}: {}({}) {:?} reflected",
                label, dice, rolled, damage_type
            ));
            rolled
        }
    };
    effects.push(Box::new(DealDamage {
        actor_id: attacker_id,
        amount,
        damage_type,
    }));
}

/// Damage payload for a melee retaliation rider. Some shields roll dice
/// (Fire Shield 2d8, Investiture of Flame 1d10); others deal flat damage
/// (Armor of Agathys 5). Captured as an enum so the rider table stays a
/// flat array of plain-data entries.
#[derive(Clone, Copy)]
pub enum ReflectDamage {
    Dice(Dice),
    Flat(u32),
}

/// Plain-data melee reflect descriptor, used by creature-intrinsic
/// retaliation features (Black Pudding Corrosive Form, Salamander
/// Heated Body). Shares the `damage` / `damage_type` / `label` shape
/// with `MeleeReflectRider` but doesn't carry a condition key — the
/// holder's body itself is the trigger, no transient buff needed.
/// Const-constructible so templates can declare it inline.
#[derive(Clone, Copy)]
pub struct MeleeReflect {
    pub damage: ReflectDamage,
    pub damage_type: DamageType,
    /// Log-friendly tag ("corrosive form", "heated body", ...).
    pub label: &'static str,
}

/// A target-side "creature hit me in melee, take this damage back" rider.
/// Mirrors `OnHitRider` in shape but lives on the *target* of the swing
/// rather than the caster: any actor who holds `condition` reflects
/// `damage` of `damage_type` onto every melee attacker that connects.
#[derive(Clone, Copy)]
pub struct MeleeReflectRider {
    pub condition: Condition,
    pub damage: ReflectDamage,
    pub damage_type: DamageType,
    /// Log-friendly tag ("fire shield", "armor of agathys", ...).
    pub label: &'static str,
}

/// Melee retaliation rider table. Symmetric with `ON_HIT_RIDERS` but
/// consumed at the target side of `resolve_attack_outcome`. `Dice::new`
/// is a const fn so the whole table lives as a `const &[..]` — adding
/// a rider doesn't bump a hardcoded length.
const MELEE_REFLECT_RIDERS: &[MeleeReflectRider] = &[
    // 5e Fire Shield — 2d8 fire on every melee contact. Concentration-
    // free, self-only; the warm / cool variant only matters for the
    // resistance lane (we collapse to a single Fire Shield condition).
    MeleeReflectRider {
        condition: Condition::FireShielded,
        damage: ReflectDamage::Dice(Dice::new(2, 8)),
        damage_type: DamageType::Fire,
        label: "fire shield",
    },
    // 5e Armor of Agathys — flat 5 cold on melee contact. We don't
    // scale with slot level (the spell's install site sets the temp
    // HP buffer instead). Mirrors Fire Shield's shape but cheaper.
    MeleeReflectRider {
        condition: Condition::AgathysShielded,
        damage: ReflectDamage::Flat(5),
        damage_type: DamageType::Cold,
        label: "armor of agathys",
    },
    // 5e Investiture of Flame — 1d10 fire on melee contact. Smaller
    // die than Fire Shield (concentration-bound on the caster, paired
    // with the broader fire resistance baked into the install).
    MeleeReflectRider {
        condition: Condition::InvestedInFlame,
        damage: ReflectDamage::Dice(Dice::new(1, 10)),
        damage_type: DamageType::Fire,
        label: "investiture of flame",
    },
    // 5e Investiture of Ice — 1d10 cold on melee contact. Symmetric to
    // Investiture of Flame: same die / shape, swapped element. Paired
    // with cold resistance on the caster via the InvestedInIce branch
    // of `effective_damage`.
    MeleeReflectRider {
        condition: Condition::InvestedInIce,
        damage: ReflectDamage::Dice(Dice::new(1, 10)),
        damage_type: DamageType::Cold,
        label: "investiture of ice",
    },
    // 5e Investiture of Stone — 1d10 force on melee contact. Sibling to
    // Investiture of Flame / Ice but force-typed (the stone shell
    // crackles with telekinetic recoil rather than burning / freezing
    // the attacker). Paired with broad physical resistance on the
    // caster via the InvestedInStone row of TYPED_RESISTANCE_CONDITIONS.
    MeleeReflectRider {
        condition: Condition::InvestedInStone,
        damage: ReflectDamage::Dice(Dice::new(1, 10)),
        damage_type: DamageType::Force,
        label: "investiture of stone",
    },
    // 5e Shadow of Moil — 2d8 necrotic on melee contact. The clinging
    // shadows lash out at anyone who reaches into them. Concentration-
    // bound on the caster (paired with the attacker-disadvantage half
    // via `imposes_disadvantage_to_attackers`). Same shape as Fire
    // Shield, swapped element — necrotic-typed.
    MeleeReflectRider {
        condition: Condition::MoilShrouded,
        damage: ReflectDamage::Dice(Dice::new(2, 8)),
        damage_type: DamageType::Necrotic,
        label: "shadow of moil",
    },
];

/// Roll a single rider die for an on-hit bonus, doubling on crit per
/// 5e RAW. Used by every "per-hit weapon-bonus damage" effect — Hex /
/// Hunter's Mark (1d6), Crusader's Mantle (1d4), Crown of Stars (1d8).
/// Keeps the crit-doubling rule in one place. Public so the spell-attack
/// resolver can share the same crit-doubling rule for Hex.
pub fn roll_rider(encounter: &mut EncounterInstance, dice: Dice, is_crit: bool) -> u32 {
    let base = encounter.roll(&dice);
    let crit_extra = if is_crit { encounter.roll(&dice) } else { 0 };
    base + crit_extra
}

/// Roll a single-die on-hit rider (e.g. Improved Divine Smite, Colossus
/// Slayer, Hex), log it, and queue the resulting `DealDamage` payload
/// against `target_id`. Returns the rolled amount so callers needing the
/// raw value (Hex composes the post-damage total into its `damage`
/// running tally) can read it off the same chokepoint.
///
/// Collapses the recurring `roll_rider → log → push DealDamage` triple
/// that several on-hit features open-coded. Keeps the log shape uniform
/// across rider sources so a future grep / log-scanning test reads from
/// one shape.
pub fn push_die_rider(
    encounter: &mut EncounterInstance,
    effects: &mut Vec<Box<dyn ApplicableSideEffect>>,
    target_id: usize,
    dice: Dice,
    is_crit: bool,
    damage_type: DamageType,
    label: &str,
) -> u32 {
    let extra = roll_rider(encounter, dice, is_crit);
    encounter.log(format!("  {}: +{} {:?}", label, extra, damage_type));
    effects.push(Box::new(DealDamage {
        actor_id: target_id,
        amount: extra,
        damage_type,
    }));
    extra
}

/// A "+Xdy damage on hit" rider sourced from one of the caster's active
/// conditions. The rider table is consumed once per weapon hit by
/// `resolve_attack_outcome` — any caster who holds `condition` adds the
/// rolled `dice` of `damage_type` to that swing.
#[derive(Clone, Copy)]
pub struct OnHitRider {
    /// Caster-side flag the rider keys off (Smiting / Crusader's
    /// Mantled / Crown of Stars / one of the four Smite-spell primes).
    pub condition: Condition,
    pub dice: Dice,
    /// Log-friendly name ("divine smite", "searing smite", ...).
    pub label: &'static str,
    pub damage_type: DamageType,
    /// True iff the rider only fires on melee swings (every Paladin
    /// Smite, Divine Smite). Ranged carriers like Crown of Stars or
    /// Crusader's Mantle leave this false so they tag arrow hits too.
    pub melee_only: bool,
    /// True iff the rider only fires on ranged swings (5e Lightning
    /// Arrow RAW: "the next attack you make with a ranged weapon").
    /// Symmetric to `melee_only` — both default to false so the rider
    /// fires on either lane. Defaulting to false keeps every existing
    /// rider entry unchanged.
    pub ranged_only: bool,
    /// True iff the condition is stripped from the caster the moment
    /// the rider lands (one-shot primes). Persistent buffs leave this
    /// false so they stay up until the spell ends.
    pub consume_on_trigger: bool,
    /// Optional save-then-condition follow-up that fires only on the
    /// swing that consumed the rider. Powers Blinding Smite (CON save
    /// or Blinded) and Wrathful Smite (WIS save or Frightened). `None`
    /// for damage-only riders.
    pub follow_up: Option<SmiteFollowUp>,
}

/// Secondary save + effect rider tagged onto a Smite-spell hit. When
/// `save_ability` is `Some(ability)`, the target rolls that save against
/// the caster's spell DC (driven by `dc_ability`); on fail, the rider's
/// `effect` lands. When `save_ability` is `None`, the rider auto-applies
/// on hit with no save (Branding Smite's "brand", Searing Smite's ignite).
/// Stored as a value so the on-hit rider table stays a flat array of
/// plain-data entries.
#[derive(Clone, Copy)]
pub struct SmiteFollowUp {
    /// Save the target rolls (CON for Blinding Smite, WIS for Wrathful
    /// Smite). `None` skips the save entirely — the effect lands
    /// unconditionally on the consuming hit (subject to `hp_threshold`).
    pub save_ability: Option<AbilityScoreType>,
    /// Ability whose mod feeds the caster's spell save DC. Ignored when
    /// `save_ability` is `None` (no save means no DC).
    pub dc_ability: AbilityScoreType,
    /// What lands on a failed save: a condition apply, a push, or both.
    pub effect: FollowUpEffect,
    /// Log-friendly tag ("blinding smite blind", "wrathful smite fear",
    /// "pushing attack push").
    pub label: &'static str,
    /// Optional post-damage HP gate. Banishing Smite RAW: the rider lands
    /// only "if this damage reduces the target to 50 hp or fewer." We
    /// fold this into the smite follow-up site by predicting post-damage
    /// HP as `current_hp - rider_damage` and gating the apply on that.
    /// `None` (the default) skips the gate.
    pub hp_threshold: Option<u32>,
}

/// What a smite follow-up does on a failed save (or auto-trigger).
/// Most riders apply a single condition (`Condition`); the Battle Master
/// Pushing Attack maneuver shoves the target backward instead, and the
/// Sweeping Attack maneuver splashes damage onto an adjacent enemy.
#[derive(Clone, Copy)]
pub enum FollowUpEffect {
    /// Apply a single condition with the given timer. Used by every
    /// Smite-spell rider that produces a debuff (Blinded, Frightened,
    /// Prone, Stunned, Outlined, Burning, ...).
    Condition {
        condition: Condition,
        timer: ConditionTimer,
    },
    /// Push the target `tiles` away from the attacker. Used by the
    /// Battle Master Pushing Attack maneuver. No condition apply —
    /// the displacement IS the effect.
    Push { tiles: u32 },
    /// Splash damage onto one adjacent enemy of the original target.
    /// Used by the Battle Master Sweeping Attack maneuver: the swing's
    /// momentum carries through to a nearby foe for an extra die of
    /// damage. We pick the closest hostile (to the caster) that's
    /// footprint-adjacent to the target and apply the rolled `dice` as
    /// `damage_type`. No save — RAW: the original attack roll is
    /// re-used. If no adjacent enemy exists the effect is a no-op.
    Splash {
        dice: Dice,
        damage_type: DamageType,
    },
}

/// Caster-side on-hit rider table. Every per-hit damage rider that keys
/// off a caster condition (Smite spells, Crown of Stars, Crusader's
/// Mantle, persistent weapon-buff concentration spells, Battle Master
/// maneuvers, etc.) lives here. `Dice::new` is a const fn so the table
/// stays a `const &[..]` — adding a rider doesn't bump a hardcoded
/// length.
const ON_HIT_RIDERS: &[OnHitRider] = &[
        OnHitRider {
            condition: Condition::CrusadersMantled,
            dice: Dice::new(1, 4),
            label: "crusader's mantle",
            damage_type: DamageType::Radiant,
            melee_only: false,
            ranged_only: false,
            consume_on_trigger: false,
            follow_up: None,
        },
        OnHitRider {
            condition: Condition::CrownOfStars,
            dice: Dice::new(1, 8),
            label: "crown of stars",
            damage_type: DamageType::Radiant,
            melee_only: false,
            ranged_only: false,
            consume_on_trigger: false,
            follow_up: None,
        },
        // 5e Bigby's Hand (level-5 concentration). Persistent +1d10 force
        // rider on every attack the caster lands — not melee-only since
        // the spectral hand can punch at range. Slots cleanly between
        // Crown of Stars (1d8 radiant) and the Smiting one-shot primes.
        OnHitRider {
            condition: Condition::BigbysHanded,
            dice: Dice::new(1, 10),
            label: "bigby's hand",
            damage_type: DamageType::Force,
            melee_only: false,
            ranged_only: false,
            consume_on_trigger: false,
            follow_up: None,
        },
        // 5e Spirit Shroud (level-3 concentration). Persistent +1d8 cold
        // rider on every melee swing the holder lands. Mirrors Crown of
        // Stars but cold-typed and melee-only.
        OnHitRider {
            condition: Condition::SpiritShrouded,
            dice: Dice::new(1, 8),
            label: "spirit shroud",
            damage_type: DamageType::Cold,
            melee_only: true,
            ranged_only: false,
            consume_on_trigger: false,
            follow_up: None,
        },
        // 5e Flame Arrows (XGE level-3 transmutation, concentration).
        // Persistent +1d6 fire rider on every ranged swing the holder
        // lands. Mirrors Spirit Shroud's shape but ranged-only — the
        // `ranged_only` gate folds through the rider dispatch so a melee
        // fallback can't burn the buff. Concentration-bound on the
        // caster; persistent (non-consumed) per-hit rider that drops
        // when the caster ends concentration.
        OnHitRider {
            condition: Condition::FlamingArrowed,
            dice: Dice::new(1, 6),
            label: "flame arrows",
            damage_type: DamageType::Fire,
            melee_only: false,
            ranged_only: true,
            consume_on_trigger: false,
            follow_up: None,
        },
        // 5e Tasha's Otherworldly Guise (TCE level-6 concentration). The
        // celestial-form flavor adds +2d6 radiant per melee weapon hit
        // (RAW: "your weapon attacks deal extra radiant damage equal to
        // your CHA mod"; we collapse to a flat 2d6 for the engine's
        // rider-table envelope, sized between Spirit Shroud's 1d8 and
        // Divine Smite's 2d8). Melee-only — the spell flavors the
        // caster's weapon, not their ranged toolkit. Persistent (non-
        // consumed) per-hit rider, concentration-bound on the caster.
        OnHitRider {
            condition: Condition::OtherworldlyGuised,
            dice: Dice::new(2, 6),
            label: "otherworldly guise",
            damage_type: DamageType::Radiant,
            melee_only: true,
            ranged_only: false,
            consume_on_trigger: false,
            follow_up: None,
        },
        // 5e Elemental Weapon (level-3 transmutation, concentration). The
        // weapon is sheathed in elemental energy: +1d4 fire per melee
        // weapon hit. The matching `+1 attack` half rides the standard
        // `attack_bonus_buff` lane via the Elemental Weapon spell's
        // `with_attack_buffs` ledger — kept off the OnHitRider table since
        // that's a damage-only chokepoint. Melee-only (RAW: "the next time
        // you hit a creature with this weapon," but we honor the spell's
        // melee weapon-imbue flavor by gating ranged swings out — distinct
        // from Flame Arrows which exists for the ranged lane). Persistent
        // (non-consumed); drops when the caster ends concentration.
        OnHitRider {
            condition: Condition::ElementallyWeaponed,
            dice: Dice::new(1, 4),
            label: "elemental weapon",
            damage_type: DamageType::Fire,
            melee_only: true,
            ranged_only: false,
            consume_on_trigger: false,
            follow_up: None,
        },
        OnHitRider {
            condition: Condition::Smiting,
            dice: Dice::new(2, 8),
            label: "divine smite",
            damage_type: DamageType::Radiant,
            melee_only: true,
            ranged_only: false,
            consume_on_trigger: true,
            follow_up: None,
        },
        // 5e Searing Smite — 1st-level paladin evocation, bonus action.
        // +1d6 fire on the primed hit, and the target catches fire
        // (Burning) for 3 rounds. We bake the Burning rider in as a
        // SmiteFollowUp with a permissive save (no save in RAW — the
        // target makes ongoing WIS saves to extinguish; we approximate
        // with a flat 3-round Burning).
        OnHitRider {
            condition: Condition::SearingSmiting,
            dice: Dice::new(1, 6),
            label: "searing smite",
            damage_type: DamageType::Fire,
            melee_only: true,
            ranged_only: false,
            consume_on_trigger: true,
            follow_up: Some(SmiteFollowUp {
                // Auto-apply — RAW Searing Smite ignites the target on
                // hit with no save. `save_ability: None` skips the save
                // roll in `apply_smite_follow_up`.
                save_ability: None,
                dc_ability: AbilityScoreType::Charisma,
                effect: FollowUpEffect::Condition {
                    condition: Condition::Burning,
                    timer: ConditionTimer::Rounds(3),
                },
                label: "searing smite ignite",
                hp_threshold: None,
            }),
        },
        // 5e Wrathful Smite — 1st-level. +1d6 psychic on the primed hit;
        // target makes WIS save or is Frightened of the paladin for
        // up to 10 rounds (RAW: 1 minute).
        OnHitRider {
            condition: Condition::WrathfulSmiting,
            dice: Dice::new(1, 6),
            label: "wrathful smite",
            damage_type: DamageType::Psychic,
            melee_only: true,
            ranged_only: false,
            consume_on_trigger: true,
            follow_up: Some(SmiteFollowUp {
                save_ability: Some(AbilityScoreType::Wisdom),
                dc_ability: AbilityScoreType::Charisma,
                effect: FollowUpEffect::Condition {
                    condition: Condition::Frightened,
                    timer: ConditionTimer::Rounds(10),
                },
                label: "wrathful smite fear",
                hp_threshold: None,
            }),
        },
        // 5e Branding Smite — 2nd-level. +2d6 radiant; target glows
        // (Outlined for 10 rounds), giving advantage to attackers and
        // ending Invisibility / Hidden status.
        OnHitRider {
            condition: Condition::BrandingSmiting,
            dice: Dice::new(2, 6),
            label: "branding smite",
            damage_type: DamageType::Radiant,
            melee_only: true,
            ranged_only: false,
            consume_on_trigger: true,
            follow_up: Some(SmiteFollowUp {
                // RAW Branding Smite is auto-apply on hit (no save).
                // `save_ability: None` skips the save roll.
                save_ability: None,
                dc_ability: AbilityScoreType::Charisma,
                effect: FollowUpEffect::Condition {
                    condition: Condition::Outlined,
                    timer: ConditionTimer::Rounds(10),
                },
                label: "branding smite brand",
                hp_threshold: None,
            }),
        },
        // 5e Blinding Smite — 3rd-level. +3d8 radiant; target makes CON
        // save or is Blinded for 10 rounds.
        OnHitRider {
            condition: Condition::BlindingSmiting,
            dice: Dice::new(3, 8),
            label: "blinding smite",
            damage_type: DamageType::Radiant,
            melee_only: true,
            ranged_only: false,
            consume_on_trigger: true,
            follow_up: Some(SmiteFollowUp {
                save_ability: Some(AbilityScoreType::Constitution),
                dc_ability: AbilityScoreType::Charisma,
                effect: FollowUpEffect::Condition {
                    condition: Condition::Blinded,
                    timer: ConditionTimer::Rounds(10),
                },
                label: "blinding smite blind",
                hp_threshold: None,
            }),
        },
        // 5e Monk Stunning Strike — bonus action prime; on the next
        // melee hit, the target makes a CON save vs the monk's
        // WIS-based DC or is Stunned for 1 round. Zero rider dice (the
        // stun *is* the effect); the rider loop's `count > 0` guard
        // skips the damage line.
        OnHitRider {
            condition: Condition::StunningStrike,
            dice: Dice::new(0, 1),
            label: "stunning strike",
            damage_type: DamageType::Bludgeoning,
            melee_only: true,
            ranged_only: false,
            consume_on_trigger: true,
            follow_up: Some(SmiteFollowUp {
                save_ability: Some(AbilityScoreType::Constitution),
                dc_ability: AbilityScoreType::Wisdom,
                effect: FollowUpEffect::Condition {
                    condition: Condition::Stunned,
                    timer: ConditionTimer::Rounds(1),
                },
                label: "stunning strike stun",
                hp_threshold: None,
            }),
        },
        // 5e Cleric Divine Strike (level 8 class feature, here exposed as
        // a Channel-Divinity-flavored bonus-action prime). +1d8 radiant
        // on the next melee weapon hit; no save. Mirrors the Smiting
        // shape but auto-applies no follow-up condition (the radiant
        // rider IS the effect). Consumed the moment a melee swing lands.
        OnHitRider {
            condition: Condition::DivineStriking,
            dice: Dice::new(1, 8),
            label: "divine strike",
            damage_type: DamageType::Radiant,
            melee_only: true,
            ranged_only: false,
            consume_on_trigger: true,
            follow_up: None,
        },
        // 5e Battle Master Trip Attack maneuver. No bonus damage in our
        // model (RAW: +superiority die damage; we skip the die since the
        // existing dice infra doesn't carry a per-class scaling pool);
        // the prone-on-fail-STR-save IS the effect. Mirrors Stunning
        // Strike's "zero-damage rider with a save follow-up" shape.
        OnHitRider {
            condition: Condition::TripAttacking,
            dice: Dice::new(0, 1),
            label: "trip attack",
            damage_type: DamageType::Bludgeoning,
            melee_only: true,
            ranged_only: false,
            consume_on_trigger: true,
            follow_up: Some(SmiteFollowUp {
                save_ability: Some(AbilityScoreType::Strength),
                dc_ability: AbilityScoreType::Strength,
                effect: FollowUpEffect::Condition {
                    condition: Condition::Prone,
                    timer: ConditionTimer::Permanent,
                },
                label: "trip attack prone",
                hp_threshold: None,
            }),
        },
        // 5e Staggering Smite — 4th-level paladin enchantment, bonus
        // action prime. +4d6 psychic on the primed hit; target makes a
        // WIS save vs the caster's CHA-based DC or is Stunned until the
        // end of the paladin's next turn (we model as 1 round). One-shot.
        OnHitRider {
            condition: Condition::StaggeringSmiting,
            dice: Dice::new(4, 6),
            label: "staggering smite",
            damage_type: DamageType::Psychic,
            melee_only: true,
            ranged_only: false,
            consume_on_trigger: true,
            follow_up: Some(SmiteFollowUp {
                save_ability: Some(AbilityScoreType::Wisdom),
                dc_ability: AbilityScoreType::Charisma,
                effect: FollowUpEffect::Condition {
                    condition: Condition::Stunned,
                    timer: ConditionTimer::Rounds(1),
                },
                label: "staggering smite stun",
                hp_threshold: None,
            }),
        },
        // 5e Banishing Smite — 5th-level paladin abjuration, bonus
        // action prime. +5d10 force on the primed hit; if the target
        // ends the swing at 50 HP or fewer they are banished. We model
        // the banishment via the existing `Mazed` envelope (zero
        // movement + blocked action economy + blocked reactions) for
        // 10 rounds — distinct log line, identical end-state. The HP
        // threshold gate is evaluated at the smite-follow-up site (see
        // `apply_smite_follow_up`'s threshold extension below).
        OnHitRider {
            condition: Condition::BanishingSmiting,
            dice: Dice::new(5, 10),
            label: "banishing smite",
            damage_type: DamageType::Force,
            melee_only: true,
            ranged_only: false,
            consume_on_trigger: true,
            follow_up: Some(SmiteFollowUp {
                // No save — RAW: the banish is auto-apply if HP ≤ 50.
                save_ability: None,
                dc_ability: AbilityScoreType::Charisma,
                effect: FollowUpEffect::Condition {
                    condition: Condition::Mazed,
                    timer: ConditionTimer::Rounds(10),
                },
                label: "banishing smite banish",
                // RAW: only banished "if this damage reduces the target
                // to 50 hp or fewer." We honor the gate by predicting
                // post-damage HP at the smite follow-up site.
                hp_threshold: Some(50),
            }),
        },
        // 5e Shillelagh — druid cantrip prime. The caster's club /
        // staff is wreathed in sylvan magic: the next melee weapon hit
        // deals an extra 1d8 force damage. RAW also lets the swing use
        // WIS instead of STR for the to-hit / damage roll; we skip the
        // stat-swap (the +1d8 rider IS the load-bearing buff). One-shot
        // — the rider table strips this flag the moment a melee swing
        // lands. Concentration-free per RAW (1-minute duration).
        OnHitRider {
            condition: Condition::Shillelaghed,
            dice: Dice::new(1, 8),
            label: "shillelagh",
            damage_type: DamageType::Force,
            melee_only: true,
            ranged_only: false,
            consume_on_trigger: true,
            follow_up: None,
        },
        // 5e Thunderous Smite — 1st-level paladin evocation, bonus
        // action prime. +2d6 thunder on the primed hit; target makes a
        // STR save vs the caster's CHA-based DC or is knocked Prone.
        // RAW also pushes 10 ft on the failed save; we collapse the
        // push since the smite follow-up table only carries one
        // condition apply — the prone tag is the load-bearing
        // crowd-control bit and the push helper is reserved for spells
        // whose entire effect is repositioning (Thunderwave).
        OnHitRider {
            condition: Condition::ThunderousSmiting,
            dice: Dice::new(2, 6),
            label: "thunderous smite",
            damage_type: DamageType::Thunder,
            melee_only: true,
            ranged_only: false,
            consume_on_trigger: true,
            follow_up: Some(SmiteFollowUp {
                save_ability: Some(AbilityScoreType::Strength),
                dc_ability: AbilityScoreType::Charisma,
                effect: FollowUpEffect::Condition {
                    condition: Condition::Prone,
                    timer: ConditionTimer::Permanent,
                },
                label: "thunderous smite prone",
                hp_threshold: None,
            }),
        },
        // 5e Enlarge / Reduce (Enlarge half) — 2nd-level transmutation,
        // concentration. The holder rolls +1d4 bonus damage on every
        // weapon attack hit (RAW: melee or ranged). Persistent — the
        // rider doesn't consume_on_trigger; it stays up until the
        // caster drops concentration. Damage type matches the weapon
        // (no fixed magical type) — we model with Bludgeoning as a
        // baseline since the engine doesn't carry per-attack weapon
        // typing into the rider table. Slots between Crusader's Mantle
        // (1d4 radiant) and Spirit Shroud (1d8 cold) in the rider table.
        OnHitRider {
            condition: Condition::Enlarged,
            dice: Dice::new(1, 4),
            label: "enlarge",
            damage_type: DamageType::Bludgeoning,
            melee_only: false,
            ranged_only: false,
            consume_on_trigger: false,
            follow_up: None,
        },
        // 5e Lightning Arrow — 3rd-level ranger evocation, bonus action,
        // concentration. Primes the ranger's next ranged weapon attack
        // with +4d8 lightning. The 10-ft splash (2d8 lightning on
        // adjacent creatures, DEX save for half) is layered on by the
        // spell's `side_effects` at cast time via a follow-up burst — the
        // rider table just carries the per-hit prime damage, mirroring
        // every other Smite-style one-shot. melee_only=false so a
        // longbow swing tags it; consume_on_trigger=true so the first
        // hit consumes the prime.
        OnHitRider {
            condition: Condition::LightningArrowPrimed,
            dice: Dice::new(4, 8),
            label: "lightning arrow",
            damage_type: DamageType::Lightning,
            melee_only: false,
            ranged_only: true,
            consume_on_trigger: true,
            follow_up: None,
        },
        // 5e Holy Weapon — 5th-level paladin evocation, concentration.
        // The caster's weapon glows with radiant light: every weapon
        // attack hit deals an extra 2d8 radiant damage. Persistent (not
        // consumed on trigger) and lane-agnostic — melee or ranged hits
        // both fire the rider. Mirrors the Crusader's Mantle / Spirit
        // Shroud persistent rider shape but tier-5 dice and radiant.
        OnHitRider {
            condition: Condition::HolyWeaponed,
            dice: Dice::new(2, 8),
            label: "holy weapon",
            damage_type: DamageType::Radiant,
            melee_only: false,
            ranged_only: false,
            consume_on_trigger: false,
            follow_up: None,
        },
        // 5e Absorb Elements — 1st-level abjuration, reaction. The caster
        // stores captured elemental energy and releases it on their next
        // melee attack: +1d6 of the absorbed element's damage type. We
        // model the rider as generic Force since the per-element type
        // isn't tracked. One-shot — consumed the moment a melee swing
        // lands. The resistance half is handled by the spell's
        // DamageResistant condition install.
        OnHitRider {
            condition: Condition::AbsorbedElements,
            dice: Dice::new(1, 6),
            label: "absorb elements",
            damage_type: DamageType::Force,
            melee_only: true,
            ranged_only: false,
            consume_on_trigger: true,
            follow_up: None,
        },
        // 5e Battle Master Menacing Attack maneuver. Zero rider damage
        // (RAW: +1 superiority die; we collapse the die since the engine
        // has no per-class scaling pool). On the consuming melee hit the
        // target makes a WIS save vs the fighter's STR-based maneuver DC
        // (8 + prof + STR); on fail, they're Frightened until the end of
        // the fighter's next turn (we model as 1 round). Mirrors Stunning
        // Strike's "no rider damage, save-or-condition" shape.
        OnHitRider {
            condition: Condition::MenacingAttacking,
            dice: Dice::new(0, 1),
            label: "menacing attack",
            damage_type: DamageType::Bludgeoning,
            melee_only: true,
            ranged_only: false,
            consume_on_trigger: true,
            follow_up: Some(SmiteFollowUp {
                save_ability: Some(AbilityScoreType::Wisdom),
                dc_ability: AbilityScoreType::Strength,
                effect: FollowUpEffect::Condition {
                    condition: Condition::Frightened,
                    timer: ConditionTimer::Rounds(1),
                },
                label: "menacing attack fear",
                hp_threshold: None,
            }),
        },
        // 5e Battle Master Disarming Attack maneuver. Zero rider damage
        // (same caveat as Menacing / Trip). On the consuming melee hit the
        // target makes a STR save vs the fighter's STR-based maneuver DC;
        // on fail, they're Disarmed — attack rolls have disadvantage
        // until the start of their next turn (`UntilStartOfNextTurn`
        // mirrors how Mocked / Dodging clear). One-shot.
        OnHitRider {
            condition: Condition::DisarmingAttacking,
            dice: Dice::new(0, 1),
            label: "disarming attack",
            damage_type: DamageType::Bludgeoning,
            melee_only: true,
            ranged_only: false,
            consume_on_trigger: true,
            follow_up: Some(SmiteFollowUp {
                save_ability: Some(AbilityScoreType::Strength),
                dc_ability: AbilityScoreType::Strength,
                effect: FollowUpEffect::Condition {
                    condition: Condition::Disarmed,
                    timer: ConditionTimer::UntilStartOfNextTurn,
                },
                label: "disarming attack disarm",
                hp_threshold: None,
            }),
        },
        // 5e Battle Master Pushing Attack maneuver. Zero rider damage
        // (same caveat as Menacing / Disarming). On the consuming melee
        // hit the target makes a STR save vs the fighter's STR-based
        // maneuver DC; on fail, they're shoved 15 ft (4 tiles in our
        // 2.5ft grid — round 15ft/2.5 = 6, but we cap at the engine's
        // PushActor maximum behavior of 4 tiles, matching Thunderwave's
        // 10ft push). The first follow-up to use the `Push` variant of
        // `FollowUpEffect` — no condition apply, just forced movement.
        OnHitRider {
            condition: Condition::PushingAttacking,
            dice: Dice::new(0, 1),
            label: "pushing attack",
            damage_type: DamageType::Bludgeoning,
            melee_only: true,
            ranged_only: false,
            consume_on_trigger: true,
            follow_up: Some(SmiteFollowUp {
                save_ability: Some(AbilityScoreType::Strength),
                dc_ability: AbilityScoreType::Strength,
                effect: FollowUpEffect::Push { tiles: 4 },
                label: "pushing attack shove",
                hp_threshold: None,
            }),
        },
        // 5e Battle Master Goading Attack maneuver. Zero rider damage
        // (same caveat as Menacing / Disarming / Pushing). On the
        // consuming melee hit the target makes a WIS save vs the
        // fighter's STR-based maneuver DC; on fail, they're Goaded —
        // attack rolls against anyone *other* than the goading fighter
        // are at disadvantage. The follow-up handler also wires the
        // `goaded_by` link via the auto-chained SetGoadedBy emission in
        // `push_follow_up_effect`. Mirrors Compelled Duel's mechanical
        // envelope, but is per-rest rather than concentration-bound.
        OnHitRider {
            condition: Condition::GoadingAttacking,
            dice: Dice::new(0, 1),
            label: "goading attack",
            damage_type: DamageType::Bludgeoning,
            melee_only: true,
            ranged_only: false,
            consume_on_trigger: true,
            follow_up: Some(SmiteFollowUp {
                save_ability: Some(AbilityScoreType::Wisdom),
                dc_ability: AbilityScoreType::Strength,
                effect: FollowUpEffect::Condition {
                    condition: Condition::Goaded,
                    timer: ConditionTimer::UntilStartOfNextTurn,
                },
                label: "goading attack goad",
                hp_threshold: None,
            }),
        },
        // 5e Battle Master Sweeping Attack maneuver. Zero rider damage on
        // the primary target (RAW: damage goes to the secondary creature,
        // not the original); the follow-up's `Splash` variant deals 1d8
        // slashing to one footprint-adjacent enemy of the primary target.
        // `save_ability: None` makes the follow-up auto-apply on hit —
        // RAW: the splash uses the original attack roll, which already
        // hit. If no adjacent enemy exists, the splash is a no-op
        // (logged inside `push_follow_up_effect`).
        OnHitRider {
            condition: Condition::SweepingAttacking,
            dice: Dice::new(0, 1),
            label: "sweeping attack",
            damage_type: DamageType::Slashing,
            melee_only: true,
            ranged_only: false,
            consume_on_trigger: true,
            follow_up: Some(SmiteFollowUp {
                save_ability: None,
                dc_ability: AbilityScoreType::Strength,
                effect: FollowUpEffect::Splash {
                    dice: Dice::new(1, 8),
                    damage_type: DamageType::Slashing,
                },
                label: "sweeping attack splash",
                hp_threshold: None,
            }),
        },
        // 5e Battle Master Distracting Strike maneuver. +1d6 bonus damage
        // on the consuming melee hit (RAW: add the superiority die to
        // the damage roll) plus a no-save auto-apply Distracted tag on
        // the target. The Distracted condition itself carries the
        // load-bearing rider — `compute_attack_mode` grants advantage
        // to any attacker *other* than the fighter, gated via the
        // `distracted_by` link set in the chained `SetDistractedBy`
        // emission inside `push_follow_up_effect`. RAW duration is
        // "until the start of your next turn" — modeled with the
        // `UntilStartOfNextTurn` target-side tick-down envelope shared
        // with Goaded / Mocked / Helped.
        OnHitRider {
            condition: Condition::DistractingAttacking,
            dice: Dice::new(1, 6),
            label: "distracting strike",
            damage_type: DamageType::Slashing,
            melee_only: true,
            ranged_only: false,
            consume_on_trigger: true,
            follow_up: Some(SmiteFollowUp {
                save_ability: None,
                dc_ability: AbilityScoreType::Strength,
                effect: FollowUpEffect::Condition {
                    condition: Condition::Distracted,
                    timer: ConditionTimer::UntilStartOfNextTurn,
                },
                label: "distracting strike distract",
                hp_threshold: None,
            }),
        },
];

/// Build the `SetXBy` side-effect that records the attacker for a
/// linked condition (`Goaded`/`goaded_by`, `Distracted`/`distracted_by`
/// etc.). Thin wrapper over `condition_link_side_effect` —
/// the central dispatch lives in side_effects.rs and serves both the
/// weapon on-hit rider chain (here) AND the direct-cast actions
/// (Compelled Duel, Vow of Enmity). Returns `None` for conditions that
/// don't carry a link — the caller just emits the bare `ApplyCondition`.
fn attacker_link_side_effect(
    condition: Condition,
    target_id: usize,
    caster_id: usize,
) -> Option<Box<dyn ApplicableSideEffect>> {
    crate::engine::side_effects::condition_link_side_effect(condition, target_id, caster_id)
}

/// Process the optional secondary save-and-apply step that some Smite
/// spells stack on top of their bonus damage. `save_ability: None`
/// auto-applies the condition on hit (Branding Smite, Searing Smite's
/// ignite); `Some(ability)` rolls that save against the caster's DC.
/// `total_damage` is the swing's combined weapon + rider damage value —
/// used to predict the target's post-damage HP for `hp_threshold` gates
/// (Banishing Smite RAW: banishes "if this damage reduces the target to
/// 50 hp or fewer"). `0` is fine for follow-ups with no threshold set.
fn apply_smite_follow_up(
    encounter: &mut EncounterInstance,
    effects: &mut Vec<Box<dyn ApplicableSideEffect>>,
    caster_id: usize,
    target_id: usize,
    follow: SmiteFollowUp,
    total_damage: u32,
) {
    // HP-threshold gate (Banishing Smite). Predict post-damage HP as
    // `current_hp - total_damage` and bail if the target would still be
    // above the threshold. Saturating-sub keeps the math clean when the
    // hit would drop them past zero (the threshold still triggers).
    if let Some(threshold) = follow.hp_threshold {
        let Some(target) = encounter.actors.get(&target_id) else {
            return;
        };
        let predicted = target.hitpoints().saturating_sub(total_damage);
        if predicted > threshold {
            encounter.log(format!(
                "  {}: target stays above {} HP threshold (predicted {} HP)",
                follow.label, threshold, predicted
            ));
            return;
        }
    }
    let Some(save_ability) = follow.save_ability else {
        encounter.log(format!("  {}: auto-apply on hit", follow.label));
        push_follow_up_effect(encounter, effects, caster_id, target_id, follow.effect);
        return;
    };
    let Some(caster) = encounter.actors.get(&caster_id) else {
        return;
    };
    let dc = caster.spell_save_dc(follow.dc_ability);
    let save = encounter.roll_save(target_id, save_ability, dc);
    if save.passed() {
        encounter.log(format!("  {}: target saves", follow.label));
        return;
    }
    encounter.log(format!("  {}: target fails save", follow.label));
    push_follow_up_effect(encounter, effects, caster_id, target_id, follow.effect);
}

/// Materialize a `FollowUpEffect` into the side-effect queue. Splits the
/// "what does the rider actually do?" decision from the save / threshold
/// gates above so a future variant (e.g. forced grapple, dispel) plugs
/// in here without re-walking the gates. `caster_id` is the attacker —
/// used by `Push` to anchor the shove on the attacker's tile and by
/// `Splash` to scope the secondary-target search to enemies of the
/// caster.
fn push_follow_up_effect(
    encounter: &mut EncounterInstance,
    effects: &mut Vec<Box<dyn ApplicableSideEffect>>,
    caster_id: usize,
    target_id: usize,
    effect: FollowUpEffect,
) {
    match effect {
        FollowUpEffect::Condition { condition, timer } => {
            effects.push(Box::new(ApplyCondition {
                actor_id: target_id,
                condition,
                timer,
            }));
            // Some conditions carry a back-reference to the attacker
            // (`goaded_by`, `distracted_by`) — chain the matching
            // `SetXBy` side-effect alongside the apply so the link is
            // never stale relative to the condition flag. Single match
            // chokepoint so a new linked condition lands in one place
            // rather than growing another if-let here. Mirrors how
            // Compelled Duel's spell-site pairing emits ApplyCondition
            // (Dueled) + SetDueledBy together — same shape, lifted
            // behind the rider helper so the on-hit path doesn't
            // re-state the chain at every smite-follow-up site.
            if let Some(link) =
                attacker_link_side_effect(condition, target_id, caster_id)
            {
                effects.push(link);
            }
        }
        FollowUpEffect::Push { tiles } => {
            let Some(caster) = encounter.actors.get(&caster_id) else {
                return;
            };
            effects.push(Box::new(PushActor {
                actor_id: target_id,
                from: caster.location(),
                max_tiles: tiles,
            }));
        }
        FollowUpEffect::Splash { dice, damage_type } => {
            // Pick the closest hostile (to the caster) that's
            // footprint-adjacent to the primary target. Closest = lowest
            // (footprint-distance, id) tuple so the choice is
            // deterministic across runs with the same RNG seed.
            let caster_team = match encounter.actors.get(&caster_id) {
                Some(c) => c.team(),
                None => return,
            };
            let mut best: Option<(isize, usize)> = None;
            for (id, other) in encounter.actors.iter() {
                if *id == caster_id || *id == target_id {
                    continue;
                }
                if other.team() == caster_team || !other.is_combat_active() {
                    continue;
                }
                let Some(dist) = encounter.footprint_distance(*id, target_id) else {
                    continue;
                };
                if dist > 0 {
                    continue;
                }
                let dist_from_caster = encounter
                    .footprint_distance(caster_id, *id)
                    .unwrap_or(isize::MAX);
                let key = (dist_from_caster, *id);
                if best.map(|b| key < b).unwrap_or(true) {
                    best = Some(key);
                }
            }
            let Some((_, splash_id)) = best else {
                encounter.log("  sweeping attack: no adjacent enemy to splash".to_string());
                return;
            };
            let rolled = encounter.roll(&dice);
            encounter.log(format!(
                "  sweeping attack: +{} {:?} splashes to {}",
                rolled,
                damage_type,
                encounter.actor_name(splash_id)
            ));
            effects.push(Box::new(DealDamage {
                actor_id: splash_id,
                amount: rolled,
                damage_type,
            }));
        }
    }
}
