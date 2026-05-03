use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::spells::{FIRE_BOLT, MAGIC_MISSILE};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{Language, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Squishy INT-primary caster. Fire Bolt as the at-will damage option,
/// Magic Missile as the always-lands burst when slots are available.
/// Light HP, low AC, no melee — leans on range and force damage to
/// punch through resistances most monsters carry.
pub static WIZARD_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*FIRE_BOLT);
    actions.push(&*MAGIC_MISSILE);
    CreatureTemplate {
        name: "Wizard",
        glyph: 'M',
        n_instances: 0,
        ac: 12,
        hitpoints: "2d6+2".parse().unwrap(),
        speed: 30.,
        strength: 9,
        intelligence: 16, // primary spellcasting ability
        dexterity: 14,
        wisdom: 12,
        constitution: 12,
        charisma: 10,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Common]),
        cr: 0.5,
        size: Size::Medium,
        actions,
        // 3 level-1 slots — enough for 3 Magic Missiles before falling
        // back to cantrip Fire Bolt. No level-2 yet (no level-2 wizard
        // spell wired today).
        spell_slots_by_level: vec![3],
        rolls_death_saves: false,
        resistances: HashSet::new(),
        vulnerabilities: HashSet::new(),
        damage_immunities: HashSet::new(),
    }
});
