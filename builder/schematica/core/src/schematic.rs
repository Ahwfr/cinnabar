use crate::state::BlockState;
use std::collections::HashMap;

/// Dense block volume with a palette. Cell order is Y, then Z, then X
/// (`index = (y * size_z + z) * size_x + x`), matching Sponge `.schem`, so
/// importers for Java formats need no reshuffling. `.mcstructure` is X,Y,Z
/// order and is reordered by the importer.
#[derive(Clone, Debug)]
pub struct Schematic {
    pub name: String,
    size: [i32; 3],
    palette: Vec<BlockState>,
    palette_air: Vec<bool>,
    lookup: Option<HashMap<BlockState, u32>>,
    blocks: Vec<u32>,
}

impl Schematic {
    /// New volume filled with air.
    pub fn new(name: impl Into<String>, size: [i32; 3]) -> Self {
        let n = (size[0].max(0) as usize) * (size[1].max(0) as usize) * (size[2].max(0) as usize);
        let mut s = Self {
            name: name.into(),
            size,
            palette: Vec::new(),
            palette_air: Vec::new(),
            lookup: Some(HashMap::new()),
            blocks: vec![0; n],
        };
        s.intern(BlockState::air());
        s
    }

    /// Builds from importer output. Validates every palette index.
    pub fn from_raw(
        name: impl Into<String>,
        size: [i32; 3],
        palette: Vec<BlockState>,
        blocks: Vec<u32>,
    ) -> Result<Self, String> {
        if size.iter().any(|&d| d <= 0) {
            return Err(format!("non-positive size {size:?}"));
        }
        let expected = size[0] as usize * size[1] as usize * size[2] as usize;
        if blocks.len() != expected {
            return Err(format!("expected {expected} cells, got {}", blocks.len()));
        }
        if let Some(bad) = blocks.iter().find(|&&b| b as usize >= palette.len()) {
            return Err(format!(
                "palette index {bad} out of range ({})",
                palette.len()
            ));
        }
        let palette_air = palette.iter().map(BlockState::is_air).collect();
        Ok(Self {
            name: name.into(),
            size,
            palette,
            palette_air,
            lookup: None,
            blocks,
        })
    }

    pub fn size(&self) -> [i32; 3] {
        self.size
    }

    pub fn palette(&self) -> &[BlockState] {
        &self.palette
    }

    pub fn palette_counts(&self) -> Vec<u64> {
        let mut counts = vec![0; self.palette.len()];
        for &index in &self.blocks {
            counts[index as usize] += 1;
        }
        counts
    }

    pub fn is_air_index(&self, idx: u32) -> bool {
        self.palette_air[idx as usize]
    }

    pub fn contains(&self, p: [i32; 3]) -> bool {
        (0..3).all(|i| p[i] >= 0 && p[i] < self.size[i])
    }

    fn index(&self, p: [i32; 3]) -> usize {
        ((p[1] as usize * self.size[2] as usize) + p[2] as usize) * self.size[0] as usize
            + p[0] as usize
    }

    pub fn palette_index(&self, p: [i32; 3]) -> Option<u32> {
        self.contains(p).then(|| self.blocks[self.index(p)])
    }

    pub fn get(&self, p: [i32; 3]) -> Option<&BlockState> {
        self.palette_index(p).map(|i| &self.palette[i as usize])
    }

    fn intern(&mut self, state: BlockState) -> u32 {
        let lookup = self.lookup.get_or_insert_with(|| {
            let mut lookup = HashMap::new();
            for (i, state) in self.palette.iter().enumerate() {
                lookup.entry(state.clone()).or_insert(i as u32);
            }
            lookup
        });
        if let Some(&i) = lookup.get(&state) {
            return i;
        }
        let i = self.palette.len() as u32;
        self.palette_air.push(state.is_air());
        lookup.insert(state.clone(), i);
        self.palette.push(state);
        i
    }

    pub fn set(&mut self, p: [i32; 3], state: BlockState) -> bool {
        if !self.contains(p) {
            return false;
        }
        let i = self.intern(state);
        let idx = self.index(p);
        self.blocks[idx] = i;
        true
    }

    pub fn block_count(&self) -> usize {
        self.blocks
            .iter()
            .filter(|&&b| !self.palette_air[b as usize])
            .count()
    }
}
