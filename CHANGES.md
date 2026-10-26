# CHANGES

Planned new work, feature plans and architectural changes, appended in order
(AGENTS.md, Rule 0). Entries are never edited.

## 2026-10-15 23:25 EDT — Modal layer

Plan: [Modal layer](docs/design/20261015_01_MODAL_PLAN.md) (`Draft`).

- Adds a modal layer to libcontrols for Noble's Keep, whose UI is limited to
  modals opened from on-screen icons: a persistent `ModalState` (phases,
  transition progress, `ModalEvent`s), a per-build `Modal` with an
  app-drawn `ModalFrame`, backdrop and dismiss rules, and a modal layer in
  `ControlHost` that captures all input and draws above every overlay.
- Additive only: no existing public type changes; tinytrowel keeps compiling.

## 2026-10-15 23:31 EDT — Key::Back and a modal stack (breaking: 0.3.0)

Plan: [Modal layer](docs/design/20261015_01_MODAL_PLAN.md), amendment
"2026-10-15 23:31 EDT: Key::Back and a modal stack".

- **Breaking change:** `Key` gains a `Back` variant for the platform back
  action (Android Back, browser back) and becomes `#[non_exhaustive]`.
  `Key` was exhaustive, so a downstream exhaustive `match` on it no longer
  compiles; the version goes to 0.3.0 instead of 0.2.1. Shells send back as
  `Key::Back`, no longer as `Escape`. Both dismiss the top modal, reported
  as `DismissReason::Back` and `DismissReason::Escape`.
- `ModalState` holds a stack of up to four modals instead of one: push,
  pop, replace-top, dismiss-all, per-entry transitions; only the top takes
  input and is dismissed, and lower modals stay drawn under its backdrop.

## 2026-10-16 00:23 EDT — Modals and input filters (supersedes Modal layer)

Plan: [Modals and input filters](docs/design/20261016_01_MODALS_AND_INPUT_FILTERS_PLAN.md)
(`Draft`), superseding [Modal layer](docs/design/20261015_01_MODAL_PLAN.md).

- Adds an input filter stack: `HostedControl::intercept`, a defaulted method
  asked last-added control first before normal routing; a control that
  answers `Capture` receives the event alone. With every control passing,
  routing is 0.2.0's.
- Modals are rebuilt on it: a `ModalStack` hosted control, added last,
  captures every event while its `ModalState` holds any modal, so the base
  UI is level 0 and gets nothing, modifiers included. The host-level modal
  API of the old plan is dropped.
- Answers the old plan's open questions: `Modal::with_hold_open` and
  `ModalEvent::DismissRefused` veto user dismissals; Escape and Back
  dismiss even with an `Edit` focused; scrolling is a later plan; the
  per-instance fade stays in libcontrols.
- `Panel` uses the filter: hover inside panels follows plain moves (a fix),
  and opt-in `with_focus_traversal` lets Tab and Escape traverse children.
- Still breaking (0.3.0) only through `Key::Back` and `#[non_exhaustive]`
  `Key`.

## 2026-10-16 00:58 EDT — Input routing and filters; Modal stack (supersede Modals and input filters)

Plans: [Input routing and filters](docs/design/20261016_02_INPUT_ROUTING_AND_FILTERS_PLAN.md)
(`Draft`) and [Modal stack](docs/design/20261016_03_MODAL_STACK_PLAN.md)
(`Draft`, depends on the first), together superseding
[Modals and input filters](docs/design/20261016_01_MODALS_AND_INPUT_FILTERS_PLAN.md).

- The owner rejected `HostedControl::intercept` (every control asked on
  every event, inside a GPU render loop) and asked for a clean split into a
  filters plan and a modal plan. Nothing of the old plan was implemented.
- Routing contract (plan 02): an app-owned `InputFilter` stack sees every
  event first and answers `Pass`, `Consumed(action)` or `Redirect(key)`;
  controls are never asked and are never filters themselves. Pointer events
  and gestures route by area with a press grab; keys route by focus; Tab,
  BackTab and Escape belong to the focus manager; `Key::Back` is an
  ordinary key.
- Focus changes are events: `FocusEvent::{Gained, Lost}` through
  `HostedControl::handle_focus`; `accepts_focus` lets a control refuse Tab.
  Hover is enter and leave: at most two `set_hover` calls per pointer
  event, with `hovered_key` carried across rebuilds like focus and drag.
