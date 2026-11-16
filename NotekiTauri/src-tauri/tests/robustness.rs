//! Property-style robustness checks over generated Markdown, ported from the
//! GPUI prototype: every function that sees a note's text must never panic
//! (it runs on the watcher thread for every changed file), and the ranges it
//! returns must be valid `str` slices.
//!
//! Deterministic (fixed seeds); raise `NOTEKI_FUZZ_CASES` for a longer run.

use noteki_tauri_lib::document::NoteDoc;
use noteki_tauri_lib::index::{extract_note_tags, parse_search_tokens};
use noteki_tauri_lib::markdown::{inline_tokens, lines, wikilinks};
use noteki_tauri_lib::merge::merge_three_way;
use noteki_tauri_lib::wikilink::Wikilink;

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        // xorshift64*
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
    fn pick<'a>(&mut self, items: &[&'a str]) -> &'a str {
        items[self.below(items.len())]
    }
}

/// Markdown-ish fragments, with the characters that broke the first prototype
/// (a backslash before a multi-byte letter) and every construct the scanner knows.
const PIECES: &[&str] = &[
    "a",
    "word",
    " ",
    " ",
    "\n",
    "\n",
    "\r\n",
    "\t",
    "è",
    "perché",
    "日本",
    "🚀",
    "é\u{301}",
    "\\",
    "\\è",
    "\\*",
    "#",
    "# ",
    "## ",
    "#tag",
    "#tag/sub",
    "#perché",
    "#1",
    "*",
    "**",
    "***",
    "_",
    "__",
    "~",
    "~~",
    "=",
    "==",
    "`",
    "``",
    "```",
    "~~~",
    "```rust\n",
    "[",
    "]",
    "[[",
    "]]",
    "![[",
    "|",
    "\\|",
    "(",
    ")",
    "[x](y)",
    "[[Note]]",
    "[[Note|Alias]]",
    "[[Note#Head]]",
    "[[#Head]]",
    "![[img.png|120]]",
    "%%",
    "%%c%%",
    ">",
    "> ",
    "> [!note] T",
    "- ",
    "* ",
    "+ ",
    "1. ",
    "- [ ] ",
    "- [x] ",
    "[ ]",
    "[x]",
    "---",
    "---\n",
    "===",
    "$",
    "$$",
    "<",
    ">",
    "&",
    "%20",
    "%",
    ":",
];

fn gen_text(rng: &mut Rng, max_pieces: usize) -> String {
    let n = rng.below(max_pieces + 1);
    (0..n).map(|_| rng.pick(PIECES)).collect()
}

fn cases() -> usize {
    std::env::var("NOTEKI_FUZZ_CASES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(3000)
}

fn assert_slice(text: &str, r: &std::ops::Range<usize>, what: &str) {
    assert!(
        r.start <= r.end && r.end <= text.len(),
        "{what}: range {r:?} outside text of len {}: {text:?}",
        text.len()
    );
    assert!(
        text.is_char_boundary(r.start) && text.is_char_boundary(r.end),
        "{what}: range {r:?} not on char boundaries: {text:?}"
    );
}

#[test]
fn inline_tokens_nest_inside_their_line() {
    let mut rng = Rng(7);
    for _ in 0..cases() {
        let text = gen_text(&mut rng, 30);
        for line in lines(&text) {
            for tok in inline_tokens(line.text) {
                assert_slice(line.text, &tok.range, "token");
                assert_slice(line.text, &tok.inner, "token inner");
                assert!(
                    tok.range.start <= tok.inner.start && tok.inner.end <= tok.range.end,
                    "{tok:?} in {:?}",
                    line.text
                );
            }
        }
    }
}

#[test]
fn lines_cover_the_text() {
    let mut rng = Rng(11);
    for _ in 0..cases() {
        let text = gen_text(&mut rng, 30);
        let ls = lines(&text);
        assert_eq!(ls.len(), text.split('\n').count());
        for l in &ls {
            assert_slice(&text, &l.range(), "line");
            assert_eq!(&text[l.range()], l.text);
        }
    }
}

#[test]
fn wikilink_lookup_at_every_offset() {
    let mut rng = Rng(5);
    for _ in 0..cases() / 3 {
        let text = gen_text(&mut rng, 25);
        for (_, tok) in wikilinks(&text) {
            assert_slice(&text, &tok.range, "wikilink");
        }
    }
}

#[test]
fn tags_and_links_never_panic() {
    let mut rng = Rng(99);
    for _ in 0..cases() {
        let text = gen_text(&mut rng, 40);
        for t in extract_note_tags(&text) {
            assert!(!t.is_empty() && !t.starts_with('#'), "bad tag {t:?}");
        }
        let _ = parse_search_tokens(&text);
        let _ = Wikilink::parse(&text).display_text();
    }
}

#[test]
fn merge_laws() {
    let mut rng = Rng(21);
    let line_pieces = &["a", "b", "c", "", "d e", "è"];
    let gen = |rng: &mut Rng| -> String {
        let n = rng.below(6);
        (0..n)
            .map(|_| rng.pick(line_pieces))
            .collect::<Vec<_>>()
            .join("\n")
    };
    for _ in 0..cases() {
        let (base, mine, theirs) = (gen(&mut rng), gen(&mut rng), gen(&mut rng));
        // One side unchanged: the result is the other side.
        assert_eq!(
            merge_three_way(&base, &base, &theirs).as_deref(),
            Some(theirs.as_str())
        );
        assert_eq!(
            merge_three_way(&base, &mine, &base).as_deref(),
            Some(mine.as_str())
        );
        // Both made the same change: no conflict.
        assert_eq!(
            merge_three_way(&base, &mine, &mine).as_deref(),
            Some(mine.as_str())
        );
        let _ = merge_three_way(&base, &mine, &theirs);
    }
}

#[test]
fn note_doc_invariants_under_random_events() {
    let mut rng = Rng(1234);
    let texts = ["", "a", "a\nb", "a\nB", "x\nb", "a\nb\nc"];
    for _ in 0..cases() {
        let mut doc = NoteDoc::new(rng.pick(&texts));
        let mut disk: Option<String> = Some(doc.text().to_string());
        for _ in 0..12 {
            match rng.below(7) {
                0 | 1 => {
                    doc.edit(rng.pick(&texts));
                }
                2 => {
                    if doc.should_save() {
                        disk = Some(doc.text().to_string());
                        let t = doc.text().to_string();
                        doc.mark_saved(&t);
                    }
                }
                3 => {
                    disk = if rng.below(4) == 0 {
                        None
                    } else {
                        Some(rng.pick(&texts).to_string())
                    };
                    doc.on_disk_changed(disk.as_deref());
                }
                4 => doc.keep_mine(),
                5 => {
                    doc.take_theirs();
                }
                _ => doc.save_again(),
            }
            // Never write over a conflict or recreate a deleted file on our own.
            if doc.has_conflict() || doc.is_deleted() {
                assert!(!doc.should_save());
            }
            // Nothing unsaved after the disk was found equal to the editor.
            if disk.as_deref() == Some(doc.text()) && !doc.has_conflict() && !doc.is_deleted() {
                let mut probe = doc.clone();
                probe.on_disk_changed(disk.as_deref());
                assert!(!probe.is_dirty());
            }
        }
    }
}
