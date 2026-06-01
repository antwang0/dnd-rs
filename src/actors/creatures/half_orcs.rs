use crate::actions::class_features::{
    ACTION_SURGE, ACTION_SURGE_TAG, RELENTLESS_ENDURANCE_TAG, SECOND_WIND, SECOND_WIND_TAG,
};
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::GREATAXE;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{AbilityScoreType, CreatureType, Language, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Half-Orc Marauder — orcish-blood STR-primary fighter chassis. The
/// headline racial trait is the pair **Relentless Endurance + Savage
/// Attacks**:
///   - Relentless Endurance: once per long rest, damage that would
///     drop the holder to 0 HP drops them to 1 HP instead. Hooked into
///     `ActorInstance::take_damage` next to the Death Ward intercept.
///   - Savage Attacks: a critical melee weapon hit rolls one extra
///     weapon damage die. Hooked into `engine::attack`'s brutal-critical
///     lane so the bonus die stacks with Brutal Critical (a half-orc
///     barbarian gets both).
///
/// Stat shape targets a level-3 build: AC 16 (chain mail), 28 HP
/// (3d10+9), STR 17, greataxe as the signature 1d12 swing. Fighter
/// class chassis (Second Wind + Action Surge) so the half-orc plays as
/// a high-burst melee striker — pop Action Surge on a Savage-Attacks
/// crit and the round-one swing can one-shot mid-CR enemies.
pub static HALF_ORC_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*GREATAXE);
    actions.push(&*SECOND_WIND);
    actions.push(&*ACTION_SURGE);
    CreatureTemplate {
        name: "Half-Orc Marauder",
        // 'H' — distinct from 'h' (Halfling Scout) and 'h' lowercase.
        glyph: 'H',
        ac: 16,
        hitpoints: "3d10+9".parse().unwrap(),
        speed: 30.,
        strength: 17,
        intelligence: 9,
        dexterity: 12,
        wisdom: 11,
        constitution: 16,
        charisma: 10,
        skills: HashSet::new(),
        items: Vec::new(),
        // 5e Half-Orc Darkvision: see in dim light out to 60 ft as if it
        // were bright light.
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Common, Language::Orc]),
        cr: 2.0,
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
        // Racial Relentless Endurance + fighter Second Wind / Action Surge.
        features: HashSet::from([
            RELENTLESS_ENDURANCE_TAG,
            SECOND_WIND_TAG,
            ACTION_SURGE_TAG,
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
        crit_threshold: 20,
        has_lucky: false,
        has_brave: false,
        has_aura_of_protection: false,
        has_aura_of_courage: false,
        // 5e Half-Orc racial: Savage Attacks — +1 weapon damage die on a
        // critical melee hit.
        has_savage_attacks: true,
        has_dwarven_resilience: false,
        has_gnome_cunning: false,
        draconic_ancestry: None,
        sorcery_points: 0,
    }
});
