use crate::core::frontmatter::{extract_frontmatter_tags, frontmatter_end_line, is_valid_tag};
use crate::core::vault::read_note;
use crate::core::wikilink::{folder_of, ResolvedItem, VaultIndex, Wikilink};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::ops::Range;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeadingItem {
    pub level: u8,
    pub text: String,
    pub line: usize, // 1-based
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagNode {
    pub full_lower: String,
    pub label: String,
    pub count: usize,
    pub note_paths: Vec<String>,
    pub children: Vec<TagNode>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BacklinkMatch {
    pub source_path: String,
    pub source_name: String,
    pub line: usize, // 1-based
    pub snippet: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchLineMatch {
    pub line: usize, // 1-based
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchResult {
    pub path: String,
    pub name: String,
    pub name_matched: bool,
    pub lines: Vec<SearchLineMatch>,
    pub total_line_matches: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuickSwitcherEntry {
    pub name: String,
    pub path: String,
    pub folder: String,
    pub score: i64,
}

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
}

#[derive(Debug, Clone)]
pub struct MdSpan {
    pub range: Range<usize>,
    pub kind: MdSpanKind,
}

pub fn extract_outline(text: &str) -> Vec<HeadingItem> {
    let fm_end = frontmatter_end_line(text).unwrap_or(0);
    let lines: Vec<&str> = text.lines().collect();
    let mut headings = Vec::new();
    let mut fence: Option<(char, usize)> = None;

    for (idx, line) in lines.iter().enumerate() {
        let line_no = idx + 1;
        if line_no <= fm_end {
            continue;
        }
        let trimmed = line.trim_start();
        if let Some((ch, len)) = fence {
            if is_closing_fence(trimmed, ch, len) {
                fence = None;
            }
            continue;
        }
        if let Some((ch, len)) = opening_fence(trimmed) {
            fence = Some((ch, len));
            continue;
        }

        if let Some((level, raw_title)) = parse_atx_heading(trimmed) {
            let cleaned = strip_inline_markdown(raw_title);
            if !cleaned.is_empty() {
                headings.push(HeadingItem {
                    level,
                    text: cleaned,
                    line: line_no,
                });
            }
            continue;
        }

        if idx + 1 < lines.len() && idx >= fm_end {
            let next_trimmed = lines[idx + 1].trim();
            let is_h1 = next_trimmed.len() >= 3 && next_trimmed.chars().all(|c| c == '=');
            let is_h2 = next_trimmed.len() >= 3 && next_trimmed.chars().all(|c| c == '-');
            if (is_h1 || is_h2) && is_setext_candidate(trimmed) {
                let cleaned = strip_inline_markdown(trimmed);
                if !cleaned.is_empty() {
                    headings.push(HeadingItem {
                        level: if is_h1 { 1 } else { 2 },
                        text: cleaned,
                        line: line_no,
                    });
                }
            }
        }
    }

    headings
}

fn is_setext_candidate(trimmed: &str) -> bool {
    if trimmed.is_empty() {
        return false;
    }
    if trimmed.starts_with('>')
        || trimmed.starts_with("- ")
        || trimmed.starts_with("* ")
        || trimmed.starts_with("+ ")
        || trimmed.starts_with('#')
    {
        return false;
    }
    if trimmed.chars().all(|c| c == '-' || c == '*' || c == '_') {
        return false;
    }
    true
}

fn parse_atx_heading(trimmed: &str) -> Option<(u8, &str)> {
    let hashes = trimmed.bytes().take_while(|&b| b == b'#').count();
    if hashes == 0 || hashes > 6 {
        return None;
    }
    let rest = &trimmed[hashes..];
    if !rest.is_empty() && !rest.starts_with(' ') && !rest.starts_with('\t') {
        return None;
    }
    let title = rest.trim().trim_end_matches('#').trim();
    Some((hashes as u8, title))
}

fn opening_fence(trimmed: &str) -> Option<(char, usize)> {
    let first = trimmed.chars().next()?;
    if first != '`' && first != '~' {
        return None;
    }
    let count = trimmed.chars().take_while(|&c| c == first).count();
    if count >= 3 {
        Some((first, count))
    } else {
        None
    }
}

fn is_closing_fence(trimmed: &str, ch: char, min_len: usize) -> bool {
    let count = trimmed.chars().take_while(|&c| c == ch).count();
    count >= min_len && trimmed[count..].trim().is_empty()
}

pub fn strip_inline_markdown(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if i + 1 < chars.len() && chars[i] == '[' && chars[i + 1] == '[' {
            if let Some(end) = find_double_close(&chars, i + 2, ']') {
                let inner: String = chars[i + 2..end].iter().collect();
                out.push_str(&Wikilink::parse(&inner).display_text());
                i = end + 2;
                continue;
            }
        }
        if chars[i] == '[' {
            if let Some(close_br) = chars[i + 1..].iter().position(|&c| c == ']') {
                let close_br = i + 1 + close_br;
                if close_br + 1 < chars.len() && chars[close_br + 1] == '(' {
                    if let Some(close_paren) = chars[close_br + 2..].iter().position(|&c| c == ')') {
                        let label: String = chars[i + 1..close_br].iter().collect();
                        out.push_str(&label);
                        i = close_br + 2 + close_paren + 1;
                        continue;
                    }
                }
            }
        }
        if matches!(chars[i], '*' | '_' | '~' | '`' | '=') {
            i += 1;
            continue;
        }
        out.push(chars[i]);
        i += 1;
    }
    out.trim().to_string()
}

fn find_double_close(chars: &[char], start: usize, ch: char) -> Option<usize> {
    let mut i = start;
    while i + 1 < chars.len() {
        if chars[i] == ch && chars[i + 1] == ch {
            return Some(i);
        }
        i += 1;
    }
    None
}

pub fn extract_note_tags(text: &str) -> Vec<String> {
    let mut tags = extract_frontmatter_tags(text);
    let fm_end = frontmatter_end_line(text).unwrap_or(0);
    let mut fence: Option<(char, usize)> = None;

    for (idx, line) in text.lines().enumerate() {
        let line_no = idx + 1;
        if line_no <= fm_end {
            continue;
        }
        let trimmed = line.trim_start();
        if let Some((ch, len)) = fence {
            if is_closing_fence(trimmed, ch, len) {
                fence = None;
            }
            continue;
        }
        if let Some((ch, len)) = opening_fence(trimmed) {
            fence = Some((ch, len));
            continue;
        }

        scan_line_tags(line, &mut tags);
    }

    tags
}

fn scan_line_tags(line: &str, out: &mut Vec<String>) {
    let bytes = line.as_bytes();
    let mut in_code = false;
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' {
            i += 2;
            continue;
        }
        if bytes[i] == b'`' {
            in_code = !in_code;
            i += 1;
            continue;
        }
        if !in_code && bytes[i] == b'#' {
            let at_boundary = i == 0 || bytes[i - 1].is_ascii_whitespace();
            if at_boundary {
                let rest = &line[i + 1..];
                let tag_len = rest
                    .char_indices()
                    .take_while(|(_, c)| c.is_alphanumeric() || *c == '_' || *c == '-' || *c == '/')
                    .last()
                    .map(|(idx, c)| idx + c.len_utf8())
                    .unwrap_or(0);
                if tag_len > 0 {
                    let candidate = rest[..tag_len].trim_end_matches('/');
                    if is_valid_tag(candidate) {
                        out.push(candidate.to_string());
                    }
                    i += 1 + tag_len;
                    continue;
                }
            }
        }
        let ch_len = line[i..].chars().next().map(|c| c.len_utf8()).unwrap_or(1);
        i += ch_len;
    }
}

pub fn build_tag_tree(root: &Path, notes: &[(String, String)]) -> Vec<TagNode> {
    let mut spelling_counts: HashMap<String, HashMap<String, usize>> = HashMap::new();
    let mut tag_notes: HashMap<String, HashSet<String>> = HashMap::new();

    for (_, path) in notes {
        let Ok(text) = read_note(root, path) else {
            continue;
        };
        let mut seen_in_note = HashSet::new();
        for raw_tag in extract_note_tags(&text) {
            let lower = raw_tag.to_lowercase();
            *spelling_counts
                .entry(lower.clone())
                .or_default()
                .entry(raw_tag)
                .or_insert(0) += 1;

            let parts: Vec<&str> = lower.split('/').collect();
            let mut acc = String::new();
            for part in parts {
                if !acc.is_empty() {
                    acc.push('/');
                }
                acc.push_str(part);
                if seen_in_note.insert(acc.clone()) {
                    tag_notes.entry(acc.clone()).or_default().insert(path.clone());
                }
            }
        }
    }

    let preferred_spelling = |full_lower: &str, segment_lower: &str| -> String {
        if let Some(counts) = spelling_counts.get(full_lower) {
            if let Some((best, _)) = counts
                .iter()
                .max_by(|(a_str, a_cnt), (b_str, b_cnt)| {
                    a_cnt.cmp(b_cnt).then_with(|| {
                        (*a_str == &a_str.to_lowercase()).cmp(&(*b_str == &b_str.to_lowercase()))
                    })
                })
            {
                return best.rsplit('/').next().unwrap_or(segment_lower).to_string();
            }
        }
        segment_lower.to_string()
    };

    #[derive(Default)]
    struct Builder {
        children: BTreeMap<String, Builder>,
    }

    let mut root_builder = Builder::default();
    for full_lower in tag_notes.keys() {
        let mut cur = &mut root_builder;
        for seg in full_lower.split('/') {
            cur = cur.children.entry(seg.to_string()).or_default();
        }
    }

    fn finalize(
        prefix: &str,
        node: Builder,
        tag_notes: &HashMap<String, HashSet<String>>,
        preferred_spelling: &dyn Fn(&str, &str) -> String,
    ) -> Vec<TagNode> {
        let mut out = Vec::new();
        for (seg, child_builder) in node.children {
            let full_lower = if prefix.is_empty() {
                seg.clone()
            } else {
                format!("{prefix}/{seg}")
            };
            let mut note_paths: Vec<String> = tag_notes
                .get(&full_lower)
                .map(|s| s.iter().cloned().collect())
                .unwrap_or_default();
            note_paths.sort_by_key(|p| p.to_lowercase());
            let count = note_paths.len();
            let children = finalize(&full_lower, child_builder, tag_notes, preferred_spelling);
            let label = preferred_spelling(&full_lower, &seg);
            out.push(TagNode {
                full_lower,
                label,
                count,
                note_paths,
                children,
            });
        }
        out.sort_by(|a, b| a.label.to_lowercase().cmp(&b.label.to_lowercase()));
        out
    }

    finalize("", root_builder, &tag_notes, &preferred_spelling)
}

pub fn find_backlinks(
    root: &Path,
    notes: &[(String, String)],
    target_path: &str,
    index: &VaultIndex,
) -> Vec<BacklinkMatch> {
    let mut results = Vec::new();
    for (name, path) in notes {
        if path == target_path {
            continue;
        }
        let Ok(text) = read_note(root, path) else {
            continue;
        };
        let mut fence: Option<(char, usize)> = None;
        for (idx, line) in text.lines().enumerate() {
            let trimmed = line.trim_start();
            if let Some((ch, len)) = fence {
                if is_closing_fence(trimmed, ch, len) {
                    fence = None;
                }
                continue;
            }
            if let Some((ch, len)) = opening_fence(trimmed) {
                fence = Some((ch, len));
                continue;
            }

            if line_links_to_target(line, path, target_path, index) {
                results.push(BacklinkMatch {
                    source_path: path.clone(),
                    source_name: name.clone(),
                    line: idx + 1,
                    snippet: trim_snippet(line.trim(), 110),
                });
            }
        }
    }
    results.sort_by(|a, b| {
        a.source_name
            .to_lowercase()
            .cmp(&b.source_name.to_lowercase())
            .then_with(|| a.line.cmp(&b.line))
    });
    results
}

fn line_links_to_target(
    line: &str,
    source_path: &str,
    target_path: &str,
    index: &VaultIndex,
) -> bool {
    let bytes = line.as_bytes();
    let mut in_code = false;
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' {
            i += 2;
            continue;
        }
        if bytes[i] == b'`' {
            in_code = !in_code;
            i += 1;
            continue;
        }
        if !in_code && i + 1 < bytes.len() && bytes[i] == b'[' && bytes[i + 1] == b'[' {
            if let Some(rel_end) = line[i + 2..].find("]]") {
                let inner = &line[i + 2..i + 2 + rel_end];
                let link = Wikilink::parse(inner);
                if !link.target.is_empty() {
                    if let Some(ResolvedItem::Note { path, .. }) =
                        index.resolve(&link.target, Some(source_path))
                    {
                        if path == target_path {
                            return true;
                        }
                    }
                }
                i += 2 + rel_end + 2;
                continue;
            }
        }
        let ch_len = line[i..].chars().next().map(|c| c.len_utf8()).unwrap_or(1);
        i += ch_len;
    }
    false
}

