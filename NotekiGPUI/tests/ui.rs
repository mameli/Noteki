//! Headless UI integration tests: the real `NotekiApp` view in a GPUI test
//! window, on a temporary copy of the demo vault. Time is simulated, so the
//! one-second autosave is checked deterministically.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use gpui_kit::test::TestWindowExt;
use gpui_kit::{
    px, size, AnyWindowHandle, App, AppContext as _, Bounds, Context, Entity, Point,
    TestAppContext, Window, WindowBounds, WindowOptions,
};
use noteki_gpui::app::{register_keybindings, AppConfig, NotekiApp, AUTOSAVE_DELAY};
use noteki_gpui::core::tabs::OpenIn;

// ---- Fixtures ----

struct Fixture {
    dir: PathBuf,
    vault: PathBuf,
    window: AnyWindowHandle,
    app: Entity<NotekiApp>,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap().flatten() {
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

fn temp_dir() -> PathBuf {
    static N: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "noteki-ui-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::SeqCst)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // Canonical, as the app canonicalizes the vault root (/var → /private/var).
    std::fs::canonicalize(dir).unwrap()
}

fn open_app(
    cx: &mut TestAppContext,
    dir: &Path,
    vault: &Path,
) -> (AnyWindowHandle, Entity<NotekiApp>) {
    std::fs::create_dir_all(dir.join("Trash")).unwrap();
    let config = AppConfig {
        settings_path: Some(dir.join("settings.json")),
        vault: Some(vault.to_path_buf()),
        watch: false, // tests deliver file-system changes themselves
        trash_dir: Some(dir.join("Trash")),
        slot: 0,
    };
    cx.update(|cx| {
        gpui_kit::init(cx);
        register_keybindings(cx);
        let bounds = Bounds {
            origin: Point::default(),
            size: size(px(1280.), px(800.)),
        };
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            ..Default::default()
        };
        let (window, app) = gpui_kit::open_window(options, cx, |window, cx| {
            cx.new(|cx| NotekiApp::new(config, window, cx))
        })
        .expect("window");
        (window, app)
    })
}

fn fixture(cx: &mut TestAppContext) -> Fixture {
    let dir = temp_dir();
    let vault = dir.join("vault");
    copy_dir(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("../docs/demo-vault"),
        &vault,
    );
    let (window, app) = open_app(cx, &dir, &vault);
    let f = Fixture {
        dir,
        vault,
        window,
        app,
    };
    frame(cx, &f);
    f
}

fn app<R>(
    cx: &mut TestAppContext,
    f: &Fixture,
    run: impl FnOnce(&mut NotekiApp, &mut Window, &mut Context<NotekiApp>) -> R,
) -> R {
    let app = f.app.clone();
    cx.update_window(f.window, |_, window, cx| {
        app.update(cx, |a, cx| run(a, window, cx))
    })
    .unwrap()
}

fn ui<R>(cx: &mut TestAppContext, f: &Fixture, run: impl FnOnce(&mut Window, &mut App) -> R) -> R {
    cx.update_window(f.window, |_, window, cx| run(window, cx))
        .unwrap()
}

fn frame(cx: &mut TestAppContext, f: &Fixture) {
    cx.run_until_parked();
    ui(cx, f, |window, cx| window.render_frame(cx));
}

fn read(f: &Fixture, rel: &str) -> String {
    std::fs::read_to_string(f.vault.join(rel)).unwrap()
}

fn status(cx: &mut TestAppContext, f: &Fixture) -> String {
    frame(cx, f);
    ui(cx, f, |window, _| {
        window
            .find("status")
            .label()
            .unwrap_or_default()
            .to_string()
    })
}

fn type_in_editor(cx: &mut TestAppContext, f: &Fixture, text: &str) {
    app(cx, f, |a, window, cx| {
        a.editor().clone().update(cx, |ed, cx| ed.focus(window, cx))
    });
    frame(cx, f);
    ui(cx, f, |window, cx| window.input(text, cx));
    cx.run_until_parked();
}

fn wait_autosave(cx: &mut TestAppContext) {
    cx.executor()
        .advance_clock(AUTOSAVE_DELAY + Duration::from_millis(50));
    cx.run_until_parked();
}

fn external_write(cx: &mut TestAppContext, f: &Fixture, rel: &str, text: Option<&str>) {
    let abs = f.vault.join(rel);
    match text {
        Some(t) => std::fs::write(&abs, t).unwrap(),
        None => std::fs::remove_file(&abs).unwrap(),
    }
    app(cx, f, |a, window, cx| {
        a.handle_fs_changes(&[abs], window, cx)
    });
    frame(cx, f);
}

// ---- Opening ----

#[gpui_kit::test]
fn opens_the_welcome_note_in_the_editor(cx: &mut TestAppContext) {
    let f = fixture(cx);
    let (path, text) = app(cx, &f, |a, _, cx| {
        (a.active_path().map(str::to_string), a.editor_text(cx))
    });
    assert_eq!(path.as_deref(), Some("Welcome.md"));
    assert_eq!(text, read(&f, "Welcome.md"));
    assert_eq!(status(cx, &f), "Saved");
    assert_eq!(
        app(cx, &f, |a, _, _| a
            .outline()
            .first()
            .map(|h| h.text.clone())),
        Some("Welcome to Noteki".into())
    );
}

#[gpui_kit::test]
fn clicking_a_folder_expands_it_once(cx: &mut TestAppContext) {
    // Regression: the tree and the row both toggled the folder, so it never opened.
    let f = fixture(cx);
    ui(cx, &f, |window, cx| {
        window.within("file-tree").click(0usize, cx)
    }); // "Daily"
    frame(cx, &f);
    assert!(app(cx, &f, |a, _, _| a
        .expanded_folders()
        .contains("Daily")));
    ui(cx, &f, |window, cx| {
        window.within("file-tree").click(0usize, cx)
    });
    frame(cx, &f);
    assert!(!app(cx, &f, |a, _, _| a
        .expanded_folders()
        .contains("Daily")));
}

#[gpui_kit::test]
fn clicking_a_note_in_the_tree_opens_it(cx: &mut TestAppContext) {
    let f = fixture(cx);
    // Folders first (Daily, Media, Meetings, Physics, Projects), then Ideas.md.
    ui(cx, &f, |window, cx| {
        window.within("file-tree").click(5usize, cx)
    });
    frame(cx, &f);
    assert_eq!(
        app(cx, &f, |a, _, _| a.active_path().map(str::to_string)).as_deref(),
        Some("Ideas.md")
    );
}

#[gpui_kit::test]
fn a_middle_click_on_a_note_opens_it_in_a_new_tab(cx: &mut TestAppContext) {
    use gpui_kit::test::ClickOptions;
    let f = fixture(cx);
    let tabs = app(cx, &f, |a, _, _| a.tabs().len());
    let middle = ClickOptions::new().with_button(gpui_kit::MouseButton::Middle);
    ui(cx, &f, |window, cx| {
        window
            .within("file-tree")
            .click_with_options(5usize, middle, cx)
    }); // Ideas.md
    frame(cx, &f);
    assert_eq!(app(cx, &f, |a, _, _| a.tabs().len()), tabs + 1);
    assert_eq!(
        app(cx, &f, |a, _, _| a.active_path().map(str::to_string)).as_deref(),
        Some("Ideas.md")
    );
}

#[gpui_kit::test]
fn files_from_the_finder_go_to_a_tab_files_or_a_window_of_their_own(cx: &mut TestAppContext) {
    let f = fixture(cx);
    let tabs = app(cx, &f, |a, _, _| a.tabs().len());
    let ideas = f.vault.join("Ideas.md");
    app(cx, &f, |a, w, cx| a.open_external(&ideas, w, cx));
    frame(cx, &f);
    assert_eq!(
        app(cx, &f, |a, _, _| (
            a.active_path().map(str::to_string),
            a.tabs().len()
        )),
        (Some("Ideas.md".into()), tabs + 1),
        "a new Tab, the others kept"
    );

    let daily = f.vault.join("Daily");
    app(cx, &f, |a, w, cx| a.open_external(&daily, w, cx));
    frame(cx, &f);
    assert_eq!(
        app(cx, &f, |a, _, cx| a.selected_in_files(cx)).as_deref(),
        Some("Daily")
    );

    // A Markdown file outside the Vault: a Loose file, saved where it is.
    let loose = f.dir.join("Fuori.md");
    std::fs::write(&loose, "fuori").unwrap();
    let before = cx.update(|cx| cx.windows().len());
    app(cx, &f, |a, w, cx| a.open_external(&loose, w, cx));
    cx.run_until_parked();
    let windows = cx.update(|cx| cx.windows());
    assert_eq!(windows.len(), before + 1, "a window of its own");
    let window = *windows.iter().find(|w| **w != f.window).unwrap();
    cx.update_window(window, |_, window, cx| window.render_frame(cx))
        .unwrap();
    cx.update_window(window, |_, window, cx| window.input("Ecco ", cx))
        .unwrap();
    cx.run_until_parked();
    assert_eq!(
        std::fs::read_to_string(&loose).unwrap(),
        "fuori",
        "saved too early"
    );
    wait_autosave(cx);
    assert_eq!(std::fs::read_to_string(&loose).unwrap(), "Ecco fuori");
    assert_eq!(
        app(cx, &f, |a, _, _| a.tabs().len()),
        tabs + 1,
        "nothing added to the Vault's window"
    );
}

// ---- Settings ----

#[gpui_kit::test]
fn a_shorter_line_length_rewraps_and_is_saved(cx: &mut TestAppContext) {
    let f = fixture(cx);
    let long = "parola ".repeat(60);
    std::fs::write(f.vault.join("Lunga.md"), &long).unwrap();
    let abs = f.vault.join("Lunga.md");
    app(cx, &f, |a, w, cx| {
        a.handle_fs_changes(&[abs], w, cx);
        a.open_path("Lunga.md", OpenIn::CurrentTab, w, cx)
    });
    frame(cx, &f);
    let bottom = |cx: &mut TestAppContext| {
        frame(cx, &f);
        app(cx, &f, |a, _, cx| {
            f32::from(
                a.editor()
                    .read(cx)
                    .caret_bounds(long.len())
                    .unwrap()
                    .origin
                    .y,
            )
        })
    };
    let wide = bottom(cx);
    app(cx, &f, |a, w, cx| {
        a.update_settings(|s| s.line_length = 40, w, cx)
    });
    let narrow = bottom(cx);
    assert!(
        narrow > wide,
        "more rows at 40 characters: {wide} → {narrow}"
    );
    app(cx, &f, |a, w, cx| {
        a.update_settings(|s| s.limit_line_length = false, w, cx)
    });
    assert!(bottom(cx) <= wide, "no limit: as wide as the window");
    let saved = std::fs::read_to_string(f.dir.join("settings.json")).unwrap();
    assert!(saved.contains("\"line_length\": 40"), "{saved}");
}

#[gpui_kit::test]
fn an_excluded_folder_leaves_the_vault_and_closes_its_tabs(cx: &mut TestAppContext) {
    let f = fixture(cx);
    app(cx, &f, |a, w, cx| {
        a.open_path("Daily/2026-11-12.md", OpenIn::NewTab, w, cx)
    });
    frame(cx, &f);
    assert!(app(cx, &f, |a, _, _| a
        .tabs()
        .contains("Daily/2026-11-12.md")));
    let root = app(cx, &f, |a, _, _| a.vault().unwrap().root.clone());
    let key = root.to_string_lossy().to_string();
    app(cx, &f, |a, w, cx| {
        a.update_settings(
            |s| {
                s.excluded_folders.insert(key, vec!["daily".into()]);
            },
            w,
            cx,
        )
    });
    frame(cx, &f);
    let (in_vault, tab) = app(cx, &f, |a, _, _| {
        (
            a.vault().unwrap().contains("Daily/2026-11-12.md"),
            a.tabs().contains("Daily/2026-11-12.md"),
        )
    });
    assert!(
        !in_vault && !tab,
        "left out, in any letter case, and its Tab closed"
    );
}

#[gpui_kit::test]
fn the_settings_window_opens_once(cx: &mut TestAppContext) {
    let f = fixture(cx);
    let before = cx.update(|cx| cx.windows().len());
    ui(cx, &f, |w, cx| w.click("settings", cx));
    app(cx, &f, |a, w, cx| a.open_settings(w, cx));
    cx.run_until_parked();
    let windows = cx.update(|cx| cx.windows());
    assert_eq!(windows.len(), before + 1);
    let settings = *windows.iter().find(|w| **w != f.window).unwrap();
    // Choosing a theme there applies it to the Vault's window and saves it.
    let click_label = |cx: &mut TestAppContext, label: &str| {
        use gpui_kit::test::TestQueryExt as _;
        use gpui_kit::{MouseButton, MouseDownEvent, MouseUpEvent, PlatformInput};
        cx.update_window(settings, |_, window, cx| {
            window.render_frame(cx);
            let at = window.find_by_label(label).bounds().center();
            window.dispatch_event(
                PlatformInput::MouseDown(MouseDownEvent {
                    button: MouseButton::Left,
                    position: at,
                    modifiers: Default::default(),
                    click_count: 1,
                    first_mouse: false,
                }),
                cx,
            );
            window.dispatch_event(
                PlatformInput::MouseUp(MouseUpEvent {
                    button: MouseButton::Left,
                    position: at,
                    modifiers: Default::default(),
                    click_count: 1,
                }),
                cx,
            );
        })
        .unwrap();
        cx.run_until_parked();
    };
    click_label(cx, "Theme: Ayu");
    click_label(cx, "Everforest");
    assert_eq!(
        app(cx, &f, |a, _, _| a.theme_name()),
        noteki_gpui::theme::NotekiThemeName::Everforest
    );
    let saved = std::fs::read_to_string(f.dir.join("settings.json")).unwrap();
    assert!(saved.to_lowercase().contains("everforest"), "{saved}");
}

// ---- Export ----

#[gpui_kit::test]
fn export_as_html_writes_one_page_with_the_image_inside(cx: &mut TestAppContext) {
    let f = fixture(cx);
    app(cx, &f, |a, w, cx| {
        a.open_path("Welcome.md", OpenIn::CurrentTab, w, cx)
    });
    app(cx, &f, |a, _, cx| a.export_html(cx));
    let target = f.dir.join("Welcome.html");
    let chosen = target.clone();
    cx.simulate_new_path_selection(move |_| Some(chosen));
    cx.run_until_parked();
    let html = std::fs::read_to_string(&target).unwrap();
    assert!(
        html.contains("<h1>Welcome to Noteki</h1>"),
        "{}",
        &html[..html.len().min(400)]
    );
    assert!(
        html.contains("src=\"data:image/png;base64,"),
        "the Vault's image put in"
    );
    assert!(!html.contains("noteki-link:") && !html.contains("noteki-embed:"));
}

// ---- Windows ----

/// The Noteki view of every window open but `except`.
fn other_apps(
    cx: &mut TestAppContext,
    except: &[AnyWindowHandle],
) -> Vec<(AnyWindowHandle, Entity<NotekiApp>)> {
    let windows = cx.update(|cx| cx.windows());
    windows
        .into_iter()
        .filter(|w| !except.contains(w))
        .filter_map(|w| {
            cx.update_window(w, |root, _, cx| {
                root.downcast::<gpui_kit::base::Root>()
                    .ok()
                    .and_then(|r| r.read(cx).view().clone().downcast::<NotekiApp>().ok())
            })
            .ok()
            .flatten()
            .map(|a| (w, a))
        })
        .collect()
}

fn tab_paths(cx: &mut TestAppContext, app: &Entity<NotekiApp>) -> Vec<String> {
    cx.update(|cx| app.read(cx).tabs().iter().map(|t| t.path.clone()).collect())
}

#[gpui_kit::test]
fn a_second_window_has_its_own_tabs_and_follows_the_file(cx: &mut TestAppContext) {
    let f = fixture(cx);
    app(cx, &f, |a, w, cx| {
        a.open_path("Ideas.md", OpenIn::CurrentTab, w, cx)
    });
    app(cx, &f, |a, _, cx| a.open_another_window(cx));
    cx.run_until_parked();
    let others = other_apps(cx, &[f.window]);
    assert_eq!(others.len(), 1, "⇧⌘N opens one window");
    let (w2, b) = others[0].clone();
    let in_b = |cx: &mut TestAppContext,
                run: &dyn Fn(&mut NotekiApp, &mut Window, &mut Context<NotekiApp>)| {
        cx.update_window(w2, |_, window, cx| b.update(cx, |b, cx| run(b, window, cx)))
            .unwrap();
        cx.run_until_parked();
    };
    in_b(cx, &|b, w, cx| {
        b.open_path("Projects/Launch plan.md", OpenIn::CurrentTab, w, cx);
        b.open_path("Ideas.md", OpenIn::NewTab, w, cx);
    });
    assert!(tab_paths(cx, &b).contains(&"Projects/Launch plan.md".to_string()));
    assert!(
        !app(cx, &f, |a, _, _| a
            .tabs()
            .contains("Projects/Launch plan.md")),
        "its own Tabs"
    );

    // Typed in the first window, saved, and seen by the second through the file.
    type_in_editor(cx, &f, "Insieme ");
    wait_autosave(cx);
    let abs = f.vault.join("Ideas.md");
    in_b(cx, &|b, w, cx| b.handle_fs_changes(&[abs.clone()], w, cx));
    let text_b = cx.update(|cx| b.read(cx).editor_text(cx));
    assert!(text_b.starts_with("Insieme "), "{text_b}");

    // A setting changed in one window reaches the other.
    app(cx, &f, |a, w, cx| {
        a.update_settings(|s| s.font_size = 21., w, cx)
    });
    cx.run_until_parked();
    assert_eq!(cx.update(|cx| b.read(cx).settings().font_size), 21.);

    // Its Tabs come back at the next launch; the first window's stay its own.
    let saved = noteki_gpui::app::settings::load(Some(&f.dir.join("settings.json")));
    let root = app(cx, &f, |a, _, _| {
        a.vault().unwrap().root.to_string_lossy().to_string()
    });
    let b_tabs = &saved.sessions[&format!("{root}#1")].open_tabs;
    assert!(
        b_tabs.contains(&"Projects/Launch plan.md".to_string()),
        "{b_tabs:?}"
    );
    assert!(!saved.sessions[&root]
        .open_tabs
        .contains(&"Projects/Launch plan.md".to_string()));
    cx.update_window(w2, |_, window, _| window.remove_window())
        .unwrap(); // as at quit
    cx.run_until_parked();
    let (relaunched, _) = open_app(cx, &f.dir, &f.vault);
    cx.run_until_parked();
    let restored = other_apps(cx, &[f.window, relaunched]);
    assert_eq!(restored.len(), 1, "the second window reopened");
    let tabs = tab_paths(cx, &restored[0].1);
    assert!(
        tabs.contains(&"Projects/Launch plan.md".to_string()),
        "{tabs:?}"
    );
}

#[gpui_kit::test]
fn choosing_another_vault_closes_the_other_windows(cx: &mut TestAppContext) {
    let f = fixture(cx);
    app(cx, &f, |a, _, cx| a.open_another_window(cx));
    cx.run_until_parked();
    assert_eq!(other_apps(cx, &[f.window]).len(), 1);
    let other = f.dir.join("Altro");
    std::fs::create_dir_all(&other).unwrap();
    std::fs::write(other.join("Nota.md"), "x").unwrap();
    app(cx, &f, |a, w, cx| a.open_vault(&other, w, cx)).unwrap();
    cx.run_until_parked();
    assert!(other_apps(cx, &[f.window]).is_empty());
    let saved = noteki_gpui::app::settings::load(Some(&f.dir.join("settings.json")));
    assert!(
        !saved.sessions.keys().any(|k| k.ends_with("#1")),
        "{:?}",
        saved.sessions.keys()
    );
}

// ---- Saving ----

#[gpui_kit::test]
fn typing_saves_about_a_second_later(cx: &mut TestAppContext) {
    let f = fixture(cx);
    let before = read(&f, "Ideas.md");
    app(cx, &f, |a, w, cx| {
        a.open_path("Ideas.md", OpenIn::CurrentTab, w, cx)
    });
    type_in_editor(cx, &f, "Typed ");
    assert_eq!(read(&f, "Ideas.md"), before, "saved too early");
    assert_eq!(status(cx, &f), "Edited");

    cx.executor().advance_clock(AUTOSAVE_DELAY / 2);
    type_in_editor(cx, &f, "more ");
    cx.executor()
        .advance_clock(AUTOSAVE_DELAY / 2 + Duration::from_millis(100));
    cx.run_until_parked();
    assert_eq!(
        read(&f, "Ideas.md"),
        before,
        "the delay restarts at each change"
    );

    wait_autosave(cx);
    assert_eq!(read(&f, "Ideas.md"), format!("Typed more {before}"));
    assert_eq!(status(cx, &f), "Saved");
}

#[gpui_kit::test]
fn switching_notes_saves_the_one_left(cx: &mut TestAppContext) {
    let f = fixture(cx);
    app(cx, &f, |a, w, cx| {
        a.open_path("Ideas.md", OpenIn::CurrentTab, w, cx)
    });
    type_in_editor(cx, &f, "X");
    app(cx, &f, |a, w, cx| {
        a.open_path("Reading list.md", OpenIn::CurrentTab, w, cx)
    });
    assert!(read(&f, "Ideas.md").starts_with("X# Ideas"));
}

// ---- External changes ----

#[gpui_kit::test]
fn external_change_reloads_a_clean_note_keeping_the_cursor(cx: &mut TestAppContext) {
    let f = fixture(cx);
    app(cx, &f, |a, w, cx| {
        a.open_path("Ideas.md", OpenIn::CurrentTab, w, cx)
    });
    app(cx, &f, |a, _, cx| {
        a.editor()
            .clone()
            .update(cx, |ed, cx| ed.set_selected_range(3..3, cx))
    });
    let new = format!("{}- Added by another app\n", read(&f, "Ideas.md"));
    external_write(cx, &f, "Ideas.md", Some(&new));
    let (text, cursor) = app(cx, &f, |a, _, cx| {
        (a.editor_text(cx), a.editor().read(cx).cursor())
    });
    assert_eq!(text, new);
    assert_eq!(cursor, 3);
}

#[gpui_kit::test]
fn external_change_merges_with_unsaved_edits_and_saves(cx: &mut TestAppContext) {
    let f = fixture(cx);
    app(cx, &f, |a, w, cx| {
        a.open_path("Ideas.md", OpenIn::CurrentTab, w, cx)
    });
    type_in_editor(cx, &f, "Mine ");
    let theirs = read(&f, "Ideas.md").replace("Graph view?", "Graph view (theirs)?");
    external_write(cx, &f, "Ideas.md", Some(&theirs));
    let merged = format!("Mine {theirs}");
    assert_eq!(app(cx, &f, |a, _, cx| a.editor_text(cx)), merged);
    assert_eq!(read(&f, "Ideas.md"), merged, "a merge is saved at once");
}

#[gpui_kit::test]
fn conflict_shows_a_banner_and_keep_mine_wins(cx: &mut TestAppContext) {
    let f = fixture(cx);
    app(cx, &f, |a, w, cx| {
        a.open_path("Ideas.md", OpenIn::CurrentTab, w, cx)
    });
    type_in_editor(cx, &f, "Mine ");
    external_write(cx, &f, "Ideas.md", Some("Theirs # Ideas\n"));
    assert!(ui(cx, &f, |w, _| w.find("conflict-banner").visible()));
    wait_autosave(cx);
    assert_eq!(
        read(&f, "Ideas.md"),
        "Theirs # Ideas\n",
        "no save during a conflict"
    );

    ui(cx, &f, |w, cx| w.click("keep-mine", cx));
    frame(cx, &f);
    assert!(read(&f, "Ideas.md").starts_with("Mine # Ideas"));
    assert!(ui(cx, &f, |w, _| w.try_find("conflict-banner").is_none()));
}

#[gpui_kit::test]
fn conflict_take_theirs_replaces_the_editor_text(cx: &mut TestAppContext) {
    let f = fixture(cx);
    app(cx, &f, |a, w, cx| {
        a.open_path("Ideas.md", OpenIn::CurrentTab, w, cx)
    });
    type_in_editor(cx, &f, "Mine ");
    external_write(cx, &f, "Ideas.md", Some("Theirs # Ideas\n"));
    ui(cx, &f, |w, cx| w.click("take-theirs", cx));
    frame(cx, &f);
    assert_eq!(
        app(cx, &f, |a, _, cx| a.editor_text(cx)),
        "Theirs # Ideas\n"
    );
    wait_autosave(cx);
    assert_eq!(read(&f, "Ideas.md"), "Theirs # Ideas\n");
}

#[gpui_kit::test]
fn deleted_note_is_kept_on_screen_until_saved_again(cx: &mut TestAppContext) {
    let f = fixture(cx);
    app(cx, &f, |a, w, cx| {
        a.open_path("Ideas.md", OpenIn::CurrentTab, w, cx)
    });
    type_in_editor(cx, &f, "Keep ");
    external_write(cx, &f, "Ideas.md", None);
    assert!(ui(cx, &f, |w, _| w.find("deleted-banner").visible()));
    wait_autosave(cx);
    assert!(
        !f.vault.join("Ideas.md").exists(),
        "autosave must not recreate a deleted note"
    );

    ui(cx, &f, |w, cx| w.click("save-again", cx));
    frame(cx, &f);
    assert!(read(&f, "Ideas.md").starts_with("Keep # Ideas"));
}

#[gpui_kit::test]
fn a_rename_by_another_app_is_followed(cx: &mut TestAppContext) {
    let f = fixture(cx);
    app(cx, &f, |a, w, cx| {
        a.open_path("Ideas.md", OpenIn::CurrentTab, w, cx)
    });
    let (old, new) = (
        f.vault.join("Ideas.md"),
        f.vault.join("Projects/Ideas moved.md"),
    );
    std::fs::rename(&old, &new).unwrap();
    app(cx, &f, |a, w, cx| a.handle_fs_changes(&[old, new], w, cx));
    assert_eq!(
        app(cx, &f, |a, _, _| a.active_path().map(str::to_string)).as_deref(),
        Some("Projects/Ideas moved.md")
    );
    assert!(ui(cx, &f, |w, _| w.try_find("deleted-banner").is_none()));
}

// ---- Links ----

#[gpui_kit::test]
fn links_open_notes_and_back_returns(cx: &mut TestAppContext) {
    let f = fixture(cx);
    app(cx, &f, |a, w, cx| {
        a.follow_link("Launch plan", None, false, w, cx)
    });
    frame(cx, &f);
    assert_eq!(
        app(cx, &f, |a, _, _| a.active_path().map(str::to_string)).as_deref(),
        Some("Projects/Launch plan.md")
    );
    ui(cx, &f, |w, cx| w.click("nav-back", cx));
    frame(cx, &f);
    assert_eq!(
        app(cx, &f, |a, _, _| a.active_path().map(str::to_string)).as_deref(),
        Some("Welcome.md")
    );
}

#[gpui_kit::test]
fn following_an_unresolved_link_creates_an_empty_note_next_to_the_source(cx: &mut TestAppContext) {
    let f = fixture(cx);
    app(cx, &f, |a, w, cx| {
        a.open_path("Physics/Euler's identity.md", OpenIn::CurrentTab, w, cx)
    });
    app(cx, &f, |a, w, cx| {
        a.follow_link("Brand new", None, false, w, cx)
    });
    assert_eq!(read(&f, "Physics/Brand new.md"), "");
    assert_eq!(
        app(cx, &f, |a, _, _| a.active_path().map(str::to_string)).as_deref(),
        Some("Physics/Brand new.md")
    );
    // Never over an existing file, nor with a refused character.
    app(cx, &f, |a, w, cx| {
        a.follow_link("bad:name", None, false, w, cx)
    });
    assert!(status(cx, &f).contains("Cannot create"));
}

#[gpui_kit::test]
fn rename_and_trash_follow_open_tabs(cx: &mut TestAppContext) {
    let f = fixture(cx);
    app(cx, &f, |a, w, cx| {
        a.open_path("Ideas.md", OpenIn::NewTab, w, cx)
    });
    type_in_editor(cx, &f, "Edit ");
    let new = app(cx, &f, |a, w, cx| a.rename("Ideas.md", "Thoughts", w, cx)).unwrap();
    assert_eq!(new, "Thoughts.md");
    assert!(
        read(&f, "Thoughts.md").starts_with("Edit # Ideas"),
        "unsaved text saved before the rename"
    );
    assert_eq!(
        app(cx, &f, |a, _, _| a.active_path().map(str::to_string)).as_deref(),
        Some("Thoughts.md")
    );

    type_in_editor(cx, &f, "Last ");
    app(cx, &f, |a, w, cx| a.trash("Thoughts.md", w, cx)).unwrap();
    assert!(!f.vault.join("Thoughts.md").exists());
    let trashed = std::fs::read_to_string(f.dir.join("Trash/Thoughts.md")).unwrap();
    assert!(
        trashed.starts_with("Last Edit # Ideas"),
        "saved before the move: {trashed}"
    );
    assert!(app(cx, &f, |a, _, _| !a.tabs().contains("Thoughts.md")));
}

#[gpui_kit::test]
fn renaming_a_folder_moves_its_tabs(cx: &mut TestAppContext) {
    let f = fixture(cx);
    app(cx, &f, |a, w, cx| {
        a.open_path("Physics/Euler's identity.md", OpenIn::NewTab, w, cx)
    });
    app(cx, &f, |a, w, cx| a.rename("Physics", "Science", w, cx)).unwrap();
    assert_eq!(
        app(cx, &f, |a, _, _| a.active_path().map(str::to_string)).as_deref(),
        Some("Science/Euler's identity.md")
    );
}

#[gpui_kit::test]
fn create_note_and_invalid_names(cx: &mut TestAppContext) {
    let f = fixture(cx);
    let p = app(cx, &f, |a, w, cx| {
        a.create_note("Projects", "Roadmap", w, cx)
    })
    .unwrap();
    assert_eq!(p, "Projects/Roadmap.md");
    assert!(f.vault.join(&p).is_file());
    assert!(app(cx, &f, |a, w, cx| a.create_note("", "a/b", w, cx)).is_err());
    assert!(
        app(cx, &f, |a, w, cx| a
            .create_note("Projects", "Roadmap", w, cx))
        .is_err(),
        "never over an existing file"
    );
}

// ---- Session ----

#[gpui_kit::test]
fn tabs_are_restored_when_the_vault_reopens(cx: &mut TestAppContext) {
    let f = fixture(cx);
    app(cx, &f, |a, w, cx| {
        a.open_path("Projects/Launch plan.md", OpenIn::NewTab, w, cx)
    });

    let (window, app2) = open_app(cx, &f.dir, &f.vault);
    cx.run_until_parked();
    let (tabs, active) = cx
        .update_window(window, |_, _, cx| {
            let a = app2.read(cx);
            (
                a.tabs().iter().map(|t| t.path.clone()).collect::<Vec<_>>(),
                a.active_path().map(str::to_string),
            )
        })
        .unwrap();
    assert_eq!(tabs, vec!["Welcome.md", "Projects/Launch plan.md"]);
    assert_eq!(active.as_deref(), Some("Projects/Launch plan.md"));
}

#[gpui_kit::test]
fn crlf_line_endings_and_final_newline_are_kept(cx: &mut TestAppContext) {
    // SPEC › Saving: keeps line endings and the final newline as they were.
    let f = fixture(cx);
    std::fs::write(f.vault.join("Windows.md"), "# Title\r\n\r\nBody\r\n").unwrap();
    app(cx, &f, |a, w, cx| {
        a.handle_fs_changes(&[f.vault.join("Windows.md")], w, cx)
    });
    app(cx, &f, |a, w, cx| {
        a.open_path("Windows.md", OpenIn::CurrentTab, w, cx)
    });
    type_in_editor(cx, &f, "X");
    wait_autosave(cx);
    assert_eq!(read(&f, "Windows.md"), "X# Title\r\n\r\nBody\r\n");
}

// ---- The editor element ----

/// Opens a Note, puts `text` in the editor with the cursor at `cursor`, and focuses it.
fn editor_with(cx: &mut TestAppContext, f: &Fixture, text: &str, cursor: usize) {
    app(cx, f, |a, w, cx| {
        a.open_path("Ideas.md", OpenIn::CurrentTab, w, cx)
    });
    let text = text.to_string();
    app(cx, f, |a, window, cx| {
        a.editor().clone().update(cx, |ed, cx| {
            ed.set_text(&text, cursor, cx);
            ed.focus(window, cx);
        })
    });
    frame(cx, f);
}

fn press(cx: &mut TestAppContext, f: &Fixture, keys: &str) {
    for key in keys.split_whitespace() {
        ui(cx, f, |window, cx| window.press(key, cx));
    }
    cx.run_until_parked();
}

fn editor_state(cx: &mut TestAppContext, f: &Fixture) -> (String, usize) {
    app(cx, f, |a, _, cx| {
        let ed = a.editor().read(cx);
        (ed.text().to_string(), ed.cursor())
    })
}

#[gpui_kit::test]
fn keys_move_by_character_word_and_line_and_delete(cx: &mut TestAppContext) {
    let f = fixture(cx);
    editor_with(cx, &f, "Ciao, mondo bello\nseconda riga", 0);
    press(cx, &f, "alt-right alt-right");
    assert_eq!(editor_state(cx, &f).1, "Ciao, mondo".len());
    press(cx, &f, "cmd-right");
    assert_eq!(editor_state(cx, &f).1, "Ciao, mondo bello".len());
    press(cx, &f, "alt-backspace");
    assert_eq!(editor_state(cx, &f).0, "Ciao, mondo \nseconda riga");
    press(cx, &f, "backspace enter");
    assert_eq!(editor_state(cx, &f).0, "Ciao, mondo\n\nseconda riga");
    press(cx, &f, "cmd-down left left shift-alt-left");
    let sel = app(cx, &f, |a, _, cx| {
        a.editor().read(cx).selected_text().to_string()
    });
    assert_eq!(sel, "ri");
    type_in_editor(cx, &f, "R");
    assert_eq!(editor_state(cx, &f).0, "Ciao, mondo\n\nseconda Rga");
}

#[gpui_kit::test]
fn typing_marks_the_note_edited_but_set_text_does_not(cx: &mut TestAppContext) {
    let f = fixture(cx);
    editor_with(cx, &f, "uno", 3);
    assert_ne!(status(cx, &f), "Edited");
    press(cx, &f, "backspace");
    assert_eq!(status(cx, &f), "Edited");
}

#[gpui_kit::test]
fn up_and_down_keep_the_x_across_a_shorter_line(cx: &mut TestAppContext) {
    let f = fixture(cx);
    // Monospace or not, "abcdefgh" is wider than "ab": down from after "abcde"
    // lands at the end of "ab", then back under the "e" on the third line.
    editor_with(cx, &f, "abcdefgh\nab\nabcdefgh", 5);
    press(cx, &f, "down");
    assert_eq!(editor_state(cx, &f).1, 9 + 2);
    press(cx, &f, "down");
    assert_eq!(editor_state(cx, &f).1, 12 + 5);
    press(cx, &f, "up up");
    assert_eq!(editor_state(cx, &f).1, 5);
    press(cx, &f, "up");
    assert_eq!(
        editor_state(cx, &f).1,
        0,
        "up on the first row goes to the start"
    );
    press(cx, &f, "shift-down shift-down shift-down shift-down");
    let (text, cursor) = editor_state(cx, &f);
    assert_eq!(cursor, text.len(), "down on the last row goes to the end");
}

#[gpui_kit::test]
fn a_dead_key_composes_an_accent_through_the_input_handler(cx: &mut TestAppContext) {
    use gpui_kit::test::TestInput;
    use gpui_kit::ElementInputHandler;
    let f = fixture(cx);
    editor_with(cx, &f, "caff", 4);
    let editor = app(cx, &f, |a, _, _| a.editor().clone());
    ui(cx, &f, |window, cx| {
        let bounds = window.find("editor-pane").bounds();
        let mut input = TestInput::new(ElementInputHandler::new(bounds, editor), window);
        input.compose_text(None, "´", Some(1..1), cx);
        assert_eq!(input.marked_text_range(cx), Some(4..5));
        input.commit_text("é", cx);
        assert_eq!(input.marked_text_range(cx), None);
    });
    assert_eq!(editor_state(cx, &f), ("caffé".to_string(), "caffé".len()));
    assert_eq!(status(cx, &f), "Edited");
}

#[gpui_kit::test]
fn the_view_follows_the_cursor_moved_by_keys(cx: &mut TestAppContext) {
    let f = fixture(cx);
    let text: String = (0..300).map(|i| format!("Riga {i}\n")).collect();
    editor_with(cx, &f, &text, 0);
    let scrolled = |cx: &mut TestAppContext| {
        app(cx, &f, |a, _, cx| {
            f32::from(-a.editor().read(cx).scroll_offset().y)
        })
    };
    assert_eq!(scrolled(cx), 0.);
    press(cx, &f, "cmd-down");
    frame(cx, &f);
    let at_end = scrolled(cx);
    assert!(at_end > 1000., "scrolled to the end: {at_end}");
    // Typing at the end keeps it in view; moving up a few lines does not scroll.
    press(cx, &f, "up up");
    frame(cx, &f);
    assert_eq!(scrolled(cx), at_end);
    press(cx, &f, "cmd-up");
    frame(cx, &f);
    assert_eq!(scrolled(cx), 0.);
}

#[gpui_kit::test]
fn command_arrows_go_to_the_ends_of_the_wrapped_row(cx: &mut TestAppContext) {
    let f = fixture(cx);
    let long = "parola ".repeat(200);
    let text = format!("{long}\nfine");
    editor_with(cx, &f, &text, 0);
    press(cx, &f, "cmd-right");
    let row_end = editor_state(cx, &f).1;
    assert!(
        row_end > 0 && row_end < long.len(),
        "end of the first row: {row_end}"
    );
    assert!(
        long[..row_end].ends_with("parola"),
        "before the space it wraps at"
    );
    press(cx, &f, "down cmd-left");
    let next_row = editor_state(cx, &f).1;
    assert!(
        next_row > row_end && next_row < long.len(),
        "start of the second row: {next_row}"
    );
    press(cx, &f, "ctrl-e");
    assert_eq!(
        editor_state(cx, &f).1,
        long.len(),
        "⌃E goes to the end of the paragraph"
    );
    press(cx, &f, "ctrl-a");
    assert_eq!(editor_state(cx, &f).1, 0);
}

fn heights(cx: &mut TestAppContext, f: &Fixture) -> Vec<f32> {
    frame(cx, f);
    app(cx, f, |a, _, cx| a.editor().read(cx).line_heights())
}

#[gpui_kit::test]
fn headings_are_taller_the_higher_their_level(cx: &mut TestAppContext) {
    let f = fixture(cx);
    editor_with(
        cx,
        &f,
        "testo\n# Uno\n## Due\n### Tre\n###### Sei\n```\n# codice\n```",
        0,
    );
    let h = heights(cx, &f);
    let text = h[0];
    assert!(h[1] > h[2] && h[2] > h[3], "H1 > H2 > H3: {h:?}");
    assert!(h[3] > text, "a Heading is taller than text: {h:?}");
    assert_eq!(h[6], text, "a # in a code block is no Heading: {h:?}");
}

#[gpui_kit::test]
fn typing_a_hash_and_space_makes_a_heading_at_once(cx: &mut TestAppContext) {
    let f = fixture(cx);
    editor_with(cx, &f, "Titolo\naltro", 0);
    let before = heights(cx, &f);
    type_in_editor(cx, &f, "# ");
    let after = heights(cx, &f);
    assert!(after[0] > before[0], "{before:?} → {after:?}");
    assert_eq!(after[1], before[1], "the other line keeps its height");
    press(cx, &f, "backspace backspace");
    assert_eq!(heights(cx, &f)[0], before[0]);
}

#[gpui_kit::test]
fn up_and_down_across_a_heading_keep_the_x(cx: &mut TestAppContext) {
    let f = fixture(cx);
    let text = "abcdefgh\n# abcdefghijkl\nabcdefgh";
    editor_with(cx, &f, text, 6);
    press(cx, &f, "down");
    let on_heading = editor_state(cx, &f).1;
    assert!(
        (9..24).contains(&on_heading),
        "on the Heading: {on_heading}"
    );
    press(cx, &f, "down");
    assert_eq!(
        editor_state(cx, &f).1,
        24 + 6,
        "back under the same x below"
    );
    press(cx, &f, "up up");
    assert_eq!(editor_state(cx, &f).1, 6);
}

#[gpui_kit::test]
fn opening_a_code_block_above_a_heading_draws_it_as_code(cx: &mut TestAppContext) {
    // The Heading's own text does not change: only its style does.
    let f = fixture(cx);
    // Two backticks are no fence; a third, typed on the same line, opens one.
    editor_with(cx, &f, "``\n# Titolo\nfine", 0);
    let before = heights(cx, &f);
    assert!(before[1] > before[2]);
    type_in_editor(cx, &f, "`");
    let after = heights(cx, &f);
    assert_eq!(
        after[1], after[2],
        "the Heading is inside the code block now: {after:?}"
    );
}

fn caret_at(cx: &mut TestAppContext, f: &Fixture, offset: usize) -> (f32, f32) {
    frame(cx, f);
    app(cx, f, |a, _, cx| {
        let b = a.editor().read(cx).caret_bounds(offset).expect("a layout");
        (f32::from(b.origin.x), f32::from(b.origin.y))
    })
}

#[gpui_kit::test]
fn heading_marks_are_hidden_off_the_cursor_line_and_shown_on_it(cx: &mut TestAppContext) {
    let f = fixture(cx);
    // "## Titolo": its text starts at 3.
    editor_with(cx, &f, "## Titolo\ntesto", 10);
    let left = caret_at(cx, &f, 10).0;
    assert_eq!(
        caret_at(cx, &f, 3).0,
        left,
        "the Heading's text starts at the left edge"
    );
    assert_eq!(caret_at(cx, &f, 0).0, left);
    press(cx, &f, "up");
    assert_eq!(
        editor_state(cx, &f).1,
        3,
        "↑ lands before the first letter, after the marks"
    );
    assert!(caret_at(cx, &f, 3).0 > left, "on its line, the `## ` shows");
    press(cx, &f, "down");
    assert_eq!(
        caret_at(cx, &f, 3).0,
        left,
        "and hides again when the cursor leaves"
    );
}

#[gpui_kit::test]
fn showing_marks_never_moves_the_lines_above_the_cursor(cx: &mut TestAppContext) {
    let f = fixture(cx);
    let text = "# Uno\ntesto lungo\n## Due\naltro\n### Tre\nfine";
    let starts: Vec<usize> = std::iter::once(0)
        .chain(text.match_indices('\n').map(|(i, _)| i + 1))
        .collect();
    editor_with(cx, &f, text, text.len());
    let tops = |cx: &mut TestAppContext| -> Vec<f32> {
        starts.iter().map(|&o| caret_at(cx, &f, o).1).collect()
    };
    let mut before = tops(cx);
    // Up through every line: the lines above the cursor stay where they were.
    for line in (0..starts.len() - 1).rev() {
        press(cx, &f, "up");
        let now = tops(cx);
        assert_eq!(now[..line], before[..line], "moving onto line {line}");
        before = now;
    }
}

// ---- Mouse ----

fn mouse_down(cx: &mut TestAppContext, f: &Fixture, at: (f32, f32), count: usize, shift: bool) {
    let modifiers = gpui_kit::Modifiers {
        shift,
        ..Default::default()
    };
    mouse_down_with(cx, f, at, count, modifiers);
}

fn mouse_down_with(
    cx: &mut TestAppContext,
    f: &Fixture,
    at: (f32, f32),
    count: usize,
    modifiers: gpui_kit::Modifiers,
) {
    use gpui_kit::{MouseButton, MouseDownEvent, PlatformInput};
    ui(cx, f, |window, cx| {
        window.dispatch_event(
            PlatformInput::MouseDown(MouseDownEvent {
                button: MouseButton::Left,
                position: gpui_kit::point(px(at.0), px(at.1)),
                modifiers,
                click_count: count,
                first_mouse: false,
            }),
            cx,
        );
    });
    frame(cx, f);
}

fn mouse_move(cx: &mut TestAppContext, f: &Fixture, at: (f32, f32)) {
    use gpui_kit::{MouseButton, MouseMoveEvent, PlatformInput};
    ui(cx, f, |window, cx| {
        window.dispatch_event(
            PlatformInput::MouseMove(MouseMoveEvent {
                position: gpui_kit::point(px(at.0), px(at.1)),
                pressed_button: Some(MouseButton::Left),
                modifiers: Default::default(),
            }),
            cx,
        );
    });
    frame(cx, f);
}

fn mouse_up(cx: &mut TestAppContext, f: &Fixture, at: (f32, f32)) {
    use gpui_kit::{MouseButton, MouseUpEvent, PlatformInput};
    ui(cx, f, |window, cx| {
        window.dispatch_event(
            PlatformInput::MouseUp(MouseUpEvent {
                button: MouseButton::Left,
                position: gpui_kit::point(px(at.0), px(at.1)),
                modifiers: Default::default(),
                click_count: 1,
            }),
            cx,
        );
    });
    frame(cx, f);
}

fn click(cx: &mut TestAppContext, f: &Fixture, at: (f32, f32), count: usize) {
    mouse_down(cx, f, at, count, false);
    mouse_up(cx, f, at);
}

/// A point just right of the caret at `offset`, halfway down its row.
fn point_at(cx: &mut TestAppContext, f: &Fixture, offset: usize) -> (f32, f32) {
    frame(cx, f);
    app(cx, f, |a, _, cx| {
        let b = a.editor().read(cx).caret_bounds(offset).expect("a layout");
        (
            f32::from(b.origin.x) + 1.,
            f32::from(b.origin.y + b.size.height / 2.),
        )
    })
}

fn pane(cx: &mut TestAppContext, f: &Fixture) -> gpui_kit::Bounds<gpui_kit::Pixels> {
    ui(cx, f, |window, _| window.find("editor-pane").bounds())
}

fn selection(cx: &mut TestAppContext, f: &Fixture) -> std::ops::Range<usize> {
    app(cx, f, |a, _, cx| a.editor().read(cx).selected_range())
}

#[gpui_kit::test]
fn a_click_on_a_heading_lands_before_the_letter_clicked(cx: &mut TestAppContext) {
    let f = fixture(cx);
    // Off the cursor's line, "## " is hidden; the click on "t" of "Titolo"
    // shows it, and the cursor is still before that "t".
    editor_with(cx, &f, "## Titolo\ntesto", 12);
    let t = "## Ti".len();
    let at = point_at(cx, &f, t);
    click(cx, &f, at, 1);
    assert_eq!(editor_state(cx, &f).1, t);
}

#[gpui_kit::test]
fn a_click_past_a_line_or_below_the_text_goes_to_its_end(cx: &mut TestAppContext) {
    let f = fixture(cx);
    editor_with(cx, &f, "corta\nseconda riga\nfine", 0);
    let (_, y) = point_at(cx, &f, 0);
    let right = f32::from(pane(cx, &f).right()) - 10.;
    click(cx, &f, (right, y), 1);
    assert_eq!(editor_state(cx, &f).1, "corta".len());
    let (x, y) = point_at(cx, &f, "corta\nseconda riga\nfi".len());
    click(cx, &f, (x, y + 300.), 1);
    let (text, cursor) = editor_state(cx, &f);
    assert_eq!(cursor, text.len());
}

#[gpui_kit::test]
fn a_click_on_a_wrapped_row_lands_on_that_row(cx: &mut TestAppContext) {
    let f = fixture(cx);
    let long = "parola ".repeat(200);
    editor_with(cx, &f, &long, 0);
    press(cx, &f, "down");
    let second_row = editor_state(cx, &f).1;
    press(cx, &f, "cmd-up");
    let at = point_at(cx, &f, second_row + 3);
    click(cx, &f, at, 1);
    assert_eq!(editor_state(cx, &f).1, second_row + 3);
}

#[gpui_kit::test]
fn double_and_triple_clicks_select_a_word_and_a_line(cx: &mut TestAppContext) {
    let f = fixture(cx);
    editor_with(cx, &f, "uno perché tre\nseconda", 0);
    let at = point_at(cx, &f, "uno per".len());
    click(cx, &f, at, 2);
    assert_eq!(selection(cx, &f), 4..4 + "perché".len());
    click(cx, &f, at, 3);
    assert_eq!(selection(cx, &f), 0.."uno perché tre\n".len());
}

#[gpui_kit::test]
fn drag_and_shift_click_select(cx: &mut TestAppContext) {
    let f = fixture(cx);
    editor_with(cx, &f, "uno due tre\nquattro", 0);
    let from = point_at(cx, &f, 4);
    let to = point_at(cx, &f, 7);
    mouse_down(cx, &f, from, 1, false);
    mouse_move(cx, &f, to);
    mouse_up(cx, &f, to);
    assert_eq!(selection(cx, &f), 4..7);
    let further = point_at(cx, &f, "uno due tre\nquat".len());
    mouse_down(cx, &f, further, 1, true);
    mouse_up(cx, &f, further);
    assert_eq!(selection(cx, &f), 4.."uno due tre\nquat".len());
}

#[gpui_kit::test]
fn dragging_below_the_view_scrolls_and_extends_the_selection(cx: &mut TestAppContext) {
    let f = fixture(cx);
    let text: String = (0..300).map(|i| format!("Riga {i}\n")).collect();
    editor_with(cx, &f, &text, 0);
    let start = point_at(cx, &f, 0);
    mouse_down(cx, &f, start, 1, false);
    let below = (start.0, f32::from(pane(cx, &f).bottom()) + 10.);
    mouse_move(cx, &f, below);
    let first = selection(cx, &f).end;
    for _ in 0..20 {
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(16));
        cx.run_until_parked();
        frame(cx, &f);
    }
    let scrolled = app(cx, &f, |a, _, cx| {
        f32::from(-a.editor().read(cx).scroll_offset().y)
    });
    assert!(scrolled > 0., "the view scrolled down");
    assert!(selection(cx, &f).end > first, "the selection grew with it");
    mouse_up(cx, &f, below);
    let after_up = scrolled;
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(200));
    cx.run_until_parked();
    let now = app(cx, &f, |a, _, cx| {
        f32::from(-a.editor().read(cx).scroll_offset().y)
    });
    assert_eq!(now, after_up, "and stops when the button is released");
}

