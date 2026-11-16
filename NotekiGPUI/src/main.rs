use std::path::PathBuf;

use gpui_kit::component::TitleBar;
use gpui_kit::{px, size, AppContext as _, Bounds, WindowBounds, WindowOptions};
use noteki_gpui::app::{register_keybindings, AppConfig, NotekiApp};

/// Environment overrides, used by `scripts/eval.sh` for a smoke run that
/// leaves the user's preferences alone:
/// - `NOTEKI_SETTINGS`: preferences file to use instead of the user's.
/// - `NOTEKI_VAULT`: vault to open at launch.
fn config() -> AppConfig {
    let mut config = AppConfig::for_user();
    if let Some(path) = std::env::var_os("NOTEKI_SETTINGS") {
        config.settings_path = Some(PathBuf::from(path));
    }
    if let Some(vault) = std::env::var_os("NOTEKI_VAULT") {
        config.vault = Some(PathBuf::from(vault));
    }
    config
}

fn main() {
    let config = config();
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(move |cx| {
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
                cx.new(|cx| NotekiApp::new(config, window, cx))
            })
            .expect("failed to open the Noteki window");
            cx.activate(true);
        });
}
