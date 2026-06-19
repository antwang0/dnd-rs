use crate::actions::class_attacks::ROGUE_SHORTSWORD;
use crate::actions::class_features::{
    CUNNING_DASH, CUNNING_DISENGAGE, CUNNING_HIDE, CUNNING_STRIKE_DAZE, CUNNING_STRIKE_POISON,
    CUNNING_STRIKE_TRIP, CUNNING_STRIKE_WITHDRAW,
};
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{AbilityScoreType, CreatureType, Language, Size};
use std::collections::HashSet;
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
    // 5e 2024 Rogue Cunning Strike (lv5): mirror the rogue chassis.
    actions.push(&*CUNNING_STRIKE_POISON);
    actions.push(&*CUNNING_STRIKE_TRIP);
    actions.push(&*CUNNING_STRIKE_WITHDRAW);
    actions.push(&*CUNNING_STRIKE_DAZE);
    CreatureTemplate {
        name: "Halfling Scout",
        // 'h' — distinct from 'H' (already taken by harpy).
        glyph: 'h',
        ac: 14,
        hitpoints: "3d8+3".parse().unwrap(),
        // Halflings have 25 ft speed per RAW.
        speed: 25.,
        strength: 8,
        dexterity: 16, // primary
        constitution: 12,
        intelligence: 12,
        wisdom: 12,
        charisma: 12,
        languages: HashSet::from([Language::Common, Language::Halfling, Language::ThievesCant]),
        cr: 1.0,
        // 5e Halfling: Small size category.
        size: Size::Small,
        creature_type: CreatureType::Humanoid,
        actions,
        rolls_death_saves: true,
        proficient_saves: HashSet::from([
            AbilityScoreType::Dexterity,
            AbilityScoreType::Intelligence,
        ]),
        has_evasion: true,
        has_uncanny_dodge: true,
        // 5e Halfling racial: Lucky. Reroll nat 1s on attack rolls,
        // ability checks, and saving throws.
        has_lucky: true,
        // 5e Halfling racial: Brave. Advantage on saves vs Frightened —
        // we approximate as immunity to the Frightened install (the
        // `dynamic_immunity_to` lane in actor_template).
        has_brave: true,
        ..CreatureTemplate::defaults()
    }
});
