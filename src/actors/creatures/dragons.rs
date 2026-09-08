//! SRD 5.2's dragon ladder — ten colours across four age categories,
//! generated from one table rather than written out forty times.
//!
//! Four of these forty existed before: an Adult Red, an Adult Green, an
//! Ancient Blue and a Young White, each a hand-written eighty-line
//! literal. The other thirty-six were the largest single gap in the
//! bestiary — a Copper Dragon Wyrmling and an Ancient Gold are the two
//! ends of the game's most recognisable monster, and neither could be
//! rolled.
//!
//! ## Why a table
//!
//! Because a dragon is two independent axes and nothing else. **Colour**
//! decides exactly one thing — a damage type — which is what it
//! breathes, what rides its Rend, and what it is immune to.
//! **Age** decides size, reach, how many Rends an Action buys, and
//! whether the thing is a monster or a boss. Everything left over is
//! four numbers off the stat block (AC, hit dice, the ability array, and
//! the breath's dice and DC).
//!
//! Written out as forty literals, that structure is invisible and the
//! divergences are silent: the four that existed already disagreed with
//! each other about whether an adult dragon has Legendary Actions,
//! whether it has one Legendary Resistance or three, and whether it is
//! Large or Huge — all three of which are properties of the *rung*, not
//! of the dragon. `DRAGONS` is one row per stat block and
//! `dragon_template` is the only place any of those questions is
//! answered.
//!
//! ## What is deliberately not modeled
//!
//!   - **Spellcasting.** SRD 5.2 gives every adult and ancient dragon an
//!     innate list keyed to its colour. Ten colours times two rungs is
//!     twenty spell lists, and the engine would need each of them
//!     curated against what it actually implements; the breath and the
//!     Rends are the whole of what a dragon does at the table.
//!   - **The metallics' second breath.** Brass Sleep, Bronze Repulsion,
//!     Copper Slowing, Gold Weakening, Silver Paralyzing — five distinct
//!     save-or-suffer cones, each with its own condition, and the
//!     `BreathWeapon` chassis they would ride shares the
//!     `"breath_weapon"` recharge pool with the elemental breath, so a
//!     metallic dragon would silently lose one of its two. Dropped
//!     together rather than one at a time, which is the honest way to
//!     drop five clauses that are the same clause.
//!   - **Burrow and the in-lair bonuses.** The board has no third axis
//!     for a burrowing dragon to use, and "4/Day in Lair" needs a lair
//!     flag the engine reads for resistances rather than for actions.
//!
//! ## One divergence worth naming
//!
//! SRD 5.2 prints no condition immunities on any dragon. This ladder
//! gives Adult and Ancient dragons immunity to Frightened and Charmed,
//! which is what the four hand-written templates did and is the older
//! printing's reading. It is kept because it is the clause that stops
//! the party's Fear and Dominate Monster from ending a boss fight on
//! one failed save, and because a dragon that can be frightened by an
//! adventurer is a strange thing to have written down.

use crate::engine::areas::AreaShape;
use crate::actions::class_features::{SWIM_SPEED_TAG, UNDERWATER_BREATHING_TAG};
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{
    BreathWeapon, FRIGHTFUL_PRESENCE, Multiattack, WeaponWithRider,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::dice::Dice;
use crate::engine::types::{
    AbilityScoreType, CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense,
};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// The DEX / CON / WIS / CHA save profile every dragon in this engine
/// shares. A `LazyLock` because `HashSet` isn't `const`-constructible
/// from a literal; all forty templates clone the same value.
static DRAGON_LEGENDARY_SAVES: LazyLock<HashSet<AbilityScoreType>> = LazyLock::new(|| {
    HashSet::from([
        AbilityScoreType::Dexterity,
        AbilityScoreType::Constitution,
        AbilityScoreType::Wisdom,
        AbilityScoreType::Charisma,
    ])
});

/// The colour axis. Everything a colour decides is one damage type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DragonColor {
    Black,
    Blue,
    Brass,
    Bronze,
    Copper,
    Gold,
    Green,
    Red,
    Silver,
    White,
}

impl DragonColor {
    /// What this dragon breathes, what rides its Rend, and what it
    /// cannot be hurt by — one answer, because in SRD 5.2 they are one
    /// answer. Two colours share each element except poison, which is
    /// the green's alone.
    pub fn damage_type(self) -> DamageType {
        match self {
            DragonColor::Black | DragonColor::Copper => DamageType::Acid,
            DragonColor::Blue | DragonColor::Bronze => DamageType::Lightning,
            DragonColor::Brass | DragonColor::Gold | DragonColor::Red => DamageType::Fire,
            DragonColor::Green => DamageType::Poison,
            DragonColor::Silver | DragonColor::White => DamageType::Cold,
        }
    }

    /// True for the colours whose stat block carries **Amphibious** —
    /// "the dragon can breathe air and water". Black, bronze, gold and
    /// green, at every rung of the ladder.
    ///
    /// A property of the colour rather than a column on `DragonRow`,
    /// because SRD 5.2 prints it the same way for all four rungs of
    /// each colour and a per-row bool would be the same value written
    /// out four times with four chances to mistype it. `swims` is a row
    /// for the opposite reason — a wyrmling and an ancient of the same
    /// colour genuinely differ on the Speed line.
    ///
    /// The two are *not* the same set, which is the whole reason this
    /// exists: the white dragon has a swim speed and no Amphibious
    /// trait, so it crosses a lake for free and drowns under one. That
    /// is RAW, and the difference used to be invisible because nothing
    /// read it.
    fn amphibious(self) -> bool {
        matches!(
            self,
            DragonColor::Black | DragonColor::Bronze | DragonColor::Gold | DragonColor::Green
        )
    }

    /// Map glyph. Every letter of the alphabet is already spoken for
    /// somewhere in this bestiary, so these are chosen to be readable
    /// against *each other* rather than unique on the board: the
    /// colour's initial where the ladder leaves it free, and a letter
    /// out of the middle of the word where another colour took it —
    /// blacK, brAss, bronZe, gOld.
    fn glyph(self) -> char {
        match self {
            DragonColor::Black => 'K',
            DragonColor::Blue => 'B',
            DragonColor::Brass => 'A',
            DragonColor::Bronze => 'Z',
            DragonColor::Copper => 'C',
            DragonColor::Gold => 'O',
            DragonColor::Green => 'G',
            DragonColor::Red => 'R',
            DragonColor::Silver => 'S',
            DragonColor::White => 'W',
        }
    }
}

/// The age axis. Four rungs, and the rung decides everything the colour
/// does not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DragonAge {
    Wyrmling,
    Young,
    Adult,
    Ancient,
}

impl DragonAge {
    fn size(self) -> Size {
        match self {
            DragonAge::Wyrmling => Size::Medium,
            DragonAge::Young => Size::Large,
            DragonAge::Adult => Size::Huge,
            DragonAge::Ancient => Size::Gargantuan,
        }
    }

    /// True for the two rungs SRD 5.2 gives Legendary Actions, a lair
    /// and a Frightful Presence — which is the same line as "is this a
    /// boss fight or a monster".
    fn is_legendary(self) -> bool {
        matches!(self, DragonAge::Adult | DragonAge::Ancient)
    }

    /// "Legendary Resistance (3/Day, or 4/Day in Lair)" on an adult;
    /// 4/Day on an ancient. The in-lair bonus is not modeled.
    fn legendary_resistances(self) -> u32 {
        match self {
            DragonAge::Wyrmling | DragonAge::Young => 0,
            DragonAge::Adult => 3,
            DragonAge::Ancient => 4,
        }
    }
}

/// One stat block's worth of the numbers that are neither colour nor
/// age — the four lines off the top of the page, plus the two senses
/// that widen with the rung.
struct DragonRow {
    color: DragonColor,
    age: DragonAge,
    ac: u32,
    hitpoints: &'static str,
    speed: f32,
    fly_speed: f32,
    /// RAW swim speed, as a flag rather than a number: the engine's
    /// `SWIM_SPEED_TAG` is what makes `TerrainType::Water` free to
    /// cross and lifts the underwater melee penalty, and it has no
    /// magnitude.
    swims: bool,
    /// STR, DEX, CON, INT, WIS, CHA, in that order — the order the stat
    /// block prints them.
    abilities: [u32; 6],
    cr: f32,
    blindsight: u32,
    darkvision: u32,
}

