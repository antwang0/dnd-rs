use crate::actions::class_features::{SWIM_SPEED_TAG, UNDERWATER_BREATHING_TAG};
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::OCTOPUS_TENTACLES;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, Skill, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Octopus — CR 0 small beast. The small end of the cephalopod ladder
/// below the Giant Octopus (CR 1), and the roster's best hider: Stealth
/// proficiency on a Dexterity 15 frame, in water, in the dark.
///
/// Action lane:
/// - **octopus tentacles** — flat 1 bludgeoning on a DEX-based roll.
///   Nothing about an octopus is the damage.
///
/// Three RAW traits collapse here, and it is worth being explicit about
/// which and why:
///   - **Compression** ("can move through a space as narrow as 1 inch")
///     has no surface — the engine's board has no gaps narrower than a
///     tile, so there is no space the clause would open.
///   - **Water Breathing** ("can breathe only underwater") is the
///     inverse of the crab's Amphibious and lands the same way: water
///     is terrain here rather than an atmosphere, so nothing suffocates
///     on land.
///   - **Ink Cloud** (a 1/day reaction filling a 5-foot Cube with
///     heavy obscurement and swimming away) is the one with real
///     mechanical content and no home: the engine's obscurement lives
///     on the lighting layer rather than on placeable zones, and a
///     one-tile cloud that also moves its creator is two features
///     rather than one. Left off rather than half-built.
///
/// Stat shape: AC 12, ~3 HP (1d6), STR 4, DEX 15, CON 11, INT 3,
/// WIS 10, CHA 4. Speed 30 (RAW swim 30; the walking 5 is collapsed
/// away, as the octopus does not meaningfully walk). Skills Perception,
/// Stealth. Senses Darkvision 30. Size Small. CR 0. XP 10 per RAW.
pub static OCTOPUS_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&OCTOPUS_TENTACLES);
    CreatureTemplate {
        name: "Octopus",
        // 'o' (lowercase) — the small cephalopod beside the Giant
        // Octopus's uppercase silhouette.
        glyph: 'o',
        ac: 12,
        hitpoints: "1d6".parse().unwrap(),
        speed: 30.,
        strength: 4,
        intelligence: 3,
        dexterity: 15,
        wisdom: 10,
        constitution: 11,
        charisma: 4,
        skills: HashSet::from([Skill::Perception, Skill::Stealth]),
        senses: HashSet::from([SpecialSense::Darkvision(30)]),
        cr: 0.0,
        size: Size::Small,
        creature_type: CreatureType::Beast,
        actions,
        features: HashSet::from([SWIM_SPEED_TAG, UNDERWATER_BREATHING_TAG]),
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
            &OCTOPUS_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn octopus_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 0.0);
        assert_eq!(a.size(), Size::Small);
        assert_eq!(a.creature_type(), CreatureType::Beast);
        assert!(a.find_action("octopus tentacles").is_some());
        assert!(a.has_swim_speed());
    }
}
