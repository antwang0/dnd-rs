use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{
    ELEPHANT_CHARGE, ELEPHANT_GORE, ELEPHANT_MULTI, ELEPHANT_TRAMPLE,
};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size};
use std::sync::LazyLock;

/// Elephant — CR 4 huge beast, and the roster's clearest example of a
/// creature whose damage lives in a *sequence* rather than in a number.
///
/// The three lanes only add up in one order:
///   1. Move twenty feet in a straight line and gore. The **charge**
///      rider adds no damage — it puts the target on the floor.
///   2. The knockdown immediately earns **trample** as a bonus action,
///      which is gated on the target being Prone and would refuse to
///      fire a moment earlier.
///   3. The second gore of the **multiattack** then lands against a
///      prone target, which is advantage.
///
/// Standing still, an elephant is two 2d8+6 gores — respectable and
/// nothing more. Coming off a run it is two gores, a 2d10+6 stomp, and
/// advantage on half of it. Room to move is the elephant's whole stat
/// block, and a corridor is the counterplay.
///
/// Action lanes:
/// - **elephant multiattack** — two gores per Action.
/// - **elephant gore** — the single swing, and the limb the charge
///   rider names.
/// - **elephant trample** — a bonus-action 2d10+STR bludgeoning stomp,
///   admitted only against a Prone target via the shared
///   `ProneOnlyAttack` gate.
///
/// RAW writes Trample as a DC-16 Dexterity save for half rather than as
/// an attack roll; the collapse is documented at
/// `ELEPHANT_TRAMPLE_STOMP` and matches how the mammoth's stomp has
/// always resolved.
///
/// Stat shape: AC 12, ~76 HP (8d12+24), STR 22, DEX 9, CON 17, INT 3,
/// WIS 11, CHA 6. Speed 40. Size Huge. CR 4. XP 1,100 per RAW.
pub static ELEPHANT_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&ELEPHANT_GORE);
    actions.push(&*ELEPHANT_MULTI);
    actions.push(&*ELEPHANT_TRAMPLE);
    CreatureTemplate {
        name: "Elephant",
        // 'P' — the uppercase pachyderm. 'E' is the giant elk and 'e'
        // the eagle; 'P' is free and reads as the heavy silhouette.
        glyph: 'P',
        ac: 12,
        hitpoints: "8d12+24".parse().unwrap(),
        speed: 40.,
        strength: 22,
        intelligence: 3,
        dexterity: 9,
        wisdom: 11,
        constitution: 17,
        charisma: 6,
        cr: 4.0,
        size: Size::Huge,
        creature_type: CreatureType::Beast,
        actions,
        charge: Some(ELEPHANT_CHARGE),
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
            &ELEPHANT_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn elephant_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 4.0);
        assert_eq!(a.size(), Size::Huge);
        assert_eq!(a.creature_type(), CreatureType::Beast);
        assert!(a.find_action("elephant multiattack").is_some());
        assert!(a.find_action("elephant trample").is_some());
    }

    /// The charge names the trample, and the trample is on the list.
    ///
    /// `ChargeRider::prone_follow_up` is matched by `find_action`, so a
    /// renamed action fails closed as "the elephant has no such attack"
    /// — silently, with the charge still knocking targets down and the
    /// stomp never arriving. That is the whole damage spike going
    /// missing with nothing to show for it, which is why the link is
    /// pinned from both ends here.
    #[test]
    fn the_charge_hands_off_to_an_attack_the_elephant_has() {
        let a = make();
        let follow_up = a
            .charge()
            .expect("the elephant charges")
            .prone_follow_up
            .expect("and the charge earns a trample");
        assert_eq!(follow_up, "elephant trample");
        assert!(a.find_action(follow_up).is_some());
    }
}
