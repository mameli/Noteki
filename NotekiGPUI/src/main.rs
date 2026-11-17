use std::path::PathBuf;

use noteki_gpui::app::{open_noteki_window, register_keybindings, AppConfig};
use noteki_gpui::core::opening::url_to_path;

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
    let app = gpui_kit::application().with_assets(noteki_gpui::assets::NotekiAssets);
    // The Finder, Takku or `open` hand files over here, at launch too: they
    // wait in the channel until the window is there.
    let (opened_tx, opened_rx) = async_channel::unbounded::<Vec<String>>();
    app.on_open_urls(move |urls| {
        let _ = opened_tx.send_blocking(urls);
    });
    app.run(move |cx| {
        gpui_kit::init(cx);
        register_keybindings(cx);

        let (window, noteki) =
            open_noteki_window(config, cx).expect("failed to open the Noteki window");
        cx.activate(true);
        cx.spawn(async move |cx| {
            while let Ok(urls) = opened_rx.recv().await {
                let _ = window.update(cx, |_, window, cx| {
                    window.activate_window();
                    for path in urls.iter().filter_map(|u| url_to_path(u)) {
                        noteki.update(cx, |app, cx| app.open_external(&path, window, cx));
                    }
                });
            }
        })
        .detach();
    });
}
