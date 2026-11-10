use crate::{budget, drawing as draw, terrain};
use schematica_core::{BlockState, CellStatus, LayerMode, Placement, Schematic, StateValue, Tally};
use serde_json::{Map, json};

const TILE_EDGE: i32 = 4;
const DEFAULT_LIGHT: u32 = 0xffff0000;

#[derive(Default)]
pub struct Preview {
    placement: Option<Placement>,
    palette: Vec<draw::NamedBlock>,
    layers: Option<LayerMode>,
}

impl Preview {
    pub fn new(placement: Placement, layers: LayerMode) -> Self {
        Self {
            placement: Some(placement),
            palette: Vec::new(),
            layers: Some(layers),
        }
    }

    pub fn prepare(&mut self, schematic: &Schematic) -> bool {
        let placement = self.placement.unwrap();
        for state in schematic.palette().iter().skip(self.palette.len()).take(4) {
            if budget::remaining_fuel() < 500_000 {
                break;
            }
            let state = placement.transform_state(state);
            self.palette.push(draw::NamedBlock {
                states: canonical(&state),
                name: state.name,
            });
        }
        self.palette.len() == schematic.palette().len()
    }

    pub fn palette_ready(&self, schematic: &Schematic) -> bool {
        self.palette.len() == schematic.palette().len()
    }

    pub fn material_indices(&self, name: &str) -> Vec<usize> {
        self.palette
            .iter()
            .enumerate()
            .filter_map(|(index, block)| (block.name == name).then_some(index))
            .collect()
    }

    pub fn tiles(&self, size: [i32; 3]) -> Vec<Tile> {
        let p = self.placement.unwrap();
        let (mut min, mut max) = p.world_bounds(size);
        match self.layers.unwrap_or(LayerMode::All) {
            LayerMode::All => {}
            LayerMode::Single(layer) => {
                min[1] += layer;
                max[1] = min[1] + 1;
            }
            LayerMode::Range {
                min: bottom,
                max: top,
            } => {
                max[1] = min[1] + top + 1;
                min[1] += bottom;
            }
        }
        let mut tiles = Vec::new();
        for x in (min[0]..max[0]).step_by(TILE_EDGE as usize) {
            for z in (min[2]..max[2]).step_by(TILE_EDGE as usize) {
                for y in (min[1]..max[1]).step_by(TILE_EDGE as usize) {
                    let lo = [x, y, z];
                    let hi = [0, 1, 2].map(|i| (lo[i] + TILE_EDGE).min(max[i]));
                    tiles.push(Tile {
                        lo,
                        hi,
                        ..Tile::default()
                    });
                }
            }
        }
        tiles
    }
}

#[derive(Default)]
pub struct Tile {
    lo: [i32; 3],
    hi: [i32; 3],
    pub stamp: Option<terrain::Stamp>,
    pub tally: Tally,
    pub fallback: bool,
    pub error: Option<String>,
    materials: Vec<[u16; 3]>,
    statuses: Vec<CellStatus>,
    pub mismatches: Vec<([i32; 3], CellStatus)>,
    markers: Option<DrawPair>,
    blocks: Option<DrawPair>,
    pending_frames: u8,
    sample: Option<terrain::Region>,
}

struct DrawPair {
    geometry: draw::Handle,
    instance: draw::Handle,
}

impl DrawPair {
    fn remove(self) {
        let _ = draw::remove(self.instance);
        let _ = draw::remove(self.geometry);
    }
}

impl Tile {
    pub(crate) fn is_missing(&self, world: [i32; 3]) -> bool {
        if (0..3).any(|axis| world[axis] < self.lo[axis] || world[axis] >= self.hi[axis]) {
            return false;
        }
        let [x, y, z] = [0, 1, 2].map(|axis| world[axis] - self.lo[axis]);
        let height = self.hi[1] - self.lo[1];
        let depth = self.hi[2] - self.lo[2];
        self.statuses.get(((x * depth + z) * height + y) as usize) == Some(&CellStatus::Missing)
    }

    pub fn has_draws(&self) -> bool {
        self.markers.is_some() || self.blocks.is_some()
    }

    pub fn clear(&mut self) {
        if let Some(pair) = self.markers.take() {
            pair.remove();
        }
        if let Some(pair) = self.blocks.take() {
            pair.remove();
        }
        self.stamp = None;
        self.statuses.clear();
        self.materials.clear();
        self.mismatches.clear();
        self.tally = Tally::default();
    }

    pub fn material_counts(&self, indices: &[usize]) -> [u32; 3] {
        indices
            .iter()
            .filter_map(|&index| self.materials.get(index))
            .fold([0; 3], |sum, counts| {
                std::array::from_fn(|i| sum[i] + u32::from(counts[i]))
            })
    }

    pub fn mismatch_count(&self) -> usize {
        self.mismatches.len()
    }

