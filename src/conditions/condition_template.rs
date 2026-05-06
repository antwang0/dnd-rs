/// Persistent status effects an actor can have. Mechanical impact lives
/// at the consumer (e.g. `ActorInstance::remaining_movement` returns 0
/// while `Prone`; `can_consume_resource` blocks Action/BonusAction/Reaction
/// while `Stunned`). New variants land here and then plug into the
/// relevant accessor — no central dispatcher.
///
/// Durations aren't tracked yet: conditions persist until something
/// explicitly removes them via `RemoveCondition`. Round-tracked durations
/// (e.g. "stunned for 1 round") need a turn-end hook the engine doesn't
/// have yet; deferred.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Condition {
    /// Speed = 0; ranged attacks against you have disadvantage; melee
    /// against you have advantage; you have disadvantage on attacks.
    /// Today only the speed clause is wired.
    Prone,
    /// Cannot take Actions, Bonus Actions, or Reactions. Movement is also
    /// 0 (in 5e via Incapacitated, but we collapse for simplicity).
    Stunned,
    /// Disadvantage on attack rolls and ability checks. Marker only today
    /// (no advantage/disadvantage system yet).
    Poisoned,
    /// Attack rolls against you have advantage; your attack rolls have
    /// disadvantage; auto-fail any check requiring sight. Modeled here as
    /// the attack mode adjustments only — sight checks aren't a thing yet.
    Blinded,
    /// Disadvantage on ability checks and attack rolls while you can see
    /// the source of your fear. We don't track "source of fear" so we
    /// always apply the disadvantage.
    Frightened,
    /// Speed = 0; attacks against you have advantage; your attacks have
    /// disadvantage; disadvantage on DEX saves. The speed/0 clause is
    /// shared with `Prone` via `remaining_movement`.
    Restrained,
    /// Cannot take Actions, Bonus Actions, or Reactions. Same action-
    /// economy block as Stunned, but doesn't impose attack-mode penalties
    /// or zero movement.
    Incapacitated,
    /// Cannot be seen without special senses. Attack rolls against you
    /// have disadvantage; your attack rolls have advantage. Wears off when
    /// you attack or cast a spell, but we don't auto-clear yet.
    Invisible,
    /// 0 HP, prone, can't move/speak. Attacks within 5ft auto-crit, all
    /// auto-fail STR/DEX saves. Set when an actor enters Dying — we don't
    /// auto-add it today; reserved for explicit application by future
    /// effects (e.g. Sleep spell).
    Unconscious,
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
            Condition::Invisible => "invisible",
            Condition::Unconscious => "unconscious",
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
