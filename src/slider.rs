//! Continuous horizontal value slider with customizable track, fill, and thumb.

use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use libmsdf::drawlist::{DrawList, SdfInstance, SdfKind};

use crate::handle::{ControlHandle, ReadValue};
use crate::rect::Rect;
use crate::ring::{GlowRingStyle, draw_glowing_ring};
use crate::traits::{DrawContext, DrawControl};

/// Mutable runtime state for a continuous value slider.
#[derive(Debug)]
pub struct SliderState {
    /// Current numerical value.
    pub value: Mutex<f32>,
    /// Whether this slider is actively focused/selected.
    pub is_active: AtomicBool,
    /// Whether this slider is actively being dragged.
    pub is_dragging: AtomicBool,
}

impl SliderState {
    /// Create a new slider state with initial value.
    pub fn new(value: f32) -> Self {
        Self {
            value: Mutex::new(value),
            is_active: AtomicBool::new(false),
            is_dragging: AtomicBool::new(false),
        }
    }

    /// Read the current value.
    #[inline]
    pub fn value(&self) -> f32 {
        *self.value.lock().unwrap()
    }

    /// Update the current value.
    #[inline]
    pub fn set_value(&self, val: f32) {
        *self.value.lock().unwrap() = val;
    }

    /// Check if the slider is currently focused/active.
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

/// Visual shape style for the slider handle/thumb.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub enum ThumbStyle {
    /// Squared-off rounded rectangular notch handle.
    #[default]
    SquaredNotch,
    /// Circular thumb indicator.
    Circle,
}

/// Horizontal continuous value slider.
#[derive(Clone, Debug)]
pub struct Slider {
    /// Shared mutable slider state.
    pub state: Arc<SliderState>,
    /// Minimum value range bound.
    pub min: f32,
    /// Maximum value range bound.
    pub max: f32,
    /// Bounding rectangle for the control.
    pub rect: Rect,
    /// Height of the slider track bar in pixels.
    pub track_height: f32,
    /// Extent of the draggable thumb `[width, height]`.
    pub thumb_size: [f32; 2],
    /// Thumb geometric shape style.
    pub thumb_style: ThumbStyle,
    /// Optional track background color override.
    pub track_bg_color: Option<[f32; 4]>,
    /// Optional active progress track fill color override.
    pub track_fill_color: Option<[f32; 4]>,
    /// Optional thumb fill color override.
    pub thumb_color: Option<[f32; 4]>,
    /// Optional thumb border outline color override.
    pub thumb_border_color: Option<[f32; 4]>,
    /// Optional custom glow ring style.
    pub glow_style: Option<GlowRingStyle>,
}

impl Slider {
    /// Create a new slider with range `[min, max]` and initial value.
    pub fn new(value: f32, min: f32, max: f32, rect: Rect) -> Self {
        Self {
            state: Arc::new(SliderState::new(value)),
            min,
            max,
            rect,
            track_height: 5.0,
            thumb_size: [12.0, 20.0],
            thumb_style: ThumbStyle::SquaredNotch,
            track_bg_color: None,
            track_fill_color: None,
            thumb_color: None,
            thumb_border_color: None,
            glow_style: None,
        }
    }

    /// Attach an existing shared `SliderState`.
    pub fn with_state(mut self, state: Arc<SliderState>) -> Self {
        self.state = state;
        self
    }

    /// Read the current value.
    #[inline]
    pub fn value(&self) -> f32 {
        self.state.value()
    }

    /// Set the current value.
    #[inline]
    pub fn set_value(&self, val: f32) {
        self.state.set_value(val);
    }

    /// Check if the slider is currently focused/active.
    #[inline]
    pub fn is_active(&self) -> bool {
        self.state.is_active()
    }

    /// Normalized progress fraction in `[0.0, 1.0]`.
    pub fn normalized_fraction(&self) -> f32 {
        let val = self.value();
        if (self.max - self.min).abs() < f32::EPSILON {
            0.0
        } else {
            ((val - self.min) / (self.max - self.min)).clamp(0.0, 1.0)
        }
    }

    /// Calculate the current thumb bounding box.
    pub fn thumb_rect(&self) -> Rect {
        let frac = self.normalized_fraction();
        let thumb_w = self.thumb_size[0];
        let thumb_h = self.thumb_size[1];
        let travel_w = (self.rect.w - thumb_w).max(0.0);
        let thumb_x = self.rect.x + travel_w * frac;
        let thumb_y = self.rect.y + (self.rect.h - thumb_h) * 0.5;
        Rect::new(thumb_x, thumb_y, thumb_w, thumb_h)
    }

    /// Set track height.
    pub fn with_track_height(mut self, height: f32) -> Self {
        self.track_height = height;
        self
    }

    /// Set thumb dimensions.
    pub fn with_thumb_size(mut self, size: [f32; 2]) -> Self {
        self.thumb_size = size;
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

    /// Obtain a borrowed handle to inspect the slider's value.
    pub fn handle(&self) -> ControlHandle<'_, Self> {
        ControlHandle::new(self)
    }
}

