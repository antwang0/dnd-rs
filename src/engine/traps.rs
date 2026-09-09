//! **Traps** — SRD 5.2's *Example Traps*, or the half of them a fight
//! can walk into.
//!
//! The section prints eight. They share one shape and differ in six
//! numbers: something on the floor is triggered by the first creature
//! to stand on it, a saving throw is offered or it is not, damage or a
//! condition lands, and the thing is spent. That is a sentence the zone
//! layer has been able to say since Glyph of Warding arrived — see
//! `ZoneEffect::ward`, whose three clauses (invisible to the
//! pathfinder, triggered by somebody, spent when it fires) are the
//! whole of what a trap is.
//!
//! ## Why this belongs on the board rather than in the bestiary
//!
//! The map generator lays BSP rooms with one door punched in each wall,
//! and every fight on it is decided by which side of a doorway each
//! creature is standing on. A trapped doorway is the cheapest thing
//! that has ever made a *tile* worth thinking about: it costs the party
//! an Action to find, a detour to avoid, and a saving throw to ignore,
//! and it does all three without adding a body to the initiative order.
//!
//! ## What a trap is, mechanically
//!
//! A [`Trap`] is a row of numbers. `install` turns one into a zone with
//! `WardTrigger::Anyone`, which is the one thing that separates it from
//! a glyph: a glyph is set by somebody *for somebody else* and spares
//! its setter's side, and a trap was set by the dungeon and spares
//! nobody. That symmetry is most of what makes a trapped corridor a
//! decision rather than a hazard the monsters happen to be immune to.
//!
//! Finding one is the Search action's job (`default_actions::Search`),
//! against [`Trap::find_dc`]. A found trap stops being invisible to the
//! pathfinder — `Zone::deters_walkers` starts answering `true` — so the
//! whole payoff of searching is that everybody starts walking around
//! the thing. It does *not* stop being a trap: RAW's disarm is a rope,
//! an iron spike and ten minutes, none of which a six-second round has,
//! so a creature that walks onto a spotted plate still springs it.
//!
//! ## The two traps that are missing, and why
//!
//! **Fire-Casting Statue** and **Poisoned Darts** both print *"the trap
//! resets at the start of the next turn"*. The ward lane is
//! spent-when-triggered — `detonate_ward` removes the zone, which is
//! the lifecycle RAW's glyph asks for and the one clause of it a
//! resetting trap contradicts. A fifth ward clause would be the honest
//! way in, and it would need an answer for whether a reset trap is
//! re-hidden as well (RAW says the *trap* resets, not that the party
//! forgets where it is), which the book does not give.
//!
//! **Poisoned Needle** is in a lock and **Rolling Stone** travels; the
//! first has no tile and the second is a moving hazard with its own
//! geometry.
//!
//! ## The clauses that are dropped, named rather than lost
//!
//!   - **Collapsing Roof's rubble** — *"turns the trapped area into
//!     Difficult Terrain"*. A zone's `difficult` flag is read by
//!     `zone_movement_multiplier`, which does not care whether anybody
//!     can see the area; a roof that made its own tiles cost double
//!     before it fell would be a trap the pathfinder could feel through
//!     the floor.
//!   - **The Spiked Pit's fall damage** — RAW is 1d6 Bludgeoning *plus*
//!     2d8 Piercing, and a `ZoneContact` carries one damage instance.
//!     The spikes are the half the trap is named for and the half that
//!     separates it from the Hidden Pit, which ships its own 1d6 in
//!     full one row up.
//!   - **Every "At Higher Levels" table.** All eight scale by party
//!     level, and the engine has no party level — encounters are
//!     generated against a CR target that rises per fight. Shipping the
//!     level-1–4 column is shipping the printed trap.

use crate::conditions::{Condition, ConditionTimer};
use crate::engine::dice::Dice;
use crate::engine::types::{AbilityScoreType, Coordinate, DamageType, Size};
use crate::engine::zones::{Zone, ZoneContact, ZoneEffect, ZoneMotion, ZoneSave};

/// The `owner_id` a trap carries — nobody.
///
/// Every other zone on the layer is somebody's spell, and `owner_id` is
/// read for two things: tearing the area down when its caster's
/// concentration lapses, and making the save caster-aware (Heightened
/// Spell, a target's Magic Resistance against *this* caster). A trap
/// holds no concentration and was cast by nobody, so both readers want
/// the same answer — an id no actor has, which the caster-aware save
/// path already degrades on: with no caster to read riders off, it
/// falls through to a plain save, which is exactly what a pressure
/// plate deserves.
///
/// `usize::MAX` rather than 0, which is a perfectly ordinary actor id
/// and would have made the first creature in the initiative order the
/// author of every trap in the dungeon.
pub const NOBODY: usize = usize::MAX;

/// Rounds a trap sits armed. Longer than any fight, because a trap
/// waits: the round-end tick that expires an area is the wrong
/// lifecycle for one, and RAW's is "until it is triggered".
pub const TRAP_ROUNDS: u32 = 10_000;

