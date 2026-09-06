use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::LIFE_DRAIN;
use crate::actors::actor_template::{
    CreatureTemplate, INCORPOREAL_UNDEAD_CONDITION_IMMUNITIES, damage_modifiers_from,
};
use crate::engine::lighting::SunlightFrailty;
use crate::engine::types::{CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Specter — CR 1 incorporeal undead. The wraith's weaker cousin: same
/// life-drain attack shape (necrotic damage + max-HP reduction on a
/// failed CON save), but lower HP, lower AC, and a smaller damage die.
/// Shares the wraith's resistance suite and undead condition immunities.
pub static SPECTER_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*LIFE_DRAIN);
    CreatureTemplate {
        name: "Specter",
        // 'S' is reserved for spider. Use 'P' (sPecter) — also free.
        glyph: 'P',
        ac: 12,
        hitpoints: "5d8".parse().unwrap(),
        // RAW speed line: Speed 30 ft., Fly 50 ft. (hover). SRD 5.2 gives
        // the specter a real walking speed where the older printing gave
        // it none.
        speed: 30.0,
        fly_speed: 50.0,
        hovers: true,
        strength: 1,
        intelligence: 10,
        dexterity: 14,
        wisdom: 10,
        constitution: 11,
        charisma: 11,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Common]),
        cr: 1.0,
        size: Size::Medium,
        creature_type: CreatureType::Undead,
        actions,
        // 5e Specter **Sunlight Sensitivity**: "while in sunlight, the
        // specter has disadvantage on attack rolls, as well as on
        // Wisdom (Perception) checks that rely on sight." The kobold's
        // tier, not the shadow's: the specter's saves are untouched.
        // The Perception half is dropped throughout — the engine rolls
        // no Perception checks.
        sunlight_frailty: Some(SunlightFrailty::Sensitivity),
        damage_modifiers: damage_modifiers_from([
            (DamageType::Acid, DamageModifier::Resistance),
            (DamageType::Cold, DamageModifier::Resistance),
            (DamageType::Fire, DamageModifier::Resistance),
            (DamageType::Lightning, DamageModifier::Resistance),
            (DamageType::Thunder, DamageModifier::Resistance),
            (DamageType::Necrotic, DamageModifier::Immunity),
            (DamageType::Poison, DamageModifier::Immunity),
        ]),
        // SRD 5.2 "Immunities Necrotic, Poison; Charmed, Exhaustion,
        // Grappled, Paralyzed, Petrified, Poisoned, Prone, Restrained,
        // Unconscious" — the incorporeal-undead envelope, and this is
        // one of the four stat blocks whose name is in its docstring.
        // It had been open-coded here and had drifted: Paralyzed and Petrified were missing.
        condition_immunities: INCORPOREAL_UNDEAD_CONDITION_IMMUNITIES.clone(),
        ..CreatureTemplate::resistant_to_nonmagical_physical()
    }
});
