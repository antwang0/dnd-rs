//! Images the map is not really wearing — the **illusion** layer.
//!
//! 5e's image spells are the one family whose whole effect is a
//! disagreement about what is on the board. Minor Illusion, Silent
//! Image and Major Image do no damage, impose no condition, and change
//! nothing about the floor; what they change is what a creature
//! *thinks* is there, and every RAW clause they print is about that
//! gap: *"the image can't cause damage or any other physical effect"*,
//! *"physical interaction with the image reveals it to be an illusion,
//! because things can pass through it"*, *"a creature that uses its
//! Study action to examine the image can determine that it is an
//! illusion with a successful Intelligence (Investigation) check
//! against your spell save DC"*.
//!
//! The engine had no way to say any of that. Every other layer on the
//! board is objective — a wall is a wall for everybody, a web catches
//! whoever steps in it, a fog bank blinds both sides equally — and an
//! illusion is the first thing in the game that is only true for some
//! of the creatures looking at it.
//!
//! ## Why not conjured terrain
//!
//! A Wall of Stone and a Silent Image of a wall look identical on the
//! map and are opposites in the rules. Conjured terrain *replaces the
//! map*: it blocks the pathfinder because the tile genuinely is not
//! passable, it blocks the line-of-sight walk because the tile
//! genuinely is opaque, and there is one answer for every creature
//! because the map has one state. Writing an illusion that way would
//! make it a wall — the one thing RAW is at pains to say it is not, and
//! the caster would have bought a fifth-level effect with a cantrip.
//!
//! So the tiles stay exactly as they were, and what the layer carries
//! instead is a **disbelief ledger**: [`Illusion::disbelieved`], the set
//! of actors who know. Every question the layer answers is asked of a
//! particular creature, and there is no creature-free answer to any of
//! them. That is the whole shape of the module, and it is why the three
//! consumers ([`crate::engine::encounter::EncounterInstance`]'s sight
//! gate, its pathfinder, and the Study action) all take an actor id.
//!
//! ## Who believes what
//!
//! Four ways to be out of the ledger's reach, resolved by
//! `EncounterInstance::believes_illusion`:
//!
//!   - **You cast it, or you are on the side that did.** RAW excuses
//!     only the caster. The engine excuses the caster's whole team,
//!     because the alternative is a wizard's own fighter refusing to
//!     charge through the screen the wizard just put up to hide the
//!     charge — a party that watched the spell being cast is not fooled
//!     by it, and modelling the difference would cost the spell its
//!     only friendly use.
//!   - **Truesight.** RAW: *"a creature with Truesight … automatically
//!     detects visual illusions"* — no check, no action.
//!   - **A sense that is not sight.** Blindsight is *"perceiving without
//!     relying on sight"*, which is exactly the sense an image of a
//!     boulder has nothing to say to. Read through the same
//!     `nonvisual_sense_reaches` cohort the fog layer uses, so a bat is
//!     as unimpressed by a Silent Image as it is by a Fog Cloud.
//!   - **You touched it.** The RAW reveal clause, charged at the one
//!     chokepoint that knows a creature has arrived somewhere — see
//!     `EncounterInstance::touch_ground`.
//!
//! Everybody else believes it until they spend an Action to Study it.
//!
//! ## What believing costs
//!
//! Two things, and deliberately only two.
//!
//!   - **You will not walk into it.** The pathfinder refuses a step
//!     into a believed image's tiles for the creature that believes it,
//!     through `EncounterInstance::illusion_bars_step` — the same gate
//!     Magic Circle's ward rides, and for the same reason: a creature
//!     that thinks there is a boulder in the corridor routes around the
//!     boulder instead of walking up to it and stopping. The step is
//!     refused as a *choice*, not as a rule: nothing stops a shove, a
//!     teleport or a player who insists from putting a body in those
//!     tiles, and the moment one lands there the image is revealed to
//!     it.
//!   - **You cannot see through it.** An image of a slab of rock is an
//!     opaque thing standing in the way; a believer's sight stops at it
//!     exactly as it would stop at the rock. Folded into
//!     `EncounterInstance::sight_denied_between`, beside the fog and
//!     the dark, so it reaches the two places that gate already
//!     reaches: `viewer_can_see` (every *"a creature you can see"*
//!     reaction) and the attack-mode sweep (an archer shooting into a
//!     screen it believes in shoots at disadvantage).
//!
//! **Not** targeting. A spell's `requires_los` gate runs through
//! `actor_has_line_of_sight`, which is a fact about terrain, symmetric,
//! and asked of no viewer — there is no creature in that call to have a
//! belief. Teaching it one would mean threading a believer through
//! every targeting path in the engine to buy a rule the disadvantage
//! above already approximates, so the boundary is drawn here and named
//! rather than left to be discovered.
//!
//! ## What deliberately isn't here
//!
//! **Sound.** Minor Illusion's *"or a sound"* mode and Major Image's
//! audible component have nothing to act on: the engine has no hearing
//! layer, so an illusory noise would be a log line with no consumer.
//! The object mode is the whole of what is modelled, and
//! [`Illusion::guise`] is what it says the object is.
//!
//! **A re-arming trigger.** A [`Illusion::armed`] image that has fired
//! is an ordinary image from then on. RAW's Programmed Illusion goes
//! dormant again after ten minutes, which is a hundred rounds — longer
//! than any fight this engine runs — so the field that would carry it
//! is a field nothing could read.
//!
//! **Illusory creatures.** An image shaped like an ogre would need to
//! be in the actor table to be attacked, and an entry in the actor
//! table is a creature — with hit points, a turn, and a place in
//! initiative — which is the opposite of what the spell makes. The
//! tractable half of that rule is already here: a believer will not
//! walk through it, and an attack aimed past it is made blind.

