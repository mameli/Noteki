//! Markdown scanning for the vault indexes (tags, backlinks). The editor itself
//! is CodeMirror and does its own parsing; this only has to agree with it on
//! what is code, frontmatter, a wikilink or a tag.
//!
//! Ported from the GPUI prototype, where a fuzzer checked it. It works on UTF-8
//! byte offsets and only ever advances by whole characters.

use std::ops::Range;

use crate::frontmatter::{frontmatter_end_line, is_valid_tag};

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
        } else if let Some(open) = opening_fence(line_text.trim_start()) {
            fence = Some(open);
            BlockKind::FenceDelimiter
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
            b'\\' => {
                let next = if i + 1 < to { char_len_at(s, i + 1) } else { 0 };
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
        assert_eq!(
            kinds("\\è #perché"),
            vec![(InlineKind::Escape, "\\è"), (InlineKind::Tag, "#perché")]
        );
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
}
