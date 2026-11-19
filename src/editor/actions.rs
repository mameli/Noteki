//! Each of the editor's actions (keys and menus) bound to what it does.

use super::*;

macro_rules! handlers {
    ($($action:ident => |$ed:ident, $cx:ident| $body:expr;)*) => {
        impl NoteEditor {
            pub(super) fn bind_actions(&self, el: gpui_kit::Stateful<gpui_kit::Div>, cx: &mut Context<Self>) -> gpui_kit::Stateful<gpui_kit::Div> {
                el $(.on_action(cx.listener(|$ed: &mut Self, _: &$action, _w, $cx| $body)))*
            }
        }
    };
}

handlers! {
    Backspace => |ed, cx| ed.delete(Motion::Left, cx);
    Delete => |ed, cx| ed.delete(Motion::Right, cx);
    DeleteWordLeft => |ed, cx| ed.delete(Motion::WordLeft, cx);
    DeleteWordRight => |ed, cx| ed.delete(Motion::WordRight, cx);
    DeleteToLineStart => |ed, cx| ed.delete_row(false, cx);
    DeleteToLineEnd => |ed, cx| ed.delete_row(true, cx);
    DeleteToParagraphEnd => |ed, cx| ed.delete(Motion::LineEnd, cx);
    Newline => |ed, cx| if !ed.accept_completion(cx) { ed.newline(cx) };
    SoftNewline => |ed, cx| if !ed.accept_completion(cx) { ed.replace_selection("\n", cx) };
    TabKey => |ed, cx| if !ed.accept_completion(cx) { ed.replace_selection("\t", cx) };
    Left => |ed, cx| ed.motion(Motion::Left, false, cx);
    Right => |ed, cx| ed.motion(Motion::Right, false, cx);
    Up => |ed, cx| if !ed.completion_move(-1, cx) { ed.vertical(true, false, cx) };
    Down => |ed, cx| if !ed.completion_move(1, cx) { ed.vertical(false, false, cx) };
    WordLeft => |ed, cx| ed.motion(Motion::WordLeft, false, cx);
    WordRight => |ed, cx| ed.motion(Motion::WordRight, false, cx);
    LineStart => |ed, cx| ed.row_motion(false, false, cx);
    LineEnd => |ed, cx| ed.row_motion(true, false, cx);
    ParagraphStart => |ed, cx| ed.motion(Motion::LineStart, false, cx);
    ParagraphEnd => |ed, cx| ed.motion(Motion::LineEnd, false, cx);
    DocStart => |ed, cx| ed.motion(Motion::DocStart, false, cx);
    DocEnd => |ed, cx| ed.motion(Motion::DocEnd, false, cx);
    SelectLeft => |ed, cx| ed.motion(Motion::Left, true, cx);
    SelectRight => |ed, cx| ed.motion(Motion::Right, true, cx);
    SelectUp => |ed, cx| ed.vertical(true, true, cx);
    SelectDown => |ed, cx| ed.vertical(false, true, cx);
    SelectWordLeft => |ed, cx| ed.motion(Motion::WordLeft, true, cx);
    SelectWordRight => |ed, cx| ed.motion(Motion::WordRight, true, cx);
    SelectLineStart => |ed, cx| ed.row_motion(false, true, cx);
    SelectLineEnd => |ed, cx| ed.row_motion(true, true, cx);
    SelectDocStart => |ed, cx| ed.motion(Motion::DocStart, true, cx);
    SelectDocEnd => |ed, cx| ed.motion(Motion::DocEnd, true, cx);
    SelectAll => |ed, cx| { ed.buf.select_all(); ed.wake(cx) };
    Heading1 => |ed, cx| ed.format(Format::Lines(LineFormat::Heading(1)), cx);
    Heading2 => |ed, cx| ed.format(Format::Lines(LineFormat::Heading(2)), cx);
    Heading3 => |ed, cx| ed.format(Format::Lines(LineFormat::Heading(3)), cx);
    Heading4 => |ed, cx| ed.format(Format::Lines(LineFormat::Heading(4)), cx);
    Heading5 => |ed, cx| ed.format(Format::Lines(LineFormat::Heading(5)), cx);
    Heading6 => |ed, cx| ed.format(Format::Lines(LineFormat::Heading(6)), cx);
    ToggleTask => |ed, cx| ed.format(Format::Lines(LineFormat::Task), cx);
    MakeLink => |ed, cx| ed.format(Format::Link, cx);
    ToggleComment => |ed, cx| ed.format(Format::Comment, cx);
    Indent => |ed, cx| ed.format(Format::Lines(LineFormat::Indent), cx);
    Outdent => |ed, cx| ed.format(Format::Lines(LineFormat::Outdent), cx);
    SelectNextOccurrence => |ed, cx| { ed.buf.select_next_occurrence(); ed.wake(cx) };
    SingleSelection => |ed, cx| {
        if ed.dismiss_completion(cx) {
            return;
        }
        let main = ed.buf.selection();
        ed.buf.set_selection(main.anchor, main.head);
        ed.wake(cx)
    };
    Undo => |ed, cx| { let changed = ed.buf.undo(); ed.edited(changed, cx) };
    Redo => |ed, cx| { let changed = ed.buf.redo(); ed.edited(changed, cx) };
    Copy => |ed, cx| {
        if !ed.buf.selection().is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(ed.buf.selected_text().to_string()));
        }
    };
    Cut => |ed, cx| {
        if !ed.buf.selection().is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(ed.buf.selected_text().to_string()));
            ed.replace_selection("", cx);
        }
    };
    Paste => |ed, cx| {
        let Some(item) = cx.read_from_clipboard() else { return };
        // An image (a screenshot): saved next to the Note, embedded at the cursor.
        let image = item.entries().iter().find_map(|e| match e {
            gpui_kit::ClipboardEntry::Image(img) => Some(img.clone()),
            _ => None,
        });
        if let (Some(img), None) = (image, item.text()) {
            cx.emit(NoteEditorEvent::PasteImage {
                bytes: img.bytes.clone(),
                extension: image_extension(img.format),
            });
        } else if let Some(text) = item.text() {
            // Formatted text (a web page, Mail, Word) as Markdown, unless in code,
            // the Frontmatter or a Comment.
            let at = ed.buf.selection().range().start;
            let markdown = clipboard::html_of(&text)
                .filter(|_| !crate::core::paste::pastes_plain_at(ed.buf.text(), at))
                .and_then(|html| crate::core::paste::markdown_of(&html, &text));
            ed.replace_selection(markdown.as_deref().unwrap_or(&text), cx);
        }
    };
    // The text alone, never an image.
    PasteAndMatchStyle => |ed, cx| {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            ed.replace_selection(&text, cx);
        }
    };
}
