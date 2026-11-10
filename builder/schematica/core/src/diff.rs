use crate::placed::Placed;
use crate::state::BlockState;

/// Read access to the *live* world. `None` means "not loaded / unknown",
/// which is never treated as air (Cinnabar's terrain API reports unknown
/// cells the same way).
pub trait WorldView {
    fn block_at(&self, pos: [i32; 3]) -> Option<&BlockState>;
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum CellStatus {
    /// World matches the schematic.
    Correct,
    /// Schematic wants a block, world has air.
    Missing,
    /// Both are solid but differ.
    Wrong,
    /// Schematic wants air, world has a block.
    Extra,
    /// World data not loaded.
    Unloaded,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum MatchMode {
    /// Identifier and every state must match.
    Exact,
    /// Identifier only (useful while block-state translation is imperfect).
    IgnoreStates,
}

pub fn classify(expected: &BlockState, actual: Option<&BlockState>, mode: MatchMode) -> CellStatus {
    let Some(actual) = actual else {
        return CellStatus::Unloaded;
    };
    match (expected.is_air(), actual.is_air()) {
        (true, true) => CellStatus::Correct,
        (true, false) => CellStatus::Extra,
        (false, true) => CellStatus::Missing,
        (false, false) => {
            let same = match mode {
                MatchMode::Exact => expected == actual,
                MatchMode::IgnoreStates => expected.name == actual.name,
            };
            if same {
                CellStatus::Correct
            } else {
                CellStatus::Wrong
            }
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct ScanOptions {
    pub mode: MatchMode,
    pub include_correct: bool,
    /// Report cells where the schematic is air but the world is not.
    pub report_extra: bool,
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            mode: MatchMode::Exact,
            include_correct: false,
            report_extra: false,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Cell {
    pub pos: [i32; 3],
    pub status: CellStatus,
    pub palette_index: u32,
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Tally {
    pub correct: u64,
    pub missing: u64,
    pub wrong: u64,
    pub extra: u64,
    pub unloaded: u64,
}

impl Tally {
    pub fn add(&mut self, s: CellStatus) {
        match s {
            CellStatus::Correct => self.correct += 1,
            CellStatus::Missing => self.missing += 1,
            CellStatus::Wrong => self.wrong += 1,
            CellStatus::Extra => self.extra += 1,
            CellStatus::Unloaded => self.unloaded += 1,
        }
    }
}

/// Diffs the world AABB `[min, max)` (clipped to the schematic) against the
/// placed schematic. Call per 16-block section so work stays incremental.
pub fn scan_region(
    placed: &Placed,
    world: &dyn WorldView,
    min: [i32; 3],
    max: [i32; 3],
    opts: ScanOptions,
) -> Vec<Cell> {
    let (bmin, bmax) = placed.bounds();
    let lo = [
        min[0].max(bmin[0]),
        min[1].max(bmin[1]),
        min[2].max(bmin[2]),
    ];
    let hi = [
        max[0].min(bmax[0]),
        max[1].min(bmax[1]),
        max[2].min(bmax[2]),
    ];
    let mut out = Vec::new();
    for y in lo[1]..hi[1] {
        for z in lo[2]..hi[2] {
            for x in lo[0]..hi[0] {
                let pos = [x, y, z];
                let Some(idx) = placed.palette_index_at(pos) else {
                    continue;
                };
                let expected = &placed.palette()[idx as usize];
                let status = classify(expected, world.block_at(pos), opts.mode);
                let keep = match status {
                    CellStatus::Correct => opts.include_correct,
                    CellStatus::Extra => opts.report_extra,
                    _ => true,
                };
                if keep {
                    out.push(Cell {
                        pos,
                        status,
                        palette_index: idx,
                    });
                }
            }
        }
    }
    out
}
