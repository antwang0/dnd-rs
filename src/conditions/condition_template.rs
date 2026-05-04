/// Persistent status effects an actor can have. Mechanical impact lives
/// at the consumer (e.g. `ActorInstance::remaining_movement` returns 0
/// while `Prone`; `can_consume_resource` blocks Action/BonusAction/Reaction
/// while `Stunned`). New variants land here and then plug into the
/// relevant accessor — no central dispatcher.
///
/// Durations are tracked via `ConditionTimer`: `Permanent` persists
/// until explicit removal, `Rounds(n)` ticks down on every initiative
/// wrap and clears at 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Condition {
    /// Speed = 0; ranged attacks against you have disadvantage; melee
    /// against you have advantage; you have disadvantage on attacks.
    /// Today only the speed clause is wired.
    Prone,
    /// Cannot take Actions, Bonus Actions, or Reactions. Movement is also
    /// 0 (in 5e via Incapacitated, but we collapse for simplicity).
    Stunned,
    /// Disadvantage on attack rolls and ability checks.
    Poisoned,
    /// Disadvantage on attack rolls and ability checks. Critically: you
    /// can't willingly move closer to the source of your fear (we model
    /// the simpler "any enemy" version since we don't track per-source
    /// fear yet — a Frightened actor's `Move` action validates the
    /// destination doesn't decrease distance to *any* enemy).
    Frightened,
    /// Speed = 0, attacks against you have advantage, you have
    /// disadvantage on attacks and DEX saves. Imposed by spells like
    /// Web / Hold Monster / a grappler with reach.
    Restrained,
    /// Attacks against you have disadvantage; your attacks have
    /// advantage. Other clauses (silent movement, can't be targeted by
    /// sight-based effects) are flavor we don't yet model.
    Invisible,
    /// Auto-fail any check that requires sight. Attacks against you have
    /// advantage; your attacks have disadvantage. Stacks with everything
    /// — most often imposed by darkness or a Color Spray-style burst.
    Blinded,
}

impl Condition {
    pub fn name(&self) -> &'static str {
        match self {
            Condition::Prone => "prone",
            Condition::Stunned => "stunned",
            Condition::Poisoned => "poisoned",
            Condition::Frightened => "frightened",
            Condition::Restrained => "restrained",
            Condition::Invisible => "invisible",
            Condition::Blinded => "blinded",
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
