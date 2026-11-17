//! Drawing of the Noteki window. Element IDs are stable so UI tests can find them.

use gpui_kit::assets::IconName as AssetIcon;
use gpui_kit::base::Disableable as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::command::{Command, CommandItem, CommandState};
use gpui_kit::component::dialog::DialogButtonProps;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::list::ListItem;
use gpui_kit::component::resizable::{h_resizable, resizable_panel};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::tab::{Tab, TabBar};
use gpui_kit::component::tree::tree;
use gpui_kit::component::{
    h_flex, v_flex, ActiveTheme as _, Icon, Sizable as _, StyledExt as _, TitleBar, WindowExt as _,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    div, hsla, px, uniform_list, AnyElement, AppContext as _, ClickEvent, Context, Hsla,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, Role, SharedString,
    StatefulInteractiveElement as _, Styled as _, TestSupportExt as _, Window,
};

use super::*;
use crate::core::vault::resolve_vault_path;
use crate::core::wikilink::note_name;
use gpui_kit::component::menu::{ContextMenuExt as _, PopupMenu, PopupMenuItem};
use gpui_kit::{MouseButton, NavigationDirection};

impl Render for NotekiApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let s = &self.settings;
        let mut split = h_resizable("workspace");
        if s.left_sidebar_open {
            split = split.child(
                resizable_panel()
                    .size(px(260.))
                    .size_range(px(180.)..px(480.))
                    .child(self.render_left_sidebar(cx)),
            );
        }
        split = split.child(
            resizable_panel()
                .size_range(px(320.)..px(10_000.))
                .child(self.render_center(cx)),
        );
        if s.right_sidebar_open {
            split = split.child(
                resizable_panel()
                    .size(px(250.))
                    .size_range(px(180.)..px(480.))
                    .child(self.render_right_sidebar(cx)),
            );
        }

        div()
            .id("noteki")
            .track_focus(&self.focus_handle)
            .size_full()
            .flex()
            .flex_col()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .on_action(
                cx.listener(|this, _: &ToggleLeftSidebar, w, cx| this.toggle_left_sidebar(w, cx)),
            )
            .on_action(
                cx.listener(|this, _: &ToggleRightSidebar, _, cx| this.toggle_right_sidebar(cx)),
            )
            .on_action(cx.listener(|this, _: &SwitchFilesPane, window, cx| {
                this.show_left_pane(LeftPane::Files, window, cx)
            }))
            .on_action(cx.listener(|this, _: &SwitchTagsPane, window, cx| {
                this.show_left_pane(LeftPane::Tags, window, cx)
            }))
            .on_action(cx.listener(|this, _: &ShowOutlineAction, _, cx| {
                this.show_right_pane(RightPane::Outline, cx)
            }))
            .on_action(cx.listener(|this, _: &ShowBacklinksAction, _, cx| {
                this.show_right_pane(RightPane::Backlinks, cx)
            }))
            .on_action(
                cx.listener(|this, _: &SwitchSearchPane, window, cx| this.show_search(window, cx)),
            )
            .on_action(
                cx.listener(|this, _: &SearchInVaultAction, window, cx| {
                    this.show_search(window, cx)
                }),
            )
            .on_action(cx.listener(|this, _: &TrashSelectedAction, window, cx| {
                if let Some(path) = this.selected_in_files(cx) {
                    let r = this.trash(&path, window, cx);
                    this.report(r);
                    cx.notify();
                }
            }))
            .on_action(cx.listener(|this, _: &OpenQuickSwitcher, window, cx| {
                this.open_quick_switcher(OpenIn::CurrentTab, window, cx)
            }))
            .on_action(
                cx.listener(|this, _: &OpenQuickSwitcherNewTab, window, cx| {
                    this.open_quick_switcher(OpenIn::NewTab, window, cx)
                }),
            )
            .on_action(cx.listener(|this, _: &OpenCommandPalette, window, cx| {
                this.open_command_palette(window, cx)
            }))
            .on_action(cx.listener(|this, _: &NewNoteAction, window, cx| {
                let folder = this.target_folder(cx);
                this.open_name_prompt(PromptKind::NewNote, String::new(), Some(folder), window, cx)
            }))
            .on_action(cx.listener(|this, _: &NewFolderAction, window, cx| {
                let folder = this.target_folder(cx);
                this.open_name_prompt(
                    PromptKind::NewFolder,
                    String::new(),
                    Some(folder),
                    window,
                    cx,
                )
            }))
            .on_action(cx.listener(|this, _: &NewWindowAction, _, cx| this.open_another_window(cx)))
            .on_action(cx.listener(|this, _: &OpenSettingsAction, window, cx| {
                this.open_settings(window, cx)
            }))
            .on_action(
                cx.listener(|this, _: &DailyNoteAction, window, cx| {
                    this.open_daily_note(window, cx)
                }),
            )
            .on_action(
                cx.listener(|this, _: &CloseTabAction, window, cx| {
                    this.close_active_tab(window, cx)
                }),
            )
            .on_action(cx.listener(|this, _: &ReopenClosedTabAction, window, cx| {
                this.reopen_closed_tab(window, cx)
            }))
            .on_action(
                cx.listener(|this, _: &NextTabAction, window, cx| this.cycle_tab(true, window, cx)),
            )
            .on_action(
                cx.listener(|this, _: &PrevTabAction, window, cx| {
                    this.cycle_tab(false, window, cx)
                }),
            )
            .on_action(cx.listener(|this, _: &ZoomIn, w, cx| this.zoom(Some(1.), w, cx)))
            .on_action(cx.listener(|this, _: &ZoomOut, w, cx| this.zoom(Some(-1.), w, cx)))
            .on_action(cx.listener(|this, _: &ZoomReset, w, cx| this.zoom(None, w, cx)))
            .on_action(cx.listener(|this, _: &NavigateBackAction, window, cx| {
                this.navigate(true, window, cx)
            }))
            // The mouse's side buttons go back and forward, anywhere in the window.
            .on_mouse_down(
                MouseButton::Navigate(NavigationDirection::Back),
                cx.listener(|this, _, window, cx| this.navigate(true, window, cx)),
            )
            .on_mouse_down(
                MouseButton::Navigate(NavigationDirection::Forward),
                cx.listener(|this, _, window, cx| this.navigate(false, window, cx)),
            )
            .on_action(cx.listener(|this, _: &NavigateForwardAction, window, cx| {
                this.navigate(false, window, cx)
            }))
            .on_action(
                cx.listener(|this, _: &OpenVaultAction, window, cx| this.pick_vault(window, cx)),
            )
            .on_action(
                cx.listener(|this, _: &CycleThemeAction, window, cx| this.cycle_theme(window, cx)),
            )
            .on_action(cx.listener(|this, _: &CycleAppearanceAction, window, cx| {
                this.cycle_appearance(window, cx)
            }))
            .on_action(cx.listener(|this, _: &FormatBoldAction, window, cx| {
                this.format(Format::Wrap(Wrap::Bold), window, cx)
            }))
            .on_action(cx.listener(|this, _: &FormatItalicAction, window, cx| {
                this.format(Format::Wrap(Wrap::Italic), window, cx)
            }))
            .on_action(cx.listener(|this, _: &FormatStrikeAction, window, cx| {
                this.format(Format::Wrap(Wrap::Strikethrough), window, cx)
            }))
            .on_action(cx.listener(|this, _: &FormatCodeAction, window, cx| {
                this.format(Format::Wrap(Wrap::Code), window, cx)
            }))
            .on_action(cx.listener(|this, _: &InsertWikilinkAction, window, cx| {
                this.open_wikilink_picker(window, cx)
            }))
            .on_action(
                cx.listener(|this, _: &FollowLinkUnderCursorAction, window, cx| {
                    this.follow_link_under_cursor(window, cx)
                }),
            )
            .child(self.render_title_bar(cx))
            .child(div().flex_1().min_h_0().child(split))
            .child(self.render_status_bar(cx))
    }
}

