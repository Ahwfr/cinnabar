wit_bindgen::generate!({
    path: "wit",
    world: "cinnabar:extension/extension@0.1.0",
    generate_all,
});

mod menu;
mod overlay;

use cinnabar::{
    extension::{budget, gameplay, hud, input, panel, settings, terrain},
    world_draw::drawing,
};
use menu::Screen;
use overlay::{Preview, Tile};
use schematica_core::{LayerMode, MaterialLine, Mirror, Placement, Schematic, blueprint};
use std::{cell::RefCell, collections::BTreeMap, f32::consts::TAU};

thread_local! {
    static RUNTIME: RefCell<(Runtime, Vec<&'static [u8]>, Options)> =
        const { RefCell::new((Runtime::Start, Vec::new(), Options { textured: true, assist: false })) };
}

#[derive(Clone, Copy)]
struct Options {
    textured: bool,
    assist: bool,
}

impl Options {
    fn save(self) {
        let _ = settings::save(
            &serde_json::json!({
                "textured":self.textured, "assist":self.assist,
            })
            .to_string(),
        );
    }
}

enum Runtime {
    Start,
    Loading(blueprint::Decoder<'static>),
    Decoded(Schematic),
    Counting {
        schematic: Schematic,
        counts: Vec<u64>,
        totals: BTreeMap<String, u64>,
        cursor: usize,
    },
    Finishing(Schematic, BTreeMap<String, u64>),
    Ready(Viewer),
    Invalid,
}

struct Schematica;

impl Guest for Schematica {
    fn init() {
        if let Ok(raw) = settings::load()
            && raw.len() <= 256
            && let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw)
        {
            RUNTIME.with(|state| {
                let options = &mut state.borrow_mut().2;
                options.textured = value["textured"].as_bool().unwrap_or(options.textured);
                options.assist = value["assist"].as_bool().unwrap_or(options.assist);
            });
        }
        label("Schematica | Alt+J library | Alt+I controls | Alt+K settings");
    }

    fn frame() {
        RUNTIME.with(|runtime| {
            let (runtime, catalog, options) = &mut *runtime.borrow_mut();
            if matches!(*runtime, Runtime::Start) {
                static SLOT: [u8; blueprint::SLOT_BYTES] = blueprint::empty_slot();
                let bytes = std::hint::black_box(&SLOT).as_slice();
                let bytes = blueprint::embedded_payload(bytes).unwrap_or_default();
                match blueprint::library(bytes) {
                    Ok(entries) => {
                        *catalog = entries;
                        *runtime = Runtime::Ready(Viewer::new(
                            Schematic::new("No schematic loaded", [1; 3]),
                            Vec::new(),
                            library_names(catalog),
                            *options,
                            false,
                        ));
                    }
                    Err(_) => invalidate(runtime),
                }
            }
            match &mut *runtime {
                Runtime::Ready(viewer) => {
                    if let Some(index) = viewer.request {
                        if viewer.release_for_switch() {
                            *options = viewer.options;
                            match blueprint::Decoder::new(catalog[index]) {
                                Ok(decoder) => *runtime = Runtime::Loading(decoder),
                                Err(_) => invalidate(runtime),
                            }
                        }
                    } else {
                        viewer.frame();
                    }
                }
                Runtime::Loading(decoder) => {
                    for _ in 0..8 {
                        if budget::remaining_fuel() < 500_000 {
                            break;
                        }
                        match decoder.step() {
                            Ok(false) => {}
                            Ok(true) => {
                                let Runtime::Loading(decoder) =
                                    std::mem::replace(&mut *runtime, Runtime::Invalid)
                                else {
                                    unreachable!()
                                };
                                match decoder.finish() {
                                    Ok(schematic) => *runtime = Runtime::Decoded(schematic),
                                    Err(_) => invalidate(runtime),
                                }
                                break;
                            }
                            Err(_) => {
                                invalidate(runtime);
                                break;
                            }
                        }
                    }
                }
                Runtime::Decoded(_) => {
                    let Runtime::Decoded(schematic) =
                        std::mem::replace(&mut *runtime, Runtime::Invalid)
                    else {
                        unreachable!()
                    };
                    let counts = schematic.palette_counts();
                    *runtime = Runtime::Counting {
                        schematic,
                        counts,
                        totals: BTreeMap::new(),
                        cursor: 0,
                    };
                }
                Runtime::Counting {
                    schematic,
                    counts,
                    totals,
                    cursor,
                } => {
                    let stop = (*cursor + 8).min(counts.len());
                    while *cursor < stop && budget::remaining_fuel() >= 500_000 {
                        let state = &schematic.palette()[*cursor];
                        let count = counts[*cursor];
                        if count > 0 && !state.is_air() {
                            *totals.entry(state.name.clone()).or_insert(0) += count;
                        }
                        *cursor += 1;
                    }
                    if *cursor == counts.len() {
                        let Runtime::Counting {
                            schematic, totals, ..
                        } = std::mem::replace(&mut *runtime, Runtime::Invalid)
                        else {
                            unreachable!()
                        };
                        *runtime = Runtime::Finishing(schematic, totals);
                    }
                }
                Runtime::Finishing(..) => {
                    let Runtime::Finishing(schematic, totals) =
                        std::mem::replace(&mut *runtime, Runtime::Invalid)
                    else {
                        unreachable!()
                    };
                    let mut materials: Vec<_> = totals
                        .into_iter()
                        .map(|(name, total)| MaterialLine {
                            name,
                            total,
                            missing: total,
                        })
                        .collect();
                    materials.sort_by_key(|line| std::cmp::Reverse(line.total));
                    *runtime = Runtime::Ready(Viewer::new(
                        schematic,
                        materials,
                        library_names(catalog),
                        *options,
                        true,
                    ));
                    label("Schematica | Alt+J library | Alt+I controls | Alt+K settings");
                }
                Runtime::Invalid => {}
                Runtime::Start => unreachable!(),
            }
        });
    }
}

fn library_names(catalog: &[&[u8]]) -> Vec<String> {
    catalog
        .iter()
        .map(|entry| blueprint::title(entry).unwrap_or_else(|_| "Invalid schematic".into()))
        .collect()
}

fn invalidate(runtime: &mut Runtime) {
    *runtime = Runtime::Invalid;
    label("Schematica | Build the component with a valid embedded blueprint.");
}

export!(Schematica);

fn label(message: &str) {
    let mut end = message.len().min(256);
    while !message.is_char_boundary(end) {
        end -= 1;
    }
    let _ = hud::set_label(&message[..end]);
}

struct Viewer {
    schematic: Schematic,
    materials: Vec<schematica_core::MaterialLine>,
    placement: Option<Placement>,
    world: Option<(u64, i32)>,
    pose: Option<gameplay::Snapshot>,
    preview: Preview,
    tiles: Vec<Tile>,
    retired: Vec<Tile>,
    cursor: usize,
    layer: i32,
    layer_mode: u8,
    shown: bool,
    options: Options,
    material_page: usize,
    screen: Screen,
    library: Vec<String>,
    selected: Option<usize>,
    request: Option<usize>,
    loaded: bool,
    auto_place: bool,
    rotate_ccw: bool,
    flip_z: bool,
    sort_name: bool,
    dirty: bool,
    last_label: String,
}

impl Viewer {
    fn new(
        schematic: Schematic,
        materials: Vec<MaterialLine>,
        library: Vec<String>,
        options: Options,
        loaded: bool,
    ) -> Self {
        Self {
            schematic,
            materials,
            placement: None,
            world: None,
            pose: None,
            preview: Preview::default(),
            tiles: Vec::new(),
            retired: Vec::new(),
            cursor: 0,
            layer: 0,
            layer_mode: 0,
            shown: true,
            options,
            material_page: 0,
            screen: Screen::Control,
            library,
            selected: None,
            request: None,
            loaded,
            auto_place: loaded,
            rotate_ccw: false,
            flip_z: true,
            sort_name: false,
            dirty: false,
            last_label: String::new(),
        }
    }

    fn frame(&mut self) {
        let Ok(world) = gameplay::read_frame() else {
            label("Schematica | Grant terrain access in Settings > Mods.");
            return;
        };
        let active_world = world.is_some();
        if let Some(world) = world {
            let scope = (world.session, world.dimension);
            if self.world != Some(scope) {
                // Cinnabar invalidates drawing handles on dimension changes.
                self.tiles.clear();
                self.retired.clear();
                self.placement = None;
                self.world = Some(scope);
                self.last_label.clear();
            }
            self.pose = Some(world);
        }

        let Ok(keys) = input::read_controls() else {
            label("Schematica | Grant controls access in Settings > Mods.");
            return;
        };
        if !keys.focused {
            return;
        }
        let alt = keys
            .keys_held
            .iter()
            .any(|key| key == "AltLeft" || key == "AltRight");
        let shortcut = alt
            .then(|| {
                keys.keys_pressed.iter().find_map(|key| match key.as_str() {
                    "KeyJ" => Some(Screen::Load),
                    "KeyI" => Some(Screen::Control),
                    "KeyK" => Some(Screen::Settings),
                    "KeyP" if !self.loaded => Some(Screen::Load),
                    _ => None,
                })
            })
            .flatten();
        if keys.panel_open {
            let old = self.screen;
            let mut changed = false;
            if self.screen == Screen::Control {
                for edit in input::read_text_events().unwrap_or_default() {
                    changed |= self.edit_number(&edit.id, &edit.value);
                }
            }
            for event in keys.events {
                self.menu_action(&event.id);
                changed = true;
            }
            if old != self.screen {
                menu::open(self, true);
            } else if changed {
                menu::refresh(self);
            }
            return;
        }
        if !active_world || !keys.gameplay {
            return;
        }
        if let Some(screen) = shortcut {
            self.screen = screen;
            self.selected = None;
            menu::open(self, false);
            return;
        }
        let caps = drawing::capabilities();
        if !caps.contains(drawing::Permissions::WORLD_DRAW) {
            label("Schematica | Grant world_draw in Settings > Mods.");
            return;
        }

        if alt {
            for key in &keys.keys_pressed {
                self.dirty |= self.control(key);
            }
        }
        if alt && keys.keys_pressed.iter().any(|key| key == "KeyU") {
            let message = self.aimed_block().map_or_else(
                || "Schematica | Aim at a planned block".into(),
                |(block, pos)| format!("Schematica | {} at {pos:?}", block.name),
            );
            label(&message);
            return;
        }
        if self.auto_place {
            self.auto_place = false;
            self.dirty |= self.control("KeyP");
        }
        if self.dirty {
            self.rebuild();
            self.dirty = false;
            return;
        }
        if !self.retired.is_empty() {
            // A tile owns up to four handles. Stay under the host's 32 drawing
            // calls per callback even when moving or hiding a large guide.
            self.clear_retired();
            self.publish_label();
            return;
        }
        if !self.tiles.is_empty() && !self.preview.palette_ready(&self.schematic) {
            self.preview.prepare(&self.schematic);
            return;
        }
        if budget::remaining_fuel() < 650_000 {
            return;
        }
        let textured = self.options.textured
            && self.tiles.len() <= 128
            && caps.contains(drawing::Permissions::RENDER_MATERIAL);
        if self.shown && !self.tiles.is_empty() {
            let tile = &mut self.tiles[self.cursor];
            if tile.step(
                &self.preview,
                &self.schematic,
                self.world.unwrap().1,
                textured,
            ) {
                self.cursor = (self.cursor + 1) % self.tiles.len();
            }
        }
        if self.options.textured && !textured && self.placement.is_some() {
            self.show_label(
                "Schematica | Textures need render_material and at most 128 visible tiles",
            );
        } else {
            self.publish_label();
        }
    }

    fn control(&mut self, key: &str) -> bool {
        match key {
            "KeyP" => {
                if !self.loaded {
                    return false;
                }
                let Some(frame) = &self.pose else {
                    return false;
                };
                let yaw = frame.yaw.rem_euclid(TAU);
                let [width, _, depth] = self.placement.map_or(self.schematic.size(), |p| {
                    p.footprint(self.schematic.size())
                });
                let distance = (width.max(depth) as f32 / 2. + 3.).min(36.);
                let origin = [
                    (frame.eye.x - yaw.sin() * distance).floor() as i32 - width / 2,
                    (frame.eye.y - 1.62).round() as i32,
                    (frame.eye.z - yaw.cos() * distance).floor() as i32 - depth / 2,
                ];
                self.placement
                    .get_or_insert_with(|| Placement::new(origin))
                    .origin = origin;
                self.shown = true;
            }
            "KeyH" => self.shown = !self.shown,
            "KeyB" => {
                self.options.textured = !self.options.textured;
                self.options.save();
            }
            "KeyL" => self.layer_mode = (self.layer_mode + 1) % 3,
            "BracketLeft" => {
                if self.layer_mode == 0 {
                    self.layer_mode = 1;
                }
                self.layer = (self.layer - 1).max(0);
            }
            "BracketRight" => {
                if self.layer_mode == 0 {
                    self.layer_mode = 1;
                } else {
                    self.layer = (self.layer + 1).min(self.schematic.size()[1] - 1);
                }
            }
            _ => {
                let Some(p) = &mut self.placement else {
                    return false;
                };
                match key {
                    "KeyR" => p.rotate_cw(),
                    "RotateLeft" => p.rotate_ccw(),
                    "KeyM" => {
                        p.mirror = match p.mirror {
                            Mirror::None => Mirror::X,
                            Mirror::X => Mirror::Z,
                            Mirror::Z => Mirror::None,
                        }
                    }
                    "ArrowLeft" => p.origin[0] -= 1,
                    "ArrowRight" => p.origin[0] += 1,
                    "ArrowUp" => p.origin[2] -= 1,
                    "ArrowDown" => p.origin[2] += 1,
                    "PageUp" => p.origin[1] += 1,
                    "PageDown" => p.origin[1] -= 1,
                    _ => return false,
                }
            }
        }
        true
    }

    fn move_here(&mut self) -> bool {
        if !self.loaded {
            return false;
        }
        let Some(pose) = &self.pose else { return false };
        let placement = self.placement.get_or_insert_with(|| Placement::new([0; 3]));
        let [width, _, depth] = placement.footprint(self.schematic.size());
        let quadrant = ((180. - pose.yaw.to_degrees()) / 90.).floor() as i32 & 3;
        let (dx, dz) = [(-width, 1), (-width, -depth), (1, -depth), (1, 1)][quadrant as usize];
        placement.origin = [
            pose.eye.x.floor() as i32 + dx,
            (pose.eye.y - 1.62).round() as i32,
            pose.eye.z.floor() as i32 + dz,
        ];
        self.shown = true;
        true
    }

    fn edit_number(&mut self, id: &str, text: &str) -> bool {
        if let Ok(value) = text.trim().parse::<i32>() {
            if id == "layer" {
                self.layer = value.clamp(0, self.schematic.size()[1] - 1);
                if self.layer_mode == 0 {
                    self.layer_mode = 1;
                }
                self.dirty = true;
            } else if let (Some(p), Some(axis)) = (
                &mut self.placement,
                ["x", "y", "z"].iter().position(|name| *name == id),
            ) {
                p.origin[axis] = value.clamp(-30_000_000, 30_000_000);
                self.dirty = true;
            }
        }
        // Repaint the accepted value even when the submitted text was invalid.
        true
    }

    fn aimed_block(&self) -> Option<(&schematica_core::BlockState, [i32; 3])> {
        let (Some(placement), Some(pose)) = (self.placement, &self.pose) else {
            return None;
        };
        if !self.shown {
            return None;
        }
        let forward = [
            -pose.yaw.sin() * pose.pitch.cos(),
            pose.pitch.sin(),
            -pose.yaw.cos() * pose.pitch.cos(),
        ];
        let eye = [pose.eye.x, pose.eye.y, pose.eye.z];
        for step in 0..=80 {
            let world =
                [0, 1, 2].map(|i| (eye[i] + forward[i] * (step as f32 / 16.)).floor() as i32);
            let Some(local) = placement.world_to_local(self.schematic.size(), world) else {
                continue;
            };
            if self.layers().includes(local[1])
                && let Some(block) = self.schematic.get(local)
                && !block.is_air()
            {
                return Some((block, world));
            }
        }
        None
    }

    fn menu_action(&mut self, id: &str) {
        if let Some(index) = id
            .strip_prefix("select-")
            .and_then(|value| value.parse::<usize>().ok())
        {
            if index < self.library.len() {
                self.selected = Some(index);
            }
            return;
        }
        let key = match id {
            "place" => {
                self.dirty |= self.move_here();
                return;
            }
            "load" => {
                if let Some(index) = self.selected {
                    self.request = Some(index);
                    let _ = panel::close();
                    label("Schematica | Loading selected blueprint");
                }
                return;
            }
            "library" => {
                self.screen = Screen::Load;
                self.selected = None;
                return;
            }
            "settings" => {
                self.screen = Screen::Settings;
                return;
            }
            "assist" => {
                self.options.assist = !self.options.assist;
                self.options.save();
                return;
            }
            "unload" => {
                self.placement = None;
                self.dirty = true;
                let _ = panel::close();
                return;
            }
            "materials" => {
                self.screen = Screen::Materials;
                self.material_page = 0;
                return;
            }
            "back" => {
                self.screen = Screen::Control;
                return;
            }
            "sort" => {
                self.sort_name = !self.sort_name;
                if self.sort_name {
                    self.materials.sort_by(|a, b| a.name.cmp(&b.name));
                } else {
                    self.materials
                        .sort_by(|a, b| b.total.cmp(&a.total).then_with(|| a.name.cmp(&b.name)));
                }
                self.material_page = 0;
                return;
            }
            "flip-axis" => {
                self.flip_z = !self.flip_z;
                return;
            }
            "rotate-axis" => {
                self.rotate_ccw = !self.rotate_ccw;
                return;
            }
            "rotate" if self.rotate_ccw => "RotateLeft",
            "rotate" => "KeyR",
            "cw" => "KeyR",
            "ccw" => "RotateLeft",
            "mirror" => {
                if let Some(p) = &mut self.placement {
                    // The button's axis is in world space. The core mirrors
                    // before rotating, so swap local axes on odd turns.
                    let flip = if self.flip_z ^ (p.quarter_turns % 2 != 0) {
                        Mirror::Z
                    } else {
                        Mirror::X
                    };
                    if p.mirror == Mirror::None {
                        p.mirror = flip;
                    } else {
                        if p.mirror != flip {
                            p.quarter_turns = (p.quarter_turns + 2) % 4;
                        }
                        p.mirror = Mirror::None;
                    }
                    self.dirty = true;
                }
                return;
            }
            "mode" => "KeyL",
            "previous-layer" => "BracketLeft",
            "next-layer" => "BracketRight",
            "visibility" => "KeyH",
            "look" => "KeyB",
            "previous-page" => {
                self.material_page = self.material_page.saturating_sub(1);
                return;
            }
            "next-page" => {
                let last = self.materials.len().div_ceil(menu::ROWS).saturating_sub(1);
                self.material_page = (self.material_page + 1).min(last);
                return;
            }
            "close" => {
                let _ = panel::close();
                return;
            }
            _ => return,
        };
        self.dirty |= self.control(key);
    }

    fn layers(&self) -> LayerMode {
        match self.layer_mode {
            1 => LayerMode::Single(self.layer),
            2 => LayerMode::Range {
                min: 0,
                max: self.layer,
            },
            _ => LayerMode::All,
        }
    }

    fn layers_text(&self) -> String {
        let height = self.schematic.size()[1];
        match self.layers() {
            LayerMode::All => format!("All {height} layers"),
            LayerMode::Single(y) => format!("Layer {} of {height}", y + 1),
            LayerMode::Range { min, max } => format!("Layers {}-{} of {height}", min + 1, max + 1),
        }
    }

    fn rebuild(&mut self) {
        self.retired
            .extend(self.tiles.drain(..).filter(Tile::has_draws));
        self.cursor = 0;
        if let Some(p) = self.placement.filter(|_| self.shown) {
            self.preview = Preview::new(p, self.layers());
            self.tiles = self.preview.tiles(self.schematic.size());
        }
    }

    fn clear_retired(&mut self) {
        for _ in 0..6 {
            let Some(mut tile) = self.retired.pop() else {
                break;
            };
            tile.clear();
        }
    }

    fn release_for_switch(&mut self) -> bool {
        self.retired
            .extend(self.tiles.drain(..).filter(Tile::has_draws));
        self.clear_retired();
        self.retired.is_empty()
    }

    fn show_label(&mut self, message: &str) {
        if message != self.last_label {
            label(message);
            self.last_label = message.to_owned();
        }
    }

    fn publish_label(&mut self) {
        if self.options.assist
            && let Some((block, pos)) = self.aimed_block()
        {
            let message = format!("Target | {} at {pos:?}", block.name);
            self.show_label(&message);
            return;
        }
        let pending = self
            .tiles
            .iter()
            .filter(|tile| tile.stamp.is_none())
            .count();
        let message = if let Some(p) = self.placement {
            let needed: u64 = self.tiles.iter().map(|t| t.tally.missing).sum();
            let wrong: u64 = self.tiles.iter().map(|t| t.tally.wrong).sum();
            let extra: u64 = self.tiles.iter().map(|t| t.tally.extra).sum();
            let unknown: u64 = self.tiles.iter().map(|t| t.tally.unloaded).sum();
            let layer = self.layers_text();
            let note = if !self.shown {
                "hidden".into()
            } else if let Some(error) = self.tiles.iter().find_map(|t| t.error.as_ref()) {
                error.clone()
            } else {
                format!("{pending} tiles pending")
            };
            format!(
                "{} {:?} | {} | missing {} wrong {} extra {} unknown {} | {}",
                self.schematic.name, p.origin, layer, needed, wrong, extra, unknown, note
            )
        } else {
            format!(
                "{} | Alt+J library | Alt+I controls | Alt+K settings",
                self.schematic.name
            )
        };
        self.show_label(&message);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_edits_turn_the_guide_and_select_layer_ranges() {
        let mut viewer = Viewer::new(
            Schematic::new("fixture", [2, 4, 3]),
            Vec::new(),
            vec!["Demo".into(), "Wagon".into()],
            Options {
                textured: true,
                assist: false,
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
}
