//! The Noteki window: state and behaviour. Drawing lives in `render.rs`.
//!
//! The rules (saving, merging, tabs, links) live in `crate::core` and are
//! unit-tested there; this module wires them to GPUI and the file system.

mod format_bar;
mod loose;
pub mod menus;
pub mod print;
mod render;
pub mod settings;
mod settings_window;
mod swift_prefs;
pub mod terminal;
pub mod updates;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

use notify::{RecommendedWatcher, RecursiveMode, Watcher};

use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::tree::{TreeEvent, TreeItem, TreeState};
use gpui_kit::{
    actions, App, AppContext as _, Context, Entity, FocusHandle, Focusable, ScrollStrategy,
    SharedString, Subscription, Task, UniformListScrollHandle, Window,
};

use crate::core::attachment;
use crate::core::document::{ExternalOutcome, NoteDoc};
use crate::core::editor::lines::logical_lines;
use crate::core::index::{
    build_tag_tree, extract_outline, find_backlinks, search_vault, BacklinkMatch, Corpus,
    HeadingItem, SearchResult, TagNode,
};
use crate::core::markdown::{self, Link};
use crate::core::opening::{route, Opened};
use crate::core::tabs::{rename_target, OpenIn, Tabs};
use crate::core::vault::{
    create_empty_note, create_folder, make_copy, move_to_trash, new_note_path_for_link,
    normalize_excluded, read_note, rename_item, scan_vault, today_daily_note_path,
    validate_item_name, write_note_atomic, AttachmentKind, VaultItem,
};
use crate::core::wikilink::{folder_of, note_name, ResolvedItem, VaultIndex};
use crate::editor::{Format, LineFormat, LinkContext, NoteEditor, NoteEditorEvent, Wrap};
use crate::theme::{apply_noteki_theme, AppearanceMode, NotekiPalette, NotekiThemeName};

pub use menus::{app_menus, set_app_menus};
pub use settings::{LeftPane, RightPane, Settings};

actions!(
    noteki,
    [
        ToggleLeftSidebar,
        ToggleRightSidebar,
        SwitchFilesPane,
        SwitchTagsPane,
        SwitchSearchPane,
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
        ZoomIn,
        ZoomOut,
        ZoomReset,
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
        OpenSettingsAction,
        ExportHtmlAction,
        NewWindowAction,
        ShowOutlineAction,
        ShowBacklinksAction,
        SearchInVaultAction,
        TrashSelectedAction,
        KeyboardShortcutsAction,
        CheckForUpdatesAction,
        SupportNotekiAction,
        OpenVaultInTerminalAction,
        ToggleFocusMode,
        ToggleTypewriterMode,
        ExportPdfAction,
        PrintAction,
        ToggleCheckSpelling,
        ToggleCorrectSpelling,
        ToggleFormatBar,
        FormatQuoteAction,
        FormatBulletAction,
        FormatNumberedAction,
    ]
);

/// The app's id: its windows' on Linux, matched with
/// `linux/app.noteki.noteki.desktop`; the Swift app's bundle id.
pub const APP_ID: &str = "app.noteki.noteki";

/// Help › Support Noteki…: an optional payment; the app stays the same without it.
pub const SUPPORT_URL: &str = "https://buy.stripe.com/6oU5kCeuudcFeoIeac6wE05";

