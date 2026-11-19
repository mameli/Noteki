//! Block level: each line's kind (text, heading, list, quote, code, table,
//! Frontmatter, comment), list items and tasks.

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockKind {
    /// A `---` line that opens or closes the frontmatter.
    FrontmatterDelimiter,
    /// A YAML line inside the frontmatter.
    Frontmatter,
    /// A ```` ``` ```` / `~~~` line that opens or closes a fenced code block.
    FenceDelimiter,
    /// A line inside a fenced code block.
    Code,
    /// A thematic break (`---`, `***`, `___`), drawn as a rule. Not a setext
    /// Heading's underline, which stays `Text`.
    Rule,
    /// A line of a `$$` math block across lines, its `$$` lines included:
    /// left as written.
    Math,
    /// Ordinary Markdown.
    Text,
}

/// One source line. `start` is the byte offset of the line in the document;
/// `text` excludes the `\n` (and a trailing `\r`, so CRLF files behave the same).
#[derive(Debug, Clone, Copy)]
pub struct Line<'a> {
    pub index: usize,
    pub start: usize,
    pub text: &'a str,
    pub kind: BlockKind,
}

impl Line<'_> {
    pub fn range(&self) -> Range<usize> {
        self.start..self.start + self.text.len()
    }
}

/// Splits `text` into lines and classifies each one (frontmatter, code, text).
pub fn lines(text: &str) -> Vec<Line<'_>> {
    let fm_end = frontmatter_end_line(text).unwrap_or(0); // 1-based closing line
    let mut out = Vec::new();
    let mut fence: Option<(u8, usize)> = None;
    let mut math = false;
    let mut start = 0usize;
    for (index, raw) in text.split('\n').enumerate() {
        let line_text = raw.strip_suffix('\r').unwrap_or(raw);
        let line_no = index + 1;
        let kind = if line_no <= fm_end {
            if line_no == 1 || line_no == fm_end {
                BlockKind::FrontmatterDelimiter
            } else {
                BlockKind::Frontmatter
            }
        } else if let Some((ch, len)) = fence {
            if is_closing_fence(line_text.trim_start(), ch, len) {
                fence = None;
                BlockKind::FenceDelimiter
            } else {
                BlockKind::Code
            }
        } else if math {
            math = !line_text.trim_end().ends_with("$$");
            BlockKind::Math
        } else if line_text.trim_start().starts_with("$$") && !line_text.trim()[2..].contains("$$")
        {
            math = true;
            BlockKind::Math
        } else if let Some(open) = opening_fence(line_text.trim_start()) {
            fence = Some(open);
            BlockKind::FenceDelimiter
        } else if is_thematic_break(line_text)
            && !(line_text.trim().starts_with('-')
                && out.last().is_some_and(|prev: &Line| {
                    prev.kind == BlockKind::Text && is_setext_candidate(prev.text.trim_start())
                }))
        {
            BlockKind::Rule
        } else {
            BlockKind::Text
        };
        out.push(Line {
            index,
            start,
            text: line_text,
            kind,
        });
        start += raw.len() + 1;
    }
    out
}

/// Three or more `-`, `*` or `_` (the same), spaces allowed between, indented
/// at most three spaces.
pub(super) fn is_thematic_break(line: &str) -> bool {
    let trimmed = line.trim_start_matches(' ');
    if line.len() - trimmed.len() > 3 {
        return false;
    }
    let mut marks = trimmed.chars().filter(|c| *c != ' ' && *c != '\t');
    let Some(first) = marks.next().filter(|c| matches!(c, '-' | '*' | '_')) else {
        return false;
    };
    let mut count = 1;
    for c in marks {
        if c != first {
            return false;
        }
        count += 1;
    }
    count >= 3
}

/// A paragraph line that a `---` or `===` under it makes a setext Heading:
/// not a quote, a list item, an ATX Heading or a rule.
pub fn is_setext_candidate(trimmed: &str) -> bool {
    !trimmed.is_empty()
        && !trimmed.starts_with('>')
        && !trimmed.starts_with("- ")
        && !trimmed.starts_with("* ")
        && !trimmed.starts_with("+ ")
        && !trimmed.starts_with('#')
        && !trimmed
            .chars()
            .all(|c| matches!(c, '-' | '*' | '_' | '=' | ' '))
}

/// A quote's marks: its depth (the number of `>`) and the length of its prefix
/// (indent, `>`s and the spaces after each), in a line.
pub fn quote_prefix(line: &str) -> Option<(u8, usize)> {
    let b = line.as_bytes();
    let mut i = 0;
    while i < b.len() && i < 3 && b[i] == b' ' {
        i += 1;
    }
    let mut depth = 0u8;
    while i < b.len() && b[i] == b'>' {
        depth = depth.saturating_add(1);
        i += 1;
        if i < b.len() && b[i] == b' ' {
            i += 1;
        }
    }
    (depth > 0).then_some((depth, i))
}

pub(super) fn opening_fence(trimmed: &str) -> Option<(u8, usize)> {
    let first = *trimmed.as_bytes().first()?;
    if first != b'`' && first != b'~' {
        return None;
    }
    let count = trimmed.bytes().take_while(|&b| b == first).count();
    // A backtick fence's info string cannot contain backticks.
    if count >= 3 && !(first == b'`' && trimmed[count..].contains('`')) {
        Some((first, count))
    } else {
        None
    }
}

