use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::SPIDER_BITE;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, DamageModifier, DamageType, Size, Skill, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Giant Spider — small fast melee biter that injects poison on a failed
/// CON save. Showcases the Poisoned condition source paired with raw
/// poison damage. Resistant to bludgeoning (gooey body), immune to its
/// own venom.
pub static SPIDER_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&SPIDER_BITE);
    CreatureTemplate {
        name: "Spider",
        glyph: 'X',
        ac: 14,
        hitpoints: "3d8+3".parse().unwrap(),
        strength: 14,
        dexterity: 16,
        constitution: 12,
        intelligence: 2,
        wisdom: 11,
        charisma: 4,
        senses: HashSet::from([
            SpecialSense::Blindsight(10),
            SpecialSense::Darkvision(60),
        ]),
        cr: 1.0,
        size: Size::Medium,
        creature_type: CreatureType::Beast,
        actions,
        damage_modifiers: HashMap::from([(DamageType::Poison, DamageModifier::Immunity)]),
        skills: HashSet::from([Skill::Stealth]),
        ..CreatureTemplate::defaults()
    }
});
