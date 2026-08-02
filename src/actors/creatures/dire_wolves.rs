use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::DIRE_WOLF_BITE;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, Skill, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Dire Wolf — CR 1 large beast. Stronger sibling of the standard
/// wolf: 2d6+3 bite with a DC 13 trip rider. Large footprint means
/// it tanks more space and has the standard "reach 2 vs adjacent"
/// adjacency footprint the engine handles via `get_tiles_from_size`.
pub static DIRE_WOLF_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&DIRE_WOLF_BITE);
    CreatureTemplate {
        name: "Dire Wolf",
        // 'D' — distinct from 'W' (wolf).
        glyph: 'D',
        ac: 14,
        hitpoints: "5d10+10".parse().unwrap(),
        speed: 50.,
        strength: 17,
        dexterity: 15,
        constitution: 15,
        intelligence: 3,
        wisdom: 12,
        charisma: 7,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        cr: 1.0,
        size: Size::Large,
        creature_type: CreatureType::Beast,
        actions,
        has_pack_tactics: true,
        skills: HashSet::from([Skill::Perception, Skill::Stealth]),
        ..CreatureTemplate::defaults()
    }
});
