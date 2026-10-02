//! Composite slider and numeric value readout pair composing Slider, Edit, and Label.

use std::sync::Arc;
use std::sync::Mutex;

use libmsdf::drawlist::DrawList;

use crate::edit::{Edit, EditState};
use crate::event::{ControlAction, ControlPart, KeyboardEvent, MouseEvent};
use crate::handle::{ControlHandle, ReadValue};
use crate::host::HostedControl;
use crate::label::Label;
use crate::rect::{Rect, TextAlign};
use crate::ring::{GlowRingStyle, draw_glowing_ring};
use crate::slider::{Slider, SliderState};
use crate::traits::{DrawContext, DrawControl};

/// Formatting options for the numeric value display.
#[derive(Clone, Debug)]
pub enum NumericFormat {
    /// Fixed decimal places, e.g.
    Decimal { places: usize },
    /// Percentage display with `%` suffix, e.g.
    Percentage,
    /// Integer display, e.g.
    Integer,
    /// Custom static string mapper.
    Custom(fn(f32) -> String),
}

impl Default for NumericFormat {
    fn default() -> Self {
        Self::Decimal { places: 2 }
    }
}

impl NumericFormat {
    /// Format a numeric value into a string.
    pub fn format_value(&self, value: f32) -> String {
        match self {
            Self::Decimal { places } => format!("{value:.places$}"),
            Self::Percentage => format!("{:.0}%", value * 100.0),
            Self::Integer => format!("{:.0}", value.round()),
            Self::Custom(f) => f(value),
        }
    }
}

/// Specifies which sub-component of `SliderNumeric` is actively focused.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum SliderNumericActive {
    #[default]
    None,
    /// The slider handle is active.
    Slider,
    /// The numeric value readout box is active.
    Value,
    /// The leading label is active.
    Label,
    /// The entire composite control is active.
    EntireControl,
}

/// Mutable runtime state for a composite slider numeric pair.
#[derive(Debug)]
pub struct SliderNumericState {
    /// Shared mutable slider state.
    pub slider: Arc<SliderState>,
    /// Shared mutable edit box state.
    pub edit: Arc<EditState>,
    /// Active focus sub-component.
    pub active_part: Mutex<SliderNumericActive>,
}

impl SliderNumericState {
    /// Create a new composite state with initial numerical value.
    pub fn new(value: f32) -> Self {
        let default_formatted = NumericFormat::default().format_value(value);
        Self {
            slider: Arc::new(SliderState::new(value)),
            edit: Arc::new(EditState::new(default_formatted)),
            active_part: Mutex::new(SliderNumericActive::None),
        }
    }

    /// Create from an existing shared `SliderState`.
    pub fn with_slider_state(slider: Arc<SliderState>) -> Self {
        let default_formatted = NumericFormat::default().format_value(slider.value());
        Self {
            slider: Arc::clone(&slider),
            edit: Arc::new(EditState::new(default_formatted)),
            active_part: Mutex::new(SliderNumericActive::None),
        }
    }

    /// Read the current numerical value.
    #[inline]
    pub fn value(&self) -> f32 {
        self.slider.value()
    }

    /// Update the current numerical value.
    #[inline]
    pub fn set_value(&self, val: f32) {
        self.slider.set_value(val);
    }

    /// Read the active focus sub-component.
    #[inline]
    pub fn active_part(&self) -> SliderNumericActive {
        *self.active_part.lock().unwrap()
    }

    /// Set the active focus sub-component.
    #[inline]
    pub fn set_active_part(&self, part: SliderNumericActive) {
        *self.active_part.lock().unwrap() = part;
    }
}

/// Composite slider and numeric readout pair composing [`Slider`], [`Edit`], and [`Label`].
#[derive(Clone, Debug)]
pub struct SliderNumeric {
    /// Shared mutable composite state.
    pub state: Arc<SliderNumericState>,
    /// Composed slider control.
    pub slider: Slider,
    /// Composed edit box control for the numeric readout.
    pub edit: Edit,
    /// Optional composed leading label control.
    pub label: Option<Label>,
    /// Bounding rectangle for the complete composite control.
    pub rect: Rect,
    /// Width reserved for the leading label (if present).
    pub label_width: f32,
    /// Width reserved for the numeric readout on the trailing side.
    pub numeric_width: f32,
    /// Gap between adjacent sub-components in pixels.
    pub gap: f32,
    /// Number formatting strategy.
    pub format: NumericFormat,
    /// Optional custom glow ring style.
    pub glow_style: Option<GlowRingStyle>,
}

