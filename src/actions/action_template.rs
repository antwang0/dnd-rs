use std::collections::HashSet;

use crate::engine::{
    action_overrides::ActionOverride,
    encounter::EncounterInstance,
    side_effects::{ApplicableSideEffect, ConsumeResource, Resource},
    types::Coordinate,
};

pub enum TargetingSchema {
    NoArgs,
    SinglePoint,
    Custom,
}

pub trait Action {
    fn name(&self) -> &str;

    fn aliases(&self) -> Vec<&str>;

    fn targeting_schema(&self) -> TargetingSchema;

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
        let schema_validation = match self.targeting_schema() {
            TargetingSchema::NoArgs => {
                if target_ids != None {
                    return false;
                }
                if target_locations != None {
                    return false;
                }
                if overrides != None {
                    return false;
                }
                true
            }
            TargetingSchema::SinglePoint => {
                if target_ids != None {
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
            TargetingSchema::Custom => {
                panic!(
                    "custom targeting validation not implemented for {:?}",
                    self.name()
                )
            }
        };
        if !schema_validation {
            return false;
        }
        if let Some(cost) = self.cost(
            encounter,
            caster_id,
            target_ids,
            target_locations,
            overrides,
        ) {
            let actor = encounter.actors.get(&caster_id).expect("missing actor");
            if !actor.can_consume_resource(cost) {
                return false;
            }
        }
        return self.custom_validate_input(
            encounter,
            caster_id,
            target_ids,
            target_locations,
            overrides,
        );
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
            panic!(
                "Tried to execute action {:?} with illegal args",
                self.name()
            )
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
            action: action,
            caster_id: caster_id,
            target_ids: target_ids,
            target_locations: target_locations,
            overrides: overrides,
        }
    }

    pub fn validate(&self, encounter: &EncounterInstance) -> bool {
        return self.action.validate_input(
            encounter,
            self.caster_id,
            self.target_ids.as_ref(),
            self.target_locations.as_ref(),
            self.overrides.as_ref(),
        );
    }

    pub fn execute(&self, encounter: &mut EncounterInstance) -> Vec<Box<dyn ApplicableSideEffect>> {
        return self.action.execute(
            encounter,
            self.caster_id,
            self.target_ids.as_ref(),
            self.target_locations.as_ref(),
            self.overrides.as_ref(),
        );
    }
}
