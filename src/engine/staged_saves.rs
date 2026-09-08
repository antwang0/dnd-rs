//! SRD 5.2's **two-stage save ladder** — the stat-block clause that
//! reads *"First Failure: … Second Failure: …"*.
//!
//! Eight stat blocks carry one, and they all say the same thing in the
//! same order. A creature that fails the save is caught but not lost:
//! it takes a lesser condition, and *"repeats the save at the end of its
//! next turn, ending the effect on itself on a success."* Fail that
//! second one and the lesser condition is replaced by the real one.
//!
//! The engine had neither half. Every petrification on the roster —
//! the basilisk's gaze, the cockatrice's bite, the gorgon's breath, the
//! medusa's gaze — resolved as one save and, on a failure, a flat
//! `Petrified` for a single round. Each of those four had the same
//! docstring apology beside it: RAW's petrification is permanent and
//! lethal, so the timer was cut to one round *"so a single hit doesn't
//! game-over the target on a missed save."*
//!
//! That apology is the tell. The clause the engine was missing is
//! precisely the one RAW uses to solve the same problem: you do not get
//! turned to stone by a missed save, you get turned to stone by missing
//! twice with a round in between to be rescued. Modelling the ladder
//! lets the second rung be a real petrification instead of a
//! one-round flicker, because the first rung is what keeps a single
//! unlucky die from ending somebody's fight.
//!
//! ## What it is, and what it is not
//!
//! It is not a condition. `Restrained` is a real condition with a dozen
//! consumers and the first rung installs exactly that; what the engine
//! needs on top is a *pending escalation* — a DC, a source, and a
//! second condition waiting on one more failed roll. That is three
//! values with no home on the actor, so they live in a ledger on the
//! encounter, keyed by victim, the same way `trait_immunities` does.
//!
//! It is not `ROUND_END_SAVES` either, though it sits beside it. That
//! table is "roll again, and a success ends it" — the ladder shares the
//! roll and disagrees about the failure branch, which is the whole
//! point of it. It is also gated on the condition having come from a
//! concentration spell, and a basilisk is not concentrating on
//! anything.
//!
//! ## Where it diverges
//!
//! RAW's second rung has no duration: a petrified creature stays stone
//! until Greater Restoration finds it. The engine caps it (see
//! `PETRIFIED_ROUNDS`) because an encounter that ends with a character
//! permanently statue-shaped has no way to continue, and the cap is now
//! *long* rather than nominal — five rounds instead of one — which it
//! can afford to be precisely because getting there costs two failures
//! rather than one.

use crate::conditions::{Condition, ConditionTimer};
use crate::engine::encounter::EncounterInstance;
use crate::engine::types::AbilityScoreType;

/// How long the second rung holds, for the ladders that end in stone.
///
/// RAW is indefinite — "the target has the Petrified condition", with
/// no timer and no repeated save, ended by Greater Restoration or not
/// at all. Five rounds is the engine's cap on that, and the number
/// moved up from the one round the flat petrifications used to install:
/// a rung reached by failing twice, with a full round in between for
/// somebody to do something about it, can be allowed to actually
/// matter. A creature that spends half a minute as a statue has lost
/// the fight, which is the correct outcome for two failed saves against
/// a basilisk.
pub const PETRIFIED_ROUNDS: u32 = 5;

/// One stat block's *"First Failure / Second Failure"* clause.
///
/// Declared as a static per trait and handed to
/// `EncounterInstance::begin_staged_save`, so a stat block's ladder is
/// one value at one site rather than a shape spread across the action
/// that opens it and the round-end sweep that closes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StagedSave {
    /// The trait's name as printed, for the log and for telling two
    /// ladders on the same victim apart.
    pub name: &'static str,
    /// The ability the repeat is rolled with. Always the same one the
    /// opening save used — RAW says "repeats the save", not "makes a
    /// save".
    pub ability: AbilityScoreType,
    /// What the first failure installs. RAW's petrification ladders all
    /// say `Restrained`; the silver dragon's paralysis ladder says
    /// `Incapacitated`.
    pub first: Condition,
    /// What a second failure installs *instead of* the first — RAW's
    /// wording, and the reason the first is removed rather than added
    /// to.
    pub second: Condition,
    /// How long the second rung holds. See `PETRIFIED_ROUNDS`.
    pub second_timer: ConditionTimer,
    /// The clause of the log line for the moment the ladder opens —
    /// "feels its limbs stiffening".
    pub caught_flavor: &'static str,
    /// …for shaking it off — "shrugs off the creeping stone".
    pub escaped_flavor: &'static str,
    /// …and for the second failure — "turns to stone".
    pub lost_flavor: &'static str,
}

