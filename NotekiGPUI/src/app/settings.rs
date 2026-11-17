//! Preferences and per-vault session, stored as JSON.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::theme::{AppearanceMode, NotekiThemeName};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum LeftPane {
    #[default]
    Files,
    Tags,
    Search,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum RightPane {
    #[default]
    Outline,
    Backlinks,
}

/// What is restored when a vault is opened again.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct VaultSession {
    pub open_tabs: Vec<String>,
    pub active_tab: Option<String>,
    pub recent_notes: Vec<String>,
    pub expanded_folders: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub vault_path: Option<String>,
    pub recent_vaults: Vec<String>,
    pub sessions: HashMap<String, VaultSession>,
    pub left_sidebar_open: bool,
    pub right_sidebar_open: bool,
    pub left_pane: LeftPane,
    pub right_pane: RightPane,
    pub theme_name: NotekiThemeName,
    pub appearance: AppearanceMode,
    /// The text's size in points (SPEC › Window: 10 to 32, 15 by default).
    pub font_size: f32,
    /// The text's font family; empty is the system font.
    pub font_family: String,
    /// The code's font family (inline and in blocks); empty is the system's monospaced one.
    pub code_font: String,
    /// The longest line, in characters (half an em each): 40 to 200.
    pub line_length: u32,
    /// Whether lines are limited to `line_length` (kept while off).
    pub limit_line_length: bool,
    /// Folders left out of each Vault (by its root), by their path from it.
    pub excluded_folders: HashMap<String, Vec<String>>,
    pub daily_note_folder: String,
    /// moment.js format: `[Daily_]YYYY/YYYY-MM-DD` makes `Daily_2026/2026-11-17.md`.
    pub daily_note_format: String,
}

pub const DEFAULT_FONT_SIZE: f32 = 15.;
pub const MIN_FONT_SIZE: f32 = 10.;
pub const MAX_FONT_SIZE: f32 = 32.;
pub const DEFAULT_LINE_LENGTH: u32 = 100;
pub const MIN_LINE_LENGTH: u32 = 40;
pub const MAX_LINE_LENGTH: u32 = 200;

impl Settings {
    /// The text size a step larger (`+1`) or smaller (`-1`), within its limits.
    pub fn zoomed(&self, step: f32) -> f32 {
        (self.font_size + step).clamp(MIN_FONT_SIZE, MAX_FONT_SIZE)
    }

    /// The longest line in characters, if lines are limited.
    pub fn line_limit(&self) -> Option<u32> {
        self.limit_line_length
            .then(|| self.line_length.clamp(MIN_LINE_LENGTH, MAX_LINE_LENGTH))
    }

    /// The folders left out of the Vault at `root`.
    pub fn excluded_in(&self, root: &Path) -> &[String] {
        self.excluded_folders
            .get(&root.to_string_lossy().to_string())
            .map_or(&[], Vec::as_slice)
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            vault_path: None,
            recent_vaults: Vec::new(),
            sessions: HashMap::new(),
            left_sidebar_open: true,
            right_sidebar_open: true,
            left_pane: LeftPane::default(),
            right_pane: RightPane::default(),
            theme_name: NotekiThemeName::default(),
            appearance: AppearanceMode::default(),
            font_size: DEFAULT_FONT_SIZE,
            font_family: String::new(),
            code_font: String::new(),
            line_length: DEFAULT_LINE_LENGTH,
            limit_line_length: true,
            excluded_folders: HashMap::new(),
            daily_note_folder: "Journal".into(),
            daily_note_format: "[Daily_]YYYY/YYYY-MM-DD".into(),
        }
    }
}

/// The key a window's session is kept under: the Vault's root for the first
/// window, `root#n` for the others (ADR 0010: each window its own Tabs).
pub fn session_key(root: &str, slot: usize) -> String {
    if slot == 0 {
        root.to_string()
    } else {
        format!("{root}#{slot}")
    }
}

impl Settings {
    /// The slots of the other windows remembered for the Vault at `root`.
    pub fn other_slots(&self, root: &str) -> Vec<usize> {
        let prefix = format!("{root}#");
        let mut slots: Vec<usize> = self
            .sessions
            .keys()
            .filter_map(|k| k.strip_prefix(&prefix)?.parse().ok())
            .filter(|&n| n > 0)
            .collect();
        slots.sort_unstable();
        slots
    }

    /// What one window saves: its settings, with the sessions on `disk` (the
    /// other windows') and its own under `key`.
    pub fn merged_for_save(&self, disk: Settings, key: &str) -> Settings {
        let mut out = self.clone();
        out.sessions = disk.sessions;
        match self.sessions.get(key) {
            Some(mine) => out.sessions.insert(key.to_string(), mine.clone()),
            None => out.sessions.remove(key),
        };
        out
    }

    /// Takes the settings every window shares (SPEC › Window: they apply at
    /// once to every window's editor); sidebars and view stay per window.
    pub fn adopt_shared(&mut self, from: &Settings) {
        self.theme_name = from.theme_name;
        self.appearance = from.appearance;
        self.font_size = from.font_size;
        self.font_family = from.font_family.clone();
        self.code_font = from.code_font.clone();
        self.line_length = from.line_length;
        self.limit_line_length = from.limit_line_length;
        self.excluded_folders = from.excluded_folders.clone();
        self.daily_note_folder = from.daily_note_folder.clone();
        self.daily_note_format = from.daily_note_format.clone();
    }
}

