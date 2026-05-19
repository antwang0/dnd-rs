use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{HIPPOGRIFF_BEAK, HIPPOGRIFF_MULTI, HIPPOGRIFF_TALONS};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{Size};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Hippogriff — CR 1 monstrosity. Mid-tier melee threat with a beak +
/// talons multiattack option. We don't model the 5e fly speed, so the
/// hippogriff behaves like a fast ground unit (60ft walk in our
/// engine's terms). Pairs well with mid-air-flavor encounters even
/// without the flight model.
pub static HIPPOGRIFF_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&HIPPOGRIFF_BEAK);
    actions.push(&HIPPOGRIFF_TALONS);
    actions.push(&*HIPPOGRIFF_MULTI);
    CreatureTemplate {
        name: "Hippogriff",
        // 'H' was free (Harpy uses 'h' lowercase). Picking 'H' for
        // hippogriff so the map differentiates Harpy / Hippogriff.
        glyph: 'H',
        ac: 11,
        // 3d10+3 = 19 average per MM.
        hitpoints: "3d10+3".parse().unwrap(),
        speed: 40.,
        strength: 17,
        intelligence: 2,
        dexterity: 13,
        wisdom: 12,
        constitution: 13,
        charisma: 8,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::new(),
        languages: HashSet::new(),
        cr: 1.0,
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
    }
});
