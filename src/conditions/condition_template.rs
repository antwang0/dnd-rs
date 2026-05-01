/// Persistent status effects an actor can have. Mechanical impact lives
/// at the consumer (e.g. `ActorInstance::remaining_movement` returns 0
/// while `Prone`; `can_consume_resource` blocks Action/BonusAction/Reaction
/// while `Stunned`). New variants land here and then plug into the
/// relevant accessor — no central dispatcher.
///
/// Durations: `ConditionTimer::Rounds(n)` ticks down at every wrap of the
/// initiative queue and clears the condition at 0; `Permanent` requires an
/// explicit removal (Stand-up clears Prone, Lesser Restoration clears
/// Poisoned, etc).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Condition {
    /// Speed = 0; ranged attacks against you have disadvantage; melee
    /// against you have advantage; you have disadvantage on attacks.
    Prone,
    /// Cannot take Actions, Bonus Actions, or Reactions. Movement is also
    /// 0 (in 5e via Incapacitated, but we collapse for simplicity).
    Stunned,
    /// Disadvantage on attack rolls and ability checks (and DEX-derived
    /// saves, which we conflate with checks until a save/check distinction
    /// matters).
    Poisoned,
    /// You can't see. Attacks against you have advantage; your attacks
    /// have disadvantage. Auto-fail any check requiring sight.
    Blinded,
    /// Speed = 0; attacks against you have advantage; your attacks have
    /// disadvantage; disadvantage on DEX saves.
    Restrained,
    /// Disadvantage on attacks while the source of fear is in line of
    /// sight. Today we approximate by blanket disadvantage while
    /// Frightened — the source is not tracked.
    Frightened,
    /// Attacks against you have disadvantage; your attacks have advantage.
    /// Doesn't actually hide you on the map (rendering is unchanged) — the
    /// mechanical advantage/disadvantage clause is what matters in combat.
    Invisible,
    /// Cannot take Actions, Bonus Actions, or Reactions (but movement is
    /// allowed). 5e Incapacitated. Distinct from Stunned (no movement).
    Incapacitated,
    /// Speed = 0 and the grappled creature can be moved by the grappler
    /// (movement clause not yet wired). Today only the speed-0 effect
    /// applies; ending the grapple is not modeled.
    Grappled,
    /// Hit points doubled while raging plus advantage on STR checks/saves —
    /// kept simple: half damage from physical (Bludgeoning/Piercing/Slashing)
    /// is wired via the resistance pipeline, and STR-attack damage gets +2
    /// while raging via `damage_bonus`. Lasts 10 rounds.
    Raging,
    /// Granted by the Dodge action. Attacks against you are at disadvantage,
    /// and you make DEX saves with advantage. Lasts until your next turn —
    /// the engine clears it on `reset_for_new_round`.
    Dodging,
    /// Granted by the Help action. The next attack against the helped
    /// target (within 1 round) gains advantage. Single-shot consumable —
    /// the next attack against you uses & clears it. Today modeled as
    /// blanket advantage for the next attack against the helped actor.
    Helped,
    /// Granted by Bless. Adds 1d4 to attack rolls and saving throws.
    /// Concentration spell on the caster; lasts 10 rounds.
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
            Condition::Frightened => "frightened",
            Condition::Invisible => "invisible",
            Condition::Incapacitated => "incapacitated",
            Condition::Grappled => "grappled",
            Condition::Raging => "raging",
            Condition::Dodging => "dodging",
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
    /// Until the holder's next turn starts. Cleared by
    /// `reset_for_new_round`. Used for one-round buffs like Dodge and
    /// Help that don't fit the round-end ticking model (which runs at
    /// initiative-wrap, not the holder's specific turn).
    UntilOwnTurn,
}
