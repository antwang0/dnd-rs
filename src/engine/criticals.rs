//! 5e's other answer to a critical hit: the clauses that take one back.
//!
//! Every rule in the engine that *makes* a crit — the natural 20, the
//! widened threshold a Champion fights on, the auto-crit a Paralyzed
//! target hands a melee attacker — lives on the attacker's side of the
//! swing and is resolved by the two attack chokepoints. This module is
//! the defender's side, and it holds exactly one sentence: *"any
//! Critical Hit against you becomes a normal hit."*
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
