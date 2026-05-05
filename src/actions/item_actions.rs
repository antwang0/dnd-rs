use std::collections::HashSet;

use crate::{
    actions::action_template::{Action, TargetingSchema},
    engine::{
        action_overrides::ActionOverride,
        dice::Dice,
        encounter::EncounterInstance,
        side_effects::{ApplicableSideEffect, DealDamage, Heal, Resource},
        types::{Coordinate, DamageType},
    },
};

const POTION_OF_HEALING_NAME: &str = "Potion of Healing";
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

    fn is_healing(&self) -> bool {
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

        // Find every actor whose footprint sits inside the blast radius.
        const BLAST_RADIUS: isize = 4;
        let dc: i32 = 15;
        let actor_ids: Vec<usize> = encounter.actors.keys().copied().collect();
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        for id in actor_ids {
            let Some(dist) = encounter.footprint_distance_to_point(id, center) else {
                continue;
            };
            if dist > BLAST_RADIUS {
                continue;
            }
            use crate::engine::saves::SaveOutcome;
            use crate::engine::types::AbilityScoreType;
            let outcome = encounter.roll_save(id, AbilityScoreType::Dexterity, dc);
            let final_damage = match outcome {
                SaveOutcome::Pass => damage / 2,
                SaveOutcome::Fail => damage,
            };
            effects.push(Box::new(DealDamage {
                actor_id: id,
                amount: final_damage,
                damage_type: DamageType::Fire,
            }));
        }
        effects
    }
}

pub static READ_FIREBALL_SCROLL: ReadFireballScroll = ReadFireballScroll {};
