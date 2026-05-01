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
    /// 0 (in 5e via Incapacitated, but we collapse for simplicity).
    Stunned,
    /// Disadvantage on attack rolls and ability checks.
    Poisoned,
    /// Can't see — auto-fail any check requiring sight, attacks against
    /// have advantage, attacks made have disadvantage.
    Blinded,
    /// Speed = 0; attacks against have advantage; you have disadvantage on
    /// attacks and DEX saves. Same speed clause as Prone (folded into
    /// `remaining_movement`).
    Restrained,
    /// Disadvantage on attacks and ability checks while you can see the
    /// source of fear (we conflate to "always, while frightened" — no
    /// per-source line-of-sight tracking yet).
    Frightened,
    /// You can't take attacks of opportunity vs your charmer; you have
    /// disadvantage on attacks against them. Marker only today — we apply
    /// the disadvantage clause globally rather than per-charmer because
    /// our combat doesn't model the source of charm.
    Charmed,
    /// Attacks against have disadvantage; attacks you make have advantage.
    /// (5e RAW also requires "the creature cannot see you," which we
    /// conflate to plain Invisible since our LOS is binary.)
    Invisible,
}

impl Condition {
    pub fn name(&self) -> &'static str {
        match self {
            Condition::Prone => "prone",
            Condition::Stunned => "stunned",
            Condition::Poisoned => "poisoned",
            Condition::Blinded => "blinded",
            Condition::Restrained => "restrained",
            Condition::Frightened => "frightened",
            Condition::Charmed => "charmed",
            Condition::Invisible => "invisible",
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
