//! Interactive push button and toggle button control.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use libmsdf::drawlist::{DrawList, LINE_BOX_RATIO, SdfInstance, SdfKind};

use crate::event::{ControlAction, ControlPart, Key, KeyboardEvent, MouseButton, MouseEvent};
use crate::handle::{ControlHandle, ReadValue};
use crate::host::HostedControl;
use crate::rect::{Rect, TextAlign};
use crate::ring::{GlowRingStyle, draw_glowing_ring};
use crate::tooltip::{TooltipStyle, draw_tooltip, split_label};
use crate::traits::{DrawContext, DrawControl};

/// Mutable runtime state for a push or toggle button.
#[derive(Debug)]
pub struct ButtonState {
    /// Whether this button is currently in a pressed or toggled-on state.
    pub is_pressed: AtomicBool,
    /// Whether this button is currently focused/active.
    pub is_active: AtomicBool,
    /// Whether the button has been pressed since anyone last looked.
    pub was_clicked: AtomicBool,
    /// Whether a press should leave the button down.
    pub is_momentary: AtomicBool,
    /// Whether the pointer is over the button.
    pub is_hovered: AtomicBool,
}

impl ButtonState {
    /// Create a latching button: a press leaves it down, and its pressed state is what it
    /// means.
    pub fn new(is_pressed: bool) -> Self {
        Self {
            is_pressed: AtomicBool::new(is_pressed),
            is_active: AtomicBool::new(false),
            was_clicked: AtomicBool::new(false),
            is_momentary: AtomicBool::new(false),
            is_hovered: AtomicBool::new(false),
        }
    }

    /// Create a momentary button: a press is an event to be taken with `take_click`, and the
    /// pressed state is left to whoever owns it to set, so a tab or a radio button can still be
    /// shown as the one that is on.
    pub fn momentary(is_pressed: bool) -> Self {
        let state = Self::new(is_pressed);
        state.is_momentary.store(true, Ordering::Relaxed);
        state
    }

    /// Whether this button lets go of a press.
    #[inline]
    pub fn is_momentary(&self) -> bool {
        self.is_momentary.load(Ordering::Relaxed)
    }

    /// Record a press, and leave the button down if it latches.
    #[inline]
    pub fn click(&self) {
        self.was_clicked.store(true, Ordering::Relaxed);
        if !self.is_momentary() {
            self.toggle();
        }
    }

    /// Take the press, if there has been one since this was last called.
    #[inline]
    pub fn take_click(&self) -> bool {
        self.was_clicked.swap(false, Ordering::Relaxed)
    }

    /// Read pressed/toggled state.
    #[inline]
    pub fn is_pressed(&self) -> bool {
        self.is_pressed.load(Ordering::Relaxed)
    }

    /// Set pressed/toggled state.
    #[inline]
    pub fn set_pressed(&self, pressed: bool) {
        self.is_pressed.store(pressed, Ordering::Relaxed);
    }

    /// Toggle pressed state.
    #[inline]
    pub fn toggle(&self) -> bool {
        let prev = self.is_pressed.fetch_xor(true, Ordering::Relaxed);
        !prev
    }

    /// Check if active/focused.
    #[inline]
    pub fn is_active(&self) -> bool {
        self.is_active.load(Ordering::Relaxed)
    }

    /// Set active/focused state.
    #[inline]
    pub fn set_active(&self, active: bool) {
        self.is_active.store(active, Ordering::Relaxed);
    }

    /// Whether the pointer is over the button.
    #[inline]
    pub fn is_hovered(&self) -> bool {
        self.is_hovered.load(Ordering::Relaxed)
    }

    /// Set whether the pointer is over the button.
    #[inline]
    pub fn set_hovered(&self, hovered: bool) {
        self.is_hovered.store(hovered, Ordering::Relaxed);
    }
}

/// Interactive push or toggle button control.
#[derive(Clone, Debug)]
pub struct Button {
    /// Shared mutable button state.
    pub state: Arc<ButtonState>,
    /// Button label text.
    pub text: String,
    /// Bounding box for button placement.
    pub rect: Rect,
    /// Text horizontal alignment.
    pub align: TextAlign,
    /// Optional font size override.
    pub font_size: Option<f32>,
    /// Optional text color override.
    pub text_color: Option<[f32; 4]>,
    /// Optional background fill color.
    pub bg_color: Option<[f32; 4]>,
    /// Optional border outline color.
    pub border_color: Option<[f32; 4]>,
    /// Border thickness in pixels.
    pub border_thickness: f32,
    /// Optional corner radius.
    pub corner_radius: Option<f32>,
    /// Optional custom glow ring style.
    pub glow_style: Option<GlowRingStyle>,
    /// Optional custom tooltip style.
    pub tooltip_style: Option<TooltipStyle>,
}

impl Button {
    /// Create a new button with label and rectangle.
    pub fn new(text: impl Into<String>, rect: Rect) -> Self {
        Self {
            state: Arc::new(ButtonState::new(false)),
            text: text.into(),
            rect,
            align: TextAlign::Center,
            font_size: None,
            text_color: None,
            bg_color: None,
            border_color: None,
            border_thickness: 1.0,
            corner_radius: None,
            glow_style: None,
            tooltip_style: None,
        }
    }

    /// Attach a shared `ButtonState`.
    pub fn with_state(mut self, state: Arc<ButtonState>) -> Self {
        self.state = state;
        self
    }

