use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{MAMMOTH_GORE, MAMMOTH_STOMP, MAMMOTH_TRAMPLING_CHARGE};
use crate::actors::actor_template::CreatureTemplate;
use crate::engine::types::{CreatureType, Size};
use std::sync::LazyLock;

/// Mammoth — CR 6 huge beast. The ice-age elephant — a tusked,
/// shaggy-coated quadruped whose signature combat clause is the
/// **Trampling Charge** opener: gore the target, knock them Prone, then
/// follow up with a heavy **Stomp** that only validates against prone
/// targets. Fills the apex huge-beast slot in the CR-6 bench between
/// the Cyclops (CR 6 giant) and the Roc (CR 11 huge beast) on the
/// non-dragon brute ladder.
///
/// Action lanes:
/// - **mammoth gore** — STR-based 4d8+STR piercing melee, reach 1
///   (5 ft). The vanilla swing the AI falls back to when the
///   Trampling Charge recharge isn't available. Averages ~25 per
///   swing on the CR-6 huge frame.
/// - **trampling charge** — Recharge 5-6 gore that, on a hit, lands a
///   DC 18 STR save-or-Prone rider via the standard
///   `save_or_condition_rider` chassis. RAW: "If the mammoth moves at
///   least 20 ft straight toward a target and then hits it with a gore
///   attack on the same turn..." — we collapse the "moved 20 ft
///   straight" gate to a recharge-style validate since the engine
///   can't introspect path geometry at attack time.
/// - **mammoth stomp** — STR-based 4d10+STR bludgeoning melee, gated
///   on the target having the Prone condition (RAW: "The mammoth can
///   only use this attack against a creature that is prone"). Routes
///   through `MammothStomp::custom_validate_input` reading the
///   target's condition set. Pairs with the Trampling Charge: turn
///   one charge knocks the target prone, turn two stomp lands at the
///   higher damage die without consuming the recharge resource.
///
/// Defensive identity: AC 13 (natural armor — the shaggy hide of an
/// arctic beast), 126 HP (11d12+55 ≈ 126). No special resistance or
/// condition envelope — the mammoth is a pure brute, not an arcane
/// holdover or a tamed magical predator. Distinct from the Roc (CR 11
/// huge beast) which trades damage for flight; the mammoth is the
/// ground-bound damage-king on the same Huge frame.
///
/// Stat shape: AC 13, ~126 HP (11d12+55), STR 24, DEX 9, CON 21,
/// INT 3, WIS 11, CHA 6. Speed 40. Senses: none beyond the default
/// (mammoths have no darkvision RAW). Languages: none. Size Huge. CR 6.
pub static MAMMOTH_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&MAMMOTH_GORE);
    actions.push(&*MAMMOTH_STOMP);
    actions.push(&*MAMMOTH_TRAMPLING_CHARGE);
    CreatureTemplate {
        name: "Mammoth",
        // 'M' (uppercase) — distinct mnemonic for Mammoth. 'm' is taken
        // by Mage; uppercase M is currently used by Manticore but only
        // one of the two appears in any given encounter glyph-map at a
        // time, and the silhouette (broad tusked head) reads cleanly
        // for both flying-monstrosity and tusked-quadruped at the
        // Large/Huge size category. If glyph collisions become a
        // problem, swap to 'Y' (untaken) — the Y-shape of tusks +
        // trunk is a strong fallback mnemonic.
        glyph: 'M',
        ac: 13,
        // 11d12+55 ≈ 126 average per MM (CR 6).
        hitpoints: "11d12+55".parse().unwrap(),
        speed: 40.,
        strength: 24,
        intelligence: 3,
        dexterity: 9,
        wisdom: 11,
        constitution: 21,
        charisma: 6,
        cr: 6.0,
        size: Size::Huge,
        creature_type: CreatureType::Beast,
        actions,
        // Trampling Charge recharges on a d6 roll of 5+. Mirrors the
        // dragon breath / dao stone snare gate via the encounter's
        // start-of-turn recharge hook flipping the resource back on
        // automatically. Higher threshold than the dao stone snare's
        // 5+ because the rider's Prone install pairs with the heavy
        // standalone Stomp follow-up — getting the charge once per
        // fight is a strong opener; once-per-other-round would let
        // the mammoth juggle a target Prone every other turn.
        recharge_abilities: vec![("trampling_charge", 5)],
        ..CreatureTemplate::defaults()
    }
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::actor_template::ActorInstance;
    use crate::conditions::{Condition, ConditionTimer};
    use crate::engine::actor_gen::ActorGenParams;
    use crate::engine::dice::FastRandRoller;
    use crate::engine::encounter::EncounterInstance;
    use crate::engine::terrain_gen::TerrainGenParams;
    use crate::engine::types::Coordinate;

    fn empty_encounter() -> EncounterInstance {
        let tp = TerrainGenParams {
            width: 20,
            height: 20,
            branch_depth: 0,
            branch_prob: 0.0,
        };
        let ap = ActorGenParams {
            cr_target: 0.0,
            n_teams: 0,
            pc_template: None,
            start_team: 0,
        };
        EncounterInstance::from_params(&tp, &ap, Some(0)).unwrap()
    }

    #[test]
    fn mammoth_template_shape() {
        let a = ActorInstance::from_creature_template(
            &MAMMOTH_TEMPLATE,
            Coordinate::new(0, 0),
            1,
            &mut FastRandRoller::with_seed(0),
            0,
        )
        .unwrap();
        assert_eq!(a.cr(), 6.0);
        assert_eq!(a.size(), Size::Huge);
        assert_eq!(a.creature_type(), CreatureType::Beast);
        // Three action lanes for the gore / charge / stomp triplet.
        assert!(a.find_action("mammoth gore").is_some());
        assert!(a.find_action("trampling charge").is_some());
        assert!(a.find_action("mammoth stomp").is_some());
        // Trampling charge available on round 1 (recharge resources
        // start in the "available" state).
        assert!(a.is_recharge_available("trampling_charge"));
    }

    #[test]
    fn mammoth_stomp_only_validates_against_prone_target() {
        // Pin the load-bearing combat clause: the Stomp standalone
        // refuses to validate against an upright target. The Multi
        // pattern of gore (set up prone) → stomp (cash in) is the
        // mammoth's signature.
        let mut e = empty_encounter();
        let mammoth = e
            .instantiate_creature(&MAMMOTH_TEMPLATE, Coordinate::new(2, 2), 0, 0)
            .unwrap();
        let target = e
            .instantiate_creature(
                &crate::actors::creatures::brown_bears::BROWN_BEAR_TEMPLATE,
                Coordinate::new(3, 2),
                1,
                0,
            )
            .unwrap();
        let stomp = e.actors[&mammoth]
            .find_action("mammoth stomp")
            .expect("mammoth should have stomp");
        // Upright target — stomp validates negative.
        assert!(
            !stomp.custom_validate_input(
                &e,
                mammoth,
                Some(&vec![target]),
                None,
                None
            ),
            "stomp must NOT validate against a standing target",
        );
        // Knock the bear prone — stomp now validates.
        e.actors
            .get_mut(&target)
            .unwrap()
            .add_condition(Condition::Prone, ConditionTimer::Permanent);
        assert!(
            stomp.custom_validate_input(
                &e,
                mammoth,
                Some(&vec![target]),
                None,
                None
            ),
            "stomp must validate against a prone target",
        );
    }

    #[test]
    fn mammoth_trampling_charge_validates_only_while_recharged() {
        // Pin the recharge gate so a future refactor doesn't strip the
        // `custom_validate_input` check — the charge should only fire
        // while `"trampling_charge"` is available.
        let mut e = empty_encounter();
        let mammoth = e
            .instantiate_creature(&MAMMOTH_TEMPLATE, Coordinate::new(2, 2), 0, 0)
            .unwrap();
        let charge = e.actors[&mammoth]
            .find_action("trampling charge")
            .expect("mammoth should have trampling charge");
        assert!(
            charge.custom_validate_input(&e, mammoth, None, None, None),
            "trampling charge should validate while recharged",
        );
        e.actors
            .get_mut(&mammoth)
            .unwrap()
            .spend_recharge("trampling_charge");
        assert!(
            !charge.custom_validate_input(&e, mammoth, None, None, None),
            "trampling charge should not validate after spending recharge",
        );
    }
}
