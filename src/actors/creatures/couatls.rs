use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{COUATL_BITE, COUATL_SLEEP_GAZE};
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{
    AbilityScoreType, CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense,
};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Couatl — CR 4 good-aligned outsider. Celestial winged serpent with a
/// poisonous bite, a single-target sleep gaze, and broad damage
/// resistances befitting an extraplanar creature. Acts as a "white
/// dragon-lite" boss option: high mobility, fight-or-disable kit, and
/// the resistance envelope rewards a varied damage party (psychic and
/// radiant slip through; fire / cold / lightning eat half).
///
/// Stats target the MM couatl (CR 4): 97 HP, AC 19, DEX-primary,
/// proficient in CON / WIS / CHA saves, immune to psychic damage.
pub static COUATL_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*COUATL_BITE);
    actions.push(&*COUATL_SLEEP_GAZE);
    CreatureTemplate {
        name: "Couatl",
        // 'c' (lowercase) — distinct from 'C' (Cleric / Cockatrice).
        glyph: 'c',
        ac: 19,
        // 13d10+26 ≈ 97 average per the MM Couatl stat block.
        hitpoints: "13d10+26".parse().unwrap(),
        speed: 30.,
        strength: 16,
        intelligence: 18,
        dexterity: 20, // primary attack stat (finesse bite)
        wisdom: 20,    // spell save DC anchor
        constitution: 17,
        charisma: 18,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([
            SpecialSense::Truesight(120),
        ]),
        languages: HashSet::from([
            Language::Common,
            Language::Celestial,
            Language::Draconic,
        ]),
        cr: 4.0,
        size: Size::Medium,
        creature_type: CreatureType::Celestial,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: false,
        // MM Couatl: resistant to radiant; magical-damage immunity (we
        // omit the magical-physical resistance line since we don't track
        // magical vs mundane weapon damage). Psychic immunity matches RAW.
        damage_modifiers: HashMap::from([
            (DamageType::Radiant, DamageModifier::Resistance),
            (DamageType::Psychic, DamageModifier::Immunity),
        ]),
        // MM Couatl proficient saves: CON / WIS / CHA.
        proficient_saves: HashSet::from([
            AbilityScoreType::Constitution,
            AbilityScoreType::Wisdom,
            AbilityScoreType::Charisma,
        ]),
        // Couatls can't be magically charmed or frightened.
        condition_immunities: HashSet::from([Condition::Charmed, Condition::Frightened]),
        features: HashSet::new(),
        regen_per_round: 0,
        regen_suppressors: HashSet::new(),
        legendary_resistances: 0,
        has_evasion: false,
        has_uncanny_dodge: false,
        has_displacement: false,
        has_danger_sense: false,
        has_pack_tactics: false,
        has_magic_resistance: true,
        recharge_abilities: Vec::new(),
        legendary_actions_per_round: 0,
        has_extra_attack: false,
        brutal_critical_dice: 0,
        crit_threshold: 20,
        has_lucky: false,
        has_aura_of_protection: false,
        has_aura_of_courage: false,
    }
});
