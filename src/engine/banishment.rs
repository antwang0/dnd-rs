//! 5e **Banishment** — the off-board lane, and the only place in the
//! engine where a living creature is not standing anywhere.
//!
//! SRD: *"You attempt to send one creature that you can see within range
//! to another place of existence. … The target is transported to a
//! harmless demiplane. While there, the target is Incapacitated. The
//! target remains there until the spell ends, at which point the target
//! reappears in the space it left or in the nearest unoccupied space if
//! that space is occupied."*
//!
//! Three effects in this engine quote that paragraph — Banishment,
//! Banishing Smite, and the Arcana Cleric's Arcane Abjuration — and
//! until this module none of them could honour it. Each collapsed to an
//! on-board `Incapacitated`, and three separate docstrings said so in
//! almost the same words: *"the engine doesn't yet model off-board
//! status"*, *"we collapse the demi-plane mechanic to a 10-round inert
//! condition"*, *"banishment needs an off-board actor lane the engine
//! doesn't have"*. Maze made it four.
//!
//! The gap was not cosmetic. An inert body left on the grid is still a
//! body: it soaks a Fireball, it blocks a corridor, it hands
//! three-quarters cover to whoever is shooting from behind it, and a
//! paladin's aura still reaches it. Banishing the ogre in the doorway
//! made the doorway *more* blocked, not less, which inverts the entire
//! tactical point of the spell.
//!
//! # The model
//!
//! A creature that is off the board keeps everything about itself —
//! hit points, conditions, spell slots, the timer counting its
//! banishment down — and loses exactly one thing: its **footprint**.
//! [`ActorInstance::off_board`] is the flag, and while it is set the
//! actor owns no tiles in `actor_map`.
//!
//! Three mechanisms carry the consequences of that, and it is worth
//! being exact about which carries what.
//!
//!   - **The grid**, for everything that asks "is anything standing
//!     here". The pathfinder walks around bodies it finds on the map,
//!     the cover ladder counts them, and the renderer draws them. A
//!     creature with no footprint is absent from all three for free.
//!   - **[`ActorInstance::is_combat_active`]**, which the off-board
//!     flag also clears, for everything that asks "is this creature in
//!     the fight". That is the AoE hit list, every AI target walk, the
//!     opportunity-attack dispatcher, the aura sweeps and the
//!     initiative skip — two hundred call sites that already knew how
//!     to say it, because a creature on a demiplane is out of the fight
//!     in precisely the way a creature bleeding out on the floor is.
//!   - **An explicit clause in `Action::validate_input`**, for
//!     targeting something *by name*. That one could not ride either of
//!     the other two: reach is measured off `location`, which an
//!     off-board actor still has, and the targeting lane deliberately
//!     does not ask `is_combat_active` because a dying creature is not
//!     combat-active either and is still very much lying there to be
//!     finished off.
//!
//! `location` is deliberately **not** cleared. It is the space the
//! creature left, and RAW sends it back there.
//!
//! # Why a sweep and not a hook
//!
//! [`EncounterInstance::reconcile_board_presence`] is a reconcile in the
//! exact sense `reconcile_footprints` and `reconcile_altitudes` are, and
//! it runs at the same three chokepoints. The condition
//! (`Condition::removes_from_board`) states where the creature *ought*
//! to be; the sweep is what the board actually does about it.
//!
//! Splitting it that way is what makes the lane safe to extend. A
//! banishment can end four ways here — a lapsed timer, a failed
//! concentration save, a Dispel Magic, a second concentration spell
//! displacing the first — and a fifth way arrives every time somebody
//! adds a spell. None of those paths knows the actor map exists, and
//! none of them has to: they remove a condition, and the next sweep
//! puts the body back. The alternative, an "un-banish" hook on each of
//! the four, is four chances to forget and a growing number of ways to
//! leave a creature stranded on a demiplane for the rest of the fight.
//!
//! # Coming back
//!
//! RAW: *"the space it left, or the nearest unoccupied space"*. The
//! return walks [`EncounterInstance::find_return_anchor`], which tries
//! the remembered tile first and then rings outward. Two things can have
//! happened to the old space in the meantime — somebody moved into it,
//! or a conjured wall was dropped on it — and the ring walk handles both
//! without distinguishing them.
//!
//! A creature that finds **no** legal tile stays off the board and keeps
//! its condition, and the sweep tries again next round. That is a
//! deliberate choice over the two alternatives: dropping it on an
//! occupied tile corrupts the actor map (two ids, one square, and
//! whichever is written second erases the first), and despawning it
//! kills a creature the spell explicitly does not kill. Waiting is the
//! only option that is merely *unlucky*, and it self-repairs the moment
//! anything moves.
//!
//! # What deliberately does not happen off-board
//!
//!   - **No turn.** The initiative slot is reached and passed over —
//!     `process_stack`'s turn loop skips any actor that is not
//!     combat-active, which a banished creature now is not. RAW's
//!     demiplane is harmless and there is nothing there to do.
//!   - **No regeneration.** The round-end regenerator gates on
//!     combat-active, so a banished troll stops knitting. Arguably it
//!     should keep healing on its own plane; charging the party for
//!     removing a regenerator from the fight is the worse of the two
//!     wrong answers.
//!   - **No death saves, no drop.** A creature is banished at whatever
//!     HP it had and comes back with the same number. Nothing on a
//!     harmless demiplane can hurt it, and nothing here tries.
//!
//! And the two things that keep happening, because they are what makes
//! the spell temporary rather than permanent: **condition timers still
//! tick** (`round_end` walks every actor in the table, not every actor
//! in the fight), and **the caster still has to concentrate**.

