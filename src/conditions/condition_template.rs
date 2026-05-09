/// Persistent status effects an actor can have. Mechanical impact lives
/// at the consumer (e.g. `ActorInstance::remaining_movement` returns 0
/// while `Prone`; `can_consume_resource` blocks Action/BonusAction/Reaction
/// while `Stunned`). New variants land here and then plug into the
/// relevant accessor — no central dispatcher.
///
/// Durations aren't tracked yet: conditions persist until something
/// explicitly removes them via `RemoveCondition`. Round-tracked durations
/// (e.g. "stunned for 1 round") need a turn-end hook the engine doesn't
/// have yet; deferred.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Condition {
    /// Speed = 0; ranged attacks against you have disadvantage; melee
    /// against you have advantage; you have disadvantage on attacks.
    Prone,
    /// Cannot take Actions, Bonus Actions, or Reactions. Movement is also
    /// 0 (in 5e via Incapacitated, but we collapse for simplicity). Also
    /// auto-fails STR and DEX saves while stunned.
    Stunned,
    /// Disadvantage on attack rolls and ability checks. (5e also imposes
    /// disadv on saves derived from ability checks; we apply that to all
    /// saves today since save-vs-check distinction isn't modeled.)
    Poisoned,
    /// Disadvantage on attack rolls and ability checks while you can see
    /// the source of your fear. We don't model line-of-sight to the
    /// fearful source — Frightened just imposes the disadv unconditionally
    /// while present. Fearful actors also can't willingly move closer to
    /// the source; the engine doesn't track per-source fear today, so
    /// movement clauses are deferred.
    Frightened,
    /// Speed = 0; attacks against you have advantage; you have disadv
    /// on attacks; disadvantage on DEX saves. Common rider on grapples,
    /// webs, and ensnaring effects.
    Restrained,
    /// Speed = 0. Less limiting than Restrained — only the movement
    /// clause is implemented today (5e adds attack-against-grappled
    /// nuance, but the basic hold is the speed lock).
    Grappled,
    /// Attacks vs you have advantage; you have disadv on attacks vs anyone.
    /// Auto-fails sight-based ability checks. We collapse "can't see" to
    /// the attack-mode clauses since hidden-target mechanics aren't
    /// otherwise modeled.
    Blinded,
    /// Defensive Dodge stance: any attack roll against you has disadvantage,
    /// and you make DEX saves with advantage. Cleared on the actor's next
    /// turn (set when the action runs; the round-end tick decrements its
    /// `Rounds` timer to expiry).
    Dodging,
    /// Disengaging: opportunity attacks triggered by the actor's movement
    /// don't fire while this condition is active. Same lifecycle as Dodging
    /// — set by the action, cleared on the next turn-tick.
    Disengaging,
}

impl Condition {
    pub fn name(&self) -> &'static str {
        match self {
            Condition::Prone => "prone",
            Condition::Stunned => "stunned",
            Condition::Poisoned => "poisoned",
            Condition::Frightened => "frightened",
            Condition::Restrained => "restrained",
            Condition::Grappled => "grappled",
            Condition::Blinded => "blinded",
            Condition::Dodging => "dodging",
            Condition::Disengaging => "disengaging",
        }
    }
}

/// How long a condition application persists. `Permanent` requires an
/// explicit removal (e.g. Stand-up clears Prone, Lesser Restoration clears
/// Poisoned). `Rounds(n)` ticks down by 1 every time the initiative queue
/// wraps; the condition is removed when the timer hits 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConditionTimer {
    Permanent,
    Rounds(u32),
}
