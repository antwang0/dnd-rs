use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::SLAM;
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{CreatureType, DamageModifier, DamageType, Size, SpecialSense};
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
        // 'I' for "iron armor" — distinct from 'A' (Aboleth) and 'a'
        // (Amulet of Health ground glyph).
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
        // Blindsight — animated objects "see" without conventional sight.
        senses: HashSet::from([SpecialSense::Blindsight(60)]),
        cr: 1.0,
        size: Size::Medium,
        creature_type: CreatureType::Construct,
        actions,
        // Constructs are immune to poison and psychic damage in 5e.
        damage_modifiers: HashMap::from([
            (DamageType::Poison, DamageModifier::Immunity),
            (DamageType::Psychic, DamageModifier::Immunity),
        ]),
        // Standard construct immunity suite — Asleep is explicitly listed
        // alongside Charmed for documentation clarity (the engine's
        // dynamic_immunity_to chokepoint already gates Asleep on Charmed
        // for several other immunity sources).
        condition_immunities: HashSet::from([
            Condition::Poisoned,
            Condition::Charmed,
            Condition::Frightened,
            Condition::Paralyzed,
            Condition::Blinded,
            Condition::Asleep,
        ]),
        ..CreatureTemplate::defaults()
    }
});
