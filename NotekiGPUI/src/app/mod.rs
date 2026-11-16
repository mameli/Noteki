//! The Noteki window: state and behaviour. Drawing lives in `render.rs`.
//!
//! The rules (saving, merging, tabs, links) live in `crate::core` and are
//! unit-tested there; this module wires them to GPUI and the file system.

mod render;
pub mod settings;
#[allow(dead_code)] // Live preview styles: drawn by the new editor again from ticket 03.
mod style;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

use notify::{RecommendedWatcher, RecursiveMode, Watcher};

use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::tree::{TreeEvent, TreeItem, TreeState};
use gpui_kit::{
    actions, App, AppContext as _, Context, Entity, FocusHandle, Focusable, KeyBinding,
    ScrollStrategy, SharedString, Subscription, Task, Window,
};

use crate::core::document::{ExternalOutcome, NoteDoc};
use crate::core::editor::lines::logical_lines;
use crate::core::index::{
    build_tag_tree, extract_outline, find_backlinks, search_vault, BacklinkMatch, Corpus,
    HeadingItem, SearchResult, TagNode,
};
use crate::core::markdown::{
    self, line_of_offset, preprocess_for_reader, set_task_checked, ReaderLink,
};
use crate::core::tabs::{rename_target, OpenIn, Tabs};
use crate::core::vault::{
    create_empty_note, create_folder, make_copy, move_to_trash, new_note_path_for_link,
    normalize_excluded, read_note, rename_item, scan_vault, today_daily_note_path,
    validate_item_name, write_note_atomic, AttachmentKind, VaultItem,
};
use crate::core::wikilink::{folder_of, ResolvedItem, VaultIndex};
use crate::editor::{NoteEditor, NoteEditorEvent};
use crate::theme::{apply_noteki_theme, AppearanceMode, NotekiPalette, NotekiThemeName};

pub use settings::{LeftPane, RightPane, Settings, ViewMode};

actions!(
    noteki,
    [
        ToggleLeftSidebar,
        ToggleRightSidebar,
        SwitchFilesPane,
        SwitchTagsPane,
        SwitchSearchPane,
        ToggleViewMode,
        OpenQuickSwitcher,
        OpenQuickSwitcherNewTab,
        OpenCommandPalette,
        NewNoteAction,
        NewFolderAction,
        DailyNoteAction,
        CloseTabAction,
        ReopenClosedTabAction,
        NextTabAction,
        PrevTabAction,
        NavigateBackAction,
        NavigateForwardAction,
        OpenVaultAction,
        CycleThemeAction,
        CycleAppearanceAction,
        FormatBoldAction,
        FormatItalicAction,
        FormatStrikeAction,
        FormatCodeAction,
        InsertWikilinkAction,
        FollowLinkUnderCursorAction,
    ]
);

pub fn register_keybindings(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("ctrl-cmd-s", ToggleLeftSidebar, None),
        KeyBinding::new("alt-cmd-0", ToggleRightSidebar, None),
        KeyBinding::new("ctrl-cmd-1", SwitchFilesPane, None),
        KeyBinding::new("ctrl-cmd-2", SwitchTagsPane, None),
        KeyBinding::new("ctrl-cmd-3", SwitchSearchPane, None),
        KeyBinding::new("cmd-shift-f", SwitchSearchPane, None),
        KeyBinding::new("cmd-e", ToggleViewMode, None),
        KeyBinding::new("cmd-o", OpenQuickSwitcher, None),
        KeyBinding::new("cmd-t", OpenQuickSwitcherNewTab, None),
        KeyBinding::new("cmd-shift-p", OpenCommandPalette, None),
        KeyBinding::new("cmd-n", NewNoteAction, None),
        KeyBinding::new("cmd-shift-n", NewFolderAction, None),
        KeyBinding::new("cmd-shift-d", DailyNoteAction, None),
        KeyBinding::new("cmd-w", CloseTabAction, None),
        KeyBinding::new("cmd-shift-t", ReopenClosedTabAction, None),
        KeyBinding::new("ctrl-tab", NextTabAction, None),
        KeyBinding::new("cmd-shift-]", NextTabAction, None),
        KeyBinding::new("ctrl-shift-tab", PrevTabAction, None),
        KeyBinding::new("cmd-shift-[", PrevTabAction, None),
        KeyBinding::new("cmd-[", NavigateBackAction, None),
        KeyBinding::new("cmd-]", NavigateForwardAction, None),
        KeyBinding::new("cmd-alt-left", NavigateBackAction, None),
        KeyBinding::new("cmd-alt-right", NavigateForwardAction, None),
        KeyBinding::new("cmd-shift-o", OpenVaultAction, None),
        KeyBinding::new("cmd-b", FormatBoldAction, None),
        KeyBinding::new("cmd-i", FormatItalicAction, None),
        KeyBinding::new("cmd-shift-x", FormatStrikeAction, None),
        KeyBinding::new("ctrl-`", FormatCodeAction, None),
        KeyBinding::new("cmd-k", InsertWikilinkAction, None),
        KeyBinding::new("cmd-enter", FollowLinkUnderCursorAction, None),
    ]);
    crate::editor::register_keybindings(cx);
}

/// SPEC › Saving: about one second after the last change.
pub const AUTOSAVE_DELAY: Duration = Duration::from_millis(1000);
/// File-system events are handled once they have been quiet for this long,
/// so a burst (an atomic write, a git checkout) is one reload.
pub const FS_SETTLE: Duration = Duration::from_millis(250);
const FS_POLL: Duration = Duration::from_millis(100);

#[derive(Debug, Clone, Default)]
pub struct AppConfig {
    /// Where preferences are stored; `None` keeps them in memory (tests).
    pub settings_path: Option<PathBuf>,
    /// Vault to open at launch, instead of the remembered one.
    pub vault: Option<PathBuf>,
    /// Watch the vault for changes made by other apps.
    pub watch: bool,
    /// Where "Move to Trash" moves files; `None` is `~/.Trash`.
    pub trash_dir: Option<PathBuf>,
}

