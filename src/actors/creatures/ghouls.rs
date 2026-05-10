use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::GHOUL_CLAWS;
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{DamageType, Language, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Undead ambusher. Headline feature: claws on hit force a CON save (DC
/// 10) or be Stunned (5e Paralyzed). Combined with the per-condition
/// disadvantage clauses, a successful paralysis often spirals into a
/// near-instant kill — pack a friend with healing word.
pub static GHOUL_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*GHOUL_CLAWS);
    CreatureTemplate {
        name: "Ghoul",
        glyph: 'H',
        n_instances: 0,
        ac: 12,
        hitpoints: "5d8".parse().unwrap(),
        speed: 30.,
        strength: 13,
        intelligence: 7,
        dexterity: 15,
        wisdom: 10,
        constitution: 10,
        charisma: 6,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Common]),
        cr: 1.0,
        size: Size::Medium,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: false,
        // Undead nature: poison runs through dead tissue without effect.
        // Resistant to necrotic — they're already half-dead.
        damage_resistances: HashSet::from([DamageType::Necrotic]),
        damage_immunities: HashSet::from([DamageType::Poison]),
        damage_vulnerabilities: HashSet::new(),
        condition_immunities: HashSet::from([Condition::Poisoned]),
    }
});
