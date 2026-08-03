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
    /// 5e half cover from an object — "a low wall, a large piece of
    /// furniture, a narrow tree trunk" (PHB p.196). A creature with a low
    /// wall between it and an attacker gains +2 AC.
    ///
    /// The variant carries two clauses that no other terrain combines:
    ///
    ///   - **It does not block line of sight.** That is the whole point
    ///     of it. `Wall` is total cover and gates the attack entirely;
    ///     this is the tile you can shoot over at a penalty, which is
    ///     the case 5e's cover rules are mostly about and the one the
    ///     engine had no way to express — `cover_ac_bonus` counted
    ///     intervening *creatures* and said so in its own docstring,
    ///     because the terrain layer had no per-tile cover semantics.
    ///
    ///   - **It is passable, at difficult-terrain cost.** RAW a low wall
    ///     is an obstacle you climb rather than a square you cannot
    ///     enter, and on a 2.5-ft grid one tile of it is exactly that
    ///     climb. Making it impassable instead would have been the
    ///     simpler rule and a worse map: an impassable scatter can seal
    ///     a corridor, and a low wall that cannot be crossed is a wall.
    ///
    /// Clambering is the same movement surcharge difficult terrain
    /// charges, and it is waived by the same things — Freedom of
    /// Movement, magical flight, Land's Stride — through
    /// `ActorInstance::ignores_difficult_terrain`. A creature flying over
    /// a low wall pays nothing for it, and still gets no cover from it.
    LowWall,
    /// A barrier that stops bodies and not eyes — 5e Wall of Force's
    /// "an invisible wall of force… nothing can physically pass through
    /// the wall", and the same sentence in every other transparent
    /// barrier.
    ///
    /// The one combination `Wall` and `LowWall` between them could not
    /// express, and the reason it needs its own variant rather than a
    /// flag on either of them: `Wall` is opaque and impassable,
    /// `LowWall` is transparent and passable, and a force wall is the
    /// remaining corner. Everything a caster does with one turns on
    /// being able to see through it — you put it between the party and
    /// the dragon and then shoot the dragon.
    ///
    /// Never generated. The terrain generator lays scenery; this
    /// variant exists for the conjured-terrain lane
    /// (`crate::engine::conjured_terrain`) to write and take back.
    ForceWall,
}

impl TerrainType {
    /// Movement cost multiplier for stepping onto this tile. Normal
    /// terrain costs 1; difficult terrain and the clamber over a low
    /// wall cost 2 (5e PHB p.182).
    pub fn movement_cost(self) -> f32 {
        match self {
            TerrainType::DifficultTerrain | TerrainType::LowWall => 2.0,
            _ => 1.0,
        }
    }

    pub fn is_passable(self) -> bool {
        !matches!(
            self,
            TerrainType::Wall | TerrainType::Empty | TerrainType::ForceWall
        )
    }

    /// True if this tile stops a line of sight dead.
    ///
    /// The rule used to live inline in `has_line_of_sight` as a bare
    /// `== TerrainType::Wall`, which was fine while "solid" and "opaque"
    /// named the same single variant. `ForceWall` is the case that
    /// separates them — solid, and transparent — so the question gets
    /// its own name next to `is_passable`, and the two properties can
    /// disagree.
    pub fn blocks_sight(self) -> bool {
        matches!(self, TerrainType::Wall)
    }

    /// True if a line of attack crossing this tile is obstructed enough
    /// to grant the target cover.
    ///
    /// Only `LowWall` does. `Wall` is deliberately excluded: it blocks
    /// line of sight outright, so an attack that crosses one never
    /// reaches the cover walk at all — counting it here would be dead
    /// code that looked like a rule. `ForceWall` is excluded for the
    /// opposite reason: RAW it is a clean, transparent pane, and a
    /// creature on the far side of it is in plain view — the wall's
    /// protection is that nothing can *reach* them, which the
    /// impassability already says.
    pub fn grants_cover(self) -> bool {
        matches!(self, TerrainType::LowWall)
    }
}
