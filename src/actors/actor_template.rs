use crate::engine::{
    types::AbilityScoreType,
    types::{Language, Size, Skill, SpecialSense},
};
use crate::items::item_template::Item;
use std::collections::HashSet;

use tyche::Expr;
use tyche::dice::roller::Roller;

use std::error::Error;

#[derive(Clone, PartialEq)]
pub struct CreatureTemplate {
    pub ac: u32,
    pub hitpoints: Expr,
    pub speed: f32,
    pub strength: u32,
    pub intelligence: u32,
    pub dexterity: u32,
    pub wisdom: u32,
    pub constitution: u32,
    pub charisma: u32,
    pub skills: HashSet<Skill>,
    pub items: Vec<Item>,
    pub senses: HashSet<SpecialSense>,
    pub languages: HashSet<Language>,
    pub cr: f32,
    pub size: Size,
}

#[derive(Clone, PartialEq)]
pub struct SpellSlotInfo {
    pub max_spell_slots: u32,
    pub spell_slots: u32,
}

#[derive(Clone, PartialEq)]
pub struct SpellSlotManager {
    ssi_by_lvl: Vec<SpellSlotInfo>,
    warlock_ssi: SpellSlotInfo,
    warlock_spell_slot_lvl: u32,
}

impl SpellSlotManager {
    pub fn spell_slots(&self, lvl: u32) -> SpellSlotInfo {
        let i_usize = (lvl - 1) as usize;
        if let Some(ssi) = self.ssi_by_lvl.get(i_usize) {
            ssi.clone()
        } else {
            SpellSlotInfo {
                max_spell_slots: 0,
                spell_slots: 0,
            }
        }
    }

    pub fn consume_spell_slot(&mut self, lvl: u32) -> bool {
        let i_usize = (lvl - 1) as usize;
        if let Some(ssi) = self.ssi_by_lvl.get_mut(i_usize) {
            if ssi.spell_slots == 0 {
                return false;
            }
            ssi.spell_slots -= 1;
            true
        } else {
            false
        }
    }

    pub fn restore_spell_slot(&mut self, lvl: u32, qty: u32) -> bool {
        let i_usize = (lvl - 1) as usize;
        if let Some(ssi) = self.ssi_by_lvl.get_mut(i_usize) {
            if ssi.spell_slots + qty > ssi.max_spell_slots {
                return false;
            }
            ssi.spell_slots += qty;
            true
        } else {
            false
        }
    }

    pub fn restore_spell_slots(&mut self) {
        for ssi in self.ssi_by_lvl.iter_mut() {
            ssi.spell_slots = ssi.max_spell_slots;
        }
    }

    pub fn increase_max_spell_slot(&mut self, lvl: u32, qty: u32) {
        if lvl == 0 {
            return;
        }

        let i_usize = (lvl - 1) as usize;
        for _ in self.ssi_by_lvl.len()..=i_usize {
            self.ssi_by_lvl.push(SpellSlotInfo {
                max_spell_slots: 0,
                spell_slots: 0,
            });
        }

        self.ssi_by_lvl[i_usize].max_spell_slots += qty;
        self.ssi_by_lvl[i_usize].spell_slots += qty;
    }

    pub fn warlock_spell_slots(&self) -> SpellSlotInfo {
        self.warlock_ssi.clone()
    }

    pub fn warlock_spell_slot_lvl(&self) -> u32 {
        self.warlock_spell_slot_lvl
    }

    pub fn upgrade_warlock_spell_slots(&mut self, lvls: u32) {
        self.warlock_spell_slot_lvl += lvls;
    }

    pub fn consume_warlock_spell_slot(&mut self) -> bool {
        if self.warlock_ssi.spell_slots == 0 {
            return false;
        }
        self.warlock_ssi.spell_slots -= 1;
        true
    }

