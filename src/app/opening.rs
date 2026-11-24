//! Opening Notes and attachments in Tabs, the history of each Tab, and
//! the Daily note.

use super::*;

impl NotekiApp {
    /// Adds `path` to the tabs, loading its note; false if it cannot be read.
    pub(super) fn load_into_tabs(&mut self, path: &str, mode: OpenIn) -> bool {
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
            self.save_logged(&leaving, cx);
        }
        self.show_active(window, cx);
        true
    }

    /// A file or folder the system asks to open (the Finder, Takku, `open`).
    pub fn open_external(&mut self, path: &Path, window: &mut Window, cx: &mut Context<Self>) {
        let path = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        let route_now = |app: &Self| {
            let vault = app.vault.as_ref();
            route(vault.map(|v| v.root.as_path()), &path, |rel| {
                let v = vault?;
                v.contains(rel).then(|| v.is_folder(rel))
            })
        };
        let mut opened = route_now(self);
        // A file just made in the Vault (by the `noteki` command, or another
        // app) before the watcher has told: listed now, unless it is in an
        // Excluded folder.
        let in_vault = self
            .vault
            .as_ref()
            .is_some_and(|v| v.relative(&path).is_some());
        if opened == Opened::Nothing && in_vault && path.exists() {
            self.handle_fs_changes(std::slice::from_ref(&path), window, cx);
            opened = route_now(self);
        }
        // A Loose file comes forward in its own window. On the Mac this
        // window's activation lands after that window opens, so activating
        // it here too would cover the Loose file.
        if !matches!(opened, Opened::Loose(_)) {
            window.activate_window();
        }
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
                if let Err(e) = loose::LooseFile::open(path, Some(self.palette), &self.settings, cx)
                {
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

    pub(super) fn remember_cursor(&mut self, cx: &App) {
        if self.editor_path.is_some() && self.editor_path.as_deref() == self.active_path() {
            let cursor = self.editor.read(cx).cursor();
            self.tabs.set_active_cursor(cursor);
        }
    }

    /// Makes the editor, inspector and tree follow the active tab.
    pub(super) fn show_active(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.prune_docs(cx);
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
    pub(super) fn prune_docs(&mut self, cx: &mut Context<Self>) {
        let shown: HashSet<String> = self.tabs.iter().map(|t| t.path.clone()).collect();
        let unsaved: Vec<String> = self
            .docs
            .iter()
            .filter(|(p, d)| !shown.contains(*p) && d.should_save())
            .map(|(p, _)| p.clone())
            .collect();
        for p in unsaved {
            self.save_logged(&p, cx);
        }
        self.docs
            .retain(|p, d| shown.contains(p) || d.is_only_on_screen() || d.should_save());
    }

    pub(super) fn set_editor_text(
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

    pub(super) fn set_editor_cursor(&mut self, cursor: usize, cx: &mut Context<Self>) {
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
                self.save_logged(&l, cx);
            }
            self.show_active(window, cx);
        }
    }

    pub fn close_tab(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.remember_cursor(cx);
        if let Some(path) = self.tabs.close(index) {
            if !self.tabs.contains(&path) {
                self.save_logged(&path, cx);
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
            self.save_logged(&l, cx);
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
            self.save_logged(&l, cx);
        }
        self.show_active(window, cx);
    }
}
