use std::path::PathBuf;

use noteki::app::{
    hand_over, open_noteki_window, register_keybindings, reopen, set_app_menus, AppConfig,
};
use noteki::core::opening::url_to_path;

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
    let app = gpui_kit::application().with_assets(noteki::assets::NotekiAssets);
    // The Finder, Takku or `open` hand files over here, at launch too: they
    // wait in the channel until the window is there.
    let (opened_tx, opened_rx) = async_channel::unbounded::<Vec<String>>();
    // On Linux they come as arguments (`noteki Nota.md`, a `.desktop`'s %F).
    if !cfg!(target_os = "macos") {
        let urls = file_urls(std::env::args().skip(1));
        if !urls.is_empty() {
            let _ = opened_tx.send_blocking(urls);
        }
    }
    app.on_open_urls(move |urls| {
        let _ = opened_tx.send_blocking(urls);
    });
    let dock_config = config.clone();
    app.on_reopen(move |cx| reopen(&dock_config, cx));
    app.run(move |cx| {
        gpui_kit::init(cx);
        register_keybindings(cx);
        set_app_menus(cx);
        noteki::app::updates::start();

        open_noteki_window(config.clone(), cx).expect("failed to open the Noteki window");
        cx.activate(true);
        cx.spawn(async move |cx| {
            while let Ok(urls) = opened_rx.recv().await {
                let paths: Vec<PathBuf> = urls.iter().filter_map(|u| url_to_path(u)).collect();
                cx.update(|cx| hand_over(&paths, &config, cx));
            }
        })
        .detach();
    });
}

/// The files named on the command line, as `file://` URLs.
fn file_urls(args: impl Iterator<Item = String>) -> Vec<String> {
    args.filter_map(|a| std::fs::canonicalize(a).ok())
        .map(|p| {
            format!(
                "file://{}",
                noteki::core::markdown::percent_encode(&p.to_string_lossy())
            )
        })
        .collect()
}
