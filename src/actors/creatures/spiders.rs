use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{SPIDER_BITE, WEB};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{DamageType, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Giant Spider. Headline mechanic: Web — a ranged save-or-be-restrained
/// debuff that strips a target's mobility for 5 rounds. Pairs with the
/// Spider's bite (1d6 piercing + 2d6 poison-on-fail rider) to demonstrate
/// the Restrained condition driving combat math (target gets advantage
/// against; restrained creature gets disadvantage on its own attacks).
pub static SPIDER_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*SPIDER_BITE);
    actions.push(&*WEB);
    CreatureTemplate {
        name: "Spider",
        glyph: 'P',
        n_instances: 0,
        ac: 14,
        hitpoints: "4d10".parse().unwrap(),
        speed: 30.,
        strength: 14,
        intelligence: 2,
        dexterity: 16,
        wisdom: 11,
        constitution: 12,
        charisma: 4,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([SpecialSense::Blindsight(10), SpecialSense::Darkvision(60)]),
        languages: HashSet::new(),
        cr: 1.0,
        size: Size::Large,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: false,
        damage_resistances: HashSet::new(),
        damage_immunities: HashSet::from([DamageType::Poison]),
        damage_vulnerabilities: HashSet::new(),
    }
});
