use crate::actions::action_template::Action;
use crate::conditions::{Condition, ConditionTimer};
use crate::engine::dice::{Dice, DiceExpr, Roller};
use crate::engine::side_effects::Resource;
use crate::engine::types::{
    AbilityScoreType, Coordinate, DamageModifier, DamageType, Language, Size, Skill, SpecialSense,
};
use crate::engine::util::modifier_from_score;
use crate::items::item_template::{Item, ItemBonuses};
use std::collections::{HashMap, HashSet};
use std::error::Error;

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
    NotDying,
    Continuing,
    Stabilized,
    Dead,
    Revived,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DamageOutcome {
    Reduced,
    Downed,
    Killed,
    DyingFailure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealOutcome {
    Healed,
    Revived,
    AlreadyFull,
    NoOp,
}

/// State of an actor that's concentrating on a spell. Tracks what they
/// applied so dropping concentration can clean up automatically.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConcentrationData {
    pub spell_name: String,
    /// Conditions this concentration applied. On drop, each is removed
    /// from its target. `(target_id, condition)`.
    pub conditions: Vec<(usize, Condition)>,
    /// Attack-roll buff deltas to roll back on drop.
    pub attack_buffs: Vec<(usize, i32)>,
    pub save_buffs: Vec<(usize, i32)>,
}

/// 5e Help grant — a snapshot of "actor X has helped actor Y get
/// advantage against enemy Z." Stored on the recipient actor; consumed
/// by their next attack against `against`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HelpGrant {
    pub helper_id: usize,
    pub against: usize,
}

impl ConcentrationData {
    pub fn with_conditions(
        spell_name: impl Into<String>,
        conditions: Vec<(usize, Condition)>,
    ) -> Self {
        Self {
            spell_name: spell_name.into(),
            conditions,
            attack_buffs: Vec::new(),
            save_buffs: Vec::new(),
        }
    }
}

pub struct CreatureTemplate {
    pub name: &'static str,
    pub glyph: char,
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
    pub items: Vec<&'static Item>,
    pub senses: HashSet<SpecialSense>,
    pub languages: HashSet<Language>,
    pub cr: f32,
    pub size: Size,
    pub actions: Vec<&'static (dyn Action + Send + Sync)>,
    pub spell_slots_by_level: Vec<u32>,
    pub rolls_death_saves: bool,
    /// Per-damage-type modifiers (resistance / immunity / vulnerability).
    /// Looked up by `damage_modifier` on the instance.
    pub damage_modifiers: HashMap<DamageType, DamageModifier>,
    /// Saving throws this creature is proficient with. Optional;
    /// templates that don't care can leave this empty (default new).
    pub proficient_saves: HashSet<AbilityScoreType>,
    /// Conditions this creature is immune to (e.g. zombies vs Charm,
    /// elementals vs Poisoned).
    pub condition_immunities: HashSet<Condition>,
    /// Class-feature tags available to this creature (Second Wind,
    /// Action Surge, etc.). Empty for ordinary monsters.
    pub features: HashSet<&'static str>,
}

#[derive(Clone, PartialEq)]
pub struct SpellSlotInfo {
    pub max_spell_slots: u32,
    pub spell_slots: u32,
}

#[derive(Clone, PartialEq)]
pub struct SpellSlotManager {
    ssi_by_lvl: Vec<SpellSlotInfo>,
}

impl SpellSlotManager {
    fn idx(lvl: u32) -> Option<usize> {
        if lvl == 0 { None } else { Some((lvl - 1) as usize) }
    }

    pub fn spell_slots(&self, lvl: u32) -> SpellSlotInfo {
        Self::idx(lvl)
            .and_then(|i| self.ssi_by_lvl.get(i).cloned())
            .unwrap_or(SpellSlotInfo {
                max_spell_slots: 0,
                spell_slots: 0,
            })
    }

    pub fn consume_spell_slot(&mut self, lvl: u32) -> bool {
        let Some(i) = Self::idx(lvl) else {
            return false;
        };
        let Some(ssi) = self.ssi_by_lvl.get_mut(i) else {
            return false;
        };
        if ssi.spell_slots == 0 {
            return false;
        }
        ssi.spell_slots -= 1;
        true
    }

    pub fn restore_spell_slot(&mut self, lvl: u32, qty: u32) -> bool {
        let Some(i) = Self::idx(lvl) else {
            return false;
        };
        let Some(ssi) = self.ssi_by_lvl.get_mut(i) else {
            return false;
        };
        if ssi.spell_slots + qty > ssi.max_spell_slots {
            return false;
        }
        ssi.spell_slots += qty;
        true
    }

