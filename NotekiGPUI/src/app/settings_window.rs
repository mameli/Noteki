//! The Settings window (⌘,): one page, as the Swift app's and System Settings
//! group it, under the app's icon, name and version. Every setting is applied
//! at once to the Vault's window, which keeps and saves them (SPEC › Window).

use std::sync::Arc;

use gpui_kit::base::Disableable as _;
use gpui_kit::component::button::{Button, ButtonGroup, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::slider::{Slider, SliderEvent, SliderState};
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{
    h_flex, v_flex, ActiveTheme as _, Selectable as _, Sizable as _, TitleBar,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    div, img, px, size, Anchor, AnyElement, AnyWindowHandle, App, AppContext as _, Bounds, Context,
    Entity, FontWeight, Image, ImageFormat, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, SharedString, StatefulInteractiveElement as _, Styled as _,
    Subscription, Window, WindowBounds, WindowOptions,
};

use super::settings::{self, Settings};
use super::{updates, NotekiApp};
use crate::core::vault::{folder_to_exclude, today_daily_note_path};
use crate::theme::{AppearanceMode, NotekiThemeName};

/// The app's icon, as the bundle has it.
const ICON: &[u8] =
    include_bytes!("../../../Noteki/Assets.xcassets/AppIcon.appiconset/icon_128x128@2x.png");

const THEMES: [NotekiThemeName; 3] = [
    NotekiThemeName::Ayu,
    NotekiThemeName::Everforest,
    NotekiThemeName::OneDarkPro,
];
const MODES: [AppearanceMode; 3] = [
    AppearanceMode::System,
    AppearanceMode::Light,
    AppearanceMode::Dark,
];

pub struct SettingsWindow {
    app: Entity<NotekiApp>,
    /// The Vault's window, where a change is applied.
    main: AnyWindowHandle,
    /// `(family, label)` of every installed font, the system's first.
    fonts: Vec<(SharedString, SharedString)>,
    icon: Arc<Image>,
    text_size: Entity<SliderState>,
    line_length: Entity<SliderState>,
    daily_folder: Entity<InputState>,
    daily_format: Entity<InputState>,
    /// Why the last folder chosen cannot be left out.
    error: Option<String>,
    _subscriptions: Vec<Subscription>,
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
        let fonts = names
            .into_iter()
            .map(|n| (n.clone().into(), n.into()))
            .collect();
        // Nearly square, as the Swift app's; taller to see more without scrolling.
        let bounds = Bounds::centered(None, size(px(520.), px(680.)), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitleBar::title_bar_options()),
            app_id: Some(super::APP_ID.into()),
            ..Default::default()
        };
        gpui_kit::open_window(options, cx, |window, cx| {
            cx.new(|cx| Self::new(app, main, fonts, window, cx))
        })
        .ok()
        .map(|(handle, _)| handle)
    }

    fn new(
        app: Entity<NotekiApp>,
        main: AnyWindowHandle,
        fonts: Vec<(SharedString, SharedString)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let s = app.read(cx).settings().clone();
        let text_size = cx.new(|_| {
            SliderState::new()
                .min(settings::MIN_FONT_SIZE)
                .max(settings::MAX_FONT_SIZE)
                .step(1.)
                .default_value(s.font_size)
        });
        let line_length = cx.new(|_| {
            SliderState::new()
                .min(settings::MIN_LINE_LENGTH as f32)
                .max(settings::MAX_LINE_LENGTH as f32)
                .step(5.)
                .default_value(s.line_length as f32)
        });
        let daily_folder = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("The Vault's root")
                .default_value(s.daily_note_folder.clone())
        });
        let daily_format = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("YYYY-MM-DD")
                .default_value(s.daily_note_format.clone())
        });
        let subscriptions = vec![
            // The Vault's window changed them (⌘+, another Settings…): shown here.
            cx.observe(&app, |_, _, cx| cx.notify()),
            cx.subscribe(&text_size, |me, _, ev: &SliderEvent, cx| {
                if let SliderEvent::Change(v) = ev {
                    let size = v.start().round();
                    me.change(cx, move |t| t.font_size = size);
                }
            }),
            cx.subscribe(&line_length, |me, _, ev: &SliderEvent, cx| {
                if let SliderEvent::Change(v) = ev {
                    let chars = v.start().round() as u32;
                    me.change(cx, move |t| t.line_length = chars);
                }
            }),
            cx.subscribe(&daily_folder, |me, input, ev: &InputEvent, cx| {
                if let InputEvent::Change = ev {
                    let v = input.read(cx).value().to_string();
                    me.change(cx, move |t| t.daily_note_folder = v);
                }
            }),
            cx.subscribe(&daily_format, |me, input, ev: &InputEvent, cx| {
                if let InputEvent::Change = ev {
                    let v = input.read(cx).value().to_string();
                    me.change(cx, move |t| t.daily_note_format = v);
                }
            }),
        ];
        Self {
            app,
            main,
            fonts,
            icon: Arc::new(Image::from_bytes(ImageFormat::Png, ICON.to_vec())),
            text_size,
            line_length,
            daily_folder,
            daily_format,
            error: None,
            _subscriptions: subscriptions,
        }
    }

    fn change(&self, cx: &mut App, change: impl FnOnce(&mut Settings) + 'static) {
        apply(&self.app, self.main, cx, change)
    }
}

