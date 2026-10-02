//! Label flanked with left/right change chevrons for cycling discrete values.

use std::sync::Arc;
use std::sync::Mutex;

use libmsdf::drawlist::{DrawList, LINE_BOX_RATIO, SdfInstance, SdfKind};

use crate::event::{ControlAction, ControlPart, Key, KeyboardEvent, MouseButton, MouseEvent};
use crate::handle::{ControlHandle, ReadValue};
use crate::host::HostedControl;
use crate::rect::Rect;
use crate::ring::{GlowRingStyle, draw_glowing_ring};
use crate::traits::{DrawContext, DrawControl};

/// Specifies which part of the arrow label control is actively highlighted/focused.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum ArrowLabelActive {
    #[default]
    None,
    /// Center value label has the glowing focus ring.
    Value,
    /// Left arrow button has the glowing focus ring.
    LeftArrow,
    /// Right arrow button has the glowing focus ring.
    RightArrow,
    /// Entire control bounding box has the glowing focus ring.
    EntireControl,
}

/// Mutable runtime state for an arrow label control.
#[derive(Debug)]
pub struct ArrowLabelState {
    /// Displayed text content.
    pub text: Mutex<String>,
    /// Active focus part.
    pub active_part: Mutex<ArrowLabelActive>,
}

impl ArrowLabelState {
    /// Create a new arrow label state with initial text.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: Mutex::new(text.into()),
            active_part: Mutex::new(ArrowLabelActive::None),
        }
    }

    /// Read text content.
    #[inline]
    pub fn text(&self) -> String {
        self.text.lock().unwrap().clone()
    }

    /// Set text content.
    #[inline]
    pub fn set_text(&self, text: impl Into<String>) {
        *self.text.lock().unwrap() = text.into();
    }

    /// Read active focus part.
    #[inline]
    pub fn active_part(&self) -> ArrowLabelActive {
        *self.active_part.lock().unwrap()
    }

    /// Set active focus part.
    #[inline]
    pub fn set_active_part(&self, part: ArrowLabelActive) {
        *self.active_part.lock().unwrap() = part;
    }
}

/// Text label flanked with left (`<`) and right (`>`) change arrows.
#[derive(Clone, Debug)]
pub struct ArrowLabel {
    /// Shared mutable state.
    pub state: Arc<ArrowLabelState>,
    /// Bounding rectangle for the complete control.
    pub rect: Rect,
    /// Width of the left and right arrow touch targets in pixels.
    pub arrow_width: f32,
    /// Thickness of the chevron lines in logical pixels.
    pub chevron_thickness: f32,
    /// Optional font size override.
    pub font_size: Option<f32>,
    /// Optional label text color override.
    pub text_color: Option<[f32; 4]>,
    /// Optional arrow color override.
    pub arrow_color: Option<[f32; 4]>,
    /// Optional background fill color.
    pub bg_color: Option<[f32; 4]>,
    /// Whether left arrow is enabled (dimmed if false).
    pub left_enabled: bool,
    /// Whether right arrow is enabled (dimmed if false).
    pub right_enabled: bool,
    /// Optional custom glow ring style.
    pub glow_style: Option<GlowRingStyle>,
}

impl ArrowLabel {
    /// Create a new arrow label with default dimensions.
    pub fn new(text: impl Into<String>, rect: Rect) -> Self {
        Self {
            state: Arc::new(ArrowLabelState::new(text)),
            rect,
            arrow_width: 24.0,
            chevron_thickness: 1.8,
            font_size: None,
            text_color: None,
            arrow_color: None,
            bg_color: None,
            left_enabled: true,
            right_enabled: true,
            glow_style: None,
        }
    }

    /// Attach a shared `ArrowLabelState`.
    pub fn with_state(mut self, state: Arc<ArrowLabelState>) -> Self {
        self.state = state;
        self
    }

    /// Read text content.
    #[inline]
    pub fn text(&self) -> String {
        self.state.text()
    }

