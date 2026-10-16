//! `Router` — a persistent tree of controls that routes input to them and
//! draws them each frame.

use std::any::Any;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};

use libmsdf::drawlist::DrawList;

use crate::control::Control;
use crate::event::{
    ControlAction, ControlPart, EventContext, FocusEvent, GestureEvent, InputEvent, Key,
    KeyboardEvent, Modifiers, MouseButton, MouseEvent,
};
use crate::filter::{FilterContext, FilterStack, Filtered};
use crate::flags::{NodeFlags, NodeState};
use crate::symbol::Symbol;
use crate::traits::{DrawContext, DrawControl};

/// The prefix of the aliases the library keeps for its own nodes.
pub(crate) const LIBRARY_PREFIX: &str = "libcontrols/";

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

/// A node's identity: a number from one process-wide counter, never reused,
/// so an id kept after its node was removed never names another node, in
/// this router or any other. It is also the node's name, written `@<n>`.
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(u64);

impl NodeId {
    fn next() -> Self {
        Self(NEXT_ID.fetch_add(1, Ordering::Relaxed))
    }

    /// The number.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "@{}", self.0)
    }
}

impl fmt::Debug for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

/// The text was not of the form `@<n>`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ParseNodeIdError;

impl fmt::Display for ParseNodeIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a node id is written @<number>")
    }
}

impl std::error::Error for ParseNodeIdError {}

impl FromStr for NodeId {
    type Err = ParseNodeIdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let digits = s.strip_prefix('@').ok_or(ParseNodeIdError)?;
        if digits.is_empty()
            || !digits.bytes().all(|b| b.is_ascii_digit())
            || (digits.len() > 1 && digits.starts_with('0'))
        {
            return Err(ParseNodeIdError);
        }
        digits.parse().map(Self).map_err(|_| ParseNodeIdError)
    }
}

/// Why `Router::add_named` refused an alias.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum NameError {
    /// Another node of this router already has this alias.
    Taken(NodeId),
    /// The alias starts with '@' (the id form) or with `libcontrols/` (the
    /// library's own namespace).
    Reserved,
}

impl fmt::Display for NameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Taken(id) => write!(f, "the alias is taken by {id}"),
            Self::Reserved => f.write_str("the alias is reserved"),
        }
    }
}

impl std::error::Error for NameError {}

/// A control the router can own and reach by its concrete type.
pub(crate) trait NodeControl: Control + Any {}

impl<T: Control + Any> NodeControl for T {}

pub(crate) struct Node {
    pub(crate) id: NodeId,
    pub(crate) control: Box<dyn NodeControl>,
    pub(crate) alias: Option<Symbol>,
    pub(crate) parent: Option<usize>,
    pub(crate) children: Vec<usize>,
    pub(crate) flags: NodeFlags,
    pub(crate) state: std::cell::Cell<NodeState>,
}

/// A persistent tree of controls. It owns them, routes input to them and
/// draws them; the app edits the tree when its UI changes and draws it every
/// frame.
#[derive(Default)]
pub struct Router {
    pub(crate) nodes: Vec<Option<Node>>,
    free: Vec<usize>,
    by_id: HashMap<NodeId, usize>,
    by_alias: HashMap<Symbol, usize>,
    pub(crate) roots: Vec<usize>,
    pub(crate) tab_stale: Cell<bool>,
    tab_order: RefCell<Vec<usize>>,
    filters: Option<FilterStack>,
    focus: Cell<Option<usize>>,
    hover: Cell<Option<(usize, ControlPart)>>,
    grab: Cell<Option<Grab>>,
    pointer: Cell<Option<[f32; 2]>>,
    modifiers: Cell<Modifiers>,
    /// Who took each button's press in progress: the rest of it (drags, its
    /// release, the Click after) goes only to that owner, and is consumed if
    /// another now receives input (a layer closed or opened mid-press).
    presses: RefCell<Vec<Press>>,
}

/// Who receives input at the root: the app's nodes, or a layer of the modal
/// stack (by the serial of its entry).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Owner {
    App,
    Layer(u64),
}

/// One button's press: who took it, and whether its release was seen (then
/// only the Click right after it still belongs to the press).
#[derive(Copy, Clone, Debug)]
struct Press {
    button: MouseButton,
    owner: Owner,
    released: bool,
}

/// The press grab: which node, by which button, and what was held.
#[derive(Copy, Clone, Debug)]
struct Grab {
    index: usize,
    button: MouseButton,
    start_pos: [f32; 2],
    press_modifiers: Modifiers,
}

/// A drag in progress: the node the press grabbed and how.
#[derive(Copy, Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct DragGrab {
    /// The node being dragged.
    pub node: NodeId,
    /// Where the press that started it landed.
    pub start_pos: [f32; 2],
    /// The button holding it.
    pub button: MouseButton,
    /// The modifiers held at the press.
    pub press_modifiers: Modifiers,
}

/// What a router reports for one event it was given.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct EventResponse {
    /// Whether any node, or a filter, took the event.
    pub handled: bool,
    /// The node it went to, if any.
    pub target: Option<NodeId>,
    /// The part of that node it was for.
    pub target_part: Option<ControlPart>,
    /// What the node (or a filter) answered.
    pub action: Option<ControlAction>,
    /// The focused node after the event.
    pub focused: Option<NodeId>,
}

impl EventResponse {
    /// Taken by nothing.
    #[must_use]
    pub fn ignored(focused: Option<NodeId>) -> Self {
        Self {
            handled: false,
            target: None,
            target_part: None,
            action: None,
            focused,
        }
    }

    /// Taken, naming no node: a filter spent it, or it was blocked.
    #[must_use]
    pub fn consumed(action: Option<ControlAction>, focused: Option<NodeId>) -> Self {
        Self {
            handled: true,
            target: None,
            target_part: None,
            action,
            focused,
        }
    }

