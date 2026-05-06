use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::GREATAXE;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{Language, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Orc — fast, tough melee bruiser. Headline feature: 1d12 greataxe
/// damage spike on hit. Faster than a goblin, hits harder than a zombie,
/// rounds out the CR 1/2 bracket. Aggressive Run-In stat profile (STR
/// 16) puts pressure on backline ranged casters.
pub static ORC_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*GREATAXE);
    CreatureTemplate {
        name: "Orc",
        glyph: 'R',
        n_instances: 0,
        ac: 13,
        hitpoints: "2d8+6".parse().unwrap(),
        speed: 30.,
        strength: 16,
        intelligence: 7,
        dexterity: 12,
        wisdom: 11,
        constitution: 16,
        charisma: 10,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Common, Language::Orc]),
        cr: 0.5,
        size: Size::Medium,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: false,
        resistances: HashSet::new(),
        vulnerabilities: HashSet::new(),
        immunities: HashSet::new(),
    }
});
