use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::DIRE_WOLF_BITE;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Larger, scarier sister of Wolf. Large size (4×4 footprint), 2d6 bite,
/// DC-13 prone rider. Pack tactics from RAW isn't modeled (we don't have
/// per-attack ally-adjacency advantage yet) but the raw stats already
/// pull weight on their own.
pub static DIRE_WOLF_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*DIRE_WOLF_BITE);
    CreatureTemplate {
        name: "Dire Wolf",
        glyph: 'D',
        n_instances: 0,
        ac: 14,
        hitpoints: "5d10".parse().unwrap(),
        speed: 50.,
        strength: 17,
        intelligence: 3,
        dexterity: 15,
        wisdom: 12,
        constitution: 15,
        charisma: 7,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::new(),
        cr: 1.0,
        size: Size::Large,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: false,
        damage_resistances: HashSet::new(),
        damage_immunities: HashSet::new(),
        damage_vulnerabilities: HashSet::new(),
        condition_immunities: HashSet::new(),
    }
});