// ---- The test note (tests/fixtures/rendering-test.md) ----

const RENDERING: &str = include_str!("fixtures/rendering-test.md");

#[gpui_kit::test]
fn walking_down_the_test_note_never_moves_the_lines_above(cx: &mut TestAppContext) {
    let f = fixture(cx);
    let starts: Vec<usize> = std::iter::once(0)
        .chain(RENDERING.match_indices('\n').map(|(i, _)| i + 1))
        .filter(|&o| o < RENDERING.len())
        .collect();
    editor_with(cx, &f, RENDERING, 0);
    let mut tops: Vec<f32> = Vec::new();
    for line in 0..starts.len() {
        // The top of every line up to this one, in the text (the view scrolls
        // to follow the cursor), the cursor on this one.
        frame(cx, &f);
        let all: Vec<f32> = app(cx, &f, |a, _, cx| {
            let ed = a.editor().read(cx);
            let scrolled = -ed.scroll_offset().y;
            starts
                .iter()
                .map(|&o| f32::from(ed.caret_bounds(o).unwrap().origin.y + scrolled))
                .collect()
        });
        let now = all[..line].to_vec();
        let moved = now.iter().zip(&tops).any(|(a, b)| (a - b).abs() > 0.5); // the scroll is rounded to pixels
        assert!(
            !moved,
            "the cursor came onto line {line}: {now:?} vs {tops:?}"
        );
        tops = now;
        tops.push(all[line]);
        press(cx, &f, "down");
        // ↓ moves by wrapped row: put the cursor at the next line's start.
        app(cx, &f, |a, _, cx| {
            let next = starts.get(line + 1).copied().unwrap_or(RENDERING.len());
            a.editor()
                .clone()
                .update(cx, |ed, cx| ed.set_selected_range(next..next, cx))
        });
    }
}

