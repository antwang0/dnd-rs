use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::BITE;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

pub static RUST_MONSTER_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*BITE);
    CreatureTemplate {
        name: "Rust Monster",
        glyph: 'r',
        ac: 14,
        hitpoints: "5d8+5".parse().unwrap(),
        speed: 40.,
        strength: 13,
        intelligence: 2,
        dexterity: 12,
        wisdom: 13,
        constitution: 13,
        charisma: 6,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::new(),
        cr: 0.5,
        size: Size::Medium,
        creature_type: CreatureType::Monstrosity,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: false,
        damage_modifiers: HashMap::new(),
        proficient_saves: HashSet::new(),
        condition_immunities: HashSet::new(),
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
