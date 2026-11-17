use chrono::Local;
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

/// The path from `root` of a folder the user chose to leave out (SPEC ›
/// Vault: inside the Vault and not already left out), or why it cannot be.
pub fn folder_to_exclude(
    root: &Path,
    chosen: &Path,
    excluded: &[String],
) -> Result<String, String> {
    let chosen = fs::canonicalize(chosen).unwrap_or_else(|_| chosen.to_path_buf());
    let rel = chosen
        .strip_prefix(root)
        .map_err(|_| "Choose a folder inside the Vault".to_string())?
        .to_string_lossy()
        .to_string();
    if rel.is_empty() || !chosen.is_dir() {
        return Err("Choose a folder inside the Vault".into());
    }
    if is_in_excluded_folder(root, &rel, &normalize_excluded(excluded)) {
        return Err(format!("{rel} is already left out"));
    }
    Ok(rel)
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
            if ext == NOTE_EXT {
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
        if !exists_case_insensitive(root, &rel) {
            return rel;
        }
        index += 1;
    }
}

pub fn next_untitled_folder_path(root: &Path, parent_rel: &str) -> String {
    let mut index = 0usize;
    loop {
        let name = if index == 0 {
            "Untitled".to_string()
        } else {
            format!("Untitled {index}")
        };
        let rel = if parent_rel.is_empty() {
            name
        } else {
            format!("{parent_rel}/{name}")
        };
        if !exists_case_insensitive(root, &rel) {
            return rel;
        }
        index += 1;
    }
}

pub fn create_folder(root: &Path, rel_path: &str) -> Result<(), String> {
    let abs = resolve_vault_path(root, rel_path)?;
    fs::create_dir_all(&abs).map_err(|e| format!("Could not create folder: {e}"))
}

pub fn validate_item_name(name: &str) -> Result<(), String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("Name cannot be empty".to_string());
    }
    if trimmed.starts_with('.') {
        return Err("Name cannot start with a dot".to_string());
    }
    if CODE_FOLDERS.contains(&trimmed) {
        return Err("Name matches an excluded folder".to_string());
    }
    if trimmed
        .chars()
        .any(|c| matches!(c, '/' | '\\' | ':' | '[' | ']' | '#' | '^' | '|'))
    {
        return Err("Name contains unsupported characters (/ \\ : [ ] # ^ |)".to_string());
    }
    Ok(())
}

pub fn rename_item(
    root: &Path,
    old_rel: &str,
    new_stem_or_folder_name: &str,
    is_folder: bool,
) -> Result<String, String> {
    validate_item_name(new_stem_or_folder_name)?;
    let trimmed = new_stem_or_folder_name.trim();
    let old_abs = resolve_vault_path(root, old_rel)?;
    let parent_rel = old_rel.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
    let new_file_name = if is_folder {
        trimmed.to_string()
    } else {
        let ext = Path::new(old_rel)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("md");
        format!("{trimmed}.{ext}")
    };
    let new_rel = if parent_rel.is_empty() {
        new_file_name
    } else {
        format!("{parent_rel}/{new_file_name}")
    };
    if new_rel == old_rel {
        return Ok(new_rel);
    }
    let new_abs = resolve_vault_path(root, &new_rel)?;
    if !old_rel.eq_ignore_ascii_case(&new_rel) && exists_case_insensitive(root, &new_rel) {
        return Err(format!("Already exists: {new_rel}"));
    }
    fs::rename(&old_abs, &new_abs).map_err(|e| format!("Could not rename: {e}"))?;
    Ok(new_rel)
}

pub fn make_copy(root: &Path, rel_path: &str) -> Result<String, String> {
    let abs = resolve_vault_path(root, rel_path)?;
    if !abs.is_file() {
        return Err("Only files can be copied".to_string());
    }
    let parent_rel = rel_path.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
    let file_path = Path::new(rel_path);
    let stem = file_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Note");
    let ext = file_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("md");
    let mut index = 1usize;
    loop {
        let candidate_name = format!("{stem} {index}.{ext}");
        let candidate_rel = if parent_rel.is_empty() {
            candidate_name
        } else {
            format!("{parent_rel}/{candidate_name}")
        };
        if !exists_case_insensitive(root, &candidate_rel) {
            let dest = resolve_vault_path(root, &candidate_rel)?;
            fs::copy(&abs, &dest).map_err(|e| format!("Could not copy file: {e}"))?;
            return Ok(candidate_rel);
        }
        index += 1;
    }
}

