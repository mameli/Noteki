//! The Swift app's preferences, taken once: this Noteki replaces it under the
//! same bundle id, so whoever updates finds their Vault, theme and text as they
//! left them. Read from its defaults (`app.noteki.noteki`) the first time
//! there are no settings of this app's own.

use serde_json::Value;

use super::settings::{
    Settings, VaultSession, MAX_FONT_SIZE, MAX_LINE_LENGTH, MAX_RECENT_NOTES, MIN_FONT_SIZE,
    MIN_LINE_LENGTH,
};

/// The Swift app's preferences as settings, or `None` when it left none
/// (never used, or not on a Mac).
pub fn import() -> Option<Settings> {
    #[cfg(target_os = "macos")]
    {
        let plist = defaults_export()?;
        from_swift(|key| value(&plist, key))
    }
    #[cfg(not(target_os = "macos"))]
    None
}

/// The preferences as an XML plist, through cfprefsd (the file may lag).
#[cfg(target_os = "macos")]
fn defaults_export() -> Option<Vec<u8>> {
    let out = std::process::Command::new("/usr/bin/defaults")
        .args(["export", "app.noteki.noteki", "-"])
        .output()
        .ok()?;
    out.status.success().then_some(out.stdout)
}

/// One key of the plist, as JSON: dates and data, which JSON lacks, are only
/// in keys left alone.
#[cfg(target_os = "macos")]
fn value(plist: &[u8], key: &str) -> Option<Value> {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let mut child = Command::new("/usr/bin/plutil")
        .args(["-extract", key, "json", "-o", "-", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    child.stdin.take()?.write_all(plist).ok()?;
    let out = child.wait_with_output().ok()?;
    if !out.status.success() {
        return None;
    }
    serde_json::from_slice(&out.stdout).ok()
}

/// The settings the Swift app's keys give, `get` reading one: `None` when no
/// Vault was ever chosen there.
pub fn from_swift(get: impl Fn(&str) -> Option<Value>) -> Option<Settings> {
    let string = |key: &str| get(key).and_then(|v| v.as_str().map(str::to_string));
    let number = |key: &str| get(key).and_then(|v| v.as_f64());
    let flag = |key: &str| get(key).and_then(|v| v.as_bool());
    // Path → strings, the path as Swift's URLs write a folder's (a slash at the end).
    let lists = |key: &str| {
        get(key)
            .and_then(|v| v.as_object().cloned())
            .into_iter()
            .flatten()
            .filter_map(|(root, list)| {
                let list: Vec<String> = serde_json::from_value(list).ok()?;
                Some((folder(&root), list))
            })
            .collect::<Vec<_>>()
    };

    let vault = folder(&string("vaultPath")?);
    let mut s = Settings {
        vault_path: Some(vault.clone()),
        recent_vaults: vec![vault],
        ..Settings::default()
    };
    if let Some(theme) = get("theme").and_then(|v| serde_json::from_value(v).ok()) {
        s.theme_name = theme;
    }
    if let Some(appearance) = get("appearance").and_then(|v| serde_json::from_value(v).ok()) {
        s.appearance = appearance;
    }
    if let Some(size) = number("textSize") {
        s.font_size = (size as f32).clamp(MIN_FONT_SIZE, MAX_FONT_SIZE);
    }
    if let Some(font) = string("textFont") {
        s.font_family = font;
    }
    if let Some(font) = string("codeFont") {
        s.code_font = font;
    }
    if let Some(length) = number("lineLength") {
        s.line_length = (length.round() as u32).clamp(MIN_LINE_LENGTH, MAX_LINE_LENGTH);
    }
    if let Some(limit) = flag("limitsLineLength") {
        s.limit_line_length = limit;
    }
    if let Some(open) = flag("rightSidebar") {
        s.right_sidebar_open = open;
    }
    if let Some(pane) = get("leftPane").and_then(|v| serde_json::from_value(v).ok()) {
        s.left_pane = pane;
    }
    if let Some(pane) = get("rightPane").and_then(|v| serde_json::from_value(v).ok()) {
        s.right_pane = pane;
    }
    if let Some(folder) = string("dailyNoteFolder") {
        s.daily_note_folder = folder;
    }
    if let Some(format) = string("dailyNoteFormat") {
        s.daily_note_format = format;
    }
    if let Some(on) = flag("focusMode") {
        s.focus_mode = on;
    }
    if let Some(on) = flag("typewriterMode") {
        s.typewriter_mode = on;
    }
    if let Some(size) = get("interfaceSize").and_then(|v| serde_json::from_value(v).ok()) {
        s.interface_size = size;
    }
    // WebKit kept them in the app's preferences.
    if let Some(on) = flag("WebContinuousSpellCheckingEnabled") {
        s.check_spelling = on;
    }
    if let Some(on) = flag("WebAutomaticSpellingCorrectionEnabled") {
        s.correct_spelling = on;
    }
    if let Some(on) = flag("showsFormatBar") {
        s.show_format_bar = on;
    }
    s.excluded_folders = lists("excludedFolders").into_iter().collect();
    for (root, mut notes) in lists("recentNotes") {
        notes.truncate(MAX_RECENT_NOTES);
        s.sessions.insert(
            root,
            VaultSession {
                recent_notes: notes,
                ..VaultSession::default()
            },
        );
    }
    Some(s)
}

/// A folder's path as this app keys it: no slash at the end.
fn folder(path: &str) -> String {
    match path.trim_end_matches('/') {
        "" => "/".to_string(),
        p => p.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::settings::{LeftPane, RightPane};
    use crate::theme::{AppearanceMode, NotekiThemeName};
    use serde_json::json;

    #[test]
    fn the_swift_apps_vault_and_text_carry_over() {
        let prefs = json!({
            "vaultPath": "/Users/me/Vault/",
            "theme": "one-dark-pro",
            "appearance": "dark",
            "textSize": 17.0,
            "codeFont": "Menlo",
            "lineLength": 200.0,
            "limitsLineLength": false,
            "leftPane": "tags",
            "rightPane": "backlinks",
            "rightSidebar": false,
            "dailyNoteFolder": "Diario",
            "typewriterMode": true,
            "showsFormatBar": true,
            "interfaceSize": "large",
            "WebAutomaticSpellingCorrectionEnabled": false,
            "excludedFolders": {"/Users/me/Vault/": ["Archive"]},
            "recentNotes": {"/Users/me/Vault/": ["a.md", "b/c.md"]},
            "windows": "data the import leaves",
        });
        let s = from_swift(|k| prefs.get(k).cloned()).expect("a Vault");
        assert_eq!(s.vault_path.as_deref(), Some("/Users/me/Vault"));
        assert_eq!(s.recent_vaults, ["/Users/me/Vault"]);
        assert_eq!(s.theme_name, NotekiThemeName::OneDarkPro);
        assert_eq!(s.appearance, AppearanceMode::Dark);
        assert_eq!(s.font_size, 17.);
        assert_eq!(s.code_font, "Menlo");
        assert_eq!(s.font_family, "", "the system font, as there");
        assert_eq!((s.line_length, s.limit_line_length), (200, false));
        assert_eq!(
            (s.left_pane, s.right_pane),
            (LeftPane::Tags, RightPane::Backlinks)
        );
        assert!(!s.right_sidebar_open);
        assert_eq!(s.daily_note_folder, "Diario");
        assert!(s.typewriter_mode && !s.focus_mode && s.show_format_bar);
        assert_eq!(s.interface_size, crate::app::settings::InterfaceSize::Large);
        assert!(s.check_spelling && !s.correct_spelling, "on unless turned off there");
        assert_eq!(s.daily_note_format, "[Daily_]YYYY/YYYY-MM-DD");
        assert_eq!(
            s.excluded_in(std::path::Path::new("/Users/me/Vault")),
            ["Archive"]
        );
        assert_eq!(
            s.sessions["/Users/me/Vault"].recent_notes,
            ["a.md", "b/c.md"]
        );
    }

    #[test]
    fn without_a_vault_there_is_nothing_to_take() {
        let prefs = json!({"theme": "ayu", "SUHasLaunchedBefore": true});
        assert!(from_swift(|k| prefs.get(k).cloned()).is_none());
    }

    #[test]
    fn values_out_of_range_or_unknown_are_left_at_their_defaults() {
        let prefs = json!({"vaultPath": "/V", "theme": "solarized", "textSize": 90.0});
        let s = from_swift(|k| prefs.get(k).cloned()).unwrap();
        assert_eq!(s.theme_name, NotekiThemeName::default());
        assert_eq!(s.font_size, MAX_FONT_SIZE);
    }
}
