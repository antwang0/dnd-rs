use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::EAGLE_TALONS;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, Skill};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Eagle — CR 0 small beast. The middle rung of the raptor ladder: a
/// real 1d4+DEX talon where the Hawk has a flat 1, and a fraction of
/// the Giant Eagle (CR 1) that can carry a rider.
///
/// Action lane:
/// - **eagle talons** — DEX-based 1d4+DEX slashing.
///
/// **Keen Sight** (RAW: advantage on sight-based Perception) is
/// flavor-only; the engine surfaces no sight checks in combat, so the
/// clause collapses into the Perception proficiency the template
/// already carries.
///
/// Defensive identity: AC 12, ~4 HP (1d6+1), fly 60 and no Flyby. The
/// eagle's tactical contribution is reach in three dimensions and a
/// passive Perception of 16 — it finds things and it hovers over them.
///
/// Stat shape: AC 12, ~4 HP, STR 6, DEX 15, CON 12, INT 2, WIS 14,
/// CHA 7. Speed 10 walking, fly 60. Skills Perception. Size Small.
/// CR 0. XP 10 per RAW.
pub static EAGLE_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&EAGLE_TALONS);
    CreatureTemplate {
        name: "Eagle",
        // 'e' (lowercase) — the small raptor below the Giant Eagle's
        // uppercase silhouette.
        glyph: 'e',
        ac: 12,
        hitpoints: "1d6+1".parse().unwrap(),
        speed: 10.,
        fly_speed: 60.,
        strength: 6,
        intelligence: 2,
        dexterity: 15,
        wisdom: 14,
        constitution: 12,
        charisma: 7,
        skills: HashSet::from([Skill::Perception]),
        cr: 0.0,
        size: Size::Small,
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
            &EAGLE_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn eagle_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 0.0);
        assert_eq!(a.size(), Size::Small);
        assert_eq!(a.creature_type(), CreatureType::Beast);
        assert!(a.find_action("eagle talons").is_some());
    }

    /// The eagle's whole tactical identity is the sixty feet of flight.
    #[test]
    fn eagle_is_a_flier() {
        assert!(make().base_fly_speed() >= 60.0);
    }
}
