//! What GPUI's clipboard does not give: the HTML a copy with its formatting
//! carries (a web page, Mail, Word), pasted as Markdown (core::paste).

/// The pasteboard's HTML, when the copy whose text is `text` has some: the
/// pasteboard's own text must be `text`, so the HTML is of the same copy (and
/// a test's clipboard never meets the Mac's).
pub fn html_of(text: &str) -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        html_in(&objc2_app_kit::NSPasteboard::generalPasteboard(), text)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = text;
        None
    }
}

#[cfg(target_os = "macos")]
fn html_in(board: &objc2_app_kit::NSPasteboard, text: &str) -> Option<String> {
    use objc2_app_kit::{NSPasteboardTypeHTML, NSPasteboardTypeString};
    // SAFETY: constants of AppKit's.
    let (html, string) = unsafe { (NSPasteboardTypeHTML, NSPasteboardTypeString) };
    if board.stringForType(string)?.to_string() != text {
        return None;
    }
    Some(board.stringForType(html)?.to_string())
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    use objc2_app_kit::{NSPasteboard, NSPasteboardTypeHTML, NSPasteboardTypeString};
    use objc2_foundation::NSString;

    /// On a pasteboard of its own: the user's clipboard is left alone.
    #[test]
    fn the_html_of_the_same_copy_only() {
        let board = NSPasteboard::pasteboardWithUniqueName();
        // SAFETY: constants of AppKit's.
        let (html, string) = unsafe { (NSPasteboardTypeHTML, NSPasteboardTypeString) };
        board.clearContents();
        board.setString_forType(&NSString::from_str("ciao"), string);
        board.setString_forType(&NSString::from_str("<b>ciao</b>"), html);
        assert_eq!(html_in(&board, "ciao").as_deref(), Some("<b>ciao</b>"));
        assert_eq!(html_in(&board, "altro"), None, "another copy's text");
    }
}
