//! The user's terminal, to open on a folder of the Vault (SPEC › Files): the
//! first one installed of those people install first, Terminal otherwise.
//! Nothing is embedded in Noteki.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// A terminal app: its name for the menus, and how it is started.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Terminal {
    pub name: &'static str,
    program: PathBuf,
}

/// On the Mac, by app name: Ghostty, iTerm, Warp, WezTerm, kitty, Alacritty,
/// then Terminal, which every Mac has.
#[cfg(target_os = "macos")]
const PREFERRED: [(&str, &str); 7] = [
    ("Ghostty", "Ghostty.app"),
    ("iTerm", "iTerm.app"),
    ("Warp", "Warp.app"),
    ("WezTerm", "WezTerm.app"),
    ("kitty", "kitty.app"),
    ("Alacritty", "Alacritty.app"),
    ("Terminal", "Utilities/Terminal.app"),
];

/// On Linux, by command: `$TERMINAL` comes first, then these.
#[cfg(not(target_os = "macos"))]
const PREFERRED: [(&str, &str); 8] = [
    ("Ghostty", "ghostty"),
    ("kitty", "kitty"),
    ("Alacritty", "alacritty"),
    ("WezTerm", "wezterm"),
    ("Terminal", "gnome-terminal"),
    ("Konsole", "konsole"),
    ("Terminal", "x-terminal-emulator"),
    ("XTerm", "xterm"),
];

/// The first of `PREFERRED` that `exists` finds, looked for in `places`.
pub fn first_installed(places: &[PathBuf], exists: impl Fn(&Path) -> bool) -> Option<Terminal> {
    PREFERRED.iter().find_map(|&(name, file)| {
        places
            .iter()
            .map(|p| p.join(file))
            .find(|p| exists(p))
            .map(|program| Terminal { name, program })
    })
}

/// Where apps (or commands) are installed.
fn places() -> Vec<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        let mut p = vec![
            PathBuf::from("/Applications"),
            PathBuf::from("/System/Applications"),
        ];
        if let Some(home) = dirs::home_dir() {
            p.insert(1, home.join("Applications"));
        }
        p
    }
    #[cfg(not(target_os = "macos"))]
    {
        std::env::var_os("PATH")
            .map(|p| std::env::split_paths(&p).collect())
            .unwrap_or_default()
    }
}

/// The user's terminal, looked up once.
pub fn terminal() -> Option<&'static Terminal> {
    static TERMINAL: OnceLock<Option<Terminal>> = OnceLock::new();
    TERMINAL
        .get_or_init(|| {
            #[cfg(not(target_os = "macos"))]
            if let Some(own) = std::env::var_os("TERMINAL").map(PathBuf::from) {
                return Some(Terminal {
                    name: "Terminal",
                    program: own,
                });
            }
            first_installed(&places(), Path::exists)
        })
        .as_ref()
}

/// "Open in Ghostty": the name the menus give the command.
pub fn open_in_label() -> &'static str {
    static LABEL: OnceLock<String> = OnceLock::new();
    LABEL.get_or_init(|| format!("Open in {}", terminal().map_or("Terminal", |t| t.name)))
}

/// "Open Vault in Ghostty".
pub fn open_vault_label() -> &'static str {
    static LABEL: OnceLock<String> = OnceLock::new();
    LABEL.get_or_init(|| format!("Open Vault in {}", terminal().map_or("Terminal", |t| t.name)))
}

/// A new window of the terminal, in `folder`.
pub fn open(folder: &Path) -> std::io::Result<()> {
    let Some(t) = terminal() else {
        return Err(std::io::Error::other("no terminal found"));
    };
    #[cfg(target_os = "macos")]
    let child = std::process::Command::new("/usr/bin/open")
        .arg("-a")
        .arg(&t.program)
        .arg(folder)
        .spawn();
    #[cfg(not(target_os = "macos"))]
    let child = std::process::Command::new(&t.program)
        .current_dir(folder)
        .spawn();
    // Reaped in the background: `open` returns at once, a terminal stays.
    let mut child = child?;
    std::thread::spawn(move || child.wait());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_installed_of_the_preferred_terminals_wins() {
        let places = [PathBuf::from("/A"), PathBuf::from("/B")];
        let (first, second) = (PREFERRED[1].1, PREFERRED[3].1);
        let installed = [Path::new("/B").join(second), Path::new("/A").join(first)];
        let t = first_installed(&places, |p| installed.iter().any(|i| i == p)).unwrap();
        assert_eq!(t.name, PREFERRED[1].0);
        assert_eq!(t.program, Path::new("/A").join(first));
    }

    #[test]
    fn without_any_there_is_none() {
        assert!(first_installed(&[PathBuf::from("/A")], |_| false).is_none());
    }
}
