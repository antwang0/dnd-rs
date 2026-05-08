use std::collections::HashSet;

use crate::{
    actions::action_template::{Action, TargetingSchema},
    engine::{
        action_overrides::ActionOverride,
        dice::Dice,
        encounter::EncounterInstance,
        side_effects::{ApplicableSideEffect, Heal, Resource},
        types::{Coordinate, DamageType},
    },
};

const POTION_OF_HEALING_NAME: &str = "Potion of Healing";
const POTION_OF_GREATER_HEALING_NAME: &str = "Potion of Greater Healing";
const ANTITOXIN_NAME: &str = "Antitoxin";
const SCROLL_OF_FIREBALL_NAME: &str = "Scroll of Fireball";

/// Drink a Potion of Healing. Self-targeted, costs an Action, heals
/// 2d4+2 and removes one potion from inventory. The validate hook
/// rejects the action if the caster has no potion left (so a duplicate
/// "drink" entry with zero stock can't fire), and the side-effects
/// step pops one potion before the Heal applies.
pub struct DrinkHealingPotion {}

impl Action for DrinkHealingPotion {
    fn name(&self) -> &str {
        "drink healing potion"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["potion", "drink"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
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
        vec![Resource::Action]
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
            .is_some_and(|a| a.has_item_named(POTION_OF_HEALING_NAME))
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let raw = encounter.roll(&Dice::new(2, 4)) as i32;
        let amount = (raw + 2).max(1) as u32;
        // Pop the potion *now* — if the action was queued, validate
        // already confirmed at least one was carried, and consuming
        // before the Heal side-effect runs keeps inventory consistent
        // even if the heal somehow fails (e.g. caster died mid-stack).
        let removed = encounter
            .actors
            .get_mut(&caster_id)
            .is_some_and(|a| a.remove_item_by_name(POTION_OF_HEALING_NAME));
        if !removed {
            return Vec::new();
        }
        encounter.log(format!(
            "  potion of healing: 2d4({}){:+} = {} HP",
            raw, 2, amount
        ));
        vec![Box::new(Heal {
            actor_id: caster_id,
            amount,
        })]
    }
}

pub static DRINK_HEALING_POTION: DrinkHealingPotion = DrinkHealingPotion {};

/// Read a Scroll of Fireball: pick a target tile, every actor whose
/// footprint touches the burst takes 6d6 fire on a failed DEX save vs
/// DC 15, half on success. Consumes the scroll. No spell-slot cost
/// (the scroll *is* the slot).
pub struct ReadFireballScroll {}

impl Action for ReadFireballScroll {
    fn name(&self) -> &str {
        "read fireball scroll"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["fireball", "scroll"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::Burst { radius: 4 }
    }

    fn reach_tiles(&self) -> Option<isize> {
        // 150 ft range — well past any current map.
        Some(60)
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
            .is_some_and(|a| a.has_item_named(SCROLL_OF_FIREBALL_NAME))
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::actions::action_template::resolve_burst_save_damage;
        use crate::engine::types::AbilityScoreType;

        let Some(&center) = target_locations.and_then(|locs| locs.first()) else {
            return Vec::new();
        };
        let removed = encounter
            .actors
            .get_mut(&caster_id)
            .is_some_and(|a| a.remove_item_by_name(SCROLL_OF_FIREBALL_NAME));
        if !removed {
            return Vec::new();
        }

        // Roll damage once and share across the burst so everyone in the
        // blast takes the same number — matches Sacred Burst's pattern.
        let damage = encounter.roll(&Dice::new(6, 6));
        encounter.log(format!("  scroll of fireball: 6d6 = {} damage", damage));

        const BLAST_RADIUS: isize = 4;
        const DC: i32 = 15;
        resolve_burst_save_damage(
            encounter,
            caster_id,
            center,
            BLAST_RADIUS,
            AbilityScoreType::Dexterity,
            DC,
            damage,
            DamageType::Fire,
        )
    }
}

pub static READ_FIREBALL_SCROLL: ReadFireballScroll = ReadFireballScroll {};

/// Drink a Potion of Greater Healing — Bonus Action self-heal for 4d4+4.
/// Cheaper economy than Healing Potion (Action) so PCs can both heal
/// and act in the same turn. Same consume-on-use semantics.
pub struct DrinkGreaterHealingPotion {}

impl Action for DrinkGreaterHealingPotion {
    fn name(&self) -> &str {
        "drink greater healing potion"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["greater potion", "greater"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
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
        vec![Resource::BonusAction]
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
            .is_some_and(|a| a.has_item_named(POTION_OF_GREATER_HEALING_NAME))
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        let raw = encounter.roll(&Dice::new(4, 4)) as i32;
        let amount = (raw + 4).max(1) as u32;
        let removed = encounter
            .actors
            .get_mut(&caster_id)
            .is_some_and(|a| a.remove_item_by_name(POTION_OF_GREATER_HEALING_NAME));
        if !removed {
            return Vec::new();
        }
        encounter.log(format!(
            "  greater healing potion: 4d4({}){:+} = {} HP",
            raw, 4, amount
        ));
        vec![Box::new(Heal {
            actor_id: caster_id,
            amount,
        })]
    }
}

pub static DRINK_GREATER_HEALING_POTION: DrinkGreaterHealingPotion = DrinkGreaterHealingPotion {};

/// Drink an Antitoxin — Action self-cleanse that removes the Poisoned
/// condition. Doesn't grant ongoing immunity (5e gives advantage on
/// poison saves for 1h; we ignore the buff and just clear the marker).
pub struct DrinkAntitoxin {}

impl Action for DrinkAntitoxin {
    fn name(&self) -> &str {
        "drink antitoxin"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["antitoxin"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
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
        vec![Resource::Action]
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
            .is_some_and(|a| a.has_item_named(ANTITOXIN_NAME))
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        use crate::conditions::Condition;
        use crate::engine::side_effects::RemoveCondition;

        let removed = encounter
            .actors
            .get_mut(&caster_id)
            .is_some_and(|a| a.remove_item_by_name(ANTITOXIN_NAME));
        if !removed {
            return Vec::new();
        }
        encounter.log("  antitoxin: clears poisoned".to_string());
        vec![Box::new(RemoveCondition {
            actor_id: caster_id,
            condition: Condition::Poisoned,
        })]
    }
}

pub static DRINK_ANTITOXIN: DrinkAntitoxin = DrinkAntitoxin {};
