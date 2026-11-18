//! What Live preview draws in place of its source while the cursor is
//! elsewhere: a `$$` block as its formula, a `mermaid` fence as its diagram,
//! and each `$…$` of a line as its formula in the text.

use std::ops::Range;

use crate::core::markdown::{inline_tokens, BlockKind, InlineKind, Line};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FigureSource {
    /// The TeX of a `$$` block, between its `$$`.
    Math(String),
    /// The source of a `mermaid` fence, between its fences.
    Mermaid(String),
}

/// A block of lines drawn as one figure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FigureBlock {
    /// The block's lines (indices), its delimiters included.
    pub lines: Range<usize>,
    pub source: FigureSource,
}

/// The `$$` blocks and `mermaid` fences of `lines`. An unclosed one is none:
/// it is still being written.
pub fn figure_blocks(lines: &[Line]) -> Vec<FigureBlock> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let line = &lines[i];
        let trimmed = line.text.trim();
        match line.kind {
            // `$$x$$` alone on its line.
            BlockKind::Text
                if trimmed.len() > 4 && trimmed.starts_with("$$") && trimmed.ends_with("$$") =>
            {
                let tex = &trimmed[2..trimmed.len() - 2];
                if !tex.contains("$$") {
                    out.push(FigureBlock {
                        lines: i..i + 1,
                        source: FigureSource::Math(tex.to_string()),
                    });
                }
            }
            BlockKind::Math => {
                let start = i;
                while i + 1 < lines.len() && lines[i + 1].kind == BlockKind::Math {
                    i += 1;
                }
                let block = &lines[start..=i];
                if let Some(tex) = math_block(block) {
                    out.push(FigureBlock {
                        lines: start..i + 1,
                        source: FigureSource::Math(tex),
                    });
                }
            }
            BlockKind::FenceDelimiter if fence_language(trimmed) == "mermaid" => {
                let start = i;
                let mut end = None;
                let mut j = i + 1;
                while j < lines.len() && lines[j].kind == BlockKind::Code {
                    j += 1;
                }
                if j < lines.len() && lines[j].kind == BlockKind::FenceDelimiter {
                    end = Some(j);
                }
                if let Some(end) = end {
                    let source = lines[start + 1..end]
                        .iter()
                        .map(|l| l.text)
                        .collect::<Vec<_>>()
                        .join("\n");
                    out.push(FigureBlock {
                        lines: start..end + 1,
                        source: FigureSource::Mermaid(source),
                    });
                    i = end;
                }
            }
            _ => {}
        }
        i += 1;
    }
    out
}

/// The TeX of a `$$` block's lines: the first opens with `$$`, the last
/// closes with one.
fn math_block(block: &[Line]) -> Option<String> {
    let first = block.first()?.text.trim();
    let last = block.last()?.text.trim();
    if block.len() < 2 || !first.starts_with("$$") || !last.ends_with("$$") {
        return None;
    }
    let mut tex = first[2..].to_string();
    for l in &block[1..block.len() - 1] {
        tex.push('\n');
        tex.push_str(l.text);
    }
    tex.push('\n');
    tex.push_str(&last[..last.len() - 2]);
    Some(tex)
}

/// The language a fence's opening line names (```` ```mermaid ````).
fn fence_language(trimmed: &str) -> &str {
    trimmed
        .trim_start_matches(['`', '~'])
        .split_whitespace()
        .next()
        .unwrap_or("")
}

/// The formulas of a line of text: each `$…$` or `$$…$$` (whole range, the
/// TeX inside, and whether it is in display style).
pub fn inline_formulas(text: &str) -> Vec<(Range<usize>, Range<usize>, bool)> {
    inline_tokens(text)
        .into_iter()
        .filter(|t| t.kind == InlineKind::Math)
        .map(|t| {
            let display = t.inner.start - t.range.start == 2;
            (t.range, t.inner, display)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::markdown::lines;

    fn blocks(text: &str) -> Vec<FigureBlock> {
        figure_blocks(&lines(text))
    }

    #[test]
    fn a_math_block_is_one_figure_of_its_lines() {
        let b = blocks("Testo\n$$\na^2 + b^2\n= c^2\n$$\ndopo");
        assert_eq!(b.len(), 1);
        assert_eq!(b[0].lines, 1..5);
        assert_eq!(
            b[0].source,
            FigureSource::Math("\na^2 + b^2\n= c^2\n".into())
        );
        let one = blocks("$$e^{i\\pi} + 1 = 0$$");
        assert_eq!(one[0].lines, 0..1);
        assert_eq!(
            one[0].source,
            FigureSource::Math("e^{i\\pi} + 1 = 0".into())
        );
    }

    #[test]
    fn a_mermaid_fence_is_one_figure_and_other_code_is_not() {
        let b = blocks("```mermaid\nflowchart LR\n  A --> B\n```\n```rust\nfn main() {}\n```");
        assert_eq!(b.len(), 1);
        assert_eq!(b[0].lines, 0..4);
        assert_eq!(
            b[0].source,
            FigureSource::Mermaid("flowchart LR\n  A --> B".into())
        );
    }

    #[test]
    fn unclosed_blocks_are_still_being_written() {
        assert!(blocks("$$\na^2").is_empty());
        assert!(blocks("```mermaid\nflowchart LR").is_empty());
    }

    #[test]
    fn a_lines_formulas_are_found_with_their_style() {
        let f = inline_formulas("con $\\rho$ e $$\\sum$$, non `$x$` né $5 e $10");
        assert_eq!(f, vec![(4..10, 5..9, false), (13..21, 15..19, true)]);
    }
}
