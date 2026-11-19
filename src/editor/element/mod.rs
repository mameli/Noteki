//! The element that measures and paints a [`NoteEditor`]: every logical line
//! shaped on its own (at its own size, wrapped to the width), stacked by
//! [`LineTops`], never by `line_height * row`.
//!
//! Here the element and what it lays out (`Layout`, `LaidLine`, hit testing);
//! `layout` shapes the lines, `cells` lays out tables and the Properties,
//! `paint` draws decorations and figures.

use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Range;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;

use gpui_kit::{
    fill, point, px, size, App, AvailableSpace, Bounds, ContentMask, Corners, DispatchPhase,
    Element, ElementId, ElementInputHandler, Entity, Font, FontWeight, GlobalElementId,
    HighlightStyle, Hsla, ImageAssetLoader, InspectorElementId, IntoElement, LayoutId,
    MouseMoveEvent, MouseUpEvent, Pixels, Point, Resource, SharedString, Size, Style, TextAlign,
    TextRun, UnderlineStyle, Window, WindowTextSystem, WrappedLine,
};

use crate::core::diagram::Diagram;
use crate::core::editor::display::{
    shown_text, shown_text_with, to_buffer, to_buffer_with, to_display, to_display_with, Insert,
};
use crate::core::editor::lines::{logical_lines, scroll_to_reveal, LineTops};
use crate::core::editor::movement::prev_grapheme;
use crate::core::editor::properties::PropertyLine;
use crate::core::editor::styling::{CalloutColor, CodePart, Decoration, LineStyle};
use crate::core::editor::table::{Align, RowKind, Table, TableRow};
use crate::core::markdown::{ListKind, MdSpanKind};
use crate::theme::NotekiPalette;

use super::math::{MathPaint, KATEX_SCALE};
use super::style::span_style;

use super::{LineStyles, NoteEditor};

/// Text line height as a multiple of the font size.
const LINE_HEIGHT: f32 = 1.6;
/// Heading 1 to 6 font sizes as multiples of the text's, as Obsidian's default theme.
const HEADING_SCALE: [f32; 6] = [1.802, 1.602, 1.424, 1.266, 1.125, 1.0];
/// A Heading's line height as a multiple of its own font size: tighter than text.
const HEADING_LINE_HEIGHT: f32 = 1.35;
const CARET_WIDTH: Pixels = px(2.);
/// How far a quote's text is indented per level, its bar at the start of it.
const QUOTE_INDENT: Pixels = px(16.);
const QUOTE_BAR: Pixels = px(3.);
/// A list item's text is indented by its level, plus room for its marker.
const LIST_INDENT: Pixels = px(20.);
const LIST_MARKER: Pixels = px(24.);
const CHECKBOX: Pixels = px(14.);
/// A code block's text is this far from its box's edges.
const CODE_PAD: Pixels = px(12.);
const CODE_RADIUS: Pixels = px(6.);
/// The code's font when the settings name none: Menlo on the Mac, DejaVu
/// Sans Mono (on nearly every Linux) elsewhere.
pub(super) const CODE_FONT: &str = if cfg!(target_os = "macos") {
    "Menlo"
} else {
    "DejaVu Sans Mono"
};
/// A table cell's text is this far from its column's edges.
const CELL_PAD: Pixels = px(8.);
/// A column is never wider than about 40 characters (ADR 0008), in ems.
const MAX_COLUMN_EMS: f32 = 22.;
/// The Properties table's key column, in ems.
const PROPERTY_KEY_EMS: f32 = 9.;
/// A table's delimiter row, drawn as the line under its header.
const DELIMITER_ROW: Pixels = px(2.);

mod cells;
mod layout;
mod paint;

use cells::*;
use layout::*;
use paint::*;

