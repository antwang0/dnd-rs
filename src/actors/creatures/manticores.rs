use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{MANTICORE_MULTIATTACK, MANTICORE_SPIKES};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Language, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Manticore — CR 3 monstrosity. The marquee dual-lane threat: ranged
/// tail spikes (3d8 piercing at 30 ft) for stand-off pressure plus a
/// bite + double-claw multiattack when an enemy closes. Speaks Common
/// (5e RAW) which mostly matters for charm-immunity tracking — the
/// manticore is a creature, not a humanoid, but the language tag is
/// kept for completeness.
pub static MANTICORE_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*MANTICORE_SPIKES);
    actions.push(&*MANTICORE_MULTIATTACK);
    CreatureTemplate {
        name: "Manticore",
        // 'n' (lowercase) is free; uppercase 'N' is taken by Gnoll and
        // Minotaur. The manticore gets its own slot at 'n'.
        glyph: 'n',
        ac: 14,
        // 9d10+9 = 58 average per MM. Beefy CR 3 frame with the high
        // AC + ranged option offsetting the smaller HP pool of a CR 3.
        hitpoints: "9d10+9".parse().unwrap(),
        // RAW speed line: Speed 30 ft., fly 50 ft.
        speed: 30.0,
        fly_speed: 50.0,
        strength: 17,
        dexterity: 16,
        constitution: 17,
        intelligence: 7,
        wisdom: 12,
        charisma: 8,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Common]),
        cr: 3.0,
        size: Size::Large,
        creature_type: CreatureType::Monstrosity,
        actions,
        ..CreatureTemplate::defaults()
    }
});
