//! Paste (SPEC › Writing): text copied with its formatting (a web page, Mail,
//! Outlook, Teams, Word, Google Docs) is pasted as Markdown: headings, bold,
//! italic, strikethrough, links, lists, tasks, quotes, code, tables and images
//! on the web. Paste and Match Style, a paste into code, the Frontmatter or a
//! Comment, and text whose HTML carries nothing but its look (an editor's
//! colours) stay plain text. As 0.3 did with Turndown.

use std::sync::OnceLock;

use htmd::element_handler::{Element, HandlerResult, Handlers};
use htmd::options::{BulletListMarker, Options};
use htmd::HtmlToMarkdown;

use super::markdown::{self, BlockKind, MdSpanKind};

/// Elements that make HTML more than plain text with a look: what is worth
/// turning into Markdown.
fn has_structure(html: &str) -> bool {
    const TAGS: [&str; 18] = [
        "h1",
        "h2",
        "h3",
        "h4",
        "h5",
        "h6",
        "b",
        "strong",
        "em",
        "i",
        "a",
        "ul",
        "ol",
        "blockquote",
        "table",
        "pre",
        "code",
        "img",
    ];
    const MORE: [&str; 5] = ["li", "del", "s", "strike", "hr"];
    let lower = html.to_ascii_lowercase();
    TAGS.iter().chain(MORE.iter()).any(|tag| {
        let open = format!("<{tag}");
        lower.match_indices(&open).any(|(i, _)| {
            matches!(
                lower[i + open.len()..].chars().next(),
                Some('>' | ' ' | '/' | '\t' | '\n' | '\r')
            )
        })
    })
}

/// An editor's code (VS Code, Xcode) is copied as coloured spans in a div
/// that keeps its spaces, and as its text: the text is what is meant.
fn is_code_from_an_editor(html: &str) -> bool {
    let lower = html.to_ascii_lowercase();
    if lower.contains("<pre") {
        return false;
    }
    let body = lower
        .trim_start()
        .strip_prefix("<meta")
        .and_then(|rest| rest.split_once('>'))
        .map_or(lower.trim_start(), |(_, rest)| rest.trim_start());
    body.starts_with("<div")
        && body
            .split_once('>')
            .is_some_and(|(open, _)| open.replace(' ', "").contains("white-space:pre"))
}

/// The content of an element's children, inline.
fn children(handlers: &dyn Handlers, element: &Element) -> String {
    handlers.walk_children_content(element.node, element.context)
}

fn converter() -> &'static HtmlToMarkdown {
    static CONVERTER: OnceLock<HtmlToMarkdown> = OnceLock::new();
    CONVERTER.get_or_init(|| {
        HtmlToMarkdown::builder()
            .options(Options {
                bullet_list_marker: BulletListMarker::Dash,
                // One space after a list's marker, as Obsidian writes it.
                ul_bullet_spacing: 1,
                ol_number_spacing: 1,
                ..Default::default()
            })
            // Not text: what a page carries besides it.
            .skip_tags(vec!["script", "style", "title", "meta", "head"])
            .add_handler(
                vec!["del", "s", "strike"],
                |h: &dyn Handlers, e: &Element| {
                    let text = children(h, e);
                    let trimmed = text.trim();
                    Some(if trimmed.is_empty() {
                        text.into()
                    } else {
                        format!("~~{trimmed}~~").into()
                    })
                },
            )
            // Google Docs wraps all it copies in a `<b style="font-weight:normal">`: not bold.
            .add_handler(vec!["b", "strong"], |h: &dyn Handlers, e: &Element| {
                let style = e
                    .node
                    .attr("style")
                    .map(|s| s.to_string())
                    .unwrap_or_default();
                let plain = style
                    .replace(' ', "")
                    .split(';')
                    .any(|d| d == "font-weight:normal" || d == "font-weight:400");
                if plain {
                    Some(HandlerResult::from(children(h, e)))
                } else {
                    h.fallback(e)
                }
            })
            // Word and Outlook: their empty paragraphs.
            .add_handler(vec!["o:p"], |h: &dyn Handlers, e: &Element| {
                Some(children(h, e).into())
            })
            // A task's checkbox, as GitHub and Obsidian copy it.
            .add_handler(vec!["input"], |_: &dyn Handlers, e: &Element| {
                let checkbox = e
                    .node
                    .attr("type")
                    .is_some_and(|t| t.eq_ignore_ascii_case("checkbox"));
                checkbox.then(|| {
                    let checked = e.node.attr("checked").is_some();
                    HandlerResult::from(if checked { "[x] " } else { "[ ] " })
                })
            })
            .build()
    })
}

