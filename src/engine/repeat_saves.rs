//! SRD 5.2's *"repeats the save at the end of each of its turns,
//! ending the effect on itself on a success"* — for the effects that
//! nobody is concentrating on.
//!
//! The engine has had that clause since Hold Person: `ROUND_END_SAVES`
//! walks a table of conditions once per actor at the end of its turn
//! and rolls a fresh save against the caster's spell save DC. What it
//! cannot do is fire without a caster, and it says so in its own
//! docstring — *"Only fires when the condition came from a
//! concentration spell; permanent or timer-only applications (e.g.
//! monster innate stun) don't grant repeated saves."*
//!
//! That is a real gap and the bestiary is full of it. A sphinx roaring
//! a party into paralysis is not concentrating on the roar; neither is
//! a metallic dragon whose weakening breath RAW lets its victims shake
//! off, nor a silver dragon whose paralysing one does, nor a Sword of
//! Wounding. Every one of those wrote the same apology into its own
//! comment and collapsed the escape into a short flat timer instead:
//!
//! > *RAW lets the target repeat the save at the end of each of its
//! > turns. The engine's repeated-save table is anchored on a
//! > concentrating caster and a dragon is not concentrating, so the
//! > escape collapses into a short timer — three rounds, which is about
//! > what a middling save buys.*
//!
//! Three rounds is not what a middling save buys. It is what a
//! *median* save buys, and the whole point of a repeat is the variance
//! either side of the median: the fighter with a +8 Constitution save
//! should shake it off on the first try most of the time, and the
//! wizard with a −1 should sometimes still be paralysed a minute
//! later. A flat timer gives them both exactly three rounds, which is
//! the one outcome RAW never produces.
//!
//! ## What it is
//!
//! A ledger on the encounter, keyed by victim: *this condition, this
//! DC, rolled with this ability, against this source.* One entry per
//! condition per victim. The end-of-turn sweep rolls it and clears the
//! condition on a success.
//!
//! ## How it differs from its two neighbours
//!
//! - **`ROUND_END_SAVES`** is the same *moment* and the same failure
//!   branch, and finds its DC by asking who is concentrating. This one
//!   is handed the DC when the effect lands, because there is nobody to
//!   ask. The two never both fire on the same condition instance: the
//!   table skips anything with no concentration owner, and this ledger
//!   only holds what was put on it explicitly.
//! - **`engine::staged_saves`** is the same moment and the *opposite*
//!   failure branch: its repeat escalates the victim to a worse
//!   condition, where this one is a pure escape hatch. A ladder gets
//!   one repeat and then resolves; this one repeats until it is shaken
//!   off or its timer runs out.
//!
//! ## Who is on it
//!
//! Four clauses, and between them they are the shape of the whole
//! problem:
//!
//! | clause | condition | cap |
//! |--------|-----------|-----|
//! | Androsphinx's second Roar | Paralyzed | 1 minute |
//! | Silver dragon's Paralyzing Breath, second rung | Paralyzed | 1 minute |
//! | Metallic Weakening Breath | Enfeebled | 1 minute |
//! | Sword of Wounding | Wounded | RAW's hour |
//!
//! The silver dragon's is the interesting one, because it arrives from
//! `engine::staged_saves` rather than from an action: a two-stage
//! ladder whose *second* rung is shakeable, which is a shape neither
//! ledger could express alone. See `StagedSave::second_escape`.
//!
//! ## The timer is still there
//!
//! Registering a repeat does not make the condition permanent. RAW's
//! clauses on this lane almost always carry a cap — *"After 1 minute,
//! it succeeds automatically"* — and the engine spells that cap as the
//! condition's own `Rounds(n)`. The ledger is the escape *hatch*, not
//! the clock: a victim who never rolls well enough is freed by the
//! timer at the end, exactly as RAW says.

use crate::conditions::{Condition, ConditionTimer};
use crate::engine::encounter::EncounterInstance;
use crate::engine::types::AbilityScoreType;

/// One stat block's *"repeats the save at the end of each of its
/// turns"* clause.
///
/// Declared as a static beside the effect that installs it, so the
/// clause is one value at one site rather than a shape spread across
/// the action that opens it and the sweep that closes it — the same
/// arrangement `staged_saves::StagedSave` uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RepeatSave {
    /// The effect's name as printed, for the log and for telling two
    /// escapes on the same victim apart.
    pub name: &'static str,
    /// The condition the victim is trying to shake off. Also the key:
    /// one entry per condition per victim.
    pub condition: Condition,
    /// The ability the repeat is rolled with. Always the one the
    /// opening save used — RAW says *"repeats the save"*, not *"makes a
    /// save"*.
    pub ability: AbilityScoreType,
    /// The clause of the log line for the moment the victim breaks out
    /// — "shakes the ringing out of its head".
    pub escaped_flavor: &'static str,
}

/// One escape the encounter owes a victim: which clause, from whom, at
/// what DC.
#[derive(Debug, Clone, Copy)]
pub struct PendingRepeat {
    pub clause: &'static RepeatSave,
    /// Who inflicted it. Only used to route the roll through
    /// `roll_save_against_caster_vs_condition`, so the source's own
    /// save-mode riders (a Heightened Spell prime, an aura) apply to
    /// the repeat exactly as they did to the opening save.
    pub source_id: usize,
    pub dc: i32,
}

