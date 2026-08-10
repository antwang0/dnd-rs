use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::RAVEN_BEAK;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, Skill};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Raven — CR 0 tiny beast. One hit point of beak on fifty feet of
/// flight, and the roster's smallest thing that can still take a turn.
///
/// **Mimicry** — RAW: "the raven can mimic simple sounds it has heard"
/// — has no combat surface and is not carried. What is left is the
/// familiar: a wizard's raven, a druid's Wild Shape scout, and the
/// creature Conjure Animals produces eight of.
///
/// Stat shape per the SRD: AC 12, 1 HP (1d4-1), STR 2 / DEX 14 / CON 8
/// / INT 2 / WIS 12 / CHA 6. Speed 50 (flying). Skills: Perception.
/// CR 0.
pub static RAVEN_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&RAVEN_BEAK);
    CreatureTemplate {
        name: "Raven",
        // 'v' (lowercase) — free; 'r' is the rug's and 'R' the Roper /
        // Remorhaz band.
        glyph: 'v',
        ac: 12,
        // 1d4-1 ≈ 1 average per the SRD (CR 0). One hit point is the
        // floor and the die can roll it.
        hitpoints: "1d4-1".parse().unwrap(),
        speed: 50.,
        strength: 2,
        dexterity: 14,
        constitution: 8,
        intelligence: 2,
        wisdom: 12,
        charisma: 6,
        skills: HashSet::from([Skill::Perception]),
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

    /// A `1d4-1` hit point pool can roll zero, and a creature that
    /// arrives at zero hit points is one the engine has to not choke
    /// on. Swept across seeds rather than asserted once, because the
    /// interesting roll is the rare one.
    #[test]
    fn a_raven_always_arrives_with_at_least_one_hit_point() {
        for seed in 0..32u64 {
            let a = ActorInstance::from_creature_template(
                &RAVEN_TEMPLATE,
                Coordinate::new(0, 0),
                1,
                &mut FastRandRoller::with_seed(seed),
                0,
            )
            .unwrap();
            assert_eq!(a.cr(), 0.0);
            assert!(
                a.hitpoints() >= 1,
                "seed {}: a raven rolled {} hit points",
                seed,
                a.hitpoints()
            );
            assert!(a.find_action("raven beak").is_some());
        }
    }
}
