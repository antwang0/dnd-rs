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
    /// 0 (in 5e via Incapacitated, but we collapse for simplicity). Attacks
    /// vs you have advantage; auto-fail STR/DEX saves.
    Stunned,
    /// Disadvantage on attack rolls and ability checks.
    Poisoned,
    /// Attack rolls against you have advantage; you have disadvantage on
    /// your own attack rolls. (5e: also auto-fail sight-based checks.)
    Blinded,
    /// Disadvantage on attacks; can't willingly move closer to the source
    /// of the fear (we conflate "the source" with "any enemy" today).
    Frightened,
    /// Speed 0; attacks vs you have advantage; you have disadvantage on
    /// your attacks; you have disadvantage on DEX saves.
    Restrained,
    /// Speed 0. No automatic adv/disadv riders on attacks (5e: only restrains
    /// movement). Cleared by stand-up-equivalent breakaway in future work.
    Grappled,
    /// Cannot take Actions, Bonus Actions, or Reactions. Movement still
    /// allowed (unlike Stunned). Used as a building-block for other
    /// conditions; rarely applied directly in 5e.
    Incapacitated,
    /// Incapacitated + speed 0; auto-fail STR/DEX saves; attacks vs you
    /// have advantage; melee hits crit. Strictly worse than Stunned.
    Paralyzed,
    /// Attacks against you have disadvantage; you have advantage on
    /// attacks. (Sight-based; the engine doesn't yet model true LOS
    /// concealment, so this is a marker.)
    Invisible,
    /// Defensive stance — until start of next turn, attacks against you
    /// have disadvantage and you have advantage on DEX saves. Granted by
    /// the Dodge action; cleared at the start of the actor's next turn.
    Dodging,
    /// Self-buff from a successful Hide check: attacks against you have
    /// disadvantage; you have advantage on the next attack roll. Cleared
    /// by attacking, taking damage, or otherwise being detected.
    Hidden,
    /// Marker buff from the Help action: the helped ally has advantage on
    /// their next attack roll against the marked target. We track the
    /// condition on the helper for simplicity (timer = until Help expires).
    Helped,
    /// Bless buff (concentration): +1d4 added to attack rolls and saving
    /// throws while concentration holds. Marker only — the +1d4 rider
    /// lives in `weapon_attack` / `roll_save` consumers.
    Blessed,
    /// Shield of Faith buff (concentration): +2 AC while concentration
    /// holds. Folded in `armor_class()`.
    ShieldOfFaith,
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
            Condition::Paralyzed => "paralyzed",
            Condition::Invisible => "invisible",
            Condition::Dodging => "dodging",
            Condition::Hidden => "hidden",
            Condition::Helped => "helped",
            Condition::Blessed => "blessed",
            Condition::ShieldOfFaith => "shielded",
        }
    }

    /// True if this condition completely blocks Action / BonusAction /
    /// Reaction usage (5e's Incapacitated clause). Stunned and Paralyzed
    /// inherit this clause.
    pub fn blocks_action_economy(&self) -> bool {
        matches!(
            self,
            Condition::Stunned | Condition::Incapacitated | Condition::Paralyzed
        )
    }

    /// True if this condition zeros out movement.
    pub fn zeros_movement(&self) -> bool {
        matches!(
            self,
            Condition::Prone
                | Condition::Stunned
                | Condition::Restrained
                | Condition::Grappled
                | Condition::Paralyzed
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
