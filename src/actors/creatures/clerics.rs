use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::spells::{BLESS, CURE_WOUNDS, HEALING_WORD, HOLD_PERSON, SACRED_BURST, SACRED_FLAME};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{Language, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Acolyte-style spellcaster. WIS-primary; Sacred Flame as the staple
/// damage option, Healing Word for support. Modeled to be roughly
/// equivalent to MM Acolyte (CR 1/4) — light HP, medium AC, no melee.
pub static CLERIC_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*SACRED_FLAME);
    actions.push(&*SACRED_BURST);
    actions.push(&*HEALING_WORD);
    actions.push(&*HOLD_PERSON);
    actions.push(&*BLESS);
    actions.push(&*CURE_WOUNDS);
    CreatureTemplate {
        name: "Cleric",
        glyph: 'C',
        n_instances: 0,
        ac: 13,
        hitpoints: "2d8+2".parse().unwrap(),
        speed: 30.,
        strength: 10,
        intelligence: 10,
        dexterity: 10,
        wisdom: 14, // primary spellcasting ability
        constitution: 12,
        charisma: 10,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Common]),
        cr: 0.25,
        size: Size::Medium,
        actions,
        // 3 level-1 slots (Healing Word) + 2 level-2 slots (Hold Person).
        spell_slots_by_level: vec![3, 2],
        rolls_death_saves: false,
        damage_modifiers: HashMap::new(),
    }
});