    /// Read pressed/toggled state.
    #[inline]
    pub fn is_pressed(&self) -> bool {
        self.state.is_pressed()
    }

    /// Read active focus state.
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

    /// Set pressed / toggled state.
    pub fn with_pressed(self, is_pressed: bool) -> Self {
        self.state.set_pressed(is_pressed);
        self
    }

    /// Set custom glow ring style.
    pub fn with_glow_style(mut self, style: GlowRingStyle) -> Self {
        self.glow_style = Some(style);
        self
    }

    /// Set custom tooltip style.
    pub fn with_tooltip_style(mut self, style: TooltipStyle) -> Self {
        self.tooltip_style = Some(style);
        self
    }

    /// Obtain a borrowed handle to inspect the button's value.
    pub fn handle(&self) -> ControlHandle<'_, Self> {
        ControlHandle::new(self)
    }

    /// The text the button draws, which is its label up to the tooltip's separator.
    #[inline]
    pub fn shown_text(&self) -> &str {
        split_label(&self.text).0
    }

    /// The button's tooltip, if its label carries one.
    #[inline]
    pub fn tooltip(&self) -> Option<&str> {
        split_label(&self.text).1
    }

    /// Whether the pointer is over the button.
    #[inline]
    pub fn is_hovered(&self) -> bool {
        self.state.is_hovered()
    }
}

impl DrawControl for Button {
    fn draw(&self, list: &mut DrawList, ctx: &DrawContext) {
        let font_size = ctx.font_size(self.font_size);
        let rad = self.corner_radius.unwrap_or(ctx.theme.corner_radius);

        let default_bg = [0.08, 0.12, 0.18, 0.90];
        let bg_col = if self.is_pressed() {
            self.bg_color
                .map(|c| [c[0] * 1.5, c[1] * 1.5, c[2] * 1.5, c[3]])
                .unwrap_or([0.16, 0.32, 0.52, 0.95])
        } else {
            self.bg_color.unwrap_or(default_bg)
        };

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

        let border_col = if self.is_pressed() {
            [0.35, 0.88, 1.0, 0.95]
        } else {
            self.border_color.unwrap_or([0.18, 0.65, 0.92, 0.60])
        };
        if self.border_thickness > 0.0 {
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

        let text_col = if self.is_pressed() {
            [1.0, 1.0, 1.0, 1.0]
        } else {
            ctx.text_color(self.text_color)
        };
        let shown = self.shown_text();
        let text_advance = ctx.text_advance(shown, font_size);
        let line_box_h = font_size * LINE_BOX_RATIO;
        let pad_h = 6.0f32;

        let pen_x = match self.align {
            TextAlign::Left => self.rect.x + pad_h,
            TextAlign::Center => self.rect.x + (self.rect.w - text_advance) * 0.5,
            TextAlign::Right => self.rect.x + self.rect.w - pad_h - text_advance,
        };
        let pen_y = self.rect.y + (self.rect.h - line_box_h) * 0.5;

        ctx.draw_text(list, shown, [pen_x, pen_y], font_size, text_col);

        if self.is_active() {
            let style = self.glow_style.unwrap_or_default();
            draw_glowing_ring(list, self.rect, &style, &ctx.theme);
        }
    }

    fn draw_overlay(&self, list: &mut DrawList, ctx: &DrawContext) {
        if !self.is_hovered() {
            return;
        }
        if let Some(tip) = self.tooltip() {
            let style = self.tooltip_style.unwrap_or_default();
            draw_tooltip(list, ctx, self.rect, tip, &style);
        }
    }
}

impl HostedControl for Button {
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
            } => {
                self.state.click();
                Some(ControlAction::Activated { part: hit_part })
            }
            _ => None,
        }
    }

    fn handle_keyboard(&self, event: &KeyboardEvent) -> Option<ControlAction> {
        if event.is_down()
            && let Some(Key::Enter | Key::Space) = event.key()
        {
            self.state.click();
            return Some(ControlAction::Activated {
                part: ControlPart::Body,
            });
        }
        None
    }

    fn set_hover(&self, hover: Option<ControlPart>) {
        self.state.set_hovered(hover.is_some());
    }
}

impl ReadValue<String> for Button {
    fn read_value(&self) -> String {
        self.text.clone()
    }
}

impl ReadValue<bool> for Button {
    fn read_value(&self) -> bool {
        self.is_pressed()
    }
}

#[cfg(test)]
mod momentary_tests {
    use super::ButtonState;

    #[test]
    fn a_latching_button_stays_down_and_a_momentary_one_does_not() {
        let latch = ButtonState::new(false);
        latch.click();
        assert!(latch.is_pressed(), "a latching button holds its press");

        let tap = ButtonState::momentary(false);
        tap.click();
        assert!(!tap.is_pressed(), "a momentary button lets go of its press");
    }

    #[test]
    fn a_press_is_taken_once_however_many_events_reported_it() {
        let tap = ButtonState::momentary(false);
        tap.click();
        tap.click();
        assert!(tap.take_click(), "the press must be there to take");
        assert!(!tap.take_click(), "and must only be taken once");
    }

    #[test]
    fn a_momentary_button_can_still_be_shown_as_the_one_that_is_on() {
        let tab = ButtonState::momentary(false);
        tab.set_pressed(true);
        assert!(tab.is_pressed(), "being on is the owner's to say");
        assert!(!tab.take_click(), "and is not a press");
    }
}
