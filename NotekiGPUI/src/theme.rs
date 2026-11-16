use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::{rgb, App, Hsla, Window, WindowAppearance};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[derive(Default)]
pub enum NotekiThemeName {
    #[default]
    Ayu,
    Everforest,
    OneDarkPro,
}

impl NotekiThemeName {
    pub fn label(self) -> &'static str {
        match self {
            Self::Ayu => "Ayu",
            Self::Everforest => "Everforest",
            Self::OneDarkPro => "One Dark Pro",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Ayu => Self::Everforest,
            Self::Everforest => Self::OneDarkPro,
            Self::OneDarkPro => Self::Ayu,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum AppearanceMode {
    #[default]
    System,
    Light,
    Dark,
}

impl AppearanceMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::System => "System",
            Self::Light => "Light",
            Self::Dark => "Dark",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::System => Self::Light,
            Self::Light => Self::Dark,
            Self::Dark => Self::System,
        }
    }

    pub fn is_dark(self, window: Option<&Window>) -> bool {
        match self {
            Self::Light => false,
            Self::Dark => true,
            Self::System => {
                if let Some(w) = window {
                    matches!(
                        w.appearance(),
                        WindowAppearance::Dark | WindowAppearance::VibrantDark
                    )
                } else {
                    false
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NotekiPalette {
    pub dark: bool,
    pub text: Hsla,
    pub background: Hsla,
    pub panel: Hsla,
    pub separator: Hsla,
    pub caret: Hsla,
    pub selection: Hsla,
    pub syntax: Hsla,
    pub link: Hsla,
    pub on_link: Hsla,
    pub accent: Hsla,
    pub quote: Hsla,
    pub code_bg: Hsla,
    pub highlight: Hsla,
    pub error: Hsla,
    pub heading: Hsla,
    pub keyword: Hsla,
    pub string: Hsla,
    pub number: Hsla,
}

impl NotekiPalette {
    pub fn for_theme(name: NotekiThemeName, dark: bool) -> Self {
        match (name, dark) {
            (NotekiThemeName::Ayu, false) => Self {
                dark: false,
                text: rgb(0x5c6166).into(),
                background: rgb(0xf8f9fa).into(),
                panel: rgb(0xf0f1f2).into(),
                separator: rgb(0xe1e4e8).into(),
                caret: rgb(0xffaa33).into(),
                selection: rgb(0xd3e1f5).into(),
                syntax: rgb(0x8a9199).into(),
                link: rgb(0x399ee6).into(),
                on_link: rgb(0xf8f9fa).into(),
                accent: rgb(0xffaa33).into(),
                quote: rgb(0x787b80).into(),
                code_bg: hsla_alpha(rgb(0x5c6166).into(), 0.08),
                highlight: hsla_alpha(rgb(0xffaa33).into(), 0.25),
                error: rgb(0xe65050).into(),
                heading: rgb(0x363b40).into(),
                keyword: rgb(0xfa8d3e).into(),
                string: rgb(0x86b300).into(),
                number: rgb(0xa37acc).into(),
            },
            (NotekiThemeName::Ayu, true) => Self {
                dark: true,
                text: rgb(0xbfbdb6).into(),
                background: rgb(0x0b0e14).into(),
                panel: rgb(0x0f131a).into(),
                separator: rgb(0x1e232b).into(),
                caret: rgb(0xe6b450).into(),
                selection: rgb(0x1b3a5b).into(),
                syntax: rgb(0x5a6378).into(),
                link: rgb(0x59c2ff).into(),
                on_link: rgb(0x0b0e14).into(),
                accent: rgb(0xe6b450).into(),
                quote: rgb(0x8a9199).into(),
                code_bg: hsla_alpha(rgb(0xbfbdb6).into(), 0.08),
                highlight: hsla_alpha(rgb(0xe6b450).into(), 0.22),
                error: rgb(0xd95757).into(),
                heading: rgb(0xe6e1cf).into(),
                keyword: rgb(0xff8f40).into(),
                string: rgb(0xaad94c).into(),
                number: rgb(0xd2a6ff).into(),
            },
            (NotekiThemeName::Everforest, false) => Self {
                dark: false,
                text: rgb(0x5c6a72).into(),
                background: rgb(0xfdf6e3).into(),
                panel: rgb(0xf4f0d9).into(),
                separator: rgb(0xe6e2cc).into(),
                caret: rgb(0x5c6a72).into(),
                selection: rgb(0xefe9d5).into(),
                syntax: rgb(0x939f91).into(),
                link: rgb(0x3a94c5).into(),
                on_link: rgb(0xfdf6e3).into(),
                accent: rgb(0x8da101).into(),
                quote: rgb(0x829181).into(),
                code_bg: rgb(0xf4f0d9).into(),
                highlight: hsla_alpha(rgb(0xdfa000).into(), 0.22),
                error: rgb(0xf85552).into(),
                heading: rgb(0x3d484d).into(),
                keyword: rgb(0xf85552).into(),
                string: rgb(0x35a77c).into(),
                number: rgb(0xdf69ba).into(),
            },
            (NotekiThemeName::Everforest, true) => Self {
                dark: true,
                text: rgb(0xd3c6aa).into(),
                background: rgb(0x2d353b).into(),
                panel: rgb(0x232a2e).into(),
                separator: rgb(0x3d484d).into(),
                caret: rgb(0xd3c6aa).into(),
                selection: rgb(0x414b51).into(),
                syntax: rgb(0x859289).into(),
                link: rgb(0x7fbbb3).into(),
                on_link: rgb(0x2d353b).into(),
                accent: rgb(0xa7c080).into(),
                quote: rgb(0x9da9a0).into(),
                code_bg: rgb(0x343f44).into(),
                highlight: hsla_alpha(rgb(0xdbbc7f).into(), 0.22),
                error: rgb(0xe67e80).into(),
                heading: rgb(0xe6dcc5).into(),
                keyword: rgb(0xe67e80).into(),
                string: rgb(0x83c092).into(),
                number: rgb(0xd699b6).into(),
            },
            (NotekiThemeName::OneDarkPro, false) => Self {
                dark: false,
                text: rgb(0x4b4e58).into(),
                background: rgb(0xfafafa).into(),
                panel: rgb(0xf0f0f1).into(),
                separator: rgb(0xdbdbdc).into(),
                caret: rgb(0x526fff).into(),
                selection: rgb(0xe5e5e6).into(),
                syntax: rgb(0xa0a1a7).into(),
                link: rgb(0x4078f2).into(),
                on_link: rgb(0xfafafa).into(),
                accent: rgb(0x4078f2).into(),
                quote: rgb(0x696c77).into(),
                code_bg: hsla_alpha(rgb(0x383a42).into(), 0.06),
                highlight: hsla_alpha(rgb(0xc18401).into(), 0.22),
                error: rgb(0xe45649).into(),
                heading: rgb(0x23262f).into(),
                keyword: rgb(0xa626a4).into(),
                string: rgb(0x50a14f).into(),
                number: rgb(0x986801).into(),
            },
            (NotekiThemeName::OneDarkPro, true) => Self {
                dark: true,
                text: rgb(0xabb2bf).into(),
                background: rgb(0x282c34).into(),
                panel: rgb(0x21252b).into(),
                separator: rgb(0x3e4452).into(),
                caret: rgb(0x528bff).into(),
                selection: rgb(0x3e4451).into(),
                syntax: rgb(0x5c6370).into(),
                link: rgb(0x61afef).into(),
                on_link: rgb(0x282c34).into(),
                accent: rgb(0x61afef).into(),
                quote: rgb(0x7f848e).into(),
                code_bg: hsla_alpha(rgb(0xabb2bf).into(), 0.08),
                highlight: hsla_alpha(rgb(0xe5c07b).into(), 0.25),
                error: rgb(0xe06c75).into(),
                heading: rgb(0xd7dae0).into(),
                keyword: rgb(0xc678dd).into(),
                string: rgb(0x98c379).into(),
                number: rgb(0xd19a66).into(),
            },
        }
    }
}

fn hsla_alpha(mut c: Hsla, a: f32) -> Hsla {
    c.a = a;
    c
}

pub fn apply_noteki_theme(
    name: NotekiThemeName,
    appearance: AppearanceMode,
    window: &mut Window,
    cx: &mut App,
) -> NotekiPalette {
    let dark = appearance.is_dark(Some(window));
    let mode = if dark {
        ThemeMode::Dark
    } else {
        ThemeMode::Light
    };
    Theme::change(mode, Some(window), cx);

    let p = NotekiPalette::for_theme(name, dark);
    Theme::update(cx, |theme| {
        theme.colors.background = p.background;
        theme.colors.foreground = p.text;
        theme.colors.caret = p.caret;
        theme.colors.selection = p.selection;
        theme.colors.border = p.separator;
        theme.colors.muted = p.code_bg;
        theme.colors.muted_foreground = p.syntax;

        theme.colors.primary = p.accent;
        theme.colors.primary_foreground = p.on_link;
        theme.colors.primary_hover = p.accent;
        theme.colors.primary_active = p.accent;

        theme.colors.button_primary = p.accent;
        theme.colors.button_primary_foreground = p.on_link;
        theme.colors.button_primary_hover = p.accent;
        theme.colors.button_primary_active = p.accent;

        theme.colors.accent = hsla_alpha(p.accent, 0.16);
        theme.colors.accent_foreground = p.text;

        theme.colors.link = p.link;
        theme.colors.link_hover = p.link;
        theme.colors.link_active = p.link;

        theme.colors.sidebar = p.panel;
        theme.colors.sidebar_foreground = p.text;
        theme.colors.sidebar_border = p.separator;
        theme.colors.sidebar_accent = hsla_alpha(p.accent, 0.16);
        theme.colors.sidebar_accent_foreground = p.text;
        theme.colors.sidebar_primary = p.accent;
        theme.colors.sidebar_primary_foreground = p.on_link;

        theme.colors.title_bar = p.background;
        theme.colors.title_bar_border = p.separator;
        theme.colors.status_bar = p.panel;
        theme.colors.status_bar_border = p.separator;

        theme.colors.tab_bar = p.background;
        theme.colors.tab = p.panel;
        theme.colors.tab_foreground = p.syntax;
        theme.colors.tab_active = p.background;
        theme.colors.tab_active_foreground = p.text;

        theme.colors.list = p.panel;
        theme.colors.list_hover = hsla_alpha(p.text, 0.06);
        theme.colors.list_active = hsla_alpha(p.accent, 0.18);
        theme.colors.list_active_border = p.accent;

        theme.colors.popover = p.panel;
        theme.colors.popover_foreground = p.text;
        theme.colors.input = p.separator;
        theme.colors.ring = p.accent;
        theme.colors.danger = p.error;
    });
    p
}
