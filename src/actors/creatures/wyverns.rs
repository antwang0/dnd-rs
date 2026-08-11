use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{WYVERN_BITE, WYVERN_STINGER};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Wyvern — CR 6 dragon. Large-footprint flying predator with two
/// action lanes:
/// - Bite: 2d6 + STR piercing melee (reach 2 ≈ 10ft).
/// - Stinger: 2d6 + STR piercing melee plus a brutal CON-save poison
///   rider (7d6 poison on fail, half on save) — the wyvern's signature
///   finisher.
///
/// No language slot per MM (wyverns are non-sentient predators); we
/// leave languages empty rather than tag a placeholder. No condition
/// immunities — wyverns are mortal, just very angry.
pub static WYVERN_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&WYVERN_BITE);
    actions.push(&*WYVERN_STINGER);
    CreatureTemplate {
        name: "Wyvern",
        // 'y' (lowercase) is free; uppercase 'Y' is Yeti.
        glyph: 'y',
        ac: 13,
        // 13d10+39 = ~110 average per MM (CR 6).
        hitpoints: "13d10+39".parse().unwrap(),
        // RAW speed line: Speed 20 ft., fly 80 ft.
        speed: 20.0,
        fly_speed: 80.0,
        strength: 19,
        dexterity: 10,
        constitution: 16,
        intelligence: 5,
        wisdom: 12,
        charisma: 6,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        cr: 6.0,
        size: Size::Large,
        creature_type: CreatureType::Monstrosity,
        actions,
        ..CreatureTemplate::defaults()
    }
});
