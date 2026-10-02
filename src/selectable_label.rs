//! Selectable label cycling through a static list of strings or typed options.

use std::marker::PhantomData;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use libmsdf::drawlist::{DrawList, LINE_BOX_RATIO, SdfInstance, SdfKind};

use crate::arrow_label::ArrowLabelActive;
use crate::handle::{ControlHandle, ReadValue};
use crate::rect::Rect;
use crate::ring::{GlowRingStyle, draw_glowing_ring};
use crate::traits::{DrawContext, DrawControl};

/// Mutable runtime state for a selectable label control.
#[derive(Debug)]
pub struct SelectableLabelState<T: 'static> {
    /// Currently selected option index.
    pub selected_index: AtomicUsize,
    /// Currently active focus part.
    pub active_part: Mutex<ArrowLabelActive>,
    _marker: PhantomData<&'static T>,
}

impl<T: 'static> SelectableLabelState<T> {
    /// Create a new state with selected index.
    pub fn new(selected_index: usize) -> Self {
        Self {
            selected_index: AtomicUsize::new(selected_index),
            active_part: Mutex::new(ArrowLabelActive::None),
            _marker: PhantomData,
        }
    }

    /// Read currently selected index.
    #[inline]
    pub fn selected_index(&self) -> usize {
        self.selected_index.load(Ordering::Relaxed)
    }

    /// Set selected index.
    #[inline]
    pub fn set_selected_index(&self, index: usize) {
        self.selected_index.store(index, Ordering::Relaxed);
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

/// Selectable label for cycling through a static list of string options or typed variants.
#[derive(Clone, Debug)]
pub struct SelectableLabel<T: 'static> {
    /// Shared mutable control state.
    pub state: Arc<SelectableLabelState<T>>,
    /// Static slice of available options.
    pub options: &'static [T],
    /// Bounding rectangle for the control.
    pub rect: Rect,
    /// Width of the left/right chevron touch zones.
    pub arrow_width: f32,
    /// Line thickness of the chevron strokes.
    pub chevron_thickness: f32,
    /// Optional font size override.
    pub font_size: Option<f32>,
    /// Optional label text color override.
    pub text_color: Option<[f32; 4]>,
    /// Optional chevron arrow color override.
    pub arrow_color: Option<[f32; 4]>,
    /// Optional background fill color.
    pub bg_color: Option<[f32; 4]>,
    /// Optional custom glow ring style.
    pub glow_style: Option<GlowRingStyle>,
    /// Custom string formatter for typed items.
    pub format_fn: Option<fn(&T) -> String>,
}

impl SelectableLabel<&'static str> {
    /// Create a selectable label from a static list of string slices.
    pub fn from_static(options: &'static [&'static str], rect: Rect) -> Self {
        Self {
            state: Arc::new(SelectableLabelState::new(0)),
            options,
            rect,
            arrow_width: 24.0,
            chevron_thickness: 1.8,
            font_size: None,
            text_color: None,
            arrow_color: None,
            bg_color: None,
            glow_style: None,
            format_fn: None,
        }
    }
}

