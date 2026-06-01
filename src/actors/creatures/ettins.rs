use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{Multiattack, GREATCLUB};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Language, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

static ETTIN_MULTI: LazyLock<Multiattack> = LazyLock::new(|| Multiattack {
    display_name: "two-headed smash",
    sub_attack: &GREATCLUB,
    count: 2,
});

/// Ettin — two-headed giant (CR 4, MM p.132). Each head wields a
/// greatclub, granting a 2-swing multiattack. Two heads means the Ettin
/// has advantage on Perception checks (not modeled) and can't be
/// surprised (not modeled). Size Large to match the giant footprint.
pub static ETTIN_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*ETTIN_MULTI);
    CreatureTemplate {
        name: "Ettin",
        glyph: 'E',
        ac: 12,
        hitpoints: "10d10+20".parse().unwrap(),
        speed: 40.,
        strength: 21,
        intelligence: 6,
        dexterity: 8,
        wisdom: 10,
        constitution: 17,
        charisma: 8,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Giant, Language::Orc]),
        cr: 4.0,
        size: Size::Large,
        creature_type: CreatureType::Giant,
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
        has_extra_attack: true,
        brutal_critical_dice: 0,
        crit_threshold: 20,
        has_lucky: false,
        has_brave: false,
        has_aura_of_protection: false,
        has_aura_of_courage: false,
        has_savage_attacks: false,
        has_dwarven_resilience: false,
        has_gnome_cunning: false,
        draconic_ancestry: None,
        sorcery_points: 0,
    }
});
