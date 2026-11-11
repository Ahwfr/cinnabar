//! Persistent library removals preserve input files and recover on a fresh import.

use crate::{Options, Viewer};

impl Options {
    pub(super) fn select_library(&mut self, revision: u64) {
        if self.library_revision != revision {
            self.deleted_entries.clear();
            self.library_revision = revision;
        }
    }
}

impl Viewer {
    pub(super) fn visible_entry(&self, row: usize) -> Option<usize> {
        (0..self.library.len())
            .filter(|index| !self.options.deleted_entries.contains(index))
            .nth(row)
    }

    pub(super) fn delete_entry(&mut self, row: usize) {
        let Some(index) = self.visible_entry(row) else {
            return;
        };
        self.options.deleted_entries.push(index);
        self.options.save();
        if self.selected == Some(index) {
            self.selected = None;
        }
        if self.request == Some(index) {
            self.request = None;
        }
        if self.active_entry == Some(index) {
            self.placement = None;
            self.active_entry = None;
            self.loaded = false;
            self.auto_place = false;
            self.materials.clear();
            self.dirty = true;
        }
    }
}
