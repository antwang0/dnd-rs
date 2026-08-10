use crate::actions::class_features::{BLOOD_FRENZY_TAG, SWIM_SPEED_TAG};
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::QUIPPER_BITE;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Quipper — CR 0 tiny beast. A piranha: one hit point, one point of
/// bite, and **Blood Frenzy**, which is advantage on every melee attack
/// against anything already wounded.
///
/// The single-fish counterpart to the Swarm of Quippers already in the
/// aquatic pool, and the reason both exist is that they are different
/// fights. A swarm is one body with one initiative; a shoal of quippers
/// is twelve, each rolling its own d20, each with advantage the moment
/// the first one draws blood. The second is worse and RAW prints both.
///
/// **Water Breathing** — RAW: "the quipper can breathe only underwater"
/// — is the half the engine cannot express: there is no drowning clock
/// and no air to be out of. `SWIM_SPEED_TAG` carries the half that does
/// exist, which is that water costs a quipper nothing and its bite
/// keeps its edge in it.
///
/// Stat shape per the SRD: AC 13, 1 HP (1d1), STR 2 / DEX 16 / CON 9 /
/// INT 1 / WIS 7 / CHA 2. Speed 40 (swimming). Darkvision 60. CR 0.
pub static QUIPPER_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&QUIPPER_BITE);
    CreatureTemplate {
        name: "Quipper",
        // 'q' (lowercase) — free, and unmistakable.
        glyph: 'q',
        ac: 13,
        // RAW is a flat 1 hit point. `1d1` is how the engine spells a
        // constant, and it keeps the field a die expression like every
        // other stat block's.
        hitpoints: "1d1".parse().unwrap(),
        speed: 40.,
        strength: 2,
        dexterity: 16,
        constitution: 9,
        intelligence: 1,
        wisdom: 7,
        charisma: 2,
        senses: HashSet::from([crate::engine::types::SpecialSense::Darkvision(60)]),
        cr: 0.0,
        size: Size::Tiny,
        creature_type: CreatureType::Beast,
        actions,
        features: HashSet::from([BLOOD_FRENZY_TAG, SWIM_SPEED_TAG]),
        ..CreatureTemplate::defaults()
    }
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::actor_template::ActorInstance;
    use crate::engine::dice::FastRandRoller;
    use crate::engine::types::Coordinate;

    #[test]
    fn quipper_template_shape() {
        let a = ActorInstance::from_creature_template(
            &QUIPPER_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert_eq!(a.cr(), 0.0);
        assert_eq!(a.size(), Size::Tiny);
        assert_eq!(a.hitpoints(), 1, "RAW's quipper has exactly one");
        assert!(a.find_action("quipper bite").is_some());
        // Both halves of what makes a shoal dangerous.
        assert!(a.has_passive_feature(BLOOD_FRENZY_TAG));
        assert!(a.has_swim_speed());
    }
}
