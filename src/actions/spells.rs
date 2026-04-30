use std::collections::HashSet;
use std::sync::LazyLock;

use crate::{
    actions::action_template::{Action, TargetingSchema},
    engine::{
        action_overrides::ActionOverride,
        dice::Dice,
        encounter::EncounterInstance,
        side_effects::{ApplicableSideEffect, DealDamage, Heal, Resource},
        types::{AbilityScoreType, Coordinate, DamageType},
        util::modifier_from_score,
    },
};

/// Sacred Flame — cleric cantrip. Range 60ft (24 tiles), DEX save vs the
/// caster's WIS-based spell save DC. On fail: 1d8 radiant. On success:
/// nothing (cantrips don't half-on-save). No spell slot consumed.
pub struct SacredFlame {}

impl Action for SacredFlame {
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

    fn cost(
        &self,
        _encounter: &EncounterInstance,
        _caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        // Cantrip — costs an Action only, no spell slot consumed.
        vec![Resource::Action]
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = target_ids.and_then(|ids| ids.first().copied()) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spell_save_dc(AbilityScoreType::Wisdom);

        let save = encounter.roll_save(target_id, AbilityScoreType::Dexterity, dc);
        if save.passed() {
            return Vec::new();
        }

        let raw = encounter.roll(&Dice::new(1, 8));
        encounter.log(format!("  sacred flame: 1d8({}) = {} radiant", raw, raw));
        vec![Box::new(DealDamage {
            actor_id: target_id,
            amount: raw,
            damage_type: DamageType::Radiant,
        })]
    }
}

pub static SACRED_FLAME: LazyLock<SacredFlame> = LazyLock::new(|| SacredFlame {});

/// Healing Word — cleric spell. Bonus action, range 60ft (24 tiles), no
/// save: target regains 1d4 + caster's WIS modifier HP. We currently treat
/// it as a cantrip (no spell slot) until spell-slot levels are wired.
/// LOS not strictly required in 5e (audibly heard), but we require it for
/// simplicity until "audible reach" is a thing.
pub struct HealingWord {}

impl Action for HealingWord {
    fn name(&self) -> &str {
        "healing word"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["hw", "heal"]
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
        _encounter: &EncounterInstance,
        _caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        // Healing Word — bonus-action level-1 spell.
        vec![Resource::BonusAction, Resource::SpellSlot(1)]
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let Some(target_id) = target_ids.and_then(|ids| ids.first().copied()) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let wis_mod = modifier_from_score(caster.ability_score(AbilityScoreType::Wisdom));
        let raw = encounter.roll(&Dice::new(1, 4)) as i32;
        let amount = (raw + wis_mod).max(1) as u32;
        encounter.log(format!(
            "  healing word: 1d4({}){:+} = {} HP",
            raw, wis_mod, amount
        ));
        vec![Box::new(Heal {
            actor_id: target_id,
            amount,
        })]
    }
}

pub static HEALING_WORD: LazyLock<HealingWord> = LazyLock::new(|| HealingWord {});

/// Sacred Burst — generic cleric AoE cantrip. Pick a tile within 30ft;
/// every actor (friend or foe) whose footprint touches the burst takes
/// 2d6 radiant on a failed DEX save vs the caster's WIS-based DC, half
/// on success. Damage is rolled once and shared. Requires LOS to the
/// burst origin (not to each target).
pub struct SacredBurst {}

impl Action for SacredBurst {
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