/// The petrification ladder, which four stat blocks share verbatim:
/// the basilisk's gaze, the cockatrice's bite, the gorgon's breath and
/// the medusa's gaze. RAW: *"First Failure: The target has the
/// Restrained condition and repeats the save at the end of its next
/// turn if it is still Restrained, ending the effect on itself on a
/// success. Second Failure: The target has the Petrified condition
/// instead of the Restrained condition."*
///
/// One static rather than four, because it is one sentence printed four
/// times. What differs between the four creatures is the DC and how
/// they deliver it, and both of those are arguments.
pub static PETRIFICATION: StagedSave = StagedSave {
    name: "petrification",
    ability: AbilityScoreType::Constitution,
    first: Condition::Restrained,
    second: Condition::Petrified,
    second_timer: ConditionTimer::Rounds(PETRIFIED_ROUNDS),
    caught_flavor: "feels their limbs stiffening",
    escaped_flavor: "shrugs off the creeping stone",
    lost_flavor: "turns to stone",
};

/// The brass dragon's **Sleep Breath** ladder. RAW: *"Failure: The
/// target has the Incapacitated condition until the end of its next
/// turn, at which point it repeats the save. Second Failure: The target
/// has the Unconscious condition for 1 minute … This effect ends for
/// the target if it takes damage or a creature within 5 feet of it
/// takes an action to wake it."*
///
/// The second rung is `Asleep` rather than `Unconscious`, which is the
/// engine's distinction and exactly the one RAW's last sentence draws:
/// both put a creature on the floor, and only one of them ends the
/// moment somebody hits it. A brass dragon that puts the party to sleep
/// has bought a round, not the fight.
pub static SLEEP_BREATH: StagedSave = StagedSave {
    name: "sleep breath",
    ability: AbilityScoreType::Constitution,
    first: Condition::Incapacitated,
    second: Condition::Asleep,
    // RAW's minute, at the engine's six seconds to the round. Longer
    // than the petrification rung can afford to be, because this one
    // ends the moment anybody lands a blow.
    second_timer: ConditionTimer::Rounds(10),
    caught_flavor: "sags, eyelids heavy",
    escaped_flavor: "shakes the drowsiness off",
    lost_flavor: "drops where they stand, fast asleep",
};

/// The silver dragon's **Paralyzing Breath** ladder. RAW: *"First
/// Failure: The target has the Incapacitated condition until the end of
/// its next turn, when it repeats the save. Second Failure: The target
/// has the Paralyzed condition, and it repeats the save at the end of
/// each of its turns … After 1 minute, it succeeds automatically."*
///
/// The same two rungs as the brass dragon's sleep with a crueller
/// second: paralysis auto-fails Strength and Dexterity saves and makes
/// every melee hit a critical, and unlike sleep it does not break when
/// somebody hits you — which is the whole reason a silver dragon
/// breathes this second and its cold first.
///
/// RAW's per-turn repeat on the second rung collapses into the timer,
/// the way every other "repeats the save each turn" outside
/// `ROUND_END_SAVES` does in this engine: that table is anchored on a
/// concentrating caster, and a dragon is not concentrating.
pub static PARALYZING_BREATH: StagedSave = StagedSave {
    name: "paralyzing breath",
    ability: AbilityScoreType::Constitution,
    first: Condition::Incapacitated,
    second: Condition::Paralyzed,
    second_timer: ConditionTimer::Rounds(3),
    caught_flavor: "stiffens as the frost bites",
    escaped_flavor: "forces their limbs back under control",
    lost_flavor: "freezes solid where they stand",
};

