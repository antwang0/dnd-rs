use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{HARPY_TALONS, LURING_SONG};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Language, Size};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Harpy — CR 1 monstrosity. Talons for melee, Luring Song for ranged
/// control: a WIS save in a 30ft radius that Charms hearers for 3
/// rounds. Charmed enemies can't make hostile actions against the
/// harpy (via the standard SetCharmedBy link), so the song is a
/// genuine threat even against full-strength parties — pulling one
/// PC out of the kill-the-harpy plan for a turn is a real swing.
pub static HARPY_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&HARPY_TALONS);
    actions.push(&*LURING_SONG);
    CreatureTemplate {
        name: "Harpy",
        // 'H' for Harpy — Hobgoblin already uses 'B', Bugbear uses 'G',
        // so 'H' is free.
        glyph: 'H',
        ac: 11,
        hitpoints: "7d8+7".parse().unwrap(),
        speed: 20., // walk speed (flying not modeled)
        strength: 12,
        intelligence: 7,
        dexterity: 13,
        wisdom: 10,
        constitution: 12,
        charisma: 13,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::new(),
        languages: HashSet::from([Language::Common]),
        cr: 1.0,
        size: Size::Medium,
        creature_type: CreatureType::Monstrosity,
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
