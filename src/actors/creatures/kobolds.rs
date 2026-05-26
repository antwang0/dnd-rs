use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{DAGGER, SHORTBOW};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{Language, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Kobold — small, fragile DEX-based skirmisher. Dagger melee + shortbow
/// bonus action lets them hit-and-run. Pack tactics (RAW: advantage when
/// an ally is near) is approximated through the existing Help action,
/// which kobolds have access to via DEFAULT_ACTIONS.
pub static KOBOLD_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&DAGGER);
    actions.push(&SHORTBOW);
    CreatureTemplate {
        name: "Kobold",
        glyph: 'K',
        ac: 12,
        hitpoints: "2d6".parse().unwrap(),
        speed: 30.,
        strength: 7,
        intelligence: 8,
        dexterity: 15,
        wisdom: 9,
        constitution: 9,
        charisma: 8,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Common, Language::Draconic]),
        cr: 0.125,
        size: Size::Small,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: false,
        // Kobolds have sunlight sensitivity in 5e; we don't model lighting
        // so we skip it here.
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
        has_pack_tactics: true,
    }
});
