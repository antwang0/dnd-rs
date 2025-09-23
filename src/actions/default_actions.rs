use std::{collections::HashSet, sync::LazyLock};

use crate::{
    actions::action_template::Action,
    engine::{
        action_overrides::ActionOverride, encounter::EncounterInstance, side_effects::{MoveActor, SkipTurn},
    },
};

pub struct Move {}

impl Action for Move {
    fn name(&self) -> &str {
        "move"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["mv"]
    }

    fn targeting_schema(&self) -> super::action_template::TargetingSchema {
        super::action_template::TargetingSchema::SinglePoint
    }

    fn execute_impl(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<(usize, usize)>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
        let target_location = target_locations.unwrap().first().unwrap();
        vec![Box::new(MoveActor {
            actor_id: caster_id,
            target: *target_location,
        })]
    }
}

pub static MOVE: LazyLock<Move> = LazyLock::new(|| Move {});

pub struct Skip {}

impl Action for Skip {
    fn name(&self) -> &str {
        "skip"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["s"]
    }

    fn targeting_schema(&self) -> super::action_template::TargetingSchema {
        super::action_template::TargetingSchema::NoArgs
    }

    fn execute_impl(
        &self,
        _encounter: &mut EncounterInstance,
        _caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<(usize, usize)>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
        vec![Box::new(SkipTurn{})]
    }
}

pub static SKIP: LazyLock<Skip> = LazyLock::new(|| Skip {});

pub static DEFAULT_ACTIONS: LazyLock<Vec<&'static (dyn Action + Send + Sync)>> = LazyLock::new(|| {
    vec![&*MOVE, &*SKIP]
});
