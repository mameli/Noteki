//! Bugs found in the first prototype, reproduced before they were fixed.
use noteki_gpui::core::index::extract_note_tags;
use noteki_gpui::core::markdown::{
    compute_markdown_spans, preprocess_for_reader as preprocess_markdown_for_reader,
};
use noteki_gpui::core::wikilink::VaultIndex;

#[test]
fn backslash_before_multibyte_char_does_not_panic() {
    let index = VaultIndex::default();
    let text = "Prezzo \\è alto #tag \\€";
    let _ = extract_note_tags(text);
    let _ = compute_markdown_spans(text, Some(0), None, &index);
}

#[test]
fn reader_wikilink_with_spaces_is_a_markdown_link() {
    let index = VaultIndex::default();
    let md = preprocess_markdown_for_reader("See [[Launch plan]].", None, &index);
    let ast = markdown::to_mdast(&md, &markdown::ParseOptions::gfm()).unwrap();
    let mut found = false;
    fn walk(n: &markdown::mdast::Node, found: &mut bool) {
        if let markdown::mdast::Node::Link(_) = n {
            *found = true;
        }
        if let Some(c) = n.children() {
            c.iter().for_each(|c| walk(c, found));
        }
    }
    walk(&ast, &mut found);
    assert!(found, "no link parsed from: {md}");
}