impl<T: 'static> SelectableLabel<T> {
    /// Create a selectable label from a static slice of typed options.
    pub fn new(options: &'static [T], rect: Rect) -> Self {
        Self {
            state: Arc::new(SelectableLabelState::new(0)),
            options,
            rect,
            arrow_width: 24.0,
            chevron_thickness: 1.8,
            font_size: None,
            text_color: None,
            arrow_color: None,
            bg_color: None,
            glow_style: None,
            format_fn: None,
        }
    }

    /// Attach a shared `SelectableLabelState`.
    pub fn with_state(mut self, state: Arc<SelectableLabelState<T>>) -> Self {
        self.state = state;
        self
    }

    /// Set the selected option index.
    pub fn with_selected_index(self, index: usize) -> Self {
        let clamped = if !self.options.is_empty() {
            index.min(self.options.len() - 1)
        } else {
            0
        };
        self.state.set_selected_index(clamped);
        self
    }

    /// Read currently selected index.
    #[inline]
    pub fn selected_index(&self) -> usize {
        self.state.selected_index()
    }

    /// Set selected index.
    #[inline]
    pub fn set_selected_index(&self, index: usize) {
        let clamped = if !self.options.is_empty() {
            index.min(self.options.len() - 1)
        } else {
            0
        };
        self.state.set_selected_index(clamped);
    }

    /// Read active focus part.
    #[inline]
    pub fn active_part(&self) -> ArrowLabelActive {
        self.state.active_part()
    }

    /// Set custom string formatting function for typed items.
    pub fn with_format_fn(mut self, format_fn: fn(&T) -> String) -> Self {
        self.format_fn = Some(format_fn);
        self
    }

    /// Set arrow touch target width.
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

    /// Set background fill color.
    pub fn with_bg(mut self, bg_color: [f32; 4]) -> Self {
        self.bg_color = Some(bg_color);
        self
    }

    /// Set active highlight state.
    pub fn with_active(self, active: ArrowLabelActive) -> Self {
        self.state.set_active_part(active);
        self
    }

    /// Set active state specifically on the value display.
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

    /// Obtain a borrowed handle to inspect the typed value.
    pub fn handle(&self) -> ControlHandle<'_, Self> {
        ControlHandle::new(self)
    }

    /// Currently selected item reference.
    pub fn selected_value(&self) -> Option<&T> {
        self.options.get(self.selected_index())
    }

    /// Check if left chevron is enabled (not at beginning).
    pub fn left_enabled(&self) -> bool {
        self.selected_index() > 0
    }

    /// Check if right chevron is enabled (not at end).
    pub fn right_enabled(&self) -> bool {
        !self.options.is_empty() && self.selected_index() + 1 < self.options.len()
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

impl SelectableLabel<&'static str> {
    /// Format currently selected string.
    pub fn display_text(&self) -> &'static str {
        self.selected_value().copied().unwrap_or("")
    }
}

impl<T: std::fmt::Display + 'static> SelectableLabel<T> {
    /// Format currently selected typed item using Display or custom format_fn.
    pub fn display_string(&self) -> String {
        if let Some(fmt_fn) = self.format_fn {
            self.selected_value().map(fmt_fn).unwrap_or_default()
        } else {
            self.selected_value()
                .map(|v| format!("{v}"))
                .unwrap_or_default()
        }
    }
}

impl DrawControl for SelectableLabel<&'static str> {
    fn draw(&self, list: &mut DrawList, ctx: &DrawContext) {
        let (left_rect, center_rect, right_rect) = self.sub_rects();
        let font_size = ctx.font_size(self.font_size);
        let text_col = ctx.text_color(self.text_color);
        let base_arrow_col = self.arrow_color.unwrap_or(ctx.theme.arrow_color);

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

        let left_col = if self.left_enabled() {
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

        let right_col = if self.right_enabled() {
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

        let text = self.display_text();
        let text_advance = ctx.text_advance(text, font_size);
        let line_box_h = font_size * LINE_BOX_RATIO;
        let pen_x = center_rect.x + (center_rect.w - text_advance) * 0.5;
        let pen_y = center_rect.y + (center_rect.h - line_box_h) * 0.5;
        ctx.draw_text(list, text, [pen_x, pen_y], font_size, text_col);

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

use crate::event::{ControlAction, ControlPart, Key, KeyboardEvent, MouseButton, MouseEvent};
use crate::host::HostedControl;

impl<T: 'static> HostedControl for SelectableLabel<T> {
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
                    if self.left_enabled() {
                        self.set_selected_index(self.selected_index().saturating_sub(1));
                        Some(ControlAction::StepLeft)
                    } else {
                        None
                    }
                }
                ControlPart::RightArrow => {
                    if self.right_enabled() {
                        self.set_selected_index(self.selected_index() + 1);
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
                    if self.left_enabled() {
                        self.set_selected_index(self.selected_index().saturating_sub(1));
                        Some(ControlAction::StepLeft)
                    } else {
                        None
                    }
                } else if delta[0] > 0.1 || delta[1] < -0.1 {
                    if self.right_enabled() {
                        self.set_selected_index(self.selected_index() + 1);
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
                    if self.left_enabled() {
                        self.set_selected_index(self.selected_index().saturating_sub(1));
                        Some(ControlAction::StepLeft)
                    } else {
                        None
                    }
                }
                Some(Key::Right | Key::Down) => {
                    if self.right_enabled() {
                        self.set_selected_index(self.selected_index() + 1);
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

impl<T: Clone + 'static> ReadValue<T> for SelectableLabel<T> {
    fn read_value(&self) -> T {
        self.selected_value()
            .cloned()
            .expect("SelectableLabel must have at least one valid option")
    }
}
