use crate::actions::class_features::RAMPAGE_TAG;
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{LONGBOW, BITE};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Language, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Gnoll — CR 1/2 hyena-headed humanoid. Bite + longbow loadout so it
/// can press in melee or harass at range. The marquee mechanic is
/// **Rampage** — half a move and another bite, as a Bonus Action, the
/// moment it drops somebody — and it is what makes a pack of gnolls a
/// different fight from a pack of anything else at the tier: the first
/// kill is the one that cascades. See `RAMPAGE_TAG`. AC is light hide
/// (12), HP a touch above goblin.
pub static GNOLL_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&BITE);
    actions.push(&LONGBOW);
    CreatureTemplate {
        // SRD 5.2 calls this stat block **Gnoll Warrior**; the engine
        // carried the 2014 heading ("Gnoll") until the sweep that
        // compares the two had to keep a translation table to do
        // its job. The file and the static keep their old spelling,
        // because that is the word this codebase files the creature
        // under and moving it buys nothing a reader wants; the name
        // a player sees is the book's.
        name: "Gnoll Warrior",
        // 'N' (gNoll) — 'G' is the goblin.
        glyph: 'N',
        ac: 15,
        hitpoints: "6d8".parse().unwrap(),
        strength: 14,
        dexterity: 12,
        constitution: 11,
        intelligence: 6,
        wisdom: 10,
        charisma: 7,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::from([Language::Common]),
        cr: 0.5,
        size: Size::Medium,
        creature_type: CreatureType::Fiend,
        actions,
        // SRD 5.2 **Rampage** — the first kill is the one that
        // cascades. See `RAMPAGE_TAG`.
        features: HashSet::from([RAMPAGE_TAG]),
        ..CreatureTemplate::defaults()
    }
});
