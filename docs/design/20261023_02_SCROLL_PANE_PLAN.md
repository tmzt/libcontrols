---
title: Stateful scroll pane
date: 2026-10-23
status: In implementation
covers: >
  A ScrollPane node whose children are laid out on a content plane larger
  than its viewport. Its state (offset, the offset the user asked for, the
  range, whether it follows the end, a drag and a fling) lives in a
  shareable ScrollState, so an app keeps a pane's place across rebuilds.
  The router clips the children to the viewport, translates them by the
  offset, skips the ones out of view, maps hits and event positions
  through the offset, and decides between a tap and a scroll: a drag past
  the touch slop along the pane's axis moves from the child to the pane,
  which cancels the child's press. Wheel and trackpad input, keyboard
  paging, scrolling the focus into view, fling with decay driven by
  Router::advance, a sticky end, keeping the content under the reader in
  place as content above it changes (scroll anchoring), a scroll
  indicator, the theme values, tests, and the release.
sources:
  - >
    Owner, relayed by another session: "libcontrols should have a stateful
    scroll pane, send the plan".
  - >
    Owner, on a consuming app's scrolling: "the scroll should remember
    where it is".
  - >
    A consuming app's scroll state, built twice by hand because libcontrols
    has none: offset, range and a wanted offset that is kept while the
    window or content is briefly smaller and returned to when there is room;
    a sticky mode that follows the end while at the end and holds still
    once scrolled back, like a terminal; a 10 pt slop between a tap and a
    drag (the tap is returned to the caller to hit-test); a fling whose
    speed decays as e^(-3.2 t), stops below 30 pt/s, and is not started
    when the finger rested 100 ms before lifting; a smoothed velocity
    (0.6 new, 0.4 old); one finger per pane. The second copy also keeps its
    place by anchoring to the entry at the window's top, and to the next
    one still present when that one leaves, as entries are added or removed
    above it. The app clips, culls and maps hits through the offset itself.
  - >
    src/router.rs at 13a9778 (0.7.0): `hit_path`, `deliver_mouse` (the
    press grab, `Grab::start_pos`), `visit_visible` and `draw_node` (the
    two draw passes), `router_at` (a nested router found by downcast),
    `context`, `tab_step`, `change_focus`. src/button.rs and src/label.rs
    (`handle_mouse` reports `Activated` on a left `Down` or a `Click`).
    src/slider.rs, src/arrow_label.rs, src/selectable_label.rs (take
    `MouseEvent::Scroll`). src/event.rs (`MouseEvent::Scroll` and
    `ScrollPixels`; no timestamps on events; the input enums are
    exhaustive). src/theme.rs (`ControlTheme`, public fields, exhaustive).
  - >
    libmsdf 0.6.0 `DrawList::push_clip`/`push_clip_end` (nestable,
    intersected, in screen pixels) and `push_translate`/`push_translate_end`
    (a transform scope, documented as being for a scroll offset; composes
    with enclosing rotations and translates).
  - >
    Material 3 and the platforms it describes: touch slop 8 dp; scroll
    containers intercept a drag past the slop and cancel the child's press;
    activation on release; a scroll indicator that fades when idle; focus
    moved by keyboard is scrolled into view.
related:
  - docs/design/20261023_01_PRESS_FEEDBACK_PLAN.md (Draft; this plan uses
    its `NodeState::PRESSED`, `Router::cancel_press`, dropped `Click` and
    `Router::advance`, and its touch slop; section 9 says what happens if
    it is not implemented first)
  - docs/design/20261016_05_PERSISTENT_ROUTER_PLAN.md (Implemented; the
    router, its tree and nested routers this builds on)
---

# Stateful scroll pane

## 1. Goal

An app should be able to put controls on a plane taller (or wider) than
the space it has, and let the user move through it with a finger, a wheel,
a trackpad or the keyboard, without writing the scrolling itself. Today an
app that needs this keeps its own offset, clips and culls its own drawing,
subtracts the offset from every hit, and tells taps from drags itself.

After this plan:

- a `ScrollPane` is a router node; its children are ordinary nodes, laid
  out as if the pane were as tall as its content;
- the router clips them to the pane's viewport, moves them by the offset,
  draws only those in view, and delivers events to them in their own
  coordinates;
- a drag that starts on a button inside the pane scrolls the pane once it
  passes the touch slop, and the button is no longer pressed and does not
  activate;
