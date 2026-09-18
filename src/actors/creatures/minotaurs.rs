use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{GREATAXE, MINOTAUR_GORE};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Language, Size, Skill, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Minotaur — CR 3 monstrosity. Mid-tier melee bruiser with two action
/// options: a big greataxe swing (1d12+STR slashing) or a gore charge
/// (2d8+STR piercing) — both single-target heavy hitters tuned for the
/// AI to pick between depending on what's in reach. AC 14 with 85
/// average HP makes them notably tankier than the bandit-tier mooks.
pub static MINOTAUR_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&GREATAXE);
    actions.push(&*MINOTAUR_GORE);
    CreatureTemplate {
        // SRD 5.2 calls this stat block **Minotaur of Baphomet**; the engine
        // carried the 2014 heading ("Minotaur") until the sweep that
        // compares the two had to keep a translation table to do
        // its job. The file and the static keep their old spelling,
        // because that is the word this codebase files the creature
        // under and moving it buys nothing a reader wants; the name
        // a player sees is the book's.
        name: "Minotaur of Baphomet",
        // 'N' for miNotaur — distinct from existing glyphs.
        glyph: 'N',
        ac: 14,
        // 10d10+30 = 85 average per MM.
        hitpoints: "10d10+30".parse().unwrap(),
        speed: 40.,
        strength: 18,
        dexterity: 11,
        constitution: 16,
        intelligence: 6,
        wisdom: 16,
        charisma: 9,
        languages: HashSet::from([Language::Common]),
        cr: 3.0,
        size: Size::Large,
        creature_type: CreatureType::Monstrosity,
        actions,
        skills: HashSet::from([Skill::Perception]),
        // RAW: when the minotaur closes at least the clause's distance in a
        // straight line and then connects with its gore, the hit carries
        // extra 2d8 piercing and a Strength save vs prone. Read at the melee attack
        // chokepoint off `ActorInstance::charge`.
        charge: Some(crate::actions::monster_attacks::MINOTAUR_CHARGE),
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        ..CreatureTemplate::defaults()
    }
});