/// Applies `change` in the Vault's window, after the event that asked it.
fn apply(
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

/// The text's settings as they are at first: what Restore Defaults puts back.
fn restore_text(s: &mut Settings) {
    let d = Settings::default();
    s.font_family = d.font_family;
    s.code_font = d.code_font;
    s.font_size = d.font_size;
    s.limit_line_length = d.limit_line_length;
    s.line_length = d.line_length;
}

fn text_is_default(s: &Settings) -> bool {
    let mut d = s.clone();
    restore_text(&mut d);
    d == *s
}

impl SettingsWindow {
    /// A menu of `options` showing `current`, the title in front for VoiceOver.
    fn dropdown(
        &self,
        id: &'static str,
        title: &'static str,
        current: SharedString,
        options: Vec<(SharedString, SharedString)>,
        set: impl Fn(&mut Settings, SharedString) + Clone + 'static,
    ) -> AnyElement {
        let (app, main) = (self.app.clone(), self.main);
        let label = options
            .iter()
            .find(|(v, _)| *v == current)
            .map_or(current.clone(), |(_, l)| l.clone());
        let scrollable = options.len() > 12;
        Button::new(id)
            .accessibility_label(format!("{title}: {label}"))
            .label(label)
            .dropdown_caret(true)
            .ghost()
            .small()
            .dropdown_menu_with_anchor(Anchor::TopRight, move |menu, _, _| {
                options
                    .iter()
                    .fold(menu, |menu, (value, label)| {
                        let (app, value, set) = (app.clone(), value.clone(), set.clone());
                        menu.item(
                            PopupMenuItem::new(label.clone())
                                .checked(value == current)
                                .on_click(move |_, _, cx| {
                                    let (value, set) = (value.clone(), set.clone());
                                    apply(&app, main, cx, move |s| set(s, value))
                                }),
                        )
                    })
                    .scrollable(scrollable)
            })
            .into_any_element()
    }

    fn appearance(&self, s: &Settings) -> AnyElement {
        let options = THEMES
            .iter()
            .map(|t| (SharedString::from(t.label()), SharedString::from(t.label())))
            .collect();
        let theme = self.dropdown(
            "theme",
            "Theme",
            s.theme_name.label().into(),
            options,
            |t, v| {
                if let Some(n) = THEMES.iter().find(|n| n.label() == v.as_ref()) {
                    t.theme_name = *n
                }
            },
        );
        let (app, main) = (self.app.clone(), self.main);
        // A segmented control: the choice in the accent colour, as on the Mac.
        let modes = ButtonGroup::new("appearance")
            .small()
            .children(MODES.iter().map(|m| {
                let on = s.appearance == *m;
                Button::new(m.label())
                    .label(m.label())
                    .selected(on)
                    .when(on, |b| b.primary())
                    .when(!on, |b| b.outline())
            }))
            .on_click(move |picked: &Vec<usize>, _, cx| {
                if let Some(m) = picked.first().and_then(|i| MODES.get(*i)).copied() {
                    apply(&app, main, cx, move |t| t.appearance = m)
                }
            });
        section(
            "Appearance",
            None,
            vec![row("Theme", theme), row("Light or dark", modes)],
            None,
        )
    }

    fn text(&self, s: &Settings, window: &mut Window, cx: &mut App) -> AnyElement {
        let system = |label: &'static str| {
            std::iter::once((SharedString::default(), SharedString::from(label)))
                .chain(self.fonts.iter().cloned())
                .collect::<Vec<_>>()
        };
        let font = self.dropdown(
            "font",
            "Font",
            s.font_family.clone().into(),
            system("System"),
            |t, v| t.font_family = v.to_string(),
        );
        let code_font = self.dropdown(
            "code-font",
            "Code font",
            s.code_font.clone().into(),
            system("System Monospaced"),
            |t, v| t.code_font = v.to_string(),
        );
        // ⌘+ and ⌘− in the Vault's window move the slider too.
        sync(&self.text_size, s.font_size, window, cx);
        sync(&self.line_length, s.line_length as f32, window, cx);
        let (app, main) = (self.app.clone(), self.main);
        let limit = Switch::new("limit-line-length")
            .checked(s.limit_line_length)
            .on_click(move |on, _, cx| {
                let on = *on;
                apply(&app, main, cx, move |t| t.limit_line_length = on)
            });
        let mut rows = vec![
            row("Font", font),
            row("Code font", code_font),
            row(
                "Size",
                slider(&self.text_size, format!("{} pt", s.font_size as u32), cx),
            ),
            row("Limit the line length", limit),
        ];
        if s.limit_line_length {
            rows.push(row(
                "Line length",
                slider(
                    &self.line_length,
                    format!("{} characters", s.line_length),
                    cx,
                ),
            ));
        }
        let (app, main) = (self.app.clone(), self.main);
        let restore = Button::new("restore-text")
            .label("Restore Defaults")
            .link()
            .small()
            .disabled(text_is_default(s))
            .on_click(move |_, _, cx| apply(&app, main, cx, restore_text));
        section("Text", Some(restore.into_any_element()), rows, None)
    }

    fn excluded(&self, cx: &mut Context<Self>) -> AnyElement {
        let a = self.app.read(cx);
        let root = a.vault().map(|v| v.root.clone());
        let muted = cx.theme().muted_foreground;
        let Some(root) = root else {
            return section(
                "Excluded folders",
                None,
                vec![div()
                    .py_2()
                    .text_sm()
                    .text_color(muted)
                    .child("Open a Vault to choose the folders it leaves out.")
                    .into_any_element()],
                None,
            );
        };
        let folders = a.settings().excluded_in(&root).to_vec();
        let mut rows: Vec<AnyElement> = folders
            .into_iter()
            .enumerate()
            .map(|(i, folder)| {
                let (app, main, root) = (self.app.clone(), self.main, root.clone());
                let name = SharedString::from(folder.clone());
                let remove = Button::new(("excluded-remove", i))
                    .label("Include Again")
                    .ghost()
                    .xsmall()
                    .on_click(move |_, _, cx| {
                        let (folder, root) = (folder.clone(), root.clone());
                        apply(&app, main, cx, move |s| {
                            if let Some(list) = s
                                .excluded_folders
                                .get_mut(&root.to_string_lossy().to_string())
                            {
                                list.retain(|f| f != &folder);
                            }
                        })
                    });
                row(name, remove)
            })
            .collect();
        let this = cx.entity();
        rows.push(
            h_flex()
                .py_2()
                .child(
                    Button::new("excluded-add")
                        .label("Exclude a Folder…")
                        .small()
                        .on_click(move |_, _, cx| add_excluded(this.clone(), cx)),
                )
                .into_any_element(),
        );
        let footer = match &self.error {
            Some(e) => SharedString::from(e.clone()),
            None => "Left out of Files, links, Tags and Search, as hidden folders are.".into(),
        };
        section("Excluded folders", None, rows, Some(footer))
    }

    fn daily_note(&self, s: &Settings, cx: &App) -> AnyElement {
        let today = today_daily_note_path(&s.daily_note_folder, &s.daily_note_format);
        let muted = cx.theme().muted_foreground;
        let field = |input: &Entity<InputState>| div().w(px(240.)).child(Input::new(input).small());
        section(
            crate::keys::tooltip("Daily note (⇧⌘D)"),
            None,
            vec![
                row("Folder", field(&self.daily_folder)),
                row("Date format", field(&self.daily_format)),
                row(
                    "Today",
                    div()
                        .id("daily-today")
                        .text_sm()
                        .text_color(muted)
                        .child(today),
                ),
            ],
            Some(
                "Obsidian's format, moment.js's: YYYY, MM, DD, dddd, MMMM, WW…; text in \
                 [brackets] stays as written, a / makes folders. Names of days and months \
                 are English."
                    .into(),
            ),
        )
    }
}

