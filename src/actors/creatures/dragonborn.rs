use crate::actions::class_features::{
    ACTION_SURGE, ACTION_SURGE_TAG, BREATH_WEAPON, BREATH_WEAPON_TAG, SECOND_WIND, SECOND_WIND_TAG,
};
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::GREATSWORD;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{
    AbilityScoreType, CreatureType, DamageModifier, DamageType, Language, Size,
};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Red Dragonborn Champion — STR-primary fighter on a Champion chassis
/// (Improved Critical via `crit_threshold = 19`). The defining racial
/// traits are the **Draconic Ancestry** pair:
///   - **Damage Resistance**: resistance to the ancestor's damage type
///     (fire for Red Dragonborn). Folded into `damage_modifiers`.
///   - **Breath Weapon**: once per short rest, exhale a 15-ft cone of
///     the ancestor's damage type (DEX save half, scales with level).
///     Gated by the `BREATH_WEAPON_TAG` feature flag and the
///     `BreathWeapon` action — refreshes on short rest via the
///     `SHORT_REST_FEATURES` registry, mirroring Second Wind / Action
///     Surge / Arcane Recovery's short-rest cadence.
///
/// Stat shape targets a level-3 Champion fighter: AC 16 (chain mail),
/// 28 HP (3d10+9), STR 17, greatsword as the signature 2d6 slashing
/// swing. Fighter class chassis (Second Wind + Action Surge) plus the
/// Champion's Improved Critical (crit on 19-20) for the spike-damage
/// niche. The Breath Weapon adds a slot-free AoE on the round where
/// the dragonborn opens against multiple targets.
pub static DRAGONBORN_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&GREATSWORD);
    actions.push(&*SECOND_WIND);
    actions.push(&*ACTION_SURGE);
    // Racial: Breath Weapon — once per short rest.
    actions.push(&*BREATH_WEAPON);
    CreatureTemplate {
        name: "Red Dragonborn Champion",
        // 'D' overlaps with the Mountain Dwarf glyph — both reasonable
        // picks but the Red Dragonborn is a more recognizable iconic.
        // We use 'Δ' as a flavorful sigil that doesn't collide with
        // any existing humanoid glyph.
        glyph: '\u{0394}',
        ac: 16,
        hitpoints: "3d10+9".parse().unwrap(),
        speed: 30.,
        strength: 17,
        intelligence: 10,
        dexterity: 12,
        wisdom: 11,
        constitution: 16,
        charisma: 14, // mild CHA bump per Dragonborn racial
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::new(),
        languages: HashSet::from([Language::Common, Language::Draconic]),
        cr: 2.0,
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: true,
        // 5e Red Dragonborn Damage Resistance: fire.
        damage_modifiers: HashMap::from([(DamageType::Fire, DamageModifier::Resistance)]),
        proficient_saves: HashSet::from([
            AbilityScoreType::Strength,
            AbilityScoreType::Constitution,
        ]),
        condition_immunities: HashSet::new(),
        // Fighter class features + the racial Breath Weapon flag.
        features: HashSet::from([
            SECOND_WIND_TAG,
            ACTION_SURGE_TAG,
            BREATH_WEAPON_TAG,
        ]),
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
        // Champion Improved Critical: crits trigger on 19 or 20.
        crit_threshold: 19,
        has_lucky: false,
        has_brave: false,
        has_aura_of_protection: false,
        has_aura_of_courage: false,
        has_savage_attacks: false,
        has_dwarven_resilience: false,
        has_gnome_cunning: false,
        // 5e Draconic Ancestry: Red — fire. Drives the breath weapon's
        // damage type via the `draconic_ancestry()` accessor.
        draconic_ancestry: Some(DamageType::Fire),
        sorcery_points: 0,
    }
});
