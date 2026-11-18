//! Writes a Note of the demo Vault as a PDF without a window or a Dock icon:
//! a check of *Export as PDF…* that leaves the screen alone.
//!   cargo run --example print_pdf -- <note.md> <out.pdf>

#[cfg(target_os = "macos")]
fn main() {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};

    let mut args = std::env::args().skip(1);
    let (note, out) = (args.next().expect("a note"), args.next().expect("a pdf"));
    let text = std::fs::read_to_string(&note).expect("readable note");
    let html = noteki::core::export::note_html(
        &text,
        "Test",
        None,
        &Default::default(),
        |_| None,
    );
    let mtm = MainThreadMarker::new().unwrap();
    let app = NSApplication::sharedApplication(mtm);
    // Never in the Dock, never in front.
    app.setActivationPolicy(NSApplicationActivationPolicy::Prohibited);
    noteki::app::print::run(&html, "Test", noteki::app::print::Job::Pdf(out.into()), |r| {
        println!("{r:?}");
        std::process::exit(if r.is_ok() { 0 } else { 1 });
    });
    app.run();
}

#[cfg(not(target_os = "macos"))]
fn main() {}
