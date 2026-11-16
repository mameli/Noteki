//! Time per keystroke in the real `NotekiApp` on a 100 KB Note, headless (the
//! test platform does not shape text, so this is Noteki's own work per frame).
//! Not run by default:
//!
//!     cargo test --release --test keystroke_timing -- --ignored --nocapture
use gpui_kit::test::TestWindowExt;
use gpui_kit::{
    px, size, AppContext as _, Bounds, Point, TestAppContext, WindowBounds, WindowOptions,
};
use noteki_gpui::app::{register_keybindings, AppConfig, NotekiApp};
use noteki_gpui::core::tabs::OpenIn;
use std::time::Instant;

#[gpui_kit::test]
#[ignore]
fn keystroke_timing(cx: &mut TestAppContext) {
    let dir = std::env::temp_dir().join(format!("noteki-kt-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("vault")).unwrap();
    let para = "Una riga di testo di una nota con **grassetto** e [[link]] e #tag.\n";
    let mut big = String::from("# Titolo\n\n");
    for i in 0..1500 {
        big.push_str(&format!("{i} {para}"));
        if i % 20 == 0 {
            big.push_str("## Sezione\n");
        }
    }
    std::fs::write(dir.join("vault/Big.md"), &big).unwrap();
    let config = AppConfig {
        settings_path: Some(dir.join("s.json")),
        vault: Some(dir.join("vault")),
        watch: false,
        trash_dir: Some(dir.join("t")),
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
    let n = 30;
    let t = Instant::now();
    for _ in 0..n {
        cx.update_window(window, |_, w, cx| {
            w.input("x", cx);
            w.render_frame(cx)
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
        cx.update_window(window, |_, w, cx| w.press("right", cx))
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
        "per idle frame: {:.2} ms",
        t.elapsed().as_secs_f64() * 1e3 / n as f64
    );
    let _ = std::fs::remove_dir_all(&dir);
}
