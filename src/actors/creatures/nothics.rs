use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::SLAM;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Language, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

pub static NOTHIC_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&SLAM);
    CreatureTemplate {
        name: "Nothic",
        glyph: 'N',
        ac: 15,
        hitpoints: "6d8+18".parse().unwrap(),
        strength: 14,
        dexterity: 16,
        constitution: 16,
        intelligence: 13,
        wisdom: 10,
        charisma: 8,
        senses: HashSet::from([
            SpecialSense::Darkvision(120),
            SpecialSense::Truesight(120),
        ]),
        languages: HashSet::from([Language::Undercommon]),
        cr: 2.0,
        size: Size::Medium,
        creature_type: CreatureType::Aberration,
        actions,
        ..CreatureTemplate::defaults()
    }
});
