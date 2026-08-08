use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::LIFE_DRAIN;
use crate::actors::actor_template::{CreatureTemplate, damage_modifiers_from};
use crate::conditions::Condition;
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
        speed: 50.,
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
        damage_modifiers: damage_modifiers_from([
            (DamageType::Acid, DamageModifier::Resistance),
            (DamageType::Cold, DamageModifier::Resistance),
            (DamageType::Fire, DamageModifier::Resistance),
            (DamageType::Lightning, DamageModifier::Resistance),
            (DamageType::Thunder, DamageModifier::Resistance),
            (DamageType::Necrotic, DamageModifier::Immunity),
            (DamageType::Poison, DamageModifier::Immunity),
        ]),
        condition_immunities: HashSet::from([
            Condition::Poisoned,
            Condition::Charmed,
            Condition::Grappled,
            Condition::Restrained,
            Condition::Prone,
            Condition::Unconscious,
        ]),
        ..CreatureTemplate::resistant_to_nonmagical_physical()
    }
});
