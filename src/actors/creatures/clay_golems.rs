use crate::actions::default_actions::DEFAULT_ACTIONS;
use crate::actions::monster_attacks::{CLAY_GOLEM_HASTEN, CLAY_GOLEM_MULTI, CLAY_GOLEM_SLAM};
use crate::actors::actor_template::{CreatureTemplate, damage_modifiers_from};
use crate::conditions::Condition;
use crate::engine::types::{CreatureType, DamageModifier, DamageType, Size, SpecialSense};
use std::collections::HashSet;
use std::sync::LazyLock;

/// Clay Golem — CR 9 construct, and the rung of the golem ladder the
/// bestiary was missing. Flesh (CR 5), stone (CR 10) and iron (CR 16)
/// were all here; the one between the first two was not, which left the
/// family's widest gap exactly where a mid-tier party meets it.
///
/// What makes it a different fight from its neighbours is that both of
/// its signature clauses attack the *party's* resources rather than its
/// hit points.
///
/// **Acid Absorption** (RAW): "whenever the golem is subjected to Acid
/// damage, it takes no damage and instead regains a number of Hit Points
/// equal to the Acid damage dealt." Carried as
/// `DamageModifier::Absorption`, the same trait the flesh golem's
/// lightning, the iron golem's fire and the shambling mound's lightning
/// now use. Acid is the element a party reaches for against armour and
/// constructs, so this is the trap it is written to be.
///
/// **The slam's ceiling drain** — every hit permanently lowers the
/// target's hit point maximum by the acid it took, with no save. A
/// cleric can keep the fighter standing and cannot keep them whole, and
/// the fight gets harder every round it lasts whatever the healing does.
/// See `ClayGolemSlam`.
///
/// Action lanes:
/// - **clay golem multiattack** — two slams, or three if Hasten went off
///   this turn.
/// - **clay slam** (standalone) — 1d10+STR bludgeoning plus 1d12 acid
///   and the drain, for when the multi is more than the target is worth.
/// - **hasten** (bonus action, Recharge 5–6) — Dash and Disengage, and
///   the third slam.
///
/// Defensive envelope: AC 14 (soft, unarmoured clay — the lowest AC on
/// the golem ladder), 123 HP (13d10+52), nonmagical B/P/S resistance,
/// immunity to poison and psychic, and Magic Resistance. The standard
/// construct condition set, with Petrified standing in for RAW's
/// Immutable Form the way it does on every other golem here.
///
/// **Berserk** (RAW: on starting a turn Bloodied, a 6 on 1d6 sends the
/// golem after the nearest creature, friend or foe) is not modeled, for
/// the same reason the flesh golem's isn't: it is an override of the
/// AI's target selection driven from the template, and the template lane
/// has no hook into the picker. It is the one clause of the four that
/// costs the *golem* rather than the party, so its absence is a small
/// overshoot in the golem's favour rather than a missing threat.
pub static CLAY_GOLEM_TEMPLATE: LazyLock<CreatureTemplate> = LazyLock::new(|| {
    let mut actions = DEFAULT_ACTIONS.clone();
    actions.push(&*CLAY_GOLEM_MULTI);
    actions.push(&*CLAY_GOLEM_SLAM);
    actions.push(&*CLAY_GOLEM_HASTEN);
    CreatureTemplate {
        name: "Clay Golem",
        // 'C' (uppercase) — the golem family's glyphs are the initial of
        // the material: 'G' stone, 'I' iron, 'F' flesh.
        glyph: 'C',
        ac: 14,
        // 13d10+52 ≈ 123 average per SRD (CR 9).
        hitpoints: "13d10+52".parse().unwrap(),
        speed: 30.,
        strength: 20,
        dexterity: 9,
        constitution: 18,
        intelligence: 3,
        wisdom: 8,
        charisma: 1,
        senses: HashSet::from([SpecialSense::Darkvision(60)]),
        cr: 9.0,
        size: Size::Large,
        creature_type: CreatureType::Construct,
        actions,
        // Construct damage envelope: nonmagical B/P/S resistance from
        // the shared helper, immunity to poison and psychic, and RAW's
        // Acid Absorption — no damage taken and an equal number of hit
        // points back.
        damage_modifiers: damage_modifiers_from([
            (DamageType::Acid, DamageModifier::Absorption),
            (DamageType::Poison, DamageModifier::Immunity),
            (DamageType::Psychic, DamageModifier::Immunity),
        ]),
        condition_immunities: HashSet::from([
            // Standard construct envelope: no mind to charm or frighten,
            // no metabolism to poison or exhaust, no joints to freeze —
            // and Petrified as the engine's surface for Immutable Form.
            Condition::Charmed,
            Condition::Frightened,
            Condition::Paralyzed,
            Condition::Petrified,
            Condition::Poisoned,
            Condition::Exhausted,
        ]),
        has_magic_resistance: true,
        // Recharge 5–6 on Hasten, the same threshold every other "5–6"
        // clause in the bestiary declares.
        recharge_abilities: vec![("clay_golem_hasten", 5)],
        // 5e: a golem's attacks are magical, which is what stops a clay
        // golem from being held off by the same resistance it has.
        features: HashSet::from([crate::actions::class_features::MAGICAL_ATTACKS_TAG]),
        ..CreatureTemplate::resistant_to_nonmagical_physical()
    }
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::actor_gen::ActorGenParams;
    use crate::engine::encounter::EncounterInstance;
    use crate::engine::terrain_gen::TerrainGenParams;
    use crate::engine::types::Coordinate;

    fn arena() -> EncounterInstance {
        let tp = TerrainGenParams {
            width: 14,
            height: 14,
            branch_depth: 0,
            branch_prob: 0.0,
        };
        let ap = ActorGenParams {
            cr_target: 0.0,
            n_teams: 0,
            pc_template: None,
            start_team: 0,
        };
        EncounterInstance::from_params(&tp, &ap, Some(11)).unwrap()
    }

    /// The load-bearing fields: the absorption lane, the construct
    /// immunity envelope, and the three action lanes.
    #[test]
    fn clay_golem_template_shape() {
        let mut e = arena();
        let id = e
            .instantiate_creature(&CLAY_GOLEM_TEMPLATE, Coordinate::new(3, 3), 1, 0)
            .unwrap();
        let g = &e.actors[&id];
        assert_eq!(
            g.damage_modifier(DamageType::Acid),
            Some(DamageModifier::Absorption)
        );
        assert!(g.is_immune_to(DamageType::Poison));
        assert!(g.is_immune_to(DamageType::Psychic));
        assert_eq!(
            g.nonmagical_damage_modifier(DamageType::Slashing),
            Some(DamageModifier::Resistance)
        );
        assert!(g.is_immune_to_condition(Condition::Petrified));
        assert!(g.find_action("clay golem multiattack").is_some());
        assert!(g.find_action("clay slam").is_some());
        assert!(g.find_action("hasten").is_some());
    }

    /// Acid aimed at a clay golem is worse than wasted: it takes none of
    /// it and comes back up by the amount that was thrown.
    ///
    /// Both halves are asserted because the trait is two clauses and
    /// only one of them is the surprise — an implementation that healed
    /// but still took the hit, or took the hit and still healed, would
    /// pass a test that only counted hit points in one direction.
    #[test]
    fn a_clay_golem_drinks_the_acid_thrown_at_it() {
        use crate::engine::side_effects::{ApplicableSideEffect, DealDamage};
        let mut e = arena();
        let id = e
            .instantiate_creature(&CLAY_GOLEM_TEMPLATE, Coordinate::new(3, 3), 1, 0)
            .unwrap();
        let full = e.actors[&id].max_hitpoints();
        // Wound it first — RAW's heal caps at the maximum like any other,
        // so a golem at full HP would show nothing either way.
        DealDamage {
            actor_id: id,
            amount: 40,
            damage_type: crate::engine::types::DamageType::Slashing,
        }
        .apply(&mut e);
        let wounded = e.actors[&id].hitpoints();
        assert!(wounded < full, "the slashing should have landed");

        DealDamage {
            actor_id: id,
            amount: 18,
            damage_type: DamageType::Acid,
        }
        .apply(&mut e);
        assert_eq!(
            e.actors[&id].hitpoints(),
            (wounded + 18).min(full),
            "acid should heal rather than harm"
        );
    }

    /// Hasten is a bonus action on a recharge, and the multiattack is
    /// three slams once it has gone off — the two halves of a clause
    /// written as one sentence, so a test that checked only the recharge
    /// would miss what the recharge is for.
    #[test]
    fn hasten_spends_its_recharge_and_buys_the_third_slam() {
        use crate::actions::action_template::Action;
        use crate::actions::monster_attacks::CLAY_GOLEM_HASTEN_TAG;
        use crate::engine::side_effects::Resource;
        let mut e = arena();
        let id = e
            .instantiate_creature(&CLAY_GOLEM_TEMPLATE, Coordinate::new(3, 3), 1, 0)
            .unwrap();
        e.actors.get_mut(&id).unwrap().reset_for_new_round();
        assert!(e.actors[&id].is_recharge_available("clay_golem_hasten"));
        let two_slams = CLAY_GOLEM_MULTI
            .expected_damage(&e, id)
            .expect("the multi should carry an estimate");

        let effects = CLAY_GOLEM_HASTEN.side_effects(&mut e, id, None, None, None);
        for ef in effects {
            ef.apply(&mut e);
        }
        assert!(
            !e.actors[&id].is_recharge_available("clay_golem_hasten"),
            "hasten burns its recharge"
        );
        assert!(e.actors[&id].once_per_turn_used(CLAY_GOLEM_HASTEN_TAG));
        assert!(
            e.actors[&id].is_disengaging(),
            "and it takes the Disengage half"
        );
        let three_slams = CLAY_GOLEM_MULTI.expected_damage(&e, id).unwrap();
        assert!(
            three_slams > two_slams,
            "the hastened multi should be worth a third swing: {} vs {}",
            three_slams,
            two_slams
        );

        // The ledger is per-turn, so next round's multi is back to two.
        e.actors.get_mut(&id).unwrap().reset_for_new_round();
        assert!(!e.actors[&id].once_per_turn_used(CLAY_GOLEM_HASTEN_TAG));
        assert_eq!(CLAY_GOLEM_MULTI.expected_damage(&e, id), Some(two_slams));

        // And it is a bonus action, not an Action — which is what lets
        // the golem hasten and then swing three times on the same turn.
        assert_eq!(
            CLAY_GOLEM_HASTEN.cost(&e, id, None, None, None),
            vec![Resource::BonusAction]
        );
    }

    /// The slam's drain is sized off what the acid actually did, so it
    /// lowers the ceiling on an ordinary target and lowers nothing at
    /// all on one immune to acid — RAW says "the Acid damage taken".
    #[test]
    fn the_slam_lowers_the_ceiling_by_the_acid_that_landed() {
        use crate::actions::action_template::Action;
        use crate::actors::creatures::commoners::COMMONER_TEMPLATE;
        let mut drained = 0;
        for seed in 0..30 {
            let tp = TerrainGenParams {
                width: 14,
                height: 14,
                branch_depth: 0,
                branch_prob: 0.0,
            };
            let ap = ActorGenParams {
                cr_target: 0.0,
                n_teams: 0,
                pc_template: None,
                start_team: 0,
            };
            let mut e = EncounterInstance::from_params(&tp, &ap, Some(seed)).unwrap();
            let golem = e
                .instantiate_creature(&CLAY_GOLEM_TEMPLATE, Coordinate::new(3, 3), 1, 0)
                .unwrap();
            // A second clay golem is the acid-immune control: it absorbs
            // the acid, so nothing is "taken" and nothing should drain.
            let sibling = e
                .instantiate_creature(&CLAY_GOLEM_TEMPLATE, Coordinate::new(4, 3), 0, 1)
                .unwrap();
            let ceiling_before = e.actors[&sibling].max_hitpoints();
            let effects =
                CLAY_GOLEM_SLAM.side_effects(&mut e, golem, Some(&vec![sibling]), None, None);
            for ef in effects {
                ef.apply(&mut e);
            }
            assert_eq!(
                e.actors[&sibling].max_hitpoints(),
                ceiling_before,
                "a target that takes no acid loses no ceiling (seed {})",
                seed
            );

            let squishy = e
                .instantiate_creature(&COMMONER_TEMPLATE, Coordinate::new(4, 4), 0, 2)
                .unwrap();
            let before = e.actors[&squishy].max_hitpoints();
            let effects =
                CLAY_GOLEM_SLAM.side_effects(&mut e, golem, Some(&vec![squishy]), None, None);
            for ef in effects {
                ef.apply(&mut e);
            }
            if let Some(a) = e.actors.get(&squishy)
                && a.max_hitpoints() < before
            {
                drained += 1;
            }
        }
        assert!(
            drained > 0,
            "across thirty seeds the slam should have connected and drained at least once"
        );
    }
}
