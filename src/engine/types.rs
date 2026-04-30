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

/// How an actor's body interacts with a given damage type. Resolved per
/// `(actor, damage_type)` from the actor's resistance / immunity / vulnerability
/// sets and applied to the dealt amount before HP is reduced. `Immune` short-
/// circuits the damage entirely (and any dependent side-effects, like
/// concentration checks); `Resistant` halves (round down per 5e); `Vulnerable`
/// doubles. Multiple sources of the same direction don't stack.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DamageModifier {
    Normal,
    Resistant,
    Immune,
    Vulnerable,
}

impl DamageModifier {
    /// Apply this modifier to a raw damage amount. Resistance halves
    /// (5e: round down via integer divide), immunity zeros, vulnerability
    /// doubles (saturating).
    pub fn apply(self, amount: u32) -> u32 {
        match self {
            DamageModifier::Normal => amount,
            DamageModifier::Resistant => amount / 2,
            DamageModifier::Immune => 0,
            DamageModifier::Vulnerable => amount.saturating_mul(2),
        }
    }

    /// Short tag for log lines so the player can see *why* the damage
    /// number changed. Empty for `Normal` so the common case stays quiet.
    pub fn log_suffix(self) -> &'static str {
        match self {
            DamageModifier::Normal => "",
            DamageModifier::Resistant => " (resisted)",
            DamageModifier::Immune => " (immune)",
            DamageModifier::Vulnerable => " (vulnerable)",
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
