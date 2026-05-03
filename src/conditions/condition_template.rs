/// Persistent status effects an actor can have. Mechanical impact lives
/// at the consumer (e.g. `ActorInstance::remaining_movement` returns 0
/// while `Prone`; `can_consume_resource` blocks Action/BonusAction/Reaction
/// while `Stunned`). New variants land here and then plug into the
/// relevant accessor — no central dispatcher.
///
/// `Permanent` durations persist until explicitly removed; `Rounds(n)`
/// timers tick down on the round-end hook (initiative wrap) and clear
/// when they hit zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Condition {
    /// Speed = 0; ranged attacks against you have disadvantage; melee
    /// against you have advantage; you have disadvantage on attacks.
    Prone,
    /// Cannot take Actions, Bonus Actions, or Reactions. Movement is also
    /// 0 (in 5e via Incapacitated, but we collapse for simplicity).
    Stunned,
    /// Disadvantage on attack rolls and ability checks. Marker only today
    /// (no advantage/disadvantage system yet).
    Poisoned,
    /// Speed = 0; attackers have advantage; the target has disadvantage
    /// on attacks and on DEX saves. Common rider on grapples / nets.
    Restrained,
    /// Self can't see — disadvantage on attacks, attackers have advantage.
    /// Doesn't auto-fail Sight checks (we don't model checks granularly).
    Blinded,
    /// You're Dodging until the start of your next turn: attackers have
    /// disadvantage on you and you have advantage on DEX saves. Cleared
    /// at the top of your own turn (`reset_for_new_round`). Costs an
    /// Action to apply.
    Dodging,
    /// You took the Disengage action this turn — your movement doesn't
    /// provoke opportunity attacks. Cleared at the top of your next
    /// turn (`reset_for_new_round`).
    Disengaged,
}

impl Condition {
    pub fn name(&self) -> &'static str {
        match self {
            Condition::Prone => "prone",
            Condition::Stunned => "stunned",
            Condition::Poisoned => "poisoned",
            Condition::Restrained => "restrained",
            Condition::Blinded => "blinded",
            Condition::Dodging => "dodging",
            Condition::Disengaged => "disengaged",
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