    /// Taken by `target`.
    #[must_use]
    pub fn handled(
        target: NodeId,
        part: ControlPart,
        action: Option<ControlAction>,
        focused: Option<NodeId>,
    ) -> Self {
        Self {
            handled: true,
            target: Some(target),
            target_part: Some(part),
            action,
            focused,
        }
    }
}

impl fmt::Debug for Router {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Router")
            .field("nodes", &self.by_id.len())
            .field("roots", &self.roots.len())
            .finish()
    }
}

impl Router {
    /// An empty router.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add `control` under `parent` (or as a root), with no alias. Returns
    /// its id. A parent that is not in this router makes it a root.
    pub fn add(&mut self, parent: Option<NodeId>, control: impl Control + 'static) -> NodeId {
        self.insert(parent, None, Box::new(control))
    }

    /// Add `control` under `parent` with an alias unique within this router.
    pub fn add_named(
        &mut self,
        parent: Option<NodeId>,
        alias: impl Into<Symbol>,
        control: impl Control + 'static,
    ) -> Result<NodeId, NameError> {
        let alias = alias.into();
        let text = alias.as_str();
        if text.starts_with('@') || text.starts_with(LIBRARY_PREFIX) {
            return Err(NameError::Reserved);
        }
        if let Some(&index) = self.by_alias.get(&alias) {
            return Err(NameError::Taken(self.node(index).id));
        }
        Ok(self.insert(parent, Some(alias), Box::new(control)))
    }

    /// Add one of the library's own nodes as a root under a `libcontrols/`
    /// alias, kept above every app root.
    pub(crate) fn add_reserved(
        &mut self,
        alias: Symbol,
        control: Box<dyn NodeControl>,
    ) -> Result<NodeId, NameError> {
        debug_assert!(alias.as_str().starts_with(LIBRARY_PREFIX));
        if let Some(&index) = self.by_alias.get(&alias) {
            return Err(NameError::Taken(self.node(index).id));
        }
        Ok(self.insert(None, Some(alias), control))
    }

    fn insert(
        &mut self,
        parent: Option<NodeId>,
        alias: Option<Symbol>,
        control: Box<dyn NodeControl>,
    ) -> NodeId {
        let id = NodeId::next();
        let parent = parent.and_then(|p| self.index(p));
        let flags = control.default_flags();
        let node = Node {
            id,
            control,
            alias,
            parent,
            children: Vec::new(),
            flags,
            state: std::cell::Cell::new(NodeState::NONE),
        };
        let index = match self.free.pop() {
            Some(index) => {
                self.nodes[index] = Some(node);
                index
            }
            None => {
                self.nodes.push(Some(node));
                self.nodes.len() - 1
            }
        };
        self.by_id.insert(id, index);
        if let Some(alias) = alias {
            self.by_alias.insert(alias, index);
        }
        match parent {
            Some(p) => self.node_mut(p).children.push(index),
            None => {
                let library = alias.is_some_and(|a| a.as_str().starts_with(LIBRARY_PREFIX));
                if library {
                    self.roots.push(index);
                } else {
                    // App roots go before the library's, which stay on top.
                    let at = self
                        .roots
                        .iter()
                        .position(|&r| self.is_library(r))
                        .unwrap_or(self.roots.len());
                    self.roots.insert(at, index);
                }
            }
        }
        self.tab_stale.set(true);
        id
    }

    fn is_library(&self, index: usize) -> bool {
        self.node(index)
            .alias
            .is_some_and(|a| a.as_str().starts_with(LIBRARY_PREFIX))
    }

    /// Remove a node and its subtree. An id that is not here does nothing.
    pub fn remove(&mut self, id: NodeId) {
        let Some(index) = self.index(id) else {
            return;
        };
        self.before_remove(index);
        match self.node(index).parent {
            Some(p) => self.node_mut(p).children.retain(|&c| c != index),
            None => self.roots.retain(|&r| r != index),
        }
        let mut doomed = vec![index];
        while let Some(i) = doomed.pop() {
            let node = self.nodes[i].take().expect("a live node");
            doomed.extend(node.children.iter().copied());
            self.by_id.remove(&node.id);
            if let Some(alias) = node.alias {
                self.by_alias.remove(&alias);
            }
            self.free.push(i);
        }
        self.tab_stale.set(true);
    }

    /// Let the focus, hover and grab go before a subtree leaves, telling the
    /// nodes concerned (`Lost`, `set_hover(None)`) while they still exist.
    fn before_remove(&mut self, index: usize) {
        if self.focus.get().is_some_and(|f| self.is_within(f, index)) {
            self.change_focus(None);
        }
        if self
            .hover
            .get()
            .is_some_and(|(h, _)| self.is_within(h, index))
        {
            self.update_hover(&[]);
        }
        if self
            .grab
            .get()
            .is_some_and(|g| self.is_within(g.index, index))
        {
            self.set_grab(None);
        }
    }

    /// Whether `index` is `ancestor` or below it.
    fn is_within(&self, index: usize, ancestor: usize) -> bool {
        let mut at = Some(index);
        while let Some(i) = at {
            if i == ancestor {
                return true;
            }
            at = self.node(i).parent;
        }
        false
    }

    /// The node an alias or an `@<n>` id names.
    pub fn find(&self, name: &str) -> Option<NodeId> {
        if let Ok(id) = name.parse::<NodeId>() {
            return self.contains(id).then_some(id);
        }
        if name.starts_with('@') {
            return None;
        }
        self.find_alias(Symbol::lookup(name)?)
    }

