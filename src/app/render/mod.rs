//! Drawing of the Noteki window. Element IDs are stable so UI tests can find them.
//!
//! Here the root (the actions of the whole window and its layout); each part
//! in its own file: `chrome` (title and status bars), `left_sidebar`,
//! `right_sidebar`, `center` (tab bar, banners, editor), `dialogs`,
//! `context_menus`.

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

use super::parts::Part;
use super::settings::InterfaceSize;
use super::*;
use crate::core::vault::resolve_vault_path;
use crate::core::wikilink::note_name;
use gpui_kit::component::menu::{ContextMenuExt as _, PopupMenu, PopupMenuItem};
use gpui_kit::{MouseButton, NavigationDirection};

mod center;
mod chrome;
mod context_menus;
mod dialogs;
mod left_sidebar;
mod right_sidebar;

use context_menus::*;

impl Render for NotekiApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let s = &self.settings;
        let mut split = h_resizable("workspace");
        if s.left_sidebar_open {
            split = split.child(
                resizable_panel()
                    .size(px(260.))
                    .size_range(px(180.)..px(480.))
                    .child(Part::cached(&self.parts.left, cx)),
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
                    .child(Part::cached(&self.parts.right, cx)),
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
            // The palette lists every command with its shortcut.
            .on_action(
                cx.listener(|this, _: &KeyboardShortcutsAction, window, cx| {
                    this.open_command_palette(window, cx)
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
            .on_action(
                cx.listener(|this, _: &ToggleFocusMode, w, cx| this.toggle_focus_mode(w, cx)),
            )
            .on_action(cx.listener(|this, _: &ExportPdfAction, _, cx| this.export_pdf(cx)))
            .on_action(cx.listener(|this, _: &ExportHtmlAction, _, cx| this.export_html(cx)))
            .on_action(
                cx.listener(|this, _: &ToggleCheckSpelling, w, cx| {
                    this.toggle_check_spelling(w, cx)
                }),
            )
            .on_action(cx.listener(|this, _: &ToggleCorrectSpelling, w, cx| {
                this.toggle_correct_spelling(w, cx)
            }))
            .on_action(cx.listener(|this, _: &PrintAction, _, cx| this.print_note(cx)))
            .on_action(cx.listener(|this, _: &ToggleTypewriterMode, w, cx| {
                this.toggle_typewriter_mode(w, cx)
            }))
            .on_action(cx.listener(|this, _: &OpenVaultInTerminalAction, _, cx| {
                this.open_in_terminal("");
                cx.notify();
            }))
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
            .on_action(cx.listener(|this, _: &FormatQuoteAction, window, cx| {
                this.format(Format::Lines(LineFormat::Quote), window, cx)
            }))
            .on_action(cx.listener(|this, _: &FormatBulletAction, window, cx| {
                this.format(Format::Lines(LineFormat::Bullet), window, cx)
            }))
            .on_action(cx.listener(|this, _: &FormatNumberedAction, window, cx| {
                this.format(Format::Lines(LineFormat::Numbered), window, cx)
            }))
            .on_action(cx.listener(|this, _: &ToggleFormatBar, window, cx| {
                this.update_settings(|s| s.show_format_bar = !s.show_format_bar, window, cx)
            }))
            .on_action(cx.listener(|this, _: &InsertWikilinkAction, window, cx| {
                this.open_wikilink_picker(window, cx)
            }))
            .on_action(
                cx.listener(|this, _: &FollowLinkUnderCursorAction, window, cx| {
                    this.follow_link_under_cursor(window, cx)
                }),
            )
            .child(Part::cached(&self.parts.title, cx))
            .child(div().flex_1().min_h_0().child(split))
            .child(Part::cached(&self.parts.status, cx))
    }
}

/// The tab bar's height: GPUI Kit's Tab (32) and the line under it. A cached
/// view is laid out from it.
pub(super) const TAB_BAR_HEIGHT: gpui_kit::Pixels = px(33.);

/// The status bar's height (a cached view is laid out from it).
pub(super) const STATUS_BAR_HEIGHT: gpui_kit::Pixels = px(24.);

fn faded(c: Hsla, a: f32) -> Hsla {
    hsla(c.h, c.s, c.l, a)
}

/// A row that opens `path` (at `line`) in the Tab in front, or in a new Tab
/// with a ⌘-click (Ctrl on Linux) or a middle click, as in a browser.
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
        // ⌘ on the Mac, Ctrl on Linux.
        let new_tab = ev.modifiers().secondary(cx);
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