    pub fn restore_spell_slots(&mut self) {
        for ssi in self.ssi_by_lvl.iter_mut() {
            ssi.spell_slots = ssi.max_spell_slots;
        }
    }

    pub fn increase_max_spell_slot(&mut self, lvl: u32, qty: u32) {
        let Some(i_usize) = Self::idx(lvl) else {
            return;
        };
        for _ in self.ssi_by_lvl.len()..=i_usize {
            self.ssi_by_lvl.push(SpellSlotInfo {
                max_spell_slots: 0,
                spell_slots: 0,
            });
        }
        self.ssi_by_lvl[i_usize].max_spell_slots += qty;
        self.ssi_by_lvl[i_usize].spell_slots += qty;
    }
}

#[derive(Clone)]
#[allow(dead_code)]
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
    items: Vec<&'static Item>,
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
    glyph: char,
    hp_state: HpState,
    conditions: HashMap<Condition, ConditionTimer>,
    concentration: Option<ConcentrationData>,
    rolls_death_saves: bool,
    /// Per-damage-type modifier table copied from the creature template.
    damage_modifiers: HashMap<DamageType, DamageModifier>,
    /// 5e temporary hit points. Damage drains temp HP before regular HP.
    /// Doesn't stack: a new grant replaces existing temp HP only if
    /// larger. Cleared on long rest.
    temp_hp: u32,
    level: u32,
    xp: u32,
    /// Saving-throw proficiencies — adds proficiency bonus to roll_save.
    proficient_saves: HashSet<AbilityScoreType>,
    /// Conditions the actor is wholly immune to.
    condition_immunities: HashSet<Condition>,
    /// Class-feature tags currently available (consumed on use, refreshed
    /// on long rest).
    features_remaining: HashSet<&'static str>,
    features_max: HashSet<&'static str>,
    /// Bless / Resistance flat to-hit and save bonuses. Independent of the
    /// `Blessed` condition flag for stacking flexibility.
    attack_bonus_buff: i32,
    save_bonus_buff: i32,
    /// Sneak Attack guard — true if the rogue has spent their once-per-turn
    /// sneak this turn. Cleared at turn-start by `reset_for_new_round`.
    sneak_attack_used: bool,
    /// Help-action grants. Map of helper_id → target_id where the helper
    /// is providing advantage on the helped actor's next attack vs the
    /// listed target. Consumed when the helped actor attacks the target.
    help_grants: HashMap<usize, usize>,
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
        Ok(ActorInstance {
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
            size: ct.size,
            spell_slot_manager: SpellSlotManager {
                ssi_by_lvl: ct
                    .spell_slots_by_level
                    .iter()
                    .map(|&n| SpellSlotInfo {
                        max_spell_slots: n,
                        spell_slots: n,
                    })
                    .collect(),
            },
            actions: ct.actions.clone(),
            glyph: ct.glyph,
            hp_state: HpState::Active,
            conditions: HashMap::new(),
            concentration: None,
            rolls_death_saves: ct.rolls_death_saves,
            damage_modifiers: ct.damage_modifiers.clone(),
            temp_hp: 0,
            level: 1,
            xp: 0,
            proficient_saves: ct.proficient_saves.clone(),
            condition_immunities: ct.condition_immunities.clone(),
            features_remaining: ct.features.clone(),
            features_max: ct.features.clone(),
            attack_bonus_buff: 0,
            save_bonus_buff: 0,
            sneak_attack_used: false,
            help_grants: HashMap::new(),
        })
    }

    pub fn rolls_death_saves(&self) -> bool {
        self.rolls_death_saves
    }

    /// First action in the actor's list whose `name()` matches `name`.
    pub fn find_action(&self, name: &str) -> Option<&'static (dyn Action + Send + Sync)> {
        self.actions.iter().find(|a| a.name() == name).copied()
    }

    /// Sum every carried item's `ItemBonuses` into one struct.
    pub fn total_item_bonuses(&self) -> ItemBonuses {
        self.items
            .iter()
            .fold(ItemBonuses::default(), |acc, it| acc + it.bonuses)
    }

    pub fn items(&self) -> &[&'static Item] {
        &self.items
    }

    pub fn pickup_item(&mut self, item: &'static Item) {
        self.items.push(item);
    }

    pub fn has_item_named(&self, name: &str) -> bool {
        self.items.iter().any(|i| i.name == name)
    }

    pub fn remove_item_by_name(&mut self, name: &str) -> bool {
        if let Some(pos) = self.items.iter().position(|i| i.name == name) {
            self.items.remove(pos);
            true
        } else {
            false
        }
    }

    /// Base actions plus one entry per unique consumable item the actor
    /// is carrying (deduped by item name).
    pub fn available_actions(&self) -> Vec<&'static (dyn Action + Send + Sync)> {
        let mut out = self.actions.clone();
        let mut seen: HashSet<&'static str> = HashSet::new();
        for item in &self.items {
            if let Some(action) = item.on_use
                && seen.insert(item.name)
            {
                out.push(action);
            }
        }
        out
    }

    /// Restore full HP, all spell slots, clear non-permanent conditions,
    /// concentration and any temp HP. 5e long rest semantics.
    pub fn long_rest(&mut self) {
        self.hp_state = HpState::Active;
        self.hitpoints = self.max_hitpoints();
        self.temp_hp = 0;
        self.spell_slot_manager.restore_spell_slots();
        self.conditions.clear();
        self.concentration = None;
        self.attack_bonus_buff = 0;
        self.save_bonus_buff = 0;
        self.features_remaining = self.features_max.clone();
    }

    pub fn temp_hp(&self) -> u32 {
        self.temp_hp
    }

    /// Alias for legacy call sites.
    pub fn temp_hitpoints(&self) -> u32 {
        self.temp_hp
    }

    /// 5e: a new application replaces the existing pool only if it's
    /// larger. Returns true if temp HP changed.
    pub fn grant_temp_hp(&mut self, amount: u32) -> bool {
        if amount > self.temp_hp {
            self.temp_hp = amount;
            true
        } else {
            false
        }
    }

    /// Same semantics as `grant_temp_hp`, but returns the resulting pool
    /// size for callers that prefer reading the post-state value directly.
    pub fn gain_temp_hp(&mut self, amount: u32) -> u32 {
        if amount > self.temp_hp {
            self.temp_hp = amount;
        }
        self.temp_hp
    }

    /// Legacy alias — same semantics.
    pub fn add_temp_hp(&mut self, amount: u32) {
        self.gain_temp_hp(amount);
    }

    /// Returns the post-modifier damage value (immunity → 0, resistance
    /// → halve, vulnerability → double, none → unchanged). Doesn't touch
    /// temp HP — that's `take_typed_damage`'s job.
    pub fn effective_damage(&self, raw: u32, dt: DamageType) -> u32 {
        let mut amt = match self.damage_modifiers.get(&dt).copied() {
            Some(m) => m.apply(raw),
            None => raw,
        };
        // Stoneskin-like generic resistance: half the damage on top.
        if self.has_condition(Condition::DamageResistant) {
            amt /= 2;
        }
        amt
    }

    /// Same as effective_damage; kept for legacy callers.
    pub fn modified_damage(&self, raw: u32, dt: DamageType) -> u32 {
        self.effective_damage(raw, dt)
    }

    pub fn adjusted_damage(&self, raw: u32, dt: DamageType) -> u32 {
        self.effective_damage(raw, dt)
    }

    pub fn apply_damage_modifiers(&self, raw: u32, dt: DamageType) -> u32 {
        self.effective_damage(raw, dt)
    }

    pub fn damage_modifier(&self, dt: DamageType) -> Option<DamageModifier> {
        self.damage_modifiers.get(&dt).copied()
    }

    pub fn is_resistant_to(&self, dt: DamageType) -> bool {
        matches!(
            self.damage_modifiers.get(&dt),
            Some(DamageModifier::Resistance)
        )
    }

    pub fn is_vulnerable_to(&self, dt: DamageType) -> bool {
        matches!(
            self.damage_modifiers.get(&dt),
            Some(DamageModifier::Vulnerability)
        )
    }

    pub fn is_immune_to(&self, dt: DamageType) -> bool {
        matches!(
            self.damage_modifiers.get(&dt),
            Some(DamageModifier::Immunity)
        )
    }

    pub fn cr(&self) -> f32 {
        self.cr
    }

    /// 5e proficiency bonus: +2 at L1-4, +3 at L5-8, +4 at L9-12, etc.
    /// Both PCs (driven by `level`) and monsters (whose CR is roughly
    /// equivalent to a player level) read from the same scale.
    pub fn proficiency_bonus(&self) -> i32 {
        let effective_level = self.level.max(self.cr.floor().max(1.0) as u32);
        2 + ((effective_level.saturating_sub(1)) / 4) as i32
    }

    /// Linear XP value: CR × 200. Linear is good enough for the dungeon
    /// loop and keeps the ramp legible.
    pub fn xp_value(&self) -> u32 {
        (self.cr * 200.0).round().max(0.0) as u32
    }

    pub fn level(&self) -> u32 {
        self.level
    }

    pub fn is_save_proficient(&self, ability: AbilityScoreType) -> bool {
        self.proficient_saves.contains(&ability)
    }

    pub fn xp(&self) -> u32 {
        self.xp
    }

    /// XP needed to reach the *next* level from the current level.
    /// Linear curve `level * 300`.
    pub fn xp_threshold_for_next_level(&self) -> u32 {
        self.level * 300
    }

    pub fn award_xp(&mut self, amount: u32) {
        self.xp = self.xp.saturating_add(amount);
    }

    /// Promote a PC to the next level if they've crossed the threshold.
    pub fn try_level_up(&mut self, roller: &mut impl Roller) -> Option<u32> {
        if self.xp < self.xp_threshold_for_next_level() {
            return None;
        }
        self.level += 1;
        let con_mod = modifier_from_score(self.constitution);
        let roll = roller.roll(&Dice::new(1, 10)) as i32;
        let gain = (roll + con_mod).max(1) as u32;
        self.base_hitpoints = self.base_hitpoints.saturating_add(gain);
        self.hitpoints = self
            .hitpoints
            .saturating_add(gain)
            .min(self.max_hitpoints());
        Some(self.level)
    }

    pub fn is_concentrating(&self) -> bool {
        self.concentration.is_some()
    }

    pub fn concentration(&self) -> Option<&ConcentrationData> {
        self.concentration.as_ref()
    }

    pub fn start_concentration(&mut self, data: ConcentrationData) -> Option<ConcentrationData> {
        self.concentration.replace(data)
    }

    pub fn end_concentration(&mut self) -> Option<ConcentrationData> {
        self.concentration.take()
    }

    pub fn has_condition(&self, c: Condition) -> bool {
        self.conditions.contains_key(&c)
    }

    /// Add a condition with the given timer. If the actor is immune to
    /// the condition (via `condition_immunities`), no-op and return false.
    ///
    /// 5e: re-applying a condition with a *longer* timer extends the
    /// effect; a shorter timer is ignored. Permanent beats any rounds
    /// timer; `UntilStartOfNextTurn` is treated as the shortest possible
    /// duration. Returns true if the condition was newly added.
    pub fn add_condition(&mut self, c: Condition, timer: ConditionTimer) -> bool {
        if self.condition_immunities.contains(&c) {
            return false;
        }
        let is_new = !self.conditions.contains_key(&c);
        let new_timer = match (self.conditions.get(&c).copied(), timer) {
            (None, t) => t,
            (Some(ConditionTimer::Permanent), _) => ConditionTimer::Permanent,
            (_, ConditionTimer::Permanent) => ConditionTimer::Permanent,
            (Some(ConditionTimer::Rounds(a)), ConditionTimer::Rounds(b)) => {
                ConditionTimer::Rounds(a.max(b))
            }
            (Some(ConditionTimer::Rounds(a)), ConditionTimer::UntilStartOfNextTurn) => {
                ConditionTimer::Rounds(a)
            }
            (Some(ConditionTimer::UntilStartOfNextTurn), ConditionTimer::Rounds(b)) => {
                ConditionTimer::Rounds(b)
            }
            (Some(ConditionTimer::UntilStartOfNextTurn), ConditionTimer::UntilStartOfNextTurn) => {
                ConditionTimer::UntilStartOfNextTurn
            }
        };
        self.conditions.insert(c, new_timer);
        is_new
    }

    pub fn is_immune_to_condition(&self, c: Condition) -> bool {
        self.condition_immunities.contains(&c)
    }

    pub fn remove_condition(&mut self, c: Condition) -> bool {
        self.conditions.remove(&c).is_some()
    }

    pub fn conditions(&self) -> &HashMap<Condition, ConditionTimer> {
        &self.conditions
    }

    /// Decrement every `Rounds(n)` timer by 1 and report which conditions
    /// expired. `Permanent` and `UntilStartOfNextTurn` are untouched.
    pub fn tick_condition_timers(&mut self) -> Vec<Condition> {
        let mut expired = Vec::new();
        let snapshot: Vec<(Condition, ConditionTimer)> = self
            .conditions
            .iter()
            .map(|(c, t)| (*c, *t))
            .collect();
        for (c, timer) in snapshot {
            match timer {
                ConditionTimer::Permanent | ConditionTimer::UntilStartOfNextTurn => {}
                ConditionTimer::Rounds(0) | ConditionTimer::Rounds(1) => {
                    self.conditions.remove(&c);
                    expired.push(c);
                }
                ConditionTimer::Rounds(n) => {
                    self.conditions.insert(c, ConditionTimer::Rounds(n - 1));
                }
            }
        }
        expired
    }

    /// Tick a Bless-like buff: returns true if it expired this tick.
    /// Layered on top of `tick_condition_timers` for callers that want
    /// a focused notification.
    pub fn tick_bless(&mut self) -> bool {
        // Already ticked by `tick_condition_timers`; this is a query for
        // the engine logger. Returns true iff the condition is now gone
        // but was set at the start of this round (we approximate by just
        // returning whether it's currently absent — callers compose it
        // with prior state).
        !self.has_condition(Condition::Blessed)
    }

    /// Ditto for Shield of Faith.
    pub fn tick_shield_of_faith(&mut self) -> bool {
        !self.has_condition(Condition::ShieldOfFaith)
    }

    /// Clear every condition with the `UntilStartOfNextTurn` timer.
    pub fn clear_until_next_turn_conditions(&mut self) -> Vec<Condition> {
        let mut expired = Vec::new();
        let to_remove: Vec<Condition> = self
            .conditions
            .iter()
            .filter_map(|(c, t)| match t {
                ConditionTimer::UntilStartOfNextTurn => Some(*c),
                _ => None,
            })
            .collect();
        for c in to_remove {
            self.conditions.remove(&c);
            expired.push(c);
        }
        expired
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
        match ast {
            AbilityScoreType::Strength => self.strength,
            AbilityScoreType::Intelligence => self.intelligence,
            AbilityScoreType::Dexterity => self.dexterity,
            AbilityScoreType::Wisdom => self.wisdom,
            AbilityScoreType::Constitution => self.constitution,
            AbilityScoreType::Charisma => self.charisma,
        }
    }

    /// True if any active condition's `blocks_action_economy` clause
    /// (Stunned / Incapacitated / Paralyzed / Unconscious) is set.
    fn is_incapacitated(&self) -> bool {
        self.conditions.keys().any(|c| c.blocks_action_economy())
    }

    pub fn can_consume_resource(&self, resource: Resource) -> bool {
        let action_blocked = self.is_incapacitated();
        match resource {
            Resource::Movement(amt) => {
                if action_blocked {
                    return false;
                }
                // Conditions that zero out movement entirely.
                if self.conditions.keys().any(|c| c.zeros_movement()) {
                    return false;
                }
                amt <= self.movement
            }
            Resource::SpellSlot(spell_lvl) => {
                if action_blocked {
                    return false;
                }
                self.spell_slot_manager.spell_slots(spell_lvl).spell_slots >= 1
            }
            Resource::Action => !action_blocked && self.action_slots >= 1,
            Resource::BonusAction => !action_blocked && self.bonus_action_slots >= 1,
            Resource::Reaction => !action_blocked && self.reaction_slots >= 1,
            Resource::LegendaryAction => !action_blocked && self.legendary_action_slots >= 1,
        }
    }

    pub fn consume_resource(&mut self, resource: Resource) -> bool {
        if !self.can_consume_resource(resource) {
            return false;
        }
        match resource {
            Resource::Movement(amt) => self.movement -= amt,
            Resource::SpellSlot(lvl) => {
                self.spell_slot_manager.consume_spell_slot(lvl);
            }
            Resource::Action => self.action_slots -= 1,
            Resource::BonusAction => self.bonus_action_slots -= 1,
            Resource::Reaction => self.reaction_slots -= 1,
            Resource::LegendaryAction => self.legendary_action_slots -= 1,
        }
        true
    }

    pub fn give_resource(&mut self, resource: Resource) {
        match resource {
            Resource::Movement(amt) => self.movement += amt,
            Resource::SpellSlot(lvl) => {
                self.spell_slot_manager.restore_spell_slot(lvl, 1);
            }
            Resource::Action => self.action_slots += 1,
            Resource::BonusAction => self.bonus_action_slots += 1,
            Resource::Reaction => self.reaction_slots += 1,
            Resource::LegendaryAction => self.legendary_action_slots += 1,
        }
    }

    pub fn armor_class(&self) -> u32 {
        (self.base_ac as i32 + self.total_item_bonuses().ac + self.condition_ac_bonus())
            .max(0) as u32
    }

    /// Flat AC contribution from active conditions. Shield of Faith
    /// (+2 from the spell), Shielded (+5 from the Shield reaction spell
    /// — RAW value).
    pub fn condition_ac_bonus(&self) -> i32 {
        let mut bonus = 0;
        if self.has_condition(Condition::ShieldOfFaith) {
            bonus += 2;
        }
        if self.has_condition(Condition::Shielded) {
            bonus += 5;
        }
        bonus
    }

    pub fn hitpoints(&self) -> u32 {
        self.hitpoints
    }

    pub fn max_hitpoints(&self) -> u32 {
        let bonus = self.total_item_bonuses().max_hp;
        (self.base_hitpoints as i32 + bonus).max(1) as u32
    }

    pub fn speed(&self) -> f32 {
        let bonus = self.total_item_bonuses().speed as f32;
        (self.base_speed + bonus).max(0.0)
    }

    pub fn item_save_bonus(&self) -> i32 {
        self.total_item_bonuses().save
    }

    /// Flat to-hit bonus contributed only by *conditions* (Bless = +2,
    /// the d4 average). Independent of `attack_bonus_buff` so callers
    /// that want both can sum them; the engine adds both at attack-roll
    /// time via `condition_attack_bonus + attack_bonus_buff`.
    pub fn condition_attack_bonus(&self) -> i32 {
        let mut bonus = 0;
        if self.has_condition(Condition::Blessed) {
            bonus += 2;
        }
        bonus
    }

    pub fn condition_save_bonus(&self) -> i32 {
        let mut bonus = 0;
        if self.has_condition(Condition::Blessed) {
            bonus += 2;
        }
        bonus
    }

    pub fn attack_bonus_buff(&self) -> i32 {
        self.attack_bonus_buff
    }

    pub fn save_bonus_buff(&self) -> i32 {
        self.save_bonus_buff
    }

    pub fn add_attack_bonus_buff(&mut self, delta: i32) {
        self.attack_bonus_buff += delta;
    }

    pub fn add_save_bonus_buff(&mut self, delta: i32) {
        self.save_bonus_buff += delta;
    }

    pub fn remaining_movement(&self) -> f32 {
        // Prone is special-cased: standing up is the only legal use of
        // movement while Prone. We surface 0 here for UI / AI purposes
        // (you can't *walk* while prone), but `can_consume_resource`
        // still allows the half-speed payment for Stand Up.
        if self.has_condition(Condition::Prone)
            || self.conditions.keys().any(|c| c.zeros_movement())
        {
            return 0.0;
        }
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
        modifier_from_score(self.dexterity)
    }

    pub fn roll_initiative(&mut self, roller: &mut impl Roller) {
        let rolled = roller.roll(&Dice::new(1, 20)) as i32;
        self.initiative = Some(rolled + self.initiative_mod());
    }

    /// Top-of-turn refresh: movement and action-economy slots regenerate,
    /// and any condition with `UntilStartOfNextTurn` (e.g. Dodge) expires.
    /// Returns the conditions that were cleared so the engine can log them.
    pub fn reset_for_new_round(&mut self) -> Vec<Condition> {
        self.movement = self.speed();
        self.action_slots = 1;
        self.bonus_action_slots = 1;
        self.reaction_slots = 1;
        // Once-per-turn flags reset at start of turn. Sneak Attack:
        // available again. Help grants from this actor live with the
        // helped actor, so we don't clear them here.
        self.sneak_attack_used = false;
        let mut expired = self.clear_until_next_turn_conditions();
        // 5e: Dodge / Disengage / Helped end at the start of the holder's
        // next turn regardless of whatever timer was used to install
        // them. Force-clear those here so a Permanent-timer Dodge from
        // a test or alternate code path still drops on the right tick.
        for c in [
            Condition::Dodging,
            Condition::Disengaging,
            Condition::Helped,
        ] {
            if self.remove_condition(c) {
                expired.push(c);
            }
        }
        expired
    }

    pub fn is_dodging(&self) -> bool {
        self.has_condition(Condition::Dodging)
            && !self.has_condition(Condition::Incapacitated)
            && !self.has_condition(Condition::Stunned)
            && !self.has_condition(Condition::Restrained)
    }

    pub fn set_dodging(&mut self, on: bool) {
        if on {
            // Dodge ends at the start of the actor's next turn (5e).
            self.add_condition(Condition::Dodging, ConditionTimer::UntilStartOfNextTurn);
        } else {
            self.remove_condition(Condition::Dodging);
        }
    }

    pub fn is_disengaging(&self) -> bool {
        self.has_condition(Condition::Disengaging)
    }

    /// Legacy alias — same semantics as `is_disengaging`.
    pub fn is_disengaged(&self) -> bool {
        self.is_disengaging()
    }

    pub fn set_disengaging(&mut self, on: bool) {
        if on {
            self.add_condition(Condition::Disengaging, ConditionTimer::UntilStartOfNextTurn);
        } else {
            self.remove_condition(Condition::Disengaging);
        }
    }

    /// Legacy alias — same semantics as `set_disengaging`.
    pub fn set_disengaged(&mut self, on: bool) {
        self.set_disengaging(on);
    }

    /// Identity of the helper who granted advantage to this actor, if
    /// any. Returns the first helper id we find — `set_help_grant`
    /// keeps the map at most one entry, so this is unambiguous.
    pub fn helped_by(&self) -> Option<usize> {
        self.help_grants.keys().next().copied()
    }

    /// True if there's an active grant on this actor against `target_id`.
    /// Used by the attack-mode computation to decide whether to fold in
    /// advantage from a pending Help.
    pub fn helped_against(&self, target_id: usize) -> bool {
        self.help_grant(target_id)
    }

    /// Flat to-hit / save bonus contributed by Bless. Returns +2 (the
    /// d4 average) when the actor is Blessed; otherwise 0.
    pub fn bless_bonus(&self) -> i32 {
        if self.is_blessed() { 2 } else { 0 }
    }

    pub fn action_slots(&self) -> u32 {
        self.action_slots
    }

    pub fn bonus_action_slots(&self) -> u32 {
        self.bonus_action_slots
    }

    /// Heal HP. A Dying or Stable actor with `amount > 0` snaps back to
    /// Active at exactly `amount` HP (5e: regaining HP from 0 sets you
    /// to the new value). Active actors heal up to their max.
    pub fn heal(&mut self, amount: u32) -> HealOutcome {
        if amount == 0 {
            return HealOutcome::AlreadyFull;
        }
        let cap = self.max_hitpoints();
        match self.hp_state {
            HpState::Dead => HealOutcome::NoOp,
            HpState::Dying { .. } | HpState::Stable => {
                self.hp_state = HpState::Active;
                self.hitpoints = amount.min(cap);
                self.remove_condition(Condition::Unconscious);
                HealOutcome::Revived
            }
            HpState::Active => {
                let new_hp = self.hitpoints.saturating_add(amount).min(cap);
                if new_hp == self.hitpoints {
                    HealOutcome::AlreadyFull
                } else {
                    self.hitpoints = new_hp;
                    HealOutcome::Healed
                }
            }
        }
    }

    pub fn spell_save_dc(&self, ability: AbilityScoreType) -> i32 {
        8 + self.proficiency_bonus() + modifier_from_score(self.ability_score(ability))
    }

    pub fn spell_attack_modifier(&self, ability: AbilityScoreType) -> i32 {
        self.proficiency_bonus() + modifier_from_score(self.ability_score(ability))
    }

    /// Apply `raw` damage of type `dt`, factoring in immunity / resistance
    /// / vulnerability and absorbing through any temp HP first. Returns
    /// `(outcome, final_amount)` where `final_amount` is the actual HP
    /// delta that landed (after all reductions and temp-HP absorption).
    pub fn take_typed_damage(&mut self, raw: u32, dt: DamageType) -> (DamageOutcome, u32) {
        let scaled = self.effective_damage(raw, dt);
        if scaled == 0 {
            return (
                match self.hp_state {
                    HpState::Dying { .. } | HpState::Stable | HpState::Dead => {
                        DamageOutcome::DyingFailure
                    }
                    HpState::Active => DamageOutcome::Reduced,
                },
                0,
            );
        }
        // Burn temp HP first; only the leftover hits real HP. Temp HP
        // is only relevant for Active actors — Dying / Stable creatures
        // route damage straight into death-save failures.
        if matches!(self.hp_state, HpState::Active) {
            let absorbed = scaled.min(self.temp_hp);
            self.temp_hp -= absorbed;
            let to_hp = scaled - absorbed;
            if to_hp == 0 {
                return (DamageOutcome::Reduced, 0);
            }
            (self.take_damage(to_hp), to_hp)
        } else {
            (self.take_damage(scaled), scaled)
        }
    }

    pub fn take_damage(&mut self, amount: u32) -> DamageOutcome {
        match self.hp_state {
            HpState::Stable => {
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
            HpState::Dead => DamageOutcome::DyingFailure,
            HpState::Active => {
                let after_temp = if self.temp_hp >= amount {
                    self.temp_hp -= amount;
                    return DamageOutcome::Reduced;
                } else {
                    let r = amount - self.temp_hp;
                    self.temp_hp = 0;
                    r
                };
                self.hitpoints = self.hitpoints.saturating_sub(after_temp);
                if self.hitpoints == 0 {
                    if self.rolls_death_saves {
                        self.hp_state = HpState::Dying {
                            successes: 0,
                            failures: 0,
                        };
                        // Falling unconscious is part of going Dying.
                        self.add_condition(Condition::Unconscious, ConditionTimer::Permanent);
                        self.add_condition(Condition::Prone, ConditionTimer::Permanent);
                        DamageOutcome::Downed
                    } else {
                        self.hp_state = HpState::Dead;
                        DamageOutcome::Killed
                    }
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
            self.hp_state = HpState::Active;
            self.hitpoints = 1;
            self.remove_condition(Condition::Unconscious);
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
        modifier_from_score(self.strength)
            + self.proficiency_bonus()
            + self.attack_bonus_buff
            + self.condition_attack_bonus()
    }

    pub fn damage_bonus(&self) -> i32 {
        modifier_from_score(self.strength)
    }

    /// Class-feature gates (Second Wind, Action Surge, etc.).
    pub fn feature_available(&self, tag: &'static str) -> bool {
        self.features_remaining.contains(tag)
    }

    pub fn spend_feature(&mut self, tag: &'static str) -> bool {
        self.features_remaining.remove(tag)
    }

    /// Has the rogue used their once-per-turn Sneak Attack already?
    pub fn sneak_attack_used(&self) -> bool {
        self.sneak_attack_used
    }

    pub fn mark_sneak_attack_used(&mut self) {
        self.sneak_attack_used = true;
    }

    /// Convenience: check Bless condition without callers having to
    /// import the Condition enum just for this single test.
    pub fn is_blessed(&self) -> bool {
        self.has_condition(Condition::Blessed)
    }

    /// Ability-mod + proficiency bonus for `ability` (the standard 5e
    /// "ability attack bonus" used by spells). `attack_bonus()` is
    /// STR-locked for melee weapons; this lets callers pick the right
    /// stat for spell attacks (INT for wizard, WIS for cleric, etc.).
    pub fn ability_attack_bonus(&self, ability: AbilityScoreType) -> i32 {
        modifier_from_score(self.ability_score(ability)) + self.proficiency_bonus()
    }

    /// Record that `helper_id` Helped this actor against `target_id`.
    /// The helped actor's next attack against `target_id` benefits from
    /// advantage; the grant is consumed (cleared) by `consume_help_for`.
    pub fn help_grant(&self, target_id: usize) -> bool {
        self.help_grants.values().any(|t| *t == target_id)
    }

    pub fn add_help_grant(&mut self, helper_id: usize, target_id: usize) {
        self.help_grants.insert(helper_id, target_id);
    }

    /// Set or clear a Help grant on this actor (legacy single-grant API).
    /// `Some(g)` overwrites any prior grant; `None` clears all grants.
    pub fn set_help_grant(&mut self, grant: Option<HelpGrant>) {
        self.help_grants.clear();
        if let Some(g) = grant {
            self.help_grants.insert(g.helper_id, g.against);
        }
    }

    /// Consume one Help grant against `target_id` (if any). Returns true
    /// if a grant was consumed — caller folds that into advantage logic.
    pub fn consume_help_for(&mut self, target_id: usize) -> bool {
        let helper = self
            .help_grants
            .iter()
            .find(|(_, t)| **t == target_id)
            .map(|(h, _)| *h);
        match helper {
            Some(h) => {
                self.help_grants.remove(&h);
                true
            }
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::creatures::skeletons::SKELETON_TEMPLATE;
    use crate::actors::creatures::slimes::SLIME_TEMPLATE;
    use crate::actors::creatures::zombies::ZOMBIE_TEMPLATE;
    use crate::engine::dice::FastRandRoller;

    fn make(ct: &'static CreatureTemplate) -> ActorInstance {
        ActorInstance::from_creature_template(
            ct,
            Coordinate::new(0, 0),
            0,
            &mut FastRandRoller::with_seed(1),
            0,
        )
        .unwrap()
    }

    #[test]
    fn poison_immunity_zeroes_damage() {
        let z = make(&ZOMBIE_TEMPLATE);
        assert_eq!(z.effective_damage(10, DamageType::Poison), 0);
        assert_eq!(z.effective_damage(10, DamageType::Slashing), 10);
    }

    #[test]
    fn skeleton_doubles_bludgeoning() {
        let s = make(&SKELETON_TEMPLATE);
        assert_eq!(s.effective_damage(7, DamageType::Bludgeoning), 14);
        assert_eq!(s.effective_damage(99, DamageType::Poison), 0);
        assert_eq!(s.effective_damage(7, DamageType::Slashing), 7);
    }

    #[test]
    fn resistance_halves_round_down() {
        // Slime resists piercing / slashing (physical weapons gum up).
        let s = make(&SLIME_TEMPLATE);
        assert_eq!(s.effective_damage(7, DamageType::Piercing), 3);
        assert_eq!(s.effective_damage(0, DamageType::Piercing), 0);
        // Acid is immune (zeroed).
        assert_eq!(s.effective_damage(7, DamageType::Acid), 0);
    }

    #[test]
    fn temp_hp_does_not_stack() {
        let mut s = make(&SKELETON_TEMPLATE);
        assert!(s.grant_temp_hp(5));
        assert_eq!(s.temp_hp(), 5);
        assert!(!s.grant_temp_hp(3));
        assert_eq!(s.temp_hp(), 5);
        assert!(s.grant_temp_hp(8));
        assert_eq!(s.temp_hp(), 8);
    }
}
