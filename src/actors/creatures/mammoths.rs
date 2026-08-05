use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{MAMMOTH_CHARGE, MAMMOTH_GORE, MAMMOTH_STOMP};
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
///   (5 ft), averaging ~25 on the CR-6 huge frame. It carries RAW's
///   **Trampling Charge** as a charge clause: twenty straight feet of
///   run and then a connecting gore forces a Strength save or knocks
///   the target prone.
///
///   This used to be a second, near-duplicate gore action of its own,
///   with the movement clause replaced by a Recharge 5-6 gate because
///   nothing could see the run. Now that something can, the stand-in is
///   gone and the mammoth has one gore — which is both RAW and stricter
///   than the recharge, since a mammoth standing still could trample on
///   a lucky d6.
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
        // RAW: twenty straight feet at the target and then a gore that
        // connects forces a Strength save or knocks them down. The
        // pacing the old Recharge 5-6 gate was standing in for now comes
        // from the board — the mammoth has to actually cross ground to
        // trample, which is a thing it can only do from range and only
        // once before it is standing on top of you.
        charge: Some(MAMMOTH_CHARGE),
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
        // Two action lanes — gore and stomp. The trampling charge is
        // not a third: it is a clause on the gore, which is what RAW
        // says and what removed the near-duplicate second gore this
        // template used to carry.
        assert!(a.find_action("mammoth gore").is_some());
        assert!(a.find_action("mammoth stomp").is_some());
        let charge = a.charge().expect("the mammoth tramples");
        assert_eq!(charge.weapon, "mammoth gore");
        assert!(charge.knocks_prone);
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

    /// The trample is gated on the run now, not on a d6.
    ///
    /// The old gate was a Recharge 5-6 standing in for RAW's "moves at
    /// least 20 feet straight toward a creature", which meant a mammoth
    /// standing nose-to-nose with its target could trample it on a lucky
    /// roll and a mammoth that had just thundered across the room might
    /// not. Both halves are pinned here.
    #[test]
    fn the_mammoth_tramples_only_after_it_has_actually_charged() {
        use crate::engine::side_effects::{ApplicableSideEffect, MoveActor, Resource};
        let mut e = empty_encounter();
        let mammoth = e
            .instantiate_creature(&MAMMOTH_TEMPLATE, Coordinate::new(2, 10), 0, 0)
            .unwrap();
        e.actors.get_mut(&mammoth).unwrap().reset_for_new_round();
        assert_eq!(
            e.actors[&mammoth].straight_run_tiles(),
            None,
            "a mammoth that has not moved is not charging"
        );

        let path: Vec<Coordinate> = (1..=10).map(|n| Coordinate::new(2 + n, 10)).collect();
        e.actors
            .get_mut(&mammoth)
            .unwrap()
            .consume_resource(Resource::Movement(25.0));
        MoveActor {
            actor_id: mammoth,
            path,
        }
        .apply(&mut e);
        let run = e.actors[&mammoth]
            .straight_run_tiles()
            .expect("the mammoth ran");
        assert!(
            run >= e.actors[&mammoth].charge().unwrap().run_tiles,
            "ten tiles should clear the twenty-foot bar, got {}",
            run
        );
    }
}
