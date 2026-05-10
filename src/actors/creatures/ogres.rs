use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::GREATCLUB;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{Language, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Slow, beefy bruiser. Headline feature is **Size::Large** — first
/// creature in the codebase that takes a 4×4 footprint, exercising the
/// footprint-aware reach, pathing, and OA logic at scale. Plus the
/// Greatclub's reach-2 swing means an Ogre threatens a wider area than
/// any other melee creature today. Stats scaled to roughly CR 1 so it
/// hits hard without trivially erasing parties.
pub static OGRE_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&GREATCLUB);
    CreatureTemplate {
        name: "Ogre",
        glyph: 'O',
        ac: 11,
        hitpoints: "4d10+8".parse().unwrap(),
        speed: 40.,
        strength: 19,
        intelligence: 5,
        dexterity: 8,
        wisdom: 7,
        constitution: 16,
        charisma: 7,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Common, Language::Giant]),
        cr: 1.0,
        size: Size::Large,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: false,
        damage_modifiers: HashMap::new(),
        proficient_saves: HashSet::new(),
        condition_immunities: HashSet::new(),
        features: HashSet::new(),
    }
});