impl SliderNumeric {
    /// Create a new composite slider and numeric readout pair.
    pub fn new(value: f32, min: f32, max: f32, rect: Rect) -> Self {
        let format = NumericFormat::default();
        let formatted = format.format_value(value);
        let slider_state = Arc::new(SliderState::new(value));
        let edit_state = Arc::new(EditState::new(formatted));
        let state = Arc::new(SliderNumericState {
            slider: Arc::clone(&slider_state),
            edit: Arc::clone(&edit_state),
            active_part: Mutex::new(SliderNumericActive::None),
        });
        let slider = Slider::new(value, min, max, rect).with_state(Arc::clone(&slider_state));
        let edit = Edit::new(format.format_value(value), rect)
            .with_state(Arc::clone(&edit_state))
            .with_align(TextAlign::Center);

        Self {
            state,
            slider,
            edit,
            label: None,
            rect,
            label_width: 60.0,
            numeric_width: 50.0,
            gap: 8.0,
            format,
            glow_style: None,
        }
    }

    /// Attach an existing shared `SliderNumericState`.
    pub fn with_state(mut self, state: Arc<SliderNumericState>) -> Self {
        self.slider = self.slider.with_state(Arc::clone(&state.slider));
        self.edit = self.edit.with_state(Arc::clone(&state.edit));
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
        self.state.edit.set_text(self.format.format_value(val));
    }

    /// Read the active focus sub-component.
    #[inline]
    pub fn active_part(&self) -> SliderNumericActive {
        self.state.active_part()
    }

    /// Attach a leading descriptor label.
    pub fn with_label(mut self, label_text: impl Into<String>) -> Self {
        let text = label_text.into();
        let label = Label::new(text, self.rect);
        self.label = Some(label);
        self
    }

    /// Set font size on composed controls.
    pub fn with_font_size(mut self, font_size: f32) -> Self {
        self.edit = self.edit.with_font_size(font_size);
        if let Some(lbl) = self.label.take() {
            self.label = Some(lbl.with_font_size(font_size));
        }
        self
    }

    /// Set label width.
    pub fn with_label_width(mut self, width: f32) -> Self {
        self.label_width = width;
        self
    }

    /// Set numeric readout box width.
    pub fn with_numeric_width(mut self, width: f32) -> Self {
        self.numeric_width = width;
        self
    }

    /// Set formatting strategy.
    pub fn with_format(mut self, format: NumericFormat) -> Self {
        let formatted = format.format_value(self.value());
        self.edit.set_text(formatted);
        self.format = format;
        self
    }

    /// Set active highlight state.
    pub fn with_active(self, active: SliderNumericActive) -> Self {
        self.state.set_active_part(active);
        self
    }

    /// Set active state specifically on the numeric value readout.
    pub fn with_active_value(self, is_active: bool) -> Self {
        self.state.set_active_part(if is_active {
            SliderNumericActive::Value
        } else {
            SliderNumericActive::None
        });
        self
    }

    /// Set custom glow ring style.
    pub fn with_glow_style(mut self, style: GlowRingStyle) -> Self {
        self.glow_style = Some(style);
        self.slider = self.slider.with_glow_style(style);
        self.edit = self.edit.with_glow_style(style);
        if let Some(lbl) = self.label.take() {
            self.label = Some(lbl.with_glow_style(style));
        }
        self
    }

    /// Obtain a borrowed handle to inspect the composite slider's value.
    pub fn handle(&self) -> ControlHandle<'_, Self> {
        ControlHandle::new(self)
    }

    /// Compute partition rectangles: `(Option<label_rect>, slider_rect, numeric_rect)`.
    pub fn sub_rects(&self) -> (Option<Rect>, Rect, Rect) {
        let num_w = self.numeric_width.min(self.rect.w * 0.4);
        let num_x = self.rect.x + self.rect.w - num_w;
        let num_rect = Rect::new(num_x, self.rect.y, num_w, self.rect.h);

        let mut available_x = self.rect.x;
        let mut available_w = (num_x - self.gap - self.rect.x).max(0.0);

        let label_rect = if self.label.is_some() {
            let lbl_w = self.label_width.min(available_w * 0.5);
            let r = Rect::new(available_x, self.rect.y, lbl_w, self.rect.h);
            available_x += lbl_w + self.gap;
            available_w = (available_w - lbl_w - self.gap).max(0.0);
            Some(r)
        } else {
            None
        };

        let slider_rect = Rect::new(available_x, self.rect.y, available_w, self.rect.h);
        (label_rect, slider_rect, num_rect)
    }
}

