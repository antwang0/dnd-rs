use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::BASILISK_BITE;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Basilisk — CR 3 monstrosity. Eight-legged reptile with a petrifying
/// gaze. Bite deals 2d6+3 piercing plus a CON save (DC 12) for Petrified
/// (1 round). AC 15, ~52 HP (8d8+16). Slow (speed 20ft) but durable,
/// with darkvision 60ft.
pub static BASILISK_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*BASILISK_BITE);
    CreatureTemplate {
        name: "Basilisk",
        glyph: 'ß',
        ac: 15,
        hitpoints: "8d8+16".parse().unwrap(),
        speed: 20.,
        strength: 16,
        intelligence: 2,
        dexterity: 8,
        wisdom: 8,
        constitution: 15,
        charisma: 7,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::new(),
        cr: 3.0,
        size: Size::Medium,
        creature_type: CreatureType::Monstrosity,
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
        has_pack_tactics: false,
        has_magic_resistance: false,
        recharge_abilities: Vec::new(),
        legendary_actions_per_round: 0,
        has_extra_attack: false,
        brutal_critical_dice: 0,
        crit_threshold: 20,
        has_lucky: false,
    }
});
