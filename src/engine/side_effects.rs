use crate::engine::encounter::EncounterInstance;

pub trait ApplicableSideEffect {
    fn apply(&self, ei: &mut EncounterInstance);
}

pub struct MoveActor {
    actor_id: usize,
    target: (usize, usize),
}

impl MoveActor {
    pub fn apply(&self, ei: &mut EncounterInstance) {}
}
