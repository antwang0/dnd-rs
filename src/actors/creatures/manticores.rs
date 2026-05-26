use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{MANTICORE_MULTIATTACK, MANTICORE_SPIKES};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{Language, Size, SpecialSense};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Manticore — CR 3 monstrosity. The marquee dual-lane threat: ranged
/// tail spikes (3d8 piercing at 30 ft) for stand-off pressure plus a
/// bite + double-claw multiattack when an enemy closes. Speaks Common
/// (5e RAW) which mostly matters for charm-immunity tracking — the
/// manticore is a creature, not a humanoid, but the language tag is
/// kept for completeness.
pub static MANTICORE_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*MANTICORE_SPIKES);
    actions.push(&*MANTICORE_MULTIATTACK);
    CreatureTemplate {
        name: "Manticore",
        // 'n' (lowercase) is free; uppercase 'N' is taken by Gnoll and
        // Minotaur. The manticore gets its own slot at 'n'.
        glyph: 'n',
        ac: 14,
        // 9d10+9 = 58 average per MM. Beefy CR 3 frame with the high
        // AC + ranged option offsetting the smaller HP pool of a CR 3.
        hitpoints: "9d10+9".parse().unwrap(),
        speed: 30.,
        strength: 17,
        intelligence: 7,
        dexterity: 16,
        wisdom: 12,
        constitution: 17,
        charisma: 8,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Common]),
        cr: 3.0,
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
    }
});
