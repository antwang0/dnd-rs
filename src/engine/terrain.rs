#[derive(Clone, PartialEq)]
pub struct TerrainInfo {
    pub terrain_type: TerrainType,
}

#[derive(Clone, PartialEq)]
pub enum TerrainType {
    Empty,
    Floor,
    Wall,
}
