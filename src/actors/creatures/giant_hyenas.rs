use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::GIANT_HYENA_BITE;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size};
use std::sync::LazyLock;

/// Giant Hyena — CR 1 large beast. The pack-leader upgrade of the
/// vanilla Hyena: bigger dice on the bite (2d6 + STR vs 1d6 + STR),
/// roughly Dire-Wolf-tier HP. RAW also has a Rampage rider
/// (bonus-action bite when you down a creature); we skip it since the
/// AI's bonus-action picker doesn't yet model down-triggered reactions
/// and the headline kit — heavy bite + pack tactics — already lands
/// the CR-1 dice tier on the chassis.
pub static GIANT_HYENA_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&GIANT_HYENA_BITE);
    CreatureTemplate {
        name: "Giant Hyena",
        // 'H' (uppercase) — free in the large-beast slot when paired
        // with the lowercase 'h' for the vanilla Hyena.
        glyph: 'H',
        ac: 12,
        // 6d10+12 = 45 average per MM (CR 1).
        hitpoints: "6d10+12".parse().unwrap(),
        speed: 50.,
        strength: 16,
        intelligence: 2,
        dexterity: 14,
        wisdom: 12,
        constitution: 14,
        charisma: 7,
        cr: 1.0,
        size: Size::Large,
        creature_type: CreatureType::Beast,
        actions,
        has_pack_tactics: true,
        ..CreatureTemplate::defaults()
    }
});
