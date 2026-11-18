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
}