/// Every dragon in SRD 5.2, ordered colour-major and youngest-first
/// inside each colour. The index into this table is the index into
/// `DRAGON_BREATHS`, `DRAGON_RENDS` and `DRAGON_MULTIATTACKS`; the four
/// arrays are one table split four ways because three of them have to
/// be `static` for their entries to be borrowed as `&'static dyn
/// Action`.
const DRAGONS: [DragonRow; 40] = [
    // Wyrmling Black Dragon — CR 2, 6d8 + 6.
    DragonRow {
        color: DragonColor::Black,
        age: DragonAge::Wyrmling,
        ac: 17,
        hitpoints: "6d8+6",
        speed: 30.0,
        fly_speed: 60.0,
        swims: true,
        abilities: [15, 14, 13, 10, 11, 13],
        cr: 2.0,
        blindsight: 10,
        darkvision: 60,
    },
    // Young Black Dragon — CR 7, 15d10 + 45.
    DragonRow {
        color: DragonColor::Black,
        age: DragonAge::Young,
        ac: 18,
        hitpoints: "15d10+45",
        speed: 40.0,
        fly_speed: 80.0,
        swims: true,
        abilities: [19, 14, 17, 12, 11, 15],
        cr: 7.0,
        blindsight: 30,
        darkvision: 120,
    },
    // Adult Black Dragon — CR 14, 17d12 + 85.
    DragonRow {
        color: DragonColor::Black,
        age: DragonAge::Adult,
        ac: 19,
        hitpoints: "17d12+85",
        speed: 40.0,
        fly_speed: 80.0,
        swims: true,
        abilities: [23, 14, 21, 14, 13, 19],
        cr: 14.0,
        blindsight: 60,
        darkvision: 120,
    },
    // Ancient Black Dragon — CR 21, 21d20 + 147.
    DragonRow {
        color: DragonColor::Black,
        age: DragonAge::Ancient,
        ac: 22,
        hitpoints: "21d20+147",
        speed: 40.0,
        fly_speed: 80.0,
        swims: true,
        abilities: [27, 14, 25, 16, 15, 22],
        cr: 21.0,
        blindsight: 60,
        darkvision: 120,
    },
    // Wyrmling Blue Dragon — CR 3, 10d8 + 20.
    DragonRow {
        color: DragonColor::Blue,
        age: DragonAge::Wyrmling,
        ac: 17,
        hitpoints: "10d8+20",
        speed: 30.0,
        fly_speed: 60.0,
        swims: false,
        abilities: [17, 10, 15, 12, 11, 15],
        cr: 3.0,
        blindsight: 10,
        darkvision: 60,
    },
    // Young Blue Dragon — CR 9, 16d10 + 64.
    DragonRow {
        color: DragonColor::Blue,
        age: DragonAge::Young,
        ac: 18,
        hitpoints: "16d10+64",
        speed: 40.0,
        fly_speed: 80.0,
        swims: false,
        abilities: [21, 10, 19, 14, 13, 17],
        cr: 9.0,
        blindsight: 30,
        darkvision: 120,
    },
    // Adult Blue Dragon — CR 16, 17d12 + 102.
    DragonRow {
        color: DragonColor::Blue,
        age: DragonAge::Adult,
        ac: 19,
        hitpoints: "17d12+102",
        speed: 40.0,
        fly_speed: 80.0,
        swims: false,
        abilities: [25, 10, 23, 16, 15, 20],
        cr: 16.0,
        blindsight: 60,
        darkvision: 120,
    },
    // Ancient Blue Dragon — CR 23, 26d20 + 208.
    DragonRow {
        color: DragonColor::Blue,
        age: DragonAge::Ancient,
        ac: 22,
        hitpoints: "26d20+208",
        speed: 40.0,
        fly_speed: 80.0,
        swims: false,
        abilities: [29, 10, 27, 18, 17, 25],
        cr: 23.0,
        blindsight: 60,
        darkvision: 120,
    },
    // Wyrmling Green Dragon — CR 2, 7d8 + 7.
    DragonRow {
        color: DragonColor::Green,
        age: DragonAge::Wyrmling,
        ac: 17,
        hitpoints: "7d8+7",
        speed: 30.0,
        fly_speed: 60.0,
        swims: true,
        abilities: [15, 12, 13, 14, 11, 13],
        cr: 2.0,
        blindsight: 10,
        darkvision: 60,
    },
    // Young Green Dragon — CR 8, 16d10 + 48.
    DragonRow {
        color: DragonColor::Green,
        age: DragonAge::Young,
        ac: 18,
        hitpoints: "16d10+48",
        speed: 40.0,
        fly_speed: 80.0,
        swims: true,
        abilities: [19, 12, 17, 16, 13, 15],
        cr: 8.0,
        blindsight: 30,
        darkvision: 120,
    },
    // Adult Green Dragon — CR 15, 18d12 + 90.
    DragonRow {
        color: DragonColor::Green,
        age: DragonAge::Adult,
        ac: 19,
        hitpoints: "18d12+90",
        speed: 40.0,
        fly_speed: 80.0,
        swims: true,
        abilities: [23, 12, 21, 18, 15, 18],
        cr: 15.0,
        blindsight: 60,
        darkvision: 120,
    },
    // Ancient Green Dragon — CR 22, 23d20 + 161.
    DragonRow {
        color: DragonColor::Green,
        age: DragonAge::Ancient,
        ac: 21,
        hitpoints: "23d20+161",
        speed: 40.0,
        fly_speed: 80.0,
        swims: true,
        abilities: [27, 12, 25, 20, 17, 22],
        cr: 22.0,
        blindsight: 60,
        darkvision: 120,
    },
    // Wyrmling Red Dragon — CR 4, 10d8 + 30.
    DragonRow {
        color: DragonColor::Red,
        age: DragonAge::Wyrmling,
        ac: 17,
        hitpoints: "10d8+30",
        speed: 30.0,
        fly_speed: 60.0,
        swims: false,
        abilities: [19, 10, 17, 12, 11, 15],
        cr: 4.0,
        blindsight: 10,
        darkvision: 60,
    },
    // Young Red Dragon — CR 10, 17d10 + 85.
    DragonRow {
        color: DragonColor::Red,
        age: DragonAge::Young,
        ac: 18,
        hitpoints: "17d10+85",
        speed: 40.0,
        fly_speed: 80.0,
        swims: false,
        abilities: [23, 10, 21, 14, 11, 19],
        cr: 10.0,
        blindsight: 30,
        darkvision: 120,
    },
    // Adult Red Dragon — CR 17, 19d12 + 133.
    DragonRow {
        color: DragonColor::Red,
        age: DragonAge::Adult,
        ac: 19,
        hitpoints: "19d12+133",
        speed: 40.0,
        fly_speed: 80.0,
        swims: false,
        abilities: [27, 10, 25, 16, 13, 23],
        cr: 17.0,
        blindsight: 60,
        darkvision: 120,
    },
    // Ancient Red Dragon — CR 24, 26d20 + 234.
    DragonRow {
        color: DragonColor::Red,
        age: DragonAge::Ancient,
        ac: 22,
        hitpoints: "26d20+234",
        speed: 40.0,
        fly_speed: 80.0,
        swims: false,
        abilities: [30, 10, 29, 18, 15, 27],
        cr: 24.0,
        blindsight: 60,
        darkvision: 120,
    },
    // Wyrmling White Dragon — CR 2, 5d8 + 10.
    DragonRow {
        color: DragonColor::White,
        age: DragonAge::Wyrmling,
        ac: 16,
        hitpoints: "5d8+10",
        speed: 30.0,
        fly_speed: 60.0,
        swims: true,
        abilities: [14, 10, 14, 5, 10, 11],
        cr: 2.0,
        blindsight: 10,
        darkvision: 60,
    },
    // Young White Dragon — CR 6, 13d10 + 52.
    DragonRow {
        color: DragonColor::White,
        age: DragonAge::Young,
        ac: 17,
        hitpoints: "13d10+52",
        speed: 40.0,
        fly_speed: 80.0,
        swims: true,
        abilities: [18, 10, 18, 6, 11, 12],
        cr: 6.0,
        blindsight: 30,
        darkvision: 120,
    },
    // Adult White Dragon — CR 13, 16d12 + 96.
    DragonRow {
        color: DragonColor::White,
        age: DragonAge::Adult,
        ac: 18,
        hitpoints: "16d12+96",
        speed: 40.0,
        fly_speed: 80.0,
        swims: true,
        abilities: [22, 10, 22, 8, 12, 12],
        cr: 13.0,
        blindsight: 60,
        darkvision: 120,
    },
    // Ancient White Dragon — CR 20, 18d20 + 144.
    DragonRow {
        color: DragonColor::White,
        age: DragonAge::Ancient,
        ac: 20,
        hitpoints: "18d20+144",
        speed: 40.0,
        fly_speed: 80.0,
        swims: true,
        abilities: [26, 10, 26, 10, 13, 18],
        cr: 20.0,
        blindsight: 60,
        darkvision: 120,
    },
    // Wyrmling Brass Dragon — CR 1, 4d8 + 4.
    DragonRow {
        color: DragonColor::Brass,
        age: DragonAge::Wyrmling,
        ac: 15,
        hitpoints: "4d8+4",
        speed: 30.0,
        fly_speed: 60.0,
        swims: false,
        abilities: [15, 10, 13, 10, 11, 13],
        cr: 1.0,
        blindsight: 10,
        darkvision: 60,
    },
    // Young Brass Dragon — CR 6, 13d10 + 39.
    DragonRow {
        color: DragonColor::Brass,
        age: DragonAge::Young,
        ac: 17,
        hitpoints: "13d10+39",
        speed: 40.0,
        fly_speed: 80.0,
        swims: false,
        abilities: [19, 10, 17, 12, 11, 15],
        cr: 6.0,
        blindsight: 30,
        darkvision: 120,
    },
    // Adult Brass Dragon — CR 13, 15d12 + 75.
    DragonRow {
        color: DragonColor::Brass,
        age: DragonAge::Adult,
        ac: 18,
        hitpoints: "15d12+75",
        speed: 40.0,
        fly_speed: 80.0,
        swims: false,
        abilities: [23, 10, 21, 14, 13, 17],
        cr: 13.0,
        blindsight: 60,
        darkvision: 120,
    },
    // Ancient Brass Dragon — CR 20, 19d20 + 133.
    DragonRow {
        color: DragonColor::Brass,
        age: DragonAge::Ancient,
        ac: 20,
        hitpoints: "19d20+133",
        speed: 40.0,
        fly_speed: 80.0,
        swims: false,
        abilities: [27, 10, 25, 16, 15, 22],
        cr: 20.0,
        blindsight: 60,
        darkvision: 120,
    },
    // Wyrmling Bronze Dragon — CR 2, 6d8 + 12.
    DragonRow {
        color: DragonColor::Bronze,
        age: DragonAge::Wyrmling,
        ac: 15,
        hitpoints: "6d8+12",
        speed: 30.0,
        fly_speed: 60.0,
        swims: true,
        abilities: [17, 10, 15, 12, 11, 15],
        cr: 2.0,
        blindsight: 10,
        darkvision: 60,
    },
    // Young Bronze Dragon — CR 8, 15d10 + 60.
    DragonRow {
        color: DragonColor::Bronze,
        age: DragonAge::Young,
        ac: 17,
        hitpoints: "15d10+60",
        speed: 40.0,
        fly_speed: 80.0,
        swims: true,
        abilities: [21, 10, 19, 14, 13, 17],
        cr: 8.0,
        blindsight: 30,
        darkvision: 120,
    },
    // Adult Bronze Dragon — CR 15, 17d12 + 102.
    DragonRow {
        color: DragonColor::Bronze,
        age: DragonAge::Adult,
        ac: 18,
        hitpoints: "17d12+102",
        speed: 40.0,
        fly_speed: 80.0,
        swims: true,
        abilities: [25, 10, 23, 16, 15, 20],
        cr: 15.0,
        blindsight: 60,
        darkvision: 120,
    },
    // Ancient Bronze Dragon — CR 22, 24d20 + 192.
    DragonRow {
        color: DragonColor::Bronze,
        age: DragonAge::Ancient,
        ac: 22,
        hitpoints: "24d20+192",
        speed: 40.0,
        fly_speed: 80.0,
        swims: true,
        abilities: [29, 10, 27, 18, 17, 25],
        cr: 22.0,
        blindsight: 60,
        darkvision: 120,
    },
    // Wyrmling Copper Dragon — CR 1, 4d8 + 4.
    DragonRow {
        color: DragonColor::Copper,
        age: DragonAge::Wyrmling,
        ac: 16,
        hitpoints: "4d8+4",
        speed: 30.0,
        fly_speed: 60.0,
        swims: false,
        abilities: [15, 12, 13, 14, 11, 13],
        cr: 1.0,
        blindsight: 10,
        darkvision: 60,
    },
    // Young Copper Dragon — CR 7, 14d10 + 42.
    DragonRow {
        color: DragonColor::Copper,
        age: DragonAge::Young,
        ac: 17,
        hitpoints: "14d10+42",
        speed: 40.0,
        fly_speed: 80.0,
        swims: false,
        abilities: [19, 12, 17, 16, 13, 15],
        cr: 7.0,
        blindsight: 30,
        darkvision: 120,
    },
    // Adult Copper Dragon — CR 14, 16d12 + 80.
    DragonRow {
        color: DragonColor::Copper,
        age: DragonAge::Adult,
        ac: 18,
        hitpoints: "16d12+80",
        speed: 40.0,
        fly_speed: 80.0,
        swims: false,
        abilities: [23, 12, 21, 18, 15, 18],
        cr: 14.0,
        blindsight: 60,
        darkvision: 120,
    },
    // Ancient Copper Dragon — CR 21, 21d20 + 147.
    DragonRow {
        color: DragonColor::Copper,
        age: DragonAge::Ancient,
        ac: 21,
        hitpoints: "21d20+147",
        speed: 40.0,
        fly_speed: 80.0,
        swims: false,
        abilities: [27, 12, 25, 20, 17, 22],
        cr: 21.0,
        blindsight: 60,
        darkvision: 120,
    },
    // Wyrmling Gold Dragon — CR 3, 8d8 + 24.
    DragonRow {
        color: DragonColor::Gold,
        age: DragonAge::Wyrmling,
        ac: 17,
        hitpoints: "8d8+24",
        speed: 30.0,
        fly_speed: 60.0,
        swims: true,
        abilities: [19, 14, 17, 14, 11, 16],
        cr: 3.0,
        blindsight: 10,
        darkvision: 60,
    },
    // Young Gold Dragon — CR 10, 17d10 + 85.
    DragonRow {
        color: DragonColor::Gold,
        age: DragonAge::Young,
        ac: 18,
        hitpoints: "17d10+85",
        speed: 40.0,
        fly_speed: 80.0,
        swims: true,
        abilities: [23, 14, 21, 16, 13, 20],
        cr: 10.0,
        blindsight: 30,
        darkvision: 120,
    },
    // Adult Gold Dragon — CR 17, 18d12 + 126.
    DragonRow {
        color: DragonColor::Gold,
        age: DragonAge::Adult,
        ac: 19,
        hitpoints: "18d12+126",
        speed: 40.0,
        fly_speed: 80.0,
        swims: true,
        abilities: [27, 14, 25, 16, 15, 24],
        cr: 17.0,
        blindsight: 60,
        darkvision: 120,
    },
    // Ancient Gold Dragon — CR 24, 28d20 + 252.
    DragonRow {
        color: DragonColor::Gold,
        age: DragonAge::Ancient,
        ac: 22,
        hitpoints: "28d20+252",
        speed: 40.0,
        fly_speed: 80.0,
        swims: true,
        abilities: [30, 14, 29, 18, 17, 28],
        cr: 24.0,
        blindsight: 60,
        darkvision: 120,
    },
    // Wyrmling Silver Dragon — CR 2, 6d8 + 18.
    DragonRow {
        color: DragonColor::Silver,
        age: DragonAge::Wyrmling,
        ac: 17,
        hitpoints: "6d8+18",
        speed: 30.0,
        fly_speed: 60.0,
        swims: false,
        abilities: [19, 10, 17, 12, 11, 15],
        cr: 2.0,
        blindsight: 10,
        darkvision: 60,
    },
    // Young Silver Dragon — CR 9, 16d10 + 80.
    DragonRow {
        color: DragonColor::Silver,
        age: DragonAge::Young,
        ac: 18,
        hitpoints: "16d10+80",
        speed: 40.0,
        fly_speed: 80.0,
        swims: false,
        abilities: [23, 10, 21, 14, 11, 19],
        cr: 9.0,
        blindsight: 30,
        darkvision: 120,
    },
    // Adult Silver Dragon — CR 16, 16d12 + 112.
    DragonRow {
        color: DragonColor::Silver,
        age: DragonAge::Adult,
        ac: 19,
        hitpoints: "16d12+112",
        speed: 40.0,
        fly_speed: 80.0,
        swims: false,
        abilities: [27, 10, 25, 16, 13, 22],
        cr: 16.0,
        blindsight: 60,
        darkvision: 120,
    },
    // Ancient Silver Dragon — CR 23, 24d20 + 216.
    DragonRow {
        color: DragonColor::Silver,
        age: DragonAge::Ancient,
        ac: 22,
        hitpoints: "24d20+216",
        speed: 40.0,
        fly_speed: 80.0,
        swims: false,
        abilities: [30, 10, 29, 18, 15, 26],
        cr: 23.0,
        blindsight: 60,
        darkvision: 120,
    },
];

