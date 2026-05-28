use std::fmt;
use std::ops::{Add, Sub};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AbilityScoreType {
    Strength,
    Dexterity,
    Constitution,
    Intelligence,
    Wisdom,
    Charisma,
}

impl fmt::Display for AbilityScoreType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AbilityScoreType::Strength => write!(f, "STR"),
            AbilityScoreType::Dexterity => write!(f, "DEX"),
            AbilityScoreType::Constitution => write!(f, "CON"),
            AbilityScoreType::Intelligence => write!(f, "INT"),
            AbilityScoreType::Wisdom => write!(f, "WIS"),
            AbilityScoreType::Charisma => write!(f, "CHA"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Hash, Eq)]
pub enum Skill {
    Acrobatics,
    AnimalHandling,
    Arcana,
    Athletics,
    Deception,
    History,
    Insight,
    Intimidation,
    Investigation,
    Medicine,
    Nature,
    Perception,
    Performance,
    Persuasion,
    Religion,
    SleightOfHand,
    Stealth,
    Survival,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DamageType {
    Acid,
    Bludgeoning,
    Cold,
    Fire,
    Force,
    Lightning,
    Necrotic,
    Piercing,
    Poison,
    Psychic,
    Radiant,
    Slashing,
    Thunder,
}

impl fmt::Display for DamageType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            DamageType::Acid => write!(f, "acid"),
            DamageType::Bludgeoning => write!(f, "bludgeoning"),
            DamageType::Cold => write!(f, "cold"),
            DamageType::Fire => write!(f, "fire"),
            DamageType::Force => write!(f, "force"),
            DamageType::Lightning => write!(f, "lightning"),
            DamageType::Necrotic => write!(f, "necrotic"),
            DamageType::Piercing => write!(f, "piercing"),
            DamageType::Poison => write!(f, "poison"),
            DamageType::Psychic => write!(f, "psychic"),
            DamageType::Radiant => write!(f, "radiant"),
            DamageType::Slashing => write!(f, "slashing"),
            DamageType::Thunder => write!(f, "thunder"),
        }
    }
}

/// 5e damage modifier categories for a creature against a damage type.
/// Resistance halves incoming damage, immunity nullifies it, vulnerability
/// doubles it. A creature can declare any subset across damage types via
/// `CreatureTemplate.damage_modifiers`. Stacking rules (5e):
/// - Immunity wins over everything else.
/// - Resistance and vulnerability of the same type cancel (we follow this
///   by simply not allowing both at once on the same template).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DamageModifier {
    Resistance,
    Immunity,
    Vulnerability,
}

impl DamageModifier {
    pub fn apply(self, raw: u32) -> u32 {
        match self {
            DamageModifier::Resistance => raw / 2,
            DamageModifier::Immunity => 0,
            DamageModifier::Vulnerability => raw.saturating_mul(2),
        }
    }
}

impl fmt::Display for DamageModifier {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            DamageModifier::Resistance => write!(f, "resistant"),
            DamageModifier::Immunity => write!(f, "immune"),
            DamageModifier::Vulnerability => write!(f, "vulnerable"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Size {
    Tiny,
    Small,
    Medium,
    Large,
    Huge,
    Gargantuan,
}

impl Size {
    /// Numeric ordering for size comparison. Tiny=0, Small=1, ..., Gargantuan=5.
    pub fn ordinal(self) -> i32 {
        match self {
            Size::Tiny => 0,
            Size::Small => 1,
            Size::Medium => 2,
            Size::Large => 3,
            Size::Huge => 4,
            Size::Gargantuan => 5,
        }
    }

    /// 5e grapple / shove gate: the target must be no more than one size
    /// category larger than the attacker. E.g. a Medium creature can
    /// grapple up to Large, but not Huge.
    pub fn can_grapple(self, target: Size) -> bool {
        target.ordinal() <= self.ordinal() + 1
    }
}

impl fmt::Display for Size {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Size::Tiny => write!(f, "Tiny"),
            Size::Small => write!(f, "Small"),
            Size::Medium => write!(f, "Medium"),
            Size::Large => write!(f, "Large"),
            Size::Huge => write!(f, "Huge"),
            Size::Gargantuan => write!(f, "Gargantuan"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CreatureType {
    Aberration,
    Beast,
    Celestial,
    Construct,
    Dragon,
    Elemental,
    Fey,
    Fiend,
    Giant,
    Humanoid,
    Monstrosity,
    Ooze,
    Plant,
    Undead,
}

impl fmt::Display for CreatureType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            CreatureType::Aberration => write!(f, "Aberration"),
            CreatureType::Beast => write!(f, "Beast"),
            CreatureType::Celestial => write!(f, "Celestial"),
            CreatureType::Construct => write!(f, "Construct"),
            CreatureType::Dragon => write!(f, "Dragon"),
            CreatureType::Elemental => write!(f, "Elemental"),
            CreatureType::Fey => write!(f, "Fey"),
            CreatureType::Fiend => write!(f, "Fiend"),
            CreatureType::Giant => write!(f, "Giant"),
            CreatureType::Humanoid => write!(f, "Humanoid"),
            CreatureType::Monstrosity => write!(f, "Monstrosity"),
            CreatureType::Ooze => write!(f, "Ooze"),
            CreatureType::Plant => write!(f, "Plant"),
            CreatureType::Undead => write!(f, "Undead"),
        }
    }
}

impl CreatureType {
    /// True if Protection from Evil and Good affects this creature type.
    /// 5e: aberrations, celestials, elementals, fey, fiends, undead.
    pub fn affected_by_protection(&self) -> bool {
        matches!(
            self,
            CreatureType::Aberration
                | CreatureType::Celestial
                | CreatureType::Elemental
                | CreatureType::Fey
                | CreatureType::Fiend
                | CreatureType::Undead
        )
    }

    /// True if Turn Undead affects this creature type.
    pub fn is_undead(&self) -> bool {
        matches!(self, CreatureType::Undead)
    }
}

#[derive(Clone, PartialEq, Hash, Eq)]
pub enum Language {
    Common,
    CommonSignLanguage,
    Draconic,
    Dwarvish,
    Elvish,
    Giant,
    Gnomish,
    Goblin,
    Halfling,
    Orc,
    Abyssal,
    Celestial,
    DeepSpeech,
    Druidic,
    Infernal,
    Primordial, //Aquan, Auran, Ignan, Terran
    Sylvan,
    ThievesCant,
    Undercommon,
}

#[derive(Clone, PartialEq, Hash, Eq)]
pub enum SpecialSense {
    Blindsight(u32),
    Darkvision(u32),
    Tremorsense(u32),
    Truesight(u32),
}

#[derive(Debug, Clone, PartialEq, Hash, Eq, Copy)]
pub struct Coordinate {
    pub x: isize,
    pub y: isize,
}

impl Coordinate {
    pub fn new(x: isize, y: isize) -> Self {
        Self { x, y }
    }

    /// Chebyshev (chessboard) distance between two single-tile points.
    /// Used for quick range checks where footprint size doesn't matter.
    pub fn chebyshev_to(self, other: Self) -> isize {
        (self.x - other.x).abs().max((self.y - other.y).abs())
    }
}

impl Add for Coordinate {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

impl Sub for Coordinate {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

impl fmt::Display for Coordinate {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "({}, {})", self.x, self.y)
    }
}
