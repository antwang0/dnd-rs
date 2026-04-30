use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{DAGGER, SLING};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{Language, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Tiny dex-build melee/ranged hybrid. Pack hunters in 5e (Pack Tactics
/// gives advantage when an ally is within 5ft of the target). We don't
/// model that yet — Kobolds today are just low-HP, AC 12 chip damagers
/// that pair well with allies because their 1d4 stings stack up.
pub static KOBOLD_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*DAGGER);
    actions.push(&*SLING);
    CreatureTemplate {
        name: "Kobold",
        glyph: 'k',
        n_instances: 0,
        ac: 12,
        hitpoints: "2d6-2".parse().unwrap(),
        speed: 30.,
        strength: 7,
        intelligence: 8,
        dexterity: 15,
        wisdom: 7,
        constitution: 9,
        charisma: 8,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Common, Language::Draconic]),
        cr: 0.125,
        size: Size::Small,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: false,
        resistances: HashSet::new(),
        immunities: HashSet::new(),
        // Sunlight Sensitivity in 5e — abstracted as plain disadvantage,
        // which we don't yet model as a passive. Keep flat for now.
        vulnerabilities: HashSet::new(),
    }
});
