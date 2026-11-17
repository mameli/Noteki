//! The `[[` completion list under the caret. The rules (what is being typed,
//! the ranking, what choosing writes) are `core::editor::completion`'s.

use gpui_kit::component::{v_flex, ActiveTheme as _};
use gpui_kit::{
    anchored, deferred, div, point, prelude::*, px, AnyElement, Context, MouseButton,
    TestSupportExt as _,
};

use crate::core::editor::completion::{choose, link_query, rank, LinkQuery};
use crate::core::wikilink::LinkCompletion;

use super::NoteEditor;

/// At most this many Notes are listed.
const SHOWN: usize = 8;

impl NoteEditor {
    /// The link being typed and the Notes that match it, while the list is
    /// open: one caret, in a `[[` name, not dismissed with Esc.
    fn completion(&self) -> Option<(LinkQuery, Vec<LinkCompletion>)> {
        if self.typed_at != Some((self.revision, self.buf.cursor()))
            || self.buf.selections().len() > 1
        {
            return None;
        }
        let q = link_query(self.buf.text(), self.buf.cursor())?;
        if self.completion_dismissed == Some(q.range.start) {
            return None;
        }
        let items: Vec<LinkCompletion> = rank(&self.link_context.completions, &q.query)
            .into_iter()
            .take(SHOWN)
            .cloned()
            .collect();
        (!items.is_empty()).then_some((q, items))
    }

    /// ↑ (`-1`) or ↓ in the open list; whether there was one.
    pub(super) fn completion_move(&mut self, delta: isize, cx: &mut Context<Self>) -> bool {
        let Some((_, items)) = self.completion() else {
            return false;
        };
        let n = items.len() as isize;
        self.completion_pick = ((self.completion_pick as isize + delta).rem_euclid(n)) as usize;
        cx.notify();
        true
    }

    /// ↩ or Tab in the open list: writes the chosen Note; whether there was one.
    pub(super) fn accept_completion(&mut self, cx: &mut Context<Self>) -> bool {
        let Some((q, items)) = self.completion() else {
            return false;
        };
        let pick = &items[self.completion_pick.min(items.len() - 1)];
        let change = choose(&q, &pick.link);
        self.buf.close_undo_step();
        let changed = self.buf.replace(change.range, &change.text);
        self.buf
            .set_selection(change.selection.start, change.selection.end);
        self.edited(changed, cx);
        true
    }

    /// Esc in the open list: closes it until the cursor leaves this link.
    pub(super) fn dismiss_completion(&mut self, cx: &mut Context<Self>) -> bool {
        let Some((q, _)) = self.completion() else {
            return false;
        };
        self.completion_dismissed = Some(q.range.start);
        cx.notify();
        true
    }

    pub(super) fn render_completion(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (_, items) = self.completion()?;
        let caret = self
            .layout
            .as_ref()?
            .caret(self.buf.text(), self.buf.cursor())?;
        let theme = cx.theme();
        let pick = self.completion_pick.min(items.len() - 1);
        let list = v_flex()
            .id("link-completion")
            .test_support()
            .min_w(px(240.))
            .max_w(px(420.))
            .py_1()
            .rounded(px(6.))
            .border_1()
            .border_color(theme.border)
            .bg(theme.popover)
            .shadow_md()
            .children(items.into_iter().enumerate().map(|(i, item)| {
                div()
                    .id(("link-completion-item", i))
                    .px_2()
                    .py(px(2.))
                    .flex()
                    .gap_2()
                    .when(i == pick, |d| d.bg(theme.accent))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |ed, _, _, cx| {
                            ed.completion_pick = i;
                            ed.accept_completion(cx);
                            cx.stop_propagation();
                        }),
                    )
                    .child(div().text_sm().child(item.name))
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(item.folder),
                    )
            }));
        Some(
            deferred(
                anchored()
                    .position(point(caret.left(), caret.bottom()))
                    .child(list),
            )
            .into_any_element(),
        )
    }
}