- the pane flings, follows the end when asked, remembers where the user
  left it when its content or viewport shrinks for a while, and keeps the
  content the reader is looking at in place when content above it changes;
- its state can outlive the node, so a screen that is torn down and built
  again comes back where it was.

## 2. Findings (read 2026-10-23, at 13a9778)

- **The router already treats containers as plain nodes.** A node's
  children are hit-tested only when the node is hit (`hit_path`), and are
  drawn in pre-order after it (`visit_visible`). There is no notion of a
  child plane: every control's `bounds` and `hit_test` are in screen
  pixels.
- **libmsdf has both pieces of drawing it needs.** `push_clip` takes screen
  pixels, nests and intersects. `push_translate` is a transform scope that
  moves everything drawn inside it, documented as being for a scroll
  offset, and composes with enclosing transforms. A clip pushed before the
  translate stays in screen space, so `push_clip(viewport)` then
  `push_translate(-offset)` clips to the viewport and moves the content.
- **Buttons and labels activate on press.** `Button::handle_mouse` and
  `Label::handle_mouse` return `Activated` for a left `Down` as well as for
  a `Click`. Inside a scroll pane that activates the control under the
  finger at the start of every scroll. Every platform M3 describes
  activates on release, and the press plan already relies on the `Click`
  being what activates.
- **Events carry no time.** A fling needs the finger's speed and whether
  it rested before lifting. The router has no clock; the press plan adds
  `Router::advance(dt_secs)`, which can be that clock.
- **The wheel already goes to controls.** Sliders, arrow labels and
  selectable labels take `MouseEvent::Scroll` to change their value, so a
  wheel over one of them inside a pane must not also scroll the pane.
- **`ControlTheme` is exhaustive**, so a new field is a breaking change
  (the press plan adds one too).
- **One pointer per router.** libcontrols has no touch identifiers; the
  router keeps one grab per mouse button. The app that keeps "one finger
  per pane" does so because it has several panes on one screen; with one
  router per screen, the router's grab already says which pane a touch
  belongs to. Multi-touch is out of scope.

## 3. Design

### 3.1 Coordinates

A pane has a **viewport**, its rectangle on the screen, and a **content
plane**. The children of a pane are laid out in content coordinates whose
origin is the viewport's top-left corner at offset zero: an app lays out a
list as if the pane were a page starting where the pane starts, and never
moves its children to scroll. At offset `[ox, oy]`, a point `p` on the
screen is the content point `p + [ox, oy]`.

Panes nest. A node's content coordinates are its screen coordinates plus
the sum of the offsets of every pane above it. Everything a control
already reports or receives (`bounds`, `hit_test`, the positions in its
`MouseEvent`s, where it draws) is in its own content coordinates, so no
existing control changes.

### 3.2 `ScrollState`

```rust
/// One pane's place. Shared (`Arc`) between the pane and the app, so the
/// place survives the pane being removed and added again.
pub struct ScrollState { /* Cells; Send + Sync like PanelState */ }

impl ScrollState {
    pub fn new() -> Arc<Self>;
    /// Follows the end while at it: a log or a chat.
    pub fn sticky(edge: ScrollEdge) -> Arc<Self>;
    pub fn offset(&self) -> [f32; 2];
    /// The largest offset on each axis: content size minus viewport size,
    /// at least 0.
    pub fn max(&self) -> [f32; 2];
    pub fn is_following(&self) -> bool;
    /// Whether a drag or a fling is moving it.
    pub fn is_moving(&self) -> bool;
}

#[non_exhaustive]
pub enum ScrollEdge { Start, End }
```

It holds:

- `offset: [f32; 2]`, what is drawn;
- `wanted: [f32; 2]`, where the user (or the app) last put it. When the
  range shrinks below it, `offset` is clamped and `wanted` is kept; when
  the range grows again, `offset` returns to `wanted`;
- `max: [f32; 2]`;
- `sticky: Option<ScrollEdge>` and `following: bool`. A sticky-End pane
  starts following. While following, every change of range sets `wanted`
  to the end. A scroll by the user that leaves the end stops following; one
  that reaches within 1 px of it starts following again;
- the drag: the node it began on, the content point it started at, the
  recent samples (position and router time, the last 100 ms), and whether
  it has passed the slop;
- the fling: velocity in px/s on the pane's axis;
- the anchor (section 3.7) and the indicator's visibility (section 3.8).

