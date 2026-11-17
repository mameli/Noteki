//! A formula drawn by GPUI itself, from RaTeX's display list: each glyph in
//! its KaTeX font through GPUI's text system (sharp at any zoom, shared with
//! the text's glyph cache), rules as quads, radicals and large delimiters as
//! paths. GPUI loads no font without an `m`: the glyphs of KaTeX's Size fonts
//! (large integrals, sums, delimiters) are drawn as their outlines. Laid out
//! once with the line, painted every frame.

use std::collections::HashMap;
use std::rc::Rc;
use std::sync::OnceLock;

use gpui_kit::{
    point, px, size, Bounds, Font, FontId, FontStyle, FontWeight, GlyphId, Hsla, PathBuilder,
    Pixels, Point, SharedString, TextRun, Window, WindowTextSystem,
};
use ratex_types::{Color, DisplayItem, PathCommand};

use crate::core::math::{katex_face, Formula};

/// KaTeX's text is 1.21 times the surrounding text's size.
pub(super) const KATEX_SCALE: f32 = 1.21;

/// A formula laid out at a size: what to paint, from its top-left.
pub(super) struct MathPaint {
    pub width: Pixels,
    /// Above the baseline, and under it.
    pub height: Pixels,
    pub depth: Pixels,
    items: Vec<Item>,
}

enum Item {
    Glyph {
        font_id: FontId,
        glyph_id: GlyphId,
        /// The glyph's origin on its baseline, from the formula's top-left.
        origin: Point<Pixels>,
        size: Pixels,
        color: Option<Hsla>,
    },
    Rect {
        bounds: Bounds<Pixels>,
        color: Option<Hsla>,
    },
    Path {
        origin: Point<Pixels>,
        commands: Rc<[PathCommand]>,
        fill: bool,
        color: Option<Hsla>,
    },
}

impl MathPaint {
    /// `formula` at `em` pixels to the em (the text's size × [`KATEX_SCALE`]).
    pub fn new(formula: &Formula, em: Pixels, ts: &WindowTextSystem) -> Self {
        let list = &formula.list;
        let at = |x: f64, y: f64| point(em * x as f32, em * y as f32);
        let items = list
            .items
            .iter()
            .filter_map(|item| match item {
                DisplayItem::GlyphPath {
                    x,
                    y,
                    scale,
                    font,
                    char_code,
                    color,
                } => {
                    let ch = char::from_u32(*char_code)?;
                    let size = em * *scale as f32;
                    if let Some(commands) = outline(font, ch, *scale) {
                        return Some(Item::Path {
                            origin: at(*x, *y),
                            commands,
                            fill: true,
                            color: own_color(color),
                        });
                    }
                    let (font_id, glyph_id) = glyph(ts, font, ch, size)?;
                    Some(Item::Glyph {
                        font_id,
                        glyph_id,
                        origin: at(*x, *y),
                        size,
                        color: own_color(color),
                    })
                }
                DisplayItem::Line {
                    x,
                    y,
                    width,
                    thickness,
                    color,
                    ..
                } => {
                    let t = (em * *thickness as f32).max(px(1.));
                    let o = at(*x, *y);
                    Some(Item::Rect {
                        bounds: Bounds::new(point(o.x, o.y - t / 2.), size(em * *width as f32, t)),
                        color: own_color(color),
                    })
                }
                DisplayItem::Rect {
                    x,
                    y,
                    width,
                    height,
                    color,
                } => Some(Item::Rect {
                    bounds: Bounds::new(at(*x, *y), size(em * *width as f32, em * *height as f32)),
                    color: own_color(color),
                }),
                DisplayItem::Path {
                    x,
                    y,
                    commands,
                    fill,
                    color,
                } => Some(Item::Path {
                    origin: at(*x, *y),
                    commands: commands.clone().into(),
                    fill: *fill,
                    color: own_color(color),
                }),
            })
            .collect();
        Self {
            width: em * list.width as f32,
            height: em * list.height as f32,
            depth: em * list.depth as f32,
            items,
        }
    }

