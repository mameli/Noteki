//! Which words the spell checker marks: those of the prose. Not code, the
//! Frontmatter, `$$` blocks, formulas, links, Wikilinks, Tags, Comments or
//! footnote labels; not the word being typed (the cursor touches it), as in
//! the Mac's text views. Each line is checked once while its text stays the
//! same; the lines not checked yet go to the checker in one call.

use std::collections::HashMap;
use std::ops::Range;
use std::rc::Rc;

use crate::core::markdown::{self, BlockKind, MdSpanKind};

/// The misspelled words found per line text, relative to the line.
pub type Checked = HashMap<String, Rc<[Range<usize>]>>;

/// The parts of a line of prose that are not prose.
fn not_prose(line: &markdown::Line) -> Vec<Range<usize>> {
    let mut spans = Vec::new();
    markdown::line_spans(line, None, None, &Default::default(), &mut spans);
    let mut out: Vec<Range<usize>> = spans
        .into_iter()
        .filter(|s| {
            matches!(
                s.kind,
                MdSpanKind::InlineCode
                    | MdSpanKind::CodeBlock
                    | MdSpanKind::WikilinkResolved
                    | MdSpanKind::WikilinkUnresolved
                    | MdSpanKind::MarkdownLink
                    | MdSpanKind::Tag
                    | MdSpanKind::TagChip
                    | MdSpanKind::Comment
                    | MdSpanKind::FrontmatterKey
                    | MdSpanKind::Footnote
                    | MdSpanKind::MathError
            )
        })
        .map(|s| s.range.start - line.start..s.range.end - line.start)
        .collect();
    out.extend(
        crate::core::editor::figures::inline_formulas(line.text)
            .into_iter()
            .map(|(r, _, _)| r),
    );
    // A web address written bare.
    for (i, _) in line.text.match_indices("http") {
        let rest = &line.text[i..];
        if rest.starts_with("http://") || rest.starts_with("https://") {
            let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
            out.push(i..i + end);
        }
    }
    out
}

/// The misspelled words of `text` (byte ranges) as `check` finds them in its
/// prose, `checked` keeping what each line gave; none touching `cursor`.
/// While `typing`, the cursor's line is not checked (nor marked): it changes
/// at every key, and the Mac's checker took a sixth of each; it is at the
/// first pause.
pub fn misspelled(
    text: &str,
    cursor: Option<usize>,
    typing: bool,
    checked: &mut Checked,
    check: impl Fn(&str) -> Vec<Range<usize>>,
) -> Vec<Range<usize>> {
    let lines: Vec<_> = markdown::lines(text)
        .into_iter()
        .filter(|l| l.kind == BlockKind::Text && !l.text.trim().is_empty())
        .collect();
    // The lines not checked yet, in one call: joined, then split again.
    let new: Vec<&markdown::Line> = {
        let mut seen = std::collections::HashSet::new();
        lines
            .iter()
            .filter(|l| !checked.contains_key(l.text) && seen.insert(l.text))
            .filter(|l| {
                let being_typed =
                    typing && cursor.is_some_and(|c| l.start <= c && c <= l.start + l.text.len());
                !being_typed
            })
            .collect()
    };
    if !new.is_empty() {
        let mut joined = String::new();
        let mut starts = Vec::with_capacity(new.len());
        for l in &new {
            starts.push(joined.len());
            joined.push_str(l.text);
            joined.push('\n');
        }
        let found = check(&joined);
        for (n, l) in new.iter().enumerate() {
            let (start, end) = (starts[n], starts[n] + l.text.len());
            let skip = not_prose(l);
            let words: Rc<[Range<usize>]> = found
                .iter()
                .filter(|r| r.start >= start && r.end <= end)
                .map(|r| r.start - start..r.end - start)
                .filter(|r| !skip.iter().any(|s| s.start < r.end && r.start < s.end))
                .collect();
            checked.insert(l.text.to_string(), words);
        }
    }
    let mut kept = Checked::with_capacity(lines.len());
    let mut out = Vec::new();
    for l in &lines {
        let Some(words) = checked.get(l.text) else {
            continue;
        };
        for r in words.iter() {
            let r = l.start + r.start..l.start + r.end;
            if cursor.is_some_and(|c| r.start <= c && c <= r.end) {
                continue;
            }
            out.push(r);
        }
        kept.insert(l.text.to_string(), words.clone());
    }
    // Lines no longer in the text are forgotten.
    *checked = kept;
    out
}

