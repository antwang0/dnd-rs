use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::PTERANODON_BITE;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Pteranodon — CR ¼ medium beast. The flying reptilian gull: a
/// pre-historic gliding hunter with a long beaked head. Slots into the
/// low-CR beast bench alongside Stirges / Wolves / Boars / Constrictor
/// Snakes / Hyenas. Completes the dinosaur cohort (Pteranodon ¼,
/// Plesiosaurus 2, Triceratops 5, Tyrannosaurus 8) as the entry-level
/// flying-dino swarmer.
///
/// Action lanes:
/// - **pteranodon bite** — STR-based 2d4+STR piercing melee. Vanilla
///   `SimpleWeapon::melee` — no rider. The pteranodon's threat lives
///   on mobility (its high speed lets it close on stragglers), not
///   damage soak or control.
///
/// Defensive identity: AC 13 (the gliding silhouette is hard to
/// pin down), ~13 HP (3d8). Standard beast envelope — no special
/// resistances or condition immunities. The pteranodon is a swarm-tier
/// nuisance: it tags soft targets and disengages.
///
/// Stat shape: AC 13, ~13 HP (3d8), STR 12, DEX 15, CON 10, INT 2,
/// WIS 9, CHA 5. Speed 60 (RAW: 10ft walk + 60ft fly — we collapse to
/// the flight speed since the engine treats movement as a single tile-
/// grid lane and the fly speed is the load-bearing kiting / closing
/// advantage). Size Medium. CR ¼.
///
/// Flyby (RAW: opportunity attacks don't trigger when the pteranodon
/// flies out of an enemy's reach) is omitted as a deliberate scope cut
/// — the engine has no per-trait "ignore opportunity attacks on
/// movement" hook for non-disengaging actors, and the load-bearing
/// combat clause for a CR-¼ swarmer is the high speed + cheap bite.
pub static PTERANODON_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&PTERANODON_BITE);
    CreatureTemplate {
        name: "Pteranodon",
        // 'v' (lowercase) — distinct from 'V' (Vrock / Vampire /
        // Vampire Spawn / Veteran) and from other lowercase animals.
        // 'v' reads as the wide-winged silhouette of a gliding flier;
        // free in the glyph map at this size and visually pairs with
        // 'V' for any future capital-V swooping apex.
        glyph: 'v',
        ac: 13,
        // 3d8 ≈ 13 average per MM (CR ¼).
        hitpoints: "3d8".parse().unwrap(),
        // RAW speed line: Speed 10 ft., fly 60 ft.
        speed: 10.0,
        fly_speed: 60.0,
        strength: 12,
        intelligence: 2,
        dexterity: 15,
        wisdom: 9,
        constitution: 10,
        charisma: 5,
        senses: HashSet::new(),
        languages: HashSet::new(),
        cr: 0.25,
        size: Size::Medium,
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
            &PTERANODON_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn pteranodon_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 0.25);
        assert_eq!(a.size(), Size::Medium);
        assert_eq!(a.creature_type(), CreatureType::Beast);
        assert!(a.find_action("pteranodon bite").is_some());
    }

    #[test]
    fn pteranodon_has_fly_speed_baked_in() {
        // Pin the 60 ft (flight-derived) speed signature — the
        // pteranodon's combat identity is "fast skirmisher". RAW splits
        // a low walk + high fly; we collapse to the fly speed because
        // the engine treats movement as one tile-grid lane. A future
        // refactor that drops the speed to the walking value (10) would
        // silently remove the load-bearing kiting advantage.
        let a = make();
        assert!(a.speed() >= 60.);
    }
}
