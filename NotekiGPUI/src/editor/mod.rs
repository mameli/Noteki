//! Noteki's editor element: draws the text of a Note line by line, each line
//! shaped and measured on its own so that lines may have different heights,
//! and takes keyboard, mouse and native text input (accents, dead keys,
//! input methods, dictation) through GPUI's `EntityInputHandler`.
//!
//! The rules (edits, selection, movement, line geometry) are in
//! `crate::core::editor` and tested there; this module only measures, draws
//! and dispatches.

mod element;
mod find_bar;
mod style;

use std::cell::RefCell;
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
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
use crate::core::editor::code;
use crate::core::editor::format::{format_lines, toggle_wrap};
pub use crate::core::editor::format::{LineFormat, Wrap};
use crate::core::editor::history::History;
use crate::core::editor::lines::edge_scroll;
use crate::core::editor::movement::{line_range_at, word_range_at};
use crate::core::editor::styling::{
    style_line, style_lines, CodeColours, CodePart, Context as StyleContext, Decoration, Image,
    LineStyle,
};
use crate::core::editor::{EditBuffer, Motion};
use crate::core::embeds::resolve_embed;
use crate::core::markdown::{
    self, compute_markdown_spans, line_of_offset, link_at, BlockKind, Link,
};
use crate::core::vault::resolve_vault_path;
use crate::core::wikilink::VaultIndex;
use crate::theme::NotekiPalette;

#[doc(hidden)]
pub use element::shape_for_bench;
use element::{EditorElement, Layout, ShapeCache};

const CONTEXT: &str = "NoteEditor";
const BLINK: Duration = Duration::from_millis(500);
const EDGE_SCROLL_TICK: Duration = Duration::from_millis(16);
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
        Heading1,
        Heading2,
        Heading3,
        Heading4,
        Heading5,
        Heading6,
        ToggleTask,
        Indent,
        Outdent,
        Undo,
        Redo,
        OpenFind,
        FindNext,
        FindPrevious,
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
        KeyBinding::new("cmd-1", Heading1, c),
        KeyBinding::new("cmd-2", Heading2, c),
        KeyBinding::new("cmd-3", Heading3, c),
        KeyBinding::new("cmd-4", Heading4, c),
        KeyBinding::new("cmd-5", Heading5, c),
        KeyBinding::new("cmd-6", Heading6, c),
        KeyBinding::new("cmd-enter", ToggleTask, c),
        KeyBinding::new("cmd-]", Indent, c),
        KeyBinding::new("cmd-[", Outdent, c),
        KeyBinding::new("cmd-f", OpenFind, c),
        KeyBinding::new("cmd-g", FindNext, c),
        KeyBinding::new("cmd-shift-g", FindPrevious, c),
        KeyBinding::new("cmd-g", FindNext, Some(find_bar::FIND_CONTEXT)),
        KeyBinding::new("cmd-shift-g", FindPrevious, Some(find_bar::FIND_CONTEXT)),
        KeyBinding::new("escape", find_bar::CloseFind, Some(find_bar::FIND_CONTEXT)),
        KeyBinding::new("cmd-z", Undo, c),
        KeyBinding::new("cmd-shift-z", Redo, c),
        KeyBinding::new("cmd-c", Copy, c),
        KeyBinding::new("cmd-x", Cut, c),
        KeyBinding::new("cmd-v", Paste, c),
        KeyBinding::new("ctrl-cmd-space", ShowCharacterPalette, c),
    ]);
}

/// A format command: marks around the selection, or a change of its lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Wrap(Wrap),
    Lines(LineFormat),
}

/// What the editor needs from the Vault: the Note's path, the index, the folder.
#[derive(Clone, Default)]
pub struct LinkContext {
    pub path: Option<String>,
    pub index: Arc<VaultIndex>,
    pub root: Option<std::path::PathBuf>,
}