    pub fn step(
        &mut self,
        preview: &Preview,
        schematic: &Schematic,
        dimension: i32,
        opacity: Option<f32>,
    ) -> bool {
        if let Some(region) = self.sample.take() {
            self.render(
                preview,
                schematic,
                dimension,
                opacity.filter(|_| !self.fallback),
                region,
            );
            return true;
        }
        if let Some(pair) = &self.blocks {
            if let Err(e) = draw::geometry_status(pair.geometry) {
                self.clear();
                self.fallback = true;
                self.error = Some(format!("Texture unavailable: {}", e.detail));
            }
        }
        let query = terrain::RegionQuery {
            min: position(self.lo),
            max: position(self.hi),
            geometry: terrain::Geometry::empty(),
        };
        match terrain::read_region(query, self.stamp) {
            Ok(terrain::ReadResult::Pending) => {
                self.pending_frames += 1;
                if self.pending_frames < 8 {
                    return false;
                }
            }
            Ok(terrain::ReadResult::Unchanged(_)) => {}
            Ok(terrain::ReadResult::Changed(region)) => {
                self.sample = Some(region);
                self.pending_frames = 0;
                return false;
            }
            Err(e) => {
                self.clear();
                self.error = Some(format!("Terrain: {e:?}"));
            }
        }
        self.pending_frames = 0;
        true
    }

    fn render(
        &mut self,
        preview: &Preview,
        schematic: &Schematic,
        dimension: i32,
        opacity: Option<f32>,
        region: terrain::Region,
    ) {
        let [sx, sy, sz] = [0, 1, 2].map(|i| self.hi[i] - self.lo[i]);
        let volume = (sx * sy * sz) as usize;
        let mut ids = Vec::with_capacity(volume);
        let mut statuses = Vec::with_capacity(volume);
        let p = preview.placement.unwrap();
        for x in 0..sx {
            for z in 0..sz {
                for y in 0..sy {
                    let world = [self.lo[0] + x, self.lo[1] + y, self.lo[2] + z];
                    let local = p.world_to_local(schematic.size(), world).unwrap();
                    let id = schematic.palette_index(local).unwrap();
                    let expected = &preview.palette[id as usize];
                    let status = if expected.name == "minecraft:structure_void" {
                        CellStatus::Correct
                    } else {
                        compare(&region, ids.len(), expected, schematic.is_air_index(id))
                    };
                    ids.push(id);
                    statuses.push(status);
                }
            }
        }
        if self.stamp.is_some() && self.statuses == statuses {
            self.stamp = Some(region.stamp);
            return;
        }
        self.clear();
        if !self.fallback {
            self.error = None;
        }
        let mut cells = Vec::with_capacity(volume);
        let mut highlights = draw::Mesh {
            vertices: Vec::new(),
            indices: Vec::new(),
        };
        let mut used_texture = false;
        let mut materials = vec![[0; 3]; preview.palette.len()];
        let mut tally = Tally::default();
        for x in 0..sx {
            for z in 0..sz {
                for y in 0..sy {
                    let id = ids[cells.len()] as usize;
                    let status = statuses[cells.len()];
                    let air = schematic.is_air_index(id as u32);
                    let mut word = DEFAULT_LIGHT;
                    if !air || status == CellStatus::Extra {
                        tally.add(status);
                        if status != CellStatus::Correct {
                            self.mismatches
                                .push(([self.lo[0] + x, self.lo[1] + y, self.lo[2] + z], status));
                        }
                    }
                    let color = match status {
                        CellStatus::Missing => {
                            materials[id][0] += 1;
                            if opacity.is_some() {
                                word |= id as u32 + 1;
                                used_texture = true;
                            }
                            None
                        }
                        CellStatus::Wrong | CellStatus::WrongState => {
                            materials[id][0] += 1;
                            Some((1., 0.7, 0.08, 0.52))
                        }
                        CellStatus::Extra => Some((1., 0.15, 0.12, 0.48)),
                        CellStatus::Correct if !air => {
                            materials[id][1] += 1;
                            None
                        }
                        CellStatus::Unloaded if !air => {
                            materials[id][2] += 1;
                            None
                        }
                        _ => None,
                    };
                    cells.push(word);
                    if let Some(color) = color {
                        add_box(&mut highlights, [x, y, z], color);
                    }
                }
            }
        }

        let anchor = draw::BlockAnchor {
            dimension,
            position: self.lo.into(),
        };
        if !highlights.vertices.is_empty() {
            match create(draw::GeometrySpec::Triangles(highlights), anchor, 1.) {
                Ok(pair) => self.markers = Some(pair),
                Err(e) => self.error = Some(format!("Overlay: {}", e.detail)),
            }
        }
        if let Some(opacity) = opacity.filter(|_| used_texture) {
            let mut palette = vec![draw::BlockState::Named(draw::NamedBlock {
                name: "minecraft:air".into(),
                states: "{}".into(),
            })];
            // Send only states referenced by this tile. A bad mapping in another tile
            // must not make this one fall back to colored boxes.
            let mut remap = vec![0; preview.palette.len() + 1];
            for word in &mut cells {
                let id = (*word & 0xffff) as usize;
                if id == 0 {
                    continue;
                }
                if remap[id] == 0 {
                    remap[id] = palette.len() as u32;
                    palette.push(draw::BlockState::Named(preview.palette[id - 1].clone()));
                }
                *word = DEFAULT_LIGHT | remap[id];
            }
            let grid = draw::BlockGrid {
                size: (sx as u32, sy as u32, sz as u32),
                palette,
                cells,
            };
            match create(draw::GeometrySpec::Blocks(grid), anchor, opacity) {
                Ok(pair) => self.blocks = Some(pair),
                Err(e) => {
                    self.fallback = true;
                    self.error = Some(format!("Texture unavailable: {}", e.detail));
                    // Resample so mismatch markers and counts still update.
                    self.materials = materials;
                    self.tally = tally;
                    return;
                }
            }
        }
        self.stamp = Some(region.stamp);
        self.statuses = statuses;
        self.materials = materials;
        self.tally = tally;
    }
}

