//! The Settings window (⌘,): every setting applied at once to the Vault's
//! window, which keeps and saves them (SPEC › Window).

use gpui_kit::base::Disableable as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::setting::{
    NumberFieldOptions, SettingField, SettingGroup, SettingItem, SettingPage,
    Settings as SettingsView,
};
use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _, Sizable as _, TitleBar};
use gpui_kit::{
    div, px, size, AnyWindowHandle, App, AppContext as _, Bounds, Context, Entity, IntoElement,
    ParentElement as _, Render, SharedString, Styled as _, Window, WindowBounds, WindowOptions,
};

use super::settings::{self, Settings};
use super::NotekiApp;
use crate::core::vault::{folder_to_exclude, today_daily_note_path};
use crate::theme::{AppearanceMode, NotekiThemeName};

pub struct SettingsWindow {
    app: Entity<NotekiApp>,
    /// The Vault's window, where a change is applied.
    main: AnyWindowHandle,
    /// `(family, label)` of every installed font, the system's first.
    fonts: Vec<(SharedString, SharedString)>,
    /// Why the last folder chosen cannot be left out.
    error: Option<String>,
}

impl SettingsWindow {
    pub fn open(
        app: Entity<NotekiApp>,
        main: AnyWindowHandle,
        cx: &mut App,
    ) -> Option<AnyWindowHandle> {
        let mut names = cx.text_system().all_font_names();
        names.retain(|n| !n.starts_with('.'));
        names.sort_by_key(|n| n.to_lowercase());
        names.dedup();
        let fonts = std::iter::once((SharedString::default(), SharedString::from("System")))
            .chain(names.into_iter().map(|n| (n.clone().into(), n.into())))
            .collect();
        let bounds = Bounds::centered(None, size(px(760.), px(620.)), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitleBar::title_bar_options()),
            ..Default::default()
        };
        gpui_kit::open_window(options, cx, |_, cx| {
            cx.new(|_| Self {
                app,
                main,
                fonts,
                error: None,
            })
        })
        .ok()
        .map(|(handle, _)| handle)
    }
}

/// Applies `change` in the Vault's window, after the event that asked it.
fn change(
    app: &Entity<NotekiApp>,
    main: AnyWindowHandle,
    cx: &mut App,
    change: impl FnOnce(&mut Settings) + 'static,
) {
    let app = app.clone();
    cx.defer(move |cx| {
        let _ = main.update(cx, |_, window, cx| {
            app.update(cx, |a, cx| a.update_settings(change, window, cx))
        });
    });
}

/// A setting read from and written to the Vault's window.
macro_rules! field {
    ($this:ident, $kind:ident, |$s:ident| $get:expr, |$t:ident, $v:ident| $set:expr) => {{
        let (app, main) = ($this.app.clone(), $this.main);
        let read = $this.app.clone();
        SettingField::$kind(
            move |cx: &App| {
                let $s = read.read(cx).settings();
                $get
            },
            move |$v, cx: &mut App| change(&app, main, cx, move |$t: &mut Settings| $set),
        )
    }};
    ($this:ident, $kind:ident($($arg:expr),*), |$s:ident| $get:expr, |$t:ident, $v:ident| $set:expr) => {{
        let (app, main) = ($this.app.clone(), $this.main);
        let read = $this.app.clone();
        SettingField::$kind(
            $($arg),*,
            move |cx: &App| {
                let $s = read.read(cx).settings();
                $get
            },
            move |$v, cx: &mut App| change(&app, main, cx, move |$t: &mut Settings| $set),
        )
    }};
}

