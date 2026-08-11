use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::SPECTATOR_EYE_RAY;
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{CreatureType, Language, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Spectator — CR 3 aberration, a mini-beholder. Hovers in place
/// (speed 0, conceptual hover 30 — we model as 30 ground speed since
/// hover vs walk doesn't mechanically differ in the engine) and fires
/// eye rays at range. Small size, decent WIS and INT, but physically
/// weak (STR 8). Immune to Prone because it hovers — can't be knocked
/// down when you don't touch the ground.
pub static SPECTATOR_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*SPECTATOR_EYE_RAY);
    CreatureTemplate {
        name: "Spectator",
        // 'E' for Eye — 'S' and 'B' are taken by spiders and beholders.
        glyph: 'E',
        ac: 14,
        hitpoints: "6d8+6".parse().unwrap(),
        // RAW speed line: Speed 0 ft., fly 30 ft. (hover)
        speed: 0.0,
        fly_speed: 30.0,
        hovers: true,
        strength: 8,
        dexterity: 14,
        constitution: 12,
        intelligence: 13,
        wisdom: 14,
        charisma: 11,
        senses: HashSet::from([SpecialSense::Darkvision(120)]),
        languages: HashSet::from([Language::DeepSpeech, Language::Undercommon]),
        cr: 3.0,
        size: Size::Small,
        creature_type: CreatureType::Aberration,
        actions,
        // Hovering creatures can't be knocked Prone.
        condition_immunities: HashSet::from([Condition::Prone]),
        ..CreatureTemplate::defaults()
    }
});
