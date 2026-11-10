use crate::diff::{classify, CellStatus, MatchMode, WorldView};
use crate::placed::Placed;
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MaterialLine {
    /// Block identifier. NOTE: this counts *blocks*, not items; doors, double
    /// slabs, beds etc. need a block->item pass (PLAN.md, phase 3).
    pub name: String,
    pub total: u64,
    /// Still needed: cells that are `Missing` or `Wrong`. Without a world,
    /// everything is still needed.
    pub missing: u64,
}

/// Bill of materials, largest first. Unloaded cells are not counted as missing
/// (they are unknown, not absent).
pub fn material_list(
    placed: &Placed,
    world: Option<&dyn WorldView>,
    mode: MatchMode,
) -> Vec<MaterialLine> {
    let mut acc: BTreeMap<&str, (u64, u64)> = BTreeMap::new();
    for (wpos, _local, idx) in placed.cells() {
        if placed.is_air_index(idx) {
            continue;
        }
        let expected = &placed.palette()[idx as usize];
        let e = acc.entry(&expected.name).or_insert((0, 0));
        e.0 += 1;
        let needed = match world {
            None => true,
            Some(w) => matches!(
                classify(expected, w.block_at(wpos), mode),
                CellStatus::Missing | CellStatus::Wrong
            ),
        };
        if needed {
            e.1 += 1;
        }
    }
    let mut v: Vec<MaterialLine> = acc
        .into_iter()
        .map(|(name, (total, missing))| MaterialLine {
            name: name.to_owned(),
            total,
            missing,
        })
        .collect();
    v.sort_by(|a, b| b.total.cmp(&a.total).then_with(|| a.name.cmp(&b.name)));
    v
}