fn trim_snippet(s: &str, max_chars: usize) -> String {
    let count = s.chars().count();
    if count <= max_chars {
        s.to_string()
    } else {
        let mut truncated: String = s.chars().take(max_chars.saturating_sub(1)).collect();
        truncated.push('…');
        truncated
    }
}

pub fn search_vault(root: &Path, notes: &[(String, String)], query: &str) -> Vec<SearchResult> {
    let tokens = parse_search_tokens(query);
    if tokens.words.is_empty() && tokens.tags.is_empty() && tokens.paths.is_empty() {
        return Vec::new();
    }

    let mut results = Vec::new();
    for (name, path) in notes {
        let lower_path = path.to_lowercase();
        if !tokens
            .paths
            .iter()
            .all(|p| lower_path.contains(&p.to_lowercase()))
        {
            continue;
        }

        let Ok(text) = read_note(root, path) else {
            continue;
        };

        if !tokens.tags.is_empty() {
            let note_tags: Vec<String> = extract_note_tags(&text)
                .into_iter()
                .map(|t| t.to_lowercase())
                .collect();
            let all_tags_match = tokens.tags.iter().all(|q_tag| {
                let q = q_tag.to_lowercase();
                note_tags
                    .iter()
                    .any(|t| t == &q || t.starts_with(&format!("{q}/")))
            });
            if !all_tags_match {
                continue;
            }
        }

        let lower_name = name.to_lowercase();
        let lower_text = text.to_lowercase();
        let all_words_present = tokens
            .words
            .iter()
            .all(|w| lower_name.contains(w) || lower_text.contains(w));
        if !all_words_present {
            continue;
        }

        let name_matched = !tokens.words.is_empty()
            && tokens.words.iter().all(|w| lower_name.contains(w));

        let mut lines = Vec::new();
        let mut total_line_matches = 0usize;
        if !tokens.words.is_empty() {
            for (idx, line) in text.lines().enumerate() {
                let lower_line = line.to_lowercase();
                if tokens.words.iter().any(|w| lower_line.contains(w)) {
                    total_line_matches += 1;
                    if lines.len() < 10 {
                        lines.push(SearchLineMatch {
                            line: idx + 1,
                            text: trim_snippet(line.trim(), 100),
                        });
                    }
                }
            }
        }

        results.push(SearchResult {
            path: path.clone(),
            name: name.clone(),
            name_matched,
            lines,
            total_line_matches,
        });
    }

    results.sort_by(|a, b| {
        b.name_matched
            .cmp(&a.name_matched)
            .then_with(|| a.path.to_lowercase().cmp(&b.path.to_lowercase()))
    });
    results.truncate(300);
    results
}