- Modifiers are no longer broadcast: `HostedControl::set_modifiers` is
  removed and every handler takes an `EventContext { part, modifiers,
  press_modifiers }`, so modified drags work without a modifier event.
  Only filters see `InputEvent::Modifiers`.
- `Symbol` (one process-wide interner, `sym!` interning once per call
  site) replaces `String` in `ControlKey::Named`, control names,
  `find_by_id` and `EventResponse::target_id`; tinytrowel's `"speed"` lines
  compile unchanged.
- Modals (plan 03): `ModalState` (stack of four, per-entry transition,
  focus, drag, hover, events via `take_events`), a `ModalFilter` created
  once from the state that answers `Redirect` to a hosted `ModalStack` by
  name while any modal exists, per-build `Modal`s with app-drawn frames,
  backdrops, dismiss rules and a hold-open veto; Escape and Back dismiss
  the top even with an `Edit` focused. `ControlHost::draw_controls` and
  `draw_control_overlays` are added there for clipping.
- **Breaking, 0.3.0**, cut once in plan 03's last step with the
  `VERSIONS.md` entry and the `v0.3.0` tag; plan 02 bumps nothing.
- Deferred to a future "focus manager: nested containers" plan: symbol
  paths through nested hosts, Tab order across nesting, `Panel`'s hover
  defects and child tooltips in the first pass.

## 2026-10-16 01:37 EDT — 0.3.0: implementation on a branch, one squash commit, the tag

Plans: [Input routing and filters](docs/design/20261016_02_INPUT_ROUTING_AND_FILTERS_PLAN.md)
and [Modal stack](docs/design/20261016_03_MODAL_STACK_PLAN.md), both frozen.
Small departures in how the steps landed, made on the owner's instruction
("if we need commits per step use a branch and squash merge when finished
(merge the commit messages in that step)"; "we don't want a broken public
API"):

- Every step of both plans was one commit on the branch `impl/0.3.0`
  (`1714da2` to `400328f`), each passing fmt, clippy and tests. `main`
  received them as one squash commit, `8c34296`, whose message merges the
  fourteen step messages. On `main` the plans therefore land in a single
  commit rather than one per step.
- Modal stack plan, step 6 and section 14: the version bump and the
  `VERSIONS.md` entry were committed in step 6 on the branch and reached
  `main` in `8c34296`; the tag `v0.3.0` is on `8c34296`, not on the
  branch commit, so it marks a commit on `main` (AGENTS.md Rule 5.1).
- The `VERSIONS.md` heading for 0.3.0 names the tag but no short commit
  (Rule 5.2): the entry is written in the version commit (Rule 5.3), and a
  commit cannot contain its own hash. The entry says so.

## 2026-10-16 02:04 EDT — Rule 5.5: recursive version bumps

- Owner's instruction (2026-10-16): "the public API rule needs to be
  recursive, if back incompat change in dep must bump to 0.x in parent for
  repos with public surface". AGENTS.md gains Rule 5.5: moving a dependency
  to a semver-incompatible version bumps this crate's minor version (major
  from 1.0), and the bump propagates up through every crate with a public
  API.
- Consequence: the release that moves libcontrols from libmsdf 0.2 to
  libmsdf 0.3.0 (filled Bézier paths) is libcontrols 0.4.0.

## 2026-10-16 02:14 EDT — Focus manager: nested containers

Plan: [Focus manager: nested containers](docs/design/20261016_04_NESTED_CONTAINERS_PLAN.md)
(`Draft`), the plan named in section 12 of
[Input routing and filters](docs/design/20261016_02_INPUT_ROUTING_AND_FILTERS_PLAN.md).

- The top host becomes the only router: a container exposes its child host
  through `HostedControl::with_children`, and focus, hover and the press
  grab are addressed as a `ControlPath` (up to four levels, `Copy`), so an
  event goes straight to the innermost control and no level runs a routing
  pass of its own.
- Fixes the `Panel` defects left by plan 02: hover on plain moves inside a
  panel, hover cleared when the pointer leaves it, Tab into and out of its
  children, and child tooltips drawn in the overlay pass.
- Breaking (Rule 5.4): `EventResponse` gains `target_path` and becomes
  `#[non_exhaustive]`; `Panel`'s handlers stop forwarding. The release is
  0.4.0, shared with the libmsdf 0.3.0 move if neither has been released.

