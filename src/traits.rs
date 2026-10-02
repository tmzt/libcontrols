//! Core traits and execution context for stateless UI rendering.

use std::fmt;

use libmsdf::drawlist::DrawList;
use libmsdf::font::{FontAtlas, ShapedRun, TextShaper};

use crate::theme::ControlTheme;

/// Stateless drawing contract for all UI controls.
pub trait DrawControl {
    /// Render this control into the target MSDF `DrawList`.
    fn draw(&self, list: &mut DrawList, ctx: &DrawContext);

    /// Render whatever this control puts *over* its neighbours, after every control in the host
    /// has had its turn at [`DrawControl::draw`].
    fn draw_overlay(&self, _list: &mut DrawList, _ctx: &DrawContext) {}
}

/// Rendering context providing font shaping, atlas metrics, and active theme.
#[derive(Clone)]
pub struct DrawContext<'a> {
    /// Optional MSDF font atlas containing baked glyph raster distance fields.
    pub atlas: Option<&'a FontAtlas>,
    /// Optional text shaper for converting Unicode text to positioned glyphs.
    pub shaper: Option<&'a TextShaper>,
    /// Active visual theme.
    pub theme: ControlTheme,
    /// Distance field antialiasing pixel range for text.
    pub px_range: f32,
}

impl<'a> fmt::Debug for DrawContext<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DrawContext")
            .field("has_atlas", &self.atlas.is_some())
            .field("has_shaper", &self.shaper.is_some())
            .field("theme", &self.theme)
            .field("px_range", &self.px_range)
            .finish()
    }
}

impl<'a> Default for DrawContext<'a> {
    fn default() -> Self {
        Self {
            atlas: None,
            shaper: None,
            theme: ControlTheme::default(),
            px_range: 6.0,
        }
    }
}

impl<'a> DrawContext<'a> {
    /// Create a new drawing context with optional atlas and shaper.
    pub fn new(atlas: Option<&'a FontAtlas>, shaper: Option<&'a TextShaper>) -> Self {
        Self {
            atlas,
            shaper,
            theme: ControlTheme::default(),
            px_range: 6.0,
        }
    }

    /// Set a custom theme.
    pub fn with_theme(mut self, theme: ControlTheme) -> Self {
        self.theme = theme;
        self
    }

    /// Set a custom distance field pixel range.
    pub fn with_px_range(mut self, px_range: f32) -> Self {
        self.px_range = px_range;
        self
    }

    /// Resolve effective font size.
    pub fn font_size(&self, override_sz: Option<f32>) -> f32 {
        override_sz.unwrap_or(self.theme.default_font_size)
    }

    /// Resolve effective text color.
    pub fn text_color(&self, override_col: Option<[f32; 4]>) -> [f32; 4] {
        override_col.unwrap_or(self.theme.text_color)
    }

    /// Shape a string using the context's shaper.
    pub fn shape_text(&self, text: &str) -> Option<ShapedRun> {
        self.shaper.map(|s| s.shape(text))
    }

    /// Compute horizontal advance width in pixels for a text string.
    pub fn text_advance(&self, text: &str, font_size: f32) -> f32 {
        if let Some(run) = self.shape_text(text) {
            run.advance_px(font_size)
        } else {
            text.chars().count() as f32 * font_size * 0.55
        }
    }

    /// Draw a text string at `pos` (pen origin) and return its advance width in pixels.
    pub fn draw_text(
        &self,
        list: &mut DrawList,
        text: &str,
        pos: [f32; 2],
        font_size: f32,
        color: [f32; 4],
    ) -> f32 {
        if let (Some(atlas), Some(shaper)) = (self.atlas, self.shaper) {
            let run = shaper.shape(text);
            list.push_shaped_text(&run, atlas, pos, font_size, self.px_range, color)
        } else {
            self.text_advance(text, font_size)
        }
    }
}
