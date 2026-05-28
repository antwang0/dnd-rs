use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{GARGOYLE_CLAWS, GARGOYLE_MULTI};
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Gargoyle — CR 2 elemental construct. Looks like a statue (False
/// Appearance, not modeled) and is resistant to non-magical physical
/// damage — we lift the "non-magical" qualifier and just give it
/// resistance to the physical trio so any weapon-wielding party feels
/// the chip. Immune to the usual elemental/construct suite: Poisoned,
/// Charmed, Exhaustion (not modeled), Petrified, Sleep/Unconscious from
/// magical sleep. We model what we can with the existing Condition
/// enum: Poisoned + Charmed + Prone (it flies) immunities.
pub static GARGOYLE_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&GARGOYLE_CLAWS);
    actions.push(&*GARGOYLE_MULTI);
    CreatureTemplate {
        name: "Gargoyle",
        glyph: 'G',
        ac: 15,
        hitpoints: "7d8+21".parse().unwrap(),
        speed: 30., // We don't model flight; treat the fly speed as walking.
        strength: 15,
        intelligence: 6,
        dexterity: 11,
        wisdom: 11,
        constitution: 16,
        charisma: 7,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Common]),
        cr: 2.0,
        size: Size::Medium,
        creature_type: CreatureType::Elemental,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: false,
        // 5e MM: resistance to bludgeoning / piercing / slashing from
        // non-magical attacks; immune to poison damage. We drop the
        // magic gate (not yet modeled) and apply physical resistance
        // wholesale — keeps the gargoyle feeling chunky without an
        // arms race.
        damage_modifiers: HashMap::from([
            (DamageType::Bludgeoning, DamageModifier::Resistance),
            (DamageType::Piercing, DamageModifier::Resistance),
            (DamageType::Slashing, DamageModifier::Resistance),
            (DamageType::Poison, DamageModifier::Immunity),
        ]),
        proficient_saves: HashSet::new(),
        condition_immunities: HashSet::from([
            Condition::Poisoned,
            Condition::Charmed,
            // Constructs ignore prone (they're not biological).
            Condition::Prone,
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
    }
});