impl DrawControl for Slider {
    fn draw(&self, list: &mut DrawList, ctx: &DrawContext) {
        let track_bg = self.track_bg_color.unwrap_or(ctx.theme.track_bg_color);
        let track_fill = self.track_fill_color.unwrap_or(ctx.theme.track_fill_color);
        let thumb_col = self.thumb_color.unwrap_or(ctx.theme.thumb_color);
        let thumb_border_col = self
            .thumb_border_color
            .unwrap_or(ctx.theme.thumb_border_color);

        let thumb_rect = self.thumb_rect();
        let track_y = self.rect.y + (self.rect.h - self.track_height) * 0.5;
        let track_rad = self.track_height * 0.5;

        let track_rect = Rect::new(self.rect.x, track_y, self.rect.w, self.track_height);
        list.push(SdfInstance {
            kind: SdfKind::RoundedBox { radius: track_rad },
            position: track_rect.pos(),
            size: track_rect.size(),
            color: track_bg,
            anim: 0,
        });

        let fill_w = (thumb_rect.center()[0] - self.rect.x).clamp(0.0, self.rect.w);
        if fill_w > 0.0 {
            list.push(SdfInstance {
                kind: SdfKind::RoundedBox { radius: track_rad },
                position: [self.rect.x, track_y],
                size: [fill_w, self.track_height],
                color: track_fill,
                anim: 0,
            });
        }

        match self.thumb_style {
            ThumbStyle::SquaredNotch => {
                let notch_rad = 1.5f32;
                list.push(SdfInstance {
                    kind: SdfKind::RoundedBox { radius: notch_rad },
                    position: thumb_rect.pos(),
                    size: thumb_rect.size(),
                    color: thumb_col,
                    anim: 0,
                });
                list.push(SdfInstance {
                    kind: SdfKind::Outline {
                        radius: notch_rad,
                        thickness: 1.2,
                    },
                    position: thumb_rect.pos(),
                    size: thumb_rect.size(),
                    color: thumb_border_col,
                    anim: 0,
                });
            }
            ThumbStyle::Circle => {
                let circle_diam = thumb_rect.w.min(thumb_rect.h);
                let circle_rect = Rect::new(
                    thumb_rect.center()[0] - circle_diam * 0.5,
                    thumb_rect.center()[1] - circle_diam * 0.5,
                    circle_diam,
                    circle_diam,
                );
                list.push(SdfInstance {
                    kind: SdfKind::Circle,
                    position: circle_rect.pos(),
                    size: circle_rect.size(),
                    color: thumb_col,
                    anim: 0,
                });
                list.push(SdfInstance {
                    kind: SdfKind::Outline {
                        radius: circle_diam * 0.5,
                        thickness: 1.2,
                    },
                    position: circle_rect.pos(),
                    size: circle_rect.size(),
                    color: thumb_border_col,
                    anim: 0,
                });
            }
        }

        if self.is_active() {
            let glow_style = self.glow_style.unwrap_or_default();
            draw_glowing_ring(list, thumb_rect, &glow_style, &ctx.theme);
        }
    }
}

use crate::event::{ControlAction, ControlPart, Key, KeyboardEvent, MouseButton, MouseEvent};
use crate::host::HostedControl;

impl HostedControl for Slider {
    fn bounds(&self) -> Rect {
        self.rect
    }

    fn hit_test(&self, pos: [f32; 2]) -> Option<(ControlPart, f32)> {
        if !self.rect.contains(pos) {
            return None;
        }
        let frac = self.rect.normalized_x(pos[0]);
        let thumb_r = self.thumb_rect();
        if thumb_r.contains(pos) {
            Some((ControlPart::Thumb, frac))
        } else {
            Some((ControlPart::Track, frac))
        }
    }

    fn handle_mouse(&self, event: &MouseEvent, _hit_part: ControlPart) -> Option<ControlAction> {
        match *event {
            MouseEvent::Down {
                pos,
                button: MouseButton::Left,
            }
            | MouseEvent::Click {
                pos,
                button: MouseButton::Left,
            }
            | MouseEvent::Move { pos }
            | MouseEvent::Drag {
                current_pos: pos,
                button: MouseButton::Left,
                ..
            } => {
                let frac = self.rect.normalized_x(pos[0]).clamp(0.0, 1.0);
                let new_value = self.min + frac * (self.max - self.min);
                self.set_value(new_value);
                Some(ControlAction::ValueChange {
                    new_value,
                    fraction: frac,
                })
            }
            MouseEvent::Scroll { delta, .. } => {
                let step = (self.max - self.min) * 0.02;
                let delta_val = if delta[1] > 0.0 || delta[0] > 0.0 {
                    step
                } else {
                    -step
                };
                let new_value = (self.value() + delta_val).clamp(self.min, self.max);
                let frac = if (self.max - self.min).abs() < f32::EPSILON {
                    0.0
                } else {
                    (new_value - self.min) / (self.max - self.min)
                };
                self.set_value(new_value);
                Some(ControlAction::ValueChange {
                    new_value,
                    fraction: frac,
                })
            }
            _ => None,
        }
    }

    fn handle_keyboard(&self, event: &KeyboardEvent) -> Option<ControlAction> {
        if !event.is_down() {
            return None;
        }
        let range = self.max - self.min;
        let small_step = range * 0.02;
        let large_step = range * 0.10;

        let new_value = match event.key() {
            Some(Key::Left | Key::Down) => (self.value() - small_step).max(self.min),
            Some(Key::Right | Key::Up) => (self.value() + small_step).min(self.max),
            Some(Key::PageDown) => (self.value() - large_step).max(self.min),
            Some(Key::PageUp) => (self.value() + large_step).min(self.max),
            Some(Key::Home) => self.min,
            Some(Key::End) => self.max,
            _ => return None,
        };

        let fraction = if range.abs() < f32::EPSILON {
            0.0
        } else {
            (new_value - self.min) / range
        };

        self.set_value(new_value);

        Some(ControlAction::ValueChange {
            new_value,
            fraction,
        })
    }
}

impl ReadValue<f32> for Slider {
    fn read_value(&self) -> f32 {
        self.value()
    }
}
