//! Class-flavored attacks. Distinct file from `monster_attacks.rs` so the
//! "vanilla weapon swing" pattern stays in one place and PC class
//! mechanics (sneak attack, smite, etc.) live here.

use std::collections::HashSet;
use std::sync::LazyLock;

use crate::{
    actions::action_template::{Action, MELEE_REACH, TargetingSchema, first_target_id},
    conditions::{Condition, ConditionTimer},
    engine::{
        action_overrides::ActionOverride,
        dice::Dice,
        encounter::EncounterInstance,
        side_effects::{ApplicableSideEffect, ApplyCondition, DealDamage, GiveResource, Resource},
        types::{Coordinate, DamageType},
        util::{footprint_chebyshev, get_tiles_from_size},
    },
};

/// A rogue's weapon: an ordinary attack through the shared pipeline,
/// carrying the Sneak Attack rider.
///
/// Config-driven for the same reason `ManeuverPrime` and `PrimeStrike`
/// are — the rogue's swings differ in name, die, damage type, range and
/// what they cost, and in nothing else. A rogue weapon that did *not*
/// carry Sneak Attack would not be a rogue weapon; the rider is the
/// class, and everything on this struct is the flavour around it.
///
/// Three literals today: the baseline shortsword, and the Soulknife's
/// two psychic blades.
pub struct RogueWeapon {
    /// Display name — action list entry, prompt parser's canonical name,
    /// and the attack log's subject.
    pub name: &'static str,
    /// Alias set for the prompt parser.
    pub aliases: &'static [&'static str],
    /// Damage die. The rogue's own die is the small half of their
    /// damage; the sneak dice are the other, larger half.
    pub dice: Dice,
    /// Damage type of both the swing and its sneak rider — the rider
    /// reads `p.damage_type`, so a psychic blade's sneak damage is
    /// psychic too, which is RAW and also the point of the weapon.
    pub damage_type: DamageType,
    /// Maximum footprint gap. `MELEE_REACH` for a melee weapon; the
    /// thrown blade's 60 ft is 24 tiles on the 2.5 ft grid.
    pub reach: isize,
    /// Whether this counts as a melee swing — read by the rider lanes,
    /// the opportunity-attack predicate and the reach-extension gates.
    pub is_melee: bool,
    /// What the swing costs. `Action` for a primary weapon;
    /// `BonusAction` for the Soulknife's second blade.
    pub cost_resource: Resource,
    /// Extra gate beyond the shared reach / LOS / affordability checks.
    /// `None` for an ordinary weapon; the second psychic blade uses it
    /// to enforce RAW's "immediately after you take the Attack action".
    pub extra_gate: Option<fn(&EncounterInstance, usize) -> bool>,
}

impl Action for RogueWeapon {
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
        Some(self.reach)
    }

    fn requires_los(&self) -> bool {
        // A thrown blade needs to see where it is going; a melee swing
        // is already in contact.
        !self.is_melee
    }

    fn damage_types(&self) -> Vec<DamageType> {
        vec![self.damage_type]
    }

    /// Sneak Attack is left out of the estimate on purpose. It is far
    /// larger than the weapon die and it rides whichever blade the rogue
    /// picks, so counting it would raise every candidate by the same
    /// amount and separate none of them — while making the number look
    /// like a damage prediction rather than the tie-break it is.
    fn expected_damage(&self, encounter: &EncounterInstance, caster_id: usize) -> Option<f32> {
        crate::actions::action_template::weapon_expected_damage(
            encounter,
            caster_id,
            self.dice,
            Some(crate::engine::types::AbilityScoreType::Dexterity),
            self.cost_resource,
            0,
        )
    }

    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        vec![self.cost_resource]
    }

    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        self.extra_gate.is_none_or(|gate| gate(encounter, caster_id))
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::attack::{AttackParams, resolve_attack_with_rider};
        use crate::engine::types::AbilityScoreType;

        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        // Every rogue weapon is finesse or thrown-finesse, so DEX drives
        // both halves of the swing.
        let dex_mod = caster.ability_modifier(AbilityScoreType::Dexterity);
        resolve_attack_with_rider(
            encounter,
            AttackParams {
                caster_id,
                target_id,
                action_name: self.name,
                attack_bonus: dex_mod,
                damage_dice: self.dice,
                damage_bonus: dex_mod,
                damage_type: self.damage_type,
                is_melee: self.is_melee,
                long_range: None,
                is_spell: false,
            },
            &SneakAttack,
        )
    }
}

/// The Sneak Attack half of the rogue's swing, as an `ActionOnHitRider`
/// on the shared attack pipeline.
///
/// This used to be an open-coded attack roll — the shortsword rolled its
/// own d20 so the sneak clause could branch on the same result, and in
/// doing so it opted out of every rule the shared resolver applies. On
/// the baseline Rogue's primary attack, and every rogue subclass's, that
/// meant:
///
///   - a natural 1 did not automatically miss;
///   - cover gave the target no AC;
///   - Bless and Bane did nothing to the roll, and neither did any
///     caster-side attack buff (Sacred Weapon, `+1 Weapon`, Magic
///     Weapon);
///   - the one-shot advantage riders — Help, Hidden, an ally's grant,
///     Invisibility, Bardic Inspiration — were read but never *cleared*,
///     so a Helped rogue kept the grant for the rest of the encounter;
///   - Mirror Image, Illusory Self and Armor of Hexes could not
///     intercept the swing;
///   - Uncanny Dodge, Parry, Deflect Missiles and Interception could not
///     clamp its damage;
///   - Protection, Warding Flare and Entropic Ward never fired;
///   - Sanctuary did not protect the target;
///   - Multiattack Defense never applied its AC penalty, and the swing
///     never recorded itself as a hit for the next one;
///   - Hunter's Mark, Hex, and the Hexblade's Curse damage bonus and
///     expanded crit range all skipped it;
///   - and no smite prime could ride it, which matters for the
///     Arcane Trickster.
///
/// None of that was a decision. It is what an open-coded roll costs, and
/// the cost is invisible at the call site — which is why the fix is to
/// remove the open-coded roll rather than to re-add the twenty missing
/// clauses to it.
///
/// What genuinely needed the special treatment is only this: the die
/// count scales with rogue level, Cunning Strike can trade dice away for
/// a rider effect, and eligibility depends on the attack's roll mode and
/// on who is standing next to the target. That is exactly what an
/// `ActionOnHitRider` carries, and nothing else here does.
struct SneakAttack;

impl crate::engine::attack::ActionOnHitRider for SneakAttack {
    fn apply(
        &self,
        encounter: &mut EncounterInstance,
        p: &crate::engine::attack::AttackParams,
        mode: crate::engine::dice::RollMode,
        is_crit: bool,
        effects: &mut Vec<Box<dyn ApplicableSideEffect>>,
    ) -> u32 {
        if !sneak_attack_eligible(encounter, p.caster_id, p.target_id, mode) {
            return 0;
        }
        let level = encounter
            .actors
            .get(&p.caster_id)
            .map(|a| a.level())
            .unwrap_or(1);
        let total_sneak_dice = sneak_attack_dice_for_level(level);
        // 5e 2024 Cunning Strike: deduct dice from the sneak pool for
        // a tactical effect. Walks the active prime table, picks the
        // first match, returns the deduction + a queued side-effect
        // builder. The cost can't drain the whole pool: if the
        // declared deduction would zero the sneak dice, the prime is
        // refused (RAW: "you can't reduce the number of dice rolled to
        // less than 1").
        let (sneak_dice, mut cunning_effects) =
            consume_cunning_strike(encounter, p.caster_id, p.target_id, total_sneak_dice);

        let sneak_raw = encounter.roll(&Dice::new(sneak_dice, 6));
        let sneak_extra = if is_crit {
            encounter.roll(&Dice::new(sneak_dice, 6))
        } else {
            0
        };
        let sneak_total = sneak_raw + sneak_extra;
        encounter.log(format!(
            "  sneak attack: {}d6({}) = {} extra {:?}",
            sneak_dice, sneak_raw, sneak_total, p.damage_type
        ));
        effects.push(Box::new(DealDamage {
            actor_id: p.target_id,
            amount: sneak_total,
            damage_type: p.damage_type,
        }));
        // Cunning Strike's own payloads land after the damage so a
        // condition follow-up reads the post-damage state.
        effects.append(&mut cunning_effects);
        // 5e Phantom Rogue **Wails from the Grave** — RAW's trigger is
        // "immediately after you deal Sneak Attack damage", which is
        // here and nowhere else, so the feature lives on this rider
        // rather than on a chokepoint of its own.
        push_wails_from_the_grave(encounter, effects, p.caster_id, p.target_id, sneak_total);
        if let Some(rogue) = encounter.actors.get_mut(&p.caster_id) {
            rogue.mark_sneak_attack_used();
        }
        sneak_total
    }
}

