//! Everything Noteki does with a vault, without Tauri: open notes and how they
//! are saved and merged (SPEC › Saving, External changes), file-system changes,
//! links, search, tags, backlinks. `lib.rs` exposes these as commands and
//! forwards the returned [`Event`]s to the window, so all of it is testable
//! with plain `cargo test`.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::document::{ExternalOutcome, NoteDoc};
use crate::embeds::{resolve_embed, EmbeddedFile};
use crate::index::{
    build_tag_tree, find_backlinks, search_vault, BacklinkMatch, Corpus, SearchResult, TagNode,
};
use crate::merge::merge_three_way;
use crate::vault::{
    create_empty_note, new_note_path_for_link, next_untitled_path, normalize_excluded, read_note,
    relative_path_from_disk, scan_vault, write_note_atomic, AttachmentKind, VaultItem,
};
use crate::wikilink::{LinkCompletions, ResolvedItem, VaultIndex, Wikilink};

pub const MAX_RECENT_NOTES: usize = 30;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
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

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum FollowLinkResult {
    Note {
        path: String,
        created: bool,
    },
    Attachment {
        path: String,
        #[serde(rename = "attachmentKind")]
        attachment_kind: AttachmentKind,
    },
}

/// Something the window must learn about. Serialized as the payload of the
/// Tauri event named by [`Event::name`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged, rename_all_fields = "camelCase")]
pub enum Event {
    VaultTreeChanged {
        tree: Vec<VaultItem>,
        note_count: usize,
    },
    /// The note's text changed (reloaded from disk, merged, or rebased): load it
    /// into the editor if the editor's `revision` is still this one.
    NoteReloaded {
        path: String,
        revision: u64,
        text: String,
        merged: bool,
    },
    NoteConflict {
        path: String,
        disk_text: String,
    },
    NoteDeleted {
        path: String,
    },
    /// Another app renamed or moved an open note (a Followed rename).
    NoteRenamed {
        from: String,
        to: String,
    },
    SaveFailed {
        path: String,
        error: String,
    },
}

impl Event {
    pub fn name(&self) -> &'static str {
        match self {
            Self::VaultTreeChanged { .. } => "vault-tree-changed",
            Self::NoteReloaded { .. } => "note-reloaded",
            Self::NoteConflict { .. } => "note-conflict",
            Self::NoteDeleted { .. } => "note-deleted",
            Self::NoteRenamed { .. } => "note-renamed",
            Self::SaveFailed { .. } => "save-failed",
        }
    }
}

/// The open vault and what is derived from its files.
pub struct Vault {
    pub root: PathBuf,
    pub tree: Vec<VaultItem>,
    pub index: VaultIndex,
    pub corpus: Corpus,
    pub tag_tree: Vec<TagNode>,
    excluded: HashSet<String>,
}

impl Vault {
    pub fn load(root: &Path) -> Result<Self, String> {
        if !root.is_dir() {
            return Err(format!("Vault folder does not exist: {}", root.display()));
        }
        let root =
            fs::canonicalize(root).map_err(|e| format!("Cannot open {}: {e}", root.display()))?;
        let mut vault = Self {
            root,
            tree: Vec::new(),
            index: VaultIndex::default(),
            corpus: Corpus::default(),
            tag_tree: Vec::new(),
            excluded: normalize_excluded(&[]),
        };
        vault.refresh(&HashSet::new())?;
        Ok(vault)
    }

    /// Rescans the tree; re-reads the notes in `changed`. Returns whether the tree changed.
    pub fn refresh(&mut self, changed: &HashSet<String>) -> Result<bool, String> {
        let tree = scan_vault(&self.root, &self.excluded)?;
        let mut notes = Vec::new();
        collect_notes(&tree, &mut notes);
        self.corpus = self.corpus.refresh(&self.root, &notes, changed);
        self.tag_tree = build_tag_tree(&self.corpus);
        let tree_changed = tree != self.tree;
        if tree_changed {
            self.index = VaultIndex::from_tree(&tree);
            self.tree = tree;
        }
        Ok(tree_changed)
    }

