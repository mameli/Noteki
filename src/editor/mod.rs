//! Noteki's editor element: draws the text of a Note line by line, each line
//! shaped and measured on its own so that lines may have different heights,
//! and takes keyboard, mouse and native text input (accents, dead keys,
//! input methods, dictation) through GPUI's `EntityInputHandler`.
//!
//! The rules (edits, selection, movement, line geometry) are in
//! `crate::core::editor` and tested there; this module only measures, draws
//! and dispatches.
//!
//! Here `NoteEditor`, its state and the interface the app uses. By subject:
//! `styles` (Live preview's styles per line), `editing` (moving and deleting),
//! `mouse`, `actions` (keys and menus bound), `input_handler` (the platform's
//! text input), `element` (laying out and painting), `find_bar`,
//! `link_completion`, `spelling`, `math`, `clipboard`.

mod actions;
mod clipboard;
mod editing;
mod element;
mod find_bar;
mod input_handler;
mod link_completion;
mod math;
mod mouse;
pub mod spelling;
mod style;
mod styles;

use mouse::*;

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

use gpui_kit::component::menu::ContextMenuExt as _;
use gpui_kit::Entity;

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
    style_line, CodeColours, CodePart, Context as StyleContext, Decoration, Image, LineStyle,
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
/// The caret blinks this many times after the last key or click, then stays
/// on (as GTK's after 10 seconds): an idle window draws nothing, and macOS
/// gives back the 90 MB it keeps for a window that draws all the time.
const BLINKS_AFTER_INPUT: u8 = 20;
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
    /// Blinks to go before the caret stays on (`BLINKS_AFTER_INPUT`).
    blinks_left: u8,
    /// Scroll the cursor into view at the next frame (keys and edits, not clicks).
    reveal: bool,
    /// The pointer is over a link that a click would open.
    over_clickable: bool,
    /// The text's size, from the settings (⌘+, ⌘−, ⌘0).
    font_size: Pixels,
    /// The text's and the code's fonts, and the longest line in characters,
    /// from the settings.
    font_family: Option<SharedString>,
    code_font: SharedString,
    line_limit: Option<u32>,
    /// View › Focus Mode and Typewriter Mode, from the settings.
    focus_mode: bool,
    typewriter_mode: bool,
    /// Spelling checked while typing, and corrected, from the settings.
    check_spelling: bool,
    correct_spelling: bool,
    /// The misspelled words each line gave, and those marked in the last
    /// frame with the `(revision, cursor)` they were found at.
    spell_checked: crate::core::editor::spelling::Checked,
    spell_marks: (Option<(u64, usize, bool)>, Words),
    /// Where the last right click was, for its menu (a misspelled word's guesses).
    context_at: Option<usize>,
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
    /// The styles of the Note's blocks, kept from one keystroke to the next.
    block_styles: crate::core::editor::styling::BlockStyles,
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
                if !ed.focus_handle.is_focused(window) {
                    return;
                }
                if ed.blinks_left > 0 {
                    ed.blinks_left -= 1;
                    ed.cursor_on = !ed.cursor_on;
                    cx.notify();
                } else if !ed.cursor_on {
                    ed.cursor_on = true;
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
            blinks_left: BLINKS_AFTER_INPUT,
            reveal: false,
            over_clickable: false,
            find: None,
            completion_pick: 0,
            completion_dismissed: None,
            typed_at: None,
            font_size: px(15.),
            font_family: None,
            code_font: element::CODE_FONT.into(),
            line_limit: Some(100),
            focus_mode: false,
            typewriter_mode: false,
            check_spelling: false,
            correct_spelling: false,
            spell_checked: Default::default(),
            spell_marks: (None, Rc::default()),
            context_at: None,
            note_key: None,
            histories: HashMap::new(),
            revision: 0,
            link_context: LinkContext::default(),
            images: HashMap::new(),
            formulas: HashMap::new(),
            diagrams: HashMap::new(),
            diagram_images: HashMap::new(),
            plain_styles: Rc::default(),
            block_styles: Default::default(),
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

    /// Whether the caret is drawn now (it blinks for a while after input).
    pub fn caret_shown(&self) -> bool {
        self.cursor_on
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
        // Wikilinks resolve, and images are found, against the new context.
        self.block_styles.clear();
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

    /// The caret at `offset` as it is drawn (for tests).
    pub fn drawn_caret_bounds(&self, offset: usize) -> Option<Bounds<Pixels>> {
        self.layout.as_ref()?.drawn_caret(self.buf.text(), offset)
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

    /// Whether the pointer is the pointing hand: over a link or a checkbox.
    pub fn shows_pointing_hand(&self) -> bool {
        self.over_clickable
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

    /// Focus Mode fades every line but the cursor's paragraph; Typewriter Mode
    /// keeps the cursor's line in the middle of the view after keys (not
    /// clicks), with half a view of room before and after the text.
    pub fn set_modes(&mut self, focus: bool, typewriter: bool, cx: &mut Context<Self>) {
        if (focus, typewriter) != (self.focus_mode, self.typewriter_mode) {
            self.focus_mode = focus;
            self.typewriter_mode = typewriter;
            self.reveal = true;
            cx.notify();
        }
    }

    /// Spelling checked while typing (misspelled words underlined) and
    /// corrected as a word ends (SPEC › Window).
    pub fn set_spelling(&mut self, check: bool, correct: bool, cx: &mut Context<Self>) {
        if (check, correct) != (self.check_spelling, self.correct_spelling) {
            self.check_spelling = check;
            self.correct_spelling = correct;
            self.spell_marks.0 = None;
            cx.notify();
        }
    }

    /// The misspelled words underlined: of the prose, not the one being typed.
    pub fn misspelled(&mut self) -> Rc<Vec<Range<usize>>> {
        let text = self.buf.text();
        if !self.check_spelling || !spelling::AVAILABLE || text.len() > LIVE_PREVIEW_MAX_BYTES {
            return Rc::default();
        }
        let at = (self.revision, self.buf.cursor());
        // Typing: a key came and the caret has not blinked since. Its line is
        // checked at that first blink, half a second into a pause.
        let typing = self.typed_at == Some(at) && self.blinks_left == BLINKS_AFTER_INPUT;
        let key = (at.0, at.1, typing);
        if self.spell_marks.0 != Some(key) {
            // The word just typed is not marked yet; one the cursor was only
            // moved to (a click, the arrows, a right-click) is.
            let cursor = (self.buf.selection().is_empty() && self.typed_at == Some(at))
                .then(|| self.buf.cursor());
            let found = crate::core::editor::spelling::misspelled(
                text,
                cursor,
                typing,
                &mut self.spell_checked,
                spelling::misspelled,
            );
            self.spell_marks = (Some(key), Rc::new(found));
        }
        self.spell_marks.1.clone()
    }

    /// The misspelled word at `offset`, if one is marked there.
    fn misspelled_at(&mut self, offset: usize) -> Option<Range<usize>> {
        self.misspelled()
            .iter()
            .find(|r| r.start <= offset && offset <= r.end)
            .cloned()
    }

    /// Corrects the word just ended by `typed` (a space or a punctuation
    /// mark), as the Mac's text views do; one undo step puts it back.
    fn correct_word_before(&mut self, typed: &str, cx: &mut Context<Self>) {
        use crate::core::editor::spelling::word_before;
        if !self.correct_spelling
            || !self.check_spelling
            || !spelling::AVAILABLE
            || !matches!(typed, " " | "." | "," | ";" | ":" | "!" | "?" | ")")
            || !self.buf.selection().is_empty()
        {
            return;
        }
        let end = self.buf.cursor() - typed.len();
        let text = self.buf.text();
        let Some(word) = word_before(text, end) else {
            return;
        };
        if crate::core::paste::pastes_plain_at(text, word.start) {
            return;
        }
        // Only a word the checker marks, in the language of its paragraph.
        let line = text[..word.start].rfind('\n').map_or(0, |i| i + 1);
        let line_end = text[word.end..]
            .find('\n')
            .map_or(text.len(), |i| word.end + i);
        let paragraph = &text[line..line_end];
        let local = word.start - line..word.end - line;
        if !crate::core::editor::spelling::misspelled(
            paragraph,
            None,
            false,
            &mut Default::default(),
            spelling::misspelled,
        )
        .contains(&local)
        {
            return;
        }
        let Some(fix) = spelling::correction(paragraph, local) else {
            return;
        };
        let cursor = self.buf.cursor();
        self.buf.close_undo_step();
        let changed = self.buf.replace(word.clone(), &fix);
        let moved = cursor + fix.len() - word.len();
        self.buf.move_to(moved, false);
        self.buf.close_undo_step();
        self.edited(changed, cx);
    }

    /// A right click: the cursor goes there unless it is in the selection, as
    /// on the Mac, and its menu (`context_menu`) knows the word under it.
    fn on_right_click(&mut self, ev: &MouseDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(layout) = &self.layout else {
            return;
        };
        let at = layout.offset_for_point(self.buf.text(), ev.position);
        if !self.buf.selection().range().contains(&at) {
            self.buf.move_to(at, false);
        }
        self.context_at = Some(at);
        self.wake(cx);
        self.reveal = false;
    }

    /// A word was learned or ignored: every line is checked again.
    fn forget_spelling(&mut self, cx: &mut Context<Self>) {
        self.spell_checked.clear();
        self.spell_marks.0 = None;
        cx.notify();
    }

    /// A guess chosen in the menu of a misspelled word: it takes the word's place.
    fn replace_word(&mut self, word: Range<usize>, with: &str, cx: &mut Context<Self>) {
        self.buf.close_undo_step();
        let changed = self.buf.replace(word.clone(), with);
        self.buf.move_to(word.start + with.len(), false);
        self.buf.close_undo_step();
        self.edited(changed, cx);
    }

    pub fn modes(&self) -> (bool, bool) {
        (self.focus_mode, self.typewriter_mode)
    }

    /// The view the text scrolls in, in the window (for tests).
    pub fn viewport(&self) -> Bounds<Pixels> {
        self.scroll.bounds()
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
}

/// Ranges of words of the text, shared with the frame that draws them.
type Words = Rc<Vec<Range<usize>>>;

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
            .cursor(if self.over_clickable {
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
        // Typewriter Mode: the first and last lines can come to the middle too.
        let room = if self.typewriter_mode {
            self.scroll.bounds().size.height / 2.
        } else {
            px(16.)
        };
        let root = root.on_mouse_down(MouseButton::Right, cx.listener(Self::on_right_click));
        let editor = self.bind_actions(root, cx).child(
            div().w_full().px_6().pt(room).pb(room).child(
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
        let entity = cx.entity();
        let editor = editor.context_menu(move |menu, _, cx| context_menu(&entity, menu, cx));
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
