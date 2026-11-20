//! A one-line label with a copy glyph at its right end; the glyph asks the
//! app to copy the text (docs/design/20261120_01_COPY_LABEL_PLAN.md).
//!
//! libcontrols has no clipboard: the control only records the request,
//! taken with [`CopyLabelState::take_copy`], and the app copies the text
//! with its own platform clipboard.

use std::cell::Cell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use libmsdf::drawlist::{DrawList, LINE_BOX_RATIO};

use crate::control::HostedControl;
use crate::editable_label::EditableLabelTheme;
use crate::event::{
    ControlAction, ControlPart, EventContext, Key, KeyboardEvent, MouseButton, MouseEvent,
};
use crate::flags::{NodeFlags, NodeState};
use crate::handle::{ControlHandle, ReadValue};
use crate::rect::{Rect, TextAlign};
use crate::ring::{GlowRingStyle, draw_glowing_ring};
use crate::scrollbar::cut_to_fit;
use crate::traits::{DrawContext, DrawControl};

/// The shared state of a [`CopyLabel`]: its text and a pending copy
/// request.
#[derive(Debug)]
pub struct CopyLabelState {
    text: Mutex<String>,
    copy: AtomicBool,
}

impl CopyLabelState {
    /// A state showing `text`, with no copy asked for.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: Mutex::new(text.into()),
            copy: AtomicBool::new(false),
        }
    }

    /// The text.
    pub fn text(&self) -> String {
        self.text.lock().unwrap().clone()
    }

    /// Replace the text.
    pub fn set_text(&self, text: impl Into<String>) {
        *self.text.lock().unwrap() = text.into();
    }

    /// True once after the glyph was clicked or tapped, or Enter or Space
    /// pressed while the label was focused; clears the request. The app
    /// then copies [`text`](Self::text) to its clipboard.
    pub fn take_copy(&self) -> bool {
        self.copy.swap(false, Ordering::Relaxed)
    }

    fn request_copy(&self) {
        self.copy.store(true, Ordering::Relaxed);
    }
}

/// A one-line label with a copy glyph at its right end.
#[derive(Clone, Debug)]
pub struct CopyLabel {
    /// Shared state: the text and the copy request.
    pub state: Arc<CopyLabelState>,
    /// Bounding rectangle.
    pub rect: Rect,
    /// The text's alignment within the text area.
    pub align: TextAlign,
    /// `None`: the theme's `default_font_size`.
    pub font_size: Option<f32>,
    /// `None`: the theme's `text_color`.
    pub text_color: Option<[f32; 4]>,
    /// `None`: the theme's `editable_label.pencil_color`, else
    /// `muted_text_color`.
    pub glyph_color: Option<[f32; 4]>,
    /// The `editable_label` values of the last draw, for hit testing.
    last: Cell<EditableLabelTheme>,
}

/// Horizontal text padding, as `Label` and `EditableLabel` use.
const PAD_H: f32 = 4.0;

impl CopyLabel {
    /// The copy glyph.
    pub const COPY: ControlPart = ControlPart::Custom(0x434C_0001);
    /// The text.
    pub const TEXT: ControlPart = ControlPart::Label;

    /// A left-aligned label showing `text` in `rect`.
    pub fn new(text: impl Into<String>, rect: Rect) -> Self {
        Self {
            state: Arc::new(CopyLabelState::new(text)),
            rect,
            align: TextAlign::Left,
            font_size: None,
            text_color: None,
            glyph_color: None,
            last: Cell::new(EditableLabelTheme::default()),
        }
    }

    /// Attach an existing shared state.
    pub fn with_state(mut self, state: Arc<CopyLabelState>) -> Self {
        self.state = state;
        self
    }

    /// Override the theme's font size.
    pub fn with_font_size(mut self, size: f32) -> Self {
        self.font_size = Some(size);
        self
    }

    /// Set the text's alignment.
    pub fn with_align(mut self, align: TextAlign) -> Self {
        self.align = align;
        self
    }

    /// Override the theme's text colour.
    pub fn with_text_color(mut self, color: [f32; 4]) -> Self {
        self.text_color = Some(color);
        self
    }

    /// Override the glyph's colour.
    pub fn with_glyph_color(mut self, color: [f32; 4]) -> Self {
        self.glyph_color = Some(color);
        self
    }

    /// The text.
    pub fn text(&self) -> String {
        self.state.text()
    }

    /// The text as drawn with `ctx`: its first line, cut with "…" to fit.
    pub fn shown_text(&self, ctx: &DrawContext) -> String {
        let theme = &ctx.theme.editable_label;
        let (text_rect, _) = self.layout(theme);
        let size = ctx.font_size(self.font_size);
        let max = (text_rect.w - 2.0 * PAD_H).max(0.0);
        cut_to_fit(&self.text(), max, |t| ctx.text_advance(t, size))
    }

