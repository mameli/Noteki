//! Live-preview styles: how each Markdown span is drawn in the editor.

use gpui_kit::{px, FontStyle, FontWeight, HighlightStyle, StrikethroughStyle, UnderlineStyle};

use crate::core::markdown::MdSpanKind;
use crate::theme::NotekiPalette;

pub fn span_style(kind: MdSpanKind, p: &NotekiPalette) -> HighlightStyle {
    let underline = |color, wavy| {
        Some(UnderlineStyle {
            thickness: px(1.),
            color: Some(color),
            wavy,
        })
    };
    match kind {
        MdSpanKind::SyntaxMarker => HighlightStyle {
            color: Some(p.syntax),
            fade_out: Some(0.6),
            ..Default::default()
        },
        MdSpanKind::FrontmatterDelimiter => HighlightStyle {
            color: Some(p.syntax),
            ..Default::default()
        },
        MdSpanKind::FrontmatterKey => HighlightStyle {
            color: Some(p.keyword),
            font_weight: Some(FontWeight::MEDIUM),
            ..Default::default()
        },
        MdSpanKind::Heading(level) => HighlightStyle {
            color: Some(p.heading),
            font_weight: Some(if level <= 2 {
                FontWeight::BOLD
            } else {
                FontWeight::SEMIBOLD
            }),
            ..Default::default()
        },
        MdSpanKind::Bold => HighlightStyle {
            font_weight: Some(FontWeight::BOLD),
            ..Default::default()
        },
        MdSpanKind::Italic => HighlightStyle {
            font_style: Some(FontStyle::Italic),
            ..Default::default()
        },
        MdSpanKind::TagChip => HighlightStyle {
            color: Some(p.link),
            background_color: Some(gpui_kit::hsla(p.link.h, p.link.s, p.link.l, 0.15)),
            ..Default::default()
        },
        MdSpanKind::Footnote => HighlightStyle {
            color: Some(p.link),
            font_weight: Some(FontWeight::MEDIUM),
            ..Default::default()
        },
        MdSpanKind::Strikethrough | MdSpanKind::TaskDone => HighlightStyle {
            color: Some(p.syntax),
            strikethrough: Some(StrikethroughStyle {
                thickness: px(1.),
                color: Some(p.syntax),
            }),
            ..Default::default()
        },
        MdSpanKind::Highlight => HighlightStyle {
            background_color: Some(p.highlight),
            ..Default::default()
        },
        MdSpanKind::InlineCode => HighlightStyle {
            background_color: Some(p.code_bg),
            ..Default::default()
        },
        // A code block's box is painted by the element, across its width.
        MdSpanKind::CodeBlock => HighlightStyle::default(),
        MdSpanKind::CodeComment => HighlightStyle {
            color: Some(p.syntax),
            font_style: Some(FontStyle::Italic),
            ..Default::default()
        },
        MdSpanKind::CodeString => HighlightStyle {
            color: Some(p.string),
            ..Default::default()
        },
        MdSpanKind::CodeNumber => HighlightStyle {
            color: Some(p.number),
            ..Default::default()
        },
        MdSpanKind::CodeKeyword => HighlightStyle {
            color: Some(p.keyword),
            ..Default::default()
        },
        MdSpanKind::CodeType => HighlightStyle {
            color: Some(p.heading),
            ..Default::default()
        },
        MdSpanKind::CodeFunction => HighlightStyle {
            color: Some(p.link),
            ..Default::default()
        },
        MdSpanKind::WikilinkResolved | MdSpanKind::MarkdownLink => HighlightStyle {
            color: Some(p.link),
            underline: underline(p.link, false),
            ..Default::default()
        },
        // SPEC: an Unresolved link is drawn faded, as in Obsidian.
        MdSpanKind::WikilinkUnresolved => HighlightStyle {
            color: Some(p.link),
            fade_out: Some(0.45),
            underline: underline(p.link, false),
            ..Default::default()
        },
        MdSpanKind::Tag => HighlightStyle {
            color: Some(p.accent),
            font_weight: Some(FontWeight::MEDIUM),
            ..Default::default()
        },
        MdSpanKind::MathError => HighlightStyle {
            color: Some(p.error),
            ..Default::default()
        },
        MdSpanKind::FrontmatterError => HighlightStyle {
            underline: underline(p.error, true),
            ..Default::default()
        },
        MdSpanKind::Comment => HighlightStyle {
            color: Some(p.syntax),
            font_style: Some(FontStyle::Italic),
            fade_out: Some(0.3),
            ..Default::default()
        },
        MdSpanKind::Quote => HighlightStyle {
            color: Some(p.quote),
            font_style: Some(FontStyle::Italic),
            ..Default::default()
        },
        // The callout's box gives it its colour (`element::callout_color`).
        MdSpanKind::CalloutHeader => HighlightStyle {
            font_weight: Some(FontWeight::SEMIBOLD),
            ..Default::default()
        },
    }
}
