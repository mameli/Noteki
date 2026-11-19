//! Inline level: emphasis, code, links, Wikilinks, Embeds, Tags, footnotes
//! and callout marks within a line.

use super::*;

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

pub(super) fn char_len_at(s: &str, i: usize) -> usize {
    s[i..].chars().next().map_or(0, char::len_utf8)
}

/// Finds `pat` in `s[from..to]`, returning an absolute offset.
pub(super) fn find_in(s: &str, from: usize, to: usize, pat: &str) -> Option<usize> {
    if from > to {
        return None;
    }
    s[from..to].find(pat).map(|p| from + p)
}

pub(super) fn scan_inline(s: &str, from: usize, to: usize, out: &mut Vec<Inline>) {
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
            // text); `$$x$$` too. Nothing inside is Markdown, and it ends
            // before any code (`` `$x$` `` stays code).
            b'$' => {
                let n = if starts(i, "$$") { 2 } else { 1 };
                let code = find_in(s, i + n, to, "`").unwrap_or(to);
                let close = if n == 2 {
                    find_in(s, i + 2, code, "$$")
                } else if i + 1 < to && !b[i + 1].is_ascii_whitespace() {
                    (i + 2..code).find(|&p| {
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
pub(super) fn find_closing(s: &str, from: usize, to: usize, marker: &str) -> Option<usize> {
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
