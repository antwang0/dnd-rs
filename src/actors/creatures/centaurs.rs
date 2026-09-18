use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{
    CENTAUR_HOOVES, CENTAUR_MULTI, CENTAUR_PIKE, LONGBOW,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Language, Size, Skill};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Centaur — CR 2 large monstrosity. Sturdy hybrid kit: a reach-2 pike
/// for opening jabs, a 2d6 hoof kick for the second swing, and a longbow
/// for ranged fallback when out of melee. Multi pairs pike + hooves so a
/// committed melee turn brings 1d10 + 2d6 down on a single target.
/// Fast 50ft walking speed reflects the equine half — kites comfortably
/// between bow shots.
pub static CENTAUR_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&CENTAUR_PIKE);
    actions.push(&CENTAUR_HOOVES);
    actions.push(&LONGBOW);
    actions.push(&*CENTAUR_MULTI);
    CreatureTemplate {
        // SRD 5.2 calls this stat block **Centaur Trooper**; the engine
        // carried the 2014 heading ("Centaur") until the sweep that
        // compares the two had to keep a translation table to do
        // its job. The file and the static keep their old spelling,
        // because that is the word this codebase files the creature
        // under and moving it buys nothing a reader wants; the name
        // a player sees is the book's.
        name: "Centaur Trooper",
        // 'C' was free in the medium / large monstrosity slot — capital
        // because Large.
        glyph: 'C',
        ac: 16,
        hitpoints: "6d10+12".parse().unwrap(),
        speed: 50.,
        strength: 18,
        intelligence: 9,
        dexterity: 14,
        wisdom: 13,
        constitution: 14,
        charisma: 11,
        languages: HashSet::from([Language::Elvish, Language::Sylvan]),
        cr: 2.0,
        size: Size::Large,
        creature_type: CreatureType::Fey,
        actions,
        // RAW: when the centaur closes at least the clause's distance in a
        // straight line and then connects with its pike, the hit carries
        // extra 3d6 piercing off a thirty-foot run-up. Read at the melee attack
        // chokepoint off `ActorInstance::charge`.
        charge: Some(crate::actions::monster_attacks::CENTAUR_CHARGE),
        skills: HashSet::from([Skill::Athletics, Skill::Perception]),
        ..CreatureTemplate::defaults()
    }
});
