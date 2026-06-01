use crate::actions::class_features::{
    ACTION_SURGE, ACTION_SURGE_TAG, SECOND_WIND, SECOND_WIND_TAG,
};
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::WARHAMMER;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{AbilityScoreType, CreatureType, Language, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Mountain Dwarf Defender — CON-tank fighter chassis. The defining
/// racial trait is **Dwarven Resilience**: advantage on saving throws
/// against poison AND resistance to poison damage. Hooked into
/// `compute_save_mode` (advantage on CON saves, which the engine treats
/// as the poison-save proxy since saves aren't tagged by source type)
/// and `effective_damage` (poison resistance, folded into the template-
/// resistance lane next to `DamageModifier::Resistance`).
///
/// Stat shape targets a level-3 mountain dwarf fighter: AC 18 (chain
/// mail + shield approximation baked into the flat 18), 33 HP (3d10+12
/// — CON 16 + the Dwarven Toughness +1/level baked into the dice
/// expression), STR 16, warhammer as the signature 1d8 bludgeoning
/// swing. Fighter chassis (Second Wind + Action Surge) gives the dwarf
/// a reliable second swing and a self-heal — pairs with the high CON
/// to push them well past the half-orc / fighter baseline survivability.
///
/// 5e Dwarven Toughness (RAW: +1 HP per character level) is folded into
/// the HP roll (3d10+12 versus a non-dwarf fighter's 3d10+9 — the +3
/// represents 1 HP per level over the fighter's standard +3 CON mod).
pub static DWARF_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&WARHAMMER);
    actions.push(&*SECOND_WIND);
    actions.push(&*ACTION_SURGE);
    CreatureTemplate {
        name: "Mountain Dwarf Defender",
        // 'D' — distinct from 'd' (drow, druid). Mountain dwarf reads as
        // a sturdy upright capital.
        glyph: 'D',
        ac: 18,
        // 3d10 (fighter hit die) + 12 (CON 16 = +3 × 3 levels) + 3
        // (Dwarven Toughness, +1 HP per level over 3 levels). Folded
        // into the dice expression as +12 since the engine doesn't yet
        // model per-level HP bumps separately.
        hitpoints: "3d10+12".parse().unwrap(),
        // 5e Dwarf: 25 ft speed, unmodified by armor (the "speed not
        // reduced by heavy armor" RAW clause is the dwarven trait).
        // We don't model armor speed penalties, so the 25 ft cap is the
        // load-bearing part.
        speed: 25.,
        strength: 16,
        intelligence: 10,
        dexterity: 11,
        wisdom: 12,
        constitution: 16,
        charisma: 10,
        skills: HashSet::new(),
        items: Vec::new(),
        // 5e Dwarf: Darkvision out to 60 ft.
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Common, Language::Dwarvish]),
        cr: 3.0,
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: true,
        damage_modifiers: HashMap::new(),
        proficient_saves: HashSet::from([
            AbilityScoreType::Strength,
            AbilityScoreType::Constitution,
        ]),
        condition_immunities: HashSet::new(),
        features: HashSet::from([SECOND_WIND_TAG, ACTION_SURGE_TAG]),
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
        has_aura_of_protection: false,
        has_aura_of_courage: false,
        has_savage_attacks: false,
        // 5e Dwarven Resilience: advantage on saves vs poison AND
        // resistance to poison damage. The single flag drives both halves.
        has_dwarven_resilience: true,
        has_gnome_cunning: false,
        draconic_ancestry: None,
        sorcery_points: 0,
    }
});