    fn cost(
        &self,
        _encounter: &EncounterInstance,
        _caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        // Cantrip — Action only, no spell slot.
        vec![Resource::Action]
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

        let Some(point) = target_locations.and_then(|tl| tl.first().copied()) else {
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

        // Roll damage once for the whole burst (5e: shared damage roll).
        let raw = encounter.roll(&Dice::new(2, 6));
        encounter.log(format!(
            "  sacred burst: 2d6({}) = {} radiant area",
            raw, raw
        ));

        // Snapshot affected ids in actor-id order for deterministic save
        // sequencing — order matters because the roller is shared and each
        // save consumes a d20.
        let mut ids: Vec<usize> = encounter.actors.keys().copied().collect();
        ids.sort_unstable();

        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        for target_id in ids {
            let Some(target) = encounter.actors.get(&target_id) else {
                continue;
            };
            // The caster is exempt — sacred-flavored AoE wouldn't burn its
            // own caster. Inactive actors (dying/stable) also skip.
            if target_id == caster_id || !target.is_combat_active() {
                continue;
            }
            let dist = footprint_chebyshev(
                target.location(),
                get_tiles_from_size(target.size()),
                point,
                1,
            );
            if dist > radius {
                continue;
            }

            let save = encounter.roll_save(target_id, AbilityScoreType::Dexterity, dc);
            let dmg = if save.passed() { raw / 2 } else { raw };
            if dmg == 0 {
                continue;
            }
            effects.push(Box::new(DealDamage {
                actor_id: target_id,
                amount: dmg,
                damage_type: DamageType::Radiant,
            }));
        }
        effects
    }
}

pub static SACRED_BURST: LazyLock<SacredBurst> = LazyLock::new(|| SacredBurst {});

/// Hold Person — 5e-flavored single-target paralysis. WIS save vs the
/// caster's WIS-based DC; on fail, target is Stunned (close enough to
/// Paralyzed for our model, since we don't yet model auto-fail STR/DEX
/// saves or melee crit-on-hit) for 10 rounds. Concentration: when the
/// caster takes damage and fails a CON save (or hits 0 HP), the spell
/// drops and Stunned clears immediately.
pub struct HoldPerson {}

impl Action for HoldPerson {
    fn name(&self) -> &str {
        "hold person"
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
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        // Level-2 leveled spell — Action + a level-2 spell slot.
        vec![Resource::Action, Resource::SpellSlot(2)]
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::actors::actor_template::ConcentrationData;
        use crate::conditions::{Condition, ConditionTimer};
        use crate::engine::side_effects::{ApplyCondition, StartConcentration};

        let Some(target_id) = target_ids.and_then(|ids| ids.first().copied()) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spell_save_dc(AbilityScoreType::Wisdom);

        let save = encounter.roll_save(target_id, AbilityScoreType::Wisdom, dc);
        if save.passed() {
            return Vec::new();
        }

        // Apply Stunned for up to 10 rounds, plus install concentration
        // tracking so a damage-failed CON save will release the target.
        vec![
            Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Stunned,
                timer: ConditionTimer::Rounds(10),
            }),
            Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData {
                    spell_name: "Hold Person".to_string(),
                    conditions: vec![(target_id, Condition::Stunned)],
                },
            }),
        ]
    }
}

pub static HOLD_PERSON: LazyLock<HoldPerson> = LazyLock::new(|| HoldPerson {});

/// Blindness — single-target denial cantrip-style spell. Range 30ft (12
/// tiles), CON save vs the caster's WIS-based DC. On fail, target is
/// Blinded for 10 rounds (advantage on attacks against them, disadvantage
/// on their attacks). Concentration: damage-failed CON save or 0 HP
/// drops the spell and clears Blinded immediately.
pub struct Blindness {}

impl Action for Blindness {
    fn name(&self) -> &str {
        "blindness"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["blind", "bl"]
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
        _encounter: &EncounterInstance,
        _caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        // Level-2 leveled spell — Action + a level-2 spell slot.
        vec![Resource::Action, Resource::SpellSlot(2)]
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::actors::actor_template::ConcentrationData;
        use crate::conditions::{Condition, ConditionTimer};
        use crate::engine::side_effects::{ApplyCondition, StartConcentration};

        let Some(target_id) = target_ids.and_then(|ids| ids.first().copied()) else {
            return Vec::new();
        };
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return Vec::new();
        };
        let dc = caster.spell_save_dc(AbilityScoreType::Wisdom);

        let save = encounter.roll_save(target_id, AbilityScoreType::Constitution, dc);
        if save.passed() {
            return Vec::new();
        }

        vec![
            Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Blinded,
                timer: ConditionTimer::Rounds(10),
            }),
            Box::new(StartConcentration {
                caster_id,
                data: ConcentrationData {
                    spell_name: "Blindness".to_string(),
                    conditions: vec![(target_id, Condition::Blinded)],
                },
            }),
        ]
    }
}

pub static BLINDNESS: LazyLock<Blindness> = LazyLock::new(|| Blindness {});

/// Web — area control spell. Pick a tile within 60ft; every actor whose
/// footprint touches the burst makes a DEX save vs the caster's WIS-based
/// DC. On fail, the actor is Restrained for 10 rounds (speed 0, attacks
/// against them have advantage, their attacks have disadvantage, DEX
/// saves at disadvantage). Concentration: damage-failed CON save or 0 HP
/// drops the spell, clearing Restrained on every webbed actor at once.
/// Cleric flavor here — in 5e Web is a wizard / sorcerer spell, but the
/// engine wires the same way regardless of class list.
pub struct Web {}

