use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{Language, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

pub static ZOMBIE_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    CreatureTemplate {
        name: &"Zombie",
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
        actions: DEFAULT_ACTIONS.clone(), // TODO
    }
});
