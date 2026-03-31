use std::collections::HashSet;

use crate::engine::{
    action_overrides::ActionOverride, encounter::EncounterInstance,
    side_effects::ApplicableSideEffect,
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

    fn execute_impl(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<(usize, usize)>>,
        overrides: Option<&HashSet<ActionOverride>>,
    ) -> Vec<Box<dyn ApplicableSideEffect>>;

    fn validate_input(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<(usize, usize)>>,
        overrides: Option<&HashSet<ActionOverride>>,
    ) -> bool {
        // TODO: overrides
        match self.targeting_schema() {
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
                }
                true
            }
            TargetingSchema::Custom => {
                panic!(
                    "custom targeting validation not implemented for {:?}",
                    self.name()
                )
            }
        }
    }

    fn execute(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: Option<&Vec<usize>>,
        target_locations: Option<&Vec<(usize, usize)>>,
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
        self.execute_impl(
            encounter,
            caster_id,
            target_ids,
            target_locations,
            overrides,
        )
    }
}

pub struct ActionExecutionInfo {
    action: &'static dyn Action,
    caster_id: usize,
    target_ids: Option<Vec<usize>>,
    target_locations: Option<Vec<(usize, usize)>>,
    overrides: Option<HashSet<ActionOverride>>,
}

impl ActionExecutionInfo {
    pub fn new(
        action: &'static dyn Action,
        caster_id: usize,
        target_ids: Option<Vec<usize>>,
        target_locations: Option<Vec<(usize, usize)>>,
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
