use crate::engine::types::Coordinate;

/// Concrete in-world events that reactions can fire on. Add a variant per
/// reaction type as we wire more in (attack rolls, spell casts, damage
/// taken, etc.).
///
/// One variant, two consumers, and they read it from opposite ends: an
/// opportunity attack fires on a creature *leaving* somebody's reach, a
/// readied attack on a creature *entering* it. Both questions are
/// answered by the same `(from, to)` pair, which is why the readied
/// attack needed no new event — only the other comparison.
#[derive(Debug, Clone, Copy)]
pub enum TriggerEvent {
    /// An actor is about to move from `from` to `to`. Fired before the
    /// `MoveActor` side effect actually shifts the actor on the map, so
    /// reactions can target them at their pre-move location.
    ActorLeaving {
        actor_id: usize,
        from: Coordinate,
        to: Coordinate,
    },
}
