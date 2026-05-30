use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{
    BALOR_FIRE_AURA, BALOR_LONGSWORD, BALOR_MULTI, BALOR_WHIP,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{
    AbilityScoreType, CreatureType, DamageModifier, DamageType, Language, Size, SpecialSense,
};
use std::collections::{HashMap, HashSet};
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
    actions.push(&*BALOR_WHIP);
    actions.push(&*BALOR_MULTI);
    actions.push(&*BALOR_FIRE_AURA);
    CreatureTemplate {
        name: "Balor",
        // 'X' is unused — chosen for the X-shape of the balor's
        // crossed longsword + whip silhouette on the glyph map.
        glyph: 'X',
        ac: 19,
        // 20d12+140 ≈ 262 average per MM (CR 19 apex demon).
        hitpoints: "20d12+140".parse().unwrap(),
        speed: 40., // walking + flying speed 80ft RAW; we use the larger walking value
        strength: 26,
        intelligence: 20,
        dexterity: 15,
        wisdom: 16,
        constitution: 22,
        charisma: 22,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([SpecialSense::Darkvision(120)]),
        languages: HashSet::from([Language::Abyssal]),
        cr: 19.0,
        size: Size::Huge,
        creature_type: CreatureType::Fiend,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: false,
        damage_modifiers: HashMap::from([
            (DamageType::Fire, DamageModifier::Immunity),
            (DamageType::Poison, DamageModifier::Immunity),
            (DamageType::Cold, DamageModifier::Resistance),
            (DamageType::Lightning, DamageModifier::Resistance),
            (DamageType::Bludgeoning, DamageModifier::Resistance),
            (DamageType::Piercing, DamageModifier::Resistance),
            (DamageType::Slashing, DamageModifier::Resistance),
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
        features: HashSet::new(),
        regen_per_round: 0,
        regen_suppressors: HashSet::new(),
        // Balor: 3/Day Legendary Resistance — boss-tier control immunity.
        legendary_resistances: 3,
        has_evasion: false,
        has_uncanny_dodge: false,
        has_displacement: false,
        has_danger_sense: false,
        has_pack_tactics: false,
        has_magic_resistance: true,
        recharge_abilities: Vec::new(),
        legendary_actions_per_round: 0,
        has_extra_attack: true,
        brutal_critical_dice: 0,
        crit_threshold: 20,
        has_lucky: false,
        has_aura_of_protection: false,
        has_aura_of_courage: false,
        has_savage_attacks: false,
        has_dwarven_resilience: false,
        sorcery_points: 0,
    }
});
