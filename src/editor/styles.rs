//! Live preview's styles of each line: the block styles kept between
//! changes, the cursor's lines shown as source, and the figures (`$$` blocks,
//! Mermaid diagrams, formulas in the text).

use super::*;

impl NoteEditor {
    /// The Live preview styles of each line: every line once per change of
    /// the text, then only the lines the cursor's position changes.
    pub(super) fn line_styles(&mut self, cx: &mut Context<Self>) -> LineStyles {
        let text = self.buf.text();
        if text.len() > LIVE_PREVIEW_MAX_BYTES {
            return Rc::default();
        }
        if let Some(block) = &self.deferred_diagram {
            if !block.contains(&line_of_offset(text, self.buf.cursor())) {
                self.base_styles.0 = None;
            }
        }
        let LinkContext {
            path, index, root, ..
        } = &self.link_context;
        // Every line with its marks hidden, once per change of the text. A code
        // block is highlighted again only when its code or language changed.
        if self.base_styles.0 != Some(self.revision) {
            let cache = &mut self.code_colours;
            let mut used = HashMap::new();
            let mut highlight = |language: &str, code: &str| {
                let mut h = DefaultHasher::new();
                (language, code).hash(&mut h);
                let key = h.finish();
                let colours = cache
                    .remove(&key)
                    .or_else(|| used.get(&key).cloned())
                    .unwrap_or_else(|| code::highlight(language, code).into());
                used.insert(key, Rc::clone(&colours));
                colours
            };
            let images = &mut self.images;
            let mut image = |target: &str| -> Option<Image> {
                images
                    .entry(target.to_string())
                    .or_insert_with(|| {
                        let root = root.as_deref()?;
                        let found = resolve_embed(root, target, path.as_deref(), index)?;
                        let abs = resolve_vault_path(root, &found.path).ok()?;
                        Some(Image {
                            path: abs.into(),
                            width: u32::try_from(found.width?).ok()?,
                            height: u32::try_from(found.height?).ok()?,
                        })
                    })
                    .clone()
            };
            // Block by block: only the one typed in is styled again.
            let mut spans =
                |block: &str| compute_markdown_spans(block, None, path.as_deref(), index);
            let styles = self.block_styles.style(
                text,
                &mut spans,
                StyleContext {
                    highlight: &mut highlight,
                    image: &mut image,
                },
            );
            *cache = used;
            let plain = keep_equal_rc(&self.plain_styles, styles);
            let figured = self.with_figures(&plain, cx);
            let figured = keep_equal(&self.base_styles.1, figured);
            self.plain_styles = Rc::new(plain);
            self.base_styles = (Some(self.revision), Rc::new(figured));
        }
        // Then the lines the cursor shows marks on: its own (the element it is
        // in), or a code block's fences while it is in the block. Moving the
        // cursor costs a line or two, not the Note.
        let cursor = self.buf.cursor();
        let key = (self.revision, cursor);
        if self.styles.0 != Some(key) {
            let text = self.buf.text();
            let LinkContext { path, index, .. } = &self.link_context;
            let base = &self.plain_styles;
            let mut styles: Vec<Rc<LineStyle>> = (*self.base_styles.1).clone();
            let lines = markdown::lines(text);
            let i = line_of_offset(text, cursor);
            // The block or line the cursor is in shows its source, not its figure.
            let block = self
                .figure_lines
                .iter()
                .find(|r| r.contains(&i))
                .cloned()
                .unwrap_or(i..i + 1);
            for j in block {
                if let (Some(slot), Some(plain)) = (styles.get_mut(j), base.get(j)) {
                    *slot = plain.clone();
                }
            }
            let mut restyle = |line: &markdown::Line, in_line: usize| {
                let mut spans = Vec::new();
                markdown::line_spans(line, Some(in_line), path.as_deref(), index, &mut spans);
                let mut style = style_line(line, &spans);
                if let Some(slot) = styles.get_mut(line.index) {
                    // A code block's box and an image are the block's, not the line's.
                    match slot.decoration {
                        Some(Decoration::Code(_)) => {
                            style.decoration = slot.decoration.clone();
                            style.segments = slot.segments.clone();
                        }
                        Some(Decoration::Image(_) | Decoration::Callout { .. }) => {
                            style.decoration = slot.decoration.clone()
                        }
                        // A table stays drawn while its row is edited: the
                        // row's cells show all their text, its marks where
                        // the cursor is. The rule under the header shows
                        // its source.
                        Some(Decoration::Table(ref row))
                            if row.kind != crate::core::editor::table::RowKind::Delimiter =>
                        {
                            let mut row = row.clone();
                            row.editing = true;
                            style.decoration = Some(Decoration::Table(row));
                        }
                        _ => {}
                    }
                    // A setext Heading is one through the line under it.
                    if style.heading.is_none() {
                        style.heading = slot.heading;
                    }
                    if **slot != style {
                        *slot = Rc::new(style);
                    }
                }
            };
            match lines.get(i) {
                Some(line) if matches!(line.kind, BlockKind::Code | BlockKind::FenceDelimiter) => {
                    let is = |j: usize, open: bool| match &base
                        .get(j)
                        .and_then(|s| s.decoration.clone())
                    {
                        Some(Decoration::Code(CodePart::Open { .. })) => open,
                        Some(Decoration::Code(CodePart::Close)) => !open,
                        _ => false,
                    };
                    let first = (0..=i).rev().find(|&j| is(j, true));
                    let last = (i..lines.len()).find(|&j| is(j, false));
                    for j in first.into_iter().chain(last) {
                        restyle(&lines[j], 0);
                    }
                }
                // In the Properties table: the whole Frontmatter shows as YAML.
                Some(line)
                    if matches!(
                        line.kind,
                        BlockKind::Frontmatter | BlockKind::FrontmatterDelimiter
                    ) =>
                {
                    for l in lines.iter().take_while(|l| {
                        matches!(
                            l.kind,
                            BlockKind::Frontmatter | BlockKind::FrontmatterDelimiter
                        )
                    }) {
                        let at = if l.index == i { cursor - l.start } else { 0 };
                        restyle(l, at);
                    }
                }
                Some(line) => restyle(line, cursor - line.start),
                None => {}
            }
            self.styles = (Some(key), Rc::new(styles));
        }
        self.styles.1.clone()
    }

