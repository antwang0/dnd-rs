use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::spells::{FIRE_BOLT, MAGIC_MISSILE};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{Language, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Wizard — INT-primary spellcaster. Cantrip Fire Bolt as the at-will
/// damage option (no slot), Magic Missile as the level-1 reliable nuke
/// (auto-hit, no save). Squishy: light HP, low AC, no melee weapon —
/// wants to stay at range. Roughly comparable to a level-3 wizard:
/// 18 HP (3d6+3), AC 12, INT 16, two level-1 slots.
pub static WIZARD_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*FIRE_BOLT);
    actions.push(&*MAGIC_MISSILE);
    CreatureTemplate {
        name: "Wizard",
        glyph: 'M', // 'M' for Mage — 'W' is wolf, 'Z' zombie etc.
        n_instances: 0,
        ac: 12,
        hitpoints: "3d6+3".parse().unwrap(),
        speed: 30.,
        strength: 8,
        intelligence: 16, // primary spellcasting ability
        dexterity: 14,
        wisdom: 12,
        constitution: 12,
        charisma: 10,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Common, Language::Draconic]),
        cr: 0.5,
        size: Size::Medium,
        actions,
        // Two level-1 slots for Magic Missile.
        spell_slots_by_level: vec![2],
        rolls_death_saves: false,
        damage_resistances: HashSet::new(),
        damage_immunities: HashSet::new(),
        damage_vulnerabilities: HashSet::new(),
    }
});