/// Sparkle's daily check and *Check Now*, where Noteki can update itself (its
/// bundle; not run from `cargo`); the optional payment, everywhere.
fn updates_and_support() -> AnyElement {
    let support = row(
        "Noteki is free. If it is useful to you, you can support it.",
        Button::new("support")
            .label("Support Noteki…")
            .small()
            .on_click(|_, _, cx| cx.open_url(super::SUPPORT_URL)),
    );
    if !updates::available() {
        return section("Support", None, vec![support], None);
    }
    let automatic = Switch::new("check-for-updates")
        .checked(updates::checks_automatically())
        .on_click(|on, window, _| {
            updates::set_checks_automatically(*on);
            window.refresh();
        });
    let now = Button::new("check-now")
        .label("Check Now…")
        .small()
        .on_click(|_, _, _| updates::check());
    section(
        "Updates",
        None,
        vec![
            row("Check for updates automatically", automatic),
            row("Look for a new version now", now),
            support,
        ],
        Some(
            "Once a day Noteki asks GitHub for its latest version, sending nothing about you or \
             your Vault."
                .into(),
        ),
    )
}

/// The slider shows the setting, wherever it was changed.
fn sync(state: &Entity<SliderState>, value: f32, window: &mut Window, cx: &mut App) {
    if (state.read(cx).value().start() - value).abs() > f32::EPSILON {
        state.update(cx, |s, cx| s.set_value(value, window, cx));
    }
}