    /// Paints the formula with its top-left at `origin`, in `color` where it
    /// names none of its own (`\color`).
    pub fn paint(&self, origin: Point<Pixels>, em: Pixels, color: Hsla, window: &mut Window) {
        for item in &self.items {
            match item {
                Item::Glyph {
                    font_id,
                    glyph_id,
                    origin: o,
                    size,
                    color: c,
                } => {
                    let _ = window.paint_glyph(
                        origin + *o,
                        *font_id,
                        *glyph_id,
                        *size,
                        c.unwrap_or(color),
                    );
                }
                Item::Rect { bounds, color: c } => {
                    window.paint_quad(gpui_kit::fill(
                        Bounds::new(origin + bounds.origin, bounds.size),
                        c.unwrap_or(color),
                    ));
                }
                Item::Path {
                    origin: o,
                    commands,
                    fill,
                    color: c,
                } => {
                    let mut b = if *fill {
                        PathBuilder::fill()
                    } else {
                        PathBuilder::stroke(px(1.))
                    };
                    let p = |x: f64, y: f64| origin + *o + point(em * x as f32, em * y as f32);
                    for cmd in commands.iter() {
                        match *cmd {
                            PathCommand::MoveTo { x, y } => b.move_to(p(x, y)),
                            PathCommand::LineTo { x, y } => b.line_to(p(x, y)),
                            PathCommand::QuadTo { x1, y1, x, y } => b.curve_to(p(x, y), p(x1, y1)),
                            PathCommand::CubicTo {
                                x1,
                                y1,
                                x2,
                                y2,
                                x,
                                y,
                            } => b.cubic_bezier_to(p(x, y), p(x1, y1), p(x2, y2)),
                            PathCommand::Close => b.close(),
                        }
                    }
                    if let Ok(path) = b.build() {
                        window.paint_path(path, c.unwrap_or(color));
                    }
                }
            }
        }
    }
}

/// The font and glyph that draw `ch` in RaTeX's font `font`, as GPUI shapes it.
fn glyph(ts: &WindowTextSystem, font: &str, ch: char, size: Pixels) -> Option<(FontId, GlyphId)> {
    let (family, bold, italic) = katex_face(font);
    let font = Font {
        family: SharedString::new_static(family),
        weight: if bold {
            FontWeight::BOLD
        } else {
            FontWeight::NORMAL
        },
        style: if italic {
            FontStyle::Italic
        } else {
            FontStyle::Normal
        },
        ..Font::default()
    };
    let text = SharedString::from(ch.to_string());
    let run = TextRun {
        len: text.len(),
        font,
        color: Hsla::default(),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let layout = ts.layout_line(&text, size, &[run], None);
    let run = layout.runs.first()?;
    Some((run.font_id, run.glyphs.first()?.id))
}

/// The KaTeX fonts GPUI does not load (no `m`), by RaTeX's names.
fn outline_faces() -> &'static HashMap<String, ttf_parser::Face<'static>> {
    static FACES: OnceLock<HashMap<String, ttf_parser::Face<'static>>> = OnceLock::new();
    FACES.get_or_init(|| {
        let mut faces = HashMap::new();
        for name in [
            "Size1-Regular",
            "Size2-Regular",
            "Size3-Regular",
            "Size4-Regular",
        ] {
            let Some(bytes) = ratex_katex_fonts::ttf_bytes(&format!("KaTeX_{name}.ttf")) else {
                continue;
            };
            let bytes: &'static [u8] = match bytes {
                std::borrow::Cow::Borrowed(b) => b,
                std::borrow::Cow::Owned(b) => Box::leak(b.into_boxed_slice()),
            };
            if let Ok(face) = ttf_parser::Face::parse(bytes, 0) {
                faces.insert(name.to_string(), face);
            }
        }
        faces
    })
}

/// The outline of `ch` in `font`, in em of the formula from the glyph's
/// origin, when GPUI cannot draw that font's glyphs.
fn outline(font: &str, ch: char, scale: f64) -> Option<Rc<[PathCommand]>> {
    let face = outline_faces().get(font)?;
    let glyph = face.glyph_index(ch)?;
    let mut out = Outline {
        k: scale / f64::from(face.units_per_em()),
        commands: Vec::new(),
    };
    face.outline_glyph(glyph, &mut out)?;
    Some(out.commands.into())
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

/// A colour the formula names (`\color{red}`); black, RaTeX's default, is the text's.
fn own_color(c: &Color) -> Option<Hsla> {
    if *c == Color::BLACK {
        return None;
    }
    let rgba = gpui_kit::Rgba {
        r: c.r,
        g: c.g,
        b: c.b,
        a: c.a,
    };
    Some(rgba.into())
}