    /// The node an alias names, with no parsing.
    pub fn find_alias(&self, alias: Symbol) -> Option<NodeId> {
        self.by_alias.get(&alias).map(|&i| self.node(i).id)
    }

    /// The control of a node, if it is a `T`.
    pub fn get<T: Control + 'static>(&self, id: NodeId) -> Option<&T> {
        let node = self.node(self.index(id)?);
        let any: &dyn Any = node.control.as_ref();
        any.downcast_ref::<T>()
    }

    /// The control of a node, mutably, if it is a `T`: for layout, labels and
    /// styles. What affects routing is changed with `set_flags`.
    pub fn get_mut<T: Control + 'static>(&mut self, id: NodeId) -> Option<&mut T> {
        let index = self.index(id)?;
        let any: &mut dyn Any = self.node_mut(index).control.as_mut();
        any.downcast_mut::<T>()
    }

    /// A node's parent.
    pub fn parent(&self, id: NodeId) -> Option<NodeId> {
        let parent = self.node(self.index(id)?).parent?;
        Some(self.node(parent).id)
    }

    /// The children of a node, or the roots for `None`, in draw and Tab
    /// order.
    pub fn children(&self, id: Option<NodeId>) -> impl Iterator<Item = NodeId> + '_ {
        let list: &[usize] = match id {
            None => &self.roots,
            Some(id) => self
                .index(id)
                .map_or(&[][..], |i| self.node(i).children.as_slice()),
        };
        list.iter().map(|&i| self.node(i).id)
    }

    /// Whether a node is in this router.
    pub fn contains(&self, id: NodeId) -> bool {
        self.by_id.contains_key(&id)
    }

    /// How many nodes the router holds.
    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    /// Whether the router holds no node.
    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }

    /// A node's declared flags.
    pub fn flags(&self, id: NodeId) -> NodeFlags {
        self.index(id)
            .map_or(NodeFlags::NONE, |i| self.node(i).flags)
    }

    /// Set or clear declared flags on a node. Marks the router's derived
    /// state stale.
    pub fn set_flags(&mut self, id: NodeId, flags: NodeFlags, on: bool) {
        let Some(index) = self.index(id) else {
            return;
        };
        let node = self.node_mut(index);
        node.flags = node.flags.with(flags, on);
        self.tab_stale.set(true);
        // Focus, hover and a grab cannot stay on a node that can no longer
        // take them.
        if self.focus.get().is_some_and(|f| !self.can_focus(f)) {
            self.change_focus(None);
        }
        if self
            .hover
            .get()
            .is_some_and(|(h, _)| !self.hover_reachable(h))
        {
            self.update_hover(&[]);
        }
        if self.grab.get().is_some_and(|g| !self.takes_input(g.index)) {
            self.set_grab(None);
        }
    }

    /// What the router knows about a node right now.
    pub fn state(&self, id: NodeId) -> NodeState {
        self.index(id)
            .map_or(NodeState::NONE, |i| self.node(i).state.get())
    }

    pub(crate) fn index(&self, id: NodeId) -> Option<usize> {
        self.by_id.get(&id).copied()
    }

    pub(crate) fn node(&self, index: usize) -> &Node {
        self.nodes[index].as_ref().expect("a live node")
    }

    pub(crate) fn node_mut(&mut self, index: usize) -> &mut Node {
        self.nodes[index].as_mut().expect("a live node")
    }

    /// Visit the visible tree in pre-order, skipping hidden subtrees.
    /// Visit the visible tree in pre-order, skipping hidden subtrees; each
    /// node with whether it is disabled (itself or through an ancestor).
    fn visit_visible(&self, mut visit: impl FnMut(&Node, bool)) {
        let mut stack: Vec<(usize, bool)> = self.roots.iter().rev().map(|&r| (r, false)).collect();
        while let Some((index, inherited)) = stack.pop() {
            let node = self.node(index);
            if node.flags.contains(NodeFlags::HIDDEN) {
                continue;
            }
            let disabled = inherited || node.flags.contains(NodeFlags::DISABLED);
            visit(node, disabled);
            stack.extend(node.children.iter().rev().map(|&c| (c, disabled)));
        }
    }

    /// Draw one node with `pass`, halving the alpha of what it pushed when it
    /// is disabled: the disabled look, the same for every control.
    fn draw_node(
        list: &mut DrawList,
        ctx: &DrawContext,
        node: &Node,
        disabled: bool,
        pass: impl FnOnce(&dyn NodeControl, &mut DrawList, &DrawContext),
    ) {
        let start = list.instances.len();
        pass(node.control.as_ref(), list, &Self::node_context(ctx, node));
        if disabled {
            for instance in &mut list.instances[start..] {
                instance.color[3] *= 0.5;
            }
        }
    }

    fn node_context<'c>(ctx: &DrawContext<'c>, node: &Node) -> DrawContext<'c> {
        let mut own = ctx.clone();
        own.flags = node.flags;
        own.state = node.state.get();
        own
    }
}

impl Router {
    /// Draw the whole tree: every node's `draw`, then every node's
    /// `draw_overlay`. A root router is drawn with this each frame; a nested
    /// router is drawn by its parent's two passes instead.
    pub fn render(&self, list: &mut DrawList, ctx: &DrawContext) {
        self.draw(list, ctx);
        self.draw_overlay(list, ctx);
    }
}

impl DrawControl for Router {
    /// Every visible node's `draw`, in pre-order.
    fn draw(&self, list: &mut DrawList, ctx: &DrawContext) {
        self.visit_visible(|node, disabled| {
            Self::draw_node(list, ctx, node, disabled, |c, l, x| c.draw(l, x));
        });
    }

