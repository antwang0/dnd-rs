//! Terrain a spell writes onto the map, and takes back when it ends.
//!
//! 5e's wall spells are not areas of effect. Wall of Stone, Wall of
//! Force and Wall of Ice do not damage anybody and do not impose a
//! condition; what they do is put something *there*, and every one of
//! their RAW clauses is about the fact that the something is solid —
//! "nothing can physically pass through the wall", "the wall blocks
//! line of sight", "a creature can't move through it".
//!
//! The engine already had a layer that answers those questions: the
//! terrain map. It gates movement through `TerrainType::is_passable`
//! and sight through `TerrainType::blocks_sight`, and it is what the
//! pathfinder and the line-of-sight walk both read. What it did not
//! have was a way for anything but the generator to write to it —
//! `set_terrain_at`'s own docstring called itself "the door" and had no
//! callers.
//!
//! This module is what walks through it. A `ConjuredTerrain` is a
//! request to retype a list of tiles, plus the ledger of what was there
//! before, so the map can be handed back intact when the spell ends.
//!
//! ## Why not a zone
//!
//! A zone (`crate::engine::zones`) overlays the map: it adds clauses to
//! ground that is otherwise unchanged, and two of them can sit on the
//! same tile because neither of them owns it. Conjured terrain
//! *replaces* the map. That difference is not stylistic — it is what
//! makes a wall of stone block a line of sight, which no combination of
//! `ZoneEffect` fields can express, because the LOS walk reads terrain
//! and always did.
//!
//! The two layers are otherwise deliberate siblings, down to the
//! lifecycle: both are owned, both expire on a round-end tick, and both
//! are torn down by `drop_concentration` when the caster's grip fails.
//!
//! ## What it deliberately doesn't do
//!
//! **It never buries anybody.** A tile with a creature standing on it
//! is skipped when the new terrain is impassable. RAW pushes the
//! creature to one side of the wall instead; skipping the tile leaves a
//! creature-shaped gap in the wall, which is the same shape the rule
//! produces and needs no forced-movement resolution to reach. The gap
//! closes the moment the creature walks out of it — not RAW, and the
//! honest price of the simplification.
//!
//! **It hands back only what it still holds.** A tile is restored only
//! if it still carries the terrain this patch wrote. Two walls raised
//! across the same tile are unwound in either order without the second
//! one's stone being handed back as the first one's floor.

use crate::engine::terrain::TerrainType;
use crate::engine::types::Coordinate;

/// One spell's worth of retyped map.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConjuredTerrain {
    /// Unique per encounter, handed out by
    /// `EncounterInstance::conjure_terrain`. Callers building one pass
    /// `0` and let the encounter overwrite it, the same contract
    /// `install_zone` has.
    pub id: usize,
    pub name: &'static str,
    /// The caster, for the concentration teardown and the log line.
    pub owner_id: usize,
    /// What the listed tiles become.
    pub terrain_type: TerrainType,
    /// The tiles to retype. Read at install time and not afterwards —
    /// what the patch is actually holding lives in `restore`, which is
    /// the shorter list whenever a tile was off the map or occupied.
    pub tiles: Vec<Coordinate>,
    /// `(tile, what was there before)`, filled in by the encounter as
    /// it writes. The teardown ledger, and the only field that says
    /// what this patch really owns.
    pub restore: Vec<(Coordinate, TerrainType)>,
    /// Rounds left, decremented at each round end — the same clock the
    /// zone layer runs on.
    pub rounds_remaining: u32,
    /// True if the owner's concentration holds it up.
    pub concentration: bool,
}

impl ConjuredTerrain {
    /// A patch that hasn't been laid yet: no id, no ledger.
    pub fn new(
        name: &'static str,
        owner_id: usize,
        terrain_type: TerrainType,
        tiles: Vec<Coordinate>,
        rounds_remaining: u32,
        concentration: bool,
    ) -> Self {
        Self {
            id: 0,
            name,
            owner_id,
            terrain_type,
            tiles,
            restore: Vec::new(),
            rounds_remaining,
            concentration,
        }
    }

    /// True if this patch is holding `coord`.
    pub fn covers(&self, coord: Coordinate) -> bool {
        self.restore.iter().any(|(c, _)| *c == coord)
    }
}

