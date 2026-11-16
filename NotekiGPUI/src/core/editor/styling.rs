//! Which Live preview styles fall on which line, and which lines are
//! Headings: what the editor needs to shape each line on its own.

use std::ops::Range;

use crate::core::markdown::{
    lines, list_item, parse_atx_heading, quote_prefix, BlockKind, Line, ListKind, MdSpan,
    MdSpanKind,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LineStyle {
    /// The level of the ATX Heading on this line (`#` to `######`), drawn larger.
    pub heading: Option<u8>,
    /// Ranges of the line, relative to its start (so a line keeps an equal
    /// style when an edit above moves it), and the span kinds covering each,
    /// in the order the scanner gave them: a later kind is drawn over an earlier.
    pub segments: Vec<(Range<usize>, Vec<MdSpanKind>)>,
    /// Ranges of the line (relative to its start, sorted, disjoint) that Live
    /// preview hides: the scanner's syntax marks.
    pub hidden: Vec<Range<usize>>,
    /// What the editor paints around the line's text, never changing its height.
    pub decoration: Option<Decoration>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decoration {
    /// A thematic break, drawn as a rule while its marks are hidden.
    Rule,
    /// A quote's bars on the left, one per level, the text indented after them.
    Quote { depth: u8 },
    /// A list item's bullet, number or checkbox, drawn before its text while
    /// its marks are hidden; the text indented by its level.
    List { level: u8, kind: ListKind },
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
            push_clipped(&mut clipped[i], line, span);
        }
    }
    lines
        .iter()
        .zip(clipped)
        .map(|(line, spans)| style_of(line, spans))
        .collect()
}

/// The style of one line from its own spans (offsets in the whole text), as
/// [`style_lines`] gives it: to restyle only the cursor's line when it moves.
pub fn style_line(line: &Line, spans: &[MdSpan]) -> LineStyle {
    let mut clipped = Vec::new();
    for span in spans {
        push_clipped(&mut clipped, line, span);
    }
    style_of(line, clipped)
}

fn push_clipped(out: &mut Vec<(Range<usize>, MdSpanKind)>, line: &Line, span: &MdSpan) {
    let r = line.range();
    let (a, b) = (span.range.start.max(r.start), span.range.end.min(r.end));
    if a < b {
        out.push((a - r.start..b - r.start, span.kind));
    }
}

