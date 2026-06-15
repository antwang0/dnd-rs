use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{HEAVY_CLUB, LIZARDFOLK_BITE, LIZARDFOLK_MULTI};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Language, Size};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Lizardfolk — CR 1/2 swamp humanoid. Natural-armor AC 15 (the highest
/// of the cr-½ humanoid tier in our pool) plus a multi that fires the
/// natural bite and a heavy club in one Action. No special features
/// beyond the AC + decent HP — the stat block is meant to be a sturdy
/// front-liner that survives a round of single-target focus and rewards
/// AoE / spell pressure to thin a wider group.
pub static LIZARDFOLK_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&LIZARDFOLK_BITE);
    actions.push(&HEAVY_CLUB);
    actions.push(&*LIZARDFOLK_MULTI);
    CreatureTemplate {
        name: "Lizardfolk",
        // 'L' — capital L was free in the medium-humanoid slot.
        glyph: 'L',
        ac: 15,
        hitpoints: "4d8+4".parse().unwrap(),
        speed: 30.,
        strength: 15,
        intelligence: 7,
        dexterity: 10,
        wisdom: 12,
        constitution: 13,
        charisma: 7,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::new(),
        languages: HashSet::from([Language::Draconic]),
        cr: 0.5,
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: false,
        damage_modifiers: HashMap::new(),
        proficient_saves: HashSet::new(),
        condition_immunities: HashSet::new(),
        features: HashSet::new(),
        regen_per_round: 0,
        regen_suppressors: HashSet::new(),
        legendary_resistances: 0,
        has_evasion: false,
        has_uncanny_dodge: false,
        has_deflect_missiles: false,
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