/// The Live preview style of each line, shared with the element.
pub(crate) type LineStyles = Rc<Vec<Rc<LineStyle>>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoteEditorEvent {
    /// The user changed the text; never sent for [`NoteEditor::set_text`].
    Changed,
    /// A click on a link: the app opens it (⌘-click: in a new Tab).
    OpenLink { link: Link, new_tab: bool },
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
    /// The selection being made with the mouse, while the button is down.
    drag: Option<Drag>,
    _edge_scroll: Option<Task<()>>,
    cursor_on: bool,
    /// Scroll the cursor into view at the next frame (keys and edits, not clicks).
    reveal: bool,
    /// The pointer is over a link that a click would open.
    over_link: bool,
    /// The find bar, while it is open (⌘F).
    find: Option<find_bar::FindBar>,
    /// The Note shown, and the undo histories of the others.
    note_key: Option<String>,
    histories: HashMap<String, History>,
    /// Bumped at every change of the text, so the styles know when to recompute.
    revision: u64,
    /// The Note's path and the Vault's links, to tell resolved Wikilinks.
    link_context: LinkContext,
    /// The images Embeds name, by target: resolved and measured once.
    images: HashMap<String, Option<Image>>,
    /// The colours of the code blocks of the last styling, by language and code.
    code_colours: HashMap<u64, CodeColours>,
    /// Live preview styles per line with every mark hidden, and the revision.
    base_styles: (Option<u64>, LineStyles),
    /// The same with the cursor's line restyled, and the `(revision, cursor)`.
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
            drag: None,
            _edge_scroll: None,
            cursor_on: true,
            reveal: false,
            over_link: false,
            find: None,
            note_key: None,
            histories: HashMap::new(),
            revision: 0,
            link_context: LinkContext::default(),
            images: HashMap::new(),
            code_colours: HashMap::new(),
            base_styles: (None, Rc::default()),
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
        // One undoable step: a reload, a Merge of another app's change.
        self.buf.replace_all(text, cursor);
        self.goal_x = None;
        self.layout = None;
        self.revision += 1;
        cx.notify();
    }

    /// Shows another Note: its text, its cursor, and its own undo history
    /// (SPEC › Tabs: each keeps its own), kept by `key` while others show.
    pub fn open_note(&mut self, key: &str, text: &str, cursor: usize, cx: &mut Context<Self>) {
        let leaving = self.buf.take_history();
        if let Some(previous) = self.note_key.replace(key.to_string()) {
            self.histories.insert(previous, leaving);
        }
        let history = self.histories.remove(key).unwrap_or_default();
        self.buf.set_text(text, cursor, history);
        self.goal_x = None;
        self.layout = None;
        self.revision += 1;
        cx.notify();
    }

    /// The Note's path, the Vault's index and folder: for Live preview to tell
    /// resolved Wikilinks from unresolved ones and to find images.
    pub fn set_link_context(&mut self, context: LinkContext, cx: &mut Context<Self>) {
        self.link_context = context;
        self.images.clear();
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

    /// A format command (SPEC › Format) on the selection: one edit, one undo step.
    pub fn format(&mut self, command: Format, cx: &mut Context<Self>) {
        let text = self.buf.text();
        let sel = self.buf.selection().range();
        let change = match command {
            Format::Wrap(w) => toggle_wrap(text, sel, w),
            Format::Lines(l) => format_lines(text, sel, l),
        };
        self.buf.close_undo_step();
        let changed = self.buf.replace(change.range, &change.text);
        self.buf
            .set_selection(change.selection.start, change.selection.end);
        self.edited(changed, cx);
    }

    /// Replaces the selection with `text`, as typing would (inserting a Wikilink).
    pub fn replace_selection(&mut self, text: &str, cx: &mut Context<Self>) {
        self.buf.close_undo_step();
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

    /// The Live preview styles of each line: every line once per change of
    /// the text, then only the lines the cursor's position changes.
    fn line_styles(&mut self) -> LineStyles {
        let text = self.buf.text();
        if text.len() > LIVE_PREVIEW_MAX_BYTES {
            return Rc::default();
        }
        let LinkContext { path, index, root } = &self.link_context;
        // Every line with its marks hidden, once per change of the text. A code
        // block is highlighted again only when its code or language changed.
        if self.base_styles.0 != Some(self.revision) {
            let spans = compute_markdown_spans(text, None, path.as_deref(), index);
            let cache = &mut self.code_colours;
            let mut used = HashMap::new();
            let mut highlight = |language: &str, code: &str| {
                let mut h = DefaultHasher::new();
                (language, code).hash(&mut h);
                let key = h.finish();
                let colours = cache
                    .remove(&key)
                    .or_else(|| used.get(&key).cloned())
                    .unwrap_or_else(|| code::highlight(language, code).into());
                used.insert(key, Rc::clone(&colours));
                colours
            };
            let images = &mut self.images;
            let mut image = |target: &str| -> Option<Image> {
                images
                    .entry(target.to_string())
                    .or_insert_with(|| {
                        let root = root.as_deref()?;
                        let found = resolve_embed(root, target, path.as_deref(), index)?;
                        let abs = resolve_vault_path(root, &found.path).ok()?;
                        Some(Image {
                            path: abs.into(),
                            width: u32::try_from(found.width?).ok()?,
                            height: u32::try_from(found.height?).ok()?,
                        })
                    })
                    .clone()
            };
            let styles = style_lines(
                text,
                &spans,
                StyleContext {
                    highlight: &mut highlight,
                    image: &mut image,
                },
            );
            *cache = used;
            let styles = keep_equal(&self.base_styles.1, styles);
            self.base_styles = (Some(self.revision), Rc::new(styles));
        }
        // Then the lines the cursor shows marks on: its own (the element it is
        // in), or a code block's fences while it is in the block. Moving the
        // cursor costs a line or two, not the Note.
        let cursor = self.buf.cursor();
        let key = (self.revision, cursor);
        if self.styles.0 != Some(key) {
            let base = &self.base_styles.1;
            let mut styles: Vec<Rc<LineStyle>> = (**base).clone();
            let lines = markdown::lines(text);
            let i = line_of_offset(text, cursor);
            let mut restyle = |line: &markdown::Line, in_line: usize| {
                let mut spans = Vec::new();
                markdown::line_spans(line, Some(in_line), path.as_deref(), index, &mut spans);
                let mut style = style_line(line, &spans);
                if let Some(slot) = styles.get_mut(line.index) {
                    // A code block's box and an image are the block's, not the line's.
                    match slot.decoration {
                        Some(Decoration::Code(_)) => {
                            style.decoration = slot.decoration.clone();
                            style.segments = slot.segments.clone();
                        }
                        Some(Decoration::Image(_)) => style.decoration = slot.decoration.clone(),
                        _ => {}
                    }
                    if **slot != style {
                        *slot = Rc::new(style);
                    }
                }
            };
            match lines.get(i) {
                Some(line) if matches!(line.kind, BlockKind::Code | BlockKind::FenceDelimiter) => {
                    let is = |j: usize, open: bool| match &base
                        .get(j)
                        .and_then(|s| s.decoration.clone())
                    {
                        Some(Decoration::Code(CodePart::Open { .. })) => open,
                        Some(Decoration::Code(CodePart::Close)) => !open,
                        _ => false,
                    };
                    let first = (0..=i).rev().find(|&j| is(j, true));
                    let last = (i..lines.len()).find(|&j| is(j, false));
                    for j in first.into_iter().chain(last) {
                        restyle(&lines[j], 0);
                    }
                }
                Some(line) => restyle(line, cursor - line.start),
                None => {}
            }
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

    /// What the mouse selects: what is under it when it went down, and the unit
    /// it goes on selecting by while it moves (a double-click: words).
    fn unit_at(unit: ClickUnit, text: &str, offset: usize) -> Range<usize> {
        match unit {
            ClickUnit::Char => offset..offset,
            ClickUnit::Word => word_range_at(text, offset),
            ClickUnit::Line => line_range_at(text, offset),
        }
    }

    fn on_mouse_down(&mut self, ev: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus_handle, cx);
        let Some(layout) = &self.layout else {
            return;
        };
        // SPEC: a click on a checkbox writes `[x]` or `[ ]` and leaves the
        // cursor where it was.
        if let Some((bracket, checked)) = layout.checkbox_at(ev.position) {
            let sel = self.buf.selection();
            self.buf.close_undo_step();
            let changed = self
                .buf
                .replace(bracket + 1..bracket + 2, if checked { " " } else { "x" });
            self.buf.set_selection(sel.anchor, sel.head);
            self.edited(changed, cx);
            self.reveal = false;
            return;
        }
        let offset = layout.offset_for_point(self.buf.text(), ev.position);
        if ev.click_count == 1 && !ev.modifiers.shift {
            if let Some(link) = self.link_to_open(offset) {
                cx.emit(NoteEditorEvent::OpenLink {
                    link,
                    new_tab: ev.modifiers.platform,
                });
                return;
            }
        }
        let unit = match ev.click_count {
            0 | 1 => ClickUnit::Char,
            2 => ClickUnit::Word,
            _ => ClickUnit::Line,
        };
        let origin = if unit == ClickUnit::Char && ev.modifiers.shift {
            // ⇧-click extends what is selected.
            let a = self.buf.selection().anchor;
            a..a
        } else {
            Self::unit_at(unit, self.buf.text(), offset)
        };
        self.drag = Some(Drag {
            unit,
            origin,
            pointer: ev.position,
        });
        self.select_to(offset);
        self.goal_x = None;
        self.wake(cx);
        self.reveal = false;
        self._edge_scroll = Some(cx.spawn_in(window, async move |this, cx| loop {
            cx.background_executor().timer(EDGE_SCROLL_TICK).await;
            match this.update(cx, |ed, cx| ed.edge_scroll(cx)) {
                Ok(true) => {}
                _ => break,
            }
        }));
    }

    /// The link a click at `offset` opens: one whose text is there, unless the
    /// cursor is in it already (its marks show: the user is editing it).
    fn link_to_open(&self, offset: usize) -> Option<Link> {
        let (range, link) = link_at(self.buf.text(), offset)?;
        let c = self.buf.cursor();
        (!(range.start <= c && c <= range.end)).then_some(link)
    }

    /// Shows a pointing hand over a link a click would open.
    fn on_hover(&mut self, ev: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.drag.is_some() {
            return;
        }
        let over = self.layout.as_ref().is_some_and(|l| {
            let offset = l.offset_for_point(self.buf.text(), ev.position);
            self.link_to_open(offset).is_some()
        });
        if over != self.over_link {
            self.over_link = over;
            cx.notify();
        }
    }

    /// Selects from where the drag began to `offset`, by the drag's unit.
    fn select_to(&mut self, offset: usize) {
        let Some(drag) = &self.drag else {
            return;
        };
        let here = Self::unit_at(drag.unit, self.buf.text(), offset);
        let o = drag.origin.clone();
        if here.start < o.start {
            self.buf.set_selection(o.end, here.start);
        } else {
            self.buf.set_selection(o.start, here.end.max(o.end));
        }
    }

    /// While a selection is dragged, the pointer is followed anywhere in the
    /// window (the element listens for the whole window, see `paint`).
    pub(super) fn on_drag_move(&mut self, ev: &MouseMoveEvent, cx: &mut Context<Self>) {
        let Some(drag) = &mut self.drag else {
            return;
        };
        drag.pointer = ev.position;
        if let Some(layout) = &self.layout {
            let offset = layout.offset_for_point(self.buf.text(), ev.position);
            self.select_to(offset);
            self.wake(cx);
            self.reveal = false;
        }
    }

    pub(super) fn on_drag_end(&mut self, _: &MouseUpEvent, _: &mut Context<Self>) {
        if self.drag.is_none() {
            return;
        }
        self.drag = None;
        self._edge_scroll = None;
    }

    /// One frame of scrolling while a selection is dragged past the top or
    /// bottom of the view; `false` once the drag is over.
    fn edge_scroll(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(drag) = &self.drag else {
            return false;
        };
        let pointer = drag.pointer;
        let view = self.scroll.bounds();
        let step = edge_scroll(
            f32::from(pointer.y),
            f32::from(view.top()),
            f32::from(view.bottom()),
        );
        if step == 0. {
            return true;
        }
        let (offset, max) = (self.scroll.offset(), self.scroll.max_offset());
        let y = (offset.y - px(step)).clamp(-max.y, px(0.));
        if y != offset.y {
            self.scroll.set_offset(point(offset.x, y));
            // The text moved under the pointer by what was scrolled.
            if let Some(layout) = &self.layout {
                let under = point(pointer.x, pointer.y - (y - offset.y));
                let o = layout.offset_for_point(self.buf.text(), under);
                self.select_to(o);
            }
            cx.notify();
        }
        true
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClickUnit {
    Char,
    Word,
    Line,
}

struct Drag {
    unit: ClickUnit,
    /// What the click selected, which the drag always keeps selected.
    origin: Range<usize>,
    /// Where the pointer is, in window coordinates.
    pointer: Point<Pixels>,
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
    Heading1 => |ed, cx| ed.format(Format::Lines(LineFormat::Heading(1)), cx);
    Heading2 => |ed, cx| ed.format(Format::Lines(LineFormat::Heading(2)), cx);
    Heading3 => |ed, cx| ed.format(Format::Lines(LineFormat::Heading(3)), cx);
    Heading4 => |ed, cx| ed.format(Format::Lines(LineFormat::Heading(4)), cx);
    Heading5 => |ed, cx| ed.format(Format::Lines(LineFormat::Heading(5)), cx);
    Heading6 => |ed, cx| ed.format(Format::Lines(LineFormat::Heading(6)), cx);
    ToggleTask => |ed, cx| ed.format(Format::Lines(LineFormat::Task), cx);
    Indent => |ed, cx| ed.format(Format::Lines(LineFormat::Indent), cx);
    Outdent => |ed, cx| ed.format(Format::Lines(LineFormat::Outdent), cx);
    Undo => |ed, cx| { let changed = ed.buf.undo(); ed.edited(changed, cx) };
    Redo => |ed, cx| { let changed = ed.buf.redo(); ed.edited(changed, cx) };
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
        let find_bar = self.render_find_bar(cx);
        let root = div()
            .id("note-editor")
            .key_context(CONTEXT)
            .track_focus(&self.focus_handle)
            .size_full()
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .cursor(if self.over_link {
                CursorStyle::PointingHand
            } else {
                CursorStyle::IBeam
            })
            .on_mouse_move(cx.listener(Self::on_hover))
            .on_action(
                cx.listener(|_, _: &ShowCharacterPalette, window, _| {
                    window.show_character_palette()
                }),
            )
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_action(cx.listener(|ed, _: &OpenFind, window, cx| ed.open_find(window, cx)))
            .on_action(
                cx.listener(|ed, _: &FindNext, window, cx| ed.find_step(true, false, window, cx)),
            )
            .on_action(cx.listener(|ed, _: &FindPrevious, window, cx| {
                ed.find_step(false, false, window, cx)
            }));
        let editor = self.bind_actions(root, cx).child(
            div().w_full().px_6().py_4().child(
                div()
                    .w_full()
                    .max_w(px(820.))
                    .mx_auto()
                    .child(EditorElement::new(cx.entity())),
            ),
        );
        // The find bar is outside the editor's key context: typing in it is
        // not editing the Note.
        div()
            .size_full()
            .flex()
            .flex_col()
            .children(find_bar)
            .child(div().flex_1().min_h_0().child(editor))
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

/// `new`, where a line's style equal to the one in `prev` keeps `prev`'s `Rc`,
/// so the element sees at a glance that the line needs no shaping.
fn keep_equal(prev: &[Rc<LineStyle>], new: Vec<LineStyle>) -> Vec<Rc<LineStyle>> {
    new.into_iter()
        .enumerate()
        .map(|(i, s)| match prev.get(i) {
            Some(p) if **p == s => p.clone(),
            _ => Rc::new(s),
        })
        .collect()
}
