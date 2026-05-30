use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::UMBER_CLAW;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Umber Hulk — CR 5 monstrosity. Heavily armoured burrowing predator
/// with massive claws and confusing gaze (gaze not yet modelled).
/// High AC (18) from its thick carapace, strong STR-based claw attacks.
/// Darkvision 120ft and Tremorsense 60ft make it a subterranean ambusher.
pub static UMBER_HULK_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&UMBER_CLAW);
    CreatureTemplate {
        name: "Umber Hulk",
        glyph: 'U',
        ac: 18,
        hitpoints: "12d10+48".parse().unwrap(),
        speed: 30.,
        strength: 20,
        intelligence: 9,
        dexterity: 13,
        wisdom: 10,
        constitution: 18,
        charisma: 10,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([
            SpecialSense::Darkvision(120),
            SpecialSense::Tremorsense(60),
        ]),
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
