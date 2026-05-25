#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerrainInfo {
    pub terrain_type: TerrainType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TerrainType {
    Empty,
    Floor,
    Wall,
    /// 5e Difficult Terrain — each tile of movement through difficult
    /// terrain costs 2 tiles of movement budget instead of 1. Rubble,
    /// undergrowth, mud, shallow water, etc.
    DifficultTerrain,
}

impl TerrainType {
    /// Movement cost multiplier for stepping onto this tile. Normal
    /// terrain costs 1; difficult terrain costs 2 (5e PHB p.182).
    pub fn movement_cost(self) -> f32 {
        match self {
            TerrainType::DifficultTerrain => 2.0,
            _ => 1.0,
        }
    }

    pub fn is_passable(self) -> bool {
        !matches!(self, TerrainType::Wall | TerrainType::Empty)
    }
}
