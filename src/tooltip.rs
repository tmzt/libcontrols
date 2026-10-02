//! Two-letter cell labels with tooltip descriptions.

use libmsdf::drawlist::{DrawList, LINE_BOX_RATIO, SdfInstance, SdfKind};

use crate::rect::Rect;
use crate::traits::DrawContext;

/// What separates a cell's abbreviation from its description.
pub const LABEL_SEPARATOR: &str = " - ";

/// Split a label into the text a cell draws and the tooltip behind it.
#[must_use]
pub fn split_label(label: &str) -> (&str, Option<&str>) {
    let Some((shown, tip)) = label.split_once(LABEL_SEPARATOR) else {
        return (label, None);
    };
    let (shown, tip) = (shown.trim(), tip.trim());
    if shown.is_empty() {
        return (label, None);
    }
    if tip.is_empty() {
        return (shown, None);
    }
    (shown, Some(tip))
}

/// How a tooltip box is drawn, and what it is kept inside.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct TooltipStyle {
    /// Font size for the description, or the context's default.
    pub font_size: Option<f32>,
    /// Space between the text and the box's edge.
    pub padding: f32,
    /// Space between the hovered cell and the box.
    pub gap: f32,
    /// Corner radius of the box.
    pub corner_radius: f32,
    /// Background fill.
    pub bg_color: [f32; 4],
    /// Border outline colour.
    pub border_color: [f32; 4],
    /// Border thickness, or no border at zero.
    pub border_thickness: f32,
    /// Description colour.
    pub text_color: [f32; 4],
    /// How far the shadow behind the box is offset, down and to the right.
    pub shadow_offset: f32,
    /// The shadow's colour.
    pub shadow_color: [f32; 4],
    /// The rect the box is kept inside, which is the panel it is drawn in.
    pub bounds: Option<Rect>,
}

impl Default for TooltipStyle {
    fn default() -> Self {
        Self {
            font_size: None,
            padding: 4.0,
            gap: 8.0,
            corner_radius: 3.0,
            bg_color: [0.01, 0.03, 0.05, 0.98],
            border_color: [0.58, 0.94, 1.0, 0.95],
            border_thickness: 1.0,
            text_color: [0.96, 0.99, 1.0, 1.0],
            shadow_offset: 2.0,
            shadow_color: [0.0, 0.01, 0.02, 0.50],
            bounds: None,
        }
    }
}

impl TooltipStyle {
    /// Set the description's font size.
    #[must_use]
    pub fn with_font_size(mut self, font_size: f32) -> Self {
        self.font_size = Some(font_size);
        self
    }

    /// Set the padding inside the box and the gap outside it.
    #[must_use]
    pub fn with_spacing(mut self, padding: f32, gap: f32) -> Self {
        self.padding = padding;
        self.gap = gap;
        self
    }

    /// Set the corner radius.
    #[must_use]
    pub fn with_corner_radius(mut self, corner_radius: f32) -> Self {
        self.corner_radius = corner_radius;
        self
    }

    /// Set the background fill.
    #[must_use]
    pub fn with_bg(mut self, bg_color: [f32; 4]) -> Self {
        self.bg_color = bg_color;
        self
    }

    /// Set the border colour and thickness.
    #[must_use]
    pub fn with_border(mut self, border_color: [f32; 4], thickness: f32) -> Self {
        self.border_color = border_color;
        self.border_thickness = thickness;
        self
    }

    /// Set the border thickness, keeping the colour.
    #[must_use]
    pub fn with_border_thickness(mut self, thickness: f32) -> Self {
        self.border_thickness = thickness;
        self
    }

    /// Set the description's colour.
    #[must_use]
    pub fn with_text_color(mut self, text_color: [f32; 4]) -> Self {
        self.text_color = text_color;
        self
    }

    /// Set how far the shadow is offset, and its colour.
    #[must_use]
    pub fn with_shadow(mut self, offset: f32, color: [f32; 4]) -> Self {
        self.shadow_offset = offset;
        self.shadow_color = color;
        self
    }

    /// Set how far the shadow is offset, keeping its colour.
    #[must_use]
    pub fn with_shadow_offset(mut self, offset: f32) -> Self {
        self.shadow_offset = offset;
        self
    }

    /// Name the rect the box is kept inside.
    #[must_use]
    pub fn within(mut self, bounds: Rect) -> Self {
        self.bounds = Some(bounds);
        self
    }

    /// Where a box of `size` goes for a tip on `anchor`.
    #[must_use]
    pub fn place(&self, anchor: Rect, size: [f32; 2]) -> Rect {
        let [w, h] = size;
        let mut x = anchor.center()[0] - w * 0.5;
        let mut y = anchor.bottom() + self.gap;

        if let Some(frame) = self.bounds {
            if y + h > frame.bottom() {
                let above = anchor.y - self.gap - h;
                if above >= frame.y {
                    y = above;
                }
            }
            x = x.min(frame.right() - w).max(frame.x);
            y = y.min(frame.bottom() - h).max(frame.y);
        }

        Rect::new(x, y, w, h)
    }
}

/// Draw a tooltip for `anchor` reading `text`.
pub fn draw_tooltip(
    list: &mut DrawList,
    ctx: &DrawContext,
    anchor: Rect,
    text: &str,
    style: &TooltipStyle,
) {
    if text.is_empty() {
        return;
    }
    let font_size = ctx.font_size(style.font_size);
    let line_box_h = font_size * LINE_BOX_RATIO;
    let advance = ctx.text_advance(text, font_size);
    let box_rect = style.place(
        anchor,
        [
            advance + style.padding * 2.0,
            line_box_h + style.padding * 2.0,
        ],
    );

    let kind = if style.corner_radius > 0.0 {
        SdfKind::RoundedBox {
            radius: style.corner_radius,
        }
    } else {
        SdfKind::Box
    };

    if style.shadow_offset > 0.0 && style.shadow_color[3] > 0.0 {
        list.push(SdfInstance {
            kind,
            position: [
                box_rect.x + style.shadow_offset,
                box_rect.y + style.shadow_offset,
            ],
            size: box_rect.size(),
            color: style.shadow_color,
            anim: 0,
        });
    }

    list.push(SdfInstance {
        kind,
        position: box_rect.pos(),
        size: box_rect.size(),
        color: style.bg_color,
        anim: 0,
    });

    if style.border_thickness > 0.0 {
        list.push(SdfInstance {
            kind: SdfKind::Outline {
                radius: style.corner_radius,
                thickness: style.border_thickness,
            },
            position: box_rect.pos(),
            size: box_rect.size(),
            color: style.border_color,
            anim: 0,
        });
    }

    ctx.draw_text(
        list,
        text,
        [box_rect.x + style.padding, box_rect.y + style.padding],
        font_size,
        style.text_color,
    );
}
