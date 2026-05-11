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
    /// Disadvantage on attack rolls and ability checks while you can see
    /// the source of fear. We don't track LOS-to-fear-source today; the
    /// effect is unconditional disadvantage on attacks.
    Frightened,
    /// Speed = 0; disadvantage on attacks; attacks against you have
    /// advantage; disadvantage on DEX saves.
    Restrained,
    /// Auto-fail any check requiring sight; attack rolls against you have
    /// advantage; you have disadvantage on attacks.
    Blinded,
    /// Cannot take actions or reactions (movement is still allowed).
    /// Distinct from Stunned — Incapacitated still moves; Stunned can't.
    Incapacitated,
    /// You have no effect on combat behavior — you can't take hostile
    /// actions against the charmer. Marker only today; we don't yet
    /// model the "can't attack the charmer" enforcement.
    Charmed,
    /// Cannot move; cannot take actions or reactions; auto-fail STR/DEX
    /// saves; attacks against you have advantage; melee crits are
    /// auto-criticals on hit. We model the action-economy / movement
    /// blocks here; the auto-crit on melee hit lives at the attack site.
    Paralyzed,
    /// Cannot be seen without special vision. Attack rolls against you
    /// have disadvantage; your attacks have advantage. Bookkept as a
    /// condition for clean removal on attack (per 5e Greater Invisibility
    /// vs Invisibility — we treat both as the simple Invisible condition).
    Invisible,
    /// Active until the start of your next turn after taking the Dodge
    /// action: attacks against you have disadvantage (if you can see the
    /// attacker); you have advantage on DEX saves. Cleared by the
    /// `UntilStartOfNextTurn` timer on your next turn.
    Dodging,
    /// Speed = 0; you can't gain a speed bonus. Ends when grappler is
    /// incapacitated or target is moved out of range. We track only the
    /// movement block here; ending is up to the grappler logic.
    Grappled,
    /// +1d4 to attack rolls and saving throws (Bless spell). Tracked as a
    /// condition so it ticks down with the spell timer and clears cleanly
    /// when concentration drops.
    Blessed,
    /// +2 AC from Shield of Faith (concentration buff). Read by
    /// `armor_class()`; drops when concentration drops.
    ShieldOfFaith,
    /// +5 AC from the Shield reaction spell, until the start of your
    /// next turn. Cleared by the `UntilStartOfNextTurn` timer.
    Shielded,
    /// Generic "halve incoming damage" buff (e.g. Stoneskin).
    /// Stacks multiplicatively with damage-type resistance.
    DamageResistant,
    /// Hostile-to-hostile attack against the holder is at advantage and
    /// the holder cannot benefit from being Hidden/Invisible. Used by
    /// Faerie Fire / Hunter's Mark style effects.
    Outlined,
    /// Lit by Guiding Bolt — next attack against this actor before the
    /// end of the caster's next turn has advantage. Burns off when the
    /// next attack lands or when its short timer expires.
    GuidingBoltLit,
    /// Took the Help action against this turn — your next attack against
    /// the helped target has advantage (cleared on use or end of round).
    Helped,
    /// Successful Stealth check; you have unseen advantage on attack and
    /// attackers have disadvantage. Distinct from Invisible: it's broken
    /// by attacking, ending hidden status.
    Hidden,
    /// On fire — takes 1d4 fire at the start of each of its turns until
    /// extinguished. Burning is a DOT condition with `Rounds(n)` timer.
    Burning,
    /// Took the Disengage action this turn: their movement doesn't
    /// provoke opportunity attacks. Cleared by `UntilStartOfNextTurn`.
    /// Alias kept for legacy call sites; `Disengaging` is preferred.
    Disengaging,
    /// Knocked unconscious (HP 0 or magical sleep). Stronger than
    /// Incapacitated: drops prone, fails STR/DEX saves, and melee crits
    /// on hit. Set automatically when an actor enters HpState::Dying.
    Unconscious,
    /// -1d4 (modeled as -2) to attack rolls and saving throws (Bane spell).
    /// Symmetric counterpart to `Blessed`. Tracked as a condition so it
    /// ticks down with the spell timer and clears on concentration drop.
    Baned,
    /// Cannot hear; auto-fail any check requiring hearing. We don't yet
    /// model verbal-component spell failure or audio-based perception, so
    /// the in-combat impact is mostly cosmetic — but the flag is here for
    /// spells like Blindness/Deafness so they can apply something.
    Deafened,
    /// Marked by Hunter's Mark — the marker (concentrating caster) deals
    /// an extra 1d6 weapon damage to this target. Tracked as a condition
    /// so dropping concentration cleans it up automatically.
    HuntersMarked,
    /// Mage Armor active — base AC becomes 13 + DEX modifier (we model as
    /// a flat AC boost via `condition_ac_bonus`). Lasts 8 hours; we just
    /// give it a long Rounds timer.
    MageArmored,
    /// Mocked by Vicious Mockery — disadvantage on the next attack roll
    /// before the end of the target's next turn. Short timer
    /// (`UntilStartOfNextTurn`) clears the debuff after the holder takes
    /// their turn (the disadvantage applies to attacks they make while
    /// the condition is up).
    Mocked,
    /// Heroism active — immune to Frightened and gaining temp HP each
    /// round from the spell's caster. Tracked as a condition so it
    /// clears cleanly on concentration drop.
    Heroic,
    /// Cannot cast spells / take reactions until the start of their next
    /// turn (Shocking Grasp's rider on a hit against a creature wearing
    /// metal armor — we model the simpler "no reaction" clause). Clears
    /// at start of own next turn.
    NoReaction,
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
            Condition::Incapacitated => "incapacitated",
            Condition::Charmed => "charmed",
            Condition::Paralyzed => "paralyzed",
            Condition::Invisible => "invisible",
            Condition::Dodging => "dodging",
            Condition::Grappled => "grappled",
            Condition::Blessed => "blessed",
            Condition::ShieldOfFaith => "shield of faith",
            Condition::Shielded => "shielded",
            Condition::DamageResistant => "damage resistant",
            Condition::Outlined => "outlined",
            Condition::GuidingBoltLit => "marked by guiding bolt",
            Condition::Helped => "helped",
            Condition::Hidden => "hidden",
            Condition::Burning => "burning",
            Condition::Disengaging => "disengaging",
            Condition::Unconscious => "unconscious",
            Condition::Baned => "baned",
            Condition::Deafened => "deafened",
            Condition::HuntersMarked => "marked by hunter's mark",
            Condition::MageArmored => "mage armored",
            Condition::Mocked => "mocked",
            Condition::Heroic => "heroic",
            Condition::NoReaction => "shocked",
        }
    }

    /// True if this condition completely blocks Action / BonusAction /
    /// Reaction usage (5e's Incapacitated clause). Stunned and Paralyzed
    /// inherit this clause.
    pub fn blocks_action_economy(&self) -> bool {
        matches!(
            self,
            Condition::Stunned
                | Condition::Incapacitated
                | Condition::Paralyzed
                | Condition::Unconscious
        )
    }

    /// True if this condition zeros out movement.
    pub fn zeros_movement(&self) -> bool {
        // Note: Prone is NOT in this list. RAW: prone halves movement
        // (you crawl). We don't yet model the half-speed reduction;
        // movement just costs the same. But blocking it entirely creates
        // a catch-22 — standing up pays in movement.
        matches!(
            self,
            Condition::Stunned
                | Condition::Restrained
                | Condition::Grappled
                | Condition::Paralyzed
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
    /// Lasts until the start of the holder's next turn — used for Dodge.
    /// The engine clears these at the start of an actor's turn.
    UntilStartOfNextTurn,
}

impl ConditionTimer {
    /// Legacy alias — pre-existing code spells this `UntilOwnTurn`.
    /// Same value as `UntilStartOfNextTurn`; kept const so it can be
    /// dropped into match arms where renaming hasn't yet propagated.
    #[allow(non_upper_case_globals)]
    pub const UntilOwnTurn: ConditionTimer = ConditionTimer::UntilStartOfNextTurn;
}
