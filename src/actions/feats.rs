//! **Feats** — SRD 5.2's fourth character-building axis, and the one
//! the engine had exactly one of.
//!
//! A class gives you a chassis, a subclass gives it a direction, a
//! species gives it a body; a feat is the thing a character picks that
//! none of those three decided. SRD 5.2 sorts them into four
//! categories — Origin, General, Fighting Style and Epic Boon — and the
//! engine already carries the whole Fighting Style column (Archery,
//! Defense, Great Weapon Fighting, Two-Weapon Fighting, plus the
//! Protection and Interception styles) on template flags of its own,
//! because those arrived as class features rather than as feats.
//!
//! What was missing is everything else. This module is the start of it,
//! and its three entries were chosen on one axis: does the feat change
//! a die the engine already rolls?
//!
//! | feat | category | what it moves |
//! |------|----------|---------------|
//! | Alert | Origin | the initiative roll |
//! | Savage Attacker | Origin | one weapon damage roll a turn |
//! | Grappler | General | attack rolls against what you are holding |
//!
//! ## Why tags rather than fields
//!
//! `CreatureTemplate` carries some thirty `has_*` booleans for exactly
//! this kind of passive, and every one of them is a field the struct
//! and its `defaults()` have to know about. A feat is the case that
//! argues hardest against another: feats are *many* and each is small,
//! so they ride `features` — the same `HashSet<&'static str>` the
//! class-feature tags use — and reach the engine through
//! `ActorInstance::has_passive_feature`. Adding a fourth feat is one
//! constant here, one row in a cohort table, and one line on whichever
//! chassis takes it.
//!
//! ## What each feat gives up
//!
//! Every one of the three has clauses with no engine surface, and they
//! are named per-feat below rather than quietly dropped. The pattern is
//! the same one the rest of the engine follows: the clause that moves a
//! die ships, and the clause that needs a subsystem nobody has built
//! (an out-of-combat economy, a choice prompt mid-roll, a
//! drag-a-body movement lane) is written down as absent.

/// **Alert** (Origin feat) — *"When you roll Initiative, you can add
/// your Proficiency Bonus to the roll."*
///
/// One row on `PROFICIENCY_INITIATIVE_BONUSES`, alongside the Watchers
/// Paladin's Aura of the Sentinel, which is the same full-proficiency
/// bump arriving from a different direction. That cohort's own docstring
/// has been listing "a hypothetical Alert-style feat" as its example of
/// a future row since it was written.
///
/// Going earlier in the order is worth more than it looks in this
/// engine: the opening round decides who is standing where when the
/// first area spell lands, and a caster who beats the enemy line to
/// initiative gets their concentration up before anything can break it.
///
/// **Initiative Swap is not modeled.** RAW lets the holder trade
/// initiative with a willing ally immediately after rolling. The engine
/// has no channel for a decision taken between the roll and the order
/// being fixed, and no AI heuristic for whether a swap is worth making;
/// a swap made badly is worse than none.
pub const ALERT_TAG: &str = "feat.alert";

/// **Savage Attacker** (Origin feat) — *"Once per turn when you hit a
/// target with a weapon, you can roll the weapon's damage dice twice
/// and use either roll against the target."*
///
/// Read at the weapon-damage chokepoint in `engine::attack`, through the
/// shared once-per-turn ledger every other "once on your turn" rider in
/// the engine uses (`ONCE_PER_TURN_RIDER_TAGS`).
///
/// "Either roll" is resolved as the better one. RAW leaves the choice
/// to the player and there is no reason a player would ever take the
/// smaller number, so the choice is not a choice — the same reading
/// Portent's substitution and the Halfling's Lucky reroll already use.
///
/// The whole pool is rerolled, crit dice included, because RAW's unit is
/// "the weapon's damage dice" for that attack and a critical hit's
/// doubled dice are those dice. Flat modifiers are not rerolled, which
/// is the same rule the crit doubling itself follows.
///
/// Weapon attacks only. A Fire Bolt is not a weapon, and the spell lane
/// rolls its damage through a different chokepoint anyway.
pub const SAVAGE_ATTACKER_TAG: &str = "feat.savage_attacker";

/// **Grappler** (General feat) — of whose four clauses the engine
/// carries the one that moves a die: *"Attack Advantage. You have
/// Advantage on attack rolls against a creature Grappled by you."*
///
/// Read at `EncounterInstance::attack_mode_tally` off the `Grappled`
/// back-link, which is what makes "by you" enforceable: a creature held
/// by somebody else, or restrained by a Web with no grappler behind it,
/// hands this feat nothing. That link is the same one `GrappleEscape`
/// contests against and the same one Grappled's own
/// disadvantage-against-anyone-else clause reads.
///
/// It is the clause that makes grappling a *plan* rather than a way to
/// spend an Action: hold the ogre, then swing at it with advantage for
/// the rest of the fight, and watch it swing back at disadvantage
/// against everyone but you.
///
/// Three clauses are absent, each for a reason the engine states
/// elsewhere:
///
///   - **Ability Score Increase** — the engine builds finished stat
///     blocks and has no level-up lane to apply one in.
///   - **Punch and Grab** ("use both the Damage and the Grapple option
///     of an Unarmed Strike") — the engine's Grapple is its own Action
///     rather than an option on a strike, so there is no pair to fold
///     together.
///   - **Fast Wrestler** ("you don't have to spend extra movement to
///     move a creature Grappled by you") — an exemption from a cost the
///     engine does not charge. `Condition::Grappled`'s third RAW clause,
///     the one about dragging a captive along at a foot per foot, is
///     itself unmodeled; a grappler walks away and the hold breaks on
///     the range check instead.
pub const GRAPPLER_TAG: &str = "feat.grappler";

/// Every feat tag in the engine.
///
/// One list, for the reason `pc_template_families` and
/// `all_summon_spells` are each one list: the invariant worth checking
/// across feats is that each of them is actually carried by something a
/// player can be. A feat nobody can take is a passive that never fires,
/// and it is invisible by construction — no test fails, no encounter
/// behaves differently, and the constant sits here reading as
/// implemented. See `every_feat_is_carried_by_a_playable_chassis`.
pub const FEAT_TAGS: &[&str] = &[ALERT_TAG, SAVAGE_ATTACKER_TAG, GRAPPLER_TAG];