fn faded(c: Hsla, a: f32) -> Hsla {
    hsla(c.h, c.s, c.l, a)
}

impl NotekiApp {
    // ---- Chrome ----

    fn render_title_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
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
                .tooltip(tip)
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
                                .tooltip("Open Vault (⇧⌘O)")
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
                                .tooltip("Quick switcher (⌘O)")
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

    fn render_status_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
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
            .h(px(24.))
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

    // ---- Left sidebar ----

    fn render_left_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
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
            let b = Button::new(id).icon(Icon::new(icon)).small().tooltip(tip);
            let b = if pane == p { b.primary() } else { b.ghost() };
            b.on_click(cx.listener(move |this, _, w, cx| this.show_left_pane(p, w, cx)))
        };
        let tabs = h_flex()
            .justify_center()
            .px_2()
            .py_1()
            .gap_2()
            .border_b_1()
            .border_color(border)
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
            ));
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

    fn render_files_pane(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (border, muted, primary) = (
            cx.theme().sidebar_border,
            cx.theme().muted_foreground,
            cx.theme().primary,
        );
        let entity = cx.entity();
        let active = self.active_path().map(str::to_string);

        let button = |id: &'static str, icon: AssetIcon, tip: &'static str| {
            Button::new(id)
                .icon(Icon::new(icon))
                .xsmall()
                .ghost()
                .tooltip(tip)
        };
        let toolbar = h_flex()
            .px_2()
            .py_1()
            .gap_1()
            .border_b_1()
            .border_color(border)
            .child(
                button("new-note", AssetIcon::FilePlus, "New note (⌘N)").on_click(cx.listener(
                    |this, _, window, cx| {
                        let folder = this.target_folder(cx);
                        this.open_name_prompt(
                            PromptKind::NewNote,
                            String::new(),
                            Some(folder),
                            window,
                            cx,
                        )
                    },
                )),
            )
            .child(
                button("new-folder", AssetIcon::FolderPlus, "New folder").on_click(cx.listener(
                    |this, _, window, cx| {
                        let folder = this.target_folder(cx);
                        this.open_name_prompt(
                            PromptKind::NewFolder,
                            String::new(),
                            Some(folder),
                            window,
                            cx,
                        )
                    },
                )),
            )
            .child(
                button("daily-note", AssetIcon::CalendarDays, "Today's note (⇧⌘D)")
                    .on_click(cx.listener(|this, _, window, cx| this.open_daily_note(window, cx))),
            )
            .child(div().flex_1())
            .when_some(active.clone(), |row, path| {
                let (p1, p3) = (path.clone(), path);
                row.child(
                    button("rename-active", AssetIcon::Pencil, "Rename").on_click(cx.listener(
                        move |this, _, window, cx| {
                            let name = note_name(&p1);
                            this.open_name_prompt(
                                PromptKind::Rename,
                                name,
                                Some(p1.clone()),
                                window,
                                cx,
                            )
                        },
                    )),
                )
                .child(
                    button("trash-active", AssetIcon::Trash, "Move to Trash").on_click(
                        cx.listener(move |this, _, window, cx| {
                            let r = this.trash(&p3, window, cx);
                            this.report(r);
                            cx.notify();
                        }),
                    ),
                )
            });

        let file_tree = tree(&self.file_tree, move |ix, entry, tree_selected, _, _| {
            let item = entry.item();
            let id = item.id.to_string();
            let is_folder = entry.is_folder();
            let icon = match (is_folder, entry.is_expanded()) {
                (true, true) => AssetIcon::ChevronDown,
                (true, false) => AssetIcon::ChevronRight,
                _ if id.ends_with(".md") => AssetIcon::FileText,
                _ if id.to_lowercase().ends_with(".pdf") => AssetIcon::File,
                _ => AssetIcon::Image,
            };
            let selected = tree_selected || active.as_deref() == Some(id.as_str());
            let entity = entity.clone();
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
                .when(!is_folder, |li| opens_note(li, entity, id, None))
        });

        let menu_app = cx.entity();
        let file_tree = file_tree.context_menu(move |_, entry, menu, _, _| {
            let path = entry.item().id.to_string();
            file_menu(menu, &menu_app, path, entry.is_folder())
        });
        v_flex().size_full().child(toolbar).child(
            div()
                .id("file-tree")
                .flex_1()
                .min_h_0()
                .p_1()
                .child(file_tree),
        )
    }

    fn render_tags_pane(&self, cx: &mut Context<Self>) -> impl IntoElement {
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

    fn render_search_pane(&self, cx: &mut Context<Self>) -> impl IntoElement {
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

    // ---- Right sidebar ----

    fn render_right_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
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
                   icon: AssetIcon,
                   p: RightPane,
                   cx: &mut Context<Self>| {
            let b = Button::new(id).label(label).icon(Icon::new(icon)).xsmall();
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
                AssetIcon::List,
                RightPane::Outline,
                cx,
            ))
            .child(tab(
                "pane-backlinks",
                backlinks_label,
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
            RightPane::Outline => v_flex()
                .gap(px(1.))
                .children(self.outline.iter().enumerate().map(|(ix, h)| {
                    let line = h.line;
                    ListItem::new(("heading", ix))
                        .rounded(px(5.))
                        .py(px(2.))
                        .pl(px(8. + (h.level.saturating_sub(1)) as f32 * 12.))
                        .pr_2()
                        .child(div().text_sm().truncate().child(h.text.clone()))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.jump_to_line(line, window, cx)
                        }))
                }))
                .into_any_element(),
            RightPane::Backlinks if self.backlinks.is_empty() => empty("No other note links here"),
            RightPane::Backlinks => v_flex()
                .gap_1()
                .children(self.backlinks.iter().enumerate().map(|(ix, b)| {
                    let (path, line) = (b.source_path.clone(), b.line);
                    let item = ListItem::new(("backlink", ix)).rounded(px(6.)).p_2().child(
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
                    opens_note(item, cx.entity(), path, Some(line))
                }))
                .into_any_element(),
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
                    .overflow_y_scrollbar()
                    .p_2()
                    .child(body),
            )
    }

    // ---- Center ----

    fn render_center(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.vault.is_none() {
            return self.render_welcome(cx).into_any_element();
        }
        let content: AnyElement = match (self.active_path().map(str::to_string), self.active_kind())
        {
            (Some(path), Some(TabKind::Attachment(kind))) => {
                self.render_attachment(&path, kind, cx).into_any_element()
            }
            (Some(_), Some(TabKind::Note)) => self.render_editor(),
            _ => self.render_no_tab(cx).into_any_element(),
        };
        v_flex()
            .size_full()
            .child(self.render_tab_bar(cx))
            .child(self.render_banners(cx))
            .child(div().flex_1().min_h_0().child(content))
            .into_any_element()
    }

    fn render_tab_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
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
                        .tooltip("New tab (⌘T)")
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.open_quick_switcher(OpenIn::NewTab, window, cx)
                        })),
                ),
            )
    }

    fn render_banners(&self, cx: &mut Context<Self>) -> AnyElement {
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

    fn render_editor(&self) -> AnyElement {
        div()
            .id("editor-pane")
            .test_support()
            .size_full()
            .child(self.editor.clone())
            .into_any_element()
    }

    fn render_attachment(
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

    fn render_no_tab(&self, cx: &mut Context<Self>) -> impl IntoElement {
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

    fn render_welcome(&self, cx: &mut Context<Self>) -> impl IntoElement {
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

    // ---- Dialogs ----

    pub(crate) fn open_name_prompt(
        &mut self,
        kind: PromptKind,
        value: String,
        target: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.vault.is_none() {
            return;
        }
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(value)
                .placeholder(match kind {
                    PromptKind::NewNote => "Note name",
                    PromptKind::NewFolder => "Folder name",
                    PromptKind::Rename => "New name",
                })
        });
        let (title, ok) = match kind {
            PromptKind::NewNote => ("New Note", "Create"),
            PromptKind::NewFolder => ("New Folder", "Create"),
            PromptKind::Rename => ("Rename", "Rename"),
        };
        let entity = cx.entity();
        let field = input.clone();
        window.open_dialog(cx, move |dialog, _, _| {
            let (input_ok, entity, target) = (input.clone(), entity.clone(), target.clone());
            dialog
                .title(title)
                .width(px(380.))
                .button_props(DialogButtonProps::default().ok_text(ok).show_cancel(true))
                .child(Input::new(&input).id("prompt-input"))
                .on_ok(move |_, window, cx| {
                    let value = input_ok.read(cx).value().trim().to_string();
                    if !value.is_empty() {
                        entity.update(cx, |app, cx| {
                            app.run_prompt(kind, &value, target.as_deref(), window, cx)
                        });
                    }
                    true
                })
        });
        // The dialog takes focus as it opens; typing goes to the name at once.
        field.update(cx, |s, cx| s.focus(window, cx));
    }

    fn open_command_dialog(
        &mut self,
        placeholder: &'static str,
        items: Vec<CommandItem>,
        on_pick: impl Fn(&mut NotekiApp, usize, &mut Window, &mut Context<NotekiApp>) + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let state = cx.new(|cx| CommandState::new(window, cx));
        let entity = cx.entity();
        let on_pick = std::rc::Rc::new(on_pick);
        let query = state.clone();
        window.open_dialog(cx, move |dialog, _, _| {
            let (entity, on_pick) = (entity.clone(), on_pick.clone());
            dialog.width(px(540.)).close_button(false).p_0().child(
                Command::new(&state)
                    .bordered(false)
                    .placeholder(placeholder)
                    .items(items.clone())
                    .on_confirm(move |index, window, cx| {
                        window.close_dialog(cx);
                        let on_pick = on_pick.clone();
                        entity.update(cx, |app, cx| on_pick(app, index.row, window, cx));
                    })
                    .on_cancel(|window, cx| window.close_dialog(cx)),
            )
        });
        // The dialog takes focus as it opens; typing goes to the query at once.
        query.update(cx, |s, cx| s.focus(window, cx));
    }

    pub(crate) fn open_quick_switcher(
        &mut self,
        mode: OpenIn,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(vault) = &self.vault else { return };
        let ranked = crate::core::index::quick_switcher_rank(&vault.notes, &self.recent_notes, "");
        let items = ranked
            .iter()
            .map(|e| {
                let label = if e.folder.is_empty() {
                    e.name.clone()
                } else {
                    format!("{}  ·  {}", e.name, e.folder)
                };
                CommandItem::new()
                    .label(label)
                    .icon(AssetIcon::FileText)
                    .keywords([e.path.clone()])
            })
            .collect();
        let paths: Vec<String> = ranked.into_iter().map(|e| e.path).collect();
        self.open_command_dialog(
            "Open a note…",
            items,
            move |app, row, window, cx| {
                if let Some(p) = paths.get(row) {
                    app.open_path(p, mode, window, cx);
                }
            },
            window,
            cx,
        );
    }

    pub(crate) fn open_wikilink_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.editor_path.is_none() {
            return;
        }
        let all = self.link_completions();
        let items = all
            .iter()
            .map(|(name, folder, _)| {
                let label = if folder.is_empty() {
                    name.clone()
                } else {
                    format!("{name}  ·  {folder}")
                };
                CommandItem::new().label(label).icon(AssetIcon::Link)
            })
            .collect();
        let links: Vec<String> = all.into_iter().map(|(_, _, l)| l).collect();
        self.open_command_dialog(
            "Insert a link to…",
            items,
            move |app, row, window, cx| {
                if let Some(l) = links.get(row) {
                    app.insert_wikilink(l, window, cx);
                }
            },
            window,
            cx,
        );
    }

    pub(crate) fn open_command_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        type Run = fn(&mut NotekiApp, &mut Window, &mut Context<NotekiApp>);
        let commands: Vec<(&'static str, AssetIcon, Run)> = vec![
            ("New note", AssetIcon::FilePlus, |a, w, cx| {
                let f = a.target_folder(cx);
                a.open_name_prompt(PromptKind::NewNote, String::new(), Some(f), w, cx)
            }),
            ("New folder", AssetIcon::FolderPlus, |a, w, cx| {
                let f = a.target_folder(cx);
                a.open_name_prompt(PromptKind::NewFolder, String::new(), Some(f), w, cx)
            }),
            ("Open today's note", AssetIcon::CalendarDays, |a, w, cx| {
                a.open_daily_note(w, cx)
            }),
            ("Quick switcher", AssetIcon::Search, |a, w, cx| {
                a.open_quick_switcher(OpenIn::CurrentTab, w, cx)
            }),
            ("Insert wikilink", AssetIcon::Link, |a, w, cx| {
                a.open_wikilink_picker(w, cx)
            }),
            (
                "Follow link under cursor",
                AssetIcon::ArrowRight,
                |a, w, cx| a.follow_link_under_cursor(w, cx),
            ),
            ("Quote", AssetIcon::Pencil, |a, w, cx| {
                a.format(Format::Lines(LineFormat::Quote), w, cx)
            }),
            ("Bulleted list", AssetIcon::Pencil, |a, w, cx| {
                a.format(Format::Lines(LineFormat::Bullet), w, cx)
            }),
            ("Numbered list", AssetIcon::Pencil, |a, w, cx| {
                a.format(Format::Lines(LineFormat::Numbered), w, cx)
            }),
            ("Toggle left sidebar", AssetIcon::PanelLeft, |a, w, cx| {
                a.toggle_left_sidebar(w, cx)
            }),
            ("Toggle right sidebar", AssetIcon::PanelRight, |a, _, cx| {
                a.toggle_right_sidebar(cx)
            }),
            ("Next theme", AssetIcon::Palette, |a, w, cx| {
                a.cycle_theme(w, cx)
            }),
            ("Light / dark / system", AssetIcon::Sun, |a, w, cx| {
                a.cycle_appearance(w, cx)
            }),
            ("Export as HTML…", AssetIcon::Download, |a, _, cx| {
                a.export_html(cx)
            }),
            ("New window", AssetIcon::SquarePlus, |a, _, cx| {
                a.open_another_window(cx)
            }),
            ("Settings…", AssetIcon::Settings, |a, w, cx| {
                a.open_settings(w, cx)
            }),
            ("Open vault…", AssetIcon::FolderOpen, |a, w, cx| {
                a.pick_vault(w, cx)
            }),
            ("Close tab", AssetIcon::X, |a, w, cx| {
                a.close_active_tab(w, cx)
            }),
            ("Reopen closed tab", AssetIcon::RefreshCw, |a, w, cx| {
                a.reopen_closed_tab(w, cx)
            }),
        ];
        let items = commands
            .iter()
            .map(|(label, icon, _)| CommandItem::new().label(*label).icon(*icon))
            .collect();
        let runs: Vec<Run> = commands.into_iter().map(|(_, _, r)| r).collect();
        self.open_command_dialog(
            "Run a command…",
            items,
            move |app, row, window, cx| {
                if let Some(run) = runs.get(row) {
                    run(app, window, cx);
                }
            },
            window,
            cx,
        );
    }
}

