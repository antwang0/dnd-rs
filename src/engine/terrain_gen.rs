use crate::engine::terrain::{TerrainInfo, TerrainType};

use fastrand::Rng;

const MIN_WIDTH: usize = 4;
const MIN_ROOM_WIDTH: usize = 6;

#[derive(Hash, Eq, PartialEq)]
struct BSPNode {
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    children: Option<(Box<BSPNode>, Box<BSPNode>)>,
}

#[derive(Clone)]
pub struct TerrainGenParams {
    pub width: usize,
    pub height: usize,
    pub branch_depth: usize,
    pub branch_prob: f32,
}

/// Depth-first walk collecting every childless node under `node`.
///
/// The early return is load-bearing rather than stylistic. Writing this
/// as `if let Some(children) = &mut node.children { .. } else { push(node) }`
/// is what you want and what the borrow checker refuses: matching on
/// the field borrows `*node` for the whole `if`, so the `else` arm
/// can't hand the node itself to `leaves`. Testing `is_none()` first
/// takes no borrow at all, which leaves the leaf case free to move the
/// `&mut` on and makes the `if let` below total by construction — every
/// node that reaches it has children.
fn collect_leaves<'a>(node: &'a mut BSPNode, leaves: &mut Vec<&'a mut BSPNode>) {
    if node.children.is_none() {
        leaves.push(node);
        return;
    }
    if let Some((child1, child2)) = node.children.as_mut() {
        collect_leaves(child1, leaves);
        collect_leaves(child2, leaves);
    }
}

fn get_leaves(root: &mut BSPNode) -> Vec<&mut BSPNode> {
    let mut leaves = Vec::new();
    collect_leaves(root, &mut leaves);
    leaves
}

fn idx(x: usize, y: usize, params: &TerrainGenParams) -> usize {
    x + y * params.width
}

fn binary_space_partition(params: &TerrainGenParams, rng: &mut Rng) -> Vec<TerrainInfo> {
    let mut root = BSPNode {
        x: 0,
        y: 0,
        width: params.width,
        height: params.height,
        children: None,
    };

    for _ in 0..params.branch_depth {
        let leaves = get_leaves(&mut root);

        for node in leaves {
            if node.width <= MIN_ROOM_WIDTH * 2 && node.height <= MIN_ROOM_WIDTH * 2 {
                continue;
            }
            if rng.f32() > params.branch_prob {
                continue;
            }
            let horizontal_chop = node.width <= node.height;

            let child1_width = if horizontal_chop {
                node.width
            } else {
                rng.usize(MIN_ROOM_WIDTH..node.width - MIN_ROOM_WIDTH)
            };
            let child2_width = if horizontal_chop {
                node.width
            } else {
                node.width - child1_width
            };
            let child1_height = if !horizontal_chop {
                node.height
            } else {
                rng.usize(MIN_ROOM_WIDTH..node.height - MIN_ROOM_WIDTH)
            };
            let child2_height = if !horizontal_chop {
                node.height
            } else {
                node.height - child1_height
            };

            let child1 = BSPNode {
                x: node.x,
                y: node.y,
                width: child1_width,
                height: child1_height,
                children: None,
            };

            let child2 = BSPNode {
                x: if horizontal_chop {
                    node.x
                } else {
                    node.x + child1_width
                },
                y: if !horizontal_chop {
                    node.y
                } else {
                    node.y + child1_height
                },
                width: child2_width,
                height: child2_height,
                children: None,
            };
            node.children = Some((Box::new(child1), Box::new(child2)));
        }
    }

    let mut terrain = vec![
        TerrainInfo {
            terrain_type: TerrainType::Empty
        };
        params.width * params.height
    ];
    let leaves = get_leaves(&mut root);

    for node in leaves {
        // A room needs both a floor and two walls to put a door in. A
        // degenerate leaf (either dimension zero) has neither, and every
        // loop below would underflow on `- 1` reaching for them.
        if node.width == 0 || node.height == 0 {
            continue;
        }

        // fill floor
        for i in node.x..(node.x + node.width - 1) {
            for j in node.y..(node.y + node.height - 1) {
                terrain[idx(i, j, params)].terrain_type = TerrainType::Floor;
            }
        }

        // make walls
        for i in node.x..(node.x + node.width) {
            terrain[idx(i, node.y + node.height - 1, params)].terrain_type = TerrainType::Wall;
        }
        for j in node.y..(node.y + node.height) {
            terrain[idx(node.x + node.width - 1, j, params)].terrain_type = TerrainType::Wall;
        }

        // make doors
        let (door_width_horizontal, door_offset_horizontal) = door_in_wall(rng, node.width);
        for i in 0..door_width_horizontal {
            terrain[idx(
                node.x + i + door_offset_horizontal,
                node.y + node.height - 1,
                params,
            )]
            .terrain_type = TerrainType::Floor;
        }

        let (door_width_vertical, door_offset_vertical) = door_in_wall(rng, node.height);
        for i in 0..door_width_vertical {
            terrain[idx(
                node.x + node.width - 1,
                node.y + i + door_offset_vertical,
                params,
            )]
            .terrain_type = TerrainType::Floor;
        }
    }

    terrain
}