use std::collections::HashSet;

use crate::engine::types::Coordinate;

/// One image standing on the board, plus the set of creatures that have
/// stopped falling for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Illusion {
    /// Unique per encounter, handed out by
    /// `EncounterInstance::install_illusion`. Callers building one pass
    /// `0` and let the encounter overwrite it — the same contract
    /// `install_zone` and `conjure_terrain` have.
    pub id: usize,
    /// The spell that cast it, for the log and the panel.
    pub name: &'static str,
    /// What the image is *of* — "a slab of rock", "a wall of briars".
    ///
    /// Separate from [`Self::name`] because the two are read by
    /// different audiences. A creature standing in front of one sees a
    /// boulder and not a Minor Illusion, so the reveal line is written
    /// in terms of the guise ("the slab of rock was never there"); the
    /// panel, which is the caster's own ledger, is written in terms of
    /// the spell.
    pub guise: &'static str,
    /// The caster. Drives the concentration teardown, the log line, and
    /// the side of the board that is never fooled.
    pub owner_id: usize,
    /// The tiles the image stands on. Unlike conjured terrain this is
    /// the whole truth about where it is: nothing was taken from the
    /// map, so there is no ledger of what was there before.
    pub tiles: Vec<Coordinate>,
    /// The Intelligence (Investigation) DC to see through it — the
    /// caster's spell save DC, snapshotted at install so a later change
    /// to the caster's stats cannot retroactively make a standing image
    /// harder to see through.
    pub save_dc: i32,
    /// Rounds left, decremented at each round end — the same clock the
    /// zone and terrain layers run on.
    pub rounds_remaining: u32,
    /// True if the owner's concentration holds it up.
    pub concentration: bool,
    /// The radius, in tiles, inside which something has to happen
    /// before this image exists at all — or `None` for an image that is
    /// simply there.
    ///
    /// SRD 5.2's Programmed Illusion: *"the illusion is imperceptible
    /// until then"*, and imperceptible is not a weak kind of visible.
    /// A dormant image is believed by nobody, blocks nobody's sight,
    /// stops nobody's feet, cannot be walked into, cannot be studied,
    /// and does not spend its clock. It is a *trigger* that happens to
    /// know what it will draw.
    ///
    /// That makes it the one field on this struct that every consumer
    /// has to ask about, and the reason it is a radius rather than a
    /// bool: what springs it is a hostile creature arriving inside the
    /// envelope, which is RAW's *"visual or audible phenomena that
    /// occur within 30 feet of the area"* read as the only phenomenon
    /// this engine has a chokepoint for. Cleared to `None` the moment
    /// it fires, which is what makes the image ordinary from then on.
    ///
    /// Not modelled: the ten minutes RAW gives it to go dormant again
    /// afterwards. Ten minutes is a hundred rounds and no fight in this
    /// engine lasts that long, so a re-arm would be a field nothing
    /// could ever read.
    pub armed: Option<isize>,
    /// Everyone who knows. Ids rather than teams, because disbelief is
    /// earned one creature at a time: the goblin that walked into the
    /// boulder knows, and the goblin behind it does not.
    ///
    /// Holds only the creatures that *learned* — the four standing
    /// exemptions in the module header (the owner's team, truesight, a
    /// nonvisual sense, and contact-in-progress) are asked fresh by
    /// `EncounterInstance::believes_illusion` rather than written in
    /// here. A truesight creature that walks onto the board after the
    /// image went up is not fooled by it, and a ledger written at
    /// install could not know that.
    pub disbelieved: HashSet<usize>,
}

