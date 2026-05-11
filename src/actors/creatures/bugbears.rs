use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::BUGBEAR_MORNINGSTAR;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{Language, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Bugbear — CR 1 goblinoid brute. Morningstar carries a Surprise
/// Attack rider that adds 2d6 extra piercing damage on the first round
/// of combat, capturing the "out of the dark" alpha-strike fantasy
/// without modeling stealth approach. Medium-sized but stronger than
/// a goblin / hobgoblin; lacks ranged options.
pub static BUGBEAR_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*BUGBEAR_MORNINGSTAR);
    CreatureTemplate {
        name: "Bugbear",
        // 'B' for bugbear — distinct from 'b' (boots-of-striding glyph).
        glyph: 'B',
        ac: 16,
        hitpoints: "5d8+5".parse().unwrap(),
        speed: 30.,
        strength: 15,
        intelligence: 8,
        dexterity: 14,
        wisdom: 11,
        constitution: 13,
        charisma: 9,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Common, Language::Goblin]),
        cr: 1.0,
        size: Size::Medium,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: false,
        damage_modifiers: HashMap::new(),
        proficient_saves: HashSet::new(),
        condition_immunities: HashSet::new(),
        features: HashSet::new(),
        regen_per_round: 0,
        regen_suppressors: HashSet::new(),
    }
});