    /// The base styles with each closed `$$` block and `mermaid` fence drawn
    /// as its figure on its first line (the others folded), and each formula of
    /// a line drawn in its text. A formula RaTeX cannot typeset, or a diagram
    /// not drawn yet (drawn in the background, then the styles again), stays
    /// as written.
    pub(super) fn with_figures(
        &mut self,
        plain: &[Rc<LineStyle>],
        cx: &mut Context<Self>,
    ) -> Vec<LineStyle> {
        let text = self.buf.text();
        let lines = markdown::lines(text);
        let cursor_line = line_of_offset(text, self.buf.cursor());
        let dark = self.palette.dark;
        // Only the formulas of this text are kept: none of every key typed.
        let mut formulas = std::mem::take(&mut self.formulas);
        let mut used = HashMap::new();
        let mut typeset_once = |tex: &str, display: bool| {
            let key = (tex.to_string(), display);
            let f = formulas
                .remove(&key)
                .or_else(|| used.get(&key).cloned())
                .unwrap_or_else(|| typeset(tex, display).map(Rc::new));
            used.insert(key, f.clone());
            f
        };
        self.deferred_diagram = None;
        let mut out: Vec<LineStyle> = plain.iter().map(|s| (**s).clone()).collect();
        let mut figure_lines = Vec::new();
        let mut to_draw = Vec::new();
        for block in figure_blocks(&lines) {
            let decoration = match &block.source {
                FigureSource::Math(tex) => typeset_once(tex, true).map(Decoration::Formula),
                FigureSource::Mermaid(source) => {
                    let id = diagram::diagram_id(source, dark);
                    match self.diagrams.get(&id) {
                        Some(Some(d)) => Some(Decoration::Diagram(d.clone())),
                        Some(None) => None,
                        None if block.lines.contains(&cursor_line) => {
                            self.deferred_diagram = Some(block.lines.clone());
                            None
                        }
                        None => {
                            self.diagrams.insert(id, None);
                            to_draw.push((id, source.clone()));
                            None
                        }
                    }
                }
            };
            let Some(decoration) = decoration else {
                continue;
            };
            for j in block.lines.clone() {
                out[j] = LineStyle {
                    decoration: Some(if j == block.lines.start {
                        decoration.clone()
                    } else {
                        Decoration::Folded
                    }),
                    hidden: vec![0..lines[j].text.len()],
                    ..LineStyle::default()
                };
            }
            figure_lines.push(block.lines);
        }
        for line in &lines {
            let style = &mut out[line.index];
            let drawn_apart = matches!(
                style.decoration,
                Some(
                    Decoration::Table(_)
                        | Decoration::Property(_)
                        | Decoration::Code(_)
                        | Decoration::Image(_)
                        | Decoration::Formula(_)
                        | Decoration::Diagram(_)
                        | Decoration::Folded
                )
            );
            if line.kind != BlockKind::Text || drawn_apart || !line.text.contains('$') {
                continue;
            }
            for (range, inner, display) in inline_formulas(line.text) {
                if let Some(f) = typeset_once(&line.text[inner], display) {
                    hide(&mut style.hidden, range.clone());
                    style.formulas.push((range, f));
                } else {
                    // As written, in red.
                    style
                        .segments
                        .push((range, vec![markdown::MdSpanKind::MathError]));
                }
            }
        }
        self.formulas = used;
        self.figure_lines = figure_lines;
        for (id, source) in to_draw {
            cx.spawn(async move |this, cx| {
                let drawn = cx
                    .background_executor()
                    .spawn(async move { diagram::render(&source, dark) })
                    .await;
                let _ = this.update(cx, |ed, cx| {
                    let d = drawn.map(|(svg, width, height)| {
                        let image = gpui_kit::Image::from_bytes(
                            gpui_kit::ImageFormat::Svg,
                            svg.into_bytes(),
                        );
                        ed.diagram_images.insert(id, Arc::new(image));
                        Rc::new(Diagram { id, width, height })
                    });
                    ed.diagrams.insert(id, d);
                    // The styles again, with the diagram.
                    ed.base_styles.0 = None;
                    ed.styles.0 = None;
                    cx.notify();
                });
            })
            .detach();
        }
        out
    }
}

