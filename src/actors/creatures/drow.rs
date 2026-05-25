use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{DROW_POISONED_CROSSBOW, SCIMITAR};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{Language, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Drow — Underdark elf raider. Trained scimitarist + ranged poisoned-bolt
/// pressure via a hand crossbow. The poison rider is the headline (CON
/// save DC 13, fail = +2d4 poison and Poisoned for 2 rounds). Stat shape
/// follows MM Drow at CR 1/4: 14 AC (chain shirt), 13 HP, +4 to hit.
/// Senses: Darkvision (120 ft, doubling a goblin's 60 ft).
pub static DROW_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&SCIMITAR);
    actions.push(&*DROW_POISONED_CROSSBOW);
    CreatureTemplate {
        name: "Drow",
        glyph: 'D',
        ac: 15,
        hitpoints: "3d8".parse().unwrap(),
        speed: 30.,
        strength: 10,
        intelligence: 11,
        dexterity: 14,
        wisdom: 11,
        constitution: 10,
        charisma: 12,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([SpecialSense::Darkvision(120)]),
        languages: HashSet::from([Language::Elvish, Language::Undercommon]),
        cr: 0.25,
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
        legendary_resistances: 0,
        has_evasion: false,
        has_uncanny_dodge: false,
    }
});
