mod embeds;
mod frontmatter;
mod merge;
mod vault;
mod wikilink;

use embeds::{resolve_embed, EmbeddedFile};
use frontmatter::{parse_frontmatter, FrontmatterReading};
use merge::merge_three_way;
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};
use vault::{
    create_empty_note, is_in_excluded_folder, new_note_path_for_link, next_untitled_path,
    normalize_excluded, read_note, relative_path_from_disk, resolve_vault_path, scan_vault,
    write_note_atomic, AttachmentKind, VaultItem,
};
use wikilink::{LinkCompletions, ResolvedItem, VaultIndex, Wikilink};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub vault_path: Option<String>,
    pub theme: String,
    pub appearance: String,
    pub source_mode: bool,
    pub recent_notes: Vec<String>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            vault_path: None,
            theme: "ayu".to_string(),
            appearance: "system".to_string(),
            source_mode: false,
            recent_notes: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
struct OpenNoteState {
    base: String,
    buffer: String,
    revision: u64,
    conflict: bool,
    deleted: bool,
    last_self_write: Option<Instant>,
}

struct WatcherHandle {
    _watcher: RecommendedWatcher,
    stop_tx: mpsc::Sender<()>,
}

struct InnerState {
    config_path: PathBuf,
    settings: AppSettings,
    vault_root: Option<PathBuf>,
    excluded: HashSet<String>,
    tree: Vec<VaultItem>,
    index: VaultIndex,
    open_notes: HashMap<String, OpenNoteState>,
    watcher: Option<WatcherHandle>,
}

