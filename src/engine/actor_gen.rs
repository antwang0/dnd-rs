use std::error::Error;

use crate::actors::actor_template::CreatureTemplate;
use crate::engine::encounter::EncounterInstance;
use crate::engine::errors::{NoLegalPosition, RngTryError};
use crate::engine::util::get_tiles_from_size;

const MAX_TRIES: usize = 512;

#[derive(Clone)]
pub struct ActorGenParams {
    pub cr_target: f32,
    pub n_teams: usize,
    /// If set, team 0 spawns this template once instead of being
    /// randomly populated to `cr_target`. Used to seed a single
    /// player-character actor on the player's side; remaining teams
    /// are still randomized per `cr_target`.
    pub pc_template: Option<&'static CreatureTemplate>,
    /// First team id to generate. Default 0. The multi-encounter loop
    /// places PCs externally and then re-uses the generator for enemies
    /// only by setting `start_team = 1`.
    pub start_team: usize,
}

/// Indices into `pool` of every template that fits inside `remaining`
/// CR of budget — or, when nothing does, of the cheapest thing that can
/// still move the total.
///
/// **The bug this fixes.** The generator used to pick uniformly from
/// the whole pool and stop once the running total reached the target,
/// which meant the CR budget bounded the fight only from *below*. The
/// game's first encounter asks for a budget of 1.0 against a pool whose
/// top end is a CR-23 kraken; roughly one first fight in every two
/// hundred and thirty-eight was a kraken, and the difficulty ramp in
/// `App::scaled_cr_target` — half a point of CR per encounter — was
/// nearly meaningless next to a uniform draw with a variance of that
/// size. The budget is a ceiling now as well as a floor: the worst a
/// fight can overshoot by is one pick of the cheapest creature that
/// did not fit.
///
/// Zero-CR templates (the commoner, the cat, the rat) ride along for
/// as long as the budget can still buy something real — they are
/// scenery with hit points and they cost nothing, so there is no
/// reason to bar them from a fight that has room left in it. They drop
/// out on the closing pick, and that is the `cr > 0.0` clause in the
/// fallback: without it, once
/// the remaining budget drops below the cheapest real monster the draw
/// would be nothing but free creatures, spinning forever without the
/// total ever moving. The fallback guarantees every iteration advances
/// `cr_total` by at least the pool's smallest positive CR, so the loop
/// terminates in a bounded number of picks rather than by exhausting
/// `MAX_TRIES` and failing the whole encounter.
///
/// Returns indices rather than templates because the caller keys its
/// per-template instance counter (`id_by_template`) off the pool
/// position, and that numbering is what makes two goblins in one fight
/// "Goblin" and "Goblin 2" rather than two things with the same name.
fn affordable_templates(pool: &[&'static CreatureTemplate], remaining: f32) -> Vec<usize> {
    let fits: Vec<usize> = (0..pool.len())
        .filter(|&i| pool[i].cr <= remaining)
        .collect();
    if fits.iter().any(|&i| pool[i].cr > 0.0) {
        return fits;
    }
    // Nothing that costs anything fits. Take the cheapest tier that
    // does cost something, so this pick closes the budget out.
    let cheapest = pool
        .iter()
        .map(|t| t.cr)
        .filter(|&cr| cr > 0.0)
        .fold(f32::INFINITY, f32::min);
    let cheapest_indices: Vec<usize> = (0..pool.len())
        .filter(|&i| pool[i].cr == cheapest)
        .collect();
    // A pool of nothing but free creatures has no cheapest positive
    // tier. Fall back to the whole pool and let `MAX_TRIES` bound it —
    // that pool cannot satisfy any positive budget by any strategy.
    if cheapest_indices.is_empty() {
        return (0..pool.len()).collect();
    }
    cheapest_indices
}

pub fn generate_actors(
    ei: &mut EncounterInstance,
    params: &ActorGenParams,
    template_pool: &[&'static CreatureTemplate],
) -> Result<(), Box<dyn Error>> {
    let mut id_by_template: Vec<usize> = vec![0; template_pool.len()];
    // Footprint widths the board has been *proven* to have no room for.
    //
    // `get_random_spawn` is an exhaustive scan of a shuffled coordinate
    // list, so an `Err` from it is not bad luck — it is a proof that no
    // anchor anywhere on the map can hold a creature of that size. The
    // loop below used to answer that proof by drawing again from the
    // same pool, which could only re-ask a settled question: on a board
    // with no room left for anything Large, every Large draw failed
    // identically and the budget of tries drained into a random walk
    // whose large steps could never land.
    //
    // Remembering the answer turns that walk into a monotone narrowing.
    // The board only ever gets fuller inside this function, so a width
    // that did not fit cannot start fitting, and there are six widths —
    // so the loop either places something or permanently removes a size
    // class on every iteration.
    //
    // Widths rather than `Size`, and `>=` rather than `==`, because the
    // proof generalizes upward: a Huge anchor needs a 3×3 of spawnable
    // tiles, which contains a 2×2, so "no room for a Large" already
    // establishes "no room for a Huge" without spending a draw finding
    // out.
    let mut full_widths: Vec<usize> = Vec::new();
    let width_is_full = |full: &[usize], size| full.iter().any(|&w| get_tiles_from_size(size) >= w);
    // SRD 5.2 **Water Breathing** — the sharks, the seahorses and the
    // piranhas drown in air, so they come in through
    // `get_random_water_spawn` and nothing else.
    //
    // The flag is the same monotone-narrowing trick `full_widths` is,
    // one axis over. A board either has an unoccupied pool big enough
    // for a body or it does not, and the answer only gets more negative
    // as the fight fills up — so the first failure settles the question
    // for the rest of the generation, and the whole cohort leaves the
    // draw rather than being re-asked once per try until `MAX_TRIES`
    // runs out.
    //
    // Dropping them is the *right* answer rather than a fallback. A
    // reef shark anchored in a dry corridor is not an encounter; it is
    // a six-round countdown to a corpse nobody fought, which is exactly
    // why the clause could not ship before the generator could say no.
    let mut no_water_room = false;
    let is_aquatic =
        |t: &CreatureTemplate| t.features.contains(crate::actions::class_features::AQUATIC_ONLY_TAG);
    for team_id in params.start_team..params.n_teams {
        // Team 0 uses the fixed PC template if provided; else fall
        // through to the random CR-target generator.
        if team_id == 0
            && let Some(pc) = params.pc_template
        {
            let location = ei.get_random_spawn(pc.size)?;
            ei.instantiate_creature(pc, location, team_id, 0)?;
            continue;
        }
        if template_pool.is_empty() {
            continue;
        }
        let mut cr_total: f32 = 0.0;
        let mut tries: usize = 0;
        while cr_total < params.cr_target {
            if tries >= MAX_TRIES {
                return Err(Box::new(RngTryError));
            }
            tries += 1;
            let affordable: Vec<usize> =
                affordable_templates(template_pool, params.cr_target - cr_total)
                    .into_iter()
                    .filter(|&i| !width_is_full(&full_widths, template_pool[i].size))
                    .filter(|&i| !(no_water_room && is_aquatic(template_pool[i])))
                    .collect();
            // Everything the budget could still buy is too big for what
            // is left of the map. That is a board that has run out of
            // room, not an unlucky roll, and it is worth saying so:
            // spinning out the remaining tries would report it as
            // "exceeded max tries for rng", which names neither the
            // cause nor anything the caller could act on.
            let Some(&idx) = affordable.get(ei.rng().usize(0..affordable.len().max(1))) else {
                return Err(Box::new(NoLegalPosition));
            };
            let creature_template = &template_pool[idx];
            let aquatic = is_aquatic(creature_template);
            let location_result = if aquatic {
                ei.get_random_water_spawn(creature_template.size)
            } else {
                ei.get_random_spawn(creature_template.size)
            };
            let instance_n = id_by_template[idx];
            match location_result {
                Ok(location) => {
                    ei.instantiate_creature(creature_template, location, team_id, instance_n)?;
                    id_by_template[idx] += 1;
                    cr_total += creature_template.cr;
                }
                Err(_) if aquatic => {
                    // A failed *water* spawn proves nothing about dry
                    // ground, so it narrows the aquatic cohort rather
                    // than the width. Recording it on `full_widths`
                    // would take every Large creature on the roster off
                    // the table because a giant shark could not find a
                    // pool.
                    no_water_room = true;
                    continue;
                }
                Err(_) => {
                    full_widths.push(get_tiles_from_size(creature_template.size));
                    continue;
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::actor_template::CreatureTemplate;
    use crate::engine::encounter::EncounterInstance;
    use crate::engine::terrain_gen::TerrainGenParams;

    /// A pool spanning the shape the real one has — free scenery, a
    /// cheap monster, and something that would end a low-level party
    /// on its own.
    fn pool() -> Vec<&'static CreatureTemplate> {
        use crate::actors::creatures::*;
        vec![
            &commoners::COMMONER_TEMPLATE,
            &goblins::GOBLIN_TEMPLATE,
            &ogres::OGRE_TEMPLATE,
            &krakens::KRAKEN_TEMPLATE,
        ]
    }

    /// The budget is a ceiling, not only a floor. With one point of CR
    /// to spend, nothing above one point of CR is on the table.
    #[test]
    fn a_one_cr_budget_cannot_draw_a_kraken() {
        let p = pool();
        let affordable = affordable_templates(&p, 1.0);
        for &i in &affordable {
            assert!(
                p[i].cr <= 1.0,
                "{} (CR {}) is affordable at a budget of 1.0",
                p[i].name,
                p[i].cr
            );
        }
        assert!(
            affordable.iter().any(|&i| p[i].name == "Goblin"),
            "a CR-1/4 goblin has to still be reachable"
        );
    }

    /// Free creatures stay on the table for as long as the budget can
    /// still buy something real — they are scenery with hit points and
    /// they cost nothing, so there is no reason to bar them from a
    /// fight that has room left in it.
    ///
    /// The last rung is where they drop out, and that is the point: a
    /// budget too small for the cheapest monster has to be closed by
    /// something that costs CR, or the loop never ends.
    #[test]
    fn zero_cr_scenery_rides_along_until_the_budget_runs_out() {
        let p = pool();
        for budget in [0.25, 1.0, 30.0] {
            let affordable = affordable_templates(&p, budget);
            assert!(
                affordable.iter().any(|&i| p[i].cr == 0.0),
                "the commoner fell out of the pool at a budget of {}",
                budget
            );
        }
        let last_rung = affordable_templates(&p, 0.1);
        assert!(
            !last_rung.iter().any(|&i| p[i].cr == 0.0),
            "the closing pick has to cost something"
        );
    }

    /// A budget too small for anything that costs CR still returns
    /// something that costs CR. Without this the loop would draw free
    /// creatures forever and never close the budget out — the whole
    /// encounter would fail with `RngTryError` rather than being one
    /// goblin over the line.
    #[test]
    fn a_budget_below_the_cheapest_monster_still_closes_out() {
        let p = pool();
        let affordable = affordable_templates(&p, 0.01);
        assert!(!affordable.is_empty());
        for &i in &affordable {
            assert!(
                p[i].cr > 0.0,
                "{} costs nothing and would not close the budget",
                p[i].name
            );
        }
    }

    /// End to end, across seeds: a CR-1 encounter never spawns
    /// something the budget could not pay for, and the total lands
    /// within one pick of the target rather than anywhere above it.
    #[test]
    fn a_generated_encounter_stays_inside_its_budget() {
        let tp = TerrainGenParams {
            width: 30,
            height: 20,
            branch_depth: 0,
            branch_prob: 0.0,
        };
        for seed in 0..40u64 {
            let ap = ActorGenParams {
                cr_target: 1.0,
                n_teams: 2,
                pc_template: Some(&crate::actors::creatures::fighters::FIGHTER_TEMPLATE),
                start_team: 0,
            };
            let e = EncounterInstance::from_params(&tp, &ap, Some(seed)).unwrap();
            let enemy_cr: f32 = e
                .actors
                .values()
                .filter(|a| a.team() != 0)
                .map(|a| a.cr())
                .sum();
            assert!(
                enemy_cr < 3.0,
                "seed {} generated {} CR of enemies against a budget of 1.0",
                seed,
                enemy_cr
            );
        }
    }

    /// A creature that drowns in air is generated in water, or not at
    /// all.
    ///
    /// The two halves of the same rule and both are load-bearing, which
    /// is why they are one test. A reef shark anchored on a dungeon
    /// floor is not an encounter — it is a six-round countdown to a
    /// corpse nobody fought — so a board with no pool has to take the
    /// whole cohort off the table rather than putting one somewhere it
    /// cannot live. The dry half is also the one that would fail
    /// silently: a shark generated on stone still walks, still bites,
    /// and still dies of the rule working correctly.
    #[test]
    fn a_water_breather_is_generated_in_water_or_not_generated() {
        use crate::actors::creatures::reef_sharks::REEF_SHARK_TEMPLATE;
        use crate::engine::terrain::TerrainType;
        use crate::engine::types::Coordinate;

        let tp = TerrainGenParams {
            width: 30,
            height: 20,
            branch_depth: 0,
            branch_prob: 0.0,
        };
        let sharks_only: Vec<&'static CreatureTemplate> = vec![&REEF_SHARK_TEMPLATE];
        let ap = ActorGenParams {
            cr_target: 2.0,
            n_teams: 2,
            pc_template: Some(&crate::actors::creatures::fighters::FIGHTER_TEMPLATE),
            start_team: 0,
        };

        // A board the generator laid itself, with whatever pools it
        // happened to flood. Every shark on it stands in one.
        let mut wet_boards = 0;
        for seed in 0..40u64 {
            let mut e = EncounterInstance::empty(&tp, Some(seed));
            if generate_actors(&mut e, &ap, &sharks_only).is_err() {
                // A board with no pool cannot pay a CR-2 budget out of
                // a pool of nothing but sharks, and says so rather than
                // beaching one.
                continue;
            }
            let ids: Vec<usize> = e.sorted_actor_ids();
            for id in ids {
                if e.actors[&id].name().starts_with("Reef Shark") {
                    wet_boards += 1;
                    assert!(
                        e.is_immersed(id),
                        "seed {seed} put a reef shark on dry ground"
                    );
                }
            }
        }
        assert!(
            wet_boards > 0,
            "forty seeds and not one shark — the fixture is not exercising the lane"
        );

        // And the dry board: a map with the water scrubbed out draws no
        // shark at all, rather than draining `MAX_TRIES` re-asking a
        // question the first failure already settled.
        let mut dry = EncounterInstance::empty(&tp, Some(7));
        for x in 0..30isize {
            for y in 0..20isize {
                if dry
                    .terrain_at(Coordinate::new(x, y))
                    .is_some_and(|t| t.terrain_type.is_water())
                {
                    dry.set_terrain_at(Coordinate::new(x, y), TerrainType::Floor);
                }
            }
        }
        assert!(
            generate_actors(&mut dry, &ap, &sharks_only).is_err(),
            "a dry board cannot field a shark and must say so"
        );
        assert!(
            dry.sorted_actor_ids()
                .iter()
                .all(|id| !dry.actors[id].name().starts_with("Reef Shark")),
            "and must not have beached one on the way to saying it"
        );
    }

    /// A board with no room left fails as a board with no room left,
    /// and fails at once.
    ///
    /// `get_random_spawn` is an exhaustive scan, so its `Err` proves no
    /// anchor on the map fits that footprint — a proof the loop used to
    /// answer by drawing again from the same pool and re-asking. The
    /// symptom was an encounter that spent all 512 tries re-failing and
    /// then reported "exceeded max tries for rng", which names the
    /// budget it ran out of rather than the wall it ran into. Neither
    /// half of that is something a caller can act on.
    ///
    /// The assertion is on the message rather than on a type, because
    /// the error is boxed by the time it leaves the generator and the
    /// message is what any caller — including the banner the player
    /// reads — actually gets.
    #[test]
    fn a_board_with_no_room_left_says_that_and_not_something_about_the_rng() {
        let tp = TerrainGenParams {
            width: 6,
            height: 6,
            branch_depth: 0,
            branch_prob: 0.0,
        };
        let ap = ActorGenParams {
            cr_target: 60.0,
            n_teams: 2,
            pc_template: Some(&crate::actors::creatures::fighters::FIGHTER_TEMPLATE),
            start_team: 0,
        };
        let err = EncounterInstance::from_params(&tp, &ap, Some(1))
            .err()
            .expect("a six-by-six room cannot hold sixty CR of monsters");
        let msg = err.to_string();
        assert!(
            msg.contains("legal position"),
            "the generator should name the wall it hit, got: {msg}"
        );
        assert!(
            !msg.contains("max tries"),
            "and should not blame the dice for it: {msg}"
        );
    }

    /// The narrowing is monotone: once a footprint is known not to fit,
    /// every footprint at least that wide is dropped from the draw
    /// without spending a try proving it separately.
    ///
    /// Asserted through the outcome rather than by inspecting the memo,
    /// since the memo is a local: a board that admits Medium creatures
    /// and nothing larger still fills to its budget, which it could only
    /// do by giving up on the bigger draws rather than re-rolling them.
    #[test]
    fn a_cramped_board_still_fills_its_budget_with_what_fits() {
        let tp = TerrainGenParams {
            width: 14,
            height: 14,
            branch_depth: 0,
            branch_prob: 0.0,
        };
        for seed in 0..25u64 {
            let ap = ActorGenParams {
                cr_target: 4.0,
                n_teams: 2,
                pc_template: Some(&crate::actors::creatures::fighters::FIGHTER_TEMPLATE),
                start_team: 0,
            };
            let e = EncounterInstance::from_params(&tp, &ap, Some(seed))
                .unwrap_or_else(|err| panic!("seed {seed}: {err}"));
            let enemy_cr: f32 = e
                .actors
                .values()
                .filter(|a| a.team() != 0)
                .map(|a| a.cr())
                .sum();
            assert!(
                enemy_cr >= 4.0,
                "seed {} closed out at {} CR against a budget of 4.0",
                seed,
                enemy_cr
            );
        }
    }
}