/// One breath weapon per dragon, in `DRAGONS` order.
///
/// Each entry carries the shape and the length RAW prints for it, on
/// the engine's 2.5 ft grid: a 60-foot cone is `Cone { length: 24 }`
/// and a 90-foot line is `Line { length: 36, half_width: 1 }`.
///
/// The distinction between a cone and a line used to be dropped, and
/// dropping it flattened the chromatic and metallic families into one
/// creature at four sizes. It is the difference RAW draws between the
/// two halves of the colour wheel, and it is a real tactical one: a
/// green dragon wants the party clumped and a blue one wants it in a
/// row, so the same party formation is the right answer against one and
/// the wrong one against the other. Twenty of these forty breathe a
/// line.
///
/// The width on the ancient lines is RAW's too — the four oldest
/// line-breathers widen from 5 feet to 10 — which is why `half_width`
/// is 2 on those rows and 1 everywhere else.
static DRAGON_BREATHS: [BreathWeapon; 40] = [
    // Wyrmling Black: 15-ft Line, 5d8 acid, DC 11 DEX.
    BreathWeapon {
        display_name: "acid breath",
        aliases: &["breath", "br"],
        damage: Some((Dice::new(5, 8), DamageType::Acid)),
        save_ability: AbilityScoreType::Dexterity,
        dc: 11,
        shape: AreaShape::Line {
            length: 6,
            half_width: 1,
        },
        recharge_key: "breath_weapon",
        condition: None,
        enemies_only: false,
    },
    // Young Black: 30-ft Line, 14d6 acid, DC 14 DEX.
    BreathWeapon {
        display_name: "acid breath",
        aliases: &["breath", "br"],
        damage: Some((Dice::new(14, 6), DamageType::Acid)),
        save_ability: AbilityScoreType::Dexterity,
        dc: 14,
        shape: AreaShape::Line {
            length: 12,
            half_width: 1,
        },
        recharge_key: "breath_weapon",
        condition: None,
        enemies_only: false,
    },
    // Adult Black: 60-ft Line, 12d8 acid, DC 18 DEX.
    BreathWeapon {
        display_name: "acid breath",
        aliases: &["breath", "br"],
        damage: Some((Dice::new(12, 8), DamageType::Acid)),
        save_ability: AbilityScoreType::Dexterity,
        dc: 18,
        shape: AreaShape::Line {
            length: 24,
            half_width: 1,
        },
        recharge_key: "breath_weapon",
        condition: None,
        enemies_only: false,
    },
    // Ancient Black: 90-ft Line, 15d8 acid, DC 22 DEX.
    BreathWeapon {
        display_name: "acid breath",
        aliases: &["breath", "br"],
        damage: Some((Dice::new(15, 8), DamageType::Acid)),
        save_ability: AbilityScoreType::Dexterity,
        dc: 22,
        shape: AreaShape::Line {
            length: 36,
            half_width: 2,
        },
        recharge_key: "breath_weapon",
        condition: None,
        enemies_only: false,
    },
    // Wyrmling Blue: 30-ft Line, 6d6 lightning, DC 12 DEX.
    BreathWeapon {
        display_name: "lightning breath",
        aliases: &["breath", "br"],
        damage: Some((Dice::new(6, 6), DamageType::Lightning)),
        save_ability: AbilityScoreType::Dexterity,
        dc: 12,
        shape: AreaShape::Line {
            length: 12,
            half_width: 1,
        },
        recharge_key: "breath_weapon",
        condition: None,
        enemies_only: false,
    },
    // Young Blue: 60-ft Line, 10d10 lightning, DC 16 DEX.
    BreathWeapon {
        display_name: "lightning breath",
        aliases: &["breath", "br"],
        damage: Some((Dice::new(10, 10), DamageType::Lightning)),
        save_ability: AbilityScoreType::Dexterity,
        dc: 16,
        shape: AreaShape::Line {
            length: 24,
            half_width: 1,
        },
        recharge_key: "breath_weapon",
        condition: None,
        enemies_only: false,
    },
    // Adult Blue: 90-ft Line, 11d10 lightning, DC 19 DEX.
    BreathWeapon {
        display_name: "lightning breath",
        aliases: &["breath", "br"],
        damage: Some((Dice::new(11, 10), DamageType::Lightning)),
        save_ability: AbilityScoreType::Dexterity,
        dc: 19,
        shape: AreaShape::Line {
            length: 36,
            half_width: 1,
        },
        recharge_key: "breath_weapon",
        condition: None,
        enemies_only: false,
    },
    // Ancient Blue: 120-ft Line, 16d10 lightning, DC 23 DEX.
    BreathWeapon {
        display_name: "lightning breath",
        aliases: &["breath", "br"],
        damage: Some((Dice::new(16, 10), DamageType::Lightning)),
        save_ability: AbilityScoreType::Dexterity,
        dc: 23,
        shape: AreaShape::Line {
            length: 48,
            half_width: 2,
        },
        recharge_key: "breath_weapon",
        condition: None,
        enemies_only: false,
    },
    // Wyrmling Green: 15-ft Cone, 6d6 poison, DC 11 CON.
    BreathWeapon {
        display_name: "poison breath",
        aliases: &["breath", "br"],
        damage: Some((Dice::new(6, 6), DamageType::Poison)),
        save_ability: AbilityScoreType::Constitution,
        dc: 11,
        shape: AreaShape::Cone { length: 6 },
        recharge_key: "breath_weapon",
        condition: None,
        enemies_only: false,
    },
    // Young Green: 30-ft Cone, 12d6 poison, DC 14 CON.
    BreathWeapon {
        display_name: "poison breath",
        aliases: &["breath", "br"],
        damage: Some((Dice::new(12, 6), DamageType::Poison)),
        save_ability: AbilityScoreType::Constitution,
        dc: 14,
        shape: AreaShape::Cone { length: 12 },
        recharge_key: "breath_weapon",
        condition: None,
        enemies_only: false,
    },
    // Adult Green: 60-ft Cone, 16d6 poison, DC 18 CON.
    BreathWeapon {
        display_name: "poison breath",
        aliases: &["breath", "br"],
        damage: Some((Dice::new(16, 6), DamageType::Poison)),
        save_ability: AbilityScoreType::Constitution,
        dc: 18,
        shape: AreaShape::Cone { length: 24 },
        recharge_key: "breath_weapon",
        condition: None,
        enemies_only: false,
    },
    // Ancient Green: 90-ft Cone, 22d6 poison, DC 22 CON.
    BreathWeapon {
        display_name: "poison breath",
        aliases: &["breath", "br"],
        damage: Some((Dice::new(22, 6), DamageType::Poison)),
        save_ability: AbilityScoreType::Constitution,
        dc: 22,
        shape: AreaShape::Cone { length: 36 },
        recharge_key: "breath_weapon",
        condition: None,
        enemies_only: false,
    },
    // Wyrmling Red: 15-ft Cone, 7d6 fire, DC 13 DEX.
    BreathWeapon {
        display_name: "fire breath",
        aliases: &["breath", "br"],
        damage: Some((Dice::new(7, 6), DamageType::Fire)),
        save_ability: AbilityScoreType::Dexterity,
        dc: 13,
        shape: AreaShape::Cone { length: 6 },
        recharge_key: "breath_weapon",
        condition: None,
        enemies_only: false,
    },
    // Young Red: 30-ft Cone, 16d6 fire, DC 17 DEX.
    BreathWeapon {
        display_name: "fire breath",
        aliases: &["breath", "br"],
        damage: Some((Dice::new(16, 6), DamageType::Fire)),
        save_ability: AbilityScoreType::Dexterity,
        dc: 17,
        shape: AreaShape::Cone { length: 12 },
        recharge_key: "breath_weapon",
        condition: None,
        enemies_only: false,
    },
    // Adult Red: 60-ft Cone, 17d6 fire, DC 21 DEX.
    BreathWeapon {
        display_name: "fire breath",
        aliases: &["breath", "br"],
        damage: Some((Dice::new(17, 6), DamageType::Fire)),
        save_ability: AbilityScoreType::Dexterity,
        dc: 21,
        shape: AreaShape::Cone { length: 24 },
        recharge_key: "breath_weapon",
        condition: None,
        enemies_only: false,
    },
    // Ancient Red: 90-ft Cone, 26d6 fire, DC 24 DEX.
    BreathWeapon {
        display_name: "fire breath",
        aliases: &["breath", "br"],
        damage: Some((Dice::new(26, 6), DamageType::Fire)),
        save_ability: AbilityScoreType::Dexterity,
        dc: 24,
        shape: AreaShape::Cone { length: 36 },
        recharge_key: "breath_weapon",
        condition: None,
        enemies_only: false,
    },
    // Wyrmling White: 15-ft Cone, 5d8 cold, DC 12 CON.
    BreathWeapon {
        display_name: "cold breath",
        aliases: &["breath", "br"],
        damage: Some((Dice::new(5, 8), DamageType::Cold)),
        save_ability: AbilityScoreType::Constitution,
        dc: 12,
        shape: AreaShape::Cone { length: 6 },
        recharge_key: "breath_weapon",
        condition: None,
        enemies_only: false,
    },
    // Young White: 30-ft Cone, 9d8 cold, DC 15 CON.
    BreathWeapon {
        display_name: "cold breath",
        aliases: &["breath", "br"],
        damage: Some((Dice::new(9, 8), DamageType::Cold)),
        save_ability: AbilityScoreType::Constitution,
        dc: 15,
        shape: AreaShape::Cone { length: 12 },
        recharge_key: "breath_weapon",
        condition: None,
        enemies_only: false,
    },
    // Adult White: 60-ft Cone, 12d8 cold, DC 19 CON.
    BreathWeapon {
        display_name: "cold breath",
        aliases: &["breath", "br"],
        damage: Some((Dice::new(12, 8), DamageType::Cold)),
        save_ability: AbilityScoreType::Constitution,
        dc: 19,
        shape: AreaShape::Cone { length: 24 },
        recharge_key: "breath_weapon",
        condition: None,
        enemies_only: false,
    },
    // Ancient White: 90-ft Cone, 14d8 cold, DC 22 CON.
    BreathWeapon {
        display_name: "cold breath",
        aliases: &["breath", "br"],
        damage: Some((Dice::new(14, 8), DamageType::Cold)),
        save_ability: AbilityScoreType::Constitution,
        dc: 22,
        shape: AreaShape::Cone { length: 36 },
        recharge_key: "breath_weapon",
        condition: None,
        enemies_only: false,
    },
    // Wyrmling Brass: 20-ft Line, 4d6 fire, DC 11 DEX.
    BreathWeapon {
        display_name: "fire breath",
        aliases: &["breath", "br"],
        damage: Some((Dice::new(4, 6), DamageType::Fire)),
        save_ability: AbilityScoreType::Dexterity,
        dc: 11,
        shape: AreaShape::Line {
            length: 8,
            half_width: 1,
        },
        recharge_key: "breath_weapon",
        condition: None,
        enemies_only: false,
    },
    // Young Brass: 40-ft Line, 11d6 fire, DC 14 DEX.
    BreathWeapon {
        display_name: "fire breath",
        aliases: &["breath", "br"],
        damage: Some((Dice::new(11, 6), DamageType::Fire)),
        save_ability: AbilityScoreType::Dexterity,
        dc: 14,
        shape: AreaShape::Line {
            length: 16,
            half_width: 1,
        },
        recharge_key: "breath_weapon",
        condition: None,
        enemies_only: false,
    },
    // Adult Brass: 60-ft Line, 10d8 fire, DC 18 DEX.
    BreathWeapon {
        display_name: "fire breath",
        aliases: &["breath", "br"],
        damage: Some((Dice::new(10, 8), DamageType::Fire)),
        save_ability: AbilityScoreType::Dexterity,
        dc: 18,
        shape: AreaShape::Line {
            length: 24,
            half_width: 1,
        },
        recharge_key: "breath_weapon",
        condition: None,
        enemies_only: false,
    },
    // Ancient Brass: 90-ft × 5-ft Line, 13d8 fire, DC 21 DEX. The one
    // ancient line-breather RAW leaves at five feet wide — the other
    // four widen to ten — so it is the row that would be wrong if the
    // width were derived from the age rather than read off the block.
    BreathWeapon {
        display_name: "fire breath",
        aliases: &["breath", "br"],
        damage: Some((Dice::new(13, 8), DamageType::Fire)),
        save_ability: AbilityScoreType::Dexterity,
        dc: 21,
        shape: AreaShape::Line {
            length: 36,
            half_width: 1,
        },
        recharge_key: "breath_weapon",
        condition: None,
        enemies_only: false,
    },
    // Wyrmling Bronze: 40-ft Line, 3d10 lightning, DC 12 DEX.
    BreathWeapon {
        display_name: "lightning breath",
        aliases: &["breath", "br"],
        damage: Some((Dice::new(3, 10), DamageType::Lightning)),
        save_ability: AbilityScoreType::Dexterity,
        dc: 12,
        shape: AreaShape::Line {
            length: 16,
            half_width: 1,
        },
        recharge_key: "breath_weapon",
        condition: None,
        enemies_only: false,
    },
    // Young Bronze: 60-ft Line, 9d10 lightning, DC 15 DEX.
    BreathWeapon {
        display_name: "lightning breath",
        aliases: &["breath", "br"],
        damage: Some((Dice::new(9, 10), DamageType::Lightning)),
        save_ability: AbilityScoreType::Dexterity,
        dc: 15,
        shape: AreaShape::Line {
            length: 24,
            half_width: 1,
        },
        recharge_key: "breath_weapon",
        condition: None,
        enemies_only: false,
    },
    // Adult Bronze: 90-ft Line, 10d10 lightning, DC 19 DEX.
    BreathWeapon {
        display_name: "lightning breath",
        aliases: &["breath", "br"],
        damage: Some((Dice::new(10, 10), DamageType::Lightning)),
        save_ability: AbilityScoreType::Dexterity,
        dc: 19,
        shape: AreaShape::Line {
            length: 36,
            half_width: 1,
        },
        recharge_key: "breath_weapon",
        condition: None,
        enemies_only: false,
    },
    // Ancient Bronze: 120-ft Line, 15d10 lightning, DC 23 DEX.
    BreathWeapon {
        display_name: "lightning breath",
        aliases: &["breath", "br"],
        damage: Some((Dice::new(15, 10), DamageType::Lightning)),
        save_ability: AbilityScoreType::Dexterity,
        dc: 23,
        shape: AreaShape::Line {
            length: 48,
            half_width: 2,
        },
        recharge_key: "breath_weapon",
        condition: None,
        enemies_only: false,
    },
    // Wyrmling Copper: 20-ft Line, 4d8 acid, DC 11 DEX.
    BreathWeapon {
        display_name: "acid breath",
        aliases: &["breath", "br"],
        damage: Some((Dice::new(4, 8), DamageType::Acid)),
        save_ability: AbilityScoreType::Dexterity,
        dc: 11,
        shape: AreaShape::Line {
            length: 8,
            half_width: 1,
        },
        recharge_key: "breath_weapon",
        condition: None,
        enemies_only: false,
    },
    // Young Copper: 40-ft Line, 9d8 acid, DC 14 DEX.
    BreathWeapon {
        display_name: "acid breath",
        aliases: &["breath", "br"],
        damage: Some((Dice::new(9, 8), DamageType::Acid)),
        save_ability: AbilityScoreType::Dexterity,
        dc: 14,
        shape: AreaShape::Line {
            length: 16,
            half_width: 1,
        },
        recharge_key: "breath_weapon",
        condition: None,
        enemies_only: false,
    },
    // Adult Copper: 60-ft Line, 12d8 acid, DC 18 DEX.
    BreathWeapon {
        display_name: "acid breath",
        aliases: &["breath", "br"],
        damage: Some((Dice::new(12, 8), DamageType::Acid)),
        save_ability: AbilityScoreType::Dexterity,
        dc: 18,
        shape: AreaShape::Line {
            length: 24,
            half_width: 1,
        },
        recharge_key: "breath_weapon",
        condition: None,
        enemies_only: false,
    },
    // Ancient Copper: 90-ft Line, 14d8 acid, DC 22 DEX.
    BreathWeapon {
        display_name: "acid breath",
        aliases: &["breath", "br"],
        damage: Some((Dice::new(14, 8), DamageType::Acid)),
        save_ability: AbilityScoreType::Dexterity,
        dc: 22,
        shape: AreaShape::Line {
            length: 36,
            half_width: 2,
        },
        recharge_key: "breath_weapon",
        condition: None,
        enemies_only: false,
    },
    // Wyrmling Gold: 15-ft Cone, 4d10 fire, DC 13 DEX.
    BreathWeapon {
        display_name: "fire breath",
        aliases: &["breath", "br"],
        damage: Some((Dice::new(4, 10), DamageType::Fire)),
        save_ability: AbilityScoreType::Dexterity,
        dc: 13,
        shape: AreaShape::Cone { length: 6 },
        recharge_key: "breath_weapon",
        condition: None,
        enemies_only: false,
    },
    // Young Gold: 30-ft Cone, 10d10 fire, DC 17 DEX.
    BreathWeapon {
        display_name: "fire breath",
        aliases: &["breath", "br"],
        damage: Some((Dice::new(10, 10), DamageType::Fire)),
        save_ability: AbilityScoreType::Dexterity,
        dc: 17,
        shape: AreaShape::Cone { length: 12 },
        recharge_key: "breath_weapon",
        condition: None,
        enemies_only: false,
    },
    // Adult Gold: 60-ft Cone, 12d10 fire, DC 21 DEX.
    BreathWeapon {
        display_name: "fire breath",
        aliases: &["breath", "br"],
        damage: Some((Dice::new(12, 10), DamageType::Fire)),
        save_ability: AbilityScoreType::Dexterity,
        dc: 21,
        shape: AreaShape::Cone { length: 24 },
        recharge_key: "breath_weapon",
        condition: None,
        enemies_only: false,
    },
    // Ancient Gold: 90-ft Cone, 13d10 fire, DC 24 DEX.
    BreathWeapon {
        display_name: "fire breath",
        aliases: &["breath", "br"],
        damage: Some((Dice::new(13, 10), DamageType::Fire)),
        save_ability: AbilityScoreType::Dexterity,
        dc: 24,
        shape: AreaShape::Cone { length: 36 },
        recharge_key: "breath_weapon",
        condition: None,
        enemies_only: false,
    },
    // Wyrmling Silver: 15-ft Cone, 4d8 cold, DC 13 CON.
    BreathWeapon {
        display_name: "cold breath",
        aliases: &["breath", "br"],
        damage: Some((Dice::new(4, 8), DamageType::Cold)),
        save_ability: AbilityScoreType::Constitution,
        dc: 13,
        shape: AreaShape::Cone { length: 6 },
        recharge_key: "breath_weapon",
        condition: None,
        enemies_only: false,
    },
    // Young Silver: 30-ft Cone, 11d8 cold, DC 17 CON.
    BreathWeapon {
        display_name: "cold breath",
        aliases: &["breath", "br"],
        damage: Some((Dice::new(11, 8), DamageType::Cold)),
        save_ability: AbilityScoreType::Constitution,
        dc: 17,
        shape: AreaShape::Cone { length: 12 },
        recharge_key: "breath_weapon",
        condition: None,
        enemies_only: false,
    },
    // Adult Silver: 60-ft Cone, 12d8 cold, DC 20 CON.
    BreathWeapon {
        display_name: "cold breath",
        aliases: &["breath", "br"],
        damage: Some((Dice::new(12, 8), DamageType::Cold)),
        save_ability: AbilityScoreType::Constitution,
        dc: 20,
        shape: AreaShape::Cone { length: 24 },
        recharge_key: "breath_weapon",
        condition: None,
        enemies_only: false,
    },
    // Ancient Silver: 90-ft Cone, 15d8 cold, DC 24 CON.
    BreathWeapon {
        display_name: "cold breath",
        aliases: &["breath", "br"],
        damage: Some((Dice::new(15, 8), DamageType::Cold)),
        save_ability: AbilityScoreType::Constitution,
        dc: 24,
        shape: AreaShape::Cone { length: 36 },
        recharge_key: "breath_weapon",
        condition: None,
        enemies_only: false,
    },
];