/// One trap from SRD 5.2's *Example Traps*, in the shape the book
/// prints it.
///
/// Every field is a clause of the entry, so a row can be checked
/// against the page line by line.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Trap {
    /// The trap's name as printed, and what the log line calls it.
    pub name: &'static str,
    /// Chebyshev radius of the trapped area, in tiles. `0` is a single
    /// tile — the pit lid; `2` is the ten-foot square a falling net
    /// covers.
    pub radius: isize,
    /// The saving throw the trigger offers, or `None` for the traps
    /// that offer none. Both pits are `None`: RAW's lid *"swings open
    /// like a trapdoor, causing the creature to fall"*, with nothing to
    /// roll.
    pub save: Option<(AbilityScoreType, i32)>,
    /// What the trigger deals, or `None` for the net, whose entire
    /// effect is the hold.
    pub damage: Option<(Dice, DamageType)>,
    /// True when a made save halves the damage rather than dodging it.
    /// Meaningless without `save`.
    pub half_on_save: bool,
    /// What a failure installs, or `None`.
    pub condition: Option<(Condition, ConditionTimer)>,
    /// RAW's *"The target succeeds automatically if it's Huge or
    /// larger"* — the largest creature the rider can catch, or `None`
    /// for a clause the book leaves ungated.
    pub catches_at_most: Option<Size>,
    /// The Perception DC a Search action beats to find this. RAW's
    /// *"Detect and Disarm"* line.
    ///
    /// The pits print theirs as an Intelligence (Investigation) check
    /// rather than a Wisdom (Perception) one; the engine's Search
    /// action rolls Perception, and re-rolling the same act of looking
    /// at the floor under a second ability would be a second search
    /// action for one difference in flavour.
    pub find_dc: i32,
}

impl Trap {
    /// This trap's trigger as a zone contact clause.
    ///
    /// The conversion is total — every field above lands somewhere —
    /// which is the point of keeping the row in the book's shape rather
    /// than writing `ZoneEffect`s by hand at the declaration site: a
    /// trap that is one number different from another trap should be
    /// one number different here.
    pub fn contact(&self) -> ZoneContact {
        ZoneContact {
            save: self.save.map(|(ability, dc)| ZoneSave {
                ability,
                dc,
                half_on_success: self.half_on_save,
            }),
            damage: self.damage,
            condition: self.condition,
            breaks_concentration: false,
            catches_at_most: self.catches_at_most,
        }
    }
}

impl Trap {
    /// This trap as a zone, ready for `EncounterInstance::install_zone`.
    ///
    /// Armed rather than triggered: installing a zone never fires its
    /// contact clause (see `install_zone`), which for a trap is exactly
    /// right — a pressure plate under somebody who is already standing
    /// on it has not been stepped *onto*.
    pub fn zone_at(&self, origin: Coordinate) -> Zone {
        Zone {
            id: 0,
            name: self.name,
            owner_id: NOBODY,
            origin,
            radius: self.radius,
            effect: ZoneEffect::trap(self.contact(), self.find_dc),
            rounds_remaining: TRAP_ROUNDS,
            concentration: false,
            motion: ZoneMotion::Fixed,
            revealed: false,
        }
    }
}

/// **Collapsing Roof** — *Deadly Trap (Levels 1–4)*.
///
/// > The first creature that crosses the trip wire causes the supports
/// > to topple and the unstable section of ceiling to collapse. Each
/// > creature beneath the unstable section of ceiling must succeed on a
/// > DC 13 Dexterity saving throw, taking 11 (2d10) Bludgeoning damage
/// > on a failed save or half as much damage on a successful one.
///
/// The only trap in the set whose blast catches more than the creature
/// that sprang it, and the reason the ward lane's *"the tripwire is
/// enemies-only; the blast is not"* split matters here as much as it
/// does for a glyph: the whole point of a collapsing roof is that it
/// lands on whoever was standing under it.
///
/// Radius 2 is the ten-foot section on the 2.5-ft grid.
pub const COLLAPSING_ROOF: Trap = Trap {
    name: "collapsing roof",
    radius: 2,
    save: Some((AbilityScoreType::Dexterity, 13)),
    damage: Some((Dice::new(2, 10), DamageType::Bludgeoning)),
    half_on_save: true,
    condition: None,
    catches_at_most: None,
    find_dc: 11,
};

/// **Falling Net** — *Nuisance Trap (Levels 1–4)*.
///
/// > The target must succeed on a DC 10 Dexterity saving throw or have
/// > the Restrained condition until it escapes. The target succeeds
/// > automatically if it's Huge or larger.
///
/// The one trap in the set that deals no damage at all, and the one
/// that is worth the most: `Restrained` is zero movement, disadvantage
/// on everything the target swings, and advantage for everybody
/// swinging back. A nuisance to a party of four and very nearly a kill
/// on whoever is standing alone in the corridor.
///
/// RAW's escape is a DC 10 Strength (Athletics) check as an action; the
/// engine's nearest is the ten-round timer, which is longer than the
/// fight and therefore reads as "until it escapes" without an escape.
/// The size gate is RAW's and is what keeps the net off a giant.
pub const FALLING_NET: Trap = Trap {
    name: "falling net",
    radius: 2,
    save: Some((AbilityScoreType::Dexterity, 10)),
    damage: None,
    half_on_save: false,
    condition: Some((Condition::Restrained, ConditionTimer::Rounds(10))),
    catches_at_most: Some(Size::Large),
    find_dc: 11,
};

