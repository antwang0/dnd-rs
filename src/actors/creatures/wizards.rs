use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::spells::{FIRE_BOLT, MAGIC_MISSILE};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{Language, Size};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Apprentice wizard — INT-primary spellcaster. Fire Bolt as the at-will
/// damage cantrip, Magic Missile as the leveled "guaranteed damage" finisher.
/// Squishy; meant to be a backline actor that stays at range.
pub static WIZARD_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*FIRE_BOLT);
    actions.push(&*MAGIC_MISSILE);
    CreatureTemplate {
        name: "Wizard",
        glyph: 'M',
        n_instances: 0,
        ac: 11,
        hitpoints: "2d6+2".parse().unwrap(),
        speed: 30.,
        strength: 9,
        intelligence: 15, // primary
        dexterity: 12,
        wisdom: 10,
        constitution: 11,
        charisma: 10,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::new(),
        languages: HashSet::from([Language::Common]),
        cr: 0.25,
        size: Size::Medium,
        actions,
        // 3 level-1 slots for Magic Missile.
        spell_slots_by_level: vec![3],
        rolls_death_saves: false,
        damage_modifiers: HashMap::new(),
    }
});
