//! `[[` completion (SPEC › Wikilinks): the name being typed in a Wikilink,
//! the Notes that match it, and what choosing one writes.

use std::ops::Range;

use super::format::Change;
use crate::core::markdown::{lines, BlockKind};
use crate::core::wikilink::LinkCompletion;

/// The Wikilink name the cursor is typing: `range` is the name to replace
/// (from after `[[` to the `|`, `#` or `]]` that ends it, or the cursor),
/// `query` what is typed of it, `closed` whether `]]` already follows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkQuery {
    pub range: Range<usize>,
    pub query: String,
    pub closed: bool,
}

/// The name of the Wikilink the cursor is in, if it is typing one: after a
/// `[[` on its line, before any `|` or `#` of it. Not in code.
pub fn link_query(text: &str, cursor: usize) -> Option<LinkQuery> {
    let line = lines(text)
        .into_iter()
        .find(|l| l.start <= cursor && cursor <= l.start + l.text.len())?;
    if line.kind != BlockKind::Text {
        return None;
    }
    let at = cursor - line.start;
    let before = &line.text[..at];
    let open = before.rfind("[[")?;
    let typed = &before[open + 2..];
    // Past the name (an Alias or a heading), or in a link already closed.
    if typed.contains(['|', '#', ']']) || before[..open].matches('`').count() % 2 == 1 {
        return None;
    }
    let after = &line.text[at..];
    let name_end = after.find(['|', '#', ']', '[']).unwrap_or(after.len());
    let closed = after[name_end..].starts_with(['|', '#']) || after[name_end..].starts_with("]]");
    let end = if closed { at + name_end } else { at };
    Some(LinkQuery {
        range: line.start + open + 2..line.start + end,
        query: typed.to_string(),
        closed,
    })
}

/// The Notes matching `query`, best first: names starting with it, then
/// holding it, then holding its letters in order; within each, in the order
/// given (the most recently opened first). Ignores case.
pub fn rank<'a>(candidates: &'a [LinkCompletion], query: &str) -> Vec<&'a LinkCompletion> {
    let q = query.to_lowercase();
    let in_order = |name: &str| {
        let mut letters = name.chars();
        q.chars().all(|c| letters.any(|n| n == c))
    };
    let mut scored: Vec<(u8, usize, &LinkCompletion)> = candidates
        .iter()
        .enumerate()
        .filter_map(|(i, c)| {
            let name = c.name.to_lowercase();
            let score = if name.starts_with(&q) {
                0
            } else if name.contains(&q) {
                1
            } else if in_order(&name) {
                2
            } else {
                return None;
            };
            Some((score, i, c))
        })
        .collect();
    scored.sort_by_key(|&(s, i, _)| (s, i));
    scored.into_iter().map(|(_, _, c)| c).collect()
}

/// What choosing `link` writes: the name in place of what was typed, and
/// `]]` when the link was not closed; the cursor after the link.
pub fn choose(q: &LinkQuery, link: &str) -> Change {
    let text = if q.closed {
        link.to_string()
    } else {
        format!("{link}]]")
    };
    let end = q.range.start + text.len() + if q.closed { 2 } else { 0 };
    Change {
        range: q.range.clone(),
        text,
        selection: end..end,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(name: &str) -> LinkCompletion {
        LinkCompletion {
            name: name.into(),
            folder: String::new(),
            link: name.into(),
        }
    }

    #[test]
    fn the_name_typed_after_double_brackets() {
        let t = "Vedi [[Rea";
        let q = link_query(t, t.len()).unwrap();
        assert_eq!((q.range, q.query.as_str(), q.closed), (7..10, "Rea", false));
        assert_eq!(link_query("Vedi [Rea", 9), None);
        assert_eq!(link_query("[[a|b", 5), None, "in the Alias");
        assert_eq!(link_query("[[a]] dopo", 9), None, "after a closed link");
        assert_eq!(link_query("```\n[[a\n```", 7), None, "in code");
        assert_eq!(link_query("`[[a", 4), None, "in inline code");
    }

    #[test]
    fn inside_a_closed_link_the_name_is_replaced_and_the_rest_kept() {
        let t = "[[Vecchia|Alias]]";
        let q = link_query(t, 4).unwrap();
        assert_eq!((q.range.clone(), q.closed), (2..9, true));
        let c = choose(&q, "Nuova");
        let mut out = t.to_string();
        out.replace_range(c.range.clone(), &c.text);
        assert_eq!(out, "[[Nuova|Alias]]");
    }

    #[test]
    fn choosing_closes_the_link_and_puts_the_cursor_after_it() {
        let t = "Vedi [[Rea";
        let q = link_query(t, t.len()).unwrap();
        let c = choose(&q, "Reading list");
        let mut out = t.to_string();
        out.replace_range(c.range.clone(), &c.text);
        assert_eq!(out, "Vedi [[Reading list]]");
        assert_eq!(c.selection, out.len()..out.len());
    }

    #[test]
    fn ranking_prefers_a_prefix_then_a_part_then_letters_in_order() {
        let notes = [
            note("Agenda riunione"),
            note("Riunione"),
            note("Diario"),
            note("Reading"),
        ];
        let names: Vec<&str> = rank(&notes, "riu")
            .iter()
            .map(|c| c.name.as_str())
            .collect();
        assert_eq!(names, vec!["Riunione", "Agenda riunione"]);
        let names: Vec<&str> = rank(&notes, "rdg")
            .iter()
            .map(|c| c.name.as_str())
            .collect();
        assert_eq!(names, vec!["Reading"]);
        assert_eq!(rank(&notes, "").len(), 4, "everything, in the order given");
    }
}
