use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::BASILISK_BITE;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Basilisk — CR 3 monstrosity. Eight-legged reptile with a petrifying
/// gaze. Bite deals 2d6+3 piercing plus a CON save (DC 12) for Petrified
/// (1 round). AC 15, ~52 HP (8d8+16). Slow (speed 20ft) but durable,
/// with darkvision 60ft.
pub static BASILISK_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*BASILISK_BITE);
    CreatureTemplate {
        name: "Basilisk",
        glyph: 'ß',
        ac: 15,
        hitpoints: "8d8+16".parse().unwrap(),
        speed: 20.,
        strength: 16,
        intelligence: 2,
        dexterity: 8,
        wisdom: 8,
        constitution: 15,
        charisma: 7,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        cr: 3.0,
        size: Size::Medium,
        creature_type: CreatureType::Monstrosity,
        actions,
        ..CreatureTemplate::defaults()
    }
});
