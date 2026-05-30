use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{AIR_ELEMENTAL_MULTI, AIR_ELEMENTAL_SLAM};
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
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
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Primordial]),
        cr: 5.0,
        size: Size::Large,
        creature_type: CreatureType::Elemental,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: false,
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
        proficient_saves: HashSet::new(),
        condition_immunities: HashSet::from([
            Condition::Charmed,
            Condition::Frightened,
            Condition::Paralyzed,
            Condition::Petrified,
            Condition::Poisoned,
            Condition::Asleep,
            Condition::Prone,
            Condition::Grappled,
            Condition::Restrained,
        ]),
        features: HashSet::new(),
        regen_per_round: 0,
        regen_suppressors: HashSet::new(),
        legendary_resistances: 0,
        has_evasion: false,
        has_uncanny_dodge: false,
        has_displacement: false,
        has_danger_sense: false,
        has_pack_tactics: false,
        has_magic_resistance: false,
        recharge_abilities: Vec::new(),
        legendary_actions_per_round: 0,
        has_extra_attack: false,
        brutal_critical_dice: 0,
        crit_threshold: 20,
        has_lucky: false,
        has_aura_of_protection: false,
        has_aura_of_courage: false,
        has_savage_attacks: false,
        has_dwarven_resilience: false,
    }
});
