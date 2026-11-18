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

/// A text shown that the line does not hold: the room a formula drawn in the
/// text takes, at `at` (its source's start, the source itself hidden).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Insert {
    pub at: usize,
    pub text: String,
}

/// [`to_display`] with `inserts` (sorted): an offset after an insert's `at`
/// is after its text.
pub fn to_display_with(hidden: &[Range<usize>], inserts: &[Insert], offset: usize) -> usize {
    let before: usize = inserts
        .iter()
        .take_while(|i| i.at < offset)
        .map(|i| i.text.len())
        .sum();
    to_display(hidden, offset) + before
}

/// [`to_buffer`] with `inserts`: inside an insert's text, the offset of its
/// `at` (before the formula).
pub fn to_buffer_with(hidden: &[Range<usize>], inserts: &[Insert], display: usize) -> usize {
    let mut d = display;
    let mut shift = 0;
    for i in inserts {
        let start = to_display(hidden, i.at) + shift;
        if display < start {
            break;
        }
        if display < start + i.text.len() {
            return i.at;
        }
        d -= i.text.len();
        shift += i.text.len();
    }
    to_buffer(hidden, d)
}

/// [`shown_text`] with `inserts` (sorted) put in.
pub fn shown_text_with(line: &str, hidden: &[Range<usize>], inserts: &[Insert]) -> String {
    let mut out = shown_text(line, hidden);
    for i in inserts.iter().rev() {
        out.insert_str(to_display(hidden, i.at), &i.text);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_formulas_room_stands_in_for_its_hidden_source() {
        // "a $x$ b": the source hidden, two spaces of room in its place.
        let line = "a $x$ b";
        let hidden = [2..5];
        let inserts = [Insert {
            at: 2,
            text: "··".into(),
        }];
        assert_eq!(shown_text_with(line, &hidden, &inserts), "a ·· b");
        assert_eq!(
            to_display_with(&hidden, &inserts, 2),
            2,
            "before the formula"
        );
        assert_eq!(
            to_display_with(&hidden, &inserts, 5),
            2 + "··".len(),
            "after it"
        );
        assert_eq!(to_display_with(&hidden, &inserts, 7), 4 + "··".len());
        assert_eq!(to_buffer_with(&hidden, &inserts, 1), 1);
        assert_eq!(
            to_buffer_with(&hidden, &inserts, 3),
            2,
            "on it: before its source"
        );
        assert_eq!(to_buffer_with(&hidden, &inserts, 2 + "··".len()), 5);
        assert_eq!(to_buffer_with(&hidden, &inserts, 2 + "··".len() + 2), 7);
    }

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
