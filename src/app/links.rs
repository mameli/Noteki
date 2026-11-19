//! Following a Wikilink or a link, and the links the editor completes.

use super::*;

impl NotekiApp {
    /// Follows `[[target#heading]]` from the active note: opens it, or creates
    /// an empty note when it resolves nowhere (SPEC › Wikilinks).
    pub fn follow_link(
        &mut self,
        target: &str,
        heading: Option<&str>,
        new_tab: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mode = if new_tab {
            OpenIn::NewTab
        } else {
            OpenIn::CurrentTab
        };
        let source = self.active_path().map(str::to_string);
        let Some(vault) = &self.vault else { return };
        if target.is_empty() {
            if let Some(h) = heading {
                self.jump_to_heading(h, window, cx);
            }
            return;
        }
        let resolved = vault
            .index
            .resolve(target, source.as_deref())
            .map(|r| match r {
                ResolvedItem::Note { path, .. } | ResolvedItem::Attachment { path, .. } => path,
            });
        let path = match resolved {
            Some(p) => p,
            None => match new_note_path_for_link(
                &vault.root,
                target,
                source.as_deref(),
                &vault.excluded,
            )
            .and_then(|p| create_empty_note(&vault.root, &p))
            {
                Ok(p) => {
                    self.refresh_vault(&HashSet::from([p.clone()]), cx);
                    p
                }
                Err(e) => {
                    self.set_error(format!("Cannot create “{target}”: {e}"));
                    cx.notify();
                    return;
                }
            },
        };
        if self.open_path(&path, mode, window, cx) {
            if let Some(h) = heading {
                self.jump_to_heading(h, window, cx);
            }
        }
    }

    pub(crate) fn follow_link_under_cursor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let ed = self.editor.read(cx);
        let (text, cursor) = (ed.text().to_string(), ed.cursor());
        if let Some(link) = markdown::wikilink_at(&text, cursor) {
            self.follow_link(&link.target, link.heading.as_deref(), false, window, cx);
        }
    }

    pub(super) fn jump_to_heading(
        &mut self,
        heading: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let wanted = heading.trim().to_lowercase();
        if let Some(line) = self
            .outline
            .iter()
            .find(|h| h.text.to_lowercase() == wanted)
            .map(|h| h.line)
        {
            self.jump_to_line(line, window, cx);
        }
    }

    pub fn jump_to_line(
        &mut self,
        line_one_based: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let line = line_one_based.saturating_sub(1);
        self.editor.update(cx, |ed, cx| {
            let lines = logical_lines(ed.text());
            let start = lines.get(line).or(lines.last()).map_or(0, |l| l.start);
            ed.set_selected_range(start..start, cx);
            ed.focus(window, cx);
        });
    }
}
