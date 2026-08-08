use std::error::Error;

use crate::actors::actor_template::CreatureTemplate;
use crate::engine::encounter::EncounterInstance;
use crate::engine::errors::RngTryError;

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
            let affordable = affordable_templates(template_pool, params.cr_target - cr_total);
            let idx = affordable[ei.rng().usize(0..affordable.len())];
            let creature_template = &template_pool[idx];
            let location_result = ei.get_random_spawn(creature_template.size);
            let instance_n = id_by_template[idx];
            match location_result {
                Ok(location) => {
                    ei.instantiate_creature(creature_template, location, team_id, instance_n)?;
                    id_by_template[idx] += 1;
                    cr_total += creature_template.cr;
                }
                Err(_) => continue,
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
}
