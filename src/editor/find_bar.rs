//! The find bar over the editor (⌘F): a query, its options, next, previous,
//! replace and replace all. The rules are `core::editor::find`'s.

use std::ops::Range;

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{h_flex, ActiveTheme as _, Sizable as _};
use gpui_kit::{
    div, prelude::*, px, AnyElement, Context, Entity, Role, SharedString, Subscription,
    TestSupportExt as _, Window,
};

use crate::core::editor::find::{find_all, replace_all, replacement_for, Query};
use crate::core::editor::movement::word_range_at;

use super::{NoteEditor, NoteEditorEvent};

pub(super) const FIND_CONTEXT: &str = "NoteFind";

pub(super) struct FindBar {
    query_input: Entity<InputState>,
    replace_input: Entity<InputState>,
    pub(super) query: Query,
    /// The matches of `query`, and the `(revision, query)` they are for.
    matches: Vec<Range<usize>>,
    found_for: Option<(u64, Query)>,
    error: Option<String>,
    _subscriptions: Vec<Subscription>,
}

impl NoteEditor {
    /// ⌘F: opens the bar with the selection, or the word at the cursor, as the query.
    pub fn open_find(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.buf.text();
        let sel = self.buf.selection().range();
        let seed = if sel.is_empty() {
            let w = word_range_at(text, sel.start);
            text[w].to_string()
        } else {
            text[sel].to_string()
        };
        let seed = if seed.contains('\n') || seed.trim().is_empty() {
            String::new()
        } else {
            seed
        };
        if self.find.is_none() {
            let query_input = cx.new(|cx| InputState::new(window, cx).placeholder("Find"));
            let replace_input = cx.new(|cx| InputState::new(window, cx).placeholder("Replace"));
            let subscriptions = vec![
                cx.subscribe_in(
                    &query_input,
                    window,
                    |ed, _, ev: &InputEvent, window, cx| match ev {
                        InputEvent::Change => {
                            let text = ed
                                .find
                                .as_ref()
                                .map(|f| f.query_input.read(cx).value().to_string());
                            if let (Some(bar), Some(text)) = (ed.find.as_mut(), text) {
                                bar.query.text = text;
                            }
                            ed.find_step(true, true, window, cx);
                        }
                        InputEvent::PressEnter { shift, .. } => {
                            ed.find_step(!shift, false, window, cx)
                        }
                        _ => {}
                    },
                ),
                cx.subscribe_in(
                    &replace_input,
                    window,
                    |ed, _, ev: &InputEvent, window, cx| {
                        if let InputEvent::PressEnter { .. } = ev {
                            ed.replace_one(window, cx);
                        }
                    },
                ),
            ];
            self.find = Some(FindBar {
                query_input,
                replace_input,
                query: Query::default(),
                matches: Vec::new(),
                found_for: None,
                error: None,
                _subscriptions: subscriptions,
            });
        }
        let bar = self.find.as_mut().expect("just opened");
        if !seed.is_empty() {
            bar.query.text = seed.clone();
            bar.query_input
                .update(cx, |input, cx| input.set_value(seed, window, cx));
        }
        let input = bar.query_input.clone();
        input.update(cx, |input, cx| {
            input.focus(window, cx);
            input.select_all(window, cx);
        });
        self.find_step(true, true, window, cx);
    }

    /// ⌥⌘F: the bar, with the keyboard in the replace field.
    pub fn open_replace(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_find(window, cx);
        if let Some(bar) = &self.find {
            bar.replace_input
                .update(cx, |input, cx| input.focus(window, cx));
        }
    }

    /// Esc: closes the bar, back to the text.
    pub fn close_find(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.find = None;
        window.focus(&self.focus_handle, cx);
        cx.notify();
    }

    /// The matches of the current query, found again only when the text or
    /// the query changed.
    pub(super) fn refresh_matches(&mut self) -> &[Range<usize>] {
        let revision = self.revision;
        let text = self.buf.text();
        let Some(bar) = self.find.as_mut() else {
            return &[];
        };
        let key = (revision, bar.query.clone());
        if bar.found_for.as_ref() != Some(&key) {
            match find_all(text, &bar.query) {
                Ok(m) => {
                    bar.matches = m;
                    bar.error = None;
                }
                Err(e) => {
                    bar.matches.clear();
                    bar.error = Some(e);
                }
            }
            bar.found_for = Some(key);
        }
        &bar.matches
    }