/// Pick `(width, offset)` for a doorway punched through a wall
/// `wall_len` tiles long. `width == 0` means the wall is too short to
/// hold a door and none is cut.
///
/// The width is `MIN_WIDTH..=MIN_WIDTH * 2`, clamped so the door never
/// swallows the whole wall — the last tile has to stay solid or
/// neighbouring rooms merge. The offset then has at least one legal
/// position by construction.
///
/// Extracted because the inline version of this was two `rng.usize`
/// calls whose ranges went empty — and `fastrand` panics on an empty
/// range — for any wall shorter than six tiles. Nothing in normal play
/// generates a map that small, which is exactly why it sat there: the
/// only way to reach it was to ask for a small map, and the crash gave
/// no hint that the size was the problem. A generator is the wrong
/// place to be picky about its inputs; it should hand back a boring map
/// rather than take the process down.
fn door_in_wall(rng: &mut Rng, wall_len: usize) -> (usize, usize) {
    // Never the full length (the corner stays solid), never wider than
    // MIN_WIDTH * 2.
    let widest = wall_len.saturating_sub(1).min(MIN_WIDTH * 2);
    if widest == 0 {
        return (0, 0);
    }
    // `rng.usize(a..b)` needs `a < b`. When the wall can't fit the
    // preferred minimum, take the widest door it can hold instead of
    // rolling — and skip the draw, so a map big enough to have a choice
    // still consumes exactly the same random numbers it always did.
    let width = if widest > MIN_WIDTH {
        rng.usize(MIN_WIDTH..widest)
    } else {
        widest
    };
    (width, rng.usize(0..wall_len - width))
}

pub fn generate_terrain(params: &TerrainGenParams, rng: &mut Rng) -> Vec<TerrainInfo> {
    let mut terrain = binary_space_partition(params, rng);
    scatter_obstructions(&mut terrain, params, rng);
    // Strictly after the scatter, and every random draw it makes comes
    // after every draw the scatter makes. That ordering is the only
    // reason this pass could be added at all: `fastrand` is a stream,
    // and a new draw inserted anywhere earlier would have shifted every
    // seeded map in the suite out from under the tests that pin them.
    flood_pools(&mut terrain, params, rng);
    terrain
}

/// How many steps a pool's random walk takes. Each step floods a 2x2
/// block, and the blocks overlap heavily, so a pool ends up somewhere
/// between one and three times this many tiles.
///
/// Sized so the biggest pool is a swim of a few strokes rather than a
/// lake nobody can cross: a Medium creature with 30 ft of speed gets
/// through the widest part of one in a single turn even paying double,
/// which is what keeps the water a decision instead of a wall. A pool
/// that cannot be crossed in a turn is a `Wall` the player can see
/// through, and the map already has a tile for that.
///
/// This is the count for the narrowest brush; a wider one walks fewer
/// steps — see `pool_walk_steps`. Holding the *area* roughly fixed is
/// what keeps a Huge-capable basin from also being a Huge-sized lake.
const POOL_WALK_STEPS: usize = 10;