## 2026-10-16 02:29 EDT — Persistent control tree and router layers (supersedes Focus manager: nested containers)

Plan: [Persistent control tree and router layers](docs/design/20261016_05_PERSISTENT_ROUTER_PLAN.md)
(`Draft`), superseding [Focus manager: nested containers](docs/design/20261016_04_NESTED_CONTAINERS_PLAN.md).

- Controls become persistent: a `Router` owns them as a tree of
  `NodeId`s and keeps focus, hover and the grab itself; the app edits the
  tree when its UI changes and draws every frame. No state is carried
  between builds and no `Gained` is repeated.
- `Panel` children are router nodes, so routing, hover and Tab order run
  across containers with no per-level routing. A `Router` implements the
  routed-control traits, so a nested router is a layer with its own focus
  scope.
- Modals: `Modal` becomes a trait (`ModalDialog` is the built-in dialog), so
  self-drawn and shaped controls can be pushed; the modal stack has no depth
  limit, and each layer owns the filter it is asked through, dropped with the
  layer. Input runs through chains: the global `FilterStack` (app-wide rules
  such as accelerators), then the modal stack's top layer, then routing.
- Each node has declared flags (`NodeFlags`: focusable, disabled, hidden,
  tab-through), changed only through the router so derived state such as
  the Tab order is invalidated exactly when needed, and router-owned state
  flags (`NodeState`: focused, focus-within, hovered, hover-within,
  grabbed) passed to controls in the draw and event contexts.
- `ControlHost` and `ControlKey` are removed. The release is 0.4.0,
  shared with the libmsdf 0.3.0 move.

## 2026-10-16 04:10 EDT — Persistent router: how it landed, and departures

Plan: [Persistent control tree and router layers](docs/design/20261016_05_PERSISTENT_ROUTER_PLAN.md),
frozen. Implemented on the branch `impl/0.4.0-router` and squash-merged
into `main` as `5567fa7`. Small departures needed to make the plan work,
or found by the two adversarial reviews the owner asked for:

- **Commits:** steps 2 and 3, and steps 6 and 7, each landed as one branch
  commit (one routing core; the removal made while the step 6 tests were
  ported). Two review commits sit between the steps. `main` has one
  commit.
- **3.1:** `children` returns an iterator rather than `&[NodeId]`.
  `Router::render` draws both passes for a root router, and `len` and
  `is_empty` are added. `Symbol::lookup` finds a name without interning
  it, so `find` never grows the interner. An id has one spelling
  (`@01` does not parse).
- **3.3:** the disabled look (half alpha) is applied by the router to
  whatever a disabled subtree draws, the same for every control, rather
  than by each control. `Radio` draws no focus ring (it never had one).
- **4.1:** `set_focus` refuses a node that cannot take the focus: not
  focusable, unless it is a nested router that is `TAB_THROUGH` or holds
  an inner focus. A press or redirect that focuses a node inside a nested
  router moves the parent's focus to the router node. A `TAB_THROUGH`
  node is in the Tab order even when not `FOCUSABLE`, and a nested router
  is a stop only if it has one of its own.
- **4.1, new:** per-button press ownership. The rest of a press (drags,
  its release, the Click after it) is consumed if what receives input
  changed since its press: a layer closed or was pushed mid-press, or the
  layer beneath a closed one became top. Without it a tap clicked through.
- **6:** `FilterStack::push` takes `&Rc<dyn InputFilter>` (the chain is
  single-threaded). A layer filter's `Redirect` is treated as `Pass`.
- **8:** `LayerMask` is a struct with `drawn` and `texture`
  constructors rather than a public enum (the drawn mask keeps a cache).
  `ModalStack::modal` returns a `Ref`. `ModalState::default_transition`
  is added. `Modal::covered` and `LayerFilter::reset` are added: a layer
  pushed over another ends its hover, content grab and filter press. The
  top modal is told `set_hover(None)` when the pointer leaves its shape.
  A hidden `ModalStack` node traps nothing; a disabled one is inert but
  still keeps input from the app while a layer is up.
- **Version:** unchanged at 0.3.0. Under section 11, the libmsdf 0.3.0
  move, which has not landed in libcontrols, cuts the shared 0.4.0.

Left for the owner:
- `ModalFrame::contains` is no longer consulted: a `ModalDialog`'s
  outside taps use the AABB of its frame (section 8.6). Remove the method,
  or let the dialog's filter use it?
