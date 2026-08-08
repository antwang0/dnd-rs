use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{VAMPIRE_CHARMING_GAZE, VAMPIRE_MULTIATTACK};
use crate::actors::actor_template::{CreatureTemplate, non_magical_physical_resistances};
use crate::engine::lighting::SunlightFrailty;
use crate::conditions::Condition;
use crate::engine::types::{
    AbilityScoreType, CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense,
};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Vampire — CR 13 boss undead. The full Vampire (Lord) writeup: stacks
/// the Vampire Spawn's lifesteal bite into a double-tap multiattack and
/// adds a charming gaze to lock down an ally before the bite-train rolls
/// in. Regenerates 20 HP at end of round (suppressed by radiant — our
/// proxy for the "sunlight / running water" weakness). Standard undead
/// immunity envelope (poison / charm) plus vulnerable-to-radiant on top.
///
/// The regen + multiattack + charm package is the marquee boss-tier
/// pattern: chip damage is wasted against the regen, so the party has
/// to bring radiant burst or stall through the charm to actually drop
/// the vampire below zero.
pub static VAMPIRE_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*VAMPIRE_CHARMING_GAZE);
    actions.push(&*VAMPIRE_MULTIATTACK);
    CreatureTemplate {
        name: "Vampire",
        // 'v' (lowercase) to avoid clashing with 'V' (Vampire Spawn).
        glyph: 'v',
        ac: 16,
        // 17d8+68 ≈ 144 average per the MM Vampire stat block.
        hitpoints: "17d8+68".parse().unwrap(),
        strength: 18,
        dexterity: 18,
        constitution: 18,
        intelligence: 17,
        wisdom: 15,
        charisma: 18, // spell DC / charm DC
        senses: HashSet::from([SpecialSense::Darkvision(120)]),
        languages: HashSet::from([Language::Common]),
        cr: 13.0,
        size: Size::Medium,
        creature_type: CreatureType::Undead,
        actions,
        // 5e MM Vampire: resistant to necrotic + non-magical BPS,
        // immune to poison. Radiant is the regen-suppressor (proxy
        // for the "sunlight / holy water" RAW downside).
        damage_modifiers: non_magical_physical_resistances([
            (DamageType::Necrotic, DamageModifier::Resistance),
            (DamageType::Poison, DamageModifier::Immunity),
        ]),
        // Vampire saves: prof in DEX / WIS / CHA per MM.
        proficient_saves: HashSet::from([
            AbilityScoreType::Dexterity,
            AbilityScoreType::Wisdom,
            AbilityScoreType::Charisma,
        ]),
        condition_immunities: HashSet::from([Condition::Poisoned]),
        // Regenerate 20 HP at end of round while combat-active. Radiant
        // damage suppresses for the round (proxy for 5e's "sunlight /
        // holy water" downside — radiant is the carrier for both).
        regen_per_round: 20,
        regen_suppressors: HashSet::from([DamageType::Radiant]),
        has_magic_resistance: true,
        legendary_actions_per_round: 3,
        has_extra_attack: true,
        // 5e Vampire **Sunlight Hypersensitivity**: "the vampire takes
        // 20 radiant damage when it starts its turn in sunlight. While
        // in sunlight, it has disadvantage on attack rolls and ability
        // checks." The 20 radiant lands at the top of the turn through
        // `apply_sunlight_hypersensitivity`, and it lands on a creature
        // that is already vulnerable to radiant — 40 a round, which is
        // exactly the point of the trait and the reason a vampire fight
        // happens at night.
        sunlight_frailty: Some(SunlightFrailty::Hypersensitivity),
        ..CreatureTemplate::defaults()
    }
});
