use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{LONGBOW, WIGHT_LIFE_DRAIN};
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{AbilityScoreType, CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Wight — CR 3 undead. Solid melee threat with the marquee Life Drain
/// rider (CON save vs max-HP reduction on a hit). Unlike the Wraith,
/// the wight is corporeal — no incorporeal-style movement immunity —
/// but keeps the standard undead poison/necrotic immunities and
/// charm/exhaustion-style Charmed/Frightened immunity is folded into
/// the lighter Charmed-only immunity (Frightened still bites them).
pub static WIGHT_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*WIGHT_LIFE_DRAIN);
    // Wights wield a longbow in 5e MM — keep the ranged option so the
    // AI can choose between closing for life drain or harassing from
    // range when blocked.
    actions.push(&LONGBOW);
    CreatureTemplate {
        name: "Wight",
        // 'i' is free (W taken by wolf; w taken by werewolf; we use
        // 'i' for "wight" since it's distinctive on the map).
        glyph: 'i',
        ac: 14,
        // 9d8+18 = 45 average per MM.
        hitpoints: "9d8+18".parse().unwrap(),
        speed: 30.,
        strength: 15,
        intelligence: 10,
        dexterity: 14,
        wisdom: 13,
        constitution: 16,
        charisma: 15,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Common]),
        cr: 3.0,
        size: Size::Medium,
        creature_type: CreatureType::Undead,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: false,
        // 5e Wight: necrotic immunity, poison immunity, resistance to
        // physical (non-magical) damage. We use resistance to keep the
        // tradeoff consistent with the rest of the undead pool.
        damage_modifiers: HashMap::from([
            (DamageType::Necrotic, DamageModifier::Immunity),
            (DamageType::Poison, DamageModifier::Immunity),
            (DamageType::Bludgeoning, DamageModifier::Resistance),
            (DamageType::Piercing, DamageModifier::Resistance),
            (DamageType::Slashing, DamageModifier::Resistance),
        ]),
        // Undead proficiencies — wights have decent CON/CHA from MM.
        proficient_saves: HashSet::from([AbilityScoreType::Constitution]),
        // Standard undead condition immunities.
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
        legendary_actions_per_round: 0,
        has_extra_attack: false,
        brutal_critical_dice: 0,
        crit_threshold: 20,
        has_lucky: false,
        has_aura_of_protection: false,
        has_aura_of_courage: false,
        has_savage_attacks: false,
        has_dwarven_resilience: false,
    }
});
