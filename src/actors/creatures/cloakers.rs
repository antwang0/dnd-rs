use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::CLOAKER_TAIL;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Cloaker — CR 8 aberration. Ray-like creature that wraps around its
/// prey. Attacks with a barbed tail at 10ft reach. Its signature
/// envelop/attach ability is not yet modelled. Ground speed 10ft (flying
/// speed 40ft not tracked). Darkvision 60ft.
pub static CLOAKER_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&CLOAKER_TAIL);
    CreatureTemplate {
        name: "Cloaker",
        glyph: 'c',
        ac: 14,
        hitpoints: "12d10+36".parse().unwrap(),
        speed: 10.,
        strength: 17,
        intelligence: 11,
        dexterity: 15,
        wisdom: 12,
        constitution: 16,
        charisma: 14,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        cr: 8.0,
        size: Size::Large,
        creature_type: CreatureType::Aberration,
        actions,
        ..CreatureTemplate::defaults()
    }
});
