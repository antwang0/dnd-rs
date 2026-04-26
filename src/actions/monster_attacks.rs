use std::collections::HashSet;
use std::sync::LazyLock;

use tyche::Dice;
use tyche::dice::roller::Roller;

use crate::{
    actions::action_template::{Action, TargetingSchema},
    engine::{
        action_overrides::ActionOverride,
        encounter::EncounterInstance,
        side_effects::{DealDamage, Resource},
        types::{Coordinate, DamageType},
        util::modifier_from_score,
    },
};

pub static SLAM: LazyLock<Slam> = LazyLock::new(|| Slam {});

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

    fn cost(
        &self,
        _encounter: &EncounterInstance,
        _caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Option<Resource> {
        Some(Resource::Action)
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
        let mut effects: Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> = vec![];
        if let Some(target_ids) = target_ids {
            let target_id = target_ids[0];
            let caster_strength = encounter
                .actors
                .get(&caster_id)
                .unwrap()
                .ability_score(crate::engine::types::AbilityScoreType::Strength);
            let attack_bonus = encounter.actors.get(&caster_id).unwrap().attack_bonus();
            let target_ac = encounter.actors.get(&target_id).unwrap().armor_class();

            let d20 = Dice::new(1, 20);
            let attack_roll = encounter
                .roller
                .roll(&d20, true)
                .unwrap()
                .total()
                .unwrap() as i32
                + attack_bonus;

            if attack_roll >= target_ac as i32 {
                let damage_dice = Dice::new(2, 6);
                let damage_roll = encounter
                    .roller
                    .roll(&damage_dice, true)
                    .unwrap()
                    .total()
                    .unwrap() as i32
                    + modifier_from_score(caster_strength);
                effects.push(Box::new(DealDamage {
                    actor_id: target_id,
                    amount: damage_roll.max(0) as u32,
                    damage_type: DamageType::Bludgeoning,
                }));
            }
        }

        effects
    }
}
