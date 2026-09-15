//! A weapon that is somewhere else — the **hovering blade** layer.
//!
//! Three things in fifth edition put a blade in the air and then leave
//! it there: Spiritual Weapon, Arcane Sword, and the Dancing Sword a
//! party finds in a chest. They are the same object with three price
//! tags, and RAW says so in nearly the same words each time:
//!
//! > You create a floating, spectral force that resembles a weapon of
//! > your choice […] The force appears within range in a space of your
//! > choice, and you can immediately make one melee spell attack
//! > against one creature within 5 feet of the force. […] As a Bonus
//! > Action on your later turns, you can move the force up to 20 feet
//! > and repeat the attack against a creature within 5 feet of it.
//!
//! The engine had none of that. Both spells shipped as approximations
//! that threw the *place* away and kept only the damage, and both said
//! so in their own docstrings — Spiritual Weapon's *"we collapse the
//! floating-weapon range to melee reach since we don't yet model
//! summoned terrain"*, Arcane Sword's *"the bonus-action recurring
//! attack pattern is awkward in this engine's one-action-per-spell
//! model, so we collapse to a heavier on-cast hit"*. What that cost was
//! the whole of the spell: a cleric had to stand next to whatever the
//! spectral mace was hitting, which is the one thing a cleric casts it
//! in order not to do.
//!
//! ## Why a layer of its own
//!
//! The engine has three board layers already, and the blade is not any
//! of them.
//!
//!   - A **zone** (`crate::engine::zones`) overlays clauses onto ground
//!     that is otherwise unchanged: a web makes its tiles difficult, a
//!     fog cloud makes them blind. A blade changes nothing about the
//!     tile it hovers over. It has a position and no area, and every
//!     question the zone layer is built to answer — *is this tile bad
//!     ground, does anyone standing here take damage, may the
//!     pathfinder cross it* — has the same answer for a blade as for
//!     empty air.
//!   - **Conjured terrain** (`crate::engine::conjured_terrain`)
//!     *replaces* the map, which is how a wall of stone stops a line of
//!     sight. Nothing about a blade is solid: it does not block a step,
//!     a shot, or a look.
//!   - An **actor** would be the closest fit and is the wrong one for
//!     the reason `creatures::faithful_hounds` names from the other
//!     side: every occupant of a tile in this engine is a target, and
//!     RAW's floating force is not a creature, has no hit points, and
//!     cannot be attacked. A blade modeled as an actor would be a
//!     second body the enemy line could spend its turns killing, which
//!     is a different spell.
//!
//! So this is a fourth layer, and a deliberately small one: a point, an
//! owner, two counters. It is a sibling of the other three down to the
//! lifecycle — owned, expiring on a round-end tick, torn down by
//! `drop_concentration` when the caster's grip fails.
//!
//! ## The leash is the spell
//!
//! Everything interesting about a hovering blade is the arithmetic of
//! where it can get to. On the cast it may be placed anywhere inside
//! the spell's range, which is how a cleric reaches the back rank
//! without walking there. On every later turn it may travel `step` and
//! no further, *measured from where it is* — so a blade parked on the
//! ogre in the doorway is a blade that cannot also answer the archer
//! behind it, and the decision the spell asks every round is whether
//! this target is worth walking away from that one.
//!
//! A blade that is not tracked between turns has no such decision in
//! it: it would be a 60-foot-range bonus-action attack that hits
//! whatever the caster likes, which is strictly better than the spell
//! on the page. The position *is* the rule.
//!
//! ## What the caller never has to decide
//!
//! The blade's tile is derived rather than asked for. A player who
//! names a target has said everything they meant to say, and
//! `EncounterInstance::blade_strike_anchor` works out the rest: the
//! closest unoccupied tile next to that target which the blade can
//! reach from where it stands, or `None` when there isn't one — which
//! is also the whole of the spell's validation, for the prompt and for
//! the AI alike. Nothing outside this module needs to know that the
//! blade has coordinates.

use crate::engine::dice::Dice;
use crate::engine::encounter::EncounterInstance;
use crate::engine::side_effects::Resource;
use crate::engine::types::{Coordinate, DamageType};

/// The unchanging half of a hovering blade: what the spell or the item
/// that conjures it says about it.
///
/// A `const` beside each of the three callers, rather than fields
/// spelled out at the conjuring site, because every number here is read
/// twice — once when the blade is placed and once, rounds later, when
/// its owner asks whether a target is inside the leash — and the two
/// reads have to agree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BladeProfile {
    /// The ledger key, and the name the log and the side panel print.
    ///
    /// Identity is `(owner, name)` for the same reason a steered zone's
    /// is: an `Action` is a zero-sized static with nowhere to keep an
    /// id, so the only way for Spiritual Weapon cast a second time to
    /// recognise its own first blade is to ask for it by name.
    pub name: &'static str,
    /// What the blade draws itself as on the map.
    pub glyph: char,
    /// How far from the caster's own body the blade may be placed on
    /// the cast that conjures it — the spell's printed Range, in tiles.
    pub cast_reach: isize,
    /// How far it may travel on each later swing, in tiles, measured
    /// from where it is rather than from its owner. RAW's *"move the
    /// force up to 20 feet"*.
    pub step: isize,
    /// Rounds it lasts if nothing ends it early. One minute is ten.
    pub rounds: u32,
    /// True when the owner's concentration holds it up, in which case
    /// `drop_concentration` takes it down with everything else.
    pub concentration: bool,
    /// How many swings the blade has in it, or `None` for one that
    /// swings until its timer runs out.
    ///
    /// The one field the two spells and the item disagree on. Both
    /// spells last for their duration and swing every round of it; the
    /// Dancing Sword is counted instead — *"after the hovering weapon
    /// attacks for the fourth time, it flies back to you"* — and the
    /// count is the item's whole balance.
    pub swings: Option<u32>,
    /// What one swing rolls, at the level the thing is printed at.
    ///
    /// The *printed* dice, which is not always what the blade in the
    /// air rolls: Spiritual Weapon gains a d8 per slot level above the
    /// second, and the slot is only spent on the cast. So the resolved
    /// number is handed to `conjure` and remembered on the blade, and
    /// this field is only ever the starting point.
    pub dice: Dice,
    /// What the swing deals. Force for both spells, and whatever the
    /// steel was for an item that tossed a real weapon into the air.
    pub damage_type: DamageType,
}

