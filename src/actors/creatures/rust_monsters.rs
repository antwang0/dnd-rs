use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::BITE;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

pub static RUST_MONSTER_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&BITE);
    CreatureTemplate {
        name: "Rust Monster",
        glyph: 'r',
        ac: 14,
        hitpoints: "6d8+6".parse().unwrap(),
        speed: 40.,
        strength: 13,
        dexterity: 12,
        constitution: 13,
        intelligence: 2,
        wisdom: 13,
        charisma: 6,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        cr: 0.5,
        size: Size::Medium,
        creature_type: CreatureType::Monstrosity,
        actions,
        ..CreatureTemplate::defaults()
    }
});
