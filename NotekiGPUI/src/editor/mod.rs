//! Noteki's editor element: draws the text of a Note line by line, each line
//! shaped and measured on its own so that lines may have different heights,
//! and takes keyboard, mouse and native text input (accents, dead keys,
//! input methods, dictation) through GPUI's `EntityInputHandler`.
//!
//! The rules (edits, selection, movement, line geometry) are in
//! `crate::core::editor` and tested there; this module only measures, draws
//! and dispatches.

mod element;
mod style;

use std::cell::RefCell;
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use gpui_kit::{
    actions, div, point, prelude::*, px, App, Bounds, ClipboardItem, Context, CursorStyle,
    EntityInputHandler, EventEmitter, FocusHandle, Focusable, KeyBinding, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, ScrollHandle, SharedString, Task,
    UTF16Selection, Window,
};

use crate::core::editor::buffer::{range_from_utf16, range_to_utf16};
use crate::core::editor::styling::{style_lines, LineStyle};
use crate::core::editor::{EditBuffer, Motion};
use crate::core::markdown::{compute_markdown_spans, line_of_offset};
use crate::core::wikilink::VaultIndex;
use crate::theme::NotekiPalette;

#[doc(hidden)]
pub use element::shape_for_bench;
use element::{EditorElement, Layout, ShapeCache};

const CONTEXT: &str = "NoteEditor";
const BLINK: Duration = Duration::from_millis(500);
/// Above this size Live preview is off: the Note is drawn as plain text.
pub const LIVE_PREVIEW_MAX_BYTES: usize = 1_000_000;

actions!(
    note_editor,
    [
        Backspace,
        Delete,
        DeleteWordLeft,
        DeleteWordRight,
        DeleteToLineStart,
        DeleteToLineEnd,
        DeleteToParagraphEnd,
        Newline,
        Left,
        Right,
        Up,
        Down,
        WordLeft,
        WordRight,
        LineStart,
        LineEnd,
        ParagraphStart,
        ParagraphEnd,
        DocStart,
        DocEnd,
        SelectLeft,
        SelectRight,
        SelectUp,
        SelectDown,
        SelectWordLeft,
        SelectWordRight,
        SelectLineStart,
        SelectLineEnd,
        SelectDocStart,
        SelectDocEnd,
        SelectAll,
        Copy,
        Cut,
        Paste,
        ShowCharacterPalette,
    ]
);

pub fn register_keybindings(cx: &mut App) {
    let c = Some(CONTEXT);
    cx.bind_keys([
        KeyBinding::new("backspace", Backspace, c),
        KeyBinding::new("shift-backspace", Backspace, c),
        KeyBinding::new("delete", Delete, c),
        KeyBinding::new("ctrl-d", Delete, c),
        KeyBinding::new("alt-backspace", DeleteWordLeft, c),
        KeyBinding::new("alt-delete", DeleteWordRight, c),
        KeyBinding::new("cmd-backspace", DeleteToLineStart, c),
        KeyBinding::new("cmd-delete", DeleteToLineEnd, c),
        KeyBinding::new("ctrl-k", DeleteToParagraphEnd, c),
        KeyBinding::new("enter", Newline, c),
        KeyBinding::new("shift-enter", Newline, c),
        KeyBinding::new("left", Left, c),
        KeyBinding::new("right", Right, c),
        KeyBinding::new("up", Up, c),
        KeyBinding::new("down", Down, c),
        KeyBinding::new("ctrl-b", Left, c),
        KeyBinding::new("ctrl-f", Right, c),
        KeyBinding::new("ctrl-p", Up, c),
        KeyBinding::new("ctrl-n", Down, c),
        KeyBinding::new("alt-left", WordLeft, c),
        KeyBinding::new("alt-right", WordRight, c),
        KeyBinding::new("cmd-left", LineStart, c),
        KeyBinding::new("cmd-right", LineEnd, c),
        KeyBinding::new("ctrl-a", ParagraphStart, c),
        KeyBinding::new("ctrl-e", ParagraphEnd, c),
        KeyBinding::new("home", LineStart, c),
        KeyBinding::new("end", LineEnd, c),
        KeyBinding::new("cmd-up", DocStart, c),
        KeyBinding::new("cmd-down", DocEnd, c),
        KeyBinding::new("shift-left", SelectLeft, c),
        KeyBinding::new("shift-right", SelectRight, c),
        KeyBinding::new("shift-up", SelectUp, c),
        KeyBinding::new("shift-down", SelectDown, c),
        KeyBinding::new("alt-shift-left", SelectWordLeft, c),
        KeyBinding::new("alt-shift-right", SelectWordRight, c),
        KeyBinding::new("cmd-shift-left", SelectLineStart, c),
        KeyBinding::new("cmd-shift-right", SelectLineEnd, c),
        KeyBinding::new("shift-home", SelectLineStart, c),
        KeyBinding::new("shift-end", SelectLineEnd, c),
        KeyBinding::new("cmd-shift-up", SelectDocStart, c),
        KeyBinding::new("cmd-shift-down", SelectDocEnd, c),
        KeyBinding::new("cmd-a", SelectAll, c),
        KeyBinding::new("cmd-c", Copy, c),
        KeyBinding::new("cmd-x", Cut, c),
        KeyBinding::new("cmd-v", Paste, c),
        KeyBinding::new("ctrl-cmd-space", ShowCharacterPalette, c),
    ]);
}

