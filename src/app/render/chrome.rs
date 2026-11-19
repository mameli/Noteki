//! The title bar and the status bar.

use super::*;

impl NotekiApp {
    pub(in crate::app) fn render_title_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (can_back, can_fwd) = self
            .tabs
            .active()
            .map_or((false, false), |t| (t.can_go_back(), t.can_go_forward()));
        let vault_label = self
            .vault
            .as_ref()
            .and_then(|v| v.root.file_name())
            .map_or_else(
                || "Open Vault…".to_string(),
                |n| n.to_string_lossy().to_string(),
            );

        let icon_button = |id: &'static str, icon: AssetIcon, tip: &'static str| {
            Button::new(id)
                .icon(Icon::new(icon))
                .ghost()
                .small()
                .tooltip(crate::keys::tooltip(tip))
        };

        TitleBar::new().child(
            h_flex()
                .w_full()
                .items_center()
                .justify_between()
                .pr_2()
                .child(
                    h_flex()
                        .items_center()
                        .gap_1()
                        .child(
                            icon_button(
                                "toggle-left",
                                AssetIcon::PanelLeft,
                                "Toggle sidebar (⌃⌘S)",
                            )
                            .on_click(
                                cx.listener(|this, _, w, cx| this.toggle_left_sidebar(w, cx)),
                            ),
                        )
                        .child(
                            icon_button("nav-back", AssetIcon::ArrowLeft, "Back (⌥⌘←)")
                                .disabled(!can_back)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.navigate(true, window, cx)
                                })),
                        )
                        .child(
                            icon_button("nav-forward", AssetIcon::ArrowRight, "Forward (⌥⌘→)")
                                .disabled(!can_fwd)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.navigate(false, window, cx)
                                })),
                        )
                        .child(
                            Button::new("vault")
                                .label(vault_label)
                                .icon(Icon::new(AssetIcon::FolderOpen))
                                .ghost()
                                .small()
                                .tooltip(crate::keys::tooltip("Open Vault (⇧⌘O)"))
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.pick_vault(window, cx)),
                                ),
                        ),
                )
                .child(
                    h_flex()
                        .items_center()
                        .gap_1()
                        .child(
                            Button::new("quick-open")
                                .label("Quick Open")
                                .icon(Icon::new(AssetIcon::Search))
                                .ghost()
                                .small()
                                .tooltip(crate::keys::tooltip("Quick switcher (⌘O)"))
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.open_quick_switcher(OpenIn::CurrentTab, window, cx)
                                })),
                        )
                        .child(
                            icon_button(
                                "command-palette",
                                AssetIcon::Command,
                                "Command palette (⇧⌘P)",
                            )
                            .on_click(cx.listener(
                                |this, _, window, cx| this.open_command_palette(window, cx),
                            )),
                        ),
                )
                .child(
                    h_flex()
                        .items_center()
                        .gap_1()
                        .child(
                            icon_button("settings", AssetIcon::Settings, "Settings (⌘,)").on_click(
                                cx.listener(|this, _, window, cx| this.open_settings(window, cx)),
                            ),
                        )
                        .child(
                            icon_button(
                                "toggle-right",
                                AssetIcon::PanelRight,
                                "Toggle inspector (⌥⌘0)",
                            )
                            .on_click(cx.listener(|this, _, _, cx| this.toggle_right_sidebar(cx))),
                        ),
                ),
        )
    }

    pub(in crate::app) fn render_status_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let (muted, danger) = (theme.muted_foreground, theme.danger);
        let doc = self.active_path().and_then(|p| self.docs.get(p));
        let state = match doc {
            Some(d) if d.has_conflict() => "Conflict",
            Some(d) if d.is_deleted() => "Deleted on disk",
            Some(d) if d.is_dirty() => "Edited",
            Some(_) => "Saved",
            None => "",
        };
        let words = doc.map(|d| d.text().split_whitespace().count());
        let message = self.status.as_ref().map(|m| (m.text.clone(), m.is_error));
        let label: SharedString = message
            .as_ref()
            .map(|(t, _)| t.clone())
            .unwrap_or_else(|| state.to_string())
            .into();
        h_flex()
            .id("status-bar")
            .w_full()
            .h(STATUS_BAR_HEIGHT)
            .px_3()
            .gap_3()
            .items_center()
            .justify_between()
            .text_xs()
            .border_t_1()
            .border_color(theme.status_bar_border)
            .bg(theme.status_bar)
            .text_color(muted)
            .child(
                div()
                    .id("status")
                    .role(Role::Status)
                    .test_support()
                    .aria_label(label.clone())
                    .truncate()
                    .when(message.as_ref().is_some_and(|(_, e)| *e), |d| {
                        d.text_color(danger)
                    })
                    .child(label),
            )
            .child(
                h_flex()
                    .gap_3()
                    .when(message.is_some() && !state.is_empty(), |r| r.child(state))
                    .when_some(words, |r, w| r.child(format!("{w} words")))
                    .when_some(self.vault.as_ref(), |r, v| {
                        r.child(format!("{} notes", v.notes.len()))
                    }),
            )
    }
}
