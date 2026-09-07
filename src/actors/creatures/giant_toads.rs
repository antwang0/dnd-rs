use crate::actions::class_features::{SWIM_SPEED_TAG, UNDERWATER_BREATHING_TAG};
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{GIANT_TOAD_BITE, SWALLOW_ACTION};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Giant Toad — CR 1 large beast. Poison-bite specialist: 1d10+2
/// piercing + 1d10 poison rider on every hit (a flat secondary damage
/// instance rather than a save-or-suck condition), and RAW's grapple on
/// anything Medium or smaller.
///
/// The grapple is what the toad is for. It leads to **Swallow**, which
/// takes one party member out of the fight entirely — 3d6 acid a round,
/// Total Cover from every heal and every arrow outside — at the price of
/// the toad's own bite for as long as it holds them down. Thirty-nine
/// hit points is the clock the rest of the party is racing. See
/// `GIANT_TOAD_SWALLOW`.
pub static GIANT_TOAD_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&GIANT_TOAD_BITE);
    actions.push(&SWALLOW_ACTION);
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
        features: HashSet::from([SWIM_SPEED_TAG, UNDERWATER_BREATHING_TAG]),
        swallow: Some(&crate::actions::monster_attacks::GIANT_TOAD_SWALLOW),
        ..CreatureTemplate::defaults()
    }
});