impl BladeProfile {
    /// The blade this profile puts at `origin`, owned by `owner_id`,
    /// rolling `dice` a swing.
    ///
    /// `dice` is a parameter rather than a read of `self.dice` because
    /// the upcast is decided at the cast and has to survive it: the
    /// blade swings on later turns for free, with no slot in hand to
    /// re-derive the number from. A caller with nothing to add passes
    /// `profile.dice` straight back.
    ///
    /// `id` is `0` here and overwritten by
    /// `EncounterInstance::conjure_blade`, the same contract
    /// `install_zone` and `conjure_terrain` have.
    pub fn conjure(&self, owner_id: usize, origin: Coordinate, dice: Dice) -> HoveringBlade {
        HoveringBlade {
            id: 0,
            name: self.name,
            glyph: self.glyph,
            owner_id,
            origin,
            step: self.step,
            rounds_remaining: self.rounds,
            concentration: self.concentration,
            swings_remaining: self.swings,
            dice,
            damage_type: self.damage_type,
        }
    }
}

/// One blade in the air.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HoveringBlade {
    /// Unique per encounter, handed out by
    /// `EncounterInstance::conjure_blade`.
    pub id: usize,
    pub name: &'static str,
    pub glyph: char,
    /// Whoever is swinging it. Used for the ledger lookup, for the
    /// concentration teardown, and for the one rule that outlives
    /// both — a blade whose owner has stopped being on the board falls
    /// out of the air.
    pub owner_id: usize,
    /// The tile it hovers over. A single point: a blade has no
    /// footprint, stamps nothing on the occupancy grid, and two of them
    /// may share a tile.
    pub origin: Coordinate,
    /// Copied off the profile so the leash can be read off the blade —
    /// the same reason `ZoneMotion::Directed` carries its own distance
    /// rather than making each spell restate it.
    pub step: isize,
    pub rounds_remaining: u32,
    pub concentration: bool,
    /// Swings left, or `None` for a blade nobody is counting. See
    /// `BladeProfile::swings`.
    pub swings_remaining: Option<u32>,
    /// What this blade rolls on a hit, resolved once at the conjuring
    /// and remembered — see `BladeProfile::conjure`.
    pub dice: Dice,
    pub damage_type: DamageType,
}

impl HoveringBlade {
    /// True if this blade could reach `tile` on its next swing.
    ///
    /// Measured from where the blade *is*, which is the clause RAW
    /// writes: a sword walked halfway across the room may travel its
    /// thirty feet from there and not from the wizard.
    pub fn can_reach(&self, tile: Coordinate) -> bool {
        self.origin.chebyshev_to(tile) <= self.step
    }
}

/// The cost half of the blade idiom: `repeat` while this owner's own
/// blade is in the air, `first_cast` otherwise.
///
/// The twin of `spells::steered_zone_cost`, and split from the
/// resolution for the same reason that one is: `cost` is asked before a
/// target is known — the UI prices an action to decide whether to offer
/// it — so the only question it can answer is "is the blade up".
///
/// This is what makes the action economy come out right without a
/// second entry on anybody's action list. A cleric's list carries one
/// "spiritual weapon", and whether naming it costs a level-2 slot or
/// nothing at all depends entirely on whether there is already a mace
/// in the air.
pub fn blade_cost(
    encounter: &EncounterInstance,
    owner_id: usize,
    profile: &BladeProfile,
    repeat: Vec<Resource>,
    first_cast: Vec<Resource>,
) -> Vec<Resource> {
    if encounter.blade_sustained_by(owner_id, profile.name).is_some() {
        repeat
    } else {
        first_cast
    }
}

/// The validation half: a blade may only be swung at something it can
/// actually get next to, and a first cast of a concentration blade has
/// to be one its caster can hold onto.
///
/// The concentration clause is gated on the profile rather than asked
/// unconditionally, because one of the three blades does not need it:
/// a Dancing Sword is tossed rather than cast, and a wizard already
/// holding a Wall of Force may still throw one.
pub fn blade_validate(
    encounter: &EncounterInstance,
    owner_id: usize,
    profile: &BladeProfile,
    target_ids: Option<&Vec<usize>>,
) -> bool {
    let Some(&target_id) = target_ids.and_then(|ids| ids.first()) else {
        return false;
    };
    if encounter.blade_sustained_by(owner_id, profile.name).is_none()
        && profile.concentration
        && !encounter.caster_can_concentrate(owner_id)
    {
        return false;
    }
    encounter
        .blade_strike_anchor(owner_id, profile, target_id)
        .is_some()
}
