use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{GARGOYLE_CLAWS, GARGOYLE_MULTI};
use crate::actors::actor_template::{CreatureTemplate, damage_modifiers_from};
use crate::conditions::Condition;
use crate::engine::types::{CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Gargoyle — CR 2 elemental construct. Looks like a statue (False
/// Appearance, not modeled) and is resistant to non-magical physical
/// damage — we lift the "non-magical" qualifier and just give it
/// resistance to the physical trio so any weapon-wielding party feels
/// the chip. Immune to the usual elemental/construct suite: Poisoned,
/// Charmed, Exhaustion, Petrified, Sleep/Unconscious from
/// magical sleep. We model what we can with the existing Condition
/// enum: Poisoned + Charmed + Prone (it flies) immunities.
pub static GARGOYLE_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&GARGOYLE_CLAWS);
    actions.push(&*GARGOYLE_MULTI);
    CreatureTemplate {
        name: "Gargoyle",
        glyph: 'G',
        ac: 15,
        hitpoints: "9d8+27".parse().unwrap(),
        // RAW speed line: Speed 30 ft., fly 60 ft.
        speed: 30.0,
        fly_speed: 60.0,
        strength: 15,
        dexterity: 11,
        constitution: 16,
        intelligence: 6,
        wisdom: 11,
        charisma: 7,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Common]),
        cr: 2.0,
        size: Size::Medium,
        creature_type: CreatureType::Elemental,
        actions,
        // 5e MM: BPS resistance from non-magical attacks + poison immunity.
        damage_modifiers: damage_modifiers_from([
            (DamageType::Poison, DamageModifier::Immunity),
        ]),
        condition_immunities: HashSet::from([
            // SRD 5.2 "Immunities Poison; Exhaustion, Petrified, Poisoned".
            // Carved rock does not tire — and the docstring above this
            // block used to call exhaustion unmodelled, which it has not
            // been for some time.
            Condition::Exhausted,
            Condition::Poisoned,
            Condition::Charmed,
            // SRD 5.2 "Immunities Poison; Exhaustion, Petrified,
            // Poisoned" — a creature that is already carved out of rock
            // cannot be turned to stone.
            Condition::Petrified,
            // Constructs ignore prone (they're not biological).
            Condition::Prone,
        ]),
        ..CreatureTemplate::resistant_to_nonmagical_physical()
    }
});
