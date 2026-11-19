//! Moving the cursor and deleting by character, word, row and line, and
//! what follows an edit (the caret shown, the event to the app).

use super::*;

impl NoteEditor {
    pub(super) fn wake(&mut self, cx: &mut Context<Self>) {
        self.cursor_on = true;
        self.blinks_left = BLINKS_AFTER_INPUT;
        self.reveal = true;
        cx.notify();
    }

    /// Where ⌘← (`end` false) or ⌘→ goes: the start or end of the row the
    /// cursor is on, as wrapped; the logical line's when there is no layout yet.
    pub(super) fn row_target(&self, end: bool) -> usize {
        let fallback = || {
            self.buf.target(if end {
                Motion::LineEnd
            } else {
                Motion::LineStart
            })
        };
        self.layout
            .as_ref()
            .and_then(|l| l.row_range(self.buf.text(), self.buf.cursor()))
            .map(|r| if end { r.end } else { r.start })
            .unwrap_or_else(fallback)
    }

    pub(super) fn row_motion(&mut self, end: bool, extend: bool, cx: &mut Context<Self>) {
        let target = self.row_target(end);
        self.buf.move_to(target, extend);
        self.goal_x = None;
        self.wake(cx);
    }

    pub(super) fn delete_row(&mut self, end: bool, cx: &mut Context<Self>) {
        let target = self.row_target(end);
        let changed = self.buf.delete_to(target);
        self.edited(changed, cx);
    }

    pub(super) fn edited(&mut self, changed: bool, cx: &mut Context<Self>) {
        self.goal_x = None;
        // The `[[` list starts at its first Note again as the name changes.
        self.completion_pick = 0;
        self.wake(cx);
        if changed {
            self.revision += 1;
            self.typed_at = Some((self.revision, self.buf.cursor()));
            cx.emit(NoteEditorEvent::Changed);
        }
    }

    pub(super) fn motion(&mut self, motion: Motion, extend: bool, cx: &mut Context<Self>) {
        self.buf.apply_motion(motion, extend);
        self.goal_x = None;
        self.wake(cx);
    }

    pub(super) fn delete(&mut self, motion: Motion, cx: &mut Context<Self>) {
        let changed = self.buf.delete(motion);
        self.edited(changed, cx);
    }

    /// ↑ (`up`) or ↓: to the row above or below, at the x the movement began
    /// with, across lines of any height.
    pub(super) fn vertical(&mut self, up: bool, extend: bool, cx: &mut Context<Self>) {
        if !extend && !self.buf.selection().is_empty() {
            let r = self.buf.selection().range();
            self.buf.move_to(if up { r.start } else { r.end }, false);
        }
        let Some(layout) = &self.layout else {
            return;
        };
        let head = self.buf.cursor();
        let Some(caret) = layout.caret(self.buf.text(), head) else {
            return;
        };
        let x = *self.goal_x.get_or_insert(caret.origin.x);
        let text = self.buf.text();
        // In a table's row: out of the row once the cell's own rows end.
        let (above, below) = match layout.past_cell(head, up) {
            Some(y) if up => (y + px(1.), caret.bottom()),
            Some(y) => (caret.top(), y - px(1.)),
            None => (caret.top(), caret.bottom()),
        };
        let mut target = if up {
            if above <= layout.text_top() + px(0.5) {
                0
            } else {
                layout.offset_for_point(text, point(x, above - px(1.)))
            }
        } else if below >= layout.text_bottom() - px(0.5) {
            text.len()
        } else {
            layout.offset_for_point(text, point(x, below + px(1.)))
        };
        // Over a table's rule to the row past it, unless already on the rule.
        if layout.past_rule(head, up).is_none() {
            if let Some(y) = layout.past_rule(target, up) {
                target = layout.offset_for_point(text, point(x, y));
            }
        }
        self.buf.move_to(target, extend);
        self.wake(cx);
    }
}
