//! Tauri shell: commands over [`session::Session`], the file watcher, the
//! `noteki-file://` protocol and saving on quit. The behaviour lives in
//! `session` and below, where it is tested without Tauri.

pub mod document;
pub mod embeds;
pub mod frontmatter;
pub mod index;
pub mod markdown;
pub mod merge;
pub mod session;
pub mod vault;
pub mod wikilink;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tauri::{AppHandle, Emitter, Manager, RunEvent};

use frontmatter::{parse_frontmatter, FrontmatterReading};
use index::{BacklinkMatch, SearchResult, TagNode};
use session::{
    AppSettings, CheckLinksResult, Event, FollowLinkResult, OpenNoteResponse, Session,
    VaultSnapshot,
};
use wikilink::LinkCompletions;

struct WatcherHandle {
    _watcher: RecommendedWatcher,
    stop_tx: mpsc::Sender<()>,
}

struct Shared {
    session: Mutex<Session>,
    watcher: Mutex<Option<WatcherHandle>>,
    quitting: AtomicBool,
}

type State<'a> = tauri::State<'a, Arc<Shared>>;

/// A panic in one command must not poison the session for all the others.
fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

fn emit_all(app: &AppHandle, events: Vec<Event>) {
    for event in events {
        if let Event::SaveFailed { path, error } = &event {
            eprintln!("Noteki: could not save {path}: {error}");
        }
        let _ = app.emit(event.name(), &event);
    }
}

fn start_watcher(
    app: AppHandle,
    shared: Arc<Shared>,
    root: PathBuf,
) -> Result<WatcherHandle, String> {
    let (fs_tx, fs_rx) = mpsc::channel::<notify::Event>();
    let (stop_tx, stop_rx) = mpsc::channel::<()>();
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if let Ok(event) = res {
            let _ = fs_tx.send(event);
        }
    })
    .map_err(|e| format!("Failed to start file watcher: {e}"))?;
    watcher
        .watch(&root, RecursiveMode::Recursive)
        .map_err(|e| format!("Failed to watch vault: {e}"))?;

    std::thread::spawn(move || loop {
        if stop_rx.try_recv().is_ok() {
            break;
        }
        let first = match fs_rx.recv_timeout(Duration::from_millis(250)) {
            Ok(ev) => ev,
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        };
        // Coalesce a burst (an atomic save is several events) into one batch.
        let mut batch = vec![first];
        std::thread::sleep(Duration::from_millis(120));
        batch.extend(fs_rx.try_iter());
        if stop_rx.try_recv().is_ok() {
            break;
        }
        let paths: Vec<PathBuf> = batch
            .into_iter()
            .filter(|e| !matches!(e.kind, EventKind::Access(_)))
            .flat_map(|e| e.paths)
            .collect();
        let events = {
            let mut session = lock(&shared.session);
            if session.vault().map(|v| v.root.as_path()) != Some(root.as_path()) {
                break;
            }
            session.fs_changed(&paths)
        };
        emit_all(&app, events);
    });

    Ok(WatcherHandle {
        _watcher: watcher,
        stop_tx,
    })
}

#[tauri::command]
fn get_settings(state: State) -> AppSettings {
    lock(&state.session).settings.clone()
}

#[tauri::command]
fn save_settings(state: State, settings: AppSettings) -> Result<(), String> {
    lock(&state.session).save_settings(settings)
}

#[tauri::command]
fn open_vault(app: AppHandle, state: State, path: String) -> Result<VaultSnapshot, String> {
    let (snapshot, events) = lock(&state.session).open_vault(Path::new(&path))?;
    emit_all(&app, events);
    if let Some(old) = lock(&state.watcher).take() {
        let _ = old.stop_tx.send(());
    }
    let watcher = start_watcher(app, state.inner().clone(), PathBuf::from(&snapshot.root))?;
    *lock(&state.watcher) = Some(watcher);
    Ok(snapshot)
}

#[tauri::command]
fn open_note(state: State, path: String) -> Result<OpenNoteResponse, String> {
    lock(&state.session).open_note(&path)
}

#[tauri::command]
fn update_note_buffer(state: State, path: String, revision: u64, text: String) {
    lock(&state.session).update_buffer(&path, revision, &text);
}

/// Saves the text last reported with `update_note_buffer`.
#[tauri::command]
fn save_note(state: State, path: String) -> Result<bool, String> {
    lock(&state.session).save_note(&path)
}

#[tauri::command]
fn resolve_conflict(state: State, path: String, choice: String) -> Result<String, String> {
    lock(&state.session).resolve_conflict(&path, &choice)
}

#[tauri::command]
fn save_again(state: State, path: String) -> Result<(), String> {
    lock(&state.session).save_again(&path)
}

/// The editor refused a `note-reloaded` because the user typed meanwhile.
#[tauri::command]
fn reload_rejected(app: AppHandle, state: State, path: String) {
    let events = lock(&state.session).reload_rejected(&path);
    emit_all(&app, events);
}

#[tauri::command]
fn create_new_note(state: State, folder_rel: String) -> Result<(String, VaultSnapshot), String> {
    lock(&state.session).create_new_note(&folder_rel)
}

