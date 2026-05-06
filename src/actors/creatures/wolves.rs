use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::WOLF_BITE;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{Size, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Fast melee with a built-in trip rider. Bite always rolls the STR save
/// on hit, so a Wolf naturally knocks targets prone — making subsequent
/// melee attacks (its own next-turn bite, or an ally's swing) hit at
/// advantage. Demonstrates rider-on-hit baked into a creature's
/// canonical action.
pub static WOLF_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*WOLF_BITE);
    CreatureTemplate {
        name: "Wolf",
        glyph: 'W',
        n_instances: 0,
        ac: 13,
        hitpoints: "2d8+2".parse().unwrap(),
        speed: 40.,
        strength: 12,
        intelligence: 3,
        dexterity: 15,
        wisdom: 12,
        constitution: 12,
        charisma: 6,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::new(),
        cr: 0.25,
        size: Size::Medium,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: false,
        damage_reactions: HashMap::new(),
    }
});
