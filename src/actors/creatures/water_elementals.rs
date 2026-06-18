use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{
    WATER_ELEMENTAL_MULTI, WATER_ELEMENTAL_SLAM, WATER_ELEMENTAL_WHELM,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::actors::creatures::fire_elementals::{
    ELEMENTAL_CONDITION_IMMUNITIES, elemental_damage_modifiers,
};
use crate::engine::types::{CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Water Elemental — CR 5 elemental. The aquatic sibling of the Fire /
/// Earth / Air variants: 2d8 + STR slam doubled in the multiattack
/// (same per-swing dice as Air, lighter per-swing than Earth), full
/// immunity to poison, resistance to acid + BPS, and a recharge 4-6
/// `WHELM` burst that surges around its footprint, dealing 2d8 + STR
/// bludgeoning (DC 15 STR save, half on pass) and knocking failing
/// targets Prone. Completes the elemental quartet — the random
/// encounter pool now has all four primordial flavors available at
/// CR 5.
pub static WATER_ELEMENTAL_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&WATER_ELEMENTAL_SLAM);
    actions.push(&*WATER_ELEMENTAL_MULTI);
    actions.push(&*WATER_ELEMENTAL_WHELM);
    CreatureTemplate {
        name: "Water Elemental",
        // 'U' (uppercase) — free in the large-elemental slot. 'W' is
        // taken by Wolf; 'U' for "undine" reads as a watery mnemonic.
        glyph: 'U',
        ac: 14,
        // 12d10+48 = ~114 average per MM (CR 5 Water Elemental).
        hitpoints: "12d10+48".parse().unwrap(),
        // RAW: 30 ft walk + 90 ft swim. Engine isn't 3D so we collapse
        // to a fast ground speed — comparable to Air's flier without
        // tilting the encounter generator against it.
        speed: 50.,
        strength: 18,
        intelligence: 5,
        dexterity: 14,
        wisdom: 10,
        constitution: 18,
        charisma: 8,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Primordial]),
        cr: 5.0,
        size: Size::Large,
        creature_type: CreatureType::Elemental,
        actions,
        // Base BPS + Poison entries live in
        // `elemental_damage_modifiers`; acid resistance is the water
        // variant's signature overlay (waves diluting acid).
        damage_modifiers: elemental_damage_modifiers([(
            DamageType::Acid,
            DamageModifier::Resistance,
        )]),
        condition_immunities: ELEMENTAL_CONDITION_IMMUNITIES.clone(),
        // `WATER_ELEMENTAL_WHELM` reads this slot via the shared recharge
        // table; the engine's start-of-turn d6 flips it back on 4+.
        recharge_abilities: vec![("whelm", 4)],
        ..CreatureTemplate::defaults()
    }
});