    pub fn restore_warlock_spell_slots(&mut self) {
        self.warlock_ssi.spell_slots = self.warlock_ssi.max_spell_slots;
    }

    pub fn increase_max_warlock_spell_slots(&mut self) {
        self.warlock_ssi.max_spell_slots += 1;
        self.warlock_ssi.spell_slots += 1;
    }
}

#[derive(Clone, PartialEq)]
pub struct ActorInstance {
    location: (usize, usize),
    team_id: usize,
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
    items: Vec<Item>,
    senses: HashSet<SpecialSense>,
    languages: HashSet<Language>,
    cr: f32,
    hitpoints: u32,
    movement: f32,
    action_slots: u32,
    bonus_action_slots: u32,
    reaction_slots: u32,
    size: Size,
    pub spell_slot_manager: SpellSlotManager,
}

impl ActorInstance {
    pub fn from_creature_template(
        ct: &CreatureTemplate,
        location: (usize, usize),
        team_id: usize,
        roller: &mut impl Roller,
    ) -> Result<ActorInstance, Box<dyn Error>> {
        let hp_roll_result = ct.hitpoints.eval(roller)?;
        let hp_roll_val = hp_roll_result.calc()? as u32;

        // variable stats should derive from below calls(such as max_hitpoints())
        // as they can be affected by item, effects, etc
        Result::Ok(ActorInstance {
            location: location,
            team_id: team_id,
            base_ac: ct.ac,
            base_hipoints: hp_roll_val,
            base_speed: ct.speed,
            base_size: ct.size.clone(),
            initiative: 0, // TODO?
            strength: ct.strength,
            intelligence: ct.intelligence,
            dexterity: ct.dexterity,
            wisdom: ct.wisdom,
            constitution: ct.constitution,
            charisma: ct.charisma,
            skills: ct.skills.clone(),
            items: ct.items.clone(),
            senses: ct.senses.clone(),
            languages: ct.languages.clone(),
            cr: ct.cr,
            hitpoints: 0,
            movement: 0.0,
            action_slots: 0,
            bonus_action_slots: 0,
            reaction_slots: 0,
            size: ct.size.clone(), // TODO: should derive from function call
            spell_slot_manager: SpellSlotManager {
                ssi_by_lvl: Vec::new(),
                warlock_ssi: SpellSlotInfo {
                    max_spell_slots: 0,
                    spell_slots: 0,
                },
                warlock_spell_slot_lvl: 0,
            },
        })
    }

    pub fn team(&self) -> usize {
        self.team_id
    }

    pub fn ability_score(&self, ast: AbilityScoreType) -> u32 {
        // TODO: apply modifiers to ability scores (such as temporary buffs)
        match ast {
            AbilityScoreType::Strength => self.strength,
            AbilityScoreType::Intelligence => self.intelligence,
            AbilityScoreType::Dexterity => self.dexterity,
            AbilityScoreType::Wisdom => self.wisdom,
            AbilityScoreType::Constitution => self.constitution,
            AbilityScoreType::Charisma => self.charisma,
        }
    }

    pub fn armor_class(&self) -> u32 {
        // TODO: apply modifiers to ability scores (such as temporary buffs)
        self.base_ac
    }

    pub fn hitpoints(&self) -> u32 {
        // TODO: apply modifiers to ability scores (such as temporary buffs)
        self.hitpoints
    }

    pub fn max_hitpoints(&self) -> u32 {
        // TODO: apply modifiers to ability scores (such as temporary buffs)
        self.base_hipoints
    }
    // TODO: bonus hitpoints?

    pub fn speed(&self) -> f32 {
        // TODO: apply modifiers to ability scores (such as temporary buffs)
        return self.base_speed;
    }

    pub fn remaining_movement(&self) -> f32 {
        self.movement
    }

    pub fn size(&self) -> Size {
        self.size.clone()
    }

    pub fn set_location(&mut self, target: (usize, usize)) {
        self.location = target;
    }
}