/// 5e Phantom Rogue **Wails from the Grave** (subclass level 3): right
/// after Sneak Attack damage lands, a second creature the rogue can see
/// within 30 ft of the first takes half that damage again as necrotic.
///
/// The lane is genuinely new. Sweeping Attack splashes to a creature
/// *adjacent to the target*; every other secondary-damage rider in the
/// engine is either an area centred on a point or a rider on the swing
/// itself. This one reaches across the room from the victim, not from
/// the attacker, and the rogue's own position has nothing to do with
/// which creature is eligible — only with whether they can see it.
///
/// **Target choice: the lowest-HP eligible enemy**, ties broken by the
/// lower id. RAW hands the rogue the choice, and the choice a rogue
/// makes is the one that finishes something: half a sneak pool is 3–5
/// damage at this chassis, which is a rounding error against a healthy
/// ogre and a kill against a bloodied kobold. Deterministic under a
/// fixed seed like every other picker in the engine.
///
/// **Uncapped, where RAW allows proficiency-bonus uses per long rest.**
/// The engine's per-tag charge lane is one use deep, which would make
/// the feature fire once per fight against RAW's three-ish — further
/// from RAW in the other direction. The real limiter is above it
/// anyway: Sneak Attack is once per turn, so this is once per turn too.
///
/// Necrotic regardless of the weapon's type (RAW), which also keeps the
/// wail distinct in the log from the swing that caused it.
fn push_wails_from_the_grave(
    encounter: &mut EncounterInstance,
    effects: &mut Vec<Box<dyn ApplicableSideEffect>>,
    rogue_id: usize,
    primary_id: usize,
    sneak_total: u32,
) {
    use crate::actions::class_features::WAILS_FROM_THE_GRAVE_TAG;
    let carries = encounter
        .actors
        .get(&rogue_id)
        .is_some_and(|a| a.has_passive_feature(WAILS_FROM_THE_GRAVE_TAG));
    if !carries {
        return;
    }
    // Half the Sneak Attack damage, rounded down. A one-die pool that
    // rolled a 1 wails for nothing, which is RAW and is also why the
    // zero case exits before the log line — a "wails from the grave: 0
    // necrotic" entry would be noise on every low roll.
    let wail = sneak_total / 2;
    if wail == 0 {
        return;
    }
    let Some(team) = encounter.actors.get(&rogue_id).map(|a| a.team()) else {
        return;
    };
    // 30 ft RAW = 12 tiles, measured from the *primary target*.
    const WAIL_RANGE: isize = 12;
    let mut best: Option<(u32, usize)> = None;
    for id in encounter.sorted_actor_ids() {
        if id == primary_id || id == rogue_id {
            continue;
        }
        let Some(candidate) = encounter.actors.get(&id) else {
            continue;
        };
        if candidate.team() == team || !candidate.is_combat_active() {
            continue;
        }
        if encounter
            .footprint_distance(primary_id, id)
            .is_none_or(|d| d > WAIL_RANGE)
        {
            continue;
        }
        // RAW: "a creature of your choice that you can see". The sight
        // check is the rogue's, not the victim's.
        if !encounter.actor_has_line_of_sight(rogue_id, id) {
            continue;
        }
        let hp = candidate.hitpoints();
        if best.is_none_or(|(best_hp, _)| hp < best_hp) {
            best = Some((hp, id));
        }
    }
    let Some((_, second_id)) = best else {
        return;
    };
    let name = encounter.actor_name(second_id);
    encounter.log(format!(
        "  wails from the grave: {} takes {} necrotic from the echo.",
        name, wail
    ));
    effects.push(Box::new(DealDamage {
        actor_id: second_id,
        amount: wail,
        damage_type: DamageType::Necrotic,
    }));
}

/// The baseline rogue's shortsword — finesse, 1d6 piercing, melee.
pub static ROGUE_SHORTSWORD: LazyLock<RogueWeapon> = LazyLock::new(|| RogueWeapon {
    name: "shortsword",
    aliases: &["ss", "stab"],
    dice: Dice::new(1, 6),
    damage_type: DamageType::Piercing,
    reach: MELEE_REACH,
    is_melee: true,
    cost_resource: Resource::Action,
    extra_gate: None,
});

/// Psychic Blades — Soulknife Rogue (subclass level 3). A blade of
/// psionic energy manifested in the hand and thrown up to 60 ft, 1d6
/// psychic, finesse.
///
/// The range is what makes the subclass. Every other rogue on the roster
/// has to be in contact to land Sneak Attack — the baseline shortsword,
/// the Assassin's, the Swashbuckler's — and Sneak Attack is where a
/// rogue's damage actually lives. The Soulknife throws it across the
/// room, and the blade is psychic, which almost nothing in the bestiary
/// resists.
pub static PSYCHIC_BLADE: LazyLock<RogueWeapon> = LazyLock::new(|| RogueWeapon {
    name: "psychic blade",
    aliases: &["pb", "blade"],
    dice: Dice::new(1, 6),
    damage_type: DamageType::Psychic,
    // RAW's 60 ft thrown range on the engine's 2.5 ft grid.
    reach: 24,
    is_melee: false,
    cost_resource: Resource::Action,
    extra_gate: None,
});

/// The Soulknife's second blade — RAW: "immediately after you take the
/// Attack action, you can manifest a second psychic blade… as a bonus
/// action", for a smaller die.
///
/// The gate is not decoration. `extra_gate` reads the rogue's remaining
/// Action, so the blade only becomes legal once the Attack action has
/// been spent — which is RAW, and which also keeps the AI's attack
/// picker honest: the picker scores candidate swings by reach and
/// damage-type matchup, not by what they cost, so a legal-at-turn-start
/// 1d4 bonus-action blade would have been chosen over the 1d6 one about
/// half the time.
pub static PSYCHIC_BLADE_FLOURISH: LazyLock<RogueWeapon> = LazyLock::new(|| RogueWeapon {
    name: "second blade",
    aliases: &["sb", "flourish"],
    dice: Dice::new(1, 4),
    damage_type: DamageType::Psychic,
    reach: 24,
    is_melee: false,
    cost_resource: Resource::BonusAction,
    extra_gate: Some(|encounter, caster_id| {
        encounter
            .actors
            .get(&caster_id)
            .is_some_and(|a| a.action_slots() == 0)
    }),
});

