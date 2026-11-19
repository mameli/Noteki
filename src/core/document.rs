//! The state of one open Note: what is on screen, what is on disk, and how
//! external changes are reconciled (SPEC › Saving, External changes).

use crate::core::merge::merge_three_way;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteDoc {
    /// The text last loaded from or written to disk: the base of a Merge.
    base: String,
    /// The text in the editor.
    buffer: String,
    /// Disk text that could not be merged with the user's edits.
    conflict: Option<String>,
    /// The file is gone from disk; the text lives on screen only.
    deleted: bool,
    /// Must be written even if `buffer == base` (re-creating a deleted file).
    force_write: bool,
}

/// What an external change did to the document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalOutcome {
    /// Nothing to do: the disk has what we last loaded or saved.
    Unchanged,
    /// The disk now has exactly the editor text.
    Synced,
    /// Nothing was unsaved: the editor now shows the disk text.
    Reloaded,
    /// Unsaved edits were merged with the disk text; the result must be saved.
    Merged,
    /// Both sides changed the same lines: saving stops until the user decides.
    Conflict,
    /// The file was deleted by another app.
    Deleted,
}

impl NoteDoc {
    pub fn new(text: impl Into<String>) -> Self {
        let text = text.into();
        Self {
            base: text.clone(),
            buffer: text,
            conflict: None,
            deleted: false,
            force_write: false,
        }
    }

    pub fn text(&self) -> &str {
        &self.buffer
    }

    pub fn base(&self) -> &str {
        &self.base
    }

    pub fn is_dirty(&self) -> bool {
        self.buffer != self.base || self.force_write
    }

    pub fn has_conflict(&self) -> bool {
        self.conflict.is_some()
    }

    pub fn is_deleted(&self) -> bool {
        self.deleted
    }

    /// Text that exists only on screen: closing it would lose something.
    pub fn is_only_on_screen(&self) -> bool {
        self.has_conflict() || (self.deleted && !self.buffer.is_empty())
    }

    /// Whether autosave may write the buffer now.
    pub fn should_save(&self) -> bool {
        self.is_dirty() && self.conflict.is_none() && !self.deleted
    }

    /// The user typed: the editor now holds `text`. Returns whether it changed.
    pub fn edit(&mut self, text: &str) -> bool {
        if self.buffer == text {
            return false;
        }
        self.buffer = text.to_string();
        true
    }

    /// `text` was written to disk.
    pub fn mark_saved(&mut self, text: &str) {
        self.base = text.to_string();
        self.force_write = false;
        self.deleted = false;
    }

    /// The file at this note's path changed on disk; `disk` is its new text,
    /// or `None` if it no longer exists.
    pub fn on_disk_changed(&mut self, disk: Option<&str>) -> ExternalOutcome {
        let Some(disk) = disk else {
            if self.deleted {
                return ExternalOutcome::Unchanged;
            }
            self.deleted = true;
            return ExternalOutcome::Deleted;
        };
        let was_deleted = std::mem::take(&mut self.deleted);
        if was_deleted {
            // Re-created: whatever is there now is the base.
            self.force_write = false;
        }
        if disk == self.buffer {
            self.base = disk.to_string();
            self.conflict = None;
            self.force_write = false;
            return ExternalOutcome::Synced;
        }
        if disk == self.base && !was_deleted {
            // Our own write coming back, or a no-op touch. A pending conflict
            // whose other side went back to the base resolves to "mine".
            self.conflict = None;
            return ExternalOutcome::Unchanged;
        }
        if !self.is_dirty() && self.conflict.is_none() {
            self.base = disk.to_string();
            self.buffer = disk.to_string();
            return ExternalOutcome::Reloaded;
        }
        match merge_three_way(&self.base, &self.buffer, disk) {
            Some(merged) => {
                self.base = disk.to_string();
                self.buffer = merged;
                self.conflict = None;
                ExternalOutcome::Merged
            }
            None => {
                self.conflict = Some(disk.to_string());
                ExternalOutcome::Conflict
            }
        }
    }

    /// Conflict → *Keep mine*: the editor text will overwrite the disk.
    pub fn keep_mine(&mut self) {
        if let Some(disk) = self.conflict.take() {
            self.base = disk;
        }
    }

    /// Conflict → *Take theirs*: the editor shows the disk text.
    pub fn take_theirs(&mut self) -> bool {
        match self.conflict.take() {
            Some(disk) => {
                self.base = disk.clone();
                self.buffer = disk;
                true
            }
            None => false,
        }
    }

