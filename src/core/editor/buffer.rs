//! The text of the editor with its selection and the text being composed by an
//! input method. Every offset is a byte offset on a char boundary; the methods
//! clamp what they are given, so a bad offset can never split a character.

use std::ops::Range;

use super::history::{diff, Edit, History};
use super::movement::{self, floor_boundary};

/// A selection is where it was started (`anchor`) and where it is now (`head`,
/// the cursor). Empty when both are equal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Selection {
    pub anchor: usize,
    pub head: usize,
}

impl Selection {
    pub fn caret(offset: usize) -> Self {
        Self {
            anchor: offset,
            head: offset,
        }
    }

    pub fn range(&self) -> Range<usize> {
        self.anchor.min(self.head)..self.anchor.max(self.head)
    }

    pub fn is_empty(&self) -> bool {
        self.anchor == self.head
    }

    /// The head is before the anchor (the selection was made leftwards).
    pub fn reversed(&self) -> bool {
        self.head < self.anchor
    }
}

/// How a key moves the cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Motion {
    Left,
    Right,
    WordLeft,
    WordRight,
    LineStart,
    LineEnd,
    DocStart,
    DocEnd,
}

#[derive(Debug, Clone, Default)]
pub struct EditBuffer {
    text: String,
    selection: Selection,
    history: History,
    /// Text an input method is composing (dead key, press-and-hold, IME).
    marked: Option<Range<usize>>,
    /// More selections than the main one, from ⌘D: an edit acts on all.
    extra: Vec<Selection>,
}

impl EditBuffer {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            ..Default::default()
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn selection(&self) -> Selection {
        self.selection
    }

    pub fn cursor(&self) -> usize {
        self.selection.head
    }

    pub fn marked(&self) -> Option<Range<usize>> {
        self.marked.clone()
    }

    pub fn selected_text(&self) -> &str {
        &self.text[self.selection.range()]
    }

    /// Replaces the whole text and its history, as when a Note is opened.
    pub fn set_text(&mut self, text: impl Into<String>, cursor: usize, history: History) {
        self.text = text.into();
        self.marked = None;
        self.selection = Selection::caret(self.clamp(cursor));
        self.history = history;
    }

    /// Replaces the whole text with `text` as one undoable step (a reload or
    /// a Merge of another app's change), the cursor at `cursor`.
    pub fn replace_all(&mut self, text: &str, cursor: usize) {
        if let Some(edit) = diff(&self.text, text) {
            let before = self.selection;
            self.text
                .replace_range(edit.at..edit.at + edit.removed.len(), &edit.inserted);
            self.marked = None;
            self.selection = Selection::caret(self.clamp(cursor));
            self.history.close();
            self.history.record(edit, before, self.selection, false);
            self.history.close();
        } else {
            self.selection = Selection::caret(self.clamp(cursor));
        }
    }

    /// The history, taken away (left empty), to keep while another Note shows.
    pub fn take_history(&mut self) -> History {
        std::mem::take(&mut self.history)
    }

    /// ⌘Z: whether there was something to undo.
    pub fn undo(&mut self) -> bool {
        self.marked = None;
        match self.history.undo(&mut self.text) {
            Some(sel) => {
                self.set_selection(sel.anchor, sel.head);
                true
            }
            None => false,
        }
    }

    /// ⇧⌘Z: whether there was something to redo.
    pub fn redo(&mut self) -> bool {
        self.marked = None;
        match self.history.redo(&mut self.text) {
            Some(sel) => {
                self.set_selection(sel.anchor, sel.head);
                true
            }
            None => false,
        }
    }

    /// The next edit is a step of its own (a click on a checkbox, a paste).
    pub fn close_undo_step(&mut self) {
        self.history.close();
    }

    fn clamp(&self, offset: usize) -> usize {
        floor_boundary(&self.text, offset)
    }

    fn clamp_range(&self, r: Range<usize>) -> Range<usize> {
        let (a, b) = (self.clamp(r.start), self.clamp(r.end));
        a.min(b)..a.max(b)
    }

    pub fn set_selection(&mut self, anchor: usize, head: usize) {
        self.history.close();
        self.extra.clear();
        self.selection = Selection {
            anchor: self.clamp(anchor),
            head: self.clamp(head),
        };
    }

    pub fn select_all(&mut self) {
        self.history.close();
        self.extra.clear();
        self.selection = Selection {
            anchor: 0,
            head: self.text.len(),
        };
    }

    /// Puts the cursor at `offset`; with `extend`, keeps the anchor (⇧).
    pub fn move_to(&mut self, offset: usize, extend: bool) {
        self.history.close(); // the cursor moved: what follows is a new undo step
        self.extra.clear();
        let head = self.clamp(offset);
        let anchor = if extend { self.selection.anchor } else { head };
        self.selection = Selection { anchor, head };
    }

