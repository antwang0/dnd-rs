use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{LONGBOW, WIGHT_LIFE_DRAIN};
use crate::actors::actor_template::{CreatureTemplate, damage_modifiers_from};
use crate::conditions::Condition;
use crate::engine::types::{CreatureType, DamageModifier, DamageType, Language, Size, Skill, SpecialSense};
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
        // 11d8+33 = 45 average per MM.
        hitpoints: "11d8+33".parse().unwrap(),
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
        // SRD 5.2 "Resistances Necrotic" and "Immunities Poison" — the
        // necrotic half is a resistance, not the immunity this carried.
        // Which matters: necrotic is what a warlock's Eldritch Blast
        // build and a Death Cleric aim at undead, and an immune wight
        // turned all of it off.
        damage_modifiers: damage_modifiers_from([
            (DamageType::Necrotic, DamageModifier::Resistance),
            (DamageType::Poison, DamageModifier::Immunity),
        ]),
        // None. SRD 5.2 prints the wight's six columns as bare
        // modifiers; the CON proficiency here was 2014's.
        proficient_saves: HashSet::new(),
        // SRD 5.2 "Immunities Poison; Exhaustion, Poisoned" — two
        // conditions, and the two the list carries. The `Charmed` and
        // `Frightened` that used to sit beside them came from the 2014
        // undead convention rather than from the line quoted above
        // them, and they were the two that mattered most: a wight that
        // cannot be frightened is a wight a Turn Undead does nothing
        // to, which is the one answer a cleric is carrying for it.
        condition_immunities: HashSet::from([
            Condition::Exhausted,
            Condition::Poisoned,
        ]),
        skills: HashSet::from([Skill::Perception, Skill::Stealth]),
        ..CreatureTemplate::resistant_to_nonmagical_physical()
    }
});
