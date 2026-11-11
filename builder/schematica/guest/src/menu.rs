use crate::{Viewer, label, panel};
use schematica_core::CellStatus;
use serde_json::{Map, Value, json};

pub const ROWS: usize = 7;
const CONTROL: &str = include_str!("../ui/control.json");
const LOAD: &str = include_str!("../ui/load.json");
const SETTINGS: &str = include_str!("../ui/settings.json");
const MATERIALS: &str = include_str!("../ui/materials.json");
const MAIN: &str = include_str!("../ui/main.json");
const VERIFIER: &str = include_str!("../ui/verifier.json");
const HOTKEYS: &str = include_str!("../ui/hotkeys.json");

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Main,
    Control,
    Load,
    Settings,
    Materials,
    Verifier,
    Hotkeys,
}

pub fn open(viewer: &Viewer, replace: bool) {
    let content = content(viewer.screen, viewer.options.fullscreen_ui);
    let result = panel::set_content(content).and_then(|()| update(viewer));
    let result = if replace {
        result
    } else {
        result.and_then(|()| panel::open())
    };
    if let Err(e) = result {
        label(&format!("Schematica panel: {e}"));
    }
}

/// Uses the same hit-tested controls for a compact window or a viewport-filling layout.
fn content(screen: Screen, fullscreen: bool) -> &'static str {
    let (compact, full) = match screen {
        Screen::Main => (MAIN, include_str!("../ui/main-full.json")),
        Screen::Load => (LOAD, include_str!("../ui/load-full.json")),
        Screen::Control => (CONTROL, include_str!("../ui/control-full.json")),
        Screen::Settings => (SETTINGS, include_str!("../ui/settings-full.json")),
        Screen::Materials => (MATERIALS, include_str!("../ui/materials-full.json")),
        Screen::Verifier => (VERIFIER, include_str!("../ui/verifier-full.json")),
        Screen::Hotkeys => (HOTKEYS, include_str!("../ui/hotkeys-full.json")),
    };
    if fullscreen { full } else { compact }
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
    let revision = option_env!("SCHEMATICA_BUILD_REVISION").unwrap_or("local");
    out.insert(
        "#build".into(),
        json!(format!(
            "v{} / {}",
            env!("CARGO_PKG_VERSION"),
            &revision[..revision.len().min(7)]
        )),
    );
    let mut controls = Vec::new();
    match viewer.screen {
        Screen::Main => crate::dashboard::bindings(viewer, &mut out),
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
                "#lock".into(),
                json!(if viewer.locked { "Unlock" } else { "Lock" }),
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
        Screen::Load => crate::dashboard::bindings(viewer, &mut out),
        Screen::Settings => {
            out.insert(
                "#ui-layout".into(),
                json!(if viewer.options.fullscreen_ui {
                    "Full screen"
                } else {
                    "Compact"
                }),
            );
            out.insert(
                "#air-place".into(),
                json!(if viewer.options.air_place {
                    "On"
                } else {
                    "Off"
                }),
            );
            out.insert(
                "#opacity".into(),
                json!(format!("Opacity {}", viewer.options.opacity)),
            );
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
            out.insert(
                "#fast-place".into(),
                json!(if viewer.options.fast_place {
                    "On"
                } else {
                    "Off"
                }),
            );
            out.insert(
                "#correct-aim".into(),
                json!(if viewer.options.correct_aim {
                    "On"
                } else {
                    "Off"
                }),
            );
        }
        Screen::Materials => materials(viewer, &mut out),
        Screen::Verifier => verifier(viewer, &mut out),
        Screen::Hotkeys => {
            for (index, (_, title, _)) in crate::hotkeys::BINDINGS.into_iter().enumerate() {
                let value = crate::hotkeys::binding(&viewer.options.hotkeys, index);
                out.insert(format!("#title{index}"), json!(title));
                out.insert(format!("#bind{index}"), json!(value));
                controls.push(json!({"id":format!("bind-{index}"),"value":value}));
            }
        }
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
        out.insert(format!("#materialIcon{row}"), material_icon(line));
        let name = line.map_or(String::new(), |line| {
            let name = line.name.strip_prefix("minecraft:").unwrap_or(&line.name);
            short(&name.replace('_', " "), 21)
        });
        let total = line.map_or(String::new(), |line| line.total.to_string());
        let counts = line.map_or_else(
            || [String::new(), String::new(), String::new()],
            |line| {
                if !scanned {
                    return ["-".into(), "-".into(), "-".into()];
                }
                let indices = viewer.preview.material_indices(&line.name);
                viewer
                    .tiles
                    .iter()
                    .map(|tile| tile.material_counts(&indices))
                    .fold([0; 3], |sum, counts| {
                        std::array::from_fn(|i| sum[i] + counts[i])
                    })
                    .map(|n| n.to_string())
            },
        );
        out.insert(format!("#row{row}"), json!(name));
        out.insert(format!("#total{row}"), json!(total));
        for (column, count) in ["needed", "placed", "unknown"].into_iter().zip(counts) {
            out.insert(format!("#{column}{row}"), json!(count));
        }
    }
}

