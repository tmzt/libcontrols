//! `ControlHost` — Input event dispatcher and focus manager for UI controls.

use crate::event::{
    ControlAction, ControlHit, ControlPart, EventContext, EventResponse, FilterContext, Filtered,
    FocusEvent, GestureEvent, InputEvent, Key, KeyboardEvent, Modifiers, MouseButton, MouseEvent,
};
use crate::handle::ControlHandle;
use crate::rect::Rect;
use crate::symbol::Symbol;
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

    /// Whether Tab and presses may give this control the focus.
    fn accepts_focus(&self) -> bool {
        true
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
    fn accepts_focus(&self) -> bool {
        self.control().accepts_focus()
    }
}

/// Sees every event a host is given before it is routed. App-owned and
/// persistent: created once and borrowed by each host build. A control is
/// never registered as a filter itself; a feature that needs one owns a small
/// filter object that routes to its control by name.
pub trait InputFilter {
    /// Answer for one event: pass it on, spend it, or send it to one control.
    fn filter(&self, event: &InputEvent, ctx: &FilterContext) -> Filtered;
}

/// Builder for constructing a [`ControlHost`].
#[derive(Default)]
pub struct ControlHostBuilder<'a> {
    controls: Vec<Box<dyn Control + 'a>>,
    control_ids: Vec<Option<Symbol>>,
    filters: Vec<&'a dyn InputFilter>,
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
        id: impl Into<Symbol>,
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

    /// Register a filter. The last added is asked first.
    pub fn with_filter(mut self, filter: &'a dyn InputFilter) -> Self {
        self.filters.push(filter);
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
            filters: self.filters,
            focused: None,
            hovered: None,
            active_drag: None,
            pointer: None,
            modifiers: Modifiers::default(),
        };
        host.set_focused_index(self.initial_focus);
        host
    }
}

/// A lasting way of naming one control, for state that outlives a rebuild.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum ControlKey {
    /// A control registered under a name, found again by that name.
    Named(Symbol),
    /// A control registered without one, which is only its place in the list.
    Slot(usize),
}

impl ControlKey {
    /// A key for a control registered under `id`.
    #[must_use]
    pub fn named(id: impl Into<Symbol>) -> Self {
        Self::Named(id.into())
    }

    /// A key for a control that has no name, which can only be its place.
    #[must_use]
    pub fn at(index: usize) -> Self {
        Self::Slot(index)
    }

    /// The control's name, if it has one.
    #[must_use]
    pub fn id(self) -> Option<Symbol> {
        match self {
            Self::Named(id) => Some(id),
            Self::Slot(_) => None,
        }
    }
}

/// A drag in progress, in a form that can be carried across a rebuild.
#[derive(Copy, Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct DragGrab {
    /// Which control is being dragged.
    pub key: ControlKey,
    /// Where the press that started it landed.
    pub start_pos: [f32; 2],
    /// The button holding it.
    pub button: MouseButton,
    /// The modifiers held at the press.
    pub press_modifiers: Modifiers,
}

impl DragGrab {
    /// A grab of `key` by `button`, pressed at `start_pos` with no modifiers held.
    #[must_use]
    pub fn new(key: ControlKey, start_pos: [f32; 2], button: MouseButton) -> Self {
        Self {
            key,
            start_pos,
            button,
            press_modifiers: Modifiers::default(),
        }
    }

    /// The same grab, pressed with these modifiers held.
    #[must_use]
    pub fn with_press_modifiers(mut self, modifiers: Modifiers) -> Self {
        self.press_modifiers = modifiers;
        self
    }
}

/// A key resolved to the control's place, once, when it was set. Controls are
/// only ever appended, so the place stays right for the host's life.
#[derive(Copy, Clone, Debug)]
struct Slot {
    key: ControlKey,
    index: usize,
}

/// The control under the pointer and the part it was last told about;
/// `part` is `None` after a rebuild, so the part is sent again once.
#[derive(Copy, Clone, Debug)]
struct Hovered {
    key: ControlKey,
    index: usize,
    part: Option<ControlPart>,
}

