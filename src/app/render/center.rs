//! The middle of the window: the tab bar, the banners and what the Tab in
//! front shows (the editor, an attachment, the welcome page).

use super::*;

impl NotekiApp {
    pub(in crate::app) fn render_center(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.vault.is_none() {
            return self.render_welcome(cx).into_any_element();
        }
        let content: AnyElement = match (self.active_path().map(str::to_string), self.active_kind())
        {
            (Some(path), Some(TabKind::Attachment(kind))) => {
                self.render_attachment(&path, kind, cx).into_any_element()
            }
            (Some(_), Some(TabKind::Note)) => self.render_editor(cx),
            _ => self.render_no_tab(cx).into_any_element(),
        };
        v_flex()
            .size_full()
            .child(Part::cached(&self.parts.tabs, cx))
            .child(self.render_banners(cx))
            .child(div().flex_1().min_h_0().child(content))
            .into_any_element()
    }

    pub(in crate::app) fn render_tab_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();
        let mut bar = TabBar::new("tabs")
            .selected_index(self.tabs.active_index().unwrap_or(0))
            .on_click(
                cx.listener(|this, ix: &usize, window, cx| this.activate_tab(*ix, window, cx)),
            );
        for (ix, tab) in self.tabs.iter().enumerate() {
            let doc = self.docs.get(&tab.path);
            let mut label = if tab.path.ends_with(".md") {
                note_name(&tab.path)
            } else {
                tab.path.rsplit('/').next().unwrap_or(&tab.path).to_string()
            };
            // No mark while typing: the Note is saved a second after, and a dot
            // that comes and goes at each pause only distracts.
            if doc.is_some_and(|d| d.has_conflict() || d.is_deleted()) {
                label = format!("⚠ {label}");
            }
            let (entity, middle, path) = (entity.clone(), entity.clone(), tab.path.clone());
            let menu_app = entity.clone();
            let label = div()
                .id(("tab-label", ix))
                .test_support()
                .child(label)
                .context_menu(move |menu, _, _| tab_menu(menu, &menu_app, ix, path.clone()));
            bar = bar.child(
                Tab::new()
                    .child(label)
                    // A middle click closes the Tab, as in a browser.
                    .on_mouse_down(MouseButton::Middle, move |_, window, cx| {
                        cx.stop_propagation();
                        middle.update(cx, |app, cx| app.close_tab(ix, window, cx));
                    })
                    .suffix(
                        Button::new(("close-tab", ix))
                            .icon(Icon::new(AssetIcon::X))
                            .xsmall()
                            .ghost()
                            .on_click(move |_, window, cx| {
                                cx.stop_propagation();
                                entity.update(cx, |app, cx| app.close_tab(ix, window, cx));
                            }),
                    ),
            );
        }
        h_flex()
            .w_full()
            .h(TAB_BAR_HEIGHT)
            .bg(cx.theme().tab_bar)
            .border_b_1()
            .border_color(cx.theme().border)
            .child(div().flex_1().min_w_0().child(bar))
            .child(
                div().px_1().child(
                    Button::new("new-tab")
                        .icon(Icon::new(AssetIcon::Plus))
                        .xsmall()
                        .ghost()
                        .tooltip(crate::keys::tooltip("New tab (⌘T)"))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.open_quick_switcher(OpenIn::NewTab, window, cx)
                        })),
                ),
            )
    }

    pub(in crate::app) fn render_banners(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(path) = self.active_path().map(str::to_string) else {
            return div().into_any_element();
        };
        let Some(doc) = self.docs.get(&path) else {
            return div().into_any_element();
        };
        let (danger, warning) = (cx.theme().danger, cx.theme().warning);
        let banner = |id: &'static str, color: Hsla, text: &'static str| {
            h_flex()
                .id(id)
                .test_support()
                .w_full()
                .px_3()
                .py_1p5()
                .gap_2()
                .items_center()
                .justify_between()
                .bg(faded(color, 0.14))
                .border_b_1()
                .border_color(color)
                .child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .child(
                            Icon::new(AssetIcon::TriangleAlert)
                                .size_4()
                                .text_color(color),
                        )
                        .child(div().text_sm().child(text)),
                )
        };
        if doc.has_conflict() {
            let (p1, p2) = (path.clone(), path);
            banner(
                "conflict-banner",
                danger,
                "This note changed on disk and here, on the same lines.",
            )
            .child(
                h_flex()
                    .gap_1p5()
                    .child(
                        Button::new("keep-mine")
                            .label("Keep mine")
                            .xsmall()
                            .primary()
                            .on_click(cx.listener(move |this, _, _, cx| this.keep_mine(&p1, cx))),
                    )
                    .child(
                        Button::new("take-theirs")
                            .label("Take theirs")
                            .xsmall()
                            .outline()
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.take_theirs(&p2, window, cx)
                            })),
                    ),
            )
            .into_any_element()
        } else if doc.is_deleted() {
            banner(
                "deleted-banner",
                warning,
                "This note was deleted by another app. Its text is only here.",
            )
            .child(
                Button::new("save-again")
                    .label("Save again")
                    .xsmall()
                    .primary()
                    .on_click(cx.listener(move |this, _, _, cx| this.save_again(&path, cx))),
            )
            .into_any_element()
        } else {
            div().into_any_element()
        }
    }

    pub(in crate::app) fn render_editor(&self, cx: &mut Context<Self>) -> AnyElement {
        let bar = self.settings.show_format_bar.then(|| {
            let (a, b) = (cx.entity(), cx.entity());
            super::format_bar::format_bar(
                std::rc::Rc::new(move |f, w, cx| a.update(cx, |app, cx| app.format(f, w, cx))),
                Some(std::rc::Rc::new(move |w, cx| {
                    b.update(cx, |app, cx| app.pick_attachments(w, cx))
                })),
                cx,
            )
        });
        v_flex()
            .id("editor-pane")
            .test_support()
            .size_full()
            .children(bar)
            .child(div().flex_1().min_h_0().child(self.editor.clone()))
            .into_any_element()
    }

    pub(in crate::app) fn render_attachment(
        &self,
        path: &str,
        kind: AttachmentKind,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let muted = cx.theme().muted_foreground;
        let Some(abs) = self
            .vault
            .as_ref()
            .and_then(|v| resolve_vault_path(&v.root, path).ok())
        else {
            return div().into_any_element();
        };
        let caption = div().text_xs().text_color(muted).child(path.to_string());
        let body = v_flex()
            .id("attachment")
            .size_full()
            .items_center()
            .justify_center()
            .gap_3()
            .p_6();
        match kind {
            AttachmentKind::Image => body
                .child(gpui_kit::img(abs).max_w_full().max_h(px(720.)))
                .child(caption)
                .into_any_element(),
            AttachmentKind::Pdf => body
                .child(Icon::new(AssetIcon::FileText).size_8().text_color(muted))
                .child(caption)
                .child(
                    Button::new("open-pdf")
                        .label("Open in Preview")
                        .primary()
                        .on_click(move |_, _, cx| {
                            cx.open_url(&format!("file://{}", abs.display()))
                        }),
                )
                .into_any_element(),
        }
    }

    pub(in crate::app) fn render_no_tab(&self, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .gap_3()
            .child(
                div()
                    .text_color(cx.theme().muted_foreground)
                    .child("No note open"),
            )
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("empty-new-note")
                            .label("New Note")
                            .primary()
                            .on_click(cx.listener(|this, _, window, cx| {
                                let folder = this.target_folder(cx);
                                this.open_name_prompt(
                                    PromptKind::NewNote,
                                    String::new(),
                                    Some(folder),
                                    window,
                                    cx,
                                )
                            })),
                    )
                    .child(
                        Button::new("empty-quick-open")
                            .label("Quick Open")
                            .outline()
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.open_quick_switcher(OpenIn::CurrentTab, window, cx)
                            })),
                    ),
            )
    }

    pub(in crate::app) fn render_welcome(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let demo = find_demo_vault();
        v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .gap_4()
            .child(div().text_2xl().font_bold().child("Noteki"))
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child("Open the folder of a Markdown vault kept with Obsidian."),
            )
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("welcome-open")
                            .label("Open Vault…")
                            .icon(Icon::new(AssetIcon::FolderOpen))
                            .primary()
                            .on_click(
                                cx.listener(|this, _, window, cx| this.pick_vault(window, cx)),
                            ),
                    )
                    .when_some(demo, |row, demo| {
                        row.child(
                            Button::new("welcome-demo")
                                .label("Open Demo Vault")
                                .outline()
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    let _ = this.open_vault(&demo, window, cx);
                                })),
                        )
                    }),
            )
    }
}
