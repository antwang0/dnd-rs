use tyche::Expr;
use crate::{actions::action_template::Action, units::unit_template::ActorInstance};

use std::collections::HashSet;

#[derive(Clone, PartialEq)]
pub enum AbilityScoreType {
    Strength,
    Dexterity,
    Constitution,
    Intelligence,
    Wisdom,
    Charisma
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
    Survival
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
    Thunder
}

pub struct Damage<'a> {
    source: &'a mut ActorInstance,
    target: &'a mut ActorInstance,
    dice: Expr,
    is_crit: bool,
    damage_type: DamageType
}

#[derive(Clone, PartialEq)]
pub enum Size {
    Tiny,
    Small,
    Medium,
    Large,
    Huge,
    Gargantuan
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
    Primordial,  //Aquan, Auran, Ignan, Terran
    Sylvan,
    ThievesCant,
    Undercommon
}

#[derive(Clone, PartialEq, Hash, Eq)]
pub enum SpecialSense {
    Blindsight(u32),
    Darkvision(u32),
    Tremorsense(u32),
    Truesight(u32)
}