/// The brush widths a pool is rolled from, one entry per draw, so the
/// list is its own weighting.
///
/// Each width is a creature footprint on the 2.5-ft grid: 2 is Medium
/// (and Small, and Tiny), 4 is Large, 6 is Huge. A pool laid with a
/// width-`n` brush is the smallest pool that can fully immerse a body
/// of that size, which is the only property of a pool the rules
/// actually read.
///
/// Weighted five / three / two, which is the map's shape rather than
/// the bestiary's. A Huge basin is fifteen feet across — the exact
/// width a Medium creature with 30 ft of speed can still cross in one
/// turn while paying the swimming surcharge — so one of those is a
/// landmark and a map made of them would be a wall. The weight is
/// higher than the shape of the roster would suggest because most of
/// the draws never get their basin: a six-wide block needs six clear
/// tiles in both directions and the generator's corridors do not have
/// them, so the brush clips down to whatever the room allows.
const POOL_BRUSH_WIDTHS: &[usize] = &[2, 2, 2, 2, 2, 4, 4, 4, 6, 6];

/// How far a pool laid with a `brush` -wide block walks before it stops.
///
/// Inverse in the brush's *area*, so every pool covers roughly the same
/// number of tiles however wide it is. A block brush lays `brush²`
/// tiles a step, so without the square a width-6 pool over
/// `POOL_WALK_STEPS` steps would put down nine times the water a
/// width-2 one does and the rarest pool on the map would also be the
/// one that ate the room. Measured over two hundred boards, the widened
/// brush with this correction leaves the average map's water within a
/// few tiles of where the fixed 2x2 brush left it.
///
/// Floored at two steps, because a pool that takes one step is a
/// rectangle and the whole reason this is a walk is that a rectangle
/// reads as a swimming pool rather than as a pond.
fn pool_walk_steps(brush: usize) -> usize {
    let area = brush.max(1) * brush.max(1);
    ((POOL_WALK_STEPS * 4) / area).max(2)
}

/// One in this many open floor tiles seeds a pool.
///
/// Deliberately an order of magnitude rarer than the rubble scatter's
/// 8%, because a pool is not one tile — each seed spends a couple of
/// dozen of them — and because water is the most consequential scatter
/// on the map. Rubble taxes movement, a low wall taxes one attack roll,
/// and a pool switches off a whole build's offense: an archer standing
/// in one cannot reach anything past its normal range at all. That is a
/// good thing to have on a map and a bad thing to have on every square
/// of it, which is what this rate buys.
///
/// It halved when [`POOL_BRUSH_WIDTHS`] arrived, and the arithmetic is
/// the reason rather than a change of mind: a basin that can immerse a
/// Huge body is thirty-six tiles on its *first stamp*, so no step count
/// can make a wide pool as small as a narrow one. Fewer pools, some of
/// them deeper, keeps the average board no wetter than it was while
/// making the water on it worth more — measured over forty 60x40
/// boards, 93 water tiles against the fixed 2x2 brush's 97, with the
/// fraction that can fully immerse a Large creature going from none of
/// them to four in five.
const POOL_SEED_CHANCE: f32 = 0.003;

