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

/// Dodge action. Until the start of the actor's next turn, attackers roll
/// at disadvantage and the actor gains advantage on DEX saves. Modeled as
/// a self-applied `Dodging` condition with a 1-round timer — the initiative
/// wrap ticks it down, so it expires exactly when the actor's turn comes
/// back around.
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
            // 1 round: ticked down on the next initiative wrap, so it's
            // gone when this actor's turn comes back around.
            timer: ConditionTimer::Rounds(1),
        })]
    }
}

pub static DODGE: LazyLock<Dodge> = LazyLock::new(|| Dodge {});

/// Disengage action. The actor's movement doesn't provoke opportunity
/// attacks until the start of their next turn. Modeled with a self-applied
/// `Disengaged` condition that OA dispatch respects and that expires with
/// the initiative wrap.
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
        use crate::conditions::{Condition, ConditionTimer};
        use crate::engine::side_effects::ApplyCondition;
        vec![Box::new(ApplyCondition {
            actor_id: caster_id,
            condition: Condition::Disengaged,
            timer: ConditionTimer::Rounds(1),
        })]
    }
}

pub static DISENGAGE: LazyLock<Disengage> = LazyLock::new(|| Disengage {});

/// Help another creature: their next attack roll has advantage. Requires
/// the target to be adjacent (footprint gap ≤ 1) and on the same team, so
/// you can't Help an enemy. The `Helped` condition is consumed the moment
/// they make an attack (see `consume_help_on_attack`).
pub struct Help {}

impl Action for Help {
    fn name(&self) -> &str {
        "help"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["hp-action", "assist"]
    }

    fn targeting_schema(&self) -> TargetingSchema {
        TargetingSchema::SingleActor
    }

    fn reach_tiles(&self) -> Option<isize> {
        // Adjacent-ally range: same "reach" as a melee weapon.
        Some(1)
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
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        let Some(target_id) = target_ids.and_then(|ids| ids.first().copied()) else {
            return false;
        };
        if target_id == caster_id {
            return false;
        }
        let Some(caster) = encounter.actors.get(&caster_id) else {
            return false;
        };
        let Some(target) = encounter.actors.get(&target_id) else {
            return false;
        };
        target.team() == caster.team() && target.is_combat_active()
    }

    fn side_effects(
        &self,
        _encounter: &mut EncounterInstance,
        _caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn crate::engine::side_effects::ApplicableSideEffect>> {
        use crate::conditions::{Condition, ConditionTimer};
        use crate::engine::side_effects::ApplyCondition;
        let Some(target_id) = target_ids.and_then(|ids| ids.first().copied()) else {
            return Vec::new();
        };
        vec![Box::new(ApplyCondition {
            actor_id: target_id,
            condition: Condition::Helped,
            // Lasts until the helped ally's next turn. On attack the buff
            // is stripped immediately, so a stale timer just tidies up if
            // they never actually attack.
            timer: ConditionTimer::Rounds(1),
        })]
    }
}

pub static HELP: LazyLock<Help> = LazyLock::new(|| Help {});

pub static DEFAULT_ACTIONS: LazyLock<Vec<&'static (dyn Action + Send + Sync)>> = LazyLock::new(
    || vec![&*MOVE, &*DASH, &*SKIP, &*STAND_UP, &*DODGE, &*DISENGAGE, &*HELP],
);
