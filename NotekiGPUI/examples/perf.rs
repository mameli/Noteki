//! Performance budgets on a synthetic 5,000-note vault.
//!
//!     cargo run --release --example perf [-- NOTES]
//!
//! Exits with status 1 when a budget is missed. Per-keystroke work (live
//! preview, outline) must fit in a frame; vault-wide work in a fraction of a second.

use std::path::Path;
use std::time::{Duration, Instant};

use noteki_gpui::app::Vault;
use noteki_gpui::core::index::{
    build_tag_tree, extract_outline, find_backlinks, quick_switcher_rank, search_vault,
};
use noteki_gpui::core::markdown::{compute_markdown_spans, preprocess_for_reader};

struct Rng(u64);
impl Rng {
    fn below(&mut self, n: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 % n as u64) as usize
    }
}

const WORDS: &[&str] = &[
    "project", "alpha", "meeting", "notes", "release", "design", "review", "è", "perché", "sprint",
    "roadmap", "bug", "customer", "draft", "idea", "timeline", "budget", "risk", "launch", "retro",
];

fn note_text(rng: &mut Rng, n_notes: usize, paragraphs: usize) -> String {
    let mut s = format!(
        "---\ntags: [area/{}, status]\n---\n# Note title\n\n",
        rng.below(30)
    );
    for p in 0..paragraphs {
        if p % 4 == 0 {
            s.push_str(&format!("## Section {p}\n\n"));
        }
        for _ in 0..40 {
            s.push_str(WORDS[rng.below(WORDS.len())]);
            s.push(' ');
        }
        s.push_str(&format!(
            "see [[Note {}]] and **bold** `code` #tag{} ==mark==\n- [ ] task {p}\n\n",
            rng.below(n_notes),
            rng.below(50)
        ));
    }
    s.push_str("```rust\nfn main() {}\n```\n");
    s
}

fn make_vault(root: &Path, n: usize) {
    let mut rng = Rng(0xDEAD_BEEF);
    for i in 0..n {
        let dir = root.join(format!("Folder {}", i % 50));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(format!("Note {i}.md")), note_text(&mut rng, n, 4)).unwrap();
    }
}

fn time<T>(f: impl FnOnce() -> T) -> (T, Duration) {
    let t = Instant::now();
    let out = f();
    (out, t.elapsed())
}

/// Best of `runs`, to keep scheduler noise out of the verdict.
fn best<T>(runs: usize, mut f: impl FnMut() -> T) -> Duration {
    (0..runs).map(|_| time(&mut f).1).min().unwrap()
}

fn main() {
    let n: usize = std::env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(5000);
    let root = std::env::temp_dir().join(format!("noteki-perf-{n}"));
    if !root.join("Folder 0").exists() {
        make_vault(&root, n);
    }

    let (vault, cold) = time(|| Vault::load(&root).unwrap());
    let load = best(3, || Vault::load(&root).unwrap());
    let mut refreshed = Vault::load(&root).unwrap();
    let changed = ["Folder 0/Note 0.md".to_string()].into_iter().collect();
    let refresh = best(3, || refreshed.refresh(&changed).unwrap());
    let mut rng = Rng(7);
    let big = note_text(&mut rng, n, 300); // ~100 KB note
    let huge = note_text(&mut rng, n, 3000); // ~1 MB note (live preview limit)

    let rows: Vec<(&str, Duration, Duration)> = vec![
        (
            "open vault, warm cache (scan, read, index, tags)",
            load,
            Duration::from_millis(1500),
        ),
        (
            "external change: rescan tree, re-read 1 note",
            refresh,
            Duration::from_millis(500),
        ),
        (
            "search two words",
            best(5, || search_vault(&vault.corpus, "project alpha")),
            Duration::from_millis(200),
        ),
        (
            "search tag: + path:",
            best(5, || search_vault(&vault.corpus, "tag:area/3 path:folder")),
            Duration::from_millis(200),
        ),
        (
            "backlinks of a note",
            best(5, || {
                find_backlinks(&vault.corpus, "Folder 7/Note 7.md", &vault.index)
            }),
            Duration::from_millis(250),
        ),
        (
            "tag tree (after each save)",
            best(5, || build_tag_tree(&vault.corpus)),
            Duration::from_millis(250),
        ),
        (
            "quick switcher ranking",
            best(5, || quick_switcher_rank(&vault.notes, &[], "note 42")),
            Duration::from_millis(50),
        ),
        (
            "live preview, 100 KB note (per keystroke)",
            best(10, || {
                compute_markdown_spans(&big, 10, Some("x.md"), &vault.index)
            }),
            Duration::from_millis(16),
        ),
        (
            "live preview, 1 MB note (per keystroke)",
            best(3, || {
                compute_markdown_spans(&huge, 10, Some("x.md"), &vault.index)
            }),
            Duration::from_millis(120),
        ),
        (
            "outline, 100 KB note (per keystroke)",
            best(10, || extract_outline(&big)),
            Duration::from_millis(8),
        ),
        (
            "reader markdown, 100 KB note",
            best(10, || {
                preprocess_for_reader(&big, Some("x.md"), &vault.index)
            }),
            Duration::from_millis(30),
        ),
    ];

    println!(
        "Vault: {} notes, {:.1} MB of text; big note {} KB",
        vault.notes.len(),
        vault.corpus.iter().map(|n| n.text.len()).sum::<usize>() as f64 / 1e6,
        big.len() / 1000
    );
    println!(
        "First open (cold file cache, informational): {:.0} ms",
        cold.as_secs_f64() * 1e3
    );
    println!("| Operation | Time | Budget | |");
    println!("|---|---:|---:|---|");
    let mut failed = 0;
    for (name, took, budget) in &rows {
        let ok = took <= budget;
        failed += usize::from(!ok);
        println!(
            "| {name} | {:.1} ms | {} ms | {} |",
            took.as_secs_f64() * 1e3,
            budget.as_millis(),
            if ok { "PASS" } else { "FAIL" }
        );
    }
    if failed > 0 {
        eprintln!("{failed} budget(s) missed");
        std::process::exit(1);
    }
}