pub const MAX_RECENT_NOTES: usize = 40;
pub const MAX_RECENT_VAULTS: usize = 8;

/// `~/Library/Application Support/Noteki/gpui-settings.json`
pub fn default_settings_path() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join("Noteki").join("gpui-settings.json"))
}

/// Missing or unreadable settings fall back to defaults; a corrupt file is
/// kept aside as `.bak` rather than silently overwritten.
pub fn load(path: Option<&Path>) -> Settings {
    let Some(path) = path else {
        return Settings::default();
    };
    match std::fs::read_to_string(path) {
        Ok(raw) => serde_json::from_str(&raw).unwrap_or_else(|_| {
            let _ = std::fs::rename(path, path.with_extension("json.bak"));
            Settings::default()
        }),
        Err(_) => Settings::default(),
    }
}

pub fn save(path: Option<&Path>, settings: &Settings) -> Result<(), String> {
    let Some(path) = path else {
        return Ok(());
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, json).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_corrupt_file() {
        let dir = std::env::temp_dir().join(format!("noteki-settings-{}", std::process::id()));
        let path = dir.join("s.json");
        let mut s = Settings {
            vault_path: Some("/v".into()),
            ..Default::default()
        };
        s.sessions.insert(
            "/v".into(),
            VaultSession {
                open_tabs: vec!["a.md".into()],
                ..Default::default()
            },
        );
        save(Some(&path), &s).unwrap();
        assert_eq!(load(Some(&path)), s);

        std::fs::write(&path, "{ not json").unwrap();
        assert_eq!(load(Some(&path)), Settings::default());
        assert!(path.with_extension("json.bak").exists());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn old_files_with_missing_fields_load() {
        let s: Settings =
            serde_json::from_str(r#"{"vault_path":"/x","view_mode":"reader"}"#).unwrap();
        assert_eq!(
            s.vault_path.as_deref(),
            Some("/x"),
            "an old field is ignored"
        );
        assert!(s.left_sidebar_open);
    }

    #[test]
    fn text_size_steps_within_its_limits_and_is_remembered() {
        let mut s = Settings::default();
        assert_eq!(s.font_size, 15.);
        s.font_size = 31.;
        assert_eq!(s.zoomed(1.), 32.);
        assert_eq!(s.zoomed(5.), 32.);
        s.font_size = 10.;
        assert_eq!(s.zoomed(-1.), 10.);
        let old: Settings = serde_json::from_str("{}").unwrap();
        assert_eq!(old.font_size, 15., "a file from before keeps the default");
    }

    #[test]
    fn each_window_saves_its_own_session_and_keeps_the_others() {
        let tabs = |p: &str| VaultSession {
            open_tabs: vec![p.into()],
            ..Default::default()
        };
        let mut disk = Settings::default();
        disk.sessions.insert(session_key("/v", 0), tabs("a.md"));
        disk.sessions.insert(session_key("/v", 2), tabs("b.md"));
        // The second window, whose copy of the first window's session is stale.
        let mut mine = Settings::default();
        mine.sessions.insert("/v".into(), tabs("old.md"));
        mine.sessions.insert(session_key("/v", 2), tabs("c.md"));
        mine.font_size = 20.;
        let out = mine.merged_for_save(disk.clone(), "/v#2");
        assert_eq!(out.sessions["/v"], tabs("a.md"), "the other window's kept");
        assert_eq!(out.sessions["/v#2"], tabs("c.md"), "its own written");
        assert_eq!(out.font_size, 20.);
        assert_eq!(disk.other_slots("/v"), vec![2]);
        assert!(disk.other_slots("/w").is_empty());
        mine.sessions.remove("/v#2");
        assert!(
            !mine
                .merged_for_save(disk, "/v#2")
                .sessions
                .contains_key("/v#2"),
            "a closed window forgotten"
        );
    }

    #[test]
    fn shared_settings_leave_the_window_layout_alone() {
        let mut a = Settings::default();
        let b = Settings {
            font_size: 22.,
            line_length: 60,
            left_sidebar_open: false,
            ..Default::default()
        };
        a.adopt_shared(&b);
        assert_eq!((a.font_size, a.line_length), (22., 60));
        assert!(a.left_sidebar_open);
    }

    #[test]
    fn text_settings_round_trip_and_the_line_limit_is_kept_while_off() {
        let dir = std::env::temp_dir().join(format!("noteki-settings-text-{}", std::process::id()));
        let path = dir.join("s.json");
        let mut s = Settings {
            font_family: "Georgia".into(),
            code_font: "Courier".into(),
            line_length: 72,
            daily_note_folder: "Diario".into(),
            ..Default::default()
        };
        s.excluded_folders
            .insert("/v".into(), vec!["Archivio".into()]);
        save(Some(&path), &s).unwrap();
        assert_eq!(load(Some(&path)), s);
        assert_eq!(s.excluded_in(Path::new("/v")), ["Archivio".to_string()]);
        assert!(s.excluded_in(Path::new("/w")).is_empty());
        assert_eq!(s.line_limit(), Some(72));
        s.limit_line_length = false;
        assert_eq!(s.line_limit(), None);
        assert_eq!(s.line_length, 72, "kept for when the limit is on again");
        s.limit_line_length = true;
        s.line_length = 500;
        assert_eq!(s.line_limit(), Some(200));
        let old: Settings = serde_json::from_str("{}").unwrap();
        assert_eq!(
            (old.line_limit(), old.daily_note_folder.as_str()),
            (Some(100), "Journal")
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}
