//! Formulas, typeset as KaTeX does by RaTeX: what to draw, glyph by glyph, in
//! em from the formula's top-left, its baseline `height` below the top. The
//! editor draws them with the KaTeX fonts (editor::math).

use std::fmt;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::rc::Rc;

use ratex_layout::{layout, to_display_list, validate_layout, LayoutOptions};
use ratex_types::{DisplayList, MathStyle};

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
