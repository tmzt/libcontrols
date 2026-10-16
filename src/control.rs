//! The control traits: what a control must do to be drawn, hosted in a
//! router and given input.

use crate::event::{
    ControlAction, ControlPart, EventContext, FocusEvent, GestureEvent, KeyboardEvent, MouseEvent,
};
use crate::flags::NodeFlags;
use crate::handle::ControlHandle;
use crate::rect::Rect;
use crate::traits::{DrawContext, DrawControl};

/// Trait implemented by controls that can be hosted, drawn, and receive input events.
pub trait Control: DrawControl + HostedControl {}

impl<T: DrawControl + HostedControl> Control for T {}

/// Trait implemented by controls that can be hosted and receive input events.
pub trait HostedControl {
    /// Bounding rectangle for the control in absolute coordinates.
    fn bounds(&self) -> Rect;

    /// Hit test a point against this control's geometry.
    fn hit_test(&self, pos: [f32; 2]) -> Option<(ControlPart, f32)>;

    /// Handle a mouse event targeting this control. `ctx.part` is the part
    /// it is for; `ctx.modifiers` the modifier keys held.
    fn handle_mouse(&self, event: &MouseEvent, ctx: &EventContext) -> Option<ControlAction>;

    /// Handle a keyboard event targeting this control when focused.
    fn handle_keyboard(&self, event: &KeyboardEvent, ctx: &EventContext) -> Option<ControlAction>;

    /// Record which part of this control the pointer is over, or that it has left.
    fn set_hover(&self, _hover: Option<ControlPart>) {}

    /// Handle a gesture made with the pointer over this control.
    fn handle_gesture(&self, _event: &GestureEvent, _ctx: &EventContext) -> Option<ControlAction> {
        None
    }

    /// The host's focus arrived at this control or left it. May be told
    /// `Gained` again by a rebuilt host, so it must tolerate repeats.
    fn handle_focus(&self, _event: FocusEvent) {}

    /// The flags a router node starts with when this control is added.
    fn default_flags(&self) -> NodeFlags {
        NodeFlags::FOCUSABLE
    }
}

impl<'a, C: DrawControl> DrawControl for ControlHandle<'a, C> {
    #[inline]
    fn draw(&self, list: &mut libmsdf::drawlist::DrawList, ctx: &DrawContext) {
        self.control().draw(list, ctx);
    }

    #[inline]
    fn draw_overlay(&self, list: &mut libmsdf::drawlist::DrawList, ctx: &DrawContext) {
        self.control().draw_overlay(list, ctx);
    }
}

impl<'a, C: HostedControl> HostedControl for ControlHandle<'a, C> {
    #[inline]
    fn bounds(&self) -> Rect {
        self.control().bounds()
    }

    #[inline]
    fn hit_test(&self, pos: [f32; 2]) -> Option<(ControlPart, f32)> {
        self.control().hit_test(pos)
    }

    #[inline]
    fn handle_mouse(&self, event: &MouseEvent, ctx: &EventContext) -> Option<ControlAction> {
        self.control().handle_mouse(event, ctx)
    }

    #[inline]
    fn handle_keyboard(&self, event: &KeyboardEvent, ctx: &EventContext) -> Option<ControlAction> {
        self.control().handle_keyboard(event, ctx)
    }

    #[inline]
    fn set_hover(&self, hover: Option<ControlPart>) {
        self.control().set_hover(hover);
    }

    #[inline]
    fn handle_gesture(&self, event: &GestureEvent, ctx: &EventContext) -> Option<ControlAction> {
        self.control().handle_gesture(event, ctx)
    }

    #[inline]
    fn handle_focus(&self, event: FocusEvent) {
        self.control().handle_focus(event);
    }

    #[inline]
    fn default_flags(&self) -> NodeFlags {
        self.control().default_flags()
    }
}