### 3.3 `ScrollPane`

```rust
pub struct ScrollPane { /* viewport: Cell<Rect>, axis, state, options */ }

impl ScrollPane {
    pub fn new(viewport: Rect, axis: ScrollAxis) -> Self;
    pub fn with_state(self, state: Arc<ScrollState>) -> Self;
    /// The content's size, when the app knows it. Without it, the content
    /// is the union of the children's bounds, from the content origin.
    pub fn with_content_size(self, size: [f32; 2]) -> Self;
    pub fn with_anchoring(self, on: bool) -> Self;   // default on
    pub fn with_indicator(self, on: bool) -> Self;   // default on
    pub fn with_background(self, color: [f32; 4]) -> Self;
    pub fn state(&self) -> &Arc<ScrollState>;
    pub fn set_viewport(&self, viewport: Rect);
    pub fn set_content_size(&self, size: Option<[f32; 2]>);
}

#[non_exhaustive]
pub enum ScrollAxis { Vertical, Horizontal }
```

- `bounds` and `hit_test` are the viewport, in the pane's own (parent's
  content) coordinates. `default_flags` is `NodeFlags::NONE`: the pane is
  not a Tab stop; its children are.
- `draw` fills the background, if any. The children and the indicator are
  drawn by the router (section 3.4).
- The router finds a pane the way it finds a nested router today: by
  downcasting the node's control (`scroll_at`, beside `router_at`).

The router's API for panes:

```rust
impl Router {
    /// Scroll a pane to `offset` (clamped), stopping any fling. Sets
    /// `wanted`, and following as a user scroll would.
    pub fn scroll_to(&self, pane: NodeId, offset: [f32; 2]);
    pub fn scroll_by(&self, pane: NodeId, delta: [f32; 2]);
    /// Scroll every pane above `node` the least that brings its bounds
    /// into view (all of it when it fits, else its start).
    pub fn scroll_into_view(&self, node: NodeId);
    /// A screen point in `node`'s content coordinates.
    pub fn to_content(&self, node: NodeId, screen: [f32; 2]) -> [f32; 2];
    /// `node`'s bounds on the screen, clipped by every pane above it;
    /// `None` when none of it is visible.
    pub fn visible_bounds(&self, node: NodeId) -> Option<Rect>;
}
```

**The range** is brought up to date by `Router::advance` and at the start
of `render`: from `with_content_size` or the children's bounds, minus the
viewport, then the offset is set from `wanted` (or the end, while
following) and clamped, then anchoring is applied (section 3.7). It is
computed only for panes whose subtree is visible.

### 3.4 Drawing

In the draw pass, when the router reaches a visible pane:

1. it calls the pane's `draw` (background);
2. `list.push_clip(viewport on the screen)`, then
   `list.push_translate(-ox, -oy)`;
