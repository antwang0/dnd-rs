use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{VAMPIRE_CHARMING_GAZE, VAMPIRE_MULTIATTACK};
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{
    AbilityScoreType, CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense,
};
use std::collections::{HashMap, HashSet};
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
        speed: 30.,
        strength: 18,
        intelligence: 17,
        dexterity: 18,
        wisdom: 15,
        constitution: 18,
        charisma: 18, // spell DC / charm DC
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([SpecialSense::Darkvision(120)]),
        languages: HashSet::from([Language::Common]),
        cr: 13.0,
        size: Size::Medium,
        creature_type: CreatureType::Undead,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: false,
        // 5e MM Vampire: resistant to necrotic + non-magical physical,
        // immune to poison. We model radiant as a damage type the regen
        // can't suppress (covered via `regen_suppressors` below) but
        // don't add full vulnerability — the regen mechanic is the
        // primary "radiant beats vampires" knob.
        damage_modifiers: HashMap::from([
            (DamageType::Necrotic, DamageModifier::Resistance),
            (DamageType::Bludgeoning, DamageModifier::Resistance),
            (DamageType::Piercing, DamageModifier::Resistance),
            (DamageType::Slashing, DamageModifier::Resistance),
            (DamageType::Poison, DamageModifier::Immunity),
        ]),
        // Vampire saves: prof in DEX / WIS / CHA per MM (the "saving
        // throws +6 / +6 / +6" line).
        proficient_saves: HashSet::from([
            AbilityScoreType::Dexterity,
            AbilityScoreType::Wisdom,
            AbilityScoreType::Charisma,
        ]),
        condition_immunities: HashSet::from([Condition::Poisoned]),
        features: HashSet::new(),
        // Regenerate 20 HP at end of round while combat-active. Radiant
        // damage suppresses for the round (proxy for 5e's "sunlight /
        // holy water" downside — radiant is the carrier for both).
        regen_per_round: 20,
        regen_suppressors: HashSet::from([DamageType::Radiant]),
        legendary_resistances: 0,
        has_evasion: false,
        has_uncanny_dodge: false,
        has_displacement: false,
        has_danger_sense: false,
        has_pack_tactics: false,
        has_magic_resistance: true,
        recharge_abilities: Vec::new(),
        legendary_actions_per_round: 3,
        has_extra_attack: true,
        brutal_critical_dice: 0,
        crit_threshold: 20,
        has_lucky: false,
        has_aura_of_protection: false,
        has_aura_of_courage: false,
        has_savage_attacks: false,
        has_dwarven_resilience: false,
    }
});