/// 5e Sneak Attack dice scaling: ceil(level / 2) d6.
/// Level 1 = 1d6, level 3 = 2d6, level 5 = 3d6, etc.
/// Exposed for cross-module gates (the Cunning Strike (Daze) prime and
/// the AI's Cunning Strike heuristic both check the pool size before
/// committing the bonus action).
pub fn sneak_attack_dice_for_level(level: u32) -> u32 {
    level.div_ceil(2).max(1)
}

/// 5e 2024 Rogue **Cunning Strike** consume site. Walks the active
/// prime conditions on `rogue_id`, picks the first match, deducts the
/// die cost from `total_sneak_dice`, builds the queued side-effects (a
/// save + condition apply for Poison / Trip / Daze, a free half-speed
/// move-resource grant + Disengaging install for Withdraw), and clears
/// the prime via `remove_condition`.
///
/// Returns `(remaining_sneak_dice, side_effects)`. Refuses to consume
/// the prime if doing so would drop the sneak pool below 1 die (RAW:
/// "you can't reduce the number of dice rolled to less than 1"); the
/// prime stays installed so a later swing with a richer pool can
/// connect.
fn consume_cunning_strike(
    encounter: &mut EncounterInstance,
    rogue_id: usize,
    target_id: usize,
    total_sneak_dice: u32,
) -> (u32, Vec<Box<dyn ApplicableSideEffect>>) {
    use crate::engine::types::AbilityScoreType;
    let Some(rogue) = encounter.actors.get(&rogue_id) else {
        return (total_sneak_dice, Vec::new());
    };
    // First-match wins. Order is fixed (Poison → Trip → Daze → Withdraw)
    // so the consume order is deterministic for tests; the prime-install
    // gates prevent double-priming so only one condition is ever active
    // anyway.
    const PRIORITY: &[Condition] = &[
        Condition::CunningStrikePoison,
        Condition::CunningStrikeTrip,
        Condition::CunningStrikeDaze,
        Condition::CunningStrikeWithdraw,
    ];
    let Some(prime) = PRIORITY.iter().copied().find(|c| rogue.has_condition(*c))
    else {
        return (total_sneak_dice, Vec::new());
    };
    let cost = match prime {
        Condition::CunningStrikeDaze => 2,
        _ => 1,
    };
    if total_sneak_dice <= cost {
        // RAW: can't reduce sneak below 1 die. Leave the prime up — the
        // rogue will get another chance next swing or next round.
        return (total_sneak_dice, Vec::new());
    }
    let remaining = total_sneak_dice - cost;
    let dc = rogue.spell_save_dc(AbilityScoreType::Dexterity);
    let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
    match prime {
        Condition::CunningStrikePoison => {
            encounter.log(format!(
                "  cunning strike (poison): -1d6 sneak \u{2192} CON save vs DC {}",
                dc
            ));
            let save = encounter.roll_save(target_id, AbilityScoreType::Constitution, dc);
            if !save.passed() {
                encounter.log("  cunning strike (poison): target is poisoned.");
                effects.push(Box::new(ApplyCondition {
                    actor_id: target_id,
                    condition: Condition::Poisoned,
                    timer: ConditionTimer::Rounds(10),
                }));
            }
        }
        Condition::CunningStrikeTrip => {
            // Trip RAW: target must be Large or smaller. Sized cap mirrors
            // Shove / Grapple's footprint gate.
            let target_too_big = encounter
                .actors
                .get(&target_id)
                .is_some_and(|t| t.size().ordinal() > crate::engine::types::Size::Large.ordinal());
            if target_too_big {
                encounter
                    .log("  cunning strike (trip): target too large to knock down.".to_string());
            } else {
                encounter.log(format!(
                    "  cunning strike (trip): -1d6 sneak \u{2192} DEX save vs DC {}",
                    dc
                ));
                let save = encounter.roll_save(target_id, AbilityScoreType::Dexterity, dc);
                if !save.passed() {
                    encounter.log("  cunning strike (trip): target is knocked prone.");
                    effects.push(Box::new(ApplyCondition {
                        actor_id: target_id,
                        condition: Condition::Prone,
                        timer: ConditionTimer::Permanent,
                    }));
                }
            }
        }
        Condition::CunningStrikeDaze => {
            encounter.log(format!(
                "  cunning strike (daze): -2d6 sneak \u{2192} CON save vs DC {}",
                dc
            ));
            let save = encounter.roll_save(target_id, AbilityScoreType::Constitution, dc);
            if !save.passed() {
                encounter.log(
                    "  cunning strike (daze): target loses their next action and reaction.",
                );
                // MindWhipped: action-economy clip on next turn.
                // NoReaction: blocks reactions until start of next turn.
                effects.push(Box::new(ApplyCondition {
                    actor_id: target_id,
                    condition: Condition::MindWhipped,
                    timer: ConditionTimer::Rounds(1),
                }));
                effects.push(Box::new(ApplyCondition {
                    actor_id: target_id,
                    condition: Condition::NoReaction,
                    timer: ConditionTimer::Rounds(1),
                }));
            }
        }
        Condition::CunningStrikeWithdraw => {
            // RAW: "Immediately after the attack, you can move up to half
            // your Speed without provoking opportunity attacks."
            // Model: grant Movement budget for half speed + install
            // Disengaging so the granted budget skips OAs.
            let half_speed = rogue.speed() / 2.0;
            encounter.log(format!(
                "  cunning strike (withdraw): -1d6 sneak \u{2192} +{:.1}ft of OA-free movement.",
                half_speed
            ));
            effects.push(Box::new(GiveResource {
                actor_id: rogue_id,
                resource: Resource::Movement(half_speed),
            }));
            effects.push(Box::new(ApplyCondition {
                actor_id: rogue_id,
                condition: Condition::Disengaging,
                timer: ConditionTimer::UntilStartOfNextTurn,
            }));
        }
        _ => {}
    }
    // Burn the prime regardless of save outcome — RAW: "the die cost is
    // subtracted whether or not the effect lands."
    if let Some(rogue) = encounter.actors.get_mut(&rogue_id) {
        rogue.remove_condition(prime);
    }
    (remaining, effects)
}

