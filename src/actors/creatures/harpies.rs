use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{HARPY_TALONS, LURING_SONG};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Language, Size};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Harpy — CR 1 monstrosity. Talons for melee, Luring Song for ranged
/// control: a WIS save in a 30ft radius that Charms hearers for 3
/// rounds. Charmed enemies can't make hostile actions against the
/// harpy (via the standard SetCharmedBy link), so the song is a
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
        speed: 20., // walk speed (flying not modeled)
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
