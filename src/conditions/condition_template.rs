/// Persistent status effects an actor can have. Mechanical impact lives
/// at the consumer (e.g. `ActorInstance::remaining_movement` returns 0
/// while `Prone`; `can_consume_resource` blocks Action/BonusAction/Reaction
/// while `Stunned`). New variants land here and then plug into the
/// relevant accessor — no central dispatcher.
///
/// Many conditions imply or interact with each other (Stunned implies
/// Incapacitated, etc.); we model them flat for now and let consumers
/// branch on the specific variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Condition {
    /// Speed = 0; ranged attacks against you have disadvantage; melee
    /// against you have advantage; you have disadvantage on attacks.
    Prone,
    /// Cannot take Actions, Bonus Actions, or Reactions; movement = 0.
    /// Stronger than Incapacitated — also implies the action-economy lock.
    Stunned,
    /// Disadvantage on attack rolls and ability checks (and saves derived
    /// from ability checks).
    Poisoned,
    /// Cannot see; auto-fails sight-dependent checks. Attack rolls against
    /// you have advantage; your attack rolls have disadvantage.
    Blinded,
    /// Disadvantage on attack rolls and ability checks. (We don't model
    /// the "source-of-fear in LOS" wrinkle yet — disadvantage applies
    /// flatly while the condition is present.)
    Frightened,
    /// Speed = 0; attack rolls against you have advantage; your attacks
    /// have disadvantage; disadvantage on DEX saves.
    Restrained,
    /// Attack rolls against you have disadvantage; your attack rolls have
    /// advantage. Doesn't yet block being targeted (no "must see" gating).
    Invisible,
    /// Cannot take Actions, Bonus Actions, or Reactions. Doesn't zero
    /// movement (that's Stunned's stronger lockdown).
    Incapacitated,
    /// Speed = 0. Ends when the grappler is incapacitated or moved out of
    /// reach (we don't track the grappler ref yet — manual removal).
    Grappled,
    /// Concentration / vigilance bonus: until your next turn, attacks
    /// against you have disadvantage and you have advantage on DEX saves.
    /// Granted by the Dodge action.
    Dodging,
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
            Condition::Invisible => "invisible",
            Condition::Incapacitated => "incapacitated",
            Condition::Grappled => "grappled",
            Condition::Dodging => "dodging",
        }
    }

    /// True when this condition prevents the actor from spending Action /
    /// Bonus Action / Reaction slots. Aggregated here so the action-economy
    /// gate has one source of truth instead of scattered match arms.
    pub fn blocks_action_economy(&self) -> bool {
        matches!(
            self,
            Condition::Stunned | Condition::Incapacitated
        )
    }

    /// True when this condition forces the actor's speed to 0.
    pub fn zeros_movement(&self) -> bool {
        matches!(
            self,
            Condition::Prone
                | Condition::Stunned
                | Condition::Restrained
                | Condition::Grappled
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
    /// Lasts until the start of the holder's next turn — used for Dodge.
    /// The engine clears these at the start of an actor's turn.
    UntilStartOfNextTurn,
}
