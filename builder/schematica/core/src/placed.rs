use crate::placement::Placement;
use crate::schematic::Schematic;
use crate::state::BlockState;

/// A schematic bound to a placement, with its palette pre-transformed so
/// per-cell lookups are just an index.
pub struct Placed<'a> {
    schem: &'a Schematic,
    placement: Placement,
    palette: Vec<BlockState>,
}

impl<'a> Placed<'a> {
    pub fn new(schem: &'a Schematic, placement: Placement) -> Self {
        let palette = schem
            .palette()
            .iter()
            .map(|s| placement.transform_state(s))
            .collect();
        Self {
            schem,
            placement,
            palette,
        }
    }

    pub fn schematic(&self) -> &Schematic {
        self.schem
    }

    pub fn placement(&self) -> Placement {
        self.placement
    }

    /// Transformed palette: what each index must look like *in the world*.
    pub fn palette(&self) -> &[BlockState] {
        &self.palette
    }

    pub fn is_air_index(&self, idx: u32) -> bool {
        self.schem.is_air_index(idx)
    }

    /// World AABB, inclusive min / exclusive max.
    pub fn bounds(&self) -> ([i32; 3], [i32; 3]) {
        self.placement.world_bounds(self.schem.size())
    }

    pub fn palette_index_at(&self, world: [i32; 3]) -> Option<u32> {
        let local = self.placement.world_to_local(self.schem.size(), world)?;
        self.schem.palette_index(local)
    }

    pub fn state_at(&self, world: [i32; 3]) -> Option<&BlockState> {
        self.palette_index_at(world)
            .map(|i| &self.palette[i as usize])
    }

    /// Every cell as `(world_pos, local_pos, palette_index)`.
    pub fn cells(&self) -> impl Iterator<Item = ([i32; 3], [i32; 3], u32)> + '_ {
        let size = self.schem.size();
        (0..size[1]).flat_map(move |y| {
            (0..size[2]).flat_map(move |z| {
                (0..size[0]).map(move |x| {
                    let local = [x, y, z];
                    let idx = self.schem.palette_index(local).expect("in range");
                    (self.placement.local_to_world(size, local), local, idx)
                })
            })
        })
    }
}
