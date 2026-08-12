use crate::actions::class_features::SWIM_SPEED_TAG;
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{HIPPOPOTAMUS_BITE, HIPPOPOTAMUS_MULTI};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{AbilityScoreType, CreatureType, Size, Skill};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Hippopotamus — CR 4 large beast. Two 2d10+5 bites a turn off an
/// 82-hit-point frame, with no rider, no gate, and no clause of any
/// kind. It is the plainest heavy hitter on the roster and among the
/// hardest-hitting things at its challenge rating, which is the joke
/// the stat block is making: the animal people are actually killed by
/// is not the one with the interesting abilities.
///
/// Action lanes:
/// - **hippopotamus multiattack** — two bites per Action, averaging
///   thirty-two damage before any modifiers.
/// - **hippopotamus bite** — the single swing.
///
/// **Hold Breath** (RAW: ten minutes) collapses to flavor — the engine
/// counts rounds, and ten minutes is a hundred of them. The swim speed
/// is the half that lands, and it matters: a hippo in water crosses it
/// free and swings without the underwater melee penalty, which on a
/// river map is where the whole fight is.
///
/// Stat shape: AC 14, ~82 HP (11d10+22), STR 21, DEX 7, CON 15, INT 2,
/// WIS 12, CHA 4. Speed 30 (walk and swim alike). Skills Perception.
/// Saves STR. Size Large. CR 4. XP 1,100 per RAW.
pub static HIPPOPOTAMUS_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&HIPPOPOTAMUS_BITE);
    actions.push(&*HIPPOPOTAMUS_MULTI);
    CreatureTemplate {
        name: "Hippopotamus",
        // 'J' — free on the uppercase shelf; 'H' is the crowded
        // Harpy / Hippogriff / Hobgoblin / Hydra cohort and the hippo
        // deserves not to be mistaken for a hippogriff.
        glyph: 'J',
        ac: 14,
        hitpoints: "11d10+22".parse().unwrap(),
        speed: 30.,
        strength: 21,
        intelligence: 2,
        dexterity: 7,
        wisdom: 12,
        constitution: 15,
        charisma: 4,
        skills: HashSet::from([Skill::Perception]),
        cr: 4.0,
        size: Size::Large,
        creature_type: CreatureType::Beast,
        actions,
        proficient_saves: HashSet::from([AbilityScoreType::Strength]),
        features: HashSet::from([SWIM_SPEED_TAG]),
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
            &HIPPOPOTAMUS_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn hippopotamus_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 4.0);
        assert_eq!(a.size(), Size::Large);
        assert_eq!(a.creature_type(), CreatureType::Beast);
        assert!(a.find_action("hippopotamus multiattack").is_some());
        assert!(a.has_swim_speed());
    }
}
