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

/// How an actor's body responds to a given damage type. Resistance halves
/// damage (rounded down), Vulnerability doubles it, Immunity zeros it.
/// Default for unlisted types is Normal. Multiple sources don't stack
/// per 5e RAW — there's only one reaction per type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DamageReaction {
    Normal,
    Resistant,
    Vulnerable,
    Immune,
}

impl DamageReaction {
    /// Apply this reaction to a raw damage amount. Resistance halves
    /// (5e: rounded down, with a 1-damage floor only when raw was nonzero
    /// — RAW actually allows resisting to 0, so we honor that).
    pub fn apply(self, raw: u32) -> u32 {
        match self {
            DamageReaction::Normal => raw,
            DamageReaction::Resistant => raw / 2,
            DamageReaction::Vulnerable => raw.saturating_mul(2),
            DamageReaction::Immune => 0,
        }
    }

    /// Short tag for log lines ("(resisted)" / "(vulnerable)" / "(immune)").
    /// Empty string for Normal so the common case stays terse.
    pub fn log_tag(self) -> &'static str {
        match self {
            DamageReaction::Normal => "",
            DamageReaction::Resistant => " (resisted)",
            DamageReaction::Vulnerable => " (vulnerable)",
            DamageReaction::Immune => " (immune)",
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
