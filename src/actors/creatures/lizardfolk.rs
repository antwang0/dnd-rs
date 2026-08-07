use crate::actions::class_features::SWIM_SPEED_TAG;
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{HEAVY_CLUB, LIZARDFOLK_BITE, LIZARDFOLK_MULTI};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Language, Size};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Lizardfolk — CR 1/2 swamp humanoid. Natural-armor AC 15 (the highest
/// of the cr-½ humanoid tier in our pool) plus a multi that fires the
/// natural bite and a heavy club in one Action. No special features
/// beyond the AC + decent HP — the stat block is meant to be a sturdy
/// front-liner that survives a round of single-target focus and rewards
/// AoE / spell pressure to thin a wider group.
pub static LIZARDFOLK_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&LIZARDFOLK_BITE);
    actions.push(&HEAVY_CLUB);
    actions.push(&*LIZARDFOLK_MULTI);
    CreatureTemplate {
        name: "Lizardfolk",
        // 'L' — capital L was free in the medium-humanoid slot.
        glyph: 'L',
        ac: 15,
        hitpoints: "4d8+4".parse().unwrap(),
        speed: 30.,
        strength: 15,
        intelligence: 7,
        dexterity: 10,
        wisdom: 12,
        constitution: 13,
        charisma: 7,
        languages: HashSet::from([Language::Draconic]),
        cr: 0.5,
        size: Size::Medium,
        creature_type: CreatureType::Humanoid,
        actions,
        // RAW swim speed: the tag is what makes `TerrainType::Water`
        // free to cross and lifts the underwater melee penalty.
        features: HashSet::from([SWIM_SPEED_TAG]),
        ..CreatureTemplate::defaults()
    }
});
