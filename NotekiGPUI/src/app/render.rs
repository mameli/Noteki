//! Drawing of the Noteki window. Element IDs are stable so UI tests can find them.

use gpui_kit::assets::IconName as AssetIcon;
use gpui_kit::base::Disableable as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::command::{Command, CommandItem, CommandState};
use gpui_kit::component::dialog::DialogButtonProps;
use gpui_kit::component::input::{Editor, Input, InputState};
use gpui_kit::component::list::ListItem;
use gpui_kit::component::resizable::{h_resizable, resizable_panel};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::tab::{Tab, TabBar};
use gpui_kit::component::text::{FrontmatterPlugin, MarkdownExtensions, TextView};
use gpui_kit::component::tree::tree;
use gpui_kit::component::{
    h_flex, v_flex, ActiveTheme as _, Icon, Sizable as _, StyledExt as _, TitleBar, WindowExt as _,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    div, hsla, px, relative, AnyElement, AppContext as _, ClickEvent, Context, Hsla, ImageSource,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, Role, SharedString,
    StatefulInteractiveElement as _, Styled as _, TestSupportExt as _, Window,
};

use super::*;
use crate::core::markdown::{percent_decode, EMBED_SCHEME};
use crate::core::vault::resolve_vault_path;
use crate::core::wikilink::note_name;

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
                cx.listener(|this, _: &ToggleLeftSidebar, _, cx| this.toggle_left_sidebar(cx)),
            )
            .on_action(
                cx.listener(|this, _: &ToggleRightSidebar, _, cx| this.toggle_right_sidebar(cx)),
            )
            .on_action(cx.listener(|this, _: &SwitchFilesPane, _, cx| {
                this.show_left_pane(LeftPane::Files, cx)
            }))
            .on_action(cx.listener(|this, _: &SwitchTagsPane, _, cx| {
                this.show_left_pane(LeftPane::Tags, cx)
            }))
            .on_action(cx.listener(|this, _: &SwitchSearchPane, window, cx| {
                this.show_left_pane(LeftPane::Search, cx);
                this.search_input.update(cx, |s, cx| s.focus(window, cx));
            }))
            .on_action(cx.listener(|this, _: &ToggleViewMode, _, cx| {
                let next = this.view_mode().next();
                this.set_view_mode(next, cx)
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
            .on_action(cx.listener(|this, _: &NavigateBackAction, window, cx| {
                this.navigate(true, window, cx)
            }))
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
                this.wrap_selection("**", "**", window, cx)
            }))
            .on_action(cx.listener(|this, _: &FormatItalicAction, window, cx| {
                this.wrap_selection("*", "*", window, cx)
            }))
            .on_action(cx.listener(|this, _: &FormatStrikeAction, window, cx| {
                this.wrap_selection("~~", "~~", window, cx)
            }))
            .on_action(cx.listener(|this, _: &FormatCodeAction, window, cx| {
                this.wrap_selection("`", "`", window, cx)
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
        let border = cx.theme().border;
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
                            .on_click(cx.listener(|this, _, _, cx| this.toggle_left_sidebar(cx))),
                        )
                        .child(
                            icon_button("nav-back", AssetIcon::ArrowLeft, "Back (⌘[)")
                                .disabled(!can_back)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.navigate(true, window, cx)
                                })),
                        )
                        .child(
                            icon_button("nav-forward", AssetIcon::ArrowRight, "Forward (⌘])")
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
                        .child(self.render_view_mode_buttons(cx))
                        .child(div().w(px(1.)).h(px(16.)).mx_1().bg(border))
                        .child(
                            Button::new("theme")
                                .label(self.settings.theme_name.label())
                                .icon(Icon::new(AssetIcon::Palette))
                                .ghost()
                                .small()
                                .tooltip("Theme")
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.cycle_theme(window, cx)),
                                ),
                        )
                        .child(
                            Button::new("appearance")
                                .label(self.settings.appearance.label())
                                .icon(Icon::new(if self.palette.dark {
                                    AssetIcon::Moon
                                } else {
                                    AssetIcon::Sun
                                }))
                                .ghost()
                                .small()
                                .tooltip("Light / Dark")
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.cycle_appearance(window, cx)
                                })),
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

    fn render_view_mode_buttons(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let modes = [
            (ViewMode::Live, "view-live", "Live", AssetIcon::Pencil),
            (ViewMode::Split, "view-split", "Split", AssetIcon::Columns2),
            (
                ViewMode::Reader,
                "view-reader",
                "Reader",
                AssetIcon::BookOpen,
            ),
        ];
        h_flex()
            .gap(px(2.))
            .children(modes.into_iter().map(|(mode, id, label, icon)| {
                let b = Button::new(id).label(label).icon(Icon::new(icon)).xsmall();
                let b = if self.view_mode() == mode {
                    b.primary()
                } else {
                    b.ghost()
                };
                b.on_click(cx.listener(move |this, _, _, cx| this.set_view_mode(mode, cx)))
            }))
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
                   label: &'static str,
                   icon: AssetIcon,
                   p: LeftPane,
                   cx: &mut Context<Self>| {
            let b = Button::new(id).label(label).icon(Icon::new(icon)).xsmall();
            let b = if pane == p { b.primary() } else { b.ghost() };
            b.on_click(cx.listener(move |this, _, _, cx| this.show_left_pane(p, cx)))
        };
        let tabs = h_flex()
            .px_2()
            .py_1()
            .gap_1()
            .border_b_1()
            .border_color(border)
            .child(tab(
                "pane-files",
                "Files",
                AssetIcon::Folder,
                LeftPane::Files,
                cx,
            ))
            .child(tab("pane-tags", "Tags", AssetIcon::Tag, LeftPane::Tags, cx))
            .child(tab(
                "pane-search",
                "Search",
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
                button("new-folder", AssetIcon::FolderPlus, "New folder (⇧⌘N)").on_click(
                    cx.listener(|this, _, window, cx| {
                        let folder = this.target_folder(cx);
                        this.open_name_prompt(
                            PromptKind::NewFolder,
                            String::new(),
                            Some(folder),
                            window,
                            cx,
                        )
                    }),
                ),
            )
            .child(
                button("daily-note", AssetIcon::CalendarDays, "Today's note (⇧⌘D)")
                    .on_click(cx.listener(|this, _, window, cx| this.open_daily_note(window, cx))),
            )
            .child(div().flex_1())
            .when_some(active.clone(), |row, path| {
                let (p1, p2, p3) = (path.clone(), path.clone(), path);
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
                    button("duplicate-active", AssetIcon::Copy, "Make a copy").on_click(
                        cx.listener(move |this, _, window, cx| {
                            let r = this.duplicate(&p2, window, cx).map(|_| ());
                            this.report(r);
                        }),
                    ),
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
                .when(!is_folder, |li| {
                    li.on_click(move |ev: &ClickEvent, window, cx| {
                        let mode = if ev.modifiers().platform {
                            OpenIn::NewTab
                        } else {
                            OpenIn::CurrentTab
                        };
                        entity.update(cx, |app, cx| {
                            app.open_path(&id, mode, window, cx);
                        });
                    })
                })
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
                                ListItem::new(("tag-note", ix))
                                    .rounded(px(5.))
                                    .py(px(2.))
                                    .px_2()
                                    .child(div().text_sm().truncate().child(name))
                                    .on_click(cx.listener(
                                        move |this, ev: &ClickEvent, window, cx| {
                                            let mode = if ev.modifiers().platform {
                                                OpenIn::NewTab
                                            } else {
                                                OpenIn::CurrentTab
                                            };
                                            this.open_path(&path, mode, window, cx);
                                        },
                                    ))
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
            v_flex()
                .gap_2()
                .children(self.search_results.iter().enumerate().map(|(ix, r)| {
                    let path = r.path.clone();
                    v_flex()
                        .child(
                            ListItem::new(("result", ix))
                                .rounded(px(5.))
                                .py(px(2.))
                                .px_2()
                                .child(
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
                                )
                                .on_click(cx.listener(move |this, ev: &ClickEvent, window, cx| {
                                    let mode = if ev.modifiers().platform {
                                        OpenIn::NewTab
                                    } else {
                                        OpenIn::CurrentTab
                                    };
                                    this.open_path(&path, mode, window, cx);
                                })),
                        )
                        .children(r.lines.iter().enumerate().map(|(lx, m)| {
                            let (path, line) = (r.path.clone(), m.line);
                            ListItem::new(("result-line", ix * 1000 + lx))
                                .rounded(px(4.))
                                .py(px(1.))
                                .pl_4()
                                .pr_2()
                                .child(
                                    h_flex()
                                        .gap_1p5()
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(muted)
                                                .child(line.to_string()),
                                        )
                                        .child(div().text_xs().truncate().child(m.text.clone())),
                                )
                                .on_click(cx.listener(move |this, ev: &ClickEvent, window, cx| {
                                    let mode = if ev.modifiers().platform {
                                        OpenIn::NewTab
                                    } else {
                                        OpenIn::CurrentTab
                                    };
                                    if this.open_path(&path, mode, window, cx) {
                                        this.jump_to_line(line, window, cx);
                                    }
                                }))
                        }))
                }))
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
                    .overflow_y_scrollbar()
                    .p_2()
                    .child(body),
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
                    ListItem::new(("backlink", ix))
                        .rounded(px(6.))
                        .p_2()
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
                        )
                        .on_click(cx.listener(move |this, ev: &ClickEvent, window, cx| {
                            let mode = if ev.modifiers().platform {
                                OpenIn::NewTab
                            } else {
                                OpenIn::CurrentTab
                            };
                            if this.open_path(&path, mode, window, cx) {
                                this.jump_to_line(line, window, cx);
                            }
                        }))
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
            (Some(_), Some(TabKind::Note)) => match self.view_mode() {
                ViewMode::Live => self.render_editor(),
                ViewMode::Reader => self.render_reader(cx),
                ViewMode::Split => h_resizable("split")
                    .child(resizable_panel().child(self.render_editor()))
                    .child(resizable_panel().child(self.render_reader(cx)))
                    .into_any_element(),
            },
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
            if doc.is_some_and(|d| d.has_conflict() || d.is_deleted()) {
                label = format!("⚠ {label}");
            } else if doc.is_some_and(NoteDoc::is_dirty) {
                label = format!("• {label}");
            }
            let entity = entity.clone();
            bar = bar.child(
                Tab::new().label(label).suffix(
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
            .px_6()
            .py_4()
            .child(
                div()
                    .size_full()
                    .max_w(px(820.))
                    .mx_auto()
                    .child(Editor::new(&self.editor).bordered(false).h(relative(1.))),
            )
            .into_any_element()
    }

    fn render_reader(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some((path, md)) = self.reader_markdown.clone() else {
            return div().into_any_element();
        };
        let Some(vault) = &self.vault else {
            return div().into_any_element();
        };
        let (root, index) = (vault.root.clone(), vault.index.clone());
        let (links, tasks) = (cx.entity(), cx.entity());
        let source = path.clone();
        let view = TextView::markdown(SharedString::from(format!("reader-{path}")), md)
            .markdown_extensions(MarkdownExtensions::default().frontmatter())
            .plugin(FrontmatterPlugin::new())
            .scrollable(true)
            .selectable(true)
            .on_link_click(move |url, ev, window, cx| {
                let (url, new_tab) = (url.to_string(), ev.modifiers().platform);
                links.update(cx, |app, cx| {
                    app.follow_reader_link(&url, new_tab, window, cx)
                });
            })
            .on_task_toggle(move |ev, window, cx| {
                let (offset, checked) = (ev.marker_range().start, ev.checked());
                tasks.update(cx, |app, cx| app.toggle_task(offset, checked, window, cx));
            })
            .image_source(move |uri| reader_image_source(uri.as_ref(), &root, &index, &source));
        div()
            .id("reader")
            .test_support()
            .size_full()
            .px_8()
            .py_6()
            .child(div().size_full().max_w(px(760.)).mx_auto().child(view))
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
        input.update(cx, |s, cx| s.focus(window, cx));
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
            ("Live preview", AssetIcon::Pencil, |a, _, cx| {
                a.set_view_mode(ViewMode::Live, cx)
            }),
            (
                "Split: editor and reader",
                AssetIcon::Columns2,
                |a, _, cx| a.set_view_mode(ViewMode::Split, cx),
            ),
            ("Reader", AssetIcon::BookOpen, |a, _, cx| {
                a.set_view_mode(ViewMode::Reader, cx)
            }),
            ("Toggle left sidebar", AssetIcon::PanelLeft, |a, _, cx| {
                a.toggle_left_sidebar(cx)
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

/// Where the Reader loads an image from: our embeds, vault-relative Markdown
/// images (`%20` read as a space, found as a wikilink is), or the web.
fn reader_image_source(
    uri: &str,
    root: &std::path::Path,
    index: &VaultIndex,
    source: &str,
) -> ImageSource {
    if uri.starts_with("http://") || uri.starts_with("https://") || uri.starts_with("data:") {
        return ImageSource::from(uri.to_string());
    }
    let path = match uri.strip_prefix(EMBED_SCHEME) {
        Some(p) => percent_decode(p),
        None => {
            let target = percent_decode(uri);
            match index.resolve(&target, Some(source)) {
                Some(ResolvedItem::Attachment { path, .. }) => path,
                _ => target,
            }
        }
    };
    match resolve_vault_path(root, &path) {
        Ok(abs) => ImageSource::from(abs),
        Err(_) => ImageSource::from(uri.to_string()),
    }
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