#[derive(Clone)]
pub(super) struct LaidLine {
    /// The line's bytes in the text, without its line break.
    range: Range<usize>,
    line_height: Pixels,
    /// What of the line is not drawn, relative to its start (its display map).
    hidden: Rc<[Range<usize>]>,
    /// The line as shown: without its hidden ranges.
    shaped: Rc<WrappedLine>,
    /// How far the text starts from the left edge (a quote's bars).
    indent: Pixels,
    decoration: Option<Decoration>,
    /// The height of the text's rows: none for an image whose source is hidden.
    text_height: Pixels,
    /// An image, formula or diagram drawn under the text, at the size it takes.
    figure: Option<(Figure, Size<Pixels>)>,
    /// The room each formula drawn in the text takes in its place, and the formulas.
    inserts: Rc<[Insert]>,
    formulas: Rc<[InlineFormula]>,
    /// A table row's cells, each at its column (the line's own text is empty).
    cells: Option<Rc<[Cell]>>,
    /// A table's column edges, from the left: for the grid.
    columns: Option<Rc<[Pixels]>>,
}

/// What a line draws under its text in place of its hidden source.
#[derive(Clone)]
pub(super) enum Figure {
    /// An image of the Vault, at the left.
    File(Rc<Path>),
    /// A `$$` block's formula, centred, a little room above and under.
    Formula(Rc<MathPaint>),
    /// A `mermaid` block's diagram, centred.
    Diagram(Rc<Diagram>),
    /// A Properties list drawn on its key's row: each item a chip, at its
    /// place from the line's top-left, its width and text.
    Chips(Rc<[(Point<Pixels>, Pixels, gpui_kit::ShapedLine)]>),
}

/// A formula drawn in a line's text, over the room its insert takes.
#[derive(Clone)]
pub(super) struct InlineFormula {
    /// Where its room starts in the shown text, and how wide it is.
    at: usize,
    room: Pixels,
    paint: Rc<MathPaint>,
}

/// One table cell as drawn: its text in the line, its place, its shaped text.
pub(super) struct Cell {
    /// Its text in the line, relative to the line's start.
    range: Range<usize>,
    /// Where its text starts (alignment included), and how much room it has
    /// from there before its column's edge.
    x: Pixels,
    room: Pixels,
    /// What of its text is hidden (marks), relative to the cell's start.
    hidden: Vec<Range<usize>>,
    /// One row, cut with an ellipsis; in the row being edited, wrapped in its column.
    shaped: WrappedLine,
}

impl Cell {
    /// Where `local` (in the line) is, from the cell's row's top-left.
    fn position(&self, local: usize, line_height: Pixels) -> Point<Pixels> {
        let i = local.clamp(self.range.start, self.range.end) - self.range.start;
        let shown = to_display(&self.hidden, i);
        let s = &self.shaped;
        // At a wrap, the start of the next row (as in the text's own lines).
        let wrapped_at = s
            .wrap_boundaries()
            .iter()
            .position(|b| s.runs()[b.run_ix].glyphs[b.glyph_ix].index == shown);
        let p = match wrapped_at {
            Some(row) => point(px(0.), line_height * (row + 1) as f32),
            None => s
                .position_for_index(shown, line_height)
                .unwrap_or_else(|| point(s.width(), px(0.))),
        };
        point(self.x + p.x, p.y)
    }

    fn rows(&self) -> usize {
        self.shaped.wrap_boundaries().len() + 1
    }
}

impl LaidLine {
    fn height(&self) -> Pixels {
        self.text_height + self.figure.as_ref().map_or(px(0.), |(_, s)| s.height)
    }

