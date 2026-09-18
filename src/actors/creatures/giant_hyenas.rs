use crate::actions::class_features::RAMPAGE_TAG;
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::GIANT_HYENA_BITE;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Giant Hyena — CR 1 large beast. The pack-leader upgrade of the
/// vanilla Hyena: bigger dice on the bite (2d6 + STR vs 1d6 + STR),
/// roughly Dire-Wolf-tier HP, and the gnoll's own **Rampage** — the
/// bonus-action lunge and bite when it downs a creature. See
/// `RAMPAGE_TAG`. With Pack Tactics beside it, a pair of these is the
/// CR-1 tier's clearest lesson in why numbers matter.
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
        // SRD 5.2 **Rampage**, the gnoll's own trait on the beast it
        // keeps. See `RAMPAGE_TAG`.
        features: HashSet::from([RAMPAGE_TAG]),
        has_pack_tactics: true,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        ..CreatureTemplate::defaults()
    }
});