impl AppConfig {
    /// The configuration of the real app: user preferences, file watching, and
    /// the demo vault when nothing was opened before.
    pub fn for_user() -> Self {
        Self {
            settings_path: settings::default_settings_path(),
            vault: None,
            watch: true,
            trash_dir: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabKind {
    Note,
    Attachment(AttachmentKind),
}

/// The files of the open vault and everything derived from them.
pub struct Vault {
    pub root: PathBuf,
    pub scan: Vec<VaultItem>,
    pub index: Arc<VaultIndex>,
    /// `(name, path)` of every note.
    pub notes: Vec<(String, String)>,
    pub corpus: Corpus,
    pub tag_tree: Vec<TagNode>,
    excluded: HashSet<String>,
}

impl Vault {
    pub fn load(root: &Path) -> Result<Self, String> {
        let root = std::fs::canonicalize(root)
            .map_err(|e| format!("Cannot open {}: {e}", root.display()))?;
        if !root.is_dir() {
            return Err(format!("{} is not a folder", root.display()));
        }
        let mut vault = Self {
            root,
            scan: Vec::new(),
            index: Arc::default(),
            notes: Vec::new(),
            corpus: Corpus::default(),
            tag_tree: Vec::new(),
            excluded: normalize_excluded(&[]),
        };
        vault.refresh(&HashSet::new())?;
        Ok(vault)
    }

    /// Rescans the tree; re-reads the notes in `changed` (vault-relative paths).
    pub fn refresh(&mut self, changed: &HashSet<String>) -> Result<(), String> {
        let scan = scan_vault(&self.root, &self.excluded)?;
        let mut notes = Vec::new();
        collect_notes(&scan, &mut notes);
        self.corpus = self.corpus.refresh(&self.root, &notes, changed);
        self.index = Arc::new(VaultIndex::from_tree(&scan));
        self.tag_tree = build_tag_tree(&self.corpus);
        self.notes = notes;
        self.scan = scan;
        Ok(())
    }

    pub fn kind_of(&self, path: &str) -> Option<TabKind> {
        find_item(&self.scan, path).and_then(|item| match item {
            VaultItem::Note { .. } => Some(TabKind::Note),
            VaultItem::Attachment {
                attachment_kind, ..
            } => Some(TabKind::Attachment(*attachment_kind)),
            VaultItem::Folder { .. } => None,
        })
    }

    pub fn is_folder(&self, path: &str) -> bool {
        matches!(find_item(&self.scan, path), Some(VaultItem::Folder { .. }))
    }

    pub fn contains(&self, path: &str) -> bool {
        find_item(&self.scan, path).is_some()
    }

    /// Vault-relative path of an absolute path reported by the file system.
    fn relative(&self, abs: &Path) -> Option<String> {
        let rel = abs
            .strip_prefix(&self.root)
            .ok()
            .map(Path::to_path_buf)
            .or_else(|| {
                std::fs::canonicalize(abs.parent()?)
                    .ok()?
                    .join(abs.file_name()?)
                    .strip_prefix(&self.root)
                    .ok()
                    .map(Path::to_path_buf)
            })?;
        let parts: Vec<&str> = rel.iter().filter_map(|c| c.to_str()).collect();
        if parts.is_empty() || parts.iter().any(|p| p.starts_with('.')) {
            return None; // hidden files: .obsidian, .git, our own temp files
        }
        Some(parts.join("/"))
    }
}

fn find_item<'a>(items: &'a [VaultItem], path: &str) -> Option<&'a VaultItem> {
    for item in items {
        if item.path() == path {
            return Some(item);
        }
        if let VaultItem::Folder {
            children,
            path: folder,
            ..
        } = item
        {
            if path.starts_with(folder.as_str()) && path.as_bytes().get(folder.len()) == Some(&b'/')
            {
                return find_item(children, path);
            }
        }
    }
    None
}

fn collect_notes(items: &[VaultItem], out: &mut Vec<(String, String)>) {
    for item in items {
        match item {
            VaultItem::Note { name, path } => out.push((name.clone(), path.clone())),
            VaultItem::Folder { children, .. } => collect_notes(children, out),
            VaultItem::Attachment { .. } => {}
        }
    }
}

fn tree_items(items: &[VaultItem], expanded: &HashSet<String>) -> Vec<TreeItem> {
    items
        .iter()
        .map(|item| match item {
            VaultItem::Folder {
                name,
                path,
                children,
            } => TreeItem::new(path.clone(), name.clone())
                .expanded(expanded.contains(path))
                .children(tree_items(children, expanded)),
            VaultItem::Note { name, path } | VaultItem::Attachment { name, path, .. } => {
                TreeItem::new(path.clone(), name.clone())
            }
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PromptKind {
    NewNote,
    NewFolder,
    Rename,
}

/// A message in the status bar; errors stay until the next one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusMessage {
    pub text: String,
    pub is_error: bool,
}

pub struct NotekiApp {
    config: AppConfig,
    settings: Settings,
    focus_handle: FocusHandle,

    vault: Option<Vault>,
    tabs: Tabs,
    docs: HashMap<String, NoteDoc>,
    /// The note whose text the editor holds.
    editor_path: Option<String>,
    recent_notes: Vec<String>,

    palette: NotekiPalette,
    status: Option<StatusMessage>,

    file_tree: Entity<TreeState>,
    expanded_folders: HashSet<String>,
    selected_tag: Option<String>,
    search_input: Entity<InputState>,
    search_results: Vec<SearchResult>,

    editor: Entity<NoteEditor>,

    outline: Vec<HeadingItem>,
    backlinks: Vec<BacklinkMatch>,
    /// Reader Markdown of the active note, built only while the Reader shows.
    reader_markdown: Option<(String, SharedString)>,

    watcher: Option<RecommendedWatcher>,
    fs_events: Option<mpsc::Receiver<PathBuf>>,
    pending_fs: HashSet<PathBuf>,
    last_fs_event: Option<Instant>,
    autosave: Option<Task<()>>,

    _subscriptions: Vec<Subscription>,
}

impl Focusable for NotekiApp {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl NotekiApp {
    pub fn new(config: AppConfig, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let settings = settings::load(config.settings_path.as_deref());
        let palette = apply_noteki_theme(settings.theme_name, settings.appearance, window, cx);

        let editor = cx.new(|cx| NoteEditor::new(palette, window, cx));
        let search_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Search  ·  tag:work  path:Journal  \"a phrase\"")
        });
        let file_tree = cx.new(|cx| TreeState::new(cx));

        let subscriptions = vec![
            cx.subscribe_in(&editor, window, Self::on_editor_event),
            cx.subscribe_in(&search_input, window, |this, _, ev: &InputEvent, _, cx| {
                if matches!(ev, InputEvent::Change) {
                    this.refresh_search(cx);
                    cx.notify();
                }
            }),
            cx.subscribe(&file_tree, |this, _, ev: &TreeEvent, cx| {
                match ev {
                    TreeEvent::Expanded(id) => this.expanded_folders.insert(id.to_string()),
                    TreeEvent::Collapsed(id) => this.expanded_folders.remove(id.as_ref()),
                };
                this.persist_session();
                cx.notify();
            }),
            cx.observe_window_appearance(window, |this, window, cx| {
                if this.settings.appearance == AppearanceMode::System {
                    this.apply_theme(window, cx);
                }
            }),
            cx.on_app_quit(|this, _| {
                this.flush_all();
                async {}
            }),
        ];

        let mut app = Self {
            config,
            settings,
            focus_handle: cx.focus_handle(),
            vault: None,
            tabs: Tabs::default(),
            docs: HashMap::new(),
            editor_path: None,
            recent_notes: Vec::new(),
            palette,
            status: None,
            file_tree,
            expanded_folders: HashSet::new(),
            selected_tag: None,
            search_input,
            search_results: Vec::new(),
            editor,
            outline: Vec::new(),
            backlinks: Vec::new(),
            reader_markdown: None,
            watcher: None,
            fs_events: None,
            pending_fs: HashSet::new(),
            last_fs_event: None,
            autosave: None,
            _subscriptions: subscriptions,
        };

        let initial = app
            .config
            .vault
            .clone()
            .or_else(|| {
                app.settings
                    .vault_path
                    .as_deref()
                    .map(PathBuf::from)
                    .filter(|p| p.is_dir())
            })
            .or_else(|| {
                if app.config.settings_path.is_some() {
                    find_demo_vault()
                } else {
                    None
                }
            });
        if let Some(root) = initial {
            let _ = app.open_vault(&root, window, cx);
        }
        app.start_fs_poller(window, cx);
        app
    }

    // ---- Accessors (used by the view and by the tests) ----

    pub fn vault(&self) -> Option<&Vault> {
        self.vault.as_ref()
    }

    pub fn tabs(&self) -> &Tabs {
        &self.tabs
    }

    pub fn active_path(&self) -> Option<&str> {
        self.tabs.active_path()
    }

    pub fn active_kind(&self) -> Option<TabKind> {
        let path = self.active_path()?;
        if self.docs.contains_key(path) {
            return Some(TabKind::Note);
        }
        self.vault.as_ref()?.kind_of(path)
    }

    pub fn doc(&self, path: &str) -> Option<&NoteDoc> {
        self.docs.get(path)
    }

    pub fn editor_text(&self, cx: &App) -> String {
        self.editor.read(cx).text().to_string()
    }

    pub fn editor(&self) -> &Entity<NoteEditor> {
        &self.editor
    }

    pub fn outline(&self) -> &[HeadingItem] {
        &self.outline
    }

    pub fn backlinks(&self) -> &[BacklinkMatch] {
        &self.backlinks
    }

    pub fn search_results(&self) -> &[SearchResult] {
        &self.search_results
    }

    pub fn reader_markdown(&self) -> Option<&str> {
        self.reader_markdown.as_ref().map(|(_, md)| md.as_ref())
    }

    pub fn status(&self) -> Option<&StatusMessage> {
        self.status.as_ref()
    }

    pub fn view_mode(&self) -> ViewMode {
        self.settings.view_mode
    }

    pub fn expanded_folders(&self) -> &HashSet<String> {
        &self.expanded_folders
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    pub fn recent_notes(&self) -> &[String] {
        &self.recent_notes
    }

    // ---- Status ----

    fn report(&mut self, result: Result<(), String>) {
        if let Err(e) = result {
            self.set_error(e);
        }
    }

    fn set_error(&mut self, text: impl Into<String>) {
        self.status = Some(StatusMessage {
            text: text.into(),
            is_error: true,
        });
    }

    fn set_info(&mut self, text: impl Into<String>) {
        self.status = Some(StatusMessage {
            text: text.into(),
            is_error: false,
        });
    }

    // ---- Settings ----

    fn persist(&mut self) {
        if let Err(e) = settings::save(self.config.settings_path.as_deref(), &self.settings) {
            self.set_error(format!("Could not save preferences: {e}"));
        }
    }

    fn persist_session(&mut self) {
        let Some(vault) = &self.vault else { return };
        let key = vault.root.to_string_lossy().to_string();
        let mut expanded: Vec<String> = self.expanded_folders.iter().cloned().collect();
        expanded.sort();
        let session = settings::VaultSession {
            open_tabs: self.tabs.iter().map(|t| t.path.clone()).collect(),
            active_tab: self.active_path().map(str::to_string),
            recent_notes: self.recent_notes.clone(),
            expanded_folders: expanded,
        };
        if self.settings.sessions.get(&key) != Some(&session) {
            self.settings.sessions.insert(key, session);
            self.persist();
        }
    }

    pub fn set_view_mode(&mut self, mode: ViewMode, cx: &mut Context<Self>) {
        self.settings.view_mode = mode;
        self.refresh_reader();
        self.persist();
        cx.notify();
    }

    pub(crate) fn toggle_left_sidebar(&mut self, cx: &mut Context<Self>) {
        self.settings.left_sidebar_open = !self.settings.left_sidebar_open;
        self.persist();
        cx.notify();
    }

    pub(crate) fn toggle_right_sidebar(&mut self, cx: &mut Context<Self>) {
        self.settings.right_sidebar_open = !self.settings.right_sidebar_open;
        self.persist();
        cx.notify();
    }

    pub(crate) fn show_left_pane(&mut self, pane: LeftPane, cx: &mut Context<Self>) {
        self.settings.left_sidebar_open = true;
        self.settings.left_pane = pane;
        self.persist();
        cx.notify();
    }

    pub(crate) fn show_right_pane(&mut self, pane: RightPane, cx: &mut Context<Self>) {
        self.settings.right_sidebar_open = true;
        self.settings.right_pane = pane;
        self.persist();
        cx.notify();
    }

    fn apply_theme(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.palette = apply_noteki_theme(
            self.settings.theme_name,
            self.settings.appearance,
            window,
            cx,
        );
        let palette = self.palette;
        self.editor.update(cx, |ed, cx| ed.set_palette(palette, cx));
        cx.notify();
    }

    pub(crate) fn cycle_theme(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.settings.theme_name = self.settings.theme_name.next();
        self.persist();
        self.apply_theme(window, cx);
    }

    pub(crate) fn cycle_appearance(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.settings.appearance = self.settings.appearance.next();
        self.persist();
        self.apply_theme(window, cx);
    }

    pub fn theme_name(&self) -> NotekiThemeName {
        self.settings.theme_name
    }

    // ---- Vault ----

    pub fn open_vault(
        &mut self,
        root: &Path,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        self.flush_all();
        let vault = match Vault::load(root) {
            Ok(v) => v,
            Err(e) => {
                self.set_error(e.clone());
                cx.notify();
                return Err(e);
            }
        };
        let key = vault.root.to_string_lossy().to_string();
        let session = self
            .settings
            .sessions
            .get(&key)
            .cloned()
            .unwrap_or_default();

        self.tabs = Tabs::default();
        self.docs.clear();
        self.editor_path = None;
        self.selected_tag = None;
        self.status = None;
        self.recent_notes = session
            .recent_notes
            .iter()
            .filter(|p| vault.contains(p))
            .cloned()
            .collect();
        self.expanded_folders = session
            .expanded_folders
            .iter()
            .filter(|p| vault.is_folder(p))
            .cloned()
            .collect();
        self.settings.vault_path = Some(key.clone());
        self.settings.recent_vaults.retain(|v| v != &key);
        self.settings.recent_vaults.insert(0, key);
        self.settings
            .recent_vaults
            .truncate(settings::MAX_RECENT_VAULTS);
        let root = vault.root.clone();
        self.vault = Some(vault);
        self.start_watcher(&root);
        self.refresh_tree(cx);

        for path in &session.open_tabs {
            self.load_into_tabs(path, OpenIn::NewTab);
        }
        if let Some(i) = session
            .active_tab
            .as_deref()
            .and_then(|a| self.tabs.iter().position(|t| t.path == a))
        {
            self.tabs.activate(i);
        }
        if self.tabs.is_empty() {
            let first = self
                .vault
                .as_ref()
                .and_then(|v| {
                    v.notes
                        .iter()
                        .find(|(n, _)| n.eq_ignore_ascii_case("welcome"))
                        .or(v.notes.first())
                })
                .map(|(_, p)| p.clone());
            if let Some(first) = first {
                self.load_into_tabs(&first, OpenIn::NewTab);
            }
        }
        self.persist();
        self.show_active(window, cx);
        Ok(())
    }

    pub(crate) fn pick_vault(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(gpui_kit::PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Open Vault".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(paths))) = rx.await {
                if let Some(folder) = paths.into_iter().next() {
                    let _ =
                        this.update_in(cx, |app, window, cx| app.open_vault(&folder, window, cx));
                }
            }
        })
        .detach();
    }

