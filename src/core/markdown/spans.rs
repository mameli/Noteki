//! Live preview spans: what each part of a line is drawn as, and which of
//! its marks hide.

use super::*;

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
    /// A formula RaTeX cannot typeset, shown as written in red (SPEC).
    MathError,
    /// The Frontmatter line where Noteki stopped reading its YAML, which then
    /// stays as source (SPEC › Frontmatter, ADR 0009).
    FrontmatterError,
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
                        // Math as written, marks and all, as code: the editor draws
                        // it as its formula off the cursor (core::editor::figures).
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
