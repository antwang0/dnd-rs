//! 5e's critical hits, for the two parties the d20 face does not
//! belong to: the defender who takes one back, and the effect that
//! rewrites what one is.
//!
//! Most rules in the engine that *make* a crit — the natural 20, the
//! widened threshold a Champion fights on, the auto-crit a Paralyzed
//! target hands a melee attacker — belong to the attacker or to the
//! pair, and are resolved by the two attack chokepoints out of
//! `ActorInstance::crit_threshold` and
//! `EncounterInstance::crit_threshold_against`. This module holds the
//! two that are neither.
//!
//! # The defender's half
//!
//! One sentence: *"any Critical Hit against you becomes a normal
//! hit."*
//!
//! That sentence is not a re-roll, a resistance, or a miss. It is a
//! demotion, and the distinction is what earns it a module rather than
//! a branch:
//!
//!   - **The swing still hits.** Adamantine Armor does not turn a
//!     critical into a miss; the blow lands for its ordinary damage.
//!     So the demotion has to happen after the hit/miss verdict and
//!     before the damage roll, which is one specific point in a
//!     pipeline that has half a dozen plausible-looking ones.
//!
//!   - **It is about the target, not the attacker.** Nothing the
//!     swinger carries can answer it, which is why this is not a row on
//!     `MAGICAL_ATTACK_SOURCES` or on any of the attacker-scoped
//!     cohorts in `engine::attack`.
//!
//!   - **It reaches every crit, not just the natural 20.** The armour's
//!     RAW says "any Critical Hit", so a Paralyzed wearer whose
//!     attacker was promised an auto-crit gets the demotion too, and so
//!     does an assassin's opening shot. Reading the cohort at the one
//!     place `is_crit` is finally known — rather than at the nat-20
//!     test — is what makes that true for free.
//!
//! Both chokepoints ask (`engine::attack::resolve_attack_outcome` for
//! weapon swings, `actions::spells::spell_attack_outcome` for spell
//! attacks) because RAW's clause says *any* critical hit and a Fire
//! Bolt crit is one.
//!
//! ## Why a cohort for one row
//!
//! Adamantine Armor is the only source on today's roster, and the table
//! is still a table for the reason every other cohort in this engine is
//! one: the next source — a monster trait, the Adamantine Weapon's
//! defensive sibling, a subclass's "critical hits against you are
//! normal hits" capstone — should land as one row and a log label,
//! rather than as a second `||` on a predicate whose name has stopped
//! describing it.
//!
//! # The effect's half
//!
//! [`ACTION_CRIT_PROFILES`] is the other cohort, and it answers a
//! question the engine could not ask at all before it existed: *what
//! counts as a critical hit for **this particular attack**, whoever is
//! making it?*
//!
//! Both halves of 5e's crit rule are otherwise properties of the
//! creature swinging. The face is `crit_threshold` on the stat block
//! (a Champion's 19, Superior Critical's 18) optionally narrowed
//! against one victim (a Hexblade's Curse); the payout is the shared
//! *"roll the damage dice twice"* sentence, plus the attacker-scoped
//! extra dice on `CRIT_EXTRA_DICE_SOURCES` (Brutal Critical, Savage
//! Attacks, Piercer). Nothing in either lane can be said by a spell,
//! because a spell is not a creature and the tables are keyed by one.
//!
//! Blade of Disaster is the effect that needs both said: *"the blade
//! scores a critical hit on a roll of 18–20, and it deals an extra 4d12
//! Force damage on a critical hit"* — an 18 threshold the caster does
//! not own, and triple dice rather than double. Its docstring in
//! `actions::spells` used to concede the whole clause as unmodelled, on
//! the grounds that *"both would have to become per-action to express
//! this"*. This table is that, and it costs one row.
//!
//! Read at the same two chokepoints as the defender's half, from the
//! action name each of them already carries — the same string
//! `swing_comes_from_a_blade` and `UnderwaterVerdict::for_attack` key
//! off, and for the same reason: an attack's identity reaches the die
//! as its name and not as a `&dyn Action`.

use crate::actors::actor_template::ActorInstance;
use crate::engine::encounter::EncounterInstance;

/// One way a critical hit against a creature is demoted to an ordinary
/// hit.
///
/// Shape sibling to `engine::magic::MagicalAttackSource` — a predicate
/// plus the name the log prints when that predicate is what answered.
/// The label matters as much as the verdict here: a player who watches
/// a natural 20 print `hit` instead of `CRIT!` will read it as a bug
/// unless the line beside it says what ate the crit.
struct CriticalNegationSource {
    /// Named in the log when this row is what demoted the swing.
    /// Lowercase, the RAW feature or item name.
    label: &'static str,
    applies: fn(&ActorInstance) -> bool,
}

