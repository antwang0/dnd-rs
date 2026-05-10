use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{DAGGER, SHORTBOW};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{Language, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Sneaky DEX-based skirmisher; the cheap, swarm-friendly cousin of the
/// goblin. Uses a dagger (finesse, DEX-based melee) for an Action and a
/// shortbow for a bonus-action follow-up. Sunlight Sensitivity is not
/// modeled — there's no day/night system today.
pub static KOBOLD_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*DAGGER);
    actions.push(&*SHORTBOW);
    CreatureTemplate {
        name: "Kobold",
        glyph: 'K',
        n_instances: 0,
        ac: 12,
        hitpoints: "2d6-2".parse().unwrap(),
        speed: 30.,
        strength: 7,
        intelligence: 8,
        dexterity: 15,
        wisdom: 9,
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
        damage_resistances: HashSet::new(),
        damage_immunities: HashSet::new(),
        damage_vulnerabilities: HashSet::new(),
        condition_immunities: HashSet::new(),
    }
});
