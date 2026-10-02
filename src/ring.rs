//! Active element visual feedback: glowing squared-off rings and focus indicators.

use libmsdf::drawlist::{DrawList, SdfInstance, SdfKind};

use crate::rect::Rect;
use crate::theme::ControlTheme;

/// Configuration for drawing a glowing squared-off focus ring around active elements.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct GlowRingStyle {
    /// Padding between the target bounds and the inner ring (logical pixels).
    pub padding: f32,
    /// Corner radius for the squared-off ring (0.0 = sharp, 1.0..2.0 = slightly softened).
    pub corner_radius: f32,
    /// Stroke thickness of the crisp inner ring.
    pub inner_thickness: f32,
    /// Stroke thickness of the diffuse outer halo bloom.
    pub outer_thickness: f32,
    /// Explicit inner ring color (falls back to theme if `None`).
    pub inner_color: Option<[f32; 4]>,
    /// Explicit outer bloom color (falls back to theme if `None`).
    pub outer_color: Option<[f32; 4]>,
}

impl Default for GlowRingStyle {
    fn default() -> Self {
        Self {
            padding: 2.0,
            corner_radius: 1.0,
            inner_thickness: 1.2,
            outer_thickness: 2.8,
            inner_color: None,
            outer_color: None,
        }
    }
}

impl GlowRingStyle {
    /// Create a compact squared-off focus ring style.
    pub fn compact() -> Self {
        Self {
            padding: 1.5,
            corner_radius: 0.5,
            inner_thickness: 1.0,
            outer_thickness: 2.2,
            inner_color: None,
            outer_color: None,
        }
    }

    /// Create a sharp, zero-radius squared-off ring style.
    pub fn sharp() -> Self {
        Self {
            corner_radius: 0.0,
            ..Self::default()
        }
    }
}

/// Draw a small glowing squared-off ring around `target_rect`.
pub fn draw_glowing_ring(
    list: &mut DrawList,
    target_rect: Rect,
    style: &GlowRingStyle,
    theme: &ControlTheme,
) {
    let inner_col = style.inner_color.unwrap_or(theme.active_ring_color);
    let outer_col = style.outer_color.unwrap_or(theme.active_glow_color);

    let pad = style.padding;
    let rad = style.corner_radius;

    let halo_offset = (style.outer_thickness - style.inner_thickness) * 0.5;
    let outer_rect = target_rect.pad(pad + halo_offset);
    list.push(SdfInstance {
        kind: SdfKind::Outline {
            radius: (rad + halo_offset).max(0.0),
            thickness: style.outer_thickness,
        },
        position: outer_rect.pos(),
        size: outer_rect.size(),
        color: outer_col,
        anim: 0,
    });

    let inner_rect = target_rect.pad(pad);
    list.push(SdfInstance {
        kind: SdfKind::Outline {
            radius: rad,
            thickness: style.inner_thickness,
        },
        position: inner_rect.pos(),
        size: inner_rect.size(),
        color: inner_col,
        anim: 0,
    });
}