pub fn register_keybindings(cx: &mut App) {
    cx.bind_keys(
        [
            crate::keys::binding("ctrl-cmd-s", ToggleLeftSidebar, None),
            crate::keys::binding("alt-cmd-0", ToggleRightSidebar, None),
            crate::keys::binding("ctrl-cmd-1", SwitchFilesPane, None),
            crate::keys::binding("ctrl-cmd-2", SwitchTagsPane, None),
            crate::keys::binding("ctrl-cmd-3", SwitchSearchPane, None),
            crate::keys::binding("alt-cmd-1", ShowOutlineAction, None),
            crate::keys::binding("alt-cmd-2", ShowBacklinksAction, None),
            // Its own action: the menu shows ⇧⌘F for it, ⌃⌘3 for View › Search.
            crate::keys::binding("cmd-shift-f", SearchInVaultAction, None),
            crate::keys::binding("cmd-o", OpenQuickSwitcher, None),
            crate::keys::binding("cmd-t", OpenQuickSwitcherNewTab, None),
            crate::keys::binding("cmd-shift-p", OpenCommandPalette, None),
            crate::keys::binding("cmd-n", NewNoteAction, None),
            crate::keys::binding("cmd-shift-n", NewWindowAction, None),
            crate::keys::binding("cmd-shift-d", DailyNoteAction, None),
            crate::keys::binding("cmd-w", CloseTabAction, None),
            crate::keys::binding("cmd-shift-t", ReopenClosedTabAction, None),
            crate::keys::binding("ctrl-tab", NextTabAction, None),
            // macOS gives ⇧⌘] as ⌘}: a `shift-]` binding is never pressed.
            crate::keys::binding("cmd-}", NextTabAction, None),
            crate::keys::binding("ctrl-shift-tab", PrevTabAction, None),
            crate::keys::binding("cmd-{", PrevTabAction, None),
            // The first is the one the menu shows: ⌘+, as the Swift app's.
            crate::keys::binding("cmd-+", ZoomIn, None),
            crate::keys::binding("cmd-=", ZoomIn, None),
            crate::keys::binding("cmd--", ZoomOut, None),
            crate::keys::binding("cmd-0", ZoomReset, None),
            crate::keys::binding("cmd-alt-left", NavigateBackAction, None),
            crate::keys::binding("cmd-alt-right", NavigateForwardAction, None),
            crate::keys::binding("cmd-shift-o", OpenVaultAction, None),
            crate::keys::binding("cmd-b", FormatBoldAction, None),
            crate::keys::binding("cmd-i", FormatItalicAction, None),
            crate::keys::binding("cmd-shift-x", FormatStrikeAction, None),
            crate::keys::binding("ctrl-`", FormatCodeAction, None),
            crate::keys::binding("cmd-shift-k", InsertWikilinkAction, None),
            crate::keys::binding("cmd-,", OpenSettingsAction, None),
            crate::keys::binding("cmd-p", PrintAction, None),
            // ⇧⌘/, which macOS sends as ⌘?.
            crate::keys::binding("cmd-?", KeyboardShortcutsAction, None),
            // As in the Finder, only while Files has the keyboard: in the editor ⌘⌫
            // deletes to the start of the line.
            // Delete there, as in Linux's file managers.
            Some(crate::keys::binding_os(
                "cmd-backspace",
                "delete",
                TrashSelectedAction,
                Some(gpui_kit::base::tree_key_context()),
            )),
        ]
        .into_iter()
        .flatten(),
    );
    crate::editor::register_keybindings(cx);
}

/// Every Noteki window open, so a setting changed in one reaches the others.
#[derive(Default)]
struct OpenWindows(Vec<gpui_kit::WeakEntity<NotekiApp>>);
impl gpui_kit::Global for OpenWindows {}

/// Opens a window of Noteki with `config`.
pub fn open_noteki_window(
    config: AppConfig,
    cx: &mut App,
) -> Option<(gpui_kit::AnyWindowHandle, Entity<NotekiApp>)> {
    use gpui_kit::{px, size, Bounds, WindowBounds, WindowOptions};
    let bounds = Bounds::centered(None, size(px(1280.), px(820.)), cx);
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: Some(gpui_kit::component::TitleBar::title_bar_options()),
        window_min_size: Some(size(px(640.), px(420.))),
        app_id: Some(APP_ID.into()),
        ..Default::default()
    };
    gpui_kit::open_window(options, cx, |window, cx| {
        cx.new(|cx| NotekiApp::new(config, window, cx))
    })
    .ok()
}

/// SPEC › Saving: about one second after the last change.
pub const AUTOSAVE_DELAY: Duration = Duration::from_millis(1000);
/// How long typing stops before the window around the editor is drawn again.
const TYPING_PAUSE: Duration = Duration::from_millis(150);
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
    /// Where "Move to Trash" moves files; `None` is the system's Trash.
    pub trash_dir: Option<PathBuf>,
    /// Which window of the Vault this is: 0 the first, then ⇧⌘N's.
    pub slot: usize,
}