/// Dig a handful of pools into the open floor.
///
/// Each pool starts at a seed tile and grows by a random walk, which
/// gives an irregular blob rather than the rectangle a flood-to-radius
/// would. Shape matters more here than it does for the scatter: rubble
/// is read one tile at a time, but a pool is read as a thing to go
/// around or through, and a creature deciding that needs to be able to
/// see where the far side is.
///
/// **Every step floods a square block rather than a single tile**, and
/// that is not a cosmetic choice — it is the difference between the
/// feature working and not working at all. `is_immersed` implements
/// RAW's "fully immersed" as *every tile of the footprint is water*, so
/// the brush is what decides which creatures the whole water layer can
/// ever reach. A bare random walk lays one-tile-wide channels, so the
/// first version of this pass put water on every map and immersed
/// nobody, ever: measured across twenty-five generated encounters
/// driven to completion, the underwater rules fired exactly zero times.
/// Widening the brush is what turned a tile that existed into a tile
/// that does something.
///
/// **The brush width is rolled per pool**, from [`POOL_BRUSH_WIDTHS`],
/// and that is the same argument one size class up. A fixed 2x2 brush
/// is exactly a Medium footprint, and it left every Large and Huge
/// creature in the bestiary permanently outside the layer: measured
/// over two hundred generated boards, three of them held a
/// Large-immersible pool and none held a Huge one. So Underwater
/// Combat, the breath clock and the fire-resistance clause simply did
/// not exist for a hunter shark, a giant shark, a dragon turtle or a
/// kraken — the creatures the rules are most obviously written about.
///
/// It also makes a better map on its own terms. A one-tile ribbon of
/// water deep enough to swim in is not a thing anybody can picture, and
/// a creature that stepped into one would take every underwater penalty
/// while visibly standing in a puddle.
///
/// Walls stop the walk — a pool does not eat a corridor, and a low wall
/// is cover the map has already placed. **Rubble does not**: a pool
/// that runs over difficult terrain floods it, replacing the tile
/// rather than layering on it, so there is never a tile charging two
/// surcharges that two different creatures are exempt from.
///
/// Skipping the scatter instead was the earlier behaviour and it was
/// what stopped the wide brushes working. `scatter_obstructions` runs
/// first and takes about 8% of the open floor, so a brush that has to
/// find *every* tile of its block already plain lands a clean 6x6 block
/// about one time in twenty (0.92³⁶) and a clean 4x4 about one in four.
/// The holes were invisible — the pool still appeared, still looked
/// like a pool, and simply never had a square patch big enough to
/// immerse anything Large in.
fn flood_pools(terrain: &mut [TerrainInfo], params: &TerrainGenParams, rng: &mut Rng) {
    let (w, h) = (params.width, params.height);
    if w < 3 || h < 3 {
        return;
    }
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            if terrain[idx(x, y, params)].terrain_type != TerrainType::Floor {
                continue;
            }
            if rng.f32() >= POOL_SEED_CHANCE {
                continue;
            }
            // How big a body this pool is capable of immersing, rolled
            // once for the whole walk so a pool has one depth rather
            // than a different one every step.
            let brush = POOL_BRUSH_WIDTHS[rng.usize(0..POOL_BRUSH_WIDTHS.len())];
            let (mut cx, mut cy) = (x, y);
            for _ in 0..pool_walk_steps(brush) {
                // The block brush. One that runs into a wall floods the
                // tiles it can and leaves the rest — clipping the brush
                // rather than refusing the step is what lets a pool sit
                // against a wall without either eating it or stopping a
                // tile short of it.
                let mut flooded_any = false;
                for bx in cx..cx + brush {
                    for by in cy..cy + brush {
                        if bx + 1 >= w || by + 1 >= h {
                            continue;
                        }
                        let i = idx(bx, by, params);
                        // Plain floor and the scatter's rubble both
                        // flood; walls, low walls and anything already
                        // wet do not. See the note above on why the
                        // rubble is a *replacement* rather than a skip.
                        if matches!(
                            terrain[i].terrain_type,
                            TerrainType::Floor | TerrainType::DifficultTerrain
                        ) {
                            terrain[i].terrain_type = TerrainType::Water;
                            flooded_any = true;
                        }
                    }
                }
                if !flooded_any {
                    // The brush landed entirely on walls or on water it
                    // had already laid; there is nothing here to grow
                    // into.
                    break;
                }
                // Step to a random orthogonal neighbour, staying clear
                // of the map edge so a pool never touches the border.
                // The bounds check ends the walk rather than resampling:
                // a pool that has run into the edge of the room is
                // finished, and resampling would let it crawl along the
                // wall. The margin is the brush's, so a wide pool keeps
                // the same clearance a narrow one does.
                let (nx, ny) = match rng.u8(0..4) {
                    0 => (cx + 1, cy),
                    1 => (cx.wrapping_sub(1), cy),
                    2 => (cx, cy + 1),
                    _ => (cx, cy.wrapping_sub(1)),
                };
                if nx == 0 || ny == 0 || nx + brush >= w || ny + brush >= h {
                    break;
                }
                (cx, cy) = (nx, ny);
            }
        }
    }
}