/// 5e Sneak Attack trigger. Fires when:
///   - The rogue has advantage on the attack roll (RAW's canonical
///     surprise / help / hidden path — disadvantage cancels advantage
///     via `RollMode::combine` so the branch trivially subsumes RAW's
///     "no disadvantage" clause). OR
///   - An ally of the rogue is footprint-adjacent to the target AND
///     the rogue's attack is not at disadvantage. RAW: "another enemy
///     of the target is within 5 feet of it, that enemy isn't
///     incapacitated, and you don't have disadvantage on the attack
///     roll". The incapacitated-ally check collapses to the
///     `is_combat_active` gate. OR
///   - The rogue holds `has_rakish_audacity` (Swashbuckler subclass
///     level 3) AND the target is footprint-adjacent to the rogue AND
///     no OTHER creature is within 5 ft of the rogue AND the attack
///     is not at disadvantage. RAW: "you don't need advantage on the
///     attack roll to use your Sneak Attack against a creature if you
///     are within 5 feet of it, no other creatures are within 5 feet
///     of you, and you don't have disadvantage on the attack roll".
///
/// AND the rogue hasn't already used Sneak Attack this turn.
///
/// Takes the full `RollMode` (not just `has_advantage`) so the ally-
/// adjacent and Rakish Audacity paths can enforce RAW's "no
/// disadvantage" clause uniformly — a rogue swinging at Disadvantage
/// (Blinded, Poisoned, prone-target-melee, etc.) no longer picks up
/// Sneak Attack via either path.
fn sneak_attack_eligible(
    encounter: &EncounterInstance,
    rogue_id: usize,
    target_id: usize,
    mode: crate::engine::dice::RollMode,
) -> bool {
    let Some(rogue) = encounter.actors.get(&rogue_id) else {
        return false;
    };
    if rogue.sneak_attack_used() {
        return false;
    }
    if mode == crate::engine::dice::RollMode::Advantage {
        return true;
    }
    // Both remaining paths (ally-adjacent, Rakish Audacity solo-duelist)
    // require the swing to not be at disadvantage per RAW.
    if mode == crate::engine::dice::RollMode::Disadvantage {
        return false;
    }
    let Some(target) = encounter.actors.get(&target_id) else {
        return false;
    };
    let target_loc = target.location();
    let target_size = get_tiles_from_size(target.size());
    // Ally-adjacent path — the classic "flanker enables sneak" trigger.
    let ally_adjacent = encounter.actors.iter().any(|(id, a)| {
        if *id == rogue_id || a.team() != rogue.team() || !a.is_combat_active() {
            return false;
        }
        let dist = footprint_chebyshev(
            a.location(),
            get_tiles_from_size(a.size()),
            target_loc,
            target_size,
        );
        // 5ft adjacency = footprint-Chebyshev gap of 0 (touching).
        dist == 0
    });
    if ally_adjacent {
        return true;
    }
    // Rakish Audacity path (Swashbuckler subclass level 3) — the rogue
    // stands alone with the target: target footprint-adjacent, no other
    // hostile creature footprint-adjacent to the rogue. "Other creatures"
    // in RAW means anyone besides the rogue and the target — a nearby
    // ally does NOT gate the trigger since the ally-adjacent path above
    // would have already fired. We check for "no other enemies within
    // 5 ft of the rogue" — matching the RAW spirit that the swash
    // duels one target without interference from other threats.
    if rogue.has_rakish_audacity() {
        let rogue_loc = rogue.location();
        let rogue_size = get_tiles_from_size(rogue.size());
        // Target must be adjacent to the swash.
        let target_adjacent = footprint_chebyshev(rogue_loc, rogue_size, target_loc, target_size)
            == 0;
        // No OTHER hostile creature within 5 ft of the swash. RAW says
        // "no other creatures within 5 feet of you"; we scope to
        // hostile-to-swash to avoid a swash allied with the flanking
        // rogue's cover from being locked out of Rakish Audacity when
        // the ally is nearby. This is a mild deviation from strict RAW
        // (which counts any creature) but matches the RAW spirit — the
        // swash duels the target one-on-one, ally presence doesn't
        // cancel the "solo duel" identity.
        let no_other_enemy_adjacent = !encounter.actors.iter().any(|(id, a)| {
            if *id == rogue_id || *id == target_id || !a.is_combat_active() {
                return false;
            }
            if a.team() == rogue.team() {
                return false;
            }
            let dist =
                footprint_chebyshev(a.location(), get_tiles_from_size(a.size()), rogue_loc, rogue_size);
            dist == 0
        });
        if target_adjacent && no_other_enemy_adjacent {
            return true;
        }
    }
    false
}