3. it draws the children's subtrees, skipping any child whose bounds do
   not intersect the visible content rectangle (the viewport plus the
   offset), with its subtree. A node's descendants are expected to lie
   within its bounds, which holds for panels and nested routers (whose
   bounds are the union of their roots');
4. `push_translate_end`, then `push_clip_end`;
5. the indicator (section 3.8), in screen space.

In the overlay pass, a pane's children are translated the same way but
**not clipped**, so a tooltip of a control near the viewport's edge can
extend past it. Overlays of children out of view are skipped by the same
test.

The disabled half-alpha of `draw_node` is unchanged; it applies to what
each node pushed, inside the scopes.

`push_clip` and `push_translate` scopes must be closed before
`DrawList::append` is used on the list, which the router's passes already
guarantee by closing each pane's scopes before leaving it.

### 3.5 Hits and event positions

- `hit_path` descends into a pane's children only when the point is inside
  the viewport, and tests them with the point mapped into the pane's
  content coordinates. A point inside the viewport that hits no child hits
  the pane.
- Every `MouseEvent` delivered to a node is mapped into that node's content
  coordinates: `pos`, and a `Drag`'s `start_pos` and `current_pos`
  (`delta` is unchanged). The mapping uses the offsets at delivery time, so
  a drag on a slider inside a pane that is still flinging stays under the
  finger. A nested router under a pane receives mapped events and routes
  them in its own tree as it does now.
- `Router::pointer()` stays in screen coordinates. `DragGrab::start_pos`
  stays in screen coordinates; `Router::to_content` maps it.

### 3.6 Input

**Tap or scroll.** A left `Down` inside a pane is routed as today: the node
under it gets the grab, and with the press plan, `PRESSED`. The router also
starts a candidate drag on every pane above that node. Each `Drag` of that
press:

1. while the movement from the press point is within the touch slop, goes
   to the grabbed node only, as today;
2. when it first passes the slop, the router picks the innermost pane above
   the grabbed node that can move along the dominant axis of the movement
   (vertical when `|dy| >= |dx|`) in that direction, unless the grabbed
   node **claims the drag**;
3. if a pane is picked, the router takes the press from the node: it
   delivers an `Up` for the press button at the current point to the node,
   so any bookkeeping it keeps ends; clears `PRESSED` and starts its fade
   (the press plan's cancel); drops the `Click` that ends this press; and
   moves the grab to the pane. From then on the pane follows the finger
   along its axis, starting from the point where the slop was passed, so
   the content does not jump by the slop;
4. if no pane is picked (the node claims the drag, or no pane can move
   that way), the drag stays with the node, as today.

**Claiming a drag.** A new provided method on `HostedControl`:

```rust
/// Whether a drag that began on `part` belongs to this control even when
/// it moves along a scroll pane's axis.
fn claims_drag(&self, _part: ControlPart) -> bool { false }
```

`Slider` and `SliderNumeric` claim drags that start on the thumb or track;
`Edit` claims drags (text selection). Buttons, labels, radios and option
cells do not, so a list of them scrolls from anywhere.

**Activation on release.** `Button` and `Label` stop returning `Activated`
for `Down`; they return it for the `Click` only. Every other control's
behaviour is unchanged. This is what lets a scroll that starts on a button
not activate it, inside a pane or not.

**Lift and fling.** On the `Up` of a press the pane holds, the drag ends.
If the finger moved within the last `rest` seconds of router time, the
pane flings at the velocity of its recent samples (displacement over the
last 100 ms of samples, so several events within one frame count as one
sample), clamped to `max_fling`, if that is above `min_fling`. The `Click`
of that press is dropped: a scroll that ends over a button does not press
it. Each `Router::advance(dt)` (with `dt` capped at 0.1 s) moves a flinging
pane by `v * dt`, then `v *= e^(-decay * dt)`; the fling stops below
`min_fling`, at either end of the range, when following, or at a new
`Down` in the pane (which grabs the pane, not the child under it, while the
fling was above `min_fling`: a touch that stops a fling is not a tap).
`advance` returns true while any pane is flinging or its indicator is
fading, so the host keeps drawing frames.

**Wheel and trackpad.** A `Scroll` or `ScrollPixels` goes first to the node
under the pointer, as today. If that node returns no action, the innermost
pane above it that can move that way moves: `ScrollPixels` by its delta,
`Scroll` by `delta * wheel_line` pixels. Positive `dy` moves the content
down (towards its start), as winit reports it. A wheel stops any fling and
does not start one; trackpads deliver their own momentum as more
`ScrollPixels`.

**Keyboard.** When the focused node returns no action for a key, the
innermost pane above it takes `Up`/`Down` (or `Left`/`Right` on a
horizontal pane) by `wheel_line`, `PageUp`/`PageDown` by the viewport
less one `wheel_line`, and `Home`/`End`. A pane with no focused descendant
takes no keys; it is not a Tab stop.

**Focus into view.** When the focus moves to a node inside a pane by Tab,
BackTab or `set_focus`, the router calls `scroll_into_view` on it. A press
does not, since the pressed node is already on the screen.

**Host cancel.** `Router::cancel_press` also ends a pane's drag, without a
fling.

### 3.7 Keeping the reader's place (anchoring)

When a pane is not following and its content changes above what the
reader sees, the content they see should not move. At the end of each
range update (section 3.3) the router records the pane's **anchors**: the
`NodeId`s of its direct children whose bounds intersect the viewport, in
order from the leading edge, each with its leading edge's distance from
the viewport's leading edge. At the next range update, before clamping, it
finds the first anchor that is still a child of the pane and moves
`offset` and `wanted` by however much that child's leading edge moved in
content coordinates. If none is left, nothing moves.

So inserting a child above the view, growing one above it, or removing the
one at the top (whose successor is then the anchor) leaves the visible
content where it is on the screen. Anchoring is skipped during a drag (the
finger holds the content) and while following (the end holds it), and is
off with `with_anchoring(false)`. A child that wants to be the anchor
regardless can be kept in view with `scroll_into_view`.

A pinned element that must not move with the content (a card at the
bottom of a chat) is not a child of the pane: the app places it outside
and shrinks the viewport, which `wanted` already handles.

### 3.8 The indicator

A bar along the trailing edge of the viewport (right for vertical, bottom
for horizontal), `indicator_width` thick and inset by `indicator_inset`,
its length the viewport's share of the content (at least
`indicator_min_length`) and its position the offset's share of the range.
It is drawn only when the content is larger than the viewport. It shows at
full `indicator_color` alpha while the pane moves, stays for
`indicator_idle` seconds after it stops, then fades over `indicator_fade`,
on router time. It is not a control and takes no input; a draggable
scrollbar is a later plan.

### 3.9 Theme

`ControlTheme::scroll: ScrollTheme`, `#[non_exhaustive]` with a `Default`:

| Field | Default | From |
|---|---|---|
| `touch_slop` | 8.0 px | M3 8 dp; the same value as the press plan's `PressTheme::touch_slop` (section 9) |
| `wheel_line` | 48.0 px | three 16 px lines per notch |
| `fling_decay` | 3.2 /s | the consuming app's tuned value |
| `min_fling` | 30.0 px/s | the same |
| `max_fling` | 8000.0 px/s | Android's default maximum fling velocity, 8000 dp/s |
| `rest` | 0.100 s | the same as the app's 100 ms rest |
| `indicator_width` | 4.0 px | M3 |
| `indicator_inset` | 2.0 px | |
| `indicator_min_length` | 24.0 px | |
| `indicator_color` | the theme's `muted_text_color` at 50% alpha | |
| `indicator_idle` | 0.5 s | |
| `indicator_fade` | 0.150 s | M3 short3 |

Values are in the router's pixels; an app drawing at a scale multiplies
them, as it does the other theme values.

## 4. What is out of scope

- Scrolling on both axes in one pane (`ScrollAxis` is non-exhaustive so it
  can be added), and diagonal drags.
- Overscroll effects: M3's stretch, or a bounce. The pane stops at its
  ends.
- A draggable scrollbar, and snapping to items or pages.
- Virtualised lists, where children are created only as they come into
  view. Culling skips drawing children out of view, but they are still
  nodes; an app with thousands of rows adds and removes them itself, and
  anchoring keeps its place while it does.
- Multi-touch: one pointer per router, as today.

## 5. Tests

- **State** (src/scroll.rs): the range from a content size and from the
  children's bounds; `wanted` kept when the range shrinks and returned to
  when it grows; sticky End follows growth, stops following when scrolled
  back, follows again at the end; `scroll_to` clamps.
- **Drawing**: a pane's children are drawn inside one clip (the viewport
  on the screen) and one translate (minus the offset); a child out of view
  pushes nothing; a child's overlay is translated and not clipped; nested
  panes intersect their clips and compose their translates.
