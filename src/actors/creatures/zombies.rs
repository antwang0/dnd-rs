use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{TRIP, ZOMBIE_MULTISLAM};
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

pub static ZOMBIE_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    // Multislam is the zombie's main attack (2 swings per Action). Trip is
    // an alternative single attack that on hit forces a STR save or prone —
    // less raw damage but disables movement.
    actions.push(&*ZOMBIE_MULTISLAM);
    actions.push(&*TRIP);
    CreatureTemplate {
        name: "Zombie",
        glyph: 'Z',
        ac: 8,
        hitpoints: "2d8+6".parse().unwrap(),
        speed: 20.,
        strength: 13,
        intelligence: 3,
        dexterity: 6,
        wisdom: 6,
        constitution: 16,
        charisma: 5,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Common]), // plus one other
        cr: 0.25,
        size: Size::Medium,
        creature_type: CreatureType::Undead,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: false,
        // Zombies: undead — immune to poison; resistant to necrotic
        // (negative energy is what animates them, so it heals more than
        // it harms). Vulnerable to radiant (turn-undead flavor).
        damage_modifiers: HashMap::from([
            (DamageType::Poison, DamageModifier::Immunity),
            (DamageType::Necrotic, DamageModifier::Resistance),
            (DamageType::Radiant, DamageModifier::Vulnerability),
        ]),
        proficient_saves: HashSet::new(),
        // Undead: immune to Poisoned and Charmed.
        condition_immunities: HashSet::from([Condition::Poisoned, Condition::Charmed]),
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
