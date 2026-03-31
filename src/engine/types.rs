use std::fmt;

#[derive(Clone, PartialEq)]
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

#[derive(Clone, PartialEq)]
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

#[derive(Clone, PartialEq, Copy)]
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

#[derive(Debug, Clone)]
pub struct RngTryError;

impl fmt::Display for RngTryError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "exceede max tries for rng")
    }
}

impl std::error::Error for RngTryError {}

#[derive(Debug, Clone)]
pub struct NoLegalPosition;

impl fmt::Display for NoLegalPosition {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "could not find a legal position")
    }
}

impl std::error::Error for NoLegalPosition {}