impl Illusion {
    /// An image that hasn't been installed yet: no id, nobody wise to
    /// it.
    pub fn new(
        name: &'static str,
        guise: &'static str,
        owner_id: usize,
        tiles: Vec<Coordinate>,
        save_dc: i32,
        rounds_remaining: u32,
        concentration: bool,
    ) -> Self {
        Self {
            id: 0,
            name,
            guise,
            owner_id,
            tiles,
            save_dc,
            rounds_remaining,
            concentration,
            armed: None,
            disbelieved: HashSet::new(),
        }
    }

    /// Declare that this image is a trigger rather than a picture until
    /// something hostile comes within `radius` tiles of it.
    ///
    /// A builder rather than a constructor argument, for the reason
    /// `ConjuredTerrain::breakable` is one: every image in the engine
    /// predates this and none of them should have to say it is already
    /// visible.
    pub fn armed_within(mut self, radius: isize) -> Self {
        self.armed = Some(radius);
        self
    }

    /// True while this image is still waiting for its trigger — see
    /// [`Self::armed`].
    pub fn is_dormant(&self) -> bool {
        self.armed.is_some()
    }

    /// True if the image stands on `coord`.
    pub fn covers(&self, coord: Coordinate) -> bool {
        self.tiles.contains(&coord)
    }

    /// True if `actor_id` has already learned this one is fake.
    ///
    /// The ledger read *only* — the standing exemptions live on
    /// `EncounterInstance::believes_illusion`, which is what every
    /// consumer should be asking. Public because the panel and the
    /// tests want the narrow question: "has this creature been told?"
    pub fn is_known_to(&self, actor_id: usize) -> bool {
        self.disbelieved.contains(&actor_id)
    }

    /// Write `actor_id` into the ledger. Returns true if this is news —
    /// which is what keeps a reveal from logging twice.
    pub fn disbelieve(&mut self, actor_id: usize) -> bool {
        self.disbelieved.insert(actor_id)
    }

    /// The image's anchor tile, for the log and the panel.
    ///
    /// The first tile rather than a centroid: the tile lists the two
    /// shape helpers produce are built outward from the point the
    /// caster aimed at, so any single tile is as good a label as
    /// another, and the first one is the only one that exists for an
    /// image of a single tile.
    pub fn origin(&self) -> Coordinate {
        self.tiles.first().copied().unwrap_or(Coordinate::new(0, 0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn an_image() -> Illusion {
        Illusion::new(
            "silent image",
            "a slab of rock",
            7,
            vec![Coordinate::new(4, 4), Coordinate::new(4, 5)],
            15,
            10,
            true,
        )
    }

    #[test]
    fn a_fresh_image_is_believed_by_everyone() {
        let image = an_image();
        assert!(!image.is_known_to(1));
        assert!(!image.is_known_to(image.owner_id));
    }

    #[test]
    fn disbelief_is_news_once_and_then_isnt() {
        let mut image = an_image();
        assert!(image.disbelieve(1));
        assert!(!image.disbelieve(1));
        assert!(image.is_known_to(1));
        assert!(!image.is_known_to(2));
    }

    #[test]
    fn coverage_is_the_tile_list_and_nothing_either_side_of_it() {
        let image = an_image();
        assert!(image.covers(Coordinate::new(4, 4)));
        assert!(image.covers(Coordinate::new(4, 5)));
        assert!(!image.covers(Coordinate::new(4, 6)));
        assert!(!image.covers(Coordinate::new(3, 4)));
    }

    /// The label the log and the panel print is a tile the image is
    /// really standing on, including for the one-tile case where a
    /// centroid would have been the same tile by luck rather than by
    /// rule.
    #[test]
    fn the_origin_is_one_of_the_tiles_the_image_stands_on() {
        let image = an_image();
        assert!(image.covers(image.origin()));
        let single = Illusion::new(
            "minor illusion",
            "a boulder",
            1,
            vec![Coordinate::new(9, 2)],
            13,
            10,
            false,
        );
        assert_eq!(single.origin(), Coordinate::new(9, 2));
    }
}
