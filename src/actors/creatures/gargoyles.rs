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
        // SRD 5.2 "Immunities Poison; Exhaustion, Petrified, Poisoned"
        // — carved rock does not tire, and a creature already carved
        // out of rock cannot be turned to stone.
        //
        // Three conditions in the quotation and three in the list. The
        // `Charmed` and `Prone` that used to sit among them came from
        // the construct convention rather than from the line, which
        // this block quoted twice while contradicting it: a gargoyle is
        // an Elemental, and RAW is perfectly happy to charm one or
        // knock it over.
        condition_immunities: HashSet::from([
            Condition::Exhausted,
            Condition::Petrified,
            Condition::Poisoned,
        ]),
        ..CreatureTemplate::resistant_to_nonmagical_physical()
    }
});