    fn refresh_vault(&mut self, changed: &HashSet<String>, cx: &mut Context<Self>) {
        let Some(vault) = self.vault.as_mut() else {
            return;
        };
        if let Err(e) = vault.refresh(changed) {
            self.set_error(e);
            return;
        }
        // Attachments gone from disk: their tabs close (notes keep theirs with a banner).
        let gone: Vec<String> = self
            .tabs
            .iter()
            .map(|t| t.path.clone())
            .filter(|p| !self.docs.contains_key(p) && !vault.contains(p))
            .collect();
        for p in gone {
            self.tabs.remove(&p);
        }
        self.refresh_tree(cx);
        self.refresh_search(cx);
        self.refresh_backlinks();
    }

    fn refresh_tree(&mut self, cx: &mut Context<Self>) {
        let Some(vault) = &self.vault else { return };
        // Keep what the user expanded or collapsed in the tree itself.
        let tree = self.file_tree.read(cx);
        let mut ix = 0;
        while let Some(entry) = tree.entry(ix) {
            if entry.is_folder() {
                let id = entry.item().id.to_string();
                if entry.is_expanded() {
                    self.expanded_folders.insert(id);
                } else {
                    self.expanded_folders.remove(&id);
                }
            }
            ix += 1;
        }
        self.expanded_folders.retain(|p| vault.is_folder(p));
        let items = tree_items(&vault.scan, &self.expanded_folders);
        let selected = self.active_path().map(SharedString::from);
        self.file_tree.update(cx, |tree, cx| {
            tree.set_items(items, cx);
            if let Some(id) = selected {
                if let Some(ix) = tree.index_of(&id) {
                    tree.set_selected_index(Some(ix), cx);
                }
            }
        });
    }

