//! The menu bar, as the Swift app's: every command with its shortcut, Settings
//! under Noteki's name, the Command palette as the list of shortcuts.

use gpui_kit::{actions, App, Menu, MenuItem, OsAction, SystemMenuType};

use super::*;
use crate::editor::{
    Copy, Cut, FindNext, FindPrevious, Heading1, Heading2, Heading3, Heading4, Heading5, Heading6,
    Indent, MakeLink, OpenFind, OpenReplace, Outdent, Paste, Redo, SelectAll, ToggleComment,
    ToggleTask, Undo,
};

actions!(noteki_app, [Quit, Hide, HideOthers, ShowAll, Minimize]);

/// Sets the menu bar and the application's own commands (Quit, Hide…).
pub fn set_app_menus(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("cmd-q", Quit, None),
        KeyBinding::new("cmd-h", Hide, None),
        KeyBinding::new("alt-cmd-h", HideOthers, None),
        KeyBinding::new("cmd-m", Minimize, None),
    ]);
    cx.on_action(|_: &Quit, cx| cx.quit());
    cx.on_action(|_: &Hide, cx| cx.hide());
    cx.on_action(|_: &HideOthers, cx| cx.hide_other_apps());
    cx.on_action(|_: &ShowAll, cx| cx.unhide_other_apps());
    cx.on_action(|_: &Minimize, cx| {
        if let Some(window) = cx.active_window() {
            let _ = window.update(cx, |_, window, _| window.minimize_window());
        }
    });
    cx.set_menus(app_menus());
}

fn action(name: &'static str, action: impl gpui_kit::Action) -> MenuItem {
    MenuItem::action(name, action)
}

pub fn app_menus() -> Vec<Menu> {
    vec![
        Menu::new("Noteki").items([
            action("Settings…", OpenSettingsAction),
            MenuItem::separator(),
            MenuItem::os_submenu("Services", SystemMenuType::Services),
            MenuItem::separator(),
            action("Hide Noteki", Hide),
            action("Hide Others", HideOthers),
            action("Show All", ShowAll),
            MenuItem::separator(),
            action("Quit Noteki", Quit),
        ]),
        Menu::new("File").items([
            action("New Note", NewNoteAction),
            action("Today's Note", DailyNoteAction),
            action("New Window", NewWindowAction),
            MenuItem::separator(),
            action("Quick Switcher…", OpenQuickSwitcher),
            action("New Tab…", OpenQuickSwitcherNewTab),
            action("Open Vault…", OpenVaultAction),
            MenuItem::separator(),
            action("Export as HTML…", ExportHtmlAction),
            MenuItem::separator(),
            action("Close Tab", CloseTabAction),
            action("Reopen Closed Tab", ReopenClosedTabAction),
        ]),
        Menu::new("Edit").items([
            MenuItem::os_action("Undo", Undo, OsAction::Undo),
            MenuItem::os_action("Redo", Redo, OsAction::Redo),
            MenuItem::separator(),
            MenuItem::os_action("Cut", Cut, OsAction::Cut),
            MenuItem::os_action("Copy", Copy, OsAction::Copy),
            MenuItem::os_action("Paste", Paste, OsAction::Paste),
            MenuItem::os_action("Select All", SelectAll, OsAction::SelectAll),
            MenuItem::separator(),
            action("Find…", OpenFind),
            action("Find Next", FindNext),
            action("Find Previous", FindPrevious),
            action("Replace…", OpenReplace),
            action("Search in Vault…", SearchInVaultAction),
        ]),
        Menu::new("Format").items([
            action("Bold", FormatBoldAction),
            action("Italic", FormatItalicAction),
            action("Strikethrough", FormatStrikeAction),
            action("Inline Code", FormatCodeAction),
            action("Comment", ToggleComment),
            MenuItem::separator(),
            action("Link", MakeLink),
            action("Link to a Note…", InsertWikilinkAction),
            MenuItem::separator(),
            action("Heading 1", Heading1),
            action("Heading 2", Heading2),
            action("Heading 3", Heading3),
            action("Heading 4", Heading4),
            action("Heading 5", Heading5),
            action("Heading 6", Heading6),
            action("Task", ToggleTask),
            MenuItem::separator(),
            action("Indent", Indent),
            action("Outdent", Outdent),
        ]),
        Menu::new("View").items([
            action("Command Palette…", OpenCommandPalette),
            MenuItem::separator(),
            action("Toggle Left Sidebar", ToggleLeftSidebar),
            action("Toggle Right Sidebar", ToggleRightSidebar),
            MenuItem::separator(),
            action("Files", SwitchFilesPane),
            action("Tags", SwitchTagsPane),
            action("Search", SwitchSearchPane),
            action("Outline", ShowOutlineAction),
            action("Backlinks", ShowBacklinksAction),
            MenuItem::separator(),
            action("Zoom In", ZoomIn),
            action("Zoom Out", ZoomOut),
            action("Actual Size", ZoomReset),
        ]),
        Menu::new("Go").items([
            action("Back", NavigateBackAction),
            action("Forward", NavigateForwardAction),
            MenuItem::separator(),
            action("Show Previous Tab", PrevTabAction),
            action("Show Next Tab", NextTabAction),
        ]),
        Menu::new("Window").items([action("Minimize", Minimize)]),
        Menu::new("Help").items([action("Keyboard Shortcuts", OpenCommandPalette)]),
    ]
}
