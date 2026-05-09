use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::SPIDER_BITE;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{DamageType, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Giant Spider — small fast melee biter that injects poison on a failed
/// CON save. Showcases the Poisoned condition source paired with raw
/// poison damage. Resistant to bludgeoning (gooey body), immune to its
/// own venom.
pub static SPIDER_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*SPIDER_BITE);
    CreatureTemplate {
        name: "Spider",
        glyph: 'X',
        n_instances: 0,
        ac: 14,
        hitpoints: "3d8+3".parse().unwrap(),
        speed: 30.,
        strength: 14,
        intelligence: 2,
        dexterity: 16,
        wisdom: 11,
        constitution: 12,
        charisma: 4,
        skills: HashSet::new(),
        items: Vec::new(),
        senses: HashSet::from([
            SpecialSense::Blindsight(10),
            SpecialSense::Darkvision(60),
        ]),
        languages: HashSet::new(),
        cr: 1.0,
        size: Size::Medium,
        actions,
        spell_slots_by_level: Vec::new(),
        rolls_death_saves: false,
        damage_resistances: HashSet::new(),
        damage_vulnerabilities: HashSet::new(),
        damage_immunities: HashSet::from([DamageType::Poison]),
    }
});
