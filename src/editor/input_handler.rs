//! The platform's text input: typed text, the input methods' marked text,
//! and where the text is for the system's popups.

use super::*;

impl EntityInputHandler for NoteEditor {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        adjusted: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let t = self.buf.text();
        let r = range_from_utf16(t, &range_utf16);
        adjusted.replace(range_to_utf16(t, &r));
        Some(t[r].to_string())
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let s = self.buf.selection();
        Some(UTF16Selection {
            range: range_to_utf16(self.buf.text(), &s.range()),
            reversed: s.reversed(),
        })
    }

    fn set_selected_text_range(
        &mut self,
        range_utf16: Range<usize>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let r = range_from_utf16(self.buf.text(), &range_utf16);
        self.set_selected_range(r, cx);
    }

    fn text_length_utf16(&mut self, _: &mut Window, _: &mut Context<Self>) -> Option<usize> {
        Some(self.buf.text().encode_utf16().count())
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.buf
            .marked()
            .map(|r| range_to_utf16(self.buf.text(), &r))
    }

    fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.buf.unmark();
        cx.notify();
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let changed = match range_utf16 {
            Some(r) => {
                let r = range_from_utf16(self.buf.text(), &r);
                self.buf.replace(r, text)
            }
            None => self.buf.insert(text),
        };
        window.notify_text_input_changed(&self.focus_handle);
        self.edited(changed, cx);
        if changed {
            self.correct_word_before(text, cx);
        }
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        text: &str,
        selected_utf16: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range_utf16.map(|r| range_from_utf16(self.buf.text(), &r));
        let selected = selected_utf16.map(|r| range_from_utf16(text, &r));
        let changed = self.buf.replace_and_mark(range, text, selected);
        window.notify_text_input_changed(&self.focus_handle);
        self.edited(changed, cx);
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        _: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let layout = self.layout.as_ref()?;
        let t = self.buf.text();
        let r = range_from_utf16(t, &range_utf16);
        let start = layout.caret(t, r.start)?;
        let end = layout.caret(t, r.end)?;
        Some(if start.top() == end.top() {
            Bounds::from_corners(start.origin, end.bottom_right())
        } else {
            start
        })
    }

    fn character_index_for_point(
        &mut self,
        p: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        let layout = self.layout.as_ref()?;
        let t = self.buf.text();
        Some(crate::core::editor::buffer::offset_to_utf16(
            t,
            layout.offset_for_point(t, p),
        ))
    }
}
