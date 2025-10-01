use std::collections::HashSet;

use crate::{
    actions::action_template::{Action, TargetingSchema},
    engine::{
        action_overrides::ActionOverride, encounter::EncounterInstance, side_effects::Resource,
        types::Coordinate,
    },
};

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
        // TODO
        // if let Some(target_ids) = target_ids {
        //     let target_id = target_ids[0];
        //     let caster = encounter.actors.get(&caster_id).unwrap();
        //     let target = encounter.actors.get(&target_id).unwrap();

        //     // Attack roll
        //     let attack_roll = Roll::d20().roll() + caster.attack_bonus();
        //     let attack_successful = attack_roll >= target.armor_class();

        //     if attack_successful {
        //         // Damage roll
        //         let damage_roll = Roll::from_str("2d6").unwrap().roll() + caster.damage_bonus();
        //         effects.push(Box::new(DealDamage {
        //             actor_id: target_id,
        //             amount: damage_roll,
        //             damage_type: DamageType::Bludgeoning,
        //         }));
        //     }
        // }

        effects
    }
}