    /// Deleted → *Save again*: write the text back to its path.
    pub fn save_again(&mut self) {
        if self.deleted {
            self.deleted = false;
            self.force_write = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_doc_reloads_silently() {
        let mut d = NoteDoc::new("a\nb\n");
        assert_eq!(d.on_disk_changed(Some("a\nB\n")), ExternalOutcome::Reloaded);
        assert_eq!(d.text(), "a\nB\n");
        assert!(!d.is_dirty());
    }

    #[test]
    fn own_save_echo_is_ignored_even_after_more_typing() {
        let mut d = NoteDoc::new("v1");
        d.edit("v2");
        d.mark_saved("v2");
        d.edit("v3");
        assert_eq!(d.on_disk_changed(Some("v2")), ExternalOutcome::Unchanged);
        assert_eq!(d.text(), "v3");
        assert!(d.should_save());
    }

    #[test]
    fn unsaved_edits_merge_then_need_saving() {
        let mut d = NoteDoc::new("Title\nIntro\n\nEnd\n");
        d.edit("Title\nIntro edited\n\nEnd\n");
        assert_eq!(
            d.on_disk_changed(Some("Title\nIntro\n\nEnd by Takku\n")),
            ExternalOutcome::Merged
        );
        assert_eq!(d.text(), "Title\nIntro edited\n\nEnd by Takku\n");
        assert!(d.should_save());
    }

    #[test]
    fn edits_on_adjacent_lines_conflict_like_diff3() {
        // Same rule as git and NotekiCore's Merge.swift: no unchanged line between the edits.
        let mut d = NoteDoc::new("Title\nIntro\nEnd\n");
        d.edit("Title\nIntro edited\nEnd\n");
        assert_eq!(
            d.on_disk_changed(Some("Title\nIntro\nEnd by Takku\n")),
            ExternalOutcome::Conflict
        );
    }

    #[test]
    fn conflict_stops_saving_until_resolved() {
        let mut d = NoteDoc::new("A\nB\nC");
        d.edit("A\nmine\nC");
        assert_eq!(
            d.on_disk_changed(Some("A\ntheirs\nC")),
            ExternalOutcome::Conflict
        );
        assert!(!d.should_save());
        assert!(d.is_only_on_screen());
        assert_eq!(d.text(), "A\nmine\nC");

        let mut keep = d.clone();
        keep.keep_mine();
        assert!(keep.should_save());
        assert_eq!(keep.base(), "A\ntheirs\nC");

        assert!(d.take_theirs());
        assert_eq!(d.text(), "A\ntheirs\nC");
        assert!(!d.is_dirty());
    }

    #[test]
    fn keep_mine_when_texts_end_up_equal_writes_nothing() {
        let mut d = NoteDoc::new("x");
        d.edit("y");
        d.on_disk_changed(Some("z")); // conflict
        d.edit("z"); // user retypes the same as disk
        d.keep_mine();
        assert!(!d.has_conflict());
        assert!(!d.should_save(), "the disk already has this text");
    }

    #[test]
    fn deleted_note_is_not_recreated_by_autosave() {
        let mut d = NoteDoc::new("text");
        d.edit("text more");
        assert_eq!(d.on_disk_changed(None), ExternalOutcome::Deleted);
        assert!(!d.should_save());
        assert_eq!(d.on_disk_changed(None), ExternalOutcome::Unchanged);
        d.save_again();
        assert!(d.should_save());
        d.mark_saved("text more");
        assert!(!d.is_dirty());
    }

    #[test]
    fn save_again_writes_even_without_edits() {
        let mut d = NoteDoc::new("same");
        d.on_disk_changed(None);
        d.save_again();
        assert!(d.should_save());
    }

    #[test]
    fn recreated_file_is_handled_as_a_change() {
        let mut d = NoteDoc::new("old");
        d.on_disk_changed(None);
        assert_eq!(d.on_disk_changed(Some("new")), ExternalOutcome::Reloaded);
        assert!(!d.is_deleted());
        assert_eq!(d.text(), "new");
    }

    #[test]
    fn conflict_cleared_when_disk_matches_editor() {
        let mut d = NoteDoc::new("A");
        d.edit("B");
        d.on_disk_changed(Some("C"));
        assert_eq!(d.on_disk_changed(Some("B")), ExternalOutcome::Synced);
        assert!(!d.has_conflict());
        assert!(!d.is_dirty());
    }
}
