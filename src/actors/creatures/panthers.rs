use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{PANTHER_BITE, PANTHER_CLAW, PANTHER_POUNCE};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, Skill};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Panther — CR ¼ medium beast. Fifty feet of speed and a pounce, which
/// between them are the whole animal: the panther is never adjacent
/// until the turn it wants to be, and the turn it wants to be, whatever
/// it lands on ends up on the floor.
///
/// Action lanes:
/// - **panther bite** — 1d6+STR piercing.
/// - **panther claw** — 1d4+STR slashing, and the swing the pounce
///   rides. RAW attaches the knockdown to the claw and not the bite,
///   which is why the two are separate actions rather than a
///   multiattack: the panther's choice of swing is a real choice.
///
/// **Pounce** rides the shared charge lane: twenty feet of straight run
/// into a connecting claw, and the target makes a Strength save or goes
/// prone. Damage-free, unlike the boar's charge — the whole clause is
/// the knockdown, and a prone target hands the panther's next swing
/// advantage and hands the rest of the party's melee the same.
///
/// The lighter sibling of the Tiger (CR 1) and the Saber-Toothed Tiger
/// (CR 2) already on the roster, and the only one of the three at CR ¼:
/// a panther is what a low-level party meets in the jungle before it
/// meets anything with teeth worth the name.
///
/// Stat shape per the SRD: AC 13, 13 HP (3d8), STR 14 / DEX 16 / CON 10
/// / INT 3 / WIS 14 / CHA 7. Speed 50. Skills: Perception, Stealth. CR ¼.
pub static PANTHER_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&PANTHER_CLAW);
    actions.push(&PANTHER_BITE);
    CreatureTemplate {
        name: "Panther",
        // 'f' (lowercase) — the feline band, one rung below the Tiger's
        // and the Lion's, and free.
        glyph: 'f',
        ac: 13,
        // 3d8 ≈ 13 average per the SRD (CR ¼).
        hitpoints: "3d8".parse().unwrap(),
        speed: 50.,
        strength: 14,
        dexterity: 16,
        constitution: 10,
        intelligence: 3,
        wisdom: 14,
        charisma: 7,
        skills: HashSet::from([Skill::Perception, Skill::Stealth]),
        cr: 0.25,
        size: Size::Medium,
        creature_type: CreatureType::Beast,
        actions,
        // Pounce — twenty feet of run into a claw, then a Strength save
        // or prone. Read at the melee attack chokepoint off
        // `ActorInstance::charge`.
        charge: Some(PANTHER_POUNCE),
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
            &PANTHER_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn panther_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 0.25);
        assert_eq!(a.creature_type(), CreatureType::Beast);
        assert!(a.find_action("panther claw").is_some());
        assert!(a.find_action("panther bite").is_some());
    }

    /// The pounce names the claw, and the claw has to exist for the
    /// clause to ever fire. This is the exact join the bestiary-wide
    /// sweep checks; asserting it here as well is what makes the
    /// panther's own file say which of its two swings the clause
    /// belongs to.
    #[test]
    fn the_pounce_rides_the_claw_and_not_the_bite() {
        let a = make();
        let charge = a.charge().expect("pounce");
        assert_eq!(charge.weapon, Some("panther claw"));
        assert!(charge.knocks_prone);
        assert!(a.find_action("panther claw").is_some());
    }
}
