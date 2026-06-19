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
        strength: 10,
        dexterity: 14,
        constitution: 15,
        intelligence: 6,
        wisdom: 8,
        charisma: 5,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Common]),
        cr: 0.25,
        size: Size::Medium,
        creature_type: CreatureType::Undead,
        actions,
        // Skeletons: vulnerable to bludgeoning (brittle bones), immune
        // to poison (no body chemistry), piercing slips between ribs.
        damage_modifiers: HashMap::from([
            (DamageType::Bludgeoning, DamageModifier::Vulnerability),
            (DamageType::Poison, DamageModifier::Immunity),
            (DamageType::Piercing, DamageModifier::Resistance),
        ]),
        // Undead: immune to Poisoned, Charmed.
        condition_immunities: HashSet::from([Condition::Poisoned, Condition::Charmed]),
        ..CreatureTemplate::defaults()
    }
});
