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


## 2026-10-28 12:50 EDT — Plans: Drawer and DrawerLayout, and their departures

Planned work: docs/design/20261028_01_DRAWER_PLAN.md and
docs/design/20261028_04_DRAWER_LAYOUT_PLAN.md (both In implementation),
built in `cf12470` and released as 0.10.1. A `DrawerLayout` with two
`Router` children (main, and the drawer's content in sheet coordinates)
holds a call-driven `Drawer`; while the drawer is visible each side is
drawn into a wgpu texture and one composite pass slides the sheet and
darkens the main view by an eased uniform; at `t = 0` there is no texture,
pass or copy. Departures:

1. **Plan 01, size defaults:** `DrawerTheme` has `size` (Left/Right, 0.85
   share, at most 360) and `size_vertical` (Top/Bottom, 0.6 share);
   `DrawerOptions::size` overrides either.
2. **Plan 01, `main_mark`** moved from `DrawerLayers` to `Drawer`, so the
   layout bumps it on main input; the app calls `layout.mark_main()`.
3. **Plan 01, the uniform** is 80 bytes (frame, viewport, sheet_open,
   scrim, shadow): no shift, scale or clear terms (scrim only), a viewport
   rect added; the WGSL field is `frame` since `target` is reserved.
4. **Plan 01, rounded corners** come from the sheet texture's alpha
   (`RoundedBoxPerCorner`), not a test in the shader.
5. **libmsdf writes straight, not premultiplied, alpha** (`blend_over`), so
   the composite blends the sheet with `mix(c, s.rgb, s.a)`; libmsdf's
   `BlurPass` overlay assumes premultiplied alpha, which may be a latent
   bug there (not changed here).
6. **Plan 01, `draw_direct`** drops `main_start`; content is drawn with a
   crate-internal `Router::render_at(origin)` so scroll-pane clips land
   under the translate (plan 04 step 1).
7. **Plan 01, `DrawerLayers::render_sheet`** takes `Option<&AtlasSet>`.
8. **Plan 01, the direct path** is decided by `t == 0`, not the Closed
   phase, so Opening or Dragging at `t = 0` costs nothing; textures are
   freed (or kept warm) only once Closed; the main layer is redrawn on the
   first visible frame after a direct one.
9. **Plan 01, events:** `DrawerReason` has only `App`; `close()` or
   `settle(false)` at `t = 0` emits Closing then Hidden at once; `settle`
   with no velocity, or velocity away from the target, uses the Bézier
   curve, else the Hermite; `open()`/`close()` do nothing while Dragging.
10. **Plans 01/04, `DrawerRoute::Main`** carries `Option<EventResponse>`;
    while the drawer is visible a press on the scrim reaches nothing.
11. **Plan 01, the GPU check** is the integration test `tests/drawer_gpu.rs`
    (images under `out/drawer/` behind `LIBCONTROLS_EVIDENCE`), not an
    example; it skips without an adapter and ran on headless Metal.
12. **Plan 01, extra public queries:** `Drawer::{sheet_at, extent,
    resolved, set_theme, is_visible}` and the forwarded calls,
    `DrawerLayers::{has_textures, texture_bytes}`.
13. **The default sheet colour** is the theme's `bg_color`, 85% alpha, so
    the sheet is slightly translucent by default (the spec's open question
    3); the tests set an opaque `sheet_color`.

## 2026-10-28 18:55 EDT — EditableLabel (plan 20261028_05_EDITABLE_LABEL_PLAN)

New work: an `EditableLabel` control for the flankr companion's remote
nickname, at the owner's request ("nickname should use an EditableLabel …
label with pencil icon which switches to check and optional X (clear)").
Plan docs/design/20261028_05_EDITABLE_LABEL_PLAN.md, approved by that
request and in implementation; additive, 0.10.2 (tag `v0.10.2`).

Small departures from the plan's section 3 (interfaces), needed to test and
use it, all additive: `EditableLabel::TEXT` and `FIELD` part constants
(`ControlPart::Label` / `NumericBox`, named in the plan's prose but not
listed as constants); `shown_text()`, `shows_placeholder()` and
`clear_shown()`, so the placeholder and the X rule can be tested without a
font atlas.

## 2026-10-28 20:00 EDT — PopupMenu (plan 20261028_06_POPUP_MENU_PLAN)

New work: a `PopupMenu` control asked of libcontrols by flankr's plugin plan
(~/src/flankr/docs/design/20261028_04_PLUGINS_PLAN.md, 3.6 item 3, step 4)
for its ⋮ title-bar menu, owner's words "we'll need a popup menu with
actions, maybe in the title bar?". Plan
docs/design/20261028_06_POPUP_MENU_PLAN.md, approved with that plan and in
implementation; additive, 0.10.3 (tag `v0.10.3`).

Small departures, none changing the plan's interfaces:
1. `PopupMenuOptions` derives `Default` (clippy), with the same values as
   the plan's: nothing overridden, `dismiss` all on.
2. Opening through the state does not run the router's `layer_opened`
   (it needs the router); the app's hover and grab are cleared on the next
   routed event instead (`run_layers`), as for any layer.

## 2026-10-28 23:27 EDT — Scrollbar and bookmarks, steps 1-5 and 8 (plan 20261028_08_SCROLLBAR_PLAN)

New work: flankr's agent-view scrollbar backported as a `Scrollbar`
control, with bookmarks added on a section's id, asked by the owner ("we
should backport the advanced scrollbar to libcontrols and use it for the
file viewer, with the optional features like labels") for flankr's file
preview (~/src/flankr/docs/design/20261028_05_BINARY_FILES_PLAN.md, its
23:00 EDT amendment). Plan docs/design/20261028_08_SCROLLBAR_PLAN.md,
approved (its 23:13 EDT amendment): steps 1-5 and 8 only, additive, 0.10.4
(tag `v0.10.4`). Steps 6-7 (sections drawn by a pane) wait for libmsdf's
scoped append; until then `SectionedContent` is only an index of sections
and bookmarks (decision 9's fallback).

Departures (Rule 1.7, small):
1. Decisions 2, 3 and 5: the thumb's length and travel, the drag and the
   bookmarks' places use the track's height where the formulas say the
   window; on a pane they are the same. Standalone, this keeps the thumb
   inside the track when the fed window and the track differ.
2. Decision 5: a standalone `Scrollbar` node draws its pills without a
   clip, since a node does not know its screen position and libmsdf clips
   are in screen pixels; on a pane the router clips them to the viewport.
3. Step 2's "moving 75 (a quarter of the 750 travel) scrubs to 750": a
   quarter of 750 is 187.5; the tests move 187.5 to scrub to 750.
4. Step 4: the text alphas (0.60, 0.90) are tested on the colour helper,
   as the router fixture has no shaper and draws no glyphs.
5. `ScrollbarState` keeps the track and resolved theme values it reads,
   since `press`, `drag_to` and `release` take no track; the theme's
   `bookmark_code_pad` and `bookmark_code_radius` are unused until
   Markdown bookmarks are drawn; a pane's scrollbar state is not kept
   across rebuilds (no `with_scrollbar_state`).

## 2026-10-28 23:44 EDT — Markdown behind features (plan 20261028_09_MARKDOWN_VIEW_PLAN)

New work: flankr's Markdown layout (flankr-markdown) and its `MarkdownView`
moved into libcontrols, basic formatting only, behind a `markdown` feature
with a minimal parser and a `cmark` feature adding pulldown-cmark, at the
owner's word ("we could move the markdown render as well, if it doesn't
have heavy callbacks"; "feature based with cmark as a further feature").
Plan docs/design/20261028_09_MARKDOWN_VIEW_PLAN.md; additive, 0.10.5
(tag `v0.10.5`, 8b1d82e), built after the Scrollbar, so `BookmarkText::Markdown`
pills draw through `parse_inline` with `ScrollbarState::set_faces`.

Departures needed to make the plan work (plan steps 2-4):
1. `MdBlock` gains `outer_markers: Vec<String>` beside `list_depth`: the
   enclosing items' markers, whose widths are the list indent, so a list
   nested under "1." hangs as flankr-markdown's does (decision 3).
2. A hard break is a `'\n'` in a block's run (flankr's `Piece::Break`),
   since the model has no pieces (decision 3).
3. `parse` uses the default theme's gaps; the view parses with its
   resolved `block_gap`/`item_gap` (decisions 7-8).
4. The view lays out with `ControlTheme::default()` until drawn, then with
   the last drawn theme's `markdown` and default font size (section 3).
5. With faces, code is measured by the terminal face's own cell, as the app
   measured; `code_cell_em` applies without faces (decision 7).
6. In the minimal parser a fence's joined lines pair as an inline code span
   (CommonMark's inline rule), so it shows as code, not with its backticks
   (decision 2).
7. A Markdown pill parses its first line only ("…" when it has more), and
   its code chip's padding counts in the pill's width (Scrollbar plan,
   decision 6).
8. Added beyond the plan's list: `MarkdownResolved::default()` and its
   `*_in(&ControlTheme)` colour and size helpers, `MarkdownMeasure::px`.

## 2026-10-28 23:52 EDT — PopupMenu toggle items and columns (plan 20261028_10_POPUP_MENU_TOGGLES_AND_COLUMNS_PLAN)

New work: toggle items that keep the menu open, asked by flankr's bottom
bar plan (~/src/flankr/docs/design/20261028_06_BOTTOM_BAR_UPDATE_PLAN.md,
decision 3) for its volume menu, and cells in shared columns, asked by
flankr's plugin plan (~/src/flankr/docs/design/20261028_04_PLUGINS_PLAN.md,
its 23:14-23:16 EDT amendments) for its plan table. Plan
docs/design/20261028_10_POPUP_MENU_TOGGLES_AND_COLUMNS_PLAN.md;
additive, 0.10.6 (tag `v0.10.6`, 97c2804).

Departures from the plan (Rule 1.7):

1. Step 3: the cells' drawing is tested by unit tests in
   `src/popup_menu.rs`, not router tests: without a shaper or atlas,
   `DrawContext::draw_text` pushes nothing into the `DrawList`, so a router
   test cannot see a cell's text, pen, size or colour. `draw` takes each
   cell from a private `cell_runs` helper, which the unit tests check.
2. Decisions 1 and 3: any toggle, including one whose `checked` is `None`,
   reserves the check column, so a `None` toggle's first flip does not
   widen the menu. Menus with no toggles lay out as before.

## 2026-10-28 23:56 EDT — PopupMenu radio marks (plan 20261028_07_POPUP_MENU_RADIO_PLAN)

New work: a radio style for the `PopupMenu`'s mark column, asked of
libcontrols by flankr's subagents plan
(~/src/flankr/docs/design/20261023_07_SUBAGENTS_PLAN.md, its 20:51 EDT
amendment, items 2 and 4) for its subagents dropdown, owner's words "the
empty dot is the menu icon then the dot is used as a radio selector in
the menu", no colour. Plan docs/design/20261028_07_POPUP_MENU_RADIO_PLAN.md;
additive, 0.10.7 (tag `v0.10.7`, d35e2fa), after 0.10.6's toggles and columns;
toggle items keep the check mark in either style (the plan's 23:49 EDT
amendment).

Departures from the plan (Rule 1.7):

1. Decision 4 and the 23:49 EDT amendment: radio marks are drawn for
   `Action` items only, as that amendment says. A heading with a check
   state draws the check as before in either style (decision 4 had named
   the heading colour for one); toggles always draw the check.
2. Step 2: two tests beyond the list: a dot radius larger than the ring
   is clamped to `R - t`, and rows with cells put their radio marks in
   the same mark column, left of the first column of cells.

## 2026-10-29 00:10 EDT — Sections drawn by a scroll pane, steps 6-7 (plan 20261028_08_SCROLLBAR_PLAN)

Planned work resumed: the Scrollbar plan's steps 6 and 7, deferred by its
23:13 EDT amendment until libmsdf's `DrawList::append_scoped`, now
released as libmsdf 0.6.1 (tag `v0.6.1`, 76448a7; libmsdf plan
20261028_02_SCOPED_APPEND_PLAN). Sections drawn by a pane through
`render_section(id) -> DrawList`, cached per `SectionId`, with heights,
section anchoring and `Router::scroll_to_section`; additive, 0.10.8 (tag
`v0.10.8`, 1d813a1); the libmsdf pin moves 0.6.0 → 0.6.1 (compatible). flankr
moves its libmsdf pin to the same rev when it takes this release.

Departures (Rule 1.7):
1. Decision 9 and step 6: libmsdf's call takes an `offset: [f32; 2]` the
   plan does not spell. Sections are placed with
   `append_scoped(&kept, [vp.x, vp.y + top])` inside the pane's translate,
   instead of `push_translate(vp.x, vp.y + top)`, an append and the end
   (libmsdf plan 3.8-3.9): the same pixels, one transform fewer per
   section.
2. Section 3: `with_source`/`set_source` take
   `Arc<dyn SectionSource + Send + Sync>`, so `SectionedContent` stays
   `Send + Sync` as released in 0.10.4; the trait has no bound (open
   question 4). Agreed by the owner (2026-10-29, "Accept and release").
3. Step 6: "a second frame renders nothing" contradicts decision 7's two
   prefetches a frame; the second frame renders no visible section and at
   most two prefetches, and once the margin is full a frame renders
   nothing.
4. Decision 10: a reported height that changes the list re-anchors in the
   same frame, and one more round draws the sections then in view (the
   prefetch limit shared), so the content does not jump for a frame.
5. Decision 7: freshness is one width-and-theme generation, so returning
   to an earlier width or theme redraws.
6. Section anchoring takes the place of node anchoring for the update it
   applies to; the shift held back under a held thumb is applied on the
   first update after release; `scroll_to_section` re-records the anchors;
   `set_source` drops every drawing.
