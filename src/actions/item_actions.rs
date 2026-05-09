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
const SCROLL_OF_MAGIC_MISSILE_NAME: &str = "Scroll of Magic Missile";

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

    fn heals(&self) -> bool {
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

/// Drink a Potion of Greater Healing. Self-targeted, costs an Action,
/// heals 4d4+4. Same lifecycle as the standard healing potion (validate
/// requires the item in inventory; remove on use).
pub struct DrinkGreaterHealingPotion {}

impl Action for DrinkGreaterHealingPotion {
    fn name(&self) -> &str {
        "drink greater healing potion"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["potion+", "drink+"]
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
            .is_some_and(|a| a.has_item_named(POTION_OF_GREATER_HEALING_NAME))
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _ti: Option<&Vec<usize>>,
        _tl: Option<&Vec<Coordinate>>,
        _o: Option<&HashSet<ActionOverride>>,
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
            "  potion of greater healing: 4d4({})+4 = {} HP",
            raw, amount
        ));
        vec![Box::new(Heal {
            actor_id: caster_id,
            amount,
        })]
    }
}

pub static DRINK_GREATER_HEALING_POTION: DrinkGreaterHealingPotion = DrinkGreaterHealingPotion {};

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

        // Find every combat-active actor whose footprint sits inside the
        // blast radius (helper handles the sort + dist filter).
        const BLAST_RADIUS: isize = 4;
        let dc: i32 = 15;
        let mut effects: Vec<Box<dyn ApplicableSideEffect>> = Vec::new();
        for id in encounter.actors_in_burst(center, BLAST_RADIUS) {
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

/// Read a Scroll of Magic Missile: spend an Action to fire 3 darts at one
/// enemy in line-of-sight (range 30 tiles). Each dart deals 1d4+1 force.
/// Auto-hit, no save. The scroll is consumed regardless of outcome.
pub struct ReadMagicMissileScroll {}

impl Action for ReadMagicMissileScroll {
    fn name(&self) -> &str {
        "read magic missile scroll"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["mm scroll", "missile scroll"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }

    fn reach_tiles(&self) -> Option<isize> {
        Some(30)
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
            .is_some_and(|a| a.has_item_named(SCROLL_OF_MAGIC_MISSILE_NAME))
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
        let removed = encounter
            .actors
            .get_mut(&caster_id)
            .is_some_and(|a| a.remove_item_by_name(SCROLL_OF_MAGIC_MISSILE_NAME));
        if !removed {
            return Vec::new();
        }
        let mut total = 0u32;
        let mut rolls = [0u32; 3];
        for r in rolls.iter_mut() {
            *r = encounter.roll(&Dice::new(1, 4));
            total = total.saturating_add(*r + 1);
        }
        encounter.log(format!(
            "  scroll of magic missile: 3*(1d4+1) [{}, {}, {}] = {} force",
            rolls[0] + 1,
            rolls[1] + 1,
            rolls[2] + 1,
            total
        ));
        vec![Box::new(DealDamage {
            actor_id: target_id,
            amount: total,
            damage_type: DamageType::Force,
        })]
    }
}

pub static READ_MAGIC_MISSILE_SCROLL: ReadMagicMissileScroll = ReadMagicMissileScroll {};
