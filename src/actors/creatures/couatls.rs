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
        // RAW speed line: Speed 30 ft., fly 90 ft.
        speed: 30.0,
        fly_speed: 90.0,
        strength: 16,
        dexterity: 20, // primary attack stat (finesse bite)
        constitution: 17,
        intelligence: 18,
        wisdom: 20,    // spell save DC anchor
        charisma: 18,
        senses: HashSet::from([SpecialSense::Truesight(120)]),
        languages: HashSet::from([
            Language::Common,
            Language::Celestial,
            Language::Draconic,
        ]),
        cr: 4.0,
        size: Size::Medium,
        creature_type: CreatureType::Celestial,
        actions,
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
        has_magic_resistance: true,
        // 5e **Magic Weapons**: "the couatl's weapon attacks are magical."
        features: HashSet::from([crate::actions::class_features::MAGICAL_ATTACKS_TAG]),
        ..CreatureTemplate::defaults()
    }
});
