//! [`MarkdownView`], moved from flankr's app (the plan's decision 9): a
//! column of Markdown laid out at construction and drawn as runs in their
//! faces. A tap on a link answers
//! `ControlAction::Activated { part: ControlPart::Custom(<its index>) }`.

use std::cell::Cell;
use std::sync::Arc;

use libmsdf::drawlist::{DrawList, SdfInstance, SdfKind};

use crate::control::HostedControl;
use crate::event::{
    ControlAction, ControlPart, EventContext, KeyboardEvent, MouseButton, MouseEvent,
};
use crate::rect::Rect;
use crate::theme::{ControlTheme, MarkdownOptions, MarkdownResolved, MarkdownTheme};
use crate::traits::{DrawContext, DrawControl};

use super::faces::{MarkdownFaces, draw_run};
use super::layout::{MarkdownMeasure, lay_out};
use super::model::{MdLaid, MdStyle};
use super::parse_with_gaps;

/// A run where it is drawn.
#[derive(Clone, Debug, PartialEq)]
struct Placed {
    text: String,
    style: MdStyle,
    link: Option<usize>,
    dim: bool,
    x: f32,
    y: f32,
    w: f32,
}

/// Markdown laid out in a column, as a control.
///
/// It parses and lays out when it is built and at [`set_text`] and
/// [`set_width`], measured by its faces or, without them, at `0.55` a
/// character (code at the theme's `code_cell_em`). Its sizes come from
/// `ControlTheme::default()` until it is first drawn, then from the theme it
/// was last drawn with; its colours always from the theme it is drawn with.
///
/// [`set_text`]: MarkdownView::set_text
/// [`set_width`]: MarkdownView::set_width
#[derive(Debug)]
pub struct MarkdownView {
    source: String,
    laid: MdLaid,
    placed: Vec<Placed>,
    rect: Rect,
    options: MarkdownOptions,
    faces: Option<Arc<MarkdownFaces>>,
    px: f32,
    /// The options resolved at the last layout.
    resolved: MarkdownResolved,
    /// The `markdown` theme and default font size it lays out with: the
    /// default theme's until drawn, then the last drawn theme's.
    theme: Cell<(MarkdownTheme, f32)>,
}

impl MarkdownView {
    /// `md` at `origin`, `width` wide.
    pub fn new(md: &str, origin: [f32; 2], width: f32, options: MarkdownOptions) -> Self {
        let t = ControlTheme::default();
        let mut view = Self {
            source: md.to_string(),
            laid: MdLaid::default(),
            placed: Vec::new(),
            rect: Rect::new(origin[0], origin[1], width, 0.0),
            options,
            faces: None,
            px: 0.0,
            resolved: MarkdownResolved::default(),
            theme: Cell::new((t.markdown, t.default_font_size)),
        };
        view.lay_out();
        view
    }

    /// Draw and measure in `faces` (laid out again).
    #[must_use]
    pub fn with_faces(mut self, faces: Arc<MarkdownFaces>) -> Self {
        self.faces = Some(faces);
        self.lay_out();
        self
    }

    /// Show `md` instead (laid out again, at the same place and width).
    pub fn set_text(&mut self, md: &str) {
        self.source = md.to_string();
        self.lay_out();
    }

    /// Lay out again at `width`.
    pub fn set_width(&mut self, width: f32) {
        self.rect.w = width;
        self.lay_out();
    }

    fn line_h(&self) -> f32 {
        self.px * self.resolved.line_height
    }

    fn lay_out(&mut self) {
        let (theme, default_size) = self.theme.get();
        let r = self.options.with_theme(&theme);
        self.resolved = r;
        self.px = r.font_size.unwrap_or(default_size);
        let doc = parse_with_gaps(&self.source, r.block_gap, r.item_gap);
        let m = MarkdownMeasure::new(self.faces.as_deref(), self.px, &r);
        self.laid = lay_out(&doc, self.rect.w, &m);
        let lh = self.line_h();
        let (x, y) = (self.rect.x, self.rect.y);
        self.placed.clear();
        let mut cy = y;
        for line in &self.laid.lines {
            cy += line.gap_before * lh;
            let mut cx = x + line.indent;
            for run in &line.runs {
                let w = m.width(&run.text, run.style);
                self.placed.push(Placed {
                    text: run.text.clone(),
                    style: run.style,
                    link: run.link,
                    dim: run.dim,
                    x: cx,
                    y: cy,
                    w,
                });
                cx += w;
            }
            cy += lh;
        }
        self.rect.h = cy - y;
        if self.options.centered {
            self.center();
        }
    }

    pub fn height(&self) -> f32 {
        self.rect.h
    }

    pub fn rect(&self) -> Rect {
        self.rect
    }

    /// Its type size, in pixels.
    pub fn px(&self) -> f32 {
        self.px
    }

