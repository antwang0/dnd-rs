use crate::actions::class_features::{NIMBLE_DISENGAGE, NIMBLE_HIDE};
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{SABER_TIGER_BITE, SABER_TIGER_CLAWS, SABER_TIGER_MULTI};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, Skill, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Saber-toothed Tiger — CR 2 large beast. The Ice Age cousin of the
/// vanilla Tiger: same Pounce-and-rake silhouette, but the dice tier
/// bumps up — bite 1d10 + STR piercing, claws 2d6 + STR slashing
/// (vs the regular Tiger's 1d10 / 1d8). Pairs nicely as a heavier
/// large-beast filler alongside Brown Bear / Polar Bear (CR 1-2)
/// without overlapping the Lion's Pack-Tactics niche. Common Druid
/// Conjure Animals upgrade target when the party can afford the
/// CR-2 slot in the encounter budget.
pub static SABER_TOOTHED_TIGER_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&SABER_TIGER_BITE);
    actions.push(&SABER_TIGER_CLAWS);
    actions.push(&*SABER_TIGER_MULTI);
    // RAW **Nimble Escape**: "the cat takes the Disengage or Hide
    // action" as a Bonus Action — the trait that lets it close, maul
    // and step back out of reach in one turn.
    actions.push(&*NIMBLE_DISENGAGE);
    actions.push(&*NIMBLE_HIDE);
    CreatureTemplate {
        name: "Saber-Toothed Tiger",
        // 't' (lowercase) — distinct from 'T' (Tiger / Troll). Mnemonic:
        // smaller-cased tiger glyph for the upgraded-stat variant.
        glyph: 't',
        ac: 13,
        // 7d10+14 ≈ 52 average per MM (CR 2).
        hitpoints: "7d10+14".parse().unwrap(),
        speed: 40.,
        strength: 18,
        intelligence: 3,
        dexterity: 17,
        wisdom: 12,
        constitution: 15,
        charisma: 8,
        skills: HashSet::from([Skill::Perception, Skill::Stealth]),
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        cr: 2.0,
        size: Size::Large,
        creature_type: CreatureType::Beast,
        actions,
        // RAW: when the cat closes at least the clause's distance in a
        // straight line and then connects with its claws, the hit carries
        // a Strength save vs prone. Read at the melee attack
        // chokepoint off `ActorInstance::charge`.
        charge: Some(crate::actions::monster_attacks::SABER_TIGER_POUNCE),
        ..CreatureTemplate::defaults()
    }
});