/// The beast form's Strength modifier, and the reason this attack has
/// its own impl instead of being a `SimpleWeapon` declaration.
///
/// RAW's Wild Shape is explicit: "your game statistics are replaced by
/// the statistics of the beast." Every other weapon in the engine reads
/// its to-hit and damage modifiers off the *wielder*, which is right
/// for a scimitar and wrong for a bear — a Wisdom-primary druid with
/// STR 10 wielding a brown bear's claws at +0 is not the feature. So
/// the modifier is a constant of the form rather than a lookup on the
/// actor: +4, the brown bear's Strength 19.
///
/// The brown bear is the reference form because Circle of the Moon's
/// **Circle Forms** caps the druid at CR 1 at subclass level 2, and the
/// bear is the CR-1 beast the class is famous for taking. Its claws are
/// 2d6+4 slashing, which is what `BEAST_FORM_CLAWS` deals.
///
/// Proficiency still comes from the druid — RAW keeps the character's
/// proficiency bonus through the transformation — so a higher-level
/// moon druid's claws get more accurate without the form changing.
const BEAST_FORM_STR_MOD: i32 = 4;

/// Beast-form claws — the natural weapon a Wild Shaped druid swings.
/// 2d6+4 slashing at melee reach, to-hit at the druid's proficiency
/// bonus plus the form's +4 Strength.
///
/// Gated on `Condition::WildShaped`, which is the whole reason it can
/// sit on the druid's action list permanently: out of form the
/// validator refuses it, in form it is the only attack that matters
/// (the scimitar is still legal but strictly worse, and the spell list
/// is locked out entirely by `blocks_spell_slots`).
///
/// Routes through the shared `resolve_attack` chokepoint rather than
/// rolling inline the way `RogueShortsword` does, so every rider that
/// keys off a weapon hit — Extra Attack chaining is the one that
/// matters here, but also the once-per-turn die cohort and the
/// Eldritch Strike mark — sees the swing as the weapon attack it is.
pub struct BeastFormClaws {}

impl Action for BeastFormClaws {
    fn name(&self) -> &str {
        "beast claws"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["bc", "maul"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }

    fn reach_tiles(&self) -> Option<isize> {
        Some(MELEE_REACH)
    }

    fn damage_types(&self) -> Vec<DamageType> {
        vec![DamageType::Slashing]
    }

    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        encounter
            .actors
            .get(&caster_id)
            .is_some_and(|a| a.has_condition(Condition::WildShaped))
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
        let Some(prof) = encounter
            .actors
            .get(&caster_id)
            .map(|a| a.proficiency_bonus())
        else {
            return Vec::new();
        };
        // The form's Strength drives both halves; the druid contributes
        // only their proficiency bonus, per RAW's "you retain your
        // proficiency bonuses" clause.
        let swing = |e: &mut EncounterInstance| {
            resolve_attack(
                e,
                AttackParams {
                    caster_id,
                    target_id,
                    action_name: "beast claws",
                    attack_bonus: prof + BEAST_FORM_STR_MOD,
                    damage_dice: Dice::new(2, 6),
                    damage_bonus: BEAST_FORM_STR_MOD,
                    damage_type: DamageType::Slashing,
                    is_melee: true,
                    long_range: None,
                    is_spell: false,
                },
            )
        };
        let mut effects = swing(encounter);
        crate::actions::monster_attacks::maybe_chain_extra_attack(
            encounter,
            caster_id,
            &mut effects,
            swing,
        );
        effects
    }
}

