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
    /// Attack rolls against the creature have advantage; the creature has
    /// disadvantage on attack rolls and auto-fails any check that requires
    /// sight (we model the attack-roll clauses; sight checks aren't a
    /// thing in this codebase yet).
    Blinded,
    /// Disadvantage on attack rolls and ability checks while the source of
    /// fear is in line of sight. We approximate with a flat disadvantage on
    /// attacks (the fear source isn't tracked per-condition yet).
    Frightened,
    /// Speed = 0; attacks against the creature have advantage; the creature
    /// has disadvantage on attacks and DEX saves.
    Restrained,
    /// Speed = 0; otherwise normal. Doesn't impose advantage/disadvantage
    /// directly. Used by spells like Hold Person's lighter cousin (e.g.
    /// being grabbed but not pinned).
    Grappled,
    /// Cannot be seen without special senses; attacks against the creature
    /// have disadvantage; the creature's attacks have advantage.
    Invisible,
    /// Bestowed by the Bless spell — +1d4 to attack rolls and saves.
    /// Modeled as a flat +2 (average of 1d4) bonus on the relevant
    /// rolls rather than rolling an extra die; keeps the math cheap and
    /// the condition stays a marker-only flag.
    Blessed,
    /// Bestowed by the Dodge action — attacks against the creature have
    /// disadvantage and the creature has advantage on DEX saves until the
    /// start of its next turn. We model the attack-disadvantage clause;
    /// the DEX-save clause is wired in `compute_save_mode`.
    Dodging,
}

impl Condition {
    pub fn name(&self) -> &'static str {
        match self {
            Condition::Prone => "prone",
            Condition::Stunned => "stunned",
            Condition::Poisoned => "poisoned",
            Condition::Blinded => "blinded",
            Condition::Frightened => "frightened",
            Condition::Restrained => "restrained",
            Condition::Grappled => "grappled",
            Condition::Invisible => "invisible",
            Condition::Blessed => "blessed",
            Condition::Dodging => "dodging",
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
