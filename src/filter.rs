//! The global filter chain: app-wide rules every event passes through
//! before a router routes it.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use crate::event::{ControlAction, InputEvent, Modifiers};
use crate::router::NodeId;

/// A filter's answer for one event.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Filtered {
    /// Not mine: ask the next rule, then route as usual.
    Pass,
    /// Spent: no node sees it. The action, if any, is returned to the app.
    Consumed(Option<ControlAction>),
    /// Deliver it to this node alone, through its normal handlers.
    Redirect(NodeId),
}

/// What the router knows when it asks a filter. Nothing per node.
///
/// Outside this crate, start from `FilterContext::default()` and assign fields.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct FilterContext {
    /// The pointer, after recording this event.
    pub pointer: Option<[f32; 2]>,
    /// The modifiers held, after recording this event.
    pub modifiers: Modifiers,
    /// The focused node, if any.
    pub focused: Option<NodeId>,
    /// The node holding the press grab, if any.
    pub grab: Option<NodeId>,
}

/// A rule in a filter chain.
pub trait InputFilter {
    /// Answer for one event: pass it on, spend it, or send it to one node.
    fn filter(&self, event: &InputEvent, ctx: &FilterContext) -> Filtered;
}

#[derive(Clone)]
enum FilterEntry {
    Owned(Rc<dyn InputFilter>),
    Pushed(Weak<dyn InputFilter>),
}

/// A handle to one filter chain; clones share it. One per input surface,
/// given to the root router, which runs it before routing. Rules are asked
/// last pushed first.
#[derive(Clone, Default)]
pub struct FilterStack(Rc<Chain>);

#[derive(Default)]
struct Chain {
    entries: RefCell<Vec<FilterEntry>>,
    /// How many runs are in progress (a rule may run the chain again).
    running: Cell<usize>,
}

impl std::fmt::Debug for FilterStack {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FilterStack")
            .field("rules", &self.0.entries.borrow().len())
            .finish()
    }
}

impl FilterStack {
    /// An empty chain.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a rule the chain owns; it stays as long as the chain.
    pub fn add(&self, filter: impl InputFilter + 'static) {
        self.0
            .entries
            .borrow_mut()
            .push(FilterEntry::Owned(Rc::new(filter)));
    }

    /// Push a rule owned elsewhere. It stays in the chain while its owner
    /// keeps the `Rc`; once the last one is dropped the chain skips it and
    /// prunes it.
    pub fn push(&self, filter: &Rc<dyn InputFilter>) {
        self.0
            .entries
            .borrow_mut()
            .push(FilterEntry::Pushed(Rc::downgrade(filter)));
    }

    /// How many live rules the chain holds.
    pub fn len(&self) -> usize {
        self.0
            .entries
            .borrow()
            .iter()
            .filter(|e| match e {
                FilterEntry::Owned(_) => true,
                FilterEntry::Pushed(w) => w.strong_count() > 0,
            })
            .count()
    }

    /// Whether the chain holds no live rule.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Run the chain for one event: the rule pushed last first, stopping at
    /// the first answer that is not `Pass`. A rule may add to or push onto
    /// the chain while it runs; the new rule is asked from the next event.
    pub fn run(&self, event: &InputEvent, ctx: &FilterContext) -> Filtered {
        let mut index = self.0.entries.borrow().len();
        let mut dead = false;
        let run = Running::start(&self.0.running);
        let answer = loop {
            if index == 0 {
                break Filtered::Pass;
            }
            index -= 1;
            let entry = self.0.entries.borrow().get(index).cloned();
            let answer = match entry {
                Some(FilterEntry::Owned(rule)) => rule.filter(event, ctx),
                Some(FilterEntry::Pushed(weak)) => match weak.upgrade() {
                    Some(rule) => rule.filter(event, ctx),
                    None => {
                        dead = true;
                        Filtered::Pass
                    }
                },
                None => Filtered::Pass,
            };
            if answer != Filtered::Pass {
                break answer;
            }
        };
        let depth = run.finish();
        // Prune only when no other run is walking the chain by index.
        if dead
            && depth == 0
            && let Ok(mut entries) = self.0.entries.try_borrow_mut()
        {
            entries.retain(|e| match e {
                FilterEntry::Owned(_) => true,
                FilterEntry::Pushed(w) => w.strong_count() > 0,
            });
        }
        answer
    }
}

/// Counts a run in progress, and stops counting it even if a rule panics.
struct Running<'c> {
    count: &'c Cell<usize>,
    done: bool,
}

impl<'c> Running<'c> {
    fn start(count: &'c Cell<usize>) -> Self {
        count.set(count.get() + 1);
        Self { count, done: false }
    }

    /// End the run; how many others are still in progress.
    fn finish(mut self) -> usize {
        self.done = true;
        let left = self.count.get() - 1;
        self.count.set(left);
        left
    }
}

impl Drop for Running<'_> {
    fn drop(&mut self) {
        if !self.done {
            self.count.set(self.count.get() - 1);
        }
    }
}
