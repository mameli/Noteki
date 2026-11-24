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
    if path.is_dir() {
        Opened::OtherFolder(path.to_path_buf())
    } else if is_text(path) && path.is_file() {
        Opened::Loose(path.to_path_buf())
    } else {
        Opened::Nothing
    }
}

/// A Markdown or text file, by its extension: what opens as a Loose file.
fn is_text(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| matches!(e.to_lowercase().as_str(), "md" | "markdown" | "txt"))
}

/// What the `noteki` command does with one of its arguments, before handing
/// it to the app (SPEC › Opening from the terminal): the absolute path to
/// open, or why it is refused. `arg` is relative to `cwd`; `vault` is the
/// Vault the app opens. Anything in the Vault, a folder, or a Markdown or text
/// file goes; a Markdown or text file that is missing is created, empty, when
/// its folder exists; anything else outside the Vault is refused.
pub fn prepare_for_command(
    arg: &Path,
    cwd: &Path,
    vault: Option<&Path>,
) -> Result<PathBuf, String> {
    let shown = arg.display();
    let path = cwd.join(arg);
    if !path.exists() {
        if !is_text(&path) {
            return Err(format!("{shown}: no such file"));
        }
        let parent = path.parent().unwrap_or(cwd);
        if !parent.is_dir() {
            let typed = arg.parent().unwrap_or(arg);
            return Err(format!(
                "{shown}: the folder {} does not exist",
                typed.display()
            ));
        }
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|e| format!("{shown}: cannot create it: {e}"))?;
    }
    let path = std::fs::canonicalize(&path).unwrap_or(path);
    let vault = vault.map(|v| std::fs::canonicalize(v).unwrap_or_else(|_| v.to_path_buf()));
    let in_vault = vault.is_some_and(|v| path.starts_with(v));
    if in_vault || path.is_dir() || is_text(&path) {
        Ok(path)
    } else {
        Err(format!(
            "{shown}: Noteki opens Markdown and text files (.md, .markdown, .txt)"
        ))
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

    #[test]
    fn the_command_creates_missing_text_files_and_refuses_the_rest() {
        let dir = std::env::temp_dir().join(format!("noteki-command-{}", std::process::id()));
        let vault = dir.join("vault");
        std::fs::create_dir_all(vault.join("Daily")).unwrap();
        std::fs::write(dir.join("main.rs"), "fn main() {}").unwrap();
        std::fs::write(vault.join("photo.png"), "png").unwrap();
        let dir = std::fs::canonicalize(&dir).unwrap();
        let vault = dir.join("vault");
        let prepare = |arg: &str| prepare_for_command(Path::new(arg), &dir, Some(&vault));

        // A missing Markdown file is created, empty: outside the Vault and in it.
        assert_eq!(prepare("idea.md"), Ok(dir.join("idea.md")));
        assert_eq!(std::fs::read_to_string(dir.join("idea.md")).unwrap(), "");
        assert_eq!(
            prepare("vault/Daily/oggi.md"),
            Ok(vault.join("Daily/oggi.md"))
        );
        assert!(vault.join("Daily/oggi.md").is_file());
        // An existing one is left as it is.
        std::fs::write(dir.join("idea.md"), "testo").unwrap();
        assert_eq!(prepare("idea.md"), Ok(dir.join("idea.md")));
        assert_eq!(
            std::fs::read_to_string(dir.join("idea.md")).unwrap(),
            "testo"
        );
        // An absolute path, a folder, an Attachment of the Vault go.
        let abs = dir.join("idea.md");
        assert_eq!(prepare(abs.to_str().unwrap()), Ok(abs.clone()));
        assert_eq!(prepare("vault"), Ok(vault.clone()));
        assert_eq!(prepare("."), Ok(dir.clone()));
        assert_eq!(prepare("vault/photo.png"), Ok(vault.join("photo.png")));

        // Refused: another kind of file outside the Vault, a missing file
        // that is not text, a missing folder (none is created).
        assert!(prepare("main.rs")
            .unwrap_err()
            .contains("opens Markdown and text files"));
        assert!(prepare("missing.png").unwrap_err().contains("no such file"));
        assert_eq!(
            prepare("nuova/cartella/idea.md"),
            Err("nuova/cartella/idea.md: the folder nuova/cartella does not exist".into())
        );
        assert!(!dir.join("nuova").exists());
        // With no Vault, the same rules outside it.
        assert!(prepare_for_command(Path::new("vault/photo.png"), &dir, None).is_err());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
