/// Persistent status effects an actor can have. Mechanical impact lives
/// at the consumer (e.g. `ActorInstance::remaining_movement` returns 0
/// while `Prone`; `can_consume_resource` blocks Action/BonusAction/Reaction
/// while `Stunned`). New variants land here and then plug into the
/// relevant accessor — no central dispatcher.
///
/// Durations are tracked via `ConditionTimer`: `Permanent` requires
/// explicit removal, `Rounds(n)` decrements each time the initiative
/// queue wraps and clears at 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Condition {
    /// Speed = 0; ranged attacks against you have disadvantage; melee
    /// against you have advantage; you have disadvantage on attacks.
    Prone,
    /// Cannot take Actions, Bonus Actions, or Reactions. Movement is also
    /// 0 (in 5e via Incapacitated, but we collapse for simplicity).
    Stunned,
    /// Disadvantage on attack rolls and ability checks (we conflate ability
    /// checks with saves in `compute_save_mode`).
    Poisoned,
    /// Cannot move; attacks against you have advantage, your attacks have
    /// disadvantage, you fail DEX saves automatically (close enough — we
    /// don't model auto-fail, so we apply disadvantage on DEX saves
    /// instead via `compute_save_mode`).
    Restrained,
    /// Disadvantage on attack rolls vs the source of fear; can't willingly
    /// move closer to it. We don't track per-source state, so the
    /// universal effect is: disadvantage on all attacks.
    Frightened,
    /// Cannot see; attacks against you have advantage, your attacks have
    /// disadvantage, automatic fail on sight-based ability checks.
    Blinded,
    /// Cannot attack the charmer or target them with harmful abilities.
    /// We don't model the charmer reference today, so the marker is
    /// informational; it's still a 5e condition the AI can read.
    Charmed,
    /// Defending — if Dodge is used, attacks against you have disadvantage
    /// and you have advantage on DEX saves. Self-imposed for one round
    /// via the Dodge action; cleared at start of next turn.
    Dodging,
    /// Buffed (target of Bless): +1d4 on attack rolls and saves. Tied to
    /// the caster's concentration; we model the bonus as a flat +2 average
    /// to avoid dragging another d4 through every roll path — see
    /// `attack_bonus_from_conditions` and `save_bonus_from_conditions`.
    Blessed,
    /// Buffed (target of Shield of Faith): +2 AC. Tied to caster
    /// concentration — see `armor_class_bonus_from_conditions`.
    Shielded,
}

impl Condition {
    pub fn name(&self) -> &'static str {
        match self {
            Condition::Prone => "prone",
            Condition::Stunned => "stunned",
            Condition::Poisoned => "poisoned",
            Condition::Restrained => "restrained",
            Condition::Frightened => "frightened",
            Condition::Blinded => "blinded",
            Condition::Charmed => "charmed",
            Condition::Dodging => "dodging",
            Condition::Blessed => "blessed",
            Condition::Shielded => "shielded",
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
