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
    /// Attack-roll buff deltas to roll back on drop. Each entry is the
    /// signed amount that was added by the spell (often +2 for Bless);
    /// cleanup negates the delta. Same shape as `save_buffs`.
    pub attack_buffs: Vec<(usize, i32)>,
    pub save_buffs: Vec<(usize, i32)>,
}

impl ConcentrationData {
    /// Build with empty buff vecs — convenience for spells that only
    /// install conditions or that piggyback on concentration purely
    /// for the duration timer.
    pub fn with_conditions(spell_name: impl Into<String>, conditions: Vec<(usize, Condition)>) -> Self {
        Self {
            spell_name: spell_name.into(),
            conditions,
            attack_buffs: Vec::new(),
            save_buffs: Vec::new(),
        }
    }
}

/// How damage was filtered by an actor's resistances / immunities /
/// vulnerabilities. `DealDamage` reads `kind` for log flavor and applies
/// `amount` as the post-filter HP delta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DamageMod {
    pub amount: u32,
    pub kind: DamageModKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DamageModKind {
    Normal,
    Resisted,
    Vulnerable,
    Immune,
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
use crate::engine::types::{Coordinate, DamageReaction, DamageType};
use crate::items::item_template::{Item, ItemBonuses};
use crate::{
    actions::action_template::Action,
    engine::{
        types::{AbilityScoreType, DamageModifier, Language, Size, Skill, SpecialSense},
        util::modifier_from_score,
    },
};
use std::collections::{HashMap, HashSet};

use std::error::Error;

/// 5e-style damage adjustment. Resistances halve incoming damage of a
/// given type; immunities zero it; vulnerabilities double it. Resolution
/// order matters (immunity beats vulnerability beats resistance) so we
/// pick the most-specific outcome at the call site rather than naïvely
/// stacking multipliers.
#[derive(Debug, Default, Clone)]
pub struct DamageAdjustments {
    pub resistances: HashSet<DamageType>,
    pub immunities: HashSet<DamageType>,
    pub vulnerabilities: HashSet<DamageType>,
}

impl DamageAdjustments {
    /// Apply this actor's resistances/immunities/vulnerabilities to a raw
    /// damage amount. 5e resolution: immunity → 0; vulnerability without
    /// immunity → doubled; resistance otherwise → halved (rounded down).
    /// Multiple categories can't overlap on the same type in our model;
    /// if they do, the order above wins.
    pub fn apply(&self, dt: DamageType, amount: u32) -> u32 {
        if self.immunities.contains(&dt) {
            return 0;
        }
        if self.vulnerabilities.contains(&dt) {
            return amount.saturating_mul(2);
        }
        if self.resistances.contains(&dt) {
            return amount / 2;
        }
        amount
    }
}

pub struct CreatureTemplate {
    pub name: &'static str,
    /// Single-character glyph for the map. Convention: capital letter
    /// matching the species (Z for zombie, S for skeleton, etc.). Multiple
    /// instances on the same team look identical on the map; the side
    /// panel and log disambiguate via the instance-numbered name.
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
    /// Per-damage-type modifiers (resistance / immunity / vulnerability).
    /// Empty for creatures that take damage normally. Skeletons should
    /// be vulnerable to Bludgeoning; zombies immune to Poison; demons
    /// resistant to Fire, etc. Looked up by `damage_modifier` on the
    /// instance.
    pub damage_modifiers: HashMap<DamageType, DamageModifier>,
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
    /// Convert a 1-based spell level to a 0-based index, returning None
    /// for level 0 (cantrips don't use slots in 5e). All public methods
    /// guard on this so an accidental `consume_spell_slot(0)` no-ops
    /// instead of wrapping the unsigned subtraction into a huge index.
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
    /// Damage types this actor resists (half damage). Cloned from the
    /// creature template at spawn; future buffs/debuffs can mutate.
    damage_resistances: HashSet<DamageType>,
    /// Damage types this actor takes 0 damage from.
    damage_immunities: HashSet<DamageType>,
    /// Damage types this actor takes double damage from.
    damage_vulnerabilities: HashSet<DamageType>,
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
    /// See `CreatureTemplate.resistances`. Cloned at instantiation; an
    /// equip / spell could mutate this at runtime, but no current effect
    /// does so.
    resistances: HashSet<DamageType>,
    vulnerabilities: HashSet<DamageType>,
    immunities: HashSet<DamageType>,
    /// 5e temporary hit points. Damage drains temp HP before regular HP.
    /// Doesn't stack: a new grant replaces existing temp HP only if larger
    /// (see `gain_temp_hp`). Cleared on long rest.
    temp_hp: u32,
    /// 5e Dodge action: attacks against this actor have disadvantage and
    /// they make DEX saves with advantage, until the start of their next
    /// turn. `reset_for_new_round` clears this on the actor's own turn.
    dodging: bool,
    /// 5e Disengage action: this actor's movement doesn't provoke
    /// opportunity attacks for the rest of the turn. Cleared on the next
    /// `reset_for_new_round`.
    disengaging: bool,
    /// Character level. Starts at 1; the multi-encounter loop's long-rest
    /// hook bumps this on hitting an XP threshold. Today only PCs (team
    /// 0) accumulate XP and level up — monsters keep level 1 and skip
    /// the threshold check.
    level: u32,
    /// Total XP earned since spawn. Reset is intentional on PC death so
    /// future "respawn at last campsite" mechanics can rebuild it; we
    /// don't decrement on level up so total-earned stays inspectable.
    xp: u32,
    /// Per-damage-type modifier table copied from the creature template.
    /// Mutable on the instance so future buffs / curses can flip a
    /// creature's resistance profile mid-fight (Bless, Protection from
    /// Energy, etc.) without rebuilding from the template.
    damage_modifiers: HashMap<DamageType, DamageModifier>,
    /// Temporary hit points (5e). Absorbed first by `take_damage` and
    /// don't stack — a new pool replaces the old if larger, otherwise
    /// the old wins. Cleared by long rest. Doesn't count toward
    /// `max_hitpoints`; pure damage soak.
    temp_hp: u32,
    /// Set by the Dodge action; cleared at the start of the actor's next
    /// turn. While true, attacks against this actor have disadvantage
    /// (5e: Dodge action) and they have advantage on DEX saves. Falls
    /// off automatically if they become Incapacitated or Stunned.
    dodging: bool,
    /// Set by the Disengage action; cleared at the start of the actor's
    /// next turn. Suppresses opportunity attacks fired by other actors
    /// when this actor leaves a threatened tile.
    disengaging: bool,
    /// Tally of attack-roll bonuses contributed by Bless-style buffs.
    /// Read by `weapon_attack` / spell-attack helpers and added to the
    /// d20 + modifier total. Cleared on long rest; concentration spells
    /// drop it via their cleanup hook.
    attack_bonus_buff: i32,
    /// Same shape as `attack_bonus_buff` but applied to saving throws
    /// (Bless, Resistance, etc.).
    save_bonus_buff: i32,
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
            },
            actions: ct.actions.clone(),
            glyph: ct.glyph,
            damage_resistances: ct.damage_resistances.clone(),
            damage_immunities: ct.damage_immunities.clone(),
            damage_vulnerabilities: ct.damage_vulnerabilities.clone(),
            hp_state: HpState::Active,
            conditions: HashMap::new(),
            concentration: None,
            rolls_death_saves: ct.rolls_death_saves,
            resistances: ct.resistances.clone(),
            vulnerabilities: ct.vulnerabilities.clone(),
            immunities: ct.immunities.clone(),
            temp_hp: 0,
            dodging: false,
            disengaging: false,
            level: 1,
            xp: 0,
            damage_modifiers: ct.damage_modifiers.clone(),
            temp_hp: 0,
            dodging: false,
            disengaging: false,
            attack_bonus_buff: 0,
            save_bonus_buff: 0,
        })
    }

    pub fn is_disengaging(&self) -> bool {
        self.disengaging
    }

    pub fn set_disengaging(&mut self, value: bool) {
        self.disengaging = value;
    }

    /// Reaction for a specific damage type, defaulting to Normal when no
    /// entry exists. The DealDamage side-effect applies this to scale the
    /// raw amount before subtracting from HP.
    pub fn damage_reaction(&self, damage_type: DamageType) -> DamageReaction {
        self.damage_reactions
            .get(&damage_type)
            .copied()
            .unwrap_or(DamageReaction::Normal)
    }

    pub fn rolls_death_saves(&self) -> bool {
        self.rolls_death_saves
    }

    /// First action in the actor's list whose `name()` matches `name`.
    /// Convenience used by AI / tests / engine helpers that look up
    /// canonical actions like "move", "skip", "stand", "dodge".
    pub fn find_action(
        &self,
        name: &str,
    ) -> Option<&'static (dyn Action + Send + Sync)> {
        self.actions.iter().find(|a| a.name() == name).copied()
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

    /// Restore full HP, all spell slots, clear non-permanent conditions,
    /// concentration and any temp HP. 5e long rest semantics — at the
    /// multi-encounter game-loop boundary, this is what "rest between
    /// fights" means.
    pub fn long_rest(&mut self) {
        self.hp_state = HpState::Active;
        self.hitpoints = self.max_hitpoints();
        self.temp_hp = 0;
        self.spell_slot_manager.restore_spell_slots();
        self.conditions.clear();
        self.concentration = None;
        self.temp_hp = 0;
        self.dodging = false;
        self.disengaging = false;
        self.attack_bonus_buff = 0;
        self.save_bonus_buff = 0;
    }

    pub fn temp_hp(&self) -> u32 {
        self.temp_hp
    }

    /// Set temporary HP. 5e: temp HP doesn't stack — a new application
    /// keeps the higher value rather than summing. Returns the value the
    /// buffer holds after the call.
    pub fn gain_temp_hp(&mut self, amount: u32) -> u32 {
        self.temp_hp = self.temp_hp.max(amount);
        self.temp_hp
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

    /// 5e proficiency bonus, derived from level. Used by save and attack
    /// roll modifiers when the actor is proficient in the relevant ability.
    pub fn proficiency_bonus(&self) -> i32 {
        crate::engine::util::proficiency_bonus_from_level(self.level)
    }

    pub fn is_save_proficient(&self, ability: AbilityScoreType) -> bool {
        self.proficient_saves.contains(&ability)
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
    /// Returns false (and does nothing) if the actor is immune to the
    /// condition via `condition_immunities`.
    pub fn add_condition(&mut self, c: Condition, timer: ConditionTimer) -> bool {
        if self.condition_immunities.contains(&c) {
            return false;
        }
        self.conditions.insert(c, timer).is_none()
    }

    pub fn is_immune_to_condition(&self, c: Condition) -> bool {
        self.condition_immunities.contains(&c)
    }

    pub fn temp_hitpoints(&self) -> u32 {
        self.temp_hitpoints
    }

    /// Add temp HP. 5e: temp HP doesn't stack — the higher value wins.
    /// Returns true if temp HP changed (e.g. because the new value was
    /// higher than the existing pool).
    pub fn grant_temp_hp(&mut self, amount: u32) -> bool {
        if amount > self.temp_hitpoints {
            self.temp_hitpoints = amount;
            true
        } else {
            false
        }
    }

    /// Apply resistance / vulnerability / immunity to a raw damage roll.
    /// Order: immunity (zero) > vulnerability (×2) > resistance (½).
    /// `DamageResistant` condition further halves on top — stacks with
    /// damage-type resistance for half-of-half = quarter damage when both
    /// apply; matches the "general damage reduction" intent of effects
    /// like Stoneskin.
    pub fn effective_damage(
        &self,
        amount: u32,
        damage_type: crate::engine::types::DamageType,
    ) -> u32 {
        if self.immunities.contains(&damage_type) {
            return 0;
        }
        let mut amt = amount;
        if self.vulnerabilities.contains(&damage_type) {
            amt = amt.saturating_mul(2);
        }
        if self.resistances.contains(&damage_type) {
            amt /= 2;
        }
        if self.has_condition(Condition::DamageResistant) {
            amt /= 2;
        }
        amt
    }

    /// Remove a condition. Returns true if the condition was present.
    pub fn remove_condition(&mut self, c: Condition) -> bool {
        self.conditions.remove(&c).is_some()
    }

    pub fn conditions(&self) -> &HashMap<Condition, ConditionTimer> {
        &self.conditions
    }

    /// Decrement every `Rounds(n)` timer by 1 and report which conditions
    /// expired (were removed because their timer hit 0). `Permanent` and
    /// `UntilStartOfNextTurn` timers are untouched here — the latter is
    /// cleared by `clear_until_next_turn_conditions` at turn-start.
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
            ConditionTimer::Rounds(n) => {
                *timer = ConditionTimer::Rounds(n - 1);
                true
            }
        });
        expired
    }

    /// Clear every condition with the `UntilStartOfNextTurn` timer. The
    /// engine calls this when it advances to this actor's turn — Dodge
    /// and similar self-buffs end here.
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

    /// True if any active condition's `blocks_action_economy` clause
    /// (Stunned / Incapacitated / Paralyzed) is set. Used by the resource
    /// gate to lock out Action / BonusAction / Reaction / SpellSlot /
    /// Movement uniformly — these conditions all share the 5e
    /// "Incapacitated" baseline.
    fn is_incapacitated(&self) -> bool {
        self.conditions
            .keys()
            .any(|c| c.blocks_action_economy())
    }

    pub fn can_consume_resource(&self, resource: Resource) -> bool {
        // Stunned actors lose their entire action economy. Incapacitated
        // is similar but movement still works. Prone is NOT checked here
        // for Movement: stand-up itself pays in Movement, so blocking the
        // resource here would create a catch-22. Move-the-action is still
        // blocked because `remaining_movement()` returns 0 when Prone or
        // Restrained, which makes `path_cost_to` find no path.
        let stunned = self.has_condition(Condition::Stunned);
        let incapacitated = self.has_condition(Condition::Incapacitated);
        let action_blocked = stunned || incapacitated;
        match resource {
            Resource::Movement(amt) => {
                if incap {
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
        // Shield of Faith — concentration buff worth +2 AC. Stack with
        // item-AC bonuses; the spell drops if the caster's concentration
        // breaks, removing the condition.
        let shield_bonus = if self.has_condition(Condition::ShieldOfFaith) {
            2
        } else {
            0
        };
        (self.base_ac as i32 + bonus + shield_bonus).max(0) as u32
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

    /// Flat bonus from buff conditions (e.g. Bless contributes +2 average,
    /// modeled as a flat bonus for simplicity rather than a separate die
    /// roll). Saves and attacks pull from the same set today, so the two
    /// public accessors share an internal sum.
    fn buff_flat_bonus(&self) -> i32 {
        let mut bonus = 0;
        if self.has_condition(Condition::Blessed) {
            bonus += 2;
        }
        bonus
    }

    pub fn condition_save_bonus(&self) -> i32 {
        self.buff_flat_bonus()
    }

    pub fn condition_attack_bonus(&self) -> i32 {
        self.buff_flat_bonus()
    }

    pub fn remaining_movement(&self) -> f32 {
        // Prone, Stunned, and Restrained all zero out movement (Restrained
        // by RAW, the others by our conflated model). Incapacitated does
        // NOT zero movement — the actor can still walk, just not act.
        if self.has_condition(Condition::Prone)
            || self.has_condition(Condition::Stunned)
            || self.has_condition(Condition::Restrained)
        {
            return 0.0;
        }
        self.movement
    }

    /// Flat AC contribution from active conditions (e.g. Shield of Faith
    /// gives +2). Folded into `armor_class` so all attack-vs-AC checks
    /// pick it up without extra plumbing.
    pub fn condition_ac_bonus(&self) -> i32 {
        let mut bonus = 0;
        if self.has_condition(Condition::Shielded) {
            bonus += 2;
        }
        bonus
    }

    /// Flat to-hit / save bonus from buff conditions (Bless gives +1d4 in
    /// 5e; we use a flat +2 — the d4 average — to avoid dragging another
    /// die roll through every code path). Folded into the relevant
    /// accessors below.
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

    /// Top-of-turn refresh: movement and action-economy slots regenerate,
    /// and any condition with `UntilStartOfNextTurn` (e.g. Dodge) expires.
    /// Returns the conditions that were cleared so the engine can log them.
    pub fn reset_for_new_round(&mut self) -> Vec<Condition> {
        self.movement = self.speed();

        // TODO: pull from function
        self.action_slots = 1;
        self.bonus_action_slots = 1;
        self.reaction_slots = 1;
        // Dodge / Disengage are turn-scoped: they clear at the start of
        // this actor's next turn (5e RAW). We zero them here so the buff
        // / OA suppression only lasts one round.
        self.dodging = false;
        self.disengaging = false;
        // TODO: legendary actions

        // Dodge / Disengage are "until the start of your next turn"
        // effects. Clear them at turn-start so the action only buffs
        // the next round of incoming events, not later rounds too.
        self.dodging = false;
        self.disengaging = false;
    }

    pub fn is_dodging(&self) -> bool {
        // 5e: Dodge fails if you're Incapacitated or your speed is 0.
        // We model the "speed 0" case implicitly via Stunned/Restrained
        // (which set remaining_movement to 0) and check Incapacitated
        // explicitly so the buff drops the moment the condition lands.
        self.dodging
            && !self.has_condition(Condition::Incapacitated)
            && !self.has_condition(Condition::Stunned)
            && !self.has_condition(Condition::Restrained)
    }

    pub fn set_dodging(&mut self, v: bool) {
        self.dodging = v;
    }

    pub fn is_disengaging(&self) -> bool {
        self.disengaging
    }

    pub fn set_disengaging(&mut self, v: bool) {
        self.disengaging = v;
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

    pub fn is_dodging(&self) -> bool {
        self.dodging
    }

    pub fn set_dodging(&mut self, on: bool) {
        self.dodging = on;
    }

    pub fn is_disengaging(&self) -> bool {
        self.disengaging
    }

    pub fn set_disengaging(&mut self, on: bool) {
        self.disengaging = on;
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
    /// max — the item-aware `max_hitpoints()`, so amulet bonuses count
    /// toward the cap. Dead actors are unrecoverable here.
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

    /// 5e spell save DC: 8 + proficiency bonus + spellcasting ability
    /// modifier. Actions that force saves call this on the caster to set
    /// their DC.
    pub fn spell_save_dc(&self, ability: AbilityScoreType) -> i32 {
        8 + self.proficiency_bonus() + modifier_from_score(self.ability_score(ability))
    }

    /// 5e spell attack modifier: proficiency bonus + spellcasting ability
    /// modifier. Used by spells with attack rolls (Fire Bolt, Eldritch
    /// Blast). Caller adds this to the d20.
    pub fn spell_attack_modifier(&self, ability: AbilityScoreType) -> i32 {
        self.proficiency_bonus() + modifier_from_score(self.ability_score(ability))
    }

    /// Damage modifier for `dt`, or `None` if the actor takes normal
    /// damage of this type. Read-only; populated from the creature
    /// template at spawn.
    pub fn damage_modifier(&self, dt: DamageType) -> Option<DamageModifier> {
        self.damage_modifiers.get(&dt).copied()
    }

    /// Apply 5e damage rules to a raw amount: vulnerability doubles,
    /// resistance halves, immunity zeroes. Returns the post-modifier
    /// value so callers can decide whether to log a "no effect" line.
    pub fn modified_damage(&self, raw: u32, dt: DamageType) -> u32 {
        match self.damage_modifier(dt) {
            Some(m) => m.apply(raw),
            None => raw,
        }
    }

    pub fn temp_hp(&self) -> u32 {
        self.temp_hp
    }

    /// Grant temp HP. 5e: temp HP doesn't stack — use the new pool only
    /// if it's larger than the current pool. Returns whether the new
    /// pool replaced the old one.
    pub fn grant_temp_hp(&mut self, amount: u32) -> bool {
        if amount > self.temp_hp {
            self.temp_hp = amount;
            true
        } else {
            false
        }
    }

    pub fn is_resistant_to(&self, dt: DamageType) -> bool {
        self.resistances.contains(&dt)
    }

    pub fn is_vulnerable_to(&self, dt: DamageType) -> bool {
        self.vulnerabilities.contains(&dt)
    }

    pub fn is_immune_to(&self, dt: DamageType) -> bool {
        self.immunities.contains(&dt)
    }

    /// Apply 5e resistance / vulnerability / immunity scaling. Immunity
    /// short-circuits to 0; resistance halves (round down); vulnerability
    /// doubles. RAW: at most one of resistance / vulnerability applies, but
    /// since they're stored in disjoint sets that invariant is implicit.
    pub fn adjusted_damage(&self, amount: u32, dt: DamageType) -> u32 {
        if self.is_immune_to(dt) {
            return 0;
        }
        if self.is_resistant_to(dt) {
            return amount / 2;
        }
        if self.is_vulnerable_to(dt) {
            return amount.saturating_mul(2);
        }
        amount
    }

    pub fn temp_hp(&self) -> u32 {
        self.temp_hp
    }

    /// 5e temporary HP doesn't stack; the new grant replaces the old one
    /// only if it's higher. Returns the new temp HP value.
    pub fn gain_temp_hp(&mut self, amount: u32) -> u32 {
        if amount > self.temp_hp {
            self.temp_hp = amount;
        }
        self.temp_hp
    }

    /// Apply 5e resistance / vulnerability / immunity to a raw damage
    /// amount. Immunity wins (returns 0); vulnerability doubles;
    /// resistance halves; nothing matches → unchanged. Halving rounds
    /// down per RAW.
    pub fn apply_damage_modifiers(&self, amount: u32, ty: DamageType) -> u32 {
        if self.damage_immunities.contains(&ty) {
            return 0;
        }
        let resisted = self.damage_resistances.contains(&ty);
        let vulnerable = self.damage_vulnerabilities.contains(&ty);
        match (resisted, vulnerable) {
            (true, false) => amount / 2,
            (false, true) => amount.saturating_mul(2),
            // 5e: resist + vulnerable on the same type → unchanged.
            (true, true) | (false, false) => amount,
        }
    }

    pub fn is_resistant_to(&self, ty: DamageType) -> bool {
        self.damage_resistances.contains(&ty)
    }

    pub fn is_vulnerable_to(&self, ty: DamageType) -> bool {
        self.damage_vulnerabilities.contains(&ty)
    }

    pub fn is_immune_to(&self, ty: DamageType) -> bool {
        self.damage_immunities.contains(&ty)
    }

    /// Apply this creature's resistance / immunity / vulnerability to
    /// `amount` of `dt` damage. Order: immunity (zeroes out) > vulnerability
    /// (doubles) > resistance (halves, rounded down). Returning the
    /// adjusted amount lets callers log the original / final pair if they
    /// care; today only `DealDamage` reads this.
    pub fn apply_damage_modifiers(&self, amount: u32, dt: DamageType) -> u32 {
        if self.damage_immunities.contains(&dt) {
            return 0;
        }
        let mut adj = amount;
        if self.damage_vulnerabilities.contains(&dt) {
            adj = adj.saturating_mul(2);
        }
        if self.damage_resistances.contains(&dt) {
            adj /= 2;
        }
        adj
    }

    pub fn is_immune_to(&self, dt: DamageType) -> bool {
        self.damage_immunities.contains(&dt)
    }

    pub fn is_resistant_to(&self, dt: DamageType) -> bool {
        self.damage_resistances.contains(&dt)
    }

    pub fn is_vulnerable_to(&self, dt: DamageType) -> bool {
        self.damage_vulnerabilities.contains(&dt)
    }

    pub fn take_damage(&mut self, amount: u32) -> DamageOutcome {
        // Temp HP only matters for Active actors — 5e: Dying/Stable
        // creatures don't carry temp HP through unconsciousness, and
        // damage to them goes straight to death saves, not the pool.
        let amount = if matches!(self.hp_state, HpState::Active) {
            if self.temp_hp >= amount {
                self.temp_hp -= amount;
                return DamageOutcome::Reduced;
            }
            let leftover = amount - self.temp_hp;
            self.temp_hp = 0;
            leftover
        } else {
            amount
        };
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
                // Drain temporary HP first; remainder hits real HP.
                let remaining = if self.temp_hitpoints >= amount {
                    self.temp_hitpoints -= amount;
                    0
                } else {
                    let r = amount - self.temp_hitpoints;
                    self.temp_hitpoints = 0;
                    r
                };
                self.hitpoints = self.hitpoints.saturating_sub(remaining);
                if self.hitpoints == 0 {
                    self.temp_hp = 0;
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

    /// 5e proficiency bonus, derived from level. The official table:
    /// L1-4 → +2, L5-8 → +3, L9-12 → +4, L13-16 → +5, L17-20 → +6.
    /// Closed form: ((level - 1) / 4) + 2. Monsters use the same curve
    /// (levels stuck at 1 → +2 across the board, which matches the MM's
    /// CR-derived proficiency for low-CR creatures).
    pub fn proficiency_bonus(&self) -> i32 {
        ((self.level.saturating_sub(1)) / 4 + 2) as i32
    }

    pub fn attack_bonus(&self) -> i32 {
        modifier_from_score(self.strength) + self.proficiency_bonus()
    }

    pub fn damage_bonus(&self) -> i32 {
        modifier_from_score(self.strength)
    }

    /// 5e proficiency bonus by level: +2 at 1-4, +3 at 5-8, +4 at 9-12,
    /// +5 at 13-16, +6 at 17+. Folded into attack bonuses, save DCs and
    /// the spell-attack modifier so a level-5 caster's Fire Bolt gets the
    /// canonical +1 jump.
    pub fn proficiency_bonus(&self) -> i32 {
        match self.level {
            0..=4 => 2,
            5..=8 => 3,
            9..=12 => 4,
            13..=16 => 5,
            _ => 6,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn damage_adjustments_resistance_halves() {
        let adj = DamageAdjustments {
            resistances: HashSet::from([DamageType::Fire]),
            ..Default::default()
        };
        assert_eq!(adj.apply(DamageType::Fire, 10), 5);
        assert_eq!(adj.apply(DamageType::Cold, 10), 10);
    }

    #[test]
    fn damage_adjustments_immunity_zeros() {
        let adj = DamageAdjustments {
            immunities: HashSet::from([DamageType::Poison]),
            ..Default::default()
        };
        assert_eq!(adj.apply(DamageType::Poison, 99), 0);
    }

    #[test]
    fn damage_adjustments_vulnerability_doubles() {
        let adj = DamageAdjustments {
            vulnerabilities: HashSet::from([DamageType::Bludgeoning]),
            ..Default::default()
        };
        assert_eq!(adj.apply(DamageType::Bludgeoning, 7), 14);
    }

    #[test]
    fn damage_adjustments_immunity_beats_vulnerability() {
        // Should never both be set in practice, but the resolver picks the
        // most-specific outcome when they do (immunity > vulnerability >
        // resistance).
        let adj = DamageAdjustments {
            immunities: HashSet::from([DamageType::Fire]),
            vulnerabilities: HashSet::from([DamageType::Fire]),
            ..Default::default()
        };
        assert_eq!(adj.apply(DamageType::Fire, 20), 0);
    }
}