/// The Live preview style of each line, shared with the element.
pub(crate) type LineStyles = Rc<Vec<Rc<LineStyle>>>;

/// Emitted when the user changes the text; never for [`NoteEditor::set_text`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteEditorEvent {
    Changed,
}

pub struct NoteEditor {
    focus_handle: FocusHandle,
    buf: EditBuffer,
    palette: NotekiPalette,
    scroll: ScrollHandle,
    /// What the last frame drew: where each line is. `None` before the first.
    layout: Option<Layout>,
    /// The x ↑ and ↓ keep, from where the vertical movement began.
    goal_x: Option<Pixels>,
    selecting: bool,
    cursor_on: bool,
    /// Scroll the cursor into view at the next frame (keys and edits, not clicks).
    reveal: bool,
    /// Bumped at every change of the text, so the styles know when to recompute.
    revision: u64,
    /// The Note's path and the Vault's links, to tell resolved Wikilinks.
    link_context: (Option<String>, Arc<VaultIndex>),
    /// Live preview styles per line, and the `(revision, cursor line)` they are for.
    styles: (Option<(u64, usize)>, LineStyles),
    /// The text as GPUI shares it, and the revision it was copied at.
    shared: (u64, SharedString),
    /// The lines shaped in earlier frames.
    shape_cache: Rc<RefCell<ShapeCache>>,
    _blink: Task<()>,
}

impl EventEmitter<NoteEditorEvent> for NoteEditor {}

impl Focusable for NoteEditor {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl NoteEditor {
    pub fn new(palette: NotekiPalette, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let blink = cx.spawn_in(window, async move |this, cx| loop {
            cx.background_executor().timer(BLINK).await;
            let alive = this.update_in(cx, |ed, window, cx| {
                if ed.focus_handle.is_focused(window) {
                    ed.cursor_on = !ed.cursor_on;
                    cx.notify();
                }
            });
            if alive.is_err() {
                break;
            }
        });
        Self {
            focus_handle: cx.focus_handle(),
            buf: EditBuffer::default(),
            palette,
            scroll: ScrollHandle::new(),
            layout: None,
            goal_x: None,
            selecting: false,
            cursor_on: true,
            reveal: false,
            revision: 0,
            link_context: (None, Arc::default()),
            styles: (None, Rc::default()),
            shared: (u64::MAX, SharedString::default()),
            shape_cache: Rc::default(),
            _blink: blink,
        }
    }

    // ---- The interface the app uses ----

    pub fn text(&self) -> &str {
        self.buf.text()
    }

    pub fn cursor(&self) -> usize {
        self.buf.cursor()
    }

    pub fn selected_text(&self) -> &str {
        self.buf.selected_text()
    }

    pub fn selected_range(&self) -> Range<usize> {
        self.buf.selection().range()
    }

    /// Replaces the whole text (opening, reloading a Note), keeping the scroll;
    /// emits no event.
    pub fn set_text(&mut self, text: &str, cursor: usize, cx: &mut Context<Self>) {
        self.buf.set_text(text, cursor);
        self.goal_x = None;
        self.layout = None;
        self.revision += 1;
        cx.notify();
    }

