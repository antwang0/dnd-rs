use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{AIR_ELEMENTAL_MULTI, AIR_ELEMENTAL_SLAM};
use crate::actors::actor_template::CreatureTemplate;
use crate::actors::creatures::fire_elementals::ELEMENTAL_CONDITION_IMMUNITIES;
use crate::engine::types::{CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
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
        speed: 90., // flying speed 90ft RAW
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
        damage_modifiers: HashMap::from([
            (DamageType::Poison, DamageModifier::Immunity),
            (DamageType::Lightning, DamageModifier::Resistance),
            (DamageType::Thunder, DamageModifier::Resistance),
            // 5e RAW: resistance to bludgeoning / piercing / slashing from
            // non-magical attacks — we collapse to a flat physical
            // resistance since the engine doesn't track magical-weapon.
            (DamageType::Bludgeoning, DamageModifier::Resistance),
            (DamageType::Piercing, DamageModifier::Resistance),
            (DamageType::Slashing, DamageModifier::Resistance),
        ]),
        condition_immunities: ELEMENTAL_CONDITION_IMMUNITIES.clone(),
        ..CreatureTemplate::defaults()
    }
});
