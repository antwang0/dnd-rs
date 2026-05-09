use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{TRIP, ZOMBIE_MULTISLAM};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{DamageType, Language, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

pub static ZOMBIE_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    // Multislam is the zombie's main attack (2 swings per Action). Trip is
    // an alternative single attack that on hit forces a STR save or prone —
    // less raw damage but disables movement.
    actions.push(&*ZOMBIE_MULTISLAM);
    actions.push(&*TRIP);
    CreatureTemplate {
        name: "Zombie",
        glyph: 'Z',
        n_instances: 0,
        ac: 8,
        hitpoints: "2d8+6".parse().unwrap(),
        speed: 20.,
        strength: 13,
        intelligence: 3,
        dexterity: 6,
        wisdom: 6,
        constitution: 16,
        charisma: 5,
        skills: HashSet::new(), // TODO
        items: Vec::new(),
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Common]), // plus one other
        cr: 0.25,
        size: Size::Medium,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: false,
        // 5e MM zombie: immune to poison.
        damage_resistances: HashSet::new(),
        damage_immunities: HashSet::from([DamageType::Poison]),
        damage_vulnerabilities: HashSet::new(),
    }
});