type SharedState = Arc<Mutex<InnerState>>;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultSnapshot {
    pub root: String,
    pub tree: Vec<VaultItem>,
    pub note_count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenNoteResponse {
    pub path: String,
    pub text: String,
    pub conflict: bool,
    pub deleted: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckLinksResult {
    pub unresolved: Vec<String>,
    pub embeds: HashMap<String, EmbeddedFile>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum FollowLinkResult {
    Note { path: String, created: bool },
    Attachment { path: String, attachment_kind: AttachmentKind },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct NoteReloadedPayload {
    path: String,
    revision: u64,
    text: String,
    merged: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct NoteConflictPayload {
    path: String,
    disk_text: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct NoteDeletedPayload {
    path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct VaultTreeChangedPayload {
    tree: Vec<VaultItem>,
    note_count: usize,
}

fn load_settings_from_disk(config_path: &Path) -> AppSettings {
    fs::read_to_string(config_path)
        .ok()
        .and_then(|s| serde_json::from_str::<AppSettings>(&s).ok())
        .unwrap_or_default()
}

fn persist_settings(config_path: &Path, settings: &AppSettings) {
    if let Some(parent) = config_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(settings) {
        let _ = fs::write(config_path, json);
    }
}

fn count_notes(tree: &[VaultItem]) -> usize {
    tree.iter()
        .map(|item| match item {
            VaultItem::Folder { children, .. } => count_notes(children),
            VaultItem::Note { .. } => 1,
            VaultItem::Attachment { .. } => 0,
        })
        .sum()
}

fn start_watcher(app: AppHandle, shared: SharedState, root: PathBuf) -> Result<WatcherHandle, String> {
    let (fs_tx, fs_rx) = mpsc::channel();
    let (stop_tx, stop_rx) = mpsc::channel::<()>();

    let mut watcher = notify::recommended_watcher(move |res| {
        if let Ok(event) = res {
            let _ = fs_tx.send(event);
        }
    })
    .map_err(|e| format!("Failed to start file watcher: {e}"))?;

    watcher
        .watch(&root, RecursiveMode::Recursive)
        .map_err(|e| format!("Failed to watch vault: {e}"))?;

    std::thread::spawn(move || {
        loop {
            if stop_rx.try_recv().is_ok() {
                break;
            }
            let first = match fs_rx.recv_timeout(Duration::from_millis(250)) {
                Ok(ev) => ev,
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            };

            // Coalesce burst events from atomic saves / external editors.
            let mut batch = vec![first];
            std::thread::sleep(Duration::from_millis(80));
            while let Ok(ev) = fs_rx.try_recv() {
                batch.push(ev);
            }

            if stop_rx.try_recv().is_ok() {
                break;
            }

            process_fs_batch(&app, &shared, &root, &batch);
        }
    });

    Ok(WatcherHandle {
        _watcher: watcher,
        stop_tx,
    })
}

fn process_fs_batch(
    app: &AppHandle,
    shared: &SharedState,
    root: &Path,
    batch: &[notify::Event],
) {
    let mut touched_paths: HashSet<String> = HashSet::new();
    let mut tree_dirty = false;

    let excluded = {
        let state = shared.lock().unwrap();
        if state.vault_root.as_deref() != Some(root) {
            return;
        }
        state.excluded.clone()
    };

    for event in batch {
        if matches!(event.kind, EventKind::Access(_)) {
            continue;
        }
        for path in &event.paths {
            // Skip temporary write files created by Noteki itself.
            if path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.ends_with(".noteki.tmp"))
            {
                continue;
            }
            if let Some(rel) = relative_path_from_disk(root, path, &excluded) {
                if !matches!(event.kind, EventKind::Modify(notify::event::ModifyKind::Data(_))) {
                    tree_dirty = true;
                }
                if rel.ends_with(".md") {
                    touched_paths.insert(rel);
                }
            }
        }
    }

    let mut reloaded_events = Vec::new();
    let mut conflict_events = Vec::new();
    let mut deleted_events = Vec::new();
    let mut tree_event: Option<VaultTreeChangedPayload> = None;

    {
        let mut state = shared.lock().unwrap();
        if state.vault_root.as_deref() != Some(root) {
            return;
        }

        if tree_dirty {
            if let Ok(new_tree) = scan_vault(root, &state.excluded) {
                if new_tree != state.tree {
                    state.index = VaultIndex::from_tree(&new_tree);
                    let note_count = count_notes(&new_tree);
                    state.tree = new_tree.clone();
                    tree_event = Some(VaultTreeChangedPayload {
                        tree: new_tree,
                        note_count,
                    });
                }
            }
        }

        let watched_open: Vec<String> = touched_paths
            .iter()
            .filter(|p| state.open_notes.contains_key(*p))
            .cloned()
            .collect();

        for rel in watched_open {
            let abs = root.join(&rel);
            let exists = abs.exists();
            let note = state.open_notes.get_mut(&rel).unwrap();

            if !exists {
                if !note.deleted {
                    note.deleted = true;
                    deleted_events.push(NoteDeletedPayload { path: rel });
                }
                continue;
            }

            let Ok(disk_text) = fs::read_to_string(&abs) else {
                continue;
            };

            if note.last_self_write.is_some_and(|t| t.elapsed() < Duration::from_secs(2))
                && disk_text == note.base
            {
                continue;
            }

            if disk_text == note.base && !note.deleted {
                continue;
            }
            note.deleted = false;

            if note.conflict {
                continue;
            }

            if note.buffer == note.base {
                note.base = disk_text.clone();
                note.buffer = disk_text.clone();
                reloaded_events.push(NoteReloadedPayload {
                    path: rel,
                    revision: note.revision,
                    text: disk_text,
                    merged: false,
                });
            } else if let Some(merged) = merge_three_way(&note.base, &note.buffer, &disk_text) {
                if write_note_atomic(root, &rel, &merged).is_ok() {
                    note.base = merged.clone();
                    note.buffer = merged.clone();
                    note.last_self_write = Some(Instant::now());
                    reloaded_events.push(NoteReloadedPayload {
                        path: rel,
                        revision: note.revision,
                        text: merged,
                        merged: true,
                    });
                }
            } else {
                note.conflict = true;
                conflict_events.push(NoteConflictPayload {
                    path: rel,
                    disk_text,
                });
            }
        }
    }

    if let Some(payload) = tree_event {
        let _ = app.emit("vault-tree-changed", payload);
    }
    for payload in reloaded_events {
        let _ = app.emit("note-reloaded", payload);
    }
    for payload in conflict_events {
        let _ = app.emit("note-conflict", payload);
    }
    for payload in deleted_events {
        let _ = app.emit("note-deleted", payload);
    }
}

#[tauri::command]
fn get_settings(state: tauri::State<SharedState>) -> AppSettings {
    state.lock().unwrap().settings.clone()
}

#[tauri::command]
fn save_settings(state: tauri::State<SharedState>, settings: AppSettings) {
    let mut inner = state.lock().unwrap();
    inner.settings = settings.clone();
    persist_settings(&inner.config_path, &settings);
}

#[tauri::command]
fn open_vault(
    app: AppHandle,
    state: tauri::State<SharedState>,
    path: String,
) -> Result<VaultSnapshot, String> {
    let root = PathBuf::from(&path);
    if !root.is_dir() {
        return Err(format!("Vault folder does not exist: {path}"));
    }
    let root = fs::canonicalize(&root).unwrap_or(root);
    let excluded = normalize_excluded(&[]);
    let tree = scan_vault(&root, &excluded)?;
    let note_count = count_notes(&tree);
    let index = VaultIndex::from_tree(&tree);

    {
        let mut inner = state.lock().unwrap();
        if let Some(old_watcher) = inner.watcher.take() {
            let _ = old_watcher.stop_tx.send(());
        }
        inner.vault_root = Some(root.clone());
        inner.excluded = excluded;
        inner.tree = tree.clone();
        inner.index = index;
        inner.open_notes.clear();
        inner.settings.vault_path = Some(root.to_string_lossy().to_string());
        persist_settings(&inner.config_path, &inner.settings);
    }

    let watcher = start_watcher(app, state.inner().clone(), root.clone())?;
    state.lock().unwrap().watcher = Some(watcher);

    Ok(VaultSnapshot {
        root: root.to_string_lossy().to_string(),
        tree,
        note_count,
    })
}

#[tauri::command]
fn open_note(state: tauri::State<SharedState>, path: String) -> Result<OpenNoteResponse, String> {
    let mut inner = state.lock().unwrap();
    let root = inner
        .vault_root
        .clone()
        .ok_or_else(|| "No vault is open".to_string())?;
    let text = read_note(&root, &path)?;

    let existing = inner.open_notes.get(&path).cloned();
    let (conflict, deleted) = match existing {
        Some(n) if n.conflict => (true, n.deleted),
        _ => {
            inner.open_notes.insert(
                path.clone(),
                OpenNoteState {
                    base: text.clone(),
                    buffer: text.clone(),
                    revision: 0,
                    conflict: false,
                    deleted: false,
                    last_self_write: None,
                },
            );
            (false, false)
        }
    };

    inner.settings.recent_notes.retain(|p| p != &path);
    inner.settings.recent_notes.insert(0, path.clone());
    inner.settings.recent_notes.truncate(30);
    persist_settings(&inner.config_path, &inner.settings);

    Ok(OpenNoteResponse {
        path,
        text,
        conflict,
        deleted,
    })
}

#[tauri::command]
fn update_note_buffer(
    state: tauri::State<SharedState>,
    path: String,
    revision: u64,
    text: String,
) {
    let mut inner = state.lock().unwrap();
    if let Some(note) = inner.open_notes.get_mut(&path) {
        note.revision = revision;
        note.buffer = text;
    }
}

#[tauri::command]
fn save_note(
    state: tauri::State<SharedState>,
    path: String,
    text: String,
) -> Result<bool, String> {
    let mut inner = state.lock().unwrap();
    let root = inner
        .vault_root
        .clone()
        .ok_or_else(|| "No vault is open".to_string())?;

    if let Some(note) = inner.open_notes.get(&path) {
        if note.conflict {
            return Ok(false);
        }
        if note.base == text && !note.deleted && resolve_vault_path(&root, &path).is_ok_and(|p| p.exists()) {
            return Ok(true);
        }
    }

    write_note_atomic(&root, &path, &text)?;
    let entry = inner.open_notes.entry(path).or_insert_with(|| OpenNoteState {
        base: text.clone(),
        buffer: text.clone(),
        revision: 0,
        conflict: false,
        deleted: false,
        last_self_write: None,
    });
    entry.base = text.clone();
    entry.buffer = text;
    entry.deleted = false;
    entry.last_self_write = Some(Instant::now());
    Ok(true)
}

#[tauri::command]
fn resolve_conflict(
    state: tauri::State<SharedState>,
    path: String,
    choice: String,
    current_text: String,
) -> Result<String, String> {
    let mut inner = state.lock().unwrap();
    let root = inner
        .vault_root
        .clone()
        .ok_or_else(|| "No vault is open".to_string())?;

    let final_text = match choice.as_str() {
        "keep_mine" => {
            write_note_atomic(&root, &path, &current_text)?;
            current_text
        }
        "take_theirs" => read_note(&root, &path)?,
        _ => return Err("Unknown conflict choice".to_string()),
    };

    if let Some(note) = inner.open_notes.get_mut(&path) {
        note.base = final_text.clone();
        note.buffer = final_text.clone();
        note.conflict = false;
        note.deleted = false;
        note.last_self_write = Some(Instant::now());
    }
    Ok(final_text)
}

#[tauri::command]
fn create_new_note(
    state: tauri::State<SharedState>,
    folder_rel: String,
) -> Result<(String, VaultSnapshot), String> {
    let mut inner = state.lock().unwrap();
    let root = inner
        .vault_root
        .clone()
        .ok_or_else(|| "No vault is open".to_string())?;
    let rel_path = next_untitled_path(&root, &folder_rel);
    create_empty_note(&root, &rel_path)?;

    let tree = scan_vault(&root, &inner.excluded)?;
    let note_count = count_notes(&tree);
    inner.index = VaultIndex::from_tree(&tree);
    inner.tree = tree.clone();
    Ok((
        rel_path,
        VaultSnapshot {
            root: root.to_string_lossy().to_string(),
            tree,
            note_count,
        },
    ))
}

#[tauri::command]
fn check_links(
    state: tauri::State<SharedState>,
    note_path: String,
    inners: Vec<String>,
) -> CheckLinksResult {
    let inner = state.lock().unwrap();
    let Some(root) = inner.vault_root.as_deref() else {
        return CheckLinksResult {
            unresolved: Vec::new(),
            embeds: HashMap::new(),
        };
    };

    let mut unresolved = Vec::new();
    let mut embeds = HashMap::new();
    for raw in inners {
        let link = Wikilink::parse(&raw);
        if inner.index.resolve(&link.target, Some(&note_path)).is_none() {
            unresolved.push(raw.clone());
        } else if let Some(file) = resolve_embed(root, &raw, Some(&note_path), &inner.index) {
            embeds.insert(raw, file);
        }
    }
    CheckLinksResult { unresolved, embeds }
}

#[tauri::command]
fn follow_link(
    state: tauri::State<SharedState>,
    source_path: String,
    inner_text: String,
) -> Result<FollowLinkResult, String> {
    let mut inner = state.lock().unwrap();
    let root = inner
        .vault_root
        .clone()
        .ok_or_else(|| "No vault is open".to_string())?;
    let parsed = Wikilink::parse(&inner_text);

    if let Some(resolved) = inner.index.resolve(&parsed.target, Some(&source_path)) {
        return Ok(match resolved {
            ResolvedItem::Note { path, .. } => FollowLinkResult::Note {
                path,
                created: false,
            },
            ResolvedItem::Attachment {
                path,
                attachment_kind,
                ..
            } => FollowLinkResult::Attachment {
                path,
                attachment_kind,
            },
        });
    }

    let new_path = new_note_path_for_link(&root, &parsed.target, Some(&source_path), &inner.excluded)?;
    create_empty_note(&root, &new_path)?;
    let tree = scan_vault(&root, &inner.excluded)?;
    inner.index = VaultIndex::from_tree(&tree);
    inner.tree = tree;
    Ok(FollowLinkResult::Note {
        path: new_path,
        created: true,
    })
}

#[tauri::command]
fn complete_link(state: tauri::State<SharedState>) -> LinkCompletions {
    let inner = state.lock().unwrap();
    inner.index.link_completions(&inner.settings.recent_notes)
}

#[tauri::command]
fn read_frontmatter(text: String) -> Option<FrontmatterReading> {
    parse_frontmatter(&text)
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .register_uri_scheme_protocol("noteki-file", |ctx, request| {
            let empty_404 = || {
                tauri::http::Response::builder()
                    .status(404)
                    .body(Vec::new())
                    .unwrap()
            };

            let state = ctx.app_handle().state::<SharedState>();
            let (root, excluded) = {
                let inner = state.lock().unwrap();
                let Some(root) = inner.vault_root.clone() else {
                    return empty_404();
                };
                (root, inner.excluded.clone())
            };

            let raw_path = request.uri().path().trim_start_matches('/');
            let raw_path = raw_path.strip_prefix("vault/").unwrap_or(raw_path);
            let decoded = urlencoding::decode(raw_path)
                .map(|c| c.into_owned())
                .unwrap_or_else(|_| raw_path.to_string());

            if is_in_excluded_folder(&root, &decoded, &excluded) {
                return empty_404();
            }
            let Ok(abs) = resolve_vault_path(&root, &decoded) else {
                return empty_404();
            };
            let Ok(bytes) = fs::read(&abs) else {
                return empty_404();
            };
            tauri::http::Response::builder()
                .status(200)
                .header("Content-Type", mime_for_path(&abs))
                .body(bytes)
                .unwrap()
        })
        .setup(|app| {
            let config_dir = app
                .path()
                .app_config_dir()
                .unwrap_or_else(|_| std::env::temp_dir().join("noteki-tauri"));
            let config_path = config_dir.join("settings.json");
            let settings = load_settings_from_disk(&config_path);

            let shared: SharedState = Arc::new(Mutex::new(InnerState {
                config_path,
                settings,
                vault_root: None,
                excluded: HashSet::new(),
                tree: Vec::new(),
                index: VaultIndex::default(),
                open_notes: HashMap::new(),
                watcher: None,
            }));

            app.manage(shared);
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
            create_new_note,
            check_links,
            follow_link,
            complete_link,
            read_frontmatter,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
