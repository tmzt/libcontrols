//! A control that records every call the host makes on it.

#![allow(dead_code)]

use std::cell::{Cell, RefCell};

use libcontrols::{
    ControlAction, ControlPart, DrawContext, DrawControl, EventContext, FocusEvent, GestureEvent,
    HostedControl, KeyboardEvent, MouseEvent, Rect,
};
use libmsdf::drawlist::DrawList;

thread_local! {
    /// Every focus event any recorder on this thread received, in order, by
    /// the recorder's left edge.
    static FOCUS_JOURNAL: RefCell<Vec<(f32, FocusEvent)>> = const { RefCell::new(Vec::new()) };
}

/// The focus events received on this thread since the last call, in the
/// order they happened, as `(left edge, event)`; empties the journal.
pub fn take_focus_journal() -> Vec<(f32, FocusEvent)> {
    FOCUS_JOURNAL.with(|j| std::mem::take(&mut *j.borrow_mut()))
}

/// One call the host made on a [`Recorder`].
#[derive(Clone, Debug, PartialEq)]
pub enum Call {
    Mouse(MouseEvent, EventContext),
    Key(KeyboardEvent, EventContext),
    Gesture(GestureEvent, EventContext),
    Hover(Option<ControlPart>),
    Focus(FocusEvent),
}

/// A square control that logs every handler call, optionally refusing focus.
pub struct Recorder {
    pub rect: Rect,
    pub calls: RefCell<Vec<Call>>,
    pub refuses_focus: Cell<bool>,
    /// Split the square into a left (`Custom(0)`) and right (`Custom(1)`) part.
    pub halves: bool,
}

impl Recorder {
    pub fn at(x: f32) -> Self {
        Self {
            rect: Rect::new(x, 0.0, 100.0, 100.0),
            calls: RefCell::new(Vec::new()),
            refuses_focus: Cell::new(false),
            halves: false,
        }
    }

    pub fn halved(x: f32) -> Self {
        Self {
            halves: true,
            ..Self::at(x)
        }
    }

    /// Every call so far, emptying the log.
    pub fn take(&self) -> Vec<Call> {
        std::mem::take(&mut *self.calls.borrow_mut())
    }

    /// The contexts of the mouse calls so far, emptying the log.
    pub fn mouse_contexts(&self) -> Vec<EventContext> {
        self.take()
            .into_iter()
            .filter_map(|c| match c {
                Call::Mouse(_, ctx) => Some(ctx),
                _ => None,
            })
            .collect()
    }

    /// Only the focus events so far, emptying the log.
    pub fn focus(&self) -> Vec<FocusEvent> {
        self.take()
            .into_iter()
            .filter_map(|c| match c {
                Call::Focus(f) => Some(f),
                _ => None,
            })
            .collect()
    }

    /// Only the hover calls so far, emptying the log.
    pub fn hovers(&self) -> Vec<Option<ControlPart>> {
        self.take()
            .into_iter()
            .filter_map(|c| match c {
                Call::Hover(h) => Some(h),
                _ => None,
            })
            .collect()
    }
}

impl DrawControl for Recorder {
    fn draw(&self, _list: &mut DrawList, _ctx: &DrawContext) {}
}

impl HostedControl for Recorder {
    fn bounds(&self) -> Rect {
        self.rect
    }

    fn hit_test(&self, pos: [f32; 2]) -> Option<(ControlPart, f32)> {
        if !self.rect.contains(pos) {
            return None;
        }
        let part = if !self.halves {
            ControlPart::Body
        } else if pos[0] < self.rect.x + self.rect.w / 2.0 {
            ControlPart::Custom(0)
        } else {
            ControlPart::Custom(1)
        };
        Some((part, 0.0))
    }

    fn handle_mouse(&self, event: &MouseEvent, ctx: &EventContext) -> Option<ControlAction> {
        self.calls.borrow_mut().push(Call::Mouse(*event, *ctx));
        None
    }

    fn handle_keyboard(&self, event: &KeyboardEvent, ctx: &EventContext) -> Option<ControlAction> {
        self.calls.borrow_mut().push(Call::Key(*event, *ctx));
        None
    }

    fn handle_gesture(&self, event: &GestureEvent, ctx: &EventContext) -> Option<ControlAction> {
        self.calls.borrow_mut().push(Call::Gesture(*event, *ctx));
        None
    }

    fn set_hover(&self, hover: Option<ControlPart>) {
        self.calls.borrow_mut().push(Call::Hover(hover));
    }

    fn handle_focus(&self, event: FocusEvent) {
        FOCUS_JOURNAL.with(|j| j.borrow_mut().push((self.rect.x, event)));
        self.calls.borrow_mut().push(Call::Focus(event));
    }

    fn accepts_focus(&self) -> bool {
        !self.refuses_focus.get()
    }
}
