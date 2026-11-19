//! The Markdown rewritten for Export: Wikilinks, Embeds, callouts and
//! Comments made plain CommonMark for markdown-it.

use super::*;

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

pub(super) fn escape_label(s: &str) -> String {
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

pub(super) fn rewrite_callout_line(line: &str) -> Option<String> {
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

pub(super) fn rewrite_inline_plain(s: &str) -> String {
    rewrite_inline(s, None, &VaultIndex::default())
}

pub(super) fn rewrite_inline(line: &str, source_path: Option<&str>, index: &VaultIndex) -> String {
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

pub(super) fn render_wikilink(
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