#[tauri::command]
fn check_links(state: State, note_path: String, inners: Vec<String>) -> CheckLinksResult {
    lock(&state.session).check_links(&note_path, inners)
}

#[tauri::command]
fn follow_link(
    state: State,
    source_path: String,
    inner_text: String,
) -> Result<FollowLinkResult, String> {
    lock(&state.session).follow_link(&source_path, &inner_text)
}

#[tauri::command]
fn complete_link(state: State) -> LinkCompletions {
    lock(&state.session).complete_link()
}

#[tauri::command]
fn read_frontmatter(text: String) -> Option<FrontmatterReading> {
    parse_frontmatter(&text)
}

#[tauri::command]
fn search(state: State, query: String) -> Vec<SearchResult> {
    lock(&state.session).search(&query)
}

#[tauri::command]
fn backlinks(state: State, path: String) -> Vec<BacklinkMatch> {
    lock(&state.session).backlinks(&path)
}

#[tauri::command]
fn tags(state: State) -> Vec<TagNode> {
    lock(&state.session).tags()
}

/// The window flushed the editor after `app-will-quit`: save and leave.
#[tauri::command]
fn quit_app(app: AppHandle, state: State) {
    let events = lock(&state.session).flush_all();
    emit_all(&app, events);
    state.quitting.store(true, Ordering::SeqCst);
    app.exit(0);
}

fn mime_for_path(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
        .as_deref()
    {
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("svg") => "image/svg+xml",
        Some("heic") => "image/heic",
        Some("pdf") => "application/pdf",
        _ => "application/octet-stream",
    }
}

/// Preferences file: `NOTEKI_SETTINGS` (smoke tests) or the app's config folder.
fn settings_path(app: &tauri::App) -> PathBuf {
    if let Some(p) = std::env::var_os("NOTEKI_SETTINGS") {
        return PathBuf::from(p);
    }
    app.path()
        .app_config_dir()
        .unwrap_or_else(|_| std::env::temp_dir().join("noteki-tauri"))
        .join("settings.json")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .register_uri_scheme_protocol("noteki-file", |ctx, request| {
            let not_found = || {
                tauri::http::Response::builder()
                    .status(404)
                    .body(Vec::new())
                    .unwrap()
            };
            let shared = ctx.app_handle().state::<Arc<Shared>>();
            let raw = request.uri().path().trim_start_matches('/');
            let raw = raw.strip_prefix("vault/").unwrap_or(raw);
            let rel = urlencoding::decode(raw)
                .map(|c| c.into_owned())
                .unwrap_or_else(|_| raw.to_string());
            let abs = lock(&shared.session)
                .vault()
                .and_then(|v| v.file_path(&rel));
            match abs.and_then(|abs| std::fs::read(&abs).ok().map(|bytes| (abs, bytes))) {
                Some((abs, bytes)) => tauri::http::Response::builder()
                    .status(200)
                    .header("Content-Type", mime_for_path(&abs))
                    .body(bytes)
                    .unwrap(),
                None => not_found(),
            }
        })
        .setup(|app| {
            let mut session = Session::new(Some(settings_path(app)));
            if let Some(vault) = std::env::var_os("NOTEKI_VAULT") {
                session.settings.vault_path =
                    Some(PathBuf::from(vault).to_string_lossy().to_string());
            }
            app.manage(Arc::new(Shared {
                session: Mutex::new(session),
                watcher: Mutex::new(None),
                quitting: AtomicBool::new(false),
            }));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_settings,
            save_settings,
            open_vault,
            open_note,
            update_note_buffer,
            save_note,
            resolve_conflict,
            save_again,
            reload_rejected,
            create_new_note,
            check_links,
            follow_link,
            complete_link,
            read_frontmatter,
            search,
            backlinks,
            tags,
            quit_app,
        ])
        .build(tauri::generate_context!())
        .expect("error while building the Noteki app");

    app.run(|app, event| {
        if let RunEvent::ExitRequested { api, code, .. } = event {
            let shared = app.state::<Arc<Shared>>();
            let window_open = !app.webview_windows().is_empty();
            if code.is_none() && window_open && !shared.quitting.swap(true, Ordering::SeqCst) {
                // ⌘Q with the window open: the editor may hold up to 250 ms of
                // unreported typing. Let the window flush it, then `quit_app`.
                api.prevent_exit();
                let _ = app.emit("app-will-quit", ());
                let app = app.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_secs(2));
                    let shared = app.state::<Arc<Shared>>();
                    let _ = lock(&shared.session).flush_all();
                    app.exit(0);
                });
            } else {
                // Whatever the window did not flush is in the session already.
                let _ = lock(&shared.session).flush_all();
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_panic_while_holding_the_session_does_not_lock_the_app_out() {
        let m = Arc::new(Mutex::new(Session::new(None)));
        let m2 = m.clone();
        let _ = std::thread::spawn(move || {
            let _guard = m2.lock().unwrap();
            panic!("a command panicked");
        })
        .join();
        assert!(m.is_poisoned());
        assert!(lock(&m).search("x").is_empty(), "later commands still work");
    }
}