#[gpui_kit::test]
fn emphasis_marks_are_hidden_off_the_element_and_shown_in_it(cx: &mut TestAppContext) {
    let f = fixture(cx);
    let text = "Questo testo è **in grassetto**.\naltro";
    let open = text.find("**").unwrap();
    editor_with(cx, &f, text, text.len());
    // Off the element, the `**` takes no room: "è " is followed by "in".
    assert_eq!(caret_at(cx, &f, open).0, caret_at(cx, &f, open + 2).0);
    // The cursor in the bold shows its marks.
    app(cx, &f, |a, _, cx| {
        a.editor()
            .clone()
            .update(cx, |ed, cx| ed.set_selected_range(open + 4..open + 4, cx))
    });
    assert!(caret_at(cx, &f, open + 2).0 > caret_at(cx, &f, open).0);
}

#[gpui_kit::test]
fn a_rule_keeps_its_height_with_the_cursor_on_it_or_not(cx: &mut TestAppContext) {
    let f = fixture(cx);
    let text = "sopra\n\n---\n\nsotto";
    editor_with(cx, &f, text, 0);
    let off = heights(cx, &f);
    let rule = text.find("---").unwrap();
    app(cx, &f, |a, _, cx| {
        a.editor()
            .clone()
            .update(cx, |ed, cx| ed.set_selected_range(rule..rule, cx))
    });
    assert_eq!(heights(cx, &f), off);
}

