use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};

use gpui_kit::assets::IconName as AssetIcon;
use gpui_kit::base::Disableable as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::command::{Command, CommandItem, CommandState};
use gpui_kit::component::dialog::DialogButtonProps;
use gpui_kit::component::input::{
    Editor, EditorState, Input, InputEvent, InputState, Position, TextDecoration,
    TextDecorationCollection,
};
use gpui_kit::component::list::ListItem;
use gpui_kit::component::resizable::{h_resizable, resizable_panel};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::tab::{Tab, TabBar};
use gpui_kit::component::text::{FrontmatterPlugin, MarkdownExtensions, TextView};
use gpui_kit::component::tree::{tree, TreeItem, TreeState};
use gpui_kit::component::{
    h_flex, v_flex, ActiveTheme as _, Icon, Sizable as _, StyledExt as _, TitleBar,
    WindowExt as _,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    actions, div, hsla, px, App, AppContext as _, ClickEvent, Context, Entity, FocusHandle,
    Focusable, FontStyle, FontWeight, HighlightStyle, InteractiveElement as _, IntoElement,
    KeyBinding, ParentElement as _, PathPromptOptions, Render, StrikethroughStyle, Styled as _,
    Subscription, UnderlineStyle, Window,
};

use crate::core::embeds::{resolve_embed, EmbeddedFile};
use crate::core::index::{
    build_tag_tree, compute_markdown_spans, extract_outline, find_backlinks,
    preprocess_markdown_for_reader, quick_switcher_rank, search_vault, BacklinkMatch,
    HeadingItem, MdSpanKind, SearchResult, TagNode,
};
use crate::core::merge::merge_three_way;
use crate::core::vault::{
    create_empty_note, create_folder, make_copy, move_to_trash, new_note_path_for_link,
    normalize_excluded, read_note, rename_item, resolve_vault_path, scan_vault,
    today_daily_note_path, write_note_atomic, AttachmentKind, VaultItem,
};
use crate::core::wikilink::{folder_of, note_name, ResolvedItem, VaultIndex, Wikilink};
use crate::theme::{apply_noteki_theme, AppearanceMode, NotekiPalette, NotekiThemeName};

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
        KeyBinding::new("cmd-shift-d", DailyNoteAction, None),
        KeyBinding::new("cmd-w", CloseTabAction, None),
        KeyBinding::new("cmd-shift-t", ReopenClosedTabAction, None),
        KeyBinding::new("ctrl-tab", NextTabAction, None),
        KeyBinding::new("cmd-shift-]", NextTabAction, None),
        KeyBinding::new("ctrl-shift-tab", PrevTabAction, None),
        KeyBinding::new("cmd-shift-[", PrevTabAction, None),
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LeftPane {
    Files,
    Tags,
    Search,
}

