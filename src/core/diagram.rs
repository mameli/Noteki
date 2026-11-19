//! Mermaid diagrams as SVG, by mermaid-rs-renderer: no browser. The editor
//! draws the SVG under the fence it replaces while the cursor is elsewhere.

use std::hash::{DefaultHasher, Hash, Hasher};
use std::panic::{catch_unwind, AssertUnwindSafe};

use mermaid_rs_renderer::{render_with_options, RenderOptions, Theme};

/// A diagram drawn: the size it is shown at, in pixels. The editor keeps its
/// image (the SVG, drawn by GPUI) by its id.
#[derive(Debug, Clone, PartialEq)]
pub struct Diagram {
    /// Its source and theme, hashed.
    pub id: u64,
    pub width: f32,
    pub height: f32,
}

impl Eq for Diagram {}

/// The key a diagram is kept under: its source, in the light or dark theme.
pub fn diagram_id(source: &str, dark: bool) -> u64 {
    let mut h = DefaultHasher::new();
    (source, dark).hash(&mut h);
    h.finish()
}

/// The diagram `source` draws, or `None` when it is none mermaid-rs-renderer
/// draws (a typo, a kind it does not know): the fence then stays as code.
/// Slow the first time (the system's fonts are read): off the main thread.
pub fn render(source: &str, dark: bool) -> Option<(String, f32, f32)> {
    let mut theme = if dark { Theme::dark() } else { Theme::modern() };
    // On the Note's own background, whatever the theme.
    theme.background = "none".into();
    let options = RenderOptions {
        theme,
        ..Default::default()
    };
    let svg = catch_unwind(AssertUnwindSafe(|| render_with_options(source, options)))
        .ok()?
        .ok()?;
    let (w, h) = (attr(&svg, "width")?, attr(&svg, "height")?);
    let (width, height): (f32, f32) = (w.parse().ok()?, h.parse().ok()?);
    // At its own size: GPUI rasterizes an SVG image at twice it
    // (`SMOOTH_SVG_SCALE_FACTOR`), what a Retina screen shows. Doubling it
    // here too rasterized four times the pixels, for nothing sharper.
    Some((svg, width, height))
}

/// An attribute of the root `<svg>` element, as written.
fn attr<'a>(svg: &'a str, name: &str) -> Option<&'a str> {
    let root = &svg[..svg.find('>')?];
    let at = root.find(&format!(" {name}=\""))? + name.len() + 3;
    root[at..].split('"').next()
}

#[cfg(test)]
mod tests {
    use super::*;

    const FLOW: &str =
        "flowchart LR\n    A[Feature freeze] --> B{Notarized?}\n    B -->|Yes| C[Release]";

    #[test]
    fn a_flowchart_is_drawn_twice_its_size() {
        let (svg, w, h) = render(FLOW, false).expect("drawn");
        assert!(w > 100. && h > 20., "{w}×{h}");
        assert_eq!(attr(&svg, "width").unwrap().parse::<f32>(), Ok(w));
        assert_eq!(attr(&svg, "height").unwrap().parse::<f32>(), Ok(h));
        assert!(svg.contains("Notarized?"));
        assert!(!svg.contains("#FFFFFF\"/>") || svg.contains("fill=\"none\""));
    }

    #[test]
    fn what_is_no_diagram_stays_code() {
        assert!(render("this is not mermaid at all", false).is_none());
    }

    #[test]
    fn the_theme_changes_the_key() {
        assert_ne!(diagram_id(FLOW, false), diagram_id(FLOW, true));
        assert_eq!(diagram_id(FLOW, true), diagram_id(FLOW, true));
    }
}
