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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ViewMode {
    #[default]
    Live,
    Split,
    Reader,
}

impl ViewMode {
    pub fn next(self) -> Self {
        match self {
            Self::Live => Self::Split,
            Self::Split => Self::Reader,
            Self::Reader => Self::Live,
        }
    }

    pub fn shows_reader(self) -> bool {
        !matches!(self, Self::Live)
    }
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub vault_path: Option<String>,
    pub recent_vaults: Vec<String>,
    pub sessions: HashMap<String, VaultSession>,
    pub left_sidebar_open: bool,
    pub right_sidebar_open: bool,
    pub left_pane: LeftPane,
    pub right_pane: RightPane,
    pub view_mode: ViewMode,
    pub theme_name: NotekiThemeName,
    pub appearance: AppearanceMode,
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
            view_mode: ViewMode::default(),
            theme_name: NotekiThemeName::default(),
            appearance: AppearanceMode::default(),
        }
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
        assert_eq!(s.view_mode, ViewMode::Reader);
        assert!(s.left_sidebar_open);
    }
}