    /// Set text content.
    #[inline]
    pub fn set_text(&self, text: impl Into<String>) {
        self.state.set_text(text);
    }

    /// Read active focus part.
    #[inline]
    pub fn active_part(&self) -> ArrowLabelActive {
        self.state.active_part()
    }

    /// Set width of the arrow touch zones.
    pub fn with_arrow_width(mut self, width: f32) -> Self {
        self.arrow_width = width;
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

    /// Set arrow color.
    pub fn with_arrow_color(mut self, color: [f32; 4]) -> Self {
        self.arrow_color = Some(color);
        self
    }

    /// Set background color.
    pub fn with_bg(mut self, bg_color: [f32; 4]) -> Self {
        self.bg_color = Some(bg_color);
        self
    }

    /// Set active highlight state.
    pub fn with_active(self, active: ArrowLabelActive) -> Self {
        self.state.set_active_part(active);
        self
    }

    /// Set active value highlight.
    pub fn with_active_value(self, is_active: bool) -> Self {
        self.state.set_active_part(if is_active {
            ArrowLabelActive::Value
        } else {
            ArrowLabelActive::None
        });
        self
    }

    /// Set custom glow ring style.
    pub fn with_glow_style(mut self, style: GlowRingStyle) -> Self {
        self.glow_style = Some(style);
        self
    }

    /// Obtain a borrowed handle to inspect the arrow label's value.
    pub fn handle(&self) -> ControlHandle<'_, Self> {
        ControlHandle::new(self)
    }

    /// Sub-rectangles for left arrow, center value, and right arrow.
    pub fn sub_rects(&self) -> (Rect, Rect, Rect) {
        let arrow_w = self.arrow_width.min(self.rect.w * 0.3);
        let left_rect = Rect::new(self.rect.x, self.rect.y, arrow_w, self.rect.h);
        let right_rect = Rect::new(
            self.rect.x + self.rect.w - arrow_w,
            self.rect.y,
            arrow_w,
            self.rect.h,
        );
        let center_rect = Rect::new(
            self.rect.x + arrow_w,
            self.rect.y,
            (self.rect.w - arrow_w * 2.0).max(0.0),
            self.rect.h,
        );
        (left_rect, center_rect, right_rect)
    }
}

impl DrawControl for ArrowLabel {
    fn draw(&self, list: &mut DrawList, ctx: &DrawContext) {
        let (left_rect, center_rect, right_rect) = self.sub_rects();
        let font_size = ctx.font_size(self.font_size);
        let text_col = ctx.text_color(self.text_color);
        let base_arrow_col = self.arrow_color.unwrap_or(ctx.theme.arrow_color);
        let text = self.text();

        if let Some(bg_col) = self.bg_color {
            let rad = ctx.theme.corner_radius;
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

        let left_col = if self.left_enabled {
            base_arrow_col
        } else {
            [
                base_arrow_col[0],
                base_arrow_col[1],
                base_arrow_col[2],
                base_arrow_col[3] * 0.35,
            ]
        };
        let chevron_sz = (left_rect.h * 0.3).clamp(4.0, 10.0);
        let l_c = left_rect.center();
        let l_top = [l_c[0] + chevron_sz * 0.45, l_c[1] - chevron_sz];
        let l_mid = [l_c[0] - chevron_sz * 0.45, l_c[1]];
        let l_bot = [l_c[0] + chevron_sz * 0.45, l_c[1] + chevron_sz];
        list.push_line(l_top, l_mid, self.chevron_thickness, left_col);
        list.push_line(l_mid, l_bot, self.chevron_thickness, left_col);

        let right_col = if self.right_enabled {
            base_arrow_col
        } else {
            [
                base_arrow_col[0],
                base_arrow_col[1],
                base_arrow_col[2],
                base_arrow_col[3] * 0.35,
            ]
        };
        let r_c = right_rect.center();
        let r_top = [r_c[0] - chevron_sz * 0.45, r_c[1] - chevron_sz];
        let r_mid = [r_c[0] + chevron_sz * 0.45, r_c[1]];
        let r_bot = [r_c[0] - chevron_sz * 0.45, r_c[1] + chevron_sz];
        list.push_line(r_top, r_mid, self.chevron_thickness, right_col);
        list.push_line(r_mid, r_bot, self.chevron_thickness, right_col);

        let text_advance = ctx.text_advance(&text, font_size);
        let line_box_h = font_size * LINE_BOX_RATIO;
        let pen_x = center_rect.x + (center_rect.w - text_advance) * 0.5;
        let pen_y = center_rect.y + (center_rect.h - line_box_h) * 0.5;
        ctx.draw_text(list, &text, [pen_x, pen_y], font_size, text_col);

        let glow_style = self.glow_style.unwrap_or_default();
        match self.active_part() {
            ArrowLabelActive::None => {}
            ArrowLabelActive::Value => {
                draw_glowing_ring(list, center_rect.pad_xy(-2.0, 1.0), &glow_style, &ctx.theme);
            }
            ArrowLabelActive::LeftArrow => {
                draw_glowing_ring(list, left_rect.pad(1.0), &glow_style, &ctx.theme);
            }
            ArrowLabelActive::RightArrow => {
                draw_glowing_ring(list, right_rect.pad(1.0), &glow_style, &ctx.theme);
            }
            ArrowLabelActive::EntireControl => {
                draw_glowing_ring(list, self.rect, &glow_style, &ctx.theme);
            }
        }
    }
}

impl HostedControl for ArrowLabel {
    fn bounds(&self) -> Rect {
        self.rect
    }