pub(super) fn is_closing_fence(trimmed: &str, ch: u8, min_len: usize) -> bool {
    let count = trimmed.bytes().take_while(|&b| b == ch).count();
    count >= min_len && trimmed[count..].trim().is_empty()
}

/// `# Title` → `(1, "Title")`. Accepts up to 6 hashes followed by a space or end of line.
pub fn parse_atx_heading(trimmed: &str) -> Option<(u8, &str)> {
    let hashes = trimmed.bytes().take_while(|&b| b == b'#').count();
    if hashes == 0 || hashes > 6 {
        return None;
    }
    let rest = &trimmed[hashes..];
    if !rest.is_empty() && !rest.starts_with(' ') && !rest.starts_with('\t') {
        return None;
    }
    let title = rest.trim();
    // Optional closing sequence: `## Title ##`
    let title = match title.trim_end_matches('#') {
        t if t.len() < title.len() && (t.is_empty() || t.ends_with(' ')) => t.trim_end(),
        _ => title,
    };
    Some((hashes as u8, title))
}

/// Byte length of the list/quote prefix of a task item (`- [ ] `), plus the
/// offset of its `[` marker, if the line is a task.
pub(super) fn task_marker(line: &str) -> Option<(usize, bool)> {
    let mut i = 0;
    let bytes = line.as_bytes();
    // Quote markers and indentation.
    loop {
        while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\t') {
            i += 1;
        }
        if i < bytes.len() && bytes[i] == b'>' {
            i += 1;
            continue;
        }
        break;
    }
    // Bullet or ordered marker.
    if i < bytes.len() && matches!(bytes[i], b'-' | b'*' | b'+') {
        i += 1;
    } else {
        let digits = bytes[i..].iter().take_while(|b| b.is_ascii_digit()).count();
        if digits == 0 || digits > 9 {
            return None;
        }
        i += digits;
        if i < bytes.len() && matches!(bytes[i], b'.' | b')') {
            i += 1;
        } else {
            return None;
        }
    }
    if i >= bytes.len() || bytes[i] != b' ' {
        return None;
    }
    while i < bytes.len() && bytes[i] == b' ' {
        i += 1;
    }
    if i + 3 > bytes.len() || bytes[i] != b'[' || bytes[i + 2] != b']' {
        return None;
    }
    let checked = match bytes[i + 1] {
        b' ' => false,
        b'x' | b'X' => true,
        _ => return None,
    };
    // `[ ]` must be followed by whitespace or end of line.
    if i + 3 < bytes.len() && !bytes[i + 3].is_ascii_whitespace() {
        return None;
    }
    Some((i, checked))
}

/// Whether a line is a task item, and whether it is done.
/// A list item's marks, in a line (not in a quote).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ListItem {
    /// How deep it is nested: a tab or two spaces of indent per level.
    pub level: u8,
    pub kind: ListKind,
    /// Where its text starts: after the indent, the marker, its spaces and,
    /// for a task, its `[ ]` and the space after.
    pub text_start: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListKind {
    Bullet,
    /// `12.` or `12)`.
    Number {
        n: u32,
        delim: u8,
    },
    /// `- [ ]` or `- [x]`; `bracket` is the offset of its `[`. A numbered item
    /// keeps its number and its brackets as written.
    Task {
        checked: bool,
        bracket: usize,
    },
}

pub fn list_item(line: &str) -> Option<ListItem> {
    let b = line.as_bytes();
    let mut i = 0;
    let mut cols = 0usize;
    while i < b.len() && matches!(b[i], b' ' | b'\t') {
        cols += if b[i] == b'\t' { 2 } else { 1 };
        i += 1;
    }
    let level = u8::try_from(cols / 2).unwrap_or(u8::MAX);
    let mut kind = if i < b.len() && matches!(b[i], b'-' | b'*' | b'+') {
        i += 1;
        ListKind::Bullet
    } else {
        let digits = b[i..].iter().take_while(|c| c.is_ascii_digit()).count();
        if digits == 0
            || digits > 9
            || i + digits >= b.len()
            || !matches!(b[i + digits], b'.' | b')')
        {
            return None;
        }
        let n = line[i..i + digits].parse().ok()?;
        let delim = b[i + digits];
        i += digits + 1;
        ListKind::Number { n, delim }
    };
    // The marker is followed by a space, or ends the line (an empty item).
    if i < b.len() && b[i] != b' ' {
        return None;
    }
    while i < b.len() && b[i] == b' ' {
        i += 1;
    }
    if kind == ListKind::Bullet && i + 3 <= b.len() && b[i] == b'[' && b[i + 2] == b']' {
        let checked = match b[i + 1] {
            b' ' => Some(false),
            b'x' | b'X' => Some(true),
            _ => None,
        };
        let after = b.get(i + 3);
        if let (Some(checked), None | Some(b' ')) = (checked, after) {
            kind = ListKind::Task {
                checked,
                bracket: i,
            };
            i = (i + 4).min(b.len());
        }
    }
    Some(ListItem {
        level,
        kind,
        text_start: i,
    })
}

pub fn task_state(line: &str) -> Option<bool> {
    task_marker(line).map(|(_, checked)| checked)
}

/// 0-based line containing byte/// 0-based line containing byte `offset` (clamped to the text).
pub fn line_of_offset(text: &str, offset: usize) -> usize {
    let mut end = offset.min(text.len());
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].bytes().filter(|&b| b == b'\n').count()
}
