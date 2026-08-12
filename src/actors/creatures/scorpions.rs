use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::SCORPION_STING;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Scorpion — CR 0 tiny beast, and the small end of the ladder below
/// the Giant Scorpion (CR 3).
///
/// Action lane:
/// - **scorpion sting** — flat 1 piercing plus a flat 1d6 poison
///   rider. The venom averages three and a half times the puncture,
///   which is the scorpion in one line: the sting is not the weapon.
///
/// The rider lands on a hit with no save, which is what SRD 5.2 prints
/// for this stat block. The save-gated venom belongs to the giant
/// scorpion, and the engine models that one separately — the two are
/// genuinely different clauses rather than the same clause at two
/// scales, so they use two different weapon chassis.
///
/// Stat shape: AC 11, ~1 HP (1d4−1, floored at 1), STR 2, DEX 11,
/// CON 8, INT 1, WIS 8, CHA 2. Speed 10. Senses Blindsight 10. Size
/// Tiny. CR 0. XP 10 per RAW.
pub static SCORPION_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&SCORPION_STING);
    CreatureTemplate {
        name: "Scorpion",
        // 'x' — the segmented low silhouette. 'S' is the giant
        // scorpion's cohort and every other 's' on the shelf is taken.
        glyph: 'x',
        ac: 11,
        hitpoints: "1d4-1".parse().unwrap(),
        speed: 10.,
        strength: 2,
        intelligence: 1,
        dexterity: 11,
        wisdom: 8,
        constitution: 8,
        charisma: 2,
        senses: HashSet::from([SpecialSense::Blindsight(10)]),
        cr: 0.0,
        size: Size::Tiny,
        creature_type: CreatureType::Beast,
        actions,
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
            &SCORPION_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn scorpion_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 0.0);
        assert_eq!(a.size(), Size::Tiny);
        assert_eq!(a.creature_type(), CreatureType::Beast);
        assert!(a.find_action("scorpion sting").is_some());
    }
}
