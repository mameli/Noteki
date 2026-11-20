//! The right sidebar: Outline and Backlinks.

use super::*;

impl NotekiApp {
    pub(in crate::app) fn render_right_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (bg, fg, border, muted) = (
            cx.theme().sidebar,
            cx.theme().sidebar_foreground,
            cx.theme().sidebar_border,
            cx.theme().muted_foreground,
        );
        let pane = self.settings.right_pane;
        let backlinks_label = match self.backlinks.len() {
            0 => "Backlinks".to_string(),
            n => format!("Backlinks ({n})"),
        };
        let tab = |id: &'static str,
                   label: String,
                   tip: &'static str,
                   icon: AssetIcon,
                   p: RightPane,
                   cx: &mut Context<Self>| {
            let b = Button::new(id)
                .label(label)
                .icon(Icon::new(icon))
                .xsmall()
                .tooltip(crate::keys::tooltip(tip));
            let b = if pane == p { b.primary() } else { b.ghost() };
            b.on_click(cx.listener(move |this, _, _, cx| this.show_right_pane(p, cx)))
        };
        let header = h_flex()
            .px_2()
            .py_1()
            .gap_1()
            .border_b_1()
            .border_color(border)
            .child(tab(
                "pane-outline",
                "Outline".into(),
                "Outline (⌥⌘1)",
                AssetIcon::List,
                RightPane::Outline,
                cx,
            ))
            .child(tab(
                "pane-backlinks",
                backlinks_label,
                "Backlinks (⌥⌘2)",
                AssetIcon::Link,
                RightPane::Backlinks,
                cx,
            ));

        let empty = |text: &'static str| {
            div()
                .p_2()
                .text_sm()
                .text_color(muted)
                .child(text)
                .into_any_element()
        };
        let body: AnyElement = match pane {
            RightPane::Outline if self.outline.is_empty() => empty("No headings"),
            // One row per Heading or Backlink, all as tall: only those in view
            // are drawn and laid out, at every frame of typing too.
            RightPane::Outline => {
                let entity = cx.entity();
                uniform_list("outline-list", self.outline.len(), move |range, _, cx| {
                    let outline = &entity.read(cx).outline;
                    range
                        .map(|ix| {
                            let h = &outline[ix];
                            let line = h.line;
                            let entity = entity.clone();
                            ListItem::new(("heading", ix))
                                .h(px(24.))
                                .rounded(px(5.))
                                .pl(px(8. + (h.level.saturating_sub(1)) as f32 * 12.))
                                .pr_2()
                                .child(div().text_sm().truncate().child(h.text.clone()))
                                .on_click(move |_, window, cx| {
                                    entity
                                        .update(cx, |this, cx| this.jump_to_line(line, window, cx))
                                })
                        })
                        .collect::<Vec<_>>()
                })
                .track_scroll(&self.inspector_scroll)
                .size_full()
                .into_any_element()
            }
            RightPane::Backlinks if self.backlinks.is_empty() => empty("No other note links here"),
            RightPane::Backlinks => {
                let entity = cx.entity();
                uniform_list(
                    "backlink-list",
                    self.backlinks.len(),
                    move |range, _, cx| {
                        let muted = cx.theme().muted_foreground;
                        let backlinks = &entity.read(cx).backlinks;
                        range
                            .map(|ix| {
                                let b = &backlinks[ix];
                                let (path, line) = (b.source_path.clone(), b.line);
                                let item = ListItem::new(("backlink", ix))
                                    .h(px(52.))
                                    .rounded(px(6.))
                                    .px_2()
                                    .child(
                                        v_flex()
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .font_semibold()
                                                    .truncate()
                                                    .child(b.source_name.clone()),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(muted)
                                                    .truncate()
                                                    .child(b.snippet.clone()),
                                            ),
                                    );
                                opens_note(item, entity.clone(), path, Some(line))
                            })
                            .collect::<Vec<_>>()
                    },
                )
                .track_scroll(&self.inspector_scroll)
                .size_full()
                .into_any_element()
            }
        };
        v_flex()
            .size_full()
            .bg(bg)
            .text_color(fg)
            .child(header)
            .child(
                div()
                    .id("inspector")
                    .flex_1()
                    .min_h_0()
                    .relative()
                    .p_2()
                    .child(body)
                    .vertical_scrollbar(&self.inspector_scroll),
            )
    }
}
