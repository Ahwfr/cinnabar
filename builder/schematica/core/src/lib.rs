//! Schematica-style schematic logic for Minecraft: Bedrock Edition.
//!
//! Pipeline (see PLAN.md):
//!   Java file --(host importer + Geyser mappings)--> `Schematic` (Bedrock states)
//!   `Schematic` + `Placement` --> `Placed` --> diff / layers / culling / materials
//!   --> draw lists consumed by the Cinnabar mod glue.

pub mod diff;
pub mod materials;
pub mod placed;
pub mod placement;
pub mod schematic;
pub mod state;
pub mod view;

pub use diff::{classify, scan_region, CellStatus, MatchMode, ScanOptions, Tally, WorldView};
pub use materials::{material_list, MaterialLine};
pub use placed::Placed;
pub use placement::{Mirror, Placement};
pub use schematic::Schematic;
pub use state::{BlockState, StateValue};
pub use view::{sections_overlapping, visible_in_section, LayerMode, VisibleBlock, SECTION};
pub mod blueprint;