    pub fn note_count(&self) -> usize {
        self.corpus.len()
    }

    pub fn snapshot(&self) -> VaultSnapshot {
        VaultSnapshot {
            root: self.root.to_string_lossy().to_string(),
            tree: self.tree.clone(),
            note_count: self.note_count(),
        }
    }

    /// The file behind a `noteki-file://` URL, unless it is outside the vault or excluded.
    pub fn file_path(&self, rel: &str) -> Option<PathBuf> {
        if crate::vault::is_in_excluded_folder(&self.root, rel, &self.excluded) {
            return None;
        }
        crate::vault::resolve_vault_path(&self.root, rel).ok()
    }

    fn relative(&self, abs: &Path) -> Option<String> {
        relative_path_from_disk(&self.root, abs, &self.excluded)
    }
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

/// The last text change sent to the editor, kept until the editor applies or
/// rejects it: `from` is the text it was computed from, `text` the text sent.
#[derive(Debug, Clone)]
struct SentReload {
    from: String,
    text: String,
}

struct OpenDoc {
    doc: NoteDoc,
    /// The last revision the editor reported for this note (the editor rejects
    /// a reload whose revision is not its last report).
    revision: u64,
    sent: Option<SentReload>,
}

pub struct Session {
    settings_path: Option<PathBuf>,
    pub settings: AppSettings,
    vault: Option<Vault>,
    docs: HashMap<String, OpenDoc>,
}

impl Session {
    /// `settings_path`: where preferences live; `None` keeps them in memory.
    pub fn new(settings_path: Option<PathBuf>) -> Self {
        let settings = settings_path
            .as_deref()
            .map(load_settings)
            .unwrap_or_default();
        Self {
            settings_path,
            settings,
            vault: None,
            docs: HashMap::new(),
        }
    }

    pub fn vault(&self) -> Option<&Vault> {
        self.vault.as_ref()
    }

    pub fn doc(&self, path: &str) -> Option<&NoteDoc> {
        self.docs.get(path).map(|d| &d.doc)
    }

    fn vault_ref(&self) -> Result<&Vault, String> {
        self.vault
            .as_ref()
            .ok_or_else(|| "No vault is open".to_string())
    }

    pub fn save_settings(&mut self, settings: AppSettings) -> Result<(), String> {
        self.settings = settings;
        self.persist()
    }

    fn persist(&self) -> Result<(), String> {
        match &self.settings_path {
            Some(path) => store_settings(path, &self.settings),
            None => Ok(()),
        }
    }

    // ---- Vault ----

    pub fn open_vault(&mut self, path: &Path) -> Result<(VaultSnapshot, Vec<Event>), String> {
        let events = self.flush_all();
        let vault = Vault::load(path)?;
        self.settings.vault_path = Some(vault.root.to_string_lossy().to_string());
        let snapshot = vault.snapshot();
        self.vault = Some(vault);
        self.docs.clear();
        self.persist()?;
        Ok((snapshot, events))
    }

    pub fn create_new_note(&mut self, folder_rel: &str) -> Result<(String, VaultSnapshot), String> {
        let vault = self.vault_ref()?;
        let rel = next_untitled_path(&vault.root, folder_rel);
        create_empty_note(&vault.root, &rel)?;
        let vault = self.vault.as_mut().expect("vault");
        vault.refresh(&HashSet::from([rel.clone()]))?;
        Ok((rel, vault.snapshot()))
    }

    // ---- Notes ----

