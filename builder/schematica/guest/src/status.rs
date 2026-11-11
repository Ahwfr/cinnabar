use crate::*;

impl Viewer {
    pub(super) fn show_label(&mut self, message: &str) {
        if self.render_all && message != self.last_label {
            label(message);
            self.last_label = message.to_owned();
        }
    }

    pub(super) fn mismatch_count(&self) -> usize {
        self.tiles.iter().map(Tile::mismatch_count).sum()
    }

    pub(super) fn mismatches(
        &self,
    ) -> impl Iterator<Item = &([i32; 3], schematica_core::CellStatus)> {
        self.tiles.iter().flat_map(|tile| &tile.mismatches)
    }

    pub(super) fn clear_verifier_marker(&mut self) {
        if let Some(marker) = self.verifier_marker.take() {
            marker.remove();
        }
    }

    pub(super) fn publish_label(&mut self, placement_cell: Option<[i32; 3]>) {
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

impl Options {
    pub(super) fn menu_hint(&self, title: &str) -> String {
        let menu = native_bindings::caption(hotkeys::Action::Main);
        let settings = native_bindings::caption(hotkeys::Action::Config);
        format!("{title} | {menu} Menu | {settings} Settings")
    }
}
