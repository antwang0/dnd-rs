use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{TRICERATOPS_GORE, TRICERATOPS_STOMP};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size};
use std::sync::LazyLock;

/// Triceratops — CR 5 huge beast. Pure stat-block muscle: a single
/// monster gore (4d8 + STR piercing at reach 2) or a 3d10 stomp at
/// reach 1. RAW also has Trampling Charge (Prone rider on a straight-
/// line move-then-hit); we collapse that to clean high-damage swings
/// because the engine doesn't track straight-line movement. Slots
/// alongside Gorgon (CR 5) and Hill Giant (CR 5) in the upper-mid
/// melee pool — the dinosaur option without breath or rider effects.
pub static TRICERATOPS_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&TRICERATOPS_GORE);
    actions.push(&TRICERATOPS_STOMP);
    CreatureTemplate {
        name: "Triceratops",
        // 'q' (lowercase) — free in the huge beast slot. 'T' is taken
        // by Treant / Tiger; 'q' reads as a four-legged silhouette.
        glyph: 'q',
        ac: 14,
        // 12d12+36 ≈ 95 average per MM (CR 5).
        hitpoints: "12d12+36".parse().unwrap(),
        speed: 50.,
        strength: 22,
        intelligence: 2,
        dexterity: 9,
        wisdom: 11,
        constitution: 17,
        charisma: 5,
        cr: 5.0,
        size: Size::Huge,
        creature_type: CreatureType::Beast,
        actions,
        // RAW: when the triceratops closes at least the clause's distance in a
        // straight line and then connects with its gore, the hit carries
        // a Strength save vs prone. Read at the melee attack
        // chokepoint off `ActorInstance::charge`.
        charge: Some(crate::actions::monster_attacks::TRICERATOPS_CHARGE),
        ..CreatureTemplate::defaults()
    }
});
