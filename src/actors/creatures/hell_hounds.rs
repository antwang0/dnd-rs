use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{HELL_HOUND_BITE, HELL_HOUND_FIRE_BREATH};
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Hell Hound — CR 3 fiend. Mastiff-sized infernal hunter with two
/// signature lanes: a STR-based bite that smolders with a 1d6 fire
/// rider, and a 15-ft cone of fire breath (DC 12 DEX save). Fire-
/// immune (the breath belongs to them) and Charmed-immune (their
/// minds belong to their devil masters, not seducers). Speaks Infernal
/// per RAW; we tag it for completeness even though languages don't
/// fully drive gameplay yet.
pub static HELL_HOUND_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*HELL_HOUND_BITE);
    actions.push(&*HELL_HOUND_FIRE_BREATH);
    CreatureTemplate {
        name: "Hell Hound",
        // 'h' (lowercase): Hippogriff uses uppercase 'H'; lowercase 'h'
        // is free. Visual reads as a small fiery dog on the map.
        glyph: 'h',
        ac: 15,
        // 7d8+14 = ~45 average per MM (CR 3).
        hitpoints: "7d8+14".parse().unwrap(),
        speed: 50.,
        strength: 17,
        intelligence: 6,
        dexterity: 12,
        wisdom: 13,
        constitution: 14,
        charisma: 6,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Infernal]),
        cr: 3.0,
        size: Size::Medium,
        creature_type: CreatureType::Fiend,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: false,
        damage_modifiers: HashMap::from([(DamageType::Fire, DamageModifier::Immunity)]),
        proficient_saves: HashSet::new(),
        // Fiends are immune to being charmed by mortals — keeps the
        // hound's pack discipline from being broken by Charm Person.
        condition_immunities: HashSet::from([Condition::Charmed]),
        features: HashSet::new(),
        regen_per_round: 0,
        regen_suppressors: HashSet::new(),
        legendary_resistances: 0,
        has_evasion: false,
        has_uncanny_dodge: false,
        has_displacement: false,
        has_danger_sense: false,
        has_pack_tactics: false,
        has_magic_resistance: false,
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