use crate::conditions::Condition;
use crate::engine::encounter::{EncounterInstance, rings_outward};
use crate::engine::mounts::UnseatCause;
use crate::engine::types::{Coordinate, Size};

impl EncounterInstance {
    /// Hold every actor's presence on the grid in step with whether
    /// anything has taken them off it.
    ///
    /// The board twin of `reconcile_footprints` and
    /// `reconcile_altitudes`, wired at the same three chokepoints and
    /// for the same reason — see the module docs for why this is a
    /// sweep rather than a hook on each of the spells involved.
    ///
    /// Two directions, and they are not symmetric. **Leaving** always
    /// succeeds: a footprint can always be given up. **Returning** can
    /// be refused, because the space the creature left may no longer be
    /// available and neither may anything near it; a refusal leaves the
    /// actor off the board with its condition intact and is retried on
    /// the next sweep.
    ///
    /// Ordered by actor id so a round in which two banishments lapse
    /// together is reproducible from the seed, and so the first returner
    /// claims its tile before the second one looks for room — the same
    /// stamp-before-the-next-check discipline `reconcile_footprints`
    /// keeps for growth.
    pub fn reconcile_board_presence(&mut self) {
        let mut pending: Vec<(usize, bool)> = self
            .actors
            .iter()
            .filter_map(|(id, a)| {
                let want = a.belongs_off_board();
                (want != a.is_off_board()).then_some((*id, want))
            })
            .collect();
        if pending.is_empty() {
            return;
        }
        pending.sort_unstable_by_key(|(id, _)| *id);
        for (actor_id, leaving) in pending {
            if leaving {
                self.lift_actor_off_board(actor_id);
            } else {
                self.return_actor_to_board(actor_id);
            }
        }
    }

    /// Take one actor's body off the grid, keeping the actor itself
    /// whole. Called only by `reconcile_board_presence`, which owns the
    /// decision of when a body is due to leave.
    ///
    /// The footprint is released and the flag is set; nothing else about
    /// the actor is touched, `location` least of all — that is the space
    /// it left and the space `return_actor_to_board` will try first.
    fn lift_actor_off_board(&mut self, actor_id: usize) {
        // A mount that vanishes takes the saddle with it, and RAW's
        // "an effect moves your mount against its will" clause is the
        // one that answers it. The DC 10 buys the rider the difference
        // between landing on their feet and landing prone and nothing
        // more — there is no staying seated on a horse that is on
        // another plane, which is why this is an `always_unseats`
        // cause.
        if self
            .actors
            .get(&actor_id)
            .is_some_and(|a| a.ridden_by().is_some())
        {
            self.unseat(actor_id, UnseatCause::MountBanished);
        }
        // Whatever the unseat left behind, this cuts — and answers the
        // question that decides the line below it. A rider's tiles were
        // never the rider's to release (they belong to the mount), and
        // a mount still carrying somebody has already handed its tiles
        // to the rider standing up on them. Read after the unseat, so
        // the common case where the rider has already been set down
        // beside the horse correctly reports the horse's own footprint
        // as still its own.
        let footprint_handled = self.sever_ride_links(actor_id);
        let Some((origin, size, name)) = self
            .actors
            .get(&actor_id)
            .map(|a| (a.location(), a.size(), a.name().to_string()))
        else {
            return;
        };
        if !footprint_handled {
            self.clear_footprint_of(actor_id, origin, size);
        }
        if let Some(a) = self.get_actor(actor_id) {
            a.set_off_board(true);
        }
        self.log(format!("{} vanishes from the field.", name));
    }

