use crate::actions::action_template::Action;
use crate::conditions::{Condition, ConditionTimer};
use crate::engine::dice::{Dice, DiceExpr, Roller};
use crate::engine::side_effects::Resource;
use crate::engine::types::{
    AbilityScoreType, Coordinate, CreatureType, DamageModifier, DamageType, Language, Size, Skill,
    SpecialSense,
};
use crate::engine::util::modifier_from_score;
use crate::items::item_template::{Item, ItemBonuses};
use std::collections::{HashMap, HashSet};
use std::error::Error;

/// Features that refresh on a 5e short rest (Fighter's Second Wind +
/// Action Surge). The warlock's Pact Magic spell-slot refresh is handled
/// separately because it operates on the slot manager rather than the
/// feature-flag pool.
const SHORT_REST_FEATURES: &[&str] = &[
    "fighter.second_wind",
    "fighter.action_surge",
];

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
    /// 5e: making an attack ends Invisibility but not Greater Invisibility.
    /// Set true for concentration data whose effect ends when the caster
    /// makes any attack roll (clear_attack_advantage_riders consumes it).
    pub breaks_on_attack: bool,
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
    /// Bare concentration mark with no associated conditions to prune on
    /// drop — used by spells whose entire effect is the concentration
    /// marker itself (Crusader's Mantle, Crown of Stars, Mordenkainen's
    /// Sword: a passive aura / persistent presence on the caster, no
    /// per-target tag to remove).
    pub fn new(spell_name: impl Into<String>) -> Self {
        Self::with_conditions(spell_name, Vec::new())
    }

    pub fn with_conditions(
        spell_name: impl Into<String>,
        conditions: Vec<(usize, Condition)>,
    ) -> Self {
        Self {
            spell_name: spell_name.into(),
            conditions,
            attack_buffs: Vec::new(),
            save_buffs: Vec::new(),
            breaks_on_attack: false,
        }
    }

    /// Mark this concentration as ending when the caster makes any attack
    /// roll. Used by Invisibility (vanilla) but not Greater Invisibility.
    pub fn breaking_on_attack(mut self) -> Self {
        self.breaks_on_attack = true;
        self
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
    pub creature_type: CreatureType,
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
    /// HP to regenerate at end-of-round while combat-active. 0 (the
    /// default for ordinary monsters) disables the heal. Trolls set this
    /// to 3; future regenerators (e.g. vampires) plug in here.
    pub regen_per_round: u32,
    /// Damage types that suppress this creature's regeneration for one
    /// round (5e troll: fire / acid). When damage of one of these types
    /// lands, `regen_suppressed` flips on the instance; `round_end`
    /// clears it after skipping that round's heal.
    pub regen_suppressors: HashSet<DamageType>,
    /// 5e Legendary Resistance — number of times per long rest the creature
    /// can choose to succeed on a save it just failed. Read by
    /// `EncounterInstance::roll_save`: when a failed save would land and
    /// the actor's `legendary_resistance_remaining` counter is non-zero,
    /// the save is promoted to a pass and the counter is decremented.
    /// Long rest restores to this template max. 0 = no legendary
    /// resistance (the default for ordinary creatures).
    pub legendary_resistances: u32,
    /// 5e Evasion (Rogue 7, Monk 7): on DEX saves for half damage, take 0
    /// on a pass and half on a fail instead of half / full.
    pub has_evasion: bool,
    /// 5e Uncanny Dodge (Rogue 5): use reaction to halve damage from one
    /// attack you can see. Modeled as a passive flag checked in the
    /// attack resolution pipeline.
    pub has_uncanny_dodge: bool,
    /// 5e Displacer Beast trait: the creature projects a displaced image.
    /// Attacks against it have disadvantage. Breaks on damage; restores
    /// at the start of the creature's next turn.
    pub has_displacement: bool,
    /// 5e Barbarian Danger Sense (level 2): advantage on DEX saves against
    /// effects you can see while not blinded, deafened, or incapacitated.
    pub has_danger_sense: bool,
    /// 5e Pack Tactics (Wolf, Dire Wolf, Kobold): advantage on attack rolls
    /// when an ally is adjacent to the target. Read by `compute_attack_mode`.
    pub has_pack_tactics: bool,
    /// 5e Magic Resistance (Balor, Lich, Pit Fiend, etc.): advantage on
    /// saving throws against spells and other magical effects. Read by
    /// `compute_save_mode` — applies to every save the creature rolls
    /// (we don't yet distinguish spell vs non-spell save sources, so we
    /// conservatively grant advantage on all saves, matching the most
    /// common interpretation for combat engines).
    pub has_magic_resistance: bool,
    /// 5e Recharge ability: some creature abilities recharge on a d6 roll
    /// at the start of each turn (e.g. "Recharge 5-6" means the ability
    /// recharges if the d6 shows 5 or 6). Each entry is (action_name,
    /// min_roll) — the action becomes available again when the d6 >=
    /// min_roll. Empty for creatures without recharge abilities.
    pub recharge_abilities: Vec<(&'static str, u32)>,
    /// 5e Legendary Actions — number of legendary action points refreshed
    /// at the start of each of the creature's turns. Dragons get 3,
    /// liches 3, beholders 3, etc. 0 = no legendary actions (the default
    /// for ordinary creatures). The encounter loop grants this many
    /// LegendaryAction resource tokens at the start of the creature's
    /// turn and the AI spends them between other actors' turns.
    pub legendary_actions_per_round: u32,
    /// 5e Extra Attack — when this creature takes the Attack action, it
    /// can make two attacks instead of one. True for Fighters, Paladins,
    /// Rangers, Barbarians, Monks (level 5+), and monsters with
    /// Multiattack. Unlike class features, this is permanent and never
    /// consumed.
    pub has_extra_attack: bool,
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
        lvl.checked_sub(1).map(|n| n as usize)
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
    creature_type: CreatureType,
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
    /// 5e regenerator state: how much HP to recover each round-end while
    /// combat-active, and which damage types disable that heal for one
    /// round. `regen_suppressed` is set by `DealDamage` whenever damage
    /// of a suppressor type lands and cleared by `round_end` after the
    /// heal is skipped.
    regen_per_round: u32,
    regen_suppressors: HashSet<DamageType>,
    regen_suppressed: bool,
    /// Remaining 5e Mirror Image decoys. Each incoming attack rolls
    /// against the decoy pool first; a hit pops one decoy and misses the
    /// caster. Cleared when concentration drops or the pool hits zero
    /// (which also strips the MirroredImages condition).
    mirror_images: u32,
    /// Identity of the actor that has Charmed this actor (if any). 5e
    /// Charmed: the target cannot make attacks against the charmer. We
    /// store the id rather than just the condition flag so the
    /// validation site knows who to block. Cleared when the Charmed
    /// condition is removed.
    charmed_by: Option<usize>,
    /// 5e Fighter Indomitable — one-shot "reroll the next failed save"
    /// marker. Set by the Indomitable action; consumed at the save
    /// site (`EncounterInstance::roll_save`) on a fail. Refreshed by
    /// long rest along with the feature pool.
    indomitable_pending: bool,
    /// Identity of the paladin that has Compelled this actor to a duel.
    /// Paired with the `Dueled` condition: attacks against anyone *other*
    /// than this id are at disadvantage. Cleared when the Dueled
    /// condition lifts.
    dueled_by: Option<usize>,
    /// 5e Legendary Resistance — remaining auto-pass charges on failed
    /// saves this long rest. Refreshed to `legendary_resistance_max` on
    /// long rest. See `EncounterInstance::roll_save` for the trigger site.
    legendary_resistance_remaining: u32,
    legendary_resistance_max: u32,
    /// Identity of the caster who has bonded with this actor via Warding
    /// Bond (5e level-2 abjuration). Paired with the `WardingBonded`
    /// condition: when this actor takes damage, the same amount is
    /// mirrored onto the partner via the damage-reflect site in
    /// `DealDamage::apply`. Cleared when the WardingBonded condition is
    /// removed (timer expiry / dispel / either party drops).
    warding_partner: Option<usize>,
    /// 5e Evasion (Rogue 7, Monk 7): on DEX saves that deal half on pass,
    /// take 0 on pass and half on fail.
    has_evasion: bool,
    /// 5e Uncanny Dodge (Rogue 5): reaction to halve damage from one
    /// visible attack per round.
    has_uncanny_dodge: bool,
    has_displacement: bool,
    has_danger_sense: bool,
    has_pack_tactics: bool,
    has_magic_resistance: bool,
    /// Recharge tracking: maps action name → (min_roll, is_available).
    /// At start-of-turn the engine rolls a d6 for each exhausted ability;
    /// if the roll >= min_roll the ability becomes available again.
    recharge_abilities: Vec<(&'static str, u32, bool)>,
    /// 5e Legendary Actions per round. See `CreatureTemplate` docs.
    legendary_actions_per_round: u32,
    /// 5e Extra Attack. See `CreatureTemplate` docs.
    has_extra_attack: bool,
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
            creature_type: ct.creature_type,
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
            regen_per_round: ct.regen_per_round,
            regen_suppressors: ct.regen_suppressors.clone(),
            regen_suppressed: false,
            mirror_images: 0,
            charmed_by: None,
            indomitable_pending: false,
            dueled_by: None,
            legendary_resistance_remaining: ct.legendary_resistances,
            legendary_resistance_max: ct.legendary_resistances,
            warding_partner: None,
            has_evasion: ct.has_evasion,
            has_uncanny_dodge: ct.has_uncanny_dodge,
            has_displacement: ct.has_displacement,
            has_danger_sense: ct.has_danger_sense,
            has_pack_tactics: ct.has_pack_tactics,
            has_magic_resistance: ct.has_magic_resistance,
            recharge_abilities: ct
                .recharge_abilities
                .iter()
                .map(|&(name, min_roll)| (name, min_roll, true))
                .collect(),
            legendary_actions_per_round: ct.legendary_actions_per_round,
            has_extra_attack: ct.has_extra_attack,
        })
    }

    /// Remaining Mirror Image decoys (5e spell). Zero = no decoys; the
    /// MirroredImages condition should be off in that state.
    pub fn mirror_images(&self) -> u32 {
        self.mirror_images
    }

    /// Grant `n` Mirror Image decoys. Overwrites any prior pool (5e: re-
    /// casting the spell creates a fresh set). Caller is responsible for
    /// applying the MirroredImages condition.
    pub fn set_mirror_images(&mut self, n: u32) {
        self.mirror_images = n;
    }

    /// Pop one Mirror Image decoy. Returns true if a decoy was consumed
    /// (caller treats the attack as a miss). When the pool hits zero the
    /// MirroredImages condition is cleared so the holder loses the
    /// disadvantage-on-attacks rider.
    pub fn pop_mirror_image(&mut self) -> bool {
        if self.mirror_images == 0 {
            return false;
        }
        self.mirror_images -= 1;
        if self.mirror_images == 0 {
            self.conditions.remove(&Condition::MirroredImages);
        }
        true
    }

    /// Who has this actor Charmed (if anyone). Used to gate attack-roll
    /// validation: a Charmed actor can't attack their charmer.
    pub fn charmed_by(&self) -> Option<usize> {
        self.charmed_by
    }

    pub fn set_charmed_by(&mut self, id: Option<usize>) {
        self.charmed_by = id;
    }

    /// Identity of the paladin that has this actor locked in a Compelled
    /// Duel (if any). Read by `compute_attack_mode` to apply the
    /// "disadvantage on attacks vs anyone other than the duelist" rider.
    pub fn dueled_by(&self) -> Option<usize> {
        self.dueled_by
    }

    pub fn set_dueled_by(&mut self, id: Option<usize>) {
        self.dueled_by = id;
    }

    /// Caster id this actor is currently Warding-Bonded to (5e
    /// `WardingBonded` condition). `None` when the bond is inactive.
    /// Read by `DealDamage::apply` to mirror damage onto the partner.
    pub fn warding_partner(&self) -> Option<usize> {
        self.warding_partner
    }

    /// Set / clear the Warding Bond partner. Cleared automatically when
    /// the `WardingBonded` condition is removed via `remove_condition`.
    pub fn set_warding_partner(&mut self, id: Option<usize>) {
        self.warding_partner = id;
    }

    pub fn has_evasion(&self) -> bool {
        self.has_evasion
    }

    pub fn has_uncanny_dodge(&self) -> bool {
        self.has_uncanny_dodge
    }

    pub fn has_displacement(&self) -> bool {
        self.has_displacement
    }

    pub fn has_danger_sense(&self) -> bool {
        self.has_danger_sense
    }

    pub fn has_pack_tactics(&self) -> bool {
        self.has_pack_tactics
    }

    pub fn has_magic_resistance(&self) -> bool {
        self.has_magic_resistance
    }

    pub fn legendary_actions_per_round(&self) -> u32 {
        self.legendary_actions_per_round
    }

    pub fn has_extra_attack(&self) -> bool {
        self.has_extra_attack
    }

    /// Check if a recharge ability is currently available.
    pub fn is_recharge_available(&self, action_name: &str) -> bool {
        self.recharge_abilities
            .iter()
            .any(|(name, _, avail)| *name == action_name && *avail)
    }

    /// Mark a recharge ability as spent (unavailable until recharged).
    pub fn spend_recharge(&mut self, action_name: &str) {
        for entry in &mut self.recharge_abilities {
            if entry.0 == action_name {
                entry.2 = false;
            }
        }
    }

    /// Raw recharge entries for inspection by the encounter engine.
    pub fn recharge_entries(&self) -> &[(& 'static str, u32, bool)] {
        &self.recharge_abilities
    }

    /// Set a recharge ability's availability state.
    pub fn set_recharge_available(&mut self, action_name: &str, available: bool) {
        for entry in &mut self.recharge_abilities {
            if entry.0 == action_name {
                entry.2 = available;
            }
        }
    }

    /// HP regenerated each round-end while combat-active. 0 disables the
    /// heal; non-zero means `EncounterInstance::round_end` will heal the
    /// actor unless `regen_suppressed` is set.
    pub fn regen_per_round(&self) -> u32 {
        self.regen_per_round
    }

    pub fn regen_suppressed(&self) -> bool {
        self.regen_suppressed
    }

    pub fn clear_regen_suppression(&mut self) {
        self.regen_suppressed = false;
    }

    /// Flag the actor's regeneration as suppressed for this round if `dt`
    /// is one of the configured suppressor types. No-op for non-regen
    /// actors (whose `regen_suppressors` set is empty).
    pub fn note_regen_damage(&mut self, dt: DamageType) {
        if self.regen_suppressors.contains(&dt) {
            self.regen_suppressed = true;
        }
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
        self.indomitable_pending = false;
        self.legendary_resistance_remaining = self.legendary_resistance_max;
        for entry in &mut self.recharge_abilities {
            entry.2 = true;
        }
        self.legendary_action_slots = self.legendary_actions_per_round;
    }

    /// 5e Short Rest — 1 hour of downtime. Restores: Hit Dice-based
    /// healing (we approximate with CON-mod * level HP), fighter features
    /// (Second Wind, Action Surge), and warlock Pact Magic slots (lv1-5).
    /// Does NOT restore full HP, clear conditions, or reset concentration.
    pub fn short_rest(&mut self, roller: &mut impl Roller) {
        if !matches!(self.hp_state, HpState::Active) {
            return;
        }
        let con_mod = modifier_from_score(self.constitution);
        let dice_count = (self.level / 2).max(1);
        let roll = roller.roll(&Dice::new(dice_count, 8)) as i32;
        let heal = (roll + con_mod * dice_count as i32).max(0) as u32;
        self.heal(heal);

        for tag in SHORT_REST_FEATURES {
            if self.features_max.contains(tag) {
                self.features_remaining.insert(tag);
            }
        }
    }

    pub fn temp_hp(&self) -> u32 {
        self.temp_hp
    }

    /// 5e: a new application replaces the existing pool only if it's
    /// larger. Returns the resulting pool size — callers that want a
    /// "did it change?" boolean can diff against `temp_hp()` from before
    /// the call, or compare against `amount` (a no-op leaves the prior
    /// pool, which is `>= amount`).
    pub fn gain_temp_hp(&mut self, amount: u32) -> u32 {
        if amount > self.temp_hp {
            self.temp_hp = amount;
        }
        self.temp_hp
    }

    /// Returns the post-modifier damage value (immunity → 0, resistance
    /// → halve, vulnerability → double, none → unchanged). Doesn't touch
    /// temp HP — that's `take_typed_damage`'s job.
    ///
    /// 5e stacking rule (PHB p.197): "Multiple instances of resistance or
    /// vulnerability that affect the same damage type count as only one
    /// instance." We track whether any resistance source has applied via
    /// `resisted` and skip further halving once it's set. Immunity still
    /// trumps everything and zeros the amount immediately.
    pub fn effective_damage(&self, raw: u32, dt: DamageType) -> u32 {
        // Template-level modifier (resistance / immunity / vulnerability).
        let modifier = self.damage_modifiers.get(&dt).copied();
        // Immunity from any source zeroes damage outright.
        if matches!(modifier, Some(DamageModifier::Immunity)) {
            return 0;
        }
        // 5e Mind Blank: psychic-damage immunity for the duration.
        if dt == DamageType::Psychic && self.has_condition(Condition::MindBlanked) {
            return 0;
        }
        // Start with raw and apply vulnerability / template resistance.
        let mut amt = match modifier {
            Some(DamageModifier::Vulnerability) => raw.saturating_mul(2),
            Some(DamageModifier::Resistance) => raw / 2,
            _ => raw,
        };
        // Collect condition-based resistance sources. Per 5e stacking
        // rules, only one halving applies regardless of how many sources
        // grant resistance. If the template already provided Resistance
        // above, we skip condition-based halving too.
        let template_resisted = matches!(modifier, Some(DamageModifier::Resistance));
        let condition_resistance = !template_resisted
            && (self.has_condition(Condition::DamageResistant)
                || self.has_condition(Condition::Globed)
                || self.has_condition(Condition::WardingBonded)
                || self.has_condition(Condition::Petrified)
                || (self.has_condition(Condition::Raging)
                    && matches!(
                        dt,
                        DamageType::Bludgeoning | DamageType::Piercing | DamageType::Slashing
                    ))
                || (dt == DamageType::Fire && self.has_condition(Condition::InvestedInFlame)));
        if condition_resistance {
            amt /= 2;
        }
        amt
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
    /// equivalent to a player level) read from the same scale via
    /// `proficiency_bonus_for_level` so the curve lives in one place.
    pub fn proficiency_bonus(&self) -> i32 {
        let effective_level = self.level.max(self.cr.floor().max(1.0) as u32);
        crate::engine::util::proficiency_bonus_for_level(effective_level)
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
    /// Heroism also confers immunity to Frightened — checked here so the
    /// gate is symmetric with the template-driven immunity list.
    ///
    /// 5e: re-applying a condition with a *longer* timer extends the
    /// effect; a shorter timer is ignored. Permanent beats any rounds
    /// timer; `UntilStartOfNextTurn` is treated as the shortest possible
    /// duration. Returns true if the condition was newly added.
    pub fn add_condition(&mut self, c: Condition, timer: ConditionTimer) -> bool {
        if self.condition_immunities.contains(&c) {
            return false;
        }
        // 5e Heroism: target is immune to the Frightened condition while
        // the spell is up. We honor that as a dynamic immunity here so
        // any source (monster fear aura, Cause Fear spell) gets blocked.
        if c == Condition::Frightened && self.has_condition(Condition::Heroic) {
            return false;
        }
        // 5e Mind Blank: target is immune to charm while the spell is up
        // (RAW also blocks divination and psychic damage; psychic damage
        // is handled in `effective_damage`). Dynamic immunity so any
        // source — Charm Person, Charm Monster, Dominate Person, Suggestion
        // — gets blocked, not just the spells we know about today.
        if c == Condition::Charmed && self.has_condition(Condition::MindBlanked) {
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
        let removed = self.conditions.remove(&c).is_some();
        if removed {
            // Keep tightly-linked auxiliary state in sync with the
            // primary condition flag.
            match c {
                Condition::Charmed => self.charmed_by = None,
                Condition::MirroredImages => self.mirror_images = 0,
                Condition::Dueled => self.dueled_by = None,
                Condition::WardingBonded => self.warding_partner = None,
                _ => {}
            }
        }
        removed
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
                    // Route through remove_condition so auxiliary state
                    // (charmed_by, mirror_images) clears too.
                    self.remove_condition(c);
                    expired.push(c);
                }
                ConditionTimer::Rounds(n) => {
                    self.conditions.insert(c, ConditionTimer::Rounds(n - 1));
                }
            }
        }
        expired
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
            self.remove_condition(c);
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
            // 5e Shocking Grasp & similar lockout effects: any condition
            // whose `blocks_reactions` clause is true (NoReaction,
            // Confused) silences the reaction lane. Stacks with the
            // Incapacitated family which already zeroes them.
            Resource::Reaction => {
                !action_blocked
                    && !self.conditions.keys().any(|c| c.blocks_reactions())
                    && self.reaction_slots >= 1
            }
            Resource::LegendaryAction => !action_blocked && self.legendary_action_slots >= 1,
        }
    }

    pub fn has_reaction(&self) -> bool {
        self.reaction_slots >= 1 && !self.conditions.keys().any(|c| c.blocks_reactions())
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
        // AC-floor conditions (Mage Armor → 13 + DEX, Barkskin → 16) act
        // as a minimum AC: the caster gets the better of their raw base
        // and the floor. They don't stack with worn armor RAW, but a
        // floor lets the caster benefit when their base AC is lower.
        // The condition AC bonus (Shield, Shield of Faith, Hasted, etc.)
        // stacks on top of whichever number wins.
        let raw_base = self.base_ac as i32 + self.total_item_bonuses().ac;
        let floor = self.ac_floor();
        (raw_base.max(floor) + self.condition_ac_bonus()).max(0) as u32
    }

    /// Flat AC contribution from active conditions. Shield of Faith
    /// (+2 from the spell), Shielded (+5 from the Shield reaction spell
    /// — RAW value), Mage Armored (sets minimum AC to 13 + DEX, which
    /// we approximate as a flat top-up — see `armor_class`).
    pub fn condition_ac_bonus(&self) -> i32 {
        let mut bonus = 0;
        if self.has_condition(Condition::ShieldOfFaith) {
            bonus += 2;
        }
        if self.has_condition(Condition::Shielded) {
            bonus += 5;
        }
        if self.has_condition(Condition::Hasted) {
            bonus += 2;
        }
        if self.has_condition(Condition::Slowed) {
            bonus -= 2;
        }
        // 5e Warding Bond: +1 AC while bonded.
        if self.has_condition(Condition::WardingBonded) {
            bonus += 1;
        }
        bonus
    }

    /// Effective AC floor from active AC-setting conditions. Mage Armor
    /// floors at `13 + DEX`; Barkskin floors at 16. The maximum across
    /// every active floor wins so the holder takes the highest qualifying
    /// minimum — RAW: "Barkskin / Mage Armor don't stack with each other
    /// or with worn armor; pick the best." Returns 0 when no floor is
    /// active so `armor_class` falls back to base AC unmodified.
    pub fn ac_floor(&self) -> i32 {
        let mut floor = 0;
        if self.has_condition(Condition::MageArmored) {
            floor = floor.max(13 + modifier_from_score(self.dexterity));
        }
        if self.has_condition(Condition::Barkskinned) {
            floor = floor.max(16);
        }
        floor
    }

    pub fn hitpoints(&self) -> u32 {
        self.hitpoints
    }

    pub fn max_hitpoints(&self) -> u32 {
        let bonus = self.total_item_bonuses().max_hp;
        (self.base_hitpoints as i32 + bonus).max(1) as u32
    }

    /// Permanently bump the actor's max HP by `delta`. Current HP rises
    /// by the same amount so the boost is immediately useful (matches
    /// 5e's Aid spell semantics: "their hit point maximum and current
    /// hit points increase by 5"). Use a negative delta to apply a
    /// max-HP penalty (e.g. exhaustion); the floor is 1 max HP.
    pub fn bump_max_hp(&mut self, delta: i32) {
        let new_base = (self.base_hitpoints as i32 + delta).max(1) as u32;
        let added = new_base.saturating_sub(self.base_hitpoints);
        self.base_hitpoints = new_base;
        if added > 0 {
            let cap = self.max_hitpoints();
            self.hitpoints = self.hitpoints.saturating_add(added).min(cap);
        } else {
            // On a downward bump, never exceed the new cap.
            self.hitpoints = self.hitpoints.min(self.max_hitpoints());
        }
    }

    pub fn speed(&self) -> f32 {
        let bonus = self.total_item_bonuses().speed as f32;
        // 5e Fly spell: the target gains a flying speed equal to its
        // walking speed (we approximate with +60ft so the buff is a
        // meaningful kiting boost rather than a no-op for fast actors).
        // Applied additively before the Haste / Slow factor so haste-fly
        // doubles the larger number — matching the "Haste doubles your
        // speed" RAW phrasing.
        let fly = if self.has_condition(Condition::Flying) { 60.0 } else { 0.0 };
        let raw = (self.base_speed + bonus + fly).max(0.0);
        // 5e Haste doubles speed; Slow halves it. If both happen to be
        // active (e.g. cross-cast), they cancel back to base — applying
        // the factor multiplicatively keeps the math symmetric.
        let mut factor = 1.0_f32;
        if self.has_condition(Condition::Hasted) {
            factor *= 2.0;
        }
        if self.has_condition(Condition::Slowed) {
            factor *= 0.5;
        }
        raw * factor
    }

    pub fn item_save_bonus(&self) -> i32 {
        self.total_item_bonuses().save
    }

    /// Flat to-hit bonus contributed only by *conditions* whose dice
    /// aren't already represented elsewhere. Bless / Bane install a
    /// separate `attack_bonus_buff` delta on top of the d4 die roll
    /// (`bless_bane_attack_die`), so they're intentionally excluded
    /// from this lane — including them here would double-count. This
    /// lane is reserved for condition-only flat bonuses (Sacred
    /// Weapon: +CHA, Bardic Inspiration: +3 d6-average).
    pub fn condition_attack_bonus(&self) -> i32 {
        let mut bonus = 0;
        // 5e Channel Divinity: Sacred Weapon — paladin's weapon glows
        // with divine light, adding their CHA modifier to attack rolls.
        // Sourced from the holder's own CHA so monsters who somehow grab
        // the buff still scale off their own stat block (no edge case
        // today, but the symmetry beats hard-coding a +3).
        if self.has_condition(Condition::Sacred) {
            bonus += modifier_from_score(self.charisma);
        }
        // 5e Bardic Inspiration: holder adds a d6 (RAW scales d6→d8→d10→d12
        // by bard level) to the next attack roll. We collapse to the
        // d6-average (+3); the condition is consumed by the next attack
        // via `clear_attack_advantage_riders` so the bonus doesn't
        // double-fire across multiple swings.
        if self.has_condition(Condition::Inspired) {
            bonus += 3;
        }
        bonus
    }

    /// Symmetric save-roll counterpart to `condition_attack_bonus`. Same
    /// rationale for excluding Bless / Bane: their +2 / -2 lives on
    /// `save_bonus_buff` and their d4 die on `bless_bane_attack_die`,
    /// so this lane is condition-only flat bonuses (Bardic
    /// Inspiration: +3 d6-average).
    pub fn condition_save_bonus(&self) -> i32 {
        let mut bonus = 0;
        if self.has_condition(Condition::Inspired) {
            bonus += 3;
        }
        // 5e Warding Bond: +1 saving throws while bonded.
        if self.has_condition(Condition::WardingBonded) {
            bonus += 1;
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
        if self.conditions.keys().any(|c| c.zeros_movement()) {
            return 0.0;
        }
        // 5e RAW: a prone creature crawls at half speed. Every tile of
        // movement costs double while prone, which we approximate by
        // halving the remaining budget so the actor gets half as far.
        if self.has_condition(Condition::Prone) {
            return self.movement * 0.5;
        }
        self.movement
    }

    pub fn size(&self) -> Size {
        self.size
    }

    pub fn creature_type(&self) -> CreatureType {
        self.creature_type
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
        self.legendary_action_slots = self.legendary_actions_per_round;
        // 5e Tasha's Mind Whip: on the holder's next turn, they lose one
        // of action / bonus action / reaction. We zero the action slot
        // (most-impactful pick) and burn the condition the moment it
        // gates the next turn. The NoReaction rider was applied
        // separately on the cast for the reaction-loss half; the
        // start-of-turn cleanup is the action-loss half.
        if self.conditions.remove(&Condition::MindWhipped).is_some() {
            self.action_slots = 0;
        }
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
            // A Sphered creature is fully encased and can't reactively
            // dodge incoming attacks — the sphere holds them in place.
            && !self.has_condition(Condition::Sphered)
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

    pub fn set_disengaging(&mut self, on: bool) {
        if on {
            self.add_condition(Condition::Disengaging, ConditionTimer::UntilStartOfNextTurn);
        } else {
            self.remove_condition(Condition::Disengaging);
        }
    }

    /// Identity of the helper who granted advantage to this actor, if
    /// any. Returns the first helper id we find — `set_help_grant`
    /// keeps the map at most one entry, so this is unambiguous.
    pub fn helped_by(&self) -> Option<usize> {
        self.help_grants.keys().next().copied()
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

    /// Convenience for `modifier_from_score(self.ability_score(ability))` —
    /// the most-repeated read of an actor's ability modifier. Replaces ~40
    /// sites of the explicit `modifier_from_score(caster.ability_score(...))`
    /// dance across the spells / class-features layer with a one-liner.
    pub fn ability_modifier(&self, ability: AbilityScoreType) -> i32 {
        modifier_from_score(self.ability_score(ability))
    }

    pub fn spell_save_dc(&self, ability: AbilityScoreType) -> i32 {
        8 + self.proficiency_bonus() + self.ability_modifier(ability)
    }

    pub fn spell_attack_modifier(&self, ability: AbilityScoreType) -> i32 {
        self.proficiency_bonus() + self.ability_modifier(ability)
    }

    /// For spells available to multiple classes (Bard / Sorcerer / Wizard /
    /// Warlock — Eyebite, Otto's, Fire Storm), pick the ability whose raw
    /// score is highest from a candidate set and return the resulting
    /// `spell_save_dc`. Ties break by the order in `candidates`. Falls back
    /// to the first ability if every candidate score is identical.
    /// Centralized so spell impls don't each open-code the
    /// "max(INT, CHA, WIS)" pattern.
    pub fn best_spell_save_dc<I>(&self, candidates: I) -> i32
    where
        I: IntoIterator<Item = AbilityScoreType>,
    {
        self.spell_save_dc(self.best_spellcasting_ability(candidates))
    }

    /// Spell-attack-modifier analogue of `best_spell_save_dc`. Picks the
    /// ability whose raw score is highest from `candidates` and returns
    /// `spell_attack_modifier` for that ability. Ties break by the order
    /// in `candidates`. Used by spells that resolve as a ranged spell
    /// attack but are available to multiple casting classes (Chromatic
    /// Orb on INT/CHA, future Witch-Bolt-style pickups, etc.) so the
    /// caller doesn't have to open-code the max-of-scores pattern.
    pub fn best_spell_attack_modifier<I>(&self, candidates: I) -> i32
    where
        I: IntoIterator<Item = AbilityScoreType>,
    {
        self.spell_attack_modifier(self.best_spellcasting_ability(candidates))
    }

    /// Pick the highest-scoring spellcasting ability from `candidates`. Ties
    /// break by the order in `candidates`. Falls back to Intelligence on an
    /// empty iterator (no caller currently passes empty — the fallback is a
    /// belt-and-suspenders so the helper is total). Shared body for the
    /// best-DC / best-attack-modifier pair so the picker logic lives at one
    /// chokepoint.
    pub fn best_spellcasting_ability<I>(&self, candidates: I) -> AbilityScoreType
    where
        I: IntoIterator<Item = AbilityScoreType>,
    {
        candidates
            .into_iter()
            .max_by_key(|a| self.ability_score(*a))
            .unwrap_or(AbilityScoreType::Intelligence)
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
                let hp_before = self.hitpoints;
                self.hitpoints = self.hitpoints.saturating_sub(after_temp);
                if self.hitpoints == 0 {
                    // 5e Massive Damage (PHB p.197): if remaining damage
                    // after hitting 0 HP equals or exceeds the creature's
                    // max HP, it dies instantly — no death saves.
                    let overflow = after_temp.saturating_sub(hp_before);
                    if overflow >= self.max_hitpoints() {
                        self.hp_state = HpState::Dead;
                        return DamageOutcome::Killed;
                    }
                    // 5e Death Ward: when the holder would drop to 0 HP,
                    // they instead drop to 1 HP and the ward burns off.
                    if self.conditions.contains_key(&Condition::DeathWarded) {
                        self.hitpoints = 1;
                        self.conditions.remove(&Condition::DeathWarded);
                        return DamageOutcome::Reduced;
                    }
                    if self.rolls_death_saves {
                        self.hp_state = HpState::Dying {
                            successes: 0,
                            failures: 0,
                        };
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

    /// Promote a Dying actor to Stable without restoring any HP (5e
    /// Spare the Dying / Medicine check stabilize semantics: they stop
    /// rolling death saves but stay at 0 HP and Unconscious). No-op for
    /// non-Dying actors. Returns true if the actor's state changed.
    pub fn stabilize(&mut self) -> bool {
        if matches!(self.hp_state, HpState::Dying { .. }) {
            self.hp_state = HpState::Stable;
            true
        } else {
            false
        }
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

    /// True if this actor has an Indomitable reroll pending — set by
    /// the Indomitable action, consumed at the next failed save.
    pub fn indomitable_pending(&self) -> bool {
        self.indomitable_pending
    }

    pub fn mark_indomitable_pending(&mut self) {
        self.indomitable_pending = true;
    }

    pub fn consume_indomitable(&mut self) -> bool {
        let pending = self.indomitable_pending;
        self.indomitable_pending = false;
        pending
    }

    /// 5e Legendary Resistance — remaining auto-pass charges this long rest.
    /// Read by `EncounterInstance::roll_save` to promote a failed save when
    /// the counter is non-zero. Zero for ordinary creatures.
    pub fn legendary_resistance_remaining(&self) -> u32 {
        self.legendary_resistance_remaining
    }

    /// Per-rest cap on Legendary Resistance charges (the template max).
    /// Surfaced for UI / AI heuristics that need to know if a creature
    /// has the trait at all without caring about the current pool.
    pub fn legendary_resistance_max(&self) -> u32 {
        self.legendary_resistance_max
    }

    /// Spend one Legendary Resistance charge. Returns true if a charge
    /// was actually consumed (counter was > 0), false otherwise. Caller
    /// is expected to check `legendary_resistance_remaining > 0` first
    /// and decide whether burning a charge is worth it.
    pub fn consume_legendary_resistance(&mut self) -> bool {
        if self.legendary_resistance_remaining == 0 {
            return false;
        }
        self.legendary_resistance_remaining -= 1;
        true
    }

    /// Convenience: check Bless condition without callers having to
    /// import the Condition enum just for this single test.
    pub fn is_blessed(&self) -> bool {
        self.has_condition(Condition::Blessed)
    }

    pub fn is_baned(&self) -> bool {
        self.has_condition(Condition::Baned)
    }

    pub fn is_heroic(&self) -> bool {
        self.has_condition(Condition::Heroic)
    }

    /// True iff the actor is currently `Petrified` — turned to stone.
    /// Convenience accessor used by the AI / UI to surface the state
    /// without each call site re-importing `Condition`.
    pub fn is_petrified(&self) -> bool {
        self.has_condition(Condition::Petrified)
    }

    /// True iff the actor holds a Death Ward — the next killing blow
    /// will be absorbed by `take_damage`. Surfaced for AI heuristics
    /// (skip dispelling targets without the buff) and UI tagging.
    pub fn has_death_ward(&self) -> bool {
        self.has_condition(Condition::DeathWarded)
    }

    /// Ability-mod + proficiency bonus for `ability` (the standard 5e
    /// "ability attack bonus" used by spells). `attack_bonus()` is
    /// STR-locked for melee weapons; this lets callers pick the right
    /// stat for spell attacks (INT for wizard, WIS for cleric, etc.).
    pub fn ability_attack_bonus(&self, ability: AbilityScoreType) -> i32 {
        self.ability_modifier(ability) + self.proficiency_bonus()
    }

    /// Record that `helper_id` Helped this actor against `target_id`.
    /// The helped actor's next attack against `target_id` benefits from
    /// advantage; the grant is consumed (cleared) by `consume_help_for`.
    pub fn help_grant(&self, target_id: usize) -> bool {
        self.help_grants.values().any(|t| *t == target_id)
    }

    /// Set or clear a Help grant on this actor.
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
        assert_eq!(s.gain_temp_hp(5), 5);
        // Smaller grant is ignored: pool stays at 5.
        assert_eq!(s.gain_temp_hp(3), 5);
        // Larger grant replaces.
        assert_eq!(s.gain_temp_hp(8), 8);
    }

    #[test]
    fn mind_blank_zeroes_psychic_damage() {
        let mut s = make(&SKELETON_TEMPLATE);
        // Sanity check: no buff = baseline psychic damage lands.
        assert_eq!(s.effective_damage(15, DamageType::Psychic), 15);
        s.add_condition(Condition::MindBlanked, ConditionTimer::Rounds(100));
        // With the buff up, psychic drops to zero — mirrors the Immunity
        // damage modifier semantics.
        assert_eq!(s.effective_damage(15, DamageType::Psychic), 0);
        // Other damage types still flow through normally.
        assert_eq!(s.effective_damage(7, DamageType::Bludgeoning), 14);
    }

    #[test]
    fn mind_blank_blocks_charm() {
        let mut s = make(&SKELETON_TEMPLATE);
        s.add_condition(Condition::MindBlanked, ConditionTimer::Rounds(100));
        // Charm application is blocked by the dynamic immunity hook even
        // though Charmed isn't on the template's `condition_immunities`.
        let added = s.add_condition(Condition::Charmed, ConditionTimer::Rounds(10));
        assert!(!added, "charm should fizzle against mind blank");
        assert!(!s.has_condition(Condition::Charmed));
    }

    #[test]
    fn mind_blank_drop_restores_psychic_lane() {
        let mut s = make(&SKELETON_TEMPLATE);
        s.add_condition(Condition::MindBlanked, ConditionTimer::Rounds(100));
        assert_eq!(s.effective_damage(20, DamageType::Psychic), 0);
        s.remove_condition(Condition::MindBlanked);
        // After dispel / expire, psychic damage flows through normally.
        assert_eq!(s.effective_damage(20, DamageType::Psychic), 20);
    }

    #[test]
    fn petrified_grants_damage_resistance() {
        let mut s = make(&SKELETON_TEMPLATE);
        assert_eq!(s.effective_damage(20, DamageType::Fire), 20);
        s.add_condition(Condition::Petrified, ConditionTimer::Permanent);
        assert_eq!(
            s.effective_damage(20, DamageType::Fire),
            10,
            "petrified creature should take half fire damage"
        );
        assert_eq!(
            s.effective_damage(20, DamageType::Slashing),
            10,
            "petrified creature should take half slashing damage"
        );
    }

    #[test]
    fn resistance_does_not_stack_per_5e_rules() {
        let mut s = make(&SKELETON_TEMPLATE);
        // Skeleton is vulnerable to bludgeoning (doubles), so test with
        // a creature that has no template-level modifier for fire.
        assert_eq!(s.effective_damage(20, DamageType::Fire), 20);
        // Add DamageResistant (Stoneskin).
        s.add_condition(Condition::DamageResistant, ConditionTimer::Rounds(10));
        assert_eq!(s.effective_damage(20, DamageType::Fire), 10);
        // Add WardingBonded on top — 5e says resistance doesn't stack.
        s.add_condition(Condition::WardingBonded, ConditionTimer::Rounds(10));
        assert_eq!(
            s.effective_damage(20, DamageType::Fire),
            10,
            "two resistance sources should halve only once (5e stacking rule)"
        );
    }
}