/// Turndown's thematic break, as this app writes it.
fn tidy(markdown: &str) -> String {
    let lines: Vec<&str> = markdown
        .lines()
        .map(|l| match l.trim() {
            "* * *" | "- - -" | "_ _ _" => "---",
            _ => l.trim_end(),
        })
        .collect();
    let mut out = String::new();
    let mut blank = 0;
    for line in lines {
        blank = if line.is_empty() { blank + 1 } else { 0 };
        if blank < 2 {
            out.push_str(line);
            out.push('\n');
        }
    }
    out.trim().to_string()
}

/// The Markdown of `html`, or `None` when it is not worth it: the plain text
/// is pasted as it is.
pub fn markdown_of(html: &str, plain: &str) -> Option<String> {
    if !has_structure(html) || is_code_from_an_editor(html) {
        return None;
    }
    let markdown = tidy(&converter().convert(html).ok()?);
    if markdown.is_empty() {
        return None;
    }
    // Nothing gained over the plain text: keep it, with its line breaks.
    let words = |s: &str| s.split_whitespace().collect::<Vec<_>>().join(" ");
    (words(&markdown) != words(plain)).then_some(markdown)
}

/// Whether a paste at `offset` of `text` stays plain text: in code, the
/// Frontmatter, a `$$` block or a Comment nothing is Markdown.
pub fn pastes_plain_at(text: &str, offset: usize) -> bool {
    let Some(line) = markdown::lines(text)
        .into_iter()
        .find(|l| l.start <= offset && offset <= l.start + l.text.len())
    else {
        return false;
    };
    if line.kind != BlockKind::Text && line.kind != BlockKind::Rule {
        return true;
    }
    let mut spans = Vec::new();
    markdown::line_spans(&line, None, None, &Default::default(), &mut spans);
    spans.iter().any(|s| {
        matches!(s.kind, MdSpanKind::InlineCode | MdSpanKind::Comment)
            && s.range.start < offset
            && offset < s.range.end
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn md(html: &str) -> Option<String> {
        markdown_of(html, "")
    }

    #[test]
    fn a_web_page_becomes_markdown() {
        let html =
            "<meta charset='utf-8'><h2>Piano</h2><p>Un <b>grassetto</b>, un <em>corsivo</em>, \
            <del>barrato</del> e un <a href=\"https://noteki.app\">link</a>.</p>\
            <ul><li>uno</li><li>due</li></ul><ol><li>primo</li><li>secondo</li></ol>\
            <blockquote><p>citazione</p></blockquote><pre><code>let x = 1;</code></pre><hr>";
        assert_eq!(
            md(html).unwrap(),
            "## Piano\n\nUn **grassetto**, un *corsivo*, ~~barrato~~ e un [link](https://noteki.app).\n\n\
             - uno\n- due\n\n1. primo\n2. secondo\n\n> citazione\n\n```\nlet x = 1;\n```\n\n---"
        );
    }

    #[test]
    fn a_table_becomes_a_markdown_table() {
        let html =
            "<table><tr><th>Nome</th><th>Ruolo</th></tr><tr><td>Ada</td><td>Dev</td></tr></table>";
        let out = md(html).unwrap();
        assert!(out.contains("| Nome | Ruolo |"), "{out}");
        assert!(out.contains("| Ada"), "{out}");
    }

    #[test]
    fn tasks_keep_their_checkbox() {
        let html = "<ul><li><input type=\"checkbox\" checked> fatto</li><li><input type=\"checkbox\"> da fare</li></ul>";
        assert_eq!(md(html).unwrap(), "- [x] fatto\n- [ ] da fare");
    }

    #[test]
    fn google_docs_wrapper_is_not_bold() {
        let html = "<b style=\"font-weight:normal;\" id=\"docs-internal-guid-1\"><p>Testo <span>e</span> <b>forte</b></p></b>";
        assert_eq!(md(html).unwrap(), "Testo e **forte**");
    }

    #[test]
    fn text_with_only_a_look_stays_plain() {
        assert_eq!(
            markdown_of("<span style=\"color:red\">ciao</span>", "ciao"),
            None
        );
        let vscode = "<meta charset='utf-8'><div style=\"color: #d4d4d4;white-space: pre;\"><div><span style=\"color: #569cd6;\">let</span> x</div></div>";
        assert_eq!(markdown_of(vscode, "let x"), None);
        assert_eq!(
            markdown_of("<p>solo <span>testo</span></p><a>x</a>", "solo testo x"),
            None,
            "nothing gained over the plain text"
        );
    }

    #[test]
    fn in_code_the_frontmatter_or_a_comment_a_paste_is_plain() {
        let text = "---\ntags: a\n---\ntesto `codice` %%nota%%\n```\ncode\n```\n";
        let at = |s: &str| text.find(s).unwrap() + 1;
        assert!(pastes_plain_at(text, at("tags")));
        assert!(pastes_plain_at(text, at("codice")));
        assert!(pastes_plain_at(text, at("nota")));
        assert!(pastes_plain_at(text, at("code\n")));
        assert!(!pastes_plain_at(text, at("testo")));
    }
}