- **Hits**: a point in the viewport hits the child at the content point;
  a point outside the viewport does not reach the children, even where a
  child's content bounds would be; events delivered to a child carry
  mapped positions; nested panes add their offsets.
- **Tap or scroll**: a `Down`, a `Drag` within the slop and an `Up` and
  `Click` on a button inside a pane activate it once (on the `Click`); the
  same with a `Drag` past the slop scrolls the pane by the movement past
  the slop, clears `PRESSED`, delivers the button an `Up`, drops the
  `Click` and does not activate; a horizontal drag in a vertical pane stays
  with the child; a drag on a slider's thumb stays with the slider; an
  inner horizontal pane takes a horizontal drag inside an outer vertical
  one, the outer takes a vertical drag.
- **Fling**, with `advance` as the clock: a fast drag and lift flings and
  decays to a stop within the range; a drag that rests 100 ms before lift
  does not fling; a `Down` during a fling stops it and grabs the pane.
- **Wheel and keys**: a wheel over a button scrolls the pane; over a slider
  it moves the slider and not the pane; PageDown moves by the viewport
  less one line; Tab to a child below the view scrolls it into view.
- **Anchoring**: inserting a child above the view keeps the visible
  child's screen position; removing the top visible child anchors to the
  next; nothing moves while following or with anchoring off.
- **Button and Label**: activate on `Click`, not on `Down`, outside panes
  too.