#[gpui_kit::test]
fn a_quote_hides_its_marks_and_indents_by_its_depth(cx: &mut TestAppContext) {
    let f = fixture(cx);
    let text = "testo\n> uno\n> > due";
    editor_with(cx, &f, text, 0);
    let left = caret_at(cx, &f, 0).0;
    let one = caret_at(cx, &f, text.find("uno").unwrap()).0;
    let two = caret_at(cx, &f, text.find("due").unwrap()).0;
    assert!(
        one > left && two > one,
        "indented by depth: {left} {one} {two}"
    );
    // On its line the `> ` shows, after the bar's indent.
    let start = text.find("> uno").unwrap();
    app(cx, &f, |a, _, cx| {
        a.editor()
            .clone()
            .update(cx, |ed, cx| ed.set_selected_range(start..start, cx))
    });
    assert!(caret_at(cx, &f, start + 2).0 > caret_at(cx, &f, start).0);
    assert_eq!(
        caret_at(cx, &f, start).0,
        one,
        "the text keeps the bar's indent"
    );
}

#[gpui_kit::test]
fn list_items_are_indented_and_wrap_under_their_text(cx: &mut TestAppContext) {
    let f = fixture(cx);
    let long = "parola ".repeat(60);
    let text = format!("testo\n- uno\n  - sotto\n- {long}");
    editor_with(cx, &f, &text, 0);
    let left = caret_at(cx, &f, 0).0;
    let uno = caret_at(cx, &f, text.find("uno").unwrap()).0;
    let sotto = caret_at(cx, &f, text.find("sotto").unwrap()).0;
    assert!(uno > left && sotto > uno, "{left} {uno} {sotto}");
    // The long item's second row starts under its text, not under the bullet.
    let item = text.rfind("- ").unwrap() + 2;
    let (x0, y0) = caret_at(cx, &f, item);
    frame(cx, &f);
    let carets: Vec<(f32, f32)> = app(cx, &f, |a, _, cx| {
        let ed = a.editor().read(cx);
        (item..=text.len())
            .filter(|&o| text.is_char_boundary(o))
            .map(|o| {
                let b = ed.caret_bounds(o).unwrap();
                (f32::from(b.origin.x), f32::from(b.origin.y))
            })
            .collect()
    });
    let y1 = carets
        .iter()
        .map(|c| c.1)
        .find(|&y| y > y0)
        .expect("the item wraps");
    let row_left = carets
        .iter()
        .filter(|c| c.1 == y1)
        .map(|c| c.0)
        .fold(f32::MAX, f32::min);
    assert!(
        (row_left - x0).abs() < 1.,
        "second row at {row_left}, text at {x0}"
    );
}

