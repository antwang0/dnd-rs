//! **Board settings** — the facts about the place a *game* is happening,
//! as opposed to the facts about one room of it.
//!
//! A dungeon run in this engine is a chain of `EncounterInstance`s:
//! `App::start_next_encounter` takes the survivors out of the room that
//! just ended, rests them, and walks them into a freshly generated one.
//! Everything about the *party* survives that boundary, because the
//! party is carried across it as a list of `ActorInstance`s. Everything
//! about the *board* is built fresh, because the board is generated from
//! `TerrainGenParams` and `ActorGenParams` and those describe a shape
//! rather than a place.
//!
//! Four things the player asked for on the command line are neither.
//! They are not the party and they are not the generator's shape — they
//! are what the sky is doing and what is under the flagstones, and they
//! belong to the whole run:
//!
//!   - the **ambient light**, so a `--dark` game does not walk into
//!     daylight the moment the party takes a rest;
//!   - the **weather**, for exactly the same reason, and it did not have
//!     it. `--rain` bought one wet room and then calm forever, which is
//!     not a weather system, it is a weather *event* — and the comment
//!     in `start_next_encounter` arguing the light across the boundary
//!     had been making this struct's whole argument about the light
//!     alone for as long as the weather had existed;
//!   - the **trap density**, which was worse off still: the count lived
//!     in `main`, was spent on the first board by `scatter_traps`, and
//!     was not recorded anywhere afterwards. A `--traps=8` run had eight
//!     traps in room one and a clean floor for the rest of the dungeon;
//!   - the **rift density**, which is the fourth answer the paragraph
//!     below predicted, and which arrived already knowing where it
//!     lived because of it.
//!
//! One struct rather than four fields on `App`, because the four are one
//! question — *what kind of place is this run happening in?* — and
//! because whatever the command line grows next will be a fifth answer
//! to it. [`BoardSettings::apply`] is the single place that knows how to
//! put them onto a board, so `main`'s first room and every room after it
//! are set up by the same four lines rather than by two copies that can
//! drift.
//!
//! ## What is deliberately *not* here
//!
//! The seed. RAW has nothing to say about it, but the engine does: a
//! seed names one board, and carrying it forward would mean every room
//! in the dungeon being the same room. `App` passes `None` to the
//! generator for exactly that reason and this struct keeps out of it.

use crate::engine::encounter::EncounterInstance;
use crate::engine::lighting::AmbientLight;
use crate::engine::weather::Weather;

/// The board-level settings a whole dungeon run is played under.
///
/// `Default` is the game as it was played before any of them existed: a
/// lit hall, still air, and an unbroken floor. That matters more
/// than it looks — it is what lets every test and every caller that does
/// not care about the sky keep saying nothing about it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BoardSettings {
    /// See [`crate::engine::lighting::AmbientLight`].
    pub ambient: AmbientLight,
    /// See [`crate::engine::weather::Weather`].
    pub weather: Weather,
    /// How many of SRD 5.2's traps each room is armed with — see
    /// [`crate::engine::traps`]. A *density*, not a list: the traps
    /// themselves are rolled fresh per room, which is the only reading
    /// that makes sense of carrying the number forward. A party does not
    /// walk into the same pit twice.
    pub traps: usize,
    /// How many rifts the floor of each room is cracked open by — see
    /// [`crate::engine::encounter::EncounterInstance::carve_rifts`] and
    /// [`crate::engine::jumping`], which is the rule they exist for.
    ///
    /// A density like `traps`, and carried across the boundary for the
    /// same reason: a `--rifts=4` run is a dungeon whose floor is going,
    /// not one room with a crack in it. Also like `traps`, it is what
    /// the room *asked* for rather than what it got — a rift that would
    /// have cut the board in two is put back, so a room with no space
    /// for four gets fewer.
    pub rifts: usize,
    /// Whether the water on each board is frozen over — SRD 5.2's
    /// slippery ice, and the fifth answer this struct's own docs
    /// predicted. See [`crate::engine::footing`].
    ///
    /// A flag rather than a density, and it is the one setting here that
    /// could not be one: the generator lays no ice of its own, so what
    /// `freeze_pools` freezes is however much water the room happened to
    /// roll. Asking for "six tiles of ice" would be asking the map a
    /// question it cannot answer, where asking for six traps is asking
    /// it for six of something it makes.
    ///
    /// Carried across the room boundary like the rest: a `--frozen` run
    /// is a dungeon in winter, not one cold room.
    pub frozen: bool,
}

impl BoardSettings {
    /// Put these settings onto a freshly generated board.
    ///
    /// Order is load-bearing and is the order `main` established. The
    /// light first, because nothing reads it back; the weather second,
    /// because `set_weather` puts open flames out and has to run after
    /// every carried torch has been lit (which instantiation does); the
    /// traps next, because `scatter_traps` walks the board looking for
    /// floor tiles nobody is standing on, and it wants the actors
    /// already placed so it does not arm the square the party spawns in;
    /// and the rifts last, for that reason and one more of their own —
    /// `carve_rifts` refuses any crack that leaves a creature unable to
    /// walk to another, and it cannot ask that question until it knows
    /// where the creatures are.
    pub fn apply(&self, encounter: &mut EncounterInstance) {
        encounter.set_ambient_light(self.ambient);
        encounter.set_weather(self.weather);
        // The freeze sits between the sky and the scatter, and both
        // sides of that are the rule. After the weather, because a
        // frozen board is what the cold *did* to the room rather than a
        // second thing the sky is doing. Before the traps and the
        // rifts, because both of those read the map they are laid on:
        // `scatter_traps` arms `Floor` tiles only, so freezing afterward
        // would have been the difference between a trap under the ice
        // and none.
        if self.frozen {
            encounter.freeze_pools();
        }
        encounter.scatter_traps(self.traps);
        encounter.carve_rifts(self.rifts);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The default is the game as it was before any of this existed.
    ///
    /// Pinned because it is what every caller that says nothing about
    /// the sky is relying on, and because a default that drifted would
    /// rewrite every encounter in the suite at once and silently.
    #[test]
    fn the_default_board_is_the_one_every_fight_was_always_fought_on() {
        let board = BoardSettings::default();
        assert_eq!(board.ambient, AmbientLight::BrightLight);
        assert_eq!(board.weather, Weather::Calm);
        assert_eq!(board.traps, 0);
        assert_eq!(board.rifts, 0);
        assert!(!board.frozen);
    }
}
