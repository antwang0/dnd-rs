use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{HARPY_TALONS, LURING_SONG};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Language, Size};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Harpy — CR 1 monstrosity. Talons for melee, Luring Song for ranged
/// control: a WIS save in a 30ft radius that Charms hearers for 3
/// rounds. Charmed enemies can't make hostile actions against the
/// harpy (via the standard SetConditionLink(Charmed) link), so the song is a
/// genuine threat even against full-strength parties — pulling one
/// PC out of the kill-the-harpy plan for a turn is a real swing.
pub static HARPY_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&HARPY_TALONS);
    actions.push(&*LURING_SONG);
    CreatureTemplate {
        name: "Harpy",
        // 'H' for Harpy — Hobgoblin already uses 'B', Bugbear uses 'G',
        // so 'H' is free.
        glyph: 'H',
        ac: 11,
        hitpoints: "7d8+7".parse().unwrap(),
        // RAW is "20 ft., fly 40 ft." and the engine's convention for an
        // innate flier is to collapse the two onto the fly speed — the
        // board is 2D, nothing pathfinds through the air, and a creature
        // that spends the fight on the wing should move at the speed it
        // actually moves at. Twenty-eight other templates read that way
        // (griffon, giant eagle, gargoyle, erinyes, bone devil…); this
        // one alone took the walking number, which made the engine's
        // only flying charmer the slowest monstrosity on the roster and
        // left its 30 ft song permanently out of range of anything that
        // opened at distance.
        speed: 40.,
        strength: 12,
        dexterity: 13,
        constitution: 12,
        intelligence: 7,
        wisdom: 10,
        charisma: 13,
        languages: HashSet::from([Language::Common]),
        cr: 1.0,
        size: Size::Medium,
        creature_type: CreatureType::Monstrosity,
        actions,
        ..CreatureTemplate::defaults()
    }
});
