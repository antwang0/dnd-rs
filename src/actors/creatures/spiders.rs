use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::SPIDER_BITE;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{DamageType, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Eight-legged ambush predator. Bite forces a CON save on hit; on
/// failure the target takes a chunky poison rider and gains the
/// Poisoned condition. Doesn't crack the action-economy — single
/// Action attack — but the rider gives it teeth (literally) against
/// glass-cannon casters.
pub static GIANT_SPIDER_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*SPIDER_BITE);
    CreatureTemplate {
        name: "Giant Spider",
        glyph: 'X',
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
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::new(),
        cr: 1.0,
        size: Size::Large,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: false,
        resistances: HashSet::new(),
        // Spider venom doesn't infect spiders.
        immunities: HashSet::from([DamageType::Poison]),
        vulnerabilities: HashSet::new(),
    }
});
