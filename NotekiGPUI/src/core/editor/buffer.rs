//! The text of the editor with its selection and the text being composed by an
//! input method. Every offset is a byte offset on a char boundary; the methods
//! clamp what they are given, so a bad offset can never split a character.

use std::ops::Range;

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
    /// Text an input method is composing (dead key, press-and-hold, IME).
    marked: Option<Range<usize>>,
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

    /// Replaces the whole text, as when a Note is opened or reloaded.
    pub fn set_text(&mut self, text: impl Into<String>, cursor: usize) {
        self.text = text.into();
        self.marked = None;
        self.selection = Selection::caret(self.clamp(cursor));
    }

    fn clamp(&self, offset: usize) -> usize {
        floor_boundary(&self.text, offset)
    }

    fn clamp_range(&self, r: Range<usize>) -> Range<usize> {
        let (a, b) = (self.clamp(r.start), self.clamp(r.end));
        a.min(b)..a.max(b)
    }

    pub fn set_selection(&mut self, anchor: usize, head: usize) {
        self.selection = Selection {
            anchor: self.clamp(anchor),
            head: self.clamp(head),
        };
    }

    pub fn select_all(&mut self) {
        self.selection = Selection {
            anchor: 0,
            head: self.text.len(),
        };
    }

    /// Puts the cursor at `offset`; with `extend`, keeps the anchor (⇧).
    pub fn move_to(&mut self, offset: usize, extend: bool) {
        let head = self.clamp(offset);
        let anchor = if extend { self.selection.anchor } else { head };
        self.selection = Selection { anchor, head };
    }

    /// Where `motion` takes the cursor from where it is now.
    pub fn target(&self, motion: Motion) -> usize {
        let (t, h) = (&self.text[..], self.selection.head);
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
        let r = self.clamp_range(range);
        let changed = &self.text[r.clone()] != new;
        self.text.replace_range(r.clone(), new);
        self.selection = Selection::caret(r.start + new.len());
        self.marked = None;
        changed
    }

    /// Typing: replaces what is being composed, else the selection.
    pub fn insert(&mut self, new: &str) -> bool {
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
        b.set_text("aè", 2);
        assert_eq!(b.cursor(), 1);
        b.set_text("ab", 99);
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
}
