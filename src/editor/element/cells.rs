//! Tables and the Properties table: columns, cells, and a list's chips.

use super::*;

/// A table's column widths, its cells' padding included: each as wide as its
/// widest cell, up to about 40 characters, all shrunk in proportion when the
/// table is wider than the text (ADR 0008).
pub(super) fn column_widths(
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
pub(super) fn table_cells(
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
        let room = (*w - CELL_PAD * 2.).max(px(0.));
        let shape = |text: String, runs: &[TextRun], wrap: Option<Pixels>| {
            ts.shape_text(text.into(), input.font_size, runs, wrap, None)
                .ok()
                .and_then(|mut v| (!v.is_empty()).then(|| v.remove(0)))
                .unwrap_or_default()
        };
        // In the row being edited, all of it, wrapped in its column.
        let wrap = row.editing.then_some(room.max(px(1.)));
        let mut shaped = shape(text.clone(), &runs, wrap);
        // Elsewhere, cut at its column with an ellipsis (ADR 0008).
        if !row.editing && shaped.width() > room && room > px(0.) {
            let ell = ts.shape_line(
                "…".into(),
                input.font_size,
                &[text_run(
                    "…".len(),
                    &input.font,
                    input.palette.text,
                    Default::default(),
                )],
                None,
            );
            let keep = match shaped.closest_index_for_position(
                point((room - ell.width()).max(px(0.)), px(0.)),
                input.font_size * LINE_HEIGHT,
            ) {
                Ok(i) | Err(i) => i,
            };
            let keep = (0..=keep)
                .rev()
                .find(|&k| text.is_char_boundary(k))
                .unwrap_or(0);
            let mut cut_runs = Vec::new();
            let mut left_len = keep;
            for r in &runs {
                if left_len == 0 {
                    break;
                }
                let len = r.len.min(left_len);
                cut_runs.push(TextRun { len, ..r.clone() });
                left_len -= len;
            }
            let last = runs.last().cloned().unwrap_or_else(|| {
                text_run(0, &input.font, input.palette.text, Default::default())
            });
            cut_runs.push(TextRun {
                len: "…".len(),
                ..last
            });
            shaped = shape(format!("{}…", &text[..keep]), &cut_runs, None);
        }
        let wrapped = !shaped.wrap_boundaries().is_empty();
        let x = left
            + CELL_PAD
            + match align {
                _ if wrapped => px(0.),
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

/// A Properties list's items as chips, left to right from `left` and
/// wrapping before `right`, each row as tall as a chip and a gap; the first
/// row centred on the key's. The figure and its size from the row's top-left.
pub(super) fn property_chips(
    items: &[String],
    input: &ShapeInput,
    left: Pixels,
    right: Pixels,
    line_height: Pixels,
    ts: &WindowTextSystem,
) -> (Figure, Size<Pixels>) {
    let size_ = input.font_size * CHIP_SCALE;
    let pad = size_ * 0.45;
    let chip_h = size_ * 1.45;
    let row_h = chip_h + px(4.);
    let first = (line_height - chip_h) / 2.;
    let (mut x, mut y) = (left, first);
    let mut chips = Vec::with_capacity(items.len());
    for item in items {
        let run = text_run(
            item.len(),
            &input.font,
            input.palette.text,
            Default::default(),
        );
        let shaped = ts.shape_line(item.clone().into(), size_, &[run], None);
        let w = (shaped.width + pad * 2.).min(right - left);
        if x > left && x + w > right {
            x = left;
            y += row_h;
        }
        chips.push((point(x, y), w, shaped));
        x += w + px(4.);
    }
    // A figure is drawn under its row's text: the chips from the row's top.
    let height = (y + chip_h + first).max(line_height);
    for (at, _, _) in &mut chips {
        at.y -= line_height;
    }
    (
        Figure::Chips(chips.into()),
        size(right, height - line_height),
    )
}
