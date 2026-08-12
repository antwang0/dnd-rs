use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{APE_FIST, APE_MULTI, APE_ROCK};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, Skill};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Ape — CR ½ medium beast. The roster's smallest creature with a
/// *ranged* option, and the whole reason it is interesting: every other
/// beast at this tier has to close, and the ape can open a fight from
/// fifty feet with a rock and then close anyway.
///
/// Action lanes:
/// - **ape multiattack** — two fists per Action (1d4+STR bludgeoning
///   each) via the shared `Multiattack` chassis. The reliable lane.
/// - **ape rock** — a 2d6+STR bludgeoning throw at 25/50 ft, gated on
///   RAW's **Recharge 6**. Twice the damage of a single fist and about
///   the same as both, so the ape's best turn is the one where the rock
///   is up and the target is far enough away that the fists aren't.
///
/// The rock is the roster's first `RechargingAttack`, and the recharge
/// key `"rock"` is spelled in two places that have to agree: the
/// action's `recharge_key` and the `recharge_abilities` row below. The
/// pairing fails closed — a typo reads as "the ape never throws" rather
/// than as a rock that is always available.
///
/// **Climb 30** is not modeled; the engine's board is flat, so the
/// climb speed collapses into the walking speed the ape already has.
///
/// Defensive identity: AC 12, ~19 HP (3d8+6). No resistances, no
/// condition immunities — a vanilla beast envelope on a frame that dies
/// to two solid hits. The ape is a damage profile, not a wall.
///
/// Stat shape: AC 12, ~19 HP, STR 16, DEX 14, CON 14, INT 6, WIS 12,
/// CHA 7. Speed 30. Skills Athletics, Perception. Size Medium. CR ½.
/// XP 100 per RAW.
pub static APE_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&APE_FIST);
    actions.push(&*APE_MULTI);
    actions.push(&*APE_ROCK);
    CreatureTemplate {
        name: "Ape",
        // 'A' — the primate silhouette, shared with the rest of the
        // uppercase-A cohort at the small UI scale.
        glyph: 'A',
        ac: 12,
        // 3d8+6 ≈ 19 average per RAW (CR ½).
        hitpoints: "3d8+6".parse().unwrap(),
        speed: 30.,
        strength: 16,
        intelligence: 6,
        dexterity: 14,
        wisdom: 12,
        constitution: 14,
        charisma: 7,
        skills: HashSet::from([Skill::Athletics, Skill::Perception]),
        cr: 0.5,
        size: Size::Medium,
        creature_type: CreatureType::Beast,
        actions,
        // RAW **Recharge 6**: spent on use, rolled back at the start of
        // each of the ape's turns, restored on a 6. The key has to match
        // `APE_ROCK`'s `recharge_key` exactly — see the docstring above.
        recharge_abilities: vec![("rock", 6)],
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
            &APE_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn ape_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 0.5);
        assert_eq!(a.size(), Size::Medium);
        assert_eq!(a.creature_type(), CreatureType::Beast);
        assert!(a.find_action("ape multiattack").is_some());
        assert!(a.find_action("ape rock").is_some());
    }

    /// The rock's recharge key has to be a key the template declares,
    /// or the ape carries an action it can never use.
    ///
    /// This is the failure mode `RechargingAttack`'s docstring warns
    /// about, and it is silent: a key with no matching row answers
    /// "unavailable" forever, so the symptom is an ape that only ever
    /// punches. Pinned here rather than trusted to a reading of two
    /// files that have no reason to be read together.
    #[test]
    fn the_apes_rock_is_a_recharge_the_ape_actually_has() {
        let a = make();
        assert!(
            a.is_recharge_available("rock"),
            "the ape should start with its rock in hand"
        );
    }
}
