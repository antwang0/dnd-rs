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
    /// Water deep enough to swim in — the tile 5e's **Underwater
    /// Combat** rules (PHB p.198) are written about.
    ///
    /// Deliberately *not* the same thing as the "shallow water" already
    /// listed among `DifficultTerrain`'s flavours. Shallow water is mud
    /// you wade through and nothing more; this is water you are *in*,
    /// and being in it changes four separate rules that no other tile
    /// on the board touches:
    ///
    ///   - **Swimming costs double**, exactly as difficult terrain does
    ///     — but it is waived by a different set of things. A ranger's
    ///     Land's Stride covers "nonmagical difficult terrain" and does
    ///     nothing for a lake; a swimming speed covers the lake and
    ///     nothing for the rubble. The two surcharges are the same
    ///     magnitude and different rules, which is the whole reason
    ///     this is a variant rather than a second flavour of
    ///     `DifficultTerrain` — see `ActorInstance::swims_freely`.
    ///
    ///   - **Melee weapon attacks made in it are at disadvantage**
    ///     unless the swinger has a swimming speed, or the weapon is
    ///     one of the five that keep their edge underwater.
    ///
    ///   - **Ranged weapon attacks made in it are at disadvantage**,
    ///     and miss outright past normal range.
    ///
    ///   - **Anything fully immersed resists fire.**
    ///
    /// The last three live in `crate::engine::underwater`; the first
    /// lives in `movement_cost` below and in the pathfinder's waiver.
    ///
    /// Transparent and passable. Water is not cover — RAW gives a
    /// submerged creature no AC bonus, and the engine has a much better
    /// tile for "shoot over it at a penalty" in `LowWall` — and it does
    /// not block line of sight, because every one of the four rules
    /// above assumes the creatures can see each other well enough to
    /// swing.
    Water,
}

impl TerrainType {
    /// Movement cost multiplier for stepping onto this tile. Normal
    /// terrain costs 1; difficult terrain, the clamber over a low
    /// wall, and a stroke of swimming all cost 2 (5e PHB p.182: "each
    /// foot of movement costs 1 extra foot" for both difficult terrain
    /// and swimming without a swimming speed).
    ///
    /// The multiplier is the same for all three and the *waiver* is
    /// not: `DifficultTerrain` / `LowWall` are waived by
    /// `ActorInstance::ignores_difficult_terrain` and `Water` by
    /// `ActorInstance::swims_freely`. This function answers the price
    /// only; the pathfinder picks which waiver to ask about, keyed off
    /// `is_water`.
    pub fn movement_cost(self) -> f32 {
        match self {
            TerrainType::DifficultTerrain | TerrainType::LowWall | TerrainType::Water => 2.0,
            _ => 1.0,
        }
    }

    /// True if this tile's movement surcharge is the *swimming* one
    /// rather than the difficult-terrain one.
    ///
    /// Read by the pathfinder to pick which of the two waivers applies,
    /// and by `EncounterInstance::is_immersed` to decide whether a
    /// creature standing here is underwater. A named predicate rather
    /// than a bare `== TerrainType::Water` at both call sites for the
    /// same reason `blocks_sight` exists: the question the callers are
    /// asking is about the property, and a second wet tile (a future
    /// current, a flooded pit) should answer it by joining the match
    /// arm rather than by being missed at one of two sites.
    pub fn is_water(self) -> bool {
        matches!(self, TerrainType::Water)
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
    /// impassability already says. `Water` is excluded because 5e's
    /// Underwater Combat rules price being in the water entirely on the
    /// attacker's side — disadvantage on the swing, and a hard range
    /// cut — and never once as AC on the target. Granting cover here
    /// would tax the same shot twice for the same reason.
    pub fn grants_cover(self) -> bool {
        matches!(self, TerrainType::LowWall)
    }
}