impl Default for LeftPane {
    fn default() -> Self {
        Self::Files
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RightPane {
    Outline,
    Backlinks,
}

impl Default for RightPane {
    fn default() -> Self {
        Self::Outline
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ViewMode {
    Live,
    Split,
    Reader,
}

impl Default for ViewMode {
    fn default() -> Self {
        Self::Live
    }
}

impl ViewMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Live => "Live",
            Self::Split => "Split",
            Self::Reader => "Reader",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Live => Self::Split,
            Self::Split => Self::Reader,
            Self::Reader => Self::Live,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
struct PersistedSettings {
    vault_path: Option<String>,
    recent_vaults: Vec<String>,
    open_tabs: Vec<String>,
    active_tab: Option<String>,
    recent_notes: Vec<String>,
    left_sidebar_open: bool,
    right_sidebar_open: bool,
    left_pane: LeftPane,
    right_pane: RightPane,
    view_mode: ViewMode,
    theme_name: NotekiThemeName,
    appearance: AppearanceMode,
}

impl PersistedSettings {
    fn default_with_sidebars() -> Self {
        Self {
            left_sidebar_open: true,
            right_sidebar_open: true,
            ..Self::default()
        }
    }
}

fn settings_file_path() -> Option<PathBuf> {
    let dir = dirs::data_dir()?.join("Noteki");
    Some(dir.join("gpui-settings.json"))
}

fn load_settings() -> PersistedSettings {
    let Some(path) = settings_file_path() else {
        return PersistedSettings::default_with_sidebars();
    };
    if let Ok(raw) = std::fs::read_to_string(&path) {
        if let Ok(parsed) = serde_json::from_str::<PersistedSettings>(&raw) {
            return parsed;
        }
    }
    PersistedSettings::default_with_sidebars()
}

fn save_settings(settings: &PersistedSettings) {
    let Some(path) = settings_file_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(settings) {
        let _ = std::fs::write(path, json);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TabKind {
    Note,
    Attachment(AttachmentKind),
}

#[derive(Debug, Clone)]
struct TabEntry {
    path: String,
    kind: TabKind,
    back_stack: Vec<String>,
    forward_stack: Vec<String>,
    cursor_line: u32,
    cursor_col: u32,
}

#[derive(Debug, Clone)]
struct OpenNoteState {
    base: String,
    buffer: String,
    dirty: bool,
    conflict: bool,
    disk_conflict_text: Option<String>,
    deleted: bool,
    last_self_write: Option<Instant>,
    save_generation: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PromptKind {
    NewNote,
    NewFolder,
    RenameItem,
}

pub struct NotekiApp {
    focus_handle: FocusHandle,

    vault_root: Option<PathBuf>,
    scan: Vec<VaultItem>,
    index: VaultIndex,
    note_list: Vec<(String, String)>, // (name, path)

    tabs: Vec<TabEntry>,
    active_tab_idx: Option<usize>,
    closed_tabs: Vec<String>,
    recent_notes: Vec<String>,
    recent_vaults: Vec<String>,

    open_notes: HashMap<String, OpenNoteState>,

    left_sidebar_open: bool,
    right_sidebar_open: bool,
    left_pane: LeftPane,
    right_pane: RightPane,
    view_mode: ViewMode,

    theme_name: NotekiThemeName,
    appearance: AppearanceMode,
    palette: NotekiPalette,

    file_tree_state: Entity<TreeState>,
    expanded_folders: HashSet<String>,
    selected_tree_path: Option<String>,

    tag_tree: Vec<TagNode>,
    selected_tag: Option<String>,

    search_input: Entity<InputState>,
    search_results: Vec<SearchResult>,

    editor: Entity<EditorState>,
    live_decorations: Option<TextDecorationCollection>,
    suppress_editor_change: bool,

    outline: Vec<HeadingItem>,
    backlinks: Vec<BacklinkMatch>,
    reader_cache_source: Option<(String, String)>, // (path, preprocessed)

    watcher: Option<RecommendedWatcher>,
    watcher_rx: Option<mpsc::Receiver<Vec<PathBuf>>>,

    _subscriptions: Vec<Subscription>,
}

impl Focusable for NotekiApp {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl NotekiApp {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut settings = load_settings();
        if !settings.left_sidebar_open && !settings.right_sidebar_open && settings.vault_path.is_none() {
            settings = PersistedSettings::default_with_sidebars();
        }

        let palette = apply_noteki_theme(settings.theme_name, settings.appearance, window, cx);

        let editor = cx.new(|cx| {
            EditorState::new(window, cx)
                .language("markdown")
                .soft_wrap(true)
                .line_number(false)
                .folding(false)
                .searchable(true)
        });

        let search_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Search notes, tag:work, path:Journal...")
        });

        let file_tree_state = cx.new(|cx| TreeState::new(cx));

        let mut subs = Vec::new();
        subs.push(cx.subscribe_in(&editor, window, Self::on_editor_event));
        subs.push(cx.subscribe_in(&search_input, window, Self::on_search_input_event));

        let mut app = Self {
            focus_handle: cx.focus_handle(),
            vault_root: None,
            scan: Vec::new(),
            index: VaultIndex::default(),
            note_list: Vec::new(),
            tabs: Vec::new(),
            active_tab_idx: None,
            closed_tabs: Vec::new(),
            recent_notes: settings.recent_notes.clone(),
            recent_vaults: settings.recent_vaults.clone(),
            open_notes: HashMap::new(),
            left_sidebar_open: settings.left_sidebar_open,
            right_sidebar_open: settings.right_sidebar_open,
            left_pane: settings.left_pane,
            right_pane: settings.right_pane,
            view_mode: settings.view_mode,
            theme_name: settings.theme_name,
            appearance: settings.appearance,
            palette,
            file_tree_state,
            expanded_folders: HashSet::new(),
            selected_tree_path: None,
            tag_tree: Vec::new(),
            selected_tag: None,
            search_input,
            search_results: Vec::new(),
            editor,
            live_decorations: None,
            suppress_editor_change: false,
            outline: Vec::new(),
            backlinks: Vec::new(),
            reader_cache_source: None,
            watcher: None,
            watcher_rx: None,
            _subscriptions: subs,
        };

        // Load saved vault or demo-vault if present
        let initial_vault = settings
            .vault_path
            .as_deref()
            .map(PathBuf::from)
            .filter(|p| p.is_dir())
            .or_else(find_demo_vault);

        if let Some(vault_path) = initial_vault {
            app.load_vault(vault_path, Some(&settings), window, cx);
        }

        // Start file-watcher polling loop
        app.start_watcher_poller(window, cx);

        app
    }

    fn persist_settings(&self) {
        let settings = PersistedSettings {
            vault_path: self
                .vault_root
                .as_ref()
                .map(|p| p.to_string_lossy().to_string()),
            recent_vaults: self.recent_vaults.clone(),
            open_tabs: self.tabs.iter().map(|t| t.path.clone()).collect(),
            active_tab: self.active_tab_path().map(str::to_string),
            recent_notes: self.recent_notes.clone(),
            left_sidebar_open: self.left_sidebar_open,
            right_sidebar_open: self.right_sidebar_open,
            left_pane: self.left_pane,
            right_pane: self.right_pane,
            view_mode: self.view_mode,
            theme_name: self.theme_name,
            appearance: self.appearance,
        };
        save_settings(&settings);
    }

    fn active_tab(&self) -> Option<&TabEntry> {
        self.active_tab_idx.and_then(|i| self.tabs.get(i))
    }

    fn active_tab_path(&self) -> Option<&str> {
        self.active_tab().map(|t| t.path.as_str())
    }

    fn load_vault(
        &mut self,
        root: PathBuf,
        restore: Option<&PersistedSettings>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.flush_all_dirty();

        let root = std::fs::canonicalize(&root).unwrap_or(root);
        let root_str = root.to_string_lossy().to_string();
        self.recent_vaults.retain(|v| v != &root_str);
        self.recent_vaults.insert(0, root_str);
        self.recent_vaults.truncate(8);

        self.vault_root = Some(root.clone());
        self.tabs.clear();
        self.active_tab_idx = None;
        self.open_notes.clear();
        self.closed_tabs.clear();
        self.selected_tag = None;
        self.selected_tree_path = None;
        self.reader_cache_source = None;

        self.rescan_vault(cx);
        self.setup_watcher(&root);

        if let Some(saved) = restore {
            if saved.vault_path.as_deref() == Some(root.to_string_lossy().as_ref()) {
                for tab_path in &saved.open_tabs {
                    if self.item_exists_in_scan(tab_path) {
                        self.open_file_in_tab(tab_path, true, false, None, window, cx);
                    }
                }
                if let Some(active) = &saved.active_tab {
                    if let Some(idx) = self.tabs.iter().position(|t| &t.path == active) {
                        self.activate_tab(idx, window, cx);
                    }
                }
            }
        }

        if self.tabs.is_empty() {
            if let Some((_, first_path)) = self.note_list.first().cloned() {
                self.open_file_in_tab(&first_path, false, false, None, window, cx);
            }
        }

        self.persist_settings();
        cx.notify();
    }

    fn rescan_vault(&mut self, cx: &mut Context<Self>) {
        let Some(root) = &self.vault_root else {
            return;
        };
        let excluded = normalize_excluded(&[]);
        if let Ok(scan) = scan_vault(root, &excluded) {
            let mut notes = Vec::new();
            collect_notes_from_entries(&scan, &mut notes);
            self.index = VaultIndex::from_tree(&scan);
            self.note_list = notes;
            self.scan = scan;
            self.tag_tree = build_tag_tree(root, &self.note_list);

            let tree_items = build_tree_items(&self.scan, &self.expanded_folders);
            self.file_tree_state.update(cx, |state, cx| {
                state.set_items(tree_items, cx);
            });

            self.refresh_search(cx);
            self.refresh_inspector();
        }
    }

    fn item_exists_in_scan(&self, path: &str) -> bool {
        self.note_list.iter().any(|(_, p)| p == path)
            || attachment_kind_for(path, &self.scan).is_some()
    }

    fn setup_watcher(&mut self, root: &Path) {
        let (tx, rx) = mpsc::channel::<Vec<PathBuf>>();
        let watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            if let Ok(event) = res {
                if !event.paths.is_empty() {
                    let _ = tx.send(event.paths);
                }
            }
        });
        if let Ok(mut w) = watcher {
            if w.watch(root, RecursiveMode::Recursive).is_ok() {
                self.watcher = Some(w);
                self.watcher_rx = Some(rx);
                return;
            }
        }
        self.watcher = None;
        self.watcher_rx = None;
    }

    fn start_watcher_poller(&self, window: &mut Window, cx: &mut Context<Self>) {
        cx.spawn_in(window, async move |this, cx| loop {
            cx.background_executor()
                .timer(Duration::from_millis(180))
                .await;
            let keep_going = this.update_in(cx, |app, window, cx| {
                app.poll_watcher(window, cx);
            });
            if keep_going.is_err() {
                break;
            }
        })
        .detach();
    }

    fn poll_watcher(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(rx) = &self.watcher_rx else {
            return;
        };
        let mut changed_paths = Vec::new();
        while let Ok(paths) = rx.try_recv() {
            changed_paths.extend(paths);
        }
        if changed_paths.is_empty() {
            return;
        }
        let Some(root) = self.vault_root.clone() else {
            return;
        };

        let mut relevant_rel_paths = HashSet::new();
        for abs in changed_paths {
            if let Ok(rel) = abs.strip_prefix(&root) {
                let rel_str = rel.to_string_lossy().replace('\\', "/");
                if !rel_str.is_empty() && !rel_str.split('/').any(|s| s.starts_with('.')) {
                    relevant_rel_paths.insert(rel_str);
                }
            }
        }
        if relevant_rel_paths.is_empty() {
            return;
        }

        // Check if any open note changed externally
        let open_paths: Vec<String> = self.open_notes.keys().cloned().collect();
        let active_path = self.active_tab_path().map(str::to_string);
        for rel_path in open_paths {
            if !relevant_rel_paths.contains(&rel_path) {
                continue;
            }
            let Some(note) = self.open_notes.get(&rel_path).cloned() else {
                continue;
            };
            if let Some(last) = note.last_self_write {
                if last.elapsed() < Duration::from_millis(1000) {
                    continue;
                }
            }

            let Ok(disk_path) = resolve_vault_path(&root, &rel_path) else {
                continue;
            };
            if !disk_path.exists() {
                if let Some(entry) = self.open_notes.get_mut(&rel_path) {
                    entry.deleted = true;
                }
                continue;
            }
            let Ok(disk_text) = std::fs::read_to_string(&disk_path) else {
                continue;
            };
            if disk_text == note.buffer {
                if let Some(entry) = self.open_notes.get_mut(&rel_path) {
                    entry.base = disk_text;
                    entry.dirty = false;
                    entry.conflict = false;
                    entry.deleted = false;
                }
                continue;
            }

            let merged = merge_three_way(&note.base, &note.buffer, &disk_text);
            if let Some(entry) = self.open_notes.get_mut(&rel_path) {
                entry.deleted = false;
                if let Some(merged_text) = merged {
                    entry.base = disk_text;
                    entry.buffer = merged_text.clone();
                    entry.conflict = false;
                    entry.disk_conflict_text = None;
                    entry.dirty = entry.buffer != entry.base;
                    if active_path.as_deref() == Some(&rel_path) {
                        self.sync_editor_from_buffer(&merged_text, window, cx);
                    }
                } else {
                    entry.conflict = true;
                    entry.disk_conflict_text = Some(disk_text);
                }
            }
        }

        self.rescan_vault(cx);
        cx.notify();
    }

    fn open_file_in_tab(
        &mut self,
        rel_path: &str,
        new_tab: bool,
        record_history: bool,
        jump_line: Option<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(root) = &self.vault_root else {
            return;
        };

        // Determine Note vs Attachment
        let kind = if let Some(att_kind) = attachment_kind_for(rel_path, &self.scan) {
            TabKind::Attachment(att_kind)
        } else {
            TabKind::Note
        };

        // Load note into open_notes if Note
        if kind == TabKind::Note && !self.open_notes.contains_key(rel_path) {
            match read_note(root, rel_path) {
                Ok(text) => {
                    self.open_notes.insert(
                        rel_path.to_string(),
                        OpenNoteState {
                            base: text.clone(),
                            buffer: text,
                            dirty: false,
                            conflict: false,
                            disk_conflict_text: None,
                            deleted: false,
                            last_self_write: None,
                            save_generation: 0,
                        },
                    );
                }
                Err(_) => return,
            }
        }

        // Save current active tab's cursor + flush dirty before switching
        self.save_active_cursor(cx);
        if let Some(prev) = self.active_tab_path().map(str::to_string) {
            if prev != rel_path {
                self.flush_note(&prev);
            }
        }

        // If already open in a tab, switch to it
        if let Some(existing_idx) = self.tabs.iter().position(|t| t.path == rel_path) {
            self.activate_tab(existing_idx, window, cx);
            if let Some(line) = jump_line {
                self.jump_to_line(line, window, cx);
            }
            return;
        }

        if new_tab || self.tabs.is_empty() {
            self.tabs.push(TabEntry {
                path: rel_path.to_string(),
                kind,
                back_stack: Vec::new(),
                forward_stack: Vec::new(),
                cursor_line: jump_line.map(|l| l.saturating_sub(1) as u32).unwrap_or(0),
                cursor_col: 0,
            });
            let idx = self.tabs.len() - 1;
            self.activate_tab(idx, window, cx);
        } else if let Some(idx) = self.active_tab_idx {
            let old_path = self.tabs[idx].path.clone();
            if record_history && old_path != rel_path {
                self.tabs[idx].back_stack.push(old_path);
                self.tabs[idx].forward_stack.clear();
            }
            self.tabs[idx].path = rel_path.to_string();
            self.tabs[idx].kind = kind;
            self.tabs[idx].cursor_line = jump_line.map(|l| l.saturating_sub(1) as u32).unwrap_or(0);
            self.tabs[idx].cursor_col = 0;
            self.activate_tab(idx, window, cx);
        }

        if let Some(line) = jump_line {
            self.jump_to_line(line, window, cx);
        }
    }

    fn activate_tab(&mut self, idx: usize, window: &mut Window, cx: &mut Context<Self>) {
        if idx >= self.tabs.len() {
            return;
        }
        self.active_tab_idx = Some(idx);
        let tab = self.tabs[idx].clone();
        self.selected_tree_path = Some(tab.path.clone());

        // Update recent notes
        self.recent_notes.retain(|p| p != &tab.path);
        self.recent_notes.insert(0, tab.path.clone());
        self.recent_notes.truncate(40);

        if tab.kind == TabKind::Note {
            if let Some(note) = self.open_notes.get(&tab.path) {
                let text = note.buffer.clone();
                self.sync_editor_from_buffer(&text, window, cx);
                self.editor.update(cx, |ed, cx| {
                    ed.set_cursor_position(
                        Position {
                            line: tab.cursor_line,
                            character: tab.cursor_col,
                        },
                        window,
                        cx,
                    );
                });
                self.apply_live_decorations(window, cx);
            }
        }

        self.refresh_inspector();
        self.refresh_reader_cache();
        self.persist_settings();
        cx.notify();
    }

    fn save_active_cursor(&mut self, cx: &App) {
        let Some(idx) = self.active_tab_idx else {
            return;
        };
        if let Some(tab) = self.tabs.get_mut(idx) {
            let pos = self.editor.read(cx).cursor_position();
            tab.cursor_line = pos.line;
            tab.cursor_col = pos.character;
        }
    }

    fn close_tab(&mut self, idx: usize, window: &mut Window, cx: &mut Context<Self>) {
        if idx >= self.tabs.len() {
            return;
        }
        let path = self.tabs[idx].path.clone();
        self.flush_note(&path);
        self.tabs.remove(idx);
        self.closed_tabs.push(path.clone());
        if self.closed_tabs.len() > 20 {
            self.closed_tabs.remove(0);
        }

        // Remove from open_notes if no other tab holds it
        if !self.tabs.iter().any(|t| t.path == path) {
            self.open_notes.remove(&path);
        }

        if self.tabs.is_empty() {
            self.active_tab_idx = None;
            self.outline.clear();
            self.backlinks.clear();
            self.reader_cache_source = None;
        } else {
            let next_idx = idx.min(self.tabs.len() - 1);
            self.activate_tab(next_idx, window, cx);
        }
        self.persist_settings();
        cx.notify();
    }

    fn sync_editor_from_buffer(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.suppress_editor_change = true;
        self.editor.update(cx, |ed, cx| {
            ed.set_value(text.to_string(), window, cx);
        });
        self.suppress_editor_change = false;
        self.apply_live_decorations(window, cx);
    }

    fn on_editor_event(
        &mut self,
        _: &Entity<EditorState>,
        event: &InputEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            InputEvent::Change => {
                if self.suppress_editor_change {
                    return;
                }
                let Some(active_path) = self.active_tab_path().map(str::to_string) else {
                    return;
                };
                let text = self.editor.read(cx).value().to_string();
                let mut gen = 0u64;
                if let Some(note) = self.open_notes.get_mut(&active_path) {
                    note.buffer = text.clone();
                    note.dirty = note.buffer != note.base;
                    note.save_generation = note.save_generation.wrapping_add(1);
                    gen = note.save_generation;
                }
                self.apply_live_decorations(window, cx);
                self.refresh_inspector();
                self.refresh_reader_cache();

                // Debounced auto-save (~900ms)
                let path_clone = active_path.clone();
                cx.spawn_in(window, async move |this, cx| {
                    cx.background_executor()
                        .timer(Duration::from_millis(900))
                        .await;
                    let _ = this.update_in(cx, |app, _, cx| {
                        let should_save = app
                            .open_notes
                            .get(&path_clone)
                            .map(|n| n.dirty && n.save_generation == gen && !n.conflict)
                            .unwrap_or(false);
                        if should_save {
                            app.flush_note(&path_clone);
                            app.rescan_vault(cx);
                            cx.notify();
                        }
                    });
                })
                .detach();

                cx.notify();
            }
            _ => {
                // Recompute live-preview syntax hiding on cursor line movements / focus changes
                self.apply_live_decorations(window, cx);
            }
        }
    }

    fn apply_live_decorations(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(active_path) = self.active_tab_path() else {
            return;
        };
        let ed_state = self.editor.read(cx);
        let text = ed_state.value().to_string();
        let cursor_line = ed_state.cursor_position().line as usize;
        let spans = compute_markdown_spans(&text, cursor_line, Some(active_path), &self.index);
        let p = self.palette;

        let mut decorations = Vec::with_capacity(spans.len());
        for span in spans {
            let style = match span.kind {
                MdSpanKind::SyntaxMarker => HighlightStyle {
                    color: Some(p.syntax),
                    fade_out: Some(0.68),
                    ..Default::default()
                },
                MdSpanKind::FrontmatterDelimiter => HighlightStyle {
                    color: Some(p.syntax),
                    fade_out: Some(0.45),
                    ..Default::default()
                },
                MdSpanKind::FrontmatterKey => HighlightStyle {
                    color: Some(p.keyword),
                    font_weight: Some(FontWeight::MEDIUM),
                    ..Default::default()
                },
                MdSpanKind::Heading(level) => HighlightStyle {
                    color: Some(p.heading),
                    font_weight: Some(if level <= 2 {
                        FontWeight::BOLD
                    } else {
                        FontWeight::SEMIBOLD
                    }),
                    ..Default::default()
                },
                MdSpanKind::Bold => HighlightStyle {
                    font_weight: Some(FontWeight::BOLD),
                    ..Default::default()
                },
                MdSpanKind::Italic => HighlightStyle {
                    font_style: Some(FontStyle::Italic),
                    ..Default::default()
                },
                MdSpanKind::Strikethrough | MdSpanKind::TaskDone => HighlightStyle {
                    color: Some(p.syntax),
                    strikethrough: Some(StrikethroughStyle {
                        thickness: px(1.),
                        color: Some(p.syntax),
                    }),
                    ..Default::default()
                },
                MdSpanKind::Highlight => HighlightStyle {
                    background_color: Some(p.highlight),
                    ..Default::default()
                },
                MdSpanKind::InlineCode | MdSpanKind::CodeBlock => HighlightStyle {
                    background_color: Some(p.code_bg),
                    ..Default::default()
                },
                MdSpanKind::WikilinkResolved | MdSpanKind::MarkdownLink => HighlightStyle {
                    color: Some(p.link),
                    underline: Some(UnderlineStyle {
                        thickness: px(1.),
                        color: Some(p.link),
                        wavy: false,
                    }),
                    ..Default::default()
                },
                MdSpanKind::WikilinkUnresolved => HighlightStyle {
                    color: Some(p.error),
                    underline: Some(UnderlineStyle {
                        thickness: px(1.),
                        color: Some(p.error),
                        wavy: true,
                    }),
                    ..Default::default()
                },
                MdSpanKind::Tag => HighlightStyle {
                    color: Some(p.accent),
                    font_weight: Some(FontWeight::MEDIUM),
                    ..Default::default()
                },
                MdSpanKind::Comment => HighlightStyle {
                    color: Some(p.syntax),
                    font_style: Some(FontStyle::Italic),
                    fade_out: Some(0.45),
                    ..Default::default()
                },
                MdSpanKind::Quote => HighlightStyle {
                    color: Some(p.quote),
                    font_style: Some(FontStyle::Italic),
                    ..Default::default()
                },
                MdSpanKind::CalloutHeader => HighlightStyle {
                    color: Some(p.link),
                    font_weight: Some(FontWeight::SEMIBOLD),
                    background_color: Some(hsla(p.link.h, p.link.s, p.link.l, 0.1)),
                    ..Default::default()
                },
            };
            decorations.push(TextDecoration::new(span.range, style));
        }

        if let Some(collection) = &self.live_decorations {
            collection.set(decorations, cx);
        } else {
            let collection = self
                .editor
                .update(cx, |ed, cx| ed.create_decorations_collection(decorations, cx));
            self.live_decorations = Some(collection);
        }
    }

    fn flush_note(&mut self, rel_path: &str) {
        let Some(root) = &self.vault_root else {
            return;
        };
        let Some(note) = self.open_notes.get_mut(rel_path) else {
            return;
        };
        if !note.dirty || note.conflict {
            return;
        }
        let text = note.buffer.clone();
        if write_note_atomic(root, rel_path, &text).is_ok() {
            note.base = text;
            note.dirty = false;
            note.deleted = false;
            note.last_self_write = Some(Instant::now());
        }
    }

    fn flush_all_dirty(&mut self) {
        let paths: Vec<String> = self
            .open_notes
            .iter()
            .filter(|(_, n)| n.dirty && !n.conflict)
            .map(|(p, _)| p.clone())
            .collect();
        for p in paths {
            self.flush_note(&p);
        }
    }

    fn refresh_inspector(&mut self) {
        let Some(active) = self.active_tab_path().map(str::to_string) else {
            self.outline.clear();
            self.backlinks.clear();
            return;
        };
        if let Some(note) = self.open_notes.get(&active) {
            self.outline = extract_outline(&note.buffer);
        } else {
            self.outline.clear();
        }
        if let Some(root) = &self.vault_root {
            self.backlinks = find_backlinks(root, &self.note_list, &active, &self.index);
        } else {
            self.backlinks.clear();
        }
    }

    fn refresh_reader_cache(&mut self) {
        let Some(tab) = self.active_tab().cloned() else {
            self.reader_cache_source = None;
            return;
        };
        if tab.kind != TabKind::Note {
            self.reader_cache_source = None;
            return;
        }
        if let Some(note) = self.open_notes.get(&tab.path) {
            let preprocessed =
                preprocess_markdown_for_reader(&note.buffer, Some(&tab.path), &self.index);
            self.reader_cache_source = Some((tab.path, preprocessed));
        }
    }

    fn jump_to_line(&mut self, line_one_based: usize, window: &mut Window, cx: &mut Context<Self>) {
        let line = line_one_based.saturating_sub(1) as u32;
        self.editor.update(cx, |ed, cx| {
            ed.set_cursor_position(Position { line, character: 0 }, window, cx);
            ed.focus(window, cx);
        });
        self.apply_live_decorations(window, cx);
    }

    fn on_search_input_event(
        &mut self,
        _: &Entity<InputState>,
        event: &InputEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if matches!(event, InputEvent::Change) {
            self.refresh_search(cx);
            cx.notify();
        }
    }

    fn refresh_search(&mut self, cx: &App) {
        let Some(root) = &self.vault_root else {
            self.search_results.clear();
            return;
        };
        let query = self.search_input.read(cx).value().to_string();
        if query.trim().is_empty() {
            self.search_results.clear();
        } else {
            self.search_results = search_vault(root, &self.note_list, &query);
        }
    }

    fn handle_wikilink_click(
        &mut self,
        dest: &str,
        new_tab: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let source_path = self.active_tab_path().map(str::to_string);
        let target_raw = dest.strip_prefix("noteki-link://").unwrap_or(dest);

        // Same-note heading link `#Heading`
        if let Some(heading_target) = target_raw.strip_prefix('#') {
            if let Some(item) = self
                .outline
                .iter()
                .find(|h| h.text.eq_ignore_ascii_case(heading_target.trim()))
            {
                let line = item.line;
                self.jump_to_line(line, window, cx);
            }
            return;
        }

        let (target_note, heading) = match target_raw.split_once('#') {
            Some((t, h)) => (t.trim(), Some(h.trim())),
            None => (target_raw.trim(), None),
        };

        if let Some(resolved) = self.index.resolve(target_note, source_path.as_deref()) {
            let resolved_path = match resolved {
                ResolvedItem::Note { path, .. } | ResolvedItem::Attachment { path, .. } => path,
            };
            self.open_file_in_tab(&resolved_path, new_tab, true, None, window, cx);
            if let Some(h) = heading {
                if let Some(item) = self
                    .outline
                    .iter()
                    .find(|item| item.text.eq_ignore_ascii_case(h))
                {
                    let line = item.line;
                    self.jump_to_line(line, window, cx);
                }
            }
            return;
        }

        // Unresolved wikilink: create new note using Noteki's folder-aware link path helper
        let Some(root) = &self.vault_root else {
            return;
        };
        let excluded = normalize_excluded(&[]);
        let Ok(base_rel) = new_note_path_for_link(root, target_note, source_path.as_deref(), &excluded) else {
            return;
        };
        let title = note_name(&base_rel);
        let initial = format!("# {title}\n\n");
        if let Ok(created_path) = create_empty_note(root, &base_rel) {
            let _ = write_note_atomic(root, &created_path, &initial);
            self.rescan_vault(cx);
            self.open_file_in_tab(&created_path, new_tab, true, None, window, cx);
        }
    }

    fn toggle_reader_task(
        &mut self,
        source_range: Option<std::ops::Range<usize>>,
        checked: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(active_path) = self.active_tab_path().map(str::to_string) else {
            return;
        };
        let Some(note) = self.open_notes.get(&active_path) else {
            return;
        };
        let text = note.buffer.clone();
        // Best-effort toggle using source_range line estimate or matching task marker
        let updated = if let Some(range) = source_range {
            toggle_checkbox_near_offset(&text, range.start, checked)
        } else {
            None
        };
        if let Some(new_text) = updated {
            if let Some(note) = self.open_notes.get_mut(&active_path) {
                note.buffer = new_text.clone();
                note.dirty = note.buffer != note.base;
            }
            self.sync_editor_from_buffer(&new_text, window, cx);
            self.flush_note(&active_path);
            self.refresh_reader_cache();
            self.refresh_inspector();
            cx.notify();
        }
    }

    fn open_vault_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Select Markdown Vault Folder".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(paths))) = rx.await {
                if let Some(folder) = paths.into_iter().next() {
                    let _ = this.update_in(cx, |app, window, cx| {
                        app.load_vault(folder, None, window, cx);
                    });
                }
            }
        })
        .detach();
    }

    fn create_daily_note(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(root) = &self.vault_root else {
            return;
        };
        let rel = today_daily_note_path("Journal", "[Daily_]YYYY/YYYY-MM-DD");
        if self.item_exists_in_scan(&rel) {
            self.open_file_in_tab(&rel, false, true, None, window, cx);
            return;
        }
        let title = chrono::Local::now().format("%Y-%m-%d").to_string();
        let content = format!("# {title}\n\n");
        if let Ok(created) = create_empty_note(root, &rel) {
            let _ = write_note_atomic(root, &created, &content);
            self.rescan_vault(cx);
            self.open_file_in_tab(&created, false, true, None, window, cx);
        }
    }

    fn open_name_prompt(
        &mut self,
        kind: PromptKind,
        default_value: String,
        target_path: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let input = cx.new(|cx| {
            let mut s = InputState::new(window, cx).default_value(default_value);
            s = match kind {
                PromptKind::NewNote => s.placeholder("Note title (e.g. Meeting Notes)"),
                PromptKind::NewFolder => s.placeholder("Folder name"),
                PromptKind::RenameItem => s.placeholder("New name"),
            };
            s
        });
        let entity = cx.entity();
        let title = match kind {
            PromptKind::NewNote => "New Note",
            PromptKind::NewFolder => "New Folder",
            PromptKind::RenameItem => "Rename",
        };
        let ok_label = match kind {
            PromptKind::NewNote => "Create",
            PromptKind::NewFolder => "Create",
            PromptKind::RenameItem => "Rename",
        };

        window.open_dialog(cx, move |dialog, window, cx| {
            let input_clone = input.clone();
            let entity_clone = entity.clone();
            let target_clone = target_path.clone();

            input.update(cx, |s, cx| s.focus(window, cx));

            dialog
                .title(title)
                .width(px(400.))
                .button_props(
                    DialogButtonProps::default()
                        .ok_text(ok_label)
                        .show_cancel(true),
                )
                .child(v_flex().gap_2().child(Input::new(&input)))
                .on_ok(move |_, window, cx| {
                    let value = input_clone.read(cx).value().trim().to_string();
                    if value.is_empty() {
                        return true;
                    }
                    let target_for_update = target_clone.clone();
                    entity_clone.update(cx, |app, cx| {
                        app.execute_prompt(kind, &value, target_for_update.as_deref(), window, cx);
                    });
                    true
                })
        });
    }

    fn execute_prompt(
        &mut self,
        kind: PromptKind,
        value: &str,
        target_path: Option<&str>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(root) = self.vault_root.clone() else {
            return;
        };
        match kind {
            PromptKind::NewNote => {
                let folder = target_path.unwrap_or("");
                let clean = value.strip_suffix(".md").unwrap_or(value);
                let rel = if folder.is_empty() {
                    format!("{clean}.md")
                } else {
                    format!("{folder}/{clean}.md")
                };
                let initial = format!("# {clean}\n\n");
                if let Ok(created) = create_empty_note(&root, &rel) {
                    let _ = write_note_atomic(&root, &created, &initial);
                    self.rescan_vault(cx);
                    self.open_file_in_tab(&created, false, true, None, window, cx);
                }
            }
            PromptKind::NewFolder => {
                let parent = target_path.unwrap_or("");
                let rel = if parent.is_empty() {
                    value.to_string()
                } else {
                    format!("{parent}/{value}")
                };
                if create_folder(&root, &rel).is_ok() {
                    self.expanded_folders.insert(rel);
                    self.rescan_vault(cx);
                    cx.notify();
                }
            }
            PromptKind::RenameItem => {
                let Some(old_rel) = target_path else {
                    return;
                };
                let is_folder = is_folder_in_scan(old_rel, &self.scan);
                if let Ok(new_rel) = rename_item(&root, old_rel, value, is_folder) {
                    if let Some(note_state) = self.open_notes.remove(old_rel) {
                        self.open_notes.insert(new_rel.clone(), note_state);
                    }
                    for tab in &mut self.tabs {
                        if tab.path == old_rel {
                            tab.path = new_rel.clone();
                        }
                    }
                    self.rescan_vault(cx);
                    self.refresh_inspector();
                    self.persist_settings();
                    cx.notify();
                }
            }
        }
    }

    fn delete_selected_item(&mut self, rel_path: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(root) = &self.vault_root else {
            return;
        };
        if move_to_trash(root, rel_path).is_ok() {
            if let Some(idx) = self.tabs.iter().position(|t| t.path == rel_path) {
                self.close_tab(idx, window, cx);
            }
            self.rescan_vault(cx);
            cx.notify();
        }
    }

    fn duplicate_note(&mut self, rel_path: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.flush_note(rel_path);
        let Some(root) = &self.vault_root else {
            return;
        };
        if let Ok(new_rel) = make_copy(root, rel_path) {
            self.rescan_vault(cx);
            self.open_file_in_tab(&new_rel, true, true, None, window, cx);
        }
    }

    fn open_quick_switcher(&mut self, new_tab: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.note_list.is_empty() {
            return;
        }
        let ranked = quick_switcher_rank(&self.note_list, &self.recent_notes, "");
        let items: Vec<CommandItem> = ranked
            .iter()
            .map(|entry| {
                let label = if entry.folder.is_empty() {
                    entry.name.clone()
                } else {
                    format!("{}  ·  {}", entry.name, entry.folder)
                };
                CommandItem::new()
                    .label(label)
                    .icon(AssetIcon::FileText)
                    .keywords([entry.name.clone(), entry.path.clone()])
            })
            .collect();
        let paths: Vec<String> = ranked.into_iter().map(|e| e.path).collect();

        let cmd_state = cx.new(|cx| CommandState::new(window, cx));
        let entity = cx.entity();

        window.open_dialog(cx, move |dialog, _, _| {
            let paths_confirm = paths.clone();
            let entity_confirm = entity.clone();

            dialog
                .width(px(540.))
                .close_button(false)
                .p_0()
                .child(
                    Command::new(&cmd_state)
                        .bordered(false)
                        .placeholder("Open note in vault…")
                        .items(items.clone())
                        .on_confirm(move |index_path, window, cx| {
                            if let Some(target_path) = paths_confirm.get(index_path.row).cloned() {
                                window.close_dialog(cx);
                                entity_confirm.update(cx, |app, cx| {
                                    app.open_file_in_tab(
                                        &target_path,
                                        new_tab,
                                        true,
                                        None,
                                        window,
                                        cx,
                                    );
                                });
                            }
                        })
                        .on_cancel(|window, cx| {
                            window.close_dialog(cx);
                        }),
                )
        });
    }

    fn open_command_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        #[derive(Clone)]
        struct PaletteCmd {
            label: &'static str,
            subtitle: &'static str,
            icon: AssetIcon,
            run: fn(&mut NotekiApp, &mut Window, &mut Context<NotekiApp>),
        }

        let cmds: Vec<PaletteCmd> = vec![
            PaletteCmd {
                label: "New Note",
                subtitle: "Create a new Markdown note (⌘N)",
                icon: AssetIcon::FilePlus,
                run: |app, w, cx| app.open_name_prompt(PromptKind::NewNote, String::new(), None, w, cx),
            },
            PaletteCmd {
                label: "Open Today's Daily Note",
                subtitle: "Journal/Daily_YYYY/YYYY-MM-DD.md (⌘⇧D)",
                icon: AssetIcon::CalendarDays,
                run: |app, w, cx| app.create_daily_note(w, cx),
            },
            PaletteCmd {
                label: "Quick Switcher",
                subtitle: "Jump to any note (⌘O)",
                icon: AssetIcon::Search,
                run: |app, w, cx| app.open_quick_switcher(false, w, cx),
            },
            PaletteCmd {
                label: "Insert Wikilink…",
                subtitle: "Insert [[Note]] at cursor (⌘K)",
                icon: AssetIcon::Link,
                run: |app, w, cx| app.open_wikilink_picker(w, cx),
            },
            PaletteCmd {
                label: "Follow Wikilink Under Cursor",
                subtitle: "Open [[link]] at current cursor (⌘↩)",
                icon: AssetIcon::ArrowRight,
                run: |app, w, cx| app.follow_wikilink_under_cursor(w, cx),
            },
            PaletteCmd {
                label: "Cycle View Mode (Live / Split / Reader)",
                subtitle: "Switch editor mode (⌘E)",
                icon: AssetIcon::Columns2,
                run: |app, w, cx| {
                    app.view_mode = app.view_mode.next();
                    app.refresh_reader_cache();
                    app.persist_settings();
                    let _ = w;
                    cx.notify();
                },
            },
            PaletteCmd {
                label: "Toggle Left Sidebar",
                subtitle: "Files / Tags / Search (⌃⌘S)",
                icon: AssetIcon::PanelLeft,
                run: |app, _, cx| {
                    app.left_sidebar_open = !app.left_sidebar_open;
                    app.persist_settings();
                    cx.notify();
                },
            },
            PaletteCmd {
                label: "Toggle Right Sidebar",
                subtitle: "Outline / Backlinks (⌥⌘0)",
                icon: AssetIcon::PanelRight,
                run: |app, _, cx| {
                    app.right_sidebar_open = !app.right_sidebar_open;
                    app.persist_settings();
                    cx.notify();
                },
            },
            PaletteCmd {
                label: "Cycle Theme (Ayu / Everforest / One Dark Pro)",
                subtitle: "Switch Noteki color palette",
                icon: AssetIcon::Palette,
                run: |app, w, cx| app.cycle_theme(w, cx),
            },
            PaletteCmd {
                label: "Cycle Appearance (System / Light / Dark)",
                subtitle: "Switch light or dark mode",
                icon: AssetIcon::Sun,
                run: |app, w, cx| app.cycle_appearance(w, cx),
            },
            PaletteCmd {
                label: "Open Vault Folder…",
                subtitle: "Choose a local Markdown vault (⌘⇧O)",
                icon: AssetIcon::FolderOpen,
                run: |app, w, cx| app.open_vault_picker(w, cx),
            },
            PaletteCmd {
                label: "Close Current Tab",
                subtitle: "Close active tab (⌘W)",
                icon: AssetIcon::X,
                run: |app, w, cx| {
                    if let Some(i) = app.active_tab_idx {
                        app.close_tab(i, w, cx);
                    }
                },
            },
        ];

        let items: Vec<CommandItem> = cmds
            .iter()
            .map(|c| {
                CommandItem::new()
                    .label(format!("{}  ·  {}", c.label, c.subtitle))
                    .icon(c.icon)
                    .keywords([c.label, c.subtitle])
            })
            .collect();

        let cmd_state = cx.new(|cx| CommandState::new(window, cx));
        let entity = cx.entity();

        window.open_dialog(cx, move |dialog, _, _| {
            let cmds_confirm = cmds.clone();
            let entity_confirm = entity.clone();

            dialog
                .width(px(540.))
                .close_button(false)
                .p_0()
                .child(
                    Command::new(&cmd_state)
                        .bordered(false)
                        .placeholder("Type a command…")
                        .items(items.clone())
                        .on_confirm(move |index_path, window, cx| {
                            if let Some(cmd) = cmds_confirm.get(index_path.row).cloned() {
                                window.close_dialog(cx);
                                entity_confirm.update(cx, |app, cx| {
                                    (cmd.run)(app, window, cx);
                                });
                            }
                        })
                        .on_cancel(|window, cx| {
                            window.close_dialog(cx);
                        }),
                )
        });
    }

    fn cycle_theme(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.theme_name = self.theme_name.next();
        self.palette = apply_noteki_theme(self.theme_name, self.appearance, window, cx);
        self.apply_live_decorations(window, cx);
        self.persist_settings();
        cx.notify();
    }

    fn cycle_appearance(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.appearance = self.appearance.next();
        self.palette = apply_noteki_theme(self.theme_name, self.appearance, window, cx);
        self.apply_live_decorations(window, cx);
        self.persist_settings();
        cx.notify();
    }

    fn open_wikilink_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.note_list.is_empty() {
            return;
        }
        let completions = self.index.link_completions(&self.recent_notes);
        let mut all = completions.recent;
        all.extend(completions.others);
        if all.is_empty() {
            return;
        }

        let items: Vec<CommandItem> = all
            .iter()
            .map(|c| {
                let label = if c.folder.is_empty() {
                    c.name.clone()
                } else {
                    format!("{}  ·  {}", c.name, c.folder)
                };
                CommandItem::new()
                    .label(label)
                    .icon(AssetIcon::Link)
                    .keywords([c.name.clone(), c.link.clone()])
            })
            .collect();
        let links: Vec<String> = all.into_iter().map(|c| c.link).collect();

        let cmd_state = cx.new(|cx| CommandState::new(window, cx));
        let entity = cx.entity();

        window.open_dialog(cx, move |dialog, _, _| {
            let links_confirm = links.clone();
            let entity_confirm = entity.clone();

            dialog
                .width(px(500.))
                .close_button(false)
                .p_0()
                .child(
                    Command::new(&cmd_state)
                        .bordered(false)
                        .placeholder("Insert [[wikilink]]…")
                        .items(items.clone())
                        .on_confirm(move |index_path, window, cx| {
                            if let Some(link) = links_confirm.get(index_path.row).cloned() {
                                window.close_dialog(cx);
                                entity_confirm.update(cx, |app, cx| {
                                    app.editor.update(cx, |ed, cx| {
                                        ed.insert(format!("[[{link}]]"), window, cx);
                                        ed.focus(window, cx);
                                    });
                                });
                            }
                        })
                        .on_cancel(|window, cx| {
                            window.close_dialog(cx);
                        }),
                )
        });
    }

    fn follow_wikilink_under_cursor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let ed = self.editor.read(cx);
        let pos = ed.cursor_position();
        let text = ed.value().to_string();
        let Some(line_str) = text.split('\n').nth(pos.line as usize) else {
            return;
        };
        let char_col = pos.character as usize;
        let byte_col = line_str
            .char_indices()
            .nth(char_col)
            .map(|(b, _)| b)
            .unwrap_or(line_str.len());

        // Find [[...]] enclosing byte_col
        let mut i = 0usize;
        while i + 1 < line_str.len() {
            if let Some(rel_open) = line_str[i..].find("[[") {
                let open_idx = i + rel_open;
                if let Some(rel_close) = line_str[open_idx + 2..].find("]]") {
                    let close_idx = open_idx + 2 + rel_close;
                    if byte_col >= open_idx && byte_col <= close_idx + 2 {
                        let inner = &line_str[open_idx + 2..close_idx];
                        let parsed = Wikilink::parse(inner);
                        let dest = if parsed.target.is_empty() {
                            format!("#{}", parsed.heading.as_deref().unwrap_or(""))
                        } else if let Some(h) = &parsed.heading {
                            format!("{}#{h}", parsed.target)
                        } else {
                            parsed.target
                        };
                        self.handle_wikilink_click(&dest, false, window, cx);
                        return;
                    }
                    i = close_idx + 2;
                    continue;
                }
            }
            break;
        }
    }

    fn wrap_selection_inline(
        &mut self,
        prefix: &str,
        suffix: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.view_mode == ViewMode::Reader {
            return;
        }
        self.editor.update(cx, |ed, cx| {
            let selected = ed.selected_text().to_string();
            if !selected.is_empty() {
                let wrapped = format!("{prefix}{selected}{suffix}");
                ed.replace(wrapped, window, cx);
            } else {
                ed.insert(format!("{prefix}{suffix}"), window, cx);
            }
        });
    }

    fn resolve_conflict_keep_mine(&mut self, path: &str, cx: &mut Context<Self>) {
        let Some(root) = &self.vault_root else {
            return;
        };
        if let Some(note) = self.open_notes.get_mut(path) {
            let text = note.buffer.clone();
            if write_note_atomic(root, path, &text).is_ok() {
                note.base = text;
                note.dirty = false;
                note.conflict = false;
                note.disk_conflict_text = None;
                note.last_self_write = Some(Instant::now());
            }
        }
        cx.notify();
    }

    fn resolve_conflict_take_theirs(
        &mut self,
        path: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let disk_text = self
            .open_notes
            .get(path)
            .and_then(|n| n.disk_conflict_text.clone());
        if let Some(text) = disk_text {
            if let Some(note) = self.open_notes.get_mut(path) {
                note.base = text.clone();
                note.buffer = text.clone();
                note.dirty = false;
                note.conflict = false;
                note.disk_conflict_text = None;
            }
            if self.active_tab_path() == Some(path) {
                self.sync_editor_from_buffer(&text, window, cx);
                self.refresh_inspector();
                self.refresh_reader_cache();
            }
        }
        cx.notify();
    }

    fn resolve_deleted_save_again(&mut self, path: &str, cx: &mut Context<Self>) {
        if let Some(note) = self.open_notes.get_mut(path) {
            note.dirty = true;
            note.deleted = false;
        }
        self.flush_note(path);
        self.rescan_vault(cx);
        cx.notify();
    }

    // ---- Rendering helpers ----

    fn render_title_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let border_color = cx.theme().border;
        let vault_label = self
            .vault_root
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "Noteki".to_string());

        let has_back = self
            .active_tab()
            .map(|t| !t.back_stack.is_empty())
            .unwrap_or(false);
        let has_forward = self
            .active_tab()
            .map(|t| !t.forward_stack.is_empty())
            .unwrap_or(false);

        TitleBar::new().child(
            h_flex()
                .w_full()
                .items_center()
                .justify_between()
                .pr_3()
                .child(
                    h_flex()
                        .items_center()
                        .gap_1()
                        .child(
                            Button::new("toggle-left")
                                .icon(Icon::new(AssetIcon::PanelLeft))
                                .ghost()
                                .small()
                                .tooltip("Toggle Left Sidebar (⌃⌘S)")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.left_sidebar_open = !this.left_sidebar_open;
                                    this.persist_settings();
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new("nav-back")
                                .icon(Icon::new(AssetIcon::ArrowLeft))
                                .ghost()
                                .small()
                                .disabled(!has_back)
                                .tooltip("Back (⌥⌘←)")
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.navigate_back(window, cx);
                                })),
                        )
                        .child(
                            Button::new("nav-fwd")
                                .icon(Icon::new(AssetIcon::ArrowRight))
                                .ghost()
                                .small()
                                .disabled(!has_forward)
                                .tooltip("Forward (⌥⌘→)")
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.navigate_forward(window, cx);
                                })),
                        )
                        .child(
                            Button::new("vault-btn")
                                .label(vault_label)
                                .icon(Icon::new(AssetIcon::FolderOpen))
                                .ghost()
                                .small()
                                .tooltip("Open Vault Folder (⌘⇧O)")
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.open_vault_picker(window, cx);
                                })),
                        ),
                )
                .child(
                    h_flex()
                        .items_center()
                        .gap_1()
                        .child(
                            Button::new("quick-switcher-btn")
                                .label("Quick Open")
                                .icon(Icon::new(AssetIcon::Search))
                                .ghost()
                                .small()
                                .tooltip("Quick Switcher (⌘O)")
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.open_quick_switcher(false, window, cx);
                                })),
                        )
                        .child(
                            Button::new("cmd-palette-btn")
                                .icon(Icon::new(AssetIcon::Command))
                                .ghost()
                                .small()
                                .tooltip("Command Palette (⌘⇧P)")
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.open_command_palette(window, cx);
                                })),
                        ),
                )
                .child(
                    h_flex()
                        .items_center()
                        .gap_1()
                        .child(self.render_view_mode_buttons(cx))
                        .child(
                            div()
                                .w(px(1.))
                                .h(px(16.))
                                .mx_1()
                                .bg(border_color),
                        )
                        .child(
                            Button::new("theme-cycle")
                                .label(self.theme_name.label())
                                .icon(Icon::new(AssetIcon::Palette))
                                .ghost()
                                .small()
                                .tooltip("Cycle Theme (Ayu / Everforest / One Dark Pro)")
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.cycle_theme(window, cx);
                                })),
                        )
                        .child(
                            Button::new("appearance-cycle")
                                .label(self.appearance.label())
                                .icon(Icon::new(if self.palette.dark {
                                    AssetIcon::Moon
                                } else {
                                    AssetIcon::Sun
                                }))
                                .ghost()
                                .small()
                                .tooltip("Cycle Appearance (System / Light / Dark)")
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.cycle_appearance(window, cx);
                                })),
                        )
                        .child(
                            Button::new("toggle-right")
                                .icon(Icon::new(AssetIcon::PanelRight))
                                .ghost()
                                .small()
                                .tooltip("Toggle Right Sidebar (⌥⌘0)")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.right_sidebar_open = !this.right_sidebar_open;
                                    this.persist_settings();
                                    cx.notify();
                                })),
                        ),
                ),
        )
    }

    fn render_view_mode_buttons(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let modes = [
            (ViewMode::Live, "Live", AssetIcon::Pencil),
            (ViewMode::Split, "Split", AssetIcon::Columns2),
            (ViewMode::Reader, "Reader", AssetIcon::BookOpen),
        ];
        let mut row = h_flex().items_center().gap(px(2.));
        for (mode, label, icon) in modes {
            let active = self.view_mode == mode;
            let btn = Button::new(("view-mode", mode as usize))
                .label(label)
                .icon(Icon::new(icon))
                .xsmall();
            let btn = if active { btn.primary() } else { btn.ghost() };
            row = row.child(btn.on_click(cx.listener(move |this, _, _, cx| {
                this.view_mode = mode;
                this.refresh_reader_cache();
                this.persist_settings();
                cx.notify();
            })));
        }
        row
    }

    fn render_left_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let sidebar_bg = cx.theme().sidebar;
        let sidebar_fg = cx.theme().sidebar_foreground;
        let sidebar_border = cx.theme().sidebar_border;

        let pane_tabs = h_flex()
            .px_2()
            .py_1()
            .gap_1()
            .border_b_1()
            .border_color(sidebar_border)
            .child(self.pane_tab_btn("Files", AssetIcon::Folder, LeftPane::Files, cx))
            .child(self.pane_tab_btn("Tags", AssetIcon::Tag, LeftPane::Tags, cx))
            .child(self.pane_tab_btn("Search", AssetIcon::Search, LeftPane::Search, cx));

        let body = match self.left_pane {
            LeftPane::Files => self.render_files_pane(cx).into_any_element(),
            LeftPane::Tags => self.render_tags_pane(cx).into_any_element(),
            LeftPane::Search => self.render_search_pane(cx).into_any_element(),
        };

        v_flex()
            .size_full()
            .bg(sidebar_bg)
            .text_color(sidebar_fg)
            .child(pane_tabs)
            .child(div().flex_1().min_h_0().child(body))
    }

    fn pane_tab_btn(
        &self,
        label: &'static str,
        icon: AssetIcon,
        pane: LeftPane,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let active = self.left_pane == pane;
        let btn = Button::new(("left-pane", pane as usize))
            .label(label)
            .icon(Icon::new(icon))
            .xsmall();
        let btn = if active { btn.primary() } else { btn.ghost() };
        btn.on_click(cx.listener(move |this, _, _, cx| {
            this.left_pane = pane;
            this.persist_settings();
            cx.notify();
        }))
    }

    fn render_files_pane(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let entity = cx.entity();
        let selected_tree_path = self.selected_tree_path.clone();
        let active_tab_path = self.active_tab_path().map(str::to_string);

        let toolbar = h_flex()
            .px_2()
            .py_1()
            .gap_1()
            .border_b_1()
            .border_color(theme.sidebar_border)
            .child(
                Button::new("new-note-btn")
                    .icon(Icon::new(AssetIcon::FilePlus))
                    .label("Note")
                    .xsmall()
                    .ghost()
                    .tooltip("New Note (⌘N)")
                    .on_click(cx.listener(|this, _, window, cx| {
                        let folder = this
                            .selected_tree_path
                            .as_deref()
                            .map(|p| {
                                if p.ends_with(".md") || p.contains('.') {
                                    folder_of(p).to_string()
                                } else {
                                    p.to_string()
                                }
                            })
                            .filter(|s| !s.is_empty());
                        this.open_name_prompt(PromptKind::NewNote, String::new(), folder, window, cx);
                    })),
            )
            .child(
                Button::new("new-folder-btn")
                    .icon(Icon::new(AssetIcon::FolderPlus))
                    .xsmall()
                    .ghost()
                    .tooltip("New Folder")
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.open_name_prompt(PromptKind::NewFolder, String::new(), None, window, cx);
                    })),
            )
            .child(
                Button::new("daily-note-btn")
                    .icon(Icon::new(AssetIcon::CalendarDays))
                    .xsmall()
                    .ghost()
                    .tooltip("Today's Daily Note (⌘⇧D)")
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.create_daily_note(window, cx);
                    })),
            )
            .child(div().flex_1())
            .when_some(active_tab_path.clone(), |row, active_path| {
                let p_dup = active_path.clone();
                let p_ren = active_path.clone();
                let p_del = active_path.clone();
                row.child(
                    Button::new("rename-active-btn")
                        .icon(Icon::new(AssetIcon::Pencil))
                        .xsmall()
                        .ghost()
                        .tooltip("Rename Active Note")
                        .on_click(cx.listener(move |this, _, window, cx| {
                            let cur = note_name(&p_ren).to_string();
                            this.open_name_prompt(
                                PromptKind::RenameItem,
                                cur,
                                Some(p_ren.clone()),
                                window,
                                cx,
                            );
                        })),
                )
                .child(
                    Button::new("dup-active-btn")
                        .icon(Icon::new(AssetIcon::Copy))
                        .xsmall()
                        .ghost()
                        .tooltip("Duplicate Active Note")
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.duplicate_note(&p_dup, window, cx);
                        })),
                )
                .child(
                    Button::new("trash-active-btn")
                        .icon(Icon::new(AssetIcon::Trash))
                        .xsmall()
                        .ghost()
                        .tooltip("Move Active Note to Trash")
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.delete_selected_item(&p_del, window, cx);
                        })),
                )
            });

        v_flex()
            .size_full()
            .child(toolbar)
            .child(
                div().flex_1().min_h_0().p_1().child(
                    tree(&self.file_tree_state, move |ix, entry, selected, _, cx| {
                        let item = entry.item();
                        let id = item.id.to_string();
                        let label = item.label.clone();
                        let is_folder = entry.is_folder();
                        let is_expanded = entry.is_expanded();
                        let depth = entry.depth();
                        let is_active = selected
                            || selected_tree_path.as_deref() == Some(id.as_str());

                        let icon = if is_folder {
                            if is_expanded {
                                AssetIcon::FolderOpen
                            } else {
                                AssetIcon::Folder
                            }
                        } else {
                            AssetIcon::FileText
                        };

                        let entity_click = entity.clone();
                        let id_click = id.clone();

                        ListItem::new(ix)
                            .selected(is_active)
                            .rounded(px(5.))
                            .py(px(3.))
                            .pl(px(8. + depth as f32 * 14.))
                            .pr_2()
                            .child(
                                h_flex()
                                    .gap_2()
                                    .items_center()
                                    .min_w_0()
                                    .child(
                                        Icon::new(if is_folder {
                                            if is_expanded {
                                                AssetIcon::ChevronDown
                                            } else {
                                                AssetIcon::ChevronRight
                                            }
                                        } else {
                                            icon
                                        })
                                        .size_3p5()
                                        .text_color(cx.theme().muted_foreground),
                                    )
                                    .when(is_folder, |row| {
                                        row.child(
                                            Icon::new(icon)
                                                .size_3p5()
                                                .text_color(cx.theme().primary),
                                        )
                                    })
                                    .child(
                                        div()
                                            .flex_1()
                                            .truncate()
                                            .text_sm()
                                            .child(label),
                                    ),
                            )
                            .on_click(move |ev, window, cx| {
                                let new_tab = ev.modifiers().platform;
                                entity_click.update(cx, |app, cx| {
                                    if is_folder {
                                        if app.expanded_folders.contains(&id_click) {
                                            app.expanded_folders.remove(&id_click);
                                        } else {
                                            app.expanded_folders.insert(id_click.clone());
                                        }
                                        app.selected_tree_path = Some(id_click.clone());
                                        let items = build_tree_items(&app.scan, &app.expanded_folders);
                                        app.file_tree_state.update(cx, |s, cx| {
                                            s.set_items(items, cx);
                                        });
                                        cx.notify();
                                    } else {
                                        app.open_file_in_tab(
                                            &id_click,
                                            new_tab,
                                            true,
                                            None,
                                            window,
                                            cx,
                                        );
                                    }
                                });
                            })
                    }),
                ),
            )
    }

    fn render_tags_pane(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let primary = cx.theme().primary;
        let muted_fg = cx.theme().muted_foreground;
        let border = cx.theme().border;
        let mut flat_tags = Vec::new();
        flatten_tags(&self.tag_tree, 0, &mut flat_tags);

        let selected_notes: Vec<String> = self
            .selected_tag
            .as_deref()
            .and_then(|sel| find_tag_notes(&self.tag_tree, sel))
            .unwrap_or_default();

        v_flex()
            .size_full()
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scrollbar()
                    .p_2()
                    .child(if flat_tags.is_empty() {
                        div()
                            .text_sm()
                            .text_color(muted_fg)
                            .p_2()
                            .child("No tags in vault")
                            .into_any_element()
                    } else {
                        v_flex()
                            .gap(px(2.))
                            .children(flat_tags.into_iter().enumerate().map(|(ix, (depth, node))| {
                                let is_sel =
                                    self.selected_tag.as_deref() == Some(node.full_lower.as_str());
                                let full_lower = node.full_lower.clone();
                                ListItem::new(("tag-row", ix))
                                    .selected(is_sel)
                                    .rounded(px(5.))
                                    .py(px(3.))
                                    .pl(px(8. + depth as f32 * 14.))
                                    .pr_2()
                                    .child(
                                        h_flex()
                                            .w_full()
                                            .items_center()
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
                                                    .child(
                                                        div()
                                                            .text_sm()
                                                            .child(format!("#{}", node.label)),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(muted_fg)
                                                    .child(node.count.to_string()),
                                            ),
                                    )
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        if this.selected_tag.as_deref() == Some(full_lower.as_str()) {
                                            this.selected_tag = None;
                                        } else {
                                            this.selected_tag = Some(full_lower.clone());
                                        }
                                        cx.notify();
                                    }))
                            }))
                            .into_any_element()
                    }),
            )
            .when(!selected_notes.is_empty(), |col| {
                col.child(div().h(px(1.)).w_full().bg(border)).child(
                    div()
                        .max_h(px(220.))
                        .overflow_y_scrollbar()
                        .p_2()
                        .child(
                            v_flex()
                                .gap(px(2.))
                                .child(
                                    div()
                                        .text_xs()
                                        .font_semibold()
                                        .text_color(muted_fg)
                                        .px_2()
                                        .pb_1()
                                        .child(format!(
                                            "Notes with #{} ({})",
                                            self.selected_tag.as_deref().unwrap_or(""),
                                            selected_notes.len()
                                        )),
                                )
                                .children(selected_notes.into_iter().enumerate().map(
                                    |(ix, path)| {
                                        let name = note_name(&path);
                                        let path_click = path.clone();
                                        ListItem::new(("tag-note", ix))
                                            .rounded(px(5.))
                                            .py(px(3.))
                                            .px_2()
                                            .child(
                                                h_flex()
                                                    .gap_1p5()
                                                    .items_center()
                                                    .child(
                                                        Icon::new(AssetIcon::FileText)
                                                            .size_3p5()
                                                            .text_color(muted_fg),
                                                    )
                                                    .child(div().text_sm().truncate().child(name)),
                                            )
                                            .on_click(cx.listener(
                                                move |this, ev: &ClickEvent, window, cx| {
                                                    this.open_file_in_tab(
                                                        &path_click,
                                                        ev.modifiers().platform,
                                                        true,
                                                        None,
                                                        window,
                                                        cx,
                                                    );
                                                },
                                            ))
                                    },
                                )),
                        ),
                )
            })
    }

    fn render_search_pane(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let query_empty = self.search_input.read(cx).value().trim().is_empty();

        v_flex()
            .size_full()
            .child(
                div()
                    .p_2()
                    .border_b_1()
                    .border_color(theme.sidebar_border)
                    .child(Input::new(&self.search_input).cleanable(true).small()),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scrollbar()
                    .p_2()
                    .child(if query_empty {
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .p_2()
                            .child("Search across notes. Supports tag:work, path:Journal, and \"exact phrase\".")
                            .into_any_element()
                    } else if self.search_results.is_empty() {
                        div()
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .p_2()
                            .child("No matches found")
                            .into_any_element()
                    } else {
                        v_flex()
                            .gap_2()
                            .children(self.search_results.iter().enumerate().map(|(ix, res)| {
                                let res_path = res.path.clone();
                                v_flex()
                                    .gap(px(2.))
                                    .child(
                                        ListItem::new(("search-file", ix))
                                            .rounded(px(5.))
                                            .py(px(3.))
                                            .px_2()
                                            .child(
                                                h_flex()
                                                    .w_full()
                                                    .justify_between()
                                                    .items_center()
                                                    .child(
                                                        div()
                                                            .text_sm()
                                                            .font_semibold()
                                                            .truncate()
                                                            .child(res.name.clone()),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(theme.muted_foreground)
                                                            .child(res.total_line_matches.to_string()),
                                                    ),
                                            )
                                            .on_click(cx.listener(
                                                move |this, ev: &ClickEvent, window, cx| {
                                                    this.open_file_in_tab(
                                                        &res_path,
                                                        ev.modifiers().platform,
                                                        true,
                                                        None,
                                                        window,
                                                        cx,
                                                    );
                                                },
                                            )),
                                    )
                                    .children(res.lines.iter().enumerate().map(|(lx, lm)| {
                                        let line_path = res.path.clone();
                                        let line_no = lm.line;
                                        ListItem::new(("search-line", ix * 100 + lx))
                                            .rounded(px(4.))
                                            .py(px(2.))
                                            .pl_4()
                                            .pr_2()
                                            .child(
                                                h_flex()
                                                    .gap_1p5()
                                                    .items_baseline()
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(theme.muted_foreground)
                                                            .child(format!("{line_no}")),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .truncate()
                                                            .child(lm.text.clone()),
                                                    ),
                                            )
                                            .on_click(cx.listener(
                                                move |this, ev: &ClickEvent, window, cx| {
                                                    this.open_file_in_tab(
                                                        &line_path,
                                                        ev.modifiers().platform,
                                                        true,
                                                        Some(line_no),
                                                        window,
                                                        cx,
                                                    );
                                                },
                                            ))
                                    }))
                            }))
                            .into_any_element()
                    }),
            )
    }

    fn render_right_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let sidebar_bg = cx.theme().sidebar;
        let sidebar_fg = cx.theme().sidebar_foreground;
        let sidebar_border = cx.theme().sidebar_border;

        let header = h_flex()
            .px_2()
            .py_1()
            .gap_1()
            .border_b_1()
            .border_color(sidebar_border)
            .child({
                let active = self.right_pane == RightPane::Outline;
                let btn = Button::new("pane-outline")
                    .label("Outline")
                    .icon(Icon::new(AssetIcon::List))
                    .xsmall();
                let btn = if active { btn.primary() } else { btn.ghost() };
                btn.on_click(cx.listener(|this, _, _, cx| {
                    this.right_pane = RightPane::Outline;
                    this.persist_settings();
                    cx.notify();
                }))
            })
            .child({
                let active = self.right_pane == RightPane::Backlinks;
                let label = if self.backlinks.is_empty() {
                    "Backlinks".to_string()
                } else {
                    format!("Backlinks ({})", self.backlinks.len())
                };
                let btn = Button::new("pane-backlinks")
                    .label(label)
                    .icon(Icon::new(AssetIcon::Link))
                    .xsmall();
                let btn = if active { btn.primary() } else { btn.ghost() };
                btn.on_click(cx.listener(|this, _, _, cx| {
                    this.right_pane = RightPane::Backlinks;
                    this.persist_settings();
                    cx.notify();
                }))
            });

        let body = match self.right_pane {
            RightPane::Outline => self.render_outline_pane(cx).into_any_element(),
            RightPane::Backlinks => self.render_backlinks_pane(cx).into_any_element(),
        };

        v_flex()
            .size_full()
            .bg(sidebar_bg)
            .text_color(sidebar_fg)
            .child(header)
            .child(div().flex_1().min_h_0().overflow_y_scrollbar().p_2().child(body))
    }

    fn render_outline_pane(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        if self.outline.is_empty() {
            return div()
                .text_sm()
                .text_color(theme.muted_foreground)
                .p_2()
                .child("No headings in this note")
                .into_any_element();
        }
        v_flex()
            .gap(px(2.))
            .children(self.outline.iter().enumerate().map(|(ix, h)| {
                let line = h.line;
                let indent = (h.level.saturating_sub(1) as f32) * 12.0;
                ListItem::new(("outline-item", ix))
                    .rounded(px(5.))
                    .py(px(3.))
                    .pl(px(8. + indent))
                    .pr_2()
                    .child(
                        h_flex()
                            .gap_1p5()
                            .items_baseline()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child(format!("H{}", h.level)),
                            )
                            .child(div().text_sm().truncate().child(h.text.clone())),
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.jump_to_line(line, window, cx);
                    }))
            }))
            .into_any_element()
    }

    fn render_backlinks_pane(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        if self.backlinks.is_empty() {
            return div()
                .text_sm()
                .text_color(theme.muted_foreground)
                .p_2()
                .child("No incoming links to this note")
                .into_any_element();
        }
        v_flex()
            .gap_1p5()
            .children(self.backlinks.iter().enumerate().map(|(ix, bl)| {
                let path = bl.source_path.clone();
                let line = bl.line;
                ListItem::new(("backlink-item", ix))
                    .rounded(px(6.))
                    .p_2()
                    .child(
                        v_flex()
                            .gap(px(2.))
                            .child(
                                h_flex()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_semibold()
                                            .truncate()
                                            .child(bl.source_name.clone()),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(theme.muted_foreground)
                                            .child(format!("L{line}")),
                                    ),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .truncate()
                                    .child(bl.snippet.clone()),
                            ),
                    )
                    .on_click(cx.listener(move |this, ev: &ClickEvent, window, cx| {
                        this.open_file_in_tab(
                            &path,
                            ev.modifiers().platform,
                            true,
                            Some(line),
                            window,
                            cx,
                        );
                    }))
            }))
            .into_any_element()
    }

    fn render_center_pane(&self, cx: &mut Context<Self>) -> impl IntoElement {
        if self.vault_root.is_none() {
            return self.render_welcome_screen(cx).into_any_element();
        }

        let bg = cx.theme().background;
        let fg = cx.theme().foreground;

        let tab_bar = self.render_tab_bar(cx);
        let banners = self.render_status_banners(cx);

        let content = match self.active_tab().cloned() {
            None => self.render_empty_tab_state(cx).into_any_element(),
            Some(tab) => match tab.kind {
                TabKind::Attachment(kind) => self.render_attachment_view(&tab.path, kind, cx).into_any_element(),
                TabKind::Note => match self.view_mode {
                    ViewMode::Live => self.render_editor_pane(cx).into_any_element(),
                    ViewMode::Reader => self.render_reader_pane(cx).into_any_element(),
                    ViewMode::Split => h_resizable("split-editor-reader")
                        .child(
                            resizable_panel()
                                .size(px(520.))
                                .size_range(px(260.)..px(1400.))
                                .child(self.render_editor_pane(cx)),
                        )
                        .child(
                            resizable_panel()
                                .size_range(px(260.)..px(1400.))
                                .child(self.render_reader_pane(cx)),
                        )
                        .into_any_element(),
                },
            },
        };

        v_flex()
            .size_full()
            .bg(bg)
            .text_color(fg)
            .child(tab_bar)
            .child(banners)
            .child(div().flex_1().min_h_0().child(content))
            .into_any_element()
    }

    fn render_tab_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let active_idx = self.active_tab_idx.unwrap_or(0);
        let entity = cx.entity();

        let mut bar = TabBar::new("noteki-tabs")
            .selected_index(active_idx)
            .on_click(cx.listener(|this, ix: &usize, window, cx| {
                this.save_active_cursor(cx);
                this.activate_tab(*ix, window, cx);
            }));

        for (ix, tab) in self.tabs.iter().enumerate() {
            let name = if tab.kind == TabKind::Note {
                note_name(&tab.path).to_string()
            } else {
                tab.path.rsplit('/').next().unwrap_or(&tab.path).to_string()
            };
            let is_dirty = self
                .open_notes
                .get(&tab.path)
                .map(|n| n.dirty)
                .unwrap_or(false);
            let label = if is_dirty { format!("• {name}") } else { name };
            let entity_close = entity.clone();

            bar = bar.child(
                Tab::new().label(label).suffix(
                    Button::new(("close-tab", ix))
                        .icon(Icon::new(AssetIcon::X))
                        .xsmall()
                        .ghost()
                        .on_click(move |_, window, cx| {
                            cx.stop_propagation();
                            entity_close.update(cx, |app, cx| {
                                app.close_tab(ix, window, cx);
                            });
                        }),
                ),
            );
        }

        h_flex()
            .w_full()
            .items_center()
            .bg(theme.tab_bar)
            .border_b_1()
            .border_color(theme.border)
            .child(div().flex_1().min_w_0().child(bar))
            .child(
                div().px_1().child(
                    Button::new("new-tab-btn")
                        .icon(Icon::new(AssetIcon::Plus))
                        .xsmall()
                        .ghost()
                        .tooltip("New Tab (⌘T)")
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.open_quick_switcher(true, window, cx);
                        })),
                ),
            )
    }

    fn render_status_banners(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(active_path) = self.active_tab_path().map(str::to_string) else {
            return div().into_any_element();
        };
        let Some(note) = self.open_notes.get(&active_path) else {
            return div().into_any_element();
        };
        if !note.conflict && !note.deleted {
            return div().into_any_element();
        }
        let theme = cx.theme();
        let mut container = v_flex().w_full();

        if note.conflict {
            let p_mine = active_path.clone();
            let p_theirs = active_path.clone();
            container = container.child(
                h_flex()
                    .w_full()
                    .px_3()
                    .py_1p5()
                    .gap_2()
                    .items_center()
                    .justify_between()
                    .bg(hsla(theme.danger.h, theme.danger.s, theme.danger.l, 0.14))
                    .border_b_1()
                    .border_color(theme.danger)
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(
                                Icon::new(AssetIcon::TriangleAlert)
                                    .size_4()
                                    .text_color(theme.danger),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .child("Disk conflict: external edits could not be merged cleanly."),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap_1p5()
                            .child(
                                Button::new("keep-mine")
                                    .label("Keep mine")
                                    .xsmall()
                                    .primary()
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.resolve_conflict_keep_mine(&p_mine, cx);
                                    })),
                            )
                            .child(
                                Button::new("take-theirs")
                                    .label("Take disk version")
                                    .xsmall()
                                    .outline()
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.resolve_conflict_take_theirs(&p_theirs, window, cx);
                                    })),
                            ),
                    ),
            );
        }

        if note.deleted {
            let p_save = active_path;
            container = container.child(
                h_flex()
                    .w_full()
                    .px_3()
                    .py_1p5()
                    .gap_2()
                    .items_center()
                    .justify_between()
                    .bg(hsla(theme.warning.h, theme.warning.s, theme.warning.l, 0.14))
                    .border_b_1()
                    .border_color(theme.warning)
                    .child(
                        div()
                            .text_sm()
                            .child("This note was deleted on disk outside Noteki."),
                    )
                    .child(
                        Button::new("save-again")
                            .label("Save again")
                            .xsmall()
                            .primary()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.resolve_deleted_save_again(&p_save, cx);
                            })),
                    ),
            );
        }

        container.into_any_element()
    }

    fn render_editor_pane(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let _ = cx;
        div()
            .size_full()
            .px_6()
            .py_4()
            .child(
                div()
                    .size_full()
                    .max_w(px(820.))
                    .mx_auto()
                    .child(Editor::new(&self.editor).bordered(false).h_full()),
            )
    }

    fn render_reader_pane(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let Some((_, preprocessed)) = &self.reader_cache_source else {
            return div().into_any_element();
        };
        let entity = cx.entity();
        let entity_task = cx.entity();
        let vault_root = self.vault_root.clone();
        let index = self.index.clone();
        let active_path = self.active_tab_path().map(str::to_string);

        let mut view = TextView::markdown("noteki-reader", preprocessed.clone())
            .markdown_extensions(MarkdownExtensions::default().frontmatter())
            .plugin(FrontmatterPlugin::new())
            .scrollable(true)
            .on_link_click(move |url, ev, window, cx| {
                let url_str = url.to_string();
                let new_tab = ev.modifiers().platform;
                if url_str.starts_with("noteki-link://") || !url_str.contains("://") {
                    entity.update(cx, |app, cx| {
                        app.handle_wikilink_click(&url_str, new_tab, window, cx);
                    });
                } else {
                    cx.open_url(&url_str);
                }
            })
            .on_task_toggle(move |ev, window, cx| {
                entity_task.update(cx, |app, cx| {
                    app.toggle_reader_task(Some(ev.item_range()), ev.checked(), window, cx);
                });
            });

        if let Some(root) = vault_root {
            view = view.image_source(move |src| {
                let src_str: &str = src.as_ref();
                let target = src_str.strip_prefix("noteki-embed://").unwrap_or(src_str);
                if target.starts_with("http://") || target.starts_with("https://") {
                    return gpui_kit::ImageSource::from(target.to_string());
                }
                if let Some(ResolvedItem::Attachment { path, .. }) =
                    index.resolve(target, active_path.as_deref())
                {
                    if let Ok(disk) = resolve_vault_path(&root, &path) {
                        return gpui_kit::ImageSource::from(disk);
                    }
                }
                if let Ok(direct) = resolve_vault_path(&root, target) {
                    return gpui_kit::ImageSource::from(direct);
                }
                gpui_kit::ImageSource::from(target.to_string())
            });
        }

        div()
            .size_full()
            .px_8()
            .py_6()
            .child(div().size_full().max_w(px(760.)).mx_auto().child(view))
            .into_any_element()
    }

    fn render_attachment_view(
        &self,
        rel_path: &str,
        kind: AttachmentKind,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme();
        let Some(root) = &self.vault_root else {
            return div().into_any_element();
        };
        let embed = resolve_embed(root, rel_path, self.active_tab_path(), &self.index);
        let disk_path = resolve_vault_path(root, rel_path).ok();

        match (kind, embed, disk_path) {
            (
                AttachmentKind::Image,
                Some(EmbeddedFile {
                    width, height, ..
                }),
                Some(absolute_path),
            ) => v_flex()
                .size_full()
                .items_center()
                .justify_center()
                .gap_3()
                .p_6()
                .child(
                    gpui_kit::img(absolute_path)
                        .max_w_full()
                        .max_h(px(640.))
                        .rounded(px(8.)),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(format!(
                            "{rel_path}{}",
                            match (width, height) {
                                (Some(w), Some(h)) => format!(" ({w} × {h})"),
                                _ => String::new(),
                            }
                        )),
                )
                .into_any_element(),
            (AttachmentKind::Pdf, _, Some(absolute_path)) => v_flex()
                .size_full()
                .items_center()
                .justify_center()
                .gap_3()
                .p_6()
                .child(div().text_lg().font_semibold().child(rel_path.to_string()))
                .child(
                    Button::new("open-pdf-preview")
                        .label("Open PDF in Preview")
                        .primary()
                        .on_click(move |_, _, cx| {
                            cx.open_url(&format!("file://{}", absolute_path.display()));
                        }),
                )
                .into_any_element(),
            _ => v_flex()
                .size_full()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .text_sm()
                        .text_color(theme.muted_foreground)
                        .child(rel_path.to_string()),
                )
                .into_any_element(),
        }
    }

    fn render_empty_tab_state(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .gap_3()
            .child(
                div()
                    .text_base()
                    .text_color(theme.muted_foreground)
                    .child("No note open"),
            )
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("empty-new-note")
                            .label("New Note (⌘N)")
                            .primary()
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.open_name_prompt(
                                    PromptKind::NewNote,
                                    String::new(),
                                    None,
                                    window,
                                    cx,
                                );
                            })),
                    )
                    .child(
                        Button::new("empty-quick-open")
                            .label("Quick Open (⌘O)")
                            .outline()
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.open_quick_switcher(false, window, cx);
                            })),
                    ),
            )
    }

    fn render_welcome_screen(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let demo_path = find_demo_vault();
        v_flex()
            .size_full()
            .bg(theme.background)
            .items_center()
            .justify_center()
            .gap_4()
            .child(
                div()
                    .text_2xl()
                    .font_bold()
                    .child("Noteki (GPUI Prototype)"),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child("Open a local Markdown vault kept with Obsidian."),
            )
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("welcome-open-vault")
                            .label("Open Vault Folder…")
                            .icon(Icon::new(AssetIcon::FolderOpen))
                            .primary()
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.open_vault_picker(window, cx);
                            })),
                    )
                    .when_some(demo_path, |row, demo| {
                        row.child(
                            Button::new("welcome-open-demo")
                                .label("Open Demo Vault")
                                .outline()
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.load_vault(demo.clone(), None, window, cx);
                                })),
                        )
                    }),
            )
    }

    fn navigate_back(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(idx) = self.active_tab_idx else {
            return;
        };
        let Some(prev) = self.tabs.get_mut(idx).and_then(|t| t.back_stack.pop()) else {
            return;
        };
        let cur = self.tabs[idx].path.clone();
        self.tabs[idx].forward_stack.push(cur);
        self.open_file_in_tab(&prev, false, false, None, window, cx);
    }

    fn navigate_forward(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(idx) = self.active_tab_idx else {
            return;
        };
        let Some(next) = self.tabs.get_mut(idx).and_then(|t| t.forward_stack.pop()) else {
            return;
        };
        let cur = self.tabs[idx].path.clone();
        self.tabs[idx].back_stack.push(cur);
        self.open_file_in_tab(&next, false, false, None, window, cx);
    }
}

