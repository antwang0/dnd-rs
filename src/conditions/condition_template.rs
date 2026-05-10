/// Persistent status effects an actor can have. Mechanical impact lives
/// at the consumer (e.g. `ActorInstance::remaining_movement` returns 0
/// while `Prone`; `can_consume_resource` blocks Action/BonusAction/Reaction
/// while `Stunned`). New variants land here and then plug into the
/// relevant accessor — no central dispatcher.
///
/// Durations are tracked via `ConditionTimer`: `Permanent` persists until
/// explicit removal; `Rounds(n)` ticks down on every initiative wrap and
/// clears at 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Condition {
    /// Speed = 0; ranged attacks against you have disadvantage; melee
    /// against you have advantage; you have disadvantage on attacks.
    Prone,
    /// Cannot take Actions, Bonus Actions, or Reactions. Movement is also
    /// 0 (in 5e via Incapacitated, but we collapse for simplicity).
    Stunned,
    /// Disadvantage on attack rolls and ability checks (saves derived
    /// from those checks).
    Poisoned,
    /// Disadvantage on attack rolls; disadvantage on saves vs the source
    /// of the fear (we don't model the source today, so it's just attack
    /// disadvantage). Movement isn't blocked — fear-driven flight is left
    /// to the AI.
    Frightened,
    /// Speed = 0; attacks against you have advantage; you have
    /// disadvantage on attacks; disadvantage on DEX saves.
    Restrained,
    /// Auto-fail any check that requires sight; attacks against you have
    /// advantage; your attacks have disadvantage. Common in fights vs
    /// glitterdust / faerie fire and the basic Blindness spell.
    Blinded,
    /// Attacks against you have disadvantage; your attacks have advantage.
    /// First entry into the "good buff" condition family.
    Invisible,
    /// Speed = 0 but the action economy is intact. Used for grapples and
    /// spider webbing.
    Grappled,
    /// +1d4 to attack rolls and saving throws (Bless spell). Stacks with
    /// other bonuses. Marked here so the engine can attach the d4 in
    /// `roll_save` / `weapon_attack` and so removal cleans it up.
    Blessed,
}

impl Condition {
    pub fn name(&self) -> &'static str {
        match self {
            Condition::Prone => "prone",
            Condition::Stunned => "stunned",
            Condition::Poisoned => "poisoned",
            Condition::Frightened => "frightened",
            Condition::Restrained => "restrained",
            Condition::Blinded => "blinded",
            Condition::Invisible => "invisible",
            Condition::Grappled => "grappled",
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
