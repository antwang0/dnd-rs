use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{FRIGHTFUL_HOWL, WOLF_BITE};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Fast melee with a built-in trip rider. Bite always rolls the STR save
/// on hit, so a Wolf naturally knocks targets prone — making subsequent
/// melee attacks (its own next-turn bite, or an ally's swing) hit at
/// advantage. Demonstrates rider-on-hit baked into a creature's
/// canonical action.
pub static WOLF_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*WOLF_BITE);
    actions.push(&*FRIGHTFUL_HOWL);
    CreatureTemplate {
        name: "Wolf",
        glyph: 'W',
        ac: 13,
        hitpoints: "2d8+2".parse().unwrap(),
        speed: 40.,
        strength: 12,
        intelligence: 3,
        dexterity: 15,
        wisdom: 12,
        constitution: 12,
        charisma: 6,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::new(),
        cr: 0.25,
        size: Size::Medium,
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
        has_deflect_missiles: false,
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
