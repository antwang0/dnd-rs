use crate::conditions::{Condition, ConditionTimer};
use crate::engine::dice::{Dice, DiceExpr, Roller};
use crate::engine::types::DamageType;

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
    /// HP just hit 0; actor transitioned from combat-active to dying
    /// (rolls death saves). PCs only.
    Downed,
    /// HP just hit 0 and the actor doesn't roll death saves — they're
    /// dead outright. Monsters take this path; cleanup_dead_actors will
    /// remove them on the next pass.
    Killed,
    /// Actor was already dying (or stable, which gets re-downed); damage
    /// counts as a failed death save instead of an HP delta.
    DyingFailure,
}

/// State of an actor that's concentrating on a spell. Tracks what they
/// applied so dropping concentration can clean up automatically.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConcentrationData {
    /// Display name of the spell, used in logs ("X's concentration on
    /// Hold Person ends.").
    pub spell_name: String,
    /// Conditions this concentration applied. On drop, each is removed
    /// from its target. `(target_id, condition)`.
    pub conditions: Vec<(usize, Condition)>,
}

/// What `heal` did. Mirrors `DamageOutcome` for the inverse direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealOutcome {
    /// Active actor gained HP.
    Healed,
    /// Was Dying / Stable; healing brought them back to Active.
    Revived,
    /// Active actor was already at max HP — heal was a no-op.
    AlreadyFull,
    /// Dead actors can't be healed by ordinary means.
    NoOp,
}
use crate::engine::side_effects::Resource;
use crate::engine::types::Coordinate;
use crate::items::item_template::{Item, ItemBonuses};
use crate::{
    actions::action_template::Action,
    engine::{
        types::{AbilityScoreType, Language, Size, Skill, SpecialSense},
        util::modifier_from_score,
    },
};
use std::collections::{HashMap, HashSet};

use std::error::Error;

