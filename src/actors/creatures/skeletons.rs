use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::LONGBOW;
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// 5e-flavored skeleton archer. Lower HP than a zombie but DEX-based ranged
/// attack — pressure-tests the longbow + LOS path. Vulnerable to bludgeoning
/// (brittle bones); immune to poison and exhaustion (undead).
pub static SKELETON_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&LONGBOW);
    CreatureTemplate {
        name: "Skeleton",
        glyph: 'S',
        ac: 13,
        hitpoints: "2d8+4".parse().unwrap(),
        speed: 30.,
        strength: 10,
        intelligence: 6,
        dexterity: 14,
        wisdom: 8,
        constitution: 15,
        charisma: 5,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Common]),
        cr: 0.25,
        size: Size::Medium,
        creature_type: CreatureType::Undead,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: false,
        // Skeletons: vulnerable to bludgeoning (brittle bones), immune
        // to poison (no body chemistry).
        damage_modifiers: HashMap::from([
            (DamageType::Bludgeoning, DamageModifier::Vulnerability),
            (DamageType::Poison, DamageModifier::Immunity),
            // Bones / arrowfolk: piercing slips between ribs.
            (DamageType::Piercing, DamageModifier::Resistance),
        ]),
        proficient_saves: HashSet::new(),
        // Undead: immune to Poisoned, Charmed, Frightened (sleep too).
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
    }
});
