use crate::actions::class_attacks::ROGUE_SHORTSWORD;
use crate::actions::class_features::{CUNNING_DASH, CUNNING_DISENGAGE, CUNNING_HIDE};
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{AbilityScoreType, CreatureType, Language, Size};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Halfling Scout — a Small-size DEX-primary skirmisher built on the
/// Rogue chassis. The headline feature is the racial **Lucky** trait:
/// whenever the scout rolls a natural 1 on an attack roll, ability
/// check, or saving throw, the d20 is re-rolled and the new value is
/// used. Read at every d20 site via `EncounterInstance::roll_d20_lucky`.
///
/// Mechanically identical to the baseline Rogue (Sneak Attack, Cunning
/// Action trio, Evasion, Uncanny Dodge) but at a smaller body. Speed is
/// 25 (RAW: 5ft slower than Medium humanoids) and size is Small. Stats
/// target a level-3 build: 21 HP (3d8+3), AC 14 (leather + DEX +2).
pub static HALFLING_SCOUT_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*ROGUE_SHORTSWORD);
    actions.push(&*CUNNING_DASH);
    actions.push(&*CUNNING_DISENGAGE);
    actions.push(&*CUNNING_HIDE);
    CreatureTemplate {
        name: "Halfling Scout",
        // 'h' — distinct from 'H' (already taken by harpy).
        glyph: 'h',
        ac: 14,
        hitpoints: "3d8+3".parse().unwrap(),
        // Halflings have 25 ft speed per RAW.
        speed: 25.,
        strength: 8,
        intelligence: 12,
        dexterity: 16, // primary
        wisdom: 12,
        constitution: 12,
        charisma: 12,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::new(),
        languages: HashSet::from([Language::Common, Language::Halfling, Language::ThievesCant]),
        cr: 1.0,
        // 5e Halfling: Small size category.
        size: Size::Small,
        creature_type: CreatureType::Humanoid,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: true,
        damage_modifiers: HashMap::new(),
        proficient_saves: HashSet::from([
            AbilityScoreType::Dexterity,
            AbilityScoreType::Intelligence,
        ]),
        condition_immunities: HashSet::new(),
        features: HashSet::new(),
        regen_per_round: 0,
        regen_suppressors: HashSet::new(),
        legendary_resistances: 0,
        has_evasion: true,
        has_uncanny_dodge: true,
        has_displacement: false,
        has_danger_sense: false,
        has_pack_tactics: false,
        has_magic_resistance: false,
        recharge_abilities: Vec::new(),
        legendary_actions_per_round: 0,
        has_extra_attack: false,
        brutal_critical_dice: 0,
        crit_threshold: 20,
        // 5e Halfling racial: Lucky. Reroll nat 1s on attack rolls,
        // ability checks, and saving throws.
        has_lucky: true,
        // 5e Halfling racial: Brave. Advantage on saves vs Frightened —
        // we approximate as immunity to the Frightened install (the
        // `dynamic_immunity_to` lane in actor_template).
        has_brave: true,
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
