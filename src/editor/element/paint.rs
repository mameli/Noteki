//! Painting what a line draws around and in place of its text: quotes,
//! lists, code boxes, tables, Properties, callouts, images, formulas,
//! diagrams.

use super::*;

/// What is painted around a line's text: under it, never changing its height.
pub(super) fn paint_decoration(
    line: &LaidLine,
    (left, top, width): (Pixels, Pixels, Pixels),
    input: &ShapeInput,
    window: &mut Window,
    cx: &mut App,
) {
    let palette = &input.palette;
    match &line.decoration {
        Some(Decoration::Rule) => {
            let y = top + line.height() / 2.;
            window.paint_quad(fill(
                Bounds::new(point(left, y), size(width, px(1.))),
                palette.separator,
            ));
        }
        Some(Decoration::Quote { depth }) => {
            for d in 0..*depth {
                let x = left + QUOTE_INDENT * f32::from(d);
                window.paint_quad(fill(
                    Bounds::new(point(x, top), size(QUOTE_BAR, line.height())),
                    palette.accent,
                ));
            }
        }
        Some(Decoration::Callout {
            depth,
            color,
            header,
        }) => {
            // A box in the type's colour, its bar on the left; rounded at the top.
            let c = callout_color(*color, palette);
            let bg = gpui_kit::hsla(c.h, c.s, c.l, 0.1);
            let radius = if *header { px(4.) } else { px(0.) };
            window.paint_quad(
                fill(
                    Bounds::new(point(left, top), size(width, line.height())),
                    bg,
                )
                .corner_radii(Corners {
                    top_left: radius,
                    top_right: radius,
                    bottom_left: px(0.),
                    bottom_right: px(0.),
                }),
            );
            let x = left + QUOTE_INDENT * f32::from(depth.saturating_sub(1));
            window.paint_quad(fill(
                Bounds::new(point(x, top), size(QUOTE_BAR, line.height())),
                c,
            ));
        }
        Some(Decoration::List { level, kind }) => {
            let x = left + LIST_INDENT * f32::from(*level);
            // Centred on the first row.
            let mid = top + line.line_height / 2.;
            match *kind {
                ListKind::Bullet => {
                    let d = px(5.);
                    window.paint_quad(
                        fill(
                            Bounds::new(point(x + px(8.), mid - d / 2.), size(d, d)),
                            palette.syntax,
                        )
                        .corner_radii(d / 2.),
                    );
                }
                ListKind::Number { n, delim } => {
                    let label = format!("{n}{}", delim as char);
                    let run =
                        text_run(label.len(), &input.font, palette.syntax, Default::default());
                    let shaped = window.text_system().shape_line(
                        label.into(),
                        input.font_size,
                        &[run],
                        None,
                    );
                    let at = point(x + LIST_MARKER - shaped.width() - px(6.), top);
                    let _ = shaped.paint(at, line.line_height, TextAlign::Left, None, window, cx);
                }
                ListKind::Task { checked, .. } => {
                    let b = checkbox_bounds(x, mid);
                    let (bg, border) = if checked {
                        (palette.accent, palette.accent)
                    } else {
                        (palette.background, palette.syntax)
                    };
                    window.paint_quad(
                        fill(b, bg)
                            .corner_radii(px(3.))
                            .border_widths(px(1.5))
                            .border_color(border),
                    );
                    if checked {
                        let run = text_run(
                            "✓".len(),
                            &input.font,
                            palette.background,
                            Default::default(),
                        );
                        let shaped = window.text_system().shape_line(
                            "✓".into(),
                            CHECKBOX - px(2.),
                            &[run],
                            None,
                        );
                        let at = point(
                            b.left() + (CHECKBOX - shaped.width()) / 2.,
                            b.top() - px(1.),
                        );
                        let _ = shaped.paint(at, CHECKBOX, TextAlign::Left, None, window, cx);
                    }
                }
            }
        }
        Some(Decoration::Code(part)) => {
            let (round_top, round_bottom) = match part {
                CodePart::Open { .. } => (CODE_RADIUS, px(0.)),
                CodePart::Body => (px(0.), px(0.)),
                CodePart::Close => (px(0.), CODE_RADIUS),
            };
            let corners = Corners {
                top_left: round_top,
                top_right: round_top,
                bottom_left: round_bottom,
                bottom_right: round_bottom,
            };
            window.paint_quad(
                fill(
                    Bounds::new(point(left, top), size(width, line.height())),
                    palette.code_bg,
                )
                .corner_radii(corners),
            );
            if let CodePart::Open { language } = part {
                if !language.is_empty() {
                    let label: SharedString = language.to_string().into();
                    let font_size = input.font_size * 0.8;
                    let run =
                        text_run(label.len(), &input.font, palette.syntax, Default::default());
                    let shaped = window
                        .text_system()
                        .shape_line(label, font_size, &[run], None);
                    let at = point(left + width - shaped.width() - CODE_PAD, top);
                    let _ = shaped.paint(at, line.line_height, TextAlign::Left, None, window, cx);
                }
            }
        }
        Some(Decoration::Table(row)) => {
            let Some(edges) = &line.columns else {
                return;
            };
            let right = left + *edges.last().unwrap_or(&px(0.));
            let h = line.height();
            let border = palette.separator;
            let hline = |y: Pixels, window: &mut Window| {
                window.paint_quad(fill(
                    Bounds::new(point(left, y), size(right - left, px(1.))),
                    border,
                ))
            };
            if row.kind == RowKind::Delimiter {
                // The rule under the header, across the table: two pixels.
                window.paint_quad(fill(
                    Bounds::new(point(left, top), size(right - left, h)),
                    border,
                ));
                return;
            }
            if row.kind == RowKind::Header {
                hline(top, window);
            }
            hline(top + h - px(1.), window);
            for x in edges.iter() {
                window.paint_quad(fill(
                    Bounds::new(point(left + *x, top), size(px(1.), h)),
                    border,
                ));
            }
        }
        Some(Decoration::Property(p)) => {
            // A box like a code block's, "Properties" at its top.
            let r = px(6.);
            let (top_r, bottom_r) = match p {
                PropertyLine::Top => (r, px(0.)),
                PropertyLine::Bottom => (px(0.), r),
                _ => (px(0.), px(0.)),
            };
            window.paint_quad(
                fill(
                    Bounds::new(point(left, top), size(width, line.height())),
                    palette.code_bg,
                )
                .corner_radii(Corners {
                    top_left: top_r,
                    top_right: top_r,
                    bottom_left: bottom_r,
                    bottom_right: bottom_r,
                }),
            );
            if *p == PropertyLine::Top {
                let label: SharedString = "Properties".into();
                let run = text_run(label.len(), &input.font, palette.syntax, Default::default());
                let shaped =
                    window
                        .text_system()
                        .shape_line(label, input.font_size * 0.85, &[run], None);
                let _ = shaped.paint(
                    point(left + CELL_PAD, top),
                    line.line_height,
                    TextAlign::Left,
                    None,
                    window,
                    cx,
                );
            }
        }
        // Painted after the text, in `paint`.
        Some(
            Decoration::Image(_)
            | Decoration::Formula(_)
            | Decoration::Diagram(_)
            | Decoration::Folded,
        )
        | None => {}
    }
}

