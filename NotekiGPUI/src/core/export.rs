//! *Export as HTML…* (SPEC › Export): the Note as one page standing alone,
//! drawn by the `markdown` crate, in light colours. The
//! Frontmatter and Comments are left out, Wikilinks are their name or Alias,
//! the Vault's images are put in as data.

use crate::core::markdown::{
    lines, percent_decode, preprocess_for_html, BlockKind, EMBED_SCHEME, LINK_SCHEME,
};
use crate::core::wikilink::VaultIndex;

/// The Markdown the page is made from: no Frontmatter, no Comment across
/// lines, a line break kept as one (as in Obsidian and the editor).
fn export_source(text: &str) -> String {
    let mut kept: Vec<(BlockKind, String)> = Vec::new();
    let mut in_comment = false;
    for line in lines(text) {
        match line.kind {
            BlockKind::Frontmatter | BlockKind::FrontmatterDelimiter => continue,
            BlockKind::Text => {}
            kind => {
                kept.push((kind, line.text.to_string()));
                continue;
            }
        }
        let marks: Vec<usize> = line.text.match_indices("%%").map(|(p, _)| p).collect();
        let odd = marks.len() % 2 == 1;
        let shown = match (in_comment, odd) {
            (false, false) => Some(line.text.to_string()),
            (false, true) => Some(line.text[..marks[marks.len() - 1]].to_string()),
            (true, false) => None,
            (true, true) => Some(line.text[marks[0] + 2..].to_string()),
        };
        if odd {
            in_comment = !in_comment;
        }
        if let Some(t) = shown {
            kept.push((BlockKind::Text, t));
        }
    }
    let mut out = String::with_capacity(text.len() + 64);
    for (i, (kind, t)) in kept.iter().enumerate() {
        out.push_str(t);
        let next_text = kept
            .get(i + 1)
            .is_some_and(|(k, n)| *k == BlockKind::Text && !n.trim().is_empty());
        if *kind == BlockKind::Text && !t.trim().is_empty() && next_text && !t.ends_with('\\') {
            out.push_str("  "); // a hard break; at a block's end it is nothing
        }
        out.push('\n');
    }
    out
}

/// The page for the Note at `path` (in the Vault, for its links and images).
/// `image` gives the bytes and type of an image by its path in the Vault, or
/// by its address as written in the Note relative to it.
pub fn note_html(
    text: &str,
    title: &str,
    path: Option<&str>,
    index: &VaultIndex,
    image: impl Fn(&str) -> Option<(Vec<u8>, &'static str)>,
) -> String {
    let md = preprocess_for_html(&export_source(text), path, index);
    let mut options = markdown::Options::gfm();
    // Our own `noteki-link:` and `noteki-embed:` addresses, replaced below.
    options.compile.allow_dangerous_protocol = true;
    options.compile.allow_any_img_src = true;
    let body = markdown::to_html_with_options(&md, &options)
        .unwrap_or_else(|_| format!("<pre>{}</pre>", escape(text)));
    let body = unlink_notes(&body);
    let body = inline_images(&body, image);
    format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <title>{}</title>\n<style>{CSS}</style>\n</head>\n<body>\n<article>\n{body}</article>\n</body>\n</html>\n",
        escape(title)
    )
}

