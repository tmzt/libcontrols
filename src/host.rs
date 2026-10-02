//! `ControlHost` — Input event dispatcher and focus manager for UI controls.

use crate::event::{
    ControlAction, ControlHit, ControlPart, EventResponse, InputEvent, Key, KeyboardEvent,
    MouseButton, MouseEvent,
};
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

    /// Handle a mouse event targeting this control.
    fn handle_mouse(&self, event: &MouseEvent, hit_part: ControlPart) -> Option<ControlAction>;

    /// Handle a keyboard event targeting this control when focused.
    fn handle_keyboard(&self, event: &KeyboardEvent) -> Option<ControlAction>;

    /// Record which part of this control the pointer is over, or that it has left.
    fn set_hover(&self, _hover: Option<ControlPart>) {}
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
    fn handle_mouse(&self, event: &MouseEvent, hit_part: ControlPart) -> Option<ControlAction> {
        self.control().handle_mouse(event, hit_part)
    }

    #[inline]
    fn handle_keyboard(&self, event: &KeyboardEvent) -> Option<ControlAction> {
        self.control().handle_keyboard(event)
    }

    #[inline]
    fn set_hover(&self, hover: Option<ControlPart>) {
        self.control().set_hover(hover);
    }
}

/// Builder for constructing a [`ControlHost`].
#[derive(Default)]
pub struct ControlHostBuilder<'a> {
    controls: Vec<Box<dyn Control + 'a>>,
    control_ids: Vec<Option<String>>,
    initial_focus: Option<usize>,
}

impl<'a> ControlHostBuilder<'a> {
    /// Create an empty builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a control handle to the host.
    pub fn with_control<C: Control + 'a>(mut self, handle: ControlHandle<'a, C>) -> Self {
        self.controls.push(Box::new(handle));
        self.control_ids.push(None);
        self
    }

    /// Add a control handle with a string identifier for easy lookup.
    pub fn with_named_control<C: Control + 'a>(
        mut self,
        id: impl Into<String>,
        handle: ControlHandle<'a, C>,
    ) -> Self {
        self.controls.push(Box::new(handle));
        self.control_ids.push(Some(id.into()));
        self
    }

    /// Add an arbitrary hosted control instance.
    pub fn with_custom_control<H: Control + 'a>(mut self, control: H) -> Self {
        self.controls.push(Box::new(control));
        self.control_ids.push(None);
        self
    }

    /// Set initial focused control index.
    pub fn with_initial_focus(mut self, index: usize) -> Self {
        self.initial_focus = Some(index);
        self
    }

    /// Build the `ControlHost`.
    pub fn build(self) -> ControlHost<'a> {
        let mut host = ControlHost {
            controls: self.controls,
            control_ids: self.control_ids,
            focused: None,
            active_drag: None,
        };
        host.set_focused_index(self.initial_focus);
        host
    }
}

/// A lasting way of naming one control, for state that outlives a rebuild.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ControlKey {
    /// A control registered under a name, found again by that name.
    Named(String),
    /// A control registered without one, which is only its place in the list.
    Slot(usize),
}

impl ControlKey {
    /// A key for a control registered under `id`.
    #[must_use]
    pub fn named(id: impl Into<String>) -> Self {
        Self::Named(id.into())
    }

    /// A key for a control that has no name, which can only be its place.
    #[must_use]
    pub fn at(index: usize) -> Self {
        Self::Slot(index)
    }

    /// The control's name, if it has one.
    #[must_use]
    pub fn id(&self) -> Option<&str> {
        match self {
            Self::Named(id) => Some(id),
            Self::Slot(_) => None,
        }
    }
}

/// A drag in progress, in a form that can be carried across a rebuild.
#[derive(Clone, Debug, PartialEq)]
pub struct DragGrab {
    /// Which control is being dragged.
    pub key: ControlKey,
    /// Where the press that started it landed.
    pub start_pos: [f32; 2],
    /// The button holding it.
    pub button: MouseButton,
}

/// Host for managing absolute input event routing and focus across registered controls.
pub struct ControlHost<'a> {
    controls: Vec<Box<dyn Control + 'a>>,
    control_ids: Vec<Option<String>>,
    focused: Option<ControlKey>,
    active_drag: Option<DragGrab>,
}