    /// Every visible node's `draw_overlay`, in pre-order, after every
    /// node's `draw`.
    fn draw_overlay(&self, list: &mut DrawList, ctx: &DrawContext) {
        self.visit_visible(|node, disabled| {
            Self::draw_node(list, ctx, node, disabled, |c, l, x| c.draw_overlay(l, x));
        });
    }
}

/// One step of a hit descent: a node and the part its own `hit_test` gave.
type Hit = (usize, ControlPart);

impl Router {
    /// Give this router the filter chain it runs before routing. Only the
    /// root router of an input surface is given one.
    #[must_use]
    pub fn with_filter_stack(mut self, filters: FilterStack) -> Self {
        self.filters = Some(filters);
        self
    }

    /// The filter chain this router runs, if it was given one.
    pub fn filter_stack(&self) -> Option<&FilterStack> {
        self.filters.as_ref()
    }

    /// The focused node.
    pub fn focused(&self) -> Option<NodeId> {
        self.focus.get().map(|i| self.node(i).id)
    }

    /// Focus a node (or nothing). The node that had the focus is told
    /// `Lost` and the new one `Gained`, only if the focus actually changes.
    /// A node that is not here, or cannot take the focus, clears it.
    pub fn set_focus(&self, id: Option<NodeId>) {
        let index = id
            .and_then(|id| self.index(id))
            .filter(|&i| self.can_focus(i));
        self.change_focus(index);
    }

    /// The innermost node under the pointer.
    pub fn hovered(&self) -> Option<NodeId> {
        self.hover.get().map(|(i, _)| self.node(i).id)
    }

    /// Tell the hovered nodes the pointer left, and forget the hover.
    pub fn clear_hover(&self) {
        self.update_hover(&[]);
    }

    /// The drag in progress.
    pub fn active_drag(&self) -> Option<DragGrab> {
        self.grab.get().map(|g| DragGrab {
            node: self.node(g.index).id,
            start_pos: g.start_pos,
            button: g.button,
            press_modifiers: g.press_modifiers,
        })
    }

    /// The last pointer position seen.
    pub fn pointer(&self) -> Option<[f32; 2]> {
        self.pointer.get()
    }

    /// The modifier keys held, as last reported.
    pub fn modifiers(&self) -> Modifiers {
        self.modifiers.get()
    }

    /// Record the modifier keys held without telling anything (for a router
    /// nested in another, which hands it its modifiers).
    pub fn set_modifiers(&self, modifiers: Modifiers) {
        self.modifiers.set(modifiers);
    }

    /// Deliver one event.
    pub fn deliver_event(&self, event: InputEvent) -> EventResponse {
        match event {
            InputEvent::Mouse(m) => self.deliver_mouse(m),
            InputEvent::Keyboard(k) => self.deliver_keyboard(k),
            InputEvent::Gesture(g) => self.deliver_gesture(g),
            InputEvent::Modifiers(m) => self.deliver_modifiers(m),
        }
    }

    /// Record the modifier keys now held. No node is told.
    pub fn deliver_modifiers(&self, modifiers: Modifiers) -> EventResponse {
        self.modifiers.set(modifiers);
        if let Some(resp) = self.run_filters(InputEvent::Modifiers(modifiers)) {
            return resp;
        }
        if let Some(resp) = self.run_layers(InputEvent::Modifiers(modifiers)) {
            return resp;
        }
        EventResponse::ignored(self.focused())
    }

    /// Deliver a pointer event: to the grab, or to the innermost node under
    /// the pointer.
    pub fn deliver_mouse(&self, event: MouseEvent) -> EventResponse {
        self.pointer.set(Some(event.pos()));
        if let Some(resp) = self.run_filters(InputEvent::Mouse(event)) {
            return resp;
        }
        if self.press_belongs_elsewhere(&event) {
            return EventResponse::consumed(None, self.focused());
        }
        if let Some(resp) = self.run_layers(InputEvent::Mouse(event)) {
            return resp;
        }

        if let Some(grab) = self.grab.get() {
            let grabbed = match event {
                MouseEvent::Up { button, .. } if button == grab.button => {
                    self.set_grab(None);
                    true
                }
                MouseEvent::Move { .. } | MouseEvent::Drag { .. } => true,
                _ => false,
            };
            if grabbed {
                let part = self
                    .node(grab.index)
                    .control
                    .hit_test(event.pos())
                    .map_or(ControlPart::Body, |(p, _)| p);
                let ctx = self
                    .context(grab.index, part)
                    .with_press_modifiers(Some(grab.press_modifiers));
                let node = self.node(grab.index);
                let action = node.control.handle_mouse(&event, &ctx);
                return EventResponse::handled(node.id, part, action, self.focused());
            }
        }

        let path = self.hit_path(event.pos());
        if self.grab.get().is_none() {
            self.update_hover(&path);
        }
        if matches!(event, MouseEvent::Move { .. }) {
            // A plain move reaches no handler, except a nested router's: it
            // routes the move as hover in its own tree.
            if let Some(&(index, part)) = path.last()
                && self.router_at(index).is_some()
                && self.takes_input(index)
            {
                let ctx = self.context(index, part);
                self.node(index).control.handle_mouse(&event, &ctx);
            }
            return EventResponse::ignored(self.focused());
        }
        let presses = matches!(event, MouseEvent::Down { .. } | MouseEvent::Click { .. });
        let Some(&(index, part)) = path.last() else {
            if presses {
                self.change_focus(None);
            }
            return EventResponse::ignored(self.focused());
        };
        if !self.takes_input(index) {
            // Consumed, naming the disabled node that took it.
            return EventResponse::handled(self.node(index).id, part, None, self.focused());
        }
        if presses && self.node(index).flags.contains(NodeFlags::FOCUSABLE) {
            self.change_focus(Some(index));
        }
        if let MouseEvent::Down { button, pos } = event {
            self.set_grab(Some(Grab {
                index,
                button,
                start_pos: pos,
                press_modifiers: self.modifiers.get(),
            }));
        }
        let ctx = self.context(index, part);
        let node = self.node(index);
        let action = node.control.handle_mouse(&event, &ctx);
        // A press that focused something inside a nested router moves this
        // router's focus to it, so only one node is ever focused.
        if presses
            && let Some(inner) = self.router_at(index)
            && inner.focused().is_some()
        {
            self.change_focus(Some(index));
        }
        EventResponse::handled(node.id, part, action, self.focused())
    }