/// One Rend per dragon, in `DRAGONS` order.
///
/// SRD 5.2 folded the older bite-and-claws into a single Rend whose Hit
/// line carries the dragon's element as a rider — "13 (2d6 + 6)
/// Slashing damage plus 4 (1d8) Acid damage" — which is exactly the
/// `WeaponWithRider` shape. Nine of the forty print no rider (the
/// metallic wyrmlings and young, whose scales have not caught fire
/// yet); those carry `Dice::new(0, 1)`, which
/// `add_flat_damage_rider` reads as "no rider" and skips.
static DRAGON_RENDS: [WeaponWithRider; 40] = [
    // Wyrmling Black: 1d6 slashing, +1d4 acid, reach 5 ft.
    WeaponWithRider::reach_melee(
        "rend",
        &["rend"],
        AbilityScoreType::Strength,
        Dice::new(1, 6),
        DamageType::Slashing,
        1,
        Dice::new(1, 4),
        DamageType::Acid,
        "rend",
    ),
    // Young Black: 2d4 slashing, +1d6 acid, reach 10 ft.
    WeaponWithRider::reach_melee(
        "rend",
        &["rend"],
        AbilityScoreType::Strength,
        Dice::new(2, 4),
        DamageType::Slashing,
        2,
        Dice::new(1, 6),
        DamageType::Acid,
        "rend",
    ),
    // Adult Black: 2d6 slashing, +1d8 acid, reach 10 ft.
    WeaponWithRider::reach_melee(
        "rend",
        &["rend"],
        AbilityScoreType::Strength,
        Dice::new(2, 6),
        DamageType::Slashing,
        2,
        Dice::new(1, 8),
        DamageType::Acid,
        "rend",
    ),
    // Ancient Black: 2d8 slashing, +2d8 acid, reach 15 ft.
    WeaponWithRider::reach_melee(
        "rend",
        &["rend"],
        AbilityScoreType::Strength,
        Dice::new(2, 8),
        DamageType::Slashing,
        3,
        Dice::new(2, 8),
        DamageType::Acid,
        "rend",
    ),
    // Wyrmling Blue: 1d10 slashing, +1d6 lightning, reach 5 ft.
    WeaponWithRider::reach_melee(
        "rend",
        &["rend"],
        AbilityScoreType::Strength,
        Dice::new(1, 10),
        DamageType::Slashing,
        1,
        Dice::new(1, 6),
        DamageType::Lightning,
        "rend",
    ),
    // Young Blue: 2d6 slashing, +1d10 lightning, reach 10 ft.
    WeaponWithRider::reach_melee(
        "rend",
        &["rend"],
        AbilityScoreType::Strength,
        Dice::new(2, 6),
        DamageType::Slashing,
        2,
        Dice::new(1, 10),
        DamageType::Lightning,
        "rend",
    ),
    // Adult Blue: 2d8 slashing, +1d10 lightning, reach 10 ft.
    WeaponWithRider::reach_melee(
        "rend",
        &["rend"],
        AbilityScoreType::Strength,
        Dice::new(2, 8),
        DamageType::Slashing,
        2,
        Dice::new(1, 10),
        DamageType::Lightning,
        "rend",
    ),
    // Ancient Blue: 2d8 slashing, +2d10 lightning, reach 15 ft.
    WeaponWithRider::reach_melee(
        "rend",
        &["rend"],
        AbilityScoreType::Strength,
        Dice::new(2, 8),
        DamageType::Slashing,
        3,
        Dice::new(2, 10),
        DamageType::Lightning,
        "rend",
    ),
    // Wyrmling Green: 1d10 slashing, +1d6 poison, reach 5 ft.
    WeaponWithRider::reach_melee(
        "rend",
        &["rend"],
        AbilityScoreType::Strength,
        Dice::new(1, 10),
        DamageType::Slashing,
        1,
        Dice::new(1, 6),
        DamageType::Poison,
        "rend",
    ),
    // Young Green: 2d6 slashing, +2d6 poison, reach 10 ft.
    WeaponWithRider::reach_melee(
        "rend",
        &["rend"],
        AbilityScoreType::Strength,
        Dice::new(2, 6),
        DamageType::Slashing,
        2,
        Dice::new(2, 6),
        DamageType::Poison,
        "rend",
    ),
    // Adult Green: 2d8 slashing, +2d6 poison, reach 10 ft.
    WeaponWithRider::reach_melee(
        "rend",
        &["rend"],
        AbilityScoreType::Strength,
        Dice::new(2, 8),
        DamageType::Slashing,
        2,
        Dice::new(2, 6),
        DamageType::Poison,
        "rend",
    ),
    // Ancient Green: 2d8 slashing, +3d6 poison, reach 15 ft.
    WeaponWithRider::reach_melee(
        "rend",
        &["rend"],
        AbilityScoreType::Strength,
        Dice::new(2, 8),
        DamageType::Slashing,
        3,
        Dice::new(3, 6),
        DamageType::Poison,
        "rend",
    ),
    // Wyrmling Red: 1d10 slashing, +1d6 fire, reach 5 ft.
    WeaponWithRider::reach_melee(
        "rend",
        &["rend"],
        AbilityScoreType::Strength,
        Dice::new(1, 10),
        DamageType::Slashing,
        1,
        Dice::new(1, 6),
        DamageType::Fire,
        "rend",
    ),
    // Young Red: 2d6 slashing, +1d6 fire, reach 10 ft.
    WeaponWithRider::reach_melee(
        "rend",
        &["rend"],
        AbilityScoreType::Strength,
        Dice::new(2, 6),
        DamageType::Slashing,
        2,
        Dice::new(1, 6),
        DamageType::Fire,
        "rend",
    ),
    // Adult Red: 1d10 slashing, +2d4 fire, reach 10 ft.
    WeaponWithRider::reach_melee(
        "rend",
        &["rend"],
        AbilityScoreType::Strength,
        Dice::new(1, 10),
        DamageType::Slashing,
        2,
        Dice::new(2, 4),
        DamageType::Fire,
        "rend",
    ),
    // Ancient Red: 2d8 slashing, +3d6 fire, reach 15 ft.
    WeaponWithRider::reach_melee(
        "rend",
        &["rend"],
        AbilityScoreType::Strength,
        Dice::new(2, 8),
        DamageType::Slashing,
        3,
        Dice::new(3, 6),
        DamageType::Fire,
        "rend",
    ),
    // Wyrmling White: 1d8 slashing, +1d4 cold, reach 5 ft.
    WeaponWithRider::reach_melee(
        "rend",
        &["rend"],
        AbilityScoreType::Strength,
        Dice::new(1, 8),
        DamageType::Slashing,
        1,
        Dice::new(1, 4),
        DamageType::Cold,
        "rend",
    ),
    // Young White: 2d4 slashing, +1d4 cold, reach 10 ft.
    WeaponWithRider::reach_melee(
        "rend",
        &["rend"],
        AbilityScoreType::Strength,
        Dice::new(2, 4),
        DamageType::Slashing,
        2,
        Dice::new(1, 4),
        DamageType::Cold,
        "rend",
    ),
    // Adult White: 2d6 slashing, +1d8 cold, reach 10 ft.
    WeaponWithRider::reach_melee(
        "rend",
        &["rend"],
        AbilityScoreType::Strength,
        Dice::new(2, 6),
        DamageType::Slashing,
        2,
        Dice::new(1, 8),
        DamageType::Cold,
        "rend",
    ),
    // Ancient White: 2d8 slashing, +2d6 cold, reach 15 ft.
    WeaponWithRider::reach_melee(
        "rend",
        &["rend"],
        AbilityScoreType::Strength,
        Dice::new(2, 8),
        DamageType::Slashing,
        3,
        Dice::new(2, 6),
        DamageType::Cold,
        "rend",
    ),
    // Wyrmling Brass: 1d10 slashing, no elemental rider on this rung, reach 5 ft.
    WeaponWithRider::reach_melee(
        "rend",
        &["rend"],
        AbilityScoreType::Strength,
        Dice::new(1, 10),
        DamageType::Slashing,
        1,
        Dice::new(0, 1),
        DamageType::Fire,
        "rend",
    ),
    // Young Brass: 2d10 slashing, no elemental rider on this rung, reach 10 ft.
    WeaponWithRider::reach_melee(
        "rend",
        &["rend"],
        AbilityScoreType::Strength,
        Dice::new(2, 10),
        DamageType::Slashing,
        2,
        Dice::new(0, 1),
        DamageType::Fire,
        "rend",
    ),
    // Adult Brass: 2d10 slashing, +1d8 fire, reach 10 ft.
    WeaponWithRider::reach_melee(
        "rend",
        &["rend"],
        AbilityScoreType::Strength,
        Dice::new(2, 10),
        DamageType::Slashing,
        2,
        Dice::new(1, 8),
        DamageType::Fire,
        "rend",
    ),
    // Ancient Brass: 2d10 slashing, +2d6 fire, reach 15 ft.
    WeaponWithRider::reach_melee(
        "rend",
        &["rend"],
        AbilityScoreType::Strength,
        Dice::new(2, 10),
        DamageType::Slashing,
        3,
        Dice::new(2, 6),
        DamageType::Fire,
        "rend",
    ),
    // Wyrmling Bronze: 1d10 slashing, no elemental rider on this rung, reach 5 ft.
    WeaponWithRider::reach_melee(
        "rend",
        &["rend"],
        AbilityScoreType::Strength,
        Dice::new(1, 10),
        DamageType::Slashing,
        1,
        Dice::new(0, 1),
        DamageType::Lightning,
        "rend",
    ),
    // Young Bronze: 2d10 slashing, no elemental rider on this rung, reach 10 ft.
    WeaponWithRider::reach_melee(
        "rend",
        &["rend"],
        AbilityScoreType::Strength,
        Dice::new(2, 10),
        DamageType::Slashing,
        2,
        Dice::new(0, 1),
        DamageType::Lightning,
        "rend",
    ),
    // Adult Bronze: 2d8 slashing, +1d10 lightning, reach 10 ft.
    WeaponWithRider::reach_melee(
        "rend",
        &["rend"],
        AbilityScoreType::Strength,
        Dice::new(2, 8),
        DamageType::Slashing,
        2,
        Dice::new(1, 10),
        DamageType::Lightning,
        "rend",
    ),
    // Ancient Bronze: 2d8 slashing, +2d8 lightning, reach 15 ft.
    WeaponWithRider::reach_melee(
        "rend",
        &["rend"],
        AbilityScoreType::Strength,
        Dice::new(2, 8),
        DamageType::Slashing,
        3,
        Dice::new(2, 8),
        DamageType::Lightning,
        "rend",
    ),
    // Wyrmling Copper: 1d10 slashing, no elemental rider on this rung, reach 5 ft.
    WeaponWithRider::reach_melee(
        "rend",
        &["rend"],
        AbilityScoreType::Strength,
        Dice::new(1, 10),
        DamageType::Slashing,
        1,
        Dice::new(0, 1),
        DamageType::Acid,
        "rend",
    ),
    // Young Copper: 2d10 slashing, no elemental rider on this rung, reach 10 ft.
    WeaponWithRider::reach_melee(
        "rend",
        &["rend"],
        AbilityScoreType::Strength,
        Dice::new(2, 10),
        DamageType::Slashing,
        2,
        Dice::new(0, 1),
        DamageType::Acid,
        "rend",
    ),
    // Adult Copper: 2d10 slashing, +1d8 acid, reach 10 ft.
    WeaponWithRider::reach_melee(
        "rend",
        &["rend"],
        AbilityScoreType::Strength,
        Dice::new(2, 10),
        DamageType::Slashing,
        2,
        Dice::new(1, 8),
        DamageType::Acid,
        "rend",
    ),
    // Ancient Copper: 2d10 slashing, +2d8 acid, reach 15 ft.
    WeaponWithRider::reach_melee(
        "rend",
        &["rend"],
        AbilityScoreType::Strength,
        Dice::new(2, 10),
        DamageType::Slashing,
        3,
        Dice::new(2, 8),
        DamageType::Acid,
        "rend",
    ),
    // Wyrmling Gold: 1d10 slashing, no elemental rider on this rung, reach 5 ft.
    WeaponWithRider::reach_melee(
        "rend",
        &["rend"],
        AbilityScoreType::Strength,
        Dice::new(1, 10),
        DamageType::Slashing,
        1,
        Dice::new(0, 1),
        DamageType::Fire,
        "rend",
    ),
    // Young Gold: 2d10 slashing, no elemental rider on this rung, reach 10 ft.
    WeaponWithRider::reach_melee(
        "rend",
        &["rend"],
        AbilityScoreType::Strength,
        Dice::new(2, 10),
        DamageType::Slashing,
        2,
        Dice::new(0, 1),
        DamageType::Fire,
        "rend",
    ),
    // Adult Gold: 2d8 slashing, +1d8 fire, reach 10 ft.
    WeaponWithRider::reach_melee(
        "rend",
        &["rend"],
        AbilityScoreType::Strength,
        Dice::new(2, 8),
        DamageType::Slashing,
        2,
        Dice::new(1, 8),
        DamageType::Fire,
        "rend",
    ),
    // Ancient Gold: 2d8 slashing, +2d8 fire, reach 15 ft.
    WeaponWithRider::reach_melee(
        "rend",
        &["rend"],
        AbilityScoreType::Strength,
        Dice::new(2, 8),
        DamageType::Slashing,
        3,
        Dice::new(2, 8),
        DamageType::Fire,
        "rend",
    ),
    // Wyrmling Silver: 1d10 piercing, no elemental rider on this rung, reach 5 ft.
    WeaponWithRider::reach_melee(
        "rend",
        &["rend"],
        AbilityScoreType::Strength,
        Dice::new(1, 10),
        DamageType::Piercing,
        1,
        Dice::new(0, 1),
        DamageType::Cold,
        "rend",
    ),
    // Young Silver: 2d8 slashing, no elemental rider on this rung, reach 10 ft.
    WeaponWithRider::reach_melee(
        "rend",
        &["rend"],
        AbilityScoreType::Strength,
        Dice::new(2, 8),
        DamageType::Slashing,
        2,
        Dice::new(0, 1),
        DamageType::Cold,
        "rend",
    ),
    // Adult Silver: 2d8 slashing, +1d8 cold, reach 10 ft.
    WeaponWithRider::reach_melee(
        "rend",
        &["rend"],
        AbilityScoreType::Strength,
        Dice::new(2, 8),
        DamageType::Slashing,
        2,
        Dice::new(1, 8),
        DamageType::Cold,
        "rend",
    ),
    // Ancient Silver: 2d8 slashing, +2d8 cold, reach 15 ft.
    WeaponWithRider::reach_melee(
        "rend",
        &["rend"],
        AbilityScoreType::Strength,
        Dice::new(2, 8),
        DamageType::Slashing,
        3,
        Dice::new(2, 8),
        DamageType::Cold,
        "rend",
    ),
];

