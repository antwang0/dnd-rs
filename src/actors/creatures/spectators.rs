use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::SPECTATOR_EYE_RAY;
use crate::actors::actor_template::CreatureTemplate;
use crate::conditions::Condition;
use crate::engine::types::{CreatureType, Language, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Spectator — CR 3 aberration, a mini-beholder. Hovers in place
/// (speed 0, conceptual hover 30 — we model as 30 ground speed since
/// hover vs walk doesn't mechanically differ in the engine) and fires
/// eye rays at range. Small size, decent WIS and INT, but physically
/// weak (STR 8). Immune to Prone because it hovers — can't be knocked
/// down when you don't touch the ground.
pub static SPECTATOR_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*SPECTATOR_EYE_RAY);
    CreatureTemplate {
        name: "Spectator",
        // 'E' for Eye — 'S' and 'B' are taken by spiders and beholders.
        glyph: 'E',
        ac: 14,
        hitpoints: "6d8+6".parse().unwrap(),
        // Hover 30 ft — modeled as ground speed since the engine doesn't
        // distinguish flight from walking mechanically.
        speed: 30.,
        strength: 8,
        intelligence: 13,
        dexterity: 14,
        wisdom: 14,
        constitution: 12,
        charisma: 11,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([SpecialSense::Darkvision(120)]),
        languages: HashSet::from([Language::DeepSpeech, Language::Undercommon]),
        cr: 3.0,
        size: Size::Small,
        creature_type: CreatureType::Aberration,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: false,
        damage_modifiers: HashMap::new(),
        proficient_saves: HashSet::new(),
        // Hovering creatures can't be knocked Prone.
        condition_immunities: HashSet::from([Condition::Prone]),
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
