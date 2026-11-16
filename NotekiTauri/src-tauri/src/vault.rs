use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const NOTE_EXT: &str = "md";
const CODE_FOLDERS: &[&str] = &["node_modules", "venv", "__pycache__"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AttachmentKind {
    Image,
    Pdf,
}

impl AttachmentKind {
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_ascii_lowercase().as_str() {
            "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "heic" => Some(Self::Image),
            "pdf" => Some(Self::Pdf),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum VaultItem {
    Folder {
        name: String,
        path: String,
        children: Vec<VaultItem>,
    },
    Note {
        name: String,
        path: String,
    },
    Attachment {
        name: String,
        path: String,
        #[serde(rename = "attachmentKind")]
        attachment_kind: AttachmentKind,
    },
}

impl VaultItem {
    pub fn path(&self) -> &str {
        match self {
            VaultItem::Folder { path, .. }
            | VaultItem::Note { path, .. }
            | VaultItem::Attachment { path, .. } => path,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            VaultItem::Folder { name, .. }
            | VaultItem::Note { name, .. }
            | VaultItem::Attachment { name, .. } => name,
        }
    }
}

pub fn normalize_excluded(excluded: &[String]) -> HashSet<String> {
    excluded
        .iter()
        .map(|s| s.trim_matches('/').to_lowercase())
        .filter(|s| !s.is_empty())
        .collect()
}

pub fn is_excluded_folder(
    folder_path_on_disk: &Path,
    rel_path: &str,
    excluded: &HashSet<String>,
) -> bool {
    let name = folder_path_on_disk
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    if name.starts_with('.') || CODE_FOLDERS.contains(&name) {
        return true;
    }
    if excluded.contains(&rel_path.to_lowercase()) {
        return true;
    }
    folder_path_on_disk.join(".git").exists()
}

pub fn is_in_excluded_folder(root: &Path, rel_path: &str, excluded: &HashSet<String>) -> bool {
    let parts: Vec<&str> = rel_path.split('/').filter(|s| !s.is_empty()).collect();
    if parts.is_empty() {
        return false;
    }
    let mut current_disk = root.to_path_buf();
    let mut current_rel = String::new();
    // Check all parent directories (and if it's a directory itself, callers can check directly).
    for (idx, part) in parts.iter().enumerate() {
        current_disk.push(part);
        if !current_rel.is_empty() {
            current_rel.push('/');
        }
        current_rel.push_str(part);
        let is_last = idx + 1 == parts.len();
        if !is_last || current_disk.is_dir() {
            if is_excluded_folder(&current_disk, &current_rel, excluded) {
                return true;
            }
        } else if part.starts_with('.') {
            return true;
        }
    }
    false
}

fn compare_names(a: &str, b: &str) -> std::cmp::Ordering {
    a.to_lowercase()
        .cmp(&b.to_lowercase())
        .then_with(|| a.cmp(b))
}

pub fn scan_vault(root: &Path, excluded: &HashSet<String>) -> Result<Vec<VaultItem>, String> {
    scan_dir(root, "", excluded)
}

fn scan_dir(
    dir: &Path,
    rel_prefix: &str,
    excluded: &HashSet<String>,
) -> Result<Vec<VaultItem>, String> {
    let entries = fs::read_dir(dir).map_err(|e| format!("Failed to read vault folder: {e}"))?;
    let mut folders = Vec::new();
    let mut files = Vec::new();

    for entry in entries.flatten() {
        let file_name = entry.file_name();
        let Some(name) = file_name.to_str() else {
            continue;
        };
        let rel_path = if rel_prefix.is_empty() {
            name.to_string()
        } else {
            format!("{rel_prefix}/{name}")
        };
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        let is_dir = if file_type.is_symlink() {
            path.is_dir()
        } else {
            file_type.is_dir()
        };

        if is_dir {
            if is_excluded_folder(&path, &rel_path, excluded) {
                continue;
            }
            if let Ok(children) = scan_dir(&path, &rel_path, excluded) {
                folders.push(VaultItem::Folder {
                    name: name.to_string(),
                    path: rel_path,
                    children,
                });
            }
        } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if ext.eq_ignore_ascii_case(NOTE_EXT) && ext == NOTE_EXT {
                let stem = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or(name)
                    .to_string();
                files.push(VaultItem::Note {
                    name: stem,
                    path: rel_path,
                });
            } else if let Some(kind) = AttachmentKind::from_extension(ext) {
                files.push(VaultItem::Attachment {
                    name: name.to_string(),
                    path: rel_path,
                    attachment_kind: kind,
                });
            }
        }
    }

    folders.sort_by(|a, b| compare_names(a.name(), b.name()));
    files.sort_by(|a, b| compare_names(a.name(), b.name()));
    folders.extend(files);
    Ok(folders)
}

pub fn resolve_vault_path(root: &Path, rel_path: &str) -> Result<PathBuf, String> {
    let clean = rel_path.trim_start_matches('/');
    let rel = Path::new(clean);
    if rel.as_os_str().is_empty() {
        return Err("Empty path".to_string());
    }
    for comp in rel.components() {
        match comp {
            Component::Normal(_) => {}
            _ => return Err(format!("Invalid vault path: {rel_path}")),
        }
    }
    Ok(root.join(rel))
}

pub fn read_note(root: &Path, rel_path: &str) -> Result<String, String> {
    let abs = resolve_vault_path(root, rel_path)?;
    fs::read_to_string(&abs).map_err(|e| format!("Could not read {rel_path}: {e}"))
}

pub fn write_note_atomic(root: &Path, rel_path: &str, content: &str) -> Result<(), String> {
    let abs = resolve_vault_path(root, rel_path)?;
    let parent = abs
        .parent()
        .ok_or_else(|| format!("Invalid path {rel_path}"))?;
    fs::create_dir_all(parent).map_err(|e| format!("Could not create folder: {e}"))?;

    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let file_name = abs
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("note.md");
    let tmp = parent.join(format!(".{file_name}.{stamp}.noteki.tmp"));

    {
        let mut f = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)
            .map_err(|e| format!("Could not open temp file: {e}"))?;
        if let Err(e) = f.write_all(content.as_bytes()).and_then(|_| f.sync_all()) {
            let _ = fs::remove_file(&tmp);
            return Err(format!("Could not write note: {e}"));
        }
    }

    if let Err(e) = fs::rename(&tmp, &abs) {
        let _ = fs::remove_file(&tmp);
        return Err(format!("Could not replace note: {e}"));
    }
    Ok(())
}

pub fn new_note_path_for_link(
    root: &Path,
    target: &str,
    source_path: Option<&str>,
    excluded: &HashSet<String>,
) -> Result<String, String> {
    if target.ends_with('/') || target.contains(':') || target.contains('\\') {
        return Err("Invalid note name".to_string());
    }
    if let Some(ext) = Path::new(target).extension().and_then(|e| e.to_str()) {
        if AttachmentKind::from_extension(ext).is_some() {
            return Err("Target is an attachment".to_string());
        }
    }

    let source_folder = source_path
        .and_then(|p| p.rsplit_once('/').map(|(dir, _)| dir))
        .unwrap_or("");
    let start = if target.starts_with("./") || target.starts_with("../") || !target.contains('/') {
        if source_folder.is_empty() {
            target.to_string()
        } else {
            format!("{source_folder}/{target}")
        }
    } else {
        target.to_string()
    };

    let mut comps: Vec<&str> = Vec::new();
    for c in start.split('/').filter(|s| !s.is_empty() && *s != ".") {
        if c == ".." {
            if comps.pop().is_none() {
                return Err("Path escapes vault root".to_string());
            }
        } else {
            comps.push(c);
        }
    }
    if comps.is_empty() {
        return Err("Empty note name".to_string());
    }

    let mut path = comps.join("/");
    if let Some(stripped) = path
        .strip_suffix(".MD")
        .or_else(|| path.strip_suffix(".Md"))
        .or_else(|| path.strip_suffix(".mD"))
    {
        path = format!("{stripped}.md");
    } else if !path.ends_with(".md") {
        path.push_str(".md");
    }

    if is_in_excluded_folder(root, &path, excluded) {
        return Err("Target is inside an excluded folder".to_string());
    }
    Ok(path)
}

pub fn create_empty_note(root: &Path, rel_path: &str) -> Result<String, String> {
    let abs = resolve_vault_path(root, rel_path)?;
    if abs.exists() {
        return Err(format!("File already exists: {rel_path}"));
    }
    if let Some(parent) = abs.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Could not create folder: {e}"))?;
    }
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&abs)
        .map_err(|e| format!("Could not create note: {e}"))?;
    Ok(rel_path.to_string())
}