/// What a line draws in place of its hidden source: an image (loaded in the
/// background the first time, a placeholder until then, then from GPUI's
/// cache), a display formula or a diagram, both centred in the text's width.
pub(super) fn paint_figure(
    line: &LaidLine,
    (left, top, width): (Pixels, Pixels, Pixels),
    input: &ShapeInput,
    diagrams: &HashMap<u64, Arc<gpui_kit::Image>>,
    window: &mut Window,
    cx: &mut App,
) {
    let Some((figure, size)) = &line.figure else {
        return;
    };
    if let Figure::Chips(chips) = figure {
        let size_ = input.font_size * CHIP_SCALE;
        let (pad, chip_h) = (size_ * 0.45, size_ * 1.45);
        let p = &input.palette;
        let bg = gpui_kit::hsla(p.syntax.h, p.syntax.s, p.syntax.l, 0.14);
        let top = top + line.text_height;
        for (at, w, shaped) in chips.iter() {
            let origin = point(left + at.x, top + at.y);
            window.paint_quad(
                fill(Bounds::new(origin, gpui_kit::size(*w, chip_h)), bg).corner_radii(px(5.)),
            );
            let clip = Bounds::new(origin, gpui_kit::size(*w - pad, chip_h));
            window.with_content_mask(Some(ContentMask { bounds: clip }), |window| {
                let _ = shaped.paint(
                    point(origin.x + pad, origin.y),
                    chip_h,
                    TextAlign::Left,
                    None,
                    window,
                    cx,
                );
            });
        }
        return;
    }
    let room = width - line.indent;
    let pad = input.font_size * FORMULA_PAD;
    let centred = left + line.indent + ((room - size.width) / 2.).max(px(0.));
    let y = top + line.text_height;
    match figure {
        Figure::File(path) => {
            let bounds = Bounds::new(point(left + line.indent, y), *size);
            let source = Resource::Path(Arc::from(&**path));
            match window.use_asset::<ImageAssetLoader>(&source, cx) {
                Some(Ok(image)) => {
                    let _ =
                        window.paint_image(bounds, bounds, Corners::all(px(4.)), image, 0, false);
                }
                Some(Err(_)) => {}
                // GPUI draws the editor again once it is loaded (`use_asset`
                // notifies the views that asked). Asking a frame at every
                // frame until then made macOS keep five window-sized
                // surfaces instead of two, 49 MB more.
                None => {
                    window.paint_quad(fill(bounds, input.palette.code_bg).corner_radii(px(4.)));
                }
            }
        }
        Figure::Formula(paint) => {
            paint.paint(point(centred, y + pad), input.palette.text, window, cx);
        }
        Figure::Chips(_) => {}
        Figure::Diagram(d) => {
            let bounds = Bounds::new(
                point(centred, y + pad),
                gpui_kit::size(size.width, size.height - pad * 2.),
            );
            let image = diagrams
                .get(&d.id)
                .and_then(|i| i.clone().use_render_image(window, cx));
            // Drawn again once rasterized, as an image file (above).
            if let Some(image) = image {
                let _ = window.paint_image(bounds, bounds, Corners::default(), image, 0, false);
            }
        }
    }
}