/// One Multiattack per dragon, in `DRAGONS` order: two Rends for a
/// wyrmling and three for everything above it, each pointing at its own
/// row of `DRAGON_RENDS`.
static DRAGON_MULTIATTACKS: [Multiattack; 40] = [
    // Wyrmling Black: 2 Rends per Action.
    Multiattack {
        display_name: "dragon multiattack",
        sub_attack: &DRAGON_RENDS[0],
        count: 2,
    },
    // Young Black: 3 Rends per Action.
    Multiattack {
        display_name: "dragon multiattack",
        sub_attack: &DRAGON_RENDS[1],
        count: 3,
    },
    // Adult Black: 3 Rends per Action.
    Multiattack {
        display_name: "dragon multiattack",
        sub_attack: &DRAGON_RENDS[2],
        count: 3,
    },
    // Ancient Black: 3 Rends per Action.
    Multiattack {
        display_name: "dragon multiattack",
        sub_attack: &DRAGON_RENDS[3],
        count: 3,
    },
    // Wyrmling Blue: 2 Rends per Action.
    Multiattack {
        display_name: "dragon multiattack",
        sub_attack: &DRAGON_RENDS[4],
        count: 2,
    },
    // Young Blue: 3 Rends per Action.
    Multiattack {
        display_name: "dragon multiattack",
        sub_attack: &DRAGON_RENDS[5],
        count: 3,
    },
    // Adult Blue: 3 Rends per Action.
    Multiattack {
        display_name: "dragon multiattack",
        sub_attack: &DRAGON_RENDS[6],
        count: 3,
    },
    // Ancient Blue: 3 Rends per Action.
    Multiattack {
        display_name: "dragon multiattack",
        sub_attack: &DRAGON_RENDS[7],
        count: 3,
    },
    // Wyrmling Green: 2 Rends per Action.
    Multiattack {
        display_name: "dragon multiattack",
        sub_attack: &DRAGON_RENDS[8],
        count: 2,
    },
    // Young Green: 3 Rends per Action.
    Multiattack {
        display_name: "dragon multiattack",
        sub_attack: &DRAGON_RENDS[9],
        count: 3,
    },
    // Adult Green: 3 Rends per Action.
    Multiattack {
        display_name: "dragon multiattack",
        sub_attack: &DRAGON_RENDS[10],
        count: 3,
    },
    // Ancient Green: 3 Rends per Action.
    Multiattack {
        display_name: "dragon multiattack",
        sub_attack: &DRAGON_RENDS[11],
        count: 3,
    },
    // Wyrmling Red: 2 Rends per Action.
    Multiattack {
        display_name: "dragon multiattack",
        sub_attack: &DRAGON_RENDS[12],
        count: 2,
    },
    // Young Red: 3 Rends per Action.
    Multiattack {
        display_name: "dragon multiattack",
        sub_attack: &DRAGON_RENDS[13],
        count: 3,
    },
    // Adult Red: 3 Rends per Action.
    Multiattack {
        display_name: "dragon multiattack",
        sub_attack: &DRAGON_RENDS[14],
        count: 3,
    },
    // Ancient Red: 3 Rends per Action.
    Multiattack {
        display_name: "dragon multiattack",
        sub_attack: &DRAGON_RENDS[15],
        count: 3,
    },
    // Wyrmling White: 2 Rends per Action.
    Multiattack {
        display_name: "dragon multiattack",
        sub_attack: &DRAGON_RENDS[16],
        count: 2,
    },
    // Young White: 3 Rends per Action.
    Multiattack {
        display_name: "dragon multiattack",
        sub_attack: &DRAGON_RENDS[17],
        count: 3,
    },
    // Adult White: 3 Rends per Action.
    Multiattack {
        display_name: "dragon multiattack",
        sub_attack: &DRAGON_RENDS[18],
        count: 3,
    },
    // Ancient White: 3 Rends per Action.
    Multiattack {
        display_name: "dragon multiattack",
        sub_attack: &DRAGON_RENDS[19],
        count: 3,
    },
    // Wyrmling Brass: 2 Rends per Action.
    Multiattack {
        display_name: "dragon multiattack",
        sub_attack: &DRAGON_RENDS[20],
        count: 2,
    },
    // Young Brass: 3 Rends per Action.
    Multiattack {
        display_name: "dragon multiattack",
        sub_attack: &DRAGON_RENDS[21],
        count: 3,
    },
    // Adult Brass: 3 Rends per Action.
    Multiattack {
        display_name: "dragon multiattack",
        sub_attack: &DRAGON_RENDS[22],
        count: 3,
    },
    // Ancient Brass: 3 Rends per Action.
    Multiattack {
        display_name: "dragon multiattack",
        sub_attack: &DRAGON_RENDS[23],
        count: 3,
    },
    // Wyrmling Bronze: 2 Rends per Action.
    Multiattack {
        display_name: "dragon multiattack",
        sub_attack: &DRAGON_RENDS[24],
        count: 2,
    },
    // Young Bronze: 3 Rends per Action.
    Multiattack {
        display_name: "dragon multiattack",
        sub_attack: &DRAGON_RENDS[25],
        count: 3,
    },
    // Adult Bronze: 3 Rends per Action.
    Multiattack {
        display_name: "dragon multiattack",
        sub_attack: &DRAGON_RENDS[26],
        count: 3,
    },
    // Ancient Bronze: 3 Rends per Action.
    Multiattack {
        display_name: "dragon multiattack",
        sub_attack: &DRAGON_RENDS[27],
        count: 3,
    },
    // Wyrmling Copper: 2 Rends per Action.
    Multiattack {
        display_name: "dragon multiattack",
        sub_attack: &DRAGON_RENDS[28],
        count: 2,
    },
    // Young Copper: 3 Rends per Action.
    Multiattack {
        display_name: "dragon multiattack",
        sub_attack: &DRAGON_RENDS[29],
        count: 3,
    },
    // Adult Copper: 3 Rends per Action.
    Multiattack {
        display_name: "dragon multiattack",
        sub_attack: &DRAGON_RENDS[30],
        count: 3,
    },
    // Ancient Copper: 3 Rends per Action.
    Multiattack {
        display_name: "dragon multiattack",
        sub_attack: &DRAGON_RENDS[31],
        count: 3,
    },
    // Wyrmling Gold: 2 Rends per Action.
    Multiattack {
        display_name: "dragon multiattack",
        sub_attack: &DRAGON_RENDS[32],
        count: 2,
    },
    // Young Gold: 3 Rends per Action.
    Multiattack {
        display_name: "dragon multiattack",
        sub_attack: &DRAGON_RENDS[33],
        count: 3,
    },
    // Adult Gold: 3 Rends per Action.
    Multiattack {
        display_name: "dragon multiattack",
        sub_attack: &DRAGON_RENDS[34],
        count: 3,
    },
    // Ancient Gold: 3 Rends per Action.
    Multiattack {
        display_name: "dragon multiattack",
        sub_attack: &DRAGON_RENDS[35],
        count: 3,
    },
    // Wyrmling Silver: 2 Rends per Action.
    Multiattack {
        display_name: "dragon multiattack",
        sub_attack: &DRAGON_RENDS[36],
        count: 2,
    },
    // Young Silver: 3 Rends per Action.
    Multiattack {
        display_name: "dragon multiattack",
        sub_attack: &DRAGON_RENDS[37],
        count: 3,
    },
    // Adult Silver: 3 Rends per Action.
    Multiattack {
        display_name: "dragon multiattack",
        sub_attack: &DRAGON_RENDS[38],
        count: 3,
    },
    // Ancient Silver: 3 Rends per Action.
    Multiattack {
        display_name: "dragon multiattack",
        sub_attack: &DRAGON_RENDS[39],
        count: 3,
    },
];

