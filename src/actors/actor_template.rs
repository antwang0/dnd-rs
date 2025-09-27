use crate::engine::side_effects::Resource;
use crate::engine::types::Coordinate;
use crate::items::item_template::Item;
use crate::{
    actions::action_template::Action,
    engine::{
        types::{AbilityScoreType, Language, Size, Skill, SpecialSense},
        util::modifier_from_score,
    },
};
use std::collections::HashSet;

use tyche::dice::roller::Roller;
use tyche::{Dice, Expr};

use std::error::Error;

pub struct CreatureTemplate {
    pub name: &'static str,
    pub n_instances: usize,
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
    pub actions: Vec<&'static (dyn Action + Send + Sync)>,
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

#[derive(Clone)]
pub struct ActorInstance {
    name: String,
    location: Coordinate,
    team_id: usize,
    base_ac: u32,
    base_hipoints: u32,
    base_speed: f32,
    base_size: Size,
    initiative: Option<i32>,
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
    legendary_action_slots: u32,
    size: Size,
    pub spell_slot_manager: SpellSlotManager,
    pub actions: Vec<&'static (dyn Action + Send + Sync)>,
}

impl ActorInstance {
    pub fn from_creature_template(
        ct: &'static CreatureTemplate,
        location: Coordinate,
        team_id: usize,
        roller: &mut impl Roller,
        instance_n: usize,
    ) -> Result<ActorInstance, Box<dyn Error>> {
        let hp_roll_result = ct.hitpoints.eval(roller)?;
        let hp_roll_val = hp_roll_result.calc()? as u32;

        let name: String = format!("{} {}", ct.name, instance_n);

        // variable stats should derive from below calls(such as max_hitpoints())
        // as they can be affected by item, effects, etc
        Result::Ok(ActorInstance {
            name: name,
            location: location,
            team_id: team_id,
            base_ac: ct.ac,
            base_hipoints: hp_roll_val,
            base_speed: ct.speed,
            base_size: ct.size.clone(),
            initiative: None,
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
            hitpoints: hp_roll_val,
            movement: 0.0,
            action_slots: 0,
            bonus_action_slots: 0,
            reaction_slots: 0,
            legendary_action_slots: 0,
            size: ct.size.clone(), // TODO: should derive from function call
            spell_slot_manager: SpellSlotManager {
                ssi_by_lvl: Vec::new(),
                warlock_ssi: SpellSlotInfo {
                    max_spell_slots: 0,
                    spell_slots: 0,
                },
                warlock_spell_slot_lvl: 0,
            },
            actions: ct.actions.clone(),
        })
    }

    pub fn name(&self) -> String {
        self.name.clone()
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

    pub fn can_consume_resource(&self, resource: Resource) -> bool {
        match resource {
            Resource::Movement(movement_amt) => movement_amt <= self.movement,
            Resource::SpellSlot(spell_lvl) => {
                self.spell_slot_manager.spell_slots(spell_lvl).spell_slots >= 1
            }
            Resource::Action => self.action_slots >= 1,
            Resource::BonusAction => self.bonus_action_slots >= 1,
            Resource::Reaction => self.reaction_slots >= 1,
            Resource::LegendaryAction => self.legendary_action_slots >= 1,
        }
    }

    pub fn consume_resource(&mut self, resource: Resource) {
        if !self.can_consume_resource(resource) {
            panic!("illegal resource consumption")
        }
        match resource {
            Resource::Movement(movement_amt) => {
                self.movement -= movement_amt;
            }
            Resource::SpellSlot(spell_lvl) => {
                self.spell_slot_manager.consume_spell_slot(spell_lvl);
            }
            Resource::Action => {
                self.action_slots -= 1;
            }
            Resource::BonusAction => {
                self.bonus_action_slots -= 1;
            }
            Resource::Reaction => {
                self.reaction_slots -= 1;
            }
            Resource::LegendaryAction => {
                self.legendary_action_slots -= 1;
            }
        }
    }

    pub fn give_resource(&mut self, resource: Resource) {
        match resource {
            Resource::Movement(movement_amt) => {
                self.movement += movement_amt;
            }
            Resource::SpellSlot(spell_lvl) => {
                self.spell_slot_manager.restore_spell_slot(spell_lvl, 1);
            }
            Resource::Action => {
                self.action_slots += 1;
            }
            Resource::BonusAction => {
                self.bonus_action_slots += 1;
            }
            Resource::Reaction => {
                self.reaction_slots += 1;
            }
            Resource::LegendaryAction => {
                self.legendary_action_slots += 1;
            }
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

    pub fn set_location(&mut self, target: Coordinate) {
        self.location = target;
    }

    pub fn location(&self) -> Coordinate {
        self.location
    }

    pub fn initiative(&self) -> Option<i32> {
        self.initiative
    }

    pub fn initiative_mod(&self) -> i32 {
        // TODO: apply modifiers to ability scores (such as feats)
        modifier_from_score(self.dexterity)
    }

    pub fn roll_initiative(&mut self, roller: &mut impl Roller) {
        let dice = Dice::new(1, 6);
        let rolled = roller
            .roll(&dice, true)
            .expect("somehow roll failed")
            .total()
            .expect("roll conversion failed");

        self.initiative = Some(rolled as i32 + self.initiative_mod());
    }

    pub fn reset_for_new_round(&mut self) {
        self.movement = self.speed();

        // TODO: pull from function
        self.action_slots = 1;
        self.bonus_action_slots = 1;
        self.reaction_slots = 1;
        // TODO: legendary actions
    }

    pub fn action_slots(&self) -> u32 {
        self.action_slots
    }

    pub fn bonus_action_slots(&self) -> u32 {
        self.bonus_action_slots
    }
}
