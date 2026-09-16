//! **The Ring of Elemental Command** — SRD 5.2's four legendary rings,
//! and the one item on the table whose whole point is *who you are
//! fighting*.
//!
//! Everything else the loot table grants is a fact about its holder. A
//! Ring of Fire Resistance halves fire, a Belt of Giant Strength sets a
//! score, a Cloak of Protection adds a number — and every one of them
//! answers the same way whoever is standing opposite. This ring does
//! not:
//!
//! > **Elemental Bane.** *While wearing the ring, you have Advantage on
//! > attack rolls against Elementals and they have Disadvantage on
//! > attack rolls against you.*
//!
//! The identical ring is a legendary advantage against a fire elemental
//! and nothing at all against the goblin beside it. That sentence is
//! why `Item::attack_advantage_against` and
//! `Item::taxes_attackers_of_type` exist — see either for the lane, and
//! `EncounterInstance::attack_mode_tally` for where the two halves are
//! read.
//!
//! ## The four clauses, and where each one lands
//!
//! 1. **Elemental Bane** — the two `Item` slices above, on the attacker
//!    and target sides of one sweep. Both halves, because RAW prints
//!    both and they are not the same rule: they land on two different
//!    rolls and so never cancel.
//! 2. **Elemental Compulsion** — a [`SingleSaveConditionItem`] with the
//!    `target_types` gate this file motivated, at RAW's flat DC 18 and
//!    RAW's sixty feet. Free: the clause prints no charge.
//! 3. **Elemental Focus** — the per-plane passives, on the lanes the
//!    loot table already had, plus one it did not (see
//!    `Item::grants_swim_speed`).
//! 4. **Spellcasting** — a menu, which is what [`StaffSpell`] is for.
//!    Fifteen of RAW's seventeen rows ship; see below for the two that
//!    do not and why.
//!
//! ## Why the spell menu is `StaffSpell` and not a pile of wands
//!
//! Because it is a menu, and that is the whole argument
//! `crate::actions::staves` opens with: a wand is one spell and can be
//! restated as a dice pool and a DC, and an item that prints four rows
//! at four prices cannot be restated four times without four chances to
//! get it wrong. The ring's Fireball *is* `spells::FIREBALL` — same
//! dice, same evasion, same everything — bought with two charges
//! instead of a slot.
//!
//! It inherits that chassis's one standing divergence, and here it
//! costs more than usual: RAW gives every row on this ring a flat
//! *"save DC of 18"*, and `StaffSpell` rolls the **holder's own** spell
//! save DC. On a staff that trade is nearly free, because every staff
//! in the book requires attunement by a caster and a caster has a DC.
//! This ring requires no such thing — RAW hands it to anybody — so a
//! fighter wearing it throws a weaker Fireball than the book's and a
//! high-level wizard throws a stronger one. Taking the other side of
//! the trade would mean hand-restating fifteen spells, which is
//! fifteen chances to mis-copy a die; see `WIND_FAN_GUST`, which made
//! the same call for the same reason.
//!
//! ## What is not here
//!
//! **Two spell rows.** *Create or Destroy Water* (water, 1 charge) has
//! no implementation in the engine and no combat surface that would
//! motivate one. *Feather Fall* (air, **0 charges**) is not a row at
//! all — it is a passive on the air ring, which is what RAW's zero in
//! the charge column amounts to: a clause with no pool to draw on and
//! nothing to decide. It rides `Condition::Feathered`, the same lane the
//! Ring of Feather Falling uses.
//!
//! **Three clauses of the Elemental Focus.** The earth ring's *"terrain
//! composed of rubble, rocks, or dirt isn't Difficult Terrain for
//! you"* and its earth-glide are both unmodeled: the engine's
//! difficult-terrain waiver is all-or-nothing
//! (`DIFFICULT_TERRAIN_IMMUNITIES`) and RAW's is scoped to a material
//! the terrain layer does not record, so the honest choices were "waive
//! every kind of rough ground including the magical ones" or "waive
//! none", and none is the smaller lie. The four elemental languages are
//! not modeled for the reason no item in the file grants a language:
//! there is nothing in a fight that reads one.
//!
//! **The control half of the compulsion.** RAW's *"you determine what
//! it does with its move and action on its next turn"* has no channel
//! — nothing in this engine lets one creature spend another's turn.
//! What ships is the sentence RAW actually writes the condition in:
//! *"the Elemental has the Charmed condition until the start of your
//! next turn"*, with the back-link, so a compelled elemental cannot
//! swing at the wearer for a round. `Condition::Dominated` is the
//! nearest thing in the engine and is deliberately **not** used: it is
//! a ten-round concentration effect whose blanket-disadvantage clause
//! is an approximation of a much longer leash, and borrowing it for one
//! round would make this ring's one turn of respite look like a
//! fifth-level spell.

