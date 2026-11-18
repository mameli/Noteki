//! The vertical geometry of the editor, without GPUI: logical lines of the text,
//! and where each one starts when lines have different heights.

use std::ops::Range;

/// The logical lines of `text`, as byte ranges without their `\n` or `\r\n`.
/// An empty text, or one ending with `\n`, has an empty last line.
pub fn logical_lines(text: &str) -> Vec<Range<usize>> {
    let mut lines = Vec::new();
    let mut start = 0;
    for (i, _) in text.match_indices('\n') {
        let end = if text[..i].ends_with('\r') { i - 1 } else { i };
        lines.push(start..end.max(start));
        start = i + 1;
    }
    lines.push(start..text.len());
    lines
}

/// The index of the line holding `offset`; an offset in a line's `\n` belongs
/// to that line.
pub fn line_index_of(lines: &[Range<usize>], offset: usize) -> usize {
    lines
        .partition_point(|l| l.start <= offset)
        .saturating_sub(1)
}

/// The top of each line, as the sum of the heights of the lines above it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LineTops {
    tops: Vec<f32>,
    total: f32,
}

impl LineTops {
    pub fn new(heights: impl IntoIterator<Item = f32>) -> Self {
        let mut tops = Vec::new();
        let mut y = 0.;
        for h in heights {
            tops.push(y);
            y += h;
        }
        Self { tops, total: y }
    }

    pub fn top(&self, line: usize) -> f32 {
        self.tops.get(line).copied().unwrap_or(self.total)
    }

    pub fn total(&self) -> f32 {
        self.total
    }

    /// The line under `y`; above the first line it is the first, below the
    /// last, the last.
    pub fn line_at(&self, y: f32) -> usize {
        self.tops.partition_point(|&t| t <= y).saturating_sub(1)
    }
}

/// How far to scroll (from the top, never negative, at most `max`) so that
/// `top..bottom` of the content shows in a viewport `height` tall with `margin`
/// around it, moving from `scroll` as little as possible. A span too tall for
/// its margins keeps what margin fits.
pub fn scroll_to_reveal(
    scroll: f32,
    height: f32,
    top: f32,
    bottom: f32,
    margin: f32,
    max: f32,
) -> f32 {
    let margin = margin.min(((height - (bottom - top)) / 2.).max(0.));
    let wanted = if top - margin < scroll {
        top - margin
    } else if bottom + margin > scroll + height {
        bottom + margin - height
    } else {
        scroll
    };
    wanted.clamp(0., max.max(0.))
}

/// Typewriter Mode: the scroll that puts the middle of `top..bottom` in the
/// middle of a viewport `height` tall, within `0..=max`.
pub fn scroll_to_centre(height: f32, top: f32, bottom: f32, max: f32) -> f32 {
    ((top + bottom) / 2. - height / 2.).clamp(0., max.max(0.))
}

/// Focus Mode: the lines of the paragraph line `line` is in, among `lines`
/// (each without its line break): the lines around it up to a blank one. A
/// blank line is a paragraph of its own.
pub fn paragraph_of<'a>(lines: impl IntoIterator<Item = &'a str>, line: usize) -> Range<usize> {
    let blank: Vec<bool> = lines.into_iter().map(|l| l.trim().is_empty()).collect();
    if line >= blank.len() || blank[line] {
        return line..line + 1;
    }
    let start = blank[..line].iter().rposition(|&b| b).map_or(0, |b| b + 1);
    let end = blank[line..]
        .iter()
        .position(|&b| b)
        .map_or(blank.len(), |b| line + b);
    start..end
}

/// While a selection is dragged past the top or bottom of the view, how far
/// to scroll per frame (negative: up): the further past the edge, the faster.
pub fn edge_scroll(pointer_y: f32, top: f32, bottom: f32) -> f32 {
    const PER_PIXEL: f32 = 0.3;
    const MAX: f32 = 60.;
    let past = if pointer_y < top {
        pointer_y - top
    } else if pointer_y > bottom {
        pointer_y - bottom
    } else {
        return 0.;
    };
    (past * PER_PIXEL).clamp(-MAX, MAX)
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_paragraph_is_the_lines_between_blank_ones() {
        let lines = ["# Title", "", "one", "two", "three", "", "four"];
        assert_eq!(paragraph_of(lines, 3), 2..5);
        assert_eq!(paragraph_of(lines, 6), 6..7);
        assert_eq!(paragraph_of(lines, 0), 0..1);
        assert_eq!(paragraph_of(lines, 1), 1..2, "a blank line alone");
    }

    #[test]
    fn typewriter_puts_the_line_in_the_middle_within_the_text() {
        assert_eq!(scroll_to_centre(400., 1000., 1020., 5000.), 810.);
        assert_eq!(scroll_to_centre(400., 10., 30., 5000.), 0.);
        assert_eq!(scroll_to_centre(400., 4990., 5010., 4800.), 4800.);
    }

    use super::*;

    #[test]
    fn lines_exclude_their_line_breaks() {
        assert_eq!(logical_lines("a\r\nbc\n"), vec![0..1, 3..5, 6..6]);
        assert_eq!(logical_lines(""), vec![0..0]);
        assert_eq!(logical_lines("\n"), vec![0..0, 1..1]);
    }

    #[test]
    fn an_offset_belongs_to_its_line_even_on_the_newline() {
        let t = "ab\ncd";
        let lines = logical_lines(t);
        assert_eq!(line_index_of(&lines, 0), 0);
        assert_eq!(line_index_of(&lines, 2), 0);
        assert_eq!(line_index_of(&lines, 3), 1);
        assert_eq!(line_index_of(&lines, 5), 1);
    }

    #[test]
    fn tops_add_up_lines_of_different_heights() {
        let tops = LineTops::new([40., 20., 20., 30.]);
        assert_eq!(tops.top(0), 0.);
        assert_eq!(tops.top(2), 60.);
        assert_eq!(tops.total(), 110.);
        assert_eq!(tops.line_at(0.), 0);
        assert_eq!(tops.line_at(39.9), 0);
        assert_eq!(tops.line_at(40.), 1);
        assert_eq!(tops.line_at(85.), 3);
        assert_eq!(tops.line_at(-5.), 0);
        assert_eq!(tops.line_at(500.), 3);
    }

    #[test]
    fn revealing_scrolls_as_little_as_needed() {
        // Viewport 100 tall, margin 10, content up to 1000.
        let r = |scroll, top, bottom| scroll_to_reveal(scroll, 100., top, bottom, 10., 900.);
        assert_eq!(r(0., 50., 70.), 0., "already in view");
        assert_eq!(
            r(0., 120., 140.),
            50.,
            "below: its bottom plus margin at the bottom"
        );
        assert_eq!(
            r(300., 200., 220.),
            190.,
            "above: its top minus margin at the top"
        );
        assert_eq!(r(300., 5., 25.), 0., "never above the start");
        assert_eq!(r(0., 990., 1010.), 900., "never past the end");
        // A caret taller than the room left by the margins keeps what fits.
        assert_eq!(scroll_to_reveal(0., 30., 100., 120., 10., 900.), 95.);
    }

    #[test]
    fn dragging_past_an_edge_scrolls_faster_the_further_it_is() {
        assert_eq!(edge_scroll(50., 0., 100.), 0.);
        assert!(edge_scroll(-10., 0., 100.) < 0.);
        assert!(edge_scroll(110., 0., 100.) > 0.);
        assert!(edge_scroll(150., 0., 100.) > edge_scroll(110., 0., 100.));
        assert_eq!(edge_scroll(10_000., 0., 100.), 60., "up to a limit");
    }
}