    /// The position of `offset` (clamped to the line) relative to the line's top-left.
    fn position(&self, offset: usize) -> Point<Pixels> {
        let local = offset.clamp(self.range.start, self.range.end) - self.range.start;
        if let Some(cells) = &self.cells {
            // In a cell, or the next cell's start (a pipe, a space).
            return cells
                .iter()
                .find(|c| local <= c.range.end)
                .or(cells.last())
                .map_or(point(px(0.), px(0.)), |c| {
                    c.position(local, self.line_height)
                });
        }
        let shown = to_display_with(&self.hidden, &self.inserts, local);
        // Where a line wraps, the caret is at the start of the next row (GPUI
        // gives the end of the row before), as in every macOS text view.
        let wrapped_at = self
            .shaped
            .wrap_boundaries()
            .iter()
            .position(|b| self.shaped.runs()[b.run_ix].glyphs[b.glyph_ix].index == shown);
        let p = match wrapped_at {
            Some(row) => point(px(0.), self.line_height * (row + 1) as f32),
            None => self
                .shaped
                .position_for_index(shown, self.line_height)
                .unwrap_or_else(|| point(self.shaped.width(), px(0.))),
        };
        point(p.x + self.indent, p.y)
    }
}

/// Where each line of the text was drawn in the last frame, in window coordinates.
#[derive(Clone)]
pub(super) struct Layout {
    origin: Point<Pixels>,
    width: Pixels,
    lines: Rc<Vec<LaidLine>>,
    tops: Rc<LineTops>,
}

impl Layout {
    fn top_of(&self, line: usize) -> Pixels {
        px(self.tops.top(line))
    }

    pub(super) fn line_heights(&self) -> Vec<f32> {
        self.lines.iter().map(|l| f32::from(l.height())).collect()
    }

    pub(super) fn text_top(&self) -> Pixels {
        self.origin.y
    }

    pub(super) fn text_bottom(&self) -> Pixels {
        self.origin.y + px(self.tops.total())
    }

    fn line_of(&self, offset: usize) -> usize {
        self.lines
            .partition_point(|l| l.range.start <= offset)
            .saturating_sub(1)
    }

    /// In a table's row, ↑ from its cell's first row and ↓ from its last go to
    /// the row above or below (its y), not to another cell's wrapped rows in
    /// the same row: `None` elsewhere, or within a wrapped cell.
    pub(super) fn past_cell(&self, offset: usize, up: bool) -> Option<Pixels> {
        let i = self.line_of(offset);
        let line = self.lines.get(i)?;
        let cells = line.cells.as_ref()?;
        let local = offset.clamp(line.range.start, line.range.end) - line.range.start;
        let cell = cells
            .iter()
            .find(|c| local <= c.range.end)
            .or(cells.last())?;
        let row = (f32::from(cell.position(local, line.line_height).y)
            / f32::from(line.line_height))
        .round() as usize;
        let top = self.origin.y + self.top_of(i);
        if up && row == 0 {
            Some(top - px(1.))
        } else if !up && row + 1 >= cell.rows() {
            Some(top + line.height() + px(1.))
        } else {
            None
        }
    }

    /// A table's delimiter row, drawn as a rule: ↑ and ↓ go over it, to the
    /// row past it (its y, above for `up`), as over the line it is.
    pub(super) fn past_rule(&self, offset: usize, up: bool) -> Option<Pixels> {
        let i = self.line_of(offset);
        let Some(Decoration::Table(row)) = &self.lines.get(i)?.decoration else {
            return None;
        };
        if row.kind != RowKind::Delimiter {
            return None;
        }
        let top = self.origin.y + self.top_of(i);
        Some(if up {
            top - px(1.)
        } else {
            top + self.lines[i].height() + px(1.)
        })
    }

    /// The caret's rectangle at `offset`, as tall as its row. `None` when the
    /// layout is of another text (the text changed since the last frame).
    pub(super) fn caret(&self, text: &str, offset: usize) -> Option<Bounds<Pixels>> {
        if self.lines.last()?.range.end != text.len() {
            return None;
        }
        let i = self.line_of(offset);
        let line = &self.lines[i];
        let p = line.position(offset);
        Some(Bounds::new(
            point(self.origin.x + p.x, self.origin.y + self.top_of(i) + p.y),
            size(CARET_WIDTH, line.line_height),
        ))
    }