/// **Hidden Pit** — *Nuisance Trap (Levels 1–4)*.
///
/// > When a creature moves onto the lid, it swings open like a
/// > trapdoor, causing the creature to fall into the pit. … A creature
/// > that falls into the pit takes 3 (1d6) Bludgeoning damage from the
/// > fall.
///
/// No save, which is the whole of what makes a pit different from
/// everything else on this list: the only defence against one is
/// finding it. Radius 0 — a pit is a hole, and the tile it is in is the
/// tile it is in.
///
/// The hardest of the four to find (DC 15, RAW's), and the cheapest to
/// walk into. That pairing is the trap.
pub const HIDDEN_PIT: Trap = Trap {
    name: "hidden pit",
    radius: 0,
    save: None,
    damage: Some((Dice::new(1, 6), DamageType::Bludgeoning)),
    half_on_save: false,
    condition: None,
    catches_at_most: None,
    find_dc: 15,
};

/// **Spiked Pit** — *Deadly Trap (Levels 1–4)*.
///
/// > A creature that falls into the pit lands at the bottom and takes
/// > 3 (1d6) Bludgeoning damage from the fall plus 9 (2d8) Piercing
/// > damage from the spikes.
///
/// The Hidden Pit with something at the bottom. Ships the 2d8 and not
/// the 1d6 — see the module docs — because the spikes are the half the
/// trap is named for and the fall is printed in full one row up.
pub const SPIKED_PIT: Trap = Trap {
    name: "spiked pit",
    radius: 0,
    save: None,
    damage: Some((Dice::new(2, 8), DamageType::Piercing)),
    half_on_save: false,
    condition: None,
    catches_at_most: None,
    find_dc: 15,
};

/// The traps a generated dungeon draws from, in the order the book
/// prints them.
///
/// One list rather than a weighting, because the four are close enough
/// in cost that which one a corridor gets is more interesting than how
/// often: two of them are nuisances and two are deadly, and a party
/// that has just found a net has learned nothing about the next tile.
pub const ALL_TRAPS: [Trap; 4] = [COLLAPSING_ROOF, FALLING_NET, HIDDEN_PIT, SPIKED_PIT];

#[cfg(test)]
mod tests {
    use super::*;

    /// Every row converts to a contact clause that can actually cost a
    /// creature something.
    ///
    /// The sweep exists because the failure it catches is silent: a
    /// trap with no damage, no condition and no save is a tile that
    /// logs a trigger and does nothing, and nothing in the engine
    /// complains about one.
    #[test]
    fn every_trap_can_cost_a_creature_something() {
        for trap in ALL_TRAPS {
            assert!(
                trap.contact().is_harmful(),
                "{} triggers and does nothing",
                trap.name
            );
        }
    }

    /// A trap has to be findable and worth finding: a DC nobody could
    /// beat is a trap that is never searched for, and a DC everybody
    /// beats is one that is never sprung.
    #[test]
    fn every_trap_is_findable_within_the_range_a_search_can_roll() {
        for trap in ALL_TRAPS {
            assert!(
                (5..=20).contains(&trap.find_dc),
                "{} has a find DC of {}",
                trap.name,
                trap.find_dc
            );
        }
    }

    /// The pits are the no-save rows, and that is the design rather
    /// than an omission — the only defence against a pit is finding it,
    /// which is why both of them are the hardest things here to find.
    #[test]
    fn the_pits_offer_no_save_and_are_the_hardest_to_spot() {
        let easiest_with_a_save = ALL_TRAPS
            .iter()
            .filter(|t| t.save.is_some())
            .map(|t| t.find_dc)
            .max()
            .expect("some traps offer a save");
        for pit in [HIDDEN_PIT, SPIKED_PIT] {
            assert!(pit.save.is_none(), "{} should offer no save", pit.name);
            assert!(
                pit.find_dc > easiest_with_a_save,
                "{} is dodgeable *and* easy to spot",
                pit.name
            );
        }
    }

    /// The net's size gate is RAW's, and it is the clause that keeps a
    /// ten-foot square of rope off a giant.
    #[test]
    fn the_net_lets_a_huge_creature_walk_through_it() {
        let contact = FALLING_NET.contact();
        assert_eq!(contact.catches_at_most, Some(Size::Large));
        assert!(Size::Medium.clears_gate(contact.catches_at_most));
        assert!(Size::Large.clears_gate(contact.catches_at_most));
        assert!(!Size::Huge.clears_gate(contact.catches_at_most));
    }
}
