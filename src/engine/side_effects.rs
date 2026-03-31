use crate::engine::encounter::EncounterInstance;
use crate::engine::types::Coordinate;

pub trait ApplicableSideEffect {
    fn apply(&self, ei: &mut EncounterInstance);
}

#[derive(Clone, PartialEq, Copy)]
pub enum Resource {
    Movement(f32),
    SpellSlot(u32),
    Action,
    BonusAction,
    Reaction,
    LegendaryAction,
}

#[derive(Clone, PartialEq, Copy)]
pub struct ConsumeResource {
    pub actor_id: usize,
    pub resource: Resource,
}

impl ApplicableSideEffect for ConsumeResource {
    fn apply(&self, ei: &mut EncounterInstance) {
        let actor = ei.actors.get_mut(&self.actor_id).unwrap();
        actor.consume_resource(self.resource);
    }
}

#[derive(Clone, PartialEq, Copy)]
pub struct GiveResource {
    pub actor_id: usize,
    pub resource: Resource,
}

impl ApplicableSideEffect for GiveResource {
    fn apply(&self, ei: &mut EncounterInstance) {
        let actor = ei.actors.get_mut(&self.actor_id).unwrap();
        actor.give_resource(self.resource);
    }
}

#[derive(Clone, PartialEq, Hash, Eq)]
pub struct MoveActor {
    pub actor_id: usize,
    pub target: Coordinate,
}

impl ApplicableSideEffect for MoveActor {
    fn apply(&self, ei: &mut EncounterInstance) {
        ei.set_actor_map(self.actor_id, self.target)
            .expect("failed to move actor");
        let actor = ei.actors.get_mut(&self.actor_id).expect("missing actor id");
        actor.set_location(self.target);
    }
}

#[derive(Clone, PartialEq, Hash, Eq)]
pub struct SkipTurn {}

impl ApplicableSideEffect for SkipTurn {
    fn apply(&self, ei: &mut EncounterInstance) {
        ei.skip_turn();
    }
}