    /// Where `motion` takes the cursor from where it is now.
    pub fn target(&self, motion: Motion) -> usize {
        self.target_from(motion, self.selection.head)
    }

    fn target_from(&self, motion: Motion, h: usize) -> usize {
        let t = &self.text[..];
        match motion {
            Motion::Left => movement::prev_grapheme(t, h),
            Motion::Right => movement::next_grapheme(t, h),
            Motion::WordLeft => movement::prev_word_start(t, h),
            Motion::WordRight => movement::next_word_end(t, h),
            Motion::LineStart => movement::line_start(t, h),
            Motion::LineEnd => movement::line_end(t, h),
            Motion::DocStart => 0,
            Motion::DocEnd => t.len(),
        }
    }

    /// Moves the cursor as a key does. Without ⇧, ← and → on a selection
    /// collapse it to its start or end rather than moving from the head.
    pub fn apply_motion(&mut self, motion: Motion, extend: bool) {
        if !extend && !self.selection.is_empty() {
            let r = self.selection.range();
            match motion {
                Motion::Left => return self.move_to(r.start, false),
                Motion::Right => return self.move_to(r.end, false),
                _ => {}
            }
        }
        self.move_to(self.target(motion), extend);
    }

    /// Replaces `range` with `new`, leaving the cursor after it. Returns whether
    /// the text changed.
    pub fn replace(&mut self, range: Range<usize>, new: &str) -> bool {
        self.extra.clear();
        let r = self.clamp_range(range);
        let changed = &self.text[r.clone()] != new;
        let before = self.selection;
        let removed = self.text[r.clone()].to_string();
        self.text.replace_range(r.clone(), new);
        self.selection = Selection::caret(r.start + new.len());
        if changed {
            let edit = Edit {
                at: r.start,
                removed,
                inserted: new.to_string(),
            };
            self.history
                .record(edit, before, self.selection, self.marked.is_some());
        }
        self.marked = None;
        changed
    }

    /// Every selection, the main one first.
    pub fn selections(&self) -> Vec<Selection> {
        std::iter::once(self.selection)
            .chain(self.extra.iter().copied())
            .collect()
    }

