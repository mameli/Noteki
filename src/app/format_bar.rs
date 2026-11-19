//! The Format bar (SPEC › Writing): a row of Format buttons over the Note,
//! for whoever does not know the shortcuts yet, shown only when chosen in
//! Settings. Each does what its Format menu item does; the paperclip attaches
//! images as if dropped (not in a Loose file's window: nothing is embedded
//! next to it).

use std::rc::Rc;

use gpui_kit::assets::IconName as AssetIcon;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::{h_flex, ActiveTheme as _, Icon, Sizable as _};
use gpui_kit::{div, prelude::*, px, AnyElement, App, TestSupportExt as _, Window};

use crate::core::editor::format::{LineFormat, Wrap};
use crate::editor::Format;

pub type OnFormat = Rc<dyn Fn(Format, &mut Window, &mut App)>;
pub type OnAttach = Rc<dyn Fn(&mut Window, &mut App)>;

/// The buttons, by id: their icon, tooltip and Format.
const BUTTONS: [(&str, AssetIcon, &str, Format); 9] = [
    (
        "bold",
        AssetIcon::Bold,
        "Bold (⌘B)",
        Format::Wrap(Wrap::Bold),
    ),
    (
        "italic",
        AssetIcon::Italic,
        "Italic (⌘I)",
        Format::Wrap(Wrap::Italic),
    ),
    (
        "strikethrough",
        AssetIcon::Strikethrough,
        "Strikethrough (⇧⌘X)",
        Format::Wrap(Wrap::Strikethrough),
    ),
    (
        "quote",
        AssetIcon::TextQuote,
        "Quote",
        Format::Lines(LineFormat::Quote),
    ),
    (
        "code",
        AssetIcon::Code,
        "Inline Code (⌃`)",
        Format::Wrap(Wrap::Code),
    ),
    ("link", AssetIcon::Link, "Link (⌘K)", Format::Link),
    (
        "numbered",
        AssetIcon::ListOrdered,
        "Numbered List",
        Format::Lines(LineFormat::Numbered),
    ),
    (
        "bulleted",
        AssetIcon::List,
        "Bulleted List",
        Format::Lines(LineFormat::Bullet),
    ),
    (
        "task",
        AssetIcon::ListTodo,
        "Task (⌘↩)",
        Format::Lines(LineFormat::Task),
    ),
];

fn divider(cx: &App) -> AnyElement {
    div()
        .mx_1()
        .w(px(1.))
        .h(px(14.))
        .bg(cx.theme().border)
        .into_any_element()
}

/// The bar, on the Note's background, its buttons at the right, out of the
/// way of the text's start.
pub fn format_bar(on_format: OnFormat, on_attach: Option<OnAttach>, cx: &App) -> AnyElement {
    let headings = on_format.clone();
    let heading =
        Button::new("format-bar-heading")
            .icon(Icon::new(AssetIcon::Heading))
            .xsmall()
            .ghost()
            .tooltip("Heading")
            .dropdown_menu(move |menu, _, _| {
                (1..=6u8).fold(menu, |menu, level| {
                    let on = headings.clone();
                    menu.item(PopupMenuItem::new(format!("Heading {level}")).on_click(
                        move |_, w, cx| on(Format::Lines(LineFormat::Heading(level)), w, cx),
                    ))
                })
            });
    let button = |(id, icon, tip, format): (&'static str, AssetIcon, &'static str, Format)| {
        let on = on_format.clone();
        Button::new(format!("format-bar-{id}"))
            .icon(Icon::new(icon))
            .xsmall()
            .ghost()
            .tooltip(crate::keys::tooltip(tip))
            .on_click(move |_, w, cx| on(format, w, cx))
            .into_any_element()
    };
    let mut bar = h_flex()
        .id("format-bar")
        .test_support()
        .w_full()
        .justify_end()
        .gap_0p5()
        .px_2()
        .py(px(3.))
        .bg(cx.theme().background)
        .text_color(cx.theme().muted_foreground)
        .child(heading)
        .children(BUTTONS[..6].iter().copied().map(button))
        .child(divider(cx))
        .children(BUTTONS[6..].iter().copied().map(button));
    if let Some(attach) = on_attach {
        bar = bar.child(divider(cx)).child(
            Button::new("format-bar-attach")
                .icon(Icon::new(AssetIcon::Paperclip))
                .xsmall()
                .ghost()
                .tooltip("Attach Images…")
                .on_click(move |_, w, cx| attach(w, cx)),
        );
    }
    bar.into_any_element()
}
