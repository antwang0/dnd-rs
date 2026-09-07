use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{ALLOSAURUS_BITE, ALLOSAURUS_CLAWS, ALLOSAURUS_POUNCE};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, Skill};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Allosaurus — CR 2 large beast, and the fastest predator on the
/// dinosaur shelf at sixty feet.
///
/// It has no multiattack. What it has instead is a **pounce** on a
/// thirty-foot run — half again the distance every other charge in the
/// bestiary asks for, which a sixty-foot speed makes cheap — that
/// knocks the target down and hands the allosaurus a free bite. The
/// claws are the trigger and the bite is the payoff, so a turn that
/// opens with a run is 1d8+4 slashing plus 2d10+4 piercing, and a turn
/// that opens in melee is one or the other.
///
/// That is the entire tactical identity: an allosaurus wants to be
/// thirty feet away at the top of its turn, which is a thing it can
/// arrange and a party cannot easily prevent on open ground.
///
/// Action lanes:
/// - **allosaurus claws** — STR-based 1d8+STR slashing, and the limb
///   the pounce rides.
/// - **allosaurus bite** — STR-based 2d10+STR piercing, both as a
///   standalone Action and as the pounce's follow-up.
///
/// RAW gates the pounce on the target being Large or smaller — that is
/// `ChargeRider::max_target_size` — and knocks it prone outright, where
/// the engine's chassis rolls a Strength save at the allosaurus's
/// derived DC, the convention every charge on the roster follows.
///
/// Stat shape: AC 13, ~51 HP (6d10+18), STR 19, DEX 13, CON 17, INT 2,
/// WIS 12, CHA 5. Speed 60. Skills Perception. Size Large. CR 2.
/// XP 450 per RAW.
pub static ALLOSAURUS_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&ALLOSAURUS_CLAWS);
    actions.push(&ALLOSAURUS_BITE);
    CreatureTemplate {
        name: "Allosaurus",
        // 'L' — the long-tailed uppercase theropod, beside the
        // tyrannosaurus on the dinosaur shelf.
        glyph: 'L',
        ac: 13,
        hitpoints: "6d10+18".parse().unwrap(),
        speed: 60.,
        strength: 19,
        intelligence: 2,
        dexterity: 13,
        wisdom: 12,
        constitution: 17,
        charisma: 5,
        skills: HashSet::from([Skill::Perception]),
        cr: 2.0,
        size: Size::Large,
        creature_type: CreatureType::Beast,
        actions,
        charge: Some(ALLOSAURUS_POUNCE),
        ..CreatureTemplate::defaults()
    }
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::actor_template::ActorInstance;
    use crate::engine::attack::charge_run_tiles;
    use crate::engine::dice::FastRandRoller;
    use crate::engine::types::Coordinate;

    fn make() -> ActorInstance {
        ActorInstance::from_creature_template(
            &ALLOSAURUS_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn allosaurus_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 2.0);
        assert_eq!(a.size(), Size::Large);
        assert_eq!(a.creature_type(), CreatureType::Beast);
        assert!(a.find_action("allosaurus claws").is_some());
        assert!(a.find_action("allosaurus bite").is_some());
    }

    /// Thirty feet of run-up, not the usual twenty, and a follow-up
    /// the allosaurus actually carries.
    ///
    /// The longer run is the one number that distinguishes this pounce
    /// from every other one in the bestiary, and it is the sort of
    /// thing a copy-paste of a neighbouring rider would quietly
    /// normalise back to twenty.
    #[test]
    fn the_pounce_asks_for_a_longer_run_than_the_rest() {
        let a = make();
        let pounce = a.charge().expect("the allosaurus pounces");
        assert_eq!(pounce.run_tiles, charge_run_tiles(30));
        let follow_up = pounce.prone_follow_up.expect("and then bites");
        assert!(a.find_action(follow_up).is_some());
    }
}
