use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{
    STORM_GIANT_GREATSWORD, STORM_GIANT_LIGHTNING_STRIKE, STORM_GIANT_ROCK,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{AbilityScoreType, CreatureType, DamageModifier, DamageType, Language, Size};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Storm Giant — CR 13 giant. The apex of the giant ladder: bigger
/// HP than the Frost Giant, a heavier greatsword (6d6 slashing + STR),
/// a thrown rock for stand-off (4d12 bludgeoning), AND a bonus-action
/// lightning strike (8d10 DEX-save half) that lets the giant trade
/// blow for blow even when the front line is buttoned up. Immune to
/// lightning and thunder (the storm's elements), resistant to cold.
///
/// Proficient saves: STR / CON / WIS / CHA per MM. Speaks Common and
/// Giant — Storm Giants are mythic and articulate, the rare giant
/// kin who don't smash first and ask questions never.
pub static STORM_GIANT_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&STORM_GIANT_GREATSWORD);
    actions.push(&STORM_GIANT_ROCK);
    actions.push(&*STORM_GIANT_LIGHTNING_STRIKE);
    CreatureTemplate {
        name: "Storm Giant",
        // 'L' is unused in the creature pool — chosen here because the
        // storm giant's signature ability is a Lightning bolt; "L for
        // Lightning" keeps the map glyph mnemonic.
        glyph: 'L',
        ac: 16,
        // 20d12+100 ≈ 230 average per MM (CR 13).
        hitpoints: "20d12+100".parse().unwrap(),
        speed: 50.,
        strength: 29,
        intelligence: 16,
        dexterity: 14,
        wisdom: 18,
        constitution: 20,
        charisma: 18,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::new(),
        languages: HashSet::from([Language::Common, Language::Giant]),
        cr: 13.0,
        size: Size::Huge,
        creature_type: CreatureType::Giant,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: false,
        // Storm Giants are immune to lightning + thunder; resistant to
        // cold. The lightning immunity in particular makes them the
        // natural counter-pick to a Lightning Bolt build.
        damage_modifiers: HashMap::from([
            (DamageType::Lightning, DamageModifier::Immunity),
            (DamageType::Thunder, DamageModifier::Immunity),
            (DamageType::Cold, DamageModifier::Resistance),
        ]),
        // Storm Giant proficient saves: STR, CON, WIS, CHA per MM.
        proficient_saves: HashSet::from([
            AbilityScoreType::Strength,
            AbilityScoreType::Constitution,
            AbilityScoreType::Wisdom,
            AbilityScoreType::Charisma,
        ]),
        condition_immunities: HashSet::new(),
        features: HashSet::new(),
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
        has_extra_attack: true,
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
