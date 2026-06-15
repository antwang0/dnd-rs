use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{GIANT_APE_FIST, GIANT_APE_MULTI, GIANT_APE_ROCK};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Giant Ape — CR 7 huge beast. The classic Kong-style brute: AC 12, big
/// HP pool, and a Multiattack of two crushing fists. Out of fist range it
/// throws boulders for 7d6 — a brutal single-die ranged option that
/// rewards positioning out of melee. No special features: pure stat block
/// muscle slots in between the Stone Giant (CR 7 ranged-bias) and the
/// Frost Giant (CR 8 melee-bias) for the upper-mid CR pool.
pub static GIANT_APE_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&GIANT_APE_FIST);
    actions.push(&GIANT_APE_ROCK);
    actions.push(&*GIANT_APE_MULTI);
    CreatureTemplate {
        name: "Giant Ape",
        // 'A' for Ape — capital because Huge.
        glyph: 'A',
        ac: 12,
        hitpoints: "12d12+36".parse().unwrap(),
        speed: 40.,
        strength: 23,
        intelligence: 7,
        dexterity: 14,
        wisdom: 12,
        constitution: 18,
        charisma: 7,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::new(),
        languages: HashSet::new(),
        cr: 7.0,
        size: Size::Huge,
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
