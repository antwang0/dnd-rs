use crate::engine::dice::{Dice, DiceExpr, Roller};

/// Lifecycle state of an actor's hit points. Replaces the previous
/// `dying: bool` + `stable: bool` pair so the four meaningful states are
/// type-checked, and the death-save counters are scoped to the only
/// variant that uses them. `Dead` exists transiently between failure-3
/// and removal from `EncounterInstance.actors`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HpState {
    Active,
    Dying { successes: u32, failures: u32 },
    Stable,
    Dead,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeathSaveOutcome {
    /// Actor wasn't dying — caller did something wrong.
    NotDying,
    /// Save processed; actor still in the dying state.
    Continuing,
    /// 3 successes accumulated; actor is now stable at 0 HP (no more saves).
    Stabilized,
    /// 3 failures accumulated; actor is dead and will be removed.
    Dead,
    /// Natural 20: actor wakes at 1 HP and rejoins the fight.
    Revived,
}

/// What `take_damage` did to the actor's state. `DealDamage::apply` reads
/// this to emit the appropriate log line; tests use it to verify state
/// transitions without inspecting private fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DamageOutcome {
    /// Actor took normal HP damage and is still combat-active.
    Reduced,
    /// HP just hit 0; actor transitioned from combat-active to dying.
    Downed,
    /// Actor was already dying (or stable, which gets re-downed); damage
    /// counts as a failed death save instead of an HP delta.
    DyingFailure,
}
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

use std::error::Error;

pub struct CreatureTemplate {
    pub name: &'static str,
    /// Single-character glyph for the map. Convention: capital letter
    /// matching the species (Z for zombie, S for skeleton, etc.). Multiple
    /// instances on the same team look identical on the map; the side
    /// panel and log disambiguate via the instance-numbered name.
    pub glyph: char,
    pub n_instances: usize,
    pub ac: u32,
    pub hitpoints: DiceExpr,
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
#[allow(dead_code)] // many fields are placeholders for not-yet-wired systems
pub struct ActorInstance {
    name: String,
    location: Coordinate,
    team_id: usize,
    base_ac: u32,
    base_hitpoints: u32,
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
    /// Map glyph copied from the template. Static across an actor's life.
    glyph: char,
    /// Lifecycle state. Driven by `take_damage` (damage transitions
    /// `Active -> Dying`, hits on `Dying`/`Stable` add failures) and
    /// `apply_death_save` (rolls move within `Dying` and into `Stable`,
    /// `Dead`, or `Active` on a nat 20). See `HpState`.
    hp_state: HpState,
}

impl ActorInstance {
    pub fn from_creature_template(
        ct: &'static CreatureTemplate,
        location: Coordinate,
        team_id: usize,
        roller: &mut impl Roller,
        instance_n: usize,
    ) -> Result<ActorInstance, Box<dyn Error>> {
        let hp_roll_val: u32 = ct.hitpoints.eval(roller).max(0) as u32;

        let name: String = format!("{} {}", ct.name, instance_n);

        // variable stats should derive from below calls(such as max_hitpoints())
        // as they can be affected by item, effects, etc
        Result::Ok(ActorInstance {
            name,
            location,
            team_id,
            base_ac: ct.ac,
            base_hitpoints: hp_roll_val,
            base_speed: ct.speed,
            base_size: ct.size,
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
            size: ct.size, // TODO: should derive from function call
            spell_slot_manager: SpellSlotManager {
                ssi_by_lvl: Vec::new(),
                warlock_ssi: SpellSlotInfo {
                    max_spell_slots: 0,
                    spell_slots: 0,
                },
                warlock_spell_slot_lvl: 0,
            },
            actions: ct.actions.clone(),
            glyph: ct.glyph,
            hp_state: HpState::Active,
        })
    }

    pub fn glyph(&self) -> char {
        self.glyph
    }

