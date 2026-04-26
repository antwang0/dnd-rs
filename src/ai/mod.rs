pub mod simple;

use crate::actions::action_template::ActionExecutionInfo;
use crate::engine::encounter::EncounterInstance;

pub use self::simple::SimpleAi;

/// What a controller wants to happen for the actor whose turn it is.
pub enum ControllerDecision {
    /// Defer to external (player) input — the App should pump the keyboard.
    AwaitInput,
    /// Run this action immediately. The caller pops the prompt and enqueues
    /// the action; the engine will surface the next prompt.
    Act(ActionExecutionInfo),
}

/// A turn-by-turn decision-maker for one or more actors. Implementations may
/// be stateless (e.g. greedy heuristics) or hold scratch state across turns
/// (planners, BTs, learned policies). The trait intentionally takes only an
/// `&EncounterInstance` so controllers cannot mutate engine state directly —
/// they communicate intent via `ControllerDecision::Act`.
pub trait Controller: Send + Sync {
    fn decide(&self, encounter: &EncounterInstance, actor_id: usize) -> ControllerDecision;
}

/// Sentinel controller for human-driven teams — always defers to input.
pub struct PlayerController;

impl Controller for PlayerController {
    fn decide(&self, _encounter: &EncounterInstance, _actor_id: usize) -> ControllerDecision {
        ControllerDecision::AwaitInput
    }
}
