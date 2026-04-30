/// Persistent status effects an actor can have. Mechanical impact lives
/// at the consumer (e.g. `ActorInstance::remaining_movement` returns 0
/// while `Prone`; `can_consume_resource` blocks Action/BonusAction/Reaction
/// while `Stunned`). New variants land here and then plug into the
/// relevant accessor — no central dispatcher.
///
/// Durations are encoded by the matching `ConditionTimer`: `Permanent`
/// requires explicit removal, `Rounds(n)` decrements every initiative
/// wrap and clears at zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Condition {
    /// Speed = 0; ranged attacks against you have disadvantage; melee
    /// against you have advantage; you have disadvantage on attacks.
    Prone,
    /// Cannot take Actions, Bonus Actions, or Reactions. Movement is also
    /// 0 (in 5e via Incapacitated, but we collapse for simplicity).
    /// Attacks against a stunned target have advantage.
    Stunned,
    /// Disadvantage on attack rolls and ability checks. Today wired for
    /// attacks and saves (we conflate save-vs-check).
    Poisoned,
    /// Disadvantage on your attacks; advantage on attacks against you.
    /// Auto-fails sight-based ability checks (we don't model those yet).
    Blinded,
    /// Speed = 0; disadvantage on your attacks and DEX saves; attacks
    /// against you have advantage.
    Restrained,
    /// Speed = 0. Otherwise normal — the grappler still fights, but in
    /// place. Attack mode unchanged for either side.
    Grappled,
    /// Your attacks have advantage; attacks against you have disadvantage.
    /// (We collapse "unseen" with full invisibility.)
    Invisible,
    /// Disadvantage on attacks (we don't yet model "while source is in
    /// LOS" — disadvantage applies categorically while the condition
    /// is active).
    Frightened,
    /// Cannot take Actions, Bonus Actions, or Reactions. Movement is
    /// allowed (this is the difference from Stunned: incapacitated keeps
    /// the legs but loses the action economy).
    Incapacitated,
    /// You can't attack the source of the charm or target them with
    /// harmful abilities. Without a "source" reference yet, we don't
    /// enforce the targeting clause; the variant exists so spells can
    /// apply it for future wiring.
    Charmed,
}

impl Condition {
    pub fn name(&self) -> &'static str {
        match self {
            Condition::Prone => "prone",
            Condition::Stunned => "stunned",
            Condition::Poisoned => "poisoned",
            Condition::Blinded => "blinded",
            Condition::Restrained => "restrained",
            Condition::Grappled => "grappled",
            Condition::Invisible => "invisible",
            Condition::Frightened => "frightened",
            Condition::Incapacitated => "incapacitated",
            Condition::Charmed => "charmed",
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
