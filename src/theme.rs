//! Visual styling, palettes, and typography metrics for UI controls.

/// Visual theme specifying colors, margins, and geometric styles for UI controls.
#[derive(Clone, Debug, PartialEq)]
pub struct ControlTheme {
    /// Background color for panels and bounded controls `[R, G, B, A]`.
    pub bg_color: [f32; 4],
    /// Primary foreground / label text color `[R, G, B, A]`.
    pub text_color: [f32; 4],
    /// Secondary / muted text color `[R, G, B, A]`.
    pub muted_text_color: [f32; 4],
    /// Primary accent / highlight color `[R, G, B, A]`.
    pub accent_color: [f32; 4],
    /// Unfilled slider track background `[R, G, B, A]`.
    pub track_bg_color: [f32; 4],
    /// Filled slider track active progress `[R, G, B, A]`.
    pub track_fill_color: [f32; 4],
    /// Slider handle / thumb fill color `[R, G, B, A]`.
    pub thumb_color: [f32; 4],
    /// Slider handle / thumb border outline color `[R, G, B, A]`.
    pub thumb_border_color: [f32; 4],
    /// Arrow chevron glyph/vector stroke color `[R, G, B, A]`.
    pub arrow_color: [f32; 4],
    /// Crisp inner outline color for active glowing ring `[R, G, B, A]`.
    pub active_ring_color: [f32; 4],
    /// Diffuse outer halo bloom color for active glowing ring `[R, G, B, A]`.
    pub active_glow_color: [f32; 4],
    /// Default font size in logical screen pixels.
    pub default_font_size: f32,
    /// Distance field antialiasing pixel range for MSDF text.
    pub px_range: f32,
    /// Default corner radius for rounded rects and control boundaries.
    pub corner_radius: f32,
}

impl Default for ControlTheme {
    fn default() -> Self {
        Self {
            bg_color: [0.08, 0.10, 0.14, 0.85],
            text_color: [0.92, 0.95, 0.98, 1.0],
            muted_text_color: [0.55, 0.62, 0.70, 1.0],
            accent_color: [0.18, 0.72, 0.98, 1.0],
            track_bg_color: [0.15, 0.18, 0.24, 0.9],
            track_fill_color: [0.18, 0.72, 0.98, 0.95],
            thumb_color: [0.95, 0.98, 1.0, 1.0],
            thumb_border_color: [0.18, 0.72, 0.98, 1.0],
            arrow_color: [0.75, 0.85, 0.95, 1.0],
            active_ring_color: [0.25, 0.88, 1.0, 0.95],
            active_glow_color: [0.15, 0.70, 0.98, 0.35],
            default_font_size: 14.0,
            px_range: 6.0,
            corner_radius: 2.0,
        }
    }
}

impl ControlTheme {
    /// Dark fantasy / cyberpunk cyan theme.
    pub fn dark_fantasy() -> Self {
        Self::default()
    }

    /// Gold / solar royalty theme.
    pub fn solar_gold() -> Self {
        Self {
            accent_color: [0.98, 0.78, 0.22, 1.0],
            track_fill_color: [0.95, 0.72, 0.18, 0.95],
            thumb_border_color: [0.98, 0.82, 0.30, 1.0],
            arrow_color: [0.98, 0.90, 0.60, 1.0],
            active_ring_color: [1.0, 0.85, 0.30, 0.95],
            active_glow_color: [0.95, 0.70, 0.15, 0.35],
            ..Self::default()
        }
    }
}
