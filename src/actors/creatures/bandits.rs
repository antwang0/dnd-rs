use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{SCIMITAR, SHORTBOW};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{Language, Size};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Bandit — light humanoid skirmisher. Scimitar in melee; shortbow
/// bonus-action ranged follow-up mirrors the Goblin's action economy but
/// at slightly higher AC and average HP. First human-shaped enemy,
/// providing variety against undead / beast encounters.
pub static BANDIT_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*SCIMITAR);
    actions.push(&*SHORTBOW);
    CreatureTemplate {
        name: "Bandit",
        glyph: 'B',
        n_instances: 0,
        ac: 12,
        hitpoints: "2d8+2".parse().unwrap(),
        speed: 30.,
        strength: 11,
        intelligence: 10,
        dexterity: 12,
        wisdom: 10,
        constitution: 12,
        charisma: 10,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::new(),
        languages: HashSet::from([Language::Common]),
        cr: 0.125,
        size: Size::Medium,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: false,
        immunities: HashSet::new(),
        resistances: HashSet::new(),
        vulnerabilities: HashSet::new(),
    }
});