use crate::actions::item_actions::{ItemUseBilling, SingleSaveConditionItem};
use crate::actions::staves::StaffSpell;
use crate::conditions::{Condition, ConditionTimer};
use crate::engine::types::{AbilityScoreType, CreatureType};

/// RAW's *"60 feet"*, in the engine's 2.5-ft tiles. Named once because
/// all four compulsion rows print it.
const COMPULSION_REACH_TILES: isize = 24;

/// RAW's flat *"DC 18 Wisdom saving throw"* for Elemental Compulsion.
///
/// A fixed number rather than the wearer's spell save DC, and this is
/// the one clause on the ring where the engine can honour that: the
/// compulsion is not a spell, so it rides `SingleSaveConditionItem`,
/// which carries its own `dc` field. The spell menu below cannot — see
/// the module docstring.
const COMPULSION_DC: i32 = 18;

/// The only creature type any of this ring's gates admit.
///
/// One constant rather than four copies of a one-element slice, because
/// the four rings differ in their plane and not in what they are for:
/// every one of them is a ring for fighting elementals, and a typo in
/// one of four hand-written slices would be a ring that compels
/// everything or nothing.
const ELEMENTALS: &[CreatureType] = &[CreatureType::Elemental];

pub const RING_OF_ELEMENTAL_COMMAND_AIR_NAME: &str = "Ring of Elemental Command (air)";
pub const RING_OF_ELEMENTAL_COMMAND_EARTH_NAME: &str = "Ring of Elemental Command (earth)";
pub const RING_OF_ELEMENTAL_COMMAND_FIRE_NAME: &str = "Ring of Elemental Command (fire)";
pub const RING_OF_ELEMENTAL_COMMAND_WATER_NAME: &str = "Ring of Elemental Command (water)";

/// Elemental Compulsion, once per ring.
///
/// Four near-identical rows rather than one shared row, because
/// `SingleSaveConditionItem::item_name` is the ledger key the validator
/// gates on: a single row naming one of the four rings would be
/// unusable from the other three, and a row naming none of them would
/// be usable with no ring at all.
macro_rules! compulsion_row {
    ($ident:ident, $name:expr, $aliases:expr, $item:expr, $plane:expr) => {
        #[doc = concat!(
            "Elemental Compulsion on the ", $plane, " ring — *\"take a Magic \
             action to try to compel an Elemental you see within 60 feet of \
             yourself. The Elemental makes a DC 18 Wisdom saving throw.\"*"
        )]
        ///
        /// Free rather than charged: RAW prints the clause with no price,
        /// which is what `ItemUseBilling::Free` is for — an item whose own
        /// sentence has no limit on it, as distinct from a pool that
        /// happens to be empty.
        pub static $ident: SingleSaveConditionItem = SingleSaveConditionItem {
            action_name: $name,
            action_aliases: $aliases,
            item_name: $item,
            log_text: concat!(
                "{actor} turns the ring of elemental command (",
                $plane,
                "); the air bends toward its command."
            ),
            save: AbilityScoreType::Wisdom,
            dc: COMPULSION_DC,
            reach: COMPULSION_REACH_TILES,
            condition: Condition::Charmed,
            // RAW's *"until the start of your next turn"*, exactly.
            timer: ConditionTimer::UntilStartOfNextTurn,
            billing: ItemUseBilling::Free,
            target_types: ELEMENTALS,
        };
    };
}

