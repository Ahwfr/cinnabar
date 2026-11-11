use crate::*;

impl Viewer {
    pub(super) fn menu_action(&mut self, id: &str) {
        if let Some(row) = id
            .strip_prefix("delete-")
            .and_then(|value| value.parse().ok())
        {
            self.delete_entry(row);
            return;
        }
        if let Some(index) = id
            .strip_prefix("toggle-")
            .and_then(|value| value.parse::<usize>().ok())
        {
            let Some(index) = self.visible_entry(index) else {
                return;
            };
            if self.active_entry == Some(index) && self.loaded && self.placement.is_some() {
                self.shown = !self.shown;
                self.dirty = true;
            } else if index < self.library.len() {
                self.selected = Some(index);
                self.request = Some(index);
            }
            return;
        }
        if let Some(index) = id
            .strip_prefix("unload-")
            .and_then(|value| value.parse::<usize>().ok())
        {
            if self.active_entry == Some(index) && !self.locked {
                self.placement = None;
                self.active_entry = None;
                self.loaded = false;
                self.dirty = true;
            }
            return;
        }
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
            let Some(index) = self.visible_entry(index) else {
                return;
            };
            if index < self.library.len() {
                self.selected = Some(index);
                if self.active_entry != Some(index) {
                    self.request = Some(index);
                }
            }
            return;
        }
        let key = match id {
            "main" => {
                self.screen = Screen::Main;
                return;
            }
            "hotkeys" => {
                let _ = panel::open_settings("keyboard_and_mouse_forced_index");
                return;
            }
            "import" => {
                let _ = panel::open_settings("cinnabar_mods_index");
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
            "ui-layout" => {
                self.options.fullscreen_ui = !self.options.fullscreen_ui;
                self.options.save();
                menu::open(self, true);
                return;
            }
            "air-place" => {
                self.options.air_place = !self.options.air_place;
                self.options.save();
                if self.options.air_place
                    && !cinnabar::extension::permissions::read()
                        .iter()
                        .any(|permission| permission.name == "commands" && permission.granted)
                {
                    let _ = panel::open_settings("cinnabar_mods_index");
                }
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
}