impl Render for NotekiApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut split = h_resizable("noteki-workspace");
        if self.left_sidebar_open {
            split = split.child(
                resizable_panel()
                    .size(px(260.))
                    .size_range(px(180.)..px(460.))
                    .child(self.render_left_sidebar(cx)),
            );
        }

        split = split.child(
            resizable_panel()
                .size_range(px(360.)..px(4000.))
                .child(self.render_center_pane(cx)),
        );

        if self.right_sidebar_open {
            split = split.child(
                resizable_panel()
                    .size(px(260.))
                    .size_range(px(180.)..px(460.))
                    .child(self.render_right_sidebar(cx)),
            );
        }

        div()
            .track_focus(&self.focus_handle)
            .size_full()
            .flex()
            .flex_col()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .on_action(cx.listener(|this, _: &ToggleLeftSidebar, _, cx| {
                this.left_sidebar_open = !this.left_sidebar_open;
                this.persist_settings();
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &ToggleRightSidebar, _, cx| {
                this.right_sidebar_open = !this.right_sidebar_open;
                this.persist_settings();
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &SwitchFilesPane, _, cx| {
                this.left_sidebar_open = true;
                this.left_pane = LeftPane::Files;
                this.persist_settings();
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &SwitchTagsPane, _, cx| {
                this.left_sidebar_open = true;
                this.left_pane = LeftPane::Tags;
                this.persist_settings();
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &SwitchSearchPane, window, cx| {
                this.left_sidebar_open = true;
                this.left_pane = LeftPane::Search;
                this.search_input.update(cx, |s, cx| s.focus(window, cx));
                this.persist_settings();
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &ToggleViewMode, _, cx| {
                this.view_mode = this.view_mode.next();
                this.refresh_reader_cache();
                this.persist_settings();
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &OpenQuickSwitcher, window, cx| {
                this.open_quick_switcher(false, window, cx);
            }))
            .on_action(cx.listener(|this, _: &OpenQuickSwitcherNewTab, window, cx| {
                this.open_quick_switcher(true, window, cx);
            }))
            .on_action(cx.listener(|this, _: &OpenCommandPalette, window, cx| {
                this.open_command_palette(window, cx);
            }))
            .on_action(cx.listener(|this, _: &NewNoteAction, window, cx| {
                this.open_name_prompt(PromptKind::NewNote, String::new(), None, window, cx);
            }))
            .on_action(cx.listener(|this, _: &NewFolderAction, window, cx| {
                this.open_name_prompt(PromptKind::NewFolder, String::new(), None, window, cx);
            }))
            .on_action(cx.listener(|this, _: &DailyNoteAction, window, cx| {
                this.create_daily_note(window, cx);
            }))
            .on_action(cx.listener(|this, _: &CloseTabAction, window, cx| {
                if let Some(ix) = this.active_tab_idx {
                    this.close_tab(ix, window, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &ReopenClosedTabAction, window, cx| {
                if let Some(last) = this.closed_tabs.pop() {
                    this.open_file_in_tab(&last, true, true, None, window, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &NextTabAction, window, cx| {
                if !this.tabs.is_empty() {
                    let next = (this.active_tab_idx.unwrap_or(0) + 1) % this.tabs.len();
                    this.save_active_cursor(cx);
                    this.activate_tab(next, window, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &PrevTabAction, window, cx| {
                if !this.tabs.is_empty() {
                    let cur = this.active_tab_idx.unwrap_or(0);
                    let prev = if cur == 0 { this.tabs.len() - 1 } else { cur - 1 };
                    this.save_active_cursor(cx);
                    this.activate_tab(prev, window, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &NavigateBackAction, window, cx| {
                this.navigate_back(window, cx);
            }))
            .on_action(cx.listener(|this, _: &NavigateForwardAction, window, cx| {
                this.navigate_forward(window, cx);
            }))
            .on_action(cx.listener(|this, _: &OpenVaultAction, window, cx| {
                this.open_vault_picker(window, cx);
            }))
            .on_action(cx.listener(|this, _: &CycleThemeAction, window, cx| {
                this.cycle_theme(window, cx);
            }))
            .on_action(cx.listener(|this, _: &CycleAppearanceAction, window, cx| {
                this.cycle_appearance(window, cx);
            }))
            .on_action(cx.listener(|this, _: &FormatBoldAction, window, cx| {
                this.wrap_selection_inline("**", "**", window, cx);
            }))
            .on_action(cx.listener(|this, _: &FormatItalicAction, window, cx| {
                this.wrap_selection_inline("*", "*", window, cx);
            }))
            .on_action(cx.listener(|this, _: &FormatStrikeAction, window, cx| {
                this.wrap_selection_inline("~~", "~~", window, cx);
            }))
            .on_action(cx.listener(|this, _: &FormatCodeAction, window, cx| {
                this.wrap_selection_inline("`", "`", window, cx);
            }))
            .on_action(cx.listener(|this, _: &InsertWikilinkAction, window, cx| {
                this.open_wikilink_picker(window, cx);
            }))
            .on_action(cx.listener(|this, _: &FollowLinkUnderCursorAction, window, cx| {
                this.follow_wikilink_under_cursor(window, cx);
            }))
            .child(self.render_title_bar(cx))
            .child(div().flex_1().min_h_0().child(split))
    }
}

fn find_demo_vault() -> Option<PathBuf> {
    let candidates = [
        PathBuf::from("docs/demo-vault"),
        PathBuf::from("../docs/demo-vault"),
    ];
    candidates.into_iter().find(|p| p.is_dir())
}

fn collect_notes_from_entries(entries: &[VaultItem], out: &mut Vec<(String, String)>) {
    for entry in entries {
        match entry {
            VaultItem::Note { name, path } => out.push((name.clone(), path.clone())),
            VaultItem::Folder { children, .. } => collect_notes_from_entries(children, out),
            VaultItem::Attachment { .. } => {}
        }
    }
}

fn attachment_kind_for(rel_path: &str, entries: &[VaultItem]) -> Option<AttachmentKind> {
    for entry in entries {
        match entry {
            VaultItem::Attachment {
                path,
                attachment_kind,
                ..
            } if path == rel_path => return Some(*attachment_kind),
            VaultItem::Folder { children, .. } => {
                if let Some(found) = attachment_kind_for(rel_path, children) {
                    return Some(found);
                }
            }
            _ => {}
        }
    }
    None
}

fn is_folder_in_scan(rel_path: &str, entries: &[VaultItem]) -> bool {
    for entry in entries {
        if let VaultItem::Folder { path, children, .. } = entry {
            if path == rel_path || is_folder_in_scan(rel_path, children) {
                return true;
            }
        }
    }
    false
}

fn build_tree_items(entries: &[VaultItem], expanded: &HashSet<String>) -> Vec<TreeItem> {
    let mut out = Vec::new();
    for entry in entries {
        match entry {
            VaultItem::Folder { name, path, children } => {
                let child_items = build_tree_items(children, expanded);
                let item = TreeItem::new(path.clone(), name.clone())
                    .expanded(expanded.contains(path))
                    .children(child_items);
                out.push(item);
            }
            VaultItem::Note { name, path } => {
                out.push(TreeItem::new(path.clone(), name.clone()));
            }
            VaultItem::Attachment { name, path, .. } => {
                out.push(TreeItem::new(path.clone(), name.clone()));
            }
        }
    }
    out
}

fn flatten_tags<'a>(nodes: &'a [TagNode], depth: usize, out: &mut Vec<(usize, &'a TagNode)>) {
    for node in nodes {
        out.push((depth, node));
        flatten_tags(&node.children, depth + 1, out);
    }
}

fn find_tag_notes(nodes: &[TagNode], target_lower: &str) -> Option<Vec<String>> {
    for node in nodes {
        if node.full_lower == target_lower {
            return Some(node.note_paths.clone());
        }
        if let Some(found) = find_tag_notes(&node.children, target_lower) {
            return Some(found);
        }
    }
    None
}

fn toggle_checkbox_near_offset(text: &str, approx_offset: usize, checked: bool) -> Option<String> {
    let clamped = approx_offset.min(text.len());
    let line_start = text[..clamped].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let line_end = text[line_start..]
        .find('\n')
        .map(|i| line_start + i)
        .unwrap_or(text.len());
    let line = &text[line_start..line_end];

    let (old_box, new_box) = if checked {
        ("[ ]", "[x]")
    } else {
        ("[x]", "[ ]")
    };
    let pos = line.find(old_box).or_else(|| {
        if !checked {
            line.find("[X]")
        } else {
            None
        }
    })?;

    let abs_pos = line_start + pos;
    let mut out = String::with_capacity(text.len());
    out.push_str(&text[..abs_pos]);
    out.push_str(new_box);
    out.push_str(&text[abs_pos + 3..]);
    Some(out)
}
