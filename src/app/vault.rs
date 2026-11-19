//! The open Vault: its files scanned, indexed and shown in the tree, and
//! opening another.

use super::*;

/// The files of the open vault and everything derived from them.
pub struct Vault {
    pub root: PathBuf,
    pub scan: Vec<VaultItem>,
    pub index: Arc<VaultIndex>,
    /// `(name, path)` of every note.
    pub notes: Vec<(String, String)>,
    pub corpus: Corpus,
    pub tag_tree: Vec<TagNode>,
    pub(super) excluded: HashSet<String>,
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
    pub(super) fn relative(&self, abs: &Path) -> Option<String> {
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

pub(super) fn find_item<'a>(items: &'a [VaultItem], path: &str) -> Option<&'a VaultItem> {
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

pub(super) fn collect_notes(items: &[VaultItem], out: &mut Vec<(String, String)>) {
    for item in items {
        match item {
            VaultItem::Note { name, path } => out.push((name.clone(), path.clone())),
            VaultItem::Folder { children, .. } => collect_notes(children, out),
            VaultItem::Attachment { .. } => {}
        }
    }
}

pub(super) fn all_folders(items: &[VaultItem], out: &mut HashSet<String>) {
    for item in items {
        if let VaultItem::Folder { path, children, .. } = item {
            out.insert(path.clone());
            all_folders(children, out);
        }
    }
}

pub(super) fn empty_folders(items: &[VaultItem], out: &mut HashSet<String>) {
    for item in items {
        if let VaultItem::Folder { path, children, .. } = item {
            if children.is_empty() {
                out.insert(path.clone());
            }
            empty_folders(children, out);
        }
    }
}

pub(super) fn tree_items(items: &[VaultItem], expanded: &HashSet<String>) -> Vec<TreeItem> {
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

impl NotekiApp {
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

    pub(super) fn refresh_vault(&mut self, changed: &HashSet<String>, cx: &mut Context<Self>) {
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
    pub(super) fn attach_bytes(
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
    pub(super) fn attach_files(
        &mut self,
        paths: &[PathBuf],
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
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
    pub(super) fn embed_attachments(&mut self, paths: &[PathBuf], cx: &mut Context<Self>) {
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
    pub(super) fn update_link_context(&mut self, cx: &mut Context<Self>) {
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

    pub(super) fn refresh_tree(&mut self, cx: &mut Context<Self>) {
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
    pub(super) fn rebuild_tree(&mut self, cx: &mut Context<Self>) {
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

    pub(super) fn select_in_tree(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.active_path().map(SharedString::from) else {
            return;
        };
        self.file_tree.update(cx, |tree, cx| {
            tree.reveal_item(&id, ScrollStrategy::Nearest, cx);
            let ix = tree.index_of(&id);
            tree.set_selected_index(ix, cx);
        });
    }
}