/// Build the template for `DRAGONS[index]`.
///
/// The whole ladder in one function, which is the point: every question
/// that used to be answered differently by four hand-written literals —
/// is an adult Large or Huge, does it have Legendary Actions, is a
/// young dragon's breath on the same recharge — is answered once here,
/// off the age.
fn dragon_template(index: usize) -> CreatureTemplate {
    let row = &DRAGONS[index];
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&DRAGON_MULTIATTACKS[index]);
    actions.push(&DRAGON_RENDS[index]);
    actions.push(&DRAGON_BREATHS[index]);
    if row.age.is_legendary() {
        actions.push(&*FRIGHTFUL_PRESENCE);
    }
    let mut senses = HashSet::from([SpecialSense::Darkvision(row.darkvision)]);
    senses.insert(SpecialSense::Blindsight(row.blindsight));
    let mut features = HashSet::new();
    if row.swims {
        features.insert(SWIM_SPEED_TAG);
    }
    if row.color.amphibious() {
        features.insert(UNDERWATER_BREATHING_TAG);
    }
    let element = row.color.damage_type();
    let mut condition_immunities = HashSet::new();
    if row.age.is_legendary() {
        condition_immunities.insert(Condition::Frightened);
        condition_immunities.insert(Condition::Charmed);
    }
    // A creature immune to poison damage is immune to the Poisoned
    // condition — the two halves of one sentence, and the green dragon
    // is the only colour they apply to.
    if element == DamageType::Poison {
        condition_immunities.insert(Condition::Poisoned);
    }
    CreatureTemplate {
        name: DRAGON_NAMES[index],
        glyph: row.color.glyph(),
        ac: row.ac,
        hitpoints: row.hitpoints.parse().expect("dragon hit dice parse"),
        speed: row.speed,
        fly_speed: row.fly_speed,
        strength: row.abilities[0],
        dexterity: row.abilities[1],
        constitution: row.abilities[2],
        intelligence: row.abilities[3],
        wisdom: row.abilities[4],
        charisma: row.abilities[5],
        senses,
        languages: HashSet::from([Language::Common, Language::Draconic]),
        cr: row.cr,
        size: row.age.size(),
        creature_type: CreatureType::Dragon,
        actions,
        damage_modifiers: HashMap::from([(element, DamageModifier::Immunity)]),
        proficient_saves: DRAGON_LEGENDARY_SAVES.clone(),
        condition_immunities,
        legendary_resistances: row.age.legendary_resistances(),
        recharge_abilities: vec![("breath_weapon", 5)],
        legendary_actions_per_round: if row.age.is_legendary() { 3 } else { 0 },
        legendary_actions: if row.age.is_legendary() {
            crate::engine::legendary_actions::DRAGON_LEGENDARY
        } else {
            &[]
        },
        lair_actions: if row.age.is_legendary() {
            crate::engine::lair_actions::DRAGON_LAIR
        } else {
            &[]
        },
        features,
        ..CreatureTemplate::defaults()
    }
}

