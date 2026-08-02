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
    terrain
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
}
