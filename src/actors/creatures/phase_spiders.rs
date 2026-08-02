use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::SPIDER_BITE;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, DamageModifier, DamageType, Size, Skill, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

pub static PHASE_SPIDER_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&SPIDER_BITE);
    CreatureTemplate {
        name: "Phase Spider",
        glyph: 'p',
        ac: 13,
        hitpoints: "5d10+5".parse().unwrap(),
        strength: 15,
        dexterity: 15,
        constitution: 12,
        intelligence: 6,
        wisdom: 10,
        charisma: 6,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        cr: 3.0,
        size: Size::Large,
        creature_type: CreatureType::Monstrosity,
        actions,
        damage_modifiers: HashMap::from([(DamageType::Poison, DamageModifier::Resistance)]),
        skills: HashSet::from([Skill::Stealth]),
        ..CreatureTemplate::defaults()
    }
});
