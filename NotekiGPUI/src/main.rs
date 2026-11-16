mod app;
mod core;
mod theme;

use app::{register_keybindings, NotekiApp};
use gpui_kit::component::TitleBar;
use gpui_kit::{px, size, AppContext as _, Bounds, WindowBounds, WindowOptions};

fn main() {
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(|cx| {
            gpui_kit::init(cx);
            register_keybindings(cx);

            let bounds = Bounds::centered(None, size(px(1280.), px(820.)), cx);
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitleBar::title_bar_options()),
                window_min_size: Some(size(px(640.), px(420.))),
                ..Default::default()
            };

            gpui_kit::open_window(options, cx, |window, cx| {
                cx.new(|cx| NotekiApp::new(window, cx))
            })
            .expect("failed to open Noteki window");

            cx.activate(true);
        });
}