    fn select_in_tree(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.active_path().map(SharedString::from) else {
            return;
        };
        self.file_tree.update(cx, |tree, cx| {
            tree.reveal_item(&id, ScrollStrategy::Nearest, cx);
            let ix = tree.index_of(&id);
            tree.set_selected_index(ix, cx);
        });
    }

    // ---- Opening notes ----

    /// Adds `path` to the tabs, loading its note; false if it cannot be read.
    fn load_into_tabs(&mut self, path: &str, mode: OpenIn) -> bool {
        let Some(vault) = &self.vault else {
            return false;
        };
        match vault.kind_of(path) {
            Some(TabKind::Note) if !self.docs.contains_key(path) => {
                match read_note(&vault.root, path) {
                    Ok(text) => {
                        self.docs.insert(path.to_string(), NoteDoc::new(text));
                    }
                    Err(e) => {
                        self.set_error(e);
                        return false;
                    }
                }
            }
            Some(_) => {}
            None if self.docs.contains_key(path) => {} // deleted on disk, text in memory
            None => return false,
        }
        self.tabs.open(path, mode);
        true
    }

    /// Opens a note or attachment of the vault. Returns false if it does not exist.
    pub fn open_path(
        &mut self,
        path: &str,
        mode: OpenIn,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        self.remember_cursor(cx);
        let leaving = self.active_path().map(str::to_string);
        if !self.load_into_tabs(path, mode) {
            if self.status.is_none() {
                self.set_error(format!("Not found: {path}"));
            }
            cx.notify();
            return false;
        }
        if let Some(leaving) = leaving.filter(|l| l != path) {
            self.save_logged(&leaving);
        }
        self.show_active(window, cx);
        true
    }

    fn remember_cursor(&mut self, cx: &App) {
        if self.editor_path.is_some() && self.editor_path.as_deref() == self.active_path() {
            let cursor = self.editor.read(cx).cursor();
            self.tabs.set_active_cursor(cursor);
        }
    }

