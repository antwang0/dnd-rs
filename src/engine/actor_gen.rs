use std::error::Error;

use crate::actors::actor_template::CreatureTemplate;
use crate::engine::encounter::EncounterInstance;
use crate::engine::errors::RngTryError;

use rand::Rng;

const MAX_TRIES: usize = 512;

pub struct ActorGenParams {
    pub cr_target: f32,
    pub n_teams: usize,
}

pub fn generate_actors(
    ei: &mut EncounterInstance,
    params: &ActorGenParams,
    template_pool: &Vec<&'static CreatureTemplate>,
) -> Result<(), Box<dyn Error>> {
    let mut id_by_template: Vec<usize> = vec![0; template_pool.len()];
    let mut rng = rand::rng();
    for team_id in 0..params.n_teams {
        let mut cr_total: f32 = 0.0;

        let mut tries: usize = 0;
        while cr_total < params.cr_target {
            if tries >= MAX_TRIES {
                return Err(Box::new(RngTryError));
            }
            tries += 1;
            let idx = rng.random_range(0..template_pool.len());
            let creature_template = &template_pool[idx];
            let location_result = ei.get_random_spawn(creature_template.size);
            let instance_n = id_by_template[idx];
            id_by_template[idx] += 1;
            match location_result {
                Ok(location) => {
                    ei.instantiate_creature(&creature_template, location, team_id, instance_n)?;
                    cr_total += creature_template.cr;
                }
                Err(_) => continue,
            }
        }
    }
    Ok(())
}
