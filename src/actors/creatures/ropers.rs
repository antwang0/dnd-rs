use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::BITE;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Roper — CR 5 monstrosity. Cave-dwelling ambush predator disguised as
/// a stalagmite. Extremely tough (AC 20) but very slow (10ft speed).
/// Attacks with a powerful bite. Its signature tendril-grapple and reel
/// abilities are not yet modelled — just the bite and the rock-hard shell.
pub static ROPER_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&BITE);
    CreatureTemplate {
        name: "Roper",
        glyph: 'r',
        ac: 20,
        hitpoints: "11d10+44".parse().unwrap(),
        speed: 10.,
        strength: 18,
        dexterity: 8,
        constitution: 18,
        intelligence: 7,
        wisdom: 16,
        charisma: 6,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        cr: 5.0,
        size: Size::Large,
        creature_type: CreatureType::Monstrosity,
        actions,
        ..CreatureTemplate::defaults()
    }
});