    fn hit_test(&self, pos: [f32; 2]) -> Option<(ControlPart, f32)> {
        if !self.rect.contains(pos) {
            return None;
        }
        let (left_r, center_r, right_r) = self.sub_rects();
        if left_r.contains(pos) {
            Some((ControlPart::LeftArrow, left_r.normalized_x(pos[0])))
        } else if right_r.contains(pos) {
            Some((ControlPart::RightArrow, right_r.normalized_x(pos[0])))
        } else if center_r.contains(pos) {
            Some((ControlPart::Value, center_r.normalized_x(pos[0])))
        } else {
            Some((ControlPart::Body, self.rect.normalized_x(pos[0])))
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
            } => match hit_part {
                ControlPart::LeftArrow => {
                    if self.left_enabled {
                        Some(ControlAction::StepLeft)
                    } else {
                        None
                    }
                }
                ControlPart::RightArrow => {
                    if self.right_enabled {
                        Some(ControlAction::StepRight)
                    } else {
                        None
                    }
                }
                ControlPart::Value | ControlPart::Body => {
                    Some(ControlAction::Activated { part: hit_part })
                }
                _ => None,
            },
            MouseEvent::Scroll { delta, .. } => {
                if delta[0] < -0.1 || delta[1] > 0.1 {
                    if self.left_enabled {
                        Some(ControlAction::StepLeft)
                    } else {
                        None
                    }
                } else if delta[0] > 0.1 || delta[1] < -0.1 {
                    if self.right_enabled {
                        Some(ControlAction::StepRight)
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn handle_keyboard(&self, event: &KeyboardEvent) -> Option<ControlAction> {
        if event.is_down() {
            match event.key() {
                Some(Key::Left | Key::Up) => {
                    if self.left_enabled {
                        Some(ControlAction::StepLeft)
                    } else {
                        None
                    }
                }
                Some(Key::Right | Key::Down) => {
                    if self.right_enabled {
                        Some(ControlAction::StepRight)
                    } else {
                        None
                    }
                }
                Some(Key::Enter | Key::Space) => Some(ControlAction::Activated {
                    part: ControlPart::Value,
                }),
                _ => None,
            }
        } else {
            None
        }
    }
}

impl ReadValue<String> for ArrowLabel {
    fn read_value(&self) -> String {
        self.text()
    }
}