impl SettingsWindow {
    fn appearance(&self) -> SettingPage {
        let themes = [
            NotekiThemeName::Ayu,
            NotekiThemeName::Everforest,
            NotekiThemeName::OneDarkPro,
        ];
        let modes = [
            AppearanceMode::System,
            AppearanceMode::Light,
            AppearanceMode::Dark,
        ];
        let options = |labels: Vec<&'static str>| {
            labels
                .into_iter()
                .map(|l| (SharedString::from(l), SharedString::from(l)))
                .collect::<Vec<_>>()
        };
        SettingPage::new("Appearance").group(
            SettingGroup::new()
                .item(SettingItem::new(
                    "Theme",
                    field!(
                        self,
                        dropdown(options(themes.iter().map(|t| t.label()).collect())),
                        |s| s.theme_name.label().into(),
                        |t, v| if let Some(n) = themes.iter().find(|n| n.label() == v.as_ref()) {
                            t.theme_name = *n
                        }
                    ),
                ))
                .item(SettingItem::new(
                    "Appearance",
                    field!(
                        self,
                        dropdown(options(modes.iter().map(|m| m.label()).collect())),
                        |s| s.appearance.label().into(),
                        |t, v| if let Some(m) = modes.iter().find(|m| m.label() == v.as_ref()) {
                            t.appearance = *m
                        }
                    ),
                )),
        )
    }

    fn text(&self) -> SettingPage {
        let fonts = self.fonts.clone();
        SettingPage::new("Text").group(
            SettingGroup::new()
                .item(SettingItem::new(
                    "Font",
                    field!(
                        self,
                        scrollable_dropdown(fonts.clone()),
                        |s| s.font_family.clone().into(),
                        |t, v| t.font_family = v.to_string()
                    ),
                ))
                .item(
                    SettingItem::new(
                        "Code font",
                        field!(
                            self,
                            scrollable_dropdown(fonts),
                            |s| s.code_font.clone().into(),
                            |t, v| t.code_font = v.to_string()
                        ),
                    )
                    .description("Inline and in blocks. A fixed-width family reads best."),
                )
                .item(SettingItem::new(
                    "Text size",
                    field!(
                        self,
                        number_input(NumberFieldOptions {
                            min: settings::MIN_FONT_SIZE as f64,
                            max: settings::MAX_FONT_SIZE as f64,
                            step: 1.,
                        }),
                        |s| s.font_size as f64,
                        |t, v| t.font_size =
                            (v as f32).clamp(settings::MIN_FONT_SIZE, settings::MAX_FONT_SIZE)
                    ),
                ))
                .item(SettingItem::new(
                    "Limit the line length",
                    field!(self, switch, |s| s.limit_line_length, |t, v| t
                        .limit_line_length =
                        v),
                ))
                .item(
                    SettingItem::new(
                        "Line length",
                        field!(
                            self,
                            number_input(NumberFieldOptions {
                                min: settings::MIN_LINE_LENGTH as f64,
                                max: settings::MAX_LINE_LENGTH as f64,
                                step: 5.,
                            }),
                            |s| s.line_length as f64,
                            |t, v| t.line_length = (v as u32)
                                .clamp(settings::MIN_LINE_LENGTH, settings::MAX_LINE_LENGTH)
                        ),
                    )
                    .description("In characters, 40 to 200."),
                ),
        )
    }

    fn excluded(&self, cx: &mut Context<Self>) -> SettingPage {
        let this = cx.entity();
        let item = SettingItem::render(move |_, _, cx| {
            let me = this.read(cx);
            let (app, main) = (me.app.clone(), me.main);
            let error = me.error.clone();
            let a = app.read(cx);
            let root = a.vault().map(|v| v.root.clone());
            let folders = root
                .as_deref()
                .map(|r| a.settings().excluded_in(r).to_vec())
                .unwrap_or_default();
            let muted = cx.theme().muted_foreground;
            let rows = folders.into_iter().enumerate().map(|(i, folder)| {
                let (app, main) = (app.clone(), main);
                let root = root.clone();
                h_flex()
                    .justify_between()
                    .gap_2()
                    .child(div().text_sm().child(folder.clone()))
                    .child(
                        Button::new(("excluded-remove", i))
                            .label("Remove")
                            .ghost()
                            .xsmall()
                            .on_click(move |_, _, cx| {
                                let (folder, root) = (folder.clone(), root.clone());
                                change(&app, main, cx, move |s| {
                                    if let Some(list) = root.and_then(|r| {
                                        s.excluded_folders.get_mut(&r.to_string_lossy().to_string())
                                    }) {
                                        list.retain(|f| f != &folder);
                                    }
                                })
                            }),
                    )
            });
            let add = {
                let this = this.clone();
                Button::new("excluded-add")
                    .label("Add folder…")
                    .small()
                    .disabled(root.is_none())
                    .on_click(move |_, _, cx| add_excluded(this.clone(), cx))
            };
            v_flex()
                .w_full()
                .gap_1()
                .children(rows)
                .child(h_flex().pt_1().child(add))
                .children(error.map(|e| div().text_xs().text_color(muted).child(e)))
        });
        SettingPage::new("Excluded folders").group(
            SettingGroup::new()
                .description(
                    "Left out of Files, search, Tags and links. Folders starting with a dot, \
                     node_modules, virtual environments and repositories are always left out.",
                )
                .item(item),
        )
    }

    fn daily_note(&self, cx: &App) -> SettingPage {
        let s = self.app.read(cx).settings();
        let today = today_daily_note_path(&s.daily_note_folder, &s.daily_note_format);
        SettingPage::new("Daily note").group(
            SettingGroup::new()
                .item(SettingItem::new(
                    "Folder",
                    field!(
                        self,
                        input,
                        |s| s.daily_note_folder.clone().into(),
                        |t, v| t.daily_note_folder = v.to_string()
                    ),
                ))
                .item(
                    SettingItem::new(
                        "Date format",
                        field!(
                            self,
                            input,
                            |s| s.daily_note_format.clone().into(),
                            |t, v| t.daily_note_format = v.to_string()
                        ),
                    )
                    .description(format!("Today: {today}")),
                ),
        )
    }
}

/// Asks for a folder of the Vault and leaves it out.
fn add_excluded(this: Entity<SettingsWindow>, cx: &mut App) {
    let rx = cx.prompt_for_paths(gpui_kit::PathPromptOptions {
        files: false,
        directories: true,
        multiple: false,
        prompt: Some("Leave Out".into()),
    });
    cx.spawn(async move |cx| {
        let Ok(Ok(Some(paths))) = rx.await else {
            return;
        };
        let Some(chosen) = paths.into_iter().next() else {
            return;
        };
        let _ = this.update(cx, |me, cx| {
            let Some(root) = me.app.read(cx).vault().map(|v| v.root.clone()) else {
                return;
            };
            let current = me.app.read(cx).settings().excluded_in(&root).to_vec();
            match folder_to_exclude(&root, &chosen, &current) {
                Ok(rel) => {
                    me.error = None;
                    change(&me.app, me.main, cx, move |s| {
                        s.excluded_folders
                            .entry(root.to_string_lossy().to_string())
                            .or_default()
                            .push(rel)
                    });
                }
                Err(e) => me.error = Some(e),
            }
            cx.notify();
        });
    })
    .detach();
}

impl Render for SettingsWindow {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pages = [
            self.appearance(),
            self.text(),
            self.excluded(cx),
            self.daily_note(cx),
        ];
        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .child(TitleBar::new().child(div().text_sm().child("Settings")))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .child(SettingsView::new("settings").pages(pages)),
            )
    }
}