compulsion_row!(
    COMPEL_ELEMENTAL_AIR,
    "air ring: compel elemental",
    &["air-compel"],
    RING_OF_ELEMENTAL_COMMAND_AIR_NAME,
    "air"
);
compulsion_row!(
    COMPEL_ELEMENTAL_EARTH,
    "earth ring: compel elemental",
    &["earth-compel"],
    RING_OF_ELEMENTAL_COMMAND_EARTH_NAME,
    "earth"
);
compulsion_row!(
    COMPEL_ELEMENTAL_FIRE,
    "fire ring: compel elemental",
    &["fire-compel"],
    RING_OF_ELEMENTAL_COMMAND_FIRE_NAME,
    "fire"
);
compulsion_row!(
    COMPEL_ELEMENTAL_WATER,
    "water ring: compel elemental",
    &["water-compel"],
    RING_OF_ELEMENTAL_COMMAND_WATER_NAME,
    "water"
);

// ---------------------------------------------------------------------
// The Spellcasting menus. One block per plane, in RAW's own order, at
// RAW's own prices. Every row is the real spell — see the module
// docstring for why, and for the DC the chassis cannot carry.
// ---------------------------------------------------------------------

/// Air, row 1 of 4 — *"Chain Lightning (3 charges)"*.
pub static AIR_RING_CHAIN_LIGHTNING: StaffSpell = StaffSpell {
    action_name: "air ring: chain lightning",
    action_aliases: &["air-chain-lightning"],
    item_name: RING_OF_ELEMENTAL_COMMAND_AIR_NAME,
    billing: ItemUseBilling::Charges(3),
    spell_level: 6,
    spell: || &*crate::actions::spells::CHAIN_LIGHTNING,
    only_targets: None,
};

/// Air, row 2 of 4 — *"Gust of Wind (2 charges)"*.
pub static AIR_RING_GUST_OF_WIND: StaffSpell = StaffSpell {
    action_name: "air ring: gust of wind",
    action_aliases: &["air-gust"],
    item_name: RING_OF_ELEMENTAL_COMMAND_AIR_NAME,
    billing: ItemUseBilling::Charges(2),
    spell_level: 2,
    spell: || &*crate::actions::spells::GUST_OF_WIND,
    only_targets: None,
};

/// Air, row 3 of 4 — *"Wind Wall (1 charge)"*.
///
/// RAW's fourth row, *Feather Fall (0 charges)*, is not here: a clause
/// that costs nothing and asks nothing is a passive, and it ships as one
/// on the ring itself. See the module docstring.
pub static AIR_RING_WIND_WALL: StaffSpell = StaffSpell {
    action_name: "air ring: wind wall",
    action_aliases: &["air-wind-wall"],
    item_name: RING_OF_ELEMENTAL_COMMAND_AIR_NAME,
    billing: ItemUseBilling::Charges(1),
    spell_level: 3,
    spell: || &*crate::actions::spells::WIND_WALL,
    only_targets: None,
};

/// Earth, row 1 of 4 — *"Earthquake (5 charges)"*. The ring's whole
/// pool, which is RAW's way of saying this is the thing it is for.
pub static EARTH_RING_EARTHQUAKE: StaffSpell = StaffSpell {
    action_name: "earth ring: earthquake",
    action_aliases: &["earth-quake"],
    item_name: RING_OF_ELEMENTAL_COMMAND_EARTH_NAME,
    billing: ItemUseBilling::Charges(5),
    spell_level: 8,
    spell: || &*crate::actions::spells::EARTHQUAKE,
    only_targets: None,
};

