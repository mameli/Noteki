//! The right-click menus of the tree's files and folders and of the Tabs.

use super::*;

/// Where a file is shown: the Finder on the Mac, the file manager elsewhere.
pub(in crate::app) const REVEAL: &str = if cfg!(target_os = "macos") {
    "Reveal in Finder"
} else {
    "Show in File Manager"
};

/// An item of a context menu that calls `act` on the app.
pub(in crate::app) fn menu_item(
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
pub(in crate::app) fn file_menu(
    menu: PopupMenu,
    app: &Entity<NotekiApp>,
    path: String,
    is_folder: bool,
) -> PopupMenu {
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
    let term = p();
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
        .item(menu_item(REVEAL, app, move |app, _, cx| {
            app.reveal(&reveal, cx)
        }))
        .item(menu_item(
            super::terminal::open_in_label(),
            app,
            move |app, _, cx| {
                app.open_in_terminal(&term);
                cx.notify();
            },
        ))
        .separator()
        .item(menu_item("Move to Trash", app, move |app, w, cx| {
            let r = app.trash(&trash, w, cx);
            app.report(r);
            cx.notify();
        }))
}

/// The right-click menu of a Tab.
pub(in crate::app) fn tab_menu(
    menu: PopupMenu,
    app: &Entity<NotekiApp>,
    ix: usize,
    path: String,
) -> PopupMenu {
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
    .item(menu_item(REVEAL, app, move |app, _, cx| {
        app.reveal(&reveal, cx)
    }))
}
