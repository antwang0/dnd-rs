use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::OWLBEAR_MULTIATTACK;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, Skill, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Owlbear — CR 3 monstrosity. Brute-stat bruiser whose only flair is
/// the multiattack: every Action swings beak (1d10 piercing) AND claws
/// (2d8 slashing). Heavy STR-based damage, no rider effects, decent HP
/// pool. Filling a "pure damage threat at higher CR" niche the troll
/// shares but without regeneration.
pub static OWLBEAR_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*OWLBEAR_MULTIATTACK);
    CreatureTemplate {
        name: "Owlbear",
        // 'b' (lowercase b) — distinct from 'B' (Bugbear) and 'b' is
        // currently free.
        glyph: 'b',
        ac: 13,
        hitpoints: "7d10+21".parse().unwrap(),
        speed: 40.,
        strength: 20,
        dexterity: 12,
        constitution: 17,
        intelligence: 3,
        wisdom: 12,
        charisma: 7,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        cr: 3.0,
        size: Size::Large,
        creature_type: CreatureType::Monstrosity,
        actions,
        skills: HashSet::from([Skill::Perception]),
        ..CreatureTemplate::defaults()
    }
});