fn style_of(line: &Line, spans: Vec<(Range<usize>, MdSpanKind)>) -> LineStyle {
    let heading = (line.kind == BlockKind::Text)
        .then(|| parse_atx_heading(line.text.trim_start()).map(|(level, _)| level))
        .flatten();
    let mut hidden: Vec<Range<usize>> = spans
        .iter()
        .filter(|(_, k)| *k == MdSpanKind::SyntaxMarker)
        .map(|(r, _)| r.clone())
        .collect();
    hidden.sort_unstable_by_key(|r| r.start);
    // Marks may touch or nest (an escape inside a link's label): merge them.
    let mut merged: Vec<Range<usize>> = Vec::with_capacity(hidden.len());
    for r in hidden {
        match merged.last_mut() {
            Some(last) if r.start <= last.end => last.end = last.end.max(r.end),
            _ => merged.push(r),
        }
    }
    let decoration = match line.kind {
        // A rule shows as a rule only while its marks are hidden.
        BlockKind::Rule if merged.first() == Some(&(0..line.text.len())) => Some(Decoration::Rule),
        BlockKind::Text => match (quote_prefix(line.text), list_item(line.text)) {
            (Some((depth, _)), _) => Some(Decoration::Quote { depth }),
            (None, Some(item)) if merged.first() == Some(&(0..item.text_start)) => {
                Some(Decoration::List {
                    level: item.level,
                    kind: item.kind,
                })
            }
            _ => None,
        },
        _ => None,
    };
    LineStyle {
        heading,
        segments: segments(spans),
        hidden: merged,
        decoration,
    }
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
// A list holding one hidden range is what these tests are about.
#[allow(clippy::single_range_in_vec_init)]
mod tests {
    use super::*;
    use crate::core::markdown::compute_markdown_spans;
    use crate::core::wikilink::VaultIndex;

    fn styles(text: &str, cursor: Option<usize>) -> Vec<LineStyle> {
        style_lines(
            text,
            &compute_markdown_spans(text, cursor, None, &VaultIndex::default()),
        )
    }

    #[test]
    fn headings_have_their_level_even_while_empty() {
        let s = styles("# Uno\ntesto\n### Tre\n#\n#tag\n####### sette", None);
        let levels: Vec<_> = s.iter().map(|l| l.heading).collect();
        assert_eq!(levels, vec![Some(1), None, Some(3), Some(1), None, None]);
    }

    #[test]
    fn a_hash_in_code_or_frontmatter_is_no_heading() {
        let s = styles("---\n# no: x\n---\n```\n# no\n```\n# sì", None);
        let levels: Vec<_> = s.iter().map(|l| l.heading).collect();
        assert_eq!(levels, vec![None, None, None, None, None, None, Some(1)]);
    }

    #[test]
    fn overlapping_spans_are_cut_into_segments() {
        // Off the cursor's line: the `## ` is a faded marker, the rest Heading
        // text with a bold part inside.
        let text = "## Titolo **forte**";
        let s = styles(text, None);
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
    fn heading_marks_are_hidden_off_the_cursor_line() {
        let s = styles("# Uno\n  ### Tre\n#\ntesto", None);
        assert_eq!(s[0].hidden, vec![0..2]);
        assert_eq!(s[1].hidden, vec![2..6], "the indent stays");
        assert_eq!(
            s[2].hidden,
            vec![0..1],
            "an empty Heading has no space to hide"
        );
        assert!(s[3].hidden.is_empty());
        let on_cursor = styles("# Uno\ntesto", Some(4));
        assert!(
            on_cursor[0].hidden.is_empty(),
            "shown with the cursor anywhere on the line"
        );
    }

    #[test]
    fn inline_marks_show_only_in_the_element_the_cursor_is_in() {
        let text = "uno **due** e `tre` e [qui](http://x)";
        let hidden = |cursor| styles(text, cursor)[0].hidden.clone();
        let all = hidden(None);
        let starts: Vec<usize> = all.iter().map(|r| r.start).collect();
        assert_eq!(all.len(), 6, "** ** ` ` [ ](http://x): {all:?}");
        assert_eq!(starts[0], 4);
        // In the bold (and at its edges): its marks show, the others stay hidden.
        for c in [4, 7, 11] {
            let h = hidden(Some(c));
            assert_eq!(h.len(), 4, "cursor at {c}: {h:?}");
            assert!(h.iter().all(|r| r.start >= 14), "{h:?}");
        }
        // Off every element: everything hidden, even on the cursor's line.
        assert_eq!(hidden(Some(1)), all);
    }

    #[test]
    fn a_wikilink_hides_its_target_before_an_alias_and_a_comment_its_marks() {
        let s = styles("[[Nota|Alias]] %%nascosto%%", None);
        assert_eq!(s[0].hidden, vec![0..7, 12..14, 15..17, 25..27]);
    }

    #[test]
    fn nested_and_tricky_marks() {
        let hidden = |t: &str| styles(t, None)[0].hidden.clone();
        assert_eq!(hidden("***x***"), vec![0..3, 4..7], "bold italic");
        assert_eq!(
            hidden("`a **b**`"),
            vec![0..1, 8..9],
            "code keeps its stars"
        );
        assert_eq!(
            hidden("[a](u \"t\")"),
            vec![0..1, 2..10],
            "a link with a title"
        );
        assert_eq!(
            hidden("| [[N\\|A]] |"),
            vec![2..7, 8..10],
            "an escaped bar in a table still shows the alias"
        );
    }

    #[test]
    fn rules_and_quotes_are_decorations() {
        let text = "testo\n\n---\nTitolo\n---\n> uno\n> > due\n>\n* * *";
        let s = styles(text, None);
        let deco: Vec<_> = s.iter().map(|l| l.decoration).collect();
        use Decoration::*;
        assert_eq!(
            deco,
            vec![
                None,
                None,
                Some(Rule),
                None, // a setext Heading's text
                None, // and its underline
                Some(Quote { depth: 1 }),
                Some(Quote { depth: 2 }),
                Some(Quote { depth: 1 }),
                Some(Rule),
            ]
        );
        assert_eq!(s[6].hidden, vec![0..4], "both `> ` hidden");
        // On the cursor's line: the rule's marks show, so no rule is drawn.
        let on = styles(text, Some(text.find("---").unwrap()));
        assert_eq!(on[2].decoration, None);
        assert!(on[2].hidden.is_empty());
    }

    #[test]
    fn list_items_hide_their_marks_until_the_cursor_touches_them() {
        let text = "- uno\n  - sotto\n3. tre\n- [ ] da fare\n- [x] fatto\n1. [ ] numero";
        let s = styles(text, None);
        let deco: Vec<_> = s.iter().map(|l| l.decoration).collect();
        use ListKind::*;
        let list = |level, kind| Some(Decoration::List { level, kind });
        assert_eq!(
            deco,
            vec![
                list(0, Bullet),
                list(1, Bullet),
                list(0, Number { n: 3, delim: b'.' }),
                list(
                    0,
                    Task {
                        checked: false,
                        bracket: 2
                    }
                ),
                list(
                    0,
                    Task {
                        checked: true,
                        bracket: 2
                    }
                ),
                list(0, Number { n: 1, delim: b'.' }),
            ]
        );
        assert_eq!(s[1].hidden, vec![0..4]);
        assert_eq!(s[3].hidden, vec![0..6], "`- [ ] ` hidden");
        assert!(s[4]
            .segments
            .iter()
            .any(|(_, k)| k.contains(&MdSpanKind::TaskDone)));
        // The cursor touching the marks shows them; further in the text, not.
        let at = |c| styles(text, Some(c))[0].clone();
        assert!(at(1).hidden.is_empty() && at(1).decoration.is_none());
        assert_eq!(at(4).hidden, vec![0..2]);
    }

    #[test]
    fn a_line_moved_by_an_edit_above_keeps_an_equal_style() {
        let before = styles(
            "uno
**due**",
            None,
        );
        let after = styles(
            "uno più lungo
**due**",
            None,
        );
        assert_eq!(before[1], after[1]);
    }
}
