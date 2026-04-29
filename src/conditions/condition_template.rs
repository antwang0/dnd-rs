/// Persistent status effects an actor can have. Mechanical impact lives
/// at the consumer (e.g. `ActorInstance::remaining_movement` returns 0
/// while `Prone`; `can_consume_resource` blocks Action/BonusAction/Reaction
/// while `Stunned`). New variants land here and then plug into the
/// relevant accessor — no central dispatcher.
///
/// Durations: `ConditionTimer::Permanent` requires explicit removal;
/// `ConditionTimer::Rounds(n)` ticks down each round-end.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Condition {
    /// Speed = 0; ranged attacks against you have disadvantage; melee
    /// against you have advantage; you have disadvantage on attacks.
    Prone,
    /// Cannot take Actions, Bonus Actions, or Reactions. Movement is also
    /// 0 (in 5e via Incapacitated, but we collapse for simplicity).
    Stunned,
    /// Disadvantage on attack rolls and ability checks (we lump saves in
    /// as ability-derived).
    Poisoned,
    /// Disadvantage on attack rolls against creatures other than the
    /// fear source; cannot willingly move closer to it. We don't track
    /// the source, so the simplification is "disadvantage on all
    /// attacks". Free fear effects skip the willing-move clause.
    Frightened,
    /// Speed 0, attack rolls vs you have advantage, you have disadvantage
    /// on attacks and DEX saves.
    Restrained,
    /// Speed 0. Ends when the grappler is incapacitated or moved out
    /// of reach (not modelled — clears on timer).
    Grappled,
    /// Auto-fail vision-based ability checks; attacks vs you have
    /// advantage; you have disadvantage on attacks.
    Blinded,
    /// Cannot take Actions, Bonus Actions, or Reactions. Distinct from
    /// `Stunned` because Stunned also auto-fails STR/DEX saves; for now
    /// they share most effects but `Incapacitated` does NOT impose
    /// disadvantage on saves.
    Incapacitated,
    /// Attack rolls against you have disadvantage; your attacks have
    /// advantage. We don't model perception-based reveals.
    Invisible,
    /// Concentrated focus: this turn, attack rolls against you have
    /// disadvantage (not the canonical Dodge action's "DEX saves with
    /// advantage" because we don't model action-economy save buffs).
    /// Cleared at the start of the actor's next turn.
    Dodging,
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
            Condition::Incapacitated => "incapacitated",
            Condition::Invisible => "invisible",
            Condition::Dodging => "dodging",
        }
    }

    /// True if this condition denies all action-economy slots (Action,
    /// BonusAction, Reaction) and movement. Used by `can_consume_resource`.
    pub fn is_incapacitating(&self) -> bool {
        matches!(self, Condition::Stunned | Condition::Incapacitated)
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