/// 5e damage adjustment. `Vulnerable` doubles incoming damage of that
/// type, `Resistant` halves (rounded down), `Immune` zeroes it. Applied
/// in `ActorInstance::damage_after_resistances` before HP changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DamageAdjust {
    Vulnerable,
    Resistant,
    Immune,
}

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
    pub items: Vec<&'static Item>,
    pub senses: HashSet<SpecialSense>,
    pub languages: HashSet<Language>,
    pub cr: f32,
    pub size: Size,
    pub actions: Vec<&'static (dyn Action + Send + Sync)>,
    /// Initial leveled spell slots by level. Index 0 = level-1 slots,
    /// index 1 = level-2 slots, etc. Empty / shorter vec = no slots at
    /// that level (cantrip-only or martial caster). Slots are spent
    /// during an encounter; restoration requires a long rest (not
    /// modeled today — slots stay drained between encounters).
    pub spell_slots_by_level: Vec<u32>,
    /// Whether this creature uses the dying / death-save state on
    /// reaching 0 HP. 5e: PCs roll death saves; monsters drop outright.
    /// Default for new templates: `false`. Player characters override
    /// to `true` so they get the standard 3-success / 3-failure cycle.
    pub rolls_death_saves: bool,
    /// Per-damage-type vulnerability/resistance/immunity. Empty by default
    /// — most creatures take baseline damage from everything. Skeletons
    /// pick `Vulnerable(Bludgeoning)` and `Immune(Poison)`; zombies might
    /// `Resistant(Necrotic)`. See `DamageAdjust`.
    pub damage_adjustments: HashMap<DamageType, DamageAdjust>,
    /// Ability score saving-throw proficiencies. Each listed ability
    /// adds the actor's proficiency bonus to that save's d20. Empty for
    /// monsters that don't have spelled-out save proficiencies.
    pub save_proficiencies: HashSet<AbilityScoreType>,
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
    /// Map glyph copied from the template. Static across an actor's life.
    glyph: char,
    /// Lifecycle state. Driven by `take_damage` (damage transitions
    /// `Active -> Dying`, hits on `Dying`/`Stable` add failures) and
    /// `apply_death_save` (rolls move within `Dying` and into `Stable`,
    /// `Dead`, or `Active` on a nat 20). See `HpState`.
    hp_state: HpState,
    /// Active status conditions, mapped to their per-application timer.
    /// `Permanent` entries persist until explicitly removed; `Rounds(n)`
    /// entries tick down on every initiative wrap and clear at 0.
    conditions: HashMap<Condition, ConditionTimer>,
    /// Currently-concentrated-on spell, if any. Each actor concentrates
    /// on at most one spell at a time (5e). Casting a new concentration
    /// spell or taking damage that fails a CON save drops it; the engine
    /// then removes any conditions the spell installed.
    concentration: Option<ConcentrationData>,
    /// Mirrored from `CreatureTemplate.rolls_death_saves`. Drives the
    /// 0-HP transition: true = enter Dying and roll saves; false = enter
    /// Dead immediately.
    rolls_death_saves: bool,
    /// Character level. Starts at 1; the multi-encounter loop's long-rest
    /// hook bumps this on hitting an XP threshold. Today only PCs (team
    /// 0) accumulate XP and level up — monsters keep level 1 and skip
    /// the threshold check.
    level: u32,
    /// Total XP earned since spawn. Reset is intentional on PC death so
    /// future "respawn at last campsite" mechanics can rebuild it; we
    /// don't decrement on level up so total-earned stays inspectable.
    xp: u32,
    /// Per-damage-type adjustment table (resistance / immunity /
    /// vulnerability). Read by `damage_after_resistances` to scale
    /// incoming damage before HP changes.
    damage_adjustments: HashMap<DamageType, DamageAdjust>,
    /// Abilities for which this actor adds proficiency bonus to saves.
    save_proficiencies: HashSet<AbilityScoreType>,
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
                ssi_by_lvl: ct
                    .spell_slots_by_level
                    .iter()
                    .map(|&n| SpellSlotInfo {
                        max_spell_slots: n,
                        spell_slots: n,
                    })
                    .collect(),
                warlock_ssi: SpellSlotInfo {
                    max_spell_slots: 0,
                    spell_slots: 0,
                },
                warlock_spell_slot_lvl: 0,
            },
            actions: ct.actions.clone(),
            glyph: ct.glyph,
            hp_state: HpState::Active,
            conditions: HashMap::new(),
            concentration: None,
            rolls_death_saves: ct.rolls_death_saves,
            level: 1,
            xp: 0,
            damage_adjustments: ct.damage_adjustments.clone(),
            save_proficiencies: ct.save_proficiencies.clone(),
        })
    }

    /// Resistance / vulnerability / immunity-aware damage scaling. 5e
    /// rounds resistance down (3 → 1). Immunity zeroes; vulnerability
    /// doubles (with saturation). Applied once per damage application
    /// before HP changes — does not stack.
    pub fn damage_after_resistances(&self, raw: u32, damage_type: DamageType) -> u32 {
        match self.damage_adjustments.get(&damage_type) {
            None => raw,
            Some(DamageAdjust::Immune) => 0,
            Some(DamageAdjust::Resistant) => raw / 2,
            Some(DamageAdjust::Vulnerable) => raw.saturating_mul(2),
        }
    }

    pub fn damage_adjustments(&self) -> &HashMap<DamageType, DamageAdjust> {
        &self.damage_adjustments
    }

    pub fn has_save_proficiency(&self, ability: AbilityScoreType) -> bool {
        self.save_proficiencies.contains(&ability)
    }

    /// 5e proficiency bonus by character level: +2 at 1-4, +3 at 5-8, etc.
    /// Used for save bonuses, attack bonuses (once wired through), and
    /// spell save DCs. Monsters with declared proficient saves use this
    /// same scale even when they're "level 1" — close enough for our
    /// model.
    pub fn proficiency_bonus(&self) -> i32 {
        2 + ((self.level.saturating_sub(1)) / 4) as i32
    }

    pub fn rolls_death_saves(&self) -> bool {
        self.rolls_death_saves
    }

    /// Sum every carried item's `ItemBonuses` into one struct. Stat
    /// accessors (`armor_class`, `speed`, `max_hitpoints`, etc.) fold
    /// this in so callers don't need to think about items at all.
    pub fn total_item_bonuses(&self) -> ItemBonuses {
        self.items
            .iter()
            .fold(ItemBonuses::default(), |acc, it| acc + it.bonuses)
    }

    pub fn items(&self) -> &[&'static Item] {
        &self.items
    }

    /// Add an item to this actor's inventory. Used by the auto-pickup
    /// hook in `MoveActor::apply` and by tests / character setup.
    pub fn pickup_item(&mut self, item: &'static Item) {
        self.items.push(item);
    }

    pub fn has_item_named(&self, name: &str) -> bool {
        self.items.iter().any(|i| i.name == name)
    }

    /// Remove the first item matching `name`. Returns true on success.
    /// Used by consumable on_use actions to remove the item after use.
    pub fn remove_item_by_name(&mut self, name: &str) -> bool {
        if let Some(pos) = self.items.iter().position(|i| i.name == name) {
            self.items.remove(pos);
            true
        } else {
            false
        }
    }

    /// Base actions plus one entry per unique consumable item the actor
    /// is carrying (deduped by item name). The prompt builder uses this
    /// so picking up a Healing Potion immediately surfaces "drink
    /// healing potion" in the action list without touching the action
    /// vec on the template.
    pub fn available_actions(
        &self,
    ) -> Vec<&'static (dyn Action + Send + Sync)> {
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

    /// Restore full HP, all spell slots, clear non-permanent conditions
    /// and concentration. 5e long rest semantics — at the multi-encounter
    /// game-loop boundary, this is what "rest between fights" means.
    pub fn long_rest(&mut self) {
        self.hp_state = HpState::Active;
        self.hitpoints = self.max_hitpoints();
        self.spell_slot_manager.restore_spell_slots();
        self.conditions.clear();
        self.concentration = None;
    }

    pub fn cr(&self) -> f32 {
        self.cr
    }

    /// XP a slain instance of this actor awards. Linear in CR
    /// (CR 1 → 200 XP, CR 2 → 400 XP). The 5e table is non-linear at
    /// the ends, but linear is good enough for the dungeon loop and
    /// keeps the ramp legible.
    pub fn xp_value(&self) -> u32 {
        (self.cr * 200.0).round().max(0.0) as u32
    }

    pub fn level(&self) -> u32 {
        self.level
    }

    pub fn xp(&self) -> u32 {
        self.xp
    }

    /// XP needed to reach the *next* level from current level. Linear
    /// curve `level * 300` — keeps the math readable in the UI and
    /// scales roughly with the difficulty ramp (cr_target × 0.5/encounter).
    /// Returns the cumulative XP threshold, not the delta from current.
    pub fn xp_threshold_for_next_level(&self) -> u32 {
        self.level * 300
    }

    /// Add XP earned (kill rewards, quest completion). Doesn't auto-level —
    /// `try_level_up` is called explicitly during long rest so leveling
    /// is a tidy between-encounter beat instead of a mid-fight power spike.
    pub fn award_xp(&mut self, amount: u32) {
        self.xp = self.xp.saturating_add(amount);
    }

    /// Promote a PC to the next level if they've crossed the threshold.
    /// Returns the new level on success. Today level-up just bumps base
    /// HP by 1d10+CON-mod (fighter-style) so the PC's max HP grows with
    /// the difficulty curve; ability scores and slot counts stay fixed
    /// until ASI/feat/casting progression are modeled.
    pub fn try_level_up(&mut self, roller: &mut impl Roller) -> Option<u32> {
        if self.xp < self.xp_threshold_for_next_level() {
            return None;
        }
        self.level += 1;
        let con_mod = modifier_from_score(self.constitution);
        let roll = roller.roll(&Dice::new(1, 10)) as i32;
        let gain = (roll + con_mod).max(1) as u32;
        self.base_hitpoints = self.base_hitpoints.saturating_add(gain);
        // Heal up by the same amount so a level on long rest feels like
        // a tangible HP gain rather than a stat-sheet curiosity.
        self.hitpoints = self.hitpoints.saturating_add(gain).min(self.max_hitpoints());
        Some(self.level)
    }

    pub fn is_concentrating(&self) -> bool {
        self.concentration.is_some()
    }

    pub fn concentration(&self) -> Option<&ConcentrationData> {
        self.concentration.as_ref()
    }

    /// Install a new concentration. Returns the previous concentration if
    /// any (caller is expected to clean up its effects via the engine).
    pub fn start_concentration(
        &mut self,
        data: ConcentrationData,
    ) -> Option<ConcentrationData> {
        self.concentration.replace(data)
    }

    /// End concentration and return its data. Returns `None` if the actor
    /// wasn't concentrating.
    pub fn end_concentration(&mut self) -> Option<ConcentrationData> {
        self.concentration.take()
    }

    pub fn has_condition(&self, c: Condition) -> bool {
        self.conditions.contains_key(&c)
    }

    /// Add a condition with the given timer. If the condition was already
    /// present, the timer is replaced (longer-lasting application overrides
    /// shorter — but for now we just take the new value either way; revisit
    /// when stacking semantics matter). Returns true if newly added.
    pub fn add_condition(&mut self, c: Condition, timer: ConditionTimer) -> bool {
        self.conditions.insert(c, timer).is_none()
    }

    /// Remove a condition. Returns true if the condition was present.
    pub fn remove_condition(&mut self, c: Condition) -> bool {
        self.conditions.remove(&c).is_some()
    }

    pub fn conditions(&self) -> &HashMap<Condition, ConditionTimer> {
        &self.conditions
    }

    /// Decrement every `Rounds(n)` timer by 1 and report which conditions
    /// expired (were removed because their timer hit 0). Permanent timers
    /// are untouched. The engine calls this on every round-end.
    pub fn tick_condition_timers(&mut self) -> Vec<Condition> {
        let mut expired = Vec::new();
        let snapshot: Vec<(Condition, ConditionTimer)> = self
            .conditions
            .iter()
            .map(|(c, t)| (*c, *t))
            .collect();
        for (c, timer) in snapshot {
            match timer {
                ConditionTimer::Permanent => {}
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
        // Stunned actors lose their entire action economy. Prone is NOT
        // checked here for Movement: stand-up itself pays in Movement, so
        // blocking the resource here would create a catch-22. Move-the-
        // action is still blocked because `remaining_movement()` returns 0
        // when Prone, which makes `path_cost_to` find no path.
        let stunned = self.has_condition(Condition::Stunned);
        match resource {
            Resource::Movement(amt) => {
                if stunned {
                    return false;
                }
                amt <= self.movement
            }
            Resource::SpellSlot(spell_lvl) => {
                if stunned {
                    return false;
                }
                self.spell_slot_manager.spell_slots(spell_lvl).spell_slots >= 1
            }
            Resource::Action => !stunned && self.action_slots >= 1,
            Resource::BonusAction => !stunned && self.bonus_action_slots >= 1,
            Resource::Reaction => !stunned && self.reaction_slots >= 1,
            Resource::LegendaryAction => !stunned && self.legendary_action_slots >= 1,
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
        let bonus = self.total_item_bonuses().ac;
        (self.base_ac as i32 + bonus).max(0) as u32
    }

    pub fn hitpoints(&self) -> u32 {
        self.hitpoints
    }

    pub fn max_hitpoints(&self) -> u32 {
        let bonus = self.total_item_bonuses().max_hp;
        (self.base_hitpoints as i32 + bonus).max(1) as u32
    }
    // TODO: bonus hitpoints?

    pub fn speed(&self) -> f32 {
        let bonus = self.total_item_bonuses().speed as f32;
        (self.base_speed + bonus).max(0.0)
    }

    /// Flat save bonus contributed by carried items. The engine adds this
    /// to the rolled save modifier; abstracting it lets `roll_save` ignore
    /// inventory details.
    pub fn item_save_bonus(&self) -> i32 {
        self.total_item_bonuses().save
    }

    pub fn remaining_movement(&self) -> f32 {
        if self.has_condition(Condition::Prone) || self.has_condition(Condition::Stunned) {
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

    /// Restore HP. A Dying or Stable actor with `amount > 0` snaps back to
    /// Active at exactly `amount` HP (5e: regaining HP from 0 sets you to
    /// the new value, not adds to it). Active actors heal up to their
    /// max. Dead actors are unrecoverable here. Caps against
    /// `max_hitpoints()` so item-granted max-HP bonuses (Amulet of Health,
    /// etc.) actually grant headroom for healing.
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

    /// 5e spell save DC: 8 + spellcasting ability modifier (we don't track
    /// proficiency yet; once we do, add it here). Actions that force saves
    /// call this on the caster to set their DC.
    pub fn spell_save_dc(&self, ability: AbilityScoreType) -> i32 {
        8 + modifier_from_score(self.ability_score(ability))
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
                    if self.rolls_death_saves {
                        self.hp_state = HpState::Dying {
                            successes: 0,
                            failures: 0,
                        };
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
