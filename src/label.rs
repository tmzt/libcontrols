//! Basic drawable text label with alignment, background, and active glow rings.

use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use libmsdf::drawlist::{DrawList, LINE_BOX_RATIO, SdfInstance, SdfKind};

use crate::event::{ControlAction, ControlPart, Key, KeyboardEvent, MouseButton, MouseEvent};
use crate::handle::{ControlHandle, ReadValue};
use crate::host::HostedControl;
use crate::rect::{Rect, TextAlign};
use crate::ring::{GlowRingStyle, draw_glowing_ring};
use crate::traits::{DrawContext, DrawControl};

/// Mutable runtime state for a basic text label.
#[derive(Debug)]
pub struct LabelState {
    /// Text to display.
    pub text: Mutex<String>,
    /// Whether this label is in an active/selected state.
    pub is_active: AtomicBool,
}

impl LabelState {
    /// Create a new label state.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: Mutex::new(text.into()),
            is_active: AtomicBool::new(false),
        }
    }

    /// Read the text content.
    #[inline]
    pub fn text(&self) -> String {
        self.text.lock().unwrap().clone()
    }

    /// Update the text content.
    #[inline]
    pub fn set_text(&self, text: impl Into<String>) {
        *self.text.lock().unwrap() = text.into();
    }

    /// Check if the label is active.
    #[inline]
    pub fn is_active(&self) -> bool {
        self.is_active.load(Ordering::Relaxed)
    }

    /// Set the active state.
    #[inline]
    pub fn set_active(&self, active: bool) {
        self.is_active.store(active, Ordering::Relaxed);
    }
}

/// Basic drawable text label.
#[derive(Clone, Debug)]
pub struct Label {
    /// Shared mutable label state.
    pub state: Arc<LabelState>,
    /// Bounding box for label placement.
    pub rect: Rect,
    /// Text horizontal alignment.
    pub align: TextAlign,
    /// Optional font size override (falls back to theme if `None`).
    pub font_size: Option<f32>,
    /// Optional text color override (falls back to theme if `None`).
    pub color: Option<[f32; 4]>,
    /// Optional background fill color.
    pub bg_color: Option<[f32; 4]>,
    /// Optional corner radius for background box.
    pub corner_radius: Option<f32>,
    /// Optional custom glow ring style.
    pub glow_style: Option<GlowRingStyle>,
}

impl Label {
    /// Create a simple left-aligned label.
    pub fn new(text: impl Into<String>, rect: Rect) -> Self {
        Self {
            state: Arc::new(LabelState::new(text)),
            rect,
            align: TextAlign::Left,
            font_size: None,
            color: None,
            bg_color: None,
            corner_radius: None,
            glow_style: None,
        }
    }

    /// Attach an existing shared `LabelState`.
    pub fn with_state(mut self, state: Arc<LabelState>) -> Self {
        self.state = state;
        self
    }

    /// Read label text.
    #[inline]
    pub fn text(&self) -> String {
        self.state.text()
    }

    /// Set label text.
    #[inline]
    pub fn set_text(&self, text: impl Into<String>) {
        self.state.set_text(text);
    }

    /// Check if active.
    #[inline]
    pub fn is_active(&self) -> bool {
        self.state.is_active()
    }

    /// Set text alignment.
    pub fn with_align(mut self, align: TextAlign) -> Self {
        self.align = align;
        self
    }

    /// Set font size.
    pub fn with_font_size(mut self, font_size: f32) -> Self {
        self.font_size = Some(font_size);
        self
    }

    /// Set text color.
    pub fn with_color(mut self, color: [f32; 4]) -> Self {
        self.color = Some(color);
        self
    }

    /// Set background fill color.
    pub fn with_bg(mut self, bg_color: [f32; 4]) -> Self {
        self.bg_color = Some(bg_color);
        self
    }

    /// Set active state (draws a glowing squared-off ring when true).
    pub fn with_active(self, is_active: bool) -> Self {
        self.state.set_active(is_active);
        self
    }

    /// Set custom glowing ring style.
    pub fn with_glow_style(mut self, style: GlowRingStyle) -> Self {
        self.glow_style = Some(style);
        self
    }

    /// Obtain a borrowed handle to inspect the label's value.
    pub fn handle(&self) -> ControlHandle<'_, Self> {
        ControlHandle::new(self)
    }
}

impl DrawControl for Label {
    fn draw(&self, list: &mut DrawList, ctx: &DrawContext) {
        let font_size = ctx.font_size(self.font_size);
        let text_color = ctx.text_color(self.color);
        let rad = self.corner_radius.unwrap_or(ctx.theme.corner_radius);
        let text = self.text();

        if let Some(bg_col) = self.bg_color {
            let kind = if rad > 0.0 {
                SdfKind::RoundedBox { radius: rad }
            } else {
                SdfKind::Box
            };
            list.push(SdfInstance {
                kind,
                position: self.rect.pos(),
                size: self.rect.size(),
                color: bg_col,
                anim: 0,
            });
        }

        let text_advance = ctx.text_advance(&text, font_size);
        let line_box_h = font_size * LINE_BOX_RATIO;
        let pad_h = 4.0f32;

        let pen_x = match self.align {
            TextAlign::Left => self.rect.x + pad_h,
            TextAlign::Center => self.rect.x + (self.rect.w - text_advance) * 0.5,
            TextAlign::Right => self.rect.x + self.rect.w - pad_h - text_advance,
        };
        let pen_y = self.rect.y + (self.rect.h - line_box_h) * 0.5;

        ctx.draw_text(list, &text, [pen_x, pen_y], font_size, text_color);

        if self.is_active() {
            let style = self.glow_style.unwrap_or_default();
            draw_glowing_ring(list, self.rect, &style, &ctx.theme);
        }
    }
}

impl HostedControl for Label {
    fn bounds(&self) -> Rect {
        self.rect
    }

    fn hit_test(&self, pos: [f32; 2]) -> Option<(ControlPart, f32)> {
        if self.rect.contains(pos) {
            Some((ControlPart::Body, self.rect.normalized_x(pos[0])))
        } else {
            None
        }
    }

    fn handle_mouse(&self, event: &MouseEvent, hit_part: ControlPart) -> Option<ControlAction> {
        match event {
            MouseEvent::Click {
                button: MouseButton::Left,
                ..
            }
            | MouseEvent::Down {
                button: MouseButton::Left,
                ..
            } => Some(ControlAction::Activated { part: hit_part }),
            _ => None,
        }
    }

    fn handle_keyboard(&self, event: &KeyboardEvent) -> Option<ControlAction> {
        if event.is_down()
            && let Some(Key::Enter | Key::Space) = event.key()
        {
            return Some(ControlAction::Activated {
                part: ControlPart::Body,
            });
        }
        None
    }
}

impl ReadValue<String> for Label {
    fn read_value(&self) -> String {
        self.text()
    }
}
