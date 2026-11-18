//! Golden checks on `docs/demo-vault`, the vault shipped with the repository.
//! They read the real files, so a regression in scanning, links, the HTML
//! conversion or the indexes shows up here as a concrete note and line.

use std::path::PathBuf;

use markdown::mdast::Node;
use noteki::app::Vault;
use noteki::core::index::{extract_outline, find_backlinks, search_vault};
use noteki::core::markdown::{
    percent_decode, preprocess_for_html, wikilinks, EMBED_SCHEME, LINK_SCHEME,
};
use noteki::core::wikilink::{ResolvedItem, Wikilink};

fn demo() -> Vault {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/demo-vault");
    Vault::load(&root).expect("demo vault loads")
}

fn collect<'a>(node: &'a Node, links: &mut Vec<&'a str>, images: &mut Vec<&'a str>) {
    match node {
        Node::Link(l) => links.push(&l.url),
        Node::Image(i) => images.push(&i.url),
        _ => {}
    }
    for child in node.children().into_iter().flatten() {
        collect(child, links, images);
    }
}

#[test]
fn scans_every_note_and_attachment() {
    let v = demo();
    let mut paths: Vec<&str> = v.notes.iter().map(|(_, p)| p.as_str()).collect();
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
    assert!(v.contains("Media/noteki.png"));
    assert_eq!(v.corpus.len(), 8);
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
}

#[test]
fn html_turns_every_wikilink_into_a_working_link() {
    let v = demo();
    for note in v.corpus.iter() {
        let md = preprocess_for_html(&note.text, Some(&note.path), &v.index);
        let ast = markdown::to_mdast(&md, &markdown::ParseOptions::gfm())
            .unwrap_or_else(|e| panic!("{}: {e}", note.path));
        let (mut links, mut images) = (Vec::new(), Vec::new());
        collect(&ast, &mut links, &mut images);

        let expected_links = wikilinks(&note.text)
            .iter()
            .filter(|(_, t)| !note.text[t.range.clone()].starts_with('!'))
            .count();
        let ours: Vec<&&str> = links
            .iter()
            .filter(|u| u.starts_with(LINK_SCHEME))
            .collect();
        assert_eq!(
            ours.len(),
            expected_links,
            "{}: links in the HTML {links:?}\n{md}",
            note.path
        );
        for url in ours {
            let body = url.strip_prefix(LINK_SCHEME).unwrap();
            let target = percent_decode(body.split('#').next().unwrap());
            assert!(
                v.index.resolve(&target, Some(&note.path)).is_some(),
                "{}: {url} → {target} does not resolve",
                note.path
            );
        }
        for url in images {
            let path = percent_decode(
                url.strip_prefix(EMBED_SCHEME)
                    .unwrap_or_else(|| panic!("unexpected image {url}")),
            );
            assert!(
                v.root.join(&path).is_file(),
                "{}: image {path} missing",
                note.path
            );
        }
    }
}

#[test]
fn welcome_embeds_the_logo_with_alt_text_not_its_width() {
    let v = demo();
    let text = v.corpus.get("Welcome.md").unwrap();
    let md = preprocess_for_html(text, Some("Welcome.md"), &v.index);
    assert!(
        md.contains("![noteki.png](noteki-embed:Media/noteki.png)"),
        "{md}"
    );
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
    assert_eq!(
        sources,
        // Sorted by note name: "2026-11-10 Product sync" < "2026-11-12".
        vec![
            ("Meetings/2026-11-10 Product sync.md", 19),
            ("Daily/2026-11-12.md", 5),
            ("Welcome.md", 11)
        ]
    );
}

#[test]
fn outline_skips_code_and_frontmatter() {
    let v = demo();
    let outline: Vec<String> =
        extract_outline(v.corpus.get("Physics/Maxwell's equations.md").unwrap())
            .into_iter()
            .map(|h| h.text)
            .collect();
    assert_eq!(
        outline,
        vec!["Maxwell's equations", "The same, in integral form"]
    );
    let plan: Vec<String> = extract_outline(v.corpus.get("Projects/Launch plan.md").unwrap())
        .into_iter()
        .map(|h| h.text)
        .collect();
    assert_eq!(plan, vec!["Launch plan"]);
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

#[test]
fn attachments_resolve_as_attachments() {
    let v = demo();
    assert!(matches!(
        v.index.resolve("noteki.png", Some("Welcome.md")),
        Some(ResolvedItem::Attachment { .. })
    ));
}
