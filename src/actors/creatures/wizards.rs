use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::spells::{BURNING_HANDS, FIRE_BOLT, MAGIC_MISSILE};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{Language, Size};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Apprentice-style INT-caster. Fire Bolt as the spammable cantrip,
/// Burning Hands and Magic Missile as the level-1 spell budget. Modeled
/// roughly equivalent to MM Apprentice Mage / Cult Fanatic stats — light
/// HP, robe-tier AC, no melee. Distinct from CLERIC: WIS-spellcasting vs
/// INT, healing vs blasting, save-vs-attack-roll vs aura.
pub static WIZARD_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*FIRE_BOLT);
    actions.push(&*MAGIC_MISSILE);
    actions.push(&*BURNING_HANDS);
    CreatureTemplate {
        name: "Wizard",
        glyph: 'M', // 'W' would collide with Wolf
        n_instances: 0,
        ac: 12, // mage armor flavor
        hitpoints: "2d6+2".parse().unwrap(),
        speed: 30.,
        strength: 9,
        intelligence: 15, // primary spellcasting ability
        dexterity: 14,
        wisdom: 12,
        constitution: 11,
        charisma: 11,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::new(),
        languages: HashSet::from([Language::Common]),
        cr: 0.5,
        size: Size::Medium,
        actions,
        // 3 level-1 slots — split between Burning Hands and Magic Missile.
        spell_slots_by_level: vec![3],
        rolls_death_saves: false,
        resistances: HashSet::new(),
        immunities: HashSet::new(),
        vulnerabilities: HashSet::new(),
    }
});
