use crate::placed::Placed;

/// Edge length of the unit the mod rebuilds/draws independently.
pub const SECTION: i32 = 16;

/// Schematica's layer slider. Layers are schematic-local Y (0 = bottom);
/// rotation and mirroring never change Y so they are stable across placements.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum LayerMode {
    All,
    Single(i32),
    /// Inclusive on both ends.
    Range {
        min: i32,
        max: i32,
    },
}

impl LayerMode {
    pub fn includes(&self, local_y: i32) -> bool {
        match *self {
            LayerMode::All => true,
            LayerMode::Single(y) => local_y == y,
            LayerMode::Range { min, max } => (min..=max).contains(&local_y),
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct VisibleBlock {
    pub pos: [i32; 3],
    pub palette_index: u32,
}

const NEIGHBORS: [[i32; 3]; 6] = [
    [1, 0, 0],
    [-1, 0, 0],
    [0, 1, 0],
    [0, -1, 0],
    [0, 0, 1],
    [0, 0, -1],
];

/// A block is exposed when some face-neighbour is outside the schematic, air,
/// or hidden by the active layer filter (so single-layer views stay solid).
pub fn is_exposed(placed: &Placed, pos: [i32; 3], layers: LayerMode) -> bool {
    let oy = placed.placement().origin[1];
    NEIGHBORS.iter().any(|d| {
        let n = [pos[0] + d[0], pos[1] + d[1], pos[2] + d[2]];
        match placed.palette_index_at(n) {
            None => true,
            Some(i) => placed.is_air_index(i) || !layers.includes(n[1] - oy),
        }
    })
}

/// Section coordinates (`pos.div_euclid(16)`) overlapping the placed schematic.
pub fn sections_overlapping(placed: &Placed) -> Vec<[i32; 3]> {
    let (min, max) = placed.bounds();
    let lo = [0, 1, 2].map(|i| min[i].div_euclid(SECTION));
    let hi = [0, 1, 2].map(|i| (max[i] - 1).div_euclid(SECTION));
    let mut out = Vec::new();
    for y in lo[1]..=hi[1] {
        for z in lo[2]..=hi[2] {
            for x in lo[0]..=hi[0] {
                out.push([x, y, z]);
            }
        }
    }
    out
}

/// Non-air blocks of one section that should be drawn.
pub fn visible_in_section(
    placed: &Placed,
    section: [i32; 3],
    layers: LayerMode,
    cull_hidden: bool,
) -> Vec<VisibleBlock> {
    let (bmin, bmax) = placed.bounds();
    let smin = section.map(|c| c * SECTION);
    let lo = [0, 1, 2].map(|i| smin[i].max(bmin[i]));
    let hi = [0, 1, 2].map(|i| (smin[i] + SECTION).min(bmax[i]));
    let oy = placed.placement().origin[1];
    let mut out = Vec::new();
    for y in lo[1]..hi[1] {
        if !layers.includes(y - oy) {
            continue;
        }
        for z in lo[2]..hi[2] {
            for x in lo[0]..hi[0] {
                let pos = [x, y, z];
                let Some(idx) = placed.palette_index_at(pos) else {
                    continue;
                };
                if placed.is_air_index(idx) {
                    continue;
                }
                if cull_hidden && !is_exposed(placed, pos, layers) {
                    continue;
                }
                out.push(VisibleBlock {
                    pos,
                    palette_index: idx,
                });
            }
        }
    }
    out
}
