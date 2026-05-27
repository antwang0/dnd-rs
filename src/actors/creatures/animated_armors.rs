use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::SLAM;
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{DamageModifier, DamageType, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Animated Armor — CR 1 construct. A walking suit of armor: high AC,
/// modest HP, and the usual construct immunity suite. Slams instead of
/// any natural weapon. Construct immunities make it a natural pairing
/// with charm / sleep / poison spell loadouts — Color Spray, Sleep, and
/// Charm Person all bounce off.
pub static ANIMATED_ARMOR_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&SLAM);
    CreatureTemplate {
        name: "Animated Armor",
        // 'I' for "iron armor" — 'A' is already taken (antitoxin glyph
        // when on the ground; conflicts felt OK to break here but we
        // pick a distinct char anyway).
        glyph: 'I',
        ac: 18,
        hitpoints: "5d8+10".parse().unwrap(),
        speed: 25.,
        strength: 14,
        intelligence: 1,
        dexterity: 11,
        wisdom: 3,
        constitution: 13,
        charisma: 1,
        skills: HashSet::new(),
        items: Vec::new(),
        // Blindsight — animated objects "see" without conventional sight.
        senses: HashSet::from([SpecialSense::Blindsight(60)]),
        languages: HashSet::new(),
        cr: 1.0,
        size: Size::Medium,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: false,
        // Constructs are immune to poison and psychic damage in 5e.
        damage_modifiers: HashMap::from([
            (DamageType::Poison, DamageModifier::Immunity),
            (DamageType::Psychic, DamageModifier::Immunity),
        ]),
        proficient_saves: HashSet::new(),
        // Standard construct immunity suite: poisoned, charmed, frightened,
        // paralyzed, blinded (blindsight). Sleep and incapacitate go
        // through the engine's condition-immunity gate; we cover the
        // mind-affecting ones here.
        condition_immunities: HashSet::from([
            Condition::Poisoned,
            Condition::Charmed,
            Condition::Frightened,
            Condition::Paralyzed,
            Condition::Blinded,
            // Sleep is gated on Charmed immunity in our engine; making
            // it explicit here keeps the immunity check obvious.
            Condition::Asleep,
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
    }
});