/// Moves a file or folder of the vault to the Trash (`~/.Trash` unless
/// `trash_dir` is given). Never deletes: if it cannot be moved, it stays.
pub fn move_to_trash(root: &Path, rel_path: &str, trash_dir: Option<&Path>) -> Result<(), String> {
    let abs = resolve_vault_path(root, rel_path)?;
    if !abs.exists() {
        return Ok(());
    }
    let trash_dir = trash_dir
        .map(Path::to_path_buf)
        .or_else(|| dirs::home_dir().map(|h| h.join(".Trash")));
    if let Some(trash_dir) = trash_dir {
        if trash_dir.is_dir() {
            let file_name = abs.file_name().and_then(|s| s.to_str()).unwrap_or("item");
            let mut dest = trash_dir.join(file_name);
            if dest.exists() {
                let stamp = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                dest = trash_dir.join(format!("{file_name}.{stamp}"));
            }
            return fs::rename(&abs, &dest)
                .map_err(|e| format!("Could not move {rel_path} to the Trash: {e}"));
        }
    }
    Err(format!("No Trash folder: {rel_path} was not moved"))
}

pub fn today_daily_note_path(folder: &str, moment_format: &str) -> String {
    let now = Local::now();
    let fmt = if moment_format.trim().is_empty() {
        "[Daily_]YYYY/YYYY-MM-DD"
    } else {
        moment_format.trim()
    };
    let formatted = format_moment_date(&now, fmt);
    let folder = folder.trim().trim_matches('/');
    let mut rel = if folder.is_empty() {
        formatted
    } else {
        format!("{folder}/{formatted}")
    };
    if !rel.ends_with(".md") {
        rel.push_str(".md");
    }
    rel
}

fn format_moment_date(dt: &chrono::DateTime<Local>, fmt: &str) -> String {
    let mut out = String::new();
    let chars: Vec<char> = fmt.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '[' {
            i += 1;
            while i < chars.len() && chars[i] != ']' {
                out.push(chars[i]);
                i += 1;
            }
            if i < chars.len() && chars[i] == ']' {
                i += 1;
            }
            continue;
        }
        if chars[i] == '\\' && i + 1 < chars.len() {
            out.push(chars[i + 1]);
            i += 2;
            continue;
        }
        let rest: String = chars[i..].iter().collect();
        if rest.starts_with("YYYY") {
            out.push_str(&dt.format("%Y").to_string());
            i += 4;
        } else if rest.starts_with("YY") {
            out.push_str(&dt.format("%y").to_string());
            i += 2;
        } else if rest.starts_with("MMMM") {
            out.push_str(&dt.format("%B").to_string());
            i += 4;
        } else if rest.starts_with("MMM") {
            out.push_str(&dt.format("%b").to_string());
            i += 3;
        } else if rest.starts_with("MM") {
            out.push_str(&dt.format("%m").to_string());
            i += 2;
        } else if rest.starts_with('M') {
            out.push_str(&dt.format("%-m").to_string());
            i += 1;
        } else if rest.starts_with("DD") {
            out.push_str(&dt.format("%d").to_string());
            i += 2;
        } else if rest.starts_with('D') {
            out.push_str(&dt.format("%-d").to_string());
            i += 1;
        } else if rest.starts_with("dddd") {
            out.push_str(&dt.format("%A").to_string());
            i += 4;
        } else if rest.starts_with("ddd") {
            out.push_str(&dt.format("%a").to_string());
            i += 3;
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}

fn exists_case_insensitive(root: &Path, rel_path: &str) -> bool {
    let abs = root.join(rel_path);
    if abs.exists() {
        return true;
    }
    let Some(parent) = abs.parent() else {
        return false;
    };
    let Some(target_name) = abs.file_name().and_then(|s| s.to_str()) else {
        return false;
    };
    let Ok(entries) = fs::read_dir(parent) else {
        return false;
    };
    entries.flatten().any(|entry| {
        entry
            .file_name()
            .to_str()
            .is_some_and(|n| n.eq_ignore_ascii_case(target_name))
    })
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
    #[test]
    fn a_folder_to_leave_out_is_inside_the_vault_and_not_left_out_yet() {
        let root = std::env::temp_dir().join(format!("noteki-exclude-{}", std::process::id()));
        std::fs::create_dir_all(root.join("Archivio/Vecchio")).unwrap();
        std::fs::create_dir_all(root.join(".obsidian")).unwrap();
        let root = std::fs::canonicalize(&root).unwrap();
        assert_eq!(
            super::folder_to_exclude(&root, &root.join("Archivio"), &[]),
            Ok("Archivio".into())
        );
        assert!(
            super::folder_to_exclude(&root, &root, &[]).is_err(),
            "the Vault itself"
        );
        assert!(
            super::folder_to_exclude(&root, &std::env::temp_dir(), &[]).is_err(),
            "outside"
        );
        assert!(super::folder_to_exclude(&root, &root.join(".obsidian"), &[]).is_err());
        let ex = vec!["archivio".to_string()];
        assert!(
            super::folder_to_exclude(&root, &root.join("Archivio/Vecchio"), &ex).is_err(),
            "inside one left out, any case"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    use super::*;

    #[test]
    fn scans_notes_and_ignores_excluded() {
        let dir = std::env::temp_dir().join(format!(
            "noteki-gpui-vault-test-{}",
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
            "noteki-gpui-write-test-{}",
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