/// A row that opens `path` (at `line`) in the Tab in front, or in a new Tab
/// with a ⌘-click or a middle click, as in a browser.
fn opens_note(
    item: ListItem,
    app: Entity<NotekiApp>,
    path: String,
    line: Option<usize>,
) -> ListItem {
    let open = std::rc::Rc::new(move |mode: OpenIn, window: &mut Window, cx: &mut App| {
        app.update(cx, |a, cx| {
            if a.open_path(&path, mode, window, cx) {
                if let Some(line) = line {
                    a.jump_to_line(line, window, cx);
                }
            }
        })
    });
    let middle = open.clone();
    item.on_click(move |ev: &ClickEvent, window, cx| {
        let new_tab = ev.modifiers().platform;
        open(
            if new_tab {
                OpenIn::NewTab
            } else {
                OpenIn::CurrentTab
            },
            window,
            cx,
        )
    })
    .on_mouse_down(MouseButton::Middle, move |_, window, cx| {
        cx.stop_propagation();
        middle(OpenIn::NewTab, window, cx)
    })
}

fn flatten_tags<'a>(nodes: &'a [TagNode], depth: usize, out: &mut Vec<(usize, &'a TagNode)>) {
    for n in nodes {
        out.push((depth, n));
        flatten_tags(&n.children, depth + 1, out);
    }
}

