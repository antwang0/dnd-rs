use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{GIANT_SCORPION_CLAW, GIANT_SCORPION_STING};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Giant Scorpion — CR 3 beast. Massive arachnid with two claws and a
/// venomous stinger. Claw deals 1d8+2 bludgeoning + grapple on hit.
/// Sting deals 1d10+2 piercing + 4d10 poison (CON save DC 12 for half).
/// AC 15, ~52 HP (7d10+14). Blindsight 60ft. Multiattack: two claws +
/// one sting per turn.
pub static GIANT_SCORPION_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&GIANT_SCORPION_CLAW);
    actions.push(&*GIANT_SCORPION_STING);
    CreatureTemplate {
        name: "Giant Scorpion",
        glyph: '†',
        ac: 15,
        hitpoints: "7d10+14".parse().unwrap(),
        speed: 40.,
        strength: 16,
        dexterity: 13,
        constitution: 15,
        intelligence: 1,
        wisdom: 9,
        charisma: 3,
        senses: HashSet::from([SpecialSense::Blindsight(60)]),
        cr: 3.0,
        size: Size::Large,
        creature_type: CreatureType::Beast,
        actions,
        ..CreatureTemplate::defaults()
    }
});
