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
        // Bows use DEX for both attack and damage in 5e (finesse / ranged).
        simple_weapon_attack(
            encounter,
            caster_id,
            target_ids,
            self.name(),
            crate::engine::types::AbilityScoreType::Dexterity,
            Dice::new(1, 8),
            DamageType::Piercing,
            false,
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
        simple_weapon_attack(
            encounter,
            caster_id,
            target_ids,
            self.name(),
            crate::engine::types::AbilityScoreType::Strength,
            Dice::new(2, 6),
            DamageType::Bludgeoning,
            true,
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

        let target_id = target_ids.and_then(|ids| ids.first().copied());
        let mut effects = simple_weapon_attack(
            encounter,
            caster_id,
            target_ids,
            self.name(),
            AbilityScoreType::Strength,
            Dice::new(1, 6),
            DamageType::Bludgeoning,
            true,
        );
        // simple_weapon_attack returns empty Vec on miss — only roll
        // the save if damage was queued (the attack landed).
        if effects.is_empty() {
            return effects;
        }
        let Some(target_id) = target_id else { return effects };
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
        let attack_bonus = caster.ability_attack_bonus(AbilityScoreType::Dexterity);
        let Some(target_ac) = encounter.actors.get(&target_id).map(|a| a.armor_class() as i32)
        else {
            return Vec::new();
        };

        // Primary attack — reuse weapon_attack so logging matches other
        // attacks. weapon_attack returns Vec containing the primary
        // DealDamage on hit, empty on miss.
        let mut effects = weapon_attack(
            encounter,
            caster_id,
            target_id,
            self.name(),
            attack_bonus,
            target_ac,
            Dice::new(1, 6),
            0, // no DEX-to-damage rider; keep splash potential as the perk
            DamageType::Acid,
            false, // ranged
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
        simple_weapon_attack(
            encounter,
            caster_id,
            target_ids,
            self.name(),
            crate::engine::types::AbilityScoreType::Strength,
            Dice::new(1, 6),
            DamageType::Slashing,
            true,
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
        simple_weapon_attack(
            encounter,
            caster_id,
            target_ids,
            self.name(),
            crate::engine::types::AbilityScoreType::Dexterity,
            Dice::new(1, 4),
            DamageType::Piercing,
            false,
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
        // is_melee = true even at reach 2 — polearms are still melee
        // attacks for the prone-target advantage clause.
        simple_weapon_attack(
            encounter,
            caster_id,
            target_ids,
            self.name(),
            crate::engine::types::AbilityScoreType::Strength,
            Dice::new(1, 10),
            DamageType::Bludgeoning,
            true,
        )
    }
}
pub static GREATCLUB: LazyLock<Greatclub> = LazyLock::new(|| Greatclub {});

/// Greataxe — Orc-flavored heavy two-hander. STR-based 1d12 slashing,
/// melee reach. Hits harder than a longsword on a single die; pairs
/// with the orc's high STR for a punishing single-attack profile.
pub struct Greataxe {}

impl Action for Greataxe {
    fn name(&self) -> &str {
        "greataxe"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["ga"]
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
        simple_weapon_attack(
            encounter,
            caster_id,
            target_ids,
            self.name(),
            crate::engine::types::AbilityScoreType::Strength,
            Dice::new(1, 12),
            DamageType::Slashing,
            true,
        )
    }
}
pub static GREATAXE: LazyLock<Greataxe> = LazyLock::new(|| Greataxe {});

/// Heavy Crossbow — DEX-based 1d10 piercing ranged. Differs from the
/// Longbow in damage die (1d10 vs 1d8) and conceptually loading time
/// (we don't model the loading property today). Used by bandits.
pub struct HeavyCrossbow {}

impl Action for HeavyCrossbow {
    fn name(&self) -> &str {
        "heavy crossbow"
    }
    fn aliases(&self) -> Vec<&str> {
        vec!["hcb", "crossbow"]
    }
    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }
    fn reach_tiles(&self) -> Option<isize> {
        Some(16)
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
        simple_weapon_attack(
            encounter,
            caster_id,
            target_ids,
            self.name(),
            crate::engine::types::AbilityScoreType::Dexterity,
            Dice::new(1, 10),
            DamageType::Piercing,
            false,
        )
    }
}
pub static HEAVY_CROSSBOW: LazyLock<HeavyCrossbow> = LazyLock::new(|| HeavyCrossbow {});

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

        let target_id = target_ids.and_then(|ids| ids.first().copied());
        let mut effects = simple_weapon_attack(
            encounter,
            caster_id,
            target_ids,
            self.name(),
            AbilityScoreType::Strength,
            Dice::new(1, 4),
            DamageType::Piercing,
            true,
        );
        if effects.is_empty() {
            return effects;
        }
        let Some(target_id) = target_id else { return effects };
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
        encounter: &EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        // Inherit the sub-attack's cost shape so a `Multiattack { sub:
        // SHORTBOW }` correctly costs a BonusAction, not an Action. We
        // filter out Movement (sub-attacks shouldn't charge per-swing).
        self.sub_attack
            .cost(encounter, caster_id, target_ids, target_locations, overrides)
            .into_iter()
            .filter(|r| !matches!(r, Resource::Movement(_)))
            .collect()
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

/// Common scaffolding for ability-based weapon attacks: extracts target +
/// caster, computes the attack/damage modifiers from `ability` (also
/// applies proficiency for the attack roll), then defers to
/// `weapon_attack`. Returns an empty vec if any prerequisite is missing
/// (target / caster gone) or the attack misses.
pub(crate) fn simple_weapon_attack(
    encounter: &mut EncounterInstance,
    caster_id: usize,
    target_ids: Option<&Vec<usize>>,
    action_name: &str,
    ability: crate::engine::types::AbilityScoreType,
    damage_dice: Dice,
    damage_type: DamageType,
    is_melee: bool,
) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
    let Some(target_id) = target_ids.and_then(|ids| ids.first().copied()) else {
        return Vec::new();
    };
    let Some(caster) = encounter.actors.get(&caster_id) else {
        return Vec::new();
    };
    let ability_mod = modifier_from_score(caster.ability_score(ability));
    let attack_bonus = caster.ability_attack_bonus(ability);
    let Some(target_ac) = encounter.actors.get(&target_id).map(|a| a.armor_class() as i32) else {
        return Vec::new();
    };
    weapon_attack(
        encounter,
        caster_id,
        target_id,
        action_name,
        attack_bonus,
        target_ac,
        damage_dice,
        ability_mod,
        damage_type,
        is_melee,
    )
}

/// Roll a d20 attack against `target_ac`, log the breakdown, and on a hit
/// roll `damage_dice + damage_bonus` of `damage_type` against `target_id`.
/// `is_melee` drives Prone-target advantage / ranged disadvantage clauses.
///
/// **Critical hits**: a final d20 of 20 (after advantage / disadvantage)
/// auto-hits regardless of AC and rolls the damage dice twice — the
/// modifier is added once. 5e RAW.
///
/// Returns the side-effect vec (empty on miss). Centralizes the pattern
/// so every weapon-style attack logs in the same shape.
#[allow(clippy::too_many_arguments)]
fn weapon_attack(
    encounter: &mut EncounterInstance,
    caster_id: usize,
    target_id: usize,
    action_name: &str,
    attack_bonus: i32,
    target_ac: i32,
    damage_dice: Dice,
    damage_bonus: i32,
    damage_type: DamageType,
    is_melee: bool,
) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
    use crate::conditions::Condition;

    let mode = encounter.compute_attack_mode(caster_id, target_id, is_melee);
    let raw_attack = encounter.roll_d20_with_mode(mode) as i32;
    let is_crit = raw_attack == 20;
    // Bless rider — +1d4 to the attack roll while concentration holds
    // on the attacker. Rolled here so the log reflects the breakdown.
    let blessed = encounter
        .actors
        .get(&caster_id)
        .is_some_and(|a| a.has_condition(Condition::Blessed));
    let bless_extra = if blessed {
        encounter.roll(&Dice::new(1, 4)) as i32
    } else {
        0
    };
    let attack_total = raw_attack + attack_bonus + bless_extra;
    // Crits auto-hit regardless of AC. Otherwise compare normally.
    let hit = is_crit || attack_total >= target_ac;
    let outcome = if is_crit {
        "CRIT!"
    } else if hit {
        "hit"
    } else {
        "miss"
    };
    let bless_suffix = if blessed {
        format!(" + bless 1d4({})", bless_extra)
    } else {
        String::new()
    };
    encounter.log(format!(
        "  {}: 1d20({}){:+}{} = {} vs AC {}{} \u{2014} {}",
        action_name,
        raw_attack,
        attack_bonus,
        bless_suffix,
        attack_total,
        target_ac,
        mode.log_suffix(),
        outcome,
    ));
    if !hit {
        return Vec::new();
    }
    let raw_damage = encounter.roll(&damage_dice) as i32;
    let crit_extra = if is_crit {
        encounter.roll(&damage_dice) as i32
    } else {
        0
    };
    let damage = (raw_damage + crit_extra + damage_bonus).max(0) as u32;
    if is_crit {
        encounter.log(format!(
            "  {}: {}({})+{}({}){:+} = {} {:?} damage (crit)",
            action_name,
            damage_dice,
            raw_damage,
            damage_dice,
            crit_extra,
            damage_bonus,
            damage,
            damage_type,
        ));
    } else {
        encounter.log(format!(
            "  {}: {}({}){:+} = {} {:?} damage",
            action_name, damage_dice, raw_damage, damage_bonus, damage, damage_type,
        ));
    }
    vec![Box::new(DealDamage {
        actor_id: target_id,
        amount: damage,
        damage_type,
    })]
}