/// Links to Notes are their text: the page stands alone.
fn unlink_notes(html: &str) -> String {
    let re = regex::Regex::new(&format!(
        r#"<a href="{}[^"]*">((?s:.*?))</a>"#,
        regex::escape(LINK_SCHEME)
    ))
    .expect("valid pattern");
    let html = re.replace_all(html, "<span class=\"wikilink\">$1</span>");
    // Allowed for our own addresses only: a script address stays dead.
    let script = regex::Regex::new(r#"(?i)href="\s*(javascript|vbscript|data):[^"]*""#)
        .expect("valid pattern");
    script.replace_all(&html, "href=\"\"").into_owned()
}

/// Every `<img src>` of the Vault as a `data:` address; a web address stays.
fn inline_images(html: &str, image: impl Fn(&str) -> Option<(Vec<u8>, &'static str)>) -> String {
    let re = regex::Regex::new(r#"<img src="([^"]*)""#).expect("valid pattern");
    re.replace_all(html, |c: &regex::Captures| {
        let src = html_unescape(&c[1]);
        if src.starts_with("http:") || src.starts_with("https:") || src.starts_with("data:") {
            return c[0].to_string();
        }
        let path = percent_decode(src.strip_prefix(EMBED_SCHEME).unwrap_or(&src));
        match image(&path) {
            Some((bytes, mime)) => format!("<img src=\"data:{mime};base64,{}\"", base64(&bytes)),
            None => c[0].to_string(),
        }
    })
    .into_owned()
}

fn html_unescape(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// The type of an image by its extension.
pub fn image_mime(path: &str) -> Option<&'static str> {
    let ext = path.rsplit('.').next()?.to_lowercase();
    Some(match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "bmp" => "image/bmp",
        _ => return None,
    })
}

fn base64(bytes: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = (chunk[0] as u32) << 16
            | (*chunk.get(1).unwrap_or(&0) as u32) << 8
            | *chunk.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(A[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

const CSS: &str = "\
:root{color-scheme:light}\
body{margin:0;background:#fcfcfc;color:#3c4048;font:16px/1.6 -apple-system,BlinkMacSystemFont,\"Helvetica Neue\",sans-serif}\
article{max-width:50em;margin:0 auto;padding:3em 1.5em}\
h1,h2,h3,h4,h5,h6{line-height:1.25;margin:1.4em 0 .5em;color:#2a2d33}\
a{color:#2f7ad1}\
.wikilink{color:#2f7ad1}\
img{max-width:100%;height:auto}\
code{font:.9em ui-monospace,Menlo,monospace;background:#f0f0f0;padding:.1em .3em;border-radius:3px}\
pre{background:#f5f5f5;padding:.8em 1em;border-radius:6px;overflow:auto}\
pre code{background:none;padding:0}\
blockquote{margin:1em 0;padding:.2em 1em;border-left:3px solid #d6d6d6;color:#5c6066}\
table{border-collapse:collapse}th,td{border:1px solid #ddd;padding:.3em .7em}\
strong{color:#2a2d33}\
hr{border:none;border-top:1px solid #ddd}\
li input[type=checkbox]{margin-right:.4em}";

#[cfg(test)]
mod tests {
    use super::*;

    fn html(text: &str) -> String {
        note_html(text, "T", None, &VaultIndex::default(), |_| None)
    }

    #[test]
    fn frontmatter_and_comments_left_out_and_breaks_kept() {
        let h = html("---\ntags: [a]\n---\n# Titolo\n\nuna\ndue %%nascosto%%\n\n%% via\nancora via\n%% resta");
        assert!(!h.contains("tags"), "{h}");
        assert!(h.contains("<h1>Titolo</h1>"));
        assert!(h.contains("una<br />\ndue"), "a line break is one: {h}");
        assert!(!h.contains("nascosto") && !h.contains("via"), "{h}");
        assert!(h.contains("resta"));
        assert!(h.starts_with("<!doctype html>") && h.contains("<title>T</title>"));
    }

    #[test]
    fn wikilinks_are_text_and_images_are_data() {
        let h = html("Vedi [[Nota|l'altra]] e [web](https://x.y)");
        assert!(h.contains("<span class=\"wikilink\">l'altra</span>"), "{h}");
        assert!(!h.contains(LINK_SCHEME));
        assert!(h.contains("href=\"https://x.y\""));
        assert!(
            !html("[x](javascript:alert(1))").contains("javascript"),
            "no script links"
        );
        let h = note_html("![](Media/a.png)", "T", None, &VaultIndex::default(), |p| {
            (p == "Media/a.png").then(|| (b"abc".to_vec(), "image/png"))
        });
        assert!(h.contains("src=\"data:image/png;base64,YWJj\""), "{h}");
        assert_eq!(base64(b"ab"), "YWI=");
        assert_eq!(base64(b"a"), "YQ==");
    }

    #[test]
    fn the_test_note_exports_in_one_piece() {
        let text = include_str!("../../tests/fixtures/rendering-test.md");
        let h = html(text);
        assert!(h.contains("<table>") && h.contains("<pre>") && h.contains("<blockquote>"));
        assert!(h.contains("type=\"checkbox\""), "tasks");
        assert!(!h.contains(LINK_SCHEME) && !h.contains("%%"));
        assert!(h.ends_with("</html>\n"));
    }
}
