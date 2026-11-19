//! Formulas, typeset as KaTeX does by RaTeX: what to draw, glyph by glyph, in
//! em from the formula's top-left, its baseline `height` below the top. The
//! editor draws them with the KaTeX fonts (editor::math).

use std::fmt;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::rc::Rc;

use ratex_layout::{layout, to_display_list, validate_layout, LayoutOptions};
use ratex_types::{DisplayList, MathStyle, PathCommand};

/// A formula as typeset: `$$…$$` (display) or `$…$` (in a line).
pub struct Formula {
    pub tex: Rc<str>,
    pub display: bool,
    pub list: DisplayList,
}

impl Formula {
    /// Width, height above the baseline and depth under it, in em.
    pub fn width(&self) -> f32 {
        self.list.width as f32
    }
    pub fn height(&self) -> f32 {
        self.list.height as f32
    }
    pub fn depth(&self) -> f32 {
        self.list.depth as f32
    }
}

// The same TeX in the same style is the same formula: styles compare by it.
impl PartialEq for Formula {
    fn eq(&self, other: &Self) -> bool {
        self.display == other.display && self.tex == other.tex
    }
}
impl Eq for Formula {}

impl fmt::Debug for Formula {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Formula({:?}, display: {})", self.tex, self.display)
    }
}

/// `tex` typeset, or `None` when it is no formula RaTeX knows (a typo while
/// writing): the editor then shows it as written.
pub fn typeset(tex: &str, display: bool) -> Option<Formula> {
    if tex.trim().is_empty() {
        return None;
    }
    let style = if display {
        MathStyle::Display
    } else {
        MathStyle::Text
    };
    // A formula is typed one letter at a time: whatever RaTeX makes of a
    // half-written one, it never takes the editor down.
    let list = catch_unwind(AssertUnwindSafe(|| {
        let ast = ratex_parser::parse(tex).ok()?;
        let lbox = layout(&ast, &LayoutOptions::default().with_style(style));
        validate_layout(&lbox).ok()?;
        Some(to_display_list(&lbox))
    }))
    .ok()
    .flatten()?;
    Some(Formula {
        tex: tex.into(),
        display,
        list,
    })
}

/// The KaTeX fonts the formulas are drawn in, by their family names.
pub fn katex_fonts() -> impl Iterator<Item = std::borrow::Cow<'static, [u8]>> {
    [
        "KaTeX_AMS-Regular.ttf",
        "KaTeX_Caligraphic-Bold.ttf",
        "KaTeX_Caligraphic-Regular.ttf",
        "KaTeX_Fraktur-Bold.ttf",
        "KaTeX_Fraktur-Regular.ttf",
        "KaTeX_Main-Bold.ttf",
        "KaTeX_Main-BoldItalic.ttf",
        "KaTeX_Main-Italic.ttf",
        "KaTeX_Main-Regular.ttf",
        "KaTeX_Math-BoldItalic.ttf",
        "KaTeX_Math-Italic.ttf",
        "KaTeX_SansSerif-Bold.ttf",
        "KaTeX_SansSerif-Italic.ttf",
        "KaTeX_SansSerif-Regular.ttf",
        "KaTeX_Script-Regular.ttf",
        "KaTeX_Size1-Regular.ttf",
        "KaTeX_Size2-Regular.ttf",
        "KaTeX_Size3-Regular.ttf",
        "KaTeX_Size4-Regular.ttf",
        "KaTeX_Typewriter-Regular.ttf",
    ]
    .into_iter()
    .filter_map(ratex_katex_fonts::ttf_bytes)
}

/// The family, bold and italic of a RaTeX font name (`Main-BoldItalic`).
pub fn katex_face(font: &str) -> (&'static str, bool, bool) {
    match font {
        "Main-Bold" => ("KaTeX_Main", true, false),
        "Main-Italic" => ("KaTeX_Main", false, true),
        "Main-BoldItalic" => ("KaTeX_Main", true, true),
        "Math-Italic" => ("KaTeX_Math", false, true),
        "Math-BoldItalic" => ("KaTeX_Math", true, true),
        "AMS-Regular" => ("KaTeX_AMS", false, false),
        "Caligraphic-Regular" => ("KaTeX_Caligraphic", false, false),
        "Caligraphic-Bold" => ("KaTeX_Caligraphic", true, false),
        "Fraktur-Regular" => ("KaTeX_Fraktur", false, false),
        "Fraktur-Bold" => ("KaTeX_Fraktur", true, false),
        "SansSerif-Regular" => ("KaTeX_SansSerif", false, false),
        "SansSerif-Bold" => ("KaTeX_SansSerif", true, false),
        "SansSerif-Italic" => ("KaTeX_SansSerif", false, true),
        "Script-Regular" => ("KaTeX_Script", false, false),
        "Typewriter-Regular" => ("KaTeX_Typewriter", false, false),
        "Size1-Regular" => ("KaTeX_Size1", false, false),
        "Size2-Regular" => ("KaTeX_Size2", false, false),
        "Size3-Regular" => ("KaTeX_Size3", false, false),
        "Size4-Regular" => ("KaTeX_Size4", false, false),
        _ => ("KaTeX_Main", false, false),
    }
}

