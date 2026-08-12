use crate::actions::class_features::AGILE_TAG;
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::DEER_RAM;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, Skill, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Deer — CR 0 medium beast, and the roster's first carrier of the
/// **Agile** trait: *"the deer doesn't provoke an Opportunity Attack
/// when it moves out of an enemy's reach."*
///
/// That clause is the entire creature. A deer does two points of damage
/// on a hit and dies to one, so nothing about a fight with a deer is
/// about the fight — it is about whether anybody can catch it. Fifty
/// feet of speed and free disengagement mean the answer, on an open
/// board, is no.
///
/// Agile is Flyby's ground-bound twin, and it is a separate tag for a
/// reason worth stating: Flyby is gated on *"when it flies"* and drops
/// the moment its holder is walking, and Agile has no such gate. See
/// `AGILE_TAG` and `MOVER_OA_SUPPRESSORS`.
///
/// Action lane:
/// - **deer ram** — STR-based 1d4+STR bludgeoning. Strength 11 makes
///   the modifier zero, so RAW's printed `2 (1d4)` and an ordinary STR
///   swing are the same number.
///
/// Stat shape: AC 13, ~4 HP (1d8), STR 11, DEX 16, CON 11, INT 2,
/// WIS 14, CHA 5. Speed 50. Skills Perception. Senses Darkvision 60.
/// Size Medium. CR 0. XP 10 per RAW.
pub static DEER_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&DEER_RAM);
    CreatureTemplate {
        name: "Deer",
        // 'd' — the lowercase grazer, beside the goat's and the elk's
        // silhouettes on the herbivore shelf.
        glyph: 'd',
        ac: 13,
        hitpoints: "1d8".parse().unwrap(),
        speed: 50.,
        strength: 11,
        intelligence: 2,
        dexterity: 16,
        wisdom: 14,
        constitution: 11,
        charisma: 5,
        skills: HashSet::from([Skill::Perception]),
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        cr: 0.0,
        size: Size::Medium,
        creature_type: CreatureType::Beast,
        actions,
        // RAW **Agile** — read by `dispatch_opportunity_attacks` through
        // the mover-side blanket-suppression cohort.
        features: HashSet::from([AGILE_TAG]),
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
            &DEER_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn deer_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 0.0);
        assert_eq!(a.size(), Size::Medium);
        assert_eq!(a.creature_type(), CreatureType::Beast);
        assert!(a.find_action("deer ram").is_some());
    }

    /// Agile, and — unlike Flyby — with both feet on the ground.
    ///
    /// The distinction is the point of the tag existing at all, so it
    /// is pinned on a creature that cannot fly: a deer standing on the
    /// floor suppresses opportunity attacks, where an owl standing on
    /// the floor does not.
    #[test]
    fn a_deer_on_the_ground_still_leaves_for_free() {
        let a = make();
        assert!(!a.is_airborne());
        assert!(a.suppresses_opportunity_attacks());
    }
}
