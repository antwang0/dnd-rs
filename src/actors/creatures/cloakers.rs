use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::CLOAKER_TAIL;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Cloaker — CR 8 aberration. Ray-like creature that wraps around its
/// prey. Attacks with a barbed tail at 10ft reach. Its signature
/// envelop/attach ability is not yet modelled. Ground speed 10ft (flying
/// speed 40ft not tracked). Darkvision 60ft.
pub static CLOAKER_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&CLOAKER_TAIL);
    CreatureTemplate {
        name: "Cloaker",
        glyph: 'c',
        ac: 14,
        hitpoints: "12d10+36".parse().unwrap(),
        speed: 10.,
        strength: 17,
        intelligence: 11,
        dexterity: 15,
        wisdom: 12,
        constitution: 16,
        charisma: 14,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::new(),
        cr: 8.0,
        size: Size::Large,
        creature_type: CreatureType::Aberration,
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
        has_displacement: false,
    }
});
