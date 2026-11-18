//! The menu bar, as the Swift app's: every command with its shortcut, Settings
//! under Noteki's name, the Command palette as the list of shortcuts.

use gpui_kit::{actions, App, Menu, MenuItem, OsAction, SystemMenuType};

use super::*;
use crate::editor::{
    Copy, Cut, FindNext, FindPrevious, Heading1, Heading2, Heading3, Heading4, Heading5, Heading6,
    Indent, MakeLink, OpenFind, OpenReplace, Outdent, Paste, PasteAndMatchStyle, Redo, SelectAll,
    ToggleComment, ToggleTask, Undo,
};

actions!(noteki_app, [Quit, Hide, HideOthers, ShowAll, Minimize]);

/// Sets the menu bar and the application's own commands (Quit, Hide…).
pub fn set_app_menus(cx: &mut App) {
    cx.bind_keys(
        [
            crate::keys::binding("cmd-q", Quit, None),
            crate::keys::binding("cmd-h", Hide, None),
            crate::keys::binding("alt-cmd-h", HideOthers, None),
            crate::keys::binding("cmd-m", Minimize, None),
        ]
        .into_iter()
        .flatten(),
    );
    cx.on_action(|_: &Quit, cx| cx.quit());
    cx.on_action(|_: &CheckForUpdatesAction, _| updates::check());
    cx.on_action(|_: &SupportNotekiAction, cx| cx.open_url(SUPPORT_URL));
    cx.on_action(|_: &Hide, cx| cx.hide());
    cx.on_action(|_: &HideOthers, cx| cx.hide_other_apps());
    cx.on_action(|_: &ShowAll, cx| cx.unhide_other_apps());
    cx.on_action(|_: &Minimize, cx| {
        if let Some(window) = cx.active_window() {
            let _ = window.update(cx, |_, window, _| window.minimize_window());
        }
    });
    cx.set_menus(app_menus(&Settings::default()));
}

/// The menus again, their ticks after `settings` (the View menu's modes).
pub fn refresh(settings: &Settings, cx: &mut App) {
    cx.set_menus(app_menus(settings));
}

fn action(name: &'static str, action: impl gpui_kit::Action) -> MenuItem {
    MenuItem::action(name, action)
}

pub fn app_menus(settings: &Settings) -> Vec<Menu> {
    vec![
        Menu::new("Noteki").items([
            action("Check for Updates…", CheckForUpdatesAction),
            MenuItem::separator(),
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
            action(terminal::open_vault_label(), OpenVaultInTerminalAction),
            MenuItem::separator(),
            action("Export as HTML…", ExportHtmlAction),
            action("Export as PDF…", ExportPdfAction),
            MenuItem::separator(),
            action("Print…", PrintAction),
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
            action("Paste and Match Style", PasteAndMatchStyle),
            MenuItem::os_action("Select All", SelectAll, OsAction::SelectAll),
            MenuItem::separator(),
            action("Find…", OpenFind),
            action("Find Next", FindNext),
            action("Find Previous", FindPrevious),
            action("Replace…", OpenReplace),
            action("Search in Vault…", SearchInVaultAction),
            MenuItem::separator(),
            MenuItem::submenu(Menu::new("Spelling and Grammar").items([
                action("Check Spelling While Typing", ToggleCheckSpelling)
                    .checked(settings.check_spelling),
                action("Correct Spelling Automatically", ToggleCorrectSpelling)
                    .checked(settings.correct_spelling),
            ])),
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
            action("Focus Mode", ToggleFocusMode).checked(settings.focus_mode),
            action("Typewriter Mode", ToggleTypewriterMode).checked(settings.typewriter_mode),
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
        Menu::new("Help").items([
            action("Keyboard Shortcuts", KeyboardShortcutsAction),
            MenuItem::separator(),
            action("Support Noteki…", SupportNotekiAction),
        ]),
    ]
}