pub static BEAST_FORM_CLAWS: LazyLock<BeastFormClaws> = LazyLock::new(|| BeastFormClaws {});

/// What a Path of the Beast natural weapon does beyond rolling its die.
///
/// The three forms differ in exactly one clause each — the bite heals,
/// the claws swing again, the tail reaches further — and the tail's
/// difference is already carried by `BeastNaturalWeapon::reach`. So the
/// enum names the two riders and a "nothing else" arm rather than
/// splitting the weapon into three near-identical structs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BeastFormRider {
    /// **Bite**: once per turn, a landed bite on a barbarian who is
    /// below half their hit points heals them for their proficiency
    /// bonus.
    BloodiedHeal,
    /// **Claws**: one additional swing as part of the same Attack
    /// action, on top of Extra Attack.
    ExtraSwing,
    /// **Tail**: nothing beyond the reach the weapon already declares.
    ReachOnly,
}

/// One of the three natural weapons a Path of the Beast barbarian
/// manifests while raging (TCE, subclass level 3).
///
/// Config-driven for the same reason `RogueWeapon` is: the three forms
/// differ in name, die, damage type, reach and one rider clause, and in
/// nothing else. Spelling them out as three `impl Action` blocks would
/// have triplicated the rage gate, the STR derivation and the Extra
/// Attack chain — the three things that must not drift between them.
///
/// Both gates are checked, not just the rage one. `Raging` is RAW's
/// trigger (the form manifests when the barbarian rages and vanishes
/// when the rage ends), and `feature_tag` is what makes the three forms
/// mutually exclusive: a barbarian who somehow carried two of these
/// actions could still only swing the one whose tag their template
/// grants.
pub struct BeastNaturalWeapon {
    /// Display name — action list entry, prompt parser's canonical
    /// name, and the attack log's subject.
    pub name: &'static str,
    /// Alias set for the prompt parser.
    pub aliases: &'static [&'static str],
    /// Damage die.
    pub dice: Dice,
    /// Damage type of the swing.
    pub damage_type: DamageType,
    /// Maximum footprint gap, in tile-gap units. `MELEE_REACH` for the
    /// bite and claws; `2` for the tail's 10 ft.
    pub reach: isize,
    /// The Form of the Beast tag this weapon belongs to. Gated on as
    /// well as declared, so the forms stay exclusive.
    pub feature_tag: &'static str,
    /// The form's one extra clause.
    pub rider: BeastFormRider,
}

impl Action for BeastNaturalWeapon {
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
        Some(self.reach)
    }

    fn damage_types(&self) -> Vec<DamageType> {
        vec![self.damage_type]
    }

    /// The claws' extra swing is the whole reason this hint exists. On
    /// dice alone the greataxe wins every comparison — 1d12 against 1d6
    /// — and on dice plus the Strength modifier, three claw swings
    /// (3 x 7.5 = 22.5) edge out two axe swings (2 x 10.5 = 21). A
    /// per-swing estimate would report the opposite.
    fn expected_damage(&self, encounter: &EncounterInstance, caster_id: usize) -> Option<f32> {
        crate::actions::action_template::weapon_expected_damage(
            encounter,
            caster_id,
            self.dice,
            Some(crate::engine::types::AbilityScoreType::Strength),
            Resource::Action,
            u32::from(self.rider == BeastFormRider::ExtraSwing),
        )
    }

    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        encounter.actors.get(&caster_id).is_some_and(|a| {
            a.has_condition(Condition::Raging) && a.has_passive_feature(self.feature_tag)
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
        use crate::engine::attack::{AttackParams, resolve_attack_with_rider};
        use crate::engine::types::AbilityScoreType;

        let Some(target_id) = first_target_id(target_ids) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        // Natural weapons are Strength weapons: STR to hit (with
        // proficiency — RAW counts them as simple weapons the barbarian
        // is proficient with) and STR to damage.
        let attack_bonus = caster.spell_attack_modifier(AbilityScoreType::Strength);
        let damage_bonus = caster.ability_modifier(AbilityScoreType::Strength);
        let bite_heal = BloodiedBite;
        let mut swing = |e: &mut EncounterInstance| {
            resolve_attack_with_rider(
                e,
                AttackParams {
                    caster_id,
                    target_id,
                    action_name: self.name,
                    attack_bonus,
                    damage_dice: self.dice,
                    damage_bonus,
                    damage_type: self.damage_type,
                    is_melee: true,
                    long_range: None,
                    is_spell: false,
                },
                // The rider no-ops on the forms that don't hold the bite
                // tag, so all three swings can share one call site.
                &bite_heal,
            )
        };
        let mut effects = swing(encounter);
        crate::actions::monster_attacks::maybe_chain_extra_attack(
            encounter,
            caster_id,
            &mut effects,
            &mut swing,
        );
        // RAW: "you can make one additional attack with them as part of
        // the Attack action". Suppressed inside a Multiattack expansion
        // for the same reason Extra Attack is — the wrapper already
        // encodes the swing count.
        if self.rider == BeastFormRider::ExtraSwing && !encounter.in_multiattack() {
            encounter.log("  Form of the Beast (claws): additional claw:");
            effects.extend(swing(encounter));
        }
        effects
    }
}

