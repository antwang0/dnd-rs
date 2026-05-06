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
    /// 5e: disadvantage on ability checks and attack rolls while the
    /// source of fear is in line of sight; cannot willingly move closer
    /// to the source. We don't track the fear source today, so we apply
    /// the attack-disadvantage clause unconditionally and skip the
    /// movement clause.
    Frightened,
    /// 5e Bless target: +1d4 to attack rolls and saving throws. Each
    /// invocation rolls the d4 fresh — no pre-rolled bonus carried on
    /// the actor. The engine reads this flag in weapon_attack and
    /// roll_save.
    Blessed,
}

impl Condition {
    pub fn name(&self) -> &'static str {
        match self {
            Condition::Prone => "prone",
            Condition::Stunned => "stunned",
            Condition::Poisoned => "poisoned",
            Condition::Frightened => "frightened",
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