/// The formulas drawn in a line's text, each centred on the room its insert
/// takes, its baseline on the text's.
pub(super) fn paint_inline_formulas(
    line: &LaidLine,
    (left, top): (Pixels, Pixels),
    input: &ShapeInput,
    window: &mut Window,
    cx: &mut App,
) {
    let layout = &line.shaped.unwrapped_layout;
    let baseline = (line.line_height - layout.ascent - layout.descent) / 2. + layout.ascent;
    for f in line.formulas.iter() {
        let Some(p) = line.shaped.position_for_index(f.at, line.line_height) else {
            continue;
        };
        let x = left + line.indent + p.x + (f.room - f.paint.width) / 2.;
        let y = top + p.y + baseline - f.paint.height;
        f.paint.paint(point(x, y), input.palette.text, window, cx);
    }
}

/// A callout type's colour in the theme.
pub(super) fn callout_color(color: CalloutColor, p: &NotekiPalette) -> Hsla {
    match color {
        CalloutColor::Blue => p.link,
        CalloutColor::Green => p.string,
        CalloutColor::Orange => p.accent,
        CalloutColor::Red => p.error,
        CalloutColor::Purple => p.keyword,
        CalloutColor::Gray => p.syntax,
    }
}

/// Where a task's checkbox is drawn, for a list item at `x` whose first row's
/// middle is at `mid`.
pub(super) fn checkbox_bounds(x: Pixels, mid: Pixels) -> Bounds<Pixels> {
    Bounds::new(
        point(x + px(2.), mid - CHECKBOX / 2.),
        size(CHECKBOX, CHECKBOX),
    )
}
