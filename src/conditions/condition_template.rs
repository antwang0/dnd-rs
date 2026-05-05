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
    Prone,
    /// Cannot take Actions, Bonus Actions, or Reactions. Movement is also
    /// 0 (in 5e via Incapacitated, but we collapse for simplicity).
    Stunned,
    /// Disadvantage on attack rolls and saving throws.
    Poisoned,
    /// Speed = 0; attacks against you have advantage; your attacks have
    /// disadvantage; disadvantage on Dex saves. (Web, Hold spells, etc.)
    Restrained,
    /// You can't see. Disadvantage on attacks; attacks against have advantage.
    Blinded,
    /// You're unseen for combat purposes. Your attacks have advantage,
    /// attacks against have disadvantage (until you attack or cast).
    Invisible,
    /// Speed = 0; can't take Actions, Bonus Actions or Reactions.
    /// (Engine-wise functionally equivalent to Stunned, but separated so
    /// removal effects can target them independently.)
    Incapacitated,
    /// Disadvantage on attacks and ability checks while the source of fear
    /// is in line of sight. We approximate by always imposing disadvantage
    /// on attacks while the condition holds.
    Frightened,
    /// Speed = 0. Doesn't impose attack-roll modifiers. Cleared by an
    /// explicit Escape (not modeled — Remove on timer or by ally action).
    Grappled,
    /// Disadvantage on attacks against the charmer (we don't model the
    /// charmer relationship yet — marker only).
    Charmed,
    /// Self-buff from the Dodge action. Attacks against you have
    /// disadvantage; you have advantage on Dex saves. Lasts until the
    /// end of your next turn (1 round).
    Dodging,
    /// You ignore opportunity attacks for the rest of this turn (Disengage).
    /// Single-round timer.
    Disengaging,
    /// One-shot buff: your next attack roll has advantage (Help action).
    /// Cleared by `compute_attack_mode` after consumption.
    Helped,
    /// Bless concentration buff: +1d4 on attack rolls and saving throws.
    /// Bonus is applied in the attack/save resolution layer; this marker
    /// is the trigger.
    Blessed,
}

impl Condition {
    pub fn name(&self) -> &'static str {
        match self {
            Condition::Prone => "prone",
            Condition::Stunned => "stunned",
            Condition::Poisoned => "poisoned",
            Condition::Restrained => "restrained",
            Condition::Blinded => "blinded",
            Condition::Invisible => "invisible",
            Condition::Incapacitated => "incapacitated",
            Condition::Frightened => "frightened",
            Condition::Grappled => "grappled",
            Condition::Charmed => "charmed",
            Condition::Dodging => "dodging",
            Condition::Disengaging => "disengaging",
            Condition::Helped => "helped",
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
