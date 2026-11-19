//! Shaping: each logical line laid out on its own, at its size and wrapped
//! to the width, with its formulas' room and its runs of colour and weight.

use super::*;

/// The lines of `input` wrapped to `width`, reusing what the cache holds.
pub(super) fn shape(
    input: &ShapeInput,
    width: Pixels,
    ts: &WindowTextSystem,
) -> (Rc<Vec<LaidLine>>, Rc<LineTops>) {
    let look = Look {
        font: input.font.clone(),
        code_font: input.code_font.clone(),
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

pub(super) fn shape_line(
    input: &ShapeInput,
    range: Range<usize>,
    style: &LineStyle,
    width: Pixels,
    ts: &WindowTextSystem,
    columns: &mut HashMap<*const Table, Rc<[Pixels]>>,
) -> LaidLine {
    // A Properties row: its key and value as two cells, the key column fixed.
    if let Some(Decoration::Property(p)) = &style.decoration {
        let key_col = input.font_size * PROPERTY_KEY_EMS;
        let line_height = input.font_size * LINE_HEIGHT;
        let cells = match p {
            PropertyLine::Row { key, value, .. } => vec![key.clone(), value.clone()],
            PropertyLine::Item { value, .. } => vec![value.start..value.start, value.clone()],
            PropertyLine::List { key, .. } => vec![key.clone(), key.end..key.end],
            PropertyLine::Top | PropertyLine::Bottom | PropertyLine::Folded => Vec::new(),
        };
        let figure = match p {
            PropertyLine::List { items, .. } => Some(property_chips(
                items,
                input,
                key_col + CELL_PAD,
                width - CELL_PAD,
                line_height,
                ts,
            )),
            _ => None,
        };
        let cells = (!cells.is_empty()).then(|| {
            let row = TableRow {
                table: Rc::new(Table {
                    align: vec![Align::Left, Align::Left],
                    rows: Vec::new(),
                }),
                kind: RowKind::Body,
                cells,
                editing: false,
            };
            table_cells(input, &range, style, &row, &[key_col, width - key_col], ts)
        });
        return LaidLine {
            range,
            line_height,
            hidden: style.hidden.clone().into(),
            shaped: Rc::default(),
            indent: px(0.),
            decoration: style.decoration.clone(),
            text_height: line_height,
            figure,
            inserts: Rc::default(),
            formulas: Rc::default(),
            cells,
            columns: None,
        };
    }
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
        // A row being edited is as tall as its cell of most rows.
        let rows = cells
            .as_ref()
            .and_then(|c| c.iter().map(Cell::rows).max())
            .unwrap_or(1);
        let text_height = if row.kind == RowKind::Delimiter {
            DELIMITER_ROW
        } else {
            line_height * rows as f32
        };
        return LaidLine {
            range,
            line_height,
            hidden: style.hidden.clone().into(),
            shaped: Rc::default(),
            indent: px(0.),
            decoration: style.decoration.clone(),
            text_height,
            figure: None,
            inserts: Rc::default(),
            formulas: Rc::default(),
            cells,
            columns: Some(edges.into()),
        };
    }
    let (font_size, mut line_height) = match style.heading {
        Some(level) => {
            let size = input.font_size * HEADING_SCALE[usize::from(level.clamp(1, 6)) - 1];
            (size, size * HEADING_LINE_HEIGHT)
        }
        None => (input.font_size, input.font_size * LINE_HEIGHT),
    };
    let indent = match &style.decoration {
        Some(Decoration::Quote { depth } | Decoration::Callout { depth, .. }) => {
            QUOTE_INDENT * f32::from(*depth)
        }
        Some(Decoration::List { level, .. }) => LIST_INDENT * f32::from(*level) + LIST_MARKER,
        Some(Decoration::Code(_)) => CODE_PAD,
        _ => px(0.),
    };
    let room = (width - indent).max(px(1.));
    // Each formula in the text: its source hidden, room for it in its place.
    let em = font_size * KATEX_SCALE;
    let (inserts, formulas) = inline_formulas(style, em, font_size, &input.font, ts);
    let mut runs = runs_for(input, &range, style);
    if !inserts.is_empty() {
        let at: Vec<(usize, usize)> = inserts
            .iter()
            .map(|i| (to_display(&style.hidden, i.at), i.text.len()))
            .collect();
        runs = with_insert_runs(runs, &at, &input.font);
    }
    let shaped = ts
        .shape_text(
            SharedString::from(shown_text_with(
                &input.text[range.clone()],
                &style.hidden,
                &inserts,
            )),
            font_size,
            &runs,
            Some(room),
            None,
        )
        .ok()
        .and_then(|mut v| (!v.is_empty()).then(|| v.remove(0)))
        .unwrap_or_default();
    // A tall formula makes its line taller, its baseline where the text's is.
    if !formulas.is_empty() {
        let (a, d) = (
            shaped.unwrapped_layout.ascent,
            shaped.unwrapped_layout.descent,
        );
        for f in formulas.iter() {
            line_height = line_height
                .max(f.paint.height * 2. - a + d + px(2.))
                .max(f.paint.depth * 2. + a - d + px(2.));
        }
    }
    let figure = match &style.decoration {
        // An image as wide as its file, or as the text if narrower, in proportion.
        Some(Decoration::Image(img)) if img.width > 0 => {
            let w = px(img.width as f32).min(room);
            let h = w * (img.height as f32 / img.width as f32);
            Some((Figure::File(img.path.clone()), size(w, h)))
        }
        // A display formula, smaller when wider than the text.
        Some(Decoration::Formula(f)) => {
            let mut paint = MathPaint::new(f, em, ts);
            let mut em = em;
            if paint.width > room {
                em *= f32::from(room) / f32::from(paint.width);
                paint = MathPaint::new(f, em, ts);
            }
            let h = paint.height + paint.depth + font_size * (2. * FORMULA_PAD);
            let w = paint.width;
            Some((Figure::Formula(Rc::new(paint)), size(w, h)))
        }
        // A diagram as wide as drawn, or as the text if narrower.
        Some(Decoration::Diagram(d)) if d.width > 0. => {
            let w = px(d.width).min(room);
            let h = w * (d.height / d.width) + font_size * (2. * FORMULA_PAD);
            Some((Figure::Diagram(d.clone()), size(w, h)))
        }
        _ => None,
    };
    let folded = matches!(style.decoration, Some(Decoration::Folded));
    let text_height = if folded || (figure.is_some() && shaped.len() == 0) {
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
        figure,
        inserts: inserts.into(),
        formulas: formulas.into(),
        cells: None,
        columns: None,
    }
}

/// The inserts that make room for the line's formulas, and the formulas laid
/// out: each room a run of narrow no-break spaces (never wrapped inside) as
/// wide as its formula, give or take half a space.
pub(super) fn inline_formulas(
    style: &LineStyle,
    em: Pixels,
    font_size: Pixels,
    font: &Font,
    ts: &WindowTextSystem,
) -> (Vec<Insert>, Vec<InlineFormula>) {
    const ROOM: char = '\u{202F}';
    if style.formulas.is_empty() {
        return Default::default();
    }
    let space = ts
        .advance(ts.resolve_font(font), font_size, ROOM)
        .map(|s| s.width)
        .unwrap_or(font_size * 0.2)
        .max(px(0.5));
    let mut inserts = Vec::new();
    let mut formulas = Vec::new();
    let mut shift = 0;
    for (range, formula) in &style.formulas {
        let paint = MathPaint::new(formula, em, ts);
        let n = ((paint.width / space).round() as usize).max(1);
        let text: String = std::iter::repeat_n(ROOM, n).collect();
        formulas.push(InlineFormula {
            at: to_display(&style.hidden, range.start) + shift,
            room: space * n as f32,
            paint: Rc::new(paint),
        });
        shift += text.len();
        inserts.push(Insert {
            at: range.start,
            text,
        });
    }
    (inserts, formulas)
}

/// `runs` (of the shown text without inserts) with a transparent run for each
/// insert `(where in that text, its length)`, cut where the inserts go.
pub(super) fn with_insert_runs(
    runs: Vec<TextRun>,
    at: &[(usize, usize)],
    font: &Font,
) -> Vec<TextRun> {
    let mut pieces = Vec::new();
    let mut pos = 0;
    for run in runs {
        let end = pos + run.len;
        let mut start = pos;
        for &(p, _) in at {
            if p > start && p < end {
                pieces.push(TextRun {
                    len: p - start,
                    ..run.clone()
                });
                start = p;
            }
        }
        pieces.push(TextRun {
            len: end - start,
            ..run
        });
        pos = end;
    }
    let room = |len| {
        text_run(
            len,
            font,
            Hsla::transparent_black(),
            HighlightStyle::default(),
        )
    };
    let mut out = Vec::new();
    let (mut pos, mut k) = (0, 0);
    for piece in pieces {
        while k < at.len() && at[k].0 <= pos {
            out.push(room(at[k].1));
            k += 1;
        }
        pos += piece.len;
        if piece.len > 0 {
            out.push(piece);
        }
    }
    for &(_, len) in &at[k..] {
        out.push(room(len));
    }
    out
}

/// The text runs of the shown line: the Live preview styles of its segments,
/// and the text being composed underlined. Ranges are relative to the line;
/// its hidden ranges get no run, as they are not in the shown text.
pub(super) fn runs_for(input: &ShapeInput, line: &Range<usize>, style: &LineStyle) -> Vec<TextRun> {
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
        family: input.code_font.clone(),
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

pub(super) fn text_run(len: usize, font: &Font, base: Hsla, style: HighlightStyle) -> TextRun {
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
