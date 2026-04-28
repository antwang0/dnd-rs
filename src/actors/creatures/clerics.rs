use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::spells::{
    GUIDING_BOLT, HEALING_WORD, HOLD_PERSON, INFLICT_WOUNDS, SACRED_BURST, SACRED_FLAME,
    THUNDERWAVE,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{Language, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Acolyte-style spellcaster. WIS-primary; Sacred Flame as the staple
/// damage cantrip, Guiding Bolt / Inflict Wounds for burst, Healing Word
/// for support, Hold Person / Thunderwave for control.
pub static CLERIC_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*SACRED_FLAME);
    actions.push(&*SACRED_BURST);
    actions.push(&*HEALING_WORD);
    actions.push(&*HOLD_PERSON);
    actions.push(&*GUIDING_BOLT);
    actions.push(&*INFLICT_WOUNDS);
    actions.push(&*THUNDERWAVE);
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
        // 3 level-1 slots (Healing Word, Guiding Bolt) + 2 level-2 slots (Hold Person).
        spell_slots_by_level: vec![3, 2],
        rolls_death_saves: false,
        immunities: HashSet::new(),
        resistances: HashSet::new(),
        vulnerabilities: HashSet::new(),
    }
});
