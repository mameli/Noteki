//! The Noteki window: `NotekiApp`, its state and what it does. The rules
//! (saving, merging, tabs, links) live in `crate::core` and are unit-tested
//! there; this module wires them to GPUI and the file system.
//!
//! Here: the state, opening a window, the accessors the view and the tests
//! read. What it does, by subject: `vault` (the open Vault and its tree),
//! `opening` (Notes in Tabs), `editing` (after an edit), `external` (changes
//! on disk), `links`, `files`, `preferences` (settings changed from the
//! window). Drawing: `render`, and `parts` for the cached parts around the
//! editor.

mod editing;
mod external;
mod files;
mod format_bar;
mod links;
mod loose;
pub mod menus;
mod opening;
mod parts;
mod preferences;
pub mod print;
mod render;
pub mod settings;
mod settings_window;
mod swift_prefs;
pub mod terminal;
pub mod updates;
mod vault;

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
pub use vault::Vault;

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

/// Files handed over by the Finder, Takku or `open`: to the Noteki window in
/// front, else to any open one, else to a new window (the first window may
/// have been closed, or all of them, Noteki still running).
pub fn hand_over(paths: &[PathBuf], config: &AppConfig, cx: &mut App) {
    let open: Vec<Entity<NotekiApp>> = cx
        .try_global::<OpenWindows>()
        .map(|w| w.0.iter().filter_map(|a| a.upgrade()).collect())
        .unwrap_or_default();
    let open: Vec<Entity<NotekiApp>> = open
        .into_iter()
        .filter(|a| cx.windows().contains(&a.read(cx).window))
        .collect();
    let front = cx.active_window();
    let target = open
        .iter()
        .find(|a| Some(a.read(cx).window) == front)
        .or(open.first())
        .map(|a| (a.read(cx).window, a.clone()))
        .or_else(|| open_noteki_window(config.clone(), cx));
    let Some((handle, app)) = target else { return };
    let _ = handle.update(cx, |_, window, cx| {
        window.activate_window();
        for path in paths {
            app.update(cx, |a, cx| a.open_external(path, window, cx));
        }
    });
}

/// A click on the Dock icon: a new window when none is open (all closed,
/// Noteki still running), as Mac apps do; with windows open, nothing.
pub fn reopen(config: &AppConfig, cx: &mut App) {
    let any_open = cx.try_global::<OpenWindows>().is_some_and(|w| {
        w.0.iter()
            .filter_map(|a| a.upgrade())
            .any(|a| cx.windows().contains(&a.read(cx).window))
    });
    if !any_open {
        if let Some((handle, _)) = open_noteki_window(config.clone(), cx) {
            let _ = handle.update(cx, |_, window, _| window.activate_window());
        }
    }
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

    /// The parts around the editor, cached views (`parts`).
    parts: Parts,

    _subscriptions: Vec<Subscription>,
}

struct Parts {
    title: Entity<parts::Part>,
    left: Entity<parts::Part>,
    right: Entity<parts::Part>,
    tabs: Entity<parts::Part>,
    status: Entity<parts::Part>,
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
            cx.on_app_quit(|this, cx| {
                this.flush_all(cx);
                async {}
            }),
        ];

        let me = cx.entity();
        let parts = Parts {
            title: parts::Part::new(&me, parts::PartKind::TitleBar, cx),
            left: parts::Part::new(&me, parts::PartKind::LeftSidebar, cx),
            right: parts::Part::new(&me, parts::PartKind::RightSidebar, cx),
            tabs: parts::Part::new(&me, parts::PartKind::TabBar, cx),
            status: parts::Part::new(&me, parts::PartKind::StatusBar, cx),
        };
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
            parts,
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
        // Closed by the user: what was typed in the last second is saved
        // (the autosave goes with the window), and a window other than the
        // first does not come back at the next launch.
        let this = cx.entity().downgrade();
        window.on_window_should_close(cx, move |_, cx| {
            let _ = this.update(cx, |app, cx| {
                app.flush_all(cx);
                if app.config.slot > 0 {
                    app.forget_session();
                }
            });
            true
        });
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
    // ---- Vault ----
    // ---- Opening notes ----
    // ---- Editing and saving ----
    // ---- External changes ----
    // ---- Links ----
    // ---- Files ----
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