    /// The caret as drawn: as tall as the letters (their ascent and descent),
    /// centred in its row like them, not the whole row with its spacing.
    pub(super) fn drawn_caret(&self, text: &str, offset: usize) -> Option<Bounds<Pixels>> {
        let row = self.caret(text, offset)?;
        let line = &self.lines[self.line_of(offset)];
        let font = &line.shaped.unwrapped_layout;
        let letters = font.ascent + font.descent;
        if letters <= px(0.) || letters >= line.line_height {
            return Some(row);
        }
        Some(Bounds::new(
            point(row.left(), row.top() + (line.line_height - letters) / 2.),
            size(CARET_WIDTH, letters),
        ))
    }

    /// The offset under `p`: above the text, its start; below it, its end.
    pub(super) fn offset_for_point(&self, text: &str, p: Point<Pixels>) -> usize {
        let Some(last) = self.lines.last() else {
            return 0;
        };
        if last.range.end != text.len() {
            return text.len(); // a layout of an older text: nothing better to give
        }
        let rel = p - self.origin;
        if rel.y < px(0.) {
            return 0;
        }
        if rel.y >= px(self.tops.total()) {
            return text.len();
        }
        let i = self.tops.line_at(f32::from(rel.y));
        let line = &self.lines[i];
        if let Some(cells) = &line.cells {
            // The cell whose column holds `x` (by the column's edges: a cell
            // aligned right or centred starts its text further in), else the nearest.
            let cell = match &line.columns {
                Some(edges) => {
                    let column = edges[1..].iter().take_while(|&&e| e <= rel.x).count();
                    cells.get(column.min(cells.len().saturating_sub(1)))
                }
                None => cells
                    .iter()
                    .rev()
                    .find(|c| rel.x >= c.x - CELL_PAD)
                    .or(cells.first()),
            };
            let y = (rel.y - self.top_of(i)).clamp(px(0.), line.text_height - px(0.5));
            let local = cell.map_or(0, |c| {
                let at = point((rel.x - c.x).max(px(0.)), y);
                let shown = match c.shaped.closest_index_for_position(at, line.line_height) {
                    Ok(i) | Err(i) => i,
                };
                c.range.start + to_buffer(&c.hidden, shown)
            });
            return (line.range.start + local).min(line.range.end);
        }
        let y = (rel.y - self.top_of(i)).clamp(px(0.), line.height() - px(0.5));
        let shown = match line.shaped.closest_index_for_position(
            point((rel.x - line.indent).max(px(0.)), y),
            line.line_height,
        ) {
            Ok(i) | Err(i) => i,
        };
        let local = to_buffer_with(&line.hidden, &line.inserts, shown);
        (line.range.start + local).min(line.range.end)
    }

    /// The task whose checkbox is under `p`: the offset of its `[` and whether
    /// it is checked.
    pub(super) fn checkbox_at(&self, p: Point<Pixels>) -> Option<(usize, bool)> {
        let rel_y = f32::from(p.y - self.origin.y);
        if rel_y < 0. || rel_y >= self.tops.total() {
            return None;
        }
        let i = self.tops.line_at(rel_y);
        let line = &self.lines[i];
        let Some(Decoration::List {
            level,
            kind: ListKind::Task { checked, bracket },
        }) = line.decoration.clone()
        else {
            return None;
        };
        let x = self.origin.x + LIST_INDENT * f32::from(level);
        let mid = self.origin.y + self.top_of(i) + line.line_height / 2.;
        // A little room around the box, as small targets are hard to hit.
        let hit = checkbox_bounds(x, mid).dilate(px(3.));
        hit.contains(&p)
            .then_some((line.range.start + bracket, checked))
    }

