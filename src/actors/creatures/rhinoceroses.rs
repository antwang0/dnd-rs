use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{RHINOCEROS_CHARGE, RHINOCEROS_GORE};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size};
use std::sync::LazyLock;

/// Rhinoceros — CR 2 large beast. A single gore, a forty-foot speed,
/// and a **Charge** clause that nearly doubles the hit that comes off a
/// twenty-foot run and puts the target on the floor besides.
///
/// That is the whole creature, and it makes the rhino the cleanest
/// example on the roster of what the charge chassis is for: its damage
/// on a standing start is unremarkable for CR 2, and its damage after a
/// run is the second-highest single hit at its tier. A rhinoceros that
/// is allowed to back up and come again is a different monster from one
/// that is pinned in melee, and nothing about its action list says so.
///
/// Action lane:
/// - **rhinoceros gore** — STR-based 2d8+STR piercing, plus the charge
///   rider when it earns one.
///
/// RAW's charge knocks the target prone outright and gates on the
/// target being Large or smaller. The engine's chassis rolls a Strength
/// save against the rhino's derived DC instead and has no size gate —
/// the convention every other charge in the bestiary follows, so the
/// rhino is not a special case.
///
/// Stat shape: AC 13, ~45 HP (6d10+12), STR 21, DEX 8, CON 15, INT 2,
/// WIS 12, CHA 6. Speed 40. Size Large. CR 2. XP 450 per RAW.
pub static RHINOCEROS_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&RHINOCEROS_GORE);
    CreatureTemplate {
        name: "Rhinoceros",
        // 'R' — the heavy uppercase charger, beside the other large
        // quadrupeds on the board.
        glyph: 'R',
        ac: 13,
        hitpoints: "6d10+12".parse().unwrap(),
        speed: 40.,
        strength: 21,
        intelligence: 2,
        dexterity: 8,
        wisdom: 12,
        constitution: 15,
        charisma: 6,
        cr: 2.0,
        size: Size::Large,
        creature_type: CreatureType::Beast,
        actions,
        charge: Some(RHINOCEROS_CHARGE),
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
            &RHINOCEROS_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn rhinoceros_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 2.0);
        assert_eq!(a.size(), Size::Large);
        assert_eq!(a.creature_type(), CreatureType::Beast);
        assert!(a.find_action("rhinoceros gore").is_some());
    }

    /// The charge is the rhinoceros. A template refactor that dropped
    /// it would leave a CR-2 monster with one below-curve attack and
    /// nothing else, and nothing would fail.
    #[test]
    fn rhinoceros_carries_its_charge() {
        let a = make();
        let charge = a.charge().expect("the rhinoceros charges");
        assert_eq!(charge.weapon, Some("rhinoceros gore"));
        assert!(charge.knocks_prone);
    }
}
