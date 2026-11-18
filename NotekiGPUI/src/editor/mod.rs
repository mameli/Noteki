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
mod link_completion;
mod math;
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
    EntityInputHandler, EventEmitter, FocusHandle, Focusable, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, Pixels, Point, ScrollHandle, SharedString, Task, UTF16Selection,
    Window,
};

use crate::core::diagram::{self, Diagram};
use crate::core::editor::buffer::{range_from_utf16, range_to_utf16};
use crate::core::editor::code;
use crate::core::editor::figures::{figure_blocks, inline_formulas, FigureSource};
use crate::core::editor::format::{format_lines, make_link, toggle_comment, toggle_wrap};
pub use crate::core::editor::format::{LineFormat, Wrap};
use crate::core::editor::history::History;
use crate::core::editor::lines::{edge_scroll, logical_lines};
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
use crate::core::math::{typeset, Formula};
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
        SoftNewline,
        TabKey,
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
        MakeLink,
        ToggleComment,
        Indent,
        Outdent,
        Undo,
        Redo,
        OpenFind,
        OpenReplace,
        SelectNextOccurrence,
        SingleSelection,
        FindNext,
        FindPrevious,
        Copy,
        Cut,
        Paste,
        PasteAndMatchStyle,
        ShowCharacterPalette,
    ]
);

pub fn register_keybindings(cx: &mut App) {
    // The formulas' fonts, for GPUI to draw their glyphs (editor::math).
    let _ = cx
        .text_system()
        .add_fonts(crate::core::math::katex_fonts().collect());
    let c = Some(CONTEXT);
    cx.bind_keys(
        [
            crate::keys::binding("backspace", Backspace, c),
            crate::keys::binding("shift-backspace", Backspace, c),
            crate::keys::binding("delete", Delete, c),
            crate::keys::binding("ctrl-d", Delete, c),
            crate::keys::binding("alt-backspace", DeleteWordLeft, c),
            crate::keys::binding("alt-delete", DeleteWordRight, c),
            crate::keys::binding("cmd-backspace", DeleteToLineStart, c),
            crate::keys::binding("cmd-delete", DeleteToLineEnd, c),
            crate::keys::binding("ctrl-k", DeleteToParagraphEnd, c),
            crate::keys::binding("enter", Newline, c),
            crate::keys::binding("tab", TabKey, c),
            crate::keys::binding("shift-enter", SoftNewline, c),
            crate::keys::binding("left", Left, c),
            crate::keys::binding("right", Right, c),
            crate::keys::binding("up", Up, c),
            crate::keys::binding("down", Down, c),
            crate::keys::binding("ctrl-b", Left, c),
            crate::keys::binding("ctrl-f", Right, c),
            crate::keys::binding("ctrl-p", Up, c),
            crate::keys::binding("ctrl-n", Down, c),
            crate::keys::binding("alt-left", WordLeft, c),
            crate::keys::binding("alt-right", WordRight, c),
            crate::keys::binding("cmd-left", LineStart, c),
            crate::keys::binding("cmd-right", LineEnd, c),
            crate::keys::binding("ctrl-a", ParagraphStart, c),
            crate::keys::binding("ctrl-e", ParagraphEnd, c),
            crate::keys::binding("home", LineStart, c),
            crate::keys::binding("end", LineEnd, c),
            crate::keys::binding("cmd-up", DocStart, c),
            crate::keys::binding("cmd-down", DocEnd, c),
            crate::keys::binding("shift-left", SelectLeft, c),
            crate::keys::binding("shift-right", SelectRight, c),
            crate::keys::binding("shift-up", SelectUp, c),
            crate::keys::binding("shift-down", SelectDown, c),
            crate::keys::binding("alt-shift-left", SelectWordLeft, c),
            crate::keys::binding("alt-shift-right", SelectWordRight, c),
            crate::keys::binding("cmd-shift-left", SelectLineStart, c),
            crate::keys::binding("cmd-shift-right", SelectLineEnd, c),
            crate::keys::binding("shift-home", SelectLineStart, c),
            crate::keys::binding("shift-end", SelectLineEnd, c),
            crate::keys::binding("cmd-shift-up", SelectDocStart, c),
            crate::keys::binding("cmd-shift-down", SelectDocEnd, c),
            crate::keys::binding("cmd-a", SelectAll, c),
            crate::keys::binding("cmd-1", Heading1, c),
            crate::keys::binding("cmd-2", Heading2, c),
            crate::keys::binding("cmd-3", Heading3, c),
            crate::keys::binding("cmd-4", Heading4, c),
            crate::keys::binding("cmd-5", Heading5, c),
            crate::keys::binding("cmd-6", Heading6, c),
            crate::keys::binding("cmd-enter", ToggleTask, c),
            crate::keys::binding("cmd-k", MakeLink, c),
            crate::keys::binding("cmd-/", ToggleComment, c),
            crate::keys::binding("cmd-]", Indent, c),
            crate::keys::binding("cmd-[", Outdent, c),
            crate::keys::binding("cmd-f", OpenFind, c),
            crate::keys::binding("alt-cmd-f", OpenReplace, c),
            crate::keys::binding("cmd-d", SelectNextOccurrence, c),
            crate::keys::binding("escape", SingleSelection, c),
            crate::keys::binding("cmd-g", FindNext, c),
            crate::keys::binding("cmd-shift-g", FindPrevious, c),
            crate::keys::binding("cmd-g", FindNext, Some(find_bar::FIND_CONTEXT)),
            crate::keys::binding("cmd-shift-g", FindPrevious, Some(find_bar::FIND_CONTEXT)),
            crate::keys::binding("escape", find_bar::CloseFind, Some(find_bar::FIND_CONTEXT)),
            crate::keys::binding("cmd-z", Undo, c),
            crate::keys::binding("cmd-shift-z", Redo, c),
            crate::keys::binding("cmd-c", Copy, c),
            crate::keys::binding("cmd-x", Cut, c),
            crate::keys::binding("cmd-v", Paste, c),
            crate::keys::binding("cmd-shift-v", PasteAndMatchStyle, c),
            crate::keys::binding("ctrl-cmd-space", ShowCharacterPalette, c),
        ]
        .into_iter()
        .flatten(),
    );
}

