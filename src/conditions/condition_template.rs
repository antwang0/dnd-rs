/// Persistent status effects an actor can have. Mechanical impact lives
/// at the consumer (e.g. `ActorInstance::remaining_movement` returns 0
/// while `Prone`; `can_consume_resource` blocks Action/BonusAction/Reaction
/// while `Stunned`). New variants land here and then plug into the
/// relevant accessor — no central dispatcher.
///
/// Durations are tracked via [`ConditionTimer`]; permanent applications
/// require an explicit removal action (e.g. Stand-up clears Prone, Lesser
/// Restoration clears Poisoned).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Condition {
    /// Speed = 0; ranged attacks against you have disadvantage; melee
    /// against you have advantage; you have disadvantage on attacks.
    Prone,
    /// Cannot take Actions, Bonus Actions, or Reactions. Movement is also
    /// 0 (in 5e via Incapacitated, but we collapse for simplicity).
    Stunned,
    /// Disadvantage on attack rolls and ability checks. Disadvantage on
    /// saves (the engine treats the save → check distinction loosely).
    Poisoned,
    /// Attack rolls against the creature have advantage; the creature has
    /// disadvantage on attack rolls. Auto-fails any check that requires
    /// sight (we don't model checks yet, so only the attack clauses are
    /// wired today).
    Blinded,
    /// Speed = 0 (no movement); attacks against = advantage; attack rolls
    /// = disadvantage; DEX saves = disadvantage.
    Restrained,
    /// Speed = 0 (the attacker is gripping you). No effect on attack rolls
    /// directly — we collapse the "until escape" mechanic by using a timer.
    Grappled,
    /// Disadvantage on ability checks and attack rolls while the source of
    /// fear is in line of sight. We don't track the fear source today, so
    /// the disadvantage applies whenever Frightened is present (worst-case
    /// for the frightened actor, which is the right default for combat).
    Charmed,
    /// Disadvantage on ability checks and attack rolls. Cannot willingly
    /// move closer to the source of fear. Same source-tracking caveat as
    /// `Charmed`.
    Frightened,
    /// Cannot take Actions or Reactions. Implies the action-economy clauses
    /// of `Stunned` minus the auto-fail STR/DEX saves; movement is allowed.
    Incapacitated,
    /// Attacks against = disadvantage; attacks made = advantage. Tracked
    /// as a marker — turn-end reveal mechanics aren't modeled.
    Invisible,
    /// Shield of Faith aura — +2 to AC while present. Concentration spell;
    /// dropped when the caster's concentration ends.
    ShieldedByFaith,
    /// Bless aura — +1d4 to attack rolls and saves while present. The d4
    /// is rolled at use-time; we stick the bonus into compute_attack_mode
    /// and roll_save by reading the condition.
    Blessed,
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
            Condition::Charmed => "charmed",
            Condition::Frightened => "frightened",
            Condition::Incapacitated => "incapacitated",
            Condition::Invisible => "invisible",
            Condition::ShieldedByFaith => "shielded by faith",
            Condition::Blessed => "blessed",
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