impl AppConfig {
    /// The configuration of the real app: user preferences, file watching, and
    /// the demo vault when nothing was opened before.
    pub fn for_user() -> Self {
        let settings_path = settings::default_settings_path();
        // The first launch after the Swift app: its preferences, kept as ours.
        if let Some(path) = settings_path.as_deref().filter(|p| !p.exists()) {
            if let Some(imported) = swift_prefs::import() {
                let _ = settings::save(Some(path), &imported);
            }
        }
        Self {
            settings_path,
            vault: None,
            watch: true,
            trash_dir: None,
            slot: 0,
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
        Self::load_excluding(root, &[])
    }

    /// Loads the Vault leaving out `excluded` too (paths from its root, any case).
    pub fn load_excluding(root: &Path, excluded: &[String]) -> Result<Self, String> {
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
            excluded: normalize_excluded(excluded),
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

fn all_folders(items: &[VaultItem], out: &mut HashSet<String>) {
    for item in items {
        if let VaultItem::Folder { path, children, .. } = item {
            out.insert(path.clone());
            all_folders(children, out);
        }
    }
}

fn empty_folders(items: &[VaultItem], out: &mut HashSet<String>) {
    for item in items {
        if let VaultItem::Folder { path, children, .. } = item {
            if children.is_empty() {
                out.insert(path.clone());
            }
            empty_folders(children, out);
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
    /// The folders with nothing shown in them: the tree takes an item without
    /// children for a file, so they are told apart by their paths.
    empty_folders: std::rc::Rc<HashSet<String>>,
    expanded_folders: HashSet<String>,
    selected_tag: Option<String>,
    search_input: Entity<InputState>,
    search_results: Vec<SearchResult>,
    search_scroll: UniformListScrollHandle,
    /// The Outline's and Backlinks' lists, only their rows in view drawn.
    inspector_scroll: UniformListScrollHandle,

    editor: Entity<NoteEditor>,

    outline: Vec<HeadingItem>,
    backlinks: Vec<BacklinkMatch>,

    watcher: Option<RecommendedWatcher>,
    fs_events: Option<mpsc::Receiver<PathBuf>>,
    pending_fs: HashSet<PathBuf>,
    last_fs_event: Option<Instant>,
    autosave: Option<Task<()>>,
    /// The window drawn again once typing pauses (`after_text_change`).
    typing_redraw: Option<Task<()>>,
    settings_window: Option<gpui_kit::AnyWindowHandle>,
    /// This view's window.
    window: gpui_kit::AnyWindowHandle,

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
        crate::theme::set_interface_size(settings.interface_size.rem(), cx);

        let font_size = gpui_kit::px(settings.font_size);
        let editor = cx.new(|cx| {
            let mut ed = NoteEditor::new(palette, window, cx);
            ed.set_font_size(font_size, cx);
            ed.set_text_settings(
                &settings.font_family,
                &settings.code_font,
                settings.line_limit(),
                cx,
            );
            ed.set_modes(settings.focus_mode, settings.typewriter_mode, cx);
            ed.set_spelling(settings.check_spelling, settings.correct_spelling, cx);
            ed
        });
        menus::refresh(&settings, cx);
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
            empty_folders: Default::default(),
            expanded_folders: HashSet::new(),
            selected_tag: None,
            search_input,
            search_results: Vec::new(),
            search_scroll: UniformListScrollHandle::new(),
            inspector_scroll: UniformListScrollHandle::new(),
            editor,
            outline: Vec::new(),
            backlinks: Vec::new(),
            watcher: None,
            fs_events: None,
            pending_fs: HashSet::new(),
            last_fs_event: None,
            autosave: None,
            typing_redraw: None,
            settings_window: None,
            window: window.window_handle(),
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
        let me = cx.entity().downgrade();
        cx.default_global::<OpenWindows>().0.push(me);
        if app.config.slot > 0 {
            // Closed by the user, it does not come back at the next launch.
            let this = cx.entity().downgrade();
            window.on_window_should_close(cx, move |_, cx| {
                let _ = this.update(cx, |app, _| app.forget_session());
                true
            });
        }
        if let Some(root) = initial {
            let _ = app.open_vault(&root, window, cx);
        }
        // Something has to hold the keyboard, or the window's shortcuts reach
        // nothing until a click: the Note in front, else the window.
        if app.editor_path.is_some() {
            app.editor.update(cx, |ed, cx| ed.focus(window, cx));
        } else {
            window.focus(&app.focus_handle, cx);
        }
        app.start_fs_poller(window, cx);
        app
    }

    /// The other Noteki windows open.
    fn other_windows(&self, cx: &Context<Self>) -> Vec<Entity<NotekiApp>> {
        let me = cx.entity_id();
        cx.try_global::<OpenWindows>()
            .map(|w| w.0.iter().filter_map(|a| a.upgrade()).collect::<Vec<_>>())
            .unwrap_or_default()
            .into_iter()
            .filter(|a| a.entity_id() != me)
            .filter(|a| cx.windows().contains(&a.read(cx).window)) // not closed
            .collect()
    }

    /// *Export as HTML…*: the Note in front as one page standing alone,
    /// saved where the user says (never in the Vault unasked).
    pub fn export_html(&mut self, cx: &mut Context<Self>) {
        let Some((html, title, root)) = self.export_page() else {
            return;
        };
        let dir = dirs::download_dir().unwrap_or(root);
        let rx = cx.prompt_for_new_path(&dir, Some(&format!("{title}.html")));
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(target))) = rx.await else {
                return;
            };
            let result = std::fs::write(&target, html);
            let _ = this.update(cx, |app, cx| {
                match result {
                    Ok(()) => app.set_info(format!("Exported {}", target.display())),
                    Err(e) => app.set_error(format!("Could not export: {e}")),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// The page of the Note in front, standing alone (SPEC › Export), its
    /// title and the Vault's root.
    fn export_page(&self) -> Option<(String, String, PathBuf)> {
        let (path, vault) = (self.editor_path.clone()?, self.vault.as_ref()?);
        let doc = self.docs.get(&path)?;
        let root = vault.root.clone();
        let folder = root.join(folder_of(&path));
        let title = note_name(&path).to_string();
        let html =
            crate::core::export::note_html(doc.text(), &title, Some(&path), &vault.index, |src| {
                let mime = crate::core::export::image_mime(src)?;
                // A path of the Vault, or one relative to the Note.
                let file = [root.join(src), folder.join(src)]
                    .into_iter()
                    .find(|f| f.starts_with(&root) && f.is_file())?;
                Some((std::fs::read(file).ok()?, mime))
            });
        Some((html, title, root))
    }

    /// *Export as PDF…*: the page paged as a print is, on the Mac's paper.
    pub fn export_pdf(&mut self, cx: &mut Context<Self>) {
        let Some((html, title, root)) = self.export_page() else {
            return;
        };
        let dir = dirs::download_dir().unwrap_or(root);
        let rx = cx.prompt_for_new_path(&dir, Some(&format!("{title}.pdf")));
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(target))) = rx.await else {
                return;
            };
            let (tx, done) = async_channel::bounded(1);
            let shown = target.clone();
            let _ = this.update(cx, move |_, _| {
                print::run(&html, &title, print::Job::Pdf(target), move |r| {
                    let _ = tx.try_send(r);
                })
            });
            let result = done.recv().await.unwrap_or(Err("cancelled".into()));
            let _ = this.update(cx, |app, cx| {
                match result {
                    Ok(()) => app.set_info(format!("Exported {}", shown.display())),
                    Err(e) => app.set_error(format!("Could not export: {e}")),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// *Print…* (⌘P): the page in the print panel.
    pub fn print_note(&mut self, cx: &mut Context<Self>) {
        let Some((html, title, _)) = self.export_page() else {
            return;
        };
        let app = cx.entity().downgrade();
        let mut cx_async = cx.to_async();
        print::run(&html, &title, print::Job::Print, move |r| {
            if let Err(e) = r {
                let _ = app.update(&mut cx_async, |app, cx| {
                    app.set_error(format!("Could not print: {e}"));
                    cx.notify();
                });
            }
        });
    }

    /// ⇧⌘N: another window of this Vault, with its own Tabs (ADR 0010).
    pub fn open_another_window(&mut self, cx: &mut Context<Self>) {
        let Some(root) = self.vault.as_ref().map(|v| v.root.clone()) else {
            return;
        };
        let taken: HashSet<usize> = self
            .other_windows(cx)
            .iter()
            .map(|a| a.read(cx).config.slot)
            .chain([self.config.slot])
            .collect();
        let slot = (1..).find(|n| !taken.contains(n)).unwrap_or(1);
        self.open_window_in_slot(&root, slot, cx);
    }

    fn open_window_in_slot(&mut self, root: &Path, slot: usize, cx: &mut Context<Self>) {
        let config = AppConfig {
            vault: Some(root.to_path_buf()),
            slot,
            ..self.config.clone()
        };
        // Opened once this update is over: the new window reads the settings file.
        cx.defer(move |cx| {
            open_noteki_window(config, cx);
        });
    }

    /// Forgets this window's session (closed by the user).
    fn forget_session(&mut self) {
        if let Some(root) = self
            .vault
            .as_ref()
            .map(|v| v.root.to_string_lossy().to_string())
        {
            self.settings
                .sessions
                .remove(&settings::session_key(&root, self.config.slot));
            self.persist();
        }
    }

    /// The key of this window's session in the settings.
    fn session_key(&self, root: &Path) -> String {
        settings::session_key(&root.to_string_lossy(), self.config.slot)
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

    pub fn status(&self) -> Option<&StatusMessage> {
        self.status.as_ref()
    }

    /// The path selected in Files.
    pub fn selected_in_files(&self, cx: &App) -> Option<String> {
        let tree = self.file_tree.read(cx);
        tree.selected_item().map(|i| i.id.to_string())
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
        // Other windows write the same file: their sessions are kept as they are.
        let path = self.config.settings_path.as_deref();
        let out = match (path, &self.vault) {
            (Some(p), Some(v)) if p.exists() => {
                let key = self.session_key(&v.root);
                self.settings.merged_for_save(settings::load(Some(p)), &key)
            }
            _ => self.settings.clone(),
        };
        if let Err(e) = settings::save(path, &out) {
            self.set_error(format!("Could not save preferences: {e}"));
        }
    }

    fn persist_session(&mut self) {
        let Some(vault) = &self.vault else { return };
        let key = self.session_key(&vault.root);
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

    pub(crate) fn toggle_left_sidebar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.settings.left_sidebar_open = !self.settings.left_sidebar_open;
        self.keep_keyboard_off_search(window, cx);
        self.persist();
        cx.notify();
    }

    /// The search field leaving the window takes the keyboard with it, and the
    /// window's shortcuts would reach nothing: the window keeps it instead.
    fn keep_keyboard_off_search(&self, window: &mut Window, cx: &mut Context<Self>) {
        let shown = self.settings.left_sidebar_open && self.settings.left_pane == LeftPane::Search;
        if !shown
            && self
                .search_input
                .focus_handle(cx)
                .contains_focused(window, cx)
        {
            window.focus(&self.focus_handle, cx);
        }
    }

    pub(crate) fn toggle_right_sidebar(&mut self, cx: &mut Context<Self>) {
        self.settings.right_sidebar_open = !self.settings.right_sidebar_open;
        self.persist();
        cx.notify();
    }

    /// Search shown, with the keyboard in its field.
    pub(crate) fn show_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.show_left_pane(LeftPane::Search, window, cx);
        self.search_input.update(cx, |s, cx| s.focus(window, cx));
    }

    pub(crate) fn show_left_pane(
        &mut self,
        pane: LeftPane,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.settings.left_sidebar_open = true;
        self.settings.left_pane = pane;
        self.keep_keyboard_off_search(window, cx);
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
        crate::theme::set_interface_size(self.settings.interface_size.rem(), cx);
        let palette = self.palette;
        self.editor.update(cx, |ed, cx| ed.set_palette(palette, cx));
        cx.notify();
    }

    /// ⌘,: the Settings window, brought forward when already open.
    pub fn open_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(handle) = self.settings_window {
            if handle.update(cx, |_, w, _| w.activate_window()).is_ok() {
                return;
            }
        }
        // Opened once this update is over: the new window reads the settings.
        let (app, main) = (cx.entity(), window.window_handle());
        cx.defer(move |cx| {
            let handle = settings_window::SettingsWindow::open(app.clone(), main, cx);
            app.update(cx, |a, _| a.settings_window = handle);
        });
    }

    /// Changes the settings (from the Settings window), saves and applies them
    /// at once: the theme, the editor's text, the Vault's Excluded folders.
    pub fn update_settings(
        &mut self,
        change: impl FnOnce(&mut Settings),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let before = self.settings.clone();
        change(&mut self.settings);
        self.apply_settings(&before, window, cx);
        self.persist(); // the settings are remembered
                        // The other windows take them too.
        let shared = self.settings.clone();
        for other in self.other_windows(cx) {
            let (handle, shared) = (other.read(cx).window, shared.clone());
            cx.defer(move |cx| {
                let _ = handle.update(cx, |_, window, cx| {
                    other.update(cx, |o, cx| {
                        let before = o.settings.clone();
                        o.settings.adopt_shared(&shared);
                        o.apply_settings(&before, window, cx);
                    })
                });
            });
        }
    }

    /// Applies what changed since `before`: the theme, the editor's text, the
    /// Vault's Excluded folders.
    fn apply_settings(&mut self, before: &Settings, window: &mut Window, cx: &mut Context<Self>) {
        let look = (self.settings.theme_name, self.settings.appearance);
        if look != (before.theme_name, before.appearance) {
            self.apply_theme(window, cx);
        } else if self.settings.interface_size != before.interface_size {
            crate::theme::set_interface_size(self.settings.interface_size.rem(), cx);
        }
        let s = &self.settings;
        let (family, code, limit) = (s.font_family.clone(), s.code_font.clone(), s.line_limit());
        let size = gpui_kit::px(
            s.font_size
                .clamp(settings::MIN_FONT_SIZE, settings::MAX_FONT_SIZE),
        );
        let modes = (s.focus_mode, s.typewriter_mode);
        let spelling = (s.check_spelling, s.correct_spelling);
        self.editor.update(cx, |ed, cx| {
            ed.set_font_size(size, cx);
            ed.set_text_settings(&family, &code, limit, cx);
            ed.set_modes(modes.0, modes.1, cx);
            ed.set_spelling(spelling.0, spelling.1, cx);
        });
        if modes != (before.focus_mode, before.typewriter_mode)
            || spelling != (before.check_spelling, before.correct_spelling)
            || s.show_format_bar != before.show_format_bar
        {
            menus::refresh(&self.settings, cx);
        }
        if let Some(root) = self.vault.as_ref().map(|v| v.root.clone()) {
            let excluded = self.settings.excluded_in(&root).to_vec();
            if excluded != before.excluded_in(&root) {
                self.exclude_folders(&excluded, window, cx);
            }
        }
        cx.notify();
    }

    /// Leaves `excluded` out of the Vault: the Tabs inside close, their Notes
    /// saved first, since no change on disk would reach them any more.
    fn exclude_folders(
        &mut self,
        excluded: &[String],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(vault) = self.vault.as_mut() else {
            return;
        };
        vault.excluded = normalize_excluded(excluded);
        let root = vault.root.clone();
        let set = vault.excluded.clone();
        let inside = |app: &Self| {
            app.tabs
                .iter()
                .position(|t| crate::core::vault::is_in_excluded_folder(&root, &t.path, &set))
        };
        while let Some(ix) = inside(self) {
            self.close_tab(ix, window, cx);
        }
        self.refresh_vault(&HashSet::new(), cx);
        self.refresh_outline();
        self.persist_session();
    }

    /// ⌘+ (`step` 1), ⌘− (−1), ⌘0 (`None`): the text's size, remembered.
    pub(crate) fn zoom(&mut self, step: Option<f32>, window: &mut Window, cx: &mut Context<Self>) {
        let size = match step {
            Some(step) => self.settings.zoomed(step),
            None => settings::DEFAULT_FONT_SIZE,
        };
        self.update_settings(|s| s.font_size = size, window, cx);
    }

    /// Edit › Spelling and Grammar › Check Spelling While Typing.
    pub(crate) fn toggle_check_spelling(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.update_settings(|s| s.check_spelling = !s.check_spelling, window, cx);
    }

    /// Edit › Spelling and Grammar › Correct Spelling Automatically.
    pub(crate) fn toggle_correct_spelling(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.update_settings(|s| s.correct_spelling = !s.correct_spelling, window, cx);
    }

    /// View › Focus Mode, for every window.
    pub(crate) fn toggle_focus_mode(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.update_settings(|s| s.focus_mode = !s.focus_mode, window, cx);
    }

    /// View › Typewriter Mode, for every window.
    pub(crate) fn toggle_typewriter_mode(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.update_settings(|s| s.typewriter_mode = !s.typewriter_mode, window, cx);
    }

    pub(crate) fn cycle_theme(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.update_settings(|s| s.theme_name = s.theme_name.next(), window, cx);
    }

    pub(crate) fn cycle_appearance(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.update_settings(|s| s.appearance = s.appearance.next(), window, cx);
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
        let canonical = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
        let excluded = self.settings.excluded_in(&canonical).to_vec();
        let vault = match Vault::load_excluding(root, &excluded) {
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
            .get(&self.session_key(&vault.root))
            .cloned()
            .unwrap_or_default();
        // Another Vault: the other windows close (SPEC › Window).
        let switching = self.vault.as_ref().is_some_and(|v| v.root != vault.root);
        if switching {
            for other in self.other_windows(cx) {
                let handle = other.read(cx).window;
                other.update(cx, |o, _| {
                    o.flush_all();
                    o.forget_session();
                });
                cx.defer(move |cx| {
                    let _ = handle.update(cx, |_, window, _| window.remove_window());
                });
            }
        }

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
        if self.config.slot == 0 {
            // The Vault's other windows, as they were left.
            let open: HashSet<usize> = self
                .other_windows(cx)
                .iter()
                .map(|a| a.read(cx).config.slot)
                .collect();
            for slot in self.settings.other_slots(&root.to_string_lossy()) {
                if !open.contains(&slot) {
                    self.open_window_in_slot(&root, slot, cx);
                }
            }
        }

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
        self.update_link_context(cx);
    }

    /// Saves an Attachment next to the Note in front (under a free name) and
    /// embeds it at the cursor. Its path in the Vault.
    fn attach_bytes(
        &mut self,
        name: &str,
        bytes: &[u8],
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let (Some(vault), Some(note)) = (&self.vault, &self.editor_path) else {
            return Err("No Note to paste the image into".into());
        };
        let folder = vault.root.join(folder_of(note));
        let path = attachment::free_path(&folder, name, |p| p.exists());
        std::fs::write(&path, bytes).map_err(|e| format!("Could not save the image: {e}"))?;
        self.embed_attachments(&[path], cx);
        Ok(())
    }

    /// Copies dropped image files next to the Note in front and embeds them
    /// where they were dropped; other files are left alone.
    fn attach_files(&mut self, paths: &[PathBuf], cx: &mut Context<Self>) -> Result<(), String> {
        let (Some(vault), Some(note)) = (&self.vault, &self.editor_path) else {
            return Ok(());
        };
        let folder = vault.root.join(folder_of(note));
        let mut copied = Vec::new();
        for p in paths.iter().filter(|p| attachment::is_image(p)) {
            let name = p
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("image.png");
            let to = attachment::free_path(&folder, name, |q| q.exists());
            std::fs::copy(p, &to).map_err(|e| format!("Could not copy {name}: {e}"))?;
            copied.push(to);
        }
        self.embed_attachments(&copied, cx);
        Ok(())
    }

    /// The paperclip of the Format bar: images chosen in a dialog, copied next
    /// to the Note and embedded at the cursor, as if dropped.
    pub(crate) fn pick_attachments(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(gpui_kit::PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some("Attach".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(paths))) = rx.await {
                let _ = this.update(cx, |app, cx| {
                    let r = app.attach_files(&paths, cx);
                    app.report(r);
                    cx.notify();
                });
            }
        })
        .detach();
    }

    /// `![[name]]` for each new Attachment at the cursor, and the Vault told.
    fn embed_attachments(&mut self, paths: &[PathBuf], cx: &mut Context<Self>) {
        if paths.is_empty() {
            return;
        }
        let Some(vault) = &self.vault else { return };
        let rel: HashSet<String> = paths
            .iter()
            .filter_map(|p| p.strip_prefix(&vault.root).ok())
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        let embeds: Vec<String> = paths
            .iter()
            .filter_map(|p| p.file_name()?.to_str().map(|n| format!("![[{n}]]")))
            .collect();
        self.refresh_vault(&rel, cx);
        self.update_link_context(cx);
        let text = embeds.join("\n");
        self.editor
            .update(cx, |ed, cx| ed.replace_selection(&text, cx));
    }

    /// Tells the editor the Note it shows and the Vault's links (Live preview
    /// draws an unresolved Wikilink faded).
    fn update_link_context(&mut self, cx: &mut Context<Self>) {
        let index = self
            .vault
            .as_ref()
            .map(|v| v.index.clone())
            .unwrap_or_default();
        let offered = index.link_completions(&self.recent_notes);
        let completions = offered.recent.into_iter().chain(offered.others).collect();
        let context = LinkContext {
            path: self.editor_path.clone(),
            index,
            root: self.vault.as_ref().map(|v| v.root.clone()),
            completions: std::rc::Rc::new(completions),
        };
        self.editor
            .update(cx, |ed, cx| ed.set_link_context(context, cx));
    }

    fn refresh_tree(&mut self, cx: &mut Context<Self>) {
        if self.vault.is_none() {
            return;
        }
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
        self.rebuild_tree(cx);
    }

    /// Whether any folder of Files is open: the button then collapses them all.
    pub fn any_folder_expanded(&self) -> bool {
        !self.expanded_folders.is_empty()
    }

    /// Expand All or Collapse All, the button over Files.
    pub fn set_all_folders_expanded(&mut self, expanded: bool, cx: &mut Context<Self>) {
        self.expanded_folders.clear();
        if expanded {
            if let Some(vault) = &self.vault {
                all_folders(&vault.scan, &mut self.expanded_folders);
            }
        }
        self.rebuild_tree(cx);
        self.persist_session();
        cx.notify();
    }

    /// The tree's items from the Vault, its folders open as `expanded_folders`.
    fn rebuild_tree(&mut self, cx: &mut Context<Self>) {
        let Some(vault) = &self.vault else { return };
        self.expanded_folders.retain(|p| vault.is_folder(p));
        let items = tree_items(&vault.scan, &self.expanded_folders);
        let mut empty = HashSet::new();
        empty_folders(&vault.scan, &mut empty);
        self.empty_folders = std::rc::Rc::new(empty);
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

    /// A file or folder the system asks to open (the Finder, Takku, `open`).
    pub fn open_external(&mut self, path: &Path, window: &mut Window, cx: &mut Context<Self>) {
        let path = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        let opened = {
            let vault = self.vault.as_ref();
            route(vault.map(|v| v.root.as_path()), &path, |rel| {
                let v = vault?;
                v.contains(rel).then(|| v.is_folder(rel))
            })
        };
        match opened {
            Opened::File(rel) => {
                self.open_path(&rel, OpenIn::NewTab, window, cx);
            }
            Opened::Folder(rel) => {
                let id = SharedString::from(rel);
                self.settings.left_sidebar_open = true;
                self.file_tree.update(cx, |tree, cx| {
                    tree.reveal_item(&id, ScrollStrategy::Nearest, cx);
                    let ix = tree.index_of(&id);
                    tree.set_selected_index(ix, cx);
                });
                cx.notify();
            }
            Opened::Loose(path) => {
                if let Err(e) = loose::LooseFile::open(path, self.palette, &self.settings, cx) {
                    self.set_error(e);
                    cx.notify();
                }
            }
            Opened::OtherFolder(folder) if self.vault.is_none() => {
                let _ = self.open_vault(&folder, window, cx);
            }
            Opened::OtherFolder(folder) => {
                let name = folder.file_name().map(|n| n.to_string_lossy().to_string());
                let answer = window.prompt(
                    gpui_kit::PromptLevel::Info,
                    &format!("Open “{}” as the Vault?", name.unwrap_or_default()),
                    Some("The Vault open now closes."),
                    &["Open as Vault", "Cancel"],
                    cx,
                );
                cx.spawn_in(window, async move |this, cx| {
                    if answer.await == Ok(0) {
                        let _ = this
                            .update_in(cx, |app, window, cx| app.open_vault(&folder, window, cx));
                    }
                })
                .detach();
            }
            Opened::Nothing => {}
        }
    }

    fn remember_cursor(&mut self, cx: &App) {
        if self.editor_path.is_some() && self.editor_path.as_deref() == self.active_path() {
            let cursor = self.editor.read(cx).cursor();
            self.tabs.set_active_cursor(cursor);
        }
    }

    /// Makes the editor, inspector and tree follow the active tab.
    fn show_active(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.prune_docs();
        let active = self.tabs.active().map(|t| (t.path.clone(), t.cursor));
        match active {
            Some((path, cursor)) if self.docs.contains_key(&path) => {
                let text = self.docs[&path].text().to_string();
                let other_note = self.editor_path.as_deref() != Some(path.as_str());
                if other_note {
                    self.editor.update(cx, |ed, cx| {
                        ed.open_note(&path, &text, cursor, cx);
                        ed.scroll_to_top(cx);
                        ed.reveal_cursor(cx);
                    });
                } else if self.editor_text(cx) != text {
                    self.set_editor_text(&text, cursor, window, cx);
                } else {
                    self.set_editor_cursor(cursor, cx);
                }
                self.editor_path = Some(path.clone());
                self.recent_notes.retain(|p| p != &path);
                self.recent_notes.insert(0, path);
                self.recent_notes.truncate(settings::MAX_RECENT_NOTES);
            }
            _ => {
                self.editor_path = None;
                // The editor leaves the window: the keyboard stays with the window.
                if self.editor.focus_handle(cx).contains_focused(window, cx) {
                    window.focus(&self.focus_handle, cx);
                }
            }
        }
        self.update_link_context(cx);
        self.refresh_outline();
        self.refresh_backlinks();
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
                        "{path}: its text is kept in memory; reopen the tab ({}) to resolve it",
                        crate::keys::label("⇧⌘T")
                    ));
                }
            }
            self.show_active(window, cx);
        }
    }

    /// Closes every Tab but the `keep`-th (each saved as when closed alone).
    pub fn close_other_tabs(&mut self, keep: usize, window: &mut Window, cx: &mut Context<Self>) {
        for i in (0..self.tabs.len()).rev().filter(|&i| i != keep) {
            self.close_tab(i, window, cx);
        }
    }

    /// Closes the Tabs to the right of the `ix`-th.
    pub fn close_tabs_after(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        for i in (ix + 1..self.tabs.len()).rev() {
            self.close_tab(i, window, cx);
        }
    }

    /// Copies a file's path in the Vault, or (`full`) its absolute path.
    pub fn copy_path(&mut self, path: &str, full: bool, cx: &mut Context<Self>) {
        let text = match (&self.vault, full) {
            (Some(v), true) => v.root.join(path).to_string_lossy().into_owned(),
            _ => path.to_string(),
        };
        cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(text));
    }

    /// Shows a file or folder in the Finder.
    /// The user's terminal in the folder `path` of the Vault, or the folder of
    /// the file `path`; `""` is the Vault's root.
    pub(crate) fn open_in_terminal(&mut self, path: &str) {
        let Some(v) = &self.vault else {
            return;
        };
        let target = v.root.join(path);
        let folder = if target.is_dir() {
            target
        } else {
            target.parent().map_or(v.root.clone(), Path::to_path_buf)
        };
        if let Err(e) = terminal::open(&folder) {
            self.set_error(format!("Could not open the terminal: {e}"));
        }
    }

    pub(crate) fn reveal(&mut self, path: &str, cx: &mut Context<Self>) {
        if let Some(v) = &self.vault {
            cx.reveal_path(&v.root.join(path));
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
        match event {
            NoteEditorEvent::Changed => {}
            NoteEditorEvent::PasteImage { bytes, extension } => {
                let name =
                    attachment::pasted_image_name(chrono::Local::now().naive_local(), extension);
                let r = self.attach_bytes(&name, bytes, cx);
                self.report(r);
                return;
            }
            NoteEditorEvent::DropFiles(paths) => {
                let r = self.attach_files(paths, cx);
                self.report(r);
                return;
            }
            NoteEditorEvent::OpenLink { link, new_tab } => {
                match link {
                    Link::Url(url) => cx.open_url(url),
                    Link::Note(w) => {
                        self.follow_link(&w.target, w.heading.as_deref(), *new_tab, window, cx)
                    }
                }
                return;
            }
        }
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
        self.schedule_autosave(window, cx);
        // The editor draws each key itself; the rest of the window (Outline,
        // the words in the status bar) is drawn again at a pause in typing,
        // not at every key: laying out the whole window took a quarter of it.
        self.typing_redraw = Some(cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(TYPING_PAUSE).await;
            let _ = this.update(cx, |_, cx| cx.notify());
        }));
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
        let rel = today_daily_note_path(
            &self.settings.daily_note_folder,
            &self.settings.daily_note_format,
        );
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

    pub(crate) fn refresh_search(&mut self, cx: &App) {
        let query = self.search_input.read(cx).value().to_string();
        self.search_results = match &self.vault {
            Some(v) if !query.trim().is_empty() => search_vault(&v.corpus, &query),
            _ => Vec::new(),
        };
    }

    pub(crate) fn format(&mut self, command: Format, window: &mut Window, cx: &mut Context<Self>) {
        if self.editor_path.is_none() {
            return;
        }
        self.editor.update(cx, |ed, cx| {
            ed.format(command, cx);
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
        Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/demo-vault"),
    ]
    .into_iter()
    .find(|p| p.is_dir())
}
