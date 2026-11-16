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

#[cfg(test)]
mod tests {
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
}
