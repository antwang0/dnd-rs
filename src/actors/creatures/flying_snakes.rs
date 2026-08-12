use crate::actions::class_features::{FLYBY_TAG, SWIM_SPEED_TAG};
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::FLYING_SNAKE_BITE;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Flying Snake — CR ⅛ tiny monstrosity. A winged serpent with **Flyby**
/// and a bite that is almost entirely venom: one point of puncture and
/// five of poison.
///
/// The combination is the creature. Sixty feet of flight plus free
/// disengagement means the snake picks its target every round with no
/// tax for leaving, and the target it picks is whoever has the worst
/// answer to five points of poison a turn — which at the tier this
/// thing appears is usually the wizard.
///
/// Action lane:
/// - **flying snake bite** — flat 1 piercing plus a flat 2d4 poison
///   rider, landed on a hit with no save.
///
/// A **Monstrosity** rather than a Beast, which is not cosmetic: it
/// puts the flying snake outside the reach of every beast-typed effect
/// on the roster — a druid's Wild Shape roster, Conjure Animals, a
/// ranger's favoured-enemy lanes, Beast Bond.
///
/// Stat shape: AC 14, ~5 HP (2d4), STR 4, DEX 15, CON 11, INT 2,
/// WIS 12, CHA 5. Speed 30 walking, fly 60. Senses Blindsight 10. Size
/// Tiny. CR ⅛. XP 25 per RAW.
pub static FLYING_SNAKE_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&FLYING_SNAKE_BITE);
    CreatureTemplate {
        name: "Flying Snake",
        // 'f' — the winged serpent; distinct from the venomous snake's
        // ground-bound 'n' because on this board the difference between
        // the two is which tiles they can be on.
        glyph: 'f',
        ac: 14,
        hitpoints: "2d4".parse().unwrap(),
        speed: 30.,
        fly_speed: 60.,
        strength: 4,
        intelligence: 2,
        dexterity: 15,
        wisdom: 12,
        constitution: 11,
        charisma: 5,
        senses: HashSet::from([SpecialSense::Blindsight(10)]),
        cr: 0.125,
        size: Size::Tiny,
        creature_type: CreatureType::Monstrosity,
        actions,
        features: HashSet::from([FLYBY_TAG, SWIM_SPEED_TAG]),
        ..CreatureTemplate::defaults()
    }
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::actor_template::ActorInstance;
    use crate::engine::dice::FastRandRoller;
    use crate::engine::types::Coordinate;

    fn make() -> ActorInstance {
        ActorInstance::from_creature_template(
            &FLYING_SNAKE_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn flying_snake_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 0.125);
        assert_eq!(a.size(), Size::Tiny);
        // Not a Beast — the type is what keeps it off every
        // beast-scoped list in the game.
        assert_eq!(a.creature_type(), CreatureType::Monstrosity);
        assert!(a.find_action("flying snake bite").is_some());
    }

    #[test]
    fn flying_snake_is_a_flyby_harasser() {
        let a = make();
        assert!(a.base_fly_speed() >= 60.0);
        assert!(a.has_passive_feature(FLYBY_TAG));
    }
}
