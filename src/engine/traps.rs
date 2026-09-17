//! **Traps** — SRD 5.2's *Example Traps*, or the half of them a fight
//! can walk into.
//!
//! The section prints eight. They share one shape and differ in six
//! numbers: something on the floor is triggered by the first creature
//! to stand on it, a saving throw is offered or it is not, and damage
//! or a condition lands. That is a sentence the zone layer has been
//! able to say since Glyph of Warding arrived — see `ZoneEffect::ward`,
//! whose clauses (invisible to the pathfinder, triggered by somebody,
//! spent when it fires) are the whole of what a trap is.
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
//! ## The trap that resets
//!
//! **Fire-Casting Statue** and **Poisoned Darts** both print *"the trap
//! resets at the start of the next turn"*, and for a long time neither
//! shipped: the ward lane was spent-when-triggered — `detonate_ward`
//! removed the zone, which is the lifecycle RAW's glyph asks for and
//! the one clause of it a resetting trap contradicts.
//!
//! [`ZoneEffect::rearms`] is that clause, and the question it was
//! blocked on turns out to have an answer the book does give, in the
//! other direction. RAW says the *trap* resets; it does not say the
//! party forgets where it is, and every disarm in the section is
//! written for somebody who has already found the thing. So a sprung
//! trap stays `revealed` and rearms anyway, which is what makes the two
//! of them interesting rather than merely repeated: the pathfinder
//! routes around a plate everybody can see, and the fight is about
//! whether the corridor it blocks is worth the detour.
//!
//! The reset is *at the start of the next turn* and the engine's is
//! exactly that: `EncounterInstance::start_turn_for` clears the
//! per-turn spent ledger beside the one `zone_contacts_this_turn`
//! already uses, so a trap that goes off under the fighter is inert for
//! the rest of the fighter's turn and armed again when the rogue's
//! begins.
//!
//! ## The two traps that are still missing, and why
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
    /// True for the one trap in the set the book calls magic — the
    /// Fire-Casting Statue, whose *Detect and Disarm* entry opens *"A
    /// Detect Magic spell reveals an aura of Evocation magic around the
    /// statue."*
    ///
    /// The column that makes the Detect Magic spell's line honest. That
    /// spell finds what a caster wrote and not what the dungeon built,
    /// and the reason it can draw the line at all is `owner_id`: every
    /// area is either somebody's spell or the dungeon's, and only the
    /// first kind has a caster. The statue is the entry that breaks the
    /// inference — carpentry with a glyph on it — and the sentence
    /// above is RAW telling us so outright. See
    /// `crate::engine::zones::WardTrigger::is_magical`.
    pub magical: bool,
    /// RAW's Duration line, for the two entries whose is not
    /// *"Instantaneous"* on its own: *"and the trap resets at the start
    /// of the next turn"*.
    ///
    /// `None` is a trap that is spent by going off, which is four of
    /// the six and every other ward on the layer. `Some(None)` resets
    /// forever — the statue, which the book gives no budget. `Some(n)`
    /// is the darts' *"if it has activated fewer than three times"*,
    /// counted in springs after the first.
    pub rearms: Option<Option<u32>>,
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
            catches_at_most: self.catches_at_most,
            // A trap is the dungeon's, not a caster's: it was not
            // written against a kind of creature and it springs for
            // whoever stands on it. See `ZoneContact::only_types`.
            ..ZoneContact::DEFAULTS
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
            effect: ZoneEffect::trap(self.contact(), self.find_dc, self.magical, self.rearms),
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
    magical: false,
    rearms: None,
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
    magical: false,
    rearms: None,
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
    magical: false,
    rearms: None,
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
    magical: false,
    rearms: None,
};

/// **Fire-Casting Statue** — *Deadly Trap (Levels 1–4)*.
///
/// > When a creature moves onto this trap's pressure plate for the
/// > first time on a turn or starts its turn there, a nearby statue
/// > exhales a 15-foot Cone of magical flame. Each creature in the Cone
/// > must succeed on a DC 15 Dexterity saving throw, taking 11 (2d10)
/// > Fire damage on a failed save or half as much damage on a
/// > successful one.
/// >
/// > *Duration:* Instantaneous, and the trap resets at the start of the
/// > next turn.
///
/// The set's one piece of **magical** hardware, and the entry that
/// brought the whole rearming lane with it. Both of its late columns
/// are RAW's own lines: *Duration* says it resets, and *Detect and
/// Disarm* opens *"A Detect Magic spell reveals an aura of Evocation
/// magic around the statue"* — which is the book confirming the line
/// the Detect Magic spell already drew, from the other side. Every
/// other trap in the dungeon is carpentry and stays the Search
/// action's to find; this one lights up for a level-1 divination at
/// thirty feet, with no roll, and that is exactly what the two hundred
/// gold of spell slot is for.
///
/// **No budget.** Alone among the six it resets forever, because the
/// book gives it no limit: a party that cannot find the plate and will
/// not go round pays 2d10 every time somebody steps on it, for as long
/// as the fight lasts. That is the trap.
///
/// **The cone is a disc.** RAW's fifteen-foot Cone is a direction and
/// this layer has no directions — a ward is a centre and a radius. Set
/// at 3, which on the 2.5-ft grid is a seven-and-a-half-foot half-width
/// and puts about as many tiles under the flame as the cone would
/// cover, rather than at the 6 a fifteen-foot *radius* would be and
/// which would make the statue four times the trap the book prints.
pub const FIRE_CASTING_STATUE: Trap = Trap {
    name: "fire-casting statue",
    radius: 3,
    save: Some((AbilityScoreType::Dexterity, 15)),
    damage: Some((Dice::new(2, 10), DamageType::Fire)),
    half_on_save: true,
    condition: None,
    catches_at_most: None,
    find_dc: 15,
    magical: true,
    rearms: Some(None),
};

