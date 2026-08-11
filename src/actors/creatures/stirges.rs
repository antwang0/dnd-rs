use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::STIRGE_PROBOSCIS;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Stirge — CR 1/8 swarm-encounter staple. Tiny blood-drinking flier
/// with a proboscis attack. We don't model the "attached → drain each
/// turn" rider in 5e MM; the stirge just makes a standard 1d4+1
/// piercing strike each Action. Low HP (5) and AC (14) means they're
/// individually fragile; the design intent is to throw them in clouds
/// of 4-6 around the party so AoE damage feels relevant again.
pub static STIRGE_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&STIRGE_PROBOSCIS);
    CreatureTemplate {
        name: "Stirge",
        // 's' lower (small swarm creature).
        glyph: 's',
        ac: 14,
        hitpoints: "2d4".parse().unwrap(),
        // RAW speed line: Speed 10 ft., fly 40 ft.
        speed: 10.0,
        fly_speed: 40.0,
        strength: 4,
        intelligence: 2,
        dexterity: 16,
        wisdom: 8,
        constitution: 11,
        charisma: 6,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        cr: 0.125,
        size: Size::Tiny,
        creature_type: CreatureType::Beast,
        actions,
        ..CreatureTemplate::defaults()
    }
});