- **GPU check** (behind the existing GPU test feature, if the crate has
  one, else an example): a pane of labels, scrolled, draws nothing outside
  its viewport and draws the expected rows inside it.

## 6. Versioning (Rule 5)

Not strictly additive, so a minor bump:

- **Breaking:** `ControlTheme` gains `scroll` (exhaustive struct);
  `Button` and `Label` no longer activate on `Down`; events delivered to
  nodes inside a pane carry content coordinates (new behaviour, but only
  for nodes in a pane, which cannot exist before this release).
- **Added:** `ScrollPane`, `ScrollState`, `ScrollAxis`, `ScrollEdge`,
  `ScrollTheme`, `HostedControl::claims_drag` (provided), the `Router`
  methods of section 3.3.

If the press plan is released first as 0.8.0, this is 0.9.0. If both are
approved before either is released, they can go out together as 0.8.0
(section 9). No libmsdf change is needed; it stays at 0.6.0.

## 7. Steps

1. **`ScrollState`, `ScrollAxis`, `ScrollEdge`, `ScrollTheme`** in
   src/scroll.rs, with the range, `wanted`, sticky and clamping logic and
   its unit tests; `ControlTheme::scroll`.
   **Done when:** the state tests of section 5 pass.
2. **`ScrollPane` and drawing**: the control, `scroll_at`, the range
   update in `render`, clip and translate around the children, culling,
   unclipped overlays.
   **Done when:** the drawing tests pass, and a scrolled pane set by
   `scroll_to` draws only what is in view.
3. **Hits and mapped events**: `hit_path` through panes, mapped
   `MouseEvent`s, `to_content`, `visible_bounds`.
   **Done when:** the hit tests pass.
4. **Activation on release**: `Button` and `Label` activate on `Click`
   only.
   **Done when:** their tests pass and no other control's tests change.
5. **Tap or scroll**: candidate drags, the slop, the dominant axis,
   `claims_drag` (Slider, SliderNumeric, Edit), taking the press from the
   child, the dropped `Click`, `cancel_press` ending a drag.
   **Done when:** the tap-or-scroll tests pass.
6. **Fling and the clock**: samples on router time, `advance` moving
   flings, stopping rules.
   **Done when:** the fling tests pass.
7. **Wheel, keys and focus into view**, and `scroll_by`/`scroll_into_view`.
   **Done when:** the wheel and key tests pass.
8. **Anchoring.**
   **Done when:** the anchoring tests pass.
9. **The indicator**, with its fade on router time.
   **Done when:** a test sees it drawn while moving, kept for
   `indicator_idle`, faded after `indicator_fade`, and absent when the
   content fits.
10. **GPU check, README usage section, VERSIONS.md and the release**
    (section 6), tagged.
    **Done when:** fmt, clippy and every test pass, and the tag is on the
    release commit.

## 8. How a consuming app moves onto it

An app that keeps its own offset, wanted offset, sticky flag, drag and
fling replaces them with a `ScrollPane` and an `Arc<ScrollState>` it keeps
across rebuilds; it lays its items out from the pane's origin and stops
offsetting them, clipping them and mapping hits. A tap it used to detect
itself is now the item's own `Click`. Its own anchoring to the entry at the
top becomes the pane's anchoring, provided each entry is a child node of
the pane. A pinned card outside the content stays outside the pane.

## 9. Open questions

1. **Order with the press plan.** This plan uses the press plan's
   `PRESSED`, cancel, dropped `Click`, `advance` and touch slop. If the
   press plan is implemented first, this plan uses them as they are and
   adds `ScrollTheme` without its own `touch_slop`, reading
   `PressTheme::touch_slop` instead (the table above then loses a row). If
   this plan goes first, it adds `Router::advance` and the cancel itself,
   and the press plan builds on them. Either way there should be one slop.
   Which order, and one release or two?
2. **Activation on release** changes `Button` and `Label` everywhere, not
   only in panes. Is that wanted, or should they keep activating on `Down`
   outside panes?
3. **Wheel precedence.** A wheel over a slider inside a pane moves the
   slider. Some platforms scroll the pane unless the slider is focused.
   Keep "the node under the pointer first"?
4. **Overscroll.** Stop dead at the ends (this plan), or M3's stretch?
5. **Fling constants**: keep the consuming app's tuned decay (3.2/s) and
   minimum (30 px/s), or Android's (a friction-based spline)?
