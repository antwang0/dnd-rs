use crate::actions::class_features::{SWIM_SPEED_TAG, UNDERWATER_BREATHING_TAG};
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
    actions.push(&STORM_GIANT_LIGHTNING_STRIKE);
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
        // RAW speed line: Speed 50 ft., Fly 25 ft. (hover), Swim 50 ft.
        // The fly is half the walk and hovers — a storm giant does not
        // outrun anything in the air, it simply cannot be reached.
        fly_speed: 25.,
        hovers: true,
        strength: 29,
        intelligence: 16,
        dexterity: 14,
        wisdom: 20,
        constitution: 20,
        charisma: 18,
        languages: HashSet::from([Language::Common, Language::Giant]),
        cr: 13.0,
        size: Size::Huge,
        creature_type: CreatureType::Giant,
        actions,
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
        has_extra_attack: true,
        // RAW swim speed: the tag is what makes `TerrainType::Water`
        // free to cross and lifts the underwater melee penalty.
        features: HashSet::from([SWIM_SPEED_TAG, UNDERWATER_BREATHING_TAG]),
        ..CreatureTemplate::defaults()
    }
});