    /// The wrapped row holding `offset`: from its first character to its end,
    /// which for a row that wraps is before the character it wraps at.
    pub(super) fn row_range(&self, text: &str, offset: usize) -> Option<Range<usize>> {
        let caret = self.caret(text, offset)?;
        let y = caret.top() + caret.size.height / 2.;
        let line = &self.lines[self.line_of(offset)];
        let start = self.offset_for_point(text, point(self.origin.x, y));
        let mut end = self.offset_for_point(text, point(self.origin.x + self.width * 2., y));
        if end < line.range.end {
            end = prev_grapheme(text, end).max(start);
        }
        Some(start..end)
    }

    /// The rectangles that show `sel` on line `i`, in window coordinates.
    fn selection_rects(&self, i: usize, sel: &Range<usize>) -> Vec<Bounds<Pixels>> {
        let line = &self.lines[i];
        let r = &line.range;
        // The selection takes in this line's break: paint to the right edge.
        let through_break = sel.start <= r.end && sel.end > r.end;
        let (a, b) = (sel.start.max(r.start), sel.end.min(r.end));
        if a > b || (a == b && !through_break) {
            return Vec::new();
        }
        let lh = line.line_height;
        let (pa, pb) = (line.position(a), line.position(b));
        let top = self.origin.y + self.top_of(i);
        let x0 = self.origin.x;
        let right = if through_break { self.width } else { pb.x };
        let rect = |y: Pixels, from: Pixels, to: Pixels| {
            Bounds::from_corners(
                point(x0 + from, top + y),
                point(x0 + to.max(from), top + y + lh),
            )
        };
        if pa.y == pb.y {
            return vec![rect(pa.y, pa.x, right.max(pa.x + px(4.)))];
        }
        let mut rects = vec![rect(pa.y, pa.x, self.width)];
        let mut y = pa.y + lh;
        while y < pb.y {
            rects.push(rect(y, line.indent, self.width));
            y += lh;
        }
        rects.push(rect(pb.y, line.indent, right));
        rects
    }
}

/// What one frame needs to shape the text.
#[derive(Clone)]
pub(super) struct ShapeInput {
    text: SharedString,
    /// Bumped at every change of the text.
    revision: u64,
    marked: Option<Range<usize>>,
    /// One per logical line; empty when Live preview is off (a very large Note).
    styles: LineStyles,
    palette: NotekiPalette,
    font: Font,
    code_font: SharedString,
    font_size: Pixels,
    cache: Rc<RefCell<ShapeCache>>,
}

/// What a whole frame's lines were shaped for: when nothing of it changed
/// (the cursor blinked, the view scrolled), the lines are reused as they are.
#[derive(Clone, PartialEq)]
struct FrameKey {
    revision: u64,
    styles: *const Vec<Rc<LineStyle>>,
    marked: Option<Range<usize>>,
    width: Pixels,
    look: Look,
}

/// What every line is drawn with: when it changes, every line is shaped again.
#[derive(Clone, PartialEq)]
struct Look {
    font: Font,
    code_font: SharedString,
    font_size: Pixels,
    palette: NotekiPalette,
}

/// What one line was shaped from.
#[derive(Clone, PartialEq)]
struct LineSource {
    text: SharedString,
    style: Rc<LineStyle>,
    /// The composed text inside this line, relative to its start.
    marked: Option<Range<usize>>,
}

/// The lines shaped in earlier frames, kept by the [`NoteEditor`]: a keystroke
/// shapes again only the lines whose text or style changed.
#[derive(Default)]
pub(super) struct ShapeCache {
    frame: Option<FrameKey>,
    lines: Rc<Vec<LaidLine>>,
    tops: Rc<LineTops>,
    sources: Vec<LineSource>,
}

impl ShapeCache {
    pub(super) fn height(&self) -> Pixels {
        px(self.tops.total())
    }
}

/// A Properties list's chips, against the text's size.
const CHIP_SCALE: f32 = 0.88;

