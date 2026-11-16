//! The element that measures and paints a [`NoteEditor`]: every logical line
//! shaped on its own (at its own size, wrapped to the width), stacked by
//! [`LineTops`], never by `line_height * row`.

use std::cell::RefCell;
use std::ops::Range;
use std::rc::Rc;

use gpui_kit::{
    fill, point, px, size, App, AvailableSpace, Bounds, Element, ElementId, ElementInputHandler,
    Entity, Font, GlobalElementId, Hsla, InspectorElementId, IntoElement, LayoutId, Pixels, Point,
    SharedString, Size, Style, TextAlign, TextRun, UnderlineStyle, Window, WindowTextSystem,
    WrappedLine,
};

use crate::core::editor::lines::{line_index_of, logical_lines, scroll_to_reveal, LineTops};
use crate::core::editor::movement::prev_grapheme;

use super::NoteEditor;

/// Text line height as a multiple of the font size.
const LINE_HEIGHT: f32 = 1.6;
const CARET_WIDTH: Pixels = px(2.);

pub(super) struct LaidLine {
    /// The line's bytes in the text, without its line break.
    range: Range<usize>,
    line_height: Pixels,
    shaped: WrappedLine,
}

impl LaidLine {
    fn height(&self) -> Pixels {
        self.shaped.size(self.line_height).height
    }

    /// The position of `offset` (clamped to the line) relative to the line's top-left.
    fn position(&self, offset: usize) -> Point<Pixels> {
        let local = offset.clamp(self.range.start, self.range.end) - self.range.start;
        self.shaped
            .position_for_index(local, self.line_height)
            .unwrap_or_else(|| point(self.shaped.width(), px(0.)))
    }
}

/// Where each line of the text was drawn in the last frame, in window coordinates.
#[derive(Clone)]
pub(super) struct Layout {
    origin: Point<Pixels>,
    width: Pixels,
    lines: Rc<Vec<LaidLine>>,
    tops: LineTops,
}

impl Layout {
    fn top_of(&self, line: usize) -> Pixels {
        px(self.tops.top(line))
    }

    pub(super) fn text_top(&self) -> Pixels {
        self.origin.y
    }

    pub(super) fn text_bottom(&self) -> Pixels {
        self.origin.y + px(self.tops.total())
    }

    fn line_of(&self, offset: usize) -> usize {
        let ranges: Vec<_> = self.lines.iter().map(|l| l.range.clone()).collect();
        line_index_of(&ranges, offset)
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
        let y = (rel.y - self.top_of(i)).clamp(px(0.), line.height() - px(0.5));
        let local = match line
            .shaped
            .closest_index_for_position(point(rel.x.max(px(0.)), y), line.line_height)
        {
            Ok(i) | Err(i) => i,
        };
        (line.range.start + local).min(line.range.end)
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
            rects.push(rect(y, px(0.), self.width));
            y += lh;
        }
        rects.push(rect(pb.y, px(0.), right));
        rects
    }
}

/// What one frame needs to shape the text.
#[derive(Clone)]
pub(super) struct ShapeInput {
    text: SharedString,
    marked: Option<Range<usize>>,
    font: Font,
    font_size: Pixels,
    color: Hsla,
}

fn shape(input: &ShapeInput, width: Pixels, ts: &WindowTextSystem) -> Vec<LaidLine> {
    let line_height = input.font_size * LINE_HEIGHT;
    logical_lines(&input.text)
        .into_iter()
        .map(|range| {
            let runs = runs_for(input, &range);
            let shaped = ts
                .shape_text(
                    SharedString::from(input.text[range.clone()].to_string()),
                    input.font_size,
                    &runs,
                    Some(width),
                    None,
                )
                .ok()
                .and_then(|mut v| (!v.is_empty()).then(|| v.remove(0)))
                .unwrap_or_default();
            LaidLine {
                range,
                line_height,
                shaped,
            }
        })
        .collect()
}

/// The text runs of one line: plain, with the text being composed underlined.
fn runs_for(input: &ShapeInput, line: &Range<usize>) -> Vec<TextRun> {
    let run = |len: usize, underline: bool| TextRun {
        len,
        font: input.font.clone(),
        color: input.color,
        background_color: None,
        underline: underline.then_some(UnderlineStyle {
            color: Some(input.color),
            thickness: px(1.),
            wavy: false,
        }),
        strikethrough: None,
    };
    match &input.marked {
        Some(m) if m.start < line.end && m.end > line.start => {
            let (a, b) = (m.start.max(line.start), m.end.min(line.end));
            [
                run(a - line.start, false),
                run(b - a, true),
                run(line.end - b, false),
            ]
            .into_iter()
            .filter(|r| r.len > 0)
            .collect()
        }
        _ => vec![run(line.len(), false)],
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

pub(super) type Measured = Rc<RefCell<Option<(Pixels, Vec<LaidLine>)>>>;

pub(super) struct PrepaintState {
    layout: Layout,
    selection: Vec<Bounds<Pixels>>,
    caret: Option<Bounds<Pixels>>,
}

impl Element for EditorElement {
    type RequestLayoutState = (ShapeInput, Measured);
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
        let ed = self.editor.read(cx);
        let style = window.text_style();
        let input = ShapeInput {
            text: SharedString::from(ed.buf.text().to_string()),
            marked: ed.buf.marked(),
            font: style.font(),
            font_size: style.font_size.to_pixels(window.rem_size()),
            color: ed.palette.text,
        };
        let measured: Measured = Rc::default();
        let (inp, out) = (input.clone(), measured.clone());
        let mut layout_style = Style::default();
        layout_style.size.width = gpui_kit::relative(1.).into();
        let id =
            window.request_measured_layout(layout_style, move |known, available, window, _| {
                let width = known.width.unwrap_or(match available.width {
                    AvailableSpace::Definite(w) => w,
                    _ => px(800.),
                });
                let lines = shape(&inp, width, window.text_system());
                let height: Pixels = lines.iter().map(|l| l.height()).sum();
                *out.borrow_mut() = Some((width, lines));
                Size { width, height }
            });
        (id, (input, measured))
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        (input, measured): &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> PrepaintState {
        let lines = match measured.borrow_mut().take() {
            Some((w, lines)) if w == bounds.size.width => lines,
            _ => shape(input, bounds.size.width, window.text_system()),
        };
        let tops = LineTops::new(lines.iter().map(|l| f32::from(l.height())));
        let layout = Layout {
            origin: bounds.origin,
            width: bounds.size.width,
            lines: Rc::new(lines),
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
        PrepaintState {
            layout,
            selection,
            caret,
        }
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
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
        for r in state.selection.drain(..) {
            window.paint_quad(fill(r, palette.selection));
        }
        let visible = window.content_mask().bounds;
        let layout = &state.layout;
        for (i, line) in layout.lines.iter().enumerate() {
            let top = layout.origin.y + layout.top_of(i);
            if top > visible.bottom() {
                break;
            }
            if top + line.height() < visible.top() {
                continue;
            }
            let _ = line.shaped.paint(
                point(layout.origin.x, top),
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
        marked: None,
        font,
        font_size,
        color: Hsla::default(),
    };
    shape(&input, width, ts).iter().map(|l| l.height()).sum()
}