/// A ladder the encounter is part-way through on one creature: which
/// one, who opened it, and at what DC.
#[derive(Debug, Clone, Copy)]
pub struct PendingStage {
    pub ladder: &'static StagedSave,
    pub source_id: usize,
    pub dc: i32,
    /// True on the turn the ladder opened, so the repeat lands at the
    /// end of the victim's *next* turn rather than at the end of the
    /// one it was caught on.
    ///
    /// RAW is explicit about this — "repeats the save at the end of its
    /// next turn" — and it is the clause that makes the first rung
    /// worth having: a creature caught by a gorgon's breath on the
    /// gorgon's turn has a whole turn of its own to be dragged out of
    /// reach, healed, or to walk away, before the die that turns it to
    /// stone is rolled.
    pub opened_this_round: bool,
    /// The timer the first condition had *before* the ladder pinned it,
    /// or `None` when the ladder installed the condition itself.
    ///
    /// The engine holds one instance of a condition per creature with
    /// no refcount, which makes both halves of the ladder's lifecycle
    /// awkward and in opposite directions.
    ///
    /// The ladder has to **pin** the rung: it always installs
    /// `Permanent`, because a victim already `Restrained` on a
    /// one-round timer from an otyugh's tentacle would otherwise shed
    /// the condition before the second save came due, and the gorgon's
    /// breath — an Action *and* a recharge — would produce no roll, no
    /// log and no effect at all.
    ///
    /// And it has to **hand the rung back**: a fighter webbed by a
    /// giant spider and then caught by a basilisk must not be cut free
    /// of the web when the ladder resolves — no escape check, no
    /// contest, and the spider's back-link gone with it. That is the
    /// hazard `engine::attachment` documents for `Blinded`.
    ///
    /// So the ladder remembers what it overwrote. `None` means it put
    /// the condition there and takes it away again; `Some(t)` means it
    /// borrowed one and restores `t` through
    /// `ActorInstance::set_condition_timer`, which writes the timer
    /// without disturbing the link.
    ///
    /// The one thing that is not restored is elapsed time: a web with
    /// four rounds left when the gaze landed gets four rounds back
    /// rather than two. Two rounds of overhang on somebody else's
    /// effect, in exchange for never silently voiding a boss's Action.
    pub restored_timer: Option<ConditionTimer>,
}

impl EncounterInstance {
    /// Open a staged save on `victim_id` — RAW's First Failure branch.
    ///
    /// Installs the ladder's first condition and records the pending
    /// escalation.
    ///
    /// A bare install rather than `install_condition_with_link`, which
    /// is the one place in the engine that is the right call for
    /// `Restrained`. That condition's link means *"who is holding
    /// you"* — `GrappleEscape` reads it to tell a restraint that came
    /// off a roper's tendril from one that came off a Web across the
    /// room — and stone creeping up your legs is not a hold anybody is
    /// maintaining. A link here would offer the victim an Athletics
    /// contest against a basilisk that is not touching it.
    ///
    /// The rung is installed `Permanent` whether or not the victim was
    /// already under it, and whatever timer it displaced is remembered
    /// for the resolution to put back — see
    /// `PendingStage::restored_timer` for both halves of why.
    ///
    /// A victim already on this ladder is left where it is: RAW says
    /// nothing about being caught twice, and re-opening would hand the
    /// creature a fresh full turn before its repeat — turning a second
    /// exposure into a *reprieve*, which is the wrong direction for a
    /// second exposure to push.
    ///
    /// A victim immune to the first condition never starts the ladder
    /// at all, which is RAW by the ordinary route: the install bounces,
    /// and a ladder whose first rung is not standing has nothing to
    /// escalate from.
    pub fn begin_staged_save(
        &mut self,
        victim_id: usize,
        source_id: usize,
        dc: i32,
        ladder: &'static StagedSave,
    ) {
        use crate::engine::side_effects::{ApplicableSideEffect, ApplyCondition};
        if self.staged_saves.contains_key(&victim_id) {
            return;
        }
        let Some(victim) = self.actors.get(&victim_id) else {
            return;
        };
        if victim.effectively_immune_to_condition(ladder.first) {
            return;
        }
        // Whatever clock the rung was already on, if any. Read before
        // the install, which is about to overwrite it.
        let restored_timer = victim.conditions().get(&ladder.first).copied();
        ApplyCondition {
            actor_id: victim_id,
            condition: ladder.first,
            // No timer of its own: the ladder is what ends it, one way
            // or the other, at the end of the victim's next turn. A
            // `Rounds(n)` here would be a second clock racing the one
            // that matters — and a borrowed one that ran out first
            // would void the ladder silently.
            timer: ConditionTimer::Permanent,
        }
        .apply(self);
        // The install can still have bounced — an aura suppressor, a
        // dynamic immunity the static check does not see. A ladder with
        // no first rung is not a ladder.
        if self
            .actors
            .get(&victim_id)
            .is_none_or(|a| !a.has_condition(ladder.first))
        {
            return;
        }
        let name = self.actor_name(victim_id);
        self.log(format!("  {}: {} {}.", ladder.name, name, ladder.caught_flavor));
        self.staged_saves.insert(
            victim_id,
            PendingStage {
                ladder,
                source_id,
                dc,
                opened_this_round: true,
                restored_timer,
            },
        );
    }

