//! What follows an edit in the editor: the Note's document, the Outline,
//! the autosave and the redraw at a pause in typing.

use super::*;

impl NotekiApp {
    pub(super) fn on_editor_event(
        &mut self,
        _: &Entity<NoteEditor>,
        event: &NoteEditorEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            NoteEditorEvent::Changed => {}
            NoteEditorEvent::PasteImage { bytes, extension } => {
                let name =
                    attachment::pasted_image_name(chrono::Local::now().naive_local(), extension);
                let r = self.attach_bytes(&name, bytes, cx);
                self.report(r);
                return;
            }
            NoteEditorEvent::DropFiles(paths) => {
                let r = self.attach_files(paths, cx);
                self.report(r);
                return;
            }
            NoteEditorEvent::OpenLink { link, new_tab } => {
                match link {
                    Link::Url(url) => cx.open_url(url),
                    Link::Note(w) => {
                        self.follow_link(&w.target, w.heading.as_deref(), *new_tab, window, cx)
                    }
                }
                return;
            }
        }
        let Some(path) = self.editor_path.clone() else {
            return;
        };
        let text = self.editor_text(cx);
        let Some(doc) = self.docs.get_mut(&path) else {
            return;
        };
        if doc.edit(&text) {
            self.after_text_change(window, cx);
        }
    }

    pub(super) fn after_text_change(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.refresh_outline();
        self.schedule_autosave(window, cx);
        // The editor draws each key itself; the rest of the window (Outline,
        // the words in the status bar) is drawn again at a pause in typing,
        // not at every key: laying out the whole window took a quarter of it.
        self.typing_redraw = Some(cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(TYPING_PAUSE).await;
            let _ = this.update(cx, |_, cx| cx.notify());
        }));
    }

    pub(super) fn schedule_autosave(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Replacing the task cancels the previous one: saves happen after the last change.
        self.autosave = Some(cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(AUTOSAVE_DELAY).await;
            let _ = this.update(cx, |app, cx| {
                app.flush_all(cx);
                cx.notify();
            });
        }));
    }

    /// Writes the note if it has unsaved edits and may be saved. Returns whether it wrote.
    ///
    /// The file is read first: another app may have written it since Noteki
    /// last loaded or saved it, and the event telling so comes only after
    /// FS_SETTLE. Writing blind would erase that write (Takku's Summary,
    /// ADR 0005), so it is reconciled first, as an External change.
    pub fn save_note(&mut self, path: &str, cx: &mut Context<Self>) -> Result<bool, String> {
        let Some(vault) = self.vault.as_mut() else {
            return Ok(false);
        };
        let Some(doc) = self.docs.get_mut(path) else {
            return Ok(false);
        };
        if !doc.should_save() {
            return Ok(false);
        }
        let on_disk = std::fs::read_to_string(vault.root.join(path)).ok();
        if let Some(disk) = on_disk.filter(|d| d != doc.base()) {
            let outcome = doc.on_disk_changed(Some(&disk));
            if matches!(outcome, ExternalOutcome::Merged | ExternalOutcome::Reloaded)
                && self.editor_path.as_deref() == Some(path)
            {
                let text = doc.text().to_string();
                self.editor.update(cx, |ed, cx| {
                    let cursor = ed.cursor();
                    ed.set_text(&text, cursor, cx)
                });
            }
            if outcome == ExternalOutcome::Conflict {
                self.set_error(format!(
                    "{path} changed on disk and in Noteki: choose a version"
                ));
                return Ok(false);
            }
            let (Some(vault), Some(doc)) = (self.vault.as_mut(), self.docs.get_mut(path)) else {
                return Ok(false);
            };
            if !doc.should_save() {
                return Ok(false);
            }
            return Self::write_doc(vault, doc, path).map(|()| true);
        }
        Self::write_doc(vault, doc, path).map(|()| true)
    }

    fn write_doc(vault: &mut Vault, doc: &mut NoteDoc, path: &str) -> Result<(), String> {
        let text = doc.text().to_string();
        write_note_atomic(&vault.root, path, &text)?;
        doc.mark_saved(&text);
        if vault.corpus.get(path).is_some() {
            vault.corpus.set(path, &text);
            vault.tag_tree = build_tag_tree(&vault.corpus);
        }
        Ok(())
    }

    pub(super) fn save_logged(&mut self, path: &str, cx: &mut Context<Self>) {
        if let Err(e) = self.save_note(path, cx) {
            self.set_error(format!("Could not save {path}: {e}"));
        }
    }

    /// Saves every open note with unsaved edits.
    pub fn flush_all(&mut self, cx: &mut Context<Self>) {
        let paths: Vec<String> = self
            .docs
            .iter()
            .filter(|(_, d)| d.should_save())
            .map(|(p, _)| p.clone())
            .collect();
        for p in paths {
            self.save_logged(&p, cx);
        }
    }
}