fn find_tag_notes(nodes: &[TagNode], full_lower: &str) -> Option<Vec<String>> {
    nodes.iter().find_map(|n| {
        if n.full_lower == full_lower {
            Some(n.note_paths.clone())
        } else {
            find_tag_notes(&n.children, full_lower)
        }
    })
}

/// An item of a context menu that calls `act` on the app.
fn menu_item(
    label: &'static str,
    app: &Entity<NotekiApp>,
    act: impl Fn(&mut NotekiApp, &mut Window, &mut Context<NotekiApp>) + 'static,
) -> PopupMenuItem {
    let app = app.clone();
    PopupMenuItem::new(label).on_click(move |_, window, cx| {
        app.update(cx, |a, cx| act(a, window, cx));
    })
}

/// The right-click menu of a file or folder of the tree (feedback: a richer
/// menu, copy the path, make a copy).
fn file_menu(menu: PopupMenu, app: &Entity<NotekiApp>, path: String, is_folder: bool) -> PopupMenu {
    let p = move || path.clone();
    let menu = if is_folder {
        let (a, b) = (p(), p());
        menu.item(menu_item("New note here", app, move |a2, w, cx| {
            a2.open_name_prompt(PromptKind::NewNote, String::new(), Some(a.clone()), w, cx)
        }))
        .item(menu_item("New folder here", app, move |a2, w, cx| {
            a2.open_name_prompt(PromptKind::NewFolder, String::new(), Some(b.clone()), w, cx)
        }))
    } else {
        let (a, b) = (p(), p());
        menu.item(menu_item("Open", app, move |app, w, cx| {
            app.open_path(&a, OpenIn::CurrentTab, w, cx);
        }))
        .item(menu_item("Open in new tab", app, move |app, w, cx| {
            app.open_path(&b, OpenIn::NewTab, w, cx);
        }))
    };
    let (rename, copy, rel, abs, reveal, trash) = (p(), p(), p(), p(), p(), p());
    let menu = menu
        .separator()
        .item(menu_item("Rename…", app, move |app, w, cx| {
            let name = if is_folder {
                rename.rsplit('/').next().unwrap_or(&rename).to_string()
            } else {
                note_name(&rename)
            };
            app.open_name_prompt(PromptKind::Rename, name, Some(rename.clone()), w, cx)
        }));
    let menu = if is_folder {
        menu
    } else {
        menu.item(menu_item("Make a copy", app, move |app, w, cx| {
            let r = app.duplicate(&copy, w, cx).map(|_| ());
            app.report(r);
        }))
    };
    menu.separator()
        .item(menu_item("Copy path", app, move |app, _, cx| {
            app.copy_path(&rel, false, cx)
        }))
        .item(menu_item("Copy full path", app, move |app, _, cx| {
            app.copy_path(&abs, true, cx)
        }))
        .item(menu_item("Reveal in Finder", app, move |app, _, cx| {
            app.reveal(&reveal, cx)
        }))
        .separator()
        .item(menu_item("Move to Trash", app, move |app, w, cx| {
            let r = app.trash(&trash, w, cx);
            app.report(r);
            cx.notify();
        }))
}

/// The right-click menu of a Tab.
fn tab_menu(menu: PopupMenu, app: &Entity<NotekiApp>, ix: usize, path: String) -> PopupMenu {
    let (rel, reveal) = (path.clone(), path);
    menu.item(menu_item("Close", app, move |app, w, cx| {
        app.close_tab(ix, w, cx)
    }))
    .item(menu_item("Close others", app, move |app, w, cx| {
        app.close_other_tabs(ix, w, cx)
    }))
    .item(menu_item(
        "Close tabs to the right",
        app,
        move |app, w, cx| app.close_tabs_after(ix, w, cx),
    ))
    .separator()
    .item(menu_item("Copy path", app, move |app, _, cx| {
        app.copy_path(&rel, false, cx)
    }))
    .item(menu_item("Reveal in Finder", app, move |app, _, cx| {
        app.reveal(&reveal, cx)
    }))
}
