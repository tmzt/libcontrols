//! The faces Markdown is drawn and measured with (the plan's decision 10),
//! moved from flankr's app: libcontrols' `DrawContext` carries one face, so
//! bold, italic and code come in a [`MarkdownFaces`] the app makes once.

use std::fmt;

use libmsdf::drawlist::{AtlasId, AtlasRef, DrawList};
use libmsdf::font::{FontAtlas, GlyphStyle, TerminalShaper, TextShaper};

use crate::traits::DrawContext;

use super::model::MdStyle;

/// One prose face: its atlas in the renderer's set (metrics only, see
/// [`MarkdownFaces::metrics`]), and its shaper.
pub struct Face {
    pub id: AtlasId,
    pub atlas: FontAtlas,
    pub shaper: TextShaper,
}

impl fmt::Debug for Face {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Face")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

/// The terminal face, for `Code`.
pub struct CodeFace {
    pub shaper: TerminalShaper,
    pub text: (AtlasId, FontAtlas),
    pub symbols: (AtlasId, FontAtlas),
}

impl fmt::Debug for CodeFace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CodeFace")
            .field("text", &self.text.0)
            .field("symbols", &self.symbols.0)
            .finish_non_exhaustive()
    }
}

/// The faces Markdown is drawn with: Regular, Bold and Italic prose
/// (`BoldItalic` falls back to `bold`) and the terminal face for code.
#[derive(Debug)]
pub struct MarkdownFaces {
    pub regular: Face,
    pub bold: Face,
    pub italic: Face,
    pub code: CodeFace,
    pub px_range: f32,
}

impl MarkdownFaces {
    /// `atlas` without its texels: drawing reads only the glyph metrics, and
    /// the texels are on the GPU through the renderer's atlas set.
    pub fn metrics(mut atlas: FontAtlas) -> FontAtlas {
        atlas.pixel_data = Vec::new();
        atlas
    }

    pub(crate) fn prose(&self, style: MdStyle) -> &Face {
        match style {
            MdStyle::Bold | MdStyle::BoldItalic => &self.bold,
            MdStyle::Italic => &self.italic,
            MdStyle::Regular | MdStyle::Code => &self.regular,
        }
    }
}

/// Draw `text` in `style` at `pos` (its line's top left), at `px`: in its
/// face when `faces` are given, else in the context's font. Returns nothing:
/// callers place runs by their own measure.
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_run(
    list: &mut DrawList,
    ctx: &DrawContext,
    faces: Option<&MarkdownFaces>,
    text: &str,
    style: MdStyle,
    pos: [f32; 2],
    px: f32,
    color: [f32; 4],
) {
    let Some(f) = faces else {
        ctx.draw_text(list, text, pos, px, color);
        return;
    };
    if style == MdStyle::Code {
        let c = &f.code;
        let line = c.shaper.line(text, GlyphStyle::Regular);
        let t = AtlasRef {
            id: c.text.0,
            atlas: &c.text.1,
        };
        let s = AtlasRef {
            id: c.symbols.0,
            atlas: &c.symbols.1,
        };
        list.push_terminal_line(&line, t, s, pos, px, f.px_range, color);
        return;
    }
    let face = f.prose(style);
    let run = face.shaper.shape(text);
    let atlas = AtlasRef {
        id: face.id,
        atlas: &face.atlas,
    };
    list.push_shaped_text_with(&run, atlas, pos, px, f.px_range, color);
}