    /// Who receives input at the root right now.
    fn input_owner(&self) -> Owner {
        match self.modal_stack() {
            Some((id, stack)) if !self.flags(id).contains(NodeFlags::HIDDEN) => {
                stack.top_serial().map_or(Owner::App, Owner::Layer)
            }
            _ => Owner::App,
        }
    }

    /// Track each button's press and say whether this event is the rest of a
    /// press another owner took: a layer that closed, a layer pushed over the
    /// app mid-press, or the layer beneath a closed one must not get it.
    fn press_belongs_elsewhere(&self, event: &MouseEvent) -> bool {
        let now = self.input_owner();
        let mut presses = self.presses.borrow_mut();
        // A release is followed by at most its Click; anything else ends it.
        presses.retain(|p| {
            !p.released || matches!(*event, MouseEvent::Click { button, .. } if button == p.button)
        });
        let entry = event
            .button()
            .and_then(|b| presses.iter().position(|p| p.button == b));
        match *event {
            MouseEvent::Down { button, .. } => {
                if let Some(i) = entry {
                    presses.remove(i);
                }
                presses.push(Press {
                    button,
                    owner: now,
                    released: false,
                });
                false
            }
            MouseEvent::Up { .. } => entry.is_some_and(|i| {
                presses[i].released = true;
                presses[i].owner != now
            }),
            MouseEvent::Click { .. } => entry.is_some_and(|i| presses.remove(i).owner != now),
            MouseEvent::Drag { .. } => entry.is_some_and(|i| presses[i].owner != now),
            _ => false,
        }
    }

    /// Deliver a key: Tab and BackTab move the focus, Escape clears it, and
    /// every other key goes to the focused node.
    pub fn deliver_keyboard(&self, event: KeyboardEvent) -> EventResponse {
        if let Some(resp) = self.run_filters(InputEvent::Keyboard(event)) {
            return resp;
        }
        if let Some(resp) = self.run_layers(InputEvent::Keyboard(event)) {
            return resp;
        }
        if let Some(key @ (Key::Tab | Key::BackTab | Key::Escape)) = event.key() {
            if !event.is_down() {
                return EventResponse::ignored(self.focused());
            }
            if key == Key::Escape {
                self.change_focus(None);
                return EventResponse::consumed(None, None);
            }
            self.tab_step(key == Key::Tab, true);
            return EventResponse::consumed(None, self.focused());
        }
        let Some(index) = self.focus.get() else {
            return EventResponse::ignored(None);
        };
        let ctx = self.context(index, ControlPart::Body);
        let node = self.node(index);
        let action = node.control.handle_keyboard(&event, &ctx);
        EventResponse::handled(node.id, ControlPart::Body, action, self.focused())
    }

    /// Deliver a gesture to the innermost node under the last pointer.
    pub fn deliver_gesture(&self, event: GestureEvent) -> EventResponse {
        if let Some(resp) = self.run_filters(InputEvent::Gesture(event)) {
            return resp;
        }
        if let Some(resp) = self.run_layers(InputEvent::Gesture(event)) {
            return resp;
        }
        let path = self
            .pointer
            .get()
            .map(|p| self.hit_path(p))
            .unwrap_or_default();
        let Some(&(index, part)) = path.last() else {
            return EventResponse::ignored(self.focused());
        };
        if !self.takes_input(index) {
            return EventResponse::handled(self.node(index).id, part, None, self.focused());
        }
        let ctx = self.context(index, part);
        let node = self.node(index);
        let action = node.control.handle_gesture(&event, &ctx);
        EventResponse::handled(node.id, part, action, self.focused())
    }

    /// Run the filter chain, if this router was given one. `None` means route.
    fn run_filters(&self, event: InputEvent) -> Option<EventResponse> {
        let filters = self.filters.as_ref()?;
        let ctx = FilterContext {
            pointer: self.pointer.get(),
            modifiers: self.modifiers.get(),
            focused: self.focused(),
            grab: self.grab.get().map(|g| self.node(g.index).id),
        };
        match filters.run(&event, &ctx) {
            Filtered::Pass => None,
            Filtered::Consumed(action) => Some(EventResponse::consumed(action, self.focused())),
            Filtered::Redirect(id) => Some(self.deliver_redirected(id, event)),
        }
    }

    /// Run the layer chain: the installed modal stack's top layer, if any
    /// layer exists. `None` means route as usual.
    fn run_layers(&self, event: InputEvent) -> Option<EventResponse> {
        let (stack_id, stack) = self.modal_stack()?;
        // A hidden stack draws nothing, so it traps nothing either.
        let stack_flags = self.flags(stack_id);
        if stack_flags.contains(NodeFlags::HIDDEN) {
            return None;
        }
        if stack_flags.contains(NodeFlags::DISABLED) {
            // A disabled stack is inert: its layers take nothing, and while
            // any is up the app beneath gets nothing either.
            return stack
                .state()
                .captures_input()
                .then(|| EventResponse::consumed(None, self.focused()));
        }
        let ctx = FilterContext {
            pointer: self.pointer.get(),
            modifiers: self.modifiers.get(),
            focused: self.focused(),
            grab: self.grab.get().map(|g| self.node(g.index).id),
        };
        let outcome = stack.route(event, &ctx, self.modifiers.get())?;
        // While a layer is up the app's nodes are under it: nothing of them
        // stays hovered or grabbed.
        if self.hover.get().is_some() {
            self.update_hover(&[]);
        }
        if self.grab.get().is_some() {
            self.set_grab(None);
        }
        Some(match outcome {
            crate::modal::LayerOutcome::Consumed(action) => {
                EventResponse::consumed(action, self.focused())
            }
            crate::modal::LayerOutcome::Handled(action) => {
                EventResponse::handled(stack_id, ControlPart::Body, action, self.focused())
            }
        })
    }

