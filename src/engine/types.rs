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

#[derive(Clone, PartialEq, Hash, Eq)]
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
    SlightOfHand,
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

/// How an actor's body responds to a particular damage type. 5e:
/// `Resistant` halves incoming damage, `Vulnerable` doubles it,
/// `Immune` zeroes it out. A type with no modifier listed takes
/// damage at face value. If both Resistant and Vulnerable are listed,
/// 5e RAW: they cancel — but we keep one explicit modifier per type
/// for simplicity, so the template author chooses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DamageMod {
    Resistant,
    Vulnerable,
    Immune,
}

impl DamageMod {
    /// Apply the modifier to a raw damage amount. `Resistant` halves
    /// (rounding down per 5e), `Vulnerable` doubles, `Immune` zeroes.
    pub fn apply(self, amount: u32) -> u32 {
        match self {
            DamageMod::Resistant => amount / 2,
            DamageMod::Vulnerable => amount.saturating_mul(2),
            DamageMod::Immune => 0,
        }
    }

    /// Short suffix for log lines (e.g. " (resisted)") so the player can
    /// see why a hit landed for less / more / nothing.
    pub fn log_suffix(self) -> &'static str {
        match self {
            DamageMod::Resistant => " (resisted)",
            DamageMod::Vulnerable => " (vulnerable)",
            DamageMod::Immune => " (immune)",
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
