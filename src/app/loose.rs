//! A Loose file: a Markdown file outside the Vault, opened from the Finder,
//! in a window of its own with one Tab and no sidebars (SPEC › Takku). It is
//! saved like a Note (atomically, a second after the last change) and merged
//! with what another app wrote when the window comes back to the front.

use std::path::PathBuf;

use gpui_kit::component::TitleBar;
use gpui_kit::{
    div, px, size, AppContext as _, Bounds, Context, Entity, IntoElement, ParentElement as _,
    Render, Styled as _, Subscription, Task, Window, WindowBounds, WindowOptions,
};

use super::{Settings, AUTOSAVE_DELAY};
use crate::core::document::{ExternalOutcome, NoteDoc};
use crate::core::vault::write_note_atomic;
use crate::editor::{NoteEditor, NoteEditorEvent};
use crate::theme::NotekiPalette;

pub struct LooseFile {
    path: PathBuf,
    doc: NoteDoc,
    editor: Entity<NoteEditor>,
    autosave: Option<Task<()>>,
    error: Option<String>,
    _subscriptions: Vec<Subscription>,
}

impl LooseFile {
    /// Opens `path` in a new window; an error when it cannot be read.
    pub fn open(
        path: PathBuf,
        palette: NotekiPalette,
        settings: &Settings,
        cx: &mut gpui_kit::App,
    ) -> Result<(), String> {
        let settings = settings.clone();
        let text = std::fs::read_to_string(&path)
            .map_err(|e| format!("Cannot open {}: {e}", path.display()))?;
        let bounds = Bounds::centered(None, size(px(820.), px(760.)), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitleBar::title_bar_options()),
            app_id: Some(super::APP_ID.into()),
            window_min_size: Some(size(px(420.), px(300.))),
            ..Default::default()
        };
        gpui_kit::open_window(options, cx, |window, cx| {
            cx.new(|cx| Self::new(path, text, palette, &settings, window, cx))
        })
        .map(|_| ())
        .map_err(|e| e.to_string())
    }

    fn new(
        path: PathBuf,
        text: String,
        palette: NotekiPalette,
        settings: &Settings,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let key = path.to_string_lossy().to_string();
        let editor = cx.new(|cx| {
            let mut ed = NoteEditor::new(palette, window, cx);
            ed.set_font_size(px(settings.font_size), cx);
            ed.set_text_settings(
                &settings.font_family,
                &settings.code_font,
                settings.line_limit(),
                cx,
            );
            ed.open_note(&key, &text, 0, cx);
            ed
        });
        editor.update(cx, |ed, cx| ed.focus(window, cx));
        let subscriptions = vec![
            cx.subscribe_in(
                &editor,
                window,
                |this, _, ev: &NoteEditorEvent, window, cx| {
                    if let NoteEditorEvent::Changed = ev {
                        let text = this.editor.read(cx).text().to_string();
                        if this.doc.edit(&text) {
                            this.schedule_save(window, cx);
                        }
                    }
                },
            ),
            cx.observe_window_activation(window, |this, window, cx| {
                if window.is_window_active() {
                    this.merge_from_disk(cx);
                } else {
                    this.save(cx);
                }
            }),
            cx.on_release(|this, _| {
                let _ = this.save_now();
            }),
        ];
        Self {
            path,
            doc: NoteDoc::new(text),
            editor,
            autosave: None,
            error: None,
            _subscriptions: subscriptions,
        }
    }

    fn schedule_save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.autosave = Some(cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(AUTOSAVE_DELAY).await;
            let _ = this.update(cx, |this, cx| this.save(cx));
        }));
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        self.error = self.save_now().err();
        cx.notify();
    }

    fn save_now(&mut self) -> Result<(), String> {
        if !self.doc.should_save() {
            return Ok(());
        }
        let (Some(dir), Some(name)) = (self.path.parent(), self.path.file_name()) else {
            return Err("Invalid path".into());
        };
        let text = self.doc.text().to_string();
        write_note_atomic(dir, &name.to_string_lossy(), &text)?;
        self.doc.mark_saved(&text);
        Ok(())
    }

    /// What another app wrote meanwhile, merged as for a Note.
    fn merge_from_disk(&mut self, cx: &mut Context<Self>) {
        let disk = std::fs::read_to_string(&self.path).ok();
        match self.doc.on_disk_changed(disk.as_deref()) {
            outcome @ (ExternalOutcome::Reloaded | ExternalOutcome::Merged) => {
                let text = self.doc.text().to_string();
                self.editor.update(cx, |ed, cx| {
                    let cursor = ed.cursor().min(text.len());
                    ed.set_text(&text, cursor, cx);
                });
                if outcome == ExternalOutcome::Merged {
                    self.save(cx);
                }
            }
            ExternalOutcome::Conflict => {
                self.error = Some("Changed on disk and here: your text is kept".into())
            }
            ExternalOutcome::Deleted => {
                self.error = Some("Deleted by another app: your text is kept".into())
            }
            ExternalOutcome::Unchanged | ExternalOutcome::Synced => {}
        }
        cx.notify();
    }
}

impl Render for LooseFile {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        use gpui_kit::component::ActiveTheme as _;
        let name = self
            .path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let title = match &self.error {
            Some(e) => format!("{name} — {e}"),
            None => name,
        };
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(cx.theme().background)
            .child(TitleBar::new().child(div().text_sm().child(title)))
            .child(div().flex_1().min_h_0().child(self.editor.clone()))
    }
}
