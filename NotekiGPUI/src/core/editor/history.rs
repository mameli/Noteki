//! Undo and redo: the edits of the text as steps, each the replacements it
//! made and the selection before and after. Typing is one step until the
//! cursor moves or something else is done, as in macOS text views; so is a
//! run of ⌫, and an accent composed with a dead key.

use std::ops::Range;

use super::buffer::Selection;

/// At most this many steps are kept; the oldest go first.
const MAX_STEPS: usize = 1000;

/// One replacement: at `at`, `removed` was replaced by `inserted`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edit {
    pub at: usize,
    pub removed: String,
    pub inserted: String,
}

#[derive(Debug, Clone)]
struct Step {
    edits: Vec<Edit>,
    before: Selection,
    after: Selection,
}

/// What the last step was doing, while it may still grow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Run {
    Typing,
    Deleting,
    Composing,
}

#[derive(Debug, Clone, Default)]
pub struct History {
    undo: Vec<Step>,
    redo: Vec<Step>,
    open: Option<Run>,
}

impl History {
    /// Records `edit`, done with the selection going from `before` to
    /// `after`. `composing`: an input method's composed text was there.
    pub fn record(&mut self, edit: Edit, before: Selection, after: Selection, composing: bool) {
        self.redo.clear();
        let one_char = |s: &str| s.chars().count() == 1 && !s.contains('\n');
        let run = if composing {
            Some(Run::Composing)
        } else if edit.removed.is_empty() && one_char(&edit.inserted) {
            Some(Run::Typing)
        } else if edit.inserted.is_empty() && one_char(&edit.removed) {
            Some(Run::Deleting)
        } else {
            None
        };
        // A run goes on while the same kind of edit follows with the cursor
        // never moved in between (moving it closes the run, see `close`).
        let continues = self.open.is_some() && self.open == run && !self.undo.is_empty();
        match self.undo.last_mut() {
            Some(step) if continues => {
                step.edits.push(edit);
                step.after = after;
            }
            _ => {
                self.undo.push(Step {
                    edits: vec![edit],
                    before,
                    after,
                });
                if self.undo.len() > MAX_STEPS {
                    self.undo.remove(0);
                }
            }
        }
        self.open = run;
    }

    /// The next edit starts a step of its own (the cursor moved, the text was
    /// replaced).
    pub fn close(&mut self) {
        self.open = None;
    }

    /// Undoes the last step on `text`; the selection to restore.
    pub fn undo(&mut self, text: &mut String) -> Option<Selection> {
        let step = self.undo.pop()?;
        for e in step.edits.iter().rev() {
            text.replace_range(e.at..e.at + e.inserted.len(), &e.removed);
        }
        self.open = None;
        let before = step.before;
        self.redo.push(step);
        Some(before)
    }

    /// Redoes the last undone step on `text`; the selection to restore.
    pub fn redo(&mut self, text: &mut String) -> Option<Selection> {
        let step = self.redo.pop()?;
        for e in &step.edits {
            text.replace_range(e.at..e.at + e.removed.len(), &e.inserted);
        }
        self.open = None;
        let after = step.after;
        self.undo.push(step);
        Some(after)
    }
}

