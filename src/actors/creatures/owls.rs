use crate::actions::class_features::FLYBY_TAG;
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::OWL_TALONS;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, Skill, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Owl — CR 0 tiny beast. One hit point, one point of damage, a hundred
/// and twenty feet of darkvision, and **Flyby**. It is the cheapest
/// creature in the game that can look at something in the dark and
/// leave without being swung at.
///
/// Action lane:
/// - **owl talons** — flat 1 slashing on a DEX-based roll. Present
///   because RAW prints it, not because anybody will use it.
///
/// **Flyby** is the load-bearing line, and the owl is where it is most
/// clearly not decorative: with one hit point, the difference between
/// provoking and not provoking on the way out is the difference between
/// a scout that survives a pass and one that does not. Gated on
/// `is_airborne` at the point of use — an owl the engine has put on the
/// ground is walking, and walking away from a halberd provokes.
///
/// Stat shape: AC 11, ~1 HP (1d4−1, floored at 1), STR 3, DEX 13,
/// CON 8, INT 2, WIS 12, CHA 7. Speed 5 walking, fly 60. Skills
/// Perception, Stealth. Senses Darkvision 120. Size Tiny. CR 0. XP 10
/// per RAW.
pub static OWL_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&OWL_TALONS);
    CreatureTemplate {
        name: "Owl",
        // 'w' — the tiny night flier. 'o' is the octopus and 'O' the
        // Giant Owl; 'w' reads as the round-headed silhouette and is
        // free on the lowercase shelf.
        glyph: 'w',
        ac: 11,
        hitpoints: "1d4-1".parse().unwrap(),
        speed: 5.,
        fly_speed: 60.,
        strength: 3,
        intelligence: 2,
        dexterity: 13,
        wisdom: 12,
        constitution: 8,
        charisma: 7,
        skills: HashSet::from([Skill::Perception, Skill::Stealth]),
        // The longest darkvision on the CR-0 shelf by a factor of two,
        // and the reason to field an owl at all under `AmbientLight::Dark`.
        senses: HashSet::from([SpecialSense::Darkvision(120)]),
        cr: 0.0,
        size: Size::Tiny,
        creature_type: CreatureType::Beast,
        actions,
        features: HashSet::from([FLYBY_TAG]),
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
            &OWL_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn owl_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 0.0);
        assert_eq!(a.size(), Size::Tiny);
        assert_eq!(a.creature_type(), CreatureType::Beast);
        assert!(a.find_action("owl talons").is_some());
    }

    /// Flyby is gated on flight, and an owl that has been grounded is
    /// walking.
    ///
    /// The mirror of the deer's test one file over: same cohort, same
    /// predicate, opposite answer once the wings stop — which is the
    /// whole reason Agile and Flyby are two tags rather than one read
    /// twice. `Earthbound` is Earthbind's "the target's flying speed
    /// becomes 0 feet", the cleanest way to take an owl's flight away
    /// without also taking away its ability to act.
    #[test]
    fn an_owl_leaves_for_free_only_while_it_is_flying() {
        use crate::conditions::{Condition, ConditionTimer};

        let mut a = make();
        assert!(a.is_airborne());
        assert!(a.suppresses_opportunity_attacks());

        a.add_condition(Condition::Earthbound, ConditionTimer::Permanent);
        assert!(!a.is_airborne());
        assert!(
            !a.suppresses_opportunity_attacks(),
            "a grounded owl is walking, and walking out of reach provokes"
        );
    }
}
