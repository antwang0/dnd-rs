use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::SLAM;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{Language, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

pub static NOTHIC_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&SLAM);
    CreatureTemplate {
        name: "Nothic",
        glyph: 'N',
        ac: 15,
        hitpoints: "6d8+18".parse().unwrap(),
        speed: 30.,
        strength: 14,
        intelligence: 13,
        dexterity: 16,
        wisdom: 10,
        constitution: 16,
        charisma: 8,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([
            SpecialSense::Darkvision(120),
            SpecialSense::Truesight(120),
        ]),
        languages: HashSet::from([Language::Undercommon]),
        cr: 2.0,
        size: Size::Medium,
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
    }
});
