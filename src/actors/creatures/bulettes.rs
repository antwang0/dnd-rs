use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{BULETTE_BITE, BULETTE_DEADLY_LEAP, BULETTE_MULTI};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Bulette — CR 5 monstrosity (the iconic "land shark"). Large-footprint
/// burrowing apex predator with three attack lanes:
/// - **Bite**: 4d12+STR piercing melee, reach 1.
/// - **Multiattack** (2 bites): full Action damage burst.
/// - **Deadly Leap**: 3d6+STR bludgeoning melee that forces a STR save
///   vs Prone on fail — sets up adjacent melee allies with the prone-
///   crit advantage clause.
///
/// No languages (non-sentient predator); no spell slots. The bulette's
/// signature MM stat is its high CON / HP pool — RAW: 9d10+45 = ~94 HP,
/// which we adopt directly. AC 17 mirrors the natural armor envelope.
pub static BULETTE_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&BULETTE_BITE);
    actions.push(&*BULETTE_MULTI);
    actions.push(&*BULETTE_DEADLY_LEAP);
    CreatureTemplate {
        name: "Bulette",
        // 'B' is taken (Bracers loot glyph is lowercase 'B' in items, but
        // the creature glyph table uses uppercase). Bulette glyph 'U' is
        // free (no creature claims it today).
        glyph: 'U',
        ac: 17,
        hitpoints: "9d10+45".parse().unwrap(),
        speed: 40.,
        strength: 19,
        intelligence: 2,
        dexterity: 11,
        wisdom: 10,
        constitution: 21,
        charisma: 5,
        // Tremorsense lives in SpecialSense; falling back to Darkvision
        // keeps the template valid without inventing a new sense variant.
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        cr: 5.0,
        size: Size::Large,
        creature_type: CreatureType::Monstrosity,
        actions,
        ..CreatureTemplate::defaults()
    }
});
