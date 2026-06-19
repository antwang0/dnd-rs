use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::WISP_SHOCK;
use crate::actors::actor_template::{CreatureTemplate, non_magical_physical_resistances};
use crate::conditions::Condition;
use crate::engine::types::{CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Will-o-Wisp — CR 2 incorporeal undead. Tiny, fragile (22 HP), but
/// hard to pin: AC 19, immune to lightning / poison, resistant to most
/// damage types and to a slate of incorporeal-flavored conditions
/// (Grappled / Restrained / Prone / Exhaustion). At-will Shock attack
/// dishes 2d8 lightning. The fantasy is "wisp drifts in, zaps, drifts
/// out" — high evasion, low HP, surprising damage type.
pub static WISP_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*WISP_SHOCK);
    CreatureTemplate {
        name: "Will-o-Wisp",
        // 'w' (lowercase) — distinct from 'W' (Wolf) and 'R' (Wraith).
        glyph: 'w',
        ac: 19,
        hitpoints: "9d4".parse().unwrap(),
        speed: 50.,
        strength: 1,
        dexterity: 28, // ridiculous DEX is the wisp's signature
        constitution: 10,
        intelligence: 13,
        wisdom: 14,
        charisma: 11,
        senses: HashSet::from([SpecialSense::Darkvision(120)]),
        languages: HashSet::from([Language::Common]),
        cr: 2.0,
        size: Size::Tiny,
        creature_type: CreatureType::Undead,
        actions,
        damage_modifiers: non_magical_physical_resistances([
            (DamageType::Acid, DamageModifier::Resistance),
            (DamageType::Cold, DamageModifier::Resistance),
            (DamageType::Fire, DamageModifier::Resistance),
            (DamageType::Necrotic, DamageModifier::Resistance),
            (DamageType::Thunder, DamageModifier::Resistance),
            (DamageType::Lightning, DamageModifier::Immunity),
            (DamageType::Poison, DamageModifier::Immunity),
        ]),
        // Standard incorporeal undead suite. Note: not Charm-immune
        // RAW (the wisp can be commanded), but we bundle the Charmed
        // immunity in line with the rest of our undead pool — the
        // engine's command/charm interactions are tuned around that.
        condition_immunities: HashSet::from([
            Condition::Poisoned,
            Condition::Grappled,
            Condition::Restrained,
            Condition::Prone,
            Condition::Unconscious,
            Condition::Charmed,
        ]),
        ..CreatureTemplate::defaults()
    }
});
