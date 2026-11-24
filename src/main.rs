use std::path::PathBuf;

use noteki::app::{
    hand_over, open_noteki_window, register_keybindings, register_vault_commands, reopen,
    set_app_menus, AppConfig,
};
use noteki::core::opening::{prepare_for_command, url_to_path};

const USAGE: &str = "\
Usage: noteki [FILE|FOLDER]...

Opens files in Noteki, as the Finder does: a Note or Attachment of the Vault
in a Tab, a Markdown or text file outside the Vault in a window of its own, a
folder of the Vault in Files. A Markdown or text file that is missing is
created, empty. With no argument, Noteki opens.

  -h, --help     show this help
  -V, --version  show the version";

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
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    // `--open`: the `noteki` command (the script in the Mac app's Resources).
    let command = args.first().map(String::as_str) == Some("--open");
    if command {
        args.remove(0);
    }
    match args.first().map(String::as_str) {
        Some("-h" | "--help") => {
            println!("{USAGE}");
            return;
        }
        Some("-V" | "--version") => {
            println!("noteki {}", env!("CARGO_PKG_VERSION"));
            return;
        }
        _ => {}
    }
    let config = config();
    // On the Mac the command hands the files to the app, running or not, and
    // ends: the app gets them as from the Finder. Launched by the system, the
    // app takes no arguments.
    if cfg!(target_os = "macos") {
        if command {
            std::process::exit(open_from_command(&args, &config));
        }
        args.clear();
    }
    let app = gpui_kit::application().with_assets(noteki::assets::NotekiAssets);
    // The Finder, Takku or `open` hand files over here, at launch too: they
    // wait in the channel until the window is there.
    let (opened_tx, opened_rx) = async_channel::unbounded::<Vec<String>>();
    // On Linux they come as arguments (`noteki Nota.md`, a `.desktop`'s %F).
    if !args.is_empty() {
        let (paths, refused) = prepare(&args, &config);
        if paths.is_empty() && refused {
            std::process::exit(1);
        }
        let urls = file_urls(&paths);
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
        register_vault_commands(config.clone(), cx);

        // Launched to open files (the Mac hands them over before launching
        // finishes): they open first, and the Vault's window only if one of
        // them belongs there. A Loose file alone opens without the Vault.
        let mut launch_urls = Vec::new();
        while let Ok(urls) = opened_rx.try_recv() {
            launch_urls.extend(urls);
        }
        if launch_urls.is_empty() {
            open_noteki_window(config.clone(), cx).expect("failed to open the Noteki window");
        } else {
            let paths: Vec<PathBuf> = launch_urls.iter().filter_map(|u| url_to_path(u)).collect();
            hand_over(&paths, &config, cx);
            if cx.windows().is_empty() {
                open_noteki_window(config.clone(), cx).expect("failed to open the Noteki window");
            }
        }
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

/// The command's arguments as paths to open (SPEC › Opening from the
/// terminal), each refused one told on stderr; true if any was refused.
fn prepare(args: &[String], config: &AppConfig) -> (Vec<PathBuf>, bool) {
    let cwd = std::env::current_dir().unwrap_or_default();
    let vault = noteki::app::remembered_vault(config);
    let mut refused = false;
    let paths = args
        .iter()
        .filter_map(|a| {
            prepare_for_command(std::path::Path::new(a), &cwd, vault.as_deref())
                .map_err(|e| {
                    eprintln!("noteki: {e}");
                    refused = true;
                })
                .ok()
        })
        .collect();
    (paths, refused)
}

/// The `noteki` command on the Mac: the files to Noteki through `open`, which
/// starts it or hands them to the one running. Its exit status: 1 if a file
/// was refused.
fn open_from_command(args: &[String], config: &AppConfig) -> i32 {
    let (paths, refused) = prepare(args, config);
    if paths.is_empty() && refused {
        return 1;
    }
    // This binary's own app, so a build opens itself and not the installed one.
    let app = std::env::current_exe()
        .ok()
        .and_then(|e| std::fs::canonicalize(e).ok())
        .and_then(|e| e.ancestors().nth(3).map(PathBuf::from))
        .filter(|a| a.extension().is_some_and(|x| x == "app"))
        .unwrap_or_else(|| PathBuf::from("Noteki"));
    let status = std::process::Command::new("open")
        .arg("-a")
        .arg(&app)
        .args(&paths)
        .status();
    match status {
        Ok(s) if s.success() => i32::from(refused),
        Ok(s) => s.code().unwrap_or(1),
        Err(e) => {
            eprintln!("noteki: cannot run open: {e}");
            1
        }
    }
}

/// Paths as `file://` URLs, as the platform hands them over.
fn file_urls(paths: &[PathBuf]) -> Vec<String> {
    paths
        .iter()
        .map(|p| {
            format!(
                "file://{}",
                noteki::core::markdown::percent_encode(&p.to_string_lossy())
            )
        })
        .collect()
}
