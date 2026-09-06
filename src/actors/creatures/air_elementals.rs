use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{AIR_ELEMENTAL_MULTI, AIR_ELEMENTAL_SLAM};
use crate::actors::actor_template::CreatureTemplate;
use crate::actors::creatures::fire_elementals::{
    elemental_body_defaults,
};
use crate::engine::types::{CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Air Elemental — CR 5 elemental. A churning vortex of wind: slam
/// melee for 2d8 + STR, doubled in the multiattack. Resistant to
/// lightning + thunder (the air-storm element), full immunity to
/// poison. Mirror of the fire elemental's elemental envelope: ignores
/// most mind / body control conditions (Charmed, Frightened, Paralyzed,
/// Petrified, Poisoned, Asleep, Prone, Grappled, Restrained).
pub static AIR_ELEMENTAL_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&AIR_ELEMENTAL_SLAM);
    actions.push(&*AIR_ELEMENTAL_MULTI);
    CreatureTemplate {
        name: "Air Elemental",
        // 'A' is unused in the creature pool; mnemonic for Air.
        glyph: 'A',
        ac: 15,
        // 12d10+24 = ~90 average per MM.
        hitpoints: "12d10+24".parse().unwrap(),
        // RAW speed line: Speed 10 ft., Fly 90 ft. (hover)
        speed: 10.0,
        fly_speed: 90.0,
        hovers: true,
        strength: 14,
        intelligence: 6,
        dexterity: 20,
        wisdom: 10,
        constitution: 14,
        charisma: 6,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Primordial]),
        cr: 5.0,
        size: Size::Large,
        creature_type: CreatureType::Elemental,
        actions,
        // Base BPS + Poison entries live in
        // `elemental_body_defaults`; the lightning and thunder rows are
        // the air variant's signature overlays. SRD 5.2 splits them —
        // "Resistances Bludgeoning, Lightning, Piercing, Slashing" and
        // "Immunities Poison, Thunder" — so the thunder half is a full
        // immunity and the lightning half is not. A living gale is not
        // hurt by noise.
        ..elemental_body_defaults([
            (DamageType::Lightning, DamageModifier::Resistance),
            (DamageType::Thunder, DamageModifier::Immunity),
        ])
    }
});