#[gpui_kit::test]
fn a_click_on_a_checkbox_toggles_the_task_and_keeps_the_cursor(cx: &mut TestAppContext) {
    let f = fixture(cx);
    let text = "testo\n- [ ] da fare\n- [x] fatto";
    editor_with(cx, &f, text, 2);
    let left = caret_at(cx, &f, 0).0;
    let checkbox = |cx: &mut TestAppContext, line_start: usize| {
        frame(cx, &f);
        let b = app(cx, &f, |a, _, cx| {
            a.editor().read(cx).caret_bounds(line_start + 6).unwrap()
        });
        (left + 9., f32::from(b.origin.y + b.size.height / 2.))
    };
    let first = text.find("- [ ]").unwrap();
    let at = checkbox(cx, first);
    click(cx, &f, at, 1);
    assert_eq!(
        editor_state(cx, &f),
        (text.replace("- [ ] da", "- [x] da"), 2)
    );
    assert_eq!(status(cx, &f), "Edited");
    let second = text.find("- [x] fatto").unwrap();
    let at = checkbox(cx, second);
    click(cx, &f, at, 1);
    assert!(editor_state(cx, &f).0.ends_with("- [ ] fatto"));
}

#[gpui_kit::test]
fn a_code_block_shows_its_fences_only_while_the_cursor_is_in_it(cx: &mut TestAppContext) {
    let f = fixture(cx);
    let text = "testo\n```swift\nlet a = 1\nprint(a)\n```\nfine";
    let open_end = text.find("swift").unwrap() + "swift".len();
    let close_end = text.rfind("```").unwrap() + 3;
    editor_with(cx, &f, text, 0);
    let out = heights(cx, &f);
    let hidden_open = caret_at(cx, &f, open_end).0;
    let hidden_close = caret_at(cx, &f, close_end).0;
    // Into the block's code: both fences show, nothing changes height.
    let body = text.find("print").unwrap();
    app(cx, &f, |a, _, cx| {
        a.editor()
            .clone()
            .update(cx, |ed, cx| ed.set_selected_range(body..body, cx))
    });
    assert_eq!(heights(cx, &f), out);
    assert!(
        caret_at(cx, &f, open_end).0 > hidden_open,
        "the opening fence shows"
    );
    assert!(
        caret_at(cx, &f, close_end).0 > hidden_close,
        "the closing fence shows"
    );
    // Out again: hidden.
    press(cx, &f, "cmd-down");
    assert_eq!(caret_at(cx, &f, open_end).0, hidden_open);
}

#[gpui_kit::test]
fn a_click_on_a_wikilink_opens_its_note_and_cmd_click_a_new_tab(cx: &mut TestAppContext) {
    let f = fixture(cx);
    let text = "Vai a [[Reading list]] ora";
    editor_with(cx, &f, text, 0);
    let at = point_at(cx, &f, text.find("Reading").unwrap() + 3);
    let tabs = app(cx, &f, |a, _, _| a.tabs().len());
    mouse_down_with(
        cx,
        &f,
        at,
        1,
        gpui_kit::Modifiers {
            platform: true,
            ..Default::default()
        },
    );
    mouse_up(cx, &f, at);
    assert_eq!(
        app(cx, &f, |a, _, _| a.tabs().len()),
        tabs + 1,
        "⌘-click: a new Tab"
    );
    assert_eq!(
        app(cx, &f, |a, _, _| a.active_path().map(str::to_string)),
        Some("Reading list.md".into())
    );
    // A plain click opens it in the current Tab.
    editor_with(cx, &f, text, 0);
    let at = point_at(cx, &f, text.find("Reading").unwrap() + 3);
    let tabs = app(cx, &f, |a, _, _| a.tabs().len());
    click(cx, &f, at, 1);
    assert_eq!(app(cx, &f, |a, _, _| a.tabs().len()), tabs);
    assert_eq!(
        app(cx, &f, |a, _, _| a.active_path().map(str::to_string)),
        Some("Reading list.md".into())
    );
}

