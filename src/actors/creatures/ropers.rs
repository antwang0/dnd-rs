use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{ROPER_BITE, ROPER_MULTI, ROPER_REEL, ROPER_TENDRIL};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Roper — CR 5 monstrosity. Cave-dwelling ambush predator disguised as
/// a stalagmite. Extremely tough (AC 20) and almost immobile (10 ft
/// speed), which is the whole design: the roper does not come to you.
///
/// The full sequence is on the template now — four `ROPER_TENDRIL`
/// grabs at fifty feet, `ROPER_REEL` to haul the catch ten tiles
/// closer, and `ROPER_BITE` waiting at the end of it. A 10 ft speed is
/// a liability on a creature that has to close and a non-issue on one
/// that drags the fight to itself, and until the tendrils existed this
/// stat block was only the liability.
pub static ROPER_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&ROPER_BITE);
    actions.push(&*ROPER_TENDRIL);
    actions.push(&*ROPER_MULTI);
    actions.push(&*ROPER_REEL);
    CreatureTemplate {
        name: "Roper",
        glyph: 'r',
        ac: 20,
        hitpoints: "11d10+33".parse().unwrap(),
        speed: 10.,
        strength: 18,
        dexterity: 8,
        constitution: 17,
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