    pub fn open_note(&mut self, path: &str) -> Result<OpenNoteResponse, String> {
        let root = self.vault_ref()?.root.clone();
        match self.docs.get_mut(path) {
            Some(od) => {
                // Catch up with what the watcher may have missed while it was behind.
                let disk = read_disk(&root, path);
                od.doc.on_disk_changed(disk.as_deref());
            }
            None => {
                let text = read_note(&root, path)?;
                self.docs.insert(
                    path.to_string(),
                    OpenDoc {
                        doc: NoteDoc::new(text),
                        revision: 0,
                        sent: None,
                    },
                );
            }
        }
        let recent = &mut self.settings.recent_notes;
        recent.retain(|p| p != path);
        recent.insert(0, path.to_string());
        recent.truncate(MAX_RECENT_NOTES);
        let _ = self.persist();
        let doc = &self.docs[path].doc;
        Ok(OpenNoteResponse {
            path: path.to_string(),
            text: doc.text().to_string(),
            conflict: doc.has_conflict(),
            deleted: doc.is_deleted(),
        })
    }

    /// The editor reported its text for `path`.
    pub fn update_buffer(&mut self, path: &str, revision: u64, text: &str) {
        if let Some(od) = self.docs.get_mut(path) {
            od.revision = revision;
            od.doc.edit(text);
        }
    }

    /// Writes the note's text if it has unsaved edits and may be saved. Returns
    /// whether the disk now has the editor's text (false during a conflict or
    /// after a deletion by another app).
    pub fn save_note(&mut self, path: &str) -> Result<bool, String> {
        let Some(vault) = self.vault.as_mut() else {
            return Err("No vault is open".to_string());
        };
        let Some(od) = self.docs.get_mut(path) else {
            return Ok(false);
        };
        if od.doc.has_conflict() || od.doc.is_deleted() {
            return Ok(false);
        }
        if !od.doc.should_save() {
            return Ok(true);
        }
        let text = od.doc.text().to_string();
        write_note_atomic(&vault.root, path, &text)?;
        od.doc.mark_saved(&text);
        if vault.corpus.get(path).is_some() {
            vault.corpus.set(path, &text);
            vault.tag_tree = build_tag_tree(&vault.corpus);
        }
        Ok(true)
    }

    /// Saves every note with unsaved edits (on quit, on switching vault).
    pub fn flush_all(&mut self) -> Vec<Event> {
        let paths: Vec<String> = self
            .docs
            .iter()
            .filter(|(_, d)| d.doc.should_save())
            .map(|(p, _)| p.clone())
            .collect();
        paths
            .into_iter()
            .filter_map(|path| {
                self.save_note(&path)
                    .err()
                    .map(|error| Event::SaveFailed { path, error })
            })
            .collect()
    }

    /// Conflict banner: `keep_mine` or `take_theirs`. Returns the text the editor should show.
    pub fn resolve_conflict(&mut self, path: &str, choice: &str) -> Result<String, String> {
        let od = self
            .docs
            .get_mut(path)
            .ok_or_else(|| format!("{path} is not open"))?;
        match choice {
            "keep_mine" => od.doc.keep_mine(),
            "take_theirs" => {
                od.doc.take_theirs();
            }
            _ => return Err(format!("Unknown conflict choice: {choice}")),
        }
        od.sent = None;
        let text = od.doc.text().to_string();
        self.save_note(path)?;
        Ok(text)
    }

    /// Deleted banner: write the text back to its path.
    pub fn save_again(&mut self, path: &str) -> Result<(), String> {
        let od = self
            .docs
            .get_mut(path)
            .ok_or_else(|| format!("{path} is not open"))?;
        od.doc.save_again();
        self.save_note(path)?;
        if let Some(vault) = self.vault.as_mut() {
            vault.refresh(&HashSet::from([path.to_string()]))?;
        }
        Ok(())
    }