/// The one replacement that turns `old` into `new`: what differs between
/// their common start and common end.
pub fn diff(old: &str, new: &str) -> Option<Edit> {
    if old == new {
        return None;
    }
    let mut start = old
        .bytes()
        .zip(new.bytes())
        .take_while(|(a, b)| a == b)
        .count();
    while !old.is_char_boundary(start) || !new.is_char_boundary(start) {
        start -= 1;
    }
    let max_end = old.len().min(new.len()) - start;
    let mut end = old
        .bytes()
        .rev()
        .zip(new.bytes().rev())
        .take(max_end)
        .take_while(|(a, b)| a == b)
        .count();
    while !old.is_char_boundary(old.len() - end) || !new.is_char_boundary(new.len() - end) {
        end -= 1;
    }
    let removed: Range<usize> = start..old.len() - end;
    Some(Edit {
        at: start,
        removed: old[removed].to_string(),
        inserted: new[start..new.len() - end].to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn caret(o: usize) -> Selection {
        Selection::caret(o)
    }

    fn apply(h: &mut History, text: &mut String, at: usize, removed: usize, inserted: &str) {
        let e = Edit {
            at,
            removed: text[at..at + removed].to_string(),
            inserted: inserted.to_string(),
        };
        text.replace_range(at..at + removed, inserted);
        h.record(e, caret(at + removed), caret(at + inserted.len()), false);
    }

    #[test]
    fn typing_is_one_step_until_the_cursor_moves() {
        let (mut h, mut t) = (History::default(), String::from("x"));
        for (i, c) in ["a", "b", "c"].iter().enumerate() {
            apply(&mut h, &mut t, 1 + i, 0, c);
        }
        h.close();
        apply(&mut h, &mut t, 0, 0, "Z");
        assert_eq!(t, "Zxabc");
        assert_eq!(h.undo(&mut t), Some(caret(0)));
        assert_eq!(t, "xabc");
        assert_eq!(h.undo(&mut t), Some(caret(1)));
        assert_eq!(t, "x");
        assert_eq!(h.undo(&mut t), None);
        assert_eq!(h.redo(&mut t), Some(caret(4)));
        assert_eq!(t, "xabc");
        assert_eq!(h.redo(&mut t), Some(caret(1)));
        assert_eq!(t, "Zxabc");
    }

    #[test]
    fn a_run_of_backspaces_is_one_step_and_a_paste_its_own() {
        let (mut h, mut t) = (History::default(), String::from("abcd"));
        apply(&mut h, &mut t, 3, 1, "");
        apply(&mut h, &mut t, 2, 1, "");
        apply(&mut h, &mut t, 2, 0, "PASTED");
        assert_eq!(t, "abPASTED");
        h.undo(&mut t);
        assert_eq!(t, "ab");
        h.undo(&mut t);
        assert_eq!(t, "abcd");
    }

    #[test]
    fn a_dead_key_and_its_letter_are_one_step() {
        let (mut h, mut t) = (History::default(), String::from("caff"));
        // `´` composed, then replaced by `é`.
        t.push('´');
        h.record(
            Edit {
                at: 4,
                removed: String::new(),
                inserted: "´".into(),
            },
            caret(4),
            caret(6),
            true,
        );
        t.replace_range(4..6, "é");
        h.record(
            Edit {
                at: 4,
                removed: "´".into(),
                inserted: "é".into(),
            },
            caret(6),
            caret(6),
            true,
        );
        assert_eq!(t, "caffé");
        h.undo(&mut t);
        assert_eq!(t, "caff");
    }

    #[test]
    fn a_new_edit_after_an_undo_drops_the_redo() {
        let (mut h, mut t) = (History::default(), String::new());
        apply(&mut h, &mut t, 0, 0, "a");
        h.undo(&mut t);
        apply(&mut h, &mut t, 0, 0, "b");
        assert_eq!(h.redo(&mut t), None);
        assert_eq!(t, "b");
    }

    #[test]
    fn diff_finds_the_one_replacement() {
        assert_eq!(diff("abc", "abc"), None);
        let e = diff("uno due tre", "uno DUE tre").unwrap();
        assert_eq!(
            (e.at, e.removed.as_str(), e.inserted.as_str()),
            (4, "due", "DUE")
        );
        let e = diff("caffè", "caffé").unwrap();
        assert_eq!(
            (e.at, e.removed.as_str(), e.inserted.as_str()),
            (4, "è", "é")
        );
        let e = diff("aaa", "aaaa").unwrap();
        assert_eq!((e.removed.as_str(), e.inserted.as_str()), ("", "a"));
    }
}
