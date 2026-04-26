use crate::engine::types::Coordinate;

/// Concrete in-world events that reactions can fire on. Add a variant per
/// reaction type as we wire more in (attack rolls, spell casts, damage
/// taken, etc.). Today only opportunity attacks consume this.
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
