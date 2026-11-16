//! The element that measures and paints a [`NoteEditor`]: every logical line
//! shaped on its own (at its own size, wrapped to the width), stacked by
//! [`LineTops`], never by `line_height * row`.

use std::cell::RefCell;
use std::ops::Range;
use std::rc::Rc;

use gpui_kit::{
    fill, point, px, size, App, AvailableSpace, Bounds, DispatchPhase, Element, ElementId,
    ElementInputHandler, Entity, Font, GlobalElementId, HighlightStyle, Hsla, InspectorElementId,
    IntoElement, LayoutId, MouseMoveEvent, MouseUpEvent, Pixels, Point, SharedString, Size, Style,
    TextAlign, TextRun, UnderlineStyle, Window, WindowTextSystem, WrappedLine,
};

use crate::core::editor::display::{shown_text, to_buffer, to_display};
use crate::core::editor::lines::{logical_lines, scroll_to_reveal, LineTops};
use crate::core::editor::movement::prev_grapheme;
use crate::core::editor::styling::LineStyle;
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

#[derive(Clone)]
pub(super) struct LaidLine {
    /// The line's bytes in the text, without its line break.
    range: Range<usize>,
    line_height: Pixels,
    /// What of the line is not drawn, relative to its start (its display map).
    hidden: Rc<[Range<usize>]>,
    /// The line as shown: without its hidden ranges.
    shaped: Rc<WrappedLine>,
}

impl LaidLine {
    fn height(&self) -> Pixels {
        self.shaped.size(self.line_height).height
    }

    /// The position of `offset` (clamped to the line) relative to the line's top-left.
    fn position(&self, offset: usize) -> Point<Pixels> {
        let local = offset.clamp(self.range.start, self.range.end) - self.range.start;
        self.shaped
            .position_for_index(to_display(&self.hidden, local), self.line_height)
            .unwrap_or_else(|| point(self.shaped.width(), px(0.)))
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
        let y = (rel.y - self.top_of(i)).clamp(px(0.), line.height() - px(0.5));
        let shown = match line
            .shaped
            .closest_index_for_position(point(rel.x.max(px(0.)), y), line.line_height)
        {
            Ok(i) | Err(i) => i,
        };
        let local = to_buffer(&line.hidden, shown);
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
                shape_line(input, range.clone(), style, width, ts)
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

fn shape_line(
    input: &ShapeInput,
    range: Range<usize>,
    style: &LineStyle,
    width: Pixels,
    ts: &WindowTextSystem,
) -> LaidLine {
    let (font_size, line_height) = match style.heading {
        Some(level) => {
            let size = input.font_size * HEADING_SCALE[usize::from(level.clamp(1, 6)) - 1];
            (size, size * HEADING_LINE_HEIGHT)
        }
        None => (input.font_size, input.font_size * LINE_HEIGHT),
    };
    let runs = runs_for(input, &range, style);
    let shaped = ts
        .shape_text(
            SharedString::from(shown_text(&input.text[range.clone()], &style.hidden)),
            font_size,
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
        hidden: style.hidden.clone().into(),
        shaped: Rc::new(shaped),
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
            text_run(w[1] - w[0], &input.font, input.palette.text, style)
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
