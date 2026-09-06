use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{BROWN_BEAR_BITE, BROWN_BEAR_CLAWS, BROWN_BEAR_MULTI};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Brown Bear — CR 1 large beast. The grapple-and-rake archetype: a
/// bite + claws CompoundAttack multi swings both limbs per Action,
/// and the standalone bite / claws are exposed too so the AI can
/// fall back to a single limb when bonus-action-tagged or moving in.
/// Slower than a wolf (40 ft) but hits much harder, and bigger
/// footprint (Large) so its melee threat zone covers two tiles.
/// Common ranger / druid summon — slots cleanly into the Conjure
/// Animals beast-pool flavor.
pub static BROWN_BEAR_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&BROWN_BEAR_BITE);
    actions.push(&BROWN_BEAR_CLAWS);
    actions.push(&*BROWN_BEAR_MULTI);
    CreatureTemplate {
        name: "Brown Bear",
        // 'B' for Bear — capital because Large.
        glyph: 'B',
        ac: 11,
        hitpoints: "3d10+6".parse().unwrap(),
        speed: 40.,
        strength: 17,
        intelligence: 2,
        dexterity: 12,
        wisdom: 13,
        constitution: 15,
        charisma: 7,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        cr: 1.0,
        size: Size::Large,
        creature_type: CreatureType::Beast,
        actions,
        ..CreatureTemplate::defaults()
    }
});