    /// Put one actor's body back on the grid — "the space it left, or
    /// the nearest unoccupied space".
    ///
    /// Returns without complaint when there is nowhere to land. The
    /// actor keeps its off-board flag *and* its condition, so the next
    /// sweep asks again; see the module docs for why waiting beats the
    /// alternatives.
    fn return_actor_to_board(&mut self, actor_id: usize) {
        let Some((origin, size, name)) = self
            .actors
            .get(&actor_id)
            .map(|a| (a.location(), a.size(), a.name().to_string()))
        else {
            return;
        };
        let Some(anchor) = self.find_return_anchor(origin, size) else {
            // Nothing said about it in the log: a body pressing against
            // a full board is not an event, and a crowded fight would
            // otherwise emit the same line every round until a tile
            // opened.
            return;
        };
        self.stamp_footprint_of(actor_id, anchor, size);
        if let Some(a) = self.get_actor(actor_id) {
            a.set_off_board(false);
            a.set_location(anchor);
        }
        if anchor == origin {
            self.log(format!("{} reappears where it stood.", name));
        } else {
            self.log(format!("{} reappears at {}.", name, anchor));
        }
        // A creature that comes back inside a Cloudkill is standing in
        // the Cloudkill. Ordered after the flag is cleared and the
        // location is written, both of which this reads: `touch_zones`
        // bails on anything that is not combat-active, which an
        // off-board creature is not, and it measures the tile the actor
        // says it is on.
        //
        // The same call `unlink_ride` makes for a dismounted rider, and
        // for the same reason — a body that arrives on a tile without
        // walking there has still arrived on it, and the zone layer's
        // only other entry point is the pathfinder.
        self.touch_zones(actor_id);
    }

    /// The tile a returning body lands on: `origin` if a `size`
    /// footprint still fits there, else the closest one that does.
    ///
    /// Distinct from `find_adjacent_spawn`, which is the summoner's
    /// walk and deliberately refuses hazardous ground and faces the
    /// nearest enemy. Neither applies here. A returning creature is not
    /// being placed by anybody — RAW names its destination, and the
    /// search is only for what to do when that destination is gone. It
    /// takes the nearest tile that exists, hazard and facing included,
    /// because the alternative to a bad tile is another round on the
    /// demiplane.
    ///
    /// The radius is bounded rather than unlimited: a creature whose
    /// space and whole neighbourhood have filled in waits for room
    /// instead of being flung across the arena to whatever corner
    /// happened to be free.
    pub(crate) fn find_return_anchor(
        &self,
        origin: Coordinate,
        size: Size,
    ) -> Option<Coordinate> {
        let fits = |anchor: Coordinate| self.footprint_is_clear(anchor, size);
        if fits(origin) {
            return Some(origin);
        }
        rings_outward(origin, RETURN_SEARCH_RADIUS).find(|&c| fits(c))
    }
}

/// How far from its remembered space a returning creature will look for
/// room, in tiles.
///
/// Four rings is a little over 20 feet — far enough to clear the body
/// that walked into the space and the wall somebody conjured across it,
/// and short enough that "the nearest unoccupied space" still means
/// something. Beyond this the creature waits rather than teleporting
/// itself out of the fight it was pulled from.
const RETURN_SEARCH_RADIUS: isize = 4;

/// Condition-facing helper: every condition that would keep its holder
/// off the board, in installation order.
///
/// Exists so the tests and the AI can talk about "the banishment
/// cohort" without either re-typing the match in
/// `Condition::removes_from_board` or being able to drift from it — the
/// slice is checked against the predicate by
/// `every_off_board_condition_is_on_the_roster`.
pub const OFF_BOARD_CONDITIONS: &[Condition] = &[Condition::Banished, Condition::Mazed];

#[cfg(test)]
mod tests {
    use super::*;

    /// The roster and the predicate are two spellings of one list, and
    /// the whole point of the roster is that it cannot drift from the
    /// predicate. A future off-board condition added to
    /// `removes_from_board` and not here fails this.
    #[test]
    fn every_off_board_condition_is_on_the_roster() {
        for c in OFF_BOARD_CONDITIONS {
            assert!(
                c.removes_from_board(),
                "{} is on the roster but the predicate says otherwise",
                c.name()
            );
        }
        // The other direction needs a list of every condition, which the
        // enum does not hand out; the roster's own length is the stand-in
        // — a new arm on `removes_from_board` that forgets the roster
        // trips this count.
        let counted = [
            Condition::Banished,
            Condition::Mazed,
            // Two nearby conditions that are deliberately *not* off-board
            // — see `removes_from_board`. Listed so a change of heart
            // about either one has to be a change to this test as well.
            Condition::Sphered,
            Condition::Caged,
        ]
        .iter()
        .filter(|c| c.removes_from_board())
        .count();
        assert_eq!(
            counted,
            OFF_BOARD_CONDITIONS.len(),
            "the predicate and the roster disagree about who is off the board"
        );
    }

    /// Every off-board condition also has to be an incapacitation.
    /// Nothing enforces the pairing structurally — they are separate
    /// cohorts — and a condition that took the body away but left the
    /// action economy would be a creature swinging from another plane.
    #[test]
    fn nothing_acts_from_a_demiplane() {
        for c in OFF_BOARD_CONDITIONS {
            assert!(
                c.blocks_action_economy(),
                "{} takes the body off the board but leaves it acting",
                c.name()
            );
            assert!(
                c.zeros_movement(),
                "{} takes the body off the board but leaves it walking",
                c.name()
            );
        }
    }
}