    /// The Note's path and the Vault's index, for Live preview to tell
    /// resolved Wikilinks from unresolved ones.
    pub fn set_link_context(
        &mut self,
        path: Option<String>,
        index: Arc<VaultIndex>,
        cx: &mut Context<Self>,
    ) {
        self.link_context = (path, index);
        self.revision += 1;
        cx.notify();
    }

    /// Back to the top, when another Note is shown.
    pub fn scroll_to_top(&mut self, cx: &mut Context<Self>) {
        self.scroll.set_offset(point(px(0.), px(0.)));
        cx.notify();
    }

    /// Scrolls the cursor into view at the next frame.
    pub fn reveal_cursor(&mut self, cx: &mut Context<Self>) {
        self.reveal = true;
        cx.notify();
    }

    /// Where the caret at `offset` was drawn in the last frame, in window
    /// coordinates (for tests).
    pub fn caret_bounds(&self, offset: usize) -> Option<Bounds<Pixels>> {
        self.layout.as_ref()?.caret(self.buf.text(), offset)
    }

    /// The height of each logical line in the last frame (for tests).
    pub fn line_heights(&self) -> Vec<f32> {
        self.layout
            .as_ref()
            .map(|l| l.line_heights())
            .unwrap_or_default()
    }

    /// How far the text is scrolled: `y` is 0 at the top, negative below it.
    pub fn scroll_offset(&self) -> Point<Pixels> {
        self.scroll.offset()
    }

    pub fn set_selected_range(&mut self, range: Range<usize>, cx: &mut Context<Self>) {
        self.buf.set_selection(range.start, range.end);
        self.goal_x = None;
        self.wake(cx);
    }

    /// Replaces the selection with `text`, as typing would (format shortcuts,
    /// inserting a Wikilink).
    pub fn replace_selection(&mut self, text: &str, cx: &mut Context<Self>) {
        let changed = self.buf.insert(text);
        self.edited(changed, cx);
    }

    pub fn focus(&self, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus_handle, cx);
    }

    pub fn set_palette(&mut self, palette: NotekiPalette, cx: &mut Context<Self>) {
        self.palette = palette;
        cx.notify();
    }

    // ---- Internals ----

    /// The text, copied once per change for the element to shape from.
    fn shared_text(&mut self) -> SharedString {
        if self.shared.0 != self.revision {
            self.shared = (
                self.revision,
                SharedString::from(self.buf.text().to_string()),
            );
        }
        self.shared.1.clone()
    }

    /// The Live preview styles of each line, recomputed when the text changes
    /// or the cursor moves to another line (whose marks are then shown).
    fn line_styles(&mut self) -> LineStyles {
        let text = self.buf.text();
        if text.len() > LIVE_PREVIEW_MAX_BYTES {
            return Rc::default();
        }
        let key = (self.revision, line_of_offset(text, self.buf.cursor()));
        if self.styles.0 != Some(key) {
            let (path, index) = &self.link_context;
            let spans = compute_markdown_spans(text, key.1, path.as_deref(), index);
            // A line whose style did not change keeps the same `Rc`, so the
            // element sees at a glance that it needs no shaping.
            let prev = &self.styles.1;
            let styles = style_lines(text, &spans, key.1)
                .into_iter()
                .enumerate()
                .map(|(i, s)| match prev.get(i) {
                    Some(p) if **p == s => p.clone(),
                    _ => Rc::new(s),
                })
                .collect();
            self.styles = (Some(key), Rc::new(styles));
        }
        self.styles.1.clone()
    }

    fn wake(&mut self, cx: &mut Context<Self>) {
        self.cursor_on = true;
        self.reveal = true;
        cx.notify();
    }

