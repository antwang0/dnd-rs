use crate::{actors::actor_template::ActorInstance, engine::encounter::EncounterInstance};

pub trait ApplicableSideEffect {
    fn apply(&mut self, ei: &mut EncounterInstance);
}

struct MoveActor {
    actor_id: usize,
    target: (usize, usize),
}

impl MoveActor {
    pub fn apply(&mut self, ei: &mut EncounterInstance) {}
}
