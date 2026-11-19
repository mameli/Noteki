//! The Frontmatter as a Properties table (SPEC › Frontmatter, ADR 0009): what
//! each of its lines is as a row, read line by line, never rewritten.

use std::ops::Range;

use crate::core::frontmatter::{parse_frontmatter, FrontmatterReading};
use crate::core::markdown::{BlockKind, Line};

/// A Frontmatter line as the table draws it. Ranges are relative to the line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PropertyLine {
    /// The opening `---`: the table's top, "Properties".
    Top,
    /// `key: value` (the value may be empty).
    Row {
        key: Range<usize>,
        value: Range<usize>,
        /// The Tags of a `tags` row, as chips.
        tags: Vec<Range<usize>>,
        /// What of the value is not shown: its quotes, a list's `[`, `]` and commas.
        marks: Vec<Range<usize>>,
    },
    /// `  - item` under a key; `tag` when the key is `tags`.
    Item { value: Range<usize>, tag: bool },
    /// The closing `---`: the table's bottom.
    Bottom,
    /// A key whose list (one item per line, not `tags`) is drawn side by
    /// side on its row, as chips wrapping in the value's column, while the
    /// cursor is out of the Frontmatter: `participants:` and fifty names
    /// under it. The items as shown, without their quotes.
    List {
        key: Range<usize>,
        items: Vec<String>,
    },
    /// A line of a `List` drawn on its key's row: not drawn.
    Folded,
}

fn trimmed(line: &str, from: usize) -> Range<usize> {
    let rest = &line[from..];
    let start = from + (rest.len() - rest.trim_start().len());
    start..start + line[start..].trim_end().len()
}

/// The Tags written in a `tags` value: `a, b`, `[a, b]` or `a`.
fn tag_ranges(line: &str, value: &Range<usize>) -> Vec<Range<usize>> {
    let v = &line[value.clone()];
    let inner = v.strip_prefix('[').and_then(|s| s.strip_suffix(']'));
    let (base, inner) = match inner {
        Some(i) => (value.start + 1, i),
        None => (value.start, v),
    };
    let mut out = Vec::new();
    let mut at = 0;
    for part in inner.split(',') {
        let lead = part.len() - part.trim_start().len();
        let t = part.trim();
        if !t.is_empty() {
            let start = base + at + lead;
            out.push(start..start + t.len());
        }
        at += part.len() + 1;
    }
    out
}

/// A value's quotes, or a list's brackets and commas (each comma alone: the
/// space after it stays, between the chips).
fn value_marks(line: &str, value: &Range<usize>, list: bool) -> Vec<Range<usize>> {
    let v = &line[value.clone()];
    let quoted = v.len() >= 2
        && ((v.starts_with('"') && v.ends_with('"')) || (v.starts_with('\'') && v.ends_with('\'')));
    if quoted && !list {
        return vec![value.start..value.start + 1, value.end - 1..value.end];
    }
    if !list {
        return Vec::new();
    }
    v.char_indices()
        .filter(|(_, c)| matches!(c, '[' | ']' | ','))
        .map(|(i, _)| value.start + i..value.start + i + 1)
        .collect()
}