    pub fn name(&self) -> &str {
        &self.name
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

    /// Consumes the resource. Returns false (and changes nothing) if the
    /// actor lacks it; callers should pre-check with `can_consume_resource`.
    pub fn consume_resource(&mut self, resource: Resource) -> bool {
        if !self.can_consume_resource(resource) {
            return false;
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
        true
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
        self.base_hitpoints
    }
    // TODO: bonus hitpoints?

    pub fn speed(&self) -> f32 {
        // TODO: apply modifiers to ability scores (such as temporary buffs)
        self.base_speed
    }

    pub fn remaining_movement(&self) -> f32 {
        self.movement
    }

    pub fn size(&self) -> Size {
        self.size
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
        let rolled = roller.roll(&Dice::new(1, 20)) as i32;
        self.initiative = Some(rolled + self.initiative_mod());
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

    pub fn take_damage(&mut self, amount: u32) -> DamageOutcome {
        match self.hp_state {
            HpState::Stable => {
                // Stable creature takes damage: dying state restarts fresh
                // (5e: death-save tracking is cleared by stabilization),
                // then this damage immediately counts as one failed save.
                self.hp_state = HpState::Dying {
                    successes: 0,
                    failures: 1,
                };
                DamageOutcome::DyingFailure
            }
            HpState::Dying {
                successes,
                failures,
            } => {
                self.hp_state = HpState::Dying {
                    successes,
                    failures: failures + 1,
                };
                DamageOutcome::DyingFailure
            }
            HpState::Dead => DamageOutcome::DyingFailure, // already gone; no-op
            HpState::Active => {
                self.hitpoints = self.hitpoints.saturating_sub(amount);
                if self.hitpoints == 0 {
                    self.hp_state = HpState::Dying {
                        successes: 0,
                        failures: 0,
                    };
                    DamageOutcome::Downed
                } else {
                    DamageOutcome::Reduced
                }
            }
        }
    }

    pub fn hp_state(&self) -> HpState {
        self.hp_state
    }

    pub fn is_dying(&self) -> bool {
        matches!(self.hp_state, HpState::Dying { .. })
    }

    pub fn is_stable(&self) -> bool {
        matches!(self.hp_state, HpState::Stable)
    }

    /// True when this actor can still take meaningful turns — has HP and
    /// isn't downed. Used by the engine for end-of-combat detection.
    pub fn is_combat_active(&self) -> bool {
        matches!(self.hp_state, HpState::Active) && self.hitpoints > 0
    }

    pub fn death_save_record(&self) -> (u32, u32) {
        match self.hp_state {
            HpState::Dying {
                successes,
                failures,
            } => (successes, failures),
            _ => (0, 0),
        }
    }

    /// Apply a single d20 death-save result. Mutates the actor's state and
    /// returns the new outcome category so the caller can log appropriately.
    pub fn apply_death_save(&mut self, raw_d20: u32) -> DeathSaveOutcome {
        debug_assert!(
            (1..=20).contains(&raw_d20),
            "death-save d20 out of range: {}",
            raw_d20
        );
        let HpState::Dying {
            successes,
            failures,
        } = self.hp_state
        else {
            return DeathSaveOutcome::NotDying;
        };
        if raw_d20 == 20 {
            // Natural 20: pop back up at 1 HP.
            self.hp_state = HpState::Active;
            self.hitpoints = 1;
            return DeathSaveOutcome::Revived;
        }
        let (succ, fail) = if raw_d20 == 1 {
            (successes, failures.saturating_add(2))
        } else if raw_d20 >= 10 {
            (successes.saturating_add(1), failures)
        } else {
            (successes, failures.saturating_add(1))
        };
        if fail >= 3 {
            self.hp_state = HpState::Dead;
            DeathSaveOutcome::Dead
        } else if succ >= 3 {
            // 5e: stabilization clears tracking so a future re-down starts
            // at zero, not on top of accumulated saves.
            self.hp_state = HpState::Stable;
            DeathSaveOutcome::Stabilized
        } else {
            self.hp_state = HpState::Dying {
                successes: succ,
                failures: fail,
            };
            DeathSaveOutcome::Continuing
        }
    }

    pub fn attack_bonus(&self) -> i32 {
        // TODO: add proficiency bonus once it's tracked
        modifier_from_score(self.strength)
    }

    pub fn damage_bonus(&self) -> i32 {
        modifier_from_score(self.strength)
    }
}
