//! The display map of one line: the Note as written holds marks that Live
//! preview hides (a Heading's `# `), so a line's shown text is its text minus
//! its hidden ranges. Offsets are converted both ways here; all of them are
//! relative to the line's start, and hidden ranges are sorted and disjoint.

use std::ops::Range;

/// Where `offset` of the written line falls in the shown text. An offset
/// inside a hidden range falls where that range was cut out.
pub fn to_display(hidden: &[Range<usize>], offset: usize) -> usize {
    let mut cut = 0;
    for h in hidden {
        if offset <= h.start {
            break;
        }
        cut += offset.min(h.end) - h.start;
    }
    offset - cut
}

/// The offset of the written line shown at `display`. Where a hidden range was
/// cut out, the offset after it: a click before a Heading's first letter lands
/// before that letter, never inside or before its `# `.
pub fn to_buffer(hidden: &[Range<usize>], display: usize) -> usize {
    let mut offset = display;
    for h in hidden {
        if offset < h.start {
            break;
        }
        offset += h.len();
    }
    offset
}

/// The text of the line as shown: without its hidden ranges.
pub fn shown_text(line: &str, hidden: &[Range<usize>]) -> String {
    let mut out = String::with_capacity(line.len());
    let mut at = 0;
    for h in hidden {
        out.push_str(&line[at..h.start]);
        at = h.end;
    }
    out.push_str(&line[at..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_heading_mark_is_cut_out() {
        // "## Titolo" with "## " hidden.
        let hidden = [0..3];
        assert_eq!(shown_text("## Titolo", &hidden), "Titolo");
        assert_eq!(to_display(&hidden, 0), 0);
        assert_eq!(
            to_display(&hidden, 2),
            0,
            "inside the mark: where it was cut"
        );
        assert_eq!(to_display(&hidden, 3), 0);
        assert_eq!(to_display(&hidden, 5), 2);
        assert_eq!(
            to_buffer(&hidden, 0),
            3,
            "before the first letter, after the mark"
        );
        assert_eq!(to_buffer(&hidden, 2), 5);
        assert_eq!(to_buffer(&hidden, 6), 9);
    }

    #[test]
    fn several_ranges_and_none() {
        let hidden = [1..2, 4..6];
        assert_eq!(shown_text("abcdefg", &hidden), "acdg");
        assert_eq!(to_display(&hidden, 7), 4);
        assert_eq!(to_buffer(&hidden, 1), 2);
        assert_eq!(to_buffer(&hidden, 3), 6);
        assert_eq!(to_buffer(&[], 3), 3);
        assert_eq!(to_display(&[], 3), 3);
    }

    #[test]
    fn shown_offsets_round_trip() {
        let hidden = [0..2, 5..7, 7..8];
        let shown = shown_text("##abc**d*ef", &hidden).len();
        for d in 0..=shown {
            let o = to_buffer(&hidden, d);
            assert_eq!(to_display(&hidden, o), d);
            assert!(
                !hidden.iter().any(|h| h.start < o && o < h.end),
                "{o} inside"
            );
        }
    }
}
