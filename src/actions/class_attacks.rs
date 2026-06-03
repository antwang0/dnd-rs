//! Class-flavored attacks. Distinct file from `monster_attacks.rs` so the
//! "vanilla weapon swing" pattern stays in one place and PC class
//! mechanics (sneak attack, smite, etc.) live here.

use std::collections::HashSet;
use std::sync::LazyLock;

use crate::{
    actions::action_template::{Action, MELEE_REACH, TargetingSchema},
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

/// Rogue's Shortsword + Sneak Attack rider. Finesse weapon: uses DEX for
/// attack and damage. Damage 1d6 + DEX. On hit, if the rogue has
/// advantage on the attack roll OR an ally of the rogue is footprint-
/// adjacent to the target (5e: an enemy of the target within 5ft other
/// than the rogue), deals an extra 1d6 sneak-attack damage. Once per
/// turn (the rogue can take more attacks but only one gets the rider).
pub struct RogueShortsword {}

impl Action for RogueShortsword {
    fn name(&self) -> &str {
        "shortsword"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["ss", "stab"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }

    fn reach_tiles(&self) -> Option<isize> {
        Some(MELEE_REACH)
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::engine::types::AbilityScoreType;

        let Some(target_id) = target_ids.and_then(|ids| ids.first().copied()) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dex_mod = caster.ability_modifier(AbilityScoreType::Dexterity);
        let attack_bonus = dex_mod;
        let Some(target_ac) = encounter.actors.get(&target_id).map(|a| a.armor_class() as i32)
        else {
            return Vec::new();
        };

        // Resolve the attack roll up-front so the sneak-attack rider
        // can branch on the same d20 result. weapon_attack would re-
        // roll inside; instead we use the engine's mode-with-riders
        // helper and roll inline so we keep the mode visible.
        let mode = encounter.attack_mode_with_riders(caster_id, target_id, true, true);
        let raw_attack = encounter.roll_d20_lucky(caster_id, mode) as i32;
        let nat_crit = raw_attack >= encounter.crit_threshold(caster_id);
        let total = raw_attack + attack_bonus;
        let hit = nat_crit || total >= target_ac;
        // 5e Paralyzed / Unconscious: any melee hit within 5ft is a crit.
        // Promote *after* deciding hit so a miss stays a miss.
        let is_crit =
            nat_crit || (hit && encounter.target_grants_melee_auto_crit(caster_id, target_id, true));
        let outcome = if is_crit {
            "CRIT!"
        } else if hit {
            "hit"
        } else {
            "miss"
        };
        encounter.log(format!(
            "  shortsword: 1d20({}){:+} = {} vs AC {}{} \u{2014} {}",
            raw_attack,
            attack_bonus,
            total,
            target_ac,
            mode.log_suffix(),
            outcome
        ));
        if !hit {
            return Vec::new();
        }

        // Damage: 1d6 + DEX, doubled on crit (dice only).
        let raw_dmg = encounter.roll(&Dice::new(1, 6)) as i32;
        let crit_extra = if is_crit {
            encounter.roll(&Dice::new(1, 6)) as i32
        } else {
            0
        };
        let mut damage = (raw_dmg + crit_extra + dex_mod).max(0) as u32;

        // Sneak attack rider.
        let sneak_eligible = sneak_attack_eligible(
            encounter,
            caster_id,
            target_id,
            mode == crate::engine::dice::RollMode::Advantage,
        );
        let mut side_effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        if sneak_eligible {
            let level = encounter
                .actors
                .get(&caster_id)
                .map(|a| a.level())
                .unwrap_or(1);
            let total_sneak_dice = sneak_attack_dice_for_level(level);
            // 5e 2024 Cunning Strike: deduct dice from the sneak pool for
            // a tactical effect. Walks the active prime table, picks the
            // first match, returns the deduction + a queued side-effect
            // builder. The cost can't drain the whole pool: if the
            // declared deduction would zero the sneak dice, the prime is
            // refused (RAW: "you can't reduce the number of dice rolled to
            // less than 1"). Side-effects from the consumed prime are
            // appended to the swing's vec below.
            let (sneak_dice, mut cunning_effects) =
                consume_cunning_strike(encounter, caster_id, target_id, total_sneak_dice);
            side_effects.append(&mut cunning_effects);

            let sneak_raw = encounter.roll(&Dice::new(sneak_dice, 6));
            let sneak_extra = if is_crit {
                encounter.roll(&Dice::new(sneak_dice, 6))
            } else {
                0
            };
            let sneak_total = sneak_raw + sneak_extra;
            damage = damage.saturating_add(sneak_total);
            encounter.log(format!(
                "  sneak attack: {}d6({}) = {} extra piercing",
                sneak_dice, sneak_raw, sneak_total
            ));
            if let Some(rogue) = encounter.actors.get_mut(&caster_id) {
                rogue.mark_sneak_attack_used();
            }
        }

        encounter.log(format!(
            "  shortsword: total {} piercing damage{}",
            damage,
            if is_crit { " (crit)" } else { "" }
        ));

        // Primary damage lands first so any condition follow-ups (e.g.
        // the Daze rider's MindWhipped) read the post-damage state.
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = vec![Box::new(DealDamage {
            actor_id: target_id,
            amount: damage,
            damage_type: DamageType::Piercing,
        })];
        effects.append(&mut side_effects);
        effects
    }
}

pub static ROGUE_SHORTSWORD: LazyLock<RogueShortsword> = LazyLock::new(|| RogueShortsword {});

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

/// 5e Sneak Attack trigger:
/// - Rogue has advantage on the attack (and not disadvantage), OR
/// - An ally of the rogue (i.e. another actor on rogue's team, not the
///   rogue) is footprint-adjacent to the target,
/// - AND the rogue hasn't already used Sneak Attack this turn.
///
/// The "no-disadvantage" rider only matters in the ally-adjacent path —
/// the advantage path already implies no disadvantage by definition.
fn sneak_attack_eligible(
    encounter: &EncounterInstance,
    rogue_id: usize,
    target_id: usize,
    has_advantage: bool,
) -> bool {
    let Some(rogue) = encounter.actors.get(&rogue_id) else {
        return false;
    };
    if rogue.sneak_attack_used() {
        return false;
    }
    if has_advantage {
        return true;
    }
    let Some(target) = encounter.actors.get(&target_id) else {
        return false;
    };
    let target_loc = target.location();
    let target_size = get_tiles_from_size(target.size());
    encounter.actors.iter().any(|(id, a)| {
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
    })
}
