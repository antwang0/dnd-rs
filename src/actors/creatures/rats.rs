use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::RAT_BITE;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Giant Rat — Tiny/CR-1/8 filler. 1 HP threshold when hit right, but
/// bites at DEX-mod-to-hit with a tiny 1d4 nibble. Comes in swarms in
/// generated dungeons since CR is fractional.
pub static RAT_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*RAT_BITE);
    CreatureTemplate {
        name: "Giant Rat",
        glyph: 'r',
        n_instances: 0,
        ac: 12,
        hitpoints: "1d6".parse().unwrap(),
        speed: 30.,
        strength: 7,
        intelligence: 2,
        dexterity: 15,
        wisdom: 10,
        constitution: 11,
        charisma: 4,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::new(),
        cr: 0.125,
        size: Size::Tiny,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: false,
        resistances: HashSet::new(),
        immunities: HashSet::new(),
        vulnerabilities: HashSet::new(),
    }
});