/// The tiles of a straight, one-tile-thick wall through `anchor`,
/// running *across* the line from `caster_at` to `anchor` and extending
/// `reach` tiles to either side of it.
///
/// The orientation is derived rather than asked for, and that is the
/// whole reason a wall spell can keep a one-tile targeting schema. A
/// caster aiming a wall is aiming it at something: the useful wall is
/// the one that stands between the caster and the tile they picked, and
/// a prompt that asked for an angle as well as a point would be asking
/// for the answer it can already work out.
///
/// The perpendicular of a unit step `(dx, dy)` is `(-dy, dx)`, so a
/// wall aimed straight down a corridor stands across it, and one aimed
/// diagonally stands on the other diagonal. A wall aimed at the tile
/// the caster is standing on has no direction to be across; it runs
/// north-south, which is as good an answer as any and better than an
/// empty list.
pub fn wall_tiles(caster_at: Coordinate, anchor: Coordinate, reach: isize) -> Vec<Coordinate> {
    let toward = anchor - caster_at;
    let (dx, dy) = (toward.x.signum(), toward.y.signum());
    let step = if dx == 0 && dy == 0 {
        Coordinate::new(0, 1)
    } else {
        Coordinate::new(-dy, dx)
    };
    (-reach..=reach)
        .map(|k| anchor + Coordinate::new(step.x * k, step.y * k))
        .collect()
}

/// Every tile of the Chebyshev square of `radius` around `origin` — the
/// same footprint every burst in the engine measures, as a tile list.
/// For the spells that raise something solid over an area rather than
/// along a line.
pub fn block_tiles(origin: Coordinate, radius: isize) -> Vec<Coordinate> {
    let mut out = Vec::new();
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            out.push(origin + Coordinate::new(dx, dy));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A wall aimed straight down a row stands across it — a column,
    /// centred on the anchor.
    #[test]
    fn a_wall_stands_across_the_line_it_was_aimed_along() {
        let tiles = wall_tiles(Coordinate::new(2, 10), Coordinate::new(10, 10), 2);
        assert_eq!(
            tiles,
            vec![
                Coordinate::new(10, 8),
                Coordinate::new(10, 9),
                Coordinate::new(10, 10),
                Coordinate::new(10, 11),
                Coordinate::new(10, 12),
            ]
        );
    }

    /// Aimed up a column, it stands across as a row. (The listing runs
    /// right-to-left because the perpendicular of "north" is "west" —
    /// the order the tiles come out in carries no meaning, only the
    /// set.)
    #[test]
    fn the_orientation_follows_the_aim() {
        let tiles = wall_tiles(Coordinate::new(10, 2), Coordinate::new(10, 10), 1);
        assert_eq!(
            tiles,
            vec![
                Coordinate::new(11, 10),
                Coordinate::new(10, 10),
                Coordinate::new(9, 10),
            ]
        );
    }

    /// A diagonal aim gets the other diagonal, which is still a wall
    /// standing between the two of you.
    #[test]
    fn a_diagonal_aim_gets_the_other_diagonal() {
        let tiles = wall_tiles(Coordinate::new(0, 0), Coordinate::new(5, 5), 1);
        assert_eq!(
            tiles,
            vec![
                Coordinate::new(6, 4),
                Coordinate::new(5, 5),
                Coordinate::new(4, 6),
            ]
        );
    }

    /// A wall aimed at your own feet has no direction to be across. It
    /// gets one anyway rather than being nothing at all.
    #[test]
    fn a_wall_aimed_at_nowhere_still_stands_somewhere() {
        let here = Coordinate::new(7, 7);
        let tiles = wall_tiles(here, here, 1);
        assert_eq!(tiles.len(), 3);
        assert!(tiles.contains(&here));
    }

    #[test]
    fn a_block_is_the_chebyshev_square() {
        assert_eq!(block_tiles(Coordinate::new(5, 5), 0), vec![Coordinate::new(5, 5)]);
        assert_eq!(block_tiles(Coordinate::new(5, 5), 1).len(), 9);
        assert_eq!(block_tiles(Coordinate::new(5, 5), 2).len(), 25);
    }

    /// The ledger is what a patch owns — not the tile list it asked
    /// for, which can name tiles it never got.
    #[test]
    fn a_patch_covers_what_it_actually_wrote() {
        let mut patch = ConjuredTerrain::new(
            "test wall",
            0,
            TerrainType::Wall,
            vec![Coordinate::new(1, 1), Coordinate::new(2, 2)],
            5,
            false,
        );
        assert!(!patch.covers(Coordinate::new(1, 1)), "nothing written yet");
        patch
            .restore
            .push((Coordinate::new(1, 1), TerrainType::Floor));
        assert!(patch.covers(Coordinate::new(1, 1)));
        assert!(!patch.covers(Coordinate::new(2, 2)));
    }
}
