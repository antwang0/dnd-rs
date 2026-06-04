use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::GHOUL_CLAWS;
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{CreatureType, DamageType, Language, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Ghoul — CR 1 undead. Claws have a paralysis rider on hit (CON save
/// DC 10 or be paralyzed for two rounds). Paralysis turns subsequent
/// melee hits into auto-crits — a lone ghoul that lands a save-fail
/// claw can lock a PC out of two full turns. Resistant to nothing
/// special, immune to Poisoned / Charmed (standard undead suite).
pub static GHOUL_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*GHOUL_CLAWS);
    CreatureTemplate {
        name: "Ghoul",
        // 'U' for undead — 'G' is taken by goblin.
        glyph: 'U',
        ac: 12,
        hitpoints: "5d8".parse().unwrap(),
        speed: 30.,
        strength: 13,
        intelligence: 7,
        dexterity: 15,
        wisdom: 10,
        constitution: 10,
        charisma: 6,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Common]),
        cr: 1.0,
        size: Size::Medium,
        creature_type: CreatureType::Undead,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: false,
        damage_modifiers: HashMap::from([(DamageType::Poison, crate::engine::types::DamageModifier::Immunity)]),
        proficient_saves: HashSet::new(),
        condition_immunities: HashSet::from([Condition::Poisoned, Condition::Charmed]),
        features: HashSet::new(),
        regen_per_round: 0,
        regen_suppressors: HashSet::new(),
        legendary_resistances: 0,
        has_evasion: false,
        has_uncanny_dodge: false,
        has_deflect_missiles: false,
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
        has_brave: false,
        has_fey_ancestry: false,
        has_aura_of_protection: false,
        has_aura_of_courage: false,
        has_savage_attacks: false,
        has_dwarven_resilience: false,
        has_gnome_cunning: false,
        draconic_ancestry: None,
        sorcery_points: 0,
    }
});