pub fn next_untitled_path(root: &Path, folder_rel: &str) -> String {
    let mut index = 0usize;
    loop {
        let name = if index == 0 {
            "Untitled.md".to_string()
        } else {
            format!("Untitled {index}.md")
        };
        let rel = if folder_rel.is_empty() {
            name
        } else {
            format!("{folder_rel}/{name}")
        };
        if !root.join(&rel).exists() {
            return rel;
        }
        index += 1;
    }
}

pub fn relative_path_from_disk(
    root: &Path,
    abs_path: &Path,
    excluded: &HashSet<String>,
) -> Option<String> {
    let root_canon = fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let abs_canon = if abs_path.exists() {
        fs::canonicalize(abs_path).unwrap_or_else(|_| abs_path.to_path_buf())
    } else if let (Some(parent), Some(name)) = (abs_path.parent(), abs_path.file_name()) {
        fs::canonicalize(parent)
            .map(|p| p.join(name))
            .unwrap_or_else(|_| abs_path.to_path_buf())
    } else {
        abs_path.to_path_buf()
    };

    let rel = abs_canon.strip_prefix(&root_canon).ok()?;
    let rel_str = rel
        .components()
        .filter_map(|c| match c {
            Component::Normal(s) => s.to_str(),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/");

    if rel_str.is_empty() || is_in_excluded_folder(&root_canon, &rel_str, excluded) {
        return None;
    }
    Some(rel_str)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scans_notes_and_ignores_excluded() {
        let dir = std::env::temp_dir().join(format!(
            "noteki-vault-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(dir.join(".obsidian")).unwrap();
        fs::create_dir_all(dir.join("node_modules/pkg")).unwrap();
        fs::create_dir_all(dir.join("SubRepo")).unwrap();
        fs::create_dir_all(dir.join("SubRepo/.git")).unwrap();
        fs::create_dir_all(dir.join("Meetings")).unwrap();

        fs::write(dir.join("Hello.md"), "# Hello").unwrap();
        fs::write(dir.join("Meetings/2026.md"), "# Meeting").unwrap();
        fs::write(dir.join("photo.png"), "png").unwrap();
        fs::write(dir.join("ignored.txt"), "txt").unwrap();
        fs::write(dir.join(".obsidian/config.md"), "no").unwrap();
        fs::write(dir.join("node_modules/pkg/readme.md"), "no").unwrap();
        fs::write(dir.join("SubRepo/README.md"), "no").unwrap();

        let tree = scan_vault(&dir, &HashSet::new()).unwrap();
        assert_eq!(tree.len(), 3);
        assert_eq!(tree[0].path(), "Meetings");
        assert_eq!(tree[1].path(), "Hello.md");
        assert_eq!(tree[2].path(), "photo.png");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn atomic_write_roundtrip() {
        let dir = std::env::temp_dir().join(format!(
            "noteki-write-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        write_note_atomic(&dir, "Folder/Note.md", "Line 1\nLine 2\n").unwrap();
        assert_eq!(
            read_note(&dir, "Folder/Note.md").unwrap(),
            "Line 1\nLine 2\n"
        );
        let _ = fs::remove_dir_all(&dir);
    }
}
