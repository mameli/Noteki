//! The format commands (SPEC › Format), as changes of the text: each one
//! replacement and the selection after it, so it is one undo step. Each
//! toggles: again, the format goes.

use std::ops::Range;

use super::movement::{line_end, line_start, word_range_at};
use crate::core::markdown::{list_item, parse_atx_heading, ListKind};

/// One change of the text: `range` replaced by `text`, then `selection`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub range: Range<usize>,
    pub text: String,
    pub selection: Range<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wrap {
    Bold,
    Italic,
    Strikethrough,
    Code,
}

impl Wrap {
    fn mark(self) -> &'static str {
        match self {
            Wrap::Bold => "**",
            Wrap::Italic => "*",
            Wrap::Strikethrough => "~~",
            Wrap::Code => "`",
        }
    }
}

/// How many of `c` end `s`, and start `t`.
fn run_before(s: &str, c: char) -> usize {
    s.chars().rev().take_while(|&x| x == c).count()
}

fn run_after(s: &str, c: char) -> usize {
    s.chars().take_while(|&x| x == c).count()
}

/// Whether the marks of `wrap` are right around `a..b`. `*` and `**` share
/// their character: one star (or three) is italic, two (or three) bold.
fn wrapped(text: &str, a: usize, b: usize, wrap: Wrap) -> bool {
    let mark = wrap.mark();
    let c = mark.chars().next().unwrap_or('*');
    let (before, after) = (run_before(&text[..a], c), run_after(&text[b..], c));
    match wrap {
        Wrap::Italic => before % 2 == 1 && after % 2 == 1,
        Wrap::Bold => before >= 2 && after >= 2,
        _ => text[..a].ends_with(mark) && text[b..].starts_with(mark),
    }
}

/// ⌘B, ⌘I...: wraps the selection (or the word at the cursor) in the marks,
/// or takes them away when it already has them, around it or inside it.
pub fn toggle_wrap(text: &str, selection: Range<usize>, wrap: Wrap) -> Change {
    let mark = wrap.mark();
    let m = mark.len();
    let (a, b) = if selection.is_empty() {
        let w = word_range_at(text, selection.start);
        let is_word = text[w.clone()].chars().any(|c| c.is_alphanumeric());
        if is_word {
            (w.start, w.end)
        } else {
            (selection.start, selection.start)
        }
    } else {
        (selection.start, selection.end)
    };
    let inner = &text[a..b];
    if wrapped(text, a, b, wrap) {
        // The marks around: take them away.
        return Change {
            range: a - m..b + m,
            text: inner.to_string(),
            selection: a - m..b - m,
        };
    }
    if inner.len() >= 2 * m && inner.starts_with(mark) && inner.ends_with(mark) {
        // The marks selected with it: take them away.
        return Change {
            range: a..b,
            text: inner[m..inner.len() - m].to_string(),
            selection: a..b - 2 * m,
        };
    }
    Change {
        range: a..b,
        text: format!("{mark}{inner}{mark}"),
        selection: a + m..b + m,
    }
}

/// ⌘K: `[text](url)` of the selection (or the word at the cursor), the URL
/// from the clipboard when it holds one; else the cursor between the brackets
/// (no text) or the parentheses (no URL).
pub fn make_link(text: &str, selection: Range<usize>, clipboard: Option<&str>) -> Change {
    let (a, b) = if selection.is_empty() {
        let w = word_range_at(text, selection.start);
        if text[w.clone()].chars().any(|c| c.is_alphanumeric()) {
            (w.start, w.end)
        } else {
            (selection.start, selection.start)
        }
    } else {
        (selection.start, selection.end)
    };
    let label = &text[a..b];
    let url = clipboard
        .map(str::trim)
        .filter(|u| !u.contains(char::is_whitespace) && u.contains("://"))
        .unwrap_or("");
    let new = format!("[{label}]({url})");
    let cursor = if label.is_empty() {
        a + 1
    } else if url.is_empty() {
        a + new.len() - 1
    } else {
        a + new.len()
    };
    Change {
        range: a..b,
        text: new,
        selection: cursor..cursor,
    }
}

