//! Settings changed from the window: sidebars, theme, text size, modes,
//! spelling, Excluded folders; saved at once.

use super::*;

impl NotekiApp {
    pub(super) fn persist(&mut self) {
        // Other windows write the same file: their sessions are kept as they are.
        let path = self.config.settings_path.as_deref();
        let out = match (path, &self.vault) {
            (Some(p), Some(v)) if p.exists() => {
                let key = self.session_key(&v.root);
                self.settings.merged_for_save(settings::load(Some(p)), &key)
            }
            _ => self.settings.clone(),
        };
        if let Err(e) = settings::save(path, &out) {
            self.set_error(format!("Could not save preferences: {e}"));
        }
    }

    pub(super) fn persist_session(&mut self) {
        let Some(vault) = &self.vault else { return };
        let key = self.session_key(&vault.root);
        let mut expanded: Vec<String> = self.expanded_folders.iter().cloned().collect();
        expanded.sort();
        let session = settings::VaultSession {
            open_tabs: self.tabs.iter().map(|t| t.path.clone()).collect(),
            active_tab: self.active_path().map(str::to_string),
            recent_notes: self.recent_notes.clone(),
            expanded_folders: expanded,
        };
        if self.settings.sessions.get(&key) != Some(&session) {
            self.settings.sessions.insert(key, session);
            self.persist();
        }
    }

