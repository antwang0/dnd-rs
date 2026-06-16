use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{GIANT_EAGLE_BEAK, GIANT_EAGLE_MULTI, GIANT_EAGLE_TALONS};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size};
use std::sync::LazyLock;

/// Giant Eagle — CR 1 large beast. Aerial predator with a one-beak +
/// one-talons CompoundAttack multi. Fast walk speed (the engine doesn't
/// model 3D fly, so the eagle's 80ft fly speed surfaces as a high ground
/// speed — fast enough to keep the bird threatening at range and able to
/// reposition between bursts). Standalone beak / talons are exposed too so
/// the AI can fall back to a single swing when it's bonus-action-tagged
/// or moving in for a bite.
pub static GIANT_EAGLE_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&GIANT_EAGLE_BEAK);
    actions.push(&GIANT_EAGLE_TALONS);
    actions.push(&*GIANT_EAGLE_MULTI);
    CreatureTemplate {
        name: "Giant Eagle",
        // 'E' for Eagle — capital because Large; no other E creature yet.
        glyph: 'E',
        ac: 13,
        hitpoints: "4d10+4".parse().unwrap(),
        // Approx 40ft walking; the 80ft fly is the headline but the
        // engine collapses to a single ground speed.
        speed: 40.,
        strength: 16,
        intelligence: 8,
        dexterity: 17,
        wisdom: 14,
        constitution: 13,
        charisma: 10,
        cr: 1.0,
        size: Size::Large,
        creature_type: CreatureType::Beast,
        actions,
        ..CreatureTemplate::defaults()
    }
});