    /// Selections at `ranges`, the main one the `main`-th.
    pub fn set_selections(&mut self, ranges: &[Range<usize>], main: usize) {
        let Some(m) = ranges.get(main) else {
            return;
        };
        self.set_selection(m.start, m.end);
        self.extra = ranges
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != main)
            .map(|(_, r)| Selection {
                anchor: self.clamp(r.start),
                head: self.clamp(r.end),
            })
            .collect();
    }

    /// ⌘D: with nothing selected, selects the word at the cursor; then adds
    /// the next occurrence of the selected text as the main selection,
    /// wrapping at the end. Whether a selection was made or added.
    pub fn select_next_occurrence(&mut self) -> bool {
        if self.selection.is_empty() {
            let w = movement::word_range_at(&self.text, self.selection.head);
            if w.is_empty() {
                return false;
            }
            self.set_selection(w.start, w.end);
            return true;
        }
        let needle = self.selected_text().to_string();
        let all = self.selections();
        let taken = |at: usize| all.iter().any(|s| s.range().start == at);
        let after = all.iter().map(|s| s.range().end).max().unwrap_or(0);
        let found = self.text[after..]
            .match_indices(&needle)
            .map(|(i, _)| after + i)
            .chain(self.text[..after].match_indices(&needle).map(|(i, _)| i))
            .find(|&at| !taken(at));
        let Some(at) = found else {
            return false;
        };
        let mut extra = std::mem::take(&mut self.extra);
        extra.push(self.selection);
        self.set_selection(at, at + needle.len());
        self.extra = extra;
        true
    }

    /// Applies an edit to every selection at once, as one replacement (one
    /// undo step): `edit` gives, for a selection, what to replace and with
    /// what. The selections end as carets after their new text.
    fn edit_all(&mut self, edit: impl Fn(&Self, Selection) -> (Range<usize>, String)) -> bool {
        let main_at = self.selection.range().start;
        let mut edits: Vec<(Range<usize>, String)> = self
            .selections()
            .into_iter()
            .map(|s| edit(self, s))
            .collect();
        edits.sort_by_key(|(r, _)| r.start);
        let (start, end) = (
            edits[0].0.start,
            edits.iter().map(|(r, _)| r.end).max().unwrap_or(0),
        );
        let mut middle = String::new();
        let mut carets = Vec::new();
        let mut at = start;
        for (r, new) in &edits {
            let r = r.start.max(at)..r.end.max(at);
            middle.push_str(&self.text[at..r.start]);
            middle.push_str(new);
            carets.push((r.start, start + middle.len()));
            at = r.end;
        }
        middle.push_str(&self.text[at..end]);
        let changed = self.replace(start..end, &middle);
        let ranges: Vec<Range<usize>> = carets.iter().map(|&(_, c)| c..c).collect();
        let main = carets.iter().position(|&(s, _)| s == main_at).unwrap_or(0);
        self.set_selections(&ranges, main);
        changed
    }

    /// Typing: replaces what is being composed, else the selection.
    pub fn insert(&mut self, new: &str) -> bool {
        if !self.extra.is_empty() && self.marked.is_none() {
            return self.edit_all(|_, s| (s.range(), new.to_string()));
        }
        let r = self
            .marked
            .clone()
            .unwrap_or_else(|| self.selection.range());
        self.replace(r, new)
    }

    /// An input method puts `new` in `range` (or in place of the composed text,
    /// or the selection) and keeps it marked. `selected` is relative to `new`.
    pub fn replace_and_mark(
        &mut self,
        range: Option<Range<usize>>,
        new: &str,
        selected: Option<Range<usize>>,
    ) -> bool {
        let r = range
            .map(|r| self.clamp_range(r))
            .or_else(|| self.marked.clone())
            .unwrap_or_else(|| self.selection.range());
        // Composing from its first key: the whole composition is one step.
        self.marked.get_or_insert(r.start..r.start);
        let changed = self.replace(r.clone(), new);
        self.marked = (!new.is_empty()).then(|| r.start..r.start + new.len());
        if let Some(s) = selected {
            let a = r.start + floor_boundary(new, s.start);
            let b = r.start + floor_boundary(new, s.end);
            self.selection = Selection { anchor: a, head: b };
        }
        changed
    }

    pub fn unmark(&mut self) {
        self.marked = None;
    }

    /// ⌫ and ⌦: deletes the selection, or from the cursor to where `motion`
    /// goes (`Left` for ⌫, `Right` for ⌦, `WordLeft` for ⌥⌫...).
    pub fn delete(&mut self, motion: Motion) -> bool {
        if !self.extra.is_empty() {
            return self.edit_all(|b, s| {
                let r = if s.is_empty() {
                    let t = b.target_from(motion, s.head);
                    s.head.min(t)..s.head.max(t)
                } else {
                    s.range()
                };
                (r, String::new())
            });
        }
        self.delete_to(self.target(motion))
    }

    /// Deletes the selection, or from the cursor to `target`.
    pub fn delete_to(&mut self, target: usize) -> bool {
        let r = if self.selection.is_empty() {
            let (h, t) = (self.selection.head, self.clamp(target));
            h.min(t)..h.max(t)
        } else {
            self.selection.range()
        };
        if r.is_empty() {
            return false;
        }
        self.replace(r, "")
    }
}

// ---- UTF-16, as the macOS text input system counts ----

pub fn offset_to_utf16(text: &str, offset: usize) -> usize {
    text[..floor_boundary(text, offset)].encode_utf16().count()
}

/// The byte offset of a UTF-16 position; one inside a surrogate pair or past
/// the end is clamped.
pub fn offset_from_utf16(text: &str, utf16: usize) -> usize {
    let mut count = 0;
    for (i, c) in text.char_indices() {
        if count >= utf16 {
            return i;
        }
        count += c.len_utf16();
        if count > utf16 {
            return i;
        }
    }
    text.len()
}

pub fn range_to_utf16(text: &str, r: &Range<usize>) -> Range<usize> {
    offset_to_utf16(text, r.start)..offset_to_utf16(text, r.end)
}