#[derive(Default)]
struct SearchTokens {
    words: Vec<String>,
    tags: Vec<String>,
    paths: Vec<String>,
}

fn parse_search_tokens(query: &str) -> SearchTokens {
    let mut out = SearchTokens::default();
    let chars: Vec<char> = query.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }
        if i >= chars.len() {
            break;
        }
        let mut token = String::new();
        let mut quoted = false;
        while i < chars.len() && (quoted || !chars[i].is_whitespace()) {
            if chars[i] == '"' {
                quoted = !quoted;
                i += 1;
                continue;
            }
            token.push(chars[i]);
            i += 1;
        }
        let lower = token.to_lowercase();
        if let Some(tag) = lower.strip_prefix("tag:") {
            let clean = tag.trim_start_matches('#');
            if !clean.is_empty() {
                out.tags.push(clean.to_string());
            }
        } else if let Some(path) = lower.strip_prefix("path:") {
            if !path.is_empty() {
                out.paths.push(path.to_string());
            }
        } else if !lower.is_empty() {
            out.words.push(lower);
        }
    }
    out
}

pub fn quick_switcher_rank(
    notes: &[(String, String)],
    recent_paths: &[String],
    query: &str,
) -> Vec<QuickSwitcherEntry> {
    let recent_rank: HashMap<&str, usize> = recent_paths
        .iter()
        .enumerate()
        .map(|(i, p)| (p.as_str(), i))
        .collect();
    let q = query.trim().to_lowercase();

    let mut entries = Vec::new();
    for (name, path) in notes {
        let folder = folder_of(path).to_string();
        let score = if q.is_empty() {
            if let Some(&r) = recent_rank.get(path.as_str()) {
                10_000 - r as i64
            } else {
                0
            }
        } else {
            let Some(base_score) = fuzzy_score(name, path, &q) else {
                continue;
            };
            let recency_bonus = recent_rank
                .get(path.as_str())
                .map(|&r| (100usize.saturating_sub(r)) as i64)
                .unwrap_or(0);
            base_score * 1000 + recency_bonus
        };

        entries.push(QuickSwitcherEntry {
            name: name.clone(),
            path: path.clone(),
            folder,
            score,
        });
    }

    entries.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    entries
}

