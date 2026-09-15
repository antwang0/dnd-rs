use crate::actions::class_features::{AGGRESSIVE, AGGRESSIVE_TAG};
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::GREATAXE;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Language, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Orc — STR-build heavy hitter. Greataxe with STR 16 yields a
/// respectable 1d12+3 melee profile, balanced by no ranged option.
/// **Aggressive** is how it gets there: a Bonus Action's worth of
/// charging at anything it can see, which is what turns "no ranged
/// option" from a weakness into a two-round problem for an exposed
/// caster. See `AGGRESSIVE_TAG`.
pub static ORC_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&GREATAXE);
    actions.push(&AGGRESSIVE);
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
        // SRD 5.2 **Aggressive** — thirty feet of walking does not
        // reach a caster who started at range, and this is how the
        // orc arrives a round early. See `AGGRESSIVE_TAG`.
        features: HashSet::from([AGGRESSIVE_TAG]),
        ..CreatureTemplate::defaults()
    }
});
