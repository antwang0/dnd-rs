use crate::actions::class_features::SWIM_SPEED_TAG;
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::GIANT_TOAD_BITE;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Giant Toad — CR 1 large beast. Poison-bite specialist: 1d10+2
/// piercing + 1d10 poison rider on every hit (lands as a flat
/// secondary damage instance, not a save-or-suck condition — the
/// `GiantToadBite` action carries the rider). RAW grapples Medium-
/// or-smaller targets on hit; that grapple half isn't modeled, but
/// the toad still feels like a toad — a slow (20 ft on land) ambush
/// croaker with a venomous chomp. Common low-CR boss in swamp
/// encounters.
pub static GIANT_TOAD_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&GIANT_TOAD_BITE);
    CreatureTemplate {
        name: "Giant Toad",
        // 't' for toad — lowercase even though Large, since the
        // glyph collides with Troll/Tiger and lower-case keeps the
        // amphibian distinct.
        glyph: 't',
        ac: 11,
        hitpoints: "6d10+6".parse().unwrap(),
        // RAW: 20 ft walk, 40 ft swim. The engine collapses to one
        // ground speed so we land between the two.
        speed: 30.,
        strength: 15,
        intelligence: 2,
        dexterity: 13,
        wisdom: 10,
        constitution: 13,
        charisma: 3,
        senses: HashSet::from([SpecialSense::Darkvision(30)]),
        cr: 1.0,
        size: Size::Large,
        creature_type: CreatureType::Beast,
        actions,
        // RAW swim speed: the tag is what makes `TerrainType::Water`
        // free to cross and lifts the underwater melee penalty.
        features: HashSet::from([SWIM_SPEED_TAG]),
        ..CreatureTemplate::defaults()
    }
});