fn slider(state: &Entity<SliderState>, value: String, cx: &App) -> impl IntoElement {
    h_flex()
        .gap_3()
        .w(px(300.))
        .child(div().flex_1().child(Slider::new(state)))
        .child(
            div()
                .w(px(110.))
                .text_right()
                .whitespace_nowrap()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(value),
        )
}

/// A label on the left, its control on the right, as a grouped form's row.
fn row(label: impl Into<SharedString>, control: impl IntoElement) -> AnyElement {
    h_flex()
        .min_h(px(40.))
        .py_1()
        .gap_4()
        .justify_between()
        // A long label wraps; the control keeps its size, inside the box.
        .child(div().flex_1().min_w_0().text_sm().child(label.into()))
        .child(div().flex_none().child(control))
        .into_any_element()
}

/// A titled box of rows, divided by hairlines, with a note under it.
fn section(
    title: impl Into<SharedString>,
    accessory: Option<AnyElement>,
    rows: Vec<AnyElement>,
    footer: Option<SharedString>,
) -> AnyElement {
    let count = rows.len();
    SectionBox {
        title: title.into(),
        accessory,
        rows,
        footer,
        count,
    }
    .into_any_element()
}

#[derive(IntoElement)]
struct SectionBox {
    title: SharedString,
    accessory: Option<AnyElement>,
    rows: Vec<AnyElement>,
    footer: Option<SharedString>,
    count: usize,
}

impl gpui_kit::RenderOnce for SectionBox {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        // Outlined boxes on the theme's own page, divided by hairlines.
        let (border, muted) = (theme.border, theme.muted_foreground);
        let count = self.count;
        v_flex()
            .gap_2()
            .child(
                h_flex()
                    .px_1()
                    .justify_between()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(self.title),
                    )
                    .children(self.accessory),
            )
            .child(
                v_flex()
                    .px_3()
                    .rounded_lg()
                    .border_1()
                    .border_color(border)
                    .children(self.rows.into_iter().enumerate().map(move |(i, r)| {
                        // Every row but the last has a hairline under it.
                        div()
                            .when(i + 1 < count, |d| d.border_b_1().border_color(border))
                            .child(r)
                    })),
            )
            .children(
                self.footer
                    .map(|f| div().px_1().text_xs().text_color(muted).child(f)),
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
                    me.change(cx, move |s| {
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
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let s = self.app.read(cx).settings().clone();
        let muted = cx.theme().muted_foreground;
        let header = v_flex()
            .items_center()
            .gap_1()
            .pb_2()
            .child(img(self.icon.clone()).size(px(72.)))
            .child(
                div()
                    .text_xl()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Noteki"),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(muted)
                    .child(format!("Version {}", env!("CARGO_PKG_VERSION"))),
            );
        let page = v_flex()
            .w_full()
            .max_w(px(560.))
            .mx_auto()
            .px_5()
            .pt_2()
            .pb_6()
            .gap_6()
            .child(header)
            .child(self.appearance(&s))
            .child(self.text(&s, window, cx))
            .child(self.excluded(cx))
            .child(self.daily_note(&s, cx))
            .child(updates_and_support());
        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .child(TitleBar::new().child(div().text_sm().child("Settings")))
            .child(
                div()
                    .id("settings-page")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(page),
            )
    }
}
