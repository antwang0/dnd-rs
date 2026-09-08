//! 5e's **start-of-turn emanations** — the stat-block traits that make
//! standing next to a creature cost you a saving throw.
//!
//! SRD 5.2 writes them all the same way: an ability, a DC, "any creature
//! that starts its turn in an *N*-foot Emanation originating from the
//! X", and a condition on a failure. The Ghast's and the Hezrou's
//! *Stench*, the Sea Hag's *Vile Appearance*, the Pit Fiend's *Fear
//! Aura*. What differs between them is the radius, the ability, the
//! condition, who is eligible to be caught at all, whether the source
//! has to be conscious, and whether making the save buys anything.
//!
//! ## Why this is not an aura, an action, or a zone
//!
//! The engine had three shapes for "something happens near a creature"
//! and none of them fits:
//!
//!   - **The paladin auras** (`aura_emitters`) are passive *modifiers*.
//!     Nothing is rolled and nothing is installed; a save inside Aura of
//!     Protection is simply a save with a bigger number. Aura of
//!     Conquest is the one that bites, and it bites without a roll.
//!   - **`Action`s** are chosen, aimed and paid for. Nobody chooses a
//!     stench. It fires on the *victim's* turn, out of the emitter's
//!     initiative slot entirely, and the emitter may be unable to act at
//!     all when it does.
//!   - **`Zone`s** (`engine::zones`) are placed and then persist
//!     independently of whoever placed them. A stench is not a place;
//!     it is a fact about a body, and it moves when the body moves and
//!     stops when the body dies.
//!
//! So an emanation is a fourth thing: a declared property of the
//! creature, read at the top of *somebody else's* turn, resolved through
//! the ordinary save and condition-install chokepoints. Declared on
//! `CreatureTemplate::emanations` for the same reason `attach` and
//! `charge` are declared there — the sites that ask about it never see
//! an action.
//!
//! ## The 24-hour clause
//!
//! Three of the four end with "Success: The target is immune to this
//! *X*'s *Trait* for 24 hours", and the Hezrou's does not. That one
//! sentence is the whole tactical difference between them: a party that
//! makes its saves walks away from a ghast's stench permanently and
//! keeps paying the hezrou's every round. It is modelled with a ledger
//! keyed by *(victim, emitter, trait name)*, because RAW scopes the
//! immunity to the individual creature — surviving one ghast's stench
//! says nothing about the ghast standing next to it.
//!
//! 24 hours is longer than any encounter, so the ledger lives on the
//! encounter and is never swept. That is exactly the RAW duration
//! rounded to the granularity the engine has.
//!
//! ## What is deliberately not modeled
//!
//!   - **Damage.** None of the four SRD start-of-turn emanations deals
//!     any; all of them install a condition and nothing else. A future
//!     one that does would add a field here rather than a second
//!     chassis, but adding the field before there is a creature for it
//!     would be inventing a rule.
//!   - **Emanations that catch allies.** Three of the four say "any
//!     creature", and a hezrou's stench is as foul to the dretch beside
//!     it as to the paladin. The engine scopes them all to hostiles,
//!     for the reason `AuraSide` exists at all: a monster whose
//!     signature trait poisons its own escort would be read as a bug by
//!     everyone who saw it, and no SRD encounter is built to exploit
//!     the difference. The pit fiend's is the fourth, and RAW scopes
//!     *that* one to enemies in so many words — which is the reading
//!     applied here to all of them, and worth naming as a divergence on
//!     the other three rather than leaving as an accident.

use crate::conditions::{Condition, ConditionTimer};
use crate::engine::types::{AbilityScoreType, CreatureType};

