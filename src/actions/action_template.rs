use std::collections::HashSet;

use crate::engine::{
    action_overrides::ActionOverride, encounter::EncounterInstance,
    side_effects::ApplicableSideEffect,
};

pub trait Action {
    fn name(&self) -> &str;
    fn execute(
        &self,
        encounter: &mut EncounterInstance,
        caster_id: usize,
        target_ids: &Option<Vec<usize>>,
        target_locations: &Option<Vec<(usize, usize)>>,
        overrides: &HashSet<ActionOverride>,
    ) -> Vec<Box<dyn ApplicableSideEffect>>;
}

pub struct ActionExecutionInfo {
    action: &'static dyn Action,
    caster_id: usize,
    target_ids: Option<Vec<usize>>,
    target_locations: Option<Vec<(usize, usize)>>,
    overrides: HashSet<ActionOverride>,
}

impl ActionExecutionInfo {
    pub fn execute(&self, encounter: &mut EncounterInstance) -> Vec<Box<dyn ApplicableSideEffect>> {
        return self.action.execute(encounter, self.caster_id, &self.target_ids, &self.target_locations, &self.overrides)
    }
}
