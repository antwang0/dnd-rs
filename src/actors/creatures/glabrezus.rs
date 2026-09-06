use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{GLABREZU_FIST, GLABREZU_MULTI, GLABREZU_PINCER};
use crate::actors::actor_template::{CreatureTemplate, damage_modifiers_from};
use crate::conditions::Condition;
use crate::engine::types::{
    AbilityScoreType, CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense,
};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Glabrezu — CR 9 demon. A four-armed ape-faced fiend that slots
/// between the Bone Devil (CR 9 lawful evil) and the Balor (CR 19 apex)
/// on the demon ladder. Signature multiattack: 2 pincers (2d10) + 2
/// fists (2d4) — four swings per Action. Standard demon damage envelope:
/// immune to poison; resistant to cold + fire + lightning + mundane
/// B/P/S; immune to Charmed / Frightened / Poisoned. No legendary
/// resistance (RAW: glabrezus aren't "legendary" — that's reserved for
/// CR 11+ demon princes in our pool).
pub static GLABREZU_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&GLABREZU_PINCER);
    actions.push(&GLABREZU_FIST);
    actions.push(&*GLABREZU_MULTI);
    CreatureTemplate {
        name: "Glabrezu",
        // 'Z' was free — uppercase letter to mark a CR-9 boss. (Z is
        // distinct from 'g' = Ghost / Goblin and 'G' = Gargoyle.)
        glyph: 'Z',
        ac: 17,
        // 18d10+90 = 189 average per MM (CR 9 demon HP envelope).
        hitpoints: "18d10+90".parse().unwrap(),
        speed: 40.,
        strength: 20,
        dexterity: 15,
        constitution: 21,
        intelligence: 19,
        wisdom: 17,
        charisma: 16,
        senses: HashSet::from([SpecialSense::Truesight(120)]),
        languages: HashSet::from([Language::Abyssal]),
        cr: 9.0,
        size: Size::Large,
        creature_type: CreatureType::Fiend,
        actions,
        // Standard demon envelope: immune to poison; resistant to
        // cold / fire / lightning + mundane B/P/S. Same shape as the
        // Bone Devil / Balor pool — radiant / force land cleanly.
        damage_modifiers: damage_modifiers_from([
            (DamageType::Poison, DamageModifier::Immunity),
            (DamageType::Cold, DamageModifier::Resistance),
            (DamageType::Fire, DamageModifier::Resistance),
            (DamageType::Lightning, DamageModifier::Resistance),
        ]),
        // Glabrezu proficient saves: STR / CON / WIS / CHA per MM.
        proficient_saves: HashSet::from([
            AbilityScoreType::Strength,
            AbilityScoreType::Constitution,
            AbilityScoreType::Wisdom,
            AbilityScoreType::Charisma,
        ]),
        // Demon condition envelope: Poisoned / Charmed / Frightened
        // immunity — same as the rest of the demon pool.
        condition_immunities: HashSet::from([
            Condition::Poisoned,
            Condition::Charmed,
            Condition::Frightened,
        ]),
        has_magic_resistance: true,
        ..CreatureTemplate::resistant_to_nonmagical_physical()
    }
});
