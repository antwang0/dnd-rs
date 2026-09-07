use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{
    BALOR_FIRE_AURA, BALOR_LONGSWORD, BALOR_MULTI, BALOR_WHIP,
};
use crate::actors::actor_template::{CreatureTemplate, damage_modifiers_from};
use crate::conditions::Condition;
use crate::engine::types::{
    AbilityScoreType, CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense,
};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Balor — CR 19 demon. Apex of the Abyssal hierarchy: huge winged
/// demon armed with a flaming longsword and a lightning whip. Four
/// attack lanes:
/// - **Longsword**: 3d8+STR slashing + 3d8 lightning rider, reach 2.
/// - **Whip**: 2d6+STR slashing + 3d6 lightning rider, reach 12 (30ft).
/// - **Multiattack** (longsword + whip): the full opening salvo.
/// - **Fire Aura** (bonus action): 3d6 fire to every adjacent enemy
///   (no save) — turns the balor into a walking damage zone.
///
/// Standard demon-lord envelope: immune to fire + poison; resistant to
/// cold + lightning + mundane B/P/S; can't be poisoned / frightened /
/// charmed. Legendary Resistance 3/Day — boss-tier control immunity.
/// Speaks Abyssal + telepathy (we approximate the latter as none — no
/// telepathy modeling in the engine yet).
pub static BALOR_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*BALOR_LONGSWORD);
    actions.push(&BALOR_WHIP);
    actions.push(&*BALOR_MULTI);
    actions.push(&*BALOR_FIRE_AURA);
    CreatureTemplate {
        name: "Balor",
        // 'X' is unused — chosen for the X-shape of the balor's
        // crossed longsword + whip silhouette on the glyph map.
        glyph: 'X',
        ac: 19,
        // 23d12+138 ≈ 262 average per MM (CR 19 apex demon).
        hitpoints: "23d12+138".parse().unwrap(),
        // RAW speed line: Speed 40 ft., fly 80 ft.
        speed: 40.0,
        fly_speed: 80.0,
        strength: 26,
        dexterity: 15,
        constitution: 22,
        intelligence: 20,
        wisdom: 16,
        charisma: 22,
        senses: HashSet::from([SpecialSense::Darkvision(120)]),
        languages: HashSet::from([Language::Abyssal]),
        cr: 19.0,
        size: Size::Huge,
        creature_type: CreatureType::Fiend,
        actions,
        damage_modifiers: damage_modifiers_from([
            (DamageType::Fire, DamageModifier::Immunity),
            (DamageType::Poison, DamageModifier::Immunity),
            (DamageType::Cold, DamageModifier::Resistance),
            (DamageType::Lightning, DamageModifier::Resistance),
        ]),
        // Balor proficient saves: STR / CON / WIS / CHA per MM.
        proficient_saves: HashSet::from([
            AbilityScoreType::Strength,
            AbilityScoreType::Constitution,
            AbilityScoreType::Wisdom,
            AbilityScoreType::Charisma,
        ]),
        condition_immunities: HashSet::from([
            Condition::Poisoned,
            Condition::Frightened,
            Condition::Charmed,
        ]),
        // Balor: 3/Day Legendary Resistance — boss-tier control immunity.
        legendary_resistances: 3,
        has_magic_resistance: true,
        has_extra_attack: true,
        // 5e **Magic Weapons**: "the balor's weapon attacks are magical."
        features: HashSet::from([crate::actions::class_features::MAGICAL_ATTACKS_TAG]),
        ..CreatureTemplate::resistant_to_nonmagical_physical()
    }
});