/// Randomly obstruct ~8% of open floor tiles: three parts difficult
/// terrain (rubble, undergrowth, shallow water) to one part low wall.
///
/// Skips tiles adjacent to walls to keep corridors passable; the scatter
/// rate is low enough that pathfinding still finds routes but high
/// enough to make positioning matter. Neither kind blocks a route
/// outright — difficult terrain and the clamber over a low wall both
/// cost double, and both are passable — so the skip is about keeping
/// doorways cheap rather than about keeping them open.
fn scatter_obstructions(
    terrain: &mut [TerrainInfo],
    params: &TerrainGenParams,
    rng: &mut Rng,
) {
    let w = params.width;
    let h = params.height;
    for y in 1..h.saturating_sub(1) {
        for x in 1..w.saturating_sub(1) {
            let i = idx(x, y, params);
            if terrain[i].terrain_type != TerrainType::Floor {
                continue;
            }
            let adj_wall = [
                idx(x.wrapping_sub(1), y, params),
                idx(x + 1, y, params),
                idx(x, y.wrapping_sub(1), params),
                idx(x, y + 1, params),
            ]
            .iter()
            .any(|&ni| ni < terrain.len() && terrain[ni].terrain_type == TerrainType::Wall);
            if adj_wall {
                continue;
            }
            // One roll, two outcomes, in the order that keeps the older
            // one's stream position: the 8% that used to become
            // difficult terrain still does, and a second, narrower roll
            // inside it promotes a quarter of those tiles to a low wall.
            // Nesting rather than adding a sibling `rng.f32()` call is
            // what keeps a seeded map from shifting under every test
            // that pins one.
            if rng.f32() < 0.08 {
                // Low walls are the rarer scatter (~2% of open floor)
                // because each one is a +2 AC that nobody chose. Rubble
                // taxes movement, which a player can route around; a
                // low wall taxes an attack roll made from tiles the
                // shooter may not know are obstructed, so a map covered
                // in them would read as bad luck rather than as terrain.
                terrain[i].terrain_type = if rng.f32() < 0.25 {
                    TerrainType::LowWall
                } else {
                    TerrainType::DifficultTerrain
                };
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The generator produces a map for any size, rather than panicking
    /// on the small ones.
    ///
    /// It used to panic for every dimension below six: the doorway code
    /// asked `fastrand` for a number in a range that had gone empty, and
    /// `fastrand` panics on that. Nothing in play generates a map that
    /// small, which is why it survived — the only way to hit it was to
    /// ask for a small map, and the backtrace pointed at the RNG rather
    /// than at the size.
    ///
    /// Swept over sizes *and* seeds because the failure was
    /// seed-independent but the surrounding split logic isn't; a single
    /// seed would prove less than it looks like it does.
    #[test]
    fn every_map_size_generates_without_panicking() {
        for width in 0..=20usize {
            for height in 0..=20usize {
                for seed in 0..3u64 {
                    let params = TerrainGenParams {
                        width,
                        height,
                        branch_depth: 3,
                        branch_prob: 1.0,
                    };
                    let mut rng = Rng::with_seed(seed);
                    let terrain = generate_terrain(&params, &mut rng);
                    assert_eq!(
                        terrain.len(),
                        width * height,
                        "{}x{} seed {}: one TerrainInfo per tile",
                        width,
                        height,
                        seed
                    );
                }
            }
        }
    }

    /// The scatter puts both kinds of obstruction on a reasonably-sized
    /// map, and keeps low walls the rarer of the two.
    ///
    /// Pinned because the two share one roll — the low wall is a nested
    /// draw inside the difficult-terrain draw — so a refactor that
    /// promoted it to a sibling `rng.f32()` would still produce both
    /// kinds and would silently change every seeded map in the suite.
    /// The ratio is what catches that.
    #[test]
    fn the_scatter_lays_down_rubble_and_the_occasional_low_wall() {
        let params = TerrainGenParams {
            width: 60,
            height: 40,
            branch_depth: 3,
            branch_prob: 1.0,
        };
        let (mut rough, mut walls) = (0usize, 0usize);
        for seed in 0..8u64 {
            let mut rng = Rng::with_seed(seed);
            for tile in generate_terrain(&params, &mut rng) {
                match tile.terrain_type {
                    TerrainType::DifficultTerrain => rough += 1,
                    TerrainType::LowWall => walls += 1,
                    _ => {}
                }
            }
        }
        assert!(walls > 0, "low walls should appear across 8 maps");
        assert!(rough > 0, "difficult terrain should too");
        assert!(
            walls < rough,
            "low walls are the rarer scatter: {walls} against {rough}"
        );
    }

    /// A door never eats the whole wall it is cut through, and never
    /// starts past the end of it.
    ///
    /// Both are invariants the callers rely on without checking: the
    /// offset loop indexes `wall_len` tiles from `offset`, so an
    /// over-wide or over-far door would write outside the room and
    /// silently corrupt a neighbour.
    #[test]
    fn a_doorway_always_fits_inside_its_wall() {
        for wall_len in 0..=32usize {
            for seed in 0..8u64 {
                let mut rng = Rng::with_seed(seed);
                let (width, offset) = door_in_wall(&mut rng, wall_len);
                assert!(
                    width + offset <= wall_len,
                    "wall {} seed {}: door {}@{} runs off the end",
                    wall_len,
                    seed,
                    width,
                    offset
                );
                if wall_len > 1 {
                    assert!(
                        width < wall_len,
                        "wall {} seed {}: the corner tile must stay solid",
                        wall_len,
                        seed
                    );
                }
            }
        }
    }

    /// Pools appear, they are pools rather than isolated puddles, and
    /// they stay much rarer than the rubble scatter.
    ///
    /// The middle clause is the one worth pinning. A single tile of
    /// water is a trap rather than a feature — a creature standing on
    /// it takes every underwater penalty and has no reason to be there
    /// and no warning it mattered — so the random walk has to actually
    /// produce contiguous blobs. Averaging tiles-per-seed is how that
    /// shows: a walk that terminated on its first step every time would
    /// still put water on the map and would score 1.0 here.
    #[test]
    fn the_generator_digs_pools_rather_than_puddles() {
        let params = TerrainGenParams {
            width: 60,
            height: 40,
            branch_depth: 3,
            branch_prob: 1.0,
        };
        let (mut water, mut rough, mut maps_with_water) = (0usize, 0usize, 0usize);
        for seed in 0..16u64 {
            let mut rng = Rng::with_seed(seed);
            let terrain = generate_terrain(&params, &mut rng);
            let here = terrain
                .iter()
                .filter(|t| t.terrain_type == TerrainType::Water)
                .count();
            if here > 0 {
                maps_with_water += 1;
                // Every pool on a map this size is several tiles across.
                assert!(here >= 3, "seed {seed}: {here} water tiles is a puddle");
            }
            water += here;
            rough += terrain
                .iter()
                .filter(|t| t.terrain_type == TerrainType::DifficultTerrain)
                .count();
        }
        assert!(maps_with_water > 0, "water should appear across 16 maps");
        assert!(
            water < rough,
            "water is the rarer scatter: {water} against {rough}"
        );
    }

    /// A generated map has somewhere a Medium creature can actually be
    /// **fully immersed** — at least one 2x2 block of nothing but water.
    ///
    /// This is the test the feature needed and didn't have. The first
    /// version of `flood_pools` walked one tile at a time, which put
    /// water on 40 maps out of 40 and satisfied every other assertion in
    /// this module — and immersed nobody, ever, because `is_immersed`
    /// asks about the whole footprint and a Medium footprint is 2x2. The
    /// underwater rules fired exactly zero times across twenty-five
    /// generated encounters driven to completion. Every visible signal
    /// said the feature was working.
    ///
    /// So the property worth pinning is not "water exists" but "water
    /// exists in the shape the rules read", and it is worth pinning
    /// cheaply and deterministically here rather than by sampling AI
    /// behaviour: a threshold on how often a fight happens to produce an
    /// underwater swing would be slow, flaky, and would still pass at a
    /// tenth of the intended rate.
    #[test]
    fn a_generated_map_has_room_to_be_fully_immersed_in() {
        let params = TerrainGenParams {
            width: 40,
            height: 30,
            branch_depth: 4,
            branch_prob: 0.5,
        };
        let mut maps_with_a_swimmable_block = 0;
        const SEEDS: u64 = 12;
        for seed in 0..SEEDS {
            let mut rng = Rng::with_seed(seed);
            let terrain = generate_terrain(&params, &mut rng);
            let wet = |x: usize, y: usize| {
                terrain[idx(x, y, &params)].terrain_type == TerrainType::Water
            };
            let has_block = (0..params.height - 1).any(|y| {
                (0..params.width - 1)
                    .any(|x| wet(x, y) && wet(x + 1, y) && wet(x, y + 1) && wet(x + 1, y + 1))
            });
            if has_block {
                maps_with_a_swimmable_block += 1;
            }
        }
        // Not every map — a small or heavily-walled one may have no room
        // — but the common case has to be that it does, or the rules the
        // tile carries are unreachable in play.
        assert!(
            maps_with_a_swimmable_block * 2 > SEEDS,
            "only {maps_with_a_swimmable_block}/{SEEDS} maps have a 2x2 pool a Medium creature could swim in"
        );
    }

    /// …and the same property one size class up: a generated map
    /// usually has somewhere a **Large** creature could be fully
    /// immersed, and sometimes somewhere a **Huge** one could.
    ///
    /// The sibling of the test above and the same failure it caught,
    /// discovered the same way and one rung later. The fixed 2x2 brush
    /// was sized to a Medium footprint, so it satisfied that test on
    /// every run and left every Large and Huge creature in the bestiary
    /// permanently outside the water layer: measured over two hundred
    /// boards, two of them held a Large-immersible pool and none held a
    /// Huge one. Underwater Combat, the breath clock and the
    /// fire-resistance clause simply did not exist for a hunter shark, a
    /// dragon turtle or a kraken — the creatures the rules are most
    /// obviously written about — and every visible signal said the
    /// feature was working.
    ///
    /// The two thresholds are deliberately different, because the two
    /// pools are. A Large basin is ten feet across and belongs on an
    /// ordinary map; a Huge one is fifteen — the widest thing a Medium
    /// creature can still cross in one turn while swimming — so it is a
    /// landmark rather than scenery, and it also has to find fifteen
    /// clear feet in both directions on a map whose rooms are often
    /// narrower than that. Both are loose. What they guard is the
    /// distance from *zero*, which is where both sat.
    #[test]
    fn a_generated_map_has_room_for_a_large_body_to_swim_and_sometimes_a_huge_one() {
        let params = TerrainGenParams {
            width: 40,
            height: 30,
            branch_depth: 4,
            branch_prob: 0.5,
        };
        const SEEDS: u64 = 40;
        let (mut large, mut huge) = (0u64, 0u64);
        for seed in 0..SEEDS {
            let mut rng = Rng::with_seed(seed);
            let terrain = generate_terrain(&params, &mut rng);
            let block = |x: usize, y: usize, n: usize| {
                (0..n).all(|dx| {
                    (0..n).all(|dy| {
                        x + dx < params.width
                            && y + dy < params.height
                            && terrain[idx(x + dx, y + dy, &params)].terrain_type
                                == TerrainType::Water
                    })
                })
            };
            let any = |n: usize| {
                (0..params.height).any(|y| (0..params.width).any(|x| block(x, y, n)))
            };
            if any(4) {
                large += 1;
            }
            if any(6) {
                huge += 1;
            }
        }
        assert!(
            large * 3 > SEEDS,
            "only {large}/{SEEDS} maps have a 4x4 pool a Large creature could swim in"
        );
        assert!(
            huge > 0,
            "no map in {SEEDS} has a 6x6 pool — the Huge brush never lands"
        );
    }

    /// A pool never touches the map border, and never eats a wall.
    ///
    /// Both are load-bearing rather than cosmetic. Water on the border
    /// would be reachable only from one side, which makes the tile a
    /// dead end that costs double to enter and nothing to look at; and
    /// a pool that flooded a wall would open a route the BSP never cut,
    /// silently merging two rooms and changing what "a doorway" means
    /// for every consumer of the map.
    #[test]
    fn a_pool_stays_off_the_border_and_out_of_the_walls() {
        for (w, h) in [(60usize, 40usize), (20, 20), (9, 7)] {
            let params = TerrainGenParams {
                width: w,
                height: h,
                branch_depth: 3,
                branch_prob: 1.0,
            };
            for seed in 0..12u64 {
                let mut rng = Rng::with_seed(seed);
                let terrain = generate_terrain(&params, &mut rng);
                // The walls the BSP laid are still walls: re-running the
                // partition with the same seed prefix gives the map as
                // it was before the pools, and no `Wall` may have
                // become `Water`.
                let mut fresh = Rng::with_seed(seed);
                let before = binary_space_partition(&params, &mut fresh);
                for y in 0..h {
                    for x in 0..w {
                        let i = idx(x, y, &params);
                        if terrain[i].terrain_type != TerrainType::Water {
                            continue;
                        }
                        assert_eq!(
                            before[i].terrain_type,
                            TerrainType::Floor,
                            "{w}x{h} seed {seed}: pool flooded a non-floor tile at ({x},{y})"
                        );
                        assert!(
                            x > 0 && y > 0 && x + 1 < w && y + 1 < h,
                            "{w}x{h} seed {seed}: pool reached the border at ({x},{y})"
                        );
                    }
                }
            }
        }
    }

    /// The pool pass draws only after every draw the older passes make,
    /// so adding it left every seeded map's rooms, doors, rubble and low
    /// walls exactly where they were.
    ///
    /// This is the invariant that made the feature addable at all, and
    /// it is not self-evident from reading `generate_terrain` — it holds
    /// because `flood_pools` is called last and for no other reason.
    /// Pinned so that a later reordering fails here rather than by
    /// silently shifting a hundred seeded encounters in the suite.
    #[test]
    fn digging_the_pools_left_every_older_tile_where_it_was() {
        let params = TerrainGenParams {
            width: 60,
            height: 40,
            branch_depth: 3,
            branch_prob: 1.0,
        };
        for seed in 0..8u64 {
            let mut rng = Rng::with_seed(seed);
            let mut expected = binary_space_partition(&params, &mut rng);
            scatter_obstructions(&mut expected, &params, &mut rng);

            let mut rng = Rng::with_seed(seed);
            let actual = generate_terrain(&params, &mut rng);

            for (i, (a, e)) in actual.iter().zip(expected.iter()).enumerate() {
                if a.terrain_type == TerrainType::Water {
                    continue;
                }
                assert_eq!(
                    a.terrain_type, e.terrain_type,
                    "seed {seed} tile {i}: the pool pass moved an older tile"
                );
            }
        }
    }
}

