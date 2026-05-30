use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{ANKHEG_BITE, ANKHEG_ACID_SPRAY};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Ankheg — CR 2 monstrosity. Giant burrowing insect that erupts from
/// the ground. Bite deals 2d6+3 slashing + 1d6 acid. Also has an acid
/// spray (3d6 acid, DEX save DC 13, 30ft line). AC 14 (natural armor, 11
/// when prone/burrowed), ~39 HP (6d10+6). Tremorsense 60ft, darkvision
/// 60ft.
pub static ANKHEG_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*ANKHEG_BITE);
    actions.push(&*ANKHEG_ACID_SPRAY);
    CreatureTemplate {
        name: "Ankheg",
        glyph: 'å',
        ac: 14,
        hitpoints: "6d10+6".parse().unwrap(),
        speed: 30.,
        strength: 17,
        intelligence: 1,
        dexterity: 11,
        wisdom: 13,
        constitution: 13,
        charisma: 6,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([
            SpecialSense::Darkvision(60),
            SpecialSense::Tremorsense(60),
        ]),
        languages: HashSet::new(),
        cr: 2.0,
        size: Size::Large,
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
        has_aura_of_protection: false,
        has_aura_of_courage: false,
        has_savage_attacks: false,
        has_dwarven_resilience: false,
        sorcery_points: 0,
    }
});