fn fuzzy_score(name: &str, path: &str, query_lower: &str) -> Option<i64> {
    let lower_name = name.to_lowercase();
    let lower_path = path.to_lowercase();
    let q_clean = query_lower.strip_suffix(".md").unwrap_or(query_lower);

    if lower_name == q_clean {
        return Some(100);
    }
    if lower_name.starts_with(q_clean) {
        return Some(85);
    }
    if lower_name
        .split(|c: char| !c.is_alphanumeric())
        .any(|word| word.starts_with(q_clean))
    {
        return Some(70);
    }
    if lower_name.contains(q_clean) {
        return Some(55);
    }
    if lower_path.contains(query_lower) {
        return Some(40);
    }
    if is_subsequence(&lower_name, q_clean) {
        return Some(25);
    }
    if is_subsequence(&lower_path, query_lower) {
        return Some(10);
    }
    None
}

fn is_subsequence(haystack: &str, needle: &str) -> bool {
    let mut n = needle.chars();
    let Some(mut cur) = n.next() else {
        return true;
    };
    for h in haystack.chars() {
        if h == cur {
            if let Some(next) = n.next() {
                cur = next;
            } else {
                return true;
            }
        }
    }
    false
}

/// Converts Obsidian-flavored Markdown (`[[wikilink]]`, `![[embed]]`, `==highlight==`, `%%comment%%`)
/// into standard Markdown for `TextView::markdown` Reader mode.
pub fn preprocess_markdown_for_reader(
    text: &str,
    source_path: Option<&str>,
    index: &VaultIndex,
) -> String {
    let mut out = String::with_capacity(text.len() + 64);
    let fm_end = frontmatter_end_line(text).unwrap_or(0);
    let mut fence: Option<(char, usize)> = None;

    for (idx, line) in text.split('\n').enumerate() {
        if idx > 0 {
            out.push('\n');
        }
        let line_no = idx + 1;
        if line_no <= fm_end {
            out.push_str(line);
            continue;
        }
        let trimmed = line.trim_start();
        if let Some((ch, len)) = fence {
            if is_closing_fence(trimmed, ch, len) {
                fence = None;
            }
            out.push_str(line);
            continue;
        }
        if let Some((ch, len)) = opening_fence(trimmed) {
            fence = Some((ch, len));
            out.push_str(line);
            continue;
        }

        // Convert Obsidian callout line `> [!type] Title` into a styled blockquote header.
        if let Some(callout) = rewrite_callout_line(line) {
            out.push_str(&callout);
            continue;
        }

        out.push_str(&rewrite_inline_obsidian(line, source_path, index));
    }
    out
}