impl DrawControl for SliderNumeric {
    fn draw(&self, list: &mut DrawList, ctx: &DrawContext) {
        let (label_rect, slider_rect, edit_rect) = self.sub_rects();
        let glow_style = self.glow_style.unwrap_or_default();
        let active_part = self.active_part();

        if let (Some(lbl), Some(lbl_r)) = (&self.label, label_rect) {
            let mut lbl_clone = lbl.clone();
            lbl_clone.rect = lbl_r;
            if lbl_clone.color.is_none() {
                lbl_clone.color = Some(ctx.theme.muted_text_color);
            }
            lbl_clone
                .state
                .set_active(active_part == SliderNumericActive::Label);
            lbl_clone.draw(list, ctx);
        }

        let mut slider_clone = self.slider.clone();
        slider_clone.rect = slider_rect;
        slider_clone
            .state
            .set_active(active_part == SliderNumericActive::Slider);
        slider_clone.draw(list, ctx);

        let mut edit_clone = self.edit.clone();
        edit_clone.rect = edit_rect;
        edit_clone.set_text(self.format.format_value(self.value()));
        edit_clone
            .state
            .set_active(active_part == SliderNumericActive::Value);
        edit_clone.draw(list, ctx);

        if active_part == SliderNumericActive::EntireControl {
            draw_glowing_ring(list, self.rect, &glow_style, &ctx.theme);
        }
    }
}

impl HostedControl for SliderNumeric {
    fn bounds(&self) -> Rect {
        self.rect
    }

    fn hit_test(&self, pos: [f32; 2]) -> Option<(ControlPart, f32)> {
        if !self.rect.contains(pos) {
            return None;
        }
        let (lbl_r, slider_r, edit_r) = self.sub_rects();
        if let (Some(lbl), Some(r)) = (&self.label, lbl_r) {
            let mut l = lbl.clone();
            l.rect = r;
            if let Some((part, frac)) = l.hit_test(pos) {
                let part_mapped = match part {
                    ControlPart::Body => ControlPart::Label,
                    other => other,
                };
                return Some((part_mapped, frac));
            }
        }
        if edit_r.contains(pos) {
            let mut e = self.edit.clone();
            e.rect = edit_r;
            return e.hit_test(pos);
        }
        if slider_r.contains(pos) {
            let mut s = self.slider.clone();
            s.rect = slider_r;
            return s.hit_test(pos);
        }
        Some((ControlPart::Body, self.rect.normalized_x(pos[0])))
    }

    fn handle_mouse(&self, event: &MouseEvent, hit_part: ControlPart) -> Option<ControlAction> {
        let (lbl_r, slider_r, edit_r) = self.sub_rects();
        match hit_part {
            ControlPart::Label => {
                if let (Some(lbl), Some(r)) = (&self.label, lbl_r) {
                    let mut l = lbl.clone();
                    l.rect = r;
                    l.handle_mouse(event, ControlPart::Body)
                } else {
                    None
                }
            }
            ControlPart::NumericBox => {
                let mut e = self.edit.clone();
                e.rect = edit_r;
                e.handle_mouse(event, hit_part)
            }
            ControlPart::Track | ControlPart::Thumb | ControlPart::Body => {
                let mut s = self.slider.clone();
                s.rect = slider_r;
                let action = s.handle_mouse(event, hit_part);
                self.state
                    .edit
                    .set_text(self.format.format_value(self.value()));
                action
            }
            _ => None,
        }
    }

    fn handle_keyboard(&self, event: &KeyboardEvent) -> Option<ControlAction> {
        let action = self.slider.handle_keyboard(event);
        self.state
            .edit
            .set_text(self.format.format_value(self.value()));
        action
    }
}

impl ReadValue<f32> for SliderNumeric {
    fn read_value(&self) -> f32 {
        self.value()
    }
}

impl ReadValue<String> for SliderNumeric {
    fn read_value(&self) -> String {
        self.format.format_value(self.value())
    }
}
