//! Golden checks on `docs/demo-vault`, the vault shipped with the repository.

use std::path::PathBuf;

use noteki_tauri_lib::index::{find_backlinks, search_vault};
use noteki_tauri_lib::markdown::wikilinks;
use noteki_tauri_lib::session::Vault;
use noteki_tauri_lib::wikilink::{ResolvedItem, Wikilink};

fn demo() -> Vault {
    Vault::load(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/demo-vault"))
        .expect("demo vault loads")
}

#[test]
fn scans_every_note() {
    let v = demo();
    let mut paths: Vec<&str> = v.corpus.iter().map(|n| n.path.as_str()).collect();
    paths.sort();
    assert_eq!(
        paths,
        vec![
            "Daily/2026-11-12.md",
            "Ideas.md",
            "Meetings/2026-11-10 Product sync.md",
            "Physics/Euler's identity.md",
            "Physics/Maxwell's equations.md",
            "Projects/Launch plan.md",
            "Reading list.md",
            "Welcome.md",
        ]
    );
}

#[test]
fn every_wikilink_of_the_demo_resolves() {
    let v = demo();
    let mut unresolved = Vec::new();
    let mut count = 0;
    for note in v.corpus.iter() {
        for (line, tok) in wikilinks(&note.text) {
            count += 1;
            let link = Wikilink::parse(&note.text[tok.inner.clone()]);
            if v.index.resolve(&link.target, Some(&note.path)).is_none() {
                unresolved.push(format!("{}:{} [[{}]]", note.path, line + 1, link.target));
            }
        }
    }
    assert!(count >= 12, "found only {count} wikilinks");
    assert!(unresolved.is_empty(), "unresolved: {unresolved:#?}");
    assert!(matches!(
        v.index.resolve("noteki.png", Some("Welcome.md")),
        Some(ResolvedItem::Attachment { .. })
    ));
}

#[test]
fn tags_from_frontmatter_and_text() {
    let v = demo();
    let top: Vec<&str> = v.tag_tree.iter().map(|t| t.full_lower.as_str()).collect();
    for tag in [
        "electromagnetism",
        "math",
        "meeting",
        "noteki",
        "physics",
        "product",
        "project",
        "reading",
    ] {
        assert!(top.contains(&tag), "missing #{tag} in {top:?}");
    }
    let product = v
        .tag_tree
        .iter()
        .find(|t| t.full_lower == "product")
        .unwrap();
    assert_eq!(product.children[0].full_lower, "product/noteki");
    assert_eq!(product.count, 2);
}

#[test]
fn backlinks_of_the_launch_plan() {
    let v = demo();
    let bl = find_backlinks(&v.corpus, "Projects/Launch plan.md", &v.index);
    let sources: Vec<(&str, usize)> = bl
        .iter()
        .map(|b| (b.source_path.as_str(), b.line))
        .collect();
    // Sorted by note name: "2026-11-10 Product sync" < "2026-11-12".
    assert_eq!(
        sources,
        vec![
            ("Meetings/2026-11-10 Product sync.md", 19),
            ("Daily/2026-11-12.md", 5),
            ("Welcome.md", 11)
        ]
    );
}

#[test]
fn search_finds_words_and_tags() {
    let v = demo();
    let names = |q: &str| {
        search_vault(&v.corpus, q)
            .into_iter()
            .map(|r| r.name)
            .collect::<Vec<_>>()
    };
    assert_eq!(
        names("sparkle"),
        vec!["2026-11-10 Product sync", "Launch plan"]
    );
    assert_eq!(
        names("tag:product"),
        vec!["2026-11-10 Product sync", "Launch plan"]
    );
    assert_eq!(names("tag:product path:projects"), vec!["Launch plan"]);
    assert_eq!(
        names("\"dry run\" homebrew"),
        vec!["2026-11-10 Product sync", "Launch plan"]
    );
}