/// One stat-block emanation trait, in the shape SRD 5.2 prints it.
///
/// Every field is a clause of the printed sentence, in the order the
/// book writes them, so a template entry can be checked against the
/// stat block line by line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Emanation {
    /// The trait's name as printed — "Stench", "Vile Appearance". Used
    /// in the log line and as the third key of the immunity ledger, so
    /// a creature carrying two emanations tracks them separately.
    pub name: &'static str,
    /// RAW's "*N*-foot Emanation", in feet. Converted to a
    /// footprint-gap cap by `radius_tiles`; see there for the grid
    /// convention it shares with `PALADIN_AURA_RADIUS`.
    pub radius_feet: u32,
    /// The ability the caught creature rolls.
    pub save: AbilityScoreType,
    /// The DC it rolls against.
    pub dc: i32,
    /// What a failure installs.
    pub condition: Condition,
    /// How long the failure lasts. All three SRD traits say "until the
    /// start of its next turn", which is `UntilStartOfNextTurn` — and
    /// which is why the hook runs *after* `reset_for_new_round` has
    /// expired last round's copy.
    pub timer: ConditionTimer,
    /// RAW's "and can see the hag's true form" — `true` gates the trait
    /// on `viewer_can_see`, so a blinded victim, one round a corner, or
    /// one that simply cannot make the hag out in the dark walks
    /// through it untouched.
    ///
    /// `false` for both Stenches, which is the right reading of a smell.
    pub requires_sight: bool,
    /// RAW's "any Beast or Humanoid" — the creature types the trait can
    /// catch at all. Empty means "any creature", which is what both
    /// Stenches say.
    pub affects: &'static [CreatureType],
    /// RAW's closing "Success: The target is immune to this X's *trait*
    /// for 24 hours." `true` writes the victim into the encounter's
    /// immunity ledger on a successful save; `false` (the Hezrou) means
    /// the save has to be made again next round.
    pub grants_immunity_on_save: bool,
    /// RAW's "while it doesn't have the Incapacitated condition" — the
    /// clause the pit fiend's aura carries and the three body-odour
    /// traits do not.
    ///
    /// The distinction is worth a field rather than a blanket rule,
    /// because it is the difference between something a creature *does*
    /// and something it *is*. A pit fiend's dread is projected and stops
    /// when the fiend is stunned; a hezrou's reek is a fact about the
    /// hezrou, and a hezrou lying paralysed on the floor still stinks.
    /// The paladin auras answer this the same way and always have — see
    /// `aura_emitters`, which gates every one of them on exactly this.
    pub requires_conscious_source: bool,
    /// The clause of the log line between the victim's name and the
    /// outcome — "gags on the ghast's stench", "recoils from the sea
    /// hag's true face". Written per trait because a stench and a face
    /// are not the same sentence.
    pub flavor: &'static str,
}

impl Emanation {
    /// The trait's radius as a footprint-Chebyshev **gap** cap, which is
    /// the unit every distance predicate on the board is written in.
    ///
    /// Shares the convention `PALADIN_AURA_RADIUS` fixed — 10 ft is 4
    /// tiles on the 2.5 ft grid, via `tiles_from_feet` — rather than the
    /// tighter one `MELEE_REACH` uses, where 5 ft is a 1-tile gap.
    /// The two disagree by a tile because a *gap* of zero is already
    /// contact, so a literal reading of either conversion is off by the
    /// creature's own width in one direction or the other. Matching the
    /// aura is the right call for a trait that *is* an aura: a ghast's
    /// 5-foot stench and a paladin's 10-foot aura should be measured
    /// the same way, and the engine already reads the paladin's the
    /// generous way.
    pub const fn radius_tiles(&self) -> isize {
        crate::engine::util::tiles_from_feet(self.radius_feet) as isize
    }

    /// True if `victim_type` is one this trait can catch at all — RAW's
    /// "any Beast or Humanoid" clause, and its far more common absence.
    pub fn catches_type(&self, victim_type: CreatureType) -> bool {
        self.affects.is_empty() || self.affects.contains(&victim_type)
    }
}

/// **Stench** (Ghast, SRD 5.2): "Constitution Saving Throw: DC 10, any
/// creature that starts its turn in a 5-foot Emanation originating from
/// the ghast. Failure: The target has the Poisoned condition until the
/// start of its next turn. Success: The target is immune to this
/// ghast's Stench for 24 hours."
///
/// What makes closing with a ghast cost something before a claw lands:
/// Poisoned is disadvantage on every attack roll the victim makes that
/// round, so the price of walking into reach is a round of missing.
/// The success clause is the only way out, and it is why a party that
/// opens on a ghast at range never has to smell it at all.
pub static GHAST_STENCH: Emanation = Emanation {
    name: "Stench",
    radius_feet: 5,
    save: AbilityScoreType::Constitution,
    dc: 10,
    condition: Condition::Poisoned,
    timer: ConditionTimer::UntilStartOfNextTurn,
    requires_sight: false,
    affects: &[],
    grants_immunity_on_save: true,
    requires_conscious_source: false,
    flavor: "gags on the ghast's carrion stench",
};

/// **Stench** (Hezrou, SRD 5.2): "Constitution Saving Throw: DC 16, any
/// creature that starts its turn in a 10-foot Emanation originating
/// from the hezrou. Failure: The target has the Poisoned condition
/// until the start of its next turn."
///
/// The same trait as the ghast's with three numbers changed and one
/// sentence missing, and the missing sentence is the point: there is no
/// success clause, so a melee fighter standing on a hezrou rolls a DC 16
/// Constitution save at the top of *every* round for as long as the
/// fight lasts. Twice the radius and no way out is what a CR 8 demon's
/// version of a CR 2 undead's trait should look like.
pub static HEZROU_STENCH: Emanation = Emanation {
    name: "Stench",
    radius_feet: 10,
    save: AbilityScoreType::Constitution,
    dc: 16,
    condition: Condition::Poisoned,
    timer: ConditionTimer::UntilStartOfNextTurn,
    requires_sight: false,
    affects: &[],
    grants_immunity_on_save: false,
    requires_conscious_source: false,
    flavor: "reels from the hezrou's abyssal reek",
};

