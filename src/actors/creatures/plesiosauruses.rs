use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::PLESIOSAURUS_BITE;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Plesiosaurus — CR 2 large beast. The long-necked aquatic reptilian
/// hunter: a marine dinosaur that lashes its serpentine neck out from
/// reach to snatch prey. Slots into the "huge HP-bar beast" CR-2 bracket
/// alongside the Giant Constrictor Snake (CR 2, reach 2 grappler) and
/// the Giant Toad (CR 2, swallow). Distinct from the snake by being a
/// pure-damage long-reach biter — no rider, no grapple — relying on
/// raw bite damage (3d6+4 ≈ 14) and a fat 68 HP envelope.
///
/// Slots into the dinosaur cohort alongside Triceratops (CR 5) and
/// Tyrannosaurus Rex (CR 8), filling the CR-2 spot under the apex
/// dinos. Pteranodon (CR ¼) fills the flying-dino spot.
///
/// Action lanes:
/// - **plesiosaurus bite** — STR-based 3d6+STR piercing melee at reach
///   2 tiles (10 ft — the long neck lets the plesiosaurus bite from
///   outside normal melee range, mirroring the Giant Constrictor
///   Snake's reach lane). Vanilla `SimpleWeapon::reach_melee` — no
///   rider; the load-bearing combat clause is the per-Action damage
///   budget on a huge HP bar.
///
/// Defensive identity: AC 13 (the plesiosaurus's thick reptilian hide),
/// ~68 HP (8d10+24). Standard beast envelope — no special resistances
/// or condition immunities. The plesiosaurus's threat is its raw HP
/// and per-swing damage, not control or resistance.
///
/// Stat shape: AC 13, ~68 HP (8d10+24), STR 18, DEX 15, CON 16, INT 2,
/// WIS 12, CHA 5. Speed 20 (RAW: 20ft walk + 40ft swim — we collapse
/// to the walking speed since the engine isn't aquatic-aware). Size
/// Large. CR 2.
///
/// Hold Breath (RAW: holds breath for 1 hour) is omitted as a
/// deliberate scope cut — the engine has no aquatic / holding-breath
/// model, and the load-bearing combat clause is the reach-2 heavy bite.
pub static PLESIOSAURUS_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&PLESIOSAURUS_BITE);
    CreatureTemplate {
        name: "Plesiosaurus",
        // 'p' (lowercase) — distinct from 'P' (Pit Fiend / Pegasus /
        // Pixie / Polar Bear / Purple Worm) and from existing 'p' (no
        // other lowercase 'p' creature on the roster). 'p' reads as the
        // small head + long neck silhouette of a plesiosaur at small
        // UI scale; the lowercase form pairs cleanly with 'P' for any
        // future capital-P apex.
        glyph: 'p',
        ac: 13,
        // 8d10+24 ≈ 68 average per MM (CR 2).
        hitpoints: "8d10+24".parse().unwrap(),
        speed: 20.,
        strength: 18,
        intelligence: 2,
        dexterity: 15,
        wisdom: 12,
        constitution: 16,
        charisma: 5,
        senses: HashSet::new(),
        languages: HashSet::new(),
        cr: 2.0,
        size: Size::Large,
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
            &PLESIOSAURUS_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn plesiosaurus_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 2.0);
        assert_eq!(a.size(), Size::Large);
        assert_eq!(a.creature_type(), CreatureType::Beast);
        assert!(a.find_action("plesiosaurus bite").is_some());
    }

    #[test]
    fn plesiosaurus_bite_reach_is_2_tiles() {
        // Pin the reach-2 (10 ft) signature of the long neck. RAW the
        // plesiosaurus's combat identity is "bites from outside normal
        // melee range" — a future refactor that drops the reach to 1
        // would silently remove the load-bearing kiting advantage.
        let a = make();
        let bite = a
            .find_action("plesiosaurus bite")
            .expect("plesiosaurus should carry the bite action");
        assert_eq!(bite.reach_tiles(), Some(2));
    }
}