fn compare(
    region: &terrain::Region,
    cell: usize,
    expected: &draw::NamedBlock,
    air: bool,
) -> CellStatus {
    if !region
        .loaded
        .get(cell / 64)
        .is_some_and(|word| word & (1 << (cell % 64)) != 0)
    {
        return CellStatus::Unloaded;
    }
    let Some(&index) = region.states.get(cell) else {
        return CellStatus::Unloaded;
    };
    let Some(block) = region.palette.get(index as usize) else {
        return CellStatus::Unloaded;
    };
    let Some(name) = block.identifier.as_deref() else {
        return CellStatus::Unloaded;
    };
    let actual_air = matches!(
        name,
        "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air"
    );
    match (air, actual_air) {
        (true, true) => CellStatus::Correct,
        (true, false) => CellStatus::Extra,
        (false, true) => CellStatus::Missing,
        (false, false) if name != expected.name => CellStatus::Wrong,
        (false, false) => match block.canonical_state.as_deref() {
            Some(state) if state == expected.states => CellStatus::Correct,
            Some(_) => CellStatus::WrongState,
            None => CellStatus::Unloaded,
        },
    }
}

pub(crate) fn canonical(state: &BlockState) -> String {
    let mut properties = Map::new();
    for (name, value) in &state.states {
        let typed = match value {
            StateValue::Bool(value) => json!({"type": "byte", "value": u8::from(*value)}),
            StateValue::Byte(value) => json!({"type": "byte", "value": value}),
            StateValue::Int(value) => json!({"type": "int", "value": value}),
            StateValue::Str(value) => json!({"type": "string", "value": value}),
        };
        properties.insert(name.clone(), typed);
    }
    serde_json::Value::Object(properties).to_string()
}

fn position([x, y, z]: [i32; 3]) -> terrain::Position {
    terrain::Position { x, y, z }
}

fn create(
    spec: draw::GeometrySpec,
    anchor: draw::BlockAnchor,
    opacity: f32,
) -> Result<DrawPair, draw::DrawError> {
    let geometry = draw::create_geometry(&spec)?;
    let mut params = vec![0.; 16];
    params[7] = 1.;
    params[9] = 1.;
    let mut placement = draw::Instance {
        geometry,
        material: None,
        clip: None,
        params,
        anchor: draw::Anchor::Block(anchor),
        transform: draw::Transform {
            translation: (0., 0., 0.),
            rotation: (0., 0., 0., 1.),
            scale: (1., 1., 1.),
        },
        tint: (1., 1., 1., opacity),
        depth: draw::Depth::Test,
        blend: draw::Blend::Premultiplied,
        width: 1.,
        width_mode: draw::WidthMode::World,
        join: draw::Join::Bevel,
        cap: draw::Cap::Butt,
    };
    let instance = match draw::create(&placement) {
        Err(error) if opacity < 1. && error.code == draw::ErrorCode::Invalid => {
            // Stock Cinnabar accepts native block grids only at full opacity.
            placement.tint.3 = 1.;
            draw::create(&placement)
        }
        result => result,
    };
    match instance {
        Ok(instance) => Ok(DrawPair { geometry, instance }),
        Err(error) => {
            let _ = draw::remove(geometry);
            Err(error)
        }
    }
}

fn add_box(mesh: &mut draw::Mesh, [x, y, z]: [i32; 3], color: (f32, f32, f32, f32)) {
    let base = mesh.vertices.len() as u32;
    let extent = [-0.015, 1.015];
    for dz in extent {
        for dy in extent {
            for dx in extent {
                mesh.vertices.push(draw::Vertex {
                    position: (x as f32 + dx, y as f32 + dy, z as f32 + dz),
                    color,
                    uv: (0., 0.),
                    data: (0., 0., 0., 0.),
                });
            }
        }
    }
    let triangles = [
        0, 2, 1, 1, 2, 3, 4, 5, 6, 5, 7, 6, 0, 4, 2, 2, 4, 6, 1, 3, 5, 3, 7, 5, 0, 1, 4, 1, 5, 4,
        2, 6, 3, 3, 6, 7,
    ];
    mesh.indices.extend(triangles.map(|i| base + i));
}
