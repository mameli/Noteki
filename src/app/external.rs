//! Changes made outside Noteki: the file watcher, merging a Note changed
//! on disk, files added, moved or deleted.

use super::*;

impl NotekiApp {
    pub(super) fn start_watcher(&mut self, root: &Path) {
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

    pub(super) fn start_fs_poller(&self, window: &mut Window, cx: &mut Context<Self>) {
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
            self.save_logged(&p, cx);
        }
        self.refresh_outline();
        self.persist_session();
        cx.notify();
    }

    /// Moves tabs, documents and remembered paths from `old` to `new` (a note or a folder).
    pub(super) fn follow_rename(&mut self, old: &str, new: &str) {
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
            self.save_logged(path, cx);
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
            self.save_logged(path, cx);
            let changed = HashSet::from([path.to_string()]);
            self.refresh_vault(&changed, cx);
            self.status = None;
        }
        cx.notify();
    }
}
