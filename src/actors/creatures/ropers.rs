use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::BITE;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Roper — CR 5 monstrosity. Cave-dwelling ambush predator disguised as
/// a stalagmite. Extremely tough (AC 20) but very slow (10ft speed).
/// Attacks with a powerful bite. Its signature tendril-grapple and reel
/// abilities are not yet modelled — just the bite and the rock-hard shell.
pub static ROPER_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*BITE);
    CreatureTemplate {
        name: "Roper",
        glyph: 'r',
        ac: 20,
        hitpoints: "11d10+44".parse().unwrap(),
        speed: 10.,
        strength: 18,
        intelligence: 7,
        dexterity: 8,
        wisdom: 16,
        constitution: 18,
        charisma: 6,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::new(),
        cr: 5.0,
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
        has_displacement: false,
    }
});