    /// Let go of the press grab without telling anything (a layer opened
    /// over this router).
    pub(crate) fn cancel_grab(&self) {
        self.set_grab(None);
    }

    /// A layer was pushed: the app's nodes beneath stop being hovered or
    /// grabbed.
    pub(crate) fn layer_opened(&self) {
        self.update_hover(&[]);
        self.set_grab(None);
    }

    /// Deliver `event` to one node alone: no focus change, no grab, no Tab or
    /// Escape handling. A missing node, and any modifier change, are consumed.
    fn deliver_redirected(&self, id: NodeId, event: InputEvent) -> EventResponse {
        let focused = self.focused();
        let Some(index) = self.index(id) else {
            return EventResponse::consumed(None, focused);
        };
        if self.grab.get().is_some_and(|g| g.index != index) {
            self.set_grab(None);
        }
        if matches!(event, InputEvent::Modifiers(_)) {
            return EventResponse::consumed(None, focused);
        }
        if !self.takes_input(index) {
            // A disabled or hidden node takes nothing, redirected or not.
            if !self
                .hover
                .get()
                .is_some_and(|(h, _)| self.hover_reachable(h))
            {
                self.update_hover(&[]);
            }
            return EventResponse::handled(id, ControlPart::Body, None, focused);
        }
        let part_at = |pos: Option<[f32; 2]>| {
            pos.and_then(|p| self.node(index).control.hit_test(p))
                .map(|(part, _)| part)
        };
        let node = self.node(index);
        match event {
            InputEvent::Mouse(mouse) => {
                let hit = part_at(Some(mouse.pos()));
                match hit {
                    Some(part) => {
                        // The target and its ancestors, outermost first, so
                        // HOVER_WITHIN stays on the chain.
                        let mut path: Vec<Hit> = self
                            .chain(index)
                            .into_iter()
                            .rev()
                            .map(|i| {
                                let p = self
                                    .node(i)
                                    .control
                                    .hit_test(mouse.pos())
                                    .map_or(ControlPart::Body, |(p, _)| p);
                                (i, p)
                            })
                            .collect();
                        if let Some(last) = path.last_mut() {
                            last.1 = part;
                        }
                        self.update_hover(&path);
                    }
                    None => self.update_hover(&[]),
                }
                let part = hit.unwrap_or(ControlPart::Body);
                let ctx = self.context(index, part);
                if let MouseEvent::Up { button, .. } = mouse
                    && self
                        .grab
                        .get()
                        .is_some_and(|g| g.index == index && g.button == button)
                {
                    self.set_grab(None);
                }
                let action = node.control.handle_mouse(&mouse, &ctx);
                self.follow_inner_focus(index);
                EventResponse::handled(id, part, action, self.focused())
            }
            InputEvent::Keyboard(key) => {
                let ctx = self.context(index, ControlPart::Body);
                let action = node.control.handle_keyboard(&key, &ctx);
                self.follow_inner_focus(index);
                EventResponse::handled(id, ControlPart::Body, action, self.focused())
            }
            InputEvent::Gesture(gesture) => {
                let part = part_at(self.pointer.get()).unwrap_or(ControlPart::Body);
                let ctx = self.context(index, part);
                let action = node.control.handle_gesture(&gesture, &ctx);
                self.follow_inner_focus(index);
                EventResponse::handled(id, part, action, self.focused())
            }
            InputEvent::Modifiers(_) => EventResponse::consumed(None, focused),
        }
    }

    /// If the nested router at `index` now holds a focus of its own, move
    /// this router's focus to it, so only one node is ever focused.
    fn follow_inner_focus(&self, index: usize) {
        if self.focus.get() != Some(index)
            && self
                .router_at(index)
                .is_some_and(|inner| inner.focused().is_some())
        {
            self.change_focus(Some(index));
        }
    }

    /// Whether a node can hold the focus: focusable and taking input.
    fn can_focus(&self, index: usize) -> bool {
        if !self.takes_input(index) {
            return false;
        }
        let flags = self.node(index).flags;
        flags.contains(NodeFlags::FOCUSABLE)
            || self.router_at(index).is_some_and(|inner| {
                flags.contains(NodeFlags::TAB_THROUGH) || inner.focused().is_some()
            })
    }

    /// Whether hit testing could still reach a hovered node: it is not
    /// hidden, and no ancestor is hidden or disabled (a disabled node itself
    /// is still hit, as the target).
    fn hover_reachable(&self, index: usize) -> bool {
        if self.node(index).flags.contains(NodeFlags::HIDDEN) {
            return false;
        }
        let mut at = self.node(index).parent;
        while let Some(i) = at {
            let node = self.node(i);
            if node
                .flags
                .intersects(NodeFlags::DISABLED | NodeFlags::HIDDEN)
            {
                return false;
            }
            at = node.parent;
        }
        true
    }

    /// Whether a router has any Tab stop that can really take the focus.
    fn has_stops(&self) -> bool {
        let order: Vec<usize> = self.tab_order().clone();
        order.iter().any(|&i| self.is_stop(i))
    }

