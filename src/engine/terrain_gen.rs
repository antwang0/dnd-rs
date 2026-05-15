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

impl TerrainGenParams {
    /// Open-floor `width × height` map with no procedural branching —
    /// every tile is Floor. Used as a clean baseline by tests that want
    /// to control placement without any random walls in the way.
    pub fn open(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            branch_depth: 0,
            branch_prob: 0.0,
        }
    }
}

fn collect_leaves<'a>(node: &'a mut BSPNode, leaves: &mut Vec<&'a mut BSPNode>) {
    let is_leaf = node.children.is_none();

    if is_leaf {
        leaves.push(node);
    } else {
        if let Some((child1, child2)) = &mut node.children {
            // this should always be true; I could not come up with a cleaner way to do this unfortunately
            collect_leaves(child1, leaves);
            collect_leaves(child2, leaves);
        }
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

        // maker doors
        let door_width_horizontal =
            rng.usize(MIN_WIDTH..(node.width - 1).min(MIN_WIDTH * 2));
        let door_offset_horizontal = rng.usize(0..node.width - door_width_horizontal);
        for i in 0..door_width_horizontal {
            terrain[idx(
                node.x + i + door_offset_horizontal,
                node.y + node.height - 1,
                params,
            )]
            .terrain_type = TerrainType::Floor;
        }

        let door_width_vertical = rng.usize(MIN_WIDTH..(node.height - 1).min(MIN_WIDTH * 2));
        let door_offset_vertical = rng.usize(0..node.height - door_width_vertical);
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

pub fn generate_terrain(params: &TerrainGenParams, rng: &mut Rng) -> Vec<TerrainInfo> {
    // TODO: modify terrain
    binary_space_partition(params, rng)
}
