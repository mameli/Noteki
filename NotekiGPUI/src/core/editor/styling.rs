//! Which Live preview styles fall on which line, and which lines are
//! Headings: what the editor needs to shape each line on its own.

use std::ops::Range;

use crate::core::markdown::{lines, parse_atx_heading, BlockKind, MdSpan, MdSpanKind};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LineStyle {
    /// The level of the ATX Heading on this line (`#` to `######`), drawn larger.
    pub heading: Option<u8>,
    /// Ranges of the line, relative to its start (so a line keeps an equal
    /// style when an edit above moves it), and the span kinds covering each,
    /// in the order the scanner gave them: a later kind is drawn over an earlier.
    pub segments: Vec<(Range<usize>, Vec<MdSpanKind>)>,
}

/// One [`LineStyle`] per logical line of `text` (as `lines` splits it).
/// A span across lines is cut at each line's end.
pub fn style_lines(text: &str, spans: &[MdSpan]) -> Vec<LineStyle> {
    let lines = lines(text);
    let mut clipped: Vec<Vec<(Range<usize>, MdSpanKind)>> = vec![Vec::new(); lines.len()];
    for span in spans {
        let first = lines
            .partition_point(|l| l.start <= span.range.start)
            .saturating_sub(1);
        for (i, line) in lines.iter().enumerate().skip(first) {
            if line.start >= span.range.end {
                break;
            }
            let r = line.range();
            let (a, b) = (span.range.start.max(r.start), span.range.end.min(r.end));
            if a < b {
                clipped[i].push((a - r.start..b - r.start, span.kind));
            }
        }
    }
    lines
        .iter()
        .zip(clipped)
        .map(|(line, spans)| LineStyle {
            heading: (line.kind == BlockKind::Text)
                .then(|| parse_atx_heading(line.text.trim_start()).map(|(level, _)| level))
                .flatten(),
            segments: segments(spans),
        })
        .collect()
}

/// Cuts overlapping ranges into pieces that each carry every kind over them.
fn segments(spans: Vec<(Range<usize>, MdSpanKind)>) -> Vec<(Range<usize>, Vec<MdSpanKind>)> {
    let mut bounds: Vec<usize> = spans.iter().flat_map(|(r, _)| [r.start, r.end]).collect();
    bounds.sort_unstable();
    bounds.dedup();
    bounds
        .windows(2)
        .filter_map(|w| {
            let kinds: Vec<MdSpanKind> = spans
                .iter()
                .filter(|(r, _)| r.start <= w[0] && w[1] <= r.end)
                .map(|(_, k)| *k)
                .collect();
            (!kinds.is_empty()).then(|| (w[0]..w[1], kinds))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::markdown::compute_markdown_spans;
    use crate::core::wikilink::VaultIndex;

    fn styles(text: &str, cursor_line: usize) -> Vec<LineStyle> {
        style_lines(
            text,
            &compute_markdown_spans(text, cursor_line, None, &VaultIndex::default()),
        )
    }

    #[test]
    fn headings_have_their_level_even_while_empty() {
        let s = styles("# Uno\ntesto\n### Tre\n#\n#tag\n####### sette", 99);
        let levels: Vec<_> = s.iter().map(|l| l.heading).collect();
        assert_eq!(levels, vec![Some(1), None, Some(3), Some(1), None, None]);
    }

    #[test]
    fn a_hash_in_code_or_frontmatter_is_no_heading() {
        let s = styles("---\n# no: x\n---\n```\n# no\n```\n# sì", 99);
        let levels: Vec<_> = s.iter().map(|l| l.heading).collect();
        assert_eq!(levels, vec![None, None, None, None, None, None, Some(1)]);
    }

    #[test]
    fn overlapping_spans_are_cut_into_segments() {
        // Off the cursor's line: the `## ` is a faded marker, the rest Heading
        // text with a bold part inside.
        let text = "## Titolo **forte**";
        let s = styles(text, 99);
        let seg = &s[0].segments;
        assert_eq!(seg[0], (0..3, vec![MdSpanKind::SyntaxMarker]));
        let forte = text.find("forte").unwrap(); // the first line starts at 0
        let bold = seg
            .iter()
            .find(|(r, _)| r.start == forte)
            .expect("a segment for the bold text");
        assert!(bold.1.contains(&MdSpanKind::Heading(2)));
        assert!(bold.1.contains(&MdSpanKind::Bold));
        // Segments never overlap and stay inside the line.
        for w in seg.windows(2) {
            assert!(w[0].0.end <= w[1].0.start);
        }
        assert!(seg.last().unwrap().0.end <= text.len());
    }

    #[test]
    fn a_span_across_lines_is_cut_at_each_line() {
        let text = "ab\ncd";
        let spans = vec![MdSpan {
            range: 1..5,
            kind: MdSpanKind::Comment,
        }];
        let s = style_lines(text, &spans);
        assert_eq!(s[0].segments, vec![(1..2, vec![MdSpanKind::Comment])]);
        assert_eq!(s[1].segments, vec![(0..2, vec![MdSpanKind::Comment])]);
    }

    #[test]
    fn a_line_moved_by_an_edit_above_keeps_an_equal_style() {
        let before = styles(
            "uno
**due**",
            99,
        );
        let after = styles(
            "uno più lungo
**due**",
            99,
        );
        assert_eq!(before[1], after[1]);
    }
}