/// **Vile Appearance** (Sea Hag, SRD 5.2): "Wisdom Saving Throw: DC 11,
/// any Beast or Humanoid that starts its turn within 30 feet of the hag
/// and can see the hag's true form. Failure: The target has the
/// Frightened condition until the start of its next turn. Success: The
/// target is immune to this hag's Vile Appearance for 24 hours."
///
/// The trait the sea hag's whole kit is built around, and the reason
/// her Death Glare is worth an action: RAW's glare kills a *Frightened*
/// creature outright, and this is what frightens it. Three gates rather
/// than one — the type filter, the sight clause and the 24-hour out —
/// which between them make her a threat to a party of adventurers and
/// no threat at all to the golem they brought with them.
pub static SEA_HAG_VILE_APPEARANCE: Emanation = Emanation {
    name: "Vile Appearance",
    radius_feet: 30,
    save: AbilityScoreType::Wisdom,
    dc: 11,
    condition: Condition::Frightened,
    timer: ConditionTimer::UntilStartOfNextTurn,
    requires_sight: true,
    affects: &[CreatureType::Beast, CreatureType::Humanoid],
    grants_immunity_on_save: true,
    requires_conscious_source: false,
    flavor: "recoils from the sea hag's true face",
};

/// **Fear Aura** (Pit Fiend, SRD 5.2): "The pit fiend emanates an aura
/// in a 20-foot Emanation while it doesn't have the Incapacitated
/// condition. Wisdom Saving Throw: DC 21, any enemy that starts its
/// turn in the aura. Failure: The target has the Frightened condition
/// until the start of its next turn. Success: The target is immune to
/// this pit fiend's aura for 24 hours."
///
/// The one emanation in SRD 5.2 that RAW itself scopes to *enemies* —
/// which is the reading this module applies to all of them, and this is
/// the stat block that says so out loud.
///
/// It is also the one with the conscious-source clause, and the reason
/// that clause is a field: a pit fiend's dread is something it projects
/// and a hezrou's reek is something it smells of, so stunning the fiend
/// lifts the aura and stunning the demon does nothing.
///
/// This used to be an **Action** — the fiend spent its whole turn to
/// frighten the room for ten rounds, once. That got the tactical weight
/// backwards in both directions: RAW's aura costs the pit fiend nothing
/// (it keeps its bite and two claws every round, which is the CR 20
/// damage the fight is actually about), and it lasts one round at a
/// time, re-billed every turn anybody spends inside twenty feet. What
/// makes it a boss aura is not the duration but the tax: standing next
/// to the pit fiend costs a DC 21 Wisdom save, every round, until you
/// make one.
pub static PIT_FIEND_FEAR_AURA: Emanation = Emanation {
    name: "fear aura",
    radius_feet: 20,
    save: AbilityScoreType::Wisdom,
    dc: 21,
    condition: Condition::Frightened,
    timer: ConditionTimer::UntilStartOfNextTurn,
    // RAW puts no sight clause on it; the dread does not need to be
    // looked at.
    requires_sight: false,
    affects: &[],
    grants_immunity_on_save: true,
    requires_conscious_source: true,
    flavor: "quails in the pit fiend's presence",
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_radius_in_feet_becomes_a_gap_in_tiles() {
        // The paladin-aura convention: 10 ft is a 4-tile gap. The other
        // two radii on the roster ride the same conversion.
        assert_eq!(HEZROU_STENCH.radius_tiles(), 4);
        assert_eq!(GHAST_STENCH.radius_tiles(), 2);
        assert_eq!(SEA_HAG_VILE_APPEARANCE.radius_tiles(), 12);
    }

    #[test]
    fn an_empty_type_filter_catches_everything() {
        // Both Stenches say "any creature", so nothing on the roster is
        // out of scope for them by type.
        for ty in [
            CreatureType::Beast,
            CreatureType::Construct,
            CreatureType::Undead,
            CreatureType::Ooze,
        ] {
            assert!(GHAST_STENCH.catches_type(ty));
            assert!(HEZROU_STENCH.catches_type(ty));
        }
    }

    #[test]
    fn a_type_filter_is_the_whole_of_raws_beast_or_humanoid_clause() {
        assert!(SEA_HAG_VILE_APPEARANCE.catches_type(CreatureType::Humanoid));
        assert!(SEA_HAG_VILE_APPEARANCE.catches_type(CreatureType::Beast));
        // The golem the party brought along does not have a face to be
        // frightened with.
        assert!(!SEA_HAG_VILE_APPEARANCE.catches_type(CreatureType::Construct));
        assert!(!SEA_HAG_VILE_APPEARANCE.catches_type(CreatureType::Undead));
    }

    #[test]
    fn only_the_hezrou_makes_you_keep_rolling() {
        // The one clause that separates the CR 8 demon's stench from
        // the CR 2 undead's: no success line, so there is no way to
        // stop paying for it.
        assert!(GHAST_STENCH.grants_immunity_on_save);
        assert!(SEA_HAG_VILE_APPEARANCE.grants_immunity_on_save);
        assert!(!HEZROU_STENCH.grants_immunity_on_save);
    }
}
