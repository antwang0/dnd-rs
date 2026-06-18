use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::HYENA_BITE;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size};
use std::sync::LazyLock;

/// Hyena — CR 0 medium beast. A pack scavenger that swarms wounded
/// prey: low base damage, but the `has_pack_tactics: true` flag turns
/// adjacent allies into advantage. Slots alongside the Wolf (CR ¼)
/// in the low-CR pool but without the Wolf's trip rider — pure dice
/// at the cheapest end of the encounter ladder.
pub static HYENA_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&HYENA_BITE);
    CreatureTemplate {
        name: "Hyena",
        // 'h' (lowercase) — free in the medium-beast slot. 'H' is taken
        // by Half-Orc / Harpy / Hippogriff / Hobgoblin / Hydra family.
        glyph: 'h',
        ac: 11,
        // 2d8 = 9 average per MM (CR 0).
        hitpoints: "2d8".parse().unwrap(),
        speed: 50.,
        strength: 11,
        intelligence: 2,
        dexterity: 13,
        wisdom: 12,
        constitution: 12,
        charisma: 5,
        cr: 0.0,
        size: Size::Medium,
        creature_type: CreatureType::Beast,
        actions,
        has_pack_tactics: true,
        ..CreatureTemplate::defaults()
    }
});