/// **Poisoned Darts** — *Deadly Trap (Levels 1–4)*.
///
/// > Each creature in the darts' path must succeed on a DC 13 Dexterity
/// > saving throw or be struck by 1d3 darts, taking 3 (1d6) Poison
/// > damage per dart.
/// >
/// > *Duration:* Instantaneous, and the trap resets at the start of the
/// > next turn if it has activated fewer than three times.
///
/// The other resetting entry, and the one with a budget: three springs
/// and the tubes are empty, which makes a corridor with darts in it a
/// resource the party can spend rather than a wall they cannot pass.
/// Carpentry, so a Detect Magic walks straight past it and only the
/// Search action finds it — at DC 15, the hardest in the set alongside
/// the pits.
///
/// **`2d6` for `1d3` darts of `1d6` each.** A `ZoneContact` rolls one
/// damage instance and RAW's is a count of dice rolled for a count of
/// darts. The two have the same mean — two darts on average, three and
/// a half each, seven either way — and the printed version has the
/// wider tail in both directions. Worth naming rather than hiding: a
/// creature that eats all three darts should have a worse round than
/// `2d6` can give it.
///
/// **No save-for-half.** RAW's save is pass-or-be-struck, unlike every
/// other saving throw in the set, so a creature that makes it takes
/// nothing at all.
pub const POISONED_DARTS: Trap = Trap {
    name: "poisoned darts",
    radius: 1,
    save: Some((AbilityScoreType::Dexterity, 13)),
    damage: Some((Dice::new(2, 6), DamageType::Poison)),
    half_on_save: false,
    condition: None,
    catches_at_most: None,
    find_dc: 15,
    magical: false,
    rearms: Some(Some(2)),
};

/// The traps a generated dungeon draws from, in the order the book
/// prints them.
///
/// One list rather than a weighting, because the six are close enough
/// in cost that which one a corridor gets is more interesting than how
/// often: two of them are nuisances and four are deadly, and a party
/// that has just found a net has learned nothing about the next tile.
pub const ALL_TRAPS: [Trap; 6] = [
    COLLAPSING_ROOF,
    FALLING_NET,
    FIRE_CASTING_STATUE,
    HIDDEN_PIT,
    POISONED_DARTS,
    SPIKED_PIT,
];

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
    /// which is why both of them are as hard to find as anything here.
    ///
    /// *As hard as*, not harder, and the weaker claim is the one the
    /// book supports. It read `>` while the set was four traps and the
    /// two pits were alone at DC 15; the two that print *"Duration:
    /// Instantaneous, and the trap resets"* print 15 as well, so the
    /// ceiling is now shared four ways. The interesting half of the
    /// assertion survives either way: nothing dodgeable is *harder* to
    /// spot than a hole in the floor you cannot dodge.
    #[test]
    fn the_pits_offer_no_save_and_are_the_hardest_to_spot() {
        let hardest_with_a_save = ALL_TRAPS
            .iter()
            .filter(|t| t.save.is_some())
            .map(|t| t.find_dc)
            .max()
            .expect("some traps offer a save");
        for pit in [HIDDEN_PIT, SPIKED_PIT] {
            assert!(pit.save.is_none(), "{} should offer no save", pit.name);
            assert!(
                pit.find_dc >= hardest_with_a_save,
                "{} is dodgeable *and* easier to spot than something that is not",
                pit.name
            );
        }
    }

    /// A trap that resets is a trap with a Duration line that says so,
    /// and the two clauses that ride on it never travel alone.
    ///
    /// Three things are being pinned, and all three are silent
    /// failures. A `magical` trap nothing can sense is a column that
    /// does nothing; a budget on a trap that does not reset is a number
    /// nothing reads; and a resetting trap with a budget of zero is a
    /// one-shot trap spelled the long way.
    #[test]
    fn only_a_resetting_trap_carries_a_budget() {
        let mut resetting = 0;
        for trap in ALL_TRAPS {
            match trap.rearms {
                None => {}
                Some(budget) => {
                    resetting += 1;
                    assert!(
                        budget != Some(0),
                        "{} resets and has no springs to do it with",
                        trap.name
                    );
                }
            }
        }
        assert_eq!(resetting, 2, "the book prints two, and they are the two");
        assert_eq!(
            ALL_TRAPS.iter().filter(|t| t.magical).count(),
            1,
            "and one of them is the only magic in the dungeon's own hardware"
        );
        assert!(
            FIRE_CASTING_STATUE.magical && FIRE_CASTING_STATUE.rearms == Some(None),
            "the statue is the magical one and the one with no limit"
        );
        assert!(
            !POISONED_DARTS.magical && POISONED_DARTS.rearms.is_some(),
            "the darts reset and are carpentry"
        );
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
