use crate::state::{BlockState, StateValue};
use std::collections::BTreeMap;

/// Mirror applied in schematic-local space, *before* rotation.
/// `X` flips east<->west, `Z` flips north<->south.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mirror {
    None,
    X,
    Z,
}

/// Where a schematic sits in the world: mirror, then clockwise quarter turns
/// about +Y (viewed from above), then translation of the local origin.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Placement {
    pub origin: [i32; 3],
    pub quarter_turns: u8,
    pub mirror: Mirror,
}

impl Placement {
    pub fn new(origin: [i32; 3]) -> Self {
        Self {
            origin,
            quarter_turns: 0,
            mirror: Mirror::None,
        }
    }

    pub fn rotate_cw(&mut self) {
        self.quarter_turns = (self.quarter_turns + 1) % 4;
    }

    pub fn rotate_ccw(&mut self) {
        self.quarter_turns = (self.quarter_turns + 3) % 4;
    }

    pub fn set_mirror(&mut self, m: Mirror) {
        self.mirror = m;
    }

    /// Bounding box size after rotation (X/Z swap on odd turns).
    pub fn footprint(&self, size: [i32; 3]) -> [i32; 3] {
        if self.quarter_turns % 2 == 1 {
            [size[2], size[1], size[0]]
        } else {
            size
        }
    }

    /// World AABB: inclusive min, exclusive max.
    pub fn world_bounds(&self, size: [i32; 3]) -> ([i32; 3], [i32; 3]) {
        let f = self.footprint(size);
        let min = self.origin;
        (min, [min[0] + f[0], min[1] + f[1], min[2] + f[2]])
    }

    pub fn local_to_world(&self, size: [i32; 3], local: [i32; 3]) -> [i32; 3] {
        let [mut x, y, mut z] = local;
        match self.mirror {
            Mirror::None => {}
            Mirror::X => x = size[0] - 1 - x,
            Mirror::Z => z = size[2] - 1 - z,
        }
        let (mut bx, mut bz) = (size[0], size[2]);
        for _ in 0..self.quarter_turns {
            // clockwise from above: (x, z) -> (box_z - 1 - z, x), box swaps
            let nx = bz - 1 - z;
            let nz = x;
            x = nx;
            z = nz;
            std::mem::swap(&mut bx, &mut bz);
        }
        [self.origin[0] + x, self.origin[1] + y, self.origin[2] + z]
    }

    pub fn world_to_local(&self, size: [i32; 3], world: [i32; 3]) -> Option<[i32; 3]> {
        let f = self.footprint(size);
        let (mut x, y, mut z) = (
            world[0] - self.origin[0],
            world[1] - self.origin[1],
            world[2] - self.origin[2],
        );
        if x < 0 || y < 0 || z < 0 || x >= f[0] || y >= f[1] || z >= f[2] {
            return None;
        }
        let (mut bx, mut bz) = (f[0], f[2]);
        for _ in 0..self.quarter_turns {
            // counter-clockwise: (x, z) -> (z, box_x - 1 - x), box swaps
            let nx = z;
            let nz = bx - 1 - x;
            x = nx;
            z = nz;
            std::mem::swap(&mut bx, &mut bz);
        }
        match self.mirror {
            Mirror::None => {}
            Mirror::X => x = size[0] - 1 - x,
            Mirror::Z => z = size[2] - 1 - z,
        }
        Some([x, y, z])
    }

    /// Rotates/mirrors direction-bearing Bedrock states so a placed block faces
    /// the way the rotated schematic implies. Unknown properties are left alone.
    ///
    /// The per-property value tables follow Bedrock's block-state conventions
    /// as I know them; they must be validated against the pinned target
    /// version (see PLAN.md, "Verification"). A wrong table fails loudly in the
    /// diff as `Wrong`, never silently.
    pub fn transform_state(&self, s: &BlockState) -> BlockState {
        if self.quarter_turns == 0 && self.mirror == Mirror::None {
            return s.clone();
        }
        let mut out: BTreeMap<String, StateValue> = BTreeMap::new();
        for (k, v) in &s.states {
            out.insert(k.clone(), self.transform_value(&s.name, k, v));
        }
        BlockState {
            name: s.name.clone(),
            states: out,
        }
    }

