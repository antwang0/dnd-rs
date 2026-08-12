use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{ANKYLOSAURUS_MULTI, ANKYLOSAURUS_TAIL};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{AbilityScoreType, CreatureType, Size};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Ankylosaurus — CR 3 huge beast. AC 15 and 68 hit points on a
/// creature that swings a club-tailed reach-10 weapon twice a turn,
/// each swing carrying a knockdown.
///
/// It does not out-damage the dinosaurs around it and is not trying to.
/// Two chances a turn to put something on the floor at ten feet of
/// reach is a control profile, not a damage one — a prone target is
/// swinging at disadvantage, is being swung at with advantage, and
/// spends half its movement standing up into a tail that is still
/// there. The ankylosaurus is the shelf's answer to a melee character
/// who wanted to stand still.
///
/// Action lanes:
/// - **ankylosaurus multiattack** — two tail swings per Action.
/// - **ankylosaurus tail** — STR-based 1d10+STR bludgeoning at reach 2
///   (10 ft) with a DC-14 Strength save or Prone.
///
/// RAW's knockdown is automatic against a Huge-or-smaller target; the
/// engine's `WeaponWithSaveCondition` chassis gates it behind a save at
/// the ankylosaurus's own derived DC of 14, which is the convention
/// every other knockdown weapon on the roster uses.
///
/// Stat shape: AC 15, ~68 HP (8d12+16), STR 19, DEX 11, CON 15, INT 2,
/// WIS 12, CHA 5. Speed 30. Saves STR. Size Huge. CR 3. XP 700 per RAW.
pub static ANKYLOSAURUS_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&ANKYLOSAURUS_TAIL);
    actions.push(&*ANKYLOSAURUS_MULTI);
    CreatureTemplate {
        name: "Ankylosaurus",
        // 'N' — free on the uppercase shelf and reads as the broad
        // armoured back; 'A' is the ape cohort and 'L' the allosaurus.
        glyph: 'N',
        ac: 15,
        hitpoints: "8d12+16".parse().unwrap(),
        speed: 30.,
        strength: 19,
        intelligence: 2,
        dexterity: 11,
        wisdom: 12,
        constitution: 15,
        charisma: 5,
        cr: 3.0,
        size: Size::Huge,
        creature_type: CreatureType::Beast,
        actions,
        proficient_saves: HashSet::from([AbilityScoreType::Strength]),
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
            &ANKYLOSAURUS_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn ankylosaurus_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 3.0);
        assert_eq!(a.size(), Size::Huge);
        assert_eq!(a.creature_type(), CreatureType::Beast);
        assert!(a.find_action("ankylosaurus multiattack").is_some());
    }

    /// Reach 10 and a knockdown on every swing — the two properties
    /// that make this a control stat block rather than a damage one.
    #[test]
    fn the_tail_knocks_down_from_ten_feet() {
        use crate::actions::monster_attacks::ANKYLOSAURUS_TAIL;
        use crate::conditions::Condition;
        assert_eq!(ANKYLOSAURUS_TAIL.reach, 2);
        assert_eq!(ANKYLOSAURUS_TAIL.condition, Condition::Prone);
    }
}
