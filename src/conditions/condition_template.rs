/// Persistent status effects an actor can have. Mechanical impact lives
/// at the consumer (e.g. `ActorInstance::remaining_movement` returns 0
/// while `Prone`; `can_consume_resource` blocks Action/BonusAction/Reaction
/// while `Stunned`). New variants land here and then plug into the
/// relevant accessor — no central dispatcher.
///
/// Durations are tracked via `ConditionTimer`. `Permanent` entries persist
/// until explicit removal; `Rounds(n)` ticks down each round-end and clears
/// at 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Condition {
    /// Speed = 0; ranged attacks against you have disadvantage; melee
    /// against you have advantage; you have disadvantage on attacks.
    Prone,
    /// Cannot take Actions, Bonus Actions, or Reactions. Movement is also
    /// 0 (in 5e via Incapacitated, but we collapse for simplicity).
    /// Attacks against a Stunned target have advantage.
    Stunned,
    /// Disadvantage on attack rolls and ability checks (incl. saves we
    /// derive from checks).
    Poisoned,
    /// Disadvantage on attack rolls; advantage on saves vs. effects from
    /// the source of fear (we conflate to "any saves" since we don't
    /// track fear-source today). Speed unchanged in our model.
    Frightened,
    /// Speed = 0; disadvantage on attack rolls; attacks against you have
    /// advantage; disadvantage on DEX saves.
    Restrained,
    /// Cannot see — auto-fail any check that requires sight; attack rolls
    /// against you have advantage; your attack rolls have disadvantage.
    Blinded,
    /// Attacks against you have disadvantage; your attack rolls have
    /// advantage. Doesn't reveal location automatically — but in our
    /// gridded model the position is always known.
    Invisible,
    /// You can't attack the charmer or target them with harmful abilities.
    /// We model the disposition flag only; the AI consults it.
    Charmed,
    /// Speed = 0. Doesn't impose advantage / disadvantage on its own in
    /// 5e, but combined with attack-roll context the actor is much
    /// easier to hit while immobilized.
    Grappled,
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
            Condition::Invisible => "invisible",
            Condition::Charmed => "charmed",
            Condition::Grappled => "grappled",
        }
    }

    /// True if this condition imposes any movement penalty (speed = 0 in
    /// 5e). Engine consults this when computing `remaining_movement`.
    pub fn immobilizes(&self) -> bool {
        matches!(
            self,
            Condition::Prone
                | Condition::Stunned
                | Condition::Restrained
                | Condition::Grappled
        )
    }

    /// True if this condition blocks action-economy (Action / Bonus /
    /// Reaction). Stunned is the canonical case; we don't yet model the
    /// "incapacitated" parent condition separately.
    pub fn locks_actions(&self) -> bool {
        matches!(self, Condition::Stunned)
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
