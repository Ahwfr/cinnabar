//! Schematic browser, live placement summary and material preview for the two-pane menu.

use crate::Viewer;
use serde_json::{Map, Value, json};

pub fn bindings(viewer: &Viewer, out: &mut Map<String, Value>) {
    for index in 0..schematica_core::blueprint::MAX_LIBRARY {
        let original = viewer.visible_entry(index);
        let entry = original.and_then(|index| viewer.library.get(index));
        let loaded = viewer.loaded && original.is_some() && viewer.active_entry == original;
        let active = loaded && viewer.shown && viewer.placement.is_some();
        out.insert(format!("#visible{index}"), json!(entry.is_some()));
        out.insert(format!("#active{index}"), json!(loaded));
        out.insert(
            format!("#file{index}"),
            json!(entry.map_or(String::new(), |name| short(name, 17))),
        );
        out.insert(
            format!("#entryMeta{index}"),
            json!(if loaded {
                "Loaded schematic"
            } else {
                "Ready to load"
            }),
        );
        out.insert(
            format!("#entryStatus{index}"),
            json!(if active {
                "Active"
            } else if loaded {
                "Hidden"
            } else {
                "Load"
            }),
        );
        out.insert(
            format!("#rowColor{index}"),
            json!(if loaded {
                [0.34, 0.34, 0.35, 1.]
            } else {
                [0.26, 0.26, 0.27, 1.]
            }),
        );
        out.insert(
            format!("#statusColor{index}"),
            json!(if active {
                [0.26, 0.53, 0.20, 1.]
            } else {
                [0.29, 0.29, 0.30, 1.]
            }),
        );
    }
    out.insert(
        "#libraryCount".into(),
        json!(format!(
            "{} schematic(s) in this library",
            (0..viewer.library.len())
                .filter(|index| !viewer.options.deleted_entries.contains(index))
                .count()
        )),
    );
    out.insert("#hasBlueprint".into(), json!(viewer.loaded));
    out.insert(
        "#detailTitle".into(),
        json!(if viewer.loaded {
            short(&viewer.schematic.name.to_uppercase(), 54)
        } else {
            "SELECT A SCHEMATIC".into()
        }),
    );
    out.insert(
        "#detailSubtitle".into(),
        json!(if viewer.loaded {
            "Building guide with placement controls and materials."
        } else {
            "Import schematics or choose one from the library."
        }),
    );
    let size = viewer.schematic.size();
    let total: u64 = viewer.materials.iter().map(|line| line.total).sum();
    let field = |value: String| if viewer.loaded { value } else { "-".into() };
    out.insert(
        "#detailSize".into(),
        json!(field(format!("{} x {} x {}", size[0], size[1], size[2]))),
    );
    out.insert(
        "#detailVolume".into(),
        json!(field(size.iter().product::<i32>().to_string())),
    );
    out.insert("#detailBlocks".into(), json!(field(total.to_string())));
    out.insert(
        "#detailPalette".into(),
        json!(field(viewer.schematic.palette().len().to_string())),
    );
    out.insert(
        "#detailState".into(),
        json!(field(
            if viewer.shown { "Visible" } else { "Hidden" }.into()
        )),
    );
    out.insert("#detailLayers".into(), json!(field(viewer.layers_text())));
    let placement = viewer.placement.map_or_else(
        || "Choose Place here to anchor the guide.".into(),
        |placement| {
            format!(
                "Origin {} {} {} / {} degrees",
                placement.origin[0],
                placement.origin[1],
                placement.origin[2],
                placement.quarter_turns * 90
            )
        },
    );
    out.insert("#detailPlacement".into(), json!(placement));
    out.insert(
        "#materialHeading".into(),
        json!(format!("MATERIALS / {total} BLOCKS")),
    );
    for row in 0..7 {
        let line = viewer.loaded.then(|| viewer.materials.get(row)).flatten();
        out.insert(
            format!("#materialIcon{row}"),
            crate::menu::material_icon(line),
        );
        out.insert(
            format!("#materialName{row}"),
            json!(line.map_or(String::new(), |line| {
                short(
                    line.name.strip_prefix("minecraft:").unwrap_or(&line.name),
                    44,
                )
            })),
        );
        out.insert(
            format!("#materialCount{row}"),
            json!(line.map_or(String::new(), |line| line.total.to_string())),
        );
    }
}

fn short(text: &str, count: usize) -> String {
    text.chars().take(count).collect()
}
