use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{GIANT_APE_FIST, GIANT_APE_MULTI, GIANT_APE_ROCK};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size};
use std::sync::LazyLock;

/// Giant Ape — CR 7 huge beast. The classic Kong-style brute: AC 12, big
/// HP pool, and a Multiattack of two crushing fists. Out of fist range it
/// throws boulders for 7d6 — a brutal single-die ranged option that
/// rewards positioning out of melee. No special features: pure stat block
/// muscle slots in between the Stone Giant (CR 7 ranged-bias) and the
/// Frost Giant (CR 8 melee-bias) for the upper-mid CR pool.
pub static GIANT_APE_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&GIANT_APE_FIST);
    actions.push(&GIANT_APE_ROCK);
    actions.push(&*GIANT_APE_MULTI);
    CreatureTemplate {
        name: "Giant Ape",
        // 'A' for Ape — capital because Huge.
        glyph: 'A',
        ac: 12,
        hitpoints: "16d12+64".parse().unwrap(),
        speed: 40.,
        strength: 23,
        intelligence: 5,
        dexterity: 14,
        wisdom: 12,
        constitution: 18,
        charisma: 7,
        cr: 7.0,
        size: Size::Huge,
        creature_type: CreatureType::Beast,
        actions,
        ..CreatureTemplate::defaults()
    }
});
