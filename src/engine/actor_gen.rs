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
            let idx = ei.rng().usize(0..template_pool.len());
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
