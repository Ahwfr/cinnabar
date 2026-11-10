wit_bindgen::generate!({
    path: "wit",
    world: "cinnabar:extension/extension@0.1.0",
    generate_all,
});

mod hotkeys;
mod menu;
mod overlay;

// Embed the revision in the guest itself so the importer labels the code it actually packages.
#[cfg(target_arch = "wasm32")]
const BUILD_REVISION: &str = match option_env!("SCHEMATICA_BUILD_REVISION") {
    Some(revision) => revision,
    None => "",
};

#[cfg(target_arch = "wasm32")]
#[used]
#[unsafe(link_section = "schematica:build-revision")]
static GUEST_REVISION: [u8; BUILD_REVISION.len()] = {
    let mut bytes = [0; BUILD_REVISION.len()];
    bytes.copy_from_slice(BUILD_REVISION.as_bytes());
    bytes
};

use cinnabar::{
    extension::{budget, gameplay, hud, input, item_use, panel, settings, terrain},
    world_draw::drawing,
};
use menu::Screen;
use overlay::{Preview, Tile};
use schematica_core::{LayerMode, MaterialLine, Mirror, Placement, Schematic, blueprint};
use std::{cell::RefCell, collections::BTreeMap, f32::consts::TAU};

thread_local! {
    static RUNTIME: RefCell<(Runtime, Vec<&'static [u8]>, Options)> =
        const { RefCell::new((Runtime::Start, Vec::new(), Options { textured: true, assist: false, fast_place: false, correct_aim: false, opacity: 65, hotkeys: BTreeMap::new() })) };
}

#[derive(Clone)]
struct Options {
    textured: bool,
    assist: bool,
    fast_place: bool,
    correct_aim: bool,
    opacity: u8,
    hotkeys: BTreeMap<usize, String>,
}

impl Options {
    fn menu_hint(&self, title: &str) -> String {
        let menu = hotkeys::for_action(&self.hotkeys, hotkeys::Action::Main);
        let settings = hotkeys::for_action(&self.hotkeys, hotkeys::Action::Config);
        let menu = if menu.is_empty() {
            "Alt+J Library".into()
        } else {
            format!("{menu} Menu")
        };
        let settings = if settings.is_empty() {
            "Alt+K"
        } else {
            settings
        };
        format!("{title} | {menu} | {settings} Settings")
    }

    fn save(&self) {
        let _ = settings::save(
            &serde_json::json!({
                "textured":self.textured, "assist":self.assist, "fast_place": self.fast_place,
                "correct_aim": self.correct_aim,
                "opacity": self.opacity, "hotkeys": self.hotkeys,
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
            && raw.len() <= 1024
            && let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw)
        {
            RUNTIME.with(|state| {
                let options = &mut state.borrow_mut().2;
                options.textured = value["textured"].as_bool().unwrap_or(options.textured);
                options.assist = value["assist"].as_bool().unwrap_or(options.assist);
                options.fast_place = value["fast_place"].as_bool().unwrap_or(options.fast_place);
                options.correct_aim = value["correct_aim"]
                    .as_bool()
                    .unwrap_or(options.correct_aim);
                if let Some(opacity) = value["opacity"].as_u64() {
                    options.opacity = opacity.clamp(25, 90) as u8;
                }
                if let Some(bindings) = value["hotkeys"].as_object() {
                    for (index, value) in bindings {
                        if let (Ok(index), Some(value)) = (index.parse(), value.as_str()) {
                            hotkeys::edit(&mut options.hotkeys, index, value);
                        }
                    }
                }
            });
        }
        RUNTIME.with(|runtime| label(&runtime.borrow().2.menu_hint("Schematica")));
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
                            options.clone(),
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
                            *options = viewer.options.clone();
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
                        options.clone(),
                        true,
                    ));
                    label(&options.menu_hint("Schematica"));
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
    render_all: bool,
    locked: bool,
    menu_armed: bool,
    options: Options,
    material_page: usize,
    verifier_page: usize,
    verifier_marker: Option<overlay::DrawPair>,
    verifier_target: Option<[i32; 3]>,
    verifier_marker_changed: bool,
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
            render_all: true,
            locked: false,
            menu_armed: false,
            options,
            material_page: 0,
            verifier_page: 0,
            verifier_marker: None,
            verifier_target: None,
            verifier_marker_changed: false,
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
                self.locked = false;
                self.verifier_marker = None;
                self.verifier_target = None;
                self.verifier_marker_changed = false;
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
            self.menu_armed = false;
            return;
        }
        let alt = keys
            .keys_held
            .iter()
            .any(|key| key == "AltLeft" || key == "AltRight");
        let legacy = alt
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
            self.menu_armed = false;
            let old = self.screen;
            let mut changed = false;
            if self.screen == Screen::Control {
                for edit in input::read_text_events().unwrap_or_default() {
                    changed |= self.edit_number(&edit.id, &edit.value);
                }
            }
            if self.screen == Screen::Hotkeys {
                for edit in input::read_text_events().unwrap_or_default() {
                    if let Some(index) =
                        edit.id.strip_prefix("bind-").and_then(|id| id.parse().ok())
                    {
                        hotkeys::edit(&mut self.options.hotkeys, index, &edit.value);
                        self.options.save();
                        changed = true;
                    }
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
            self.menu_armed = false;
            return;
        }
        if let Some(screen) = legacy.or_else(|| self.builder_hotkey(&keys, alt)) {
            self.screen = screen;
            self.selected = None;
            self.verifier_page = 0;
            self.material_page = 0;
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
        if std::mem::take(&mut self.verifier_marker_changed) {
            self.clear_verifier_marker();
            if let Some(cell) = self.verifier_target {
                self.verifier_marker = overlay::highlight(self.world.unwrap().1, cell).ok();
            }
        }
        let placement_cell = if self.options.fast_place && self.shown && self.placement.is_some() {
            item_use::read_placement_cell()
                .ok()
                .flatten()
                .map(|(x, y, z)| [x, y, z])
        } else {
            None
        };
        if let Some(placement) = self.placement
            && let Some(pos @ [x, y, z]) = placement_cell
            && let Some(block) = self.planned_block(pos)
        {
            let block = placement.transform_state(block);
            let states = overlay::canonical(&block);
            let offer = if self.options.correct_aim {
                item_use::offer_oriented_placement((x, y, z), &block.name, &states)
            } else {
                item_use::offer_creative_placement((x, y, z), &block.name, &states)
            };
            if let Err(error) = offer {
                self.show_label(&format!("Fast Place | {error}"));
                return;
            }
        }
        if !self.retired.is_empty() {
            // A tile owns up to four handles. Stay under the host's 32 drawing
            // calls per callback even when moving or hiding a large guide.
            self.clear_retired();
            self.publish_label(placement_cell);
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
                textured.then_some(f32::from(self.options.opacity) / 100.),
            ) {
                self.cursor = (self.cursor + 1) % self.tiles.len();
            }
        }
        if self.options.textured && !textured && self.placement.is_some() {
            self.show_label(
                "Schematica | Textures need render_material and at most 128 visible tiles",
            );
        } else {
            self.publish_label(placement_cell);
        }
    }

    /// Resolve chords before opening the main menu on release of a standalone M.
    fn builder_hotkey(&mut self, keys: &input::Controls, alt: bool) -> Option<Screen> {
        let modified = alt
            || ["Control", "Shift", "Super"]
                .iter()
                .any(|modifier| keys.keys_held.iter().any(|key| key.starts_with(modifier)));
        if modified {
            self.menu_armed = false;
            return None;
        }
        use hotkeys::Action;
        match hotkeys::pressed(&self.options.hotkeys, keys, &mut self.menu_armed)? {
            Action::Main => return Some(Screen::Main),
            Action::Config => return Some(Screen::Settings),
            Action::Placement | Action::PlacementConfig => return Some(Screen::Control),
            Action::Verifier => return Some(Screen::Verifier),
            Action::Materials => return Some(Screen::Materials),
            Action::Render => {
                self.render_all = !self.render_all;
                self.dirty = true;
                self.last_label.clear();
                if !self.render_all {
                    label("");
                }
            }
            Action::Ghosts => self.dirty |= self.control("KeyB"),
            Action::NextMode => self.dirty |= self.control("KeyL"),
            Action::PreviousMode => {
                self.layer_mode = (self.layer_mode + 2) % 3;
                self.dirty = true;
            }
            Action::NextLayer => self.dirty |= self.control("BracketRight"),
            Action::PreviousLayer => self.dirty |= self.control("BracketLeft"),
        }
        None
    }

    fn control(&mut self, key: &str) -> bool {
        match key {
            "KeyP" => {
                if !self.loaded || self.locked {
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
                if self.locked {
                    return false;
                }
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
        if !self.loaded || self.locked {
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
            } else if !self.locked
                && let (Some(p), Some(axis)) = (
                    &mut self.placement,
                    ["x", "y", "z"].iter().position(|name| *name == id),
                )
            {
                p.origin[axis] = value.clamp(-30_000_000, 30_000_000);
                self.dirty = true;
            }
        }
        // Repaint the accepted value even when the submitted text was invalid.
        true
    }

    fn planned_block(&self, world: [i32; 3]) -> Option<&schematica_core::BlockState> {
        if !self.shown {
            return None;
        }
        let local = self
            .placement?
            .world_to_local(self.schematic.size(), world)?;
        let block = self.schematic.get(local)?;
        (self.layers().includes(local[1]) && !block.is_air()).then_some(block)
    }

    fn aimed_block(&self) -> Option<(&schematica_core::BlockState, [i32; 3])> {
        let pose = self.pose.as_ref()?;
        if !self.shown || self.placement.is_none() {
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
            if let Some(block) = self.planned_block(world) {
                return Some((block, world));
            }
        }
        None
    }

    fn menu_action(&mut self, id: &str) {
        if let Some(row) = id
            .strip_prefix("verify-row-")
            .and_then(|value| value.parse::<usize>().ok())
        {
            if row < menu::ROWS && !self.dirty {
                let index = self.verifier_page * menu::ROWS + row;
                let cell = self.mismatches().nth(index).map(|&(cell, _)| cell);
                if let Some(cell) = cell {
                    self.verifier_target = Some(cell);
                    self.verifier_marker_changed = true;
                    let _ = panel::close();
                }
            }
            return;
        }
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
            "main" => {
                self.screen = Screen::Main;
                return;
            }
            "hotkeys" => {
                self.screen = Screen::Hotkeys;
                return;
            }
            "reset-hotkeys" => {
                self.options.hotkeys.clear();
                self.options.save();
                return;
            }
            "verifier" => {
                self.screen = Screen::Verifier;
                self.verifier_page = 0;
                return;
            }
            "lock" => {
                if self.placement.is_some() {
                    self.locked = !self.locked;
                }
                return;
            }
            "opacity" => {
                self.options.opacity = match self.options.opacity {
                    0..=25 => 45,
                    26..=45 => 65,
                    46..=65 => 90,
                    _ => 25,
                };
                self.options.save();
                self.dirty = true;
                return;
            }
            "verify-previous" => {
                self.verifier_page = self.verifier_page.saturating_sub(1);
                return;
            }
            "verify-next" => {
                self.verifier_page = (self.verifier_page + 1)
                    .min(self.mismatch_count().div_ceil(menu::ROWS).saturating_sub(1));
                return;
            }
            "verify-clear" => {
                self.verifier_target = None;
                self.verifier_marker_changed = true;
                return;
            }
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
            "fast-place" => {
                self.options.fast_place = !self.options.fast_place;
                self.options.save();
                return;
            }
            "correct-aim" => {
                self.options.correct_aim = !self.options.correct_aim;
                self.options.save();
                return;
            }
            "unload" => {
                if self.locked {
                    return;
                }
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
                if !self.locked
                    && let Some(p) = &mut self.placement
                {
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
        self.clear_verifier_marker();
        self.verifier_target = None;
        self.verifier_marker_changed = false;
        self.retired
            .extend(self.tiles.drain(..).filter(Tile::has_draws));
        self.cursor = 0;
        if let Some(p) = self.placement.filter(|_| self.shown && self.render_all) {
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
        self.clear_verifier_marker();
        self.retired
            .extend(self.tiles.drain(..).filter(Tile::has_draws));
        self.clear_retired();
        self.retired.is_empty()
    }

    fn show_label(&mut self, message: &str) {
        if self.render_all && message != self.last_label {
            label(message);
            self.last_label = message.to_owned();
        }
    }

    fn mismatch_count(&self) -> usize {
        self.tiles.iter().map(Tile::mismatch_count).sum()
    }

    fn mismatches(&self) -> impl Iterator<Item = &([i32; 3], schematica_core::CellStatus)> {
        self.tiles.iter().flat_map(|tile| &tile.mismatches)
    }

    fn clear_verifier_marker(&mut self) {
        if let Some(marker) = self.verifier_marker.take() {
            marker.remove();
        }
    }

    fn publish_label(&mut self, placement_cell: Option<[i32; 3]>) {
        if let Some(pos) = placement_cell
            && let Some(block) = self.planned_block(pos)
            && self.tiles.iter().any(|tile| tile.is_missing(pos))
        {
            let message = format!(
                "Fast Place | {} at {pos:?} | Right-click to place",
                block.name
            );
            self.show_label(&message);
            return;
        }
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
            self.options.menu_hint(&self.schematic.name)
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
                fast_place: false,
                correct_aim: false,
                opacity: 65,
                hotkeys: BTreeMap::new(),
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
