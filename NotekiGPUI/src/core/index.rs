//! Vault-wide indexes built from a [`Corpus`] of note texts: outline, tags,
//! backlinks, search and quick-switcher ranking. Nothing here touches the disk
//! except [`Corpus::load`], so typing never triggers vault-wide I/O.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

use crate::core::frontmatter::extract_frontmatter_tags;
use crate::core::markdown::{self, is_setext_candidate, parse_atx_heading, BlockKind};
use crate::core::vault::read_note;
use crate::core::wikilink::{folder_of, ResolvedItem, VaultIndex, Wikilink};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeadingItem {
    pub level: u8,
    pub text: String,
    /// 1-based line number.
    pub line: usize,
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
    /// 1-based line number.
    pub line: usize,
    pub snippet: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchLineMatch {
    /// 1-based line number.
    pub line: usize,
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

// ---------------------------------------------------------------------------
// Corpus
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct NoteText {
    pub name: String,
    pub path: String,
    pub text: String,
}

/// The text of every note in the vault, kept in memory between rescans.
#[derive(Debug, Clone, Default)]
pub struct Corpus {
    notes: Vec<NoteText>,
    by_path: HashMap<String, usize>,
}

impl Corpus {
    /// Reads every listed `(name, path)` note; unreadable files are skipped.
    pub fn load(root: &Path, notes: &[(String, String)]) -> Self {
        Self::from_notes(notes.iter().filter_map(|(name, path)| {
            read_note(root, path).ok().map(|text| NoteText {
                name: name.clone(),
                path: path.clone(),
                text,
            })
        }))
    }

    /// Re-reads only the notes in `changed` (or new to the list); the others
    /// keep the text already in `self`.
    pub fn refresh(
        &self,
        root: &Path,
        notes: &[(String, String)],
        changed: &HashSet<String>,
    ) -> Self {
        Self::from_notes(notes.iter().filter_map(|(name, path)| {
            let text = match self.get(path) {
                Some(text) if !changed.contains(path) => text.to_string(),
                _ => read_note(root, path).ok()?,
            };
            Some(NoteText {
                name: name.clone(),
                path: path.clone(),
                text,
            })
        }))
    }

    pub fn from_notes(notes: impl IntoIterator<Item = NoteText>) -> Self {
        let notes: Vec<NoteText> = notes.into_iter().collect();
        let by_path = notes
            .iter()
            .enumerate()
            .map(|(i, n)| (n.path.clone(), i))
            .collect();
        Self { notes, by_path }
    }

    pub fn len(&self) -> usize {
        self.notes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.notes.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &NoteText> {
        self.notes.iter()
    }

    pub fn get(&self, path: &str) -> Option<&str> {
        self.by_path.get(path).map(|&i| self.notes[i].text.as_str())
    }

    /// Updates the text of a note already in the corpus (after a save).
    pub fn set(&mut self, path: &str, text: &str) {
        if let Some(&i) = self.by_path.get(path) {
            if self.notes[i].text != text {
                self.notes[i].text = text.to_string();
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Outline
// ---------------------------------------------------------------------------

pub fn extract_outline(text: &str) -> Vec<HeadingItem> {
    let lines = markdown::lines(text);
    let mut headings = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if line.kind != BlockKind::Text {
            continue;
        }
        let trimmed = line.text.trim_start();
        if line.text.len() - trimmed.len() > 3 {
            continue; // indented code
        }
        if let Some((level, raw)) = parse_atx_heading(trimmed) {
            let text = strip_inline_markdown(raw);
            if !text.is_empty() {
                headings.push(HeadingItem {
                    level,
                    text,
                    line: line.index + 1,
                });
            }
            continue;
        }
        let Some(next) = lines.get(i + 1).filter(|n| n.kind == BlockKind::Text) else {
            continue;
        };
        let underline = next.text.trim();
        let level = if underline.len() >= 3 && underline.bytes().all(|b| b == b'=') {
            1
        } else if underline.len() >= 3 && underline.bytes().all(|b| b == b'-') {
            2
        } else {
            continue;
        };
        if is_setext_candidate(trimmed) {
            let text = strip_inline_markdown(trimmed);
            if !text.is_empty() {
                headings.push(HeadingItem {
                    level,
                    text,
                    line: line.index + 1,
                });
            }
        }
    }
    headings
}

/// Heading text as read: wikilinks show their display text, link labels stay,
/// emphasis and code marks go.
pub fn strip_inline_markdown(input: &str) -> String {
    use markdown::InlineKind as K;
    let mut out = String::with_capacity(input.len());
    let mut at = 0;
    for tok in markdown::inline_tokens(input) {
        if tok.range.start < at {
            continue;
        }
        out.push_str(&input[at..tok.range.start]);
        match tok.kind {
            K::Wikilink { .. } => {
                out.push_str(&Wikilink::parse(&input[tok.inner.clone()]).display_text())
            }
            K::Comment => {}
            K::Tag | K::Math => out.push_str(&input[tok.range.clone()]),
            K::Bold | K::Italic | K::Strikethrough | K::Highlight | K::MarkdownLink => {
                out.push_str(&strip_inline_markdown(&input[tok.inner.clone()]))
            }
            K::Code | K::Escape => out.push_str(&input[tok.inner.clone()]),
        }
        at = tok.range.end;
    }
    out.push_str(&input[at..]);
    out.trim().to_string()
}

// ---------------------------------------------------------------------------
// Tags
// ---------------------------------------------------------------------------

/// Frontmatter `tags` followed by inline `#tags`, with their original spelling.
pub fn extract_note_tags(text: &str) -> Vec<String> {
    let mut tags = extract_frontmatter_tags(text);
    tags.extend(markdown::inline_tags(text));
    tags
}

/// Nested tag tree (`client/axo` under `client`), case-insensitive, each node
/// labelled with the spelling used most often.
pub fn build_tag_tree(corpus: &Corpus) -> Vec<TagNode> {
    let mut spelling_counts: HashMap<String, HashMap<String, usize>> = HashMap::new();
    let mut tag_notes: HashMap<String, HashSet<String>> = HashMap::new();

    for note in corpus.iter() {
        let mut seen_in_note = HashSet::new();
        for raw_tag in extract_note_tags(&note.text) {
            let lower = raw_tag.to_lowercase();
            *spelling_counts
                .entry(lower.clone())
                .or_default()
                .entry(raw_tag)
                .or_insert(0) += 1;
            let mut acc = String::new();
            for part in lower.split('/') {
                if !acc.is_empty() {
                    acc.push('/');
                }
                acc.push_str(part);
                if seen_in_note.insert(acc.clone()) {
                    tag_notes
                        .entry(acc.clone())
                        .or_default()
                        .insert(note.path.clone());
                }
            }
        }
    }

    let preferred_spelling = |full_lower: &str, segment_lower: &str| -> String {
        spelling_counts
            .get(full_lower)
            .and_then(|counts| {
                // Most used; on a tie prefer one with capitals, then alphabetical.
                counts.iter().max_by(|(a, ac), (b, bc)| {
                    ac.cmp(bc)
                        .then_with(|| (**a != a.to_lowercase()).cmp(&(**b != b.to_lowercase())))
                        .then_with(|| b.cmp(a))
                })
            })
            .and_then(|(best, _)| best.rsplit('/').next().map(str::to_string))
            .unwrap_or_else(|| segment_lower.to_string())
    };

    #[derive(Default)]
    struct Builder {
        children: BTreeMap<String, Builder>,
    }
    let mut root = Builder::default();
    for full_lower in tag_notes.keys() {
        let mut cur = &mut root;
        for seg in full_lower.split('/') {
            cur = cur.children.entry(seg.to_string()).or_default();
        }
    }

    fn finalize(
        prefix: &str,
        node: Builder,
        tag_notes: &HashMap<String, HashSet<String>>,
        spelling: &dyn Fn(&str, &str) -> String,
    ) -> Vec<TagNode> {
        let mut out: Vec<TagNode> = node
            .children
            .into_iter()
            .map(|(seg, child)| {
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
                TagNode {
                    count: note_paths.len(),
                    note_paths,
                    children: finalize(&full_lower, child, tag_notes, spelling),
                    label: spelling(&full_lower, &seg),
                    full_lower,
                }
            })
            .collect();
        out.sort_by_key(|n| n.label.to_lowercase());
        out
    }

    finalize("", root, &tag_notes, &preferred_spelling)
}

// ---------------------------------------------------------------------------
// Backlinks
// ---------------------------------------------------------------------------

/// Lines of other notes whose wikilinks (or embeds) resolve to `target_path`.
pub fn find_backlinks(
    corpus: &Corpus,
    target_path: &str,
    index: &VaultIndex,
) -> Vec<BacklinkMatch> {
    let mut results = Vec::new();
    for note in corpus.iter() {
        if note.path == target_path {
            continue;
        }
        let mut last_line = None;
        for (line_index, tok) in markdown::wikilinks(&note.text) {
            if last_line == Some(line_index) {
                continue;
            }
            let link = Wikilink::parse(&note.text[tok.inner.clone()]);
            if link.target.is_empty() {
                continue;
            }
            let hit = matches!(
                index.resolve(&link.target, Some(&note.path)),
                Some(ResolvedItem::Note { ref path, .. } | ResolvedItem::Attachment { ref path, .. }) if path == target_path
            );
            if hit {
                last_line = Some(line_index);
                let line_text = note.text.split('\n').nth(line_index).unwrap_or("");
                results.push(BacklinkMatch {
                    source_path: note.path.clone(),
                    source_name: note.name.clone(),
                    line: line_index + 1,
                    snippet: trim_snippet(line_text.trim(), 110),
                });
            }
        }
    }
    results.sort_by(|a, b| {
        a.source_name
            .to_lowercase()
            .cmp(&b.source_name.to_lowercase())
            .then_with(|| a.source_path.cmp(&b.source_path))
            .then_with(|| a.line.cmp(&b.line))
    });
    results
}

fn trim_snippet(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        let mut truncated: String = s.chars().take(max_chars.saturating_sub(1)).collect();
        truncated.push('…');
        truncated
    }
}

// ---------------------------------------------------------------------------
// Search
// ---------------------------------------------------------------------------

pub const MAX_SEARCH_RESULTS: usize = 300;
pub const MAX_LINES_PER_RESULT: usize = 10;

/// Case-insensitive search. Words (or `"quoted phrases"`) must all appear in the
/// note's name or text; `tag:x` matches `#x` and `#x/…`; `path:x` filters paths.
pub fn search_vault(corpus: &Corpus, query: &str) -> Vec<SearchResult> {
    let tokens = parse_search_tokens(query);
    if tokens.words.is_empty() && tokens.tags.is_empty() && tokens.paths.is_empty() {
        return Vec::new();
    }

    let mut results = Vec::new();
    for note in corpus.iter() {
        let lower_path = note.path.to_lowercase();
        if !tokens.paths.iter().all(|p| lower_path.contains(p)) {
            continue;
        }
        if !tokens.tags.is_empty() {
            let note_tags: Vec<String> = extract_note_tags(&note.text)
                .iter()
                .map(|t| t.to_lowercase())
                .collect();
            let all = tokens.tags.iter().all(|q| {
                let prefix = format!("{q}/");
                note_tags.iter().any(|t| t == q || t.starts_with(&prefix))
            });
            if !all {
                continue;
            }
        }
        let lower_name = note.name.to_lowercase();
        let lower_text = note.text.to_lowercase();
        if !tokens
            .words
            .iter()
            .all(|w| lower_name.contains(w) || lower_text.contains(w))
        {
            continue;
        }
        let name_matched =
            !tokens.words.is_empty() && tokens.words.iter().all(|w| lower_name.contains(w));

        let mut lines = Vec::new();
        let mut total_line_matches = 0usize;
        if !tokens.words.is_empty() {
            for (idx, line) in note.text.lines().enumerate() {
                let lower_line = line.to_lowercase();
                if tokens.words.iter().any(|w| lower_line.contains(w)) {
                    total_line_matches += 1;
                    if lines.len() < MAX_LINES_PER_RESULT {
                        lines.push(SearchLineMatch {
                            line: idx + 1,
                            text: trim_snippet(line.trim(), 100),
                        });
                    }
                }
            }
        }
        results.push(SearchResult {
            path: note.path.clone(),
            name: note.name.clone(),
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
    results.truncate(MAX_SEARCH_RESULTS);
    results
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct SearchTokens {
    pub words: Vec<String>,
    pub tags: Vec<String>,
    pub paths: Vec<String>,
}

pub fn parse_search_tokens(query: &str) -> SearchTokens {
    let mut out = SearchTokens::default();
    let mut chars = query.chars().peekable();
    loop {
        while chars.peek().is_some_and(|c| c.is_whitespace()) {
            chars.next();
        }
        if chars.peek().is_none() {
            break;
        }
        let mut token = String::new();
        let mut quoted = false;
        while let Some(&c) = chars.peek() {
            if !quoted && c.is_whitespace() {
                break;
            }
            chars.next();
            if c == '"' {
                quoted = !quoted;
            } else {
                token.push(c);
            }
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

// ---------------------------------------------------------------------------
// Quick switcher
// ---------------------------------------------------------------------------

/// Notes ranked for the quick switcher: recent ones first for an empty query,
/// otherwise by match quality with a recency bonus.
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
    let mut entries: Vec<QuickSwitcherEntry> = notes
        .iter()
        .filter_map(|(name, path)| {
            let recency = recent_rank.get(path.as_str()).copied();
            let score = if q.is_empty() {
                recency.map_or(0, |r| 10_000 - r as i64)
            } else {
                fuzzy_score(name, path, &q)? * 1000
                    + recency.map_or(0, |r| 100usize.saturating_sub(r) as i64)
            };
            Some(QuickSwitcherEntry {
                name: name.clone(),
                path: path.clone(),
                folder: folder_of(path).to_string(),
                score,
            })
        })
        .collect();
    entries.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            .then_with(|| a.path.cmp(&b.path))
    });
    entries
}

fn fuzzy_score(name: &str, path: &str, query_lower: &str) -> Option<i64> {
    let lower_name = name.to_lowercase();
    let lower_path = path.to_lowercase();
    let q = query_lower.strip_suffix(".md").unwrap_or(query_lower);
    if lower_name == q {
        Some(100)
    } else if lower_name.starts_with(q) {
        Some(85)
    } else if lower_name
        .split(|c: char| !c.is_alphanumeric())
        .any(|w| w.starts_with(q))
    {
        Some(70)
    } else if lower_name.contains(q) {
        Some(55)
    } else if lower_path.contains(query_lower) {
        Some(40)
    } else if is_subsequence(&lower_name, q) {
        Some(25)
    } else if is_subsequence(&lower_path, query_lower) {
        Some(10)
    } else {
        None
    }
}

fn is_subsequence(haystack: &str, needle: &str) -> bool {
    let mut h = haystack.chars();
    needle.chars().all(|n| h.any(|c| c == n))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn corpus(notes: &[(&str, &str)]) -> Corpus {
        Corpus::from_notes(notes.iter().map(|(path, text)| NoteText {
            name: crate::core::wikilink::note_name(path),
            path: path.to_string(),
            text: text.to_string(),
        }))
    }

    #[test]
    fn extracts_outline_skipping_frontmatter_and_code() {
        let md = "---\ntitle: Test\n---\n# Intro\n```\n# Not a heading\n```\n## Details with [[Link|Alias]]\nSetext\n===\n";
        let outline = extract_outline(md);
        assert_eq!(
            outline,
            vec![
                HeadingItem {
                    level: 1,
                    text: "Intro".into(),
                    line: 4
                },
                HeadingItem {
                    level: 2,
                    text: "Details with Alias".into(),
                    line: 8
                },
                HeadingItem {
                    level: 1,
                    text: "Setext".into(),
                    line: 9
                },
            ]
        );
    }

    #[test]
    fn frontmatter_dashes_are_not_setext() {
        // The closing `---` of the frontmatter must not turn the line above into a heading.
        assert!(extract_outline("---\ntitle: x\n---\nbody").is_empty());
        assert!(extract_outline("- a\n---").is_empty());
    }

    #[test]
    fn strips_inline_markdown_in_headings() {
        assert_eq!(
            strip_inline_markdown("**Bold** [[A/B|Alias]] `c` [l](u) %%x%%"),
            "Bold Alias c l"
        );
    }

    #[test]
    fn extracts_inline_and_frontmatter_tags() {
        let md = "---\ntags:\n  - client/axo\n---\nMeeting with #Client/Axo and `#not-a-tag` and #work.\n```\n#nope\n```";
        assert_eq!(
            extract_note_tags(md),
            vec!["client/axo", "Client/Axo", "work"]
        );
    }

    #[test]
    fn tag_tree_nests_and_counts_notes_once() {
        let c = corpus(&[("a.md", "#Client/Axo #client/axo"), ("b.md", "#client")]);
        let tree = build_tag_tree(&c);
        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0].full_lower, "client");
        assert_eq!(tree[0].count, 2);
        assert_eq!(tree[0].children[0].label, "Axo");
        assert_eq!(tree[0].children[0].note_paths, vec!["a.md"]);
    }

    #[test]
    fn backlinks_ignore_code_and_self() {
        let c = corpus(&[
            ("T.md", "[[T]] self"),
            (
                "A.md",
                "see [[T]] and [[T|again]]\n`[[T]]`\n```\n[[T]]\n```\n![[T]]",
            ),
            ("B.md", "nothing"),
        ]);
        let tree = vec![
            crate::core::vault::VaultItem::Note {
                name: "T".into(),
                path: "T.md".into(),
            },
            crate::core::vault::VaultItem::Note {
                name: "A".into(),
                path: "A.md".into(),
            },
        ];
        let idx = VaultIndex::from_tree(&tree);
        let bl = find_backlinks(&c, "T.md", &idx);
        assert_eq!(bl.iter().map(|b| b.line).collect::<Vec<_>>(), vec![1, 6]);
    }

    #[test]
    fn search_words_tags_paths_and_phrases() {
        let c = corpus(&[
            (
                "Work/Retro.md",
                "#work/team\nWent well: shipping\nTo improve: tests",
            ),
            ("Home.md", "shipping a parcel"),
        ]);
        let r = search_vault(&c, "shipping tag:work");
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].lines[0].line, 2);
        assert_eq!(search_vault(&c, "path:work").len(), 1);
        assert_eq!(search_vault(&c, "\"a parcel\"").len(), 1);
        assert!(search_vault(&c, "retro")[0].name_matched);
        assert!(search_vault(&c, "   ").is_empty());
        assert_eq!(
            parse_search_tokens("tag:#A \"x y\" path:J"),
            SearchTokens {
                words: vec!["x y".into()],
                tags: vec!["a".into()],
                paths: vec!["j".into()]
            }
        );
    }

    #[test]
    fn quick_switcher_prefers_exact_then_recent() {
        let notes = vec![
            ("Retro".to_string(), "Retro.md".to_string()),
            (
                "Retrospective".to_string(),
                "A/Retrospective.md".to_string(),
            ),
            ("Other".to_string(), "Other.md".to_string()),
        ];
        let r = quick_switcher_rank(&notes, &["Other.md".into()], "");
        assert_eq!(r[0].path, "Other.md");
        let r = quick_switcher_rank(&notes, &[], "retro");
        assert_eq!(
            r.iter().map(|e| e.path.as_str()).collect::<Vec<_>>(),
            vec!["Retro.md", "A/Retrospective.md"]
        );
        assert!(is_subsequence("retrospective", "rtsp"));
        assert!(!is_subsequence("abc", "ca"));
    }
}