/// Host for managing absolute input event routing and focus across registered controls.
pub struct ControlHost<'a> {
    controls: Vec<Box<dyn Control + 'a>>,
    control_ids: Vec<Option<Symbol>>,
    filters: Vec<&'a dyn InputFilter>,
    focused: Option<Slot>,
    hovered: Option<Hovered>,
    active_drag: Option<(DragGrab, usize)>,
    pointer: Option<[f32; 2]>,
    modifiers: Modifiers,
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
    pub fn add_named_control<C: Control + 'a>(&mut self, id: impl Into<Symbol>, control: C) {
        self.controls.push(Box::new(control));
        self.control_ids.push(Some(id.into()));
    }

    /// A key naming the control at `index`, if there is one there.
    #[must_use]
    pub fn control_key(&self, index: usize) -> Option<ControlKey> {
        match self.control_ids.get(index)? {
            Some(id) => Some(ControlKey::Named(*id)),
            None => Some(ControlKey::Slot(index)),
        }
    }

    /// Where the control a key names sits now, if it is still here.
    #[must_use]
    pub fn resolve(&self, key: &ControlKey) -> Option<usize> {
        match key {
            ControlKey::Named(id) => self.find_by_id(*id),
            ControlKey::Slot(index) => (*index < self.controls.len()).then_some(*index),
        }
    }

    /// Currently focused control index.
    pub fn focused_index(&self) -> Option<usize> {
        self.focused.map(|slot| slot.index)
    }

    /// The focused control as a key, to be carried to the next build.
    #[must_use]
    pub fn focused_key(&self) -> Option<ControlKey> {
        self.focused.map(|slot| slot.key)
    }

    /// Put the focus back on a named control after a rebuild. The control is
    /// told `Gained`; a key that no longer resolves leaves the focus `None`.
    pub fn set_focused_key(&mut self, key: Option<ControlKey>) {
        let index = key.and_then(|key| self.resolve(&key));
        self.change_focus(index);
    }

    /// Set the focused control index.
    pub fn set_focused_index(&mut self, index: Option<usize>) {
        self.change_focus(index.filter(|&idx| idx < self.controls.len()));
    }

    /// Move the focus to `index`, telling the control that had it `Lost` and
    /// the one that gets it `Gained`. Setting the focus it has sends nothing.
    fn change_focus(&mut self, index: Option<usize>) {
        let old = self.focused_index();
        if old == index {
            return;
        }
        self.focused =
            index.and_then(|idx| self.control_key(idx).map(|key| Slot { key, index: idx }));
        if let Some(old) = old {
            self.controls[old].handle_focus(FocusEvent::Lost);
        }
        if let Some(new) = self.focused_index() {
            self.controls[new].handle_focus(FocusEvent::Gained);
        }
    }

    /// The first control after (or before) the focused one that accepts the
    /// focus, wrapping around; `None` when every control refuses.
    fn next_focus_stop(&self, forward: bool) -> Option<usize> {
        let n = self.controls.len();
        let start = self.focused_index();
        (1..=n)
            .map(|step| match (start, forward) {
                (Some(curr), true) => (curr + step) % n,
                (Some(curr), false) => (curr + n - step % n) % n,
                (None, true) => step - 1,
                (None, false) => n - step,
            })
            .find(|&idx| self.controls[idx].accepts_focus())
    }

    /// Move focus to the next control that accepts it.
    pub fn focus_next(&mut self) -> Option<usize> {
        let next = self.next_focus_stop(true);
        self.change_focus(next);
        next
    }

    /// Move focus to the previous control that accepts it.
    pub fn focus_prev(&mut self) -> Option<usize> {
        let prev = self.next_focus_stop(false);
        self.change_focus(prev);
        prev
    }

    /// Clear focus.
    pub fn clear_focus(&mut self) {
        self.change_focus(None);
    }

    /// Find the control index by its name.
    pub fn find_by_id(&self, id: impl Into<Symbol>) -> Option<usize> {
        let id = id.into();
        self.control_ids.iter().position(|opt| *opt == Some(id))
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

    /// The drag in progress, to be carried to the next build.
    #[must_use]
    pub fn active_drag(&self) -> Option<DragGrab> {
        self.active_drag.map(|(grab, _)| grab)
    }

    /// Put a drag back in progress after a rebuild.
    pub fn set_active_drag(&mut self, grab: Option<DragGrab>) {
        self.active_drag = grab.and_then(|grab| self.resolve(&grab.key).map(|idx| (grab, idx)));
    }

    /// Deliver a unified input event.
    pub fn deliver_event(&mut self, event: InputEvent) -> EventResponse {
        match event {
            InputEvent::Mouse(m) => self.deliver_mouse(m),
            InputEvent::Keyboard(k) => self.deliver_keyboard(k),
            InputEvent::Gesture(g) => self.deliver_gesture(g),
            InputEvent::Modifiers(m) => self.deliver_modifiers(m),
        }
    }

    /// Last known pointer position, from the most recent mouse event.
    #[must_use]
    pub fn pointer(&self) -> Option<[f32; 2]> {
        self.pointer
    }

    /// Modifier keys held, as last reported.
    #[must_use]
    pub fn modifiers(&self) -> Modifiers {
        self.modifiers
    }

    /// Deliver a gesture to the control under the last known pointer.
    pub fn deliver_gesture(&mut self, event: GestureEvent) -> EventResponse {
        if let Some(resp) = self.run_filters(InputEvent::Gesture(event)) {
            return resp;
        }
        let focused = self.focused_index();
        let Some(hit) = self.pointer.and_then(|pos| self.hit_test(pos)) else {
            return EventResponse::ignored(focused);
        };
        let idx = hit.control_index;
        let id = self.control_ids[idx];
        let ctx = self.event_context(idx, hit.part);
        let action = self.controls[idx].handle_gesture(&event, &ctx);
        EventResponse::handled(idx, id, hit.part, action, focused)
    }

    /// Record the modifier keys now held. No control is told: they arrive
    /// with each later event in its [`EventContext`].
    pub fn deliver_modifiers(&mut self, modifiers: Modifiers) -> EventResponse {
        self.modifiers = modifiers;
        if let Some(resp) = self.run_filters(InputEvent::Modifiers(modifiers)) {
            return resp;
        }
        EventResponse::ignored(self.focused_index())
    }

    /// Record the modifier keys held without telling anything: for a
    /// container seeding its child host from an [`EventContext`], or an app
    /// seeding a rebuilt host from the previous one's `modifiers()`.
    pub fn set_modifiers(&mut self, modifiers: Modifiers) {
        self.modifiers = modifiers;
    }

    /// Ask the filters, last registered first. `None` when every one passes
    /// (or none is registered) and the event is to be routed as usual.
    fn run_filters(&mut self, event: InputEvent) -> Option<EventResponse> {
        if self.filters.is_empty() {
            return None;
        }
        let ctx = FilterContext {
            pointer: self.pointer,
            modifiers: self.modifiers,
            focused: self.focused_key(),
            grab: self.active_drag.map(|(grab, _)| grab.key),
        };
        for i in (0..self.filters.len()).rev() {
            let filter = self.filters[i];
            match filter.filter(&event, &ctx) {
                Filtered::Pass => {}
                Filtered::Consumed(action) => {
                    return Some(EventResponse::consumed(action, self.focused_index()));
                }
                Filtered::Redirect(key) => return Some(self.deliver_redirected(key, event)),
            }
        }
        None
    }

    /// Deliver `event` to the control `key` names and to nothing else: no
    /// focus change, no grab, no Tab or Escape handling. A key that names no
    /// control, and any modifier change, are consumed.
    fn deliver_redirected(&mut self, key: ControlKey, event: InputEvent) -> EventResponse {
        let focused = self.focused_index();
        let Some(idx) = self.resolve(&key) else {
            return EventResponse::consumed(None, focused);
        };
        if self.active_drag.is_some_and(|(_, grabbed)| grabbed != idx) {
            self.active_drag = None;
        }
        let id = self.control_ids[idx];
        let part_at = |host: &Self, pos: Option<[f32; 2]>| {
            pos.and_then(|pos| host.controls[idx].hit_test(pos))
                .map(|(part, _)| part)
        };
        match event {
            InputEvent::Mouse(mouse) => {
                if self.hovered.is_some_and(|h| h.index != idx) {
                    self.clear_hover();
                }
                let hit = part_at(self, Some(mouse.pos()));
                self.update_hover(hit.map(|part| (idx, part)));
                let part = hit.unwrap_or(ControlPart::Body);
                let ctx = self.event_context(idx, part);
                let action = self.controls[idx].handle_mouse(&mouse, &ctx);
                EventResponse::handled(idx, id, part, action, focused)
            }
            InputEvent::Keyboard(key_event) => {
                let ctx = self.event_context(idx, ControlPart::Body);
                let action = self.controls[idx].handle_keyboard(&key_event, &ctx);
                EventResponse::handled(idx, id, ControlPart::Body, action, focused)
            }
            InputEvent::Gesture(gesture) => {
                let part = part_at(self, self.pointer).unwrap_or(ControlPart::Body);
                let ctx = self.event_context(idx, part);
                let action = self.controls[idx].handle_gesture(&gesture, &ctx);
                EventResponse::handled(idx, id, part, action, focused)
            }
            InputEvent::Modifiers(_) => EventResponse::consumed(None, focused),
        }
    }

    /// The context handed to the control at `idx` for an event on `part`.
    fn event_context(&self, idx: usize, part: ControlPart) -> EventContext {
        let press = self
            .active_drag
            .filter(|&(_, grabbed)| grabbed == idx)
            .map(|(grab, _)| grab.press_modifiers);
        EventContext::new(part)
            .with_modifiers(self.modifiers)
            .with_press_modifiers(press)
    }

    /// The hovered control as a key, to be carried to the next build.
    #[must_use]
    pub fn hovered_key(&self) -> Option<ControlKey> {
        self.hovered.map(|h| h.key)
    }

    /// Put the hover back on the control it was on before this rebuild.
    /// Tells no control anything; the part is sent again on the next
    /// pointer event. A key that no longer resolves is dropped.
    pub fn set_hovered_key(&mut self, key: Option<ControlKey>) {
        self.hovered = key.and_then(|key| {
            self.resolve(&key).map(|index| Hovered {
                key,
                index,
                part: None,
            })
        });
    }

    /// Tell the hovered control, if any, that the pointer left, and forget it.
    pub fn clear_hover(&mut self) {
        if let Some(old) = self.hovered.take() {
            self.controls[old.index].set_hover(None);
        }
    }

    /// Move the hover to `hit`, telling only the control left and the
    /// control entered, or the same control when its part changed.
    fn update_hover(&mut self, hit: Option<(usize, ControlPart)>) {
        match (self.hovered, hit) {
            (Some(old), Some((index, part))) if old.index == index => {
                if old.part != Some(part) {
                    self.controls[index].set_hover(Some(part));
                }
            }
            (Some(old), _) => self.controls[old.index].set_hover(None),
            (None, _) => {}
        }
        if let Some((index, part)) = hit
            && self.hovered.is_none_or(|old| old.index != index)
        {
            self.controls[index].set_hover(Some(part));
        }
        self.hovered = hit.and_then(|(index, part)| {
            self.control_key(index).map(|key| Hovered {
                key,
                index,
                part: Some(part),
            })
        });
    }

    /// Deliver an absolute mouse event to the registered controls.
    pub fn deliver_mouse(&mut self, event: MouseEvent) -> EventResponse {
        self.pointer = Some(event.pos());
        if let Some(resp) = self.run_filters(InputEvent::Mouse(event)) {
            return resp;
        }
        // One hit test per event, shared by hover and routing; while a grab
        // stands hover is frozen and the test is made only if routing needs it.
        let mut hit = None;
        if self.active_drag.is_none() {
            let found = self.hit_test(event.pos());
            self.update_hover(found.map(|h| (h.control_index, h.part)));
            hit = Some(found);
        }

        if let Some((drag, idx)) = self.active_drag {
            let grabbed = match event {
                MouseEvent::Up { button, .. } if button == drag.button => {
                    self.active_drag = None;
                    true
                }
                MouseEvent::Move { .. } | MouseEvent::Drag { .. } => true,
                _ => false,
            };
            if grabbed {
                let focused = self.focused_index();
                let id = self.control_ids[idx];
                let ctrl = &self.controls[idx];
                let hit_part = ctrl
                    .hit_test(event.pos())
                    .map(|(p, _)| p)
                    .unwrap_or(ControlPart::Body);
                let ctx = EventContext::new(hit_part)
                    .with_modifiers(self.modifiers)
                    .with_press_modifiers(Some(drag.press_modifiers));
                let action = ctrl.handle_mouse(&event, &ctx);
                return EventResponse::handled(idx, id, hit_part, action, focused);
            }
        }

        if matches!(event, MouseEvent::Move { .. }) {
            return EventResponse::ignored(self.focused_index());
        }

        let pos = event.pos();
        let presses = matches!(event, MouseEvent::Down { .. } | MouseEvent::Click { .. });
        if let Some(hit) = hit.unwrap_or_else(|| self.hit_test(pos)) {
            let idx = hit.control_index;
            let id = self.control_ids[idx];

            if presses && self.controls[idx].accepts_focus() {
                self.change_focus(Some(idx));
            }

            if let MouseEvent::Down { button, pos } = event {
                let modifiers = self.modifiers;
                self.active_drag = self.control_key(idx).map(|key| {
                    (
                        DragGrab::new(key, pos, button).with_press_modifiers(modifiers),
                        idx,
                    )
                });
            }

            let focused = self.focused_index();
            let ctx = self.event_context(idx, hit.part);
            let action = self.controls[idx].handle_mouse(&event, &ctx);
            EventResponse::handled(idx, id, hit.part, action, focused)
        } else {
            if presses {
                self.change_focus(None);
            }
            EventResponse::ignored(self.focused_index())
        }
    }

    /// Deliver a keyboard event to the currently focused control. Tab,
    /// BackTab and Escape belong to the host, pressed and released.
    pub fn deliver_keyboard(&mut self, event: KeyboardEvent) -> EventResponse {
        if let Some(resp) = self.run_filters(InputEvent::Keyboard(event)) {
            return resp;
        }
        if let Some(key @ (Key::Tab | Key::BackTab | Key::Escape)) = event.key() {
            if !event.is_down() {
                return EventResponse::ignored(self.focused_index());
            }
            if key == Key::Escape {
                self.clear_focus();
                return EventResponse::consumed(None, None);
            }
            let new_focus = if key == Key::Tab {
                self.focus_next()
            } else {
                self.focus_prev()
            };
            let id = new_focus.and_then(|idx| self.control_ids[idx]);
            return EventResponse {
                handled: true,
                target_index: new_focus,
                target_id: id,
                target_part: Some(ControlPart::Body),
                action: None,
                focused_index: new_focus,
            };
        }

        if let Some(idx) = self.focused_index() {
            let id = self.control_ids[idx];
            let ctx = self.event_context(idx, ControlPart::Body);
            let action = self.controls[idx].handle_keyboard(&event, &ctx);
            return EventResponse::handled(idx, id, ControlPart::Body, action, Some(idx));
        }

        EventResponse::ignored(self.focused_index())
    }
}

impl ControlHost<'_> {
    /// The first draw pass only: every control's `draw`.
    pub fn draw_controls(&self, list: &mut libmsdf::drawlist::DrawList, ctx: &DrawContext) {
        for control in &self.controls {
            control.draw(list, ctx);
        }
    }

    /// The second draw pass only: every control's `draw_overlay`.
    pub fn draw_control_overlays(&self, list: &mut libmsdf::drawlist::DrawList, ctx: &DrawContext) {
        for control in &self.controls {
            control.draw_overlay(list, ctx);
        }
    }
}

impl DrawControl for ControlHost<'_> {
    fn draw(&self, list: &mut libmsdf::drawlist::DrawList, ctx: &DrawContext) {
        self.draw_controls(list, ctx);
        self.draw_control_overlays(list, ctx);
    }
}
