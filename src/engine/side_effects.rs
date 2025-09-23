use crate::engine::encounter::EncounterInstance;

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

#[derive(Clone, PartialEq, Hash, Eq)]
pub struct MoveActor {
    pub actor_id: usize,
    pub target: (usize, usize),
}

impl ApplicableSideEffect for MoveActor {
    fn apply(&self, ei: &mut EncounterInstance) {
        ei.set_actor_map(self.actor_id, self.target)
            .expect("failed to move actor");
    }
}

#[derive(Clone, PartialEq, Hash, Eq)]
pub struct SkipTurn {}

impl ApplicableSideEffect for SkipTurn {
    fn apply(&self, ei: &mut EncounterInstance) {
        ei.skip_turn();
    }
}