/// Every way a creature on this board turns a critical hit against it
/// into a normal one.
const CRITICAL_NEGATION_SOURCES: &[CriticalNegationSource] = &[
    // 5e **Adamantine Armor** (Armor, any medium or heavy except hide;
    // Uncommon): "While you're wearing it, any Critical Hit against you
    // becomes a normal hit."
    //
    // Read off the inventory rather than installed as a condition, for
    // the reason every other `grants_*` flag on `Item` is: the armour
    // grants this by being *worn*. A Dispel Magic has nothing to strip
    // and a long rest has nothing to reinstall, and a condition would
    // have been strippable by both.
    CriticalNegationSource {
        label: "adamantine armor",
        applies: ActorInstance::blunts_critical_hits,
    },
];

/// Whether a critical hit against `target_id` is demoted to an ordinary
/// hit, and what demoted it.
///
/// Returns the label of the first matching row, or `None` — which is
/// the overwhelmingly common answer and the one an unknown target gets
/// too. Failing closed here means "the crit stands", which is the safe
/// direction: a lookup that missed leaves the swing exactly as the
/// engine resolved it rather than silently blunting a blow.
///
/// Callers ask only when they already hold a crit; the function does
/// not take `is_crit` itself, because a cohort about what happens to
/// critical hits has nothing to say about a swing that isn't one.
pub fn critical_negation_source(
    encounter: &EncounterInstance,
    target_id: usize,
) -> Option<&'static str> {
    let target = encounter.actors.get(&target_id)?;
    CRITICAL_NEGATION_SOURCES
        .iter()
        .find(|row| (row.applies)(target))
        .map(|row| row.label)
}

/// Demote `is_crit` if the target's armour says so, logging the row that
/// answered.
///
/// The whole of this module's contract at a call site, so the two attack
/// chokepoints share one spelling of it rather than each growing their
/// own three-line `if let`. Returns the crit flag the rest of the
/// pipeline should use.
pub fn apply_critical_negation(
    encounter: &mut EncounterInstance,
    target_id: usize,
    is_crit: bool,
) -> bool {
    if !is_crit {
        return false;
    }
    let Some(label) = critical_negation_source(encounter, target_id) else {
        return true;
    };
    encounter.log(format!("  {label}: the critical hit becomes a normal hit"));
    false
}

/// RAW's own critical hit, for everything that does not print a clause
/// of its own: a natural 20 promotes, and the damage dice are rolled
/// twice.
///
/// Spelled out as a named value rather than left as two literals at the
/// lookup, because the *default* is the part of this rule every reader
/// already knows and the table exists to say where it stops applying.
pub const STANDARD_CRIT: ActionCritProfile = ActionCritProfile {
    action: "",
    threshold: None,
    dice_multiple: 2,
};

/// One attack's own critical-hit clause — the face it crits on and how
/// many times its damage dice are rolled when it does.
///
/// Keyed by the action's name, which is what the two attack
/// chokepoints have in hand; see the module docs for why that is the
/// key and not a `&dyn Action`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActionCritProfile {
    /// The action's `name()`, matched exactly. Lowercase, as every
    /// action name in the engine is.
    pub action: &'static str,
    /// The lowest d20 face that crits **for this attack**, or `None` to
    /// leave the question entirely to the attacker.
    ///
    /// Combined with the attacker's own threshold by taking the lower
    /// face, which is how every other pair of crit-range sources in the
    /// engine composes — see `crit_threshold_against`, where Improved
    /// Critical and Hexblade's Curse meet the same way. Nothing in 5e
    /// adds crit ranges together, so a Champion swinging a Blade of
    /// Disaster still crits on 18 rather than on 16.
    pub threshold: Option<u32>,
    /// How many times this attack's damage dice are rolled on a
    /// critical hit. RAW's shared rule is `2`; Blade of Disaster's is
    /// `3`.
    ///
    /// Expressed as a multiple of the pool rather than as "extra dice",
    /// because that is the sentence RAW writes for the general case
    /// (*"roll the damage dice twice"*) and the one the engine already
    /// implements — a second field for extra dice exists one table
    /// over, on `CRIT_EXTRA_DICE_SOURCES`, and is where an
    /// *attacker's* extra dice belong. Never 0: a profile that zeroed
    /// the pool would make a critical hit deal less than an ordinary
    /// one.
    pub dice_multiple: u32,
}

