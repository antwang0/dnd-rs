use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{
    CARRION_CRAWLER_BITE, CARRION_CRAWLER_MULTI, CARRION_CRAWLER_TENTACLES,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, Skill, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Carrion Crawler — CR 2 large monstrosity. The classic dungeon
/// ceiling-walker: paralyzing tentacle lash at reach 2, then a follow-
/// up bite once the target is locked down. Same CR slot as Polar Bear
/// / Pegasus / Centaur but the threat profile is the paralysis lock,
/// not raw damage — Paralyzed auto-fails STR/DEX saves and locks the
/// action economy, so a single failed CON save can swing a round.
pub static CARRION_CRAWLER_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*CARRION_CRAWLER_TENTACLES);
    actions.push(&CARRION_CRAWLER_BITE);
    actions.push(&*CARRION_CRAWLER_MULTI);
    CreatureTemplate {
        name: "Carrion Crawler",
        // 'z' (lowercase) — free in the large monstrosity slot. 'C' is
        // already taken by Centaur / Cleric / Cockatrice; 'z' reads as
        // the segmented, squirming silhouette on the map.
        glyph: 'z',
        ac: 13,
        // 7d10+14 ≈ 51 average per MM (CR 2).
        hitpoints: "7d10+14".parse().unwrap(),
        speed: 30.,
        strength: 14,
        intelligence: 1,
        dexterity: 13,
        wisdom: 12,
        constitution: 14,
        charisma: 5,
        skills: HashSet::from([Skill::Perception]),
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        cr: 2.0,
        size: Size::Large,
        creature_type: CreatureType::Monstrosity,
        actions,
        ..CreatureTemplate::defaults()
    }
});
