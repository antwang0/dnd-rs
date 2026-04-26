use std::collections::HashSet;

use crate::engine::{
    action_overrides::ActionOverride,
    encounter::EncounterInstance,
    side_effects::{ApplicableSideEffect, ConsumeResource, Resource},
    types::Coordinate,
};

/// Reach for melee/touch actions, expressed as a footprint-Chebyshev gap cap.
/// 5e melee weapons are 5ft = 1-tile gap in this 2.5ft grid. Polearms /
/// reach weapons would be 2. Ranged actions return their max range here.
pub const MELEE_REACH: isize = 1;

pub enum TargetingSchema {
    NoArgs,
    SinglePoint,
    SingleActor,
    Custom,
}

pub trait Action {
    fn name(&self) -> &str;

    fn aliases(&self) -> Vec<&str>;

    fn targeting_schema(&self) -> TargetingSchema;

    /// Maximum footprint-Chebyshev gap from caster to target for this action
    /// to be valid. `None` disables the spatial check (non-targeted actions
    /// or actions that do their own range logic). Used both by the engine
    /// (validation) and by the UI (target picker filters by reach).
    fn reach_tiles(&self) -> Option<isize> {
        None
    }

    /// Whether this action requires unobstructed line-of-sight from caster
    /// to target. True for ranged attacks and most spells; false for melee
    /// (you have to be touching). Walls block, actors don't.
    fn requires_los(&self) -> bool {
        false
    }

    fn side_effects(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>>;

    fn cost(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        overrides: Option<&HashSet<ActionOverride>>,
    ) -> Option<Resource>;

    fn validate_input(
        &self,
        encounter: &EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // TODO: overrides
        // TODO: validate points and target ids
        // TODO this is gross
        let schema_validation = match self.targeting_schema() {
            TargetingSchema::NoArgs => {
                if target_ids.is_some() {
                    return false;
                }
                if target_locations.is_some() {
                    return false;
                }
                if overrides.is_some() {
                    return false;
                }
                true
            }
            TargetingSchema::SinglePoint => {
                if target_ids.is_some() {
                    return false;
                }
                if let Some(tl) = target_locations {
                    if tl.len() != 1 {
                        return false;
                    }
                } else {
                    return false;
                }
                true
            }
            TargetingSchema::SingleActor => {
                if target_locations.is_some() {
                    return false;
                }
                target_ids.is_some_and(|ids| !ids.is_empty())
            }
            TargetingSchema::Custom => true,
        };
        if !schema_validation {
            return false;
        }
        // Reach + LOS for actor-targeted actions. Skipped if reach_tiles is
        // None or if there are no target ids (covered by schema check above).
        if let Some(targets) = target_ids
            && let Some(&target_id) = targets.first()
        {
            if let Some(reach) = self.reach_tiles() {
                let Some(dist) = encounter.footprint_distance(caster_id, target_id) else {
                    return false;
                };
                if dist > reach {
                    return false;
                }
            }
            if self.requires_los() && !encounter.actor_has_line_of_sight(caster_id, target_id) {
                return false;
            }
        }
        if let Some(cost) = self.cost(
            encounter,
            caster_id,
            target_ids,
            target_locations,
            overrides,
        ) {
            let Some(actor) = encounter.actors.get(&caster_id) else {
                return false;
            };
            if !actor.can_consume_resource(cost) {
                return false;
            }
        }
        self.custom_validate_input(
            encounter,
            caster_id,
            target_ids,
            target_locations,
            overrides,
        )
    }

    fn custom_validate_input(
        &self,
        _encounter: &EncounterInstance,
        _caster_id: usize,
        _target_ids: Option<&Vec<usize>>,
        _target_locations: Option<&Vec<Coordinate>>,
        _overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        true
    }

    fn execute(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<Coordinate>>,
        overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>> {
        if !self.validate_input(
            encounter,
            caster_id,
            target_ids,
            target_locations,
            overrides,
        ) {
            // The action became invalid between enqueue and execute (e.g. target died,
            // resource was consumed elsewhere). Skip silently; the engine logs context.
            return Vec::new();
        }
        let mut side_effects = self.side_effects(
            encounter,
            caster_id,
            target_ids,
            target_locations,
            overrides,
        );
        if let Some(cost) = self.cost(
            encounter,
            caster_id,
            target_ids,
            target_locations,
            overrides,
        ) {
            side_effects.push(Box::new(ConsumeResource {
                actor_id: caster_id,
                resource: cost,
            }));
        }
        side_effects
    }
}

pub struct ActionExecutionInfo {
    action: &'static dyn Action,
    caster_id: usize,
    target_ids: Option<Vec<usize>>,
    target_locations: Option<Vec<Coordinate>>,
    overrides: Option<HashSet<ActionOverride>>,
}

impl ActionExecutionInfo {
    pub fn new(
        action: &'static dyn Action,
        caster_id: usize,
        target_ids: Option<Vec<usize>>,
        target_locations: Option<Vec<Coordinate>>,
        overrides: Option<HashSet<ActionOverride>>,
    ) -> Self {
        Self {
            action,
            caster_id,
            target_ids,
            target_locations,
            overrides,
        }
    }

    pub fn action(&self) -> &'static dyn Action {
        self.action
    }

    pub fn caster_id(&self) -> usize {
        self.caster_id
    }

    pub fn target_ids(&self) -> Option<&[usize]> {
        self.target_ids.as_deref()
    }

    pub fn target_locations(&self) -> Option<&[Coordinate]> {
        self.target_locations.as_deref()
    }

    /// Resolved cost of this specific invocation (uses the stored target/loc
    /// args, not None placeholders) — useful for the engine log filter.
    pub fn cost(&self, encounter: &EncounterInstance) -> Option<Resource> {
        self.action.cost(
            encounter,
            self.caster_id,
            self.target_ids.as_ref(),
            self.target_locations.as_ref(),
            self.overrides.as_ref(),
        )
    }

    pub fn validate(&self, encounter: &EncounterInstance) -> bool {
        self.action.validate_input(
            encounter,
            self.caster_id,
            self.target_ids.as_ref(),
            self.target_locations.as_ref(),
            self.overrides.as_ref(),
        )
    }

    pub fn execute(&self, encounter: &mut EncounterInstance) -> Vec<Box<dyn ApplicableSideEffect>> {
        self.action.execute(
            encounter,
            self.caster_id,
            self.target_ids.as_ref(),
            self.target_locations.as_ref(),
            self.overrides.as_ref(),
        )
    }
}
