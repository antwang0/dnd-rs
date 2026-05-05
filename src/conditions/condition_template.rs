/// Persistent status effects an actor can have. Mechanical impact lives
/// at the consumer (e.g. `ActorInstance::remaining_movement` returns 0
/// while `Prone`; `can_consume_resource` blocks Action/BonusAction/Reaction
/// while `Stunned`). New variants land here and then plug into the
/// relevant accessor — no central dispatcher.
///
/// Round timers tick on the initiative-queue wrap (see
/// `EncounterInstance::round_end`); `Permanent` requires explicit removal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Condition {
    /// Speed = 0; ranged attacks against you have disadvantage; melee
    /// against you have advantage; you have disadvantage on attacks.
    Prone,
    /// Cannot take Actions, Bonus Actions, or Reactions; auto-fail STR/DEX
    /// saves; attacks against you have advantage; you can't move.
    Stunned,
    /// Disadvantage on attack rolls and ability checks.
    Poisoned,
    /// Auto-fail attack rolls that need sight; attacks against you have
    /// advantage; your attacks have disadvantage.
    Blinded,
    /// While the source of the fear is in line of sight: disadvantage on
    /// ability checks and attack rolls; can't willingly move closer to the
    /// source. We don't track the fear source by id today, so the LOS
    /// clause is approximated as "any visible enemy."
    Frightened,
    /// Speed 0; attacks against you have advantage; you have disadvantage
    /// on attacks and DEX saves.
    Restrained,
    /// Can't take Actions / Bonus Actions / Reactions. Doesn't zero out
    /// movement (different from Stunned).
    Incapacitated,
    /// Out cold: `Incapacitated` plus auto-fail STR/DEX saves, attackers
    /// have advantage, melee crit on hit, and you drop prone. Used by
    /// `Sleep`, `KO from non-lethal damage`, etc. We collapse the prone
    /// rider for now (Prone is a separate condition; the engine doesn't
    /// auto-attach it).
    Unconscious,
    /// Can't be seen without special senses; attackers have disadvantage,
    /// your attacks have advantage. (We don't yet model Truesight defeating
    /// it — every attacker treats invisible targets the same way.)
    Invisible,
    /// Held in place by another creature: speed 0. We don't yet track the
    /// grappler's id; ending the grapple is a manual `RemoveCondition`.
    Grappled,
    /// Charmed: can't attack the charmer, charmer has advantage on social
    /// checks. We only model the "can't attack source" half today, but
    /// without a source-tracker the AI just skips harming charmed targets
    /// of its own making.
    Charmed,
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
            Condition::Incapacitated => "incapacitated",
            Condition::Unconscious => "unconscious",
            Condition::Invisible => "invisible",
            Condition::Grappled => "grappled",
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