    pub(crate) fn toggle_left_sidebar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.settings.left_sidebar_open = !self.settings.left_sidebar_open;
        self.keep_keyboard_off_search(window, cx);
        self.persist();
        cx.notify();
    }

    /// The search field leaving the window takes the keyboard with it, and the
    /// window's shortcuts would reach nothing: the window keeps it instead.
    pub(super) fn keep_keyboard_off_search(&self, window: &mut Window, cx: &mut Context<Self>) {
        let shown = self.settings.left_sidebar_open && self.settings.left_pane == LeftPane::Search;
        if !shown
            && self
                .search_input
                .focus_handle(cx)
                .contains_focused(window, cx)
        {
            window.focus(&self.focus_handle, cx);
        }
    }

    pub(crate) fn toggle_right_sidebar(&mut self, cx: &mut Context<Self>) {
        self.settings.right_sidebar_open = !self.settings.right_sidebar_open;
        self.persist();
        cx.notify();
    }

    /// Search shown, with the keyboard in its field.
    pub(crate) fn show_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.show_left_pane(LeftPane::Search, window, cx);
        self.search_input.update(cx, |s, cx| s.focus(window, cx));
    }

    pub(crate) fn show_left_pane(
        &mut self,
        pane: LeftPane,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.settings.left_sidebar_open = true;
        self.settings.left_pane = pane;
        self.keep_keyboard_off_search(window, cx);
        self.persist();
        cx.notify();
    }

    pub(crate) fn show_right_pane(&mut self, pane: RightPane, cx: &mut Context<Self>) {
        self.settings.right_sidebar_open = true;
        self.settings.right_pane = pane;
        self.persist();
        cx.notify();
    }

    pub(super) fn apply_theme(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.palette = apply_noteki_theme(
            self.settings.theme_name,
            self.settings.appearance,
            window,
            cx,
        );
        crate::theme::set_interface_size(self.settings.interface_size.rem(), cx);
        let palette = self.palette;
        self.editor.update(cx, |ed, cx| ed.set_palette(palette, cx));
        cx.notify();
    }

    /// ⌘,: the Settings window, brought forward when already open.
    pub fn open_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(handle) = self.settings_window {
            if handle.update(cx, |_, w, _| w.activate_window()).is_ok() {
                return;
            }
        }
        // Opened once this update is over: the new window reads the settings.
        let (app, main) = (cx.entity(), window.window_handle());
        cx.defer(move |cx| {
            let handle = settings_window::SettingsWindow::open(app.clone(), main, cx);
            app.update(cx, |a, _| a.settings_window = handle);
        });
    }

    /// Changes the settings (from the Settings window), saves and applies them
    /// at once: the theme, the editor's text, the Vault's Excluded folders.
    pub fn update_settings(
        &mut self,
        change: impl FnOnce(&mut Settings),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let before = self.settings.clone();
        change(&mut self.settings);
        self.apply_settings(&before, window, cx);
        self.persist(); // the settings are remembered
                        // The other windows take them too.
        let shared = self.settings.clone();
        for other in self.other_windows(cx) {
            let (handle, shared) = (other.read(cx).window, shared.clone());
            cx.defer(move |cx| {
                let _ = handle.update(cx, |_, window, cx| {
                    other.update(cx, |o, cx| {
                        let before = o.settings.clone();
                        o.settings.adopt_shared(&shared);
                        o.apply_settings(&before, window, cx);
                    })
                });
            });
        }
    }

    /// Applies what changed since `before`: the theme, the editor's text, the
    /// Vault's Excluded folders.
    pub(super) fn apply_settings(
        &mut self,
        before: &Settings,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let look = (self.settings.theme_name, self.settings.appearance);
        if look != (before.theme_name, before.appearance) {
            self.apply_theme(window, cx);
        } else if self.settings.interface_size != before.interface_size {
            crate::theme::set_interface_size(self.settings.interface_size.rem(), cx);
        }
        let s = &self.settings;
        let (family, code, limit) = (s.font_family.clone(), s.code_font.clone(), s.line_limit());
        let size = gpui_kit::px(
            s.font_size
                .clamp(settings::MIN_FONT_SIZE, settings::MAX_FONT_SIZE),
        );
        let modes = (s.focus_mode, s.typewriter_mode);
        let spelling = (s.check_spelling, s.correct_spelling);
        self.editor.update(cx, |ed, cx| {
            ed.set_font_size(size, cx);
            ed.set_text_settings(&family, &code, limit, cx);
            ed.set_modes(modes.0, modes.1, cx);
            ed.set_spelling(spelling.0, spelling.1, cx);
        });
        if modes != (before.focus_mode, before.typewriter_mode)
            || spelling != (before.check_spelling, before.correct_spelling)
            || s.show_format_bar != before.show_format_bar
        {
            menus::refresh(&self.settings, cx);
        }
        if let Some(root) = self.vault.as_ref().map(|v| v.root.clone()) {
            let excluded = self.settings.excluded_in(&root).to_vec();
            if excluded != before.excluded_in(&root) {
                self.exclude_folders(&excluded, window, cx);
            }
        }
        cx.notify();
    }

    /// Leaves `excluded` out of the Vault: the Tabs inside close, their Notes
    /// saved first, since no change on disk would reach them any more.
    pub(super) fn exclude_folders(
        &mut self,
        excluded: &[String],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(vault) = self.vault.as_mut() else {
            return;
        };
        vault.excluded = normalize_excluded(excluded);
        let root = vault.root.clone();
        let set = vault.excluded.clone();
        let inside = |app: &Self| {
            app.tabs
                .iter()
                .position(|t| crate::core::vault::is_in_excluded_folder(&root, &t.path, &set))
        };
        while let Some(ix) = inside(self) {
            self.close_tab(ix, window, cx);
        }
        self.refresh_vault(&HashSet::new(), cx);
        self.refresh_outline();
        self.persist_session();
    }

    /// ⌘+ (`step` 1), ⌘− (−1), ⌘0 (`None`): the text's size, remembered.
    pub(crate) fn zoom(&mut self, step: Option<f32>, window: &mut Window, cx: &mut Context<Self>) {
        let size = match step {
            Some(step) => self.settings.zoomed(step),
            None => settings::DEFAULT_FONT_SIZE,
        };
        self.update_settings(|s| s.font_size = size, window, cx);
    }

    /// Edit › Spelling and Grammar › Check Spelling While Typing.
    pub(crate) fn toggle_check_spelling(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.update_settings(|s| s.check_spelling = !s.check_spelling, window, cx);
    }

    /// Edit › Spelling and Grammar › Correct Spelling Automatically.
    pub(crate) fn toggle_correct_spelling(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.update_settings(|s| s.correct_spelling = !s.correct_spelling, window, cx);
    }

    /// View › Focus Mode, for every window.
    pub(crate) fn toggle_focus_mode(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.update_settings(|s| s.focus_mode = !s.focus_mode, window, cx);
    }

    /// View › Typewriter Mode, for every window.
    pub(crate) fn toggle_typewriter_mode(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.update_settings(|s| s.typewriter_mode = !s.typewriter_mode, window, cx);
    }

    pub(crate) fn cycle_theme(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.update_settings(|s| s.theme_name = s.theme_name.next(), window, cx);
    }

    pub(crate) fn cycle_appearance(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.update_settings(|s| s.appearance = s.appearance.next(), window, cx);
    }

    pub fn theme_name(&self) -> NotekiThemeName {
        self.settings.theme_name
    }
}