    /// Whether a node in the Tab order can really take the focus: a nested
    /// router only if it has stops of its own, anything else if focusable.
    fn is_stop(&self, index: usize) -> bool {
        match self.router_at(index) {
            Some(inner) => inner.has_stops(),
            None => self.node(index).flags.contains(NodeFlags::FOCUSABLE),
        }
    }

    /// The context handed to the node at `index` for an event on `part`.
    fn context(&self, index: usize, part: ControlPart) -> EventContext {
        let node = self.node(index);
        let press = self
            .grab
            .get()
            .filter(|g| g.index == index)
            .map(|g| g.press_modifiers);
        let mut ctx = EventContext::new(part)
            .with_modifiers(self.modifiers.get())
            .with_press_modifiers(press);
        ctx.flags = node.flags;
        ctx.state = node.state.get();
        ctx
    }

    /// The nodes under `pos`, outermost first, each with the part it gave:
    /// the roots last to first, then the hit node's children last to first,
    /// down to the deepest hit. Hidden subtrees are skipped; the descent stops
    /// at a disabled node, which is then the target.
    fn hit_path(&self, pos: [f32; 2]) -> Vec<Hit> {
        let mut path = Vec::new();
        let mut level: &[usize] = &self.roots;
        loop {
            let hit = level.iter().rev().find_map(|&i| {
                let node = self.node(i);
                if node.flags.contains(NodeFlags::HIDDEN) {
                    return None;
                }
                node.control.hit_test(pos).map(|(part, _)| (i, part))
            });
            let Some((index, part)) = hit else {
                return path;
            };
            path.push((index, part));
            let node = self.node(index);
            if node.flags.contains(NodeFlags::DISABLED) {
                return path;
            }
            level = &node.children;
        }
    }

    /// Whether a node and every ancestor can take input (not disabled, not
    /// hidden).
    fn takes_input(&self, index: usize) -> bool {
        let mut at = Some(index);
        while let Some(i) = at {
            let node = self.node(i);
            if node
                .flags
                .intersects(NodeFlags::DISABLED | NodeFlags::HIDDEN)
            {
                return false;
            }
            at = node.parent;
        }
        true
    }

    /// The node and its ancestors, innermost first.
    fn chain(&self, index: usize) -> Vec<usize> {
        let mut chain = Vec::new();
        let mut at = Some(index);
        while let Some(i) = at {
            chain.push(i);
            at = self.node(i).parent;
        }
        chain
    }

    fn set_state(&self, index: usize, flag: NodeState, on: bool) {
        let node = self.node(index);
        node.state.set(node.state.get().with(flag, on));
    }

    /// Move the focus, keeping FOCUSED and FOCUS_WITHIN along both chains and
    /// telling the old node `Lost` and the new one `Gained`.
    fn change_focus(&self, new: Option<usize>) {
        let old = self.focus.get();
        if old == new {
            return;
        }
        let old_chain = old.map(|i| self.chain(i)).unwrap_or_default();
        let new_chain = new.map(|i| self.chain(i)).unwrap_or_default();
        self.focus.set(new);
        for &i in &old_chain {
            if !new_chain.contains(&i) || Some(i) == old {
                self.set_state(i, NodeState::FOCUSED | NodeState::FOCUS_WITHIN, false);
            }
        }
        for (depth, &i) in new_chain.iter().enumerate() {
            let (on, off) = if depth == 0 {
                (NodeState::FOCUSED, NodeState::FOCUS_WITHIN)
            } else {
                (NodeState::FOCUS_WITHIN, NodeState::FOCUSED)
            };
            self.set_state(i, off, false);
            self.set_state(i, on, true);
        }
        if let Some(i) = old {
            self.node(i).control.handle_focus(FocusEvent::Lost);
        }
        if let Some(i) = new {
            self.node(i).control.handle_focus(FocusEvent::Gained);
        }
    }

    /// Move the hover to `path` (outermost first, as `hit_path` gives it):
    /// nodes left are told `set_hover(None)` innermost first, nodes entered
    /// `set_hover(Some(part))` outermost first, and the target alone when only
    /// its part changed. HOVERED and HOVER_WITHIN follow.
    fn update_hover(&self, path: &[Hit]) {
        let old = self.hover.get();
        let old_chain = old.map(|(i, _)| self.chain(i)).unwrap_or_default();
        let new_target = path.last().copied();
        if old.map(|(i, _)| i) == new_target.map(|(i, _)| i) {
            if let (Some((i, old_part)), Some((_, part))) = (old, new_target)
                && old_part != part
            {
                self.node(i).control.set_hover(Some(part));
                self.hover.set(Some((i, part)));
            }
            return;
        }
        let new_set: Vec<usize> = path.iter().map(|&(i, _)| i).collect();
        for &i in &old_chain {
            if !new_set.contains(&i) {
                self.set_state(i, NodeState::HOVERED | NodeState::HOVER_WITHIN, false);
                self.node(i).control.set_hover(None);
            }
        }
        let last = path.len().saturating_sub(1);
        for (depth, &(i, part)) in path.iter().enumerate() {
            let flag = if depth == last {
                NodeState::HOVERED
            } else {
                NodeState::HOVER_WITHIN
            };
            let entered = !old_chain.contains(&i);
            if depth != last {
                self.set_state(i, NodeState::HOVERED, false);
            } else if !entered {
                self.set_state(i, NodeState::HOVER_WITHIN, false);
            }
            self.set_state(i, flag, true);
            if entered {
                self.node(i).control.set_hover(Some(part));
            }
        }
        self.hover.set(new_target);
    }

