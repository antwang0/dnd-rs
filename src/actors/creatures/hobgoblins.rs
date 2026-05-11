use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{LONGBOW, SCIMITAR};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{Language, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Hobgoblin — CR 1/2 martial humanoid. Disciplined and well-armored
/// (chain mail + shield → AC 18) compared to the rabble goblin. Carries
/// both a scimitar (melee) and a longbow (ranged) so it can pivot to
/// whichever range suits the moment. No special features — the threat
/// is just having tankier mooks at the same XP price as bandits.
pub static HOBGOBLIN_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&SCIMITAR);
    actions.push(&LONGBOW);
    CreatureTemplate {
        name: "Hobgoblin",
        // 'H' is unused — keep the glyph mnemonic for hobgoblin.
        glyph: 'H',
        ac: 18,
        hitpoints: "2d8+2".parse().unwrap(),
        speed: 30.,
        strength: 13,
        intelligence: 10,
        dexterity: 12,
        wisdom: 10,
        constitution: 12,
        charisma: 9,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Common, Language::Goblin]),
        cr: 0.5,
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