    /// Where ⌘← (`end` false) or ⌘→ goes: the start or end of the row the
    /// cursor is on, as wrapped; the logical line's when there is no layout yet.
    fn row_target(&self, end: bool) -> usize {
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

    fn row_motion(&mut self, end: bool, extend: bool, cx: &mut Context<Self>) {
        let target = self.row_target(end);
        self.buf.move_to(target, extend);
        self.goal_x = None;
        self.wake(cx);
    }

    fn delete_row(&mut self, end: bool, cx: &mut Context<Self>) {
        let target = self.row_target(end);
        let changed = self.buf.delete_to(target);
        self.edited(changed, cx);
    }

    fn edited(&mut self, changed: bool, cx: &mut Context<Self>) {
        self.goal_x = None;
        self.wake(cx);
        if changed {
            self.revision += 1;
            cx.emit(NoteEditorEvent::Changed);
        }
    }

    fn motion(&mut self, motion: Motion, extend: bool, cx: &mut Context<Self>) {
        self.buf.apply_motion(motion, extend);
        self.goal_x = None;
        self.wake(cx);
    }

    fn delete(&mut self, motion: Motion, cx: &mut Context<Self>) {
        let changed = self.buf.delete(motion);
        self.edited(changed, cx);
    }

    /// ↑ (`up`) or ↓: to the row above or below, at the x the movement began
    /// with, across lines of any height.
    fn vertical(&mut self, up: bool, extend: bool, cx: &mut Context<Self>) {
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
        let target = if up {
            if caret.top() <= layout.text_top() + px(0.5) {
                0
            } else {
                layout.offset_for_point(self.buf.text(), point(x, caret.top() - px(1.)))
            }
        } else if caret.bottom() >= layout.text_bottom() - px(0.5) {
            self.buf.text().len()
        } else {
            layout.offset_for_point(self.buf.text(), point(x, caret.bottom() + px(1.)))
        };
        self.buf.move_to(target, extend);
        self.wake(cx);
    }

    fn on_mouse_down(&mut self, ev: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus_handle, cx);
        let Some(layout) = &self.layout else {
            return;
        };
        let offset = layout.offset_for_point(self.buf.text(), ev.position);
        self.selecting = true;
        self.buf.move_to(offset, ev.modifiers.shift);
        self.goal_x = None;
        self.wake(cx);
        self.reveal = false;
    }

    fn on_mouse_move(&mut self, ev: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if !self.selecting {
            return;
        }
        if let Some(layout) = &self.layout {
            let offset = layout.offset_for_point(self.buf.text(), ev.position);
            self.buf.move_to(offset, true);
            self.wake(cx);
            self.reveal = false;
        }
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        self.selecting = false;
    }
}

macro_rules! handlers {
    ($($action:ident => |$ed:ident, $cx:ident| $body:expr;)*) => {
        impl NoteEditor {
            fn bind_actions(&self, el: gpui_kit::Stateful<gpui_kit::Div>, cx: &mut Context<Self>) -> gpui_kit::Stateful<gpui_kit::Div> {
                el $(.on_action(cx.listener(|$ed: &mut Self, _: &$action, _w, $cx| $body)))*
            }
        }
    };
}

handlers! {
    Backspace => |ed, cx| ed.delete(Motion::Left, cx);
    Delete => |ed, cx| ed.delete(Motion::Right, cx);
    DeleteWordLeft => |ed, cx| ed.delete(Motion::WordLeft, cx);
    DeleteWordRight => |ed, cx| ed.delete(Motion::WordRight, cx);
    DeleteToLineStart => |ed, cx| ed.delete_row(false, cx);
    DeleteToLineEnd => |ed, cx| ed.delete_row(true, cx);
    DeleteToParagraphEnd => |ed, cx| ed.delete(Motion::LineEnd, cx);
    Newline => |ed, cx| ed.replace_selection("\n", cx);
    Left => |ed, cx| ed.motion(Motion::Left, false, cx);
    Right => |ed, cx| ed.motion(Motion::Right, false, cx);
    Up => |ed, cx| ed.vertical(true, false, cx);
    Down => |ed, cx| ed.vertical(false, false, cx);
    WordLeft => |ed, cx| ed.motion(Motion::WordLeft, false, cx);
    WordRight => |ed, cx| ed.motion(Motion::WordRight, false, cx);
    LineStart => |ed, cx| ed.row_motion(false, false, cx);
    LineEnd => |ed, cx| ed.row_motion(true, false, cx);
    ParagraphStart => |ed, cx| ed.motion(Motion::LineStart, false, cx);
    ParagraphEnd => |ed, cx| ed.motion(Motion::LineEnd, false, cx);
    DocStart => |ed, cx| ed.motion(Motion::DocStart, false, cx);
    DocEnd => |ed, cx| ed.motion(Motion::DocEnd, false, cx);
    SelectLeft => |ed, cx| ed.motion(Motion::Left, true, cx);
    SelectRight => |ed, cx| ed.motion(Motion::Right, true, cx);
    SelectUp => |ed, cx| ed.vertical(true, true, cx);
    SelectDown => |ed, cx| ed.vertical(false, true, cx);
    SelectWordLeft => |ed, cx| ed.motion(Motion::WordLeft, true, cx);
    SelectWordRight => |ed, cx| ed.motion(Motion::WordRight, true, cx);
    SelectLineStart => |ed, cx| ed.row_motion(false, true, cx);
    SelectLineEnd => |ed, cx| ed.row_motion(true, true, cx);
    SelectDocStart => |ed, cx| ed.motion(Motion::DocStart, true, cx);
    SelectDocEnd => |ed, cx| ed.motion(Motion::DocEnd, true, cx);
    SelectAll => |ed, cx| { ed.buf.select_all(); ed.wake(cx) };
    Copy => |ed, cx| {
        if !ed.buf.selection().is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(ed.buf.selected_text().to_string()));
        }
    };
    Cut => |ed, cx| {
        if !ed.buf.selection().is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(ed.buf.selected_text().to_string()));
            ed.replace_selection("", cx);
        }
    };
    Paste => |ed, cx| {
        if let Some(text) = cx.read_from_clipboard().and_then(|i| i.text()) {
            ed.replace_selection(&text, cx);
        }
    };
}