/// Hides `range` too, the hidden ranges kept sorted and disjoint.
pub(super) fn hide(hidden: &mut Vec<Range<usize>>, range: Range<usize>) {
    hidden.push(range);
    hidden.sort_by_key(|r| r.start);
    let mut merged: Vec<Range<usize>> = Vec::with_capacity(hidden.len());
    for r in hidden.drain(..) {
        match merged.last_mut() {
            Some(last) if r.start <= last.end => last.end = last.end.max(r.end),
            _ => merged.push(r),
        }
    }
    *hidden = merged;
}

/// `keep_equal` for styles already shared (`BlockStyles`): the same one is
/// kept without comparing it.
pub(super) fn keep_equal_rc(prev: &[Rc<LineStyle>], new: Vec<Rc<LineStyle>>) -> Vec<Rc<LineStyle>> {
    new.into_iter()
        .enumerate()
        .map(|(i, s)| match prev.get(i) {
            Some(p) if Rc::ptr_eq(p, &s) || **p == *s => p.clone(),
            _ => s,
        })
        .collect()
}

/// `new`, where a line's style equal to the one in `prev` keeps `prev`'s `Rc`,
/// so the element sees at a glance that the line needs no shaping.
pub(super) fn keep_equal(prev: &[Rc<LineStyle>], new: Vec<LineStyle>) -> Vec<Rc<LineStyle>> {
    new.into_iter()
        .enumerate()
        .map(|(i, s)| match prev.get(i) {
            Some(p) if **p == s => p.clone(),
            _ => Rc::new(s),
        })
        .collect()
}
