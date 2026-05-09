use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::GREATAXE;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{Language, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Orc — STR-build heavy hitter. Greataxe with STR 16 yields a respectable
/// 1d12+3 melee profile, balanced by no ranged option. Solid mid-tier
/// melee enemy that punishes exposed casters.
pub static ORC_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*GREATAXE);
    CreatureTemplate {
        name: "Orc",
        // Lowercase 'o' to disambiguate from 'O' (Ogre).
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
        damage_immunities: HashSet::new(),
        damage_resistances: HashSet::new(),
        damage_vulnerabilities: HashSet::new(),
    }
});