/// The Bite's self-heal, as an `ActionOnHitRider` on the shared attack
/// pipeline.
///
/// RAW: "if you're missing any of your hit points when you hit a
/// creature with it, you regain hit points equal to your proficiency
/// bonus" — with TCE's once-per-turn cap. We read "missing hit points"
/// as *below half*, which is where the 2024 reprint puts it and which is
/// also the only reading that makes the form a comeback rather than a
/// permanent trickle: an unhurt barbarian who takes 1 damage would
/// otherwise heal it back on their next swing, every turn, forever.
///
/// Returns 0 extra damage — the bite heals the barbarian, it does not
/// hit the target harder — but rides the shared pipeline anyway so the
/// heal only fires on a swing that actually connected, and fires after
/// the pipeline's own riders have had their say.
struct BloodiedBite;

impl crate::engine::attack::ActionOnHitRider for BloodiedBite {
    fn apply(
        &self,
        encounter: &mut EncounterInstance,
        p: &crate::engine::attack::AttackParams,
        _mode: crate::engine::dice::RollMode,
        _is_crit: bool,
        effects: &mut Vec<Box<dyn ApplicableSideEffect>>,
    ) -> u32 {
        use crate::actions::class_features::FORM_OF_THE_BEAST_BITE_TAG;
        use crate::engine::side_effects::Heal;
        let Some(barbarian) = encounter.actors.get(&p.caster_id) else {
            return 0;
        };
        if !barbarian.has_passive_feature(FORM_OF_THE_BEAST_BITE_TAG)
            || barbarian.once_per_turn_used(FORM_OF_THE_BEAST_BITE_TAG)
        {
            return 0;
        }
        // Strictly below half, so a barbarian at exactly half is not yet
        // hurt enough. `* 2` rather than `/ 2` keeps the odd-max case
        // honest: 25 of 51 is below half, 26 is not.
        if barbarian.hitpoints() * 2 >= barbarian.max_hitpoints() {
            return 0;
        }
        let heal = barbarian.proficiency_bonus().max(0) as u32;
        if heal == 0 {
            return 0;
        }
        if let Some(barbarian) = encounter.actors.get_mut(&p.caster_id) {
            barbarian.mark_once_per_turn_used(FORM_OF_THE_BEAST_BITE_TAG);
        }
        encounter.log(format!(
            "  form of the beast (bite): the kill feeds the rage, {} HP back.",
            heal
        ));
        effects.push(Box::new(Heal {
            actor_id: p.caster_id,
            amount: heal,
        }));
        0
    }
}

/// Form of the Beast — **Bite**. 1d8 piercing, and the barbarian's only
/// self-heal.
pub static BEAST_BITE: LazyLock<BeastNaturalWeapon> = LazyLock::new(|| BeastNaturalWeapon {
    name: "bite",
    aliases: &["bt", "maw"],
    dice: Dice::new(1, 8),
    damage_type: DamageType::Piercing,
    reach: MELEE_REACH,
    feature_tag: crate::actions::class_features::FORM_OF_THE_BEAST_BITE_TAG,
    rider: BeastFormRider::BloodiedHeal,
});

/// Form of the Beast — **Claws**. 1d6 slashing, three swings a turn on
/// the level-9 chassis.
pub static BEAST_CLAWS: LazyLock<BeastNaturalWeapon> = LazyLock::new(|| BeastNaturalWeapon {
    name: "claws",
    aliases: &["cl", "rake"],
    dice: Dice::new(1, 6),
    damage_type: DamageType::Slashing,
    reach: MELEE_REACH,
    feature_tag: crate::actions::class_features::FORM_OF_THE_BEAST_CLAWS_TAG,
    rider: BeastFormRider::ExtraSwing,
});

/// Form of the Beast — **Tail**. 1d8 piercing at 10 ft, the only reach
/// weapon on the barbarian chassis.
pub static BEAST_TAIL: LazyLock<BeastNaturalWeapon> = LazyLock::new(|| BeastNaturalWeapon {
    name: "tail",
    aliases: &["tl", "lash"],
    dice: Dice::new(1, 8),
    damage_type: DamageType::Piercing,
    // RAW's 10 ft reach on the engine's 2.5 ft grid, in footprint-gap
    // units — the same `2` every reach weapon in `monster_attacks` uses.
    reach: 2,
    feature_tag: crate::actions::class_features::FORM_OF_THE_BEAST_TAIL_TAG,
    rider: BeastFormRider::ReachOnly,
});
