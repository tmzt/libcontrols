//! Single-line edit box and value readout control.

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

/// Mutable runtime state for an edit box / value readout control.
#[derive(Debug)]
pub struct EditState {
    /// Text content of the edit box.
    pub text: Mutex<String>,
    /// Whether the edit box is currently focused/active.
    pub is_active: AtomicBool,
}

impl EditState {
    /// Create a new edit state with initial text content.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: Mutex::new(text.into()),
            is_active: AtomicBool::new(false),
        }
    }

    /// Read the current text.
    #[inline]
    pub fn text(&self) -> String {
        self.text.lock().unwrap().clone()
    }

    /// Update the text content.
    #[inline]
    pub fn set_text(&self, text: impl Into<String>) {
        *self.text.lock().unwrap() = text.into();
    }

    /// Check if the edit control is active/focused.
    #[inline]
    pub fn is_active(&self) -> bool {
        self.is_active.load(Ordering::Relaxed)
    }

    /// Set the active focus state.
    #[inline]
    pub fn set_active(&self, active: bool) {
        self.is_active.store(active, Ordering::Relaxed);
    }
}

/// Single-line edit box / value readout control.
#[derive(Clone, Debug)]
pub struct Edit {
    /// Shared mutable edit state.
    pub state: Arc<EditState>,
    /// Bounding rectangle for the control.
    pub rect: Rect,
    /// Text horizontal alignment within the box.
    pub align: TextAlign,
    /// Optional font size override.
    pub font_size: Option<f32>,
    /// Optional text color override.
    pub text_color: Option<[f32; 4]>,
    /// Optional background fill color.
    pub bg_color: Option<[f32; 4]>,
    /// Optional border outline color.
    pub border_color: Option<[f32; 4]>,
    /// Border thickness in logical pixels.
    pub border_thickness: f32,
    /// Optional corner radius.
    pub corner_radius: Option<f32>,
    /// Optional custom glow ring style.
    pub glow_style: Option<GlowRingStyle>,
}

impl Edit {
    /// Create a new edit box with initial text and bounding rectangle.
    pub fn new(text: impl Into<String>, rect: Rect) -> Self {
        Self {
            state: Arc::new(EditState::new(text)),
            rect,
            align: TextAlign::Center,
            font_size: None,
            text_color: None,
            bg_color: Some([0.10, 0.13, 0.18, 0.85]),
            border_color: Some([0.15, 0.20, 0.28, 0.60]),
            border_thickness: 1.0,
            corner_radius: None,
            glow_style: None,
        }
    }

    /// Attach an existing shared `EditState`.
    pub fn with_state(mut self, state: Arc<EditState>) -> Self {
        self.state = state;
        self
    }

    /// Read the current text.
    #[inline]
    pub fn text(&self) -> String {
        self.state.text()
    }

    /// Set the text content.
    #[inline]
    pub fn set_text(&self, text: impl Into<String>) {
        self.state.set_text(text);
    }

    /// Check if active/focused.
    #[inline]
    pub fn is_active(&self) -> bool {
        self.state.is_active()
    }

    /// Set text horizontal alignment.
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
    pub fn with_text_color(mut self, color: [f32; 4]) -> Self {
        self.text_color = Some(color);
        self
    }

    /// Set background fill color.
    pub fn with_bg(mut self, bg_color: [f32; 4]) -> Self {
        self.bg_color = Some(bg_color);
        self
    }

    /// Set border color and thickness.
    pub fn with_border(mut self, border_color: [f32; 4], thickness: f32) -> Self {
        self.border_color = Some(border_color);
        self.border_thickness = thickness;
        self
    }

    /// Set corner radius.
    pub fn with_corner_radius(mut self, radius: f32) -> Self {
        self.corner_radius = Some(radius);
        self
    }

    /// Set active focus state.
    pub fn with_active(self, is_active: bool) -> Self {
        self.state.set_active(is_active);
        self
    }

    /// Set custom glow ring style.
    pub fn with_glow_style(mut self, style: GlowRingStyle) -> Self {
        self.glow_style = Some(style);
        self
    }

    /// Obtain a borrowed handle to inspect the edit box value.
    pub fn handle(&self) -> ControlHandle<'_, Self> {
        ControlHandle::new(self)
    }
}

impl DrawControl for Edit {
    fn draw(&self, list: &mut DrawList, ctx: &DrawContext) {
        let font_size = ctx.font_size(self.font_size);
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

        if let Some(border_col) = self.border_color
            && self.border_thickness > 0.0
        {
            list.push(SdfInstance {
                kind: SdfKind::Outline {
                    radius: rad,
                    thickness: self.border_thickness,
                },
                position: self.rect.pos(),
                size: self.rect.size(),
                color: border_col,
                anim: 0,
            });
        }

        let text_col = self.text_color.unwrap_or(ctx.theme.text_color);
        let text_advance = ctx.text_advance(&text, font_size);
        let line_box_h = font_size * LINE_BOX_RATIO;
        let pad_h = 4.0f32;

        let pen_x = match self.align {
            TextAlign::Left => self.rect.x + pad_h,
            TextAlign::Center => self.rect.x + (self.rect.w - text_advance) * 0.5,
            TextAlign::Right => self.rect.x + self.rect.w - pad_h - text_advance,
        };
        let pen_y = self.rect.y + (self.rect.h - line_box_h) * 0.5;

        ctx.draw_text(list, &text, [pen_x, pen_y], font_size, text_col);

        if self.is_active() {
            let style = self.glow_style.unwrap_or_default();
            draw_glowing_ring(list, self.rect, &style, &ctx.theme);
        }
    }
}

impl HostedControl for Edit {
    fn bounds(&self) -> Rect {
        self.rect
    }

    fn hit_test(&self, pos: [f32; 2]) -> Option<(ControlPart, f32)> {
        if self.rect.contains(pos) {
            Some((ControlPart::NumericBox, self.rect.normalized_x(pos[0])))
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
                part: ControlPart::NumericBox,
            });
        }
        None
    }
}

impl ReadValue<String> for Edit {
    fn read_value(&self) -> String {
        self.text()
    }
}
