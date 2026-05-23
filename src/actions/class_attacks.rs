//! Class-flavored attacks. Distinct file from `monster_attacks.rs` so the
//! "vanilla weapon swing" pattern stays in one place and PC class
//! mechanics (sneak attack, smite, etc.) live here.

use std::collections::HashSet;
use std::sync::LazyLock;

use crate::{
    actions::action_template::{Action, MELEE_REACH, TargetingSchema},
    engine::{
        action_overrides::ActionOverride,
        dice::Dice,
        encounter::EncounterInstance,
        side_effects::{ApplicableSideEffect, DealDamage},
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
        let raw_attack = encounter.roll_d20_with_mode(mode) as i32;
        let is_crit = raw_attack == 20;
        let total = raw_attack + attack_bonus;
        let hit = is_crit || total >= target_ac;
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
        if sneak_eligible {
            let sneak_raw = encounter.roll(&Dice::new(1, 6));
            let sneak_extra = if is_crit {
                encounter.roll(&Dice::new(1, 6))
            } else {
                0
            };
            let sneak_total = sneak_raw + sneak_extra;
            damage = damage.saturating_add(sneak_total);
            encounter.log(format!(
                "  sneak attack: 1d6({}) = {} extra piercing",
                sneak_raw, sneak_total
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

        vec![Box::new(DealDamage {
            actor_id: target_id,
            amount: damage,
            damage_type: DamageType::Piercing,
        })]
    }
}

pub static ROGUE_SHORTSWORD: LazyLock<RogueShortsword> = LazyLock::new(|| RogueShortsword {});

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
