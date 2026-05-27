use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{BITE, CompoundAttack, SLAM};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{Language, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

static CHIMERA_MULTI: LazyLock<CompoundAttack> = LazyLock::new(|| CompoundAttack {
    display_name: "chimera multiattack",
    parts: vec![(&*BITE, 1), (&SLAM, 2)],
});

/// Chimera — three-headed monstrosity: lion, dragon, goat (CR 6,
/// MM p.39). Multiattack: 1 bite (lion head) + 2 slams (claws). RAW
/// also has a fire-breath action (dragon head) but we keep to the
/// physical multiattack for now. Size Large.
pub static CHIMERA_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*CHIMERA_MULTI);
    CreatureTemplate {
        name: "Chimera",
        glyph: 'C',
        ac: 14,
        hitpoints: "12d10+24".parse().unwrap(),
        speed: 30.,
        strength: 19,
        intelligence: 3,
        dexterity: 11,
        wisdom: 14,
        constitution: 19,
        charisma: 10,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Draconic]),
        cr: 6.0,
        size: Size::Large,
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
    }
});