impl EncounterInstance {
    /// Install `clause.condition` on `victim_id` and register the
    /// end-of-turn escape that goes with it.
    ///
    /// One call rather than two, because the two halves are one rule
    /// and an install without its escape is a strictly crueller effect
    /// than RAW wrote. `timer` is RAW's cap — the *"After 1 minute, it
    /// succeeds automatically"* clause — and the ledger is what happens
    /// before the cap is reached.
    ///
    /// A victim immune to the condition gets no entry at all: the
    /// install bounces, and an escape hatch on a condition that is not
    /// standing would roll dice every round for nothing and print a
    /// line about a paralysis nobody has.
    ///
    /// A victim already escaping this same condition has its entry
    /// **replaced** rather than duplicated. RAW says nothing about
    /// being caught twice, and the alternative is two ledger rows
    /// racing to free the same creature — two rolls a turn against one
    /// condition, which is a much better deal than being hit once.
    pub fn begin_repeat_save(
        &mut self,
        victim_id: usize,
        source_id: usize,
        dc: i32,
        clause: &'static RepeatSave,
        timer: ConditionTimer,
    ) {
        use crate::engine::side_effects::{ApplicableSideEffect, ApplyCondition};
        ApplyCondition {
            actor_id: victim_id,
            condition: clause.condition,
            timer,
        }
        .apply(self);
        // The install can bounce on immunity — static, dynamic, or an
        // aura's. Nothing to escape from if it did.
        if self
            .actors
            .get(&victim_id)
            .is_none_or(|a| !a.has_condition(clause.condition))
        {
            return;
        }
        let entries = self.repeat_saves.entry(victim_id).or_default();
        let pending = PendingRepeat {
            clause,
            source_id,
            dc,
        };
        match entries
            .iter_mut()
            .find(|e| e.clause.condition == clause.condition)
        {
            Some(existing) => *existing = pending,
            None => entries.push(pending),
        }
    }

    /// The end-of-turn half. Called once per actor from `round_end`,
    /// beside `apply_round_end_saves` and `tick_staged_save`.
    ///
    /// Three outcomes per entry, in the order they are checked:
    ///
    ///   1. The condition is **gone** — the timer ran out, an ally
    ///      dispelled it, the source died and took a linked condition
    ///      with it. The entry goes with it; RAW's repeat is
    ///      conditioned on still being caught.
    ///   2. The victim is **down**. No roll, and the entry *stays*: a
    ///      creature healed back to its feet is still paralysed and
    ///      still owed its escape. Matches `tick_staged_save`'s
    ///      handling of the same case.
    ///   3. Otherwise **roll**. On a success the condition and the
    ///      entry both go; on a failure both stay and it is rolled
    ///      again next turn.
    pub(crate) fn tick_repeat_saves(&mut self, victim_id: usize) {
        let Some(pending) = self.repeat_saves.get(&victim_id).cloned() else {
            return;
        };
        // Insertion order, which is deterministic — a victim with two
        // escapes owed resolves them in the order they were inflicted.
        for entry in pending {
            let still_caught = self
                .actors
                .get(&victim_id)
                .is_some_and(|a| a.has_condition(entry.clause.condition));
            if !still_caught {
                self.drop_repeat_save(victim_id, entry.clause.condition);
                continue;
            }
            if self
                .actors
                .get(&victim_id)
                .is_none_or(|a| !a.is_combat_active())
            {
                continue;
            }
            let name = self.actor_name(victim_id);
            let save = self.roll_save_against_caster_vs_condition(
                victim_id,
                entry.clause.ability,
                entry.dc,
                entry.source_id,
                entry.clause.condition,
            );
            if !save.passed() {
                continue;
            }
            if let Some(a) = self.actors.get_mut(&victim_id) {
                a.remove_condition(entry.clause.condition);
            }
            self.drop_repeat_save(victim_id, entry.clause.condition);
            self.log(format!(
                "  {}: {} {}.",
                entry.clause.name, name, entry.clause.escaped_flavor
            ));
        }
    }

    /// Forget the escape owed to `victim_id` for `condition`, leaving
    /// any other entry on the same victim alone.
    fn drop_repeat_save(&mut self, victim_id: usize, condition: Condition) {
        if let Some(entries) = self.repeat_saves.get_mut(&victim_id) {
            entries.retain(|e| e.clause.condition != condition);
            if entries.is_empty() {
                self.repeat_saves.remove(&victim_id);
            }
        }
    }

    /// True while `victim_id` is owed an escape from `condition`.
    ///
    /// Read by the tests, and by anything that wants to tell a creature
    /// paralysed by a sphinx's roar — which will get a save every turn
    /// — from one held by a Hold Monster, which gets its save from
    /// `ROUND_END_SAVES` instead, or one paralysed by a ghoul, which
    /// gets none at all.
    pub fn repeat_save_pending(&self, victim_id: usize, condition: Condition) -> bool {
        self.repeat_saves
            .get(&victim_id)
            .is_some_and(|entries| entries.iter().any(|e| e.clause.condition == condition))
    }

    /// Drop every escape owed to `victim_id`, without rolling any of
    /// them.
    ///
    /// Called when the body leaves the board, so a corpse's entries do
    /// not outlive it and greet whoever inherits the id — the same
    /// hazard `clear_staged_save` exists for.
    pub(crate) fn clear_repeat_saves(&mut self, victim_id: usize) {
        self.repeat_saves.remove(&victim_id);
    }
}