/// Room above and under a display formula or a diagram, in ems of the text.
const FORMULA_PAD: f32 = 0.5;

pub(super) struct EditorElement {
    editor: Entity<NoteEditor>,
}

impl EditorElement {
    pub(super) fn new(editor: Entity<NoteEditor>) -> Self {
        Self { editor }
    }
}

impl IntoElement for EditorElement {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

pub(super) struct PrepaintState {
    layout: Layout,
    selection: Vec<Bounds<Pixels>>,
    matches: Vec<Bounds<Pixels>>,
    carets: Vec<Bounds<Pixels>>,
    /// Under each misspelled word, the row it is on.
    misspelled: Vec<Bounds<Pixels>>,
}

impl Element for EditorElement {
    type RequestLayoutState = ShapeInput;
    type PrepaintState = PrepaintState;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let (styles, text) = self
            .editor
            .update(cx, |ed, cx| (ed.line_styles(cx), ed.shared_text()));
        let ed = self.editor.read(cx);
        let style = window.text_style();
        let input = ShapeInput {
            text,
            revision: ed.revision,
            marked: ed.buf.marked(),
            styles,
            palette: ed.palette,
            font: match &ed.font_family {
                Some(family) => Font {
                    family: family.clone(),
                    ..style.font()
                },
                None => style.font(),
            },
            code_font: ed.code_font.clone(),
            font_size: ed.font_size,
            cache: ed.shape_cache.clone(),
        };
        let inp = input.clone();
        let mut layout_style = Style::default();
        layout_style.size.width = gpui_kit::relative(1.).into();
        let id =
            window.request_measured_layout(layout_style, move |known, available, window, _| {
                let width = match (known.width, available.width) {
                    (Some(w), _) | (None, AvailableSpace::Definite(w)) => w,
                    // Asked for a natural size: the text takes any width it gets,
                    // so give the last height rather than shaping for nothing.
                    _ => {
                        return Size {
                            width: px(0.),
                            height: inp.cache.borrow().height(),
                        }
                    }
                };
                let (_, tops) = shape(&inp, width, window.text_system());
                Size {
                    width,
                    height: px(tops.total()),
                }
            });
        (id, input)
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        input: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> PrepaintState {
        let (lines, tops) = shape(input, bounds.size.width, window.text_system());
        let layout = Layout {
            origin: bounds.origin,
            width: bounds.size.width,
            lines,
            tops,
        };
        let ed = self.editor.read(cx);
        // Every selection (⌘D makes several), and a caret in each empty one.
        let mut selection = Vec::new();
        let mut carets = Vec::new();
        let show_caret = ed.cursor_on && ed.focus_handle.is_focused(window);
        for s in ed.buf.selections() {
            let sel = s.range();
            if sel.is_empty() {
                if show_caret {
                    carets.extend(layout.drawn_caret(ed.buf.text(), s.head));
                }
                continue;
            }
            for i in layout.line_of(sel.start)..=layout.line_of(sel.end) {
                selection.extend(layout.selection_rects(i, &sel));
            }
        }
        let matches = self.editor.update(cx, |ed, _| ed.find_matches());
        let mut match_rects = Vec::new();
        for m in &matches {
            for i in layout.line_of(m.start)..=layout.line_of(m.end) {
                match_rects.extend(layout.selection_rects(i, m));
            }
        }
        let matches = match_rects;
        let words = self.editor.update(cx, |ed, _| ed.misspelled());
        let mut misspelled = Vec::new();
        for w in words.iter() {
            for i in layout.line_of(w.start)..=layout.line_of(w.end) {
                misspelled.extend(layout.selection_rects(i, w));
            }
        }
        PrepaintState {
            layout,
            selection,
            matches,
            carets,
            misspelled,
        }
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        input: &mut Self::RequestLayoutState,
        state: &mut PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let (focus, palette, diagrams, (focus_mode, typewriter), cursor) = {
            let ed = self.editor.read(cx);
            (
                ed.focus_handle.clone(),
                ed.palette,
                ed.diagram_images.clone(),
                ed.modes(),
                ed.buf.cursor(),
            )
        };
        window.handle_input(
            &focus,
            ElementInputHandler::new(bounds, self.editor.clone()),
            cx,
        );
        // A drag goes on outside the editor (over the tab bar, past the window's
        // edge): follow the pointer wherever it is, not only over the element.
        if self.editor.read(cx).drag.is_some() {
            let editor = self.editor.clone();
            window.on_mouse_event(move |ev: &MouseMoveEvent, phase, _, cx| {
                if phase == DispatchPhase::Bubble {
                    editor.update(cx, |ed, cx| ed.on_drag_move(ev, cx));
                }
            });
            let editor = self.editor.clone();
            window.on_mouse_event(move |ev: &MouseUpEvent, phase, _, cx| {
                if phase == DispatchPhase::Bubble {
                    editor.update(cx, |ed, cx| ed.on_drag_end(ev, cx));
                }
            });
        }
        let visible = window.content_mask().bounds;
        let layout = &state.layout;
        let on_screen = |i: usize, line: &LaidLine| {
            let top = layout.origin.y + layout.top_of(i);
            (top <= visible.bottom() && top + line.height() >= visible.top()).then_some(top)
        };
        let clip = |cell: &Cell, top: Pixels, line: &LaidLine| ContentMask {
            bounds: Bounds::new(
                point(layout.origin.x + cell.x, top),
                size(cell.room, line.text_height),
            ),
        };
        // Backgrounds first (boxes, rules, inline code), then the selection
        // over them, then the text: selected code shows as selected.
        for (i, line) in layout.lines.iter().enumerate() {
            let Some(top) = on_screen(i, line) else {
                continue;
            };
            paint_decoration(
                line,
                (layout.origin.x, top, layout.width),
                input,
                window,
                cx,
            );
            for cell in line.cells.iter().flat_map(|c| c.iter()) {
                let at = point(layout.origin.x + cell.x, top);
                window.with_content_mask(Some(clip(cell, top, line)), |window| {
                    let _ = cell.shaped.paint_background(
                        at,
                        line.line_height,
                        TextAlign::Left,
                        None,
                        window,
                        cx,
                    );
                });
            }
            let _ = line.shaped.paint_background(
                point(layout.origin.x + line.indent, top),
                line.line_height,
                TextAlign::Left,
                None,
                window,
                cx,
            );
        }
        // Typewriter Mode: a band of the accent behind the row being written.
        if typewriter {
            if let Some(row) = layout.caret(&input.text, cursor) {
                let band = Bounds::new(
                    point(layout.origin.x, row.top()),
                    size(layout.width, row.size.height),
                );
                window.paint_quad(fill(band, palette.accent.opacity(0.08)));
            }
        }
        // Find's matches, under the selection.
        for r in state.matches.drain(..) {
            window.paint_quad(fill(r, palette.highlight));
        }
        for r in state.selection.drain(..) {
            window.paint_quad(fill(r, palette.selection));
        }
        for (i, line) in layout.lines.iter().enumerate() {
            let Some(top) = on_screen(i, line) else {
                continue;
            };
            paint_figure(
                line,
                (layout.origin.x, top, layout.width),
                input,
                &diagrams,
                window,
                cx,
            );
            paint_inline_formulas(line, (layout.origin.x, top), input, window, cx);
            // A cell's text never overflows its column: cut at its edge.
            for cell in line.cells.iter().flat_map(|c| c.iter()) {
                let at = point(layout.origin.x + cell.x, top);
                window.with_content_mask(Some(clip(cell, top, line)), |window| {
                    let _ =
                        cell.shaped
                            .paint(at, line.line_height, TextAlign::Left, None, window, cx);
                });
            }
            let _ = line.shaped.paint(
                point(layout.origin.x + line.indent, top),
                line.line_height,
                TextAlign::Left,
                None,
                window,
                cx,
            );
        }
        // A misspelled word: red dots under its letters, as on the Mac.
        let red = gpui_kit::hsla(0., 0.8, 0.55, 1.);
        for r in state.misspelled.drain(..) {
            if r.bottom() < visible.top() || r.top() > visible.bottom() {
                continue;
            }
            let line_height = r.size.height;
            // The letters' bottom: a row's spacing is shared above and under them.
            let y = r.top() + line_height * 0.89;
            let dot = px(2.);
            let mut x = r.left();
            while x + dot <= r.right() {
                window.paint_quad(
                    gpui_kit::fill(Bounds::new(point(x, y), size(dot, dot)), red)
                        .corner_radii(px(1.)),
                );
                x += dot * 2.;
            }
        }
        // Focus Mode: every line but the cursor's paragraph under a veil of
        // the background.
        if focus_mode
            && layout
                .lines
                .last()
                .is_some_and(|l| l.range.end == input.text.len())
        {
            let text = &input.text;
            let paragraph = crate::core::editor::lines::paragraph_of(
                layout.lines.iter().map(|l| &text[l.range.clone()]),
                layout.line_of(cursor),
            );
            let veil = palette.background.opacity(0.7);
            for (i, line) in layout.lines.iter().enumerate() {
                if paragraph.contains(&i) {
                    continue;
                }
                let Some(top) = on_screen(i, line) else {
                    continue;
                };
                // Wider than the text: quote bars and list markers too.
                let left = layout.origin.x - px(24.);
                let veiled = Bounds::new(
                    point(left, top),
                    size(layout.width + px(48.), line.height()),
                );
                window.paint_quad(fill(veiled, veil));
            }
        }
        for c in state.carets.drain(..) {
            window.paint_quad(fill(c, palette.caret));
        }
        let layout = state.layout.clone();
        let reveal = self.editor.update(cx, |ed, _| {
            ed.layout = Some(layout.clone());
            std::mem::take(&mut ed.reveal).then(|| {
                (
                    ed.scroll.clone(),
                    layout.caret(ed.buf.text(), ed.buf.cursor()),
                )
            })
        });
        if let Some((scroll, Some(caret))) = reveal {
            // The caret in the scrolled content, then the least scroll that shows it.
            let (viewport, offset) = (scroll.bounds(), scroll.offset());
            let scrolled = f32::from(-offset.y);
            let top = f32::from(caret.top() - viewport.top()) + scrolled;
            let height = f32::from(caret.size.height);
            let max = f32::from(scroll.max_offset().y);
            let wanted = if typewriter {
                crate::core::editor::lines::scroll_to_centre(
                    f32::from(viewport.size.height),
                    top,
                    top + height,
                    max,
                )
            } else {
                scroll_to_reveal(
                    scrolled,
                    f32::from(viewport.size.height),
                    top,
                    top + height,
                    height,
                    max,
                )
            };
            if (wanted - scrolled).abs() > 0.5 {
                scroll.set_offset(point(offset.x, px(-wanted)));
                window.refresh();
            }
        }
    }
}

/// For `examples/perf.rs`: shapes `text` as one frame of the editor does and
/// returns its height.
#[doc(hidden)]
pub fn shape_for_bench(
    text: &str,
    font: Font,
    font_size: Pixels,
    width: Pixels,
    ts: &WindowTextSystem,
) -> Pixels {
    let input = ShapeInput {
        text: SharedString::from(text.to_string()),
        revision: 0,
        marked: None,
        cache: Rc::default(),
        styles: Rc::default(),
        palette: NotekiPalette::for_theme(crate::theme::NotekiThemeName::default(), false),
        font,
        code_font: CODE_FONT.into(),
        font_size,
    };
    px(shape(&input, width, ts).1.total())
}