    /// The Markdown it shows.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Its lines' text, one a line.
    pub fn text(&self) -> String {
        self.laid
            .lines
            .iter()
            .map(|l| l.runs.iter().map(|r| r.text.as_str()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// The links its runs index.
    pub fn links(&self) -> &[String] {
        &self.laid.links
    }

    /// Moves it, keeping its size.
    pub fn move_to(&mut self, x: f32, y: f32) {
        let (dx, dy) = (x - self.rect.x, y - self.rect.y);
        for p in &mut self.placed {
            p.x += dx;
            p.y += dy;
        }
        self.rect.x = x;
        self.rect.y = y;
    }

    /// Centres each line in its width (as `MarkdownOptions::centered` does
    /// at every layout).
    pub fn center(&mut self) {
        let mut i = 0;
        while i < self.placed.len() {
            let y = self.placed[i].y;
            let end = self.placed[i..]
                .iter()
                .position(|p| p.y != y)
                .map_or(self.placed.len(), |n| i + n);
            let line = &self.placed[i..end];
            let left = line.iter().map(|p| p.x).fold(f32::INFINITY, f32::min);
            let right = line
                .iter()
                .map(|p| p.x + p.w)
                .fold(f32::NEG_INFINITY, f32::max);
            let dx = self.rect.x + (self.rect.w - (right - left)) / 2.0 - left;
            for p in &mut self.placed[i..end] {
                p.x += dx;
            }
            i = end;
        }
    }

    /// A link run's tap area: a little taller than its line.
    fn tap_rect(&self, p: &Placed) -> Rect {
        let h = self.line_h();
        Rect::new(
            p.x,
            p.y - h * self.resolved.link_tap_above,
            p.w,
            h * self.resolved.link_tap_height,
        )
    }

    /// The link at `pos`, if any.
    pub fn link_at(&self, pos: [f32; 2]) -> Option<usize> {
        self.placed
            .iter()
            .filter(|p| p.link.is_some())
            .find(|p| self.tap_rect(p).contains(pos))
            .and_then(|p| p.link)
    }

    /// Where link `index`'s first run is.
    pub fn link_rect(&self, index: usize) -> Option<Rect> {
        self.placed
            .iter()
            .find(|p| p.link == Some(index))
            .map(|p| Rect::new(p.x, p.y, p.w, self.line_h()))
    }

    /// A run's colour against `theme`: a link's, a quote's, code's, prose.
    fn color_of(&self, p: &Placed, r: &MarkdownResolved, theme: &ControlTheme) -> [f32; 4] {
        if p.link.is_some() {
            r.link_color_in(theme)
        } else if p.dim {
            r.dim_color_in(theme)
        } else if p.style == MdStyle::Code {
            r.code_color_in(theme)
        } else {
            r.text_color_in(theme)
        }
    }
}

impl DrawControl for MarkdownView {
    fn draw(&self, list: &mut DrawList, ctx: &DrawContext) {
        self.theme
            .set((ctx.theme.markdown, ctx.theme.default_font_size));
        let r = self.options.with_theme(&ctx.theme.markdown);
        let faces = self.faces.as_deref();
        let h = self.line_h();
        for p in &self.placed {
            let color = self.color_of(p, &r, &ctx.theme);
            draw_run(
                list,
                ctx,
                faces,
                &p.text,
                p.style,
                [p.x, p.y],
                self.px,
                color,
            );
            if p.link.is_some() && !p.text.trim().is_empty() {
                let thick = (self.px * r.underline_thickness).max(1.0);
                list.push(SdfInstance {
                    kind: SdfKind::Box,
                    position: [p.x, p.y + h * r.underline_at - thick],
                    size: [p.w, thick],
                    color,
                    anim: 0,
                });
            }
        }
    }
}

impl HostedControl for MarkdownView {
    fn bounds(&self) -> Rect {
        self.rect
    }

    fn hit_test(&self, pos: [f32; 2]) -> Option<(ControlPart, f32)> {
        self.rect
            .contains(pos)
            .then(|| (ControlPart::Body, self.rect.normalized_x(pos[0])))
    }

    /// A tap on a link is its index; elsewhere, nothing.
    fn handle_mouse(&self, event: &MouseEvent, _ctx: &EventContext) -> Option<ControlAction> {
        match event {
            MouseEvent::Click {
                pos,
                button: MouseButton::Left,
            } => self.link_at(*pos).map(|i| ControlAction::Activated {
                part: ControlPart::Custom(i as u32),
            }),
            _ => None,
        }
    }

    fn handle_keyboard(
        &self,
        _event: &KeyboardEvent,
        _ctx: &EventContext,
    ) -> Option<ControlAction> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn colors(view: &MarkdownView, theme: &ControlTheme) -> Vec<(String, [f32; 4])> {
        let r = view.options.with_theme(&theme.markdown);
        view.placed
            .iter()
            .map(|p| (p.text.clone(), view.color_of(p, &r, theme)))
            .collect()
    }

    #[test]
    fn prose_code_and_links_take_the_themes_colours() {
        let view = MarkdownView::new(
            "plain `code` https://a.example",
            [0.0, 0.0],
            2000.0,
            MarkdownOptions::default(),
        );
        let t = ControlTheme::default();
        let c = colors(&view, &t);
        assert_eq!(c[0], ("plain ".into(), t.text_color));
        assert_eq!(c[1], ("code".into(), t.text_color));
        assert_eq!(c[3], ("https://a.example".into(), t.accent_color));
        let green = [0.0, 1.0, 0.0, 1.0];
        let mut t2 = ControlTheme::default();
        t2.markdown.code_color = Some(green);
        assert_eq!(colors(&view, &t2)[1].1, green);
    }

    #[cfg(feature = "cmark")]
    #[test]
    fn quotes_take_the_muted_colour() {
        let view = MarkdownView::new("> quoted", [0.0, 0.0], 2000.0, MarkdownOptions::default());
        let t = ControlTheme::default();
        assert_eq!(colors(&view, &t), [("quoted".into(), t.muted_text_color)]);
    }

    #[test]
    fn a_drawn_themes_sizes_reach_the_next_layout() {
        let mut view = MarkdownView::new("text", [0.0, 0.0], 500.0, MarkdownOptions::default());
        assert_eq!(view.px(), 14.0);
        let mut theme = ControlTheme::default();
        theme.markdown.font_size = Some(20.0);
        view.draw(
            &mut DrawList::new(),
            &DrawContext::default().with_theme(theme),
        );
        assert_eq!(view.px(), 14.0, "not until it lays out again");
        view.set_width(400.0);
        assert_eq!(view.px(), 20.0);
        assert_eq!(view.height(), 20.0 * 1.35);
    }
}
