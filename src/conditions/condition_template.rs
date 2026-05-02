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
    /// Disadvantage on attack rolls and ability checks. Marker only today
    /// (no advantage/disadvantage system yet).
    Poisoned,
    /// Disadvantage on attacks while you can see the source of fear; can't
    /// willingly move closer to that source. We approximate as a flat
    /// disadvantage on attacks (no source tracking yet).
    Frightened,
    /// Auto-fail any check requiring sight; attacks vs you have advantage;
    /// your attacks have disadvantage.
    Blinded,
    /// Attacks vs you have advantage; you have disadvantage on attacks
    /// and DEX saves; speed is 0.
    Restrained,
    /// Speed is 0; advantage to attack you; advantage on STR/DEX saves you
    /// make against effects that move you (we model as disadv on DEX saves
    /// for simplicity); attacks vs you have advantage.
    Grappled,
    /// Cannot take Actions, Bonus Actions, or Reactions. Inflicts the action
    /// economy block from Stunned without the auto-fail STR/DEX saves part.
    Incapacitated,
    /// Cannot take hostile actions against the charmer (we don't track who
    /// charmed whom, so it's a marker today). Future: tie to a source actor.
    Charmed,
    /// Currently unconscious (e.g. from sleep). Acts like Incapacitated +
    /// Prone + auto-fail STR/DEX saves. We model as: action economy blocked,
    /// movement 0, attacks vs you have advantage.
    Unconscious,
    /// Lit on fire; takes a tick of damage at end-of-round. Used by
    /// Burning Hands' lingering effect on fail (when modeled).
    Burning,
    /// Bonus to attack and saving throws (Bless). +d4 abstracted as +2 in
    /// the modifier path so we don't need to roll an extra die per attack.
    Blessed,
}

impl Condition {
    pub fn name(&self) -> &'static str {
        match self {
            Condition::Prone => "prone",
            Condition::Stunned => "stunned",
            Condition::Poisoned => "poisoned",
            Condition::Frightened => "frightened",
            Condition::Blinded => "blinded",
            Condition::Restrained => "restrained",
            Condition::Grappled => "grappled",
            Condition::Incapacitated => "incapacitated",
            Condition::Charmed => "charmed",
            Condition::Unconscious => "unconscious",
            Condition::Burning => "burning",
            Condition::Blessed => "blessed",
        }
    }

    /// True for conditions that block the full action economy (Action,
    /// Bonus Action, Reaction). Used by `can_consume_resource` to gate
    /// the action-economy resources without listing each variant inline.
    pub fn blocks_action_economy(&self) -> bool {
        matches!(
            self,
            Condition::Stunned | Condition::Incapacitated | Condition::Unconscious
        )
    }

    /// True for conditions that zero an actor's movement.
    pub fn zeros_movement(&self) -> bool {
        matches!(
            self,
            Condition::Prone
                | Condition::Stunned
                | Condition::Restrained
                | Condition::Grappled
                | Condition::Incapacitated
                | Condition::Unconscious
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
