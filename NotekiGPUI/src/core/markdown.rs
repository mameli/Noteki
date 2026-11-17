//! Markdown scanning shared by the live editor, Export, and the vault indexes.
//!
//! Everything here works on UTF-8 byte offsets and only ever advances by whole
//! characters, so ranges are always valid `str` slice boundaries (the editor's
//! decorations rely on that).

use std::ops::Range;

use crate::core::frontmatter::{frontmatter_end_line, is_valid_tag};
use crate::core::vault::AttachmentKind;
use crate::core::wikilink::{ResolvedItem, VaultIndex, Wikilink};

// ---------------------------------------------------------------------------
// Block level
// ---------------------------------------------------------------------------

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
fn is_thematic_break(line: &str) -> bool {
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

fn opening_fence(trimmed: &str) -> Option<(u8, usize)> {
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

fn is_closing_fence(trimmed: &str, ch: u8, min_len: usize) -> bool {
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
fn task_marker(line: &str) -> Option<(usize, bool)> {
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

// ---------------------------------------------------------------------------
// Inline level
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InlineKind {
    /// `\x`: `range` covers the backslash and the escaped character.
    Escape,
    /// `` `code` ``
    Code,
    /// `%%comment%%`
    Comment,
    /// `[[target#heading|alias]]` or, with `embed`, `![[...]]`.
    Wikilink {
        embed: bool,
    },
    /// `[label](destination)`; `inner` is the label.
    MarkdownLink,
    Bold,
    Italic,
    Strikethrough,
    Highlight,
    /// `#tag`; `inner` excludes the `#`.
    Tag,
    /// `$x$` or `$$x$$`: math, left as written (no Markdown inside).
    Math,
}

/// An inline construct. `range` is the whole construct, `inner` its content;
/// both are byte ranges relative to the scanned line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inline {
    pub kind: InlineKind,
    pub range: Range<usize>,
    pub inner: Range<usize>,
}

/// Scans one line of Markdown into inline constructs, outermost first.
/// Emphasis is scanned recursively, so `**[[link]]**` yields both.
pub fn inline_tokens(line: &str) -> Vec<Inline> {
    let mut out = Vec::new();
    scan_inline(line, 0, line.len(), &mut out);
    out
}

fn char_len_at(s: &str, i: usize) -> usize {
    s[i..].chars().next().map_or(0, char::len_utf8)
}

/// Finds `pat` in `s[from..to]`, returning an absolute offset.
fn find_in(s: &str, from: usize, to: usize, pat: &str) -> Option<usize> {
    if from > to {
        return None;
    }
    s[from..to].find(pat).map(|p| from + p)
}

fn scan_inline(s: &str, from: usize, to: usize, out: &mut Vec<Inline>) {
    let b = s.as_bytes();
    let starts = |i: usize, pat: &str| s[i..to].starts_with(pat);
    let mut i = from;
    while i < to {
        match b[i] {
            // CommonMark: a backslash escapes ASCII punctuation only; before
            // anything else (`\pi` in a formula) it is a backslash.
            b'\\' if i + 1 < to && b[i + 1].is_ascii_punctuation() => {
                let next = char_len_at(s, i + 1);
                if next > 0 {
                    out.push(Inline {
                        kind: InlineKind::Escape,
                        range: i..i + 1 + next,
                        inner: i + 1..i + 1 + next,
                    });
                }
                i += 1 + next;
                continue;
            }
            // SPEC: `$x$` when the first `$` is followed by no space and the
            // last preceded by none and followed by no digit (`$5 and $10` is
            // text); `$$x$$` too. Nothing inside is Markdown.
            b'$' => {
                let n = if starts(i, "$$") { 2 } else { 1 };
                let close = if n == 2 {
                    find_in(s, i + 2, to, "$$")
                } else if i + 1 < to && !b[i + 1].is_ascii_whitespace() {
                    (i + 2..to).find(|&p| {
                        b[p] == b'$'
                            && !b[p - 1].is_ascii_whitespace()
                            && b[p - 1] != b'\\'
                            && !(p + 1 < to && b[p + 1].is_ascii_digit())
                    })
                } else {
                    None
                };
                if let Some(close) = close {
                    out.push(Inline {
                        kind: InlineKind::Math,
                        range: i..close + n,
                        inner: i + n..close,
                    });
                    i = close + n;
                } else {
                    i += n;
                }
                continue;
            }
            b'`' => {
                let n = s[i..to].bytes().take_while(|&c| c == b'`').count();
                let ticks = &s[i..i + n];
                let mut search = i + n;
                let mut closed = None;
                while let Some(p) = find_in(s, search, to, ticks) {
                    let run = s[p..to].bytes().take_while(|&c| c == b'`').count();
                    if run == n {
                        closed = Some(p);
                        break;
                    }
                    search = p + run;
                }
                if let Some(close) = closed {
                    out.push(Inline {
                        kind: InlineKind::Code,
                        range: i..close + n,
                        inner: i + n..close,
                    });
                    i = close + n;
                } else {
                    i += n;
                }
                continue;
            }
            b'%' if starts(i, "%%") => {
                if let Some(close) = find_in(s, i + 2, to, "%%") {
                    out.push(Inline {
                        kind: InlineKind::Comment,
                        range: i..close + 2,
                        inner: i + 2..close,
                    });
                    i = close + 2;
                    continue;
                }
            }
            b'!' if starts(i, "![[") => {
                if let Some(close) = find_in(s, i + 3, to, "]]") {
                    out.push(Inline {
                        kind: InlineKind::Wikilink { embed: true },
                        range: i..close + 2,
                        inner: i + 3..close,
                    });
                    i = close + 2;
                    continue;
                }
            }
            b'[' if starts(i, "[[") => {
                if let Some(close) = find_in(s, i + 2, to, "]]") {
                    out.push(Inline {
                        kind: InlineKind::Wikilink { embed: false },
                        range: i..close + 2,
                        inner: i + 2..close,
                    });
                    i = close + 2;
                    continue;
                }
            }
            b'[' => {
                if let Some(close_br) = find_in(s, i + 1, to, "]") {
                    if close_br + 1 < to && b[close_br + 1] == b'(' {
                        if let Some(close_paren) = find_in(s, close_br + 2, to, ")") {
                            out.push(Inline {
                                kind: InlineKind::MarkdownLink,
                                range: i..close_paren + 1,
                                inner: i + 1..close_br,
                            });
                            scan_inline(s, i + 1, close_br, out);
                            i = close_paren + 1;
                            continue;
                        }
                    }
                }
            }
            // `***x***`: italic around bold, as CommonMark reads it.
            // Unclosed, the stars are read as `**` then `*`, below.
            b'*' if starts(i, "***")
                && i + 3 < to
                && !b[i + 3].is_ascii_whitespace()
                && find_closing(s, i + 3, to, "***").is_some() =>
            {
                if let Some(close) = find_closing(s, i + 3, to, "***") {
                    out.push(Inline {
                        kind: InlineKind::Italic,
                        range: i..close + 3,
                        inner: i + 1..close + 2,
                    });
                    out.push(Inline {
                        kind: InlineKind::Bold,
                        range: i + 1..close + 2,
                        inner: i + 3..close,
                    });
                    scan_inline(s, i + 3, close, out);
                    i = close + 3;
                    continue;
                }
            }
            b'*' | b'~' | b'=' => {
                let c = b[i];
                let doubled = i + 1 < to && b[i + 1] == c;
                let (kind, width) = match (c, doubled) {
                    (b'*', true) => (Some(InlineKind::Bold), 2),
                    (b'*', false) => (Some(InlineKind::Italic), 1),
                    (b'~', true) => (Some(InlineKind::Strikethrough), 2),
                    (b'=', true) => (Some(InlineKind::Highlight), 2),
                    _ => (None, 1),
                };
                if let Some(kind) = kind {
                    let marker = &s[i..i + width];
                    let open_end = i + width;
                    // Emphasis cannot start before whitespace (`* item`, `a * b`).
                    let opens = open_end < to && !b[open_end].is_ascii_whitespace();
                    if opens {
                        if let Some(close) = find_closing(s, open_end, to, marker) {
                            out.push(Inline {
                                kind,
                                range: i..close + width,
                                inner: open_end..close,
                            });
                            scan_inline(s, open_end, close, out);
                            i = close + width;
                            continue;
                        }
                    }
                    i += width;
                    continue;
                }
            }
            b'#' if i == 0 || b[i - 1].is_ascii_whitespace() => {
                let tag_len = s[i + 1..to]
                    .char_indices()
                    .take_while(|(_, c)| c.is_alphanumeric() || matches!(c, '_' | '-' | '/'))
                    .last()
                    .map_or(0, |(idx, c)| idx + c.len_utf8());
                if tag_len > 0 {
                    let candidate = s[i + 1..i + 1 + tag_len].trim_end_matches('/');
                    if is_valid_tag(candidate) {
                        out.push(Inline {
                            kind: InlineKind::Tag,
                            range: i..i + 1 + candidate.len(),
                            inner: i + 1..i + 1 + candidate.len(),
                        });
                    }
                    i += 1 + tag_len;
                    continue;
                }
            }
            _ => {}
        }
        i += char_len_at(s, i).max(1);
    }
}

/// Closing emphasis marker: not preceded by whitespace, and for single `*`
/// not part of a `**`.
fn find_closing(s: &str, from: usize, to: usize, marker: &str) -> Option<usize> {
    let b = s.as_bytes();
    let mut search = from;
    while let Some(p) = find_in(s, search, to, marker) {
        let after = p + marker.len();
        let prev_ws = p == from || b[p - 1].is_ascii_whitespace();
        let single_star_in_double =
            marker == "*" && ((after < to && b[after] == b'*') || (p > from && b[p - 1] == b'*'));
        if p > from && !prev_ws && !single_star_in_double {
            return Some(p);
        }
        // Skip a following `*` of a `**` (ASCII, so `after + 1` stays a char boundary).
        search = if after < to && b[after] == b'*' && single_star_in_double {
            after + 1
        } else {
            after
        };
        if search > to {
            return None;
        }
    }
    None
}

/// All wikilinks (and embeds) of the text that are real Markdown: outside code,
/// comments and frontmatter. Ranges are absolute byte offsets.
pub fn wikilinks(text: &str) -> Vec<(usize, Inline)> {
    let mut out = Vec::new();
    for line in lines(text) {
        if line.kind != BlockKind::Text {
            continue;
        }
        for tok in inline_tokens(line.text) {
            if matches!(tok.kind, InlineKind::Wikilink { .. }) {
                out.push((
                    line.index,
                    Inline {
                        kind: tok.kind,
                        range: line.start + tok.range.start..line.start + tok.range.end,
                        inner: line.start + tok.inner.start..line.start + tok.inner.end,
                    },
                ));
            }
        }
    }
    out
}

/// The wikilink whose source contains the byte `offset`, if any.
pub fn wikilink_at(text: &str, offset: usize) -> Option<Wikilink> {
    wikilinks(text)
        .into_iter()
        .find(|(_, tok)| tok.range.start <= offset && offset <= tok.range.end)
        .map(|(_, tok)| Wikilink::parse(&text[tok.inner]))
}

/// What a line holding only an image Embed points at: `![[target]]` gives
/// `target`, `![alt](path)` gives `path`.
pub fn image_embed(line: &str) -> Option<&str> {
    let t = line.trim();
    if let Some(inner) = t.strip_prefix("![[").and_then(|r| r.strip_suffix("]]")) {
        return (!inner.contains("]]")).then_some(inner);
    }
    let rest = t.strip_prefix("![")?.strip_suffix(')')?;
    let (_, dest) = rest.split_once("](")?;
    let dest = dest.split_whitespace().next()?;
    Some(dest.trim_start_matches('<').trim_end_matches('>'))
}

/// A callout's `[!type]` (with a `-` or `+` and the space after) at the start
/// of a quote's text: the type's range and where the marks end.
pub fn callout_marks(after_quote: &str) -> Option<(Range<usize>, usize)> {
    let rest = after_quote.strip_prefix("[!")?;
    let close = rest.find(']')?;
    let ty = &rest[..close];
    if ty.is_empty() || ty.contains(char::is_whitespace) {
        return None;
    }
    let mut end = 2 + close + 1;
    if after_quote[end..].starts_with(['-', '+']) {
        end += 1;
    }
    if after_quote[end..].starts_with(' ') {
        end += 1;
    }
    Some((2..2 + close, end))
}

/// A footnote in a line: `[^label]` (a reference) or, at the line's start,
/// `[^label]:` (a definition).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Footnote {
    pub range: Range<usize>,
    pub label: Range<usize>,
    pub definition: bool,
}

pub fn footnotes(line: &str) -> Vec<Footnote> {
    let code: Vec<Range<usize>> = inline_tokens(line)
        .into_iter()
        .filter(|t| t.kind == InlineKind::Code)
        .map(|t| t.range)
        .collect();
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(i) = line[from..].find("[^").map(|i| from + i) {
        from = i + 2;
        let Some(len) = line[i + 2..].find(']') else {
            break;
        };
        let label = i + 2..i + 2 + len;
        if label.is_empty() || line[label.clone()].contains(char::is_whitespace) {
            continue;
        }
        if code.iter().any(|c| c.start <= i && i < c.end) {
            continue;
        }
        let definition = i == 0 && line[label.end + 1..].starts_with(':');
        let end = label.end + 1 + usize::from(definition);
        out.push(Footnote {
            range: i..end,
            label,
            definition,
        });
        from = end;
    }
    out
}

/// A click on a footnote goes to the other end: from a reference to its
/// definition, from a definition to its first reference. The offset to go to.
pub fn footnote_target(text: &str, offset: usize) -> Option<usize> {
    let all: Vec<(usize, Footnote)> = lines(text)
        .into_iter()
        .filter(|l| l.kind == BlockKind::Text)
        .flat_map(|l| footnotes(l.text).into_iter().map(move |f| (l.start, f)))
        .collect();
    let (start, here) = all
        .iter()
        .find(|(s, f)| s + f.range.start <= offset && offset <= s + f.range.end)?;
    let label = &text[start + here.label.start..start + here.label.end];
    all.iter()
        .find(|(s, f)| {
            f.definition != here.definition && &text[s + f.label.start..s + f.label.end] == label
        })
        .map(|(s, f)| s + f.range.end)
}

/// A link of the text, as a click opens it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Link {
    /// `[label](url)`: its destination, as written.
    Url(String),
    /// `[[target#heading|alias]]`.
    Note(Wikilink),
}

/// The link whose shown text (a Markdown link's label, a Wikilink's name or
/// Alias) holds `offset`, edges included, with the range of the whole link.
/// Not in code, nor the Frontmatter; an Embed is no link.
pub fn link_at(text: &str, offset: usize) -> Option<(Range<usize>, Link)> {
    let line = lines(text)
        .into_iter()
        .find(|l| l.start <= offset && offset <= l.start + l.text.len())?;
    if line.kind != BlockKind::Text {
        return None;
    }
    let at = offset - line.start;
    inline_tokens(line.text).into_iter().find_map(|tok| {
        let range = line.start + tok.range.start..line.start + tok.range.end;
        if !(tok.inner.start <= at && at <= tok.inner.end) {
            return None;
        }
        match tok.kind {
            InlineKind::Wikilink { embed: false } => Some((
                range,
                Link::Note(Wikilink::parse(&line.text[tok.inner.clone()])),
            )),
            InlineKind::MarkdownLink => {
                // `[label](destination "title")`: the destination, without a title.
                let dest = &line.text[tok.inner.end + 2..tok.range.end - 1];
                let url = dest.split_whitespace().next().unwrap_or("");
                let url = url.trim_start_matches('<').trim_end_matches('>');
                (!url.is_empty()).then(|| (range, Link::Url(url.to_string())))
            }
            _ => None,
        }
    })
}

/// Inline `#tags` (outside code and frontmatter), in order.
pub fn inline_tags(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in lines(text) {
        if line.kind != BlockKind::Text {
            continue;
        }
        for tok in inline_tokens(line.text) {
            if tok.kind == InlineKind::Tag {
                out.push(line.text[tok.inner].to_string());
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Live preview spans
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MdSpanKind {
    SyntaxMarker,
    FrontmatterDelimiter,
    FrontmatterKey,
    Heading(u8),
    Bold,
    Italic,
    Strikethrough,
    Highlight,
    InlineCode,
    CodeBlock,
    WikilinkResolved,
    WikilinkUnresolved,
    MarkdownLink,
    Tag,
    Comment,
    Quote,
    CalloutHeader,
    TaskDone,
    /// A footnote's label, in a reference `[^1]` or a definition `[^1]:`.
    Footnote,
    /// A Tag of the Properties table's `tags` row, drawn as a chip.
    TagChip,
    /// The parts of a code block's code, coloured by its language.
    CodeComment,
    CodeString,
    CodeNumber,
    CodeKeyword,
    CodeType,
    CodeFunction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MdSpan {
    pub range: Range<usize>,
    pub kind: MdSpanKind,
}

/// Live preview spans of the whole text. Syntax marks (`**`, `[[`, `# `…) are
/// `SyntaxMarker` spans, which the editor hides, except the marks of the element
/// the `cursor` (a byte offset) is in or at the edge of, and a Heading's `#`s
/// with the cursor anywhere on its line (SPEC › Live preview).
pub fn compute_markdown_spans(
    text: &str,
    cursor: Option<usize>,
    source_path: Option<&str>,
    index: &VaultIndex,
) -> Vec<MdSpan> {
    let mut spans = Vec::new();
    for line in lines(text) {
        let in_line = cursor
            .filter(|&c| line.start <= c && c <= line.start + line.text.len())
            .map(|c| c - line.start);
        line_spans(&line, in_line, source_path, index, &mut spans);
    }
    spans
}

/// The spans of one line, pushed to `spans` with offsets in the whole text.
/// `cursor` is the cursor's offset in the line when it is on it.
pub fn line_spans(
    line: &Line,
    cursor: Option<usize>,
    source_path: Option<&str>,
    index: &VaultIndex,
    spans: &mut Vec<MdSpan>,
) {
    let mut push = |range: Range<usize>, kind: MdSpanKind| {
        if range.start < range.end {
            spans.push(MdSpan { range, kind });
        }
    };
    {
        let s = line.start;
        let on_cursor = cursor.is_some();
        match line.kind {
            BlockKind::FrontmatterDelimiter => push(line.range(), MdSpanKind::FrontmatterDelimiter),
            BlockKind::Frontmatter => {
                if let Some(colon) = line.text.find(':') {
                    if !line.text.starts_with(char::is_whitespace) && !line.text.starts_with('-') {
                        push(s..s + colon + 1, MdSpanKind::FrontmatterKey);
                    }
                }
            }
            BlockKind::FenceDelimiter | BlockKind::Code => {
                // A fence is hidden off its block (the editor shows the block's
                // fences while the cursor is in it).
                if line.kind == BlockKind::FenceDelimiter && !on_cursor {
                    push(line.range(), MdSpanKind::SyntaxMarker);
                }
                push(line.range(), MdSpanKind::CodeBlock)
            }
            BlockKind::Math => push(line.range(), MdSpanKind::CodeBlock),
            BlockKind::Rule => {
                if !on_cursor {
                    push(line.range(), MdSpanKind::SyntaxMarker);
                }
            }
            BlockKind::Text => {
                let trimmed = line.text.trim_start();
                let leading = line.text.len() - trimmed.len();
                if let Some((level, _)) = parse_atx_heading(trimmed) {
                    let marker_end = (leading + level as usize + 1).min(line.text.len());
                    if !on_cursor {
                        push(s + leading..s + marker_end, MdSpanKind::SyntaxMarker);
                    }
                    push(
                        s + marker_end..s + line.text.len(),
                        MdSpanKind::Heading(level),
                    );
                } else if let Some((_, prefix)) = quote_prefix(line.text) {
                    let kind = if line.text[prefix..].starts_with("[!") {
                        MdSpanKind::CalloutHeader
                    } else {
                        MdSpanKind::Quote
                    };
                    // SPEC: a quote's `>`s show only on the cursor's line, and a
                    // callout's `[!type]` with them: its title shows, or its type.
                    if !on_cursor {
                        push(s..s + prefix, MdSpanKind::SyntaxMarker);
                        if let Some((ty, end)) = callout_marks(&line.text[prefix..]) {
                            let titled = !line.text[prefix + end..].trim().is_empty();
                            if titled {
                                push(s + prefix..s + prefix + end, MdSpanKind::SyntaxMarker);
                            } else {
                                push(s + prefix..s + prefix + ty.start, MdSpanKind::SyntaxMarker);
                                push(
                                    s + prefix + ty.end..s + prefix + end,
                                    MdSpanKind::SyntaxMarker,
                                );
                            }
                        }
                    }
                    push(s + prefix..s + line.text.len(), kind);
                } else if let Some(item) = list_item(line.text) {
                    // SPEC: the bullet and brackets show while the cursor touches them.
                    if !cursor.is_some_and(|c| c <= item.text_start) {
                        push(s..s + item.text_start, MdSpanKind::SyntaxMarker);
                    }
                    if task_state(line.text) == Some(true) {
                        push(
                            s + item.text_start..s + line.text.len(),
                            MdSpanKind::TaskDone,
                        );
                    }
                } else if task_state(line.text) == Some(true) {
                    push(s + leading..s + line.text.len(), MdSpanKind::TaskDone);
                }

                for tok in inline_tokens(line.text) {
                    let r = |r: Range<usize>| s + r.start..s + r.end;
                    // The marks of the element the cursor is in, or at the edge of, show.
                    let on_cursor =
                        cursor.is_some_and(|c| tok.range.start <= c && c <= tok.range.end);
                    let (open, close) = (
                        tok.range.start..tok.inner.start,
                        tok.inner.end..tok.range.end,
                    );
                    let markers = |push: &mut dyn FnMut(Range<usize>, MdSpanKind)| {
                        if !on_cursor {
                            push(r(open.clone()), MdSpanKind::SyntaxMarker);
                            push(r(close.clone()), MdSpanKind::SyntaxMarker);
                        }
                    };
                    match tok.kind {
                        InlineKind::Escape => {
                            if !on_cursor {
                                push(
                                    r(tok.range.start..tok.inner.start),
                                    MdSpanKind::SyntaxMarker,
                                );
                            }
                        }
                        InlineKind::Code => {
                            markers(&mut push);
                            push(r(tok.inner.clone()), MdSpanKind::InlineCode);
                        }
                        InlineKind::Comment => {
                            markers(&mut push);
                            push(r(tok.inner.clone()), MdSpanKind::Comment);
                        }
                        InlineKind::Wikilink { .. } => {
                            markers(&mut push);
                            let inner = &line.text[tok.inner.clone()];
                            let link = Wikilink::parse(inner);
                            let resolved = link.target.is_empty()
                                || index.resolve(&link.target, source_path).is_some();
                            // With an alias, only the alias is read: dim `target|`.
                            let mut shown = tok.inner.clone();
                            if let Some(bar) = inner.find('|') {
                                if !on_cursor {
                                    push(
                                        r(tok.inner.start..tok.inner.start + bar + 1),
                                        MdSpanKind::SyntaxMarker,
                                    );
                                }
                                shown.start += bar + 1;
                            }
                            push(
                                r(shown),
                                if resolved {
                                    MdSpanKind::WikilinkResolved
                                } else {
                                    MdSpanKind::WikilinkUnresolved
                                },
                            );
                        }
                        InlineKind::MarkdownLink => {
                            markers(&mut push);
                            push(r(tok.inner.clone()), MdSpanKind::MarkdownLink);
                        }
                        InlineKind::Bold => {
                            markers(&mut push);
                            push(r(tok.inner.clone()), MdSpanKind::Bold);
                        }
                        InlineKind::Italic => {
                            markers(&mut push);
                            push(r(tok.inner.clone()), MdSpanKind::Italic);
                        }
                        InlineKind::Strikethrough => {
                            markers(&mut push);
                            push(r(tok.inner.clone()), MdSpanKind::Strikethrough);
                        }
                        InlineKind::Highlight => {
                            markers(&mut push);
                            push(r(tok.inner.clone()), MdSpanKind::Highlight);
                        }
                        InlineKind::Tag => push(r(tok.range.clone()), MdSpanKind::Tag),
                        // Math stays visible as code (SPEC), marks and all.
                        InlineKind::Math => push(r(tok.range.clone()), MdSpanKind::InlineCode),
                    }
                }
                for f in footnotes(line.text) {
                    let shown = cursor.is_some_and(|c| f.range.start <= c && c <= f.range.end);
                    if !shown {
                        push(
                            s + f.range.start..s + f.label.start,
                            MdSpanKind::SyntaxMarker,
                        );
                        push(s + f.label.end..s + f.range.end, MdSpanKind::SyntaxMarker);
                    }
                    push(s + f.label.start..s + f.label.end, MdSpanKind::Footnote);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// HTML for Export
// ---------------------------------------------------------------------------

/// URL scheme for wikilinks in the HTML: `noteki-link:<target>#<heading>`.
pub const LINK_SCHEME: &str = "noteki-link:";
/// URL scheme for embedded vault images: `noteki-embed:<vault path>`.
pub const EMBED_SCHEME: &str = "noteki-embed:";

/// Percent-encodes everything but unreserved characters and `/`, so any note
/// name (spaces, parentheses, `#`, emoji) is a valid CommonMark destination.
pub fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~' | b'/') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

pub fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            let hex = |c: u8| (c as char).to_digit(16);
            if let (Some(h), Some(l)) = (hex(b[i + 1]), hex(b[i + 2])) {
                out.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn escape_label(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if matches!(
            c,
            '[' | ']' | '*' | '_' | '`' | '\\' | '<' | '>' | '!' | '#' | '|'
        ) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// Converts Obsidian-flavoured Markdown (wikilinks, embeds, `==highlight==`,
/// `%%comments%%`, callouts) to the CommonMark/GFM that Export turns into HTML.
///
/// The output has exactly the same lines as the input.
pub fn preprocess_for_html(text: &str, source_path: Option<&str>, index: &VaultIndex) -> String {
    let mut out = String::with_capacity(text.len() + 64);
    for (n, line) in lines(text).into_iter().enumerate() {
        if n > 0 {
            out.push('\n');
        }
        if line.kind != BlockKind::Text {
            out.push_str(line.text);
            continue;
        }
        if let Some(callout) = rewrite_callout_line(line.text) {
            out.push_str(&callout);
            continue;
        }
        out.push_str(&rewrite_inline(line.text, source_path, index));
    }
    out
}

fn rewrite_callout_line(line: &str) -> Option<String> {
    let gt = line.find('>')?;
    if !line[..gt].trim().is_empty() {
        return None;
    }
    let after = line[gt + 1..].trim_start();
    let rest = after.strip_prefix("[!")?;
    let close = rest.find(']')?;
    let kind = rest[..close].trim().trim_end_matches(['-', '+']);
    if kind.is_empty() {
        return None;
    }
    let title = rest[close + 1..].trim_start_matches(['-', '+']).trim();
    let title = if title.is_empty() {
        let mut c = kind.chars();
        c.next().map_or(String::new(), |f| {
            f.to_uppercase()
                .chain(c.flat_map(|c| c.to_lowercase()))
                .collect()
        })
    } else {
        rewrite_inline_plain(title)
    };
    Some(format!("{}> **{}**", &line[..gt], title))
}

fn rewrite_inline_plain(s: &str) -> String {
    rewrite_inline(s, None, &VaultIndex::default())
}

fn rewrite_inline(line: &str, source_path: Option<&str>, index: &VaultIndex) -> String {
    // Replacements for the constructs CommonMark does not know; everything else
    // is copied verbatim. Tokens never overlap except emphasis nesting, and the
    // ones replaced here (comments, wikilinks, highlight marks) are leaves.
    let mut edits: Vec<(Range<usize>, String)> = Vec::new();
    for tok in inline_tokens(line) {
        match tok.kind {
            InlineKind::Comment => edits.push((tok.range, String::new())),
            InlineKind::Highlight => {
                edits.push((tok.range.start..tok.inner.start, "**".into()));
                edits.push((tok.inner.end..tok.range.end, "**".into()));
            }
            InlineKind::Wikilink { embed } => {
                let link = Wikilink::parse(&line[tok.inner.clone()]);
                edits.push((tok.range, render_wikilink(&link, embed, source_path, index)));
            }
            _ => {}
        }
    }
    edits.sort_by_key(|(r, _)| r.start);
    let mut out = String::with_capacity(line.len());
    let mut at = 0;
    for (range, replacement) in edits {
        if range.start < at {
            continue; // nested inside something already replaced
        }
        out.push_str(&line[at..range.start]);
        out.push_str(&replacement);
        at = range.end;
    }
    out.push_str(&line[at..]);
    out
}

fn render_wikilink(
    link: &Wikilink,
    embed: bool,
    source_path: Option<&str>,
    index: &VaultIndex,
) -> String {
    let resolved = if link.target.is_empty() {
        None
    } else {
        index.resolve(&link.target, source_path)
    };
    if embed {
        if let Some(ResolvedItem::Attachment {
            path,
            attachment_kind: AttachmentKind::Image,
            ..
        }) = &resolved
        {
            // `![[img.png|120]]`: a number is a width in Obsidian, not alt text.
            let alt = link
                .alias
                .as_deref()
                .filter(|a| !a.chars().all(|c| c.is_ascii_digit() || c == 'x'))
                .unwrap_or(&link.target);
            return format!(
                "![{}]({EMBED_SCHEME}{})",
                escape_label(alt),
                percent_encode(path)
            );
        }
    }
    let mut url = String::from(LINK_SCHEME);
    url.push_str(&percent_encode(&link.target));
    if let Some(h) = &link.heading {
        url.push('#');
        url.push_str(&percent_encode(h));
    }
    let label = link.display_text();
    let label = if embed && resolved.is_none() {
        format!("{label} (missing)")
    } else {
        label
    };
    format!("[{}]({url})", escape_label(&label))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(line: &str) -> Vec<(InlineKind, &str)> {
        inline_tokens(line)
            .into_iter()
            .map(|t| (t.kind, &line[t.range]))
            .collect()
    }

    #[test]
    fn an_image_embed_alone_on_its_line() {
        assert_eq!(image_embed("![[noteki.png]]"), Some("noteki.png"));
        assert_eq!(
            image_embed("  ![alt](Media/a b.png \"t\")"),
            Some("Media/a")
        );
        assert_eq!(image_embed("![](<Media/a.png>)"), Some("Media/a.png"));
        assert_eq!(image_embed("testo ![[x.png]]"), None);
        assert_eq!(image_embed("![[a]] e ![[b]]"), None);
    }

    #[test]
    fn callout_marks_and_footnotes() {
        assert_eq!(callout_marks("[!warning]- Attento"), Some((2..9, 12)));
        assert_eq!(callout_marks("[!note]"), Some((2..6, 7)));
        assert_eq!(callout_marks("[! note]"), None);
        let f = footnotes("Testo[^1] e `[^no]` e [^nota].");
        assert_eq!(f.len(), 2);
        assert_eq!(f[0].range, 5..9);
        assert!(!f[0].definition);
        let d = footnotes("[^1]: la nota");
        assert!(d[0].definition && d[0].range == (0..5));
        let t = "Vedi[^1].\n\n[^1]: la nota";
        let def = footnote_target(t, 6).unwrap();
        assert_eq!(
            &t[def..def + 3],
            " la",
            "the reference goes to its definition"
        );
        assert_eq!(footnote_target(t, 12), Some(8), "and back to the reference");
    }

    #[test]
    fn math_is_left_as_written() {
        let k = kinds("area $A = \\pi r^2$ e $$\\sum$$ ma costa $5 e $10");
        let math: Vec<&str> = k
            .iter()
            .filter(|(k, _)| *k == InlineKind::Math)
            .map(|(_, s)| *s)
            .collect();
        assert_eq!(math, vec!["$A = \\pi r^2$", "$$\\sum$$"]);
        assert!(kinds("$x_{1,2}$ e `$x$`")
            .iter()
            .all(|(k, _)| *k != InlineKind::Escape));
        assert!(
            kinds("da $a$1 a $b")
                .iter()
                .all(|(k, _)| *k != InlineKind::Math),
            "a closing `$` followed by a digit closes nothing"
        );
        let block = "testo\n$$\n\\int_0^1 x\\,dx\n$$\ndopo";
        let k: Vec<_> = lines(block).iter().map(|l| l.kind).collect();
        assert_eq!(
            k,
            vec![
                BlockKind::Text,
                BlockKind::Math,
                BlockKind::Math,
                BlockKind::Math,
                BlockKind::Text
            ]
        );
    }

    #[test]
    fn link_at_finds_a_links_shown_text() {
        let text = "Visita [GitHub](https://github.com \"t\") e [[Nota|Alias]].\n`[x](y)`";
        let gh = text.find("GitHub").unwrap();
        assert_eq!(
            link_at(text, gh + 2).map(|l| l.1),
            Some(Link::Url("https://github.com".into()))
        );
        for edge in [gh, gh + "GitHub".len()] {
            assert!(link_at(text, edge).is_some(), "at its edge {edge}");
        }
        assert_eq!(link_at(text, gh - 1), None, "before its `[`");
        assert_eq!(link_at(text, 2), None);
        let alias = text.find("Alias").unwrap();
        match link_at(text, alias).map(|l| l.1) {
            Some(Link::Note(w)) => assert_eq!(w.target, "Nota"),
            other => panic!("{other:?}"),
        }
        assert_eq!(link_at(text, text.len() - 3), None, "not in code");
    }

    #[test]
    fn classifies_frontmatter_and_fences() {
        let text = "---\ntags: [a]\n---\n# H\n```rust\n# not\n```\ntext";
        let k: Vec<_> = lines(text).iter().map(|l| l.kind).collect();
        use BlockKind::*;
        assert_eq!(
            k,
            vec![
                FrontmatterDelimiter,
                Frontmatter,
                FrontmatterDelimiter,
                Text,
                FenceDelimiter,
                Code,
                FenceDelimiter,
                Text
            ]
        );
    }

    #[test]
    fn crlf_lines_drop_carriage_return() {
        let l = lines("a\r\nb");
        assert_eq!(l[0].text, "a");
        assert_eq!(l[1].start, 3);
    }

    #[test]
    fn tokens_basic() {
        assert_eq!(
            kinds("**bold [[Note|alias]]** and `code [[no]]` #tag"),
            vec![
                (InlineKind::Bold, "**bold [[Note|alias]]**"),
                (InlineKind::Wikilink { embed: false }, "[[Note|alias]]"),
                (InlineKind::Code, "`code [[no]]`"),
                (InlineKind::Tag, "#tag"),
            ]
        );
    }

    #[test]
    fn escapes_and_multibyte_are_safe() {
        // Only ASCII punctuation is escaped (CommonMark): `\è` and `\pi` keep
        // their backslash, and nothing lands inside the `è`.
        assert_eq!(kinds("\\è #perché"), vec![(InlineKind::Tag, "#perché")]);
        assert_eq!(kinds("$\\pi$"), vec![(InlineKind::Math, "$\\pi$")]);
        assert_eq!(kinds("\\"), vec![]);
        assert_eq!(kinds("\\#nottag"), vec![(InlineKind::Escape, "\\#")]);
    }

    #[test]
    fn bullets_are_not_italic() {
        assert_eq!(kinds("* item and a * b"), vec![]);
        assert_eq!(kinds("*it*"), vec![(InlineKind::Italic, "*it*")]);
        assert_eq!(
            kinds("**b** *i*"),
            vec![(InlineKind::Bold, "**b**"), (InlineKind::Italic, "*i*")]
        );
    }

    #[test]
    fn headings_need_space() {
        assert_eq!(parse_atx_heading("# A"), Some((1, "A")));
        assert_eq!(parse_atx_heading("#tag"), None);
        assert_eq!(parse_atx_heading("## B ##"), Some((2, "B")));
        assert_eq!(parse_atx_heading("# C#"), Some((1, "C#")));
    }

    #[test]
    fn percent_roundtrip() {
        for s in [
            "Launch plan",
            "Maxwell's equations",
            "a/b (c)#d",
            "perché 🚀",
            "100%",
        ] {
            assert_eq!(percent_decode(&percent_encode(s)), s);
        }
        assert_eq!(percent_decode("bad%zz%"), "bad%zz%");
    }

    #[test]
    fn html_keeps_line_count() {
        let text = "---\na: 1\n---\n> [!tip] Hi [[x]]\n%%gone%% [[A|b]] ==hi==\n```\n[[code]]\n```";
        let out = preprocess_for_html(text, None, &VaultIndex::default());
        assert_eq!(out.split('\n').count(), text.split('\n').count());
        assert!(out.contains("> **Hi [x](noteki-link:x)**"), "{out}");
        assert!(out.contains(" [b](noteki-link:A) **hi**"), "{out}");
        assert!(out.contains("\n[[code]]\n"), "{out}");
    }
}
