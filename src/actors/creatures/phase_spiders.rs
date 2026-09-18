use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::PHASE_SPIDER_BITE;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, Skill, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

pub static PHASE_SPIDER_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&PHASE_SPIDER_BITE);
    CreatureTemplate {
        name: "Phase Spider",
        glyph: 'p',
        ac: 14,
        hitpoints: "7d10+7".parse().unwrap(),
        strength: 15,
        dexterity: 16,
        constitution: 12,
        intelligence: 6,
        wisdom: 10,
        charisma: 6,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        cr: 3.0,
        size: Size::Large,
        creature_type: CreatureType::Monstrosity,
        actions,
        // SRD 5.2 prints no damage line for the phase spider; the
        // Poison resistance that used to be here came from neither
        // printing.
        skills: HashSet::from([Skill::Stealth]),
        ..CreatureTemplate::defaults()
    }
});