pub fn range_from_utf16(text: &str, r: &Range<usize>) -> Range<usize> {
    offset_from_utf16(text, r.start)..offset_from_utf16(text, r.end)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buf(text: &str, cursor: usize) -> EditBuffer {
        let mut b = EditBuffer::new(text);
        b.move_to(cursor, false);
        b
    }

    #[test]
    fn typing_inserts_at_the_cursor_and_replaces_the_selection() {
        let mut b = buf("Ciao", 4);
        assert!(b.insert(" mondo"));
        assert_eq!((b.text(), b.cursor()), ("Ciao mondo", 10));
        b.set_selection(0, 4);
        b.insert("Salve");
        assert_eq!((b.text(), b.cursor()), ("Salve mondo", 5));
    }

    #[test]
    fn backspace_and_delete_remove_a_grapheme_or_the_selection() {
        let mut b = buf("perché", "perché".len());
        assert!(b.delete(Motion::Left));
        assert_eq!(b.text(), "perch");
        let mut b = buf("ab", 0);
        assert!(!b.delete(Motion::Left));
        assert!(b.delete(Motion::Right));
        assert_eq!((b.text(), b.cursor()), ("b", 0));
        let mut b = buf("uno due tre", 0);
        b.set_selection(8, 4);
        b.delete(Motion::Left);
        assert_eq!((b.text(), b.cursor()), ("uno tre", 4));
    }

    #[test]
    fn option_backspace_deletes_a_word_and_command_backspace_the_line() {
        let mut b = buf("riga uno\nriga due", 17);
        b.delete(Motion::WordLeft);
        assert_eq!(b.text(), "riga uno\nriga ");
        b.delete(Motion::LineStart);
        assert_eq!((b.text(), b.cursor()), ("riga uno\n", 9));
    }

    #[test]
    fn arrows_collapse_a_selection_and_shift_extends_it() {
        let mut b = buf("abcdef", 2);
        b.apply_motion(Motion::Right, true);
        b.apply_motion(Motion::Right, true);
        assert_eq!(b.selection().range(), 2..4);
        b.apply_motion(Motion::Left, false);
        assert_eq!(b.selection(), Selection::caret(2));
        b.apply_motion(Motion::LineEnd, true);
        assert_eq!(b.selected_text(), "cdef");
        b.apply_motion(Motion::DocStart, true);
        assert!(b.selection().reversed());
        assert_eq!(b.selected_text(), "ab");
    }

    #[test]
    fn a_dead_key_composes_then_commits() {
        // ´ then e on an Italian keyboard: ´ is marked, then replaced by é.
        let mut b = buf("caff", 4);
        b.replace_and_mark(None, "´", None);
        assert_eq!((b.text(), b.marked()), ("caff´", Some(4..6)));
        b.insert("é");
        assert_eq!((b.text(), b.cursor(), b.marked()), ("caffé", 6, None));
    }

    #[test]
    fn composing_with_a_selection_inside_the_marked_text() {
        let mut b = buf("x", 1);
        b.replace_and_mark(None, "にほ", Some(3..3));
        assert_eq!(b.marked(), Some(1..7));
        assert_eq!(b.cursor(), 4);
        b.replace_and_mark(None, "", None);
        assert_eq!((b.text(), b.marked()), ("x", None));
    }

    #[test]
    fn set_text_clamps_the_cursor_to_a_char_boundary() {
        let mut b = EditBuffer::new("");
        b.set_text("aè", 2, History::default());
        assert_eq!(b.cursor(), 1);
        b.set_text("ab", 99, History::default());
        assert_eq!(b.cursor(), 2);
    }

    #[test]
    fn utf16_round_trips_and_clamps_inside_surrogates() {
        let t = "a😀è";
        assert_eq!(offset_to_utf16(t, 1), 1);
        assert_eq!(offset_to_utf16(t, 5), 3);
        assert_eq!(offset_to_utf16(t, t.len()), 4);
        assert_eq!(offset_from_utf16(t, 3), 5);
        assert_eq!(offset_from_utf16(t, 2), 1); // inside the surrogate pair
        assert_eq!(offset_from_utf16(t, 99), t.len());
        assert_eq!(range_from_utf16(t, &(1..3)), 1..5);
    }

    #[test]
    fn cmd_d_selects_the_word_then_its_next_occurrences() {
        let mut b = buf("uno due uno tre uno", 1);
        assert!(b.select_next_occurrence());
        assert_eq!(b.selection().range(), 0..3);
        assert!(b.select_next_occurrence());
        assert!(b.select_next_occurrence());
        let mut all: Vec<_> = b.selections().iter().map(|s| s.range()).collect();
        all.sort_by_key(|r| r.start);
        assert_eq!(all, vec![0..3, 8..11, 16..19]);
        assert!(!b.select_next_occurrence(), "none left");
    }

    #[test]
    fn typing_and_deleting_act_on_every_selection_as_one_undo_step() {
        let mut b = buf("uno due uno tre uno", 1);
        for _ in 0..3 {
            b.select_next_occurrence();
        }
        b.insert("1");
        assert_eq!(b.text(), "1 due 1 tre 1");
        b.insert("!");
        assert_eq!(b.text(), "1! due 1! tre 1!");
        b.delete(Motion::Left);
        assert_eq!(b.text(), "1 due 1 tre 1");
        assert_eq!(b.selections().len(), 3);
        b.undo();
        assert_eq!(b.text(), "1! due 1! tre 1!");
        b.move_to(0, false);
        assert_eq!(b.selections().len(), 1, "moving keeps one selection");
    }
}
