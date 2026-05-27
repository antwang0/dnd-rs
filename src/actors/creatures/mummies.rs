use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{MUMMY_DREADFUL_GLARE, MUMMY_ROTTING_FIST};
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{DamageModifier, DamageType, Size, SpecialSense};
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
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::new(),
        cr: 3.0,
        size: Size::Medium,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: false,
        // Mummies: necrotic + poison immunity, resistance to physical
        // (non-magical) damage, immune to fire while wrapped (per MM,
        // bandages would burn but the mummy itself resists). We model
        // fire resistance to keep them tough vs spell pool.
        damage_modifiers: HashMap::from([
            (DamageType::Necrotic, DamageModifier::Immunity),
            (DamageType::Poison, DamageModifier::Immunity),
            (DamageType::Bludgeoning, DamageModifier::Resistance),
            (DamageType::Piercing, DamageModifier::Resistance),
            (DamageType::Slashing, DamageModifier::Resistance),
            // Mummies are vulnerable to fire — their wrappings burn.
            (DamageType::Fire, DamageModifier::Vulnerability),
        ]),
        proficient_saves: HashSet::new(),
        // Standard undead condition immunities, plus exhaustion (we
        // don't model exhaustion explicitly).
        condition_immunities: HashSet::from([
            Condition::Poisoned,
            Condition::Charmed,
            Condition::Frightened,
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