/// The faces of the KaTeX fonts, by RaTeX's names (`Main-Regular`), parsed
/// once: for the outlines of their glyphs.
fn faces() -> &'static std::collections::HashMap<String, ttf_parser::Face<'static>> {
    use std::sync::OnceLock;
    static FACES: OnceLock<std::collections::HashMap<String, ttf_parser::Face<'static>>> =
        OnceLock::new();
    FACES.get_or_init(|| {
        let names = [
            "AMS-Regular",
            "Caligraphic-Bold",
            "Caligraphic-Regular",
            "Fraktur-Bold",
            "Fraktur-Regular",
            "Main-Bold",
            "Main-BoldItalic",
            "Main-Italic",
            "Main-Regular",
            "Math-BoldItalic",
            "Math-Italic",
            "SansSerif-Bold",
            "SansSerif-Italic",
            "SansSerif-Regular",
            "Script-Regular",
            "Size1-Regular",
            "Size2-Regular",
            "Size3-Regular",
            "Size4-Regular",
            "Typewriter-Regular",
        ];
        names
            .into_iter()
            .filter_map(|name| {
                let bytes = ratex_katex_fonts::ttf_bytes(&format!("KaTeX_{name}.ttf"))?;
                let bytes: &'static [u8] = match bytes {
                    std::borrow::Cow::Borrowed(b) => b,
                    std::borrow::Cow::Owned(b) => Box::leak(b.into_boxed_slice()),
                };
                Some((name.to_string(), ttf_parser::Face::parse(bytes, 0).ok()?))
            })
            .collect()
    })
}

/// The outline of `ch` in the KaTeX font `font` (RaTeX's name), in em of the
/// formula from the glyph's origin, y downwards; `scale` is the glyph's size.
pub fn glyph_outline(font: &str, ch: char, scale: f64) -> Option<Vec<PathCommand>> {
    let face = faces().get(font)?;
    let glyph = face.glyph_index(ch)?;
    let mut out = Outline {
        k: scale / f64::from(face.units_per_em()),
        commands: Vec::new(),
    };
    face.outline_glyph(glyph, &mut out)?;
    Some(out.commands)
}

/// Collects a glyph's outline in em, y downwards.
struct Outline {
    k: f64,
    commands: Vec<PathCommand>,
}

impl ttf_parser::OutlineBuilder for Outline {
    fn move_to(&mut self, x: f32, y: f32) {
        let (x, y) = (f64::from(x) * self.k, -f64::from(y) * self.k);
        self.commands.push(PathCommand::MoveTo { x, y });
    }
    fn line_to(&mut self, x: f32, y: f32) {
        let (x, y) = (f64::from(x) * self.k, -f64::from(y) * self.k);
        self.commands.push(PathCommand::LineTo { x, y });
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let k = self.k;
        self.commands.push(PathCommand::QuadTo {
            x1: f64::from(x1) * k,
            y1: -f64::from(y1) * k,
            x: f64::from(x) * k,
            y: -f64::from(y) * k,
        });
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let k = self.k;
        self.commands.push(PathCommand::CubicTo {
            x1: f64::from(x1) * k,
            y1: -f64::from(y1) * k,
            x2: f64::from(x2) * k,
            y2: -f64::from(y2) * k,
            x: f64::from(x) * k,
            y: -f64::from(y) * k,
        });
    }
    fn close(&mut self) {
        self.commands.push(PathCommand::Close);
    }
}

/// `commands` as an SVG path's data, moved by `(dx, dy)`.
fn path_data(commands: &[PathCommand], dx: f64, dy: f64) -> String {
    use std::fmt::Write;
    let mut d = String::new();
    for c in commands {
        let _ = match *c {
            PathCommand::MoveTo { x, y } => write!(d, "M{:.4} {:.4}", x + dx, y + dy),
            PathCommand::LineTo { x, y } => write!(d, "L{:.4} {:.4}", x + dx, y + dy),
            PathCommand::QuadTo { x1, y1, x, y } => {
                write!(
                    d,
                    "Q{:.4} {:.4} {:.4} {:.4}",
                    x1 + dx,
                    y1 + dy,
                    x + dx,
                    y + dy
                )
            }
            PathCommand::CubicTo {
                x1,
                y1,
                x2,
                y2,
                x,
                y,
            } => write!(
                d,
                "C{:.4} {:.4} {:.4} {:.4} {:.4} {:.4}",
                x1 + dx,
                y1 + dy,
                x2 + dx,
                y2 + dy,
                x + dx,
                y + dy
            ),
            PathCommand::Close => write!(d, "Z"),
        };
    }
    d
}