    /// The end-of-turn half — RAW's *"repeats the save at the end of
    /// its next turn"*. Called once per actor from `round_end`.
    ///
    /// Three outcomes, and the ordering of the first two is the rule:
    /// a ladder opened this round is not rolled for at all (it waits a
    /// turn, see `PendingStage::opened_this_round`), and a victim who
    /// has lost the first condition some other way — a Freedom of
    /// Movement, an ally's Dispel, the source's death taking a linked
    /// condition with it — is off the ladder, because RAW's repeat is
    /// conditioned on *"if it is still Restrained."*
    pub(crate) fn tick_staged_save(&mut self, victim_id: usize) {
        let Some(pending) = self.staged_saves.get(&victim_id).copied() else {
            return;
        };
        if pending.opened_this_round {
            if let Some(p) = self.staged_saves.get_mut(&victim_id) {
                p.opened_this_round = false;
            }
            return;
        }
        let still_caught = self
            .actors
            .get(&victim_id)
            .is_some_and(|a| a.has_condition(pending.ladder.first));
        if !still_caught {
            self.staged_saves.remove(&victim_id);
            return;
        }
        // A creature that is already down is not made more so. Left on
        // the ledger rather than cleared, so a healed-up ally picks the
        // ladder back up where it left off.
        if self
            .actors
            .get(&victim_id)
            .is_none_or(|a| !a.is_combat_active())
        {
            return;
        }
        let ladder = pending.ladder;
        let name = self.actor_name(victim_id);
        let save = self.roll_save_against_caster_vs_condition(
            victim_id,
            ladder.ability,
            pending.dc,
            pending.source_id,
            ladder.second,
        );
        self.staged_saves.remove(&victim_id);
        // Hand the rung back, or take it away — see
        // `PendingStage::restored_timer`. A victim who was already
        // webbed when the gaze caught it stays webbed, on either
        // branch, which is both the safe answer and RAW's: the ladder
        // replaces *its own* Restrained with the Petrified and has
        // nothing to say about the spider's.
        if let Some(a) = self.actors.get_mut(&victim_id) {
            match pending.restored_timer {
                Some(timer) => a.set_condition_timer(ladder.first, timer),
                None => {
                    a.remove_condition(ladder.first);
                }
            }
        }
        if save.passed() {
            self.log(format!("  {}: {} {}.", ladder.name, name, ladder.escaped_flavor));
            return;
        }
        self.log(format!("  {}: {} {}.", ladder.name, name, ladder.lost_flavor));
        crate::engine::side_effects::ApplicableSideEffect::apply(
            &crate::engine::side_effects::ApplyCondition {
                actor_id: victim_id,
                condition: ladder.second,
                timer: ladder.second_timer,
            },
            self,
        );
    }

    /// True while `victim_id` is on a ladder — the first rung installed
    /// and the second roll still to come.
    ///
    /// Read by the tests, and by anything that wants to tell a
    /// creature held by a gorgon's breath from one merely tangled in a
    /// net: both are `Restrained`, and only one of them is about to be
    /// a statue.
    pub fn staged_save_pending(&self, victim_id: usize) -> bool {
        self.staged_saves.contains_key(&victim_id)
    }

    /// Drop whatever ladder `victim_id` is on, without resolving it.
    ///
    /// Called when the body leaves the board, so a corpse's entry does
    /// not outlive it and greet whoever inherits the id.
    pub(crate) fn clear_staged_save(&mut self, victim_id: usize) {
        self.staged_saves.remove(&victim_id);
    }
}