impl Render for NoteEditor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let root =
            div()
                .id("note-editor")
                .key_context(CONTEXT)
                .track_focus(&self.focus_handle)
                .size_full()
                .overflow_y_scroll()
                .track_scroll(&self.scroll)
                .cursor(CursorStyle::IBeam)
                .on_action(cx.listener(|_, _: &ShowCharacterPalette, window, _| {
                    window.show_character_palette()
                }))
                .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
                .on_mouse_move(cx.listener(Self::on_mouse_move))
                .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
                .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up));
        self.bind_actions(root, cx).child(
            div().w_full().px_6().py_4().child(
                div()
                    .w_full()
                    .max_w(px(820.))
                    .mx_auto()
                    .child(EditorElement::new(cx.entity())),
            ),
        )
    }
}

impl EntityInputHandler for NoteEditor {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        adjusted: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let t = self.buf.text();
        let r = range_from_utf16(t, &range_utf16);
        adjusted.replace(range_to_utf16(t, &r));
        Some(t[r].to_string())
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let s = self.buf.selection();
        Some(UTF16Selection {
            range: range_to_utf16(self.buf.text(), &s.range()),
            reversed: s.reversed(),
        })
    }

    fn set_selected_text_range(
        &mut self,
        range_utf16: Range<usize>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let r = range_from_utf16(self.buf.text(), &range_utf16);
        self.set_selected_range(r, cx);
    }

    fn text_length_utf16(&mut self, _: &mut Window, _: &mut Context<Self>) -> Option<usize> {
        Some(self.buf.text().encode_utf16().count())
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.buf
            .marked()
            .map(|r| range_to_utf16(self.buf.text(), &r))
    }

    fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.buf.unmark();
        cx.notify();
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let changed = match range_utf16 {
            Some(r) => {
                let r = range_from_utf16(self.buf.text(), &r);
                self.buf.replace(r, text)
            }
            None => self.buf.insert(text),
        };
        window.notify_text_input_changed(&self.focus_handle);
        self.edited(changed, cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        text: &str,
        selected_utf16: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range_utf16.map(|r| range_from_utf16(self.buf.text(), &r));
        let selected = selected_utf16.map(|r| range_from_utf16(text, &r));
        let changed = self.buf.replace_and_mark(range, text, selected);
        window.notify_text_input_changed(&self.focus_handle);
        self.edited(changed, cx);
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        _: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let layout = self.layout.as_ref()?;
        let t = self.buf.text();
        let r = range_from_utf16(t, &range_utf16);
        let start = layout.caret(t, r.start)?;
        let end = layout.caret(t, r.end)?;
        Some(if start.top() == end.top() {
            Bounds::from_corners(start.origin, end.bottom_right())
        } else {
            start
        })
    }

    fn character_index_for_point(
        &mut self,
        p: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        let layout = self.layout.as_ref()?;
        let t = self.buf.text();
        Some(crate::core::editor::buffer::offset_to_utf16(
            t,
            layout.offset_for_point(t, p),
        ))
    }
}
