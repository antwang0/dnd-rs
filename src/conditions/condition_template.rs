/// Persistent status effects an actor can have. Mechanical impact lives
/// at the consumer (e.g. `ActorInstance::remaining_movement` returns 0
/// while `Prone`; `can_consume_resource` blocks Action/BonusAction/Reaction
/// while `Stunned`). New variants land here and then plug into the
/// relevant accessor — no central dispatcher.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Condition {
    /// Speed = 0; ranged attacks against you have disadvantage; melee
    /// against you have advantage; you have disadvantage on attacks.
    Prone,
    /// Cannot take Actions, Bonus Actions, or Reactions. Movement is also
    /// 0 (in 5e via Incapacitated, but we collapse for simplicity).
    Stunned,
    /// Disadvantage on attack rolls and saving throws. Used by acid spit
    /// rider effects and the Poisoned condition timer.
    Poisoned,
    /// Disadvantage on attacks; attacks against you have advantage.
    /// You can't see, so anything that requires sight auto-fails. Models
    /// the Blinded condition's combat clauses.
    Blinded,
    /// Speed = 0; disadvantage on attacks and DEX saves; attacks against
    /// you have advantage. Like Prone but doesn't grant melee-attack
    /// advantage from prone (different rule).
    Restrained,
    /// Disadvantage on ability checks and attack rolls while the source
    /// of fear is in line of sight. We approximate by always applying
    /// disadvantage on attacks (5e: even when not seeing the source,
    /// most fear effects last only short bursts).
    Frightened,
    /// Attacks against you have disadvantage; you have advantage on
    /// attacks. Models the Invisible condition's combat clauses.
    Invisible,
    /// Attacks against you have advantage; you have disadvantage on
    /// attacks (5e treats defending against grappler differently;
    /// we collapse to "stuck and exposed").
    Grappled,
    /// All attacks against you have advantage; you can't take actions
    /// or reactions. Lighter than Stunned (movement may still apply if
    /// an effect chains it). Charmed/Frightened auto-imply this in 5e
    /// for the action denial; we keep it as a separate marker.
    Incapacitated,
    /// You can't attack the source of the charm. Modeled here as a
    /// gentle marker; AI should not target the charmer. Currently a
    /// flag only — no charm relationship tracking yet.
    Charmed,
    /// Resistant to all damage (half). Stacks ahead of damage-type
    /// resistance from creature templates. Used by Stoneskin / Rage.
    DamageResistant,
    /// Granted by the Help action: holder's next attack roll has
    /// advantage. The attack-mode helper consumes this marker on use
    /// so the buff is one-shot, not persistent.
    Helped,
    /// Granted by the Dodge action: attacks against the holder have
    /// disadvantage and they have advantage on DEX saves. Cleared at
    /// the start of the holder's next turn (timer = Rounds(1)).
    Dodging,
    /// Granted by the Disengage action: the holder's movement does
    /// not provoke opportunity attacks for the rest of this turn.
    /// Cleared at the start of the holder's next turn.
    Disengaging,
    /// Bless buff: +1d4 on attack rolls and saving throws (we model as
    /// a flat-bonus proxy via the save / attack pipeline). Concentration
    /// from the caster maintains it.
    Blessed,
    /// Shield reaction: +5 to AC until the start of the holder's next
    /// turn (modeled by adding to armor_class while the marker is set
    /// and clearing on round wrap via the timer).
    Shielded,
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
            Condition::Grappled => "grappled",
            Condition::Incapacitated => "incapacitated",
            Condition::Charmed => "charmed",
            Condition::DamageResistant => "damage resistant",
            Condition::Helped => "helped",
            Condition::Dodging => "dodging",
            Condition::Disengaging => "disengaging",
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
