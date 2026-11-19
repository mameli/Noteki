//! Markdown scanning shared by the live editor, Export, and the vault indexes.
//!
//! Everything here works on UTF-8 byte offsets and only ever advances by whole
//! characters, so ranges are always valid `str` slice boundaries (the editor's
//! decorations rely on that).
//!
//! `blocks` (each line's kind), `inline` (within a line), `spans` (what Live
//! preview draws), `html` (the Markdown rewritten for Export); all of it
//! reachable as `markdown::…`.

use std::ops::Range;

use crate::core::frontmatter::{frontmatter_end_line, is_valid_tag};
use crate::core::vault::AttachmentKind;
use crate::core::wikilink::{ResolvedItem, VaultIndex, Wikilink};

mod blocks;
mod html;
mod inline;
mod spans;

pub use blocks::*;
pub use html::*;
pub use inline::*;
pub use spans::*;

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
        assert!(
            kinds("costa $5 e $10, scritto \\$ e il codice `$x$`.")
                .iter()
                .all(|(k, _)| *k != InlineKind::Math),
            "a formula ends before any code"
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