impl Action for Web {
    fn name(&self) -> &str {
        "web"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["wb"]
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

    fn cost(
        &self,
        _encounter: &EncounterInstance,
        _caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        vec![Resource::Action, Resource::SpellSlot(2)]
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
        use crate::conditions::{Condition, ConditionTimer};
        use crate::engine::side_effects::{ApplyCondition, StartConcentration};
        use crate::engine::util::{footprint_chebyshev, get_tiles_from_size};

        let Some(point) = target_locations.and_then(|tl| tl.first().copied()) else {
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

        // Sorted ids so RNG ordering of saves is deterministic with a seed.
        let mut ids: Vec<usize> = encounter.actors.keys().copied().collect();
        ids.sort_unstable();

        let mut conds_for_concentration: Vec<(usize, Condition)> = Vec::new();
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        for target_id in ids {
            let Some(target) = encounter.actors.get(&target_id) else {
                continue;
            };
            // Caster steps clear of their own webbing; downed actors skip.
            if target_id == caster_id || !target.is_combat_active() {
                continue;
            }
            let dist = footprint_chebyshev(
                target.location(),
                get_tiles_from_size(target.size()),
                point,
                1,
            );
            if dist > radius {
                continue;
            }

            let save = encounter.roll_save(target_id, AbilityScoreType::Dexterity, dc);
            if save.passed() {
                continue;
            }
            effects.push(Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Restrained,
                timer: ConditionTimer::Rounds(10),
            }));
            conds_for_concentration.push((target_id, Condition::Restrained));
        }
        if conds_for_concentration.is_empty() {
            // Nobody got webbed — spell fizzles, no concentration.
            return Vec::new();
        }
        effects.push(Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData {
                spell_name: "Web".to_string(),
                conditions: conds_for_concentration,
            },
        }));
        effects
    }
}

pub static WEB: LazyLock<Web> = LazyLock::new(|| Web {});

/// Faerie Fire — area outline spell. Pick a tile within 60ft; every
/// actor whose footprint touches the burst makes a DEX save vs the
/// caster's WIS-based DC. On fail, the actor is Outlined for 10 rounds:
/// attacks against them have advantage. The target's own attacks are
/// unaffected — distinct from Blinded. Concentration: damage-failed
/// CON save or 0 HP drops the spell, clearing Outlined on every lit
/// actor at once.
pub struct FaerieFire {}

impl Action for FaerieFire {
    fn name(&self) -> &str {
        "faerie fire"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["ff", "faerie"]
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

    fn cost(
        &self,
        _encounter: &EncounterInstance,
        _caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        // Level-1 leveled spell.
        vec![Resource::Action, Resource::SpellSlot(1)]
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
        use crate::conditions::{Condition, ConditionTimer};
        use crate::engine::side_effects::{ApplyCondition, StartConcentration};
        use crate::engine::util::{footprint_chebyshev, get_tiles_from_size};

        let Some(point) = target_locations.and_then(|tl| tl.first().copied()) else {
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

        let mut ids: Vec<usize> = encounter.actors.keys().copied().collect();
        ids.sort_unstable();

        let mut conds_for_concentration: Vec<(usize, Condition)> = Vec::new();
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        for target_id in ids {
            let Some(target) = encounter.actors.get(&target_id) else {
                continue;
            };
            if target_id == caster_id || !target.is_combat_active() {
                continue;
            }
            let dist = footprint_chebyshev(
                target.location(),
                get_tiles_from_size(target.size()),
                point,
                1,
            );
            if dist > radius {
                continue;
            }

            let save = encounter.roll_save(target_id, AbilityScoreType::Dexterity, dc);
            if save.passed() {
                continue;
            }
            effects.push(Box::new(ApplyCondition {
                actor_id: target_id,
                condition: Condition::Outlined,
                timer: ConditionTimer::Rounds(10),
            }));
            conds_for_concentration.push((target_id, Condition::Outlined));
        }
        if conds_for_concentration.is_empty() {
            return Vec::new();
        }
        effects.push(Box::new(StartConcentration {
            caster_id,
            data: ConcentrationData {
                spell_name: "Faerie Fire".to_string(),
                conditions: conds_for_concentration,
            },
        }));
        effects
    }
}

pub static FAERIE_FIRE: LazyLock<FaerieFire> = LazyLock::new(|| FaerieFire {});