/// Stat block names, in `DRAGONS` order. A separate table rather than a
/// `format!` off the two enums because `CreatureTemplate::name` is a
/// `&'static str` and a formatted one would have to leak.
const DRAGON_NAMES: [&str; 40] = [
    "Black Dragon Wyrmling",
    "Young Black Dragon",
    "Adult Black Dragon",
    "Ancient Black Dragon",
    "Blue Dragon Wyrmling",
    "Young Blue Dragon",
    "Adult Blue Dragon",
    "Ancient Blue Dragon",
    "Green Dragon Wyrmling",
    "Young Green Dragon",
    "Adult Green Dragon",
    "Ancient Green Dragon",
    "Red Dragon Wyrmling",
    "Young Red Dragon",
    "Adult Red Dragon",
    "Ancient Red Dragon",
    "White Dragon Wyrmling",
    "Young White Dragon",
    "Adult White Dragon",
    "Ancient White Dragon",
    "Brass Dragon Wyrmling",
    "Young Brass Dragon",
    "Adult Brass Dragon",
    "Ancient Brass Dragon",
    "Bronze Dragon Wyrmling",
    "Young Bronze Dragon",
    "Adult Bronze Dragon",
    "Ancient Bronze Dragon",
    "Copper Dragon Wyrmling",
    "Young Copper Dragon",
    "Adult Copper Dragon",
    "Ancient Copper Dragon",
    "Gold Dragon Wyrmling",
    "Young Gold Dragon",
    "Adult Gold Dragon",
    "Ancient Gold Dragon",
    "Silver Dragon Wyrmling",
    "Young Silver Dragon",
    "Adult Silver Dragon",
    "Ancient Silver Dragon",
];

/// Black Dragon Wyrmling — CR 2, acid breath, Medium.
pub static BLACK_DRAGON_WYRMLING_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| dragon_template(0));
/// Young Black Dragon — CR 7, acid breath, Large.
pub static YOUNG_BLACK_DRAGON_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| dragon_template(1));
/// Adult Black Dragon — CR 14, acid breath, Huge.
pub static ADULT_BLACK_DRAGON_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| dragon_template(2));
/// Ancient Black Dragon — CR 21, acid breath, Gargantuan.
pub static ANCIENT_BLACK_DRAGON_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| dragon_template(3));
/// Blue Dragon Wyrmling — CR 3, lightning breath, Medium.
pub static BLUE_DRAGON_WYRMLING_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| dragon_template(4));
/// Young Blue Dragon — CR 9, lightning breath, Large.
pub static YOUNG_BLUE_DRAGON_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| dragon_template(5));
/// Adult Blue Dragon — CR 16, lightning breath, Huge.
pub static ADULT_BLUE_DRAGON_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| dragon_template(6));
/// Ancient Blue Dragon — CR 23, lightning breath, Gargantuan.
pub static ANCIENT_BLUE_DRAGON_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| dragon_template(7));
/// Green Dragon Wyrmling — CR 2, poison breath, Medium.
pub static GREEN_DRAGON_WYRMLING_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| dragon_template(8));
/// Young Green Dragon — CR 8, poison breath, Large.
pub static YOUNG_GREEN_DRAGON_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| dragon_template(9));
/// Adult Green Dragon — CR 15, poison breath, Huge.
pub static ADULT_GREEN_DRAGON_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| dragon_template(10));
/// Ancient Green Dragon — CR 22, poison breath, Gargantuan.
pub static ANCIENT_GREEN_DRAGON_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| dragon_template(11));
/// Red Dragon Wyrmling — CR 4, fire breath, Medium.
pub static RED_DRAGON_WYRMLING_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| dragon_template(12));
/// Young Red Dragon — CR 10, fire breath, Large.
pub static YOUNG_RED_DRAGON_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| dragon_template(13));
/// Adult Red Dragon — CR 17, fire breath, Huge.
pub static ADULT_RED_DRAGON_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| dragon_template(14));
/// Ancient Red Dragon — CR 24, fire breath, Gargantuan.
pub static ANCIENT_RED_DRAGON_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| dragon_template(15));
/// White Dragon Wyrmling — CR 2, cold breath, Medium.
pub static WHITE_DRAGON_WYRMLING_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| dragon_template(16));
/// Young White Dragon — CR 6, cold breath, Large.
pub static YOUNG_WHITE_DRAGON_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| dragon_template(17));
/// Adult White Dragon — CR 13, cold breath, Huge.
pub static ADULT_WHITE_DRAGON_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| dragon_template(18));
/// Ancient White Dragon — CR 20, cold breath, Gargantuan.
pub static ANCIENT_WHITE_DRAGON_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| dragon_template(19));
/// Brass Dragon Wyrmling — CR 1, fire breath, Medium.
pub static BRASS_DRAGON_WYRMLING_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| dragon_template(20));
/// Young Brass Dragon — CR 6, fire breath, Large.
pub static YOUNG_BRASS_DRAGON_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| dragon_template(21));
/// Adult Brass Dragon — CR 13, fire breath, Huge.
pub static ADULT_BRASS_DRAGON_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| dragon_template(22));
/// Ancient Brass Dragon — CR 20, fire breath, Gargantuan.
pub static ANCIENT_BRASS_DRAGON_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| dragon_template(23));
/// Bronze Dragon Wyrmling — CR 2, lightning breath, Medium.
pub static BRONZE_DRAGON_WYRMLING_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| dragon_template(24));
/// Young Bronze Dragon — CR 8, lightning breath, Large.
pub static YOUNG_BRONZE_DRAGON_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| dragon_template(25));
/// Adult Bronze Dragon — CR 15, lightning breath, Huge.
pub static ADULT_BRONZE_DRAGON_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| dragon_template(26));
/// Ancient Bronze Dragon — CR 22, lightning breath, Gargantuan.
pub static ANCIENT_BRONZE_DRAGON_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| dragon_template(27));
/// Copper Dragon Wyrmling — CR 1, acid breath, Medium.
pub static COPPER_DRAGON_WYRMLING_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| dragon_template(28));
/// Young Copper Dragon — CR 7, acid breath, Large.
pub static YOUNG_COPPER_DRAGON_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| dragon_template(29));
/// Adult Copper Dragon — CR 14, acid breath, Huge.
pub static ADULT_COPPER_DRAGON_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| dragon_template(30));
/// Ancient Copper Dragon — CR 21, acid breath, Gargantuan.
pub static ANCIENT_COPPER_DRAGON_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| dragon_template(31));
/// Gold Dragon Wyrmling — CR 3, fire breath, Medium.
pub static GOLD_DRAGON_WYRMLING_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| dragon_template(32));
/// Young Gold Dragon — CR 10, fire breath, Large.
pub static YOUNG_GOLD_DRAGON_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| dragon_template(33));
/// Adult Gold Dragon — CR 17, fire breath, Huge.
pub static ADULT_GOLD_DRAGON_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| dragon_template(34));
/// Ancient Gold Dragon — CR 24, fire breath, Gargantuan.
pub static ANCIENT_GOLD_DRAGON_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| dragon_template(35));
/// Silver Dragon Wyrmling — CR 2, cold breath, Medium.
pub static SILVER_DRAGON_WYRMLING_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| dragon_template(36));
/// Young Silver Dragon — CR 9, cold breath, Large.
pub static YOUNG_SILVER_DRAGON_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| dragon_template(37));
/// Adult Silver Dragon — CR 16, cold breath, Huge.
pub static ADULT_SILVER_DRAGON_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| dragon_template(38));
/// Ancient Silver Dragon — CR 23, cold breath, Gargantuan.
pub static ANCIENT_SILVER_DRAGON_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| dragon_template(39));

/// The dragons whose SRD stat block carries a swimming speed — every
/// black, bronze, gold, green and white, at all four rungs.
///
/// Derived rather than written down, which is the one place the dragon
/// ladder can do better than `creatures::aquatic_templates`' hand-kept
/// list. That list exists because "is this thing a swimmer?" is a fact
/// about the monster manual with nothing on the template to derive it
/// from; here the fact *is* on the template — `DragonRow::swims`, read
/// off the Speed line with everything else — so a twenty-name list
/// would be a second copy of it that could drift.
pub fn swimming_dragon_templates() -> Vec<&'static CreatureTemplate> {
    all_dragon_templates()
        .into_iter()
        .zip(DRAGONS.iter())
        .filter(|(_, row)| row.swims)
        .map(|(template, _)| template)
        .collect()
}

/// The dragons whose SRD stat block prints **Amphibious** — every
/// black, bronze, gold and green, at all four rungs.
///
/// Derived from `DragonColor::amphibious` for the same reason
/// `swimming_dragon_templates` is derived from `DragonRow::swims`: the
/// fact is already on the template and a sixteen-name list would be a
/// second copy of it that could drift.
///
/// Deliberately not the same set as the swimmers. The white dragon has
/// a swim speed and no Amphibious trait, so it is on that list and not
/// on this one — it crosses a lake for free and drowns under it.
pub fn amphibious_dragon_templates() -> Vec<&'static CreatureTemplate> {
    all_dragon_templates()
        .into_iter()
        .zip(DRAGONS.iter())
        .filter(|(_, row)| row.color.amphibious())
        .map(|(template, _)| template)
        .collect()
}