    /// Selects the next (or previous) match from the selection, wrapping.
    /// `from_start`: from the selection's start (as the query is typed, so
    /// the match under the cursor stays).
    pub(super) fn find_step(
        &mut self,
        forward: bool,
        from_start: bool,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let sel = self.buf.selection().range();
        let matches = self.refresh_matches().to_vec();
        let next = if forward {
            // As the query is typed: the match the cursor is in, or the next;
            // after that, the match past the selected one.
            matches
                .iter()
                .find(|m| {
                    if from_start {
                        m.end > sel.start
                    } else {
                        m.start >= sel.end
                    }
                })
                .or(matches.first())
        } else {
            matches
                .iter()
                .rev()
                .find(|m| m.end <= sel.start)
                .or(matches.last())
        };
        if let Some(m) = next.cloned() {
            self.buf.set_selection(m.start, m.end);
            self.goal_x = None;
            self.wake(cx);
        } else {
            cx.notify();
        }
    }

    /// Replaces the selected match and goes to the next.
    pub(super) fn replace_one(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(bar) = &self.find else {
            return;
        };
        let with = bar.replace_input.read(cx).value().to_string();
        let query = bar.query.clone();
        let sel = self.buf.selection().range();
        if self.refresh_matches().contains(&sel) {
            let new = replacement_for(self.buf.text(), sel.clone(), &query, &with);
            self.buf.close_undo_step();
            let changed = self.buf.replace(sel, &new);
            self.edited(changed, cx);
        }
        self.find_step(true, false, window, cx);
    }

    /// "All": every match selected, to type over them at once.
    pub(super) fn select_every(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let matches = self.refresh_matches().to_vec();
        if matches.is_empty() {
            return;
        }
        let sel = self.buf.selection().range();
        let main = matches.iter().position(|m| *m == sel).unwrap_or(0);
        self.buf.set_selections(&matches, main);
        window.focus(&self.focus_handle, cx);
        self.wake(cx);
    }

    /// Replaces every match: one undo step.
    pub(super) fn replace_every(&mut self, cx: &mut Context<Self>) {
        let Some(bar) = &self.find else {
            return;
        };
        let with = bar.replace_input.read(cx).value().to_string();
        if let Ok((new, n)) = replace_all(self.buf.text(), &bar.query, &with) {
            if n > 0 {
                let cursor = self.buf.cursor().min(new.len());
                self.buf.replace_all(&new, cursor);
                self.revision += 1;
                self.wake(cx);
                cx.emit(NoteEditorEvent::Changed);
            }
        }
    }

    fn toggle_option(&mut self, set: fn(&mut Query), window: &mut Window, cx: &mut Context<Self>) {
        if let Some(bar) = self.find.as_mut() {
            set(&mut bar.query);
        }
        self.find_step(true, true, window, cx);
    }

    /// Puts `text` in the replace field (for tests: typing there needs the
    /// field focused).
    pub fn replace_with_for_test(
        &mut self,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(bar) = &self.find {
            let text = text.to_string();
            bar.replace_input
                .update(cx, |input, cx| input.set_value(text, window, cx));
        }
    }

    /// The matches to highlight, for the element.
    pub(super) fn find_matches(&mut self) -> Vec<Range<usize>> {
        self.refresh_matches().to_vec()
    }

