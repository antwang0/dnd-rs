use crate::engine::encounter::{EncounterInstance, construct_encounter_instance};
use crate::engine::terrain_gen::TerrainGenParams;

pub mod engine;

fn main() {
    let mut encounter_instance: EncounterInstance = construct_encounter_instance(
        &TerrainGenParams {
            width: 128,
            height: 64,
            branch_depth: 256,
            branch_prob: 0.9
        }
    );
    println!("{}", encounter_instance.ascii());
}
