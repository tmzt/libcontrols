//! Input event primitives, hit testing results, and control response actions.

use crate::flags::{NodeFlags, NodeState};
use crate::modal::LayerView;
use crate::router::NodeId;

/// Mouse button enumeration.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Other(u16),
}

/// Absolute mouse event representation.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum MouseEvent {
    /// Cursor moved to absolute position `[x, y]`.
    Move { pos: [f32; 2] },
    /// Mouse button pressed down at absolute position `[x, y]`.
    Down { pos: [f32; 2], button: MouseButton },
    /// Mouse button released at absolute position `[x, y]`.
    Up { pos: [f32; 2], button: MouseButton },
    /// Complete click event at absolute position `[x, y]`.
    Click { pos: [f32; 2], button: MouseButton },
    /// Mouse wheel scroll at absolute position `[x, y]` with `[dx, dy]` scroll deltas.
    Scroll { pos: [f32; 2], delta: [f32; 2] },
    /// Pixel-precise scroll (a trackpad) at `[x, y]` with `[dx, dy]` in pixels.
    ScrollPixels { pos: [f32; 2], delta: [f32; 2] },
    /// Drag motion from `start_pos` to `current_pos` with step `delta`.
    Drag {
        start_pos: [f32; 2],
        current_pos: [f32; 2],
        delta: [f32; 2],
        button: MouseButton,
    },
}

impl MouseEvent {
    /// Cursor position `[x, y]` associated with this mouse event.
    #[inline]
    pub fn pos(&self) -> [f32; 2] {
        match *self {
            Self::Move { pos } => pos,
            Self::Down { pos, .. } => pos,
            Self::Up { pos, .. } => pos,
            Self::Click { pos, .. } => pos,
            Self::Scroll { pos, .. } => pos,
            Self::ScrollPixels { pos, .. } => pos,
            Self::Drag { current_pos, .. } => current_pos,
        }
    }

    /// Mouse button involved in this event, if any.
    #[inline]
    pub fn button(&self) -> Option<MouseButton> {
        match *self {
            Self::Down { button, .. }
            | Self::Up { button, .. }
            | Self::Click { button, .. }
            | Self::Drag { button, .. } => Some(button),
            Self::Move { .. } | Self::Scroll { .. } | Self::ScrollPixels { .. } => None,
        }
    }
}

/// Virtual keyboard key enumeration.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Key {
    Left,
    Right,
    Up,
    Down,
    Tab,
    BackTab,
    Enter,
    Escape,
    /// The platform back action (Android Back, a browser's back).
    Back,
    Space,
    Home,
    End,
    PageUp,
    PageDown,
    Backspace,
    Delete,
    Char(char),
    Other(u32),
}

/// Keyboard key transition state.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum KeyState {
    Pressed,
    Released,
}

/// Keyboard event representation.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum KeyboardEvent {
    /// Key state transition.
    Key { key: Key, state: KeyState },
    /// Convenience key press event.
    KeyDown { key: Key },
    /// Convenience key release event.
    KeyUp { key: Key },
    /// Character text input.
    Char { ch: char },
}

impl KeyboardEvent {
    /// Check if this event represents a key press / down.
    #[inline]
    pub fn is_down(&self) -> bool {
        match self {
            Self::Key { state, .. } => *state == KeyState::Pressed,
            Self::KeyDown { .. } => true,
            Self::KeyUp { .. } => false,
            Self::Char { .. } => true,
        }
    }

    /// Extract the associated key, if available.
    #[inline]
    pub fn key(&self) -> Option<Key> {
        match *self {
            Self::Key { key, .. } | Self::KeyDown { key } | Self::KeyUp { key } => Some(key),
            Self::Char { ch } => Some(Key::Char(ch)),
        }
    }
}

/// Touch or trackpad gesture, routed to the control under the last known pointer.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum GestureEvent {
    /// Pinch by `delta`, where positive magnifies.
    Pinch { delta: f32 },
    /// Two-finger double tap.
    DoubleTap,
    /// Rotation by `delta` radians.
    Rotate { delta: f32 },
}

/// Modifier keys held.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Modifiers {
    pub shift: bool,
    pub alt: bool,
    pub ctrl: bool,
    pub meta: bool,
}

/// Unified input event wrapping mouse, keyboard, gesture or modifier input.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum InputEvent {
    Mouse(MouseEvent),
    Keyboard(KeyboardEvent),
    /// A gesture, which carries no position of its own.
    Gesture(GestureEvent),
    /// The modifier keys now held.
    Modifiers(Modifiers),
}

/// Sub-component part of a control.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum ControlPart {
    /// Main body or whole control.
    #[default]
    Body,
    /// Leading descriptor label.
    Label,
    /// Central value text or display region.
    Value,
    /// Left chevron arrow button (`<`).
    LeftArrow,
    /// Right chevron arrow button (`>`).
    RightArrow,
    /// Continuous slider track.
    Track,
    /// Draggable slider thumb notch/handle.
    Thumb,
    /// Numeric value readout box.
    NumericBox,
    /// Custom sub-part identifier.
    Custom(u32),
}

/// What the host knows about an event when it hands it to a control.
///
/// Outside this crate, build one with [`EventContext::new`] and the `with_*`
/// methods, or start from `EventContext::default()` and assign fields.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct EventContext {
    /// The part of the control the event is for. Body for keys.
    pub part: ControlPart,
    /// The modifier keys held at this event. A Shift pressed mid-drag shows
    /// here on the next Drag.
    pub modifiers: Modifiers,
    /// The modifiers held at the press that started the grab this control
    /// holds; None when this control holds no grab.
    pub press_modifiers: Option<Modifiers>,
    /// The declared flags of the node handling the event (set by the router).
    pub flags: NodeFlags,
    /// What the router knows about the node handling the event.
    pub state: NodeState,
    /// Where the modal handling the event is on its stack (set by the stack).
    pub layer: Option<LayerView>,
}

impl EventContext {
    /// A context for `part`, with no modifiers held and no grab.
    #[must_use]
    pub fn new(part: ControlPart) -> Self {
        Self {
            part,
            ..Self::default()
        }
    }

    /// The same context for another part, for controls made of controls.
    #[must_use]
    pub fn with_part(mut self, part: ControlPart) -> Self {
        self.part = part;
        self
    }

    /// The same context with these modifiers held.
    #[must_use]
    pub fn with_modifiers(mut self, modifiers: Modifiers) -> Self {
        self.modifiers = modifiers;
        self
    }

    /// The same context with these modifiers held at the grab's press.
    #[must_use]
    pub fn with_press_modifiers(mut self, press: Option<Modifiers>) -> Self {
        self.press_modifiers = press;
        self
    }
}

/// The host's focus arrived at this control or left it.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum FocusEvent {
    /// The control now has the focus.
    Gained,
    /// The control no longer has the focus.
    Lost,
}

/// Action produced by a control in response to an input event.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum ControlAction {
    /// Control or sub-component was activated / clicked.
    Activated { part: ControlPart },
    /// Discrete step left / previous item.
    StepLeft,
    /// Discrete step right / next item.
    StepRight,
    /// Continuous numerical value change.
    ValueChange {
        /// Calculated new value within control range.
        new_value: f32,
        /// Normalized fraction in `[0.0, 1.0]`.
        fraction: f32,
    },
    /// Text character input received.
    TextInput { ch: char },
    /// Custom action message.
    Custom(String),
    /// A modal asks to be dismissed: for one of a dialog's close controls,
    /// or (`None`) for itself. Outside a modal stack it is returned to the
    /// app like any action.
    Dismiss { control: Option<NodeId> },
}
