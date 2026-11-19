//! The mouse: clicks and drags select, ⌘-click follows a link, the view
//! scrolls at the edges while dragging, and the right-click menu.

use super::*;

impl NoteEditor {
    /// What the mouse selects: what is under it when it went down, and the unit
    /// it goes on selecting by while it moves (a double-click: words).
    pub(super) fn unit_at(unit: ClickUnit, text: &str, offset: usize) -> Range<usize> {
        match unit {
            ClickUnit::Char => offset..offset,
            ClickUnit::Word => word_range_at(text, offset),
            ClickUnit::Line => line_range_at(text, offset),
        }
    }

    pub(super) fn on_mouse_down(
        &mut self,
        ev: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus_handle, cx);
        let Some(layout) = &self.layout else {
            return;
        };
        // SPEC: a click on a checkbox writes `[x]` or `[ ]` and leaves the
        // cursor where it was.
        if let Some((bracket, checked)) = layout.checkbox_at(ev.position) {
            let sel = self.buf.selection();
            self.buf.close_undo_step();
            let changed = self
                .buf
                .replace(bracket + 1..bracket + 2, if checked { " " } else { "x" });
            self.buf.set_selection(sel.anchor, sel.head);
            self.edited(changed, cx);
            self.reveal = false;
            return;
        }
        let offset = layout.offset_for_point(self.buf.text(), ev.position);
        if ev.click_count == 1 && !ev.modifiers.shift {
            if let Some(link) = self.link_to_open(offset) {
                cx.emit(NoteEditorEvent::OpenLink {
                    link,
                    new_tab: ev.modifiers.secondary(cx),
                });
                return;
            }
            // A footnote goes to its definition, a definition to its reference
            // (unless the cursor is in it: then the click only moves it).
            let c = self.buf.cursor();
            let editing = markdown::footnote_target(self.buf.text(), c).is_some()
                && markdown::footnote_target(self.buf.text(), offset)
                    == markdown::footnote_target(self.buf.text(), c);
            if let Some(target) = markdown::footnote_target(self.buf.text(), offset) {
                if !editing {
                    self.buf.move_to(target, false);
                    self.goal_x = None;
                    self.wake(cx);
                    return;
                }
            }
        }
        let unit = match ev.click_count {
            0 | 1 => ClickUnit::Char,
            2 => ClickUnit::Word,
            _ => ClickUnit::Line,
        };
        let origin = if unit == ClickUnit::Char && ev.modifiers.shift {
            // ⇧-click extends what is selected.
            let a = self.buf.selection().anchor;
            a..a
        } else {
            Self::unit_at(unit, self.buf.text(), offset)
        };
        self.drag = Some(Drag {
            unit,
            origin,
            pointer: ev.position,
        });
        self.select_to(offset);
        self.goal_x = None;
        self.wake(cx);
        self.reveal = false;
        self._edge_scroll = Some(cx.spawn_in(window, async move |this, cx| loop {
            cx.background_executor().timer(EDGE_SCROLL_TICK).await;
            match this.update(cx, |ed, cx| ed.edge_scroll(cx)) {
                Ok(true) => {}
                _ => break,
            }
        }));
    }

    /// The link a click at `offset` opens: one whose text is there, unless the
    /// cursor is in it already (its marks show: the user is editing it).
    pub(super) fn link_to_open(&self, offset: usize) -> Option<Link> {
        let (range, link) = link_at(self.buf.text(), offset)?;
        let c = self.buf.cursor();
        (!(range.start <= c && c <= range.end)).then_some(link)
    }

    /// Shows a pointing hand over a link a click would open.
    pub(super) fn on_hover(&mut self, ev: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.drag.is_some() {
            return;
        }
        // What a click acts on (a link, a checkbox) shows the pointing hand.
        let over = self.layout.as_ref().is_some_and(|l| {
            l.checkbox_at(ev.position).is_some() || {
                let offset = l.offset_for_point(self.buf.text(), ev.position);
                self.link_to_open(offset).is_some()
            }
        });
        if over != self.over_clickable {
            self.over_clickable = over;
            cx.notify();
        }
    }

    /// Selects from where the drag began to `offset`, by the drag's unit.
    pub(super) fn select_to(&mut self, offset: usize) {
        let Some(drag) = &self.drag else {
            return;
        };
        let here = Self::unit_at(drag.unit, self.buf.text(), offset);
        let o = drag.origin.clone();
        if here.start < o.start {
            self.buf.set_selection(o.end, here.start);
        } else {
            self.buf.set_selection(o.start, here.end.max(o.end));
        }
    }

    /// While a selection is dragged, the pointer is followed anywhere in the
    /// window (the element listens for the whole window, see `paint`).
    pub(super) fn on_drag_move(&mut self, ev: &MouseMoveEvent, cx: &mut Context<Self>) {
        let Some(drag) = &mut self.drag else {
            return;
        };
        drag.pointer = ev.position;
        if let Some(layout) = &self.layout {
            let offset = layout.offset_for_point(self.buf.text(), ev.position);
            self.select_to(offset);
            self.wake(cx);
            self.reveal = false;
        }
    }

    pub(super) fn on_drag_end(&mut self, _: &MouseUpEvent, _: &mut Context<Self>) {
        if self.drag.is_none() {
            return;
        }
        self.drag = None;
        self._edge_scroll = None;
    }

    /// One frame of scrolling while a selection is dragged past the top or
    /// bottom of the view; `false` once the drag is over.
    pub(super) fn edge_scroll(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(drag) = &self.drag else {
            return false;
        };
        let pointer = drag.pointer;
        let view = self.scroll.bounds();
        let step = edge_scroll(
            f32::from(pointer.y),
            f32::from(view.top()),
            f32::from(view.bottom()),
        );
        if step == 0. {
            return true;
        }
        let (offset, max) = (self.scroll.offset(), self.scroll.max_offset());
        let y = (offset.y - px(step)).clamp(-max.y, px(0.));
        if y != offset.y {
            self.scroll.set_offset(point(offset.x, y));
            // The text moved under the pointer by what was scrolled.
            if let Some(layout) = &self.layout {
                let under = point(pointer.x, pointer.y - (y - offset.y));
                let o = layout.offset_for_point(self.buf.text(), under);
                self.select_to(o);
            }
            cx.notify();
        }
        true
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ClickUnit {
    Char,
    Word,
    Line,
}

pub(super) struct Drag {
    unit: ClickUnit,
    /// What the click selected, which the drag always keeps selected.
    origin: Range<usize>,
    /// Where the pointer is, in window coordinates.
    pointer: Point<Pixels>,
}

/// The editor's right-click menu: a misspelled word's guesses, *Ignore* and
/// *Learn Spelling*, then Cut, Copy and Paste.
pub(super) fn context_menu(
    editor: &Entity<NoteEditor>,
    menu: gpui_kit::component::menu::PopupMenu,
    cx: &mut App,
) -> gpui_kit::component::menu::PopupMenu {
    use gpui_kit::component::menu::PopupMenuItem;
    let word = editor.update(cx, |ed, _| {
        let at = ed.context_at?;
        let range = ed.misspelled_at(at)?;
        let guesses = spelling::guesses(ed.buf.text(), range.clone());
        Some((range.clone(), ed.buf.text()[range].to_string(), guesses))
    });
    let mut menu = menu;
    if let Some((range, word, guesses)) = word {
        if guesses.is_empty() {
            menu = menu.item(PopupMenuItem::new("No Guesses Found").disabled(true));
        }
        for guess in guesses.into_iter().take(6) {
            let (e, r) = (editor.clone(), range.clone());
            menu = menu.item(PopupMenuItem::new(guess.clone()).on_click(move |_, _, cx| {
                e.update(cx, |ed, cx| ed.replace_word(r.clone(), &guess, cx))
            }));
        }
        let (e1, e2, w1, w2) = (editor.clone(), editor.clone(), word.clone(), word);
        menu = menu
            .separator()
            .item(
                PopupMenuItem::new("Ignore Spelling").on_click(move |_, _, cx| {
                    spelling::ignore(&w1);
                    e1.update(cx, |ed, cx| ed.forget_spelling(cx));
                }),
            )
            .item(
                PopupMenuItem::new("Learn Spelling").on_click(move |_, _, cx| {
                    spelling::learn(&w2);
                    e2.update(cx, |ed, cx| ed.forget_spelling(cx));
                }),
            )
            .separator();
    }
    menu.menu("Cut", Box::new(Cut))
        .menu("Copy", Box::new(Copy))
        .menu("Paste", Box::new(Paste))
}
