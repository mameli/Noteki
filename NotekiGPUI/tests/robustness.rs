//! Property-style robustness checks over generated Markdown.
//!
//! Every function that sees user text must never panic, and the ranges it
//! returns must be valid `str` slices: the editor applies them as decorations
//! on every keystroke, so one bad offset is a crash while typing.
//!
//! Deterministic (fixed seeds); raise `NOTEKI_FUZZ_CASES` for a longer run.

use noteki::core::document::NoteDoc;
use noteki::core::export::note_html;
use noteki::core::index::{
    extract_note_tags, extract_outline, parse_search_tokens, strip_inline_markdown,
};
use noteki::core::markdown::{
    compute_markdown_spans, inline_tokens, line_of_offset, lines, percent_decode, percent_encode,
    preprocess_for_html, wikilink_at, wikilinks,
};
use noteki::core::merge::merge_three_way;
use noteki::core::tabs::{OpenIn, Tabs};
use noteki::core::vault::VaultItem;
use noteki::core::wikilink::{VaultIndex, Wikilink};

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

fn index() -> VaultIndex {
    VaultIndex::from_tree(&[
        VaultItem::Note {
            name: "Note".into(),
            path: "Note.md".into(),
        },
        VaultItem::Note {
            name: "Other".into(),
            path: "Dir/Other.md".into(),
        },
        VaultItem::Attachment {
            name: "img.png".into(),
            path: "Media/img.png".into(),
            attachment_kind: noteki::core::vault::AttachmentKind::Image,
        },
    ])
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
fn live_preview_spans_are_valid_ranges() {
    let idx = index();
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    for _ in 0..cases() {
        let text = gen_text(&mut rng, 40);
        for cursor in [
            None,
            Some(0),
            Some(rng.below(text.len() + 2)),
            Some(usize::MAX),
        ] {
            for span in compute_markdown_spans(&text, cursor, Some("Dir/Here.md"), &idx) {
                assert_slice(&text, &span.range, "span");
                assert!(
                    span.range.start < span.range.end,
                    "empty span {span:?} in {text:?}"
                );
            }
        }
    }
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
fn exported_html_keeps_lines_and_never_panics() {
    let idx = index();
    let mut rng = Rng(42);
    for _ in 0..cases() {
        let text = gen_text(&mut rng, 40);
        let md = preprocess_for_html(&text, Some("Here.md"), &idx);
        assert_eq!(
            md.split('\n').count(),
            text.split('\n').count(),
            "line count changed:\n{text:?}\n{md:?}"
        );
        let _ = note_html(&text, "T", Some("Here.md"), &idx, |_| None);
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
        for offset in 0..=text.len() + 1 {
            let _ = wikilink_at(&text, offset);
            let _ = line_of_offset(&text, offset);
        }
    }
}

#[test]
fn outline_tags_and_headings_never_panic() {
    let mut rng = Rng(99);
    for _ in 0..cases() {
        let text = gen_text(&mut rng, 40);
        for h in extract_outline(&text) {
            assert!(h.line >= 1 && h.line <= text.split('\n').count());
            assert!((1..=6).contains(&h.level));
        }
        for t in extract_note_tags(&text) {
            assert!(!t.is_empty() && !t.starts_with('#'), "bad tag {t:?}");
        }
        let _ = strip_inline_markdown(&text);
        let _ = parse_search_tokens(&text);
        let _ = Wikilink::parse(&text).display_text();
    }
}

#[test]
fn percent_encoding_roundtrips_and_links_decode() {
    let mut rng = Rng(8);
    for _ in 0..cases() {
        let s = gen_text(&mut rng, 10);
        let enc = percent_encode(&s);
        assert!(enc.bytes().all(|b| b.is_ascii_graphic()), "{enc}");
        assert_eq!(percent_decode(&enc), s);
        let _ = percent_decode(&s);
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
            match rng.below(8) {
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

#[test]
fn tabs_invariants_under_random_operations() {
    let mut rng = Rng(77);
    let paths = ["a.md", "b.md", "F/c.md", "F/G/d.md", "e.png"];
    for _ in 0..cases() {
        let mut t = Tabs::default();
        for _ in 0..25 {
            let p = rng.pick(&paths);
            match rng.below(9) {
                0 => {
                    t.open(p, OpenIn::CurrentTab);
                }
                1 => {
                    t.open(p, OpenIn::NewTab);
                }
                2 => {
                    t.close(rng.below(t.len() + 1));
                }
                3 => {
                    t.back();
                }
                4 => {
                    t.forward();
                }
                5 => t.next(),
                6 => {
                    t.rename(if rng.below(2) == 0 { "F" } else { p }, "R");
                }
                7 => {
                    t.remove(if rng.below(2) == 0 { "F" } else { p });
                }
                _ => {
                    t.pop_closed(|_| true);
                }
            }
            match t.active_index() {
                Some(i) => assert!(i < t.len()),
                None => assert!(t.is_empty()),
            }
            if !t.is_empty() {
                assert!(t.active_index().is_some());
            }
        }
    }
}

// ---- The editor's rules (core::editor) ----

#[test]
fn editor_edits_and_movements_keep_the_cursor_on_a_char_boundary() {
    use noteki::core::editor::lines::{line_index_of, logical_lines};
    use noteki::core::editor::{EditBuffer, Motion};
    const MOTIONS: &[Motion] = &[
        Motion::Left,
        Motion::Right,
        Motion::WordLeft,
        Motion::WordRight,
        Motion::LineStart,
        Motion::LineEnd,
        Motion::DocStart,
        Motion::DocEnd,
    ];
    let mut rng = Rng(0xED17_0001);
    for _ in 0..cases() / 4 {
        let original = gen_text(&mut rng, 30);
        let mut b = EditBuffer::new(original.clone());
        b.move_to(rng.below(b.text().len() + 2), false);
        for _ in 0..40 {
            let motion = MOTIONS[rng.below(MOTIONS.len())];
            match rng.below(7) {
                0 => {
                    b.insert(rng.pick(PIECES));
                }
                1 => {
                    b.delete(motion);
                }
                2 => {
                    let piece = rng.pick(PIECES);
                    let sel = rng.below(piece.len() + 2);
                    b.replace_and_mark(None, piece, Some(sel..sel));
                }
                3 => {
                    let n = b.text().len() + 3;
                    b.set_selection(rng.below(n), rng.below(n));
                }
                4 => {
                    b.select_next_occurrence();
                }
                _ => b.apply_motion(motion, rng.below(2) == 0),
            }
            let t = b.text();
            for s in b.selections() {
                for o in [s.anchor, s.head] {
                    assert!(o <= t.len() && t.is_char_boundary(o), "{o} in {t:?}");
                }
            }
            if let Some(m) = b.marked() {
                assert_slice(t, &m, "marked range");
            }
            let lines = logical_lines(t);
            assert!(line_index_of(&lines, b.cursor()) < lines.len());
            for l in &lines {
                assert_slice(t, l, "logical line");
            }
        }
        // Undo all gives the text it started from; redo all, the last.
        let last = b.text().to_string();
        while b.undo() {}
        assert_eq!(b.text(), original);
        while b.redo() {}
        assert_eq!(b.text(), last);
    }
}

#[test]
fn editor_line_styles_stay_inside_their_lines() {
    use noteki::core::editor::lines::logical_lines;
    use noteki::core::editor::styling::style_lines;
    let idx = index();
    let mut rng = Rng(0xED17_0002);
    for _ in 0..cases() {
        let text = gen_text(&mut rng, 40);
        let cursor = Some(rng.below(text.len() + 1));
        let spans = compute_markdown_spans(&text, cursor, Some("Dir/Here.md"), &idx);
        let styles = style_lines(
            &text,
            &spans,
            noteki::core::editor::styling::Context {
                highlight: &mut |l, c| noteki::core::editor::code::highlight(l, c).into(),
                image: &mut |_| None,
            },
        );
        let lines = logical_lines(&text);
        assert_eq!(styles.len(), lines.len(), "one style per line of {text:?}");
        for (style, line) in styles.iter().zip(&lines) {
            // Segments are relative to their line.
            let mut end = 0;
            for (r, kinds) in &style.segments {
                let abs = line.start + r.start..line.start + r.end;
                assert_slice(&text, &abs, "style segment");
                assert!(!kinds.is_empty());
                assert!(
                    r.start >= end && abs.end <= line.end,
                    "{r:?} in {line:?} of {text:?}"
                );
                end = r.end;
            }
        }
    }
}

#[test]
fn editor_display_map_never_puts_an_offset_inside_a_hidden_mark() {
    use noteki::core::editor::display::{shown_text, to_buffer, to_display};
    use noteki::core::editor::lines::logical_lines;
    use noteki::core::editor::styling::style_lines;
    let idx = index();
    let mut rng = Rng(0xED17_0003);
    let pieces = [
        "# ",
        "## ",
        "  ### ",
        "#",
        "\n# ",
        "\n",
        "| a | **b** |\n|---|:-:|\n| c |",
        " | ",
        "- [ ] ",
        "![[x.png]]",
        "```rust\n",
    ];
    for _ in 0..cases() {
        let mut text = gen_text(&mut rng, 30);
        let at = rng.below(text.len() + 1);
        let at = (0..=at).rev().find(|&i| text.is_char_boundary(i)).unwrap();
        text.insert_str(at, rng.pick(&pieces));
        let cursor = (0..=rng.below(text.len() + 1).min(text.len()))
            .rev()
            .find(|&i| text.is_char_boundary(i))
            .unwrap();
        // As the editor does: every line styled with the cursor off it, then
        // the cursor's line again with the cursor on it.
        let spans = compute_markdown_spans(&text, None, None, &idx);
        let lines = logical_lines(&text);
        let mut styles = style_lines(
            &text,
            &spans,
            noteki::core::editor::styling::Context {
                highlight: &mut |l, c| noteki::core::editor::code::highlight(l, c).into(),
                image: &mut |_| None,
            },
        );
        let scanned = noteki::core::markdown::lines(&text);
        let i = line_of_offset(&text, cursor);
        let mut own = Vec::new();
        noteki::core::markdown::line_spans(
            &scanned[i],
            Some(cursor - scanned[i].start),
            None,
            &idx,
            &mut own,
        );
        styles[i] = noteki::core::editor::styling::style_line(&scanned[i], &own);
        for (style, line) in styles.iter().zip(&lines) {
            if line.start <= cursor && cursor <= line.end {
                let c = cursor - line.start;
                assert!(
                    !style.hidden.iter().any(|h| h.start < c && c < h.end),
                    "the cursor {c} inside a hidden mark of {:?}: {:?}",
                    &text[line.clone()],
                    style.hidden
                );
            }
            let line_text = &text[line.clone()];
            let mut end = 0;
            for h in &style.hidden {
                assert!(
                    h.start >= end && h.end <= line_text.len(),
                    "{h:?} in {line_text:?}"
                );
                assert_slice(line_text, h, "hidden range");
                end = h.end;
            }
            let shown = shown_text(line_text, &style.hidden);
            for d in (0..=shown.len()).filter(|&d| shown.is_char_boundary(d)) {
                let o = to_buffer(&style.hidden, d);
                assert_eq!(to_display(&style.hidden, o), d, "{line_text:?}");
                assert!(line_text.is_char_boundary(o));
                assert!(!style.hidden.iter().any(|h| h.start < o && o < h.end));
            }
        }
    }
}

#[test]
fn link_completion_query_is_a_valid_range_before_the_cursor() {
    use noteki::core::editor::completion::link_query;
    let mut rng = Rng(0xED17_0004);
    for _ in 0..cases() {
        let mut text = gen_text(&mut rng, 30);
        let at = (0..=rng.below(text.len() + 1).min(text.len()))
            .rev()
            .find(|&i| text.is_char_boundary(i))
            .unwrap();
        text.insert_str(at, "[[");
        for cursor in (0..=text.len()).filter(|&c| text.is_char_boundary(c)) {
            if let Some(q) = link_query(&text, cursor) {
                assert_slice(&text, &q.range, "link name");
                assert!(
                    q.range.start <= cursor && q.range.start >= 2,
                    "{q:?} at {cursor} in {text:?}"
                );
                assert_eq!(&text[q.range.start - 2..q.range.start], "[[");
            }
        }
    }
}
