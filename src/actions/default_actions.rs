use crate::{
    actions::action_template::TargetingSchema,
    engine::{side_effects::GiveResource, types::Coordinate},
};
use std::{collections::HashSet, sync::LazyLock};

use crate::{
    actions::action_template::Action,
    engine::{
        action_overrides::ActionOverride,
        encounter::EncounterInstance,
        side_effects::{MoveActor, Resource, SkipTurn},
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

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SinglePoint
    }

    fn cost(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        let Some(dest) = target_locations.and_then(|t| t.first().copied()) else {
            return Vec::new();
        };
        let Some(dist) = encounter.path_cost_to(caster_id, dest) else {
            return Vec::new();
        };
        vec![Resource::Movement(dist)]
    }

    fn custom_validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        let Some(tl) = target_locations else {
            return false;
        };
        let Some(&coord) = tl.first() else {
            return false;
        };
        // path_cost_to verifies destination footprint AND walkable path
        // within remaining movement budget.
        encounter.path_cost_to(caster_id, coord).is_some()
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
        let Some(target_location) = target_locations.and_then(|v| v.first().copied()) else {
            return Vec::new();
        };
        // Use the same Dijkstra path that `cost` charged for, so OAs fire on
        // every threatened-square exit along the way. Falls back to a
        // single-tile teleport if pathing fails (validate should already
        // have caught this, but defense in depth).
        let path = encounter
            .path_to(caster_id, target_location)
            .unwrap_or_else(|| vec![target_location]);
        vec![Box::new(MoveActor {
            actor_id: caster_id,
            path,
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

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }

    fn cost(
        &self,
        _encounter: &EncounterInstance,
        _caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        Vec::new()
    }

    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        _caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
        vec![Box::new(SkipTurn {})]
    }
}

pub static SKIP: LazyLock<Skip> = LazyLock::new(|| Skip {});

pub struct Dash {}

impl Action for Dash {
    fn name(&self) -> &str {
        "dash"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["dsh"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
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
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
        let Some(actor) = encounter.get_actor(caster_id) else {
            return Vec::new();
        };
        let speed = actor.speed();
        vec![Box::new(GiveResource {
            actor_id: caster_id,
            resource: Resource::Movement(speed),
        })]
    }
}

pub static DASH: LazyLock<Dash> = LazyLock::new(|| Dash {});

/// Stand up from being prone. 5e: standing up costs half your speed in
/// movement. Only valid while the caster has the Prone condition.
pub struct StandUp {}

impl Action for StandUp {
    fn name(&self) -> &str {
        "stand"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["standup", "su"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::NoArgs
    }

    fn cost(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Resource> {
        match encounter.actors.get(&caster_id) {
            Some(a) => vec![Resource::Movement(a.speed() / 2.0)],
            None => Vec::new(),
        }
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
            .is_some_and(|a| a.has_condition(crate::conditions::Condition::Prone))
    }

    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
        vec![Box::new(crate::engine::side_effects::RemoveCondition {
            actor_id: caster_id,
            condition: crate::conditions::Condition::Prone,
        })]
    }
}

pub static STAND_UP: LazyLock<StandUp> = LazyLock::new(|| StandUp {});

/// Dodge — defensive Action. Self-applies the Dodging condition (1 round
/// timer = expires before this actor's next turn), which gives attacks
/// against you disadvantage and you advantage on DEX saves.
pub struct Dodge {}

impl Action for Dodge {
    fn name(&self) -> &str {
        "dodge"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["dg"]
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

    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
        use crate::conditions::{Condition, ConditionTimer};
        use crate::engine::side_effects::ApplyCondition;
        vec![Box::new(ApplyCondition {
            actor_id: caster_id,
            condition: Condition::Dodging,
            // 1 round = clears at the next round-end tick, which fires
            // before this actor's next turn.
            timer: ConditionTimer::Rounds(1),
        })]
    }
}

pub static DODGE: LazyLock<Dodge> = LazyLock::new(|| Dodge {});

/// Disengage — Action that suppresses opportunity attacks for the rest
/// of this turn. The flag clears on `reset_for_new_round`. Useful for
/// peeling out of melee without eating an OA.
pub struct Disengage {}

impl Action for Disengage {
    fn name(&self) -> &str {
        "disengage"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["de"]
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

    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
        use crate::engine::side_effects::SetDisengaging;
        vec![Box::new(SetDisengaging { actor_id: caster_id })]
    }
}

pub static DISENGAGE: LazyLock<Disengage> = LazyLock::new(|| Disengage {});

pub static DEFAULT_ACTIONS: LazyLock<Vec<&'static (dyn Action + Send + Sync)>> =
    LazyLock::new(|| vec![&*MOVE, &*DASH, &*SKIP, &*STAND_UP, &*DODGE, &*DISENGAGE]);
