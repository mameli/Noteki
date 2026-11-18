//! Colours of a code block's code, from syntect with bat's syntaxes (the set
//! GPUI Kit builds in): what each part is (keyword, string, comment...), as
//! Live preview span kinds that the theme colours.

use std::ops::Range;
use std::sync::LazyLock;

use syntect::parsing::{ParseState, Scope, ScopeStack, SyntaxReference, SyntaxSet};
use syntect::util::LinesWithEndings;

use crate::core::markdown::MdSpanKind;

/// Loading the syntaxes takes tens of milliseconds: once, on the first block.
static SYNTAXES: LazyLock<SyntaxSet> = LazyLock::new(two_face::syntax::extra_newlines);

/// TextMate scope prefixes and the kind they are drawn as; the first that
/// matches the innermost scope wins.
static KINDS: LazyLock<Vec<(Scope, MdSpanKind)>> = LazyLock::new(|| {
    [
        ("comment", MdSpanKind::CodeComment),
        ("string", MdSpanKind::CodeString),
        ("constant.numeric", MdSpanKind::CodeNumber),
        ("constant.language", MdSpanKind::CodeKeyword),
        ("keyword", MdSpanKind::CodeKeyword),
        ("storage", MdSpanKind::CodeKeyword),
        ("entity.name.type", MdSpanKind::CodeType),
        ("support.type", MdSpanKind::CodeType),
        ("support.class", MdSpanKind::CodeType),
        ("entity.name.function", MdSpanKind::CodeFunction),
        ("support.function", MdSpanKind::CodeFunction),
    ]
    .into_iter()
    .filter_map(|(s, k)| Scope::new(s).ok().map(|s| (s, k)))
    .collect()
});

fn syntax(language: &str) -> Option<&'static SyntaxReference> {
    let language = language.trim();
    if language.is_empty() {
        return None;
    }
    SYNTAXES
        .find_syntax_by_token(language)
        .or_else(|| SYNTAXES.find_syntax_by_token(&language.to_lowercase()))
        .filter(|s| s.name != "Plain Text")
}

/// The coloured ranges of `code` (offsets in `code`), for a fence naming
/// `language`; nothing for a language syntect does not know.
pub fn highlight(language: &str, code: &str) -> Vec<(Range<usize>, MdSpanKind)> {
    let Some(syntax) = syntax(language) else {
        return Vec::new();
    };
    let mut parser = ParseState::new(syntax);
    let mut stack = ScopeStack::new();
    let mut out: Vec<(Range<usize>, MdSpanKind)> = Vec::new();
    let mut push = |range: Range<usize>, stack: &ScopeStack| {
        let kind = stack.as_slice().iter().rev().find_map(|scope| {
            KINDS
                .iter()
                .find(|(prefix, _)| prefix.is_prefix_of(*scope))
                .map(|(_, k)| *k)
        });
        match (kind, out.last_mut()) {
            (Some(k), Some((last, lk))) if *lk == k && last.end == range.start => {
                last.end = range.end
            }
            (Some(k), _) if !range.is_empty() => out.push((range, k)),
            _ => {}
        }
    };
    let mut offset = 0;
    for line in LinesWithEndings::from(code) {
        let Ok(ops) = parser.parse_line(line, &SYNTAXES) else {
            break;
        };
        let mut at = 0;
        for (i, op) in ops {
            push(offset + at..offset + i, &stack);
            at = i;
            if stack.apply(&op).is_err() {
                break;
            }
        }
        push(offset + at..offset + line.len(), &stack);
        offset += line.len();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn swift_from_the_test_note_is_coloured() {
        let code = "struct Note {\n    let title: String\n}\nprint(\"Hello\") // ciao\n";
        let h = highlight("swift", code);
        let kind_of = |s: &str| {
            let at = code.find(s).unwrap();
            h.iter().find(|(r, _)| r.contains(&at)).map(|(_, k)| *k)
        };
        assert_eq!(kind_of("struct"), Some(MdSpanKind::CodeKeyword));
        assert_eq!(kind_of("\"Hello\""), Some(MdSpanKind::CodeString));
        assert_eq!(kind_of("// ciao"), Some(MdSpanKind::CodeComment));
        for (r, _) in &h {
            assert!(code.is_char_boundary(r.start) && code.is_char_boundary(r.end));
        }
    }

    #[test]
    fn an_unknown_or_missing_language_gets_no_colours() {
        assert!(highlight("", "let x = 1").is_empty());
        assert!(highlight("mermaid", "flowchart LR\n A --> B").is_empty());
        assert!(!highlight("RUST", "fn main() {}").is_empty(), "any case");
    }
}
