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
use noteki_gpui::app::{register_keybindings, AppConfig, NotekiApp, ViewMode, AUTOSAVE_DELAY};
use noteki_gpui::core::markdown::reader_can_parse;
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

// ---- Links and Reader ----

#[gpui_kit::test]
fn reader_links_open_notes_and_back_returns(cx: &mut TestAppContext) {
    let f = fixture(cx);
    app(cx, &f, |a, _, cx| a.set_view_mode(ViewMode::Reader, cx));
    frame(cx, &f);
    assert!(app(cx, &f, |a, _, _| a
        .reader_markdown()
        .unwrap()
        .contains("(noteki-link:Launch%20plan)")));
    app(cx, &f, |a, w, cx| {
        a.follow_reader_link("noteki-link:Launch%20plan", false, w, cx)
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
fn reader_checkbox_toggles_the_right_task(cx: &mut TestAppContext) {
    let f = fixture(cx);
    app(cx, &f, |a, w, cx| {
        a.open_path("Reading list.md", OpenIn::CurrentTab, w, cx)
    });
    app(cx, &f, |a, _, cx| a.set_view_mode(ViewMode::Reader, cx));
    let md = app(cx, &f, |a, _, _| a.reader_markdown().unwrap().to_string());
    let marker = md.find("[ ] *Gödel").unwrap();
    app(cx, &f, |a, w, cx| a.toggle_task(marker, true, w, cx));
    let text = read(&f, "Reading list.md");
    assert!(text.contains("- [x] *Gödel, Escher, Bach*"), "{text}");
    assert!(
        text.contains("- [ ] *The Design of Everyday Things*"),
        "{text}"
    );
}

#[gpui_kit::test]
fn a_note_that_crashes_the_markdown_parser_still_opens_in_the_reader(cx: &mut TestAppContext) {
    let f = fixture(cx);
    std::fs::write(f.vault.join("Crash.md"), "1. ```\n-").unwrap();
    app(cx, &f, |a, w, cx| {
        a.handle_fs_changes(&[f.vault.join("Crash.md")], w, cx)
    });
    app(cx, &f, |a, w, cx| {
        a.open_path("Crash.md", OpenIn::CurrentTab, w, cx)
    });
    app(cx, &f, |a, _, cx| a.set_view_mode(ViewMode::Reader, cx));
    frame(cx, &f); // would panic inside GPUI's TextView without the guard
    let md = app(cx, &f, |a, _, _| a.reader_markdown().unwrap().to_string());
    assert!(md.contains("cannot be shown in the Reader") && reader_can_parse(&md));
}

#[gpui_kit::test]
fn split_view_renders_editor_and_reader(cx: &mut TestAppContext) {
    let f = fixture(cx);
    ui(cx, &f, |w, cx| w.click("view-split", cx));
    frame(cx, &f);
    assert!(ui(cx, &f, |w, _| w.find("reader").visible()
        && w.find("editor-pane").visible()));
    ui(cx, &f, |w, cx| w.click("view-live", cx));
    frame(cx, &f);
    assert!(ui(cx, &f, |w, _| w.try_find("reader").is_none()));
}

// ---- Files ----

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
    app(cx, &f, |a, _, cx| a.set_view_mode(ViewMode::Split, cx));

    let (window, app2) = open_app(cx, &f.dir, &f.vault);
    cx.run_until_parked();
    let (tabs, active, mode) = cx
        .update_window(window, |_, _, cx| {
            let a = app2.read(cx);
            (
                a.tabs().iter().map(|t| t.path.clone()).collect::<Vec<_>>(),
                a.active_path().map(str::to_string),
                a.view_mode(),
            )
        })
        .unwrap();
    assert_eq!(tabs, vec!["Welcome.md", "Projects/Launch plan.md"]);
    assert_eq!(active.as_deref(), Some("Projects/Launch plan.md"));
    assert_eq!(mode, ViewMode::Split);
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