- A grab cancelled by the router (a layer opening, `set_flags`,
  `remove`) is not told to the grabbed control, so a control's own
  pressed or dragging state can stay set.
- One `ModalState` installed in two routers, or a stack removed and
  reinstalled over live entries, leaves entries without layers that
  consume input.

## 2026-10-23 16:25 EDT — Planned: press feedback, after Material 3

Plan: `docs/design/20261023_01_PRESS_FEEDBACK_PLAN.md` (Draft).

The owner asked for a pressed ("depress") look for buttons, planned here
rather than in an app, following Material 3 where possible, with plain-text
tap targets shifting colour slightly within the same hue. The plan has the
router track a transient press per node, separate from a button's latched
state: set on touch-down, cancelled past a touch slop or by the host,
held briefly after a quick tap and faded by a host clock. Buttons draw M3's
pressed state layer and move down slightly; labels used as tap targets shift
their text colour in OKLab with hue and chroma kept. Breaking, so 0.8.0 when
implemented. Nothing is implemented until the owner approves it.

## 2026-10-23 19:27 EDT — Plan: stateful scroll pane

Planned work: docs/design/20261023_02_SCROLL_PANE_PLAN.md (Draft), at the
owner's request ("libcontrols should have a stateful scroll pane, send the
plan"). A `ScrollPane` router node with a shareable `ScrollState`;
clipping, translation, culling and hit mapping in the router; tap or scroll
decided by the touch slop, cancelling the child's press; fling, wheel,
keys, focus into view, anchoring and an indicator; `Button` and `Label`
activate on release. Builds on the press feedback plan (20261023_01).
Nothing is implemented until the owner approves it.

## 2026-10-23 21:26 EDT — Scroll pane plan (20261023_02): departures in implementation

Small departures from docs/design/20261023_02_SCROLL_PANE_PLAN.md, made in
a9eecc6 (steps 1 to 9, in one commit):

- **Step 10, the GPU check.** libcontrols has no GPU test feature and no
  examples, so the check is CPU-side instead: `tests/scroll.rs` compares the
  router's lowered frame (`DrawList::lower`) for a scrolled pane with one
  built by hand from `push_clip`, `push_translate` and the rows in view, and
  checks the resolved clips of nested panes.
- **Section 3.7, anchoring at the start.** Anchoring is skipped while the
  offset is 0, so content added at the top of a pane resting at its start is
  shown rather than pushed out of view. Scrolled into the content, it anchors
  as planned.
- **Section 3.9, where input reads the theme.** The router has no theme of
  its own; input uses the `ScrollTheme` of the `DrawContext` it was last
  drawn with (the default before the first frame).
- **Section 3.3, before the first frame.** `scroll_to`, `scroll_by` and
  `scroll_into_view` bring the pane's range up to date first, so an app can
  restore a place before drawing.
- **Additions:** `ScrollPane::viewport` and `axis`, `ScrollState::is_moving`,
  and a nested `Router`'s `claims_drag` answering for its grabbed node.

## 2026-10-25 18:58 EDT — Plan: edit cursor and insertion point

Planned work: docs/design/20261025_01_EDIT_CURSOR_AND_INSERTION_POINT_PLAN.md
(In implementation), adding cursor / insertion point support to
libcontrols' text edit / text input control (`Edit` and `EditState`).
Tracks insertion cursor position in `EditState` clamped to valid UTF-8
character boundaries with navigation (Left, Right, Home, End), text
insertion (`insert_char`, `insert_text`), and deletion (Backspace, Delete);
draws a vertical SDF cursor bar when active or focused; hit-tests mouse
clicks and drags to the character boundary nearest to the pointer; and
handles keyboard navigation and text entry.

## 2026-10-25 22:52 EDT — Plan: TextArea multi-line text edit control

Planned work: docs/design/20261025_02_TEXT_AREA_CONTROL_PLAN.md
(In implementation), adding a `TextArea` multi-line text edit control to
libcontrols. Wraps text across multiple lines fitting within available width,
shares `EditState` for text storage and UTF-8 cursor positioning, draws
background, border, text lines, vertical SDF cursor bar and glowing active
ring, and handles mouse click/drag positioning and multi-line keyboard navigation
(arrows Up/Down/Left/Right, Home, End, Backspace, Delete, Enter, char insertion).

