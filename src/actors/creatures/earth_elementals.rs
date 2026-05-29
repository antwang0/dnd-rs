use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{EARTH_ELEMENTAL_MULTI, EARTH_ELEMENTAL_SLAM};
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
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
            // 5e RAW: vulnerable to thunder. Modeled directly — the
            // earth elemental cracks like stone under sonic strikes.
            (DamageType::Thunder, DamageModifier::Vulnerability),
            // Mundane-physical resistance per MM (collapsed to flat
            // resistance; engine doesn't track magical-weapon distinction).
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
    }
});