/// A format command: marks around the selection, or a change of its lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Wrap(Wrap),
    Lines(LineFormat),
    /// ⌘K: `[text](url)`, the URL from the clipboard.
    Link,
    /// ⌘/: `%%` around the selection or the line.
    Comment,
}

/// What the editor needs from the Vault: the Note's path, the index, the folder.
#[derive(Clone, Default)]
pub struct LinkContext {
    pub path: Option<String>,
    pub index: Arc<VaultIndex>,
    pub root: Option<std::path::PathBuf>,
    /// The Notes `[[` offers, the most recently opened first.
    pub completions: Rc<Vec<crate::core::wikilink::LinkCompletion>>,
}

/// The Live preview style of each line, shared with the element.
pub(crate) type LineStyles = Rc<Vec<Rc<LineStyle>>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoteEditorEvent {
    /// The user changed the text; never sent for [`NoteEditor::set_text`].
    Changed,
    /// A click on a link: the app opens it (⌘-click: in a new Tab).
    OpenLink { link: Link, new_tab: bool },
    /// An image pasted: the app saves it next to the Note and embeds it.
    PasteImage {
        bytes: Vec<u8>,
        extension: &'static str,
    },
    /// Files dropped on the text, the cursor where they were dropped.
    DropFiles(Vec<std::path::PathBuf>),
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
    /// The text's size, from the settings (⌘+, ⌘−, ⌘0).
    font_size: Pixels,
    /// The text's and the code's fonts, and the longest line in characters,
    /// from the settings.
    font_family: Option<SharedString>,
    code_font: SharedString,
    line_limit: Option<u32>,
    /// The Note chosen in the `[[` list, and the link where Esc closed it.
    completion_pick: usize,
    completion_dismissed: Option<usize>,
    /// The `(revision, cursor)` after the last edit: the list shows only while
    /// typing, not when the cursor is moved or clicked into a link.
    typed_at: Option<(u64, usize)>,
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
    /// Formulas typeset, by TeX and style (display or in a line): once each.
    formulas: HashMap<(String, bool), Option<Rc<Formula>>>,
    /// Diagrams by id: `None` while drawn in the background, or when the
    /// source draws none; their images, as GPUI draws them.
    diagrams: HashMap<u64, Option<Rc<Diagram>>>,
    pub(super) diagram_images: HashMap<u64, Arc<gpui_kit::Image>>,
    /// The base styles before figures replace blocks and formulas: the
    /// cursor's block or line shows its source from them.
    plain_styles: LineStyles,
    /// The lines of each block drawn as one figure, in the base styles.
    figure_lines: Vec<Range<usize>>,
    /// The diagram block the cursor was in, not drawn while it was: drawn
    /// once the cursor leaves it, not at every key typed in it.
    deferred_diagram: Option<Range<usize>>,
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
            completion_pick: 0,
            completion_dismissed: None,
            typed_at: None,
            font_size: px(15.),
            font_family: None,
            code_font: element::CODE_FONT.into(),
            line_limit: Some(100),
            note_key: None,
            histories: HashMap::new(),
            revision: 0,
            link_context: LinkContext::default(),
            images: HashMap::new(),
            formulas: HashMap::new(),
            diagrams: HashMap::new(),
            diagram_images: HashMap::new(),
            plain_styles: Rc::default(),
            figure_lines: Vec::new(),
            deferred_diagram: None,
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
        // SPEC: a Note shows with the cursor at the start of its text, after
        // the Frontmatter (which then shows as the Properties table).
        let cursor = if cursor == 0 {
            crate::core::frontmatter::frontmatter_end_line(text)
                .and_then(|end| logical_lines(text).get(end).map(|l| l.start))
                .unwrap_or(0)
        } else {
            cursor
        };
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
            Format::Link => {
                let clip = cx.read_from_clipboard().and_then(|c| c.text());
                make_link(text, sel, clip.as_deref())
            }
            Format::Comment => toggle_comment(text, sel),
        };
        self.buf.close_undo_step();
        let changed = self.buf.replace(change.range, &change.text);
        self.buf
            .set_selection(change.selection.start, change.selection.end);
        self.edited(changed, cx);
    }

    /// Return: the next item of a list or line of a quote (format::newline);
    /// with several cursors, a newline at each.
    pub fn newline(&mut self, cx: &mut Context<Self>) {
        if self.buf.selections().len() > 1 {
            return self.replace_selection("\n", cx);
        }
        let change =
            crate::core::editor::format::newline(self.buf.text(), self.buf.selection().range());
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

    pub fn set_font_size(&mut self, size: Pixels, cx: &mut Context<Self>) {
        self.font_size = size;
        self.reveal = true;
        cx.notify();
    }

    /// The settings' fonts (empty: the system's) and line length.
    pub fn set_text_settings(
        &mut self,
        font_family: &str,
        code_font: &str,
        line_limit: Option<u32>,
        cx: &mut Context<Self>,
    ) {
        self.font_family = (!font_family.is_empty()).then(|| font_family.to_string().into());
        self.code_font = if code_font.is_empty() {
            element::CODE_FONT.into()
        } else {
            code_font.to_string().into()
        };
        self.line_limit = line_limit;
        self.reveal = true;
        cx.notify();
    }

    pub fn set_palette(&mut self, palette: NotekiPalette, cx: &mut Context<Self>) {
        // Diagrams are drawn in the light or dark theme.
        if palette.dark != self.palette.dark {
            self.base_styles.0 = None;
            self.styles.0 = None;
        }
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
    fn line_styles(&mut self, cx: &mut Context<Self>) -> LineStyles {
        let text = self.buf.text();
        if text.len() > LIVE_PREVIEW_MAX_BYTES {
            return Rc::default();
        }
        if let Some(block) = &self.deferred_diagram {
            if !block.contains(&line_of_offset(text, self.buf.cursor())) {
                self.base_styles.0 = None;
            }
        }
        let LinkContext {
            path, index, root, ..
        } = &self.link_context;
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
            let plain = keep_equal(&self.plain_styles, styles);
            let figured = self.with_figures(&plain, cx);
            let figured = keep_equal(&self.base_styles.1, figured);
            self.plain_styles = Rc::new(plain);
            self.base_styles = (Some(self.revision), Rc::new(figured));
        }
        // Then the lines the cursor shows marks on: its own (the element it is
        // in), or a code block's fences while it is in the block. Moving the
        // cursor costs a line or two, not the Note.
        let cursor = self.buf.cursor();
        let key = (self.revision, cursor);
        if self.styles.0 != Some(key) {
            let text = self.buf.text();
            let LinkContext { path, index, .. } = &self.link_context;
            let base = &self.plain_styles;
            let mut styles: Vec<Rc<LineStyle>> = (*self.base_styles.1).clone();
            let lines = markdown::lines(text);
            let i = line_of_offset(text, cursor);
            // The block or line the cursor is in shows its source, not its figure.
            let block = self
                .figure_lines
                .iter()
                .find(|r| r.contains(&i))
                .cloned()
                .unwrap_or(i..i + 1);
            for j in block {
                if let (Some(slot), Some(plain)) = (styles.get_mut(j), base.get(j)) {
                    *slot = plain.clone();
                }
            }
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
                        Some(Decoration::Image(_) | Decoration::Callout { .. }) => {
                            style.decoration = slot.decoration.clone()
                        }
                        _ => {}
                    }
                    // A setext Heading is one through the line under it.
                    if style.heading.is_none() {
                        style.heading = slot.heading;
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
                // In the Properties table: the whole Frontmatter shows as YAML.
                Some(line)
                    if matches!(
                        line.kind,
                        BlockKind::Frontmatter | BlockKind::FrontmatterDelimiter
                    ) =>
                {
                    for l in lines.iter().take_while(|l| {
                        matches!(
                            l.kind,
                            BlockKind::Frontmatter | BlockKind::FrontmatterDelimiter
                        )
                    }) {
                        let at = if l.index == i { cursor - l.start } else { 0 };
                        restyle(l, at);
                    }
                }
                Some(line) => restyle(line, cursor - line.start),
                None => {}
            }
            self.styles = (Some(key), Rc::new(styles));
        }
        self.styles.1.clone()
    }

    /// The base styles with each closed `$$` block and `mermaid` fence drawn
    /// as its figure on its first line (the others folded), and each formula of
    /// a line drawn in its text. A formula RaTeX cannot typeset, or a diagram
    /// not drawn yet (drawn in the background, then the styles again), stays
    /// as written.
    fn with_figures(&mut self, plain: &[Rc<LineStyle>], cx: &mut Context<Self>) -> Vec<LineStyle> {
        let text = self.buf.text();
        let lines = markdown::lines(text);
        let cursor_line = line_of_offset(text, self.buf.cursor());
        let dark = self.palette.dark;
        // Only the formulas of this text are kept: none of every key typed.
        let mut formulas = std::mem::take(&mut self.formulas);
        let mut used = HashMap::new();
        let mut typeset_once = |tex: &str, display: bool| {
            let key = (tex.to_string(), display);
            let f = formulas
                .remove(&key)
                .or_else(|| used.get(&key).cloned())
                .unwrap_or_else(|| typeset(tex, display).map(Rc::new));
            used.insert(key, f.clone());
            f
        };
        self.deferred_diagram = None;
        let mut out: Vec<LineStyle> = plain.iter().map(|s| (**s).clone()).collect();
        let mut figure_lines = Vec::new();
        let mut to_draw = Vec::new();
        for block in figure_blocks(&lines) {
            let decoration = match &block.source {
                FigureSource::Math(tex) => typeset_once(tex, true).map(Decoration::Formula),
                FigureSource::Mermaid(source) => {
                    let id = diagram::diagram_id(source, dark);
                    match self.diagrams.get(&id) {
                        Some(Some(d)) => Some(Decoration::Diagram(d.clone())),
                        Some(None) => None,
                        None if block.lines.contains(&cursor_line) => {
                            self.deferred_diagram = Some(block.lines.clone());
                            None
                        }
                        None => {
                            self.diagrams.insert(id, None);
                            to_draw.push((id, source.clone()));
                            None
                        }
                    }
                }
            };
            let Some(decoration) = decoration else {
                continue;
            };
            for j in block.lines.clone() {
                out[j] = LineStyle {
                    decoration: Some(if j == block.lines.start {
                        decoration.clone()
                    } else {
                        Decoration::Folded
                    }),
                    hidden: vec![0..lines[j].text.len()],
                    ..LineStyle::default()
                };
            }
            figure_lines.push(block.lines);
        }
        for line in &lines {
            let style = &mut out[line.index];
            let drawn_apart = matches!(
                style.decoration,
                Some(
                    Decoration::Table(_)
                        | Decoration::Property(_)
                        | Decoration::Code(_)
                        | Decoration::Image(_)
                        | Decoration::Formula(_)
                        | Decoration::Diagram(_)
                        | Decoration::Folded
                )
            );
            if line.kind != BlockKind::Text || drawn_apart || !line.text.contains('$') {
                continue;
            }
            for (range, inner, display) in inline_formulas(line.text) {
                if let Some(f) = typeset_once(&line.text[inner], display) {
                    hide(&mut style.hidden, range.clone());
                    style.formulas.push((range, f));
                } else {
                    // As written, in red.
                    style
                        .segments
                        .push((range, vec![markdown::MdSpanKind::MathError]));
                }
            }
        }
        self.formulas = used;
        self.figure_lines = figure_lines;
        for (id, source) in to_draw {
            cx.spawn(async move |this, cx| {
                let drawn = cx
                    .background_executor()
                    .spawn(async move { diagram::render(&source, dark) })
                    .await;
                let _ = this.update(cx, |ed, cx| {
                    let d = drawn.map(|(svg, width, height)| {
                        let image = gpui_kit::Image::from_bytes(
                            gpui_kit::ImageFormat::Svg,
                            svg.into_bytes(),
                        );
                        ed.diagram_images.insert(id, Arc::new(image));
                        Rc::new(Diagram { id, width, height })
                    });
                    ed.diagrams.insert(id, d);
                    // The styles again, with the diagram.
                    ed.base_styles.0 = None;
                    ed.styles.0 = None;
                    cx.notify();
                });
            })
            .detach();
        }
        out
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
        // The `[[` list starts at its first Note again as the name changes.
        self.completion_pick = 0;
        self.wake(cx);
        if changed {
            self.revision += 1;
            self.typed_at = Some((self.revision, self.buf.cursor()));
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
                    new_tab: ev.modifiers.secondary(cx),
                });
                return;
            }
            // A footnote goes to its definition, a definition to its reference
            // (unless the cursor is in it: then the click only moves it).
            let c = self.buf.cursor();
            let editing = markdown::footnote_target(self.buf.text(), c).is_some()
                && markdown::footnote_target(self.buf.text(), offset)
                    == markdown::footnote_target(self.buf.text(), c);
            if let Some(target) = markdown::footnote_target(self.buf.text(), offset) {
                if !editing {
                    self.buf.move_to(target, false);
                    self.goal_x = None;
                    self.wake(cx);
                    return;
                }
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
    Newline => |ed, cx| if !ed.accept_completion(cx) { ed.newline(cx) };
    SoftNewline => |ed, cx| if !ed.accept_completion(cx) { ed.replace_selection("\n", cx) };
    TabKey => |ed, cx| if !ed.accept_completion(cx) { ed.replace_selection("\t", cx) };
    Left => |ed, cx| ed.motion(Motion::Left, false, cx);
    Right => |ed, cx| ed.motion(Motion::Right, false, cx);
    Up => |ed, cx| if !ed.completion_move(-1, cx) { ed.vertical(true, false, cx) };
    Down => |ed, cx| if !ed.completion_move(1, cx) { ed.vertical(false, false, cx) };
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
    MakeLink => |ed, cx| ed.format(Format::Link, cx);
    ToggleComment => |ed, cx| ed.format(Format::Comment, cx);
    Indent => |ed, cx| ed.format(Format::Lines(LineFormat::Indent), cx);
    Outdent => |ed, cx| ed.format(Format::Lines(LineFormat::Outdent), cx);
    SelectNextOccurrence => |ed, cx| { ed.buf.select_next_occurrence(); ed.wake(cx) };
    SingleSelection => |ed, cx| {
        if ed.dismiss_completion(cx) {
            return;
        }
        let main = ed.buf.selection();
        ed.buf.set_selection(main.anchor, main.head);
        ed.wake(cx)
    };
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
        let Some(item) = cx.read_from_clipboard() else { return };
        // An image (a screenshot): saved next to the Note, embedded at the cursor.
        let image = item.entries().iter().find_map(|e| match e {
            gpui_kit::ClipboardEntry::Image(img) => Some(img.clone()),
            _ => None,
        });
        if let (Some(img), None) = (image, item.text()) {
            cx.emit(NoteEditorEvent::PasteImage {
                bytes: img.bytes.clone(),
                extension: image_extension(img.format),
            });
        } else if let Some(text) = item.text() {
            ed.replace_selection(&text, cx);
        }
    };
    // The text alone, never an image.
    PasteAndMatchStyle => |ed, cx| {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            ed.replace_selection(&text, cx);
        }
    };
}

