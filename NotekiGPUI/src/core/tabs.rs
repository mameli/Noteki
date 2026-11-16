//! Tabs with their own Back/Forward history, and the stack of closed tabs.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tab {
    pub path: String,
    /// Cursor byte offset in the note, restored when the tab comes back.
    pub cursor: usize,
    back: Vec<(String, usize)>,
    forward: Vec<(String, usize)>,
}

impl Tab {
    fn new(path: &str) -> Self {
        Self {
            path: path.to_string(),
            cursor: 0,
            back: Vec::new(),
            forward: Vec::new(),
        }
    }

    pub fn can_go_back(&self) -> bool {
        !self.back.is_empty()
    }

    pub fn can_go_forward(&self) -> bool {
        !self.forward.is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenIn {
    /// Replace the active tab's note, recording it in that tab's history.
    CurrentTab,
    /// Open a new tab after the active one.
    NewTab,
}

pub const MAX_CLOSED_TABS: usize = 20;
const MAX_HISTORY: usize = 100;

#[derive(Debug, Clone, Default)]
pub struct Tabs {
    tabs: Vec<Tab>,
    active: Option<usize>,
    closed: Vec<String>,
}

impl Tabs {
    pub fn len(&self) -> usize {
        self.tabs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tabs.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Tab> {
        self.tabs.iter()
    }

    pub fn active_index(&self) -> Option<usize> {
        self.active
    }

    pub fn active(&self) -> Option<&Tab> {
        self.active.and_then(|i| self.tabs.get(i))
    }

    pub fn active_path(&self) -> Option<&str> {
        self.active().map(|t| t.path.as_str())
    }

    pub fn contains(&self, path: &str) -> bool {
        self.tabs.iter().any(|t| t.path == path)
    }

    pub fn set_active_cursor(&mut self, cursor: usize) {
        if let Some(t) = self.active.and_then(|i| self.tabs.get_mut(i)) {
            t.cursor = cursor;
        }
    }

    /// Shows `path`. If a tab already shows it, that tab becomes active.
    /// Returns the active index.
    pub fn open(&mut self, path: &str, mode: OpenIn) -> usize {
        if let Some(i) = self.tabs.iter().position(|t| t.path == path) {
            self.active = Some(i);
            return i;
        }
        match (mode, self.active) {
            (OpenIn::CurrentTab, Some(i)) => {
                let tab = &mut self.tabs[i];
                let old = std::mem::replace(&mut tab.path, path.to_string());
                tab.back.push((old, tab.cursor));
                if tab.back.len() > MAX_HISTORY {
                    tab.back.remove(0);
                }
                tab.forward.clear();
                tab.cursor = 0;
                i
            }
            _ => {
                let at = self.active.map_or(self.tabs.len(), |i| i + 1);
                self.tabs.insert(at, Tab::new(path));
                self.active = Some(at);
                at
            }
        }
    }

    pub fn activate(&mut self, index: usize) -> bool {
        if index < self.tabs.len() {
            self.active = Some(index);
            true
        } else {
            false
        }
    }

    /// Closes a tab; returns its path. The tab to the right (or left) becomes active.
    pub fn close(&mut self, index: usize) -> Option<String> {
        if index >= self.tabs.len() {
            return None;
        }
        let tab = self.tabs.remove(index);
        self.closed.push(tab.path.clone());
        if self.closed.len() > MAX_CLOSED_TABS {
            self.closed.remove(0);
        }
        self.active = match self.active {
            _ if self.tabs.is_empty() => None,
            Some(a) if a > index => Some(a - 1),
            Some(a) if a == index => Some(index.min(self.tabs.len() - 1)),
            other => other,
        };
        Some(tab.path)
    }

    /// Pops the last closed path (skipping ones that no longer exist).
    pub fn pop_closed(&mut self, exists: impl Fn(&str) -> bool) -> Option<String> {
        while let Some(path) = self.closed.pop() {
            if exists(&path) {
                return Some(path);
            }
        }
        None
    }

    pub fn next(&mut self) {
        if let (Some(a), n) = (self.active, self.tabs.len()) {
            self.active = Some((a + 1) % n);
        }
    }

    pub fn prev(&mut self) {
        if let (Some(a), n) = (self.active, self.tabs.len()) {
            self.active = Some((a + n - 1) % n);
        }
    }

    /// Goes back in the active tab's history; returns the path now shown.
    pub fn back(&mut self) -> Option<&Tab> {
        let tab = self.active.and_then(|i| self.tabs.get_mut(i))?;
        let (path, cursor) = tab.back.pop()?;
        let old = std::mem::replace(&mut tab.path, path);
        tab.forward
            .push((old, std::mem::replace(&mut tab.cursor, cursor)));
        self.active()
    }

    pub fn forward(&mut self) -> Option<&Tab> {
        let tab = self.active.and_then(|i| self.tabs.get_mut(i))?;
        let (path, cursor) = tab.forward.pop()?;
        let old = std::mem::replace(&mut tab.path, path);
        tab.back
            .push((old, std::mem::replace(&mut tab.cursor, cursor)));
        self.active()
    }

    /// A note or folder was renamed/moved: every tab, history entry and closed
    /// tab under `old` follows it to `new`.
    pub fn rename(&mut self, old: &str, new: &str) {
        let map = |p: &mut String| {
            if let Some(rest) = rename_target(p, old, new) {
                *p = rest;
            }
        };
        for t in &mut self.tabs {
            map(&mut t.path);
            t.back.iter_mut().for_each(|(p, _)| map(p));
            t.forward.iter_mut().for_each(|(p, _)| map(p));
        }
        self.closed.iter_mut().for_each(map);
    }

    /// A note or folder is gone: close its tabs and forget it in histories.
    /// Returns the closed paths.
    pub fn remove(&mut self, path: &str) -> Vec<String> {
        let gone = |p: &str| p == path || p.starts_with(&format!("{path}/"));
        let mut closed = Vec::new();
        while let Some(i) = self.tabs.iter().position(|t| gone(&t.path)) {
            if let Some(p) = self.close(i) {
                closed.push(p);
            }
        }
        for t in &mut self.tabs {
            t.back.retain(|(p, _)| !gone(p));
            t.forward.retain(|(p, _)| !gone(p));
        }
        self.closed.retain(|p| !gone(p));
        closed
    }
}

/// `path` after renaming `old` to `new`, if it is `old` or inside it.
pub fn rename_target(path: &str, old: &str, new: &str) -> Option<String> {
    if path == old {
        Some(new.to_string())
    } else {
        path.strip_prefix(old)
            .and_then(|rest| rest.strip_prefix('/'))
            .map(|rest| format!("{new}/{rest}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(t: &Tabs) -> Vec<&str> {
        t.iter().map(|t| t.path.as_str()).collect()
    }

    #[test]
    fn open_in_current_tab_records_history() {
        let mut t = Tabs::default();
        t.open("a.md", OpenIn::CurrentTab);
        t.set_active_cursor(7);
        t.open("b.md", OpenIn::CurrentTab);
        assert_eq!(paths(&t), vec!["b.md"]);
        assert_eq!(
            t.back().map(|t| (t.path.clone(), t.cursor)),
            Some(("a.md".into(), 7))
        );
        assert_eq!(t.forward().map(|t| t.path.clone()), Some("b.md".into()));
        assert!(t.forward().is_none());
    }

    #[test]
    fn open_existing_activates_it() {
        let mut t = Tabs::default();
        t.open("a.md", OpenIn::NewTab);
        t.open("b.md", OpenIn::NewTab);
        assert_eq!(t.open("a.md", OpenIn::CurrentTab), 0);
        assert_eq!(paths(&t), vec!["a.md", "b.md"]);
    }

    #[test]
    fn new_tab_goes_after_active() {
        let mut t = Tabs::default();
        t.open("a.md", OpenIn::NewTab);
        t.open("b.md", OpenIn::NewTab);
        t.activate(0);
        t.open("c.md", OpenIn::NewTab);
        assert_eq!(paths(&t), vec!["a.md", "c.md", "b.md"]);
        assert_eq!(t.active_index(), Some(1));
    }

    #[test]
    fn close_keeps_a_sensible_active_tab() {
        let mut t = Tabs::default();
        for p in ["a", "b", "c"] {
            t.open(p, OpenIn::NewTab);
        }
        t.activate(2);
        t.close(0);
        assert_eq!(t.active_path(), Some("c"));
        t.close(1);
        assert_eq!(t.active_path(), Some("b"));
        t.close(0);
        assert_eq!(t.active_index(), None);
        assert_eq!(t.pop_closed(|p| p != "b"), Some("c".into()));
        assert_eq!(t.pop_closed(|_| true), Some("a".into()));
    }

    #[test]
    fn cycling() {
        let mut t = Tabs::default();
        t.next(); // no tabs: no panic
        for p in ["a", "b"] {
            t.open(p, OpenIn::NewTab);
        }
        t.next();
        assert_eq!(t.active_path(), Some("a"));
        t.prev();
        assert_eq!(t.active_path(), Some("b"));
    }

    #[test]
    fn rename_and_remove_follow_folders() {
        let mut t = Tabs::default();
        t.open("F/a.md", OpenIn::NewTab);
        t.open("F/b.md", OpenIn::CurrentTab);
        t.open("Fx.md", OpenIn::NewTab);
        t.rename("F", "G");
        assert_eq!(paths(&t), vec!["G/b.md", "Fx.md"]);
        t.activate(0);
        assert_eq!(t.back().map(|t| t.path.clone()), Some("G/a.md".into()));
        assert_eq!(t.remove("G"), vec!["G/a.md".to_string()]);
        assert_eq!(paths(&t), vec!["Fx.md"]);
        assert_eq!(rename_target("Fx.md", "F", "G"), None);
    }
}