    /// The text area and the glyph's button.
    fn layout(&self, theme: &EditableLabelTheme) -> (Rect, Rect) {
        let r = self.rect;
        let bw = theme.button_width.clamp(0.0, r.w.max(0.0));
        let button = Rect::new(r.right() - bw, r.y, bw, r.h);
        let text_w = (button.x - theme.gap - r.x).max(0.0);
        (Rect::new(r.x, r.y, text_w, r.h), button)
    }

    /// Obtain a borrowed handle to read the text.
    pub fn handle(&self) -> ControlHandle<'_, Self> {
        ControlHandle::new(self)
    }
}

fn at(c: [f32; 2], s: f32, dx: f32, dy: f32) -> [f32; 2] {
    [c[0] + dx * s, c[1] + dy * s]
}

/// The copy icon: a front square down-left over a back square up-right,
/// the back one's hidden part left out. Eight segments.
fn draw_copy(list: &mut DrawList, c: [f32; 2], s: f32, t: f32, col: [f32; 4]) {
    // Front square x -0.38..0.14, y -0.14..0.38; back square x -0.14..0.38,
    // y -0.38..0.14 (side 0.52, offset 0.24).
    let (lo, mid_lo, mid_hi, hi) = (-0.38, -0.14, 0.14, 0.38);
    // Front.
    list.push_line(at(c, s, lo, mid_lo), at(c, s, mid_hi, mid_lo), t, col);
    list.push_line(at(c, s, mid_hi, mid_lo), at(c, s, mid_hi, hi), t, col);
    list.push_line(at(c, s, mid_hi, hi), at(c, s, lo, hi), t, col);
    list.push_line(at(c, s, lo, hi), at(c, s, lo, mid_lo), t, col);
    // Back: top, right, the left edge above the front, the bottom right of it.
    list.push_line(at(c, s, mid_lo, lo), at(c, s, hi, lo), t, col);
    list.push_line(at(c, s, hi, lo), at(c, s, hi, mid_hi), t, col);
    list.push_line(at(c, s, mid_lo, lo), at(c, s, mid_lo, mid_lo), t, col);
    list.push_line(at(c, s, mid_hi, mid_hi), at(c, s, hi, mid_hi), t, col);
}

impl DrawControl for CopyLabel {
    fn draw(&self, list: &mut DrawList, ctx: &DrawContext) {
        let theme = ctx.theme.editable_label;
        self.last.set(theme);
        let (text_rect, button) = self.layout(&theme);
        let font_size = ctx.font_size(self.font_size);
        let text = self.shown_text(ctx);
        let advance = ctx.text_advance(&text, font_size);
        let pen_x = match self.align {
            TextAlign::Left => text_rect.x + PAD_H,
            TextAlign::Center => text_rect.x + (text_rect.w - advance) * 0.5,
            TextAlign::Right => text_rect.right() - PAD_H - advance,
        };
        let pen_y = text_rect.y + (text_rect.h - font_size * LINE_BOX_RATIO) * 0.5;
        ctx.draw_text(
            list,
            &text,
            [pen_x, pen_y],
            font_size,
            ctx.text_color(self.text_color),
        );
        let col = self
            .glyph_color
            .or(theme.pencil_color)
            .unwrap_or(ctx.theme.muted_text_color);
        draw_copy(
            list,
            button.center(),
            theme.icon_size,
            theme.icon_thickness,
            col,
        );
        if ctx.state.contains(NodeState::FOCUSED) {
            draw_glowing_ring(list, self.rect, &GlowRingStyle::default(), &ctx.theme);
        }
    }
}

impl HostedControl for CopyLabel {
    fn default_flags(&self) -> NodeFlags {
        NodeFlags::FOCUSABLE
    }

    fn bounds(&self) -> Rect {
        self.rect
    }

    fn hit_test(&self, pos: [f32; 2]) -> Option<(ControlPart, f32)> {
        if !self.rect.contains(pos) {
            return None;
        }
        let (text_rect, button) = self.layout(&self.last.get());
        let part = if button.contains(pos) {
            Self::COPY
        } else if text_rect.contains(pos) {
            Self::TEXT
        } else {
            ControlPart::Body
        };
        Some((part, self.rect.normalized_x(pos[0])))
    }

    fn handle_mouse(&self, event: &MouseEvent, ctx: &EventContext) -> Option<ControlAction> {
        match *event {
            MouseEvent::Click {
                button: MouseButton::Left,
                ..
            } if ctx.part == Self::COPY => {
                self.state.request_copy();
                Some(ControlAction::Activated { part: Self::COPY })
            }
            _ => None,
        }
    }

    fn handle_keyboard(&self, event: &KeyboardEvent, _ctx: &EventContext) -> Option<ControlAction> {
        if event.is_down()
            && let Some(Key::Enter | Key::Space) = event.key()
        {
            self.state.request_copy();
            return Some(ControlAction::Activated { part: Self::COPY });
        }
        None
    }
}

impl ReadValue<String> for CopyLabel {
    fn read_value(&self) -> String {
        self.text()
    }
}
