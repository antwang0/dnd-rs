use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::GREATAXE;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Language, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Orc — STR-build heavy hitter. Greataxe with STR 16 yields a respectable
/// 1d12+3 melee profile, balanced by no ranged option. Solid mid-tier
/// melee enemy that punishes exposed casters.
pub static ORC_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&GREATAXE);
    CreatureTemplate {
        name: "Orc",
        // Lowercase 'o' to disambiguate from 'O' (Ogre).
        glyph: 'o',
        ac: 13,
        hitpoints: "2d8+6".parse().unwrap(),
        strength: 16,
        dexterity: 12,
        constitution: 16,
        intelligence: 7,
        wisdom: 11,
        charisma: 10,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Common, Language::Orc]),
        cr: 0.5,
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
        actions,
        ..CreatureTemplate::defaults()
    }
});
