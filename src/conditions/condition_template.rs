/// Persistent status effects an actor can have. Mechanical impact lives
/// at the consumer (e.g. `ActorInstance::remaining_movement` returns 0
/// while `Prone`; `can_consume_resource` blocks Action/BonusAction/Reaction
/// while `Stunned`). New variants land here and then plug into the
/// relevant accessor — no central dispatcher.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Condition {
    /// Speed = 0; ranged attacks against you have disadvantage; melee
    /// against you have advantage; you have disadvantage on attacks.
    Prone,
    /// Cannot take Actions, Bonus Actions, or Reactions. Movement is also
    /// 0 (in 5e via Incapacitated, but we collapse for simplicity). Attacks
    /// vs you have advantage; auto-fail STR/DEX saves.
    Stunned,
    /// Disadvantage on attack rolls and ability checks.
    Poisoned,
    /// Disadvantage on attack rolls and ability checks while you can see
    /// the source of fear. We don't track LOS-to-fear-source today; the
    /// effect is unconditional disadvantage on attacks.
    Frightened,
    /// Speed = 0; disadvantage on attacks; attacks against you have
    /// advantage; disadvantage on DEX saves.
    Restrained,
    /// Auto-fail any check requiring sight; attack rolls against you have
    /// advantage; you have disadvantage on attacks.
    Blinded,
    /// Cannot take actions or reactions (movement is still allowed).
    /// Distinct from Stunned — Incapacitated still moves; Stunned can't.
    Incapacitated,
    /// You have no effect on combat behavior — you can't take hostile
    /// actions against the charmer. Marker only today (we don't yet
    /// model the "can't attack the charmer" enforcement; the AI just
    /// avoids charmed targets via `is_pacified`).
    Charmed,
}

impl Condition {
    pub fn name(&self) -> &'static str {
        match self {
            Condition::Prone => "prone",
            Condition::Stunned => "stunned",
            Condition::Poisoned => "poisoned",
            Condition::Frightened => "frightened",
            Condition::Restrained => "restrained",
            Condition::Blinded => "blinded",
            Condition::Incapacitated => "incapacitated",
            Condition::Charmed => "charmed",
        }
    }

    /// True if this condition completely blocks Action / BonusAction /
    /// Reaction usage (5e's Incapacitated clause). Stunned and Paralyzed
    /// inherit this clause.
    pub fn blocks_action_economy(&self) -> bool {
        matches!(
            self,
            Condition::Stunned | Condition::Incapacitated | Condition::Paralyzed
        )
    }

    /// True if this condition zeros out movement.
    pub fn zeros_movement(&self) -> bool {
        matches!(
            self,
            Condition::Prone
                | Condition::Stunned
                | Condition::Restrained
                | Condition::Grappled
                | Condition::Paralyzed
        )
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