#[gpui_kit::test]
fn a_click_on_a_link_opens_its_url_unless_the_cursor_is_in_it(cx: &mut TestAppContext) {
    let f = fixture(cx);
    let text = "Visita [GitHub](https://github.com) oggi";
    let label = text.find("GitHub").unwrap();
    editor_with(cx, &f, text, 0);
    let at = point_at(cx, &f, label + 2);
    click(cx, &f, at, 1);
    assert_eq!(cx.opened_url(), Some("https://github.com".to_string()));
    assert_eq!(editor_state(cx, &f).1, 0, "the cursor did not move");
    // With the cursor in the link (its marks showing), a click only moves it.
    app(cx, &f, |a, _, cx| {
        a.editor()
            .clone()
            .update(cx, |ed, cx| ed.set_selected_range(label..label, cx))
    });
    let at = point_at(cx, &f, label + 4);
    click(cx, &f, at, 1);
    assert_eq!(editor_state(cx, &f).1, label + 4);
}

#[gpui_kit::test]
fn an_image_embed_is_drawn_as_the_image_and_shows_its_source_with_the_cursor(
    cx: &mut TestAppContext,
) {
    let f = fixture(cx);
    let text = "testo\n![[noteki.png]]\n![[manca.png]]\nfine";
    editor_with(cx, &f, text, 0);
    let off = heights(cx, &f);
    assert!(off[1] > off[0] * 3., "the image's height: {off:?}");
    assert_eq!(off[2], off[0], "a missing image stays one line of source");
    // The cursor onto the image's line: its source shows above the image;
    // the line above does not move.
    let top0 = caret_at(cx, &f, 0).1;
    press(cx, &f, "down");
    let on = heights(cx, &f);
    assert!(on[1] > off[1], "source row added: {off:?} → {on:?}");
    assert_eq!(caret_at(cx, &f, 0).1, top0);
    let line = text.find("![[").unwrap();
    let cursor = editor_state(cx, &f).1;
    assert!(
        (line..=line + "![[noteki.png]]".len()).contains(&cursor),
        "{cursor}"
    );
}

#[gpui_kit::test]
fn a_table_is_drawn_as_a_grid_in_place(cx: &mut TestAppContext) {
    let f = fixture(cx);
    let text = "testo\n| Nome | Tipo | Stato |\n| :--- | :---: | ---: |\n| Markdown | Testo | OK |\n| Swift | Codice | TODO |\nfine";
    let at = |s: &str| text.find(s).unwrap();
    editor_with(cx, &f, text, 0);
    let h = heights(cx, &f);
    assert!(
        [1, 3, 4].iter().all(|&i| h[i] == h[0]),
        "every row one line: {h:?}"
    );
    assert!(h[2] < h[0] / 4., "the delimiter row is only a rule: {h:?}");
    // Left: the cells start together; right: they end together; centre: their
    // middles meet.
    assert_eq!(
        caret_at(cx, &f, at("Markdown")).0,
        caret_at(cx, &f, at("Swift")).0
    );
    assert_eq!(
        caret_at(cx, &f, at("OK") + 2).0,
        caret_at(cx, &f, at("TODO") + 4).0
    );
    let mid = |cx: &mut TestAppContext, s: &str| {
        (caret_at(cx, &f, at(s)).0 + caret_at(cx, &f, at(s) + s.len()).0) / 2.
    };
    assert!((mid(cx, "Testo") - mid(cx, "Codice")).abs() < 1.);
    // The cursor's row shows its source; the other rows stay a grid.
    let row = at("| Markdown");
    app(cx, &f, |a, _, cx| {
        a.editor()
            .clone()
            .update(cx, |ed, cx| ed.set_selected_range(row..row, cx))
    });
    assert!(
        caret_at(cx, &f, at("Markdown")).0 > caret_at(cx, &f, row).0,
        "`| ` shows"
    );
    assert_eq!(heights(cx, &f), h, "nothing changes height");
}

// ---- Undo ----

#[gpui_kit::test]
fn cmd_z_undoes_a_run_of_typing_and_shift_cmd_z_redoes_it(cx: &mut TestAppContext) {
    let f = fixture(cx);
    editor_with(cx, &f, "uno", 3);
    type_in_editor(cx, &f, " due");
    press(cx, &f, "left");
    type_in_editor(cx, &f, "X");
    assert_eq!(editor_state(cx, &f).0, "uno duXe");
    press(cx, &f, "cmd-z");
    assert_eq!(editor_state(cx, &f), ("uno due".into(), 6));
    press(cx, &f, "cmd-z");
    assert_eq!(editor_state(cx, &f), ("uno".into(), 3));
    press(cx, &f, "cmd-shift-z");
    assert_eq!(editor_state(cx, &f).0, "uno due");
    assert_eq!(status(cx, &f), "Edited");
}

#[gpui_kit::test]
fn each_tab_keeps_its_own_undo(cx: &mut TestAppContext) {
    let f = fixture(cx);
    app(cx, &f, |a, w, cx| {
        a.open_path("Ideas.md", OpenIn::CurrentTab, w, cx)
    });
    let ideas = app(cx, &f, |a, _, cx| a.editor_text(cx));
    type_in_editor(cx, &f, "Idea ");
    app(cx, &f, |a, w, cx| {
        a.open_path("Reading list.md", OpenIn::NewTab, w, cx)
    });
    type_in_editor(cx, &f, "Libro ");
    app(cx, &f, |a, w, cx| a.activate_tab(0, w, cx));
    app(cx, &f, |a, w, cx| {
        a.editor().clone().update(cx, |ed, cx| ed.focus(w, cx))
    });
    press(cx, &f, "cmd-z");
    assert_eq!(app(cx, &f, |a, _, cx| a.editor_text(cx)), ideas);
    press(cx, &f, "cmd-z");
    assert_eq!(
        app(cx, &f, |a, _, cx| a.editor_text(cx)),
        ideas,
        "nothing else to undo here"
    );
}

#[gpui_kit::test]
fn a_checkbox_click_is_undone_on_its_own(cx: &mut TestAppContext) {
    let f = fixture(cx);
    let text = "testo\n- [ ] da fare";
    editor_with(cx, &f, text, 2);
    type_in_editor(cx, &f, "X");
    let left = caret_at(cx, &f, 0).0;
    let b = {
        frame(cx, &f);
        app(cx, &f, |a, _, cx| {
            a.editor()
                .read(cx)
                .caret_bounds(text.len() + 1 - 4)
                .unwrap()
        })
    };
    click(
        cx,
        &f,
        (left + 9., f32::from(b.origin.y + b.size.height / 2.)),
        1,
    );
    assert!(editor_state(cx, &f).0.ends_with("[x] da fare"));
    press(cx, &f, "cmd-z");
    assert_eq!(
        editor_state(cx, &f).0,
        "teXsto\n- [ ] da fare",
        "only the tick undone"
    );
}

// ---- Format commands ----

#[gpui_kit::test]
fn cmd_b_toggles_bold_on_the_selection_and_the_word_at_the_cursor(cx: &mut TestAppContext) {
    let f = fixture(cx);
    editor_with(cx, &f, "uno due tre", 5);
    press(cx, &f, "cmd-b");
    assert_eq!(editor_state(cx, &f).0, "uno **due** tre");
    press(cx, &f, "cmd-b");
    assert_eq!(editor_state(cx, &f).0, "uno due tre", "again, the marks go");
    press(cx, &f, "cmd-i cmd-z");
    assert_eq!(editor_state(cx, &f).0, "uno due tre", "one undo step");
}

#[gpui_kit::test]
fn cmd_number_makes_a_heading_and_cmd_enter_a_task(cx: &mut TestAppContext) {
    let f = fixture(cx);
    editor_with(cx, &f, "Titolo\ncomprare pane", 2);
    press(cx, &f, "cmd-2");
    assert_eq!(editor_state(cx, &f), ("## Titolo\ncomprare pane".into(), 5));
    press(cx, &f, "cmd-2");
    assert_eq!(editor_state(cx, &f).0, "Titolo\ncomprare pane");
    press(cx, &f, "down cmd-enter");
    assert_eq!(editor_state(cx, &f).0, "Titolo\n- [ ] comprare pane");
    press(cx, &f, "cmd-enter");
    assert_eq!(editor_state(cx, &f).0, "Titolo\n- [x] comprare pane");
    press(cx, &f, "cmd-] ");
    assert_eq!(editor_state(cx, &f).0, "Titolo\n\t- [x] comprare pane");
    press(cx, &f, "cmd-[");
    assert_eq!(editor_state(cx, &f).0, "Titolo\n- [x] comprare pane");
    // Each command one undo step: outdent, indent, tick.
    press(cx, &f, "cmd-z cmd-z cmd-z");
    assert_eq!(editor_state(cx, &f).0, "Titolo\n- [ ] comprare pane");
}

// ---- Find ----

fn find_status(cx: &mut TestAppContext, f: &Fixture) -> String {
    frame(cx, f);
    ui(cx, f, |window, _| {
        window
            .find("find-status")
            .label()
            .unwrap_or_default()
            .to_string()
    })
}

#[gpui_kit::test]
fn cmd_f_finds_the_word_at_the_cursor_and_cmd_g_goes_through_the_matches(cx: &mut TestAppContext) {
    let f = fixture(cx);
    let text = "nota uno, Nota due, notaio, nota tre";
    editor_with(cx, &f, text, 1);
    press(cx, &f, "cmd-f");
    assert_eq!(
        app(cx, &f, |a, _, cx| a.editor().read(cx).selected_range()),
        0..4
    );
    assert_eq!(find_status(cx, &f), "1 of 4");
    press(cx, &f, "cmd-g");
    assert_eq!(
        app(cx, &f, |a, _, cx| a.editor().read(cx).selected_range()),
        10..14
    );
    press(cx, &f, "cmd-shift-g cmd-shift-g");
    assert_eq!(
        app(cx, &f, |a, _, cx| a.editor().read(cx).selected_range()),
        28..32,
        "previous wraps to the last"
    );
    ui(cx, &f, |w, cx| w.click("find-word", cx));
    assert_eq!(find_status(cx, &f), "3 of 3", "whole words: not notaio");
    press(cx, &f, "escape");
    assert!(
        ui(cx, &f, |w, _| w.try_find("find-bar").is_none()),
        "Esc closes the bar"
    );
    type_in_editor(cx, &f, "X");
    assert!(
        editor_state(cx, &f).0.contains('X'),
        "the text has the focus again"
    );
}

#[gpui_kit::test]
fn replace_all_is_one_undo_step(cx: &mut TestAppContext) {
    let f = fixture(cx);
    editor_with(cx, &f, "uno due uno tre uno", 0);
    press(cx, &f, "cmd-f");
    ui(cx, &f, |w, _| w.find("find-bar").bounds()); // the bar is drawn
                                                    // Typing in the find field: the query (it was seeded with "uno").
    app(cx, &f, |a, w, cx| {
        a.editor()
            .clone()
            .update(cx, |ed, cx| ed.replace_with_for_test("1", w, cx))
    });
    ui(cx, &f, |w, cx| w.click("replace-all", cx));
    assert_eq!(editor_state(cx, &f).0, "1 due 1 tre 1");
    app(cx, &f, |a, w, cx| {
        a.editor().clone().update(cx, |ed, cx| ed.focus(w, cx))
    });
    press(cx, &f, "cmd-z");
    assert_eq!(editor_state(cx, &f).0, "uno due uno tre uno");
}

#[gpui_kit::test]
fn alt_cmd_f_opens_find_with_the_keyboard_in_replace(cx: &mut TestAppContext) {
    let f = fixture(cx);
    editor_with(cx, &f, "uno due uno", 1);
    press(cx, &f, "alt-cmd-f");
    frame(cx, &f);
    // The query is the word at the cursor; what is typed goes to Replace.
    ui(cx, &f, |w, cx| w.input("1", cx));
    ui(cx, &f, |w, cx| w.click("replace-all", cx));
    assert_eq!(editor_state(cx, &f).0, "1 due 1");
}