    pub(super) fn render_find_bar(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let sel = self.buf.selection().range();
        let matches = self.refresh_matches().to_vec();
        let bar = self.find.as_ref()?;
        let current = matches.iter().position(|m| *m == sel);
        let status: SharedString = match (&bar.error, matches.len(), current) {
            (Some(_), _, _) => "Invalid regex".into(),
            (None, 0, _) if bar.query.text.is_empty() => "".into(),
            (None, 0, _) => "No results".into(),
            (None, n, Some(i)) => format!("{} of {n}", i + 1).into(),
            (None, n, None) => format!("{n} found").into(),
        };
        let toggle = |id: &'static str, label: &'static str, tip: &'static str, on: bool| {
            let b = Button::new(id).label(label).xsmall().tooltip(tip);
            if on {
                b.primary()
            } else {
                b.ghost()
            }
        };
        let q = &bar.query;
        let theme = cx.theme();
        Some(
            div()
                .id("find-bar")
                .test_support()
                .key_context(FIND_CONTEXT)
                .on_action(cx.listener(|ed, _: &CloseFind, window, cx| ed.close_find(window, cx)))
                .on_action(cx.listener(|ed, _: &super::FindNext, window, cx| {
                    ed.find_step(true, false, window, cx)
                }))
                .on_action(cx.listener(|ed, _: &super::FindPrevious, window, cx| {
                    ed.find_step(false, false, window, cx)
                }))
                .w_full()
                .px_3()
                .py_1()
                .gap_1()
                .flex()
                .flex_col()
                .border_b_1()
                .border_color(theme.border)
                .bg(theme.tab_bar)
                .child(
                    h_flex()
                        .gap_1()
                        .child(
                            div()
                                .w(px(260.))
                                .child(Input::new(&bar.query_input).small()),
                        )
                        .child(
                            toggle("find-case", "Aa", "Match case", q.match_case).on_click(
                                cx.listener(|ed, _, w, cx| {
                                    ed.toggle_option(|q| q.match_case = !q.match_case, w, cx)
                                }),
                            ),
                        )
                        .child(
                            toggle("find-word", "W", "Whole word", q.whole_word).on_click(
                                cx.listener(|ed, _, w, cx| {
                                    ed.toggle_option(|q| q.whole_word = !q.whole_word, w, cx)
                                }),
                            ),
                        )
                        .child(
                            toggle("find-regex", ".*", "Regular expression", q.regex).on_click(
                                cx.listener(|ed, _, w, cx| {
                                    ed.toggle_option(|q| q.regex = !q.regex, w, cx)
                                }),
                            ),
                        )
                        .child(
                            Button::new("find-prev")
                                .label("Previous")
                                .xsmall()
                                .ghost()
                                .tooltip(crate::keys::label("⇧⌘G"))
                                .on_click(
                                    cx.listener(|ed, _, w, cx| ed.find_step(false, false, w, cx)),
                                ),
                        )
                        .child(
                            Button::new("find-next")
                                .label("Next")
                                .xsmall()
                                .ghost()
                                .tooltip(crate::keys::label("⌘G"))
                                .on_click(
                                    cx.listener(|ed, _, w, cx| ed.find_step(true, false, w, cx)),
                                ),
                        )
                        .child(
                            Button::new("find-all")
                                .label("All")
                                .xsmall()
                                .ghost()
                                .tooltip("Select every match")
                                .on_click(cx.listener(|ed, _, w, cx| ed.select_every(w, cx))),
                        )
                        .child(
                            div()
                                .id("find-status")
                                .test_support()
                                .role(Role::Label)
                                .aria_label(status.clone())
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child(status),
                        )
                        .child(div().flex_1())
                        .child(
                            Button::new("find-close")
                                .label("Done")
                                .xsmall()
                                .ghost()
                                .tooltip("Esc")
                                .on_click(cx.listener(|ed, _, w, cx| ed.close_find(w, cx))),
                        ),
                )
                .child(
                    h_flex()
                        .gap_1()
                        .child(
                            div()
                                .w(px(260.))
                                .child(Input::new(&bar.replace_input).small()),
                        )
                        .child(
                            Button::new("replace-one")
                                .label("Replace")
                                .xsmall()
                                .ghost()
                                .on_click(cx.listener(|ed, _, w, cx| ed.replace_one(w, cx))),
                        )
                        .child(
                            Button::new("replace-all")
                                .label("Replace all")
                                .xsmall()
                                .ghost()
                                .on_click(cx.listener(|ed, _, _, cx| ed.replace_every(cx))),
                        ),
                )
                .into_any_element(),
        )
    }
}

gpui_kit::actions!(note_find, [CloseFind]);