impl<'a> ControlHost<'a> {
    /// Create a new builder for `ControlHost`.
    pub fn builder() -> ControlHostBuilder<'a> {
        ControlHostBuilder::new()
    }

    /// Total number of registered controls.
    pub fn control_count(&self) -> usize {
        self.controls.len()
    }

    /// Add a control after the host has been built.
    pub fn add_control<C: Control + 'a>(&mut self, control: C) {
        self.controls.push(Box::new(control));
        self.control_ids.push(None);
    }

    /// Add a named control after the host has been built.
    pub fn add_named_control<C: Control + 'a>(&mut self, id: impl Into<String>, control: C) {
        self.controls.push(Box::new(control));
        self.control_ids.push(Some(id.into()));
    }

    /// A key naming the control at `index`, if there is one there.
    #[must_use]
    pub fn control_key(&self, index: usize) -> Option<ControlKey> {
        match self.control_ids.get(index)? {
            Some(id) => Some(ControlKey::Named(id.clone())),
            None => Some(ControlKey::Slot(index)),
        }
    }

    /// Where the control a key names sits now, if it is still here.
    #[must_use]
    pub fn resolve(&self, key: &ControlKey) -> Option<usize> {
        match key {
            ControlKey::Named(id) => self.find_by_id(id),
            ControlKey::Slot(index) => (*index < self.controls.len()).then_some(*index),
        }
    }

    /// Currently focused control index.
    pub fn focused_index(&self) -> Option<usize> {
        self.focused.as_ref().and_then(|key| self.resolve(key))
    }

    /// The focused control as a key, to be carried to the next build.
    #[must_use]
    pub fn focused_key(&self) -> Option<ControlKey> {
        self.focused.clone()
    }

    /// Put the focus back on a named control after a rebuild.
    pub fn set_focused_key(&mut self, key: Option<ControlKey>) {
        self.focused = key.filter(|key| self.resolve(key).is_some());
    }

    /// Set the focused control index.
    pub fn set_focused_index(&mut self, index: Option<usize>) {
        self.focused = index.and_then(|idx| self.control_key(idx));
    }

    /// Move focus to the next control.
    pub fn focus_next(&mut self) -> Option<usize> {
        if self.controls.is_empty() {
            self.focused = None;
            return None;
        }
        let next = match self.focused_index() {
            Some(curr) => (curr + 1) % self.controls.len(),
            None => 0,
        };
        self.set_focused_index(Some(next));
        Some(next)
    }

    /// Move focus to the previous control.
    pub fn focus_prev(&mut self) -> Option<usize> {
        if self.controls.is_empty() {
            self.focused = None;
            return None;
        }
        let prev = match self.focused_index() {
            Some(curr) => (curr + self.controls.len() - 1) % self.controls.len(),
            None => self.controls.len() - 1,
        };
        self.set_focused_index(Some(prev));
        Some(prev)
    }

    /// Clear focus.
    pub fn clear_focus(&mut self) {
        self.focused = None;
    }

    /// Find the control index by its string ID.
    pub fn find_by_id(&self, id: &str) -> Option<usize> {
        self.control_ids
            .iter()
            .position(|opt| opt.as_deref() == Some(id))
    }

    /// Hit test an absolute coordinate against all registered controls.
    pub fn hit_test(&self, pos: [f32; 2]) -> Option<ControlHit> {
        for (i, ctrl) in self.controls.iter().enumerate().rev() {
            if let Some((part, norm_frac)) = ctrl.hit_test(pos) {
                let b = ctrl.bounds();
                return Some(ControlHit {
                    control_index: i,
                    part,
                    hit_pos: pos,
                    local_pos: [pos[0] - b.x, pos[1] - b.y],
                    normalized_fraction: norm_frac,
                });
            }
        }
        None
    }

    /// Deliver a unified input event.
    #[must_use]
    pub fn active_drag(&self) -> Option<DragGrab> {
        self.active_drag.clone()
    }

    /// Put a drag back in progress after a rebuild.
    pub fn set_active_drag(&mut self, grab: Option<DragGrab>) {
        self.active_drag = grab.filter(|grab| self.resolve(&grab.key).is_some());
    }

    pub fn deliver_event(&mut self, event: InputEvent) -> EventResponse {
        match event {
            InputEvent::Mouse(m) => self.deliver_mouse(m),
            InputEvent::Keyboard(k) => self.deliver_keyboard(k),
        }
    }

    /// Tell the controls where the pointer is.
    fn update_hover(&self, pos: [f32; 2]) {
        let hit = self.hit_test(pos).map(|h| (h.control_index, h.part));
        for (index, control) in self.controls.iter().enumerate() {
            match hit {
                Some((hit_index, part)) if hit_index == index => control.set_hover(Some(part)),
                _ => control.set_hover(None),
            }
        }
    }

    /// Deliver an absolute mouse event to the registered controls.
    pub fn deliver_mouse(&mut self, event: MouseEvent) -> EventResponse {
        if self.active_drag.is_none() {
            self.update_hover(event.pos());
        }

        if let Some(drag) = self.active_drag.clone() {
            match self.resolve(&drag.key) {
                None => self.active_drag = None,
                Some(idx) => match event {
                    MouseEvent::Up { button, .. } if button == drag.button => {
                        self.active_drag = None;
                        let focused = self.focused_index();
                        let id = self.control_ids[idx].clone();
                        let ctrl = &self.controls[idx];
                        let hit_part = ctrl
                            .hit_test(event.pos())
                            .map(|(p, _)| p)
                            .unwrap_or(ControlPart::Body);
                        let action = ctrl.handle_mouse(&event, hit_part);
                        return EventResponse::handled(idx, id, hit_part, action, focused);
                    }
                    MouseEvent::Move { pos }
                    | MouseEvent::Drag {
                        current_pos: pos, ..
                    } => {
                        let focused = self.focused_index();
                        let id = self.control_ids[idx].clone();
                        let ctrl = &self.controls[idx];
                        let hit_part = ctrl
                            .hit_test(pos)
                            .map(|(p, _)| p)
                            .unwrap_or(ControlPart::Body);
                        let action = ctrl.handle_mouse(&event, hit_part);
                        return EventResponse::handled(idx, id, hit_part, action, focused);
                    }
                    _ => {}
                },
            }
        }

        if matches!(event, MouseEvent::Move { .. }) {
            return EventResponse::ignored(self.focused_index());
        }

        let pos = event.pos();
        if let Some(hit) = self.hit_test(pos) {
            let idx = hit.control_index;
            let id = self.control_ids[idx].clone();

            if matches!(event, MouseEvent::Down { .. } | MouseEvent::Click { .. }) {
                self.set_focused_index(Some(idx));
            }

            if let MouseEvent::Down { button, pos } = event {
                self.active_drag = self.control_key(idx).map(|key| DragGrab {
                    key,
                    start_pos: pos,
                    button,
                });
            }

            let focused = self.focused_index();
            let action = self.controls[idx].handle_mouse(&event, hit.part);
            EventResponse::handled(idx, id, hit.part, action, focused)
        } else {
            if matches!(event, MouseEvent::Down { .. } | MouseEvent::Click { .. }) {
                self.focused = None;
            }
            EventResponse::ignored(self.focused_index())
        }
    }

    /// Deliver a keyboard event to the currently focused control.
    pub fn deliver_keyboard(&mut self, event: KeyboardEvent) -> EventResponse {
        if event.is_down() {
            match event.key() {
                Some(Key::Tab) => {
                    let new_focus = self.focus_next();
                    let id = new_focus.and_then(|idx| self.control_ids[idx].clone());
                    return EventResponse {
                        handled: true,
                        target_index: new_focus,
                        target_id: id,
                        target_part: Some(ControlPart::Body),
                        action: None,
                        focused_index: new_focus,
                    };
                }
                Some(Key::BackTab) => {
                    let new_focus = self.focus_prev();
                    let id = new_focus.and_then(|idx| self.control_ids[idx].clone());
                    return EventResponse {
                        handled: true,
                        target_index: new_focus,
                        target_id: id,
                        target_part: Some(ControlPart::Body),
                        action: None,
                        focused_index: new_focus,
                    };
                }
                Some(Key::Escape) => {
                    self.clear_focus();
                    return EventResponse {
                        handled: true,
                        target_index: None,
                        target_id: None,
                        target_part: None,
                        action: None,
                        focused_index: None,
                    };
                }
                _ => {}
            }
        }

        if let Some(idx) = self.focused_index() {
            let id = self.control_ids[idx].clone();
            let action = self.controls[idx].handle_keyboard(&event);
            return EventResponse::handled(idx, id, ControlPart::Body, action, Some(idx));
        }

        EventResponse::ignored(self.focused_index())
    }
}

impl DrawControl for ControlHost<'_> {
    fn draw(&self, list: &mut libmsdf::drawlist::DrawList, ctx: &DrawContext) {
        for control in &self.controls {
            control.draw(list, ctx);
        }
        for control in &self.controls {
            control.draw_overlay(list, ctx);
        }
    }
}