6. **Anchoring by default**: on (this plan, like the web's scroll
   anchoring) or opt-in?

## Amendment — 2026-10-23 21:12 EDT: approved, and the open questions settled

The owner approved this plan and its implementation ("approve plan and
impl", 2026-10-23). The open questions of section 9 are settled as the plan
already proposes, and this replaces section 9 and the conditional parts of
sections 3.6, 3.9 and 6:

1. **This plan goes first,** before the press feedback plan (still Draft).
   It adds `Router::advance(dt_secs: f32) -> bool` and the touch slop as
   `ScrollTheme::touch_slop` (8 px), and keeps that row of section 3.9's
   table. With no `NodeState::PRESSED` yet, taking the press from a child
   (section 3.6, step 3) is the `Up` delivered to it, the dropped `Click`
   and the moved grab; clearing `PRESSED` and starting its fade join it when
   the press plan is implemented, which then builds on this plan's
   `advance` and slop. `Router::cancel_press` likewise arrives with the
   press plan; until then a host ends a pane's drag by its `Up`. This
   plan's release is its own, **0.8.0** (section 6's "0.9.0 after the press
   plan" no longer applies).
2. **Activation on release everywhere:** `Button` and `Label` activate on
   `Click` only, inside panes and outside them.
3. **Wheel precedence:** the node under the pointer first; the pane only
   when that node returns no action.
4. **Overscroll:** none; the pane stops at its ends.
5. **Fling constants:** decay 3.2 /s, minimum 30 px/s, as in section 3.9.
6. **Anchoring:** on by default, off with `with_anchoring(false)`.

## Amendment — 2026-10-23 21:14 EDT: the wheel as a two-finger drag, and optional overscroll

The owner, during implementation: "wheel scrools contents same as two
finger e4ag on supported hw" and "overscroll px/pct is optional, snap like
iphone if enabled". This replaces section 3.6's **Wheel and trackpad**
paragraph, section 4's overscroll item and item 4 of the 21:12 amendment.

**Wheel and trackpad.** A wheel (`MouseEvent::Scroll`) moves the content
exactly as a two-finger drag on a trackpad (`MouseEvent::ScrollPixels`)
does: the same direction and the same routing, a wheel notch counting as
`wheel_line` pixels. Positive `dy` (or `dx` on a horizontal pane)
moves the content towards its start, as winit reports both. Either goes
first to the node under the pointer and to the innermost pane that can
move when that node returns no action; either stops a fling and starts
none. Neither overscrolls: a trackpad's own momentum arrives as more
`ScrollPixels` with no phase, so there is no lift to spring back on.

**Overscroll, optional.** `ScrollPane::with_overscroll(Overscroll)`:

```rust
#[non_exhaustive]
pub enum Overscroll {
    /// Stops at the ends (the default).
    None,
    /// Up to this many pixels past either end.
    Px(f32),
    /// Up to this share of the viewport's length past either end.
    Fraction(f32),
}
```

When enabled, a finger drag past an end moves the content past it with
increasing resistance, as on iOS: the content's displacement past the end
is `d * (1 - 1 / (x * 0.55 / d + 1))` for a finger `x` pixels past it,
with `d` the limit, so it never reaches `d`. Dragging back takes up the
overscroll before the offset moves. On lift, and when a fling reaches an
end with speed left (carrying that speed in), the content springs back to
the end as a critically damped spring, `ScrollTheme::overscroll_spring`
(an angular frequency, default 12 rad/s, settling in about 0.4 s), on
`Router::advance`; it snaps to the end once within 0.5 px and slower than
`min_fling`. The overscroll is part of the drawn offset, so hits follow
the content. `ScrollState::overscroll() -> f32` reports it (negative
before the start, positive past the end). A sticky pane that is following
does not overscroll past its end. Without `advance`, the content returns
to the end at the lift.

Section 6 gains `Overscroll` and `ScrollTheme::overscroll_spring`
(additive within this release). Section 5 gains: a drag past the end of an
overscrolling pane moves it less than the finger and never past the limit;
it springs back to the end over `advance`; a fling into the end bounces
back to it; a pane without overscroll stops dead.

## Amendment — 2026-10-23 21:26 EDT: implementation started; this document is now immutable

Implementation has started, as approved at 21:12. From here this document
changes only on the owner's explicit instruction; departures go to
CHANGES.md (Rule 1.7).
