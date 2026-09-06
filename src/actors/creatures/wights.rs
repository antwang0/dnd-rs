use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{LONGBOW, WIGHT_LIFE_DRAIN};
use crate::actors::actor_template::{CreatureTemplate, damage_modifiers_from};
use crate::conditions::Condition;
use crate::engine::types::{AbilityScoreType, CreatureType, DamageModifier, DamageType, Language, Size, Skill, SpecialSense};
use std::collections::HashSet;
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
        strength: 15,
        dexterity: 14,
        constitution: 16,
        intelligence: 10,
        wisdom: 13,
        charisma: 15,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Common]),
        cr: 3.0,
        size: Size::Medium,
        creature_type: CreatureType::Undead,
        actions,
        // 5e Wight: necrotic + poison immunity, non-magical BPS resistance.
        damage_modifiers: damage_modifiers_from([
            (DamageType::Necrotic, DamageModifier::Immunity),
            (DamageType::Poison, DamageModifier::Immunity),
        ]),
        // Undead proficiencies — wights have decent CON/CHA from MM.
        proficient_saves: HashSet::from([AbilityScoreType::Constitution]),
        // Standard undead condition immunities.
        condition_immunities: HashSet::from([
            // SRD 5.2 "Immunities Poison; Exhaustion, Poisoned".
            Condition::Exhausted,
            Condition::Poisoned,
            Condition::Charmed,
            Condition::Frightened,
        ]),
        skills: HashSet::from([Skill::Perception, Skill::Stealth]),
        ..CreatureTemplate::resistant_to_nonmagical_physical()
    }
});