    fn set_grab(&self, grab: Option<Grab>) {
        if let Some(old) = self.grab.get() {
            self.set_state(old.index, NodeState::GRABBED, false);
        }
        if let Some(new) = grab {
            self.set_state(new.index, NodeState::GRABBED, true);
        }
        self.grab.set(grab);
    }

    /// The focus stops, in pre-order: focusable, not disabled, not inside a
    /// hidden or disabled subtree. Rebuilt only after it was marked stale.
    fn tab_order(&self) -> std::cell::Ref<'_, Vec<usize>> {
        if self.tab_stale.get() {
            let mut order = self.tab_order.borrow_mut();
            order.clear();
            let mut stack: Vec<usize> = self.roots.iter().rev().copied().collect();
            while let Some(index) = stack.pop() {
                let node = self.node(index);
                if node
                    .flags
                    .intersects(NodeFlags::HIDDEN | NodeFlags::DISABLED)
                {
                    continue;
                }
                if node
                    .flags
                    .intersects(NodeFlags::FOCUSABLE | NodeFlags::TAB_THROUGH)
                {
                    order.push(index);
                }
                stack.extend(node.children.iter().rev().copied());
            }
            self.tab_stale.set(false);
        }
        self.tab_order.borrow()
    }

    /// The router a node is, if it is a nested router.
    fn router_at(&self, index: usize) -> Option<&Router> {
        let any: &dyn Any = self.node(index).control.as_ref();
        any.downcast_ref::<Router>()
    }

    /// Move the focus one Tab stop forward (or back). With `wrap`, it wraps
    /// at the ends; without, it returns false at an end instead of moving
    /// (for a router whose stops a parent walks through).
    ///
    /// A focused nested router keeps the step: one that traps Tab (no
    /// `TAB_THROUGH` on its node) cycles its own stops; one with
    /// `TAB_THROUGH` steps inside and lets the focus go on only at its end.
    /// A `TAB_THROUGH` router reached by stepping is entered at its first
    /// (or last) stop, and skipped if it has none.
    fn tab_step(&self, forward: bool, wrap: bool) -> bool {
        if let Some(focused) = self.focus.get()
            && let Some(inner) = self.router_at(focused)
        {
            // A trapping router cycles its own stops; one with no stop lets
            // the focus go on.
            if !self.node(focused).flags.contains(NodeFlags::TAB_THROUGH)
                && inner.tab_step(forward, true)
            {
                return true;
            }
            if inner.tab_step(forward, false) {
                return true;
            }
            inner.set_focus(None);
        }
        let order: Vec<usize> = self.tab_order().clone();
        let n = order.len();
        let mut at = self
            .focus
            .get()
            .and_then(|f| order.iter().position(|&i| i == f));
        for _ in 0..n {
            let next = match at {
                Some(p) => {
                    let stepped = if forward {
                        (p + 1 < n).then_some(p + 1)
                    } else {
                        p.checked_sub(1)
                    };
                    match stepped {
                        Some(q) => q,
                        None if !wrap => return false,
                        None if forward => 0,
                        None => n - 1,
                    }
                }
                None if forward => 0,
                None => n - 1,
            };
            at = Some(next);
            let candidate = order[next];
            if let Some(inner) = self.router_at(candidate)
                && self.node(candidate).flags.contains(NodeFlags::TAB_THROUGH)
            {
                inner.set_focus(None);
                if !inner.tab_step(forward, false) {
                    continue;
                }
            } else if let Some(inner) = self.router_at(candidate)
                && !inner.has_stops()
            {
                continue;
            } else if self.router_at(candidate).is_none()
                && !self.node(candidate).flags.contains(NodeFlags::FOCUSABLE)
            {
                continue;
            }
            self.change_focus(Some(candidate));
            return true;
        }
        if wrap {
            self.change_focus(None);
        }
        false
    }
}

/// A router is a layer: a node another router hands events to, which then
/// routes them in its own tree with its own focus, hover and grab.
impl crate::control::HostedControl for Router {
    /// The union of its visible roots' bounds.
    fn bounds(&self) -> crate::rect::Rect {
        let mut bounds: Option<crate::rect::Rect> = None;
        for &r in &self.roots {
            let node = self.node(r);
            if node.flags.contains(NodeFlags::HIDDEN) {
                continue;
            }
            let b = node.control.bounds();
            bounds = Some(match bounds {
                None => b,
                Some(a) => {
                    let x = a.x.min(b.x);
                    let y = a.y.min(b.y);
                    crate::rect::Rect::new(
                        x,
                        y,
                        a.right().max(b.right()) - x,
                        a.bottom().max(b.bottom()) - y,
                    )
                }
            });
        }
        bounds.unwrap_or_default()
    }

    /// Hit where any of its nodes would be hit.
    fn hit_test(&self, pos: [f32; 2]) -> Option<(ControlPart, f32)> {
        (!self.hit_path(pos).is_empty()).then_some((ControlPart::Body, 0.0))
    }

    fn handle_mouse(&self, event: &MouseEvent, ctx: &EventContext) -> Option<ControlAction> {
        self.set_modifiers(ctx.modifiers);
        self.deliver_mouse(*event).action
    }

    fn handle_keyboard(&self, event: &KeyboardEvent, ctx: &EventContext) -> Option<ControlAction> {
        self.set_modifiers(ctx.modifiers);
        self.deliver_keyboard(*event).action
    }

    fn handle_gesture(&self, event: &GestureEvent, ctx: &EventContext) -> Option<ControlAction> {
        self.set_modifiers(ctx.modifiers);
        self.deliver_gesture(*event).action
    }

    fn set_hover(&self, hover: Option<ControlPart>) {
        if hover.is_none() {
            self.clear_hover();
        }
    }

    fn handle_focus(&self, event: FocusEvent) {
        if event == FocusEvent::Lost {
            self.set_focus(None);
        }
    }
}
