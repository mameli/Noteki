//! The left sidebar: Files, Tags and Search.

use super::*;

impl NotekiApp {
    pub(in crate::app) fn render_left_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (bg, fg, border) = (
            cx.theme().sidebar,
            cx.theme().sidebar_foreground,
            cx.theme().sidebar_border,
        );
        let pane = self.settings.left_pane;
        let tab = |id: &'static str,
                   tip: &'static str,
                   icon: AssetIcon,
                   p: LeftPane,
                   cx: &mut Context<Self>| {
            let b = Button::new(id)
                .icon(Icon::new(icon))
                .small()
                .tooltip(crate::keys::tooltip(tip));
            let b = if pane == p { b.primary() } else { b.ghost() };
            // Search's button gives its field the keyboard, as ⇧⌘F does.
            b.on_click(cx.listener(move |this, _, w, cx| match p {
                LeftPane::Search => this.show_search(w, cx),
                _ => this.show_left_pane(p, w, cx),
            }))
        };
        // In Files: a new Note, a new folder, and every folder opened or closed.
        let files_actions = (pane == LeftPane::Files && self.vault.is_some()).then(|| {
            let collapse = self.any_folder_expanded();
            let action = |id: &'static str, tip: &'static str, icon: AssetIcon| {
                Button::new(id)
                    .icon(Icon::new(icon))
                    .small()
                    .ghost()
                    .tooltip(crate::keys::tooltip(tip))
            };
            h_flex()
                .gap_0p5()
                .child(
                    action("files-new-note", "New Note (⌘N)", AssetIcon::FilePlus)
                        .on_click(|_, w, cx| w.dispatch_action(Box::new(NewNoteAction), cx)),
                )
                .child(
                    action("files-new-folder", "New Folder", AssetIcon::FolderPlus)
                        .on_click(|_, w, cx| w.dispatch_action(Box::new(NewFolderAction), cx)),
                )
                .child(
                    if collapse {
                        action(
                            "files-collapse-all",
                            "Collapse All",
                            AssetIcon::ChevronsDownUp,
                        )
                    } else {
                        action("files-expand-all", "Expand All", AssetIcon::ChevronsUpDown)
                    }
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.set_all_folders_expanded(!collapse, cx)
                    })),
                )
        });
        let tabs = h_flex()
            .justify_between()
            .px_2()
            .py_1()
            .gap_2()
            .border_b_1()
            .border_color(border)
            .child(
                h_flex()
                    .gap_2()
                    .child(tab(
                        "pane-files",
                        "Files (⌃⌘1)",
                        AssetIcon::Folder,
                        LeftPane::Files,
                        cx,
                    ))
                    .child(tab(
                        "pane-tags",
                        "Tags (⌃⌘2)",
                        AssetIcon::Tag,
                        LeftPane::Tags,
                        cx,
                    ))
                    .child(tab(
                        "pane-search",
                        "Search (⇧⌘F)",
                        AssetIcon::Search,
                        LeftPane::Search,
                        cx,
                    )),
            )
            .children(files_actions);
        let body: AnyElement = match pane {
            LeftPane::Files => self.render_files_pane(cx).into_any_element(),
            LeftPane::Tags => self.render_tags_pane(cx).into_any_element(),
            LeftPane::Search => self.render_search_pane(cx).into_any_element(),
        };
        v_flex()
            .size_full()
            .bg(bg)
            .text_color(fg)
            .child(tabs)
            .child(div().flex_1().min_h_0().child(body))
    }

    pub(in crate::app) fn render_files_pane(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (muted, primary) = (cx.theme().muted_foreground, cx.theme().primary);
        // The tree keeps its closures in `file_tree`, which this window owns:
        // they hold the window weakly, or a closed window is never freed.
        let entity = cx.entity().downgrade();
        let active = self.active_path().map(str::to_string);

        let empty = self.empty_folders.clone();
        let file_tree = tree(&self.file_tree, move |ix, entry, tree_selected, _, _| {
            let item = entry.item();
            let id = item.id.to_string();
            let is_folder = entry.is_folder() || empty.contains(&id);
            let icon = match (is_folder, entry.is_expanded()) {
                (true, true) => AssetIcon::ChevronDown,
                (true, false) => AssetIcon::ChevronRight,
                _ if id.ends_with(".md") => AssetIcon::FileText,
                _ if id.to_lowercase().ends_with(".pdf") => AssetIcon::File,
                _ => AssetIcon::Image,
            };
            let selected = tree_selected || active.as_deref() == Some(id.as_str());
            let entity = entity.upgrade();
            ListItem::new(("file", ix))
                .selected(selected)
                .rounded(px(5.))
                .py(px(2.))
                .pl(px(6. + entry.depth() as f32 * 14.))
                .pr_2()
                .child(
                    h_flex()
                        .gap_1p5()
                        .items_center()
                        .min_w_0()
                        .child(Icon::new(icon).size_3p5().text_color(if is_folder {
                            primary
                        } else {
                            muted
                        }))
                        .child(
                            div()
                                .flex_1()
                                .truncate()
                                .text_sm()
                                .child(item.label.clone()),
                        ),
                )
                // Folders expand through the tree itself; only files open on click.
                .when_some(entity.filter(|_| !is_folder), |li, entity| {
                    opens_note(li, entity, id, None)
                })
        });

        let menu_app = cx.entity().downgrade();
        let empty = self.empty_folders.clone();
        let file_tree = file_tree.context_menu(move |_, entry, menu, _, _| {
            let Some(app) = menu_app.upgrade() else {
                return menu;
            };
            let path = entry.item().id.to_string();
            let is_folder = entry.is_folder() || empty.contains(&path);
            file_menu(menu, &app, path, is_folder)
        });
        v_flex().size_full().child(
            div()
                .id("file-tree")
                .flex_1()
                .min_h_0()
                .p_1()
                .child(file_tree),
        )
    }

    pub(in crate::app) fn render_tags_pane(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (primary, muted, border) = (
            cx.theme().primary,
            cx.theme().muted_foreground,
            cx.theme().border,
        );
        let mut flat = Vec::new();
        if let Some(v) = &self.vault {
            flatten_tags(&v.tag_tree, 0, &mut flat);
        }
        let selected_notes: Vec<String> = self
            .selected_tag
            .as_deref()
            .and_then(|sel| {
                self.vault
                    .as_ref()
                    .and_then(|v| find_tag_notes(&v.tag_tree, sel))
            })
            .unwrap_or_default();

        let rows: AnyElement = if flat.is_empty() {
            div()
                .p_2()
                .text_sm()
                .text_color(muted)
                .child("No tags in this vault")
                .into_any_element()
        } else {
            v_flex()
                .gap(px(1.))
                .children(flat.into_iter().enumerate().map(|(ix, (depth, node))| {
                    let full = node.full_lower.clone();
                    ListItem::new(("tag", ix))
                        .selected(self.selected_tag.as_deref() == Some(full.as_str()))
                        .rounded(px(5.))
                        .py(px(2.))
                        .pl(px(8. + depth as f32 * 14.))
                        .pr_2()
                        .child(
                            h_flex()
                                .w_full()
                                .justify_between()
                                .child(
                                    h_flex()
                                        .gap_1p5()
                                        .items_center()
                                        .child(
                                            Icon::new(AssetIcon::Tag)
                                                .size_3p5()
                                                .text_color(primary),
                                        )
                                        .child(div().text_sm().child(format!("#{}", node.label))),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(muted)
                                        .child(node.count.to_string()),
                                ),
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.selected_tag =
                                if this.selected_tag.as_deref() == Some(full.as_str()) {
                                    None
                                } else {
                                    Some(full.clone())
                                };
                            cx.notify();
                        }))
                }))
                .into_any_element()
        };

        v_flex()
            .size_full()
            .child(
                div()
                    .id("tags-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scrollbar()
                    .p_2()
                    .child(rows),
            )
            .when(!selected_notes.is_empty(), |col| {
                col.child(div().h(px(1.)).bg(border)).child(
                    div()
                        .id("tag-notes")
                        .max_h(px(240.))
                        .overflow_y_scrollbar()
                        .p_2()
                        .child(v_flex().gap(px(1.)).children(
                            selected_notes.into_iter().enumerate().map(|(ix, path)| {
                                let name = note_name(&path);
                                let item = ListItem::new(("tag-note", ix))
                                    .rounded(px(5.))
                                    .py(px(2.))
                                    .px_2()
                                    .child(div().text_sm().truncate().child(name));
                                opens_note(item, cx.entity(), path, None)
                            }),
                        )),
                )
            })
    }

    pub(in crate::app) fn render_search_pane(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (border, muted) = (cx.theme().sidebar_border, cx.theme().muted_foreground);
        let empty_query = self.search_input.read(cx).value().trim().is_empty();
        let body: AnyElement = if empty_query {
            div()
                .p_2()
                .text_xs()
                .text_color(muted)
                .child("Words, \"a phrase\", tag:work, path:Journal")
                .into_any_element()
        } else if self.search_results.is_empty() {
            div()
                .p_2()
                .text_sm()
                .text_color(muted)
                .child("No results")
                .into_any_element()
        } else {
            // One row per Note and per line found, all as tall: only the rows
            // in view are drawn, however many results (300 Notes, 10 lines each).
            let rows: Vec<(usize, Option<usize>)> = self
                .search_results
                .iter()
                .enumerate()
                .flat_map(|(ix, r)| {
                    std::iter::once((ix, None))
                        .chain((0..r.lines.len()).map(move |l| (ix, Some(l))))
                })
                .collect();
            let entity = cx.entity();
            uniform_list("search-list", rows.len(), move |range, _, cx| {
                let muted = cx.theme().muted_foreground;
                let results = &entity.read(cx).search_results;
                range
                    .map(|row| {
                        let (ix, line) = rows[row];
                        let r = &results[ix];
                        let item = ListItem::new(("search-row", row))
                            .h(px(24.))
                            .rounded(px(4.))
                            .pr_2();
                        match line {
                            None => {
                                let item = item.px_2().child(
                                    h_flex()
                                        .w_full()
                                        .justify_between()
                                        .child(
                                            div()
                                                .text_sm()
                                                .font_semibold()
                                                .truncate()
                                                .child(r.name.clone()),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(muted)
                                                .child(r.total_line_matches.to_string()),
                                        ),
                                );
                                opens_note(item, entity.clone(), r.path.clone(), None)
                            }
                            Some(l) => {
                                let m = &r.lines[l];
                                let item = item.pl_4().child(
                                    h_flex()
                                        .gap_1p5()
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(muted)
                                                .child(m.line.to_string()),
                                        )
                                        .child(div().text_xs().truncate().child(m.text.clone())),
                                );
                                opens_note(item, entity.clone(), r.path.clone(), Some(m.line))
                            }
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .track_scroll(&self.search_scroll)
            .size_full()
            .into_any_element()
        };
        v_flex()
            .size_full()
            .child(
                div().p_2().border_b_1().border_color(border).child(
                    Input::new(&self.search_input)
                        .id("search")
                        .cleanable(true)
                        .small(),
                ),
            )
            .child(
                div()
                    .id("search-results")
                    .flex_1()
                    .min_h_0()
                    .relative()
                    .p_2()
                    .child(body)
                    .vertical_scrollbar(&self.search_scroll),
            )
    }
}