/// The SVG fill of a RaTeX colour: black, its default, is the text's own.
fn fill(c: &ratex_types::Color) -> String {
    if *c == ratex_types::Color::BLACK {
        "currentColor".into()
    } else {
        let byte = |v: f32| (v.clamp(0., 1.) * 255.).round() as u8;
        format!("#{:02x}{:02x}{:02x}", byte(c.r), byte(c.g), byte(c.b))
    }
}

/// The formula as an SVG standing alone (for Export), sized in em so it
/// takes the text's size; its baseline on the text's, in a line.
pub fn to_svg(f: &Formula) -> String {
    use ratex_types::DisplayItem;
    use std::fmt::Write;
    let list = &f.list;
    let (w, h, d) = (list.width, list.height, list.depth);
    let mut body = String::new();
    for item in &list.items {
        match item {
            DisplayItem::GlyphPath {
                x,
                y,
                scale,
                font,
                char_code,
                color,
            } => {
                let Some(ch) = char::from_u32(*char_code) else {
                    continue;
                };
                if let Some(commands) = glyph_outline(font, ch, *scale) {
                    let _ = write!(
                        body,
                        "<path fill=\"{}\" d=\"{}\"/>",
                        fill(color),
                        path_data(&commands, *x, *y)
                    );
                }
            }
            DisplayItem::Line {
                x,
                y,
                width,
                thickness,
                color,
                ..
            } => {
                let t = thickness.max(0.04);
                let _ = write!(
                    body,
                    "<rect fill=\"{}\" x=\"{x:.4}\" y=\"{:.4}\" width=\"{width:.4}\" height=\"{t:.4}\"/>",
                    fill(color),
                    y - t / 2.
                );
            }
            DisplayItem::Rect {
                x,
                y,
                width,
                height,
                color,
            } => {
                let _ = write!(
                    body,
                    "<rect fill=\"{}\" x=\"{x:.4}\" y=\"{y:.4}\" width=\"{width:.4}\" height=\"{height:.4}\"/>",
                    fill(color)
                );
            }
            DisplayItem::Path {
                x,
                y,
                commands,
                fill: filled,
                color,
            } => {
                let paint = fill(color);
                let style = if *filled {
                    format!("fill=\"{paint}\"")
                } else {
                    format!("fill=\"none\" stroke=\"{paint}\" stroke-width=\"0.04\"")
                };
                let _ = write!(
                    body,
                    "<path {style} d=\"{}\"/>",
                    path_data(commands, *x, *y)
                );
            }
        }
    }
    let total = h + d;
    format!(
        "<svg class=\"formula\" xmlns=\"http://www.w3.org/2000/svg\" width=\"{w:.3}em\" height=\"{total:.3}em\" \
         viewBox=\"0 0 {w:.4} {total:.4}\" style=\"vertical-align:-{d:.3}em\" role=\"img\" aria-label=\"{}\">{body}</svg>",
        f.tex.replace('&', "&amp;").replace('"', "&quot;").replace('<', "&lt;")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_formula_is_typeset_with_its_baseline() {
        let f = typeset(r"\frac{a}{b}", true).expect("typeset");
        assert!(f.width() > 0.5);
        assert!(f.height() > 0.5 && f.depth() > 0.3, "{f:?} over and under");
        let inline = typeset(r"\frac{a}{b}", false).unwrap();
        assert!(inline.height() < f.height(), "smaller in a line");
    }

    #[test]
    fn what_is_no_formula_stays_text() {
        assert!(typeset(r"\frac{a}{", true).is_none());
        assert!(typeset("  ", false).is_none());
        assert!(typeset(r"\nosuchcommand", false).is_none());
    }

    #[test]
    fn every_katex_font_is_there() {
        assert_eq!(katex_fonts().count(), 20);
    }

    #[test]
    fn half_written_formulas_never_panic() {
        let tex = r"\begin{aligned} \nabla \cdot \mathbf{E} &= \frac{\rho}{\varepsilon_0} \\ x^{2_{3}} \end{aligned}";
        for end in 0..=tex.len() {
            if tex.is_char_boundary(end) {
                let _ = typeset(&tex[..end], true);
                let _ = typeset(&tex[..end], false);
            }
        }
    }
}