/// Every attack in the engine whose critical hit is not RAW's default
/// one.
///
/// A short table by design and likely to stay short — 5e writes this
/// clause rarely, and every entry is a spell or item that says both
/// halves of it in one sentence.
const ACTION_CRIT_PROFILES: &[ActionCritProfile] = &[
    // Blade of Disaster (TCE, level-9 conjuration): *"The blade scores
    // a critical hit on a roll of 18–20, and it deals an extra 4d12
    // Force damage on a critical hit."*
    //
    // RAW's second half is worded as extra dice on top of the doubled
    // ones — 4d12 base, 4d12 for the crit, 4d12 more from the clause —
    // which is the same pool rolled three times, so it is spelled here
    // as the multiple it comes to rather than as a fourth field
    // nothing else would use.
    //
    // The name is the spell's `Action::name()` and must stay in step
    // with it; `the_blade_of_disaster_profile_names_a_live_action`
    // pins that.
    ActionCritProfile {
        action: "blade of disaster",
        threshold: Some(18),
        dice_multiple: 3,
    },
];

/// The critical-hit clause `action_name` carries, or [`STANDARD_CRIT`]
/// for the overwhelming majority of attacks that carry none.
///
/// Fails **open** rather than closed, unlike the negation lookup above,
/// and the asymmetry is deliberate: a miss there leaves a crit
/// standing, which is the state the engine already computed, and a miss
/// here leaves the attack on RAW's default rule, which is the state
/// every attack without a clause is in anyway. Both directions are
/// "change nothing".
pub fn action_crit_profile(action_name: &str) -> ActionCritProfile {
    ACTION_CRIT_PROFILES
        .iter()
        .copied()
        .find(|row| row.action == action_name)
        .unwrap_or(STANDARD_CRIT)
}

/// The extra rolls of the damage pool a critical hit on `action_name`
/// buys — `1` under RAW's *"roll the damage dice twice"*, `2` for a
/// Blade of Disaster.
///
/// Returned as the **extra** count rather than the total because that
/// is the shape both damage sites want: each already holds one roll of
/// the pool for the ordinary hit and adds to it only when the swing
/// crits, so a total would make every caller subtract one.
pub fn crit_extra_rolls(action_name: &str) -> u32 {
    action_crit_profile(action_name).dice_multiple.saturating_sub(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_attack_with_no_clause_of_its_own_gets_raws_default() {
        let profile = action_crit_profile("longsword");
        assert_eq!(profile, STANDARD_CRIT);
        assert_eq!(
            crit_extra_rolls("longsword"),
            1,
            "RAW's \"roll the damage dice twice\" is one extra roll of the pool"
        );
    }

    #[test]
    fn the_blade_of_disaster_crits_on_eighteen_for_triple_dice() {
        let profile = action_crit_profile("blade of disaster");
        assert_eq!(profile.threshold, Some(18));
        assert_eq!(profile.dice_multiple, 3);
        assert_eq!(crit_extra_rolls("blade of disaster"), 2);
    }

    #[test]
    fn no_profile_can_make_a_critical_hit_weaker_than_an_ordinary_one() {
        for row in ACTION_CRIT_PROFILES {
            assert!(
                row.dice_multiple >= 2,
                "{} would roll its pool {} time(s) on a crit",
                row.action,
                row.dice_multiple
            );
            assert!(
                row.threshold.is_none_or(|t| (2..=20).contains(&t)),
                "{} crits on a face the d20 cannot show",
                row.action
            );
        }
    }

    /// The table is keyed by a string, so the one failure mode it has
    /// is a row that names nothing: rename the spell and the clause
    /// silently stops applying, with no compile error and no test
    /// failure anywhere else. Pinned by asking the action itself.
    #[test]
    fn the_blade_of_disaster_profile_names_a_live_action() {
        use crate::actions::action_template::Action;
        let blade: &dyn Action = &*crate::actions::spells::BLADE_OF_DISASTER;
        assert_eq!(
            action_crit_profile(blade.name()).threshold,
            Some(18),
            "the blade's crit clause is keyed by its name and stopped matching it"
        );
    }

    #[test]
    fn every_row_names_a_distinct_action() {
        let mut seen: Vec<&str> = Vec::new();
        for row in ACTION_CRIT_PROFILES {
            assert!(
                !seen.contains(&row.action),
                "two profiles claim {}; the first would always win",
                row.action
            );
            seen.push(row.action);
        }
    }
}