    /// Makes the editor, inspector, reader and tree follow the active tab.
    fn show_active(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.prune_docs();
        let active = self.tabs.active().map(|t| (t.path.clone(), t.cursor));
        match active {
            Some((path, cursor)) if self.docs.contains_key(&path) => {
                let text = self.docs[&path].text().to_string();
                let other_note = self.editor_path.as_deref() != Some(path.as_str());
                if other_note || self.editor_text(cx) != text {
                    self.set_editor_text(&text, cursor, window, cx);
                    if other_note {
                        self.editor.update(cx, |ed, cx| ed.scroll_to_top(cx));
                    }
                } else {
                    self.set_editor_cursor(cursor, cx);
                }
                self.editor_path = Some(path.clone());
                self.recent_notes.retain(|p| p != &path);
                self.recent_notes.insert(0, path);
                self.recent_notes.truncate(settings::MAX_RECENT_NOTES);
            }
            _ => self.editor_path = None,
        }
        self.refresh_outline();
        self.refresh_backlinks();
        self.refresh_reader();
        self.select_in_tree(cx);
        self.persist_session();
        cx.notify();
    }

    /// Drops documents no tab shows, unless their text exists only on screen.
    fn prune_docs(&mut self) {
        let shown: HashSet<String> = self.tabs.iter().map(|t| t.path.clone()).collect();
        let unsaved: Vec<String> = self
            .docs
            .iter()
            .filter(|(p, d)| !shown.contains(*p) && d.should_save())
            .map(|(p, _)| p.clone())
            .collect();
        for p in unsaved {
            self.save_logged(&p);
        }
        self.docs
            .retain(|p, d| shown.contains(p) || d.is_only_on_screen() || d.should_save());
    }

    fn set_editor_text(
        &mut self,
        text: &str,
        cursor: usize,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // `set_text` emits no Changed event, so this is never taken for typing.
        self.editor
            .update(cx, |ed, cx| ed.set_text(text, cursor, cx));
    }

    fn set_editor_cursor(&mut self, cursor: usize, cx: &mut Context<Self>) {
        self.editor.update(cx, |ed, cx| {
            let c = cursor.min(ed.text().len());
            ed.set_selected_range(c..c, cx);
        });
    }

    pub fn activate_tab(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.remember_cursor(cx);
        let leaving = self.active_path().map(str::to_string);
        if self.tabs.activate(index) {
            if let Some(l) = leaving.filter(|l| Some(l.as_str()) != self.active_path()) {
                self.save_logged(&l);
            }
            self.show_active(window, cx);
        }
    }

    pub fn close_tab(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.remember_cursor(cx);
        if let Some(path) = self.tabs.close(index) {
            if !self.tabs.contains(&path) {
                self.save_logged(&path);
                if self.docs.get(&path).is_some_and(NoteDoc::is_only_on_screen) {
                    self.set_info(format!(
                        "{path}: its text is kept in memory; reopen the tab (⇧⌘T) to resolve it"
                    ));
                }
            }
            self.show_active(window, cx);
        }
    }

