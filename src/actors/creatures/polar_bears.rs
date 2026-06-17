use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{POLAR_BEAR_BITE, POLAR_BEAR_CLAWS, POLAR_BEAR_MULTI};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Polar Bear — CR 2 large beast. Sibling to the Brown Bear (CR 1) one
/// step up the bear ladder: same bite + claws CompoundAttack shape, but
/// bigger HP, higher STR (+5 mod vs +4), and the heavier rake stays at
/// 2d6 slashing. Common arctic / wilderness encounter and a staple
/// druid / ranger Conjure Animals pick when a single heavy hitter is
/// preferred over a swarm of CR-¼ wolves.
pub static POLAR_BEAR_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&POLAR_BEAR_BITE);
    actions.push(&POLAR_BEAR_CLAWS);
    actions.push(&*POLAR_BEAR_MULTI);
    CreatureTemplate {
        name: "Polar Bear",
        // 'P' for Polar — capital because Large; distinct from 'B' for
        // Brown Bear so the glyphs stay distinguishable on a shared map.
        glyph: 'P',
        ac: 12,
        // 9d10+27 ≈ 76 average per MM (CR 2).
        hitpoints: "9d10+27".parse().unwrap(),
        speed: 40.,
        strength: 20,
        intelligence: 2,
        dexterity: 10,
        wisdom: 13,
        constitution: 16,
        charisma: 7,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        cr: 2.0,
        size: Size::Large,
        creature_type: CreatureType::Beast,
        actions,
        ..CreatureTemplate::defaults()
    }
});
