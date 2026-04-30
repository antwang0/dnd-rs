/// Persistent status effects an actor can have. Mechanical impact lives
/// at the consumer (e.g. `ActorInstance::remaining_movement` returns 0
/// while `Prone`; `can_consume_resource` blocks Action/BonusAction/Reaction
/// while `Stunned`). New variants land here and then plug into the
/// relevant accessor — no central dispatcher.
///
/// Round-tracked durations (e.g. "stunned for 1 round") tick down on every
/// initiative wrap and clear at 0; permanent timers persist until removed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Condition {
    /// Speed = 0; ranged attacks against you have disadvantage; melee
    /// against you have advantage; you have disadvantage on attacks.
    Prone,
    /// Cannot take Actions, Bonus Actions, or Reactions. Movement is also
    /// 0 (in 5e via Incapacitated, but we collapse for simplicity).
    Stunned,
    /// Disadvantage on attack rolls and saves derived from ability checks.
    Poisoned,
    /// Attacker is blinded — disadvantage on attacks; attacks against you
    /// have advantage.
    Blinded,
    /// Disadvantage on attacks vs creatures NOT the source. We don't yet
    /// model the "who scared you" pointer, so today disadvantage applies
    /// to all attacks while frightened (close enough for the simulation).
    Frightened,
    /// Speed = 0; attacks against you have advantage; you have disadvantage
    /// on attacks; disadvantage on DEX saves.
    Restrained,
    /// Speed = 0; melee attacks against you have advantage; ranged attacks
    /// against you still have disadvantage. Distinguished from Restrained
    /// in that it doesn't impose disadvantage on the grappled actor's own
    /// attacks. (Simplification: reuses the no-movement clause.)
    Grappled,
    /// Cannot take Actions, Bonus Actions, or Reactions. Doesn't zero
    /// movement on its own — Stunned does both. Used for sleep / charm /
    /// concentration-loss riders that disable action economy without
    /// freezing the actor in place.
    Incapacitated,
    /// Attacker can't see you — attacks against you have disadvantage and
    /// your attacks have advantage. Doesn't model the "see invisible"
    /// counter; if both are invisible the modifiers cancel naturally.
    Invisible,
    /// Until your next turn: attackers have disadvantage on attacks
    /// against you; you make DEX saves with advantage. From the Dodge
    /// action. Cleared by `tick_condition_timers` (Rounds(1)).
    Dodging,
    /// One-shot grant from the Help action. The next attack roll the
    /// actor makes is at advantage; firing the attack consumes the marker
    /// (the engine clears it after the attack lands).
    Helped,
    /// From Bless: +1d4 to attack rolls and saving throws. Concentration
    /// effect; cleared if the caster's concentration drops. We model the
    /// dice bonus by a flat +2 (average of d4) wired into attack/save
    /// modifiers — close enough without polluting the roller seam.
    Blessed,
    /// You can't move and combat-actively keep speed at 0. Distinct from
    /// Stunned: paralyzed actors can't act either, and any melee attack
    /// against them auto-crits (we don't yet model auto-crit; treat as
    /// advantage like Stunned for now).
    Paralyzed,
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
            Condition::Invisible => "invisible",
            Condition::Dodging => "dodging",
            Condition::Helped => "helped",
            Condition::Blessed => "blessed",
            Condition::Paralyzed => "paralyzed",
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
