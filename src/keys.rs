//! Shortcuts written once, as on the Mac, and given on Linux as its apps have
//! them: Ctrl where the Mac has ⌘, words with Ctrl and lines with Home and
//! End, none of macOS's own (hiding the app, the Emacs keys of its text
//! views, which on Linux are the command keys). Shown the same way: `⌘O` on
//! the Mac, `Ctrl+O` on Linux.

use gpui_kit::{Action, KeyBinding};

/// A binding written as on the Mac, given on this system (or none).
pub fn binding<A: Action>(
    mac: &'static str,
    action: A,
    context: Option<&str>,
) -> Option<KeyBinding> {
    Some(KeyBinding::new(here(mac)?, action, context))
}

/// A binding that is another key on Linux than [`linux`] would make it.
pub fn binding_os<A: Action>(
    mac: &'static str,
    linux: &'static str,
    action: A,
    context: Option<&str>,
) -> KeyBinding {
    KeyBinding::new(
        if cfg!(target_os = "macos") {
            mac
        } else {
            linux
        },
        action,
        context,
    )
}

/// `mac` on the Mac, its Linux keystroke elsewhere (kept for the app's life).
fn here(mac: &'static str) -> Option<&'static str> {
    if cfg!(target_os = "macos") {
        return Some(mac);
    }
    let keys = linux(mac)?;
    Some(if keys == mac {
        mac
    } else {
        Box::leak(keys.into_boxed_str())
    })
}

/// The Linux keystroke of a Mac one, its modifiers in any order: `None` for
/// the Mac's own.
pub fn linux(mac: &str) -> Option<String> {
    let mac = canonical(mac);
    let mac = mac.as_str();
    let keys = match mac {
        // Hiding the app, minimizing, the character palette: macOS's.
        "cmd-h" | "alt-cmd-h" | "cmd-m" | "ctrl-cmd-space" => return None,
        // The Emacs keys of macOS's text views: Ctrl is the command key here.
        "ctrl-a" | "ctrl-e" | "ctrl-b" | "ctrl-f" | "ctrl-p" | "ctrl-n" | "ctrl-k" | "ctrl-d" => {
            return None
        }
        // A line's ends are Home and End, bound as well.
        "cmd-left" | "cmd-right" | "cmd-shift-left" | "cmd-shift-right" => return None,
        // Words with Ctrl, the Note's ends with Ctrl+Home and Ctrl+End.
        "alt-left" => "ctrl-left",
        "alt-right" => "ctrl-right",
        "alt-shift-left" => "ctrl-shift-left",
        "alt-shift-right" => "ctrl-shift-right",
        "alt-backspace" => "ctrl-backspace",
        "alt-delete" => "ctrl-delete",
        "cmd-backspace" => "ctrl-shift-backspace",
        "cmd-delete" => "ctrl-shift-delete",
        "cmd-up" => "ctrl-home",
        "cmd-down" => "ctrl-end",
        "cmd-shift-up" => "ctrl-shift-home",
        "cmd-shift-down" => "ctrl-shift-end",
        // Back and forward as in a browser; Tabs as in one too.
        "alt-cmd-left" => "alt-left",
        "alt-cmd-right" => "alt-right",
        "cmd-}" => "ctrl-pagedown",
        "cmd-{" => "ctrl-pageup",
        // Replace and help as Linux's apps have them.
        "alt-cmd-f" => "ctrl-h",
        "cmd-?" => "f1",
        // The right sidebar and its panes: Alt and a digit.
        "alt-cmd-0" => "alt-0",
        "alt-cmd-1" => "alt-1",
        "alt-cmd-2" => "alt-2",
        _ => {
            return Some(if let Some(rest) = mac.strip_prefix("ctrl-cmd-") {
                format!("ctrl-alt-{rest}")
            } else {
                mac.replace("cmd-", "ctrl-")
            })
        }
    };
    Some(keys.to_string())
}

/// A keystroke with its modifiers as the table above writes them: ctrl, alt,
/// cmd, shift, then the key (`shift-alt-left` is `alt-shift-left`).
fn canonical(keys: &str) -> String {
    let parts: Vec<&str> = keys.split('-').collect();
    let (key, mods) = match parts.split_last() {
        // `cmd--`: the key is the minus.
        Some((&"", rest)) if rest.last() == Some(&"") => ("-", &rest[..rest.len() - 1]),
        Some((key, rest)) => (*key, rest),
        None => return keys.to_string(),
    };
    let mut out = String::new();
    for m in ["ctrl", "alt", "cmd", "shift"] {
        if mods.contains(&m) {
            out.push_str(m);
            out.push('-');
        }
    }
    if mods
        .iter()
        .any(|m| !["ctrl", "alt", "cmd", "shift"].contains(m))
    {
        return keys.to_string();
    }
    out.push_str(key);
    out
}

/// A shortcut as the Mac's menus show it (`⌥⌘←`), shown on this system.
pub fn label(mac: &'static str) -> String {
    if cfg!(target_os = "macos") {
        return mac.to_string();
    }
    linux_label(mac)
}

/// A tooltip naming its shortcut in brackets (`Quick switcher (⌘O)`), shown
/// on this system.
pub fn tooltip(text: &'static str) -> String {
    match (text.rfind('('), text.ends_with(')')) {
        (Some(open), true) if !cfg!(target_os = "macos") => {
            format!(
                "{}({})",
                &text[..open],
                linux_label(&text[open + 1..text.len() - 1])
            )
        }
        _ => text.to_string(),
    }
}

/// `⇧⌘O` as Linux shows it: `Ctrl+Shift+O`; the Mac's own as written.
pub fn linux_label(mac: &str) -> String {
    // The Mac shows ⌃⌥⇧⌘; the bindings are written ctrl-, alt-, cmd-, shift-.
    let mut rest = mac;
    let mut has = |symbol: &str| match rest.strip_prefix(symbol) {
        Some(r) => {
            rest = r;
            true
        }
        None => false,
    };
    let (ctrl, alt, shift, cmd) = (has("⌃"), has("⌥"), has("⇧"), has("⌘"));
    if !(ctrl || alt || shift || cmd) {
        return mac.to_string();
    }
    let mut keys = String::new();
    for (on, name) in [
        (ctrl, "ctrl-"),
        (alt, "alt-"),
        (cmd, "cmd-"),
        (shift, "shift-"),
    ] {
        if on {
            keys.push_str(name);
        }
    }
    let key = match rest {
        "←" => "left",
        "→" => "right",
        "↑" => "up",
        "↓" => "down",
        "⌫" => "backspace",
        "↩" => "enter",
        k => k,
    }
    .to_lowercase();
    let Some(linux) = self::linux(&format!("{keys}{key}")) else {
        return mac.to_string();
    };
    linux
        .split('-')
        .map(|part| match part {
            "ctrl" => "Ctrl".to_string(),
            "alt" => "Alt".to_string(),
            "shift" => "Shift".to_string(),
            "pagedown" => "PgDn".to_string(),
            "pageup" => "PgUp".to_string(),
            "left" => "←".to_string(),
            "right" => "→".to_string(),
            k => k.to_uppercase(),
        })
        .collect::<Vec<_>>()
        .join("+")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_command_key_is_ctrl_on_linux() {
        assert_eq!(linux("cmd-o").as_deref(), Some("ctrl-o"));
        assert_eq!(linux("cmd-shift-p").as_deref(), Some("ctrl-shift-p"));
        assert_eq!(linux("ctrl-cmd-s").as_deref(), Some("ctrl-alt-s"));
        assert_eq!(linux("ctrl-tab").as_deref(), Some("ctrl-tab"));
        assert_eq!(linux("alt-left").as_deref(), Some("ctrl-left"), "a word");
        assert_eq!(linux("cmd-alt-left").as_deref(), Some("alt-left"), "back");
        assert_eq!(linux("ctrl-a"), None, "Ctrl+A selects all there");
        assert_eq!(linux("cmd-h"), None);
        assert_eq!(
            linux("shift-alt-left").as_deref(),
            Some("ctrl-shift-left"),
            "any order"
        );
        assert_eq!(linux("cmd-alt-left").as_deref(), Some("alt-left"));
        assert_eq!(linux("cmd--").as_deref(), Some("ctrl--"));
        assert_eq!(linux("cmd-shift-=").as_deref(), Some("ctrl-shift-="));
    }

    #[test]
    fn labels_follow_the_keys() {
        assert_eq!(linux_label("⌘O"), "Ctrl+O");
        assert_eq!(linux_label("⇧⌘P"), "Ctrl+Shift+P");
        assert_eq!(linux_label("⌃⌘1"), "Ctrl+Alt+1");
        assert_eq!(linux_label("⌥⌘←"), "Alt+←");
        assert_eq!(linux_label("⌥⌘0"), "Alt+0");
        assert_eq!(linux_label("⌘,"), "Ctrl+,");
        assert_eq!(linux_label("⌘G"), "Ctrl+G");
        assert_eq!(linux_label("Files"), "Files", "no shortcut, as written");
    }
}