/// The Frontmatter's lines as table rows: `(line index, row)`. Nothing when
/// there is none, or when Noteki cannot read it (it then stays as source).
pub fn property_lines(text: &str, lines: &[Line]) -> Vec<(usize, PropertyLine)> {
    if !matches!(
        parse_frontmatter(text),
        Some(FrontmatterReading::Properties { .. })
    ) {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut key: Option<String> = None;
    let mut opened = false;
    for (i, line) in lines.iter().enumerate() {
        match line.kind {
            BlockKind::FrontmatterDelimiter if !opened => {
                out.push((i, PropertyLine::Top));
                opened = true;
            }
            BlockKind::FrontmatterDelimiter => {
                out.push((i, PropertyLine::Bottom));
                break;
            }
            BlockKind::Frontmatter => {
                let t = line.text;
                let item = t
                    .trim_start()
                    .strip_prefix("- ")
                    .or_else(|| (t.trim() == "-").then_some(""));
                if let (Some(_), true) = (item, t.starts_with([' ', '\t', '-'])) {
                    let dash = t.find('-').unwrap_or(0);
                    let value = trimmed(t, (dash + 1).min(t.len()));
                    let tag = key.as_deref() == Some("tags");
                    out.push((i, PropertyLine::Item { value, tag }));
                } else if let Some(colon) = t.find(':').filter(|_| !t.starts_with([' ', '\t'])) {
                    let k = trimmed(t, 0).start..colon;
                    let value = trimmed(t, colon + 1);
                    let name = t[k.clone()].trim().to_string();
                    let tags = if name == "tags" {
                        tag_ranges(t, &value)
                    } else {
                        Vec::new()
                    };
                    let marks = value_marks(t, &value, !tags.is_empty());
                    out.push((
                        i,
                        PropertyLine::Row {
                            key: k,
                            value,
                            tags,
                            marks,
                        },
                    ));
                    key = Some(name);
                }
                // Blank lines and comments: drawn as they are (no row).
            }
            _ => break,
        }
    }
    out
}

/// `rows` with each list written one item per line under its key (not
/// `tags`, which has its chips) drawn on the key's row: the SPEC's one place
/// where the Properties change height on entering, as a long list one item
/// per line is too tall to read. The lines from the key to its last item
/// (blank lines and comments between them too) fold.
pub fn fold_lists(lines: &[Line], rows: Vec<(usize, PropertyLine)>) -> Vec<(usize, PropertyLine)> {
    let mut out: Vec<(usize, PropertyLine)> = Vec::with_capacity(rows.len());
    let mut i = 0;
    while i < rows.len() {
        let (at, row) = &rows[i];
        let list_key = match row {
            PropertyLine::Row {
                key, value, tags, ..
            } if value.is_empty() && tags.is_empty() && &lines[*at].text[key.clone()] != "tags" => {
                Some(key.clone())
            }
            _ => None,
        };
        let items = rows[i + 1..]
            .iter()
            .take_while(|(_, r)| matches!(r, PropertyLine::Item { tag: false, .. }))
            .count();
        let Some(key) = list_key.filter(|_| items > 0) else {
            out.push(rows[i].clone());
            i += 1;
            continue;
        };
        let shown: Vec<String> = rows[i + 1..=i + items]
            .iter()
            .filter_map(|(j, r)| match r {
                PropertyLine::Item { value, .. } => {
                    let v = &lines[*j].text[value.clone()];
                    let unquoted = [('"', '"'), ('\'', '\'')]
                        .iter()
                        .find_map(|(a, b)| v.strip_prefix(*a)?.strip_suffix(*b))
                        .unwrap_or(v);
                    (!unquoted.is_empty()).then(|| unquoted.to_string())
                }
                _ => None,
            })
            .collect();
        out.push((*at, PropertyLine::List { key, items: shown }));
        let last = rows[i + items].0;
        out.extend((at + 1..=last).map(|j| (j, PropertyLine::Folded)));
        i += items + 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::markdown::lines;

    #[test]
    fn rows_items_and_tags() {
        let text = "---\ntitle: Una nota\ntags: [lavoro, idee]\nauthors:\n  - Anna\n  - Bruno\ntags2:\n---\ntesto";
        let l = lines(text);
        let rows = property_lines(text, &l);
        assert_eq!(rows[0], (0, PropertyLine::Top));
        match &rows[1].1 {
            PropertyLine::Row { key, value, .. } => {
                assert_eq!(&l[1].text[key.clone()], "title");
                assert_eq!(&l[1].text[value.clone()], "Una nota");
            }
            other => panic!("{other:?}"),
        }
        match &rows[2].1 {
            PropertyLine::Row { tags, .. } => {
                let t: Vec<&str> = tags.iter().map(|r| &l[2].text[r.clone()]).collect();
                assert_eq!(t, vec!["lavoro", "idee"]);
            }
            other => panic!("{other:?}"),
        }
        match &rows[4].1 {
            PropertyLine::Item { value, tag } => {
                assert_eq!(&l[4].text[value.clone()], "Anna");
                assert!(!tag);
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(rows.last().unwrap(), &(7, PropertyLine::Bottom));
        match &rows[2].1 {
            PropertyLine::Row { marks, .. } => {
                let m: Vec<&str> = marks.iter().map(|r| &l[2].text[r.clone()]).collect();
                assert_eq!(m, vec!["[", ",", "]"], "brackets and commas hidden");
            }
            other => panic!("{other:?}"),
        }
        let q = "---\ntitle: \"Con: due punti\"\n---\n";
        match &property_lines(q, &lines(q))[1].1 {
            PropertyLine::Row { marks, .. } => assert_eq!(marks.len(), 2, "the quotes hidden"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn tags_as_a_list_are_chips_and_unreadable_yaml_stays_source() {
        let text = "---\ntags:\n  - a\n  - b\n---\n";
        let rows = property_lines(text, &lines(text));
        assert!(matches!(rows[2].1, PropertyLine::Item { tag: true, .. }));
        assert!(property_lines(
            "---\nkey: {map: 1}\n---\n",
            &lines("---\nkey: {map: 1}\n---\n")
        )
        .is_empty());
        assert!(property_lines("niente", &lines("niente")).is_empty());
    }

    #[test]
    fn a_list_under_its_key_is_drawn_on_the_key_s_row() {
        let text = "---\nparticipants:\n  - \"Anna Rossi\"\n\n  - Bruno\n  - ''\ntags:\n  - a\nempty:\ntitle: x\n---\n";
        let l = lines(text);
        let rows = fold_lists(&l, property_lines(text, &l));
        match &rows[1] {
            (1, PropertyLine::List { key, items }) => {
                assert_eq!(&l[1].text[key.clone()], "participants");
                assert_eq!(items, &vec!["Anna Rossi".to_string(), "Bruno".into()]);
            }
            other => panic!("{other:?}"),
        }
        let folded: Vec<usize> = rows
            .iter()
            .filter(|(_, r)| *r == PropertyLine::Folded)
            .map(|(i, _)| *i)
            .collect();
        assert_eq!(
            folded,
            vec![2, 3, 4, 5],
            "the items and the blank line between"
        );
        assert!(
            matches!(
                rows.iter().find(|(i, _)| *i == 7),
                Some((_, PropertyLine::Item { tag: true, .. }))
            ),
            "tags keep their chips"
        );
        assert!(matches!(
            rows.iter().find(|(i, _)| *i == 8),
            Some((_, PropertyLine::Row { .. }))
        ));
    }
}
