---
title: "Focus manager: nested containers"
date: 2026-10-16
status: "Superseded (by [Persistent control tree and router layers](20261016_05_PERSISTENT_ROUTER_PLAN.md))"
covers: >
  Routing through nested containers from the top host alone. A hosted
  control can expose the host inside it (HostedControl::with_children); the
  top host addresses focus, hover and the press grab as a ControlPath (a
  short, Copy list of ControlKeys) and delivers each event straight to the
  innermost control, so no level runs a routing pass of its own. Tab order
  walks the whole tree; hover enter and leave are told along the path; Panel
  stops forwarding and draws its children's overlays in the overlay pass.
  EventResponse gains the path and becomes #[non_exhaustive]. Tests, the
  version bump and compatibility.
sources:
  - docs/design/20261016_02_INPUT_ROUTING_AND_FILTERS_PLAN.md, section 12 (names this plan and its scope) and open questions 2 to 4
  - "Owner: \"the point of the focus manager and filter stack is to prevent the common UI deep event delivery problem.\""
  - "Owner: \"filter stack gets global events, controls have an area they receive area-based events for (mouse) and key events based on focus\""
  - "Owner: \"start the next step\""
  - src/host.rs at bcf92c4 (ControlHost routing, Slot/Hovered caches, deliver_redirected, draw passes)
  - src/panel.rs (Panel's child ControlHost, its forwarding handle_mouse/handle_keyboard/handle_gesture at panel.rs:476 to 515, Panel::draw drawing the child host whole at panel.rs:459)
  - src/modal.rs (ModalStack and Modal run their own content host for dismissal rules)
  - src/slider_numeric.rs (composes controls internally, not through a host)
  - AGENTS.md Rules 5.4 and 5.5 (version bumps)
related:
  - docs/design/20261016_02_INPUT_ROUTING_AND_FILTERS_PLAN.md (Input routing and filters; Implemented; this plan extends its routing contract across levels)
  - docs/design/20261016_03_MODAL_STACK_PLAN.md (Modal stack; Implemented; untouched here, see section 9)
  - The libcontrols move to libmsdf 0.3.0 (CHANGES.md, 2026-10-16 02:04 EDT), which is also a minor bump; section 11
---

# Focus manager: nested containers

## 1. Purpose and scope

Plan 02 fixed routing inside one `ControlHost` but left containers as they
were: a `Panel` holds a child `ControlHost` and forwards every event it
receives to it, so each level runs a full routing pass of its own (filters,
hit test, hover, grab, focus). That is the deep delivery the focus manager
exists to prevent, and it leaves four defects named in plan 02, section 12:

- `Move` never reaches a panel's children (the outer host keeps `Move` from
  every handler outside a grab), so hover inside a panel follows only
  presses and drags;
- `Panel` does not clear its children's hover when the pointer leaves it;
- Tab steps over a panel's children, and Escape clears only the outer focus;
- `Panel::draw` draws its child host whole, so a child's tooltip is drawn in
  the first pass and can end up under a control added after the panel.

This plan makes the **top host** the only router. A container exposes the
host inside it; the top host addresses focus, hover and the grab as a
**path** through the levels and delivers each event to the innermost control
directly. Containers become passive.

In scope: `ControlPath`, `HostedControl::with_children`, path-based hit
testing, hover, grab, focus and Tab order in the top host, `Panel` as the
first container, `EventResponse::target_path`, and the version bump.

