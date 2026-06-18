use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{MUMMY_DREADFUL_GLARE, MUMMY_ROTTING_FIST};
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{CreatureType, DamageModifier, DamageType, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Mummy — CR 3 undead. Slow but punishing melee threat with two
/// action options: a rotting fist (bludgeoning + necrotic rider) for
/// melee, or a dreadful glare (ranged frighten gaze) to lock down
/// approaching enemies. Resistant to physical damage, immune to
/// poison/necrotic, and immune to the usual undead conditions
/// (Charmed/Frightened/Poisoned). Slow base speed (20 ft) keeps the
/// chase pressure low — the glare is the way they project threat at
/// range.
pub static MUMMY_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*MUMMY_ROTTING_FIST);
    actions.push(&*MUMMY_DREADFUL_GLARE);
    CreatureTemplate {
        name: "Mummy",
        // 'u' (lowercase) — distinct from existing M (Mage), N (Minotaur).
        glyph: 'u',
        ac: 11,
        // 9d8+18 = 58 average per MM.
        hitpoints: "9d8+18".parse().unwrap(),
        speed: 20.,
        strength: 16,
        intelligence: 6,
        dexterity: 8,
        wisdom: 10,
        constitution: 14,
        charisma: 12,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        cr: 3.0,
        size: Size::Medium,
        creature_type: CreatureType::Undead,
        actions,
        // Mummies: necrotic + poison immunity, resistance to physical
        // (non-magical) damage; vulnerable to fire (their wrappings burn).
        damage_modifiers: HashMap::from([
            (DamageType::Necrotic, DamageModifier::Immunity),
            (DamageType::Poison, DamageModifier::Immunity),
            (DamageType::Bludgeoning, DamageModifier::Resistance),
            (DamageType::Piercing, DamageModifier::Resistance),
            (DamageType::Slashing, DamageModifier::Resistance),
            (DamageType::Fire, DamageModifier::Vulnerability),
        ]),
        // Standard undead condition immunities.
        condition_immunities: HashSet::from([
            Condition::Poisoned,
            Condition::Charmed,
            Condition::Frightened,
        ]),
        ..CreatureTemplate::defaults()
    }
});