/// Earth, row 2 of 4 — *"Stone Shape (2 charges)"*.
pub static EARTH_RING_STONE_SHAPE: StaffSpell = StaffSpell {
    action_name: "earth ring: stone shape",
    action_aliases: &["earth-stone-shape"],
    item_name: RING_OF_ELEMENTAL_COMMAND_EARTH_NAME,
    billing: ItemUseBilling::Charges(2),
    spell_level: 4,
    spell: || &*crate::actions::spells::STONE_SHAPE,
    only_targets: None,
};

/// Earth, row 3 of 4 — *"Stoneskin (3 charges)"*.
pub static EARTH_RING_STONESKIN: StaffSpell = StaffSpell {
    action_name: "earth ring: stoneskin",
    action_aliases: &["earth-stoneskin"],
    item_name: RING_OF_ELEMENTAL_COMMAND_EARTH_NAME,
    billing: ItemUseBilling::Charges(3),
    spell_level: 4,
    spell: || &*crate::actions::spells::STONESKIN,
    only_targets: None,
};

/// Earth, row 4 of 4 — *"Wall of Stone (3 charges)"*.
pub static EARTH_RING_WALL_OF_STONE: StaffSpell = StaffSpell {
    action_name: "earth ring: wall of stone",
    action_aliases: &["earth-wall-of-stone"],
    item_name: RING_OF_ELEMENTAL_COMMAND_EARTH_NAME,
    billing: ItemUseBilling::Charges(3),
    spell_level: 5,
    spell: || &*crate::actions::spells::WALL_OF_STONE,
    only_targets: None,
};

/// Fire, row 1 of 4 — *"Burning Hands (1 charge)"*.
pub static FIRE_RING_BURNING_HANDS: StaffSpell = StaffSpell {
    action_name: "fire ring: burning hands",
    action_aliases: &["fire-burning-hands"],
    item_name: RING_OF_ELEMENTAL_COMMAND_FIRE_NAME,
    billing: ItemUseBilling::Charges(1),
    spell_level: 1,
    spell: || &*crate::actions::spells::BURNING_HANDS,
    only_targets: None,
};

/// Fire, row 2 of 4 — *"Fireball (2 charges)"*. Cheaper than the Staff
/// of Fire's three, which is the legendary rarity showing.
pub static FIRE_RING_FIREBALL: StaffSpell = StaffSpell {
    action_name: "fire ring: fireball",
    action_aliases: &["fire-ring-fireball"],
    item_name: RING_OF_ELEMENTAL_COMMAND_FIRE_NAME,
    billing: ItemUseBilling::Charges(2),
    spell_level: 3,
    spell: || &*crate::actions::spells::FIREBALL,
    only_targets: None,
};

/// Fire, row 3 of 4 — *"Fire Storm (4 charges)"*.
pub static FIRE_RING_FIRE_STORM: StaffSpell = StaffSpell {
    action_name: "fire ring: fire storm",
    action_aliases: &["fire-storm-ring"],
    item_name: RING_OF_ELEMENTAL_COMMAND_FIRE_NAME,
    billing: ItemUseBilling::Charges(4),
    spell_level: 7,
    spell: || &*crate::actions::spells::FIRE_STORM,
    only_targets: None,
};

/// Fire, row 4 of 4 — *"Wall of Fire (3 charges)"*.
pub static FIRE_RING_WALL_OF_FIRE: StaffSpell = StaffSpell {
    action_name: "fire ring: wall of fire",
    action_aliases: &["fire-ring-wall-of-fire"],
    item_name: RING_OF_ELEMENTAL_COMMAND_FIRE_NAME,
    billing: ItemUseBilling::Charges(3),
    spell_level: 4,
    spell: || &*crate::actions::spells::WALL_OF_FIRE,
    only_targets: None,
};