/// ⌘/: the selection in `%%`, or the whole line without one; again, they go.
pub fn toggle_comment(text: &str, selection: Range<usize>) -> Change {
    let sel = if selection.is_empty() {
        line_start(text, selection.start)..line_end(text, selection.start)
    } else {
        selection
    };
    let inner = &text[sel.clone()];
    if let Some(bare) = inner
        .strip_prefix("%%")
        .and_then(|s| s.strip_suffix("%%"))
        .filter(|_| inner.len() >= 4)
    {
        return Change {
            range: sel.clone(),
            text: bare.to_string(),
            selection: sel.start..sel.start + bare.len(),
        };
    }
    Change {
        range: sel.clone(),
        text: format!("%%{inner}%%"),
        selection: sel.start + 2..sel.end + 2,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineFormat {
    Heading(u8),
    Quote,
    Bullet,
    Numbered,
    Task,
    Indent,
    Outdent,
}

/// The new text of one line for `format`; `n` is its place among the lines
/// changed (for numbers).
fn format_line(line: &str, format: LineFormat, n: usize) -> String {
    match format {
        LineFormat::Heading(level) => {
            let trimmed = line.trim_start();
            let (current, body) = match parse_atx_heading(trimmed) {
                Some((l, _)) => {
                    let marks = usize::from(l);
                    let rest = &trimmed[marks..];
                    (Some(l), rest.strip_prefix([' ', '\t']).unwrap_or(rest))
                }
                None => (None, line),
            };
            // SPEC: again, the Heading goes.
            if current == Some(level) {
                body.to_string()
            } else {
                format!("{} {body}", "#".repeat(usize::from(level)))
            }
        }
        LineFormat::Quote => match line.strip_prefix("> ").or_else(|| line.strip_prefix('>')) {
            Some(rest) => rest.to_string(),
            None => format!("> {line}"),
        },
        LineFormat::Bullet | LineFormat::Numbered => {
            let (indent, kind, body) = match list_item(line) {
                Some(item) => {
                    let indent = &line[..line.len() - line.trim_start().len()];
                    (indent, Some(item.kind), &line[item.text_start..])
                }
                None => ("", None, line),
            };
            let same = matches!(
                (format, kind),
                (
                    LineFormat::Bullet,
                    Some(ListKind::Bullet | ListKind::Task { .. })
                ) | (LineFormat::Numbered, Some(ListKind::Number { .. }))
            );
            if same {
                // Again, the mark goes (a task's box with it).
                body.to_string()
            } else if format == LineFormat::Bullet {
                format!("{indent}- {body}")
            } else {
                format!("{indent}{}. {body}", n + 1)
            }
        }
        LineFormat::Task => match list_item(line) {
            Some(item) => match item.kind {
                ListKind::Task { checked, bracket } => {
                    let mut out = line.to_string();
                    out.replace_range(bracket + 1..bracket + 2, if checked { " " } else { "x" });
                    out
                }
                _ => format!(
                    "{}[ ] {}",
                    &line[..item.text_start],
                    &line[item.text_start..]
                ),
            },
            None => format!("- [ ] {line}"),
        },
        LineFormat::Indent => format!("\t{line}"),
        LineFormat::Outdent => match line.strip_prefix('\t') {
            Some(rest) => rest.to_string(),
            None => line.strip_prefix("  ").unwrap_or(line).to_string(),
        },
    }
}

/// ⌘1–⌘6, ⌘↩, ⌘], ⌘[, Quote and the lists: every line the selection touches.
pub fn format_lines(text: &str, selection: Range<usize>, format: LineFormat) -> Change {
    let start = line_start(text, selection.start);
    let end = line_end(text, selection.end);
    let old = &text[start..end];
    let new: Vec<String> = old
        .split('\n')
        .enumerate()
        .map(|(n, line)| format_line(line, format, n))
        .collect();
    let new = new.join("\n");
    // A cursor stays where it was in its line's text; a selection takes
    // in the lines changed.
    let selection = if selection.is_empty() {
        let first_old = old.split('\n').next().unwrap_or("");
        let first_new = new.split('\n').next().unwrap_or("");
        let from_end = (end.min(start + first_old.len())).saturating_sub(selection.start);
        let c = (start + first_new.len())
            .saturating_sub(from_end)
            .max(start);
        c..c
    } else {
        start..start + new.len()
    };
    Change {
        range: start..end,
        text: new,
        selection,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn apply(text: &str, c: &Change) -> String {
        let mut t = text.to_string();
        t.replace_range(c.range.clone(), &c.text);
        t
    }

    #[test]
    fn bold_wraps_the_selection_or_the_word_and_toggles_back() {
        let t = "uno due tre";
        let c = toggle_wrap(t, 4..7, Wrap::Bold);
        let t2 = apply(t, &c);
        assert_eq!(t2, "uno **due** tre");
        assert_eq!(c.selection, 6..9);
        let back = toggle_wrap(&t2, c.selection.clone(), Wrap::Bold);
        assert_eq!(apply(&t2, &back), t);
        assert_eq!(
            apply(t, &toggle_wrap(t, 5..5, Wrap::Bold)),
            "uno **due** tre",
            "the word at the cursor"
        );
        let marks_selected = toggle_wrap(&t2, 4..11, Wrap::Bold);
        assert_eq!(apply(&t2, &marks_selected), t, "the marks selected with it");
    }

    #[test]
    fn italic_and_bold_tell_their_stars_apart() {
        let t = "**due**";
        // Italic around bold, then away again.
        let c = toggle_wrap(t, 2..5, Wrap::Italic);
        let t2 = apply(t, &c);
        assert_eq!(t2, "***due***");
        assert_eq!(
            apply(&t2, &toggle_wrap(&t2, c.selection.clone(), Wrap::Italic)),
            t
        );
        // Bold inside italic is no bold to take away.
        let it = "*due*";
        assert_eq!(apply(it, &toggle_wrap(it, 1..4, Wrap::Bold)), "***due***");
    }

    #[test]
    fn an_empty_place_gets_both_marks_with_the_cursor_between() {
        let c = toggle_wrap("a  b", 2..2, Wrap::Code);
        assert_eq!(apply("a  b", &c), "a `` b");
        assert_eq!(c.selection, 3..3);
    }

    #[test]
    fn headings_toggle_and_change_level() {
        let t = "Titolo";
        let h2 = apply(t, &format_lines(t, 0..0, LineFormat::Heading(2)));
        assert_eq!(h2, "## Titolo");
        assert_eq!(
            apply(&h2, &format_lines(&h2, 4..4, LineFormat::Heading(1))),
            "# Titolo"
        );
        assert_eq!(
            apply(&h2, &format_lines(&h2, 4..4, LineFormat::Heading(2))),
            "Titolo",
            "again, it goes"
        );
        let c = format_lines(t, 3..3, LineFormat::Heading(2));
        assert_eq!(c.selection, 6..6, "the cursor keeps its place in the text");
    }

    #[test]
    fn task_turns_a_line_into_a_task_and_ticks_it() {
        let t = "comprare pane";
        let t1 = apply(t, &format_lines(t, 0..0, LineFormat::Task));
        assert_eq!(t1, "- [ ] comprare pane");
        let t2 = apply(&t1, &format_lines(&t1, 8..8, LineFormat::Task));
        assert_eq!(t2, "- [x] comprare pane");
        let item = "  - voce";
        assert_eq!(
            apply(item, &format_lines(item, 0..0, LineFormat::Task)),
            "  - [ ] voce"
        );
    }

    #[test]
    fn lists_quotes_and_indents_on_every_line_selected() {
        let t = "uno\ndue\n- tre";
        assert_eq!(
            apply(t, &format_lines(t, 0..t.len(), LineFormat::Numbered)),
            "1. uno\n2. due\n3. tre"
        );
        assert_eq!(
            apply(t, &format_lines(t, 8..9, LineFormat::Bullet)),
            "uno\ndue\ntre"
        );
        assert_eq!(
            apply(t, &format_lines(t, 0..5, LineFormat::Quote)),
            "> uno\n> due\n- tre"
        );
        let indented = apply(t, &format_lines(t, 8..8, LineFormat::Indent));
        assert_eq!(indented, "uno\ndue\n\t- tre");
        assert_eq!(
            apply(
                &indented,
                &format_lines(&indented, 9..9, LineFormat::Outdent)
            ),
            t
        );
    }

    #[test]
    fn cmd_k_makes_a_link_with_the_url_on_the_clipboard() {
        let t = "vedi questo sito";
        let c = make_link(t, 5..11, Some("https://example.com"));
        assert_eq!(apply(t, &c), "vedi [questo](https://example.com) sito");
        let c = make_link(t, 5..11, Some("non un url"));
        assert_eq!(apply(t, &c), "vedi [questo]() sito");
        assert_eq!(c.selection, 14..14, "the cursor between the parentheses");
        let c = make_link("a  b", 2..2, None);
        assert_eq!(
            (apply("a  b", &c).as_str(), c.selection),
            ("a []() b", 3..3)
        );
    }

    #[test]
    fn cmd_slash_comments_the_line_or_the_selection_and_back() {
        let t = "uno\ndue";
        let c = toggle_comment(t, 5..5);
        let t2 = apply(t, &c);
        assert_eq!(t2, "uno\n%%due%%");
        assert_eq!(apply(&t2, &toggle_comment(&t2, 7..7)), t);
        assert_eq!(apply(t, &toggle_comment(t, 0..2)), "%%un%%o\ndue");
    }
}
