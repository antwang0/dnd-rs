use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{SCIMITAR, SHORTBOW};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{Language, Size};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Bandit — humanoid skirmisher with mediocre stats but flexible
/// loadout: scimitar (Action) + shortbow (BonusAction) means they
/// always have something to do regardless of range. CR 1/8 — entry-
/// tier mob, weaker than goblin / orc, used for "swarm of bandits"
/// encounters.
pub static BANDIT_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*SCIMITAR);
    actions.push(&*SHORTBOW);
    CreatureTemplate {
        name: "Bandit",
        glyph: 'B',
        n_instances: 0,
        ac: 12,
        hitpoints: "2d8".parse().unwrap(),
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
        resistances: HashSet::new(),
        vulnerabilities: HashSet::new(),
        immunities: HashSet::new(),
    }
});