    fn apply_dir(&self, d: Dir) -> Dir {
        let mut d = d.mirror(self.mirror);
        for _ in 0..self.quarter_turns {
            d = d.cw();
        }
        d
    }

    fn transform_value(&self, block: &str, key: &str, v: &StateValue) -> StateValue {
        use StateValue::*;
        match (key, v) {
            // x/z swap on odd turns; mirroring never changes an axis.
            ("pillar_axis", Str(a)) if self.quarter_turns % 2 == 1 => match a.as_str() {
                "x" => Str("z".into()),
                "z" => Str("x".into()),
                _ => v.clone(),
            },
            // 0 down, 1 up, 2 north, 3 south, 4 west, 5 east
            ("facing_direction", Int(i)) if (2..=5).contains(i) => {
                const T: [Dir; 4] = [Dir::N, Dir::S, Dir::W, Dir::E];
                let d = self.apply_dir(T[(*i - 2) as usize]);
                Int(2 + T.iter().position(|&t| t == d).unwrap() as i32)
            }
            ("weirdo_direction", Int(i)) | ("direction", Int(i)) if (0..4).contains(i) => {
                let t = int_dir_table(key, block);
                let d = self.apply_dir(t[*i as usize]);
                Int(t.iter().position(|&x| x == d).unwrap() as i32)
            }
            ("minecraft:cardinal_direction", Str(s)) | ("torch_facing_direction", Str(s)) => {
                match Dir::from_name(s) {
                    Some(d) => Str(self.apply_dir(d).name().to_string()),
                    None => v.clone(), // e.g. torch "top"
                }
            }
            // 16 steps, 0 = south, 4 = west, 8 = north, 12 = east
            ("ground_sign_direction", Int(i)) => {
                let mut i = i.rem_euclid(16);
                i = match self.mirror {
                    Mirror::None => i,
                    Mirror::X => (16 - i).rem_euclid(16),
                    Mirror::Z => (8 - i).rem_euclid(16),
                };
                Int((i + 4 * self.quarter_turns as i32).rem_euclid(16))
            }
            _ => v.clone(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Dir {
    N,
    E,
    S,
    W,
}

impl Dir {
    fn cw(self) -> Dir {
        match self {
            Dir::N => Dir::E,
            Dir::E => Dir::S,
            Dir::S => Dir::W,
            Dir::W => Dir::N,
        }
    }

    fn mirror(self, m: Mirror) -> Dir {
        match (m, self) {
            (Mirror::X, Dir::E) => Dir::W,
            (Mirror::X, Dir::W) => Dir::E,
            (Mirror::Z, Dir::N) => Dir::S,
            (Mirror::Z, Dir::S) => Dir::N,
            _ => self,
        }
    }

    fn from_name(s: &str) -> Option<Dir> {
        Some(match s {
            "north" => Dir::N,
            "east" => Dir::E,
            "south" => Dir::S,
            "west" => Dir::W,
            _ => return None,
        })
    }

    fn name(self) -> &'static str {
        match self {
            Dir::N => "north",
            Dir::E => "east",
            Dir::S => "south",
            Dir::W => "west",
        }
    }
}

/// Integer-coded 4-way directions. Trapdoors use a different order than the
/// other `direction` users (repeaters, fence gates, ...).
fn int_dir_table(key: &str, block: &str) -> [Dir; 4] {
    const EWSN: [Dir; 4] = [Dir::E, Dir::W, Dir::S, Dir::N];
    const SWNE: [Dir; 4] = [Dir::S, Dir::W, Dir::N, Dir::E];
    match key {
        "weirdo_direction" => EWSN,
        _ if block.contains("trapdoor") => EWSN,
        _ => SWNE,
    }
}
