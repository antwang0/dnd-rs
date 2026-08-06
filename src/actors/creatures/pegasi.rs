use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::PEGASUS_HOOVES;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Language, Size, Skill, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Pegasus — CR 2 large celestial. The classic winged horse: 60 ft fly
/// (we model the speed scalar, the engine isn't 3D so flight collapses
/// to "fast ground travel"), Hooves attack for the lone Action lane,
/// Perception proficiency, and the Celestial creature type for the
/// Protection from Evil and Good aura interactions. Sibling to Polar
/// Bear (CR 2 large beast) on the CR ladder but on the celestial side
/// of the pool — a non-fiend large flier slots cleanly between the
/// Hippogriff (CR 1) and the Manticore (CR 3).
pub static PEGASUS_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&PEGASUS_HOOVES);
    CreatureTemplate {
        name: "Pegasus",
        // 'e' (lowercase) — free in the large celestial slot. 'P' is
        // taken by Polar Bear; 'e' for "equus" reads as horse-flavored.
        glyph: 'e',
        ac: 12,
        // 7d10+14 ≈ 59 average per MM (CR 2).
        hitpoints: "7d10+14".parse().unwrap(),
        // 60 ft fly RAW. Engine isn't 3D so the fly speed becomes
        // ground speed; pegasus stays the fastest celestial in the pool.
        speed: 60.,
        strength: 18,
        intelligence: 10,
        dexterity: 15,
        wisdom: 15,
        constitution: 16,
        charisma: 13,
        skills: HashSet::from([Skill::Perception]),
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Celestial, Language::Common]),
        cr: 2.0,
        size: Size::Large,
        // 5e Mounted Combat: MM: the celestial steed, and the reason `mountable` is a declared flag
        // rather than a Beast-and-Large predicate.
        mountable: true,
        creature_type: CreatureType::Celestial,
        actions,
        ..CreatureTemplate::defaults()
    }
});
