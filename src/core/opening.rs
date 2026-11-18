//! A file or folder the system asks Noteki to open (the Finder, Takku, `open`):
//! where it goes (SPEC › Takku).

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Opened {
    /// A Note or Attachment of the Vault: shown in a Tab.
    File(String),
    /// A folder equal to or inside the Vault: selected in Files.
    Folder(String),
    /// A Markdown or text file outside the Vault: a Loose file's window.
    Loose(PathBuf),
    /// A folder outside the Vault: asks whether to make it the Vault.
    OtherFolder(PathBuf),
    /// Anything else (a file in an Excluded folder, another kind of file):
    /// the window only comes forward.
    Nothing,
}

/// A `file://` URL as the platform gives it, as a path.
pub fn url_to_path(url: &str) -> Option<PathBuf> {
    let rest = url.strip_prefix("file://")?;
    let rest = rest.strip_prefix("localhost").unwrap_or(rest);
    Some(PathBuf::from(crate::core::markdown::percent_decode(rest)))
}

/// Where `path` goes, for a Vault at `root` whose listed files and folders
/// `listed` knows (`Some(is_folder)`).
pub fn route(root: Option<&Path>, path: &Path, listed: impl Fn(&str) -> Option<bool>) -> Opened {
    if let Some(rel) = root.and_then(|r| path.strip_prefix(r).ok()) {
        let rel = rel.to_string_lossy().trim_end_matches('/').to_string();
        if rel.is_empty() {
            return Opened::Folder(String::new());
        }
        return match listed(&rel) {
            Some(true) => Opened::Folder(rel),
            Some(false) => Opened::File(rel),
            None => Opened::Nothing,
        };
    }
    let text = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| matches!(e.to_lowercase().as_str(), "md" | "markdown" | "txt"));
    if path.is_dir() {
        Opened::OtherFolder(path.to_path_buf())
    } else if text && path.is_file() {
        Opened::Loose(path.to_path_buf())
    } else {
        Opened::Nothing
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_vault_files_folders_and_loose_files() {
        let root = Path::new("/v");
        let listed = |p: &str| match p {
            "Ideas.md" => Some(false),
            "Daily" => Some(true),
            _ => None,
        };
        assert_eq!(
            route(Some(root), Path::new("/v/Ideas.md"), listed),
            Opened::File("Ideas.md".into())
        );
        assert_eq!(
            route(Some(root), Path::new("/v/Daily"), listed),
            Opened::Folder("Daily".into())
        );
        assert_eq!(
            route(Some(root), Path::new("/v"), listed),
            Opened::Folder(String::new())
        );
        assert_eq!(
            route(Some(root), Path::new("/v/.obsidian/x.md"), listed),
            Opened::Nothing
        );
        let outside = std::env::temp_dir().join("noteki-loose-test.md");
        std::fs::write(&outside, "x").unwrap();
        assert_eq!(
            route(Some(root), &outside, listed),
            Opened::Loose(outside.clone())
        );
        assert_eq!(
            route(Some(root), Path::new("/tmp/x.pdf"), listed),
            Opened::Nothing
        );
        let _ = std::fs::remove_file(outside);
        let dir = std::env::temp_dir();
        assert_eq!(
            route(Some(root), &dir, listed),
            Opened::OtherFolder(dir.clone())
        );
        assert_eq!(route(None, &dir, listed), Opened::OtherFolder(dir.clone()));
        assert_eq!(
            url_to_path("file:///Users/a/My%20Note.md"),
            Some(PathBuf::from("/Users/a/My Note.md"))
        );
    }
}
