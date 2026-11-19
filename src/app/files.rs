//! Files and folders: new, rename, duplicate, trash, attachments.

use super::*;

/// What one window did to the Vault's files, told to its other windows.
#[derive(Clone)]
enum Moved {
    /// From, to: a Note, an Attachment or a folder.
    Renamed(String, String),
    Trashed(String),
}

impl NotekiApp {
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
        // Unsaved text is saved first, under the old name (SPEC › Files),
        // in every window of the Vault.
        self.save_under(path, cx)?;
        self.others_save_under(path, cx)?;
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
        self.tell_others(Moved::Renamed(path.to_string(), new_path.clone()), cx);
        Ok(new_path)
    }

    /// Moves a note, attachment or folder to the Trash; its tabs close after saving.
    pub fn trash(
        &mut self,
        path: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        self.save_under(path, cx)?;
        self.others_save_under(path, cx)?;
        let vault = self.vault.as_ref().ok_or("No vault open")?;
        move_to_trash(&vault.root, path, self.config.trash_dir.as_deref())?;
        self.close_trashed(path);
        self.refresh_vault(&HashSet::new(), cx);
        self.show_active(window, cx);
        self.tell_others(Moved::Trashed(path.to_string()), cx);
        Ok(())
    }

    /// Saves the unsaved Notes at `path` or under it (a folder).
    fn save_under(&mut self, path: &str, cx: &mut Context<Self>) -> Result<(), String> {
        let under: Vec<String> = self
            .docs
            .keys()
            .filter(|p| rename_target(p, path, "").is_some())
            .cloned()
            .collect();
        for p in &under {
            self.save_note(p, cx)?;
        }
        Ok(())
    }

    /// The other windows of this Vault (ADR 0010).
    fn vault_windows(&self, cx: &Context<Self>) -> Vec<Entity<NotekiApp>> {
        let root = self.vault.as_ref().map(|v| v.root.clone());
        self.other_windows(cx)
            .into_iter()
            .filter(|o| root.is_some() && o.read(cx).vault.as_ref().map(|v| v.root.clone()) == root)
            .collect()
    }

    fn others_save_under(&mut self, path: &str, cx: &mut Context<Self>) -> Result<(), String> {
        for other in self.vault_windows(cx) {
            other.update(cx, |o, cx| o.save_under(path, cx))?;
        }
        Ok(())
    }

    /// Tells the other windows of this Vault what this one moved, at once: a
    /// folder's move reaches their watchers as the folder alone (ADR 0010).
    fn tell_others(&self, moved: Moved, cx: &mut Context<Self>) {
        for other in self.vault_windows(cx) {
            let (handle, moved) = (other.read(cx).window, moved.clone());
            cx.defer(move |cx| {
                let _ = handle.update(cx, |_, window, cx| {
                    other.update(cx, |o, cx| o.moved_elsewhere(&moved, window, cx))
                });
            });
        }
    }

    fn moved_elsewhere(&mut self, moved: &Moved, window: &mut Window, cx: &mut Context<Self>) {
        match moved {
            Moved::Renamed(old, new) => {
                self.follow_rename(old, new);
                self.refresh_vault(&HashSet::from([new.clone()]), cx);
            }
            Moved::Trashed(path) => {
                self.close_trashed(path);
                self.refresh_vault(&HashSet::new(), cx);
            }
        }
        self.show_active(window, cx);
        self.persist_session();
        cx.notify();
    }

    /// Closes the Tabs at `path` or under it, gone to the Trash, once saved.
    fn close_trashed(&mut self, path: &str) {
        let under: Vec<String> = self
            .docs
            .keys()
            .filter(|p| rename_target(p, path, "").is_some())
            .cloned()
            .collect();
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
    }

    pub fn duplicate(
        &mut self,
        path: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<String, String> {
        self.save_note(path, cx)?;
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
}
