/// Persistent status effects an actor can have. Mechanical impact lives
/// at the consumer (e.g. `ActorInstance::remaining_movement` returns 0
/// while `Prone`; `can_consume_resource` blocks Action/BonusAction/Reaction
/// while `Stunned`). New variants land here and then plug into the
/// relevant accessor — no central dispatcher.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Condition {
    /// Speed = 0; ranged attacks against you have disadvantage; melee
    /// against you have advantage; you have disadvantage on attacks.
    /// Today only the speed clause is wired.
    Prone,
    /// Cannot take Actions, Bonus Actions, or Reactions. Movement is also
    /// 0 (in 5e via Incapacitated, but we collapse for simplicity).
    Stunned,
    /// Disadvantage on attack rolls and ability checks. Marker only today
    /// (no advantage/disadvantage system yet).
    Poisoned,
    /// Cannot see; automatically fails sight-based checks. Attack rolls
    /// against a blinded creature have advantage, and the blinded
    /// creature's attack rolls have disadvantage.
    Blinded,
    /// Marker for a creature that took the Dodge action this turn.
    /// Attackers roll at disadvantage; DEX saves gain advantage. Cleared
    /// automatically when the creature's next turn starts.
    Dodging,
    /// Marker for a creature that took the Disengage action this turn —
    /// their movement doesn't provoke opportunity attacks until the start
    /// of their next turn. Consumed by the OA reactor.
    Disengaged,
    /// Marker for a creature that has been Helped — their next attack
    /// rolls at advantage, then the buff is consumed. See
    /// `EncounterInstance::compute_attack_mode`.
    Helped,
}

impl Condition {
    pub fn name(&self) -> &'static str {
        match self {
            Condition::Prone => "prone",
            Condition::Stunned => "stunned",
            Condition::Poisoned => "poisoned",
            Condition::Blinded => "blinded",
            Condition::Dodging => "dodging",
            Condition::Disengaged => "disengaged",
            Condition::Helped => "helped",
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
