use crate::engine::terrain::{TerrainInfo, TerrainType};
use crate::engine::terrain_gen::{TerrainGenParams, generate_terrain};
use crate::engine::unit::Unit;

pub struct EncounterInstance<'a> {
    width: usize,
    height: usize,
    terrain: Vec<TerrainInfo>,
    units: Vec<Option<&'a dyn Unit>>
}

impl EncounterInstance<'_> {
    fn idx(&self, x: usize, y: usize) -> usize{
        x + y * self.width
    }

    pub fn ascii(&self) -> String {
        let mut output = String::new();

        for i in 0..self.height {
            for j in 0..self.width {
                let vec_idx = self.idx(j, i);
                output.push(match self.terrain[vec_idx].terrain_type {
                    TerrainType::Empty => ' ',
                    TerrainType::Floor => '░',
                    TerrainType::Wall => '█'
                });
            }
            output.push('\n');
        }
        output
    }
}

pub fn construct_encounter_instance<'a>(params: &TerrainGenParams) -> EncounterInstance<'a> {
    EncounterInstance{
        width: params.width,
        height: params.height,
        terrain: generate_terrain(params),
        units: vec![None; params.width * params.height]
    }
}
