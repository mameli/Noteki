//! Time per keystroke in the real `NotekiApp` on a 100 KB Note, headless (the
//! test platform does not shape text, so this is Noteki's own work per frame).
//! Not run by default:
//!
//!     cargo test --release --test keystroke_timing -- --ignored --nocapture
use gpui_kit::test::TestWindowExt;
use gpui_kit::{
    px, size, AppContext as _, Bounds, Keystroke, Point, TestAppContext, WindowBounds,
    WindowOptions,
};
use noteki::app::{register_keybindings, AppConfig, NotekiApp};
use noteki::core::tabs::OpenIn;
use std::time::Instant;

#[gpui_kit::test]
#[ignore]
fn keystroke_timing(cx: &mut TestAppContext) {
    let dir = std::env::temp_dir().join(format!("noteki-kt-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("vault")).unwrap();
    let para = "Una riga di testo di una nota con **grassetto** e [[link]] e #tag.\n";
    let mut big = String::from("# Titolo\n\n");
    // NOTEKI_KT_PARAGRAPHS=1: a blank line after every fifth line, as a Note
    // written in paragraphs (Live preview keeps the styles of each).
    let paragraphs = std::env::var("NOTEKI_KT_PARAGRAPHS").is_ok();
    for i in 0..1500 {
        big.push_str(&format!("{i} {para}"));
        if paragraphs && i % 5 == 4 {
            big.push('\n');
        }
        if i % 20 == 0 {
            big.push_str("## Sezione\n```swift\nstruct Note {\n    let title: String\n}\nprint(\"Hello\")\n```\n");
        }
    }
    std::fs::write(dir.join("vault/Big.md"), &big).unwrap();
    let config = AppConfig {
        settings_path: Some(dir.join("s.json")),
        vault: Some(dir.join("vault")),
        watch: false,
        trash_dir: Some(dir.join("t")),
        slot: 0,
    };
    let (window, app) = cx.update(|cx| {
        gpui_kit::init(cx);
        register_keybindings(cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin: Point::default(),
                size: size(px(1280.), px(800.)),
            })),
            ..Default::default()
        };
        gpui_kit::open_window(options, cx, |window, cx| {
            cx.new(|cx| NotekiApp::new(config, window, cx))
        })
        .unwrap()
    });
    cx.update_window(window, |_, w, cx| {
        app.update(cx, |a, cx| a.open_path("Big.md", OpenIn::CurrentTab, w, cx))
    })
    .unwrap();
    cx.update_window(window, |_, w, cx| {
        app.update(cx, |a, cx| {
            a.editor().clone().update(cx, |ed, cx| {
                ed.set_selected_range(500..500, cx);
                ed.focus(w, cx)
            })
        })
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(window, |_, w, cx| w.render_frame(cx))
        .unwrap();
    println!("text: {} KB", big.len() / 1000);
    // NOTEKI_KT_KEYS=2000 types for long enough to profile it (`sample`).
    let n: usize = std::env::var("NOTEKI_KT_KEYS")
        .ok()
        .and_then(|k| k.parse().ok())
        .unwrap_or(30);
    // A key and the frame it asks for, as the app draws it: GPUI Kit's
    // `input` and `press` draw three frames each with `refresh`, which
    // ignores every cached view.
    let t = Instant::now();
    for _ in 0..n {
        cx.update_window(window, |_, w, cx| {
            w.dispatch_keystroke(key("x"), cx);
            w.draw(cx).clear(cx);
        })
        .unwrap();
        cx.run_until_parked();
    }
    println!(
        "per keystroke incl. frame: {:.2} ms",
        t.elapsed().as_secs_f64() * 1e3 / n as f64
    );
    let t = Instant::now();
    for _ in 0..n {
        cx.update_window(window, |_, w, cx| {
            w.dispatch_keystroke(key("right"), cx);
            w.draw(cx).clear(cx);
        })
        .unwrap();
        cx.run_until_parked();
    }
    println!(
        "per arrow key incl. frame: {:.2} ms",
        t.elapsed().as_secs_f64() * 1e3 / n as f64
    );
    let t = Instant::now();
    for _ in 0..n {
        cx.update_window(window, |_, w, cx| {
            w.refresh();
            w.render_frame(cx)
        })
        .unwrap();
    }
    println!(
        "per frame of the whole window (refresh, nothing cached): {:.2} ms",
        t.elapsed().as_secs_f64() * 1e3 / n as f64
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// A key as the platform sends it: a character with its text.
fn key(k: &'static str) -> Keystroke {
    let mut key = Keystroke::parse(k).unwrap();
    if k.chars().count() == 1 {
        key.key_char = Some(k.to_string());
    }
    key
}

/// Time per keystroke in the Search pane on a 5,000-Note vault, results drawn.
///
///     cargo test --release --test keystroke_timing search -- --ignored --nocapture
#[gpui_kit::test]
#[ignore]
fn search_timing(cx: &mut TestAppContext) {
    let dir = std::env::temp_dir().join(format!("noteki-st-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    for f in 0..50 {
        std::fs::create_dir_all(dir.join(format!("vault/folder {f}"))).unwrap();
    }
    let para =
        "Una riga di testo di una nota sul progetto, con **grassetto** e [[link]] e #area/3.\n";
    for i in 0..5000 {
        let text: String = (0..16).map(|l| format!("{l} {para}")).collect();
        std::fs::write(
            dir.join(format!("vault/folder {}/Note {i}.md", i % 50)),
            text,
        )
        .unwrap();
    }
    let config = AppConfig {
        settings_path: Some(dir.join("s.json")),
        vault: Some(dir.join("vault")),
        watch: false,
        trash_dir: Some(dir.join("t")),
        slot: 0,
    };
    let (window, app) = cx.update(|cx| {
        gpui_kit::init(cx);
        register_keybindings(cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin: Point::default(),
                size: size(px(1280.), px(800.)),
            })),
            ..Default::default()
        };
        gpui_kit::open_window(options, cx, |window, cx| {
            cx.new(|cx| NotekiApp::new(config, window, cx))
        })
        .unwrap()
    });
    cx.run_until_parked();
    cx.update_window(window, |_, w, cx| {
        w.press("cmd-shift-f", cx);
        w.render_frame(cx)
    })
    .unwrap();
    cx.run_until_parked();
    for key in ["p", "r", "o", "g"] {
        let t = Instant::now();
        cx.update_window(window, |_, w, cx| {
            w.input(key, cx);
        })
        .unwrap();
        cx.run_until_parked();
        let typed = t.elapsed();
        cx.update_window(window, |_, w, cx| w.render_frame(cx))
            .unwrap();
        let found = cx.read(|cx| app.read(cx).search_results().len());
        println!(
            "typed {key:?}: {found} results, {:.1} ms to search, {:.1} ms with the frame",
            typed.as_secs_f64() * 1e3,
            t.elapsed().as_secs_f64() * 1e3
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// Time per frame while scrolling continuously through a ~120 KB Note rich in
/// numbered lists, checked tasks, code blocks and Properties, with and without
/// Find matches and Select All.
///
///     cargo test --release --test keystroke_timing scroll -- --ignored --nocapture
#[gpui_kit::test]
#[ignore]
fn scroll_timing(cx: &mut TestAppContext) {
    let dir = std::env::temp_dir().join(format!("noteki-scroll-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("vault")).unwrap();
    let mut big = String::from("---\ntitle: Long note\ntags:\n  - perf\n  - scroll\n---\n# Titolo\n\n");
    for i in 0..1500 {
        match i % 5 {
            0 => big.push_str(&format!("{}. Una riga numerata con **grassetto** e [[link]].\n", (i % 20) + 1)),
            1 => big.push_str("- [x] Un task completato nella riga di test.\n"),
            2 => big.push_str("- [ ] Un task aperto nella riga di test.\n"),
            _ => big.push_str(&format!("{i} Una riga di testo con **grassetto** e #tag.\n")),
        }
        if i % 20 == 0 {
            big.push_str("## Sezione\n```swift\nstruct Note {\n    let title: String\n}\nprint(\"Hello\")\n```\n");
        }
    }
    std::fs::write(dir.join("vault/Big.md"), &big).unwrap();
    let config = AppConfig {
        settings_path: Some(dir.join("s.json")),
        vault: Some(dir.join("vault")),
        watch: false,
        trash_dir: Some(dir.join("t")),
        slot: 0,
    };
    let (window, app) = cx.update(|cx| {
        gpui_kit::init(cx);
        register_keybindings(cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin: Point::default(),
                size: size(px(1280.), px(800.)),
            })),
            ..Default::default()
        };
        gpui_kit::open_window(options, cx, |window, cx| {
            cx.new(|cx| NotekiApp::new(config, window, cx))
        })
        .unwrap()
    });
    cx.update_window(window, |_, w, cx| {
        app.update(cx, |a, cx| a.open_path("Big.md", OpenIn::CurrentTab, w, cx))
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(window, |_, w, cx| w.render_frame(cx))
        .unwrap();

    let steps = 120usize;
    let measure = |label: &str, cx: &mut TestAppContext| {
        let t = Instant::now();
        for s in 0..steps {
            let y = px(-((s * 80) as f32));
            cx.update_window(window, |_, w, cx| {
                app.update(cx, |a, cx| {
                    a.editor().clone().update(cx, |ed, cx| {
                        ed.set_scroll_offset(gpui_kit::point(px(0.), y), cx)
                    })
                });
                w.draw(cx).clear(cx);
            })
            .unwrap();
        }
        println!(
            "{label}: {:.3} ms/frame ({steps} frames)",
            t.elapsed().as_secs_f64() * 1e3 / steps as f64
        );
    };

    println!("text: {} KB", big.len() / 1000);
    measure("scroll plain", cx);

    // Same scroll with Find highlighting ~1500 matches across the whole note.
    cx.update_window(window, |_, w, cx| {
        app.update(cx, |a, cx| {
            a.editor().clone().update(cx, |ed, cx| {
                ed.open_find(w, cx);
            })
        });
        w.input("riga", cx);
    })
    .unwrap();
    cx.run_until_parked();
    measure("scroll with ~1500 Find matches", cx);

    // Close Find, select all, and scroll.
    cx.update_window(window, |_, w, cx| {
        app.update(cx, |a, cx| {
            a.editor().clone().update(cx, |ed, cx| {
                ed.close_find(w, cx);
                ed.set_selected_range(0..big.len(), cx);
            })
        });
        w.draw(cx).clear(cx);
    })
    .unwrap();
    measure("scroll with Select All", cx);

    let _ = std::fs::remove_dir_all(&dir);
}
