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
    /// Disadvantage on attack rolls and on saves derived from ability
    /// checks. Today we apply the disadv to all saves uniformly.
    Poisoned,
    /// Cannot take Actions, Bonus Actions, or Reactions. Looser than
    /// Stunned: still has movement (unless paired with another condition
    /// that zeroes it).
    Incapacitated,
    /// Speed = 0. Attacks against you have advantage; you have
    /// disadvantage on attacks; disadvantage on DEX saves.
    Restrained,
    /// Speed = 0. Doesn't impose attack adv/disadv on its own — just
    /// pinned in place. Removed when the grappler dies / drops it; we
    /// don't model the source today, so it's cleared by explicit means.
    Grappled,
    /// Can't see. Auto-fail any check requiring sight; disadvantage on
    /// attacks; attacks against you have advantage.
    Blinded,
    /// Disadvantage on attack rolls and ability checks. We don't model
    /// the "while source is in sight" clause yet; treat as flat disadv.
    Frightened,
    /// Cannot perform attacks or harmful effects against the charmer.
    /// Marker only — we don't yet track the charmer relationship, so
    /// the AI uses this as a soft signal.
    Charmed,
    /// Attacks have advantage; attacks against you have disadvantage.
    /// (We don't yet model who can perceive whom — assume universal.)
    Invisible,
    /// Incapacitated + immobile. Auto-fail STR/DEX saves; attacks against
    /// you have advantage and crit on hit when the attacker is within 5ft
    /// (melee reach). Modeled as the strict superset of Stunned in our
    /// engine: action economy locked, no movement, plus per-attack rules.
    Paralyzed,
    /// You took the Dodge action: attacks against you have disadvantage
    /// and you make DEX saves with advantage. Lasts until the start of
    /// your next turn (Rounds(1) ticks down at the round wrap).
    Dodging,
    /// You took the Disengage action: your movement doesn't provoke
    /// opportunity attacks for the rest of the turn. Rounds(1) timer.
    Disengaging,
}

impl Condition {
    pub fn name(&self) -> &'static str {
        match self {
            Condition::Prone => "prone",
            Condition::Stunned => "stunned",
            Condition::Poisoned => "poisoned",
            Condition::Incapacitated => "incapacitated",
            Condition::Restrained => "restrained",
            Condition::Grappled => "grappled",
            Condition::Blinded => "blinded",
            Condition::Frightened => "frightened",
            Condition::Charmed => "charmed",
            Condition::Invisible => "invisible",
            Condition::Paralyzed => "paralyzed",
            Condition::Dodging => "dodging",
            Condition::Disengaging => "disengaging",
        }
    }

    /// Conditions that zero an actor's movement budget. Centralized so
    /// `remaining_movement` and any future "could you walk to X" check
    /// agree on which conditions root the actor.
    pub fn immobilizes(&self) -> bool {
        matches!(
            self,
            Condition::Prone
                | Condition::Stunned
                | Condition::Restrained
                | Condition::Grappled
                | Condition::Paralyzed
        )
    }

    /// Conditions that lock the action / bonus-action / reaction economy.
    /// Same idea as `immobilizes` — one place to look up the rule.
    pub fn locks_action_economy(&self) -> bool {
        matches!(
            self,
            Condition::Stunned | Condition::Incapacitated | Condition::Paralyzed
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