Out of scope: `ModalStack` and `Modal` (they keep their own content host,
because dismissal needs to see the content's responses; section 9), filters
on nested hosts (a container's child host has no filters; filters belong to
the top host), indexing the hit-test scan (plan 02 open question 2, still
open), `Label` focus (plan 02 open question 3, still open), tabbed and
scrolling containers.

## 2. Constraints from the code

Read at `bcf92c4`.

1. **`ControlHost<'a>` is invariant in `'a`** (it holds `Box<dyn Control +
   'a>` inside a `RefCell` in `Panel`). A trait method on `HostedControl`
   cannot return `&RefCell<ControlHost<'_>>` with a lifetime shorter than
   the container's `'a`. A higher-ranked visitor can:
   `&mut dyn FnMut(&mut ControlHost<'_>)` is `for<'x> FnMut(&mut
   ControlHost<'x>)`, and the routing code it runs stores no controls.
2. **Panel's handlers forward** (`panel.rs:476` to `515`): `handle_mouse`,
   `handle_keyboard` and `handle_gesture` seed the child host's modifiers and
   call its `deliver_event`; `Panel` itself answers only when no child took
   the event (`Activated` on a left press or Enter/Space).
3. **Focus, hover and the grab are cached per host** as a key and an index
   (`Slot`, `Hovered`, `(DragGrab, usize)`). Controls are append-only, so a
   resolved index stays right for a host's life; the same holds per level.
4. **The public carriers** are `focused_key()`/`set_focused_key()`,
   `hovered_key()`/`set_hovered_key()`, `active_drag()`/`set_active_drag()`
   on the host, and the same pairs on `Panel` for its children.
5. **`EventResponse`** is an exhaustive struct with public fields, so adding
   a field is breaking (plan 02 open question 4).
6. **`DragGrab`** is `#[non_exhaustive]` since 0.3.0, so it can gain a field
   without a break.
7. **A new defaulted method on `HostedControl`** costs nothing for
   implementations that do not override it.

## 3. `ControlPath`

New in `src/host.rs`, re-exported:

```rust
/// How many levels a path can name: the top host and three nested hosts.
pub const MAX_NESTING: usize = 4;

/// One control named through the levels: the key in the top host, then the
/// key inside that control's child host, and so on. Copy, fixed size, no
/// allocation.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct ControlPath {
    keys: [ControlKey; MAX_NESTING],   // unused tail is ControlKey::Slot(0)
    len: u8,                           // 1..=MAX_NESTING
}

impl ControlPath {
    pub fn root(key: ControlKey) -> Self;              // a control in the top host
    pub fn child(self, key: ControlKey) -> Option<Self>; // None past MAX_NESTING
    pub fn keys(&self) -> &[ControlKey];
    pub fn first(&self) -> ControlKey;                 // the top-level control
    pub fn leaf(&self) -> ControlKey;                  // the innermost control
    pub fn len(&self) -> usize;
    pub fn parent(self) -> Option<Self>;               // None for a root
    pub fn starts_with(&self, other: &ControlPath) -> bool;
}

impl From<ControlKey> for ControlPath;                 // root
```

`PartialEq`/`Hash` compare only `keys[..len]` (implemented by hand, not
derived, so the unused tail never matters). A nesting deeper than
`MAX_NESTING` is not an error: at the fourth level a container is routed to
as a leaf and forwards as it does today (section 6.3).

## 4. Containers

### 4.1 `HostedControl::with_children`

```rust
pub trait HostedControl {
    // ... as in 0.3.0 ...

    /// Run `visit` on the host inside this control, if it is a container,
    /// and return true; return false for a leaf. The top host uses it to
    /// route straight to the controls inside; a container is told nothing
    /// about events that go to its children.
    fn with_children(&self, _visit: &mut dyn FnMut(&mut ControlHost<'_>)) -> bool {
        false
    }
}
```

`ControlHandle` forwards it. A container implements it by borrowing its
child host mutably for the duration of the call
(`visit(&mut self.children.borrow_mut()); true`). The top host never holds
two borrows of the same level at once, and never stores anything borrowed
through the visitor.

### 4.2 What a container still receives

A container is the leaf of a path when the pointer is on it but on none of
its children, or when it holds the focus itself. It then gets the ordinary
handlers for events that are its own (a press on its body, Enter while it is
focused), exactly like any control. It never receives an event meant for a
child, and it never forwards.

### 4.3 The child host's own state

A container's child host keeps its `focused`, `hovered` and `active_drag`
fields, but while the container is routed through a top host those fields
are **not used**: the top host's path is the only record. They are still
used when an app drives a container directly (`Panel::deliver_event`, kept
for that). `Panel`'s accessors (`focused_key`, `hovered_key`,
`active_drag`, `with_focus_key`, `with_hovered_key`, `with_active_drag`)
keep their meaning for that direct use; their documentation says the top
host's path supersedes them when the panel is hosted.

## 5. The top host

### 5.1 Layout

```rust
pub struct ControlHost<'a> {
    controls: Vec<Box<dyn Control + 'a>>,
    control_ids: Vec<Option<Symbol>>,
    filters: Vec<&'a dyn InputFilter>,
    focused: Option<Route>,
    hovered: Option<(Route, Option<ControlPart>)>,
    active_drag: Option<(DragGrab, Route)>,
    pointer: Option<[f32; 2]>,
    modifiers: Modifiers,
}

/// A path with each level's resolved index beside it.
#[derive(Copy, Clone, Debug)]
struct Route {
    path: ControlPath,
    indices: [usize; MAX_NESTING],
}
```

`Route` replaces plan 02's `Slot` and `Hovered`; a root route is a `Slot`.
Resolving a path walks the levels once (through `with_children`) when it is
set, as a key is resolved once today.

### 5.2 Hit testing through levels

`hit_path(pos) -> Option<(Route, ControlPart)>`: hit-test the top host (last
to first, first hit); if the hit control is a container and the path is
shorter than `MAX_NESTING`, hit-test its child host the same way inside
`with_children`; stop at the first level with no hit (the container is the
leaf) or at a leaf. One `hit_test` scan per level crossed; no handler, hover
or focus work happens during the descent.

`ControlHost::hit_test` (the public single-level test) is unchanged.

### 5.3 Delivering to a route

`with_leaf(route, f)`: walk `route.indices` down through `with_children` and
call `f(&dyn Control)` on the leaf. Every delivery in section 3 of plan 02
goes through it:

- **Pointer events and gestures**: to the leaf of `hit_path(pos)`, or of the
  grab's route while a grab stands.
- **Keys**: to the leaf of the focused route.
- **Redirect** (plan 02, 9.4): the redirect key names a top-level control;
  it is delivered to that control itself (a route of length 1), not into
  it. `ModalStack` is not a container (section 9).

The `EventContext` is built once, by the top host, for the leaf: the part
the leaf's own `hit_test` returned, the host's modifiers, and
`press_modifiers` when the leaf holds the grab. No level seeds another.

### 5.4 Hover along the path

The remembered hover is a route and the leaf's part. On a pointer event
outside a grab, with the new hit route `new` and the old route `old`:

- every control on `old` that is not on `new` (comparing level by level
  from the root: the levels after the first difference) is told
  `set_hover(None)`, innermost first;
- every control on `new` that was not on `old` is told
  `set_hover(Some(part))`, outermost first, where `part` is that control's
  own hit part (for ancestors, the part their `hit_test` returned during
  the descent; for the leaf, the leaf's);
- if the leaf is the same and only its part changed, the leaf alone is told.

So entering a panel's child tells the panel `Some(Body)` (if it was not
already hovered) and the child its part; moving from one child to a sibling
tells only the two children; leaving the panel entirely tells the child and
the panel `None`. At most `2 × MAX_NESTING` calls per event; for a flat host
it is plan 02's "at most two".

`Move` outside a grab still reaches no handler (plan 02, constraint 5): it
only drives this hover rule, now at every level. That fixes the first
defect in section 1, and the leave rule fixes the second.

### 5.5 The grab

A `Down` on a leaf grabs that leaf's route. `DragGrab` gains the path:

```rust
#[non_exhaustive]
pub struct DragGrab {
    pub key: ControlKey,           // = path.first(), as in 0.3.0
    pub path: ControlPath,         // new
    pub start_pos: [f32; 2],
    pub button: MouseButton,
    pub press_modifiers: Modifiers,
}

impl DragGrab {
    pub fn new(key: ControlKey, start_pos: [f32; 2], button: MouseButton) -> Self; // path = root(key)
    pub fn at_path(path: ControlPath, start_pos: [f32; 2], button: MouseButton) -> Self;
}
```

`set_active_drag` resolves `path`; a grab built with `DragGrab::new` is a
root grab, as in 0.3.0.

### 5.6 Focus and Tab order

The focused control is a route. `FocusEvent::Lost` and `Gained` go to the
leaves of the old and new routes, as plan 02 sends them to controls today;
containers on the path are not told (they are not focused unless they are
the leaf).

A press on a leaf that accepts focus focuses that leaf's route; a press on
a leaf that refuses focus leaves the focus alone; a press on nothing clears
it (plan 02, 7.2, unchanged per leaf).

**Tab order** is a pre-order walk of the tree: each top-level control, and
right after a container, its children in order (recursively, to
`MAX_NESTING`). Controls whose `accepts_focus` is false are skipped; a
container that accepts focus is itself a stop before its children. Tab from
the last stop wraps to the first, BackTab the reverse. Escape clears the
focused route (any depth). A walk costs one `accepts_focus` call per control
visited until a stop is found; it runs only on Tab and BackTab.

### 5.7 Carrying across a rebuild

```rust
impl ControlHost<'_> {
    pub fn focused_path(&self) -> Option<ControlPath>;
    pub fn set_focused_path(&mut self, path: Option<ControlPath>);   // Gained to the leaf, as set_focused_key
    pub fn hovered_path(&self) -> Option<ControlPath>;
    pub fn set_hovered_path(&mut self, path: Option<ControlPath>);   // tells nobody; part re-sent once
}
```

The 0.3.0 key forms stay and mean the top-level element:
`focused_key()` is `focused_path().map(|p| p.first())`, and
`set_focused_key(k)` is `set_focused_path(k.map(ControlPath::root))`. A
path that no longer resolves at any level is dropped, as a key is today.

## 6. `Panel`

1. **Container.** `Panel` implements `with_children` on its child host.
2. **No forwarding.** `handle_mouse` answers `Activated { part: ctx.part }`
   to a left `Down` or `Click` and nothing else; `handle_keyboard` answers
   `Activated` to Enter or Space; `handle_gesture` returns `None`. (Each was
   that answer's fallback when no child took the event; now no child event
   reaches the panel.)
3. **Deeper than `MAX_NESTING`.** A panel at the fourth level is routed to
   as a leaf. So that nothing is lost there, its handlers keep forwarding
   when — and only when — it is called for an event whose position its
   children hit: `handle_mouse` first checks `children.hit_test(pos)` and
   forwards as in 0.3.0 if it hits, else answers for itself. At the first
   three levels the top host never calls it with such an event, so the check
   always misses and costs one hit test.
4. **Overlays in the overlay pass.** `Panel::draw` draws its own box,
   headers and glow, then `children.draw_controls`; `Panel::draw_overlay`
   (new) calls `children.draw_control_overlays`. A child's tooltip is now
   drawn in the outer host's second pass, above every control added after
   the panel. Fixes the fourth defect.
5. `Panel::deliver_event` and the child-state accessors stay for direct use
   (4.3).

## 7. `EventResponse`

```rust
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct EventResponse {
    pub handled: bool,
    pub target_index: Option<usize>,      // the top-level control, as in 0.3.0
    pub target_id: Option<Symbol>,        // the top-level control's name, as in 0.3.0
    pub target_part: Option<ControlPart>, // the leaf's part
    pub action: Option<ControlAction>,    // the leaf's action
    pub focused_index: Option<usize>,     // the top-level element of the focus
    pub target_path: Option<ControlPath>, // new: the leaf, through the levels
}
```

`target_index` and `target_id` keep their 0.3.0 meaning (an app that names a
panel still sees the panel), and `target_path` names the control inside.
`#[non_exhaustive]` closes the struct literal outside the crate (answering
plan 02's open question 4), so later fields are additive; apps read fields
and build responses only through `ignored`, `consumed` and `handled`.
`handled` sets `target_path` to the root path of `target_index`; a new
`handled_at(path, ..)` sets a deeper one.

## 8. Cost per event

For a host whose deepest container nesting is `d` (`d ≤ MAX_NESTING`),
compared with 0.3.0 where each container level runs a full `deliver_event`:

| Event | 0.3.0 | This plan |
|---|---|---|
| pointer event over a nested control | one routing pass per level (filters, scan, hover, grab, focus, handler) | one scan per level crossed, ≤ `2d` `set_hover`, one handler |
| `Move` over a nested control | stops at the top level (children never hear it) | one scan per level, hover along the path, no handler |
| key to a nested control | one routing pass per level | `d` `with_children` calls, one handler |
| `Tab` | top level only | pre-order walk until a stop |

No per-event work is proportional to the number of controls except the
hit-test scan at each level, as in plan 02.

## 9. Modals are not containers

`ModalStack` keeps its own routing: it receives every event by redirect and
hands it to the top `Modal`, whose content host runs the dismissal rules
(Escape, Back, tap outside, close controls) and needs to see the content's
`EventResponse` to recognise a close control. Making it a container would
move those rules into the top host. It is left as it is: it does not
implement `with_children`, so the top host treats it as a leaf, and the
redirect delivers to it as today. A `Panel` inside a modal's content is a
container of the content host, which is itself a `ControlHost` and routes
through it as this plan describes.

## 10. Compatibility

**Breaking (Rule 5.4):**

- `EventResponse` gains `target_path` and becomes `#[non_exhaustive]`: a
  struct literal outside the crate no longer compiles.
- `Panel`'s handlers stop forwarding: code that called
  `panel.handle_mouse(..)` directly to reach a child gets the panel's own
  answer (use `Panel::deliver_event`).
- Observable behaviour: a hosted panel's children receive hover on plain
  moves, are told when the pointer leaves the panel, are reached by Tab, and
  their tooltips draw in the overlay pass; Tab order in hosts with panels
  changes accordingly.

**Additive:** `ControlPath`, `MAX_NESTING`,
`HostedControl::with_children`, `ControlHost::{focused_path,
set_focused_path, hovered_path, set_hovered_path}`, `DragGrab::path` and
`DragGrab::at_path`, `EventResponse::handled_at`, `Panel::draw_overlay`.

**Downstream:** an app that reads `target_id` and `action` sees their
0.3.0 meaning unchanged; one that builds `EventResponse` literals must use
its constructors.

## 11. Version

This is not strictly additive, so the release that contains it bumps the
minor version (Rule 5.4): **0.4.0** if it is the first release after 0.3.0.
The libmsdf 0.3.0 move is also a minor bump (Rule 5.5). If both land before
either is released they share one 0.4.0 release, and the last step to land
cuts it with one `VERSIONS.md` entry covering both; if one is released first
as 0.4.0, the other is 0.5.0. Step 7 states which applies when it runs.

## 12. Steps

Each step is one commit on a branch, squash-merged into `main` when the plan
is complete, as 0.3.0 was (CHANGES.md, 2026-10-16 01:37 EDT). Each passes
`cargo fmt -p libcontrols -- --check`, `cargo clippy -p libcontrols
--all-targets` with no warnings and `cargo test -p libcontrols`. New tests go
in `tests/nested.rs`, using the `Recorder` in `tests/common`.

1. **`ControlPath` and `with_children`** (sections 3, 4.1). Add the type, the
   constant, the defaulted method and its `ControlHandle` forwarding; `Panel`
   implements it. No routing change.
   Tests: `a_path_is_copy_and_compares_only_its_keys`,
   `a_path_stops_at_max_nesting`, `parent_first_leaf_and_starts_with`,
   `a_panel_lets_a_visitor_into_its_children`,
   `a_leaf_reports_no_children`.
   **Done when:** those pass and every existing test passes unmodified.

2. **Routes and hit testing through levels** (5.1, 5.2, 5.3). Replace `Slot`
   and `Hovered` with `Route`; `hit_path`; `with_leaf`; pointer events,
   gestures and keys go to leaves; `EventContext` built once for the leaf;
   `Panel` stops forwarding (6.2, 6.3).
   Tests: `a_press_on_a_panel_child_reaches_the_child_and_not_the_panel`,
   `a_press_on_the_panel_body_reaches_the_panel`,
   `a_grab_on_a_nested_control_follows_it_off_its_area`,
   `a_key_reaches_a_focused_nested_control`,
   `a_gesture_reaches_the_nested_control_under_the_pointer`,
   `the_leaf_context_carries_the_modifiers_without_seeding`,
   `nesting_past_max_falls_back_to_forwarding`,
   `the_child_host_runs_no_routing_pass` (a filter registered on the child
   host is never asked).
   **Done when:** those pass, plan 02's and plan 03's tests pass, and in a
   host of `[Panel[Panel[Recorder]]]` a press on the recorder reaches it with
   exactly one `handle_mouse` call in the whole tree.

3. **Hover along the path** (5.4).
   Tests: `a_plain_move_hovers_a_panel_child`,
   `entering_a_child_tells_the_panel_then_the_child`,
   `moving_between_siblings_tells_only_the_two`,
   `leaving_the_panel_tells_the_child_then_the_panel`,
   `a_part_change_inside_tells_the_leaf_once`,
   `hover_is_frozen_under_a_grab_at_any_depth`,
   `at_most_two_calls_per_level_per_event`.
   **Done when:** those pass and plan 02's hover tests pass unmodified.

4. **Focus routes and Tab order** (5.6).
   Tests: `tab_walks_into_a_panel_and_out_again`,
   `backtab_walks_the_same_order_backwards`,
   `a_panel_that_refuses_focus_is_skipped_but_its_children_are_not`,
   `escape_clears_a_nested_focus`,
   `a_press_focuses_a_nested_control_with_events`,
   `focus_events_go_to_leaves_only`.
   **Done when:** in a host of `[Button, Panel(2 Buttons), Button]`, eight
   Tabs from no focus visit outer 0, panel, child 0, child 1, outer 2, then
   outer 0, panel, child 0; and two Escapes from child 1 leave no focus after
   the first.

5. **Carrying paths; `DragGrab::path`; `EventResponse`** (5.5, 5.7, 7).
   Tests: `a_nested_focus_is_carried_across_a_rebuild`,
   `a_nested_hover_is_carried_so_leaving_still_clears`,
   `a_nested_grab_is_carried`, `a_path_that_no_longer_resolves_is_dropped`,
   `the_key_forms_mean_the_top_level_element`,
   `the_response_names_the_panel_and_the_path_names_the_child`.
   **Done when:** those pass and `tests/events.rs` passes with no change
   beyond what `#[non_exhaustive]` on `EventResponse` requires (it builds
   none).

6. **Panel overlays in the overlay pass** (6.4).
   Tests: `a_panel_childs_tooltip_draws_above_a_later_control`,
   `a_panel_draws_its_children_in_two_passes`.
   **Done when:** those pass and the panel draw tests in `src/lib.rs` pass.

7. **README and version.** Update the README's "Input
   routing" section (containers, paths, Tab through panels). Bump the
   version per section 11 and append the `VERSIONS.md` entry in the same
   commit.
   **Done when:** the README example for nested routing is mirrored by a test
   in `tests/nested.rs` and the squash commit on `main` is tagged with the
   new version.

## 13. Open questions

1. **Tell ancestors about focus?** Only leaves get `FocusEvent`s here. A
   container that wants to draw "a child of mine is focused" (a glow on the
   panel) would need `FocusEvent::DescendantGained`/`DescendantLost` or a
   query. Add now, or when a container needs it?
2. **`MAX_NESTING = 4`.** Enough for panel-in-panel-in-modal layouts; a
   fixed array keeps `ControlPath` `Copy` and allocation-free. Raise it, or
   keep the fallback of section 6.3 for anything deeper?
3. **Keep `Panel::deliver_event` and the child-state accessors?** They exist
   for apps that drive a panel without a host. After this plan a hosted panel
   ignores that state. Keep them documented as direct-use only, or deprecate
   them in this release?
4. **Shared release with libmsdf 0.3.0.** Section 11 lets the two share
   0.4.0 if neither has been released. Is that the owner's preference, or
   should each breaking change be its own release?

## Amendment — 2026-10-16 02:29 EDT: superseded by "Persistent control tree and router layers"

Status changes from `Draft` to `Superseded (by [Persistent control tree
and router layers](20261016_05_PERSISTENT_ROUTER_PLAN.md))`. Nothing in this
plan was implemented. The owner moved libcontrols to persistent controls
owned by a router ("we should move to expecting the controls are persistent
and only output per frame"; "router owns them, drop ControlHost, write the
plan"). What carries forward and what does not:

- Goals carried: the top router is the only router of its tree, events go
  straight to the innermost control, hover follows the pointer inside
  panels and is cleared on leaving, Tab walks into and out of panels, and a
  panel's children draw their tooltips in the overlay pass (new plan,
  sections 3.2, 4 and 7).
- Dropped: `HostedControl::with_children`, `ControlPath` and
  `MAX_NESTING` (a `NodeId` addresses any depth), the Panel fallback past
  the fourth level, the carried `focused_path`/`hovered_path` APIs (the
  router keeps its state), and `EventResponse::target_path` (the response
  names a `NodeId`).
- Open questions: 1 (telling ancestors about focus) is answered by the
  router's `FOCUS_WITHIN` state flag (new plan, section 3.3); 2 (`MAX_NESTING`) no longer applies; 3 is
  answered by removing `Panel`'s hosting methods (new plan, section 7); 4 is
  answered: 0.4.0, shared with the libmsdf 0.3.0 move (new plan, section
  11).
