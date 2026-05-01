use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::spells::{MAGIC_MISSILE, SACRED_BURST};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{Language, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Glass-cannon caster PC. INT-primary; Magic Missile is the staple
/// reliable damage tool (no attack roll, no save) and Sacred Burst
/// represents the radius-3 damage cantrip everyone secretly wishes they
/// had. Modeled as a level-3 wizard: low HP, AC 12, 4 level-1 slots and
/// 2 level-2 slots.
pub static WIZARD_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*MAGIC_MISSILE);
    actions.push(&*SACRED_BURST);
    CreatureTemplate {
        name: "Wizard",
        glyph: 'W',
        n_instances: 0,
        ac: 12,
        hitpoints: "3d6+3".parse().unwrap(),
        speed: 30.,
        strength: 8,
        intelligence: 16, // primary
        dexterity: 14,
        wisdom: 11,
        constitution: 13,
        charisma: 10,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Common]),
        cr: 1.0,
        size: Size::Medium,
        actions,
        spell_slots_by_level: vec![4, 2],
        rolls_death_saves: true,
        damage_resistances: HashSet::new(),
        damage_immunities: HashSet::new(),
        damage_vulnerabilities: HashSet::new(),
    }
});
