use std::collections::HashSet;
use std::sync::LazyLock;

use crate::{
    actions::action_template::{Action, MELEE_REACH, TargetingSchema},
    engine::{
        action_overrides::ActionOverride,
        dice::Dice,
        encounter::EncounterInstance,
        side_effects::{DealDamage, Resource},
        types::{Coordinate, DamageType},
        util::modifier_from_score,
    },
};


pub static SLAM: LazyLock<Slam> = LazyLock::new(|| Slam {});

/// Standard 5e longbow: ranged, requires line-of-sight, +DEX to hit and damage.
/// Reach is in tiles (not feet); 20 tiles = 50ft on this 2.5ft grid, which is
/// short of the 5e 80/320 normal/long range but plenty for our 40×20 maps.
pub struct Longbow {}

impl Action for Longbow {
    fn name(&self) -> &str {
        "longbow"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["bow", "shoot"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }

    fn reach_tiles(&self) -> Option<isize> {
        Some(20)
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
        vec![Resource::Action]
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
        let Some(target_id) = target_ids.and_then(|ids| ids.first().copied()) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dex = caster.ability_score(crate::engine::types::AbilityScoreType::Dexterity);
        // Bows use DEX for both attack and damage in 5e (finesse / ranged).
        let dex_mod = modifier_from_score(dex);
        weapon_attack(
            encounter,
            caster_id,
            target_id,
            WeaponAttackSpec {
                action_name: self.name(),
                attack_bonus: dex_mod,
                damage_dice: Dice::new(1, 8),
                damage_bonus: dex_mod,
                damage_type: DamageType::Piercing,
                is_melee: false,
            },
        )
    }
}

pub static LONGBOW: LazyLock<Longbow> = LazyLock::new(|| Longbow {});

pub struct Slam {}

impl Action for Slam {
    fn name(&self) -> &str {
        "slam"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["slm"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }

    fn reach_tiles(&self) -> Option<isize> {
        Some(MELEE_REACH)
    }

    fn cost(
        &self,
        _encounter: &EncounterInstance,
        _caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        vec![Resource::Action]
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
        let Some(target_id) = target_ids.and_then(|ids| ids.first().copied()) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let str_mod = modifier_from_score(
            caster.ability_score(crate::engine::types::AbilityScoreType::Strength),
        );
        let attack_bonus = caster.attack_bonus();
        weapon_attack(
            encounter,
            caster_id,
            target_id,
            WeaponAttackSpec {
                action_name: self.name(),
                attack_bonus,
                damage_dice: Dice::new(2, 6),
                damage_bonus: str_mod,
                damage_type: DamageType::Bludgeoning,
                is_melee: true,
            },
        )
    }
}

/// Melee attack that, on a hit, forces a STR save (DC 13) or knocks the
/// target prone. Demonstrates the save-then-condition pattern: damage
/// applies regardless, the prone condition only on save failure.
pub struct TripAttack {}

impl Action for TripAttack {
    fn name(&self) -> &str {
        "trip"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["tp"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }

    fn reach_tiles(&self) -> Option<isize> {
        Some(MELEE_REACH)
    }

    fn cost(
        &self,
        _encounter: &EncounterInstance,
        _caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        vec![Resource::Action]
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
        use crate::conditions::Condition;
        use crate::engine::side_effects::ApplyCondition;
        use crate::engine::types::AbilityScoreType;

        let Some(target_id) = target_ids.and_then(|ids| ids.first().copied()) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let str_mod = modifier_from_score(
            caster.ability_score(AbilityScoreType::Strength),
        );
        let attack_bonus = caster.attack_bonus();
        let mut effects = weapon_attack(
            encounter,
            caster_id,
            target_id,
            WeaponAttackSpec {
                action_name: self.name(),
                attack_bonus,
                damage_dice: Dice::new(1, 6),
                damage_bonus: str_mod,
                damage_type: DamageType::Bludgeoning,
                is_melee: true,
            },
        );
        // weapon_attack returns empty Vec on miss — only roll the save if
        // damage was queued (the attack landed).
        if effects.is_empty() {
            return effects;
        }
        let save = encounter.roll_save(target_id, AbilityScoreType::Strength, 13);
        if !save.passed() {
            effects.push(Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Prone,
                // Prone from a trip persists until stand-up clears it.
                timer: crate::conditions::ConditionTimer::Permanent,
            }));
        }
        effects
    }
}

pub static TRIP: LazyLock<TripAttack> = LazyLock::new(|| TripAttack {});

/// Ranged spit attack with splash. Primary uses an attack roll vs AC; on
/// hit deals 1d6 acid to the primary target AND auto-damages every
/// combat-active actor whose footprint is adjacent (gap ≤ 1) to the
/// primary for 1d4 acid. Splash hits *anyone* in range — friendly or foe
/// — except the caster themselves. The splash damage is rolled once and
/// shared among splash victims (5e-style shared area roll).
pub struct AcidSpit {}

impl Action for AcidSpit {
    fn name(&self) -> &str {
        "acid spit"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["spit", "as"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }

    fn reach_tiles(&self) -> Option<isize> {
        Some(8)
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
        vec![Resource::Action]
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
        use crate::engine::types::AbilityScoreType;
        use crate::engine::util::{footprint_chebyshev, get_tiles_from_size};

        let Some(target_id) = target_ids.and_then(|ids| ids.first().copied()) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dex_mod = modifier_from_score(
            caster.ability_score(AbilityScoreType::Dexterity),
        );
        // Primary attack — reuse weapon_attack so logging matches other
        // attacks. weapon_attack returns Vec containing the primary
        // DealDamage on hit, empty on miss.
        let mut effects = weapon_attack(
            encounter,
            caster_id,
            target_id,
            WeaponAttackSpec {
                action_name: self.name(),
                attack_bonus: dex_mod,
                damage_dice: Dice::new(1, 6),
                damage_bonus: 0, // no DEX-to-damage rider; splash is the perk
                damage_type: DamageType::Acid,
                is_melee: false,
            },
        );
        if effects.is_empty() {
            return effects;
        }

        // Splash: snapshot target's location & size, then sweep nearby
        // actors. Sorted by id for deterministic order.
        let Some(target) = encounter.actors.get(&target_id) else {
            return effects;
        };
        let target_loc = target.location();
        let target_size = get_tiles_from_size(target.size());
        let splash_dice = Dice::new(1, 4);
        let splash_amount = encounter.roll(&splash_dice);
        let mut hit_anyone = false;
        let mut ids: Vec<usize> = encounter.actors.keys().copied().collect();
        ids.sort_unstable();
        for sid in ids {
            if sid == caster_id || sid == target_id {
                continue;
            }
            let Some(other) = encounter.actors.get(&sid) else {
                continue;
            };
            if !other.is_combat_active() {
                continue;
            }
            let dist = footprint_chebyshev(
                other.location(),
                get_tiles_from_size(other.size()),
                target_loc,
                target_size,
            );
            // gap ≤ 1 = footprint-adjacent (touching or one tile of clear
            // space). Anyone outside that radius escapes the splash.
            if dist > 1 {
                continue;
            }
            if !hit_anyone {
                hit_anyone = true;
                encounter.log(format!(
                    "  acid spit splash: 1d4({}) = {} acid",
                    splash_amount, splash_amount
                ));
            }
            effects.push(Box::new(DealDamage {
                actor_id: sid,
                amount: splash_amount,
                damage_type: DamageType::Acid,
            }));
        }
        effects
    }
}

pub static ACID_SPIT: LazyLock<AcidSpit> = LazyLock::new(|| AcidSpit {});

/// Scimitar — generic STR-based 1d6 slashing melee attack. Used by
/// goblins and other light melee creatures that don't have a flashy
/// rider effect. Same shape as Slam but slashing instead of bludgeoning.
pub struct Scimitar {}

impl Action for Scimitar {
    fn name(&self) -> &str {
        "scimitar"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["sc"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(MELEE_REACH)
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        vec![Resource::Action]
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
        let Some(target_id) = target_ids.and_then(|ids| ids.first().copied()) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let str_mod = modifier_from_score(
            caster.ability_score(crate::engine::types::AbilityScoreType::Strength),
        );
        let attack_bonus = caster.attack_bonus();
        weapon_attack(
            encounter,
            caster_id,
            target_id,
            WeaponAttackSpec {
                action_name: self.name(),
                attack_bonus,
                damage_dice: Dice::new(1, 6),
                damage_bonus: str_mod,
                damage_type: DamageType::Slashing,
                is_melee: true,
            },
        )
    }
}
pub static SCIMITAR: LazyLock<Scimitar> = LazyLock::new(|| Scimitar {});

/// Shortbow — DEX-based 1d4 piercing ranged attack. Distinguished from
/// Longbow by *bonus-action* economy: meant to be a quick second swing
/// that pairs with an Action attack. Reach 12 tiles (30ft, half of
/// longbow). Demonstrates the BonusAction cost slot, which has been
/// underused.
pub struct Shortbow {}

impl Action for Shortbow {
    fn name(&self) -> &str {
        "shortbow"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["sb-bow", "shoot2"]
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
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        vec![Resource::BonusAction]
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
        let Some(target_id) = target_ids.and_then(|ids| ids.first().copied()) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dex_mod = modifier_from_score(
            caster.ability_score(crate::engine::types::AbilityScoreType::Dexterity),
        );
        weapon_attack(
            encounter,
            caster_id,
            target_id,
            WeaponAttackSpec {
                action_name: self.name(),
                attack_bonus: dex_mod,
                damage_dice: Dice::new(1, 4),
                damage_bonus: dex_mod,
                damage_type: DamageType::Piercing,
                is_melee: false,
            },
        )
    }
}
pub static SHORTBOW: LazyLock<Shortbow> = LazyLock::new(|| Shortbow {});

/// Greatclub — Ogre's signature weapon. STR-based 1d10 bludgeoning, but
/// the headline feature is **reach 2** (10ft), letting Large ogres swing
/// past their footprint. First polearm-style attack in the codebase —
/// exercises footprint_chebyshev > 1 reach validation.
pub struct Greatclub {}

impl Action for Greatclub {
    fn name(&self) -> &str {
        "greatclub"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["gc"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(2)
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        vec![Resource::Action]
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
        let Some(target_id) = target_ids.and_then(|ids| ids.first().copied()) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let str_mod = modifier_from_score(
            caster.ability_score(crate::engine::types::AbilityScoreType::Strength),
        );
        let attack_bonus = caster.attack_bonus();
        // is_melee = true even at reach 2 — polearms are still melee
        // attacks for the prone-target advantage clause.
        weapon_attack(
            encounter,
            caster_id,
            target_id,
            WeaponAttackSpec {
                action_name: self.name(),
                attack_bonus,
                damage_dice: Dice::new(1, 10),
                damage_bonus: str_mod,
                damage_type: DamageType::Bludgeoning,
                is_melee: true,
            },
        )
    }
}
pub static GREATCLUB: LazyLock<Greatclub> = LazyLock::new(|| Greatclub {});

/// Wolf bite — built-in trip rider on every successful hit. STR-based
/// 1d4 piercing; on hit forces a STR save vs DC 11, fail = Prone. Fuses
/// the TripAttack rider pattern into a single creature-canonical action.
pub struct WolfBite {}

impl Action for WolfBite {
    fn name(&self) -> &str {
        "bite"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["bt"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(MELEE_REACH)
    }
    fn cost(
        &self,
        _e: &EncounterInstance,
        _c: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        vec![Resource::Action]
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
        use crate::conditions::{Condition, ConditionTimer};
        use crate::engine::side_effects::ApplyCondition;
        use crate::engine::types::AbilityScoreType;

        let Some(target_id) = target_ids.and_then(|ids| ids.first().copied()) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let str_mod = modifier_from_score(caster.ability_score(AbilityScoreType::Strength));
        let attack_bonus = caster.attack_bonus();
        let mut effects = weapon_attack(
            encounter,
            caster_id,
            target_id,
            WeaponAttackSpec {
                action_name: self.name(),
                attack_bonus,
                damage_dice: Dice::new(1, 4),
                damage_bonus: str_mod,
                damage_type: DamageType::Piercing,
                is_melee: true,
            },
        );
        if effects.is_empty() {
            return effects;
        }
        let save = encounter.roll_save(target_id, AbilityScoreType::Strength, 11);
        if !save.passed() {
            effects.push(Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Prone,
                timer: ConditionTimer::Permanent,
            }));
        }
        effects
    }
}
pub static WOLF_BITE: LazyLock<WolfBite> = LazyLock::new(|| WolfBite {});

/// Wraps another action and runs it `count` times for one Action-slot
/// expenditure. Reach / LOS / targeting schema are inherited from the
/// sub-attack so creatures can declare e.g. `Multiattack { sub: &SLAM, count: 2 }`
/// without restating constraints. Each sub-attack rolls and logs separately,
/// so a zombie's two slams produce two `slam: 1d20...` lines in the log.
pub struct Multiattack {
    pub display_name: &'static str,
    pub sub_attack: &'static (dyn Action + Send + Sync),
    pub count: u32,
}

impl Action for Multiattack {
    fn name(&self) -> &str {
        self.display_name
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["multi", "ma"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        self.sub_attack.targeting_schema()
    }

    fn reach_tiles(&self) -> Option<isize> {
        self.sub_attack.reach_tiles()
    }

    fn requires_los(&self) -> bool {
        self.sub_attack.requires_los()
    }

    fn cost(
        &self,
        _encounter: &EncounterInstance,
        _caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        vec![Resource::Action]
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
        // 5e: every swing of a multiattack lands even if an earlier one
        // killed the target (extra swings tick failed death saves on a
        // dying creature). cleanup_dead_actors only runs *after* all
        // side-effects in this batch are queued, so the target is always
        // present here regardless.
        let mut all = Vec::new();
        for _ in 0..self.count {
            all.extend(self.sub_attack.side_effects(
                encounter,
                caster_id,
                target_ids,
                target_locations,
                overrides,
            ));
        }
        all
    }
}

/// Zombie multiattack: 2 slams per Action. Hits twice as hard as a vanilla
/// zombie at the cost of nothing (this game's zombies are scarier than MM).
pub static ZOMBIE_MULTISLAM: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "multislam",
    sub_attack: &*SLAM,
    count: 2,
});

/// Fire Bolt — DEX-based ranged 1d10 fire attack. Cantrip-style (no spell
/// slot consumed). The fire imp's signature ranged option; chews through
/// flammable parties but bounces off fire-resistant ones.
pub struct FireBolt {}

impl Action for FireBolt {
    fn name(&self) -> &str {
        "fire bolt"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["fb", "bolt"]
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
        vec![Resource::Action]
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
        let Some(target_id) = target_ids.and_then(|ids| ids.first().copied()) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dex_mod = modifier_from_score(
            caster.ability_score(crate::engine::types::AbilityScoreType::Dexterity),
        );
        weapon_attack(
            encounter,
            caster_id,
            target_id,
            WeaponAttackSpec {
                action_name: self.name(),
                attack_bonus: dex_mod,
                damage_dice: Dice::new(1, 10),
                damage_bonus: 0,
                damage_type: DamageType::Fire,
                is_melee: false,
            },
        )
    }
}
pub static FIRE_BOLT: LazyLock<FireBolt> = LazyLock::new(|| FireBolt {});

/// Frightful Presence — bonus action; every enemy within 6 tiles must
/// pass a WIS save vs DC 11 or become Frightened for 3 rounds. The imp's
/// signature pressure tool; spreads disadvantage on attacks across the
/// front line. Already-frightened actors don't re-roll. The save is
/// rolled once per affected actor, in actor-id order for determinism.
pub struct FrightfulPresence {}

impl Action for FrightfulPresence {
    fn name(&self) -> &str {
        "frightful presence"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["fp", "fright"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
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
        vec![Resource::BonusAction]
    }
    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
        use crate::conditions::{Condition, ConditionTimer};
        use crate::engine::side_effects::ApplyCondition;
        use crate::engine::types::AbilityScoreType;
        use crate::engine::util::{footprint_chebyshev, get_tiles_from_size};

        const RADIUS: isize = 6;
        const DC: i32 = 11;

        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let my_team = caster.team();
        let my_loc = caster.location();
        let my_size = get_tiles_from_size(caster.size());

        let mut ids: Vec<usize> = encounter.actors.keys().copied().collect();
        ids.sort_unstable();

        let mut effects: Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> =
            Vec::new();
        for tid in ids {
            let Some(target) = encounter.actors.get(&tid) else {
                continue;
            };
            if tid == caster_id || target.team() == my_team || !target.is_combat_active() {
                continue;
            }
            if target.has_condition(Condition::Frightened) {
                continue;
            }
            let dist = footprint_chebyshev(
                my_loc,
                my_size,
                target.location(),
                get_tiles_from_size(target.size()),
            );
            if dist > RADIUS {
                continue;
            }
            let save = encounter.roll_save(tid, AbilityScoreType::Wisdom, DC);
            if save.passed() {
                continue;
            }
            effects.push(Box::new(ApplyCondition {
                actor_id: tid,
                condition: Condition::Frightened,
                timer: ConditionTimer::Rounds(3),
            }));
        }
        effects
    }
}
pub static FRIGHTFUL_PRESENCE: LazyLock<FrightfulPresence> =
    LazyLock::new(|| FrightfulPresence {});

/// All knobs `weapon_attack` needs beyond the encounter / caster / target
/// trio. Callers fill out one of these per attack flavor (slam, scimitar,
/// fire bolt) and the helper handles the d20-vs-AC dance, Bless folding,
/// crit dice, and damage-type tagged log lines uniformly.
pub(crate) struct WeaponAttackSpec<'a> {
    pub action_name: &'a str,
    pub attack_bonus: i32,
    pub damage_dice: Dice,
    pub damage_bonus: i32,
    pub damage_type: DamageType,
    pub is_melee: bool,
}

/// Roll a d20 attack against the target's AC, log the breakdown, and on
/// a hit roll damage of the given type against the target.
///
/// **Critical hits**: a final d20 of 20 (after advantage / disadvantage)
/// auto-hits regardless of AC and rolls the damage dice twice — the
/// modifier is added once. 5e RAW.
///
/// Returns the side-effect vec (empty on miss). Centralizes the pattern
/// so every weapon-style attack logs in the same shape.
pub(crate) fn weapon_attack(
    encounter: &mut EncounterInstance,
    caster_id: usize,
    target_id: usize,
    spec: WeaponAttackSpec<'_>,
) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
    let Some(target_ac) = encounter
        .actors
        .get(&target_id)
        .map(|a| a.armor_class() as i32)
    else {
        return Vec::new();
    };
    let mode = encounter.compute_attack_mode(caster_id, target_id, spec.is_melee);
    let raw_attack = encounter.roll_d20_with_mode(mode) as i32;
    let bless_bonus = encounter.bless_attack_bonus(caster_id);
    let is_crit = raw_attack == 20;
    let attack_total = raw_attack + spec.attack_bonus + bless_bonus;
    // Crits auto-hit regardless of AC. Otherwise compare normally.
    let hit = is_crit || attack_total >= target_ac;
    let outcome = if is_crit {
        "CRIT!"
    } else if hit {
        "hit"
    } else {
        "miss"
    };
    let bless_suffix = if bless_bonus > 0 {
        format!(" +1d4({})", bless_bonus)
    } else {
        String::new()
    };
    encounter.log(format!(
        "  {}: 1d20({}){:+}{} = {} vs AC {}{} \u{2014} {}",
        spec.action_name,
        raw_attack,
        spec.attack_bonus,
        bless_suffix,
        attack_total,
        target_ac,
        mode.log_suffix(),
        outcome,
    ));
    if !hit {
        return Vec::new();
    }
    let raw_damage = encounter.roll(&spec.damage_dice) as i32;
    let crit_extra = if is_crit {
        encounter.roll(&spec.damage_dice) as i32
    } else {
        0
    };
    let damage = (raw_damage + crit_extra + spec.damage_bonus).max(0) as u32;
    if is_crit {
        encounter.log(format!(
            "  {}: {}({})+{}({}){:+} = {} {:?} damage (crit)",
            spec.action_name,
            spec.damage_dice,
            raw_damage,
            spec.damage_dice,
            crit_extra,
            spec.damage_bonus,
            damage,
            spec.damage_type,
        ));
    } else {
        encounter.log(format!(
            "  {}: {}({}){:+} = {} {:?} damage",
            spec.action_name,
            spec.damage_dice,
            raw_damage,
            spec.damage_bonus,
            damage,
            spec.damage_type,
        ));
    }
    vec![Box::new(DealDamage {
        actor_id: target_id,
        amount: damage,
        damage_type: spec.damage_type,
    })]
}
