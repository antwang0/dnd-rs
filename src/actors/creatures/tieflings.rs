use crate::actions::class_features::{INFERNAL_LEGACY_REBUKE, INFERNAL_LEGACY_REBUKE_TAG};
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::DAGGER;
use crate::actions::spells::{
    CHARM_PERSON, CHILL_TOUCH, ELDRITCH_BLAST, FIRE_BOLT, HEX, MAGE_ARMOR, MISTY_STEP, SHIELD,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{
    AbilityScoreType, CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense,
};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Tiefling Warlock — infernal-touched CHA-caster on a Warlock-lite
/// chassis. The two headline racial traits are:
///   - **Hellish Resistance**: resistance to fire damage. Folded into
///     the template's `damage_modifiers` table.
///   - **Infernal Legacy**: once per long rest, the tiefling fires
///     `hellish rebuke` (CHA-based, 3d10 fire, DEX save half) without
///     consuming a spell slot. Modeled via the
///     `INFERNAL_LEGACY_REBUKE_TAG` once-per-rest feature flag and the
///     `InfernalLegacyRebuke` action — mirrors how the Aasimar's
///     `HealingHands` racial action is gated.
///
/// Stat shape targets a level-3 build: AC 12 (unarmored + DEX), 21 HP
/// (3d8+6), CHA 16. The loadout pairs Eldritch Blast as the at-will
/// ranged cantrip with Hex as the per-encounter rider; lv1 slots
/// cover Shield / Mage Armor / Charm Person; a single lv2 slot
/// supports Misty Step. The racial Infernal Legacy gives a slot-free
/// fire burst when the slots run dry.
pub static TIEFLING_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&DAGGER);
    // Cantrips (at-will).
    actions.push(&*ELDRITCH_BLAST);
    actions.push(&*FIRE_BOLT);
    actions.push(&*CHILL_TOUCH);
    // Level 1
    actions.push(&*HEX);
    actions.push(&*SHIELD);
    actions.push(&*MAGE_ARMOR);
    actions.push(&*CHARM_PERSON);
    // Level 2
    actions.push(&*MISTY_STEP);
    // Racial: Infernal Legacy — Hellish Rebuke (no slot cost).
    actions.push(&*INFERNAL_LEGACY_REBUKE);
    CreatureTemplate {
        name: "Tiefling",
        // 'T' — distinct from 't' (Treant), reads as a robed
        // CHA-caster with a horned silhouette.
        glyph: 'T',
        ac: 12, // unarmored + DEX
        hitpoints: "3d8+6".parse().unwrap(),
        speed: 30.,
        strength: 10,
        intelligence: 11,
        dexterity: 14,
        wisdom: 12,
        constitution: 14,
        charisma: 16, // infernal heritage spellcasting ability
        skills: HashSet::new(),
        items: Vec::new(),
        // 5e Tiefling Darkvision: 60 ft.
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Common, Language::Infernal]),
        cr: 2.0,
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
        actions,
        // Warlock-lite Pact Magic slot table — two lv1 + one lv2 to
        // cover the level-3 spread of slot-driven picks. Lower than a
        // full warlock, which makes the racial Infernal Legacy
        // meaningful.
        spell_slots_by_level: vec![2, 1],
        rolls_death_saves: true,
        // 5e Tiefling Hellish Resistance: resistance to fire damage.
        damage_modifiers: HashMap::from([(DamageType::Fire, DamageModifier::Resistance)]),
        proficient_saves: HashSet::from([
            AbilityScoreType::Wisdom,
            AbilityScoreType::Charisma,
        ]),
        condition_immunities: HashSet::new(),
        // Racial Infernal Legacy flag — once-per-rest gated.
        features: HashSet::from([INFERNAL_LEGACY_REBUKE_TAG]),
        regen_per_round: 0,
        regen_suppressors: HashSet::new(),
        legendary_resistances: 0,
        has_evasion: false,
        has_uncanny_dodge: false,
        has_displacement: false,
        has_danger_sense: false,
        has_pack_tactics: false,
        has_magic_resistance: false,
        recharge_abilities: Vec::new(),
        legendary_actions_per_round: 0,
        has_extra_attack: false,
        brutal_critical_dice: 0,
        crit_threshold: 20,
        has_lucky: false,
        has_brave: false,
        has_fey_ancestry: false,
        has_aura_of_protection: false,
        has_aura_of_courage: false,
        has_savage_attacks: false,
        has_dwarven_resilience: false,
        has_gnome_cunning: false,
        draconic_ancestry: None,
        sorcery_points: 0,
    }
});