/// Water, row 1 of 4 — *"Ice Storm (2 charges)"*.
///
/// RAW's menu has five rows; *Create or Destroy Water (1 charge)* is the
/// missing one. It has no implementation in the engine, and nothing
/// about a fight would read it if it had. See the module docstring.
pub static WATER_RING_ICE_STORM: StaffSpell = StaffSpell {
    action_name: "water ring: ice storm",
    action_aliases: &["water-ice-storm"],
    item_name: RING_OF_ELEMENTAL_COMMAND_WATER_NAME,
    billing: ItemUseBilling::Charges(2),
    spell_level: 4,
    spell: || &*crate::actions::spells::ICE_STORM,
    only_targets: None,
};

/// Water, row 2 of 4 — *"Tsunami (5 charges)"*. The pool, all of it, for
/// the other of the ring's two ninth-rung answers.
pub static WATER_RING_TSUNAMI: StaffSpell = StaffSpell {
    action_name: "water ring: tsunami",
    action_aliases: &["water-tsunami"],
    item_name: RING_OF_ELEMENTAL_COMMAND_WATER_NAME,
    billing: ItemUseBilling::Charges(5),
    spell_level: 8,
    spell: || &*crate::actions::spells::TSUNAMI,
    only_targets: None,
};

/// Water, row 3 of 4 — *"Wall of Ice (3 charges)"*.
pub static WATER_RING_WALL_OF_ICE: StaffSpell = StaffSpell {
    action_name: "water ring: wall of ice",
    action_aliases: &["water-wall-of-ice"],
    item_name: RING_OF_ELEMENTAL_COMMAND_WATER_NAME,
    billing: ItemUseBilling::Charges(3),
    spell_level: 6,
    spell: || &*crate::actions::spells::WALL_OF_ICE,
    only_targets: None,
};

/// Water, row 4 of 4 — *"Water Walk (2 charges)"*.
pub static WATER_RING_WATER_WALK: StaffSpell = StaffSpell {
    action_name: "water ring: water walk",
    action_aliases: &["water-ring-walk"],
    item_name: RING_OF_ELEMENTAL_COMMAND_WATER_NAME,
    billing: ItemUseBilling::Charges(2),
    spell_level: 3,
    spell: || &*crate::actions::spells::WATER_WALK,
    only_targets: None,
};

/// Every row the four rings offer, in ring order.
///
/// The registry the invariants sweep, for the reason `STAFF_SPELLS` is
/// one: a row written and left off every list is a row nothing can find,
/// and the only symptom is an item that quietly does less than its
/// docstring says.
pub static ELEMENTAL_RING_SPELLS: &[&StaffSpell] = &[
    &AIR_RING_CHAIN_LIGHTNING,
    &AIR_RING_GUST_OF_WIND,
    &AIR_RING_WIND_WALL,
    &EARTH_RING_EARTHQUAKE,
    &EARTH_RING_STONE_SHAPE,
    &EARTH_RING_STONESKIN,
    &EARTH_RING_WALL_OF_STONE,
    &FIRE_RING_BURNING_HANDS,
    &FIRE_RING_FIREBALL,
    &FIRE_RING_FIRE_STORM,
    &FIRE_RING_WALL_OF_FIRE,
    &WATER_RING_ICE_STORM,
    &WATER_RING_TSUNAMI,
    &WATER_RING_WALL_OF_ICE,
    &WATER_RING_WATER_WALK,
];

/// Every compulsion row, in the same ring order.
pub static ELEMENTAL_RING_COMPULSIONS: &[&SingleSaveConditionItem] = &[
    &COMPEL_ELEMENTAL_AIR,
    &COMPEL_ELEMENTAL_EARTH,
    &COMPEL_ELEMENTAL_FIRE,
    &COMPEL_ELEMENTAL_WATER,
];