/// The word of `text` that ends at `end` (letters, digits, `'`), if any.
pub fn word_before(text: &str, end: usize) -> Option<Range<usize>> {
    let start = text[..end]
        .char_indices()
        .rev()
        .take_while(|(_, c)| c.is_alphanumeric() || *c == '\'' || *c == '’')
        .last()
        .map(|(i, _)| i)?;
    (start < end).then_some(start..end)
}

/// The sentence `word` is in: from after the last `.`, `!`, `?` or line
/// break before it to the next one after it.
pub fn sentence_around(text: &str, word: Range<usize>) -> Range<usize> {
    let ends = |c: char| matches!(c, '.' | '!' | '?' | '\n');
    let start = text[..word.start].rfind(ends).map_or(0, |i| i + 1);
    let end = text[word.end..]
        .find(ends)
        .map_or(text.len(), |i| word.end + i + 1);
    start..end
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    /// A checker that knows one wrong word: "erore".
    fn check(text: &str) -> Vec<Range<usize>> {
        text.match_indices("erore")
            .map(|(i, w)| i..i + w.len())
            .collect()
    }

    #[test]
    fn only_the_prose_is_checked() {
        let text = "---\ntags: erore\n---\nUn erore qui `erore` [[erore]] #erore %%erore%% $erore$\n```\nerore\n```\nhttps://x.y/erore e [erore](https://erore.it)";
        let mut checked = Checked::new();
        let found = misspelled(text, None, false, &mut checked, check);
        let first = text.find("Un erore").unwrap() + 3;
        assert_eq!(found, vec![first..first + 5]);
    }

    #[test]
    fn a_line_is_checked_once_and_the_word_being_typed_is_left() {
        let calls = Cell::new(0);
        let counting = |t: &str| {
            calls.set(calls.get() + 1);
            check(t)
        };
        let mut checked = Checked::new();
        let text = "erore uno\nerore due";
        assert_eq!(
            misspelled(text, None, false, &mut checked, counting).len(),
            2
        );
        assert_eq!(calls.get(), 1, "both lines in one call");
        let text2 = "erore uno\nerore due e tre";
        assert_eq!(
            misspelled(text2, None, false, &mut checked, counting).len(),
            2
        );
        assert_eq!(calls.get(), 2, "only the changed line again");
        assert_eq!(
            misspelled(text2, Some(3), false, &mut checked, counting).len(),
            1,
            "the cursor's word left"
        );
        assert_eq!(calls.get(), 2);
    }

    #[test]
    fn the_word_before_the_cursor() {
        assert_eq!(word_before("un erore", 8), Some(3..8));
        assert_eq!(word_before("l'erore", 7), Some(0..7));
        assert_eq!(word_before("fine. ", 6), None);
    }

    #[test]
    fn a_word_s_sentence_is_found_between_its_full_stops() {
        let text = "Questo ha un errrore. This sentense is wrong.\nFine";
        let w = text.find("errrore").unwrap();
        assert_eq!(
            &text[sentence_around(text, w..w + 7)],
            "Questo ha un errrore."
        );
        let w = text.find("sentense").unwrap();
        assert_eq!(
            &text[sentence_around(text, w..w + 8)],
            " This sentense is wrong."
        );
    }

    #[test]
    fn the_line_being_typed_is_checked_at_the_pause() {
        let text = "Prima riga erorre\n\nSeconda erorre qui";
        let asked = std::cell::RefCell::new(Vec::new());
        let check = |t: &str| {
            asked.borrow_mut().push(t.to_string());
            t.match_indices("erorre")
                .map(|(i, w)| i..i + w.len())
                .collect::<Vec<_>>()
        };
        let mut checked = Checked::default();
        let cursor = Some(text.len() - 1);
        let typing = misspelled(text, cursor, true, &mut checked, check);
        assert_eq!(typing.len(), 1, "only the other line: {typing:?}");
        assert!(!asked.borrow().iter().any(|t| t.contains("Seconda")));
        let paused = misspelled(text, cursor, false, &mut checked, check);
        assert_eq!(paused.len(), 2);
    }
}
