use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::GREATAXE;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{Language, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Orc — beefy STR-based melee bruiser. Bigger HP pool than a goblin,
/// 1d12 greataxe (highest single-die damage in the codebase outside
/// the ogre's club), no ranged option. Pairs well as the front line
/// for an enemy team while goblins / skeletons lurk behind with bows.
pub static ORC_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*GREATAXE);
    CreatureTemplate {
        name: "Orc",
        glyph: 'o',
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
        damage_adjustments: HashMap::new(),
        save_proficiencies: HashSet::new(),
    }
});
