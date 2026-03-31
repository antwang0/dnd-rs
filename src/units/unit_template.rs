use std::collections::HashSet;
use crate::engine::{types::AbilityScoreType, types::{Language, Skill, SpecialSense, Size}};

use tyche::Expr;

#[derive(Clone, PartialEq)]
pub struct CreatureTemplate {
    ac: u32,
    hipoints: Expr,
    speed: f32,
    strength: u32,
    intelligence: u32,
    dexterity: u32,
    wisdom: u32,
    constitution: u32,
    charisma: u32,
    skills: HashSet<Skill>,
    // TODO: gear
    senses: HashSet<SpecialSense>,
    languages: HashSet<Language>,
    cr: f32,
    size: Size
}

impl CreatureTemplate {
    // pub fn make_actor(&self) -> ActorInstance {

    // }
}

#[derive(Clone, PartialEq)]
pub struct ActorInstance {
    base_ac: u32,
    base_hipoints: u32,
    base_speed: f32,
    base_size: Size,
    initiative: u32,
    strength: u32,
    intelligence: u32,
    dexterity: u32,
    wisdom: u32,
    constitution: u32,
    charisma: u32,
    skills: HashSet<Skill>,
    // TODO: gear
    senses: HashSet<SpecialSense>,
    languages: HashSet<Language>,
    cr: f32,
    hitpoints: u32,
    movement: f32,
    action_slots: u32,
    bonus_action_slots: u32,
    reaction_slots: u32,
    size: Size
    // TODO: spell slots
}

impl ActorInstance {
    fn ability_score(&self, ast: AbilityScoreType) -> u32 {
        // TODO: apply modifiers to ability scores (such as temporary buffs)
        match ast {
            AbilityScoreType::Strength => self.strength,
            AbilityScoreType::Intelligence => self.intelligence,
            AbilityScoreType::Dexterity => self.dexterity,
            AbilityScoreType::Wisdom => self.wisdom,
            AbilityScoreType::Constitution => self.constitution,
            AbilityScoreType::Charisma => self.charisma
        }
    }

    fn armor_class(&self) -> u32 {
        // TODO: apply modifiers to ability scores (such as temporary buffs)
        self.base_ac
    }

    fn hitpoints(&self) -> u32 {
        // TODO: apply modifiers to ability scores (such as temporary buffs)
        self.hitpoints
    }

    fn max_hitpoints(&self) -> u32 {
        // TODO: apply modifiers to ability scores (such as temporary buffs)
        self.base_hipoints
    }
    // TODO: bonus hitpoints?

    fn speed(&self) -> f32 {
        // TODO: apply modifiers to ability scores (such as temporary buffs)
        return self.base_speed
    }

    fn remaining_movement(&self) -> f32 {
        self.movement
    }

    fn size(&self) -> Size {
        self.size.clone()
    }
}