//! The element that measures and paints a [`NoteEditor`]: every logical line
//! shaped on its own (at its own size, wrapped to the width), stacked by
//! [`LineTops`], never by `line_height * row`.

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
    MouseMoveEvent, MouseUpEvent, Pixels, Point, Resource, ShapedLine, SharedString, Size, Style,
    TextAlign, TextRun, UnderlineStyle, Window, WindowTextSystem, WrappedLine,
};

use crate::core::editor::display::{shown_text, to_buffer, to_display};
use crate::core::editor::lines::{logical_lines, scroll_to_reveal, LineTops};
use crate::core::editor::movement::prev_grapheme;
use crate::core::editor::styling::{CodePart, Decoration, LineStyle};
use crate::core::editor::table::{Align, RowKind, Table, TableRow};
use crate::core::markdown::{ListKind, MdSpanKind};
use crate::theme::NotekiPalette;

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
/// Code is drawn in the system's monospaced font.
const CODE_FONT: &str = "Menlo";
/// A table cell's text is this far from its column's edges.
const CELL_PAD: Pixels = px(8.);
/// A column is never wider than about 40 characters (ADR 0008), in ems.
const MAX_COLUMN_EMS: f32 = 22.;
/// A table's delimiter row, drawn as the line under its header.
const DELIMITER_ROW: Pixels = px(2.);

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
    /// An image drawn under the text, at the size it is drawn.
    image: Option<(Rc<Path>, Size<Pixels>)>,
    /// A table row's cells, each at its column (the line's own text is empty).
    cells: Option<Rc<[Cell]>>,
    /// A table's column edges, from the left: for the grid.
    columns: Option<Rc<[Pixels]>>,
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
    shaped: ShapedLine,
}

impl Cell {
    fn x_for(&self, local: usize) -> Pixels {
        let i = local.clamp(self.range.start, self.range.end) - self.range.start;
        self.x + self.shaped.x_for_index(to_display(&self.hidden, i))
    }
}

impl LaidLine {
    fn height(&self) -> Pixels {
        self.text_height + self.image.as_ref().map_or(px(0.), |(_, s)| s.height)
    }

    /// The position of `offset` (clamped to the line) relative to the line's top-left.
    fn position(&self, offset: usize) -> Point<Pixels> {
        let local = offset.clamp(self.range.start, self.range.end) - self.range.start;
        if let Some(cells) = &self.cells {
            // In a cell, or the next cell's start (a pipe, a space).
            let x = cells
                .iter()
                .find(|c| local <= c.range.end)
                .or(cells.last())
                .map_or(px(0.), |c| c.x_for(local));
            return point(x, px(0.));
        }
        let shown = to_display(&self.hidden, local);
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
            // The cell whose column holds `x`, else the nearest.
            let cell = cells
                .iter()
                .rev()
                .find(|c| rel.x >= c.x - CELL_PAD)
                .or(cells.first());
            let local = cell.map_or(0, |c| {
                let shown = c.shaped.closest_index_for_x(rel.x - c.x);
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
        let local = to_buffer(&line.hidden, shown);
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

/// The lines of `input` wrapped to `width`, reusing what the cache holds.
fn shape(
    input: &ShapeInput,
    width: Pixels,
    ts: &WindowTextSystem,
) -> (Rc<Vec<LaidLine>>, Rc<LineTops>) {
    let look = Look {
        font: input.font.clone(),
        font_size: input.font_size,
        palette: input.palette,
    };
    let key = FrameKey {
        revision: input.revision,
        styles: Rc::as_ptr(&input.styles),
        marked: input.marked.clone(),
        width,
        look: look.clone(),
    };
    let mut cache = input.cache.borrow_mut();
    if cache.frame.as_ref() == Some(&key) {
        return (cache.lines.clone(), cache.tops.clone());
    }
    let same_look = cache
        .frame
        .as_ref()
        .is_some_and(|f| f.look == look && f.width == width);
    let plain = Rc::new(LineStyle::default());
    let mut sources = Vec::with_capacity(cache.sources.len());
    let mut columns = HashMap::new();
    let lines: Vec<LaidLine> = logical_lines(&input.text)
        .into_iter()
        .enumerate()
        .map(|(i, range)| {
            let style = input.styles.get(i).unwrap_or(&plain);
            let marked = input.marked.as_ref().and_then(|m| {
                let (a, b) = (m.start.max(range.start), m.end.min(range.end));
                (a < b).then(|| a - range.start..b - range.start)
            });
            let text = &input.text[range.clone()];
            let reusable = same_look
                && cache.sources.get(i).is_some_and(|src| {
                    src.text.as_ref() == text
                        && (Rc::ptr_eq(&src.style, style) || src.style == *style)
                        && src.marked == marked
                });
            let laid = if reusable {
                LaidLine {
                    range: range.clone(),
                    ..cache.lines[i].clone()
                }
            } else {
                shape_line(input, range.clone(), style, width, ts, &mut columns)
            };
            sources.push(LineSource {
                text: if reusable {
                    cache.sources[i].text.clone()
                } else {
                    SharedString::from(text.to_string())
                },
                style: Rc::clone(style),
                marked,
            });
            laid
        })
        .collect();
    let tops = Rc::new(LineTops::new(lines.iter().map(|l| f32::from(l.height()))));
    let lines = Rc::new(lines);
    *cache = ShapeCache {
        frame: Some(key),
        lines: lines.clone(),
        tops: tops.clone(),
        sources,
    };
    (lines, tops)
}

/// What is painted around a line's text: under it, never changing its height.
fn paint_decoration(
    line: &LaidLine,
    (left, top, width): (Pixels, Pixels, Pixels),
    input: &ShapeInput,
    window: &mut Window,
    cx: &mut App,
) {
    let palette = &input.palette;
    match &line.decoration {
        Some(Decoration::Rule) => {
            let y = top + line.height() / 2.;
            window.paint_quad(fill(
                Bounds::new(point(left, y), size(width, px(1.))),
                palette.separator,
            ));
        }
        Some(Decoration::Quote { depth }) => {
            for d in 0..*depth {
                let x = left + QUOTE_INDENT * f32::from(d);
                window.paint_quad(fill(
                    Bounds::new(point(x, top), size(QUOTE_BAR, line.height())),
                    palette.accent,
                ));
            }
        }
        Some(Decoration::List { level, kind }) => {
            let x = left + LIST_INDENT * f32::from(*level);
            // Centred on the first row.
            let mid = top + line.line_height / 2.;
            match *kind {
                ListKind::Bullet => {
                    let d = px(5.);
                    window.paint_quad(
                        fill(
                            Bounds::new(point(x + px(8.), mid - d / 2.), size(d, d)),
                            palette.syntax,
                        )
                        .corner_radii(d / 2.),
                    );
                }
                ListKind::Number { n, delim } => {
                    let label = format!("{n}{}", delim as char);
                    let run =
                        text_run(label.len(), &input.font, palette.syntax, Default::default());
                    let shaped = window.text_system().shape_line(
                        label.into(),
                        input.font_size,
                        &[run],
                        None,
                    );
                    let at = point(x + LIST_MARKER - shaped.width() - px(6.), top);
                    let _ = shaped.paint(at, line.line_height, TextAlign::Left, None, window, cx);
                }
                ListKind::Task { checked, .. } => {
                    let b = checkbox_bounds(x, mid);
                    let (bg, border) = if checked {
                        (palette.accent, palette.accent)
                    } else {
                        (palette.background, palette.syntax)
                    };
                    window.paint_quad(
                        fill(b, bg)
                            .corner_radii(px(3.))
                            .border_widths(px(1.5))
                            .border_color(border),
                    );
                    if checked {
                        let run = text_run(
                            "✓".len(),
                            &input.font,
                            palette.background,
                            Default::default(),
                        );
                        let shaped = window.text_system().shape_line(
                            "✓".into(),
                            CHECKBOX - px(2.),
                            &[run],
                            None,
                        );
                        let at = point(
                            b.left() + (CHECKBOX - shaped.width()) / 2.,
                            b.top() - px(1.),
                        );
                        let _ = shaped.paint(at, CHECKBOX, TextAlign::Left, None, window, cx);
                    }
                }
            }
        }
        Some(Decoration::Code(part)) => {
            let (round_top, round_bottom) = match part {
                CodePart::Open { .. } => (CODE_RADIUS, px(0.)),
                CodePart::Body => (px(0.), px(0.)),
                CodePart::Close => (px(0.), CODE_RADIUS),
            };
            let corners = Corners {
                top_left: round_top,
                top_right: round_top,
                bottom_left: round_bottom,
                bottom_right: round_bottom,
            };
            window.paint_quad(
                fill(
                    Bounds::new(point(left, top), size(width, line.height())),
                    palette.code_bg,
                )
                .corner_radii(corners),
            );
            if let CodePart::Open { language } = part {
                if !language.is_empty() {
                    let label: SharedString = language.to_string().into();
                    let font_size = input.font_size * 0.8;
                    let run =
                        text_run(label.len(), &input.font, palette.syntax, Default::default());
                    let shaped = window
                        .text_system()
                        .shape_line(label, font_size, &[run], None);
                    let at = point(left + width - shaped.width() - CODE_PAD, top);
                    let _ = shaped.paint(at, line.line_height, TextAlign::Left, None, window, cx);
                }
            }
        }
        Some(Decoration::Table(row)) => {
            let Some(edges) = &line.columns else {
                return;
            };
            let right = left + *edges.last().unwrap_or(&px(0.));
            let h = line.height();
            let border = palette.separator;
            let hline = |y: Pixels, window: &mut Window| {
                window.paint_quad(fill(
                    Bounds::new(point(left, y), size(right - left, px(1.))),
                    border,
                ))
            };
            if row.kind == RowKind::Delimiter {
                // The rule under the header, across the table: two pixels.
                window.paint_quad(fill(
                    Bounds::new(point(left, top), size(right - left, h)),
                    border,
                ));
                return;
            }
            if row.kind == RowKind::Header {
                hline(top, window);
            }
            hline(top + h - px(1.), window);
            for x in edges.iter() {
                window.paint_quad(fill(
                    Bounds::new(point(left + *x, top), size(px(1.), h)),
                    border,
                ));
            }
        }
        // Painted after the text, in `paint`.
        Some(Decoration::Image(_)) | None => {}
    }
}

/// An image under its line's text: loaded in the background the first time
/// (a placeholder until then), then from GPUI's cache.
fn paint_image(
    line: &LaidLine,
    (left, top): (Pixels, Pixels),
    input: &ShapeInput,
    window: &mut Window,
    cx: &mut App,
) {
    let Some((path, size)) = &line.image else {
        return;
    };
    let bounds = Bounds::new(point(left + line.indent, top + line.text_height), *size);
    let source = Resource::Path(Arc::from(&**path));
    match window.use_asset::<ImageAssetLoader>(&source, cx) {
        Some(Ok(image)) => {
            let _ = window.paint_image(bounds, bounds, Corners::all(px(4.)), image, 0, false);
        }
        Some(Err(_)) => {}
        None => {
            window.paint_quad(fill(bounds, input.palette.code_bg).corner_radii(px(4.)));
            // Frames until it is ready, so it shows as soon as it is, whatever
            // GPUI notifies (only while an image in view is loading).
            window.request_animation_frame();
        }
    }
}

/// Where a task's checkbox is drawn, for a list item at `x` whose first row's
/// middle is at `mid`.
fn checkbox_bounds(x: Pixels, mid: Pixels) -> Bounds<Pixels> {
    Bounds::new(
        point(x + px(2.), mid - CHECKBOX / 2.),
        size(CHECKBOX, CHECKBOX),
    )
}

/// A table's column widths, its cells' padding included: each as wide as its
/// widest cell, up to about 40 characters, all shrunk in proportion when the
/// table is wider than the text (ADR 0008).
fn column_widths(
    table: &Table,
    input: &ShapeInput,
    width: Pixels,
    ts: &WindowTextSystem,
) -> Rc<[Pixels]> {
    let max = input.font_size * MAX_COLUMN_EMS;
    let mut widths = vec![px(0.); table.align.len()];
    for (r, row) in table.rows.iter().enumerate() {
        let weight = if r == 0 {
            FontWeight::BOLD
        } else {
            FontWeight::NORMAL
        };
        let font = Font {
            weight,
            ..input.font.clone()
        };
        for (w, cell) in widths.iter_mut().zip(row) {
            let run = text_run(
                cell.len(),
                &font,
                input.palette.text,
                HighlightStyle::default(),
            );
            let shaped = ts.shape_line(cell.clone().into(), input.font_size, &[run], None);
            *w = (*w).max(shaped.width().min(max));
        }
    }
    let padded: Vec<Pixels> = widths.iter().map(|w| *w + CELL_PAD * 2.).collect();
    let total: Pixels = padded.iter().copied().sum();
    let scale = if total > width && total > px(0.) {
        width / total
    } else {
        1.
    };
    padded.into_iter().map(|w| w * scale).collect()
}

/// A table row's cells, shaped one by one and placed in their columns.
fn table_cells(
    input: &ShapeInput,
    range: &Range<usize>,
    style: &LineStyle,
    row: &TableRow,
    widths: &[Pixels],
    ts: &WindowTextSystem,
) -> Rc<[Cell]> {
    let mut left = px(0.);
    let mut cells = Vec::new();
    for ((cell, w), align) in row.cells.iter().zip(widths).zip(&row.table.align) {
        // The cell's own style: the line's, cut to the cell.
        let clip = |r: &Range<usize>| {
            let (a, b) = (r.start.max(cell.start), r.end.min(cell.end));
            (a < b).then(|| a - cell.start..b - cell.start)
        };
        let mut cell_style = LineStyle {
            segments: style
                .segments
                .iter()
                .filter_map(|(r, k)| clip(r).map(|r| (r, k.clone())))
                .collect(),
            hidden: style.hidden.iter().filter_map(clip).collect(),
            ..LineStyle::default()
        };
        if row.kind == RowKind::Header {
            cell_style
                .segments
                .push((0..cell.len(), vec![MdSpanKind::Bold]));
        }
        let abs = range.start + cell.start..range.start + cell.end;
        let runs = runs_for(input, &abs, &cell_style);
        let text = shown_text(&input.text[abs], &cell_style.hidden);
        let shaped = ts.shape_line(text.into(), input.font_size, &runs, None);
        let room = (*w - CELL_PAD * 2.).max(px(0.));
        let x = left
            + CELL_PAD
            + match align {
                Align::Left => px(0.),
                Align::Center => ((room - shaped.width()) / 2.).max(px(0.)),
                Align::Right => (room - shaped.width()).max(px(0.)),
            };
        cells.push(Cell {
            range: cell.clone(),
            x,
            room: left + CELL_PAD + room - x,
            hidden: cell_style.hidden,
            shaped,
        });
        left += *w;
    }
    cells.into()
}

fn shape_line(
    input: &ShapeInput,
    range: Range<usize>,
    style: &LineStyle,
    width: Pixels,
    ts: &WindowTextSystem,
    columns: &mut HashMap<*const Table, Rc<[Pixels]>>,
) -> LaidLine {
    if let Some(Decoration::Table(row)) = &style.decoration {
        let widths = columns
            .entry(Rc::as_ptr(&row.table))
            .or_insert_with(|| column_widths(&row.table, input, width, ts))
            .clone();
        let cells = (row.kind != RowKind::Delimiter)
            .then(|| table_cells(input, &range, style, row, &widths, ts));
        let mut edges = vec![px(0.)];
        for w in widths.iter() {
            edges.push(*edges.last().unwrap() + *w);
        }
        let line_height = input.font_size * LINE_HEIGHT;
        // The delimiter row is only the rule under the header. Its height
        // changes only when the cursor is on it, which SPEC allows.
        let text_height = if row.kind == RowKind::Delimiter {
            DELIMITER_ROW
        } else {
            line_height
        };
        return LaidLine {
            range,
            line_height,
            hidden: style.hidden.clone().into(),
            shaped: Rc::default(),
            indent: px(0.),
            decoration: style.decoration.clone(),
            text_height,
            image: None,
            cells,
            columns: Some(edges.into()),
        };
    }
    let (font_size, line_height) = match style.heading {
        Some(level) => {
            let size = input.font_size * HEADING_SCALE[usize::from(level.clamp(1, 6)) - 1];
            (size, size * HEADING_LINE_HEIGHT)
        }
        None => (input.font_size, input.font_size * LINE_HEIGHT),
    };
    let indent = match &style.decoration {
        Some(Decoration::Quote { depth }) => QUOTE_INDENT * f32::from(*depth),
        Some(Decoration::List { level, .. }) => LIST_INDENT * f32::from(*level) + LIST_MARKER,
        Some(Decoration::Code(_)) => CODE_PAD,
        _ => px(0.),
    };
    let runs = runs_for(input, &range, style);
    let shaped = ts
        .shape_text(
            SharedString::from(shown_text(&input.text[range.clone()], &style.hidden)),
            font_size,
            &runs,
            Some((width - indent).max(px(1.))),
            None,
        )
        .ok()
        .and_then(|mut v| (!v.is_empty()).then(|| v.remove(0)))
        .unwrap_or_default();
    // An image as wide as its file, or as the text if narrower, in proportion.
    let image = match &style.decoration {
        Some(Decoration::Image(img)) if img.width > 0 => {
            let w = px(img.width as f32).min(width - indent);
            let h = w * (img.height as f32 / img.width as f32);
            Some((img.path.clone(), size(w, h)))
        }
        _ => None,
    };
    let text_height = if image.is_some() && shaped.len() == 0 {
        px(0.)
    } else {
        shaped.size(line_height).height
    };
    LaidLine {
        range,
        line_height,
        hidden: style.hidden.clone().into(),
        shaped: Rc::new(shaped),
        indent,
        decoration: style.decoration.clone(),
        text_height,
        image,
        cells: None,
        columns: None,
    }
}

/// The text runs of the shown line: the Live preview styles of its segments,
/// and the text being composed underlined. Ranges are relative to the line;
/// its hidden ranges get no run, as they are not in the shown text.
fn runs_for(input: &ShapeInput, line: &Range<usize>, style: &LineStyle) -> Vec<TextRun> {
    let len = line.len();
    let mut layers: Vec<(Range<usize>, HighlightStyle)> = style
        .segments
        .iter()
        .map(|(r, kinds)| {
            let merged = kinds.iter().fold(HighlightStyle::default(), |acc, k| {
                acc.highlight(span_style(*k, &input.palette))
            });
            (r.start.min(len)..r.end.min(len), merged)
        })
        .collect();
    if let Some(m) = &input.marked {
        let (a, b) = (m.start.max(line.start), m.end.min(line.end));
        if a < b {
            let underline = HighlightStyle {
                underline: Some(UnderlineStyle {
                    color: Some(input.palette.text),
                    thickness: px(1.),
                    wavy: false,
                }),
                ..Default::default()
            };
            layers.push((a - line.start..b - line.start, underline));
        }
    }
    // Code, inline or in a block, is in the monospaced font.
    let mono: Vec<Range<usize>> = style
        .segments
        .iter()
        .filter(|(_, k)| {
            k.iter()
                .any(|k| matches!(k, MdSpanKind::InlineCode | MdSpanKind::CodeBlock))
        })
        .map(|(r, _)| r.clone())
        .collect();
    let code_font = Font {
        family: CODE_FONT.into(),
        ..input.font.clone()
    };
    // Cut the line at every layer's and hidden range's ends; each shown piece
    // gets every layer over it.
    let mut bounds = vec![0, len];
    for r in layers.iter().map(|(r, _)| r).chain(&style.hidden) {
        bounds.extend([r.start.min(len), r.end.min(len)]);
    }
    bounds.sort_unstable();
    bounds.dedup();
    let runs: Vec<TextRun> = bounds
        .windows(2)
        .filter(|w| {
            !style
                .hidden
                .iter()
                .any(|h| h.start <= w[0] && w[1] <= h.end)
        })
        .map(|w| {
            let style = layers
                .iter()
                .filter(|(r, _)| r.start <= w[0] && w[1] <= r.end)
                .fold(HighlightStyle::default(), |acc, (_, s)| acc.highlight(*s));
            let font = if mono.iter().any(|r| r.start <= w[0] && w[1] <= r.end) {
                &code_font
            } else {
                &input.font
            };
            text_run(w[1] - w[0], font, input.palette.text, style)
        })
        .collect();
    if runs.is_empty() {
        vec![text_run(
            0,
            &input.font,
            input.palette.text,
            HighlightStyle::default(),
        )]
    } else {
        runs
    }
}

fn text_run(len: usize, font: &Font, base: Hsla, style: HighlightStyle) -> TextRun {
    let mut color = style.color.unwrap_or(base);
    if let Some(f) = style.fade_out {
        color.fade_out(f);
    }
    let mut font = font.clone();
    if let Some(w) = style.font_weight {
        font.weight = w;
    }
    if let Some(s) = style.font_style {
        font.style = s;
    }
    TextRun {
        len,
        font,
        color,
        background_color: style.background_color,
        underline: style.underline,
        strikethrough: style.strikethrough,
    }
}

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
    caret: Option<Bounds<Pixels>>,
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
            .update(cx, |ed, _| (ed.line_styles(), ed.shared_text()));
        let ed = self.editor.read(cx);
        let style = window.text_style();
        let input = ShapeInput {
            text,
            revision: ed.revision,
            marked: ed.buf.marked(),
            styles,
            palette: ed.palette,
            font: style.font(),
            font_size: style.font_size.to_pixels(window.rem_size()),
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
        let sel = ed.buf.selection().range();
        let selection = if sel.is_empty() {
            Vec::new()
        } else {
            let (first, last) = (layout.line_of(sel.start), layout.line_of(sel.end));
            (first..=last)
                .flat_map(|i| layout.selection_rects(i, &sel))
                .collect()
        };
        let caret = (ed.cursor_on && sel.is_empty() && ed.focus_handle.is_focused(window))
            .then(|| layout.caret(ed.buf.text(), ed.buf.cursor()))
            .flatten();
        let matches = self.editor.update(cx, |ed, _| ed.find_matches());
        let mut match_rects = Vec::new();
        for m in &matches {
            for i in layout.line_of(m.start)..=layout.line_of(m.end) {
                match_rects.extend(layout.selection_rects(i, m));
            }
        }
        let matches = match_rects;
        PrepaintState {
            layout,
            selection,
            matches,
            caret,
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
        let (focus, palette) = {
            let ed = self.editor.read(cx);
            (ed.focus_handle.clone(), ed.palette)
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
                size(cell.room, line.line_height),
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
            paint_image(line, (layout.origin.x, top), input, window, cx);
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
        if let Some(c) = state.caret {
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
            let wanted = scroll_to_reveal(
                scrolled,
                f32::from(viewport.size.height),
                top,
                top + height,
                height,
                f32::from(scroll.max_offset().y),
            );
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
        font_size,
    };
    px(shape(&input, width, ts).1.total())
}
