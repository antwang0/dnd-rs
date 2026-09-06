use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{HIPPOGRIFF_BEAK, HIPPOGRIFF_MULTI, HIPPOGRIFF_TALONS};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size};
use std::sync::LazyLock;

/// Hippogriff — CR 1 monstrosity. Mid-tier melee threat with a beak +
/// talons multiattack option. We don't model the 5e fly speed, so the
/// hippogriff behaves like a fast ground unit (60ft walk in our
/// engine's terms). Pairs well with mid-air-flavor encounters even
/// without the flight model.
pub static HIPPOGRIFF_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&HIPPOGRIFF_BEAK);
    actions.push(&HIPPOGRIFF_TALONS);
    actions.push(&*HIPPOGRIFF_MULTI);
    CreatureTemplate {
        name: "Hippogriff",
        // 'H' was free (Harpy uses 'h' lowercase). Picking 'H' for
        // hippogriff so the map differentiates Harpy / Hippogriff.
        glyph: 'H',
        ac: 11,
        // 4d10+4 = 26 average per MM.
        hitpoints: "4d10+4".parse().unwrap(),
        // RAW speed line: Speed 40 ft., fly 60 ft.
        speed: 40.0,
        fly_speed: 60.0,
        strength: 17,
        dexterity: 13,
        constitution: 13,
        intelligence: 2,
        wisdom: 12,
        charisma: 8,
        cr: 1.0,
        size: Size::Large,
        // 5e Mounted Combat: MM's aerial mount, and the cheap one — a griffon with no appetite for
        // the horse half of its rider's tack.
        mountable: true,
        creature_type: CreatureType::Beast,
        actions,
        ..CreatureTemplate::defaults()
    }
});
