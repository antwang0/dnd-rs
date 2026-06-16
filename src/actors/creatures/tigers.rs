use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{TIGER_BITE, TIGER_CLAWS, TIGER_MULTI};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, SpecialSense, Skill};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Tiger — CR 1 large beast. Pounce predator: bigger bite (1d10+5)
/// than the bear but slightly lighter claws (1d8+5). RAW carries a
/// Pounce rider (knock prone on a +20 ft straight charge + bonus bite)
/// — not modeled here for the same reason Boar's Charge isn't (the
/// engine doesn't track "this turn's move was straight"). The
/// standalone bite / claws are still exposed for fallback swings.
/// Stealth-proficient cat — a common druid Conjure Animals pick when
/// the player wants a single-target striker rather than a swarm.
pub static TIGER_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&TIGER_BITE);
    actions.push(&TIGER_CLAWS);
    actions.push(&*TIGER_MULTI);
    CreatureTemplate {
        name: "Tiger",
        // 'T' for Tiger — capital because Large; collides with Troll
        // on first letter, but Troll is also 'T' in this engine.
        glyph: 'T',
        ac: 12,
        hitpoints: "5d10+10".parse().unwrap(),
        speed: 40.,
        strength: 17,
        intelligence: 3,
        dexterity: 15,
        wisdom: 12,
        constitution: 14,
        charisma: 8,
        skills: HashSet::from([Skill::Perception, Skill::Stealth]),
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        cr: 1.0,
        size: Size::Large,
        creature_type: CreatureType::Beast,
        actions,
        ..CreatureTemplate::defaults()
    }
});