impl Render for NoteEditor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let find_bar = self.render_find_bar(cx);
        let completion = self.render_completion(cx);
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
            .on_drop(
                cx.listener(|ed, paths: &gpui_kit::ExternalPaths, window, cx| {
                    // The cursor where the files were dropped, then the app copies them.
                    if let Some(layout) = &ed.layout {
                        let at = layout.offset_for_point(ed.buf.text(), window.mouse_position());
                        ed.buf.move_to(at, false);
                    }
                    cx.emit(NoteEditorEvent::DropFiles(paths.paths().to_vec()));
                }),
            )
            .on_action(cx.listener(|ed, _: &OpenFind, window, cx| ed.open_find(window, cx)))
            .on_action(cx.listener(|ed, _: &OpenReplace, window, cx| ed.open_replace(window, cx)))
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
                    // A character is half an em (SPEC › Window).
                    .when_some(self.line_limit, |d, chars| {
                        d.max_w(self.font_size * (chars as f32 * 0.5))
                    })
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
            .children(completion)
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
/// Hides `range` too, the hidden ranges kept sorted and disjoint.
fn hide(hidden: &mut Vec<Range<usize>>, range: Range<usize>) {
    hidden.push(range);
    hidden.sort_by_key(|r| r.start);
    let mut merged: Vec<Range<usize>> = Vec::with_capacity(hidden.len());
    for r in hidden.drain(..) {
        match merged.last_mut() {
            Some(last) if r.start <= last.end => last.end = last.end.max(r.end),
            _ => merged.push(r),
        }
    }
    *hidden = merged;
}

fn keep_equal(prev: &[Rc<LineStyle>], new: Vec<LineStyle>) -> Vec<Rc<LineStyle>> {
    new.into_iter()
        .enumerate()
        .map(|(i, s)| match prev.get(i) {
            Some(p) if **p == s => p.clone(),
            _ => Rc::new(s),
        })
        .collect()
}

fn image_extension(format: gpui_kit::ImageFormat) -> &'static str {
    use gpui_kit::ImageFormat::*;
    match format {
        Png => "png",
        Jpeg => "jpg",
        Webp => "webp",
        Gif => "gif",
        Svg => "svg",
        Bmp => "bmp",
        Tiff => "tiff",
        _ => "png",
    }
}
