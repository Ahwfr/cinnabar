use super::*;

pub(super) fn reference_viewer() -> Viewer {
    let entries = [
        ("minecraft:oak_planks", 104),
        ("minecraft:cobblestone", 49),
        ("minecraft:oak_log", 12),
        ("minecraft:glass_pane", 4),
        ("minecraft:wooden_door", 1),
        ("minecraft:torch", 1),
    ];
    let mut schematic = Schematic::new("Lakeside Cabin", [7, 5, 7]);
    let mut cursor = 0;
    for (name, count) in entries {
        for _ in 0..count {
            schematic.set(
                [cursor % 7, cursor / 49, (cursor / 7) % 7],
                schematica_core::BlockState::new(name),
            );
            cursor += 1;
        }
    }
    let materials = entries
        .into_iter()
        .map(|(name, total)| MaterialLine {
            name: name.into(),
            total: total as u64,
            missing: total as u64,
        })
        .collect();
    let mut viewer = Viewer::new(
        schematic,
        materials,
        vec![
            "Lakeside Cabin".into(),
            "Sugar Cane Farm".into(),
            "Passenger Wagon".into(),
        ],
        Options {
            textured: true,
            assist: false,
            fast_place: true,
            correct_aim: false,
            fullscreen_ui: true,
            air_place: false,
            opacity: 65,
            hotkeys: BTreeMap::new(),
            library_revision: 0,
            deleted_entries: Vec::new(),
        },
        true,
    );
    let mut placement = Placement::new([120, 64, -48]);
    placement.quarter_turns = 1;
    viewer.placement = Some(placement);
    viewer.active_entry = Some(0);
    viewer
}

#[test]
fn dashboard_reports_the_current_schematic_and_unload_state() {
    let mut viewer = reference_viewer();
    let mut out = serde_json::Map::new();
    dashboard::bindings(&viewer, &mut out);
    assert_eq!(out["#detailBlocks"], serde_json::json!("171"));
    assert_eq!(out["#materialCount0"], serde_json::json!("104"));
    assert_eq!(out["#active0"], serde_json::json!(true));
    assert_eq!(out["#active1"], serde_json::json!(false));
    viewer.menu_action("unload-0");
    dashboard::bindings(&viewer, &mut out);
    assert_eq!(out["#hasBlueprint"], serde_json::json!(false));
    assert!(viewer.placement.is_none());
}

#[test]
fn deleting_compacts_the_library_and_reimport_restores_entries() {
    let mut viewer = reference_viewer();
    viewer.menu_action("delete-0");
    assert!(!viewer.loaded);
    assert!(viewer.placement.is_none());
    assert_eq!(viewer.visible_entry(0), Some(1));
    viewer.menu_action("select-0");
    assert_eq!(viewer.request, Some(1));
    let mut out = serde_json::Map::new();
    dashboard::bindings(&viewer, &mut out);
    assert_eq!(out["#file0"], "Sugar Cane Farm");
    viewer.options.select_library(0);
    assert_eq!(viewer.options.deleted_entries, vec![0]);
    viewer.options.select_library(1);
    assert!(viewer.options.deleted_entries.is_empty());
    assert_eq!(viewer.library[0], "Lakeside Cabin");
}

#[test]
fn menu_edits_turn_the_guide_and_select_layer_ranges() {
    let mut viewer = Viewer::new(
        Schematic::new("fixture", [2, 4, 3]),
        Vec::new(),
        vec!["Demo".into(), "Wagon".into()],
        Options {
            textured: true,
            assist: false,
            fast_place: false,
            correct_aim: false,
            fullscreen_ui: true,
            air_place: false,
            opacity: 65,
            hotkeys: BTreeMap::new(),
            library_revision: 0,
            deleted_entries: Vec::new(),
        },
        true,
    );
    viewer.placement = Some(Placement::new([12, 64, -5]));
    assert!(viewer.options.textured);
    viewer.menu_action("select-1");
    viewer.menu_action("select-8");
    assert_eq!(viewer.selected, Some(1));

    viewer.menu_action("ccw");
    let placement = viewer.placement.unwrap();
    assert_eq!(placement.quarter_turns, 3);
    assert_eq!(placement.footprint(viewer.schematic.size()), [3, 4, 2]);
    assert!(viewer.dirty);
    viewer.menu_action("cw");
    assert_eq!(viewer.placement.unwrap().quarter_turns, 0);

    viewer.menu_action("mode");
    viewer.menu_action("next-layer");
    assert_eq!(viewer.layers(), LayerMode::Single(1));
    viewer.menu_action("mode");
    assert_eq!(viewer.layers(), LayerMode::Range { min: 0, max: 1 });
    viewer.menu_action("mode");
    assert_eq!(viewer.layers(), LayerMode::All);
}