pub(super) fn material_icon(line: Option<&schematica_core::MaterialLine>) -> Value {
    // The native item renderer supplies isometric block artwork; item-shaped
    // materials retain their normal sprites. Unknown identifiers draw nothing.
    json!({"item":{"identifier": line.map_or("minecraft:air", |line| line.name.as_str()), "metadata":0}})
}

fn verifier(viewer: &Viewer, out: &mut Map<String, Value>) {
    let total = viewer.mismatch_count();
    let pending = viewer
        .tiles
        .iter()
        .filter(|tile| tile.stamp.is_none())
        .count();
    let summary = if viewer.dirty || viewer.tiles.is_empty() {
        "Show a placement in-world to scan.".into()
    } else {
        format!("{total} mismatches or unknown cells | {pending} tiles pending")
    };
    out.insert("#scan".into(), json!(summary));
    out.insert(
        "#page".into(),
        json!(format!(
            "{} / {}",
            viewer.verifier_page + 1,
            total.div_ceil(ROWS).max(1)
        )),
    );
    let mut mismatches = viewer.mismatches().skip(viewer.verifier_page * ROWS);
    for row in 0..ROWS {
        let (coordinates, kind) = match mismatches.next().filter(|_| !viewer.dirty) {
            Some(([x, y, z], status)) => {
                let kind = match status {
                    CellStatus::Missing => "Missing",
                    CellStatus::Wrong => "Wrong block",
                    CellStatus::WrongState => "Wrong state",
                    CellStatus::Extra => "Extra",
                    CellStatus::Unloaded => "Unknown",
                    CellStatus::Correct => "",
                };
                (format!("{x}, {y}, {z}"), kind)
            }
            None => (String::new(), ""),
        };
        out.insert(format!("#pos{row}"), json!(coordinates));
        out.insert(format!("#kind{row}"), json!(kind));
        out.insert(format!("#verify-visible{row}"), json!(!kind.is_empty()));
    }
}

fn short(text: &str, chars: usize) -> String {
    text.chars().take(chars).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn every_panel_has_unique_control_ids() {
        for template in [CONTROL, LOAD, SETTINGS, MATERIALS, MAIN, VERIFIER, HOTKEYS] {
            let panel: Value = serde_json::from_str(template).unwrap();
            let mut ids = HashSet::new();
            for control in panel["controls"].as_array().unwrap() {
                let id = control["id"].as_str().unwrap();
                assert!(ids.insert(id), "duplicate panel control {id}");
            }
        }
    }

    #[test]
    fn fullscreen_layout_keeps_the_same_controls() {
        let compact: Value = serde_json::from_str(SETTINGS).unwrap();
        let full = content(Screen::Settings, true);
        let full_panel: Value = serde_json::from_str(&full).unwrap();
        assert_eq!(compact["controls"], full_panel["controls"]);
        let document: Value =
            serde_json::from_str(full_panel["surface"]["document"].as_str().unwrap()).unwrap();
        fn static_children(value: &Value) {
            match value {
                Value::Object(object) => {
                    assert!(object.get("controls").is_none_or(Value::is_array));
                    for value in object.values() {
                        static_children(value);
                    }
                }
                Value::Array(array) => {
                    for value in array {
                        static_children(value);
                    }
                }
                _ => {}
            }
        }
        static_children(&document);
        assert_eq!(
            document["menu"]["controls"][0]["card"]["size"],
            json!(["100%", "100%"])
        );
        if let Some(path) = std::env::var_os("SCHEMATICA_PANEL_REVIEW") {
            let mut panel: Value = serde_json::from_str(content(Screen::Main, true)).unwrap();
            let mut bindings = Map::new();
            crate::dashboard::bindings(&crate::tests::reference_viewer(), &mut bindings);
            bindings.insert("#build".into(), json!("Builder preview"));
            panel["surface"]["bindings"] = json!(bindings);
            std::fs::write(path, panel.to_string()).unwrap();
            let path =
                std::path::PathBuf::from(std::env::var_os("SCHEMATICA_PANEL_REVIEW").unwrap());
            for (screen, fullscreen, name) in [
                (Screen::Materials, true, "materials-template.json"),
                (Screen::Main, false, "compact-template.json"),
            ] {
                let mut panel: Value = serde_json::from_str(content(screen, fullscreen)).unwrap();
                let mut bindings = Map::new();
                let viewer = crate::tests::reference_viewer();
                if screen == Screen::Materials {
                    materials(&viewer, &mut bindings);
                } else {
                    crate::dashboard::bindings(&viewer, &mut bindings);
                }
                bindings.insert("#build".into(), json!("v0.3.0 / preview"));
                panel["surface"]["bindings"] = json!(bindings);
                std::fs::write(path.with_file_name(name), panel.to_string()).unwrap();
            }
        }
    }
}
