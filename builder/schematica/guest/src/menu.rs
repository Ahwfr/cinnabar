use crate::{Viewer, label, panel};
use serde_json::{Map, Value, json};

pub const ROWS: usize = 7;
const CONTROL: &str = include_str!("../ui/control.json");
const LOAD: &str = include_str!("../ui/load.json");
const SETTINGS: &str = include_str!("../ui/settings.json");
const MATERIALS: &str = include_str!("../ui/materials.json");

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Control,
    Load,
    Settings,
    Materials,
}

pub fn open(viewer: &Viewer, replace: bool) {
    let template = match viewer.screen {
        Screen::Control => CONTROL,
        Screen::Load => LOAD,
        Screen::Settings => SETTINGS,
        Screen::Materials => MATERIALS,
    };
    let result = panel::set_content(template).and_then(|()| update(viewer));
    let result = if replace {
        result
    } else {
        result.and_then(|()| panel::open())
    };
    if let Err(e) = result {
        label(&format!("Schematica panel: {e}"));
    }
}

pub fn refresh(viewer: &Viewer) {
    if let Err(e) = update(viewer) {
        label(&format!("Schematica panel: {e}"));
    }
}

fn update(viewer: &Viewer) -> Result<(), String> {
    let mut out = Map::new();
    let blueprint = if viewer.loaded {
        short(&viewer.schematic.name, 34)
    } else {
        "No blueprint loaded".into()
    };
    out.insert("#blueprint".into(), json!(blueprint));
    let mut controls = Vec::new();
    match viewer.screen {
        Screen::Control => {
            let origin = viewer.placement.map(|p| p.origin);
            let values = [
                ("x", origin.map(|p| p[0])),
                ("y", origin.map(|p| p[1])),
                ("z", origin.map(|p| p[2])),
                ("layer", Some(viewer.layer)),
            ];
            for (id, value) in values {
                let value = value.map_or(String::new(), |n| n.to_string());
                out.insert(format!("#{id}"), json!(value));
                controls.push(json!({"id":id,"value":value}));
            }
            out.insert(
                "#mode".into(),
                json!(["All layers", "Single layer", "All below"][viewer.layer_mode as usize]),
            );
            out.insert(
                "#visibility".into(),
                json!(if viewer.shown { "Hide" } else { "Show" }),
            );
            out.insert(
                "#flip_axis".into(),
                json!(if viewer.flip_z { "North" } else { "West" }),
            );
            out.insert(
                "#rotate_axis".into(),
                json!(if viewer.rotate_ccw { "Down" } else { "Up" }),
            );
        }
        Screen::Load => {
            for index in 0..schematica_core::blueprint::MAX_LIBRARY {
                let name = viewer.library.get(index);
                out.insert(format!("#visible{index}"), json!(name.is_some()));
                let selected = viewer.selected == Some(index);
                let color = if selected {
                    [0.10, 0.30, 0.31, 1.]
                } else {
                    [0.14, 0.20, 0.24, 1.]
                };
                out.insert(format!("#rowColor{index}"), json!(color));
                out.insert(
                    format!("#file{index}"),
                    json!(name.map_or(String::new(), |name| short(name, 42))),
                );
            }
            let selected = viewer.selected.and_then(|index| viewer.library.get(index));
            out.insert(
                "#selected".into(),
                json!(selected.map_or_else(
                    || format!("{} available", viewer.library.len()),
                    |name| format!("Selected: {}", short(name, 26)),
                )),
            );
            out.insert(
                "#loadAction".into(),
                json!(if selected.is_some() {
                    "Load selected"
                } else {
                    "Select a blueprint"
                }),
            );
        }
        Screen::Settings => {
            out.insert(
                "#look".into(),
                json!(if viewer.options.textured {
                    "Textures"
                } else {
                    "Hidden"
                }),
            );
            out.insert(
                "#assist".into(),
                json!(if viewer.options.assist { "On" } else { "Off" }),
            );
        }
        Screen::Materials => materials(viewer, &mut out),
    }
    panel::update_content(&json!({"bindings":out,"controls":controls}).to_string())
}

fn materials(viewer: &Viewer, out: &mut Map<String, Value>) {
    let pages = viewer.materials.len().div_ceil(ROWS).max(1);
    let scan = if viewer.dirty || viewer.tiles.is_empty() {
        "Return to the build to scan".into()
    } else {
        let pending = viewer
            .tiles
            .iter()
            .filter(|tile| tile.stamp.is_none())
            .count();
        let unknown: u64 = viewer.tiles.iter().map(|tile| tile.tally.unloaded).sum();
        format!("{pending} tiles pending / {unknown} unknown blocks")
    };
    out.insert("#scan".into(), json!(scan));
    out.insert(
        "#page".into(),
        json!(format!("{} / {pages}", viewer.material_page + 1)),
    );
    out.insert(
        "#sort".into(),
        json!(if viewer.sort_name {
            "Sort: Name"
        } else {
            "Sort: Total"
        }),
    );
    let scanned = !viewer.dirty && viewer.tiles.iter().any(|tile| tile.stamp.is_some());
    for row in 0..ROWS {
        let line = viewer.materials.get(viewer.material_page * ROWS + row);
        let name = line.map_or(String::new(), |line| {
            let name = line.name.strip_prefix("minecraft:").unwrap_or(&line.name);
            short(&name.replace('_', " "), 26)
        });
        let total = line.map_or(String::new(), |line| line.total.to_string());
        let needed = line.map_or(String::new(), |line| {
            if !scanned {
                return "-".into();
            }
            let indices = viewer.preview.material_indices(&line.name);
            viewer
                .tiles
                .iter()
                .map(|tile| tile.needed(&indices))
                .sum::<u32>()
                .to_string()
        });
        out.insert(format!("#row{row}"), json!(name));
        out.insert(format!("#total{row}"), json!(total));
        out.insert(format!("#needed{row}"), json!(needed));
    }
}

fn short(text: &str, chars: usize) -> String {
    text.chars().take(chars).collect()
}
