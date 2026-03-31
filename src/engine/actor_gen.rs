use std::error::Error;

use crate::actors::actor_template::CreatureTemplate;
use crate::actors::creatures::zombies::zombie_template;
use crate::engine::encounter::EncounterInstance;
use crate::engine::errors::RngTryError;

use rand::seq::IndexedMutRandom;

const MAX_TRIES: usize = 512;

pub struct ActorGenParams {
    pub cr_target: f32,
    pub n_teams: usize,
}

pub fn generate_actors(
    ei: &mut EncounterInstance,
    params: &ActorGenParams,
) -> Result<(), Box<dyn Error>> {
    // TODO: move pool to fn
    let mut template_pool: Vec<CreatureTemplate> = Vec::new();
    template_pool.push(zombie_template());

    for team_id in 0..params.n_teams {
        let mut cr_total: f32 = 0.0;

        let mut tries: usize = 0;
        while cr_total < params.cr_target {
            if tries >= MAX_TRIES {
                return Err(Box::new(RngTryError));
            }
            tries += 1;
            let creature_template = template_pool
                .choose_mut(&mut rand::rng())
                .ok_or(Box::new(RngTryError))?;
            let location_result = ei.get_random_spawn(creature_template.size);
            match location_result {
                Ok(location) => {
                    ei.instantiate_creature(creature_template, location, team_id)?;
                    cr_total += creature_template.cr;
                }
                Err(_) => continue,
            }
        }
    }
    Ok(())
}
