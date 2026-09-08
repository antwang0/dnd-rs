use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{GHOST_HORRIFYING_VISAGE, GHOST_WITHERING_TOUCH};
use crate::actors::actor_template::{CreatureTemplate, damage_modifiers_from};
use crate::conditions::Condition;
use crate::engine::types::{CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Ghost — CR 4 incorporeal undead. RAW: a wandering soul with a
/// withering necrotic touch and a **Horrific Visage** — a 60-foot cone
/// of dread that deals psychic damage and sends the living running, and
/// that anyone who holds their nerve against once is done with for
/// good. We model the incorporeal-
/// movement clause via straight resistance to non-magical B/P/S (the
/// 5e formula is "resistance to non-magical weapons, immune to most
/// things else" — we keep parity with the rest of the undead pool
/// here).
///
/// Damage profile: resistance to acid / cold / fire / lightning /
/// thunder + non-magical B/P/S; immunity to necrotic and poison.
/// Condition immunity to charm / fright / exhaustion / grapple /
/// paralysis / petrification / poisoned / prone / restrained — the
/// standard incorporeal lockdown.
pub static GHOST_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*GHOST_WITHERING_TOUCH);
    actions.push(&*GHOST_HORRIFYING_VISAGE);
    CreatureTemplate {
        name: "Ghost",
        // 'g' was free (G is Gargoyle / Goblin / Giant — uppercase).
        glyph: 'g',
        ac: 11,
        // 10d8 = 45 average per MM.
        hitpoints: "10d8".parse().unwrap(),
        // RAW speed line: Speed 5 ft., Fly 40 ft. (hover). The five feet
        // is a shuffle and the forty is the ghost.
        speed: 5.0,
        fly_speed: 40.0,
        hovers: true,
        strength: 7,
        intelligence: 10,
        dexterity: 13,
        wisdom: 12,
        constitution: 10,
        charisma: 17, // drives the Possession DC if ever modeled
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Common]),
        cr: 4.0,
        size: Size::Medium,
        creature_type: CreatureType::Undead,
        actions,
        damage_modifiers: damage_modifiers_from([
            (DamageType::Necrotic, DamageModifier::Immunity),
            (DamageType::Poison, DamageModifier::Immunity),
            (DamageType::Acid, DamageModifier::Resistance),
            (DamageType::Cold, DamageModifier::Resistance),
            (DamageType::Fire, DamageModifier::Resistance),
            (DamageType::Lightning, DamageModifier::Resistance),
            (DamageType::Thunder, DamageModifier::Resistance),
        ]),
        // The 5e ghost condition envelope: immune to a long list because
        // the body is incorporeal and the mind is already dead.
        condition_immunities: HashSet::from([
            Condition::Charmed,
            Condition::Frightened,
            Condition::Exhausted,
            Condition::Grappled,
            Condition::Paralyzed,
            Condition::Petrified,
            Condition::Poisoned,
            Condition::Prone,
            Condition::Restrained,
        ]),
        ..CreatureTemplate::resistant_to_nonmagical_physical()
    }
});