    /// The editor refused the last reload because the user typed meanwhile. Its
    /// text (the last report) lacks what came from disk: merge the two again,
    /// so that saving it does not overwrite the other app's change.
    pub fn reload_rejected(&mut self, path: &str) -> Vec<Event> {
        let Some(od) = self.docs.get_mut(path) else {
            return Vec::new();
        };
        let Some(sent) = od.sent.take() else {
            return Vec::new();
        };
        let mine = od.doc.text().to_string();
        if mine == sent.text {
            return Vec::new();
        }
        match merge_three_way(&sent.from, &mine, &sent.text) {
            Some(rebased) if rebased == mine => Vec::new(), // the editor already has it all
            Some(rebased) => {
                od.doc.edit(&rebased);
                od.sent = Some(SentReload {
                    from: mine,
                    text: rebased.clone(),
                });
                let revision = od.revision;
                let mut events = vec![Event::NoteReloaded {
                    path: path.to_string(),
                    revision,
                    text: rebased,
                    merged: true,
                }];
                if let Err(error) = self.save_note(path) {
                    events.push(Event::SaveFailed {
                        path: path.to_string(),
                        error,
                    });
                }
                events
            }
            None => {
                // The disk keeps the other version; the editor keeps the user's.
                od.doc.set_conflict(&sent.text);
                vec![Event::NoteConflict {
                    path: path.to_string(),
                    disk_text: sent.text,
                }]
            }
        }
    }

    // ---- External changes ----

    /// Files changed on disk (absolute paths, as the watcher reports them):
    /// reload, merge and save, conflict, deletion, followed rename, new tree.
    pub fn fs_changed(&mut self, abs_paths: &[PathBuf]) -> Vec<Event> {
        let Some(vault) = self.vault.as_ref() else {
            return Vec::new();
        };
        let root = vault.root.clone();
        let changed: HashSet<String> = abs_paths.iter().filter_map(|p| vault.relative(p)).collect();
        if changed.is_empty() {
            return Vec::new();
        }
        let known_before: HashSet<String> = vault.corpus.iter().map(|n| n.path.clone()).collect();
        let mut events = Vec::new();
        let mut deleted = Vec::new();
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
                match fs::read_to_string(&disk_path) {
                    Ok(t) => Some(t),
                    Err(_) => continue, // mid-write: the next event tells
                }
            } else {
                None
            };
            let od = self.docs.get_mut(&path).expect("affected");
            let before = od.doc.text().to_string();
            let outcome = od.doc.on_disk_changed(disk.as_deref());
            match outcome {
                ExternalOutcome::Reloaded | ExternalOutcome::Merged => {
                    let text = od.doc.text().to_string();
                    od.sent = Some(SentReload {
                        from: before,
                        text: text.clone(),
                    });
                    let merged = outcome == ExternalOutcome::Merged;
                    events.push(Event::NoteReloaded {
                        path: path.clone(),
                        revision: od.revision,
                        text,
                        merged,
                    });
                    if merged {
                        to_save.push(path);
                    }
                }
                ExternalOutcome::Conflict => events.push(Event::NoteConflict {
                    disk_text: disk.unwrap_or_default(),
                    path,
                }),
                ExternalOutcome::Deleted => deleted.push(path),
                ExternalOutcome::Unchanged | ExternalOutcome::Synced => {}
            }
        }

        let vault = self.vault.as_mut().expect("vault");
        match vault.refresh(&changed) {
            Ok(true) => events.push(Event::VaultTreeChanged {
                tree: vault.tree.clone(),
                note_count: vault.note_count(),
            }),
            Ok(false) => {}
            Err(error) => events.push(Event::SaveFailed {
                path: String::new(),
                error,
            }),
        }

        // Followed rename: a deleted open note whose text appeared under a new path.
        let appeared: Vec<(String, String)> = changed
            .iter()
            .filter(|p| !known_before.contains(*p))
            .filter_map(|p| vault.corpus.get(p).map(|t| (p.clone(), t.to_string())))
            .collect();
        for old in std::mem::take(&mut deleted) {
            let doc = &self.docs[&old].doc;
            let target = appeared
                .iter()
                .find(|(new, t)| {
                    !self.docs.contains_key(new) && (t == doc.base() || t == doc.text())
                })
                .map(|(new, t)| (new.clone(), t.clone()));
            match target {
                Some((new, disk)) => {
                    let mut od = self.docs.remove(&old).expect("deleted doc");
                    od.doc.on_disk_changed(Some(&disk));
                    // The window loads it under its new path, where the editor
                    // has reported nothing yet.
                    od.revision = 0;
                    od.sent = None;
                    self.docs.insert(new.clone(), od);
                    for p in &mut self.settings.recent_notes {
                        if *p == old {
                            *p = new.clone();
                        }
                    }
                    events.push(Event::NoteRenamed { from: old, to: new });
                }
                None => events.push(Event::NoteDeleted { path: old }),
            }
        }