    pub(crate) fn close_active_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(i) = self.tabs.active_index() {
            self.close_tab(i, window, cx);
        }
    }

    pub(crate) fn reopen_closed_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let vault = self.vault.as_ref();
        let docs = &self.docs;
        if let Some(path) = self
            .tabs
            .pop_closed(|p| docs.contains_key(p) || vault.is_some_and(|v| v.contains(p)))
        {
            self.open_path(&path, OpenIn::NewTab, window, cx);
        }
    }

    pub(crate) fn cycle_tab(&mut self, forward: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.remember_cursor(cx);
        let leaving = self.active_path().map(str::to_string);
        if forward {
            self.tabs.next()
        } else {
            self.tabs.prev()
        }
        if let Some(l) = leaving.filter(|l| Some(l.as_str()) != self.active_path()) {
            self.save_logged(&l);
        }
        self.show_active(window, cx);
    }

    pub fn navigate(&mut self, back: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.remember_cursor(cx);
        let leaving = self.active_path().map(str::to_string);
        let moved = if back {
            self.tabs.back().is_some()
        } else {
            self.tabs.forward().is_some()
        };
        if !moved {
            return;
        }
        if let Some(path) = self.active_path().map(str::to_string) {
            if !self.docs.contains_key(&path) && !self.load_into_tabs(&path, OpenIn::CurrentTab) {
                self.set_error(format!("Not found: {path}"));
            }
        }
        if let Some(l) = leaving {
            self.save_logged(&l);
        }
        self.show_active(window, cx);
    }

    // ---- Editing and saving ----

    fn on_editor_event(
        &mut self,
        _: &Entity<NoteEditor>,
        event: &NoteEditorEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let NoteEditorEvent::Changed = event;
        let Some(path) = self.editor_path.clone() else {
            return;
        };
        let text = self.editor_text(cx);
        let Some(doc) = self.docs.get_mut(&path) else {
            return;
        };
        if doc.edit(&text) {
            self.after_text_change(window, cx);
        }
    }

    fn after_text_change(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.refresh_outline();
        self.refresh_reader();
        self.schedule_autosave(window, cx);
        cx.notify();
    }

    fn schedule_autosave(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Replacing the task cancels the previous one: saves happen after the last change.
        self.autosave = Some(cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(AUTOSAVE_DELAY).await;
            let _ = this.update(cx, |app, cx| {
                app.flush_all();
                cx.notify();
            });
        }));
    }

    /// Writes the note if it has unsaved edits and may be saved. Returns whether it wrote.
    pub fn save_note(&mut self, path: &str) -> Result<bool, String> {
        let Some(vault) = self.vault.as_mut() else {
            return Ok(false);
        };
        let Some(doc) = self.docs.get_mut(path) else {
            return Ok(false);
        };
        if !doc.should_save() {
            return Ok(false);
        }
        let text = doc.text().to_string();
        write_note_atomic(&vault.root, path, &text)?;
        doc.mark_saved(&text);
        if vault.corpus.get(path).is_some() {
            vault.corpus.set(path, &text);
            vault.tag_tree = build_tag_tree(&vault.corpus);
        }
        Ok(true)
    }

    fn save_logged(&mut self, path: &str) {
        if let Err(e) = self.save_note(path) {
            self.set_error(format!("Could not save {path}: {e}"));
        }
    }

    /// Saves every open note with unsaved edits.
    pub fn flush_all(&mut self) {
        let paths: Vec<String> = self
            .docs
            .iter()
            .filter(|(_, d)| d.should_save())
            .map(|(p, _)| p.clone())
            .collect();
        for p in paths {
            self.save_logged(&p);
        }
    }

    /// Replaces the text of an open note (Reader checkbox, formatting) as if typed.
    fn replace_note_text(
        &mut self,
        path: &str,
        text: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(doc) = self.docs.get_mut(path) else {
            return;
        };
        if !doc.edit(&text) {
            return;
        }
        if self.editor_path.as_deref() == Some(path) {
            let cursor = self.editor.read(cx).cursor();
            self.set_editor_text(&text, cursor, window, cx);
        }
        self.after_text_change(window, cx);
    }

    // ---- External changes ----

    fn start_watcher(&mut self, root: &Path) {
        self.watcher = None;
        self.fs_events = None;
        self.pending_fs.clear();
        if !self.config.watch {
            return;
        }
        let (tx, rx) = mpsc::channel();
        let watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            if let Ok(event) = res {
                for p in event.paths {
                    let _ = tx.send(p);
                }
            }
        });
        match watcher {
            Ok(mut w) => match w.watch(root, RecursiveMode::Recursive) {
                Ok(()) => {
                    self.watcher = Some(w);
                    self.fs_events = Some(rx);
                }
                Err(e) => self.set_error(format!("Cannot watch the vault for changes: {e}")),
            },
            Err(e) => self.set_error(format!("Cannot watch the vault for changes: {e}")),
        }
    }

    fn start_fs_poller(&self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.config.watch {
            return;
        }
        cx.spawn_in(window, async move |this, cx| loop {
            cx.background_executor().timer(FS_POLL).await;
            let alive = this.update_in(cx, |app, window, cx| {
                if let Some(rx) = &app.fs_events {
                    let mut got = false;
                    while let Ok(p) = rx.try_recv() {
                        app.pending_fs.insert(p);
                        got = true;
                    }
                    if got {
                        app.last_fs_event = Some(Instant::now());
                    }
                }
                let settled = app.last_fs_event.is_some_and(|t| t.elapsed() >= FS_SETTLE);
                if settled && !app.pending_fs.is_empty() {
                    let paths: Vec<PathBuf> = app.pending_fs.drain().collect();
                    app.last_fs_event = None;
                    app.handle_fs_changes(&paths, window, cx);
                }
            });
            if alive.is_err() {
                break;
            }
        })
        .detach();
    }

    /// Reconciles the app with files changed on disk (absolute paths), as the
    /// watcher reports them: reload, merge, conflict, deletion, followed rename.
    pub fn handle_fs_changes(
        &mut self,
        abs_paths: &[PathBuf],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(vault) = &self.vault else { return };
        let root = vault.root.clone();
        let changed: HashSet<String> = abs_paths.iter().filter_map(|p| vault.relative(p)).collect();
        if changed.is_empty() {
            return;
        }
        let known_before: HashSet<String> = vault.notes.iter().map(|(_, p)| p.clone()).collect();

        let mut deleted: Vec<String> = Vec::new();
        let mut to_save: Vec<String> = Vec::new();
        let affected: Vec<String> = self
            .docs
            .keys()
            .filter(|p| changed.contains(*p))
            .cloned()
            .collect();
        for path in affected {
            let disk_path = root.join(&path);
            let disk = if disk_path.is_file() {
                match std::fs::read_to_string(&disk_path) {
                    Ok(t) => Some(t),
                    Err(_) => continue, // mid-write or unreadable: the next event will tell
                }
            } else {
                None
            };
            let doc = self.docs.get_mut(&path).expect("affected doc");
            let outcome = doc.on_disk_changed(disk.as_deref());
            match outcome {
                ExternalOutcome::Reloaded | ExternalOutcome::Merged => {
                    if self.editor_path.as_deref() == Some(path.as_str()) {
                        let text = doc.text().to_string();
                        let cursor = self.editor.read(cx).cursor();
                        self.set_editor_text(&text, cursor, window, cx);
                    }
                    if outcome == ExternalOutcome::Merged {
                        to_save.push(path.clone());
                    }
                }
                ExternalOutcome::Conflict => self.set_error(format!(
                    "{path} changed on disk and in Noteki: choose a version"
                )),
                ExternalOutcome::Deleted => deleted.push(path.clone()),
                ExternalOutcome::Unchanged | ExternalOutcome::Synced => {}
            }
        }

        self.refresh_vault(&changed, cx);

        // Followed rename: a deleted open note whose text appeared under a new name.
        if let Some(vault) = &self.vault {
            let appeared: Vec<(String, String)> = changed
                .iter()
                .filter(|p| !known_before.contains(*p))
                .filter_map(|p| vault.corpus.get(p).map(|t| (p.clone(), t.to_string())))
                .collect();
            let mut renames = Vec::new();
            for old in &deleted {
                let doc = &self.docs[old];
                if let Some((new, _)) = appeared.iter().find(|(new, t)| {
                    !self.docs.contains_key(new) && (t == doc.base() || t == doc.text())
                }) {
                    renames.push((old.clone(), new.clone()));
                }
            }
            for (old, new) in renames {
                deleted.retain(|d| d != &old);
                self.follow_rename(&old, &new);
                let disk = std::fs::read_to_string(root.join(&new)).ok();
                if let Some(doc) = self.docs.get_mut(&new) {
                    doc.on_disk_changed(disk.as_deref());
                }
            }
        }
        if !deleted.is_empty() {
            self.set_error(format!(
                "Deleted by another app: {} (text kept; Save again to restore)",
                deleted.join(", ")
            ));
        }
        for p in to_save {
            self.save_logged(&p);
        }
        self.refresh_outline();
        self.refresh_reader();
        self.persist_session();
        cx.notify();
    }

    /// Moves tabs, documents and remembered paths from `old` to `new` (a note or a folder).
    fn follow_rename(&mut self, old: &str, new: &str) {
        self.tabs.rename(old, new);
        let moved: Vec<String> = self
            .docs
            .keys()
            .filter(|p| rename_target(p, old, new).is_some())
            .cloned()
            .collect();
        for p in moved {
            if let (Some(doc), Some(np)) = (self.docs.remove(&p), rename_target(&p, old, new)) {
                self.docs.insert(np, doc);
            }
        }
        let map = |p: &String| rename_target(p, old, new).unwrap_or_else(|| p.clone());
        self.recent_notes = self.recent_notes.iter().map(map).collect();
        self.expanded_folders = self.expanded_folders.iter().map(map).collect();
        if let Some(ep) = &self.editor_path {
            self.editor_path = Some(map(ep));
        }
    }

    pub fn keep_mine(&mut self, path: &str, cx: &mut Context<Self>) {
        if let Some(doc) = self.docs.get_mut(path) {
            doc.keep_mine();
            self.save_logged(path);
            self.status = None;
        }
        cx.notify();
    }

    pub fn take_theirs(&mut self, path: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(doc) = self.docs.get_mut(path) else {
            return;
        };
        if doc.take_theirs() {
            let text = doc.text().to_string();
            if self.editor_path.as_deref() == Some(path) {
                let cursor = self.editor.read(cx).cursor();
                self.set_editor_text(&text, cursor, window, cx);
            }
            self.status = None;
            self.after_text_change(window, cx);
        }
    }

    pub fn save_again(&mut self, path: &str, cx: &mut Context<Self>) {
        if let Some(doc) = self.docs.get_mut(path) {
            doc.save_again();
            self.save_logged(path);
            let changed = HashSet::from([path.to_string()]);
            self.refresh_vault(&changed, cx);
            self.status = None;
        }
        cx.notify();
    }

    // ---- Links ----

    /// Follows `[[target#heading]]` from the active note: opens it, or creates
    /// an empty note when it resolves nowhere (SPEC › Wikilinks).
    pub fn follow_link(
        &mut self,
        target: &str,
        heading: Option<&str>,
        new_tab: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mode = if new_tab {
            OpenIn::NewTab
        } else {
            OpenIn::CurrentTab
        };
        let source = self.active_path().map(str::to_string);
        let Some(vault) = &self.vault else { return };
        if target.is_empty() {
            if let Some(h) = heading {
                self.jump_to_heading(h, window, cx);
            }
            return;
        }
        let resolved = vault
            .index
            .resolve(target, source.as_deref())
            .map(|r| match r {
                ResolvedItem::Note { path, .. } | ResolvedItem::Attachment { path, .. } => path,
            });
        let path = match resolved {
            Some(p) => p,
            None => match new_note_path_for_link(
                &vault.root,
                target,
                source.as_deref(),
                &vault.excluded,
            )
            .and_then(|p| create_empty_note(&vault.root, &p))
            {
                Ok(p) => {
                    self.refresh_vault(&HashSet::from([p.clone()]), cx);
                    p
                }
                Err(e) => {
                    self.set_error(format!("Cannot create “{target}”: {e}"));
                    cx.notify();
                    return;
                }
            },
        };
        if self.open_path(&path, mode, window, cx) {
            if let Some(h) = heading {
                self.jump_to_heading(h, window, cx);
            }
        }
    }

    pub fn follow_reader_link(
        &mut self,
        url: &str,
        new_tab: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match markdown::parse_reader_link(url) {
            ReaderLink::External(url) => cx.open_url(&url),
            ReaderLink::Vault { target, heading } => {
                self.follow_link(&target, heading.as_deref(), new_tab, window, cx)
            }
        }
    }

    pub(crate) fn follow_link_under_cursor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let ed = self.editor.read(cx);
        let (text, cursor) = (ed.text().to_string(), ed.cursor());
        if let Some(link) = markdown::wikilink_at(&text, cursor) {
            self.follow_link(&link.target, link.heading.as_deref(), false, window, cx);
        }
    }

    fn jump_to_heading(&mut self, heading: &str, window: &mut Window, cx: &mut Context<Self>) {
        let wanted = heading.trim().to_lowercase();
        if let Some(line) = self
            .outline
            .iter()
            .find(|h| h.text.to_lowercase() == wanted)
            .map(|h| h.line)
        {
            self.jump_to_line(line, window, cx);
        }
    }

    pub fn jump_to_line(
        &mut self,
        line_one_based: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let line = line_one_based.saturating_sub(1);
        self.editor.update(cx, |ed, cx| {
            let lines = logical_lines(ed.text());
            let start = lines.get(line).or(lines.last()).map_or(0, |l| l.start);
            ed.set_selected_range(start..start, cx);
            ed.focus(window, cx);
        });
        if self.settings.view_mode == ViewMode::Reader {
            self.set_view_mode(ViewMode::Live, cx);
        }
    }

    /// A Reader checkbox was clicked; `marker_offset` is in the Reader Markdown.
    pub fn toggle_task(
        &mut self,
        marker_offset: usize,
        checked: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((path, md)) = self.reader_markdown.clone() else {
            return;
        };
        let line = line_of_offset(&md, marker_offset);
        let Some(new_text) = self
            .docs
            .get(&path)
            .and_then(|d| set_task_checked(d.text(), line, checked))
        else {
            return;
        };
        self.replace_note_text(&path, new_text, window, cx);
        self.save_logged(&path);
    }

    // ---- Files ----

    /// Creates `name.md` in `folder` (vault root if empty) and opens it.
    pub fn create_note(
        &mut self,
        folder: &str,
        name: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<String, String> {
        let name = name.trim().trim_end_matches(".md");
        validate_item_name(name)?;
        let vault = self.vault.as_ref().ok_or("No vault open")?;
        let rel = if folder.is_empty() {
            format!("{name}.md")
        } else {
            format!("{folder}/{name}.md")
        };
        let rel = create_empty_note(&vault.root, &rel)?;
        self.refresh_vault(&HashSet::from([rel.clone()]), cx);
        self.open_path(&rel, OpenIn::CurrentTab, window, cx);
        Ok(rel)
    }

    pub fn create_folder(
        &mut self,
        parent: &str,
        name: &str,
        cx: &mut Context<Self>,
    ) -> Result<String, String> {
        let name = name.trim();
        validate_item_name(name)?;
        let vault = self.vault.as_ref().ok_or("No vault open")?;
        let rel = if parent.is_empty() {
            name.to_string()
        } else {
            format!("{parent}/{name}")
        };
        if vault.root.join(&rel).exists() {
            return Err(format!("Already exists: {rel}"));
        }
        create_folder(&vault.root, &rel)?;
        if !parent.is_empty() {
            self.expanded_folders.insert(parent.to_string());
        }
        self.refresh_vault(&HashSet::new(), cx);
        cx.notify();
        Ok(rel)
    }

    /// Renames a note, attachment or folder; open tabs follow it.
    pub fn rename(
        &mut self,
        path: &str,
        new_name: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<String, String> {
        // Unsaved text is saved first, under the old name (SPEC › Files).
        let under: Vec<String> = self
            .docs
            .keys()
            .filter(|p| rename_target(p, path, "").is_some())
            .cloned()
            .collect();
        for p in &under {
            self.save_note(p)?;
        }
        let vault = self.vault.as_ref().ok_or("No vault open")?;
        let is_folder = vault.is_folder(path);
        let new_name = if is_folder {
            new_name.trim()
        } else {
            new_name.trim().trim_end_matches(".md")
        };
        let new_path = rename_item(&vault.root, path, new_name, is_folder)?;
        self.follow_rename(path, &new_path);
        self.refresh_vault(&HashSet::from([new_path.clone()]), cx);
        self.show_active(window, cx);
        Ok(new_path)
    }

    /// Moves a note, attachment or folder to the Trash; its tabs close after saving.
    pub fn trash(
        &mut self,
        path: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let under: Vec<String> = self
            .docs
            .keys()
            .filter(|p| rename_target(p, path, "").is_some())
            .cloned()
            .collect();
        for p in &under {
            self.save_note(p)?;
        }
        let vault = self.vault.as_ref().ok_or("No vault open")?;
        move_to_trash(&vault.root, path, self.config.trash_dir.as_deref())?;
        // A note whose text is only on screen (a conflict) keeps its tab, as deleted.
        let mut keep = HashSet::new();
        for p in &under {
            if let Some(doc) = self.docs.get_mut(p) {
                if doc.is_only_on_screen() {
                    doc.on_disk_changed(None);
                    keep.insert(p.clone());
                }
            }
        }
        let tab_paths: Vec<String> = self.tabs.iter().map(|t| t.path.clone()).collect();
        for p in tab_paths
            .iter()
            .filter(|p| rename_target(p, path, "").is_some() && !keep.contains(*p))
        {
            self.tabs.remove(p);
        }
        self.docs
            .retain(|p, _| rename_target(p, path, "").is_none() || keep.contains(p));
        self.refresh_vault(&HashSet::new(), cx);
        self.show_active(window, cx);
        Ok(())
    }

    pub fn duplicate(
        &mut self,
        path: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<String, String> {
        self.save_note(path)?;
        let vault = self.vault.as_ref().ok_or("No vault open")?;
        let new_path = make_copy(&vault.root, path)?;
        self.refresh_vault(&HashSet::from([new_path.clone()]), cx);
        self.open_path(&new_path, OpenIn::NewTab, window, cx);
        Ok(new_path)
    }

    pub(crate) fn open_daily_note(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(vault) = &self.vault else { return };
        let rel = today_daily_note_path("Daily", "YYYY-MM-DD");
        let result = if vault.contains(&rel) {
            Ok(rel)
        } else {
            create_empty_note(&vault.root, &rel)
        };
        match result {
            Ok(rel) => {
                self.refresh_vault(&HashSet::from([rel.clone()]), cx);
                self.open_path(&rel, OpenIn::CurrentTab, window, cx);
            }
            Err(e) => {
                self.set_error(e);
                cx.notify();
            }
        }
    }

    pub(crate) fn run_prompt(
        &mut self,
        kind: PromptKind,
        value: &str,
        target: Option<&str>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let target = target.unwrap_or("");
        let result = match kind {
            PromptKind::NewNote => self.create_note(target, value, window, cx).map(|_| ()),
            PromptKind::NewFolder => self.create_folder(target, value, cx).map(|_| ()),
            PromptKind::Rename => self.rename(target, value, window, cx).map(|_| ()),
        };
        self.report(result);
        cx.notify();
    }

    /// The folder new notes go to: the one selected in the tree, or the active note's.
    pub(crate) fn target_folder(&self, cx: &App) -> String {
        let vault = match &self.vault {
            Some(v) => v,
            None => return String::new(),
        };
        let selected = self
            .file_tree
            .read(cx)
            .selected_item()
            .map(|i| i.id.to_string());
        match selected.or_else(|| self.active_path().map(str::to_string)) {
            Some(p) if vault.is_folder(&p) => p,
            Some(p) => folder_of(&p).to_string(),
            None => String::new(),
        }
    }

    // ---- Derived views ----

    fn refresh_outline(&mut self) {
        self.outline = self
            .editor_path
            .as_ref()
            .and_then(|p| self.docs.get(p))
            .map(|d| extract_outline(d.text()))
            .unwrap_or_default();
    }

    fn refresh_backlinks(&mut self) {
        self.backlinks = match (&self.vault, self.active_path()) {
            (Some(v), Some(p)) => find_backlinks(&v.corpus, p, &v.index),
            _ => Vec::new(),
        };
    }

    fn refresh_reader(&mut self) {
        self.reader_markdown = None;
        if !self.settings.view_mode.shows_reader() {
            return;
        }
        let (Some(path), Some(vault)) = (self.editor_path.clone(), &self.vault) else {
            return;
        };
        if self.active_path() != Some(path.as_str()) {
            return;
        }
        if let Some(doc) = self.docs.get(&path) {
            let md = preprocess_for_reader(doc.text(), Some(&path), &vault.index);
            let (md, _parsed) = markdown::reader_guard(md, doc.text());
            self.reader_markdown = Some((path, md.into()));
        }
    }

    pub(crate) fn refresh_search(&mut self, cx: &App) {
        let query = self.search_input.read(cx).value().to_string();
        self.search_results = match &self.vault {
            Some(v) if !query.trim().is_empty() => search_vault(&v.corpus, &query),
            _ => Vec::new(),
        };
    }

    pub(crate) fn wrap_selection(
        &mut self,
        prefix: &str,
        suffix: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.editor_path.is_none() || self.settings.view_mode == ViewMode::Reader {
            return;
        }
        self.editor.update(cx, |ed, cx| {
            let selected = ed.selected_text().to_string();
            ed.replace_selection(&format!("{prefix}{selected}{suffix}"), cx);
            ed.focus(window, cx);
        });
    }

    pub(crate) fn insert_wikilink(
        &mut self,
        link: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.editor.update(cx, |ed, cx| {
            ed.replace_selection(&format!("[[{link}]]"), cx);
            ed.focus(window, cx);
        });
    }

    pub(crate) fn link_completions(&self) -> Vec<(String, String, String)> {
        let Some(v) = &self.vault else {
            return Vec::new();
        };
        let c = v.index.link_completions(&self.recent_notes);
        c.recent
            .into_iter()
            .chain(c.others)
            .map(|l| (l.name, l.folder, l.link))
            .collect()
    }
}

/// `docs/demo-vault` of this repository, when running from a checkout.
pub fn find_demo_vault() -> Option<PathBuf> {
    [
        PathBuf::from("docs/demo-vault"),
        PathBuf::from("../docs/demo-vault"),
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../docs/demo-vault"),
    ]
    .into_iter()
    .find(|p| p.is_dir())
}
