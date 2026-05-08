/// Persistent status effects an actor can have. Mechanical impact lives
/// at the consumer (e.g. `ActorInstance::remaining_movement` returns 0
/// while `Prone`; `can_consume_resource` blocks Action/BonusAction/Reaction
/// while `Stunned`). New variants land here and then plug into the
/// relevant accessor — no central dispatcher.
///
/// Round-tracked durations expire on round-end (initiative wraparound).
/// `Permanent` timers persist until something explicitly removes the
/// condition (e.g. Stand-up clears Prone, Lesser Restoration clears
/// Poisoned).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Condition {
    /// Speed = 0; ranged attacks against you have disadvantage; melee
    /// against you have advantage; you have disadvantage on attacks.
    Prone,
    /// Cannot take Actions, Bonus Actions, or Reactions. Movement is also
    /// 0 (in 5e via Incapacitated, but we collapse for simplicity).
    /// Attacks against a stunned target have advantage.
    Stunned,
    /// Disadvantage on attack rolls and ability checks (which we conflate
    /// with saving throws).
    Poisoned,
    /// Disadvantage on attack rolls; attacks against you have advantage.
    /// Auto-fail any check that requires sight.
    Blinded,
    /// Disadvantage on attacks against creatures other than the source of
    /// your fear; cannot willingly move closer to the source.
    /// (Source-tracking is not modeled today; we just apply disadvantage
    /// on all attacks.)
    Frightened,
    /// Speed = 0; disadvantage on attacks; attacks against you have
    /// advantage; disadvantage on DEX saves.
    Restrained,
    /// Auto-pass STR / DEX saves are not modeled — but cannot attack the
    /// grappler, and speed = 0. We use this for grapple-style locks.
    Grappled,
    /// Cannot take any actions, bonus actions, or reactions. Distinct
    /// from Stunned — Incapacitated does not grant attackers advantage.
    Incapacitated,
    /// Charm caster cannot be a target of attacks from the charmed
    /// creature; charm caster has advantage on social checks against
    /// them. We model only the "no attacks against caster" rule
    /// implicitly — full charm-source tracking is out of scope.
    Charmed,
    /// Attackers cannot see this actor → attacks against them have
    /// disadvantage; their attacks have advantage. Movement and stealth
    /// otherwise unaffected.
    Invisible,
    /// Out of HP, prone, can't take actions. Used for the dying/stable
    /// state to roll over to attack-mode logic (auto-fail STR/DEX saves,
    /// etc.). Today we don't apply this automatically — the death-save
    /// path uses HpState — but the variant is here for spells like
    /// Sleep that need to set it directly.
    Unconscious,
    /// Cannot hear; auto-fails checks that depend on hearing. No combat
    /// effect today; included for spell completeness (Silence).
    Deafened,
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
            Condition::Incapacitated => "incapacitated",
            Condition::Charmed => "charmed",
            Condition::Invisible => "invisible",
            Condition::Unconscious => "unconscious",
            Condition::Deafened => "deafened",
        }
    }

    /// True when this condition removes the entire action economy
    /// (Action / BonusAction / Reaction). Stunned, Incapacitated, and
    /// Unconscious all share this clause; centralized so
    /// `can_consume_resource` doesn't grow a giant `||` chain.
    pub fn blocks_action_economy(&self) -> bool {
        matches!(
            self,
            Condition::Stunned | Condition::Incapacitated | Condition::Unconscious
        )
    }

    /// True when this condition zeros the actor's movement. Prone is
    /// special-cased separately for stand-up reasons; this list covers
    /// the "speed = 0" conditions cleanly.
    pub fn zeroes_movement(&self) -> bool {
        matches!(
            self,
            Condition::Stunned
                | Condition::Restrained
                | Condition::Grappled
                | Condition::Incapacitated
                | Condition::Unconscious
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