        for path in to_save {
            if let Err(error) = self.save_note(&path) {
                events.push(Event::SaveFailed { path, error });
            }
        }
        events
    }

    // ---- Links ----

    pub fn check_links(&self, note_path: &str, inners: Vec<String>) -> CheckLinksResult {
        let mut result = CheckLinksResult {
            unresolved: Vec::new(),
            embeds: HashMap::new(),
        };
        let Some(vault) = &self.vault else {
            return result;
        };
        for raw in inners {
            let link = Wikilink::parse(&raw);
            if vault.index.resolve(&link.target, Some(note_path)).is_none() {
                result.unresolved.push(raw);
            } else if let Some(file) =
                resolve_embed(&vault.root, &raw, Some(note_path), &vault.index)
            {
                result.embeds.insert(raw, file);
            }
        }
        result
    }

    /// Opens what `[[inner]]` leads to, creating an empty note when it resolves nowhere.
    pub fn follow_link(
        &mut self,
        source_path: &str,
        inner: &str,
    ) -> Result<FollowLinkResult, String> {
        let vault = self.vault_ref()?;
        let parsed = Wikilink::parse(inner);
        if let Some(resolved) = vault.index.resolve(&parsed.target, Some(source_path)) {
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
        let new_path = new_note_path_for_link(
            &vault.root,
            &parsed.target,
            Some(source_path),
            &vault.excluded,
        )?;
        create_empty_note(&vault.root, &new_path)?;
        self.vault
            .as_mut()
            .expect("vault")
            .refresh(&HashSet::from([new_path.clone()]))?;
        Ok(FollowLinkResult::Note {
            path: new_path,
            created: true,
        })
    }

    pub fn complete_link(&self) -> LinkCompletions {
        match &self.vault {
            Some(v) => v.index.link_completions(&self.settings.recent_notes),
            None => VaultIndex::default().link_completions(&[]),
        }
    }

    // ---- Indexes ----

    pub fn search(&self, query: &str) -> Vec<SearchResult> {
        self.vault
            .as_ref()
            .map(|v| search_vault(&v.corpus, query))
            .unwrap_or_default()
    }

    pub fn backlinks(&self, path: &str) -> Vec<BacklinkMatch> {
        self.vault
            .as_ref()
            .map(|v| find_backlinks(&v.corpus, path, &v.index))
            .unwrap_or_default()
    }

    pub fn tags(&self) -> Vec<TagNode> {
        self.vault
            .as_ref()
            .map(|v| v.tag_tree.clone())
            .unwrap_or_default()
    }
}

fn read_disk(root: &Path, path: &str) -> Option<String> {
    fs::read_to_string(root.join(path)).ok()
}

/// Missing settings are the defaults; a corrupt file is kept aside as `.bak`
/// instead of being overwritten.
pub fn load_settings(path: &Path) -> AppSettings {
    match fs::read_to_string(path) {
        Ok(raw) => serde_json::from_str(&raw).unwrap_or_else(|_| {
            let _ = fs::rename(path, path.with_extension("json.bak"));
            AppSettings::default()
        }),
        Err(_) => AppSettings::default(),
    }
}

pub fn store_settings(path: &Path, settings: &AppSettings) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, json).map_err(|e| e.to_string())?;
    fs::rename(&tmp, path).map_err(|e| format!("Could not save preferences: {e}"))
}