#[gpui_kit::test]
fn cmd_d_selects_the_occurrences_and_typing_changes_them_all(cx: &mut TestAppContext) {
    let f = fixture(cx);
    editor_with(cx, &f, "uno due uno tre uno", 1);
    press(cx, &f, "cmd-d cmd-d cmd-d");
    type_in_editor(cx, &f, "1");
    assert_eq!(editor_state(cx, &f).0, "1 due 1 tre 1");
    press(cx, &f, "cmd-z");
    assert_eq!(
        editor_state(cx, &f).0,
        "uno due uno tre uno",
        "one undo step"
    );
    // Find's "All" does the same from the find bar.
    press(cx, &f, "cmd-f");
    ui(cx, &f, |w, cx| w.click("find-all", cx));
    type_in_editor(cx, &f, "U");
    assert_eq!(editor_state(cx, &f).0, "U due U tre U");
    press(cx, &f, "escape");
    type_in_editor(cx, &f, "!");
    assert_eq!(
        editor_state(cx, &f).0.matches('!').count(),
        1,
        "Esc: one selection again"
    );
}

#[gpui_kit::test]
fn cmd_plus_and_minus_change_the_text_size_and_cmd_0_resets_it(cx: &mut TestAppContext) {
    let f = fixture(cx);
    editor_with(cx, &f, "testo\naltro", 0);
    let normal = heights(cx, &f)[0];
    press(cx, &f, "cmd-= cmd-=");
    let larger = heights(cx, &f)[0];
    assert!(larger > normal, "{normal} → {larger}");
    let saved = std::fs::read_to_string(f.dir.join("settings.json")).unwrap();
    assert!(saved.contains("\"font_size\": 17"), "remembered: {saved}");
    press(cx, &f, "cmd--");
    assert!(heights(cx, &f)[0] < larger);
    press(cx, &f, "cmd-0");
    assert_eq!(heights(cx, &f)[0], normal);
}

// ---- Tabs and files ----

#[gpui_kit::test]
fn a_middle_click_closes_the_tab_under_it(cx: &mut TestAppContext) {
    use gpui_kit::{Modifiers, MouseButton, MouseDownEvent, PlatformInput};
    let f = fixture(cx);
    app(cx, &f, |a, w, cx| {
        a.open_path("Ideas.md", OpenIn::NewTab, w, cx)
    });
    app(cx, &f, |a, w, cx| {
        a.open_path("Reading list.md", OpenIn::NewTab, w, cx)
    });
    frame(cx, &f);
    let before = app(cx, &f, |a, _, _| a.tabs().len());
    let b = ui(cx, &f, |w, _| w.find(("tab-label", 1usize)).bounds());
    ui(cx, &f, |window, cx| {
        window.dispatch_event(
            PlatformInput::MouseDown(MouseDownEvent {
                button: MouseButton::Middle,
                position: b.center(),
                modifiers: Modifiers::default(),
                click_count: 1,
                first_mouse: false,
            }),
            cx,
        );
    });
    frame(cx, &f);
    let paths: Vec<String> = app(cx, &f, |a, _, _| {
        a.tabs().iter().map(|t| t.path.clone()).collect()
    });
    assert_eq!(paths.len(), before - 1);
    assert!(
        !paths.contains(&"Ideas.md".to_string()),
        "the middle-clicked Tab: {paths:?}"
    );
}

#[gpui_kit::test]
fn tab_menu_closes_the_others_and_those_to_the_right(cx: &mut TestAppContext) {
    let f = fixture(cx);
    for p in ["Ideas.md", "Reading list.md", "Welcome.md"] {
        app(cx, &f, |a, w, cx| a.open_path(p, OpenIn::NewTab, w, cx));
    }
    let n = app(cx, &f, |a, _, _| a.tabs().len());
    app(cx, &f, |a, w, cx| a.close_tabs_after(n - 2, w, cx));
    assert_eq!(app(cx, &f, |a, _, _| a.tabs().len()), n - 1);
    app(cx, &f, |a, w, cx| a.close_other_tabs(0, w, cx));
    assert_eq!(app(cx, &f, |a, _, _| a.tabs().len()), 1);
}

#[gpui_kit::test]
fn copy_path_writes_the_vault_path_or_the_full_one(cx: &mut TestAppContext) {
    let f = fixture(cx);
    app(cx, &f, |a, _, cx| {
        a.copy_path("Daily/2026-11-01.md", false, cx)
    });
    assert_eq!(
        cx.read_from_clipboard().and_then(|c| c.text()),
        Some("Daily/2026-11-01.md".to_string())
    );
    app(cx, &f, |a, _, cx| a.copy_path("Ideas.md", true, cx));
    let full = cx.read_from_clipboard().and_then(|c| c.text()).unwrap();
    assert_eq!(full, f.vault.join("Ideas.md").to_string_lossy());
}

#[gpui_kit::test]
fn a_right_click_on_a_file_and_on_a_tab_opens_a_menu(cx: &mut TestAppContext) {
    use gpui_kit::test::TestQueryExt;
    let f = fixture(cx);
    app(cx, &f, |a, w, cx| {
        a.open_path("Ideas.md", OpenIn::CurrentTab, w, cx)
    });
    frame(cx, &f);
    ui(cx, &f, |w, cx| {
        w.within("file-tree").right_click(("file", 5usize), cx)
    }); // Ideas.md, after the folders
    frame(cx, &f);
    assert!(
        ui(cx, &f, |w, _| w
            .try_find_by_label("Copy full path")
            .is_some()),
        "a menu on the file"
    );
    // "Make a copy" from the menu makes the copy.
    let files = || std::fs::read_dir(&f.vault).unwrap().count();
    let before = files();
    let item = ui(cx, &f, |w, _| {
        w.find_by_label("Make a copy").bounds().center()
    });
    click(cx, &f, (f32::from(item.x), f32::from(item.y)), 1);
    frame(cx, &f);
    assert_eq!(files(), before + 1);
    ui(cx, &f, |w, cx| w.right_click(("tab-label", 0usize), cx));
    frame(cx, &f);
    assert!(
        ui(cx, &f, |w, _| w.try_find_by_label("Close others").is_some()),
        "a menu on the Tab"
    );
}

#[gpui_kit::test]
fn the_mouse_side_buttons_and_cmd_alt_arrows_go_back_and_forward(cx: &mut TestAppContext) {
    use gpui_kit::{Modifiers, MouseButton, MouseDownEvent, NavigationDirection, PlatformInput};
    let f = fixture(cx);
    app(cx, &f, |a, w, cx| {
        a.open_path("Ideas.md", OpenIn::CurrentTab, w, cx)
    });
    app(cx, &f, |a, w, cx| {
        a.open_path("Reading list.md", OpenIn::CurrentTab, w, cx)
    });
    frame(cx, &f);
    let active =
        |cx: &mut TestAppContext| app(cx, &f, |a, _, _| a.active_path().map(str::to_string));
    let side = |cx: &mut TestAppContext, dir| {
        let at = ui(cx, &f, |w, _| w.find("editor-pane").bounds().center());
        ui(cx, &f, |window, cx| {
            window.dispatch_event(
                PlatformInput::MouseDown(MouseDownEvent {
                    button: MouseButton::Navigate(dir),
                    position: at,
                    modifiers: Modifiers::default(),
                    click_count: 1,
                    first_mouse: false,
                }),
                cx,
            );
        });
        frame(cx, &f);
    };
    side(cx, NavigationDirection::Back);
    assert_eq!(active(cx), Some("Ideas.md".into()));
    side(cx, NavigationDirection::Forward);
    assert_eq!(active(cx), Some("Reading list.md".into()));
    // From the editor too.
    app(cx, &f, |a, w, cx| {
        a.editor().clone().update(cx, |ed, cx| ed.focus(w, cx))
    });
    press(cx, &f, "cmd-alt-left");
    assert_eq!(active(cx), Some("Ideas.md".into()));
}

// ---- [[ completion ----

#[gpui_kit::test]
fn typing_double_brackets_offers_notes_and_return_writes_the_link(cx: &mut TestAppContext) {
    let f = fixture(cx);
    editor_with(cx, &f, "Vedi ", 5);
    type_in_editor(cx, &f, "[[Rea");
    frame(cx, &f);
    assert!(
        ui(cx, &f, |w, _| w.try_find("link-completion").is_some()),
        "the list opens"
    );
    press(cx, &f, "enter");
    assert_eq!(editor_state(cx, &f), ("Vedi [[Reading list]]".into(), 21));
    frame(cx, &f);
    assert!(
        ui(cx, &f, |w, _| w.try_find("link-completion").is_none()),
        "and closes"
    );
    // Esc closes it; a click into a link does not open it.
    type_in_editor(cx, &f, " [[Ide");
    press(cx, &f, "escape");
    frame(cx, &f);
    assert!(ui(cx, &f, |w, _| w.try_find("link-completion").is_none()));
    press(cx, &f, "enter");
    assert!(
        editor_state(cx, &f).0.ends_with("[[Ide\n"),
        "Return is a new line again"
    );
}

#[gpui_kit::test]
fn footnotes_setext_headings_and_callouts(cx: &mut TestAppContext) {
    let f = fixture(cx);
    let text = "Titolo\n===\n> [!tip] Consiglio\n> testo\nVedi[^1].\n\n[^1]: la nota";
    editor_with(cx, &f, text, text.len());
    let h = heights(cx, &f);
    assert!(h[0] > h[1], "the setext Heading is larger: {h:?}");
    let left = caret_at(cx, &f, text.find("Vedi").unwrap()).0;
    assert!(
        caret_at(cx, &f, text.find("testo").unwrap()).0 > left,
        "the callout's text is indented"
    );
    // A click on the reference goes to the definition.
    let at = point_at(cx, &f, text.find("[^1].").unwrap() + 2);
    click(cx, &f, at, 1);
    let cursor = editor_state(cx, &f).1;
    assert!(
        text[cursor..].starts_with(" la nota"),
        "at the definition: {cursor}"
    );
}

#[gpui_kit::test]
fn the_frontmatter_is_a_properties_table_until_the_cursor_enters_it(cx: &mut TestAppContext) {
    let f = fixture(cx);
    let text = "---\ntitle: Una nota\ndate: 2026-11-17\n---\ntesto";
    let at = |s: &str| text.find(s).unwrap();
    editor_with(cx, &f, text, text.len());
    let h = heights(cx, &f);
    let (title, date) = (
        caret_at(cx, &f, at("Una")).0,
        caret_at(cx, &f, at("2026")).0,
    );
    assert_eq!(title, date, "values in one column");
    // Into the Frontmatter: the YAML, nothing changes height.
    app(cx, &f, |a, _, cx| {
        a.editor()
            .clone()
            .update(cx, |ed, cx| ed.set_selected_range(5..5, cx))
    });
    assert_ne!(
        caret_at(cx, &f, at("Una")).0,
        caret_at(cx, &f, at("2026")).0
    );
    assert_eq!(heights(cx, &f), h);
    // A Note opens with the cursor after its Frontmatter.
    std::fs::write(f.vault.join("Props.md"), text).unwrap();
    app(cx, &f, |a, w, cx| {
        a.handle_fs_changes(&[f.vault.join("Props.md")], w, cx)
    });
    app(cx, &f, |a, w, cx| {
        a.open_path("Props.md", OpenIn::NewTab, w, cx)
    });
    assert_eq!(editor_state(cx, &f).1, at("testo"));
}

// ---- Pasted and dropped images ----

