use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::STIRGE_PROBOSCIS;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{Size, SpecialSense};
use std::collections::{HashMap, HashSet};
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
        speed: 10., // 5e: 10ft walking, 40ft fly — we model walking only.
        strength: 4,
        intelligence: 2,
        dexterity: 16,
        wisdom: 8,
        constitution: 11,
        charisma: 6,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::new(),
        cr: 0.125,
        size: Size::Tiny,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: false,
        damage_modifiers: HashMap::new(),
        proficient_saves: HashSet::new(),
        condition_immunities: HashSet::new(),
        features: HashSet::new(),
        regen_per_round: 0,
        regen_suppressors: HashSet::new(),
        legendary_resistances: 0,
    }
});
