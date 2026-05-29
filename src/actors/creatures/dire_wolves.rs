use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::DIRE_WOLF_BITE;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Dire Wolf — CR 1 large beast. Stronger sibling of the standard
/// wolf: 2d6+3 bite with a DC 13 trip rider. Large footprint means
/// it tanks more space and has the standard "reach 2 vs adjacent"
/// adjacency footprint the engine handles via `get_tiles_from_size`.
pub static DIRE_WOLF_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*DIRE_WOLF_BITE);
    CreatureTemplate {
        name: "Dire Wolf",
        // 'D' — distinct from 'W' (wolf).
        glyph: 'D',
        ac: 14,
        hitpoints: "5d10+10".parse().unwrap(),
        speed: 50.,
        strength: 17,
        intelligence: 3,
        dexterity: 15,
        wisdom: 12,
        constitution: 15,
        charisma: 7,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::new(),
        cr: 1.0,
        size: Size::Large,
        creature_type: CreatureType::Beast,
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
        has_displacement: false,
        has_danger_sense: false,
        has_pack_tactics: true,
        has_magic_resistance: false,
        recharge_abilities: Vec::new(),
        legendary_actions_per_round: 0,
        has_extra_attack: false,
        brutal_critical_dice: 0,
        crit_threshold: 20,
        has_lucky: false,
    }
});
