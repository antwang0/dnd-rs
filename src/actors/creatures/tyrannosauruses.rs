use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{T_REX_BITE, T_REX_MULTI, T_REX_TAIL};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, Skill};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Tyrannosaurus Rex — CR 8 huge beast. The apex predator's
/// signature: a brutal 4d12 piercing bite at reach 2, paired with a
/// 3d8 tail sweep for the multi. Slots alongside Frost Giant (CR 8)
/// and Cyclops (CR 6) / Giant Ape (CR 7) on the upper-mid pool — the
/// dinosaur option for "huge melee threat with two attack lanes." No
/// rider effects, no breath, no spells — pure dice-tier dominance.
pub static T_REX_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&T_REX_BITE);
    actions.push(&T_REX_TAIL);
    actions.push(&*T_REX_MULTI);
    CreatureTemplate {
        name: "Tyrannosaurus Rex",
        // 'x' (lowercase) — free in the huge beast slot. 'T' is taken
        // by Treant / Tiger; 'x' reads as the apex / boss marker.
        glyph: 'x',
        ac: 13,
        // 13d12+52 ≈ 136 average per MM (CR 8).
        hitpoints: "13d12+52".parse().unwrap(),
        speed: 50.,
        strength: 25,
        intelligence: 2,
        dexterity: 10,
        wisdom: 12,
        constitution: 19,
        charisma: 9,
        skills: HashSet::from([Skill::Perception]),
        cr: 8.0,
        size: Size::Huge,
        creature_type: CreatureType::Beast,
        actions,
        ..CreatureTemplate::defaults()
    }
});
