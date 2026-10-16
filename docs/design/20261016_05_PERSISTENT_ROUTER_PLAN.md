---
title: Persistent control tree and router layers
date: 2026-10-16
status: Implemented
covers: >
  libcontrols moves from hosts rebuilt per event to a persistent Router that
  owns its controls in a tree and only draws per frame. Focus, hover and the
  press grab are NodeIds kept by the router; Panel children are ordinary
  child nodes; each node has declared flags (NodeFlags, changed only through
  the router) and router-owned state flags (NodeState) passed to controls in
  the draw and event contexts; the Tab order is derived and invalidated
  only by edits the router sees.
  A Router implements the same traits as any routed control, so a router is
  a layer: a node another router hands events to, which then routes them in
  its own tree. Modals become router layers (each modal's content is a
  Router), so per-entry carried state goes away. ControlHost, ControlKey
  and host-scoped carrying APIs are removed. Tests, and the 0.4.0 release
  shared with the libmsdf 0.3.0 move.
supersedes: docs/design/20261016_04_NESTED_CONTAINERS_PLAN.md (Draft; its goals carry forward, its with_children/ControlPath mechanism does not)
sources:
  - "Owner: \"we should move to expecting the controls are persistent and only output per frame, that should fix some of the early concerns. im not sure if we need a generic Router and Panel instances its own or one global router\""
  - "Owner: \"can router be its own router layer?\""
  - "Owner: \"router owns them, drop ControlHost, write the plan. yes, router simply implements the same trait as something routed to so it gets events at its layer, not strictly z more conceptual layer\""
  - "Owner: \"it would be 0.4.0 with the msdf bump\""
  - "Owner: \"got it. the walk at build makes sense.\""
  - docs/design/20261016_02_INPUT_ROUTING_AND_FILTERS_PLAN.md (Implemented; its routing contract is kept, its per-rebuild carrying is removed)
  - docs/design/20261016_03_MODAL_STACK_PLAN.md (Implemented; its modal behaviour is kept, its ModalStack/Modal/content-host mechanism is replaced)
  - docs/design/20261016_04_NESTED_CONTAINERS_PLAN.md (Draft; superseded by this plan)
  - src/host.rs, src/panel.rs, src/modal.rs, src/event.rs, src/handle.rs, src/lib.rs at 696e4bb
  - ~/src/libmsdf docs/design/20261015_01_FILLED_BEZIER_PATH_PLAN.md (libmsdf 0.3.0: SdfKind gains a variant and becomes #[non_exhaustive])
  - AGENTS.md Rules 5.4 and 5.5
related:
  - docs/design/20261016_02_INPUT_ROUTING_AND_FILTERS_PLAN.md
  - docs/design/20261016_03_MODAL_STACK_PLAN.md
  - docs/design/20261016_04_NESTED_CONTAINERS_PLAN.md (superseded by this plan)
---

# Persistent control tree and router layers

## 1. Purpose and scope

libcontrols 0.3.0 expects the app to rebuild a `ControlHost` for every event
or frame, borrowing its controls. Everything that must outlive one build is
carried by the app (`focused_key`, `hovered_key`, `active_drag`,
`modifiers`), resolved again by name, and re-announced (a rebuilt host sends
`FocusEvent::Gained` to the focused control every build). Containers run a
host of their own, so every nesting level routes again; modals store their
content's focus, drag and hover in `ModalState` between builds.

The owner's direction: "we should move to expecting the controls are
persistent and only output per frame." This plan replaces the host with a
**`Router`** that owns its controls in a tree for as long as the app keeps
it. The app edits the tree when its UI changes and calls `draw` every frame.

- **One router per surface** owns every control as a node. Focus, hover and
  the grab are node ids it keeps itself; nothing is carried by the app.
- **Containers are nodes with children.** `Panel` holds no host. Events go
  straight to the innermost node; no level routes on its own.
- **A router is a layer.** `Router` implements the same traits as any routed
  control ("router simply implements the same trait as something routed to
  so it gets events at its layer, not strictly z, more conceptual layer"). A
  parent router hands events to a nested router as it would to a leaf; the
  nested router routes them in its own tree, with its own focus, hover, grab
  and Tab order.
- **Modals are layers of a modal stack.** `Modal` is a trait, so a dialog,
  a self-drawn or a shaped control can be pushed; each pushed layer pushes
  its own filter, which the stack asks while that layer is on top and drops
  with the layer when it is popped. The modal behaviour of plan 03
  (stack of four, transitions, Escape/Back/tap-outside/close controls, the
  hold-open veto, events) is kept.
- **`ControlHost` is dropped**, with `ControlKey`, the host's
  carrying APIs and `ControlHandle`'s role in hosting.

Kept from plan 02: filters see every event first; pointer events route by
area with a press grab and `Move` only driving hover outside a grab; keys go
to the focused node, Tab/BackTab/Escape belong to the focus manager;
`FocusEvent`s; hover as enter and leave; modifiers on every event in an
`EventContext`; `Symbol` names; `Key::Back`.

Out of scope: hit-test indexing (the tree makes it possible; plan 02 open
question 2 stays open), tabbed and scrolling containers, layout engines
(controls keep absolute `Rect`s set by the app).

## 2. Constraints from the code

Read at `696e4bb`.

1. **Controls already keep their values in `Arc` state** (`ButtonState`,
   `SliderState`, `OptionsState`, ...). An app that hands a control to the
   router keeps a clone of its state `Arc` to read values (`take_click`,
   `value()`), as apps already do with a host that borrows its controls.
2. **Controls are `'static` in practice.** `Options` and `SelectableLabel`
   take `&'static` slices; no control borrows from the app. So the router can
   own `Box<dyn Control>` (a `'static` trait object).
3. **`HostedControl` methods take `&self`.** A router that is routed to as a
   control must route through `&self`; its routing state (focus, hover,
   grab, modifiers) therefore lives in `Cell`s, and its tree edits take
   `&mut self`.
4. **Rust 1.94 supports trait upcasting**, so `&mut dyn Control` converts to
   `&mut dyn Any` for typed access (`Router::get_mut::<Slider>`).
5. **`EventResponse` is an exhaustive struct** whose `target_index` and
   `focused_index` are host indices; with a tree they become node ids.

## 3. `NodeId` and the tree

```rust
/// A node's identity: a number from one process-wide counter (an AtomicU64,
/// starting at 1), never reused, so an id kept after its node was removed
/// never names another node, in this router or any other. It is also the
/// node's automatic name, written "@<n>" (3.1). Copy.
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(u64);

impl NodeId {
    pub const fn get(self) -> u64;
}
impl fmt::Display for NodeId;   // "@17"
impl fmt::Debug for NodeId;     // "@17"
impl FromStr for NodeId;        // parses "@17"

struct Node {
    id: NodeId,
    control: Box<dyn Control>,
    alias: Option<Symbol>,         // its alias, if any; kept only to free it on remove (3.1)
    parent: Option<usize>,         // arena index; None for a root
    children: Vec<usize>,          // arena indices, in draw and Tab order
    flags: NodeFlags,              // declared: set at add, changed by set_flags (3.3)
    state: NodeState,              // router-owned: focus, hover, grab (3.3)
}

struct Arena {
    nodes: Vec<Option<Node>>,              // freed slots are reused for new nodes
    by_id: HashMap<NodeId, usize>,         // id -> arena index
    by_alias: HashMap<Symbol, usize>,      // defined name -> arena index
}
```

Inside the router, parents, children, focus, hover and the grab are arena
indices (`usize`): a tree walk or a routing step never hashes. Ids are what
the router hands out and takes in; turning an id into an index is one hash
lookup at the API boundary (`get`, `get_mut`, `set_focus`, `set_flags`,
`remove`, a `Redirect`). An arena slot is reused after its node is removed,
but the id is not, so a stale id finds nothing in `by_id`.

`Control` becomes `DrawControl + HostedControl + Any` (blanket impl for every
`'static` type implementing the first two).

### 3.1 Editing (`&mut self`)

```rust
impl Router {
    pub fn new() -> Self;
    pub fn add(&mut self, parent: Option<NodeId>, control: impl Control) -> NodeId;
    pub fn add_named(&mut self, parent: Option<NodeId>, name: impl Into<Symbol>, control: impl Control)
        -> Result<NodeId, NameError>;
    pub fn remove(&mut self, id: NodeId);                         // and its subtree
    pub fn find(&self, name: &str) -> Option<NodeId>;             // an alias, or "@<n>"
    pub fn find_alias(&self, alias: Symbol) -> Option<NodeId>;     // no parsing; for hot paths
    pub fn get<T: Control>(&self, id: NodeId) -> Option<&T>;
    pub fn get_mut<T: Control>(&mut self, id: NodeId) -> Option<&mut T>;   // layout, labels, styles
    pub fn parent(&self, id: NodeId) -> Option<NodeId>;
    pub fn children(&self, id: Option<NodeId>) -> &[NodeId];      // None: the roots
    pub fn contains(&self, id: NodeId) -> bool;
    pub fn with_filter_stack(self, filters: FilterStack) -> Self;  // the root router only (section 6)
    pub fn filter_stack(&self) -> Option<&FilterStack>;
}
```

**Names.** A node's id is its name: `@17` is node 17, in every router, for
the life of the process. A **defined name** is an alias for an id:

```rust
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum NameError {
    /// Another node of this router already has this alias.
    Taken(NodeId),
    /// The alias starts with '@' (the id form) or with 'libcontrols/' (the
    /// library's own namespace).
    Reserved,
}
```

- **Aliases are unique within a router.** `add_named` with an alias in use
  returns `Err(NameError::Taken)` and adds nothing; an alias is freed when its
  node is removed. Separate routers (a modal's content, a nested router) have
  separate aliases, so two modals can each have an `ok`. Their ids still
  differ.
- **Every node has an id**, named or not, so any node can be logged as
  `@17` and found again with `find("@17")` without the interner growing:
  ids are numbers, formatted only when displayed.
- **The `libcontrols/` namespace** is reserved for nodes the library itself
  installs under well-known aliases, one per router, so the app and the
  library can find them without being handed an id: today
  `libcontrols/ModalStack` (`ModalStack::NAME`, section 8.3). `add_named`
  rejects an alias in that namespace with `NameError::Reserved`; only the
  library's install functions use it. A root with a `libcontrols/` alias is
  kept after every other root, so it draws above and is hit-tested before
  the app's roots, whatever order roots are added in. Apps are free to use
  their own `<crate>/<name>` aliases; only `libcontrols/` is reserved.
- `find` takes either form: `find("ok")` looks up an alias, `find("@17")`
  parses an id and checks it belongs to this router. Both are one hash
  lookup; `find_alias(sym!("ok"))` skips the parse.
- **There is no lookup from an id back to its alias.** An app keeps the ids
  of the controls it cares about (the struct that owns a modal holds its
  `ok` and `cancel` ids) and compares ids; `EventResponse` names its target
  by id only. Aliases exist to find a node by name, chiefly the library's
  well-known nodes.

`remove` of a node that holds the focus, hover or grab
clears it, sending `FocusEvent::Lost` and `set_hover(None)` to the removed
node first, so a control that commits on blur always hears it. Removing an
id that no longer exists does nothing.

The router keeps **derived state**: today the Tab order, a pre-order list of
the nodes that are focus stops (3.3: `FOCUSABLE`, not `DISABLED`, not inside
a `HIDDEN` or `DISABLED` subtree), "the walk at build"; a hit-test index
later, if one is added. It is rebuilt once, the next time it is needed,
after it is marked stale, and never on a plain Tab. It is marked stale by
the edits that can change it: `add`, `add_named`, `remove` and `set_flags`.
Everything that routing depends on and could change is a declared flag
(3.3), and declared flags change only through `set_flags`, so the router
always sees the change: there is no `invalidate()` and no polling. This is
the invariant cooperative callers keep: what affects routing goes through
the router.

### 3.2 Drawing (per frame, `&self`)

`impl DrawControl for Router`: `draw` visits the tree in pre-order, calling
each node's `draw`; `draw_overlay` visits it again calling each node's
`draw_overlay`. A container draws its own background in `draw` and its
children follow; a child's tooltip is drawn in the second pass, above every
node's first pass. (Fixes plan 02's Panel tooltip defect.) A `HIDDEN` node
and its subtree are skipped in both passes. Each node is drawn with a copy
of the caller's `DrawContext` whose `flags` and `state` are that node's
(3.3), so a control draws its focus ring, hover or disabled look from them.

### 3.3 Flags

Two small bit sets per node, of different types, so that the router's state
can never be written by the app:

```rust
/// What a node declares about itself. Set from the control's default_flags
/// when it is added; changed only through Router::set_flags.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct NodeFlags(u16);

impl NodeFlags {
    pub const NONE: Self;
    /// A Tab stop, and focused by a press on it.
    pub const FOCUSABLE: Self;
    /// Takes no input and is no Tab stop, with its subtree; still drawn and
    /// still hit (so it blocks what lies beneath).
    pub const DISABLED: Self;
    /// Not drawn, not hit and no Tab stop, with its subtree.
    pub const HIDDEN: Self;
    /// On a nested router node: its focus stops join the parent's Tab order
    /// instead of the router being one stop (section 5).
    pub const TAB_THROUGH: Self;

    pub const fn contains(self, other: Self) -> bool;
    pub const fn union(self, other: Self) -> Self;
    pub const fn difference(self, other: Self) -> Self;
}
impl BitOr, BitAnd, BitOrAssign, Not for NodeFlags;   // and the same set of methods

/// What the router knows about a node right now. Written only by the router;
/// read by controls (in DrawContext and EventContext) and by the app.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct NodeState(u16);

impl NodeState {
    pub const NONE: Self;
    pub const FOCUSED: Self;        // this node holds the focus
    pub const FOCUS_WITHIN: Self;   // a descendant holds the focus
    pub const HOVERED: Self;        // this node is under the pointer (innermost)
    pub const HOVER_WITHIN: Self;   // a descendant is under the pointer
    pub const GRABBED: Self;        // this node holds the press grab
    pub const fn contains(self, other: Self) -> bool;
}

pub trait HostedControl {
    // ... handlers as in 0.3.0, minus accepts_focus ...
    /// The flags a node starts with when this control is added.
    fn default_flags(&self) -> NodeFlags { NodeFlags::FOCUSABLE }
}

impl Router {
    pub fn flags(&self, id: NodeId) -> NodeFlags;
    pub fn set_flags(&mut self, id: NodeId, flags: NodeFlags, on: bool);   // marks derived state stale
    pub fn state(&self, id: NodeId) -> NodeState;
}

// DrawContext and EventContext gain, for the node being drawn or handled:
pub flags: NodeFlags,
pub state: NodeState,
```

**Declared and state flags are separate types** so that `set_flags` cannot
take `FOCUSED`, so that changing a declared flag is the one thing that
invalidates derived state, and so that state flags are plainly the router's
output.

**Keeping state up to date.** When the focus moves, the router clears
`FOCUSED` on the old node and `FOCUS_WITHIN` on its ancestors, and sets them
on the new node and its ancestors (only the ancestors not shared by both
chains are touched). Hover does the same with `HOVERED` and `HOVER_WITHIN`
alongside the `set_hover` calls of 4.2; the grab sets and clears `GRABBED`.
The cost is the two parent chains, the nodes hover already visits.

**Part-level hover stays with `set_hover`.** State flags are per node;
`Radio` and `Options` still learn which cell is hovered from
`set_hover(Some(ControlPart::Custom(i)))`.

**Controls draw from the state.** `Button`, `Label`, `ArrowLabel`,
`SelectableLabel`, `Radio`, `Options`, `Slider`, `SliderNumeric` and `Edit`
draw their active ring when their app-set `is_active` **or**
`ctx.state.contains(FOCUSED)`; `Panel` draws its glow when active or
`FOCUS_WITHIN`; a `DISABLED` node is drawn with its alpha halved by the
control. `Label` declares `NodeFlags::NONE` (plan 02 open question 3: a
display label is no Tab stop).

## 4. Routing

All routing takes `&self`: focus, hover, grab, pointer and modifiers are
`Cell`s; the derived state is a `RefCell` with one stale flag (3.1).

```rust
impl Router {
    pub fn deliver_event(&self, event: InputEvent) -> EventResponse;
    pub fn deliver_mouse(&self, event: MouseEvent) -> EventResponse;
    pub fn deliver_keyboard(&self, event: KeyboardEvent) -> EventResponse;
    pub fn deliver_gesture(&self, event: GestureEvent) -> EventResponse;
    pub fn deliver_modifiers(&self, modifiers: Modifiers) -> EventResponse;

    pub fn focused(&self) -> Option<NodeId>;
    pub fn set_focus(&self, id: Option<NodeId>);        // Lost/Gained on a real change only
    pub fn hovered(&self) -> Option<NodeId>;
    pub fn clear_hover(&self);
    pub fn active_drag(&self) -> Option<DragGrab>;
    pub fn pointer(&self) -> Option<[f32; 2]>;
    pub fn modifiers(&self) -> Modifiers;
}
```

### 4.1 The contract

Plan 02's section 3, with nodes for indices:

1. Record the pointer or modifiers.
2. Ask the filters, last added first: `Pass`, `Consumed(action)` or
   `Redirect(NodeId)` (section 6).
3. **Area-based events** go to the innermost node whose `hit_test` accepts
   the pointer: test the roots last to first; on a hit, test that node's
   children last to first, and so on; the deepest hit is the target. A
   `Down` grabs the target; while the grab stands, `Move`, `Drag` and the
   matching `Up` go to it wherever the pointer is. `Move` outside a grab
   reaches no handler. A `Down` or `Click` focuses the target if it is
   `FOCUSABLE`, leaves the focus if it is not, clears it on nothing. Hit
   testing skips `HIDDEN` subtrees; a target that is `DISABLED` (or inside a
   `DISABLED` subtree) receives no handler call: the event is consumed
   (`handled`, naming the node, no action) and nothing beneath it is hit.
4. **Keys** go to the focused node. Tab/BackTab step through the Tab order
   (wrapping); Escape clears the focus; their presses and releases reach no
   node unless a filter takes them.
5. **Modifier changes** reach no node.
6. **Focus changes** send `Lost` to the old node and `Gained` to the new,
   only when the focused node actually changes. There is no rebuild, so no
   repeat `Gained`.
7. **Hover** is enter and leave along the parent chain (4.2).

The `EventContext` handed to the target carries its part (from its own
`hit_test`, `Body` for keys), the router's modifiers, `press_modifiers` when
the target holds the grab, and the target's `flags` and `state` (3.3).

### 4.2 Hover along the chain

The router remembers the hovered node and its part. On a pointer event
outside a grab with new target `new` (with part `p`) and old `old`:

- nodes on `old`'s chain (the node and its ancestors) that are not on
  `new`'s chain are told `set_hover(None)`, innermost first;
- nodes on `new`'s chain not on `old`'s are told `set_hover(Some(part))`,
  outermost first, each with the part its own `hit_test` returned during the
  descent;
- if the target is unchanged and only its part changed, it alone is told.

A pointer event over a panel's child therefore hovers both; moving between
siblings tells only the two; leaving the panel tells the child, then the
panel. At most twice the depth per event; no walk over every node.

### 4.3 Cost per event

One hit-test scan per level crossed on a pointer event (the only
per-control work left, as in plan 02); `k` filter calls for `k` filters; at
most two `handle_focus` calls per focus change; hover calls bounded by
depth; one handler call. No name lookups during routing: every reference is
a `NodeId`.

## 5. A router is a layer

`impl HostedControl for Router` and `impl DrawControl for Router`, so a
`Router` is a `Control` and can be a node in another router. Its
`HostedControl` methods route in its own tree:

| Method | Router behaviour |
|---|---|
| `bounds` | the union of its roots' bounds |
| `hit_test(pos)` | `Some((Body, 0.0))` if any of its nodes would be hit at `pos`, else `None` |
| `handle_mouse(e, ctx)` | `self.set_modifiers(ctx.modifiers)`, then its own `deliver_mouse(e)`; returns the inner response's `action` |
| `handle_keyboard(e, ctx)` | the same with `deliver_keyboard` (Tab cycles inside it) |
| `handle_gesture(e, ctx)` | the same with `deliver_gesture` |
| `set_hover(None)` | `clear_hover()` on itself |
| `handle_focus(Lost)` | `set_focus(None)` on itself |
| `default_flags` | `FOCUSABLE` |

So a nested router is one stop in its parent's Tab order and keeps the
focus inside itself while it is focused (Tab cycles its own nodes): it is a
layer, "not strictly z, more conceptual". With `TAB_THROUGH` set on its node
in the parent, Tab walks through it instead: when the parent's Tab reaches
that node, the parent hands the step to the nested router (found by
downcasting the node's control to `Router`), which moves to its next stop
and keeps the focus; when it has no stop left in that direction it clears
its own focus and the parent moves on to its next stop. The nested router
keeps its own focus, hover and grab either way. The parent sees one leaf and
routes nothing inside it; the nested router routes everything inside. Only
one router routes any event, so there is no level-by-level delivery.

`Router::set_modifiers(&self, m)` records modifiers without telling
anything (plan 02's `ControlHost::set_modifiers`, carried over for layers).

Ordinary containers (`Panel`) are **not** routers: their children are nodes
of the same router, so focus, hover and Tab order run across them. A nested
router is used where a separate focus scope is wanted: a modal's content,
or an app's self-contained widget group.

## 6. The filter stack

Input passes through **chains**, like iptables tables and chains: each
chain is an ordered list of rules with its own owner, and the chains run in
a fixed order for every event the root router is given:

1. **The global chain**, a `FilterStack` (this section): app-wide rules.
2. **The layer chain**, the modal stack (section 8.4): only the top layer's
   filter, which the layer owns.
3. **Routing** (section 4): area and focus.

A chain ends the event by answering anything but `Pass`. Routers do not own
filters; nested routers run no chain at all.

```rust
pub enum Filtered { Pass, Consumed(Option<ControlAction>), Redirect(NodeId) }
pub struct FilterContext { pub pointer, pub modifiers, pub focused: Option<NodeId>, pub grab: Option<NodeId> }
pub trait InputFilter { fn filter(&self, event: &InputEvent, ctx: &FilterContext) -> Filtered; }

/// A handle to one filter stack; clones share it. One per input surface.
#[derive(Clone, Default)]
pub struct FilterStack(Rc<RefCell<Vec<FilterEntry>>>);

enum FilterEntry {
    Owned(Box<dyn InputFilter>),       // add: stays until the stack is dropped
    Pushed(Weak<dyn InputFilter>),     // push: stays while its owner keeps the Arc
}

impl FilterStack {
    pub fn new() -> Self;
    /// Add a filter the stack owns.
    pub fn add(&self, filter: impl InputFilter + 'static);
    /// Push a filter owned elsewhere. It stays on the stack until its last
    /// Arc is dropped; the stack then skips it and prunes it on its next pass.
    pub fn push(&self, filter: &Arc<dyn InputFilter>);
    /// Run the chain for one event: last pushed first, stopping at the
    /// first answer that is not Pass.
    pub fn run(&self, event: &InputEvent, ctx: &FilterContext) -> Filtered;
}
```

- **Order is push order, reversed**: the filter pushed last is asked first,
  and the first answer that is not `Pass` ends the pass. `Pass` is "next
  rule", `Consumed` is a drop (with an optional action returned to the app),
  and `Redirect(node)` delivers the event to that node alone, as plan 02's
  section 9.4 describes (no focus change, no grab, no Tab/Escape handling,
  other grabs dropped, hover moved off other nodes, modifier changes
  consumed, a missing node consumes).
- **One per input surface.** The app makes a `FilterStack` and gives it to
  its root router (`Router::with_filter_stack`); the root router runs it at
  the start of every `deliver_*`, after recording the event. Nested routers
  are not given one: they see only what reaches them. Anything that needs to
  add a rule keeps a clone of the handle.
- **What goes here: global rules.** A global accelerator (Ctrl+Q anywhere),
  a debug overlay toggle, a screen-wide input lock. This stack is not the
  z-order stack: modal layers do not push onto it (section 8); the root
  router runs this stack **first**, then the modal stack's top layer (8.4),
  then normal routing. So a global accelerator still works while a modal is
  open, and a modal never has to know about it.
- **Removal by RAII.** A `Pushed` entry holds a `Weak`; its owner holds the
  `Arc`. Dropping the owner takes the rule out, for rules that belong to
  something with a lifetime (a screen's accelerators while that screen is
  shown).
- `FilterStack` is `Rc`-based and not `Send`: input is routed on the render
  thread. `FilterContext` and `Filtered` stay `#[non_exhaustive]`.

## 7. `Panel`

`Panel` becomes a plain container control: its frame, title, subtitle and
glow, with **no child host**. Its children are added to the router under
the panel's node (`router.add(Some(panel_id), child)`). Removed from
`Panel`: `with_control`, `with_named_control`, `deliver_event`,
`with_active_drag`, `active_drag`, `with_focus`, `with_initial_focus`,
`with_focus_key`, `focused_key`, `focused_index`, `with_hovered_key`,
`hovered_key`; its handlers answer only for its own body (`Activated` on a
left press or Enter/Space). `PanelData` and `PanelState` stay. `content_rect`
stays, for the app to lay its children out.

## 8. Modals

### 8.1 What stays

`ModalState` (the shared stack of entries with phases, transitions,
`ModalEvent`s, the `occupied` flag and the queries and dismissals of its API)
stays, with three changes: it has **no depth limit** (`MAX_MODAL_DEPTH` is
removed; the stack grows as needed); its entries lose the `focus`, `drag`,
`hover`, `focus_seeded` and `outside_press` fields (a modal keeps those in
its own state); and pushing moves to the router (8.4), so that an entry
always has a layer: `push`, `push_with`, `replace_top` and
`replace_top_with` become crate-private, while `dismiss`, `pop`, `close`,
`dismiss_all`, `advance`, `set_progress`, the queries and `take_events`
stay public. `ModalTransition`, `ModalEvent`, `ModalPhase`,
`ModalEntryView`, `Backdrop`, `ContentReveal` and `DismissOn` are
unchanged. `DismissReason::Control` carries a `NodeId` (it was a
`Symbol`) and `DismissReason` gains `Requested` (8.5); it stays `Copy`.

### 8.2 `Modal` is a trait

Anything can be pushed: a dialog, a self-drawn radial menu, a shaped book, a
popup. A modal is a control with two extra answers:

```rust
pub trait Modal: Control {
    /// Backdrop, which keys and taps dismiss it, hold-open.
    fn options(&self) -> ModalOptions { ModalOptions::default() }

    /// This layer's filter, owned by the layer and asked by the modal stack
    /// while the layer is on top (8.3). The default blocks or dismisses
    /// pointer events outside the control's bounds (an AABB); a shaped modal
    /// returns a filter with its own shape.
    fn filter_layer(&self, layer: &LayerContext) -> Box<dyn LayerFilter> {
        Box::new(ModalLayerFilter::new(self.bounds(), layer.clone()))
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct ModalOptions {
    pub backdrop: Option<Backdrop>,   // default Some(Backdrop::default())
    pub dismiss: DismissOn,           // Escape, Back, tap outside: default all
    pub hold_open: bool,              // default false
}

/// What a layer filter needs to route to its modal and dismiss it.
#[derive(Clone, Debug)]
pub struct LayerContext {
    pub state: Arc<ModalState>,
    pub id: Symbol,                   // the entry
    pub options: ModalOptions,
}
```

A modal draws itself; while it is on the stack its `DrawContext` and
`EventContext` carry `layer: Option<LayerView>` (`id`, `phase`, `progress`,
`depth`, `is_top`), from which it animates its opening and closing. A
modal that wants to close itself returns `ControlAction::Dismiss { control:
None }` (8.5).

### 8.3 The layer filter

A layer filter is an `InputFilter` that may also look at what its layer
drew, so its shape can follow the layer's animation:

```rust
pub trait LayerFilter: InputFilter {
    /// Called by the stack each frame after it draws this filter's layer,
    /// with the list and the range of instances the layer pushed (backdrop
    /// excluded). Default: nothing.
    fn drawn(&self, _list: &DrawList, _range: Range<usize>) {}
}

pub trait LayerShape {
    fn contains(&self, pos: [f32; 2]) -> bool;
    fn drawn(&self, _list: &DrawList, _range: Range<usize>) {}
}
impl LayerShape for Rect { /* the AABB */ }

pub struct ModalLayerFilter<S: LayerShape> { /* shape, LayerContext, press state */ }
impl<S: LayerShape> ModalLayerFilter<S> {
    pub fn new(shape: S, layer: LayerContext) -> Self;
}
impl<S: LayerShape> InputFilter for ModalLayerFilter<S>;
impl<S: LayerShape> LayerFilter for ModalLayerFilter<S>;   // forwards drawn to the shape
```

`ModalLayerFilter<Rect>` is the default (an AABB of the control's bounds).
The library also provides the shaped one:

```rust
/// A layer filter whose inside is the shape the layer actually draws, or an
/// alpha mask.
pub type ShapedLayerFilter = ModalLayerFilter<LayerMask>;

pub enum LayerMask {
    /// The layer's own SDF output. Each frame (drawn), the instances the
    /// layer pushed are lowered (DrawList::lower on a list holding just that
    /// range) and kept; contains(pos) evaluates them on the CPU with
    /// libmsdf's sdf_eval_with_params, the same math as the shader, and is
    /// true where any filled shape has distance <= `inside` (default 0.0).
    /// Text glyphs and textures do not count as shape.
    Drawn { inside: f32 },
    /// A CPU copy of the alpha texture the app binds for the layer, stretched
    /// over `rect`: contains(pos) samples it (nearest) and is true where the
    /// alpha is at least `threshold`. Outside `rect` is outside.
    Texture { rect: Rect, width: u32, height: u32, alpha: Arc<[u8]>, threshold: u8 },
}
impl LayerShape for LayerMask;
```

`Drawn` costs one lowering of the top layer's instances per frame and, per
pointer event, one `sdf_eval_with_params` per kept command (stopping at the
first inside); a shaped modal draws a handful of shapes. The book, the
radial menu and other self-drawn modals use it with no shape of their own;
`Texture` serves a modal drawn from a sprite. With libmsdf 0.3.0 filled
paths (`sd_path_fill`) are shapes too.

What it answers, for the entry it was made for:

| Event | Answer |
|---|---|
| Escape down, `dismiss.escape` (or Back down, `dismiss.back`) | request `Escape` (or `Back`); `Consumed` |
| any other key, any gesture | `Pass`: to the modal; focus and Tab work inside it |
| pointer event inside the shape, or any pointer event while a press that began inside is held | `Pass`: to the modal |
| `Down` outside | remember the button; `Consumed` |
| `Up` outside with that button remembered | forget it; if `dismiss.outside_tap`, request `OutsideTap`; `Consumed` |
| `Click` outside with nothing remembered | if `dismiss.outside_tap`, request `OutsideTap`; `Consumed` |
| any other pointer event outside | `Consumed` |
| a modifier change | `Consumed` |

In the modal stack's chain, `Pass` means "to this modal" and `Consumed`
means "to nothing"; a layer filter does not use `Redirect`. Either way no
lower layer and no app node sees an event while a layer is on top. With `dismiss.outside_tap` false, outside clicks
are only blocked; with it true, they dismiss: "a normal modal filter
dismisses outside clicks or treats them as dismissal". Holding a press that
began inside means a drag from a slider in the modal to outside the shape
still reaches the slider, and its release cannot dismiss.

To **request** a dismissal: with `hold_open`, queue
`ModalEvent::DismissRefused { id, by }`; otherwise close that entry with
`by` (a crate-private `ModalState::dismiss_entry(id, by)`, which closes the
named entry, not whatever is top).

A **shaped window** returns `ShapedLayerFilter::new(LayerMask::Drawn {
inside: 0.0 }, layer.clone())` (or `Texture { .. }`) from `filter_layer`,
or a `ModalLayerFilter` over a `LayerShape` of its own, or any other
`LayerFilter` the app writes.

### 8.4 `ModalStack` owns the layers

```rust
pub struct ModalStack {
    state: Arc<ModalState>,
    viewport: Rect,
    layers: Vec<Layer>,               // bottom..top
}

struct Layer {
    id: Symbol,
    modal: Box<dyn Modal>,
    options: ModalOptions,
    filter: Box<dyn LayerFilter>,     // from modal.filter_layer(); dropped with the layer
}

impl ModalStack {
    /// "libcontrols/ModalStack".
    pub const NAME: &'static str;
    /// Add the stack to `router` as a root named NAME.
    pub fn install(router: &mut Router, state: Arc<ModalState>, viewport: Rect)
        -> Result<NodeId, ModalError>;
    pub fn state(&self) -> &Arc<ModalState>;
    pub fn modal<T: Modal>(&self, id: impl Into<Symbol>) -> Option<&T>;
    pub fn modal_mut<T: Modal>(&mut self, id: impl Into<Symbol>) -> Option<&mut T>;
}

impl Router {
    /// Push `modal` as entry `id` on the installed stack: record the entry
    /// in its ModalState, make the layer, and push the layer's filter.
    pub fn push_modal(&mut self, id: impl Into<Symbol>, modal: impl Modal) -> Result<(), ModalError>;
    pub fn push_modal_with(&mut self, id: impl Into<Symbol>, modal: impl Modal, t: ModalTransition)
        -> Result<(), ModalError>;
    pub fn replace_top_modal(&mut self, id: impl Into<Symbol>, modal: impl Modal) -> Result<(), ModalError>;
}

#[non_exhaustive]
pub enum ModalError {
    NoStack,          // push_modal on a router with no installed stack
    AlreadyInstalled, // install twice in one router
    AlreadyOpen,      // push_modal with an id that is open
}
```

- **Pushing** records the entry, builds the `LayerContext`, calls
  `modal.filter_layer(&layer)` and keeps the filter in the new `Layer`.
  Pushing an id whose entry is still closing
  queues it as plan 03 describes; the layer is made when the entry is.
- **Popping is a drop.** Dismissals go through `ModalState` (the app's
  `pop`/`close`/`dismiss`, or a filter's request). When an entry finishes
  closing, the stack drops its `Layer` on its next `draw_overlay` or handler
  call; the modal, its own state and its filter go with it. There is no
  unregistering.
- **Routing.** The root router, after running the global filter stack
  (section 6) and before routing, asks its installed stack (found once by
  the reserved alias and downcast) to take the event. With no layers the
  stack declines and routing goes on as usual. Otherwise it runs the **top**
  layer's filter only: `Consumed` ends the event there; `Pass` gives it,
  with the router's modifiers, to the top layer's modal (its `handle_*`
  methods, as `EventContext` describes), only while that entry is `Open`
  (otherwise it is consumed). Either way the router routes nothing further:
  the app's nodes see nothing while a layer exists. Lower layers receive nothing; they are told `set_hover(None)`
  once when a layer opens above them.
- **Drawing.** `draw_overlay` draws every layer bottom to top: its backdrop
  over the viewport (alpha times progress when `fade_with_progress`), then
  the modal's `draw` and `draw_overlay`, with `ctx.layer` set, then
  `filter.drawn(list, range)` with the instances the modal pushed. As a
  `libcontrols/` root the stack draws after every app root (3.1).
- **Answers.** A `ControlAction::Dismiss { control }` from the top modal
  requests `Control(id)` for `Some(id)` or `Requested` for `None`, through
  the same request as the filter (hold-open applies).

### 8.5 `ControlAction::Dismiss`

`ControlAction` gains `Dismiss { control: Option<NodeId> }` and becomes
`#[non_exhaustive]`. Outside a modal stack it is an ordinary action returned
to the app.

### 8.6 `ModalDialog`, the built-in modal

Plan 03's dialog becomes one `Modal`:

```rust
pub struct ModalDialog {
    viewport: Rect, frame_rect: Rect, content_rect: Rect,
    frame: Box<dyn ModalFrame>,       // ModalFrame and PanelFrame as in plan 03
    options: ModalOptions,
    close_controls: Vec<NodeId>,
    reveal: ContentReveal, clip_content: bool,
    initial_focus: Option<NodeId>,
    content: Router,                  // the dialog's controls, focus, hover, grab
}

impl ModalDialog {
    pub fn new(viewport: Rect, frame: Rect, content: Rect) -> Self;
    pub fn centered(viewport: Rect, size: [f32; 2], padding: f32) -> Self;
    pub fn with_frame(self, frame: impl ModalFrame + 'static) -> Self;
    pub fn with_options(self, options: ModalOptions) -> Self;
    pub fn with_reveal(self, reveal: ContentReveal) -> Self;
    pub fn with_clip_content(self, clip: bool) -> Self;
    pub fn content(&self) -> &Router;
    pub fn content_mut(&mut self) -> &mut Router;      // add the dialog's controls here
    pub fn with_close_control(self, id: NodeId) -> Self;    // a node of content()
    pub fn with_initial_focus(self, id: NodeId) -> Self;    // a node of content()
}

impl Modal for ModalDialog {
    fn options(&self) -> ModalOptions;                 // as set
    // filter_layer: the default, the AABB of its frame rect (bounds())
}
```

Its handlers route through its content router (applying the initial focus
once, the first time it takes input); an `Activated` from a close control
becomes `ControlAction::Dismiss { control: Some(id) }`. It draws its frame,
its content (clipped, faded or revealed by `ctx.layer`, plan 03 section 11)
and its frame-over-content. The app builds it once and keeps the ids it
needs:

```rust
let mut dialog = ModalDialog::centered(viewport, [240.0, 120.0], 12.0);
let ok = dialog.content_mut().add(None, Button::new("OK", ok_rect));
let cancel = dialog.content_mut().add(None, Button::new("Cancel", cancel_rect));
router.push_modal(sym!("confirm"), dialog.with_close_control(cancel).with_initial_focus(ok))?;
// the owning struct keeps `ok` and `cancel`
```

libcontrols draws no close button of its own: close controls are the app's.

## 9. `EventResponse`

```rust
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct EventResponse {
    pub handled: bool,
    pub target: Option<NodeId>,          // was target_index
    pub target_part: Option<ControlPart>,
    pub action: Option<ControlAction>,
    pub focused: Option<NodeId>,         // was focused_index
}
```

For an event a router layer handled, the outer response names the layer
node and carries the inner action, as a `Panel` did in 0.3.0; the inner
response is returned by the layer's own `deliver_*` when the app calls it.

## 10. What is removed

- `ControlHost`, `ControlHostBuilder` and everything on them
  (`builder`, `with_control`, `with_named_control`, `with_custom_control`,
  `with_initial_focus`, `with_filter`, `control_count`, `add_control`,
  `add_named_control`, `control_key`, `resolve`, `focused_index`,
  `focused_key`, `set_focused_key`, `set_focused_index`, `focus_next`,
  `focus_prev`, `clear_focus`, `find_by_id`, `hit_test`, `active_drag`,
  `set_active_drag`, `hovered_key`, `set_hovered_key`, `clear_hover`,
  `set_modifiers`, `draw_controls`, `draw_control_overlays`, `deliver_*`).
  Their router equivalents are in sections 3 and 4.
- `ControlKey` (replaced by `NodeId`; aliases find nodes by name).
- `EventResponse::target_id` (responses name their target by `NodeId`).
- `HostedControl::accepts_focus` (replaced by `default_flags` and
  `NodeFlags::FOCUSABLE`; changed at run time with `set_flags`).
- `ControlHit` (the router's hit test is internal; `EventResponse` carries
  the part).
- `DragGrab::key` becomes `DragGrab::node: NodeId`; `DragGrab::new` takes a
  `NodeId`.
- The `Panel` hosting methods listed in section 7.
- The `Modal` struct (replaced by the `Modal` trait and `ModalDialog`), its
  lifetime parameter and its `with_control`/`with_named_control`/
  `with_custom_control`; `ModalStack`'s lifetime parameter and
  `new`/`with_modal`/`modals`/`top_modal`; the `ModalFilter` struct and
  `ModalState::filter` (each layer pushes its own filter, 8.3);
  `MAX_MODAL_DEPTH`; the public `ModalState::push`, `push_with`,
  `replace_top` and `replace_top_with` (pushing is `Router::push_modal`);
  `ModalEntry` per-entry content state.
- `ControlHandle` stays only as a reading helper (`ReadValue`); controls are
  no longer registered through it.

## 11. Compatibility and version

Everything in sections 3 to 10 is breaking. Under Rule 5.4 the release is a
minor bump, and the owner's decision is that it is **0.4.0, shared with the
libmsdf 0.3.0 move** (Rule 5.5; libmsdf 0.3.0 adds a variant to its
exhaustive `SdfKind` and marks it `#[non_exhaustive]`). Whichever of the two
lands last bumps `Cargo.toml` to `0.4.0`, writes one `VERSIONS.md` entry
covering both, and is tagged `v0.4.0`. This plan's last step says which
applies when it runs.

## 12. Steps

Each step is one commit on a branch, squash-merged into `main` when the plan
is complete (CHANGES.md, 2026-10-16 01:37 EDT). Each passes
`cargo fmt -p libcontrols -- --check`, `cargo clippy -p libcontrols
--all-targets` with no warnings and `cargo test -p libcontrols`. `ControlHost`
keeps working until step 7 so each step stays green; new tests go in
`tests/router.rs` and `tests/modal.rs`, using the `Recorder` of
`tests/common`.

1. **The tree and its flags.** `NodeId`, the arena, `Router::new`, `add`,
   `add_named`, `remove`, `find`, `name`, `get`, `get_mut`, `parent`,
   `children`, `contains`, `find_alias`; `NodeId` (the global
   counter, `Display`/`FromStr` as `@<n>`), `NameError`; `Control: Any`; `NodeFlags`, `NodeState`,
   `HostedControl::default_flags`, `flags`, `set_flags`, `state`; the
   `flags`/`state` fields of `DrawContext` and `EventContext`; `impl
   DrawControl for Router` (pre-order, two passes, `HIDDEN` skipped, the
   node's flags and state in the context).
   Tests: `a_removed_id_never_names_a_later_node`,
   `an_alias_is_unique_within_a_router`,
   `separate_routers_may_reuse_an_alias`,
   `every_node_is_found_by_its_id_name`,
   `ids_are_never_reused_even_when_slots_are`,
   `an_id_from_another_router_finds_nothing`,
   `an_alias_may_not_start_with_at`,
   `the_libcontrols_namespace_is_reserved`,
   `a_removed_nodes_alias_is_free_again`,
   `get_mut_reaches_the_concrete_control`, `removing_a_node_removes_its_subtree`,
   `the_tree_draws_in_pre_order_and_overlays_in_a_second_pass`,
   `a_node_starts_with_its_controls_default_flags`,
   `a_hidden_subtree_is_not_drawn`,
   `each_node_draws_with_its_own_flags_and_state`.
   **Done when:** those pass and every existing test passes unmodified.

2. **Routing in one router; the filter stack** (4.1, 4.3, 6). The
   contract, `FilterStack` (owned and pushed entries, reverse order, weak
   pruning), `with_filter_stack`, `Redirect(NodeId)`, the grab, `EventContext`, `EventResponse` with node
   ids as a new type used by the router only (`ControlHost` keeps the old
   response until step 7).
   Tests: plan 02's routing and filter tests, ported to a router (one test
   each; the names keep their 0.3.0 wording), plus
   `a_press_on_a_nested_child_reaches_only_the_child`,
   `a_grab_follows_a_nested_control_off_its_area`,
   `the_last_pushed_filter_is_asked_first`,
   `a_pushed_filter_leaves_when_its_arc_is_dropped`,
   `a_nested_router_runs_no_filter_stack`.
   **Done when:** those pass and a press on a node three levels deep produces
   exactly one handler call in the whole tree.

3. **Focus, Tab order, hover along the chain, state flags** (3.1, 3.3, 4.1
   items 3, 4 and 6, 4.2). Includes the controls drawing their ring from
   `FOCUSED`, `Panel`'s glow from `FOCUS_WITHIN`, the disabled look, and
   `Label` declaring `NONE`.
   Tests: `focus_events_fire_only_on_real_changes`,
   `tab_walks_into_a_panel_and_out_again`,
   `the_tab_order_is_rebuilt_after_an_edit_or_set_flags`,
   `tab_skips_disabled_and_hidden_subtrees`,
   `a_disabled_node_consumes_events_and_blocks_what_is_beneath`,
   `focus_within_and_hover_within_follow_the_chains`,
   `the_grab_sets_grabbed`,
   `a_focused_button_draws_its_ring`,
   `removing_the_focused_node_sends_lost_first`,
   `a_plain_move_hovers_a_panel_child`,
   `entering_a_child_tells_the_panel_then_the_child`,
   `moving_between_siblings_tells_only_the_two`,
   `leaving_the_panel_tells_the_child_then_the_panel`.
   **Done when:** in a router of `[Button, Panel{Button, Button}, Button]`,
   eight Tabs from no focus visit outer 0, panel, child 0, child 1, outer 2,
   then outer 0, panel, child 0, with exactly one `Gained` and at most one
   `Lost` per Tab.

4. **A router is a layer** (section 5).
   Tests: `a_nested_router_is_one_tab_stop_and_traps_tab_inside`,
   `a_nested_router_routes_its_own_tree`,
   `the_parent_never_routes_inside_a_nested_router`,
   `leaving_a_nested_router_clears_its_hover`,
   `a_nested_router_sees_the_parents_modifiers`,
   `tab_through_walks_into_a_nested_router_and_out_again`.
   **Done when:** those pass.

5. **`Panel` without a host** (section 7).
   Tests: `a_panels_children_are_router_nodes`,
   `a_panel_answers_only_for_its_body`,
   `a_panel_childs_tooltip_draws_above_a_later_node`.
   **Done when:** those pass; `Panel`'s 0.3.0 hosting tests in `src/lib.rs`
   are rewritten against a router with the same assertions.

6. **Modals** (section 8). The `Modal` trait, `ModalOptions`,
   `LayerContext`, `LayerView` in the contexts, `LayerFilter`,
   `ModalLayerFilter`, `LayerShape`, `ShapedLayerFilter` and `LayerMask`, the root router asking the stack after the global filter
   stack,
   `ModalStack` owning layers, `Router::push_modal`,
   `ControlAction::Dismiss`, `ModalDialog`; `ModalState` without a depth
   limit or per-entry content state.
   Tests: every plan 03 test, ported (same names), plus
   `a_modal_is_built_once_and_keeps_its_focus_across_frames`,
   `popping_drops_the_layer_and_its_filter_leaves_the_stack`,
   `a_global_accelerator_works_while_a_modal_is_open`,
   `only_the_top_layers_filter_is_asked`,
   `an_outside_click_dismisses_or_is_only_blocked_by_option`,
   `a_drag_that_began_inside_reaches_the_modal_outside_its_shape`,
   `a_shaped_modal_uses_its_own_filter_layer`,
   `a_drawn_mask_follows_the_layers_sdf_as_it_animates`,
   `a_drawn_mask_ignores_text_and_textures`,
   `a_texture_mask_decides_inside_by_alpha`,
   `a_self_drawn_modal_dismisses_itself`,
   `more_than_four_modals_can_be_pushed`,
   `installing_names_the_stack`,
   `a_second_install_in_the_same_router_is_refused`,
   `the_stack_stays_after_roots_added_later`,
   `a_closed_modals_content_is_dropped`,
   `no_gained_is_repeated_while_a_modal_stays_open`.
   **Done when:** those pass.

7. **Remove `ControlHost` and `ControlKey`** (section 10). Port
   `tests/events.rs`, `tests/routing.rs`, `tests/filter.rs` and the unit
   tests in `src/lib.rs` to the router (assertions unchanged where the
   behaviour is unchanged), delete what is removed, and make `EventResponse`
   the node-id form.
   **Done when:** the crate has no `ControlHost`, every test passes, and
   clippy is clean.

8. **README and version.** Rewrite the README's usage,
   "Input routing" and "Modals" sections for the router (the examples
   mirrored by tests in `tests/router.rs` and `tests/modal.rs`); if the
   libmsdf 0.3.0 move has already landed unreleased, bump to 0.4.0 with one
   `VERSIONS.md` entry for both and tag the squash commit `v0.4.0`;
   otherwise leave the version and record in `CHANGES.md` that the libmsdf
   move cuts 0.4.0.
   **Done when:** the README examples pass as tests and the version state
   matches section 11.

## 13. Open questions

None at present.

## Amendment — 2026-10-16 03:16 EDT: implementation started

Status changes from `Draft` to `In implementation`. The owner approved
this plan ("thats good, approve and start the work") and asked for
comprehensive tests, including tests meant to break it, with an adversarial
review by a subagent. Implementation begins with step 1 on the branch
`impl/0.4.0-router`, squash-merged into `main` when the plan is complete.
From this amendment on the document is immutable; departures are recorded
in `CHANGES.md` (AGENTS.md, Rule 1.7).

## Amendment — 2026-10-16 04:10 EDT: implemented

Status changes from `In implementation` to `Implemented`. Every step is
done and its **Done when:** holds (`cargo fmt`, `cargo clippy` with no
warnings and `cargo test` pass; 306 tests, including two rounds of
adversarial tests). The steps were commits on the branch
`impl/0.4.0-router`, all reaching `main` in the squash commit
`5567fa7` (departures: `CHANGES.md`, 2026-10-16 04:10 EDT addendum):

- Step 1 (the tree and its flags): branch commit `c15d83e`.
- Steps 2 and 3 (routing, the filter chain; focus, Tab order, hover,
  state): branch commit `6c10e87`.
- Step 4 (a router is a layer): branch commit `9568011`.
- Step 5 (`Panel` without a host): branch commit `64b708e`.
- Steps 6 and 7 (modals; `ControlHost` and `ControlKey` removed): branch
  commit `a00313d`.
- Step 8 (README and version): branch commit `6712a2f`; the version is
  unchanged until the libmsdf 0.3.0 move (section 11).
- Adversarial reviews and their fixes: branch commits `a998e8c` and
  `cb25e46`.