#[gpui_kit::test]
fn a_pasted_image_is_saved_next_to_the_note_and_embedded(cx: &mut TestAppContext) {
    use gpui_kit::{ClipboardItem, Image, ImageFormat};
    let f = fixture(cx);
    editor_with(cx, &f, "prima ", 6);
    let png = std::fs::read(f.vault.join("Media/noteki.png")).unwrap();
    cx.write_to_clipboard(ClipboardItem::new_image(&Image::from_bytes(
        ImageFormat::Png,
        png,
    )));
    press(cx, &f, "cmd-v");
    let text = editor_state(cx, &f).0;
    assert!(text.starts_with("prima ![[Pasted image "), "{text}");
    let name = text.trim_start_matches("prima ![[").trim_end_matches("]]");
    assert!(
        f.vault.join(name).is_file(),
        "saved next to Ideas.md: {name}"
    );
}

#[gpui_kit::test]
fn paste_and_match_style_pastes_the_text_alone(cx: &mut TestAppContext) {
    use gpui_kit::{ClipboardItem, Image, ImageFormat};
    let f = fixture(cx);
    editor_with(cx, &f, "prima ", 6);
    cx.write_to_clipboard(ClipboardItem::new_string("**testo**".into()));
    press(cx, &f, "cmd-shift-v");
    assert_eq!(editor_state(cx, &f).0, "prima **testo**");
    // An image alone: nothing pasted, nothing saved.
    let png = std::fs::read(f.vault.join("Media/noteki.png")).unwrap();
    cx.write_to_clipboard(ClipboardItem::new_image(&Image::from_bytes(
        ImageFormat::Png,
        png,
    )));
    press(cx, &f, "cmd-shift-v");
    assert_eq!(editor_state(cx, &f).0, "prima **testo**");
}

#[gpui_kit::test]
fn a_dropped_image_file_is_copied_next_to_the_note_and_embedded(cx: &mut TestAppContext) {
    use gpui_kit::{ExternalPaths, FileDropEvent, PlatformInput};
    let f = fixture(cx);
    editor_with(cx, &f, "testo", 5);
    let source = f.dir.join("Schermata.png");
    std::fs::copy(f.vault.join("Media/noteki.png"), &source).unwrap();
    let at = point_at(cx, &f, 5);
    let position = gpui_kit::point(px(at.0), px(at.1));
    ui(cx, &f, |window, cx| {
        window.dispatch_event(
            PlatformInput::FileDrop(FileDropEvent::Entered {
                position,
                paths: ExternalPaths(vec![source.clone()].into()),
            }),
            cx,
        );
        window.dispatch_event(
            PlatformInput::FileDrop(FileDropEvent::Submit { position }),
            cx,
        );
    });
    frame(cx, &f);
    assert_eq!(editor_state(cx, &f).0, "testo![[Schermata.png]]");
    assert!(f.vault.join("Schermata.png").is_file());
}

#[gpui_kit::test]
fn cmd_k_makes_a_link_and_cmd_slash_a_comment(cx: &mut TestAppContext) {
    let f = fixture(cx);
    editor_with(cx, &f, "vedi sito", 6);
    cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(
        "https://example.com".into(),
    ));
    press(cx, &f, "cmd-k");
    assert_eq!(editor_state(cx, &f).0, "vedi [sito](https://example.com)");
    press(cx, &f, "cmd-/");
    assert_eq!(
        editor_state(cx, &f).0,
        "%%vedi [sito](https://example.com)%%"
    );
    press(cx, &f, "cmd-z cmd-z");
    assert_eq!(editor_state(cx, &f).0, "vedi sito");
}

// ---- Keyboard-only dialogs ----

#[gpui_kit::test]
fn quick_open_takes_the_keyboard_at_once(cx: &mut TestAppContext) {
    let f = fixture(cx);
    editor_with(cx, &f, "", 0);
    press(cx, &f, "cmd-o");
    frame(cx, &f);
    ui(cx, &f, |window, cx| window.input("Launch plan", cx));
    frame(cx, &f);
    press(cx, &f, "enter");
    frame(cx, &f);
    assert_eq!(
        app(cx, &f, |a, _, _| a.active_path().map(str::to_string)),
        Some("Projects/Launch plan.md".to_string())
    );
}

#[gpui_kit::test]
fn the_command_palette_takes_the_keyboard_at_once(cx: &mut TestAppContext) {
    let f = fixture(cx);
    editor_with(cx, &f, "", 0);
    let tabs = app(cx, &f, |a, _, _| a.tabs().len());
    press(cx, &f, "cmd-shift-p");
    frame(cx, &f);
    ui(cx, &f, |window, cx| window.input("Close tab", cx));
    frame(cx, &f);
    press(cx, &f, "enter");
    frame(cx, &f);
    assert_eq!(app(cx, &f, |a, _, _| a.tabs().len()), tabs - 1);
}

#[gpui_kit::test]
fn the_name_prompt_takes_the_keyboard_at_once(cx: &mut TestAppContext) {
    let f = fixture(cx);
    editor_with(cx, &f, "", 0);
    press(cx, &f, "cmd-n");
    frame(cx, &f);
    ui(cx, &f, |window, cx| window.input("Zeta", cx));
    frame(cx, &f);
    press(cx, &f, "enter");
    frame(cx, &f);
    assert!(f.vault.join("Zeta.md").exists());
}

#[gpui_kit::test]
fn shortcuts_work_as_soon_as_the_window_opens(cx: &mut TestAppContext) {
    let f = fixture(cx);
    press(cx, &f, "cmd-o");
    frame(cx, &f);
    ui(cx, &f, |window, cx| window.input("Launch plan", cx));
    frame(cx, &f);
    press(cx, &f, "enter");
    frame(cx, &f);
    assert_eq!(
        app(cx, &f, |a, _, _| a.active_path().map(str::to_string)),
        Some("Projects/Launch plan.md".to_string())
    );
    // With every Tab closed, the window still takes the shortcuts.
    let tabs = app(cx, &f, |a, _, _| a.tabs().len());
    for _ in 0..tabs {
        press(cx, &f, "cmd-w");
        frame(cx, &f);
    }
    assert_eq!(app(cx, &f, |a, _, _| a.tabs().len()), 0);
    press(cx, &f, "cmd-o");
    frame(cx, &f);
    ui(cx, &f, |window, cx| window.input("Ideas", cx));
    frame(cx, &f);
    press(cx, &f, "enter");
    frame(cx, &f);
    assert_eq!(
        app(cx, &f, |a, _, _| a.active_path().map(str::to_string)),
        Some("Ideas.md".to_string())
    );
}

#[gpui_kit::test]
fn a_search_result_line_opens_its_note_there(cx: &mut TestAppContext) {
    let f = fixture(cx);
    press(cx, &f, "cmd-shift-f");
    frame(cx, &f);
    ui(cx, &f, |window, cx| window.input("what ships", cx));
    frame(cx, &f);
    assert_eq!(app(cx, &f, |a, _, _| a.search_results().len()), 1);
    ui(cx, &f, |window, cx| {
        window.click(("search-row", 1usize), cx)
    });
    frame(cx, &f);
    let (path, cursor, text) = app(cx, &f, |a, _, cx| {
        let ed = a.editor().read(cx);
        (
            a.active_path().map(str::to_string),
            ed.cursor(),
            ed.text().to_string(),
        )
    });
    assert_eq!(path.as_deref(), Some("Projects/Launch plan.md"));
    assert!(
        text[cursor..].starts_with("From the"),
        "{:?}",
        &text[cursor..]
    );
}

#[gpui_kit::test]
fn the_sidebar_shortcuts_are_the_swift_apps(cx: &mut TestAppContext) {
    use noteki_gpui::app::{LeftPane, RightPane};
    let f = fixture(cx);
    let layout = |cx: &mut TestAppContext| {
        app(cx, &f, |a, _, _| {
            let s = a.settings();
            (
                s.left_sidebar_open,
                s.left_pane,
                s.right_sidebar_open,
                s.right_pane,
            )
        })
    };
    let before = layout(cx);
    press(cx, &f, "ctrl-cmd-s");
    assert_eq!(layout(cx).0, !before.0);
    press(cx, &f, "ctrl-cmd-2");
    assert_eq!((layout(cx).0, layout(cx).1), (true, LeftPane::Tags));
    press(cx, &f, "ctrl-cmd-3");
    assert_eq!(layout(cx).1, LeftPane::Search);
    // From the search field too, which ⌃⌘3 gave the keyboard.
    press(cx, &f, "ctrl-cmd-1");
    assert_eq!(layout(cx).1, LeftPane::Files);
    press(cx, &f, "alt-cmd-2");
    assert_eq!((layout(cx).2, layout(cx).3), (true, RightPane::Backlinks));
    press(cx, &f, "alt-cmd-1");
    assert_eq!(layout(cx).3, RightPane::Outline);
    press(cx, &f, "alt-cmd-0");
    assert!(!layout(cx).2);
}

#[gpui_kit::test]
fn every_shortcut_is_one_a_mac_keyboard_can_send(cx: &mut TestAppContext) {
    let _f = fixture(cx);
    // macOS gives ⇧ with a key that is not a letter as the shifted character:
    // ⇧⌘] arrives as ⌘}, never as `cmd-shift-]`.
    let unreachable: Vec<String> = cx.update(|cx| {
        let keymap = cx.key_bindings();
        let keymap = keymap.borrow();
        keymap
            .bindings()
            .flat_map(|b| b.keystrokes())
            .filter(|k| {
                let key = k.key();
                k.modifiers().shift
                    && key.chars().count() == 1
                    && !key.chars().all(|c| c.is_ascii_alphabetic())
            })
            .map(|k| format!("{:?}+{}", k.modifiers(), k.key()))
            .collect()
    });
    assert!(unreachable.is_empty(), "{unreachable:?}");
}

#[gpui_kit::test]
fn the_menus_show_the_swift_apps_shortcuts(cx: &mut TestAppContext) {
    use noteki_gpui::app::{SearchInVaultAction, SwitchSearchPane};
    let _f = fixture(cx);
    // A menu item shows the first binding of its action.
    let first = |cx: &mut TestAppContext, action: &dyn gpui_kit::Action| {
        cx.update(|cx| {
            let keymap = cx.key_bindings();
            let keymap = keymap.borrow();
            let b = keymap
                .bindings_for_action(action)
                .next()
                .expect("a binding");
            b.keystrokes()
                .iter()
                .map(|k| k.unparse(gpui_kit::OperatingSystem::Mac))
                .collect::<Vec<_>>()
                .join(" ")
        })
    };
    assert_eq!(first(cx, &SwitchSearchPane), "ctrl-cmd-3");
    assert_eq!(first(cx, &SearchInVaultAction), "cmd-shift-f");
}

#[gpui_kit::test]
fn shift_cmd_brackets_change_tab_as_the_mac_sends_them(cx: &mut TestAppContext) {
    let f = fixture(cx);
    app(cx, &f, |a, w, cx| {
        a.open_path("Ideas.md", OpenIn::NewTab, w, cx)
    });
    app(cx, &f, |a, w, cx| {
        a.open_path("Reading list.md", OpenIn::NewTab, w, cx)
    });
    let active =
        |cx: &mut TestAppContext| app(cx, &f, |a, _, _| a.active_path().map(str::to_string));
    press(cx, &f, "cmd-{");
    assert_eq!(active(cx).as_deref(), Some("Ideas.md"));
    press(cx, &f, "cmd-}");
    assert_eq!(active(cx).as_deref(), Some("Reading list.md"));
}

#[gpui_kit::test]
fn the_menu_bar_has_settings_under_the_apps_name(cx: &mut TestAppContext) {
    cx.update(noteki_gpui::app::set_app_menus);
    let menus = cx.update(|cx| cx.get_menus()).expect("a menu bar");
    let names: Vec<String> = menus.iter().map(|m| m.name.to_string()).collect();
    assert_eq!(names[0], "Noteki");
    let first: Vec<String> = menus[0]
        .items
        .iter()
        .filter_map(|i| match i {
            gpui_kit::OwnedMenuItem::Action { name, .. } => Some(name.clone()),
            _ => None,
        })
        .collect();
    assert!(first.contains(&"Settings…".to_string()), "{first:?}");
    assert!(first.contains(&"Quit Noteki".to_string()), "{first:?}");
    for menu in ["File", "Edit", "Format", "View", "Go", "Window", "Help"] {
        assert!(names.contains(&menu.to_string()), "{names:?}");
    }
}