fn rewrite_callout_line(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    let after_gt = trimmed.strip_prefix('>')?.trim_start();
    let after_open = after_gt.strip_prefix("[!")?;
    let close = after_open.find(']')?;
    let callout_type = after_open[..close].trim();
    if callout_type.is_empty() {
        return None;
    }
    let rest = after_open[close + 1..].trim();
    let title = if rest.is_empty() {
        let mut chars = callout_type.chars();
        match chars.next() {
            Some(first) => format!("{}{}", first.to_uppercase(), chars.as_str().to_lowercase()),
            None => "Note".to_string(),
        }
    } else {
        rest.to_string()
    };
    Some(format!("> **[{title}]**"))
}

fn rewrite_inline_obsidian(line: &str, source_path: Option<&str>, index: &VaultIndex) -> String {
    let mut out = String::with_capacity(line.len());
    let bytes = line.as_bytes();
    let mut in_code = false;
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 1 < bytes.len() {
            out.push('\\');
            let next_ch = line[i + 1..].chars().next().unwrap();
            out.push(next_ch);
            i += 1 + next_ch.len_utf8();
            continue;
        }
        if bytes[i] == b'`' {
            in_code = !in_code;
            out.push('`');
            i += 1;
            continue;
        }
        if !in_code {
            // %%comment%% -> stripped in Reader mode
            if i + 1 < bytes.len() && bytes[i] == b'%' && bytes[i + 1] == b'%' {
                if let Some(rel_end) = line[i + 2..].find("%%") {
                    i += 2 + rel_end + 2;
                    continue;
                }
            }
            // ![[embed]]
            if i + 2 < bytes.len() && bytes[i] == b'!' && bytes[i + 1] == b'[' && bytes[i + 2] == b'[' {
                if let Some(rel_end) = line[i + 3..].find("]]") {
                    let inner = &line[i + 3..i + 3 + rel_end];
                    let link = Wikilink::parse(inner);
                    let target = &link.target;
                    let is_pdf = target.to_ascii_lowercase().ends_with(".pdf")
                        || matches!(
                            index.resolve(target, source_path),
                            Some(ResolvedItem::Attachment {
                                attachment_kind: crate::core::vault::AttachmentKind::Pdf,
                                ..
                            })
                        );
                    if is_pdf {
                        let label = link.display_text();
                        out.push_str(&format!("[PDF: {label}](noteki-link://{target})"));
                    } else {
                        let alt = link.alias.clone().unwrap_or_else(|| target.clone());
                        out.push_str(&format!("![{alt}](noteki-embed://{target})"));
                    }
                    i += 3 + rel_end + 2;
                    continue;
                }
            }
            // [[wikilink]]
            if i + 1 < bytes.len() && bytes[i] == b'[' && bytes[i + 1] == b'[' {
                if let Some(rel_end) = line[i + 2..].find("]]") {
                    let inner = &line[i + 2..i + 2 + rel_end];
                    let link = Wikilink::parse(inner);
                    let display = link.display_text();
                    let dest = if link.target.is_empty() {
                        format!("#{}", link.heading.as_deref().unwrap_or(""))
                    } else {
                        link.target.clone()
                    };
                    out.push_str(&format!("[{display}](noteki-link://{dest})"));
                    i += 2 + rel_end + 2;
                    continue;
                }
            }
            // ==highlight== -> **highlight** in reader fallback
            if i + 1 < bytes.len() && bytes[i] == b'=' && bytes[i + 1] == b'=' {
                if let Some(rel_end) = line[i + 2..].find("==") {
                    let inner = &line[i + 2..i + 2 + rel_end];
                    if !inner.is_empty() {
                        out.push_str(&format!("**{inner}**"));
                        i += 2 + rel_end + 2;
                        continue;
                    }
                }
            }
        }
        let ch = line[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

/// Computes live-preview text decorations for `EditorState`.
/// Marks (`**`, `[[`, `# `, etc.) are dimmed when the cursor is not on that line.
pub fn compute_markdown_spans(
    text: &str,
    cursor_line_zero_based: usize,
    source_path: Option<&str>,
    index: &VaultIndex,
) -> Vec<MdSpan> {
    let mut spans = Vec::new();
    let fm_end = frontmatter_end_line(text).unwrap_or(0);
    let mut fence: Option<(char, usize)> = None;
    let mut offset = 0usize;

    for (line_idx, raw_line) in text.split('\n').enumerate() {
        let line_no = line_idx + 1;
        let line_start = offset;
        let line_end = line_start + raw_line.len();
        offset = line_end + 1;

        let is_cursor_line = line_idx == cursor_line_zero_based;

        if line_no <= fm_end {
            if line_no == 1 || line_no == fm_end {
                spans.push(MdSpan {
                    range: line_start..line_end,
                    kind: MdSpanKind::FrontmatterDelimiter,
                });
            } else if let Some(colon) = raw_line.find(':') {
                spans.push(MdSpan {
                    range: line_start..line_start + colon + 1,
                    kind: MdSpanKind::FrontmatterKey,
                });
            }
            continue;
        }

        let trimmed = raw_line.trim_start();
        let leading = raw_line.len() - trimmed.len();

        if let Some((ch, len)) = fence {
            spans.push(MdSpan {
                range: line_start..line_end,
                kind: MdSpanKind::CodeBlock,
            });
            if is_closing_fence(trimmed, ch, len) {
                fence = None;
            }
            continue;
        }
        if let Some((ch, len)) = opening_fence(trimmed) {
            fence = Some((ch, len));
            spans.push(MdSpan {
                range: line_start..line_end,
                kind: MdSpanKind::CodeBlock,
            });
            continue;
        }

        // Heading
        if let Some((level, _)) = parse_atx_heading(trimmed) {
            let hashes = level as usize;
            let marker_end = (line_start + leading + hashes + 1).min(line_end);
            if !is_cursor_line {
                spans.push(MdSpan {
                    range: line_start + leading..marker_end,
                    kind: MdSpanKind::SyntaxMarker,
                });
            }
            if marker_end < line_end {
                spans.push(MdSpan {
                    range: marker_end..line_end,
                    kind: MdSpanKind::Heading(level),
                });
            }
        } else if trimmed.starts_with('>') {
            let after_gt = trimmed[1..].trim_start();
            if after_gt.starts_with("[!") {
                spans.push(MdSpan {
                    range: line_start + leading..line_end,
                    kind: MdSpanKind::CalloutHeader,
                });
            } else {
                spans.push(MdSpan {
                    range: line_start + leading..line_end,
                    kind: MdSpanKind::Quote,
                });
            }
        } else if is_done_task(trimmed) {
            spans.push(MdSpan {
                range: line_start + leading..line_end,
                kind: MdSpanKind::TaskDone,
            });
        }

        collect_inline_spans(
            raw_line,
            line_start,
            is_cursor_line,
            source_path,
            index,
            &mut spans,
        );
    }

    spans
}

fn is_done_task(trimmed: &str) -> bool {
    let rest = trimmed
        .strip_prefix("- ")
        .or_else(|| trimmed.strip_prefix("* "))
        .or_else(|| trimmed.strip_prefix("+ "));
    let Some(rest) = rest else {
        return false;
    };
    rest.starts_with("[x] ") || rest.starts_with("[X] ")
}

fn collect_inline_spans(
    line: &str,
    line_start: usize,
    is_cursor_line: bool,
    source_path: Option<&str>,
    index: &VaultIndex,
    out: &mut Vec<MdSpan>,
) {
    let bytes = line.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 1 < bytes.len() {
            i += 2;
            continue;
        }
        // Inline code `...`
        if bytes[i] == b'`' {
            if let Some(rel_end) = line[i + 1..].find('`') {
                let end = i + 1 + rel_end + 1;
                if !is_cursor_line {
                    out.push(MdSpan {
                        range: line_start + i..line_start + i + 1,
                        kind: MdSpanKind::SyntaxMarker,
                    });
                    out.push(MdSpan {
                        range: line_start + end - 1..line_start + end,
                        kind: MdSpanKind::SyntaxMarker,
                    });
                }
                out.push(MdSpan {
                    range: line_start + i + 1..line_start + end - 1,
                    kind: MdSpanKind::InlineCode,
                });
                i = end;
                continue;
            }
        }
        // %%comment%%
        if i + 1 < bytes.len() && bytes[i] == b'%' && bytes[i + 1] == b'%' {
            if let Some(rel_end) = line[i + 2..].find("%%") {
                let end = i + 2 + rel_end + 2;
                out.push(MdSpan {
                    range: line_start + i..line_start + end,
                    kind: MdSpanKind::Comment,
                });
                i = end;
                continue;
            }
        }
        // [[wikilink]] or ![[embed]]
        let is_embed = i + 2 < bytes.len() && bytes[i] == b'!' && bytes[i + 1] == b'[' && bytes[i + 2] == b'[';
        let is_wiki = i + 1 < bytes.len() && bytes[i] == b'[' && bytes[i + 1] == b'[';
        if is_embed || is_wiki {
            let prefix_len = if is_embed { 3 } else { 2 };
            if let Some(rel_end) = line[i + prefix_len..].find("]]") {
                let inner = &line[i + prefix_len..i + prefix_len + rel_end];
                let end = i + prefix_len + rel_end + 2;
                let parsed = Wikilink::parse(inner);
                let resolved = if parsed.target.is_empty() {
                    true
                } else {
                    index.resolve(&parsed.target, source_path).is_some()
                };
                if !is_cursor_line {
                    out.push(MdSpan {
                        range: line_start + i..line_start + i + prefix_len,
                        kind: MdSpanKind::SyntaxMarker,
                    });
                    out.push(MdSpan {
                        range: line_start + end - 2..line_start + end,
                        kind: MdSpanKind::SyntaxMarker,
                    });
                }
                out.push(MdSpan {
                    range: line_start + i + prefix_len..line_start + end - 2,
                    kind: if resolved {
                        MdSpanKind::WikilinkResolved
                    } else {
                        MdSpanKind::WikilinkUnresolved
                    },
                });
                i = end;
                continue;
            }
        }
        // **bold**
        if i + 1 < bytes.len() && bytes[i] == b'*' && bytes[i + 1] == b'*' {
            if let Some(rel_end) = line[i + 2..].find("**") {
                let end = i + 2 + rel_end + 2;
                if !is_cursor_line {
                    out.push(MdSpan {
                        range: line_start + i..line_start + i + 2,
                        kind: MdSpanKind::SyntaxMarker,
                    });
                    out.push(MdSpan {
                        range: line_start + end - 2..line_start + end,
                        kind: MdSpanKind::SyntaxMarker,
                    });
                }
                out.push(MdSpan {
                    range: line_start + i + 2..line_start + end - 2,
                    kind: MdSpanKind::Bold,
                });
                i = end;
                continue;
            }
        }
        // ==highlight==
        if i + 1 < bytes.len() && bytes[i] == b'=' && bytes[i + 1] == b'=' {
            if let Some(rel_end) = line[i + 2..].find("==") {
                let end = i + 2 + rel_end + 2;
                if !is_cursor_line {
                    out.push(MdSpan {
                        range: line_start + i..line_start + i + 2,
                        kind: MdSpanKind::SyntaxMarker,
                    });
                    out.push(MdSpan {
                        range: line_start + end - 2..line_start + end,
                        kind: MdSpanKind::SyntaxMarker,
                    });
                }
                out.push(MdSpan {
                    range: line_start + i + 2..line_start + end - 2,
                    kind: MdSpanKind::Highlight,
                });
                i = end;
                continue;
            }
        }
        // ~~strikethrough~~
        if i + 1 < bytes.len() && bytes[i] == b'~' && bytes[i + 1] == b'~' {
            if let Some(rel_end) = line[i + 2..].find("~~") {
                let end = i + 2 + rel_end + 2;
                if !is_cursor_line {
                    out.push(MdSpan {
                        range: line_start + i..line_start + i + 2,
                        kind: MdSpanKind::SyntaxMarker,
                    });
                    out.push(MdSpan {
                        range: line_start + end - 2..line_start + end,
                        kind: MdSpanKind::SyntaxMarker,
                    });
                }
                out.push(MdSpan {
                    range: line_start + i + 2..line_start + end - 2,
                    kind: MdSpanKind::Strikethrough,
                });
                i = end;
                continue;
            }
        }
        // [text](url)
        if bytes[i] == b'[' {
            if let Some(close_br) = line[i + 1..].find(']') {
                let close_br_abs = i + 1 + close_br;
                if close_br_abs + 1 < bytes.len() && bytes[close_br_abs + 1] == b'(' {
                    if let Some(close_paren) = line[close_br_abs + 2..].find(')') {
                        let end = close_br_abs + 2 + close_paren + 1;
                        out.push(MdSpan {
                            range: line_start + i + 1..line_start + close_br_abs,
                            kind: MdSpanKind::MarkdownLink,
                        });
                        if !is_cursor_line {
                            out.push(MdSpan {
                                range: line_start + close_br_abs + 1..line_start + end,
                                kind: MdSpanKind::SyntaxMarker,
                            });
                        }
                        i = end;
                        continue;
                    }
                }
            }
        }
        // *italic* (single *)
        if bytes[i] == b'*' && (i + 1 >= bytes.len() || bytes[i + 1] != b'*') {
            if let Some(rel_end) = line[i + 1..].find('*') {
                if rel_end > 0 {
                    let end = i + 1 + rel_end + 1;
                    out.push(MdSpan {
                        range: line_start + i + 1..line_start + end - 1,
                        kind: MdSpanKind::Italic,
                    });
                    i = end;
                    continue;
                }
            }
        }
        // #tag
        if bytes[i] == b'#' && (i == 0 || bytes[i - 1].is_ascii_whitespace()) {
            let rest = &line[i + 1..];
            let tag_len = rest
                .char_indices()
                .take_while(|(_, c)| c.is_alphanumeric() || *c == '_' || *c == '-' || *c == '/')
                .last()
                .map(|(idx, c)| idx + c.len_utf8())
                .unwrap_or(0);
            if tag_len > 0 {
                let candidate = rest[..tag_len].trim_end_matches('/');
                if is_valid_tag(candidate) {
                    out.push(MdSpan {
                        range: line_start + i..line_start + i + 1 + candidate.len(),
                        kind: MdSpanKind::Tag,
                    });
                    i += 1 + tag_len;
                    continue;
                }
            }
        }

        let ch_len = line[i..].chars().next().map(|c| c.len_utf8()).unwrap_or(1);
        i += ch_len;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_outline_skipping_frontmatter_and_code() {
        let md = "---\ntitle: Test\n---\n# Intro\n```\n# Not a heading\n```\n## Details with [[Link|Alias]]\nSetext\n===\n";
        let outline = extract_outline(md);
        assert_eq!(outline.len(), 3);
        assert_eq!(outline[0], HeadingItem { level: 1, text: "Intro".into(), line: 4 });
        assert_eq!(outline[1], HeadingItem { level: 2, text: "Details with Alias".into(), line: 8 });
        assert_eq!(outline[2], HeadingItem { level: 1, text: "Setext".into(), line: 9 });
    }

    #[test]
    fn extracts_inline_and_frontmatter_tags() {
        let md = "---\ntags:\n  - client/axo\n---\nMeeting with #Client/Axo and `#not-a-tag` and #work.\n";
        let tags = extract_note_tags(md);
        assert_eq!(tags, vec!["client/axo", "Client/Axo", "work"]);
    }
}
