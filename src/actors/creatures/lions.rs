use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{LION_BITE, LION_CLAWS, LION_MULTI};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, Skill};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Lion — CR 1 large beast. Pride hunter: Pack Tactics gives advantage
/// on melee swings when an ally is adjacent to the target, so a pair of
/// lions out-damages a lone tiger of the same CR. Bite + claws multi at
/// the standard "bite (1d8) + rake (1d6)" cat chassis. RAW also has
/// Pounce and Running Leap riders tied to straight-line charges; the
/// engine doesn't track straight movement so those collapse into the
/// Pack-Tactics-driven advantage instead — same net "lions hit harder
/// when paired" feel.
pub static LION_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&LION_BITE);
    actions.push(&LION_CLAWS);
    actions.push(&*LION_MULTI);
    CreatureTemplate {
        name: "Lion",
        // 'I' (capital) — free in the medium-large beast slot; 'L' is
        // already taken by Lizardfolk / Storm Giant.
        glyph: 'I',
        ac: 12,
        hitpoints: "4d10".parse().unwrap(),
        speed: 50.,
        strength: 17,
        intelligence: 3,
        dexterity: 15,
        wisdom: 12,
        constitution: 11,
        charisma: 8,
        skills: HashSet::from([Skill::Perception, Skill::Stealth]),
        cr: 1.0,
        size: Size::Large,
        creature_type: CreatureType::Beast,
        actions,
        has_pack_tactics: true,
        // RAW: when the lion closes at least the clause's distance in a
        // straight line and then connects with its claws, the hit carries
        // a Strength save vs prone. Read at the melee attack
        // chokepoint off `ActorInstance::charge`.
        charge: Some(crate::actions::monster_attacks::LION_POUNCE),
        ..CreatureTemplate::defaults()
    }
});
