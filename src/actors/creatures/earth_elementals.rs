use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{EARTH_ELEMENTAL_MULTI, EARTH_ELEMENTAL_SLAM};
use crate::actors::actor_template::CreatureTemplate;
use crate::actors::creatures::fire_elementals::{
    elemental_defaults,
};
use crate::engine::types::{CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Earth Elemental — CR 5 elemental. A slow-moving boulder of fury:
/// 4d8 + STR slam melee, doubled in the multiattack — the heaviest
/// per-swing damage of the four elementals. Tradeoff is the 30ft
/// walking speed (no flying / swimming) and vulnerability to thunder
/// (RAW: shatters stone). Immune to poison; resistant to mundane
/// physical attacks. Elemental condition envelope identical to the
/// fire / air variants.
pub static EARTH_ELEMENTAL_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&EARTH_ELEMENTAL_SLAM);
    actions.push(&*EARTH_ELEMENTAL_MULTI);
    CreatureTemplate {
        name: "Earth Elemental",
        // 'Q' is unused; mnemonic chosen to avoid clashing with the
        // 'E' fire-elemental glyph.
        glyph: 'Q',
        ac: 17,
        // 12d10+60 = ~126 average per MM (CR 5, heavier HP pool).
        hitpoints: "12d10+60".parse().unwrap(),
        speed: 30., // burrow + walk, no flight
        strength: 20,
        intelligence: 5,
        dexterity: 8,
        wisdom: 10,
        constitution: 20,
        charisma: 5,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Primordial]),
        cr: 5.0,
        size: Size::Large,
        creature_type: CreatureType::Elemental,
        actions,
        // Base BPS + Poison entries live in
        // `elemental_defaults`; thunder vulnerability is the
        // earth variant's signature overlay (5e RAW: cracks like stone
        // under sonic strikes).
        ..elemental_defaults([(
            DamageType::Thunder,
            DamageModifier::Vulnerability,
        )])
    }
});