/// Every dragon stat block, for `EncounterInstance::template_pool` and
/// for the sweeps that want to assert something about all forty at
/// once.
pub fn all_dragon_templates() -> [&'static CreatureTemplate; 40] {
    [
        &BLACK_DRAGON_WYRMLING_TEMPLATE,
        &YOUNG_BLACK_DRAGON_TEMPLATE,
        &ADULT_BLACK_DRAGON_TEMPLATE,
        &ANCIENT_BLACK_DRAGON_TEMPLATE,
        &BLUE_DRAGON_WYRMLING_TEMPLATE,
        &YOUNG_BLUE_DRAGON_TEMPLATE,
        &ADULT_BLUE_DRAGON_TEMPLATE,
        &ANCIENT_BLUE_DRAGON_TEMPLATE,
        &GREEN_DRAGON_WYRMLING_TEMPLATE,
        &YOUNG_GREEN_DRAGON_TEMPLATE,
        &ADULT_GREEN_DRAGON_TEMPLATE,
        &ANCIENT_GREEN_DRAGON_TEMPLATE,
        &RED_DRAGON_WYRMLING_TEMPLATE,
        &YOUNG_RED_DRAGON_TEMPLATE,
        &ADULT_RED_DRAGON_TEMPLATE,
        &ANCIENT_RED_DRAGON_TEMPLATE,
        &WHITE_DRAGON_WYRMLING_TEMPLATE,
        &YOUNG_WHITE_DRAGON_TEMPLATE,
        &ADULT_WHITE_DRAGON_TEMPLATE,
        &ANCIENT_WHITE_DRAGON_TEMPLATE,
        &BRASS_DRAGON_WYRMLING_TEMPLATE,
        &YOUNG_BRASS_DRAGON_TEMPLATE,
        &ADULT_BRASS_DRAGON_TEMPLATE,
        &ANCIENT_BRASS_DRAGON_TEMPLATE,
        &BRONZE_DRAGON_WYRMLING_TEMPLATE,
        &YOUNG_BRONZE_DRAGON_TEMPLATE,
        &ADULT_BRONZE_DRAGON_TEMPLATE,
        &ANCIENT_BRONZE_DRAGON_TEMPLATE,
        &COPPER_DRAGON_WYRMLING_TEMPLATE,
        &YOUNG_COPPER_DRAGON_TEMPLATE,
        &ADULT_COPPER_DRAGON_TEMPLATE,
        &ANCIENT_COPPER_DRAGON_TEMPLATE,
        &GOLD_DRAGON_WYRMLING_TEMPLATE,
        &YOUNG_GOLD_DRAGON_TEMPLATE,
        &ADULT_GOLD_DRAGON_TEMPLATE,
        &ANCIENT_GOLD_DRAGON_TEMPLATE,
        &SILVER_DRAGON_WYRMLING_TEMPLATE,
        &YOUNG_SILVER_DRAGON_TEMPLATE,
        &ADULT_SILVER_DRAGON_TEMPLATE,
        &ANCIENT_SILVER_DRAGON_TEMPLATE,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actions::action_template::Action;
    use crate::actors::actor_template::ActorInstance;
    use crate::engine::dice::FastRandRoller;
    use crate::engine::types::Coordinate;

    fn make(template: &'static CreatureTemplate) -> ActorInstance {
        ActorInstance::from_creature_template(
            template,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    const COLORS: [DragonColor; 10] = [
        DragonColor::Black,
        DragonColor::Blue,
        DragonColor::Brass,
        DragonColor::Bronze,
        DragonColor::Copper,
        DragonColor::Gold,
        DragonColor::Green,
        DragonColor::Red,
        DragonColor::Silver,
        DragonColor::White,
    ];
    const AGES: [DragonAge; 4] = [
        DragonAge::Wyrmling,
        DragonAge::Young,
        DragonAge::Adult,
        DragonAge::Ancient,
    ];

    /// Every cell of the ten-by-four grid is filled exactly once, and
    /// no two stat blocks share a name.
    ///
    /// The one assertion a table-driven roster needs that a
    /// hand-written one does not: a transposed row is invisible in a
    /// literal of forty entries, and it would show up here as a colour
    /// with five rungs and its neighbour with three.
    #[test]
    fn the_ladder_is_ten_colours_wide_and_four_rungs_tall() {
        assert_eq!(DRAGONS.len(), 40);
        assert_eq!(DRAGON_NAMES.len(), 40);
        assert_eq!(DRAGON_BREATHS.len(), 40);
        assert_eq!(DRAGON_RENDS.len(), 40);
        assert_eq!(DRAGON_MULTIATTACKS.len(), 40);
        for color in COLORS {
            for age in AGES {
                let found = DRAGONS
                    .iter()
                    .filter(|r| r.color == color && r.age == age)
                    .count();
                assert_eq!(found, 1, "{:?} {:?} appears {} times", age, color, found);
            }
        }
        let mut names: Vec<&str> = DRAGON_NAMES.to_vec();
        names.sort_unstable();
        let before = names.len();
        names.dedup();
        assert_eq!(names.len(), before, "two dragons share a name");
    }

    /// A colour is one damage type wearing four sizes: the breath, the
    /// Rend's rider and the damage immunity all name it.
    ///
    /// This is the invariant the table exists to make true. Written as
    /// forty literals it is forty chances to give a bronze dragon an
    /// acid breath over lightning immunity, and nothing would catch it.
    #[test]
    fn a_dragons_breath_its_rider_and_its_immunity_are_one_element() {
        for (i, row) in DRAGONS.iter().enumerate() {
            let element = row.color.damage_type();
            assert_eq!(
                DRAGON_BREATHS[i].damage.map(|(_, dt)| dt),
                Some(element),
                "{} breathes the wrong element",
                DRAGON_NAMES[i]
            );
            assert_eq!(
                DRAGON_RENDS[i].rider_type, element,
                "{}'s rend carries the wrong element",
                DRAGON_NAMES[i]
            );
            let template = all_dragon_templates()[i];
            assert_eq!(
                template.damage_modifiers.get(&element),
                Some(&DamageModifier::Immunity),
                "{} is not immune to what it breathes",
                DRAGON_NAMES[i]
            );
        }
    }

    /// The rung decides the kit, and it decides it the same way for
    /// every colour: a wyrmling and a young dragon are monsters, an
    /// adult and an ancient are boss fights.
    ///
    /// All four of the hand-written templates this ladder replaced
    /// disagreed with each other about some part of this — whether an
    /// adult has Legendary Actions, whether it has one Legendary
    /// Resistance or three, whether it is Large or Huge.
    #[test]
    fn the_rung_decides_the_kit_and_decides_it_the_same_way_every_time() {
        for (i, row) in DRAGONS.iter().enumerate() {
            let t = all_dragon_templates()[i];
            let name = DRAGON_NAMES[i];
            let expected_size = match row.age {
                DragonAge::Wyrmling => Size::Medium,
                DragonAge::Young => Size::Large,
                DragonAge::Adult => Size::Huge,
                DragonAge::Ancient => Size::Gargantuan,
            };
            assert_eq!(t.size, expected_size, "{} is the wrong size", name);
            let legendary = row.age.is_legendary();
            assert_eq!(
                t.legendary_actions_per_round > 0,
                legendary,
                "{} disagrees with its rung about legendary actions",
                name
            );
            assert_eq!(
                !t.lair_actions.is_empty(),
                legendary,
                "{} disagrees with its rung about having a lair",
                name
            );
            assert_eq!(
                t.legendary_resistances > 0,
                legendary,
                "{} disagrees with its rung about legendary resistance",
                name
            );
            // SRD 5.2 took Magic Resistance off every dragon in the
            // book; two of the four hand-written templates still had it.
            assert!(!t.has_magic_resistance, "{} should not resist magic", name);
            // The Multiattack is the Action; a second encoding of the
            // same rule as `has_extra_attack` is what the ladder
            // dropped.
            assert!(!t.has_extra_attack, "{} double-counts its swings", name);
        }
    }

    /// Every dragon can actually be built, carries the three actions
    /// its rung gives it, and gets its Frightful Presence exactly when
    /// it is legendary.
    #[test]
    fn every_dragon_instantiates_with_the_actions_its_rung_gives_it() {
        for (i, row) in DRAGONS.iter().enumerate() {
            let a = make(all_dragon_templates()[i]);
            let name = DRAGON_NAMES[i];
            assert!(a.hitpoints() > 0, "{} rolled no hit points", name);
            assert!(
                a.find_action("dragon multiattack").is_some(),
                "{} has no Multiattack",
                name
            );
            assert!(a.find_action("rend").is_some(), "{} has no Rend", name);
            assert!(
                a.find_action(DRAGON_BREATHS[i].name()).is_some(),
                "{} has no breath",
                name
            );
            assert_eq!(
                a.find_action("frightful presence").is_some(),
                row.age.is_legendary(),
                "{} disagrees with its rung about Frightful Presence",
                name
            );
        }
    }

    /// Challenge rating rises with age inside every colour, and the
    /// ladder spans the range the encounter generator needs it to: a
    /// brass wyrmling is a CR-1 fight and an ancient gold is CR 24.
    #[test]
    fn each_colour_gets_harder_with_age() {
        for color in COLORS {
            let mut last = 0.0f32;
            for age in AGES {
                let row = DRAGONS
                    .iter()
                    .find(|r| r.color == color && r.age == age)
                    .expect("every cell is filled");
                assert!(
                    row.cr > last,
                    "{:?} {:?} is CR {} but its younger sibling was CR {}",
                    age,
                    color,
                    row.cr,
                    last
                );
                last = row.cr;
            }
        }
        let crs: Vec<f32> = DRAGONS.iter().map(|r| r.cr).collect();
        assert_eq!(crs.iter().cloned().fold(f32::INFINITY, f32::min), 1.0);
        assert_eq!(crs.iter().cloned().fold(0.0f32, f32::max), 24.0);
    }

    /// The breath reaches further with every rung and never shorter —
    /// the one thing the ft-to-tile conversion could have got wrong,
    /// since RAW's lengths are not monotone *across* colours (a bronze
    /// wyrmling's 40-ft line outreaches an adult gold's 60-ft cone at
    /// its widest).
    #[test]
    fn a_dragons_breath_reaches_further_as_it_ages() {
        for color in COLORS {
            let mut last = 0isize;
            for age in AGES {
                let i = DRAGONS
                    .iter()
                    .position(|r| r.color == color && r.age == age)
                    .expect("every cell is filled");
                let length = DRAGON_BREATHS[i]
                    .shape
                    .aim_reach()
                    .expect("every dragon breath is a cone or a line");
                assert!(
                    length >= last,
                    "{:?} {:?} breathes shorter than its younger self",
                    age,
                    color
                );
                last = length;
            }
        }
    }

    /// A dragon's breath is the shape its stat block prints, and the
    /// colour wheel splits cleanly down the middle: black, blue, brass,
    /// bronze and copper breathe a line, and green, red, white, gold
    /// and silver breathe a cone. That split is the whole reason the
    /// engine grew a second area shape — under the old model these
    /// were one creature at four sizes — so it is pinned rather than
    /// left to the forty comments above.
    #[test]
    fn the_colour_wheel_splits_into_line_breathers_and_cone_breathers() {
        use crate::engine::areas::AreaShape;
        let mut lines: Vec<DragonColor> = Vec::new();
        let mut cones: Vec<DragonColor> = Vec::new();
        for color in COLORS {
            let mut kinds = DRAGONS
                .iter()
                .enumerate()
                .filter(|(_, r)| r.color == color)
                .map(|(i, _)| DRAGON_BREATHS[i].shape);
            let first = kinds.next().expect("every colour has four rungs");
            assert!(
                kinds.all(|k| std::mem::discriminant(&k) == std::mem::discriminant(&first)),
                "{color:?} changes breath shape as it ages"
            );
            match first {
                AreaShape::Line { .. } => lines.push(color),
                AreaShape::Cone { .. } => cones.push(color),
                AreaShape::Burst { .. } => panic!("{color:?} breathes a sphere"),
            }
        }
        // Sorted so the assertion pins the *membership* of each half
        // rather than the order `COLORS` happens to be written in.
        lines.sort_by_key(|c| format!("{c:?}"));
        cones.sort_by_key(|c| format!("{c:?}"));
        assert_eq!(
            lines,
            vec![
                DragonColor::Black,
                DragonColor::Blue,
                DragonColor::Brass,
                DragonColor::Bronze,
                DragonColor::Copper,
            ]
        );
        assert_eq!(
            cones,
            vec![
                DragonColor::Gold,
                DragonColor::Green,
                DragonColor::Red,
                DragonColor::Silver,
                DragonColor::White,
            ]
        );
    }

    /// Only the metallic wyrmlings and young dragons swing without an
    /// elemental rider, and they say so with a zero die rather than
    /// with a second chassis — see `add_flat_damage_rider`, which reads
    /// that as "no rider" and skips it.
    #[test]
    fn the_riderless_rends_are_exactly_the_young_metallics() {
        let riderless: Vec<&str> = DRAGONS
            .iter()
            .enumerate()
            .filter(|(i, _)| DRAGON_RENDS[*i].rider_dice.count == 0)
            .map(|(i, _)| DRAGON_NAMES[i])
            .collect();
        assert_eq!(
            riderless,
            vec![
                "Brass Dragon Wyrmling",
                "Young Brass Dragon",
                "Bronze Dragon Wyrmling",
                "Young Bronze Dragon",
                "Copper Dragon Wyrmling",
                "Young Copper Dragon",
                "Gold Dragon Wyrmling",
                "Young Gold Dragon",
                "Silver Dragon Wyrmling",
                "Young Silver Dragon",
            ]
        );
    }
}
