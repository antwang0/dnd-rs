use crate::actions::class_features::{SWIM_SPEED_TAG, UNDERWATER_BREATHING_TAG};
use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::GIANT_OCTOPUS_TENTACLES;
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Giant Octopus — CR 1 large beast. The eight-armed cephalopod ambush
/// predator: a Large-sized squid that strikes from a reach of 15 ft and
/// pins prey in its tentacular grip. Slots between the Constrictor
/// Snake (CR ¼) and the Polar Bear / Giant Crocodile mid-CR pool as
/// the canonical "aquatic Large beast" entry. Pairs thematically with
/// the Giant Constrictor Snake — both are reach-grappler ambushers with
/// the same chassis-driven save-or-Restrained rider; the octopus trades
/// the snake's huge HP bar for an extra tile of reach (3 vs 2) and a
/// stronger lock-down condition (Restrained vs Grappled).
///
/// Action lanes:
/// - **giant octopus tentacles** — STR-based 2d6+STR bludgeoning melee
///   at reach 3 tiles (15 ft) with a DC 16 STR save-or-Restrained rider
///   (10 rounds). Routes through the shared `WeaponWithSaveCondition`
///   chassis (long-reach variant) so the save-or-condition install
///   lives at one chokepoint alongside the Giant Constrictor Snake's
///   Constrict. Restrained subsumes the Grappled clause RAW and adds
///   the attack-disadvantage + advantage-to-attackers + DEX-save-
///   disadvantage envelope that the RAW grapple-then-restrain chain
///   would produce.
///
/// Defensive identity: AC 11 (the octopus is squishy — its threat lives
/// on the lock-down, not damage soak), ~45 HP (7d10+7). Standard beast
/// envelope — no special resistances or condition immunities. The
/// octopus's threat is the long-reach lock-down on a 10-round Restrained
/// timer; once the rider lands, allies can pile on a target that's both
/// unable to move AND eating attack-against advantage.
///
/// Stat shape: AC 11, ~45 HP (7d10+7), STR 17, DEX 13, CON 13, INT 5,
/// WIS 10, CHA 4. Speed 20 (RAW: 10ft walk + 60ft swim — we keep the
/// swimming speed as a flag and collapse the magnitudes to
/// a 20ft walking speed since the engine isn't aquatic-aware; the
/// reach-3 tentacles already give the octopus a kiting advantage from
/// outside normal melee range). Senses: Darkvision 60. Size Large.
/// CR 1.
///
/// Hold Breath / Underwater Camouflage / Water Breathing (RAW) are
/// omitted as deliberate scope cuts — the engine has no aquatic /
/// holding-breath / underwater-only lighting model, and the load-bearing
/// combat clause is the reach-3 grapple lock-down.
pub static GIANT_OCTOPUS_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&GIANT_OCTOPUS_TENTACLES);
    CreatureTemplate {
        name: "Giant Octopus",
        // 'o' (lowercase) — distinct from 'O' (Owlbear / Oni / Otyugh)
        // and 'q' (Quaggoth / Quasit). 'o' reads as the round bulbous
        // mantle of an octopus silhouette at small UI scale; free in
        // the glyph map at this size.
        glyph: 'o',
        ac: 11,
        // 7d10+7 ≈ 45 average per MM (CR 1).
        hitpoints: "7d10+7".parse().unwrap(),
        speed: 20.,
        strength: 17,
        intelligence: 5,
        dexterity: 13,
        wisdom: 10,
        constitution: 13,
        charisma: 4,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        languages: HashSet::new(),
        cr: 1.0,
        size: Size::Large,
        creature_type: CreatureType::Beast,
        actions,
        // RAW swim speed: the tag is what makes `TerrainType::Water`
        // free to cross and lifts the underwater melee penalty.
        features: HashSet::from([SWIM_SPEED_TAG, UNDERWATER_BREATHING_TAG]),
        ..CreatureTemplate::defaults()
    }
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::actor_template::ActorInstance;
    use crate::conditions::Condition;
    use crate::engine::dice::FastRandRoller;
    use crate::engine::types::Coordinate;

    fn make() -> ActorInstance {
        ActorInstance::from_creature_template(
            &GIANT_OCTOPUS_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap()
    }

    #[test]
    fn giant_octopus_template_shape() {
        let a = make();
        assert_eq!(a.cr(), 1.0);
        assert_eq!(a.size(), Size::Large);
        assert_eq!(a.creature_type(), CreatureType::Beast);
        assert!(a.find_action("giant octopus tentacles").is_some());
    }

    #[test]
    fn giant_octopus_carries_no_special_immunities() {
        // The octopus's threat profile lives entirely on the reach-3
        // tentacle rider — no damage resistances, no condition
        // immunities. This test pins the "vanilla beast envelope" so a
        // future refactor doesn't accidentally grant the octopus magic
        // resistance or a condition immunity.
        let a = make();
        assert!(!a.has_magic_resistance());
        assert!(!a.effectively_immune_to_condition(Condition::Grappled));
        assert!(!a.effectively_immune_to_condition(Condition::Restrained));
        assert!(!a.effectively_immune_to_condition(Condition::Poisoned));
    }

    #[test]
    fn giant_octopus_tentacles_reach_is_3_tiles() {
        // Pin the reach-3 (15 ft) signature of the tentacle attack. The
        // octopus's load-bearing combat advantage over the Giant
        // Constrictor Snake (reach 2) is the extra tile of standoff;
        // a future refactor that drops the reach to 2 would silently
        // remove the octopus's distinct combat identity.
        let a = make();
        let tentacles = a
            .find_action("giant octopus tentacles")
            .expect("octopus should carry the tentacles action");
        assert_eq!(tentacles.reach_tiles(), Some(3));
    }
}
