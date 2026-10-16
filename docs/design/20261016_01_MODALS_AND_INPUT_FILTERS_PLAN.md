---
title: Modals and input filters
date: 2026-10-16
status: Draft
covers: >
  An input filter stack for ControlHost (a defaulted HostedControl::intercept
  asked last-added control first, so any hosted control can capture an
  event), and a modal stack built on it as an ordinary hosted control,
  ModalStack, with a persistent ModalState (a stack of up to four modals,
  per-entry transitions, events), per-build Modals with app-drawn frames,
  backdrops, dismiss rules and a hold-open veto, plus Key::Back. Also the
  filter's use in Panel (hover inside panels, opt-in Tab and Escape
  traversal), what it gives a future tabbed container, tests, and the 0.3.0
  compatibility break.
supersedes: docs/design/20261015_01_MODAL_PLAN.md
sources:
  - docs/design/20261015_01_MODAL_PLAN.md (Modal layer, with its amendment "2026-10-15 23:31 EDT: Key::Back and a modal stack"; superseded by this plan, which carries its content forward)
  - "Owner (Noble's Keep): \"any UI will be limited to modals, and only if invoked via icons on the screen, such as interlocking gears on the bottom row opening the settings.\""
  - "Owner: \"Back should be Key::Back, we can note the change in CHANGES.md in the repo. Both should close the modal though, so the mapping shouldn't be a problem. We should also add a modal stack so each app doesn't need to implement it.\""
  - "Owner: \"modal needs to be modal, it should trap input events, if we want a non-modal dialog we can add that later. veto callback makes sense if libcontrols has a good way to add one, or save it for later if not (such as a default trait method), wheel/finger based scrolling should be a plan, not in this one. ... as for the ControlActions part I think that's fine.\""
  - "Owner: \"web usage usually has back/esc dismiss a modal, and we now have veto support if we want to protect changes\""
  - "Owner: \"Q8 isn't libcontrols concern at all. I think the base UI is more like level 0 ... I don't see why we need to resync or add events to reset the input state.\""
  - "Owner: \"similar to how we have a panel which forwards input events couldn't a ModalStack control do the same?\""
  - "Owner: \"would this work like an input filter stack (similar to Windows filter devices)\""
  - "Owner: \"split modal and this filter stack to a new plan, also review how it would affect/improve panel and tabbed controls\"; \"since the old plan was also a modals plan, this supercedes\""
  - src/host.rs (HostedControl, ControlHandle forwarding, ControlHost routing and draw)
  - src/panel.rs (Panel and its child ControlHost)
  - src/event.rs (InputEvent, MouseEvent, KeyboardEvent, Key, Modifiers, ControlAction, EventResponse)
  - src/traits.rs (DrawControl, draw_overlay, DrawContext)
  - src/button.rs, src/options.rs, src/radio.rs, src/tooltip.rs (draw_overlay tooltips, ButtonState::take_click, Radio as a tab strip)
  - src/rect.rs, src/theme.rs, src/handle.rs, src/lib.rs
  - tests/events.rs (test style, Legacy and Recorder controls)
  - ~/src/tinytrowel/crates/trowel-game/src/ui.rs (per-event host rebuild, Arc state, take_click, Panel use)
  - ~/src/tinytrowel/deps/libmsdf at rev 69e6c5c (src/drawlist/mod.rs: painter order, push_clip, push_translate, no push_scale; src/gpu/sdf_render.wgsl: how instance alpha, animation alpha and elevation shadows combine)
related:
  - docs/design/20261015_01_MODAL_PLAN.md (superseded by this plan)
  - ~/src/NoblesKeep/docs/design/20261015_01_INITIAL_UI_PLAN.md (Noble's Keep initial UI plan; the main consumer)
---

# Modals and input filters

## 1. Purpose and scope

Noble's Keep shows no free-standing UI. Its owner's rule is that "any UI will
be limited to modals, and only if invoked via icons on the screen, such as
interlocking gears on the bottom row opening the settings." The game draws its
own tabletop (books, tool tiles, the gear icon) directly with `libmsdf`;
libcontrols is used only for the controls inside a modal.

libcontrols 0.2.0 has no modals, and no control can take input it was not
routed by the host. This plan supersedes the modal plan
(`20261015_01_MODAL_PLAN.md`). Instead of building a modal layer into
`ControlHost`, it adds one general mechanism to the host and builds modals on
top of it as an ordinary hosted control:

- **Input filters.** `HostedControl::intercept`, a defaulted method every
  hosted control is asked before normal routing, last-added first. A control
  that answers `Capture` gets the event alone. This is a filter stack in the
  sense of Windows filter drivers, reduced to complete-or-pass.
- **`ModalStack`**, a hosted control added last to a host. While its
  `ModalState` holds any modal it captures every event, so the base UI is
  simply level 0 of the stack, and it draws in `draw_overlay`, above every
  base control and overlay.
- **`ModalState`**: app-owned, `Arc`-shared, persisting across host rebuilds:
  a stack of up to four modals with per-entry phase, transition progress,
  focus and drag, and a queue of `ModalEvent`s.
- **`Modal<'a>`**, built per host build like `Panel`: content controls, frame
  and content rects, the app's frame painter (`ModalFrame`), backdrop,
  dismiss rules, close controls and a hold-open veto.
- **`Key::Back`**, the platform back action, which dismisses like Escape.
- **`Panel`** uses the filter to fix hover inside panels and, opt-in, to let
  Tab and Escape traverse its children.

Out of scope: non-modal dialogs (a later plan; the filter supports them,
section 4.3), scrolling content by wheel or finger (a later plan; until then
modal content fits its content rect or the game pages it), a tabbed
container (section 12), rewriting events in filters, new visual control
types, any game-specific look, and how any app consumes the crate.

## 2. Constraints from the code

Read from the code at `bb865f7`; they fix the shape of the design.

1. **Hosts are rebuilt per event and per frame.** `ControlHost<'a>` borrows
   its controls. tinytrowel builds a fresh host for every pointer event and
   drops it. Anything that must survive between events (which modals are
   open, their progress, focus and drag inside them) lives in an `Arc` state
   object, as `ButtonState`, `SliderNumericState` and `PanelState` do.
2. **Controls report through state and actions.** `ButtonState::take_click`
   is take-once; `EventResponse::action` carries a `ControlAction`. Modal
   events are take-once too (`ModalState::take_events`), because `Opened` and
   `Closed` come from time passing, not from an input event.
   `ControlAction` gains no modal variant.
3. **Routing today.** `deliver_mouse` records the pointer, updates hover on
   every control (unless a drag is active), forwards to the drag holder,
   **returns early for `Move`** (host.rs:433), then hit-tests last to first.
   A control whose `hit_test` returns `None` passes the event down: for
   pointer events the host is already a filter chain that can only pass on a
   miss. `deliver_keyboard` takes Tab, BackTab and Escape for itself
   (host.rs:469, 493) and sends other keys to the focused control. Gestures go
   to the control under the pointer; modifiers to every control.
4. **Draw order is painter's order.** `ControlHost::draw` draws every
   control's `draw`, then every control's `draw_overlay`. `DrawList` has no z;
   `DrawList::instances` is public, each `SdfInstance` carries a
   straight-alpha `color`. `push_clip` and `push_translate` exist; there is no
   `push_scale` and no group alpha.
5. **The library has no clock.** Transitions are advanced by the app
   (`ModalState::advance(dt)`) or driven outright (`set_progress`).
6. **`Panel`** forwards `handle_mouse`, `handle_keyboard`, `handle_gesture`
   and `set_modifiers` to a child `ControlHost` (panel.rs:449 on) and does not
   override `set_hover`. From point 3: hover inside a panel follows only
   presses and drags, not plain moves, and stays set after the pointer leaves;
   Tab steps over a panel's children; Escape clears only the outer focus.
7. **`Panel::draw` draws its child host whole** (panel.rs:445), overlays
   included. A modal clips its content but not its content's tooltips, so it
   needs the two passes separately (`ControlHost::draw_controls` and
   `draw_control_overlays`, section 9).
8. **`Key` is exhaustive** today, so adding `Key::Back` breaks downstream
   exhaustive matches: this release is 0.3.0 (section 5).

## 3. Concepts

- **Level**: the base host's own controls are level 0; each modal on the
  stack is one level above. Only the top level receives input.
- **Viewport**: the surface the host draws to, in input-event pixels. Each
  modal's backdrop covers it.
- **Frame rect**: the area a modal visibly occupies. A press outside it is a
  tap outside. The app's `ModalFrame` may refine this shape.
- **Content rect**: where the modal's controls are laid out (absolute
  coordinates, as for `Panel` children).
- **Phase**: `Closed`, `Opening`, `Open`, `Closing`, per entry.
- **Progress**: `0.0` closed to `1.0` open, per entry.
- **Top**: the uppermost entry in `Opening` or `Open`. Entries in `Closing`
  are on their way out and are never the top.
- **Capture**: while the stack holds any entry, `ModalStack` captures every
  event. Lower levels, base included, receive nothing; only the top's content
  receives input, and only while the top is `Open`.

## 4. Input filters

### 4.1 Types

New in `src/event.rs`, re-exported from `src/lib.rs`:

```rust
/// A filter's answer for one event.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum Intercept {
    /// Not mine: the host asks the next control down, then routes as usual.
    #[default]
    Pass,
    /// Mine: the host delivers this event to this control alone (4.3).
    Capture,
}

/// What the host knows about a control when it asks it to filter an event.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct InterceptContext {
    /// This control holds the host's focus.
    pub focused: bool,
    /// Pointer events: this control is the topmost hit at the event's
    /// position (the one normal routing would choose). False otherwise.
    pub hit: bool,
    /// This control holds the host's drag grab.
    pub dragging: bool,
    /// Another control holds the host's drag grab.
    pub drag_elsewhere: bool,
    /// The host's pointer position, after recording this event.
    pub pointer: Option<[f32; 2]>,
    /// The host's modifiers, after recording this event.
    pub modifiers: Modifiers,
}
```

Outside the crate (including `tests/`), a context is made with
`InterceptContext::default()` and assigned field by field.

New defaulted method on `HostedControl`, forwarded by
`impl HostedControl for ControlHandle`:

```rust
/// Asked before normal routing, last-added control first. Return Capture to
/// receive the event alone. May update the control's own state. Default: Pass.
fn intercept(&self, _event: &InputEvent, _ctx: &InterceptContext) -> Intercept {
    Intercept::Pass
}
```

New methods on `ControlHost`:

```rust
/// set_hover(None) on every control; used by containers when the pointer leaves them.
pub fn clear_hover(&self);
/// Pass 1 only: every control's draw.
pub fn draw_controls(&self, list: &mut DrawList, ctx: &DrawContext);
/// Pass 2 only: every control's draw_overlay.
pub fn draw_control_overlays(&self, list: &mut DrawList, ctx: &DrawContext);
```

`impl DrawControl for ControlHost` becomes `draw_controls` then
`draw_control_overlays`, which is what it does today.

### 4.2 The filter pass

Each of `deliver_mouse`, `deliver_keyboard`, `deliver_gesture` and
`deliver_modifiers` (and so `deliver_event`) begins:

1. Record the pointer (mouse) or modifiers, as today.
2. Pointer events: `top = self.hit_test(pos).map(|h| h.control_index)`;
   others: `None`.
3. For `i` from the last control down to 0, build the context (`focused =
   focused_index() == Some(i)`, `hit = top == Some(i)`, `dragging` and
   `drag_elsewhere` from `active_drag`, `pointer`, `modifiers`) and ask
   `controls[i].intercept(&event, &ctx)`. On the first `Capture`, return
   `self.deliver_captured(i, event)`.
4. No capture: continue exactly as 0.2.0.

Cost: one `intercept` call per control per event and one extra `hit_test`
for pointer events. Not measured; hosts hold tens of controls.

### 4.3 Delivering a captured event (`deliver_captured(i, event)`, private)

A captured event never changes the host's focus and never starts a host drag
grab: the capturer keeps its own focus and drags inside itself (a modal in
its `ModalState` entry, a panel in its child host). So when a modal closes,
base focus is wherever it was when the modal opened.

- **Mouse.** `part = controls[i].hit_test(pos).map(|(p, _)| p).unwrap_or(Body)`.
  Every other control gets `set_hover(None)`; control `i` gets
  `set_hover(controls[i].hit_test(pos).map(|(p, _)| p))`. A drag held by
  another control is dropped (`active_drag = None`). Then
  `handle_mouse(&event, part)`, returned as
  `EventResponse::handled(i, id, part, action, focused)`.
- **Keyboard.** `handle_keyboard(&event)` on `i`; the host does **not**
  handle Tab, BackTab or Escape. Returned as `handled(i, id, Body, action,
  focused)`.
- **Gesture.** `handle_gesture(&event, part)`, `part` from `i`'s `hit_test`
  at the pointer, else `Body`. Returned as `handled`.
- **Modifiers.** `set_modifiers` on `i` only. Returned as `handled(i, id,
  Body, None, focused)`.

Rules for filters: capture no more than needed. A container that captures
`Move` checks `ctx.hit` (an overlapping sibling above it keeps the pointer)
and `!ctx.drag_elsewhere` (it does not steal another control's drag). The
topmost capturing control wins; a control added later is above, both for
drawing and for filtering. A later non-modal dialog would capture pointer
events inside its frame and keys while it has focus, and pass the rest.

With no control overriding `intercept`, every routing path is 0.2.0's.

## 5. `Key::Back` (breaking change, 0.3.0)

1. `Key` gains `Back`, the platform back action (Android Back, browser
   back). Shells send it as `Key::Back`, no longer as `Key::Escape`.
2. `Key` becomes `#[non_exhaustive]`, so later keys are not breaking. A
   downstream exhaustive `match` on `Key` stops compiling; the version goes
   from 0.2.0 to 0.3.0. tinytrowel does not name `libcontrols::Key` (its
   `Key::Escape` in `trowel-desktop/src/main.rs` is the windowing crate's), so
   it is expected to compile unchanged; step 0 checks it.
3. Without a modal, `Back` is not special: the host sends it to the focused
   control like any other key (unlike Escape, it does not clear focus).
4. In a modal, Escape and Back both dismiss the top (section 8), reported as
   `DismissReason::Escape` and `DismissReason::Back`, even when a content
   `Edit` has focus, as dialogs on the web do. An app that must protect
   unsaved edits uses `Modal::with_hold_open` (8.2).
5. The new public enums `ModalPhase`, `DismissReason` and `ModalEvent` are
   `#[non_exhaustive]` from the start.

## 6. Modal types

All modal code is in a new module `src/modal.rs`, re-exported from
`src/lib.rs`:

```rust
pub use modal::{
    Backdrop, ContentReveal, DismissOn, DismissReason, Modal, ModalEntryView, ModalEvent,
    ModalFrame, ModalPhase, ModalStack, ModalState, ModalTransition, ModalView, PanelFrame,
    MAX_MODAL_DEPTH,
};
```

### 6.1 Phase, transition, reasons, events

```rust
/// Where a modal is in its life.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ModalPhase {
    #[default]
    Closed,
    Opening,
    Open,
    Closing,
}

/// How progress moves between 0 and 1.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum ModalTransition {
    /// Progress jumps; push and dismiss finish their transition at once.
    Instant,
    /// Progress moves linearly with ModalState::advance(dt).
    /// A duration <= 0.0 behaves as Instant for that direction.
    Timed { open_secs: f32, close_secs: f32 },
    /// The app sets progress with ModalState::set_progress; advance() does nothing.
    Driven,
}

impl Default for ModalTransition {
    /// Timed { open_secs: 0.18, close_secs: 0.12 }.
    fn default() -> Self;
}

/// Why a modal began to close.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum DismissReason {
    /// A press and release both outside the top's frame.
    OutsideTap,
    /// Key::Escape.
    Escape,
    /// Key::Back, the platform back action.
    Back,
    /// A content control registered with Modal::with_close_control was activated;
    /// carries its id ("close", "apply", "cancel", ...).
    Control(String),
    /// The app called pop(), close(id), dismiss(DismissReason::App) or dismiss_all(App).
    App,
    /// The app called replace_top; the outgoing top closes with this reason.
    Replaced,
}

/// Something the modal layer reports. Taken once with ModalState::take_events.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum ModalEvent {
    /// The opening transition finished; content now takes input.
    Opened { id: String },
    /// The modal began to close, and why.
    Dismissed { id: String, by: DismissReason },
    /// The closing transition finished; the entry is gone.
    /// Every Dismissed is followed by exactly one Closed with the same id and reason.
    Closed { id: String, by: DismissReason },
    /// A user dismissal was refused because the modal is held open (8.2).
    /// The modal stays as it was.
    DismissRefused { id: String, by: DismissReason },
}
```

### 6.2 `ModalState` (persistent, app-owned)

```rust
pub const MAX_MODAL_DEPTH: usize = 4;

/// The modal stack, shared between the app and every rebuild of the host.
#[derive(Debug, Default)]
pub struct ModalState {
    inner: Mutex<ModalInner>,
}

#[derive(Debug, Default)]
struct ModalInner {
    default_transition: ModalTransition,
    entries: Vec<ModalEntry>,                 // bottom..top, at most MAX_MODAL_DEPTH
    pending: Vec<(String, ModalTransition)>,  // pushes waiting for a same-id entry to finish closing
    events: Vec<ModalEvent>,
}

#[derive(Debug)]
struct ModalEntry {
    id: String,                               // unique among entries not Closing
    phase: ModalPhase,                        // Opening, Open or Closing; Closed entries are removed
    progress: f32,                            // always clamped to [0, 1]
    transition: ModalTransition,
    reason: Option<DismissReason>,            // Some only in Closing
    focus: Option<ControlKey>,                // focus inside this modal's content, across rebuilds
    drag: Option<DragGrab>,                   // drag inside this modal's content, across rebuilds
    focus_seeded: bool,                       // Modal::with_initial_focus applied since push
    outside_press: Option<MouseButton>,       // a press outside the frame awaiting its release
}
```

Public API (all `&self`; the `Mutex` gives interior mutability, matching
`PanelData`; `ModalState` is `Send + Sync`):

```rust
impl ModalState {
    pub fn new() -> Self;
    pub fn with_transition(self, t: ModalTransition) -> Self;               // default for pushes

    pub fn push(&self, id: impl Into<String>) -> bool;                      // false: duplicate id or stack full
    pub fn push_with(&self, id: impl Into<String>, t: ModalTransition) -> bool;
    pub fn replace_top(&self, id: impl Into<String>) -> bool;               // top closes (Replaced), id opens
    pub fn replace_top_with(&self, id: impl Into<String>, t: ModalTransition) -> bool;
    pub fn dismiss(&self, by: DismissReason);                               // closes the top only; ignores hold-open
    pub fn pop(&self);                                                      // dismiss(DismissReason::App)
    pub fn close(&self, id: &str);                                          // closes that entry, wherever it is (App)
    pub fn dismiss_all(&self, by: DismissReason);                           // every live entry, top first

    pub fn advance(&self, dt_secs: f32) -> bool;                            // every Timed entry; true while any animates
    pub fn set_progress(&self, id: &str, progress: f32);                    // Driven, per entry (works in any mode)
    pub fn set_top_progress(&self, progress: f32);

    pub fn top(&self) -> Option<String>;
    pub fn depth(&self) -> usize;                                           // entries, Closing included
    pub fn contains(&self, id: &str) -> bool;                               // live (not Closing) entry
    pub fn phase(&self, id: &str) -> ModalPhase;                            // Closed if absent
    pub fn progress(&self, id: &str) -> f32;                                // 0.0 if absent
    pub fn stack(&self) -> Vec<ModalEntryView>;                             // bottom..top snapshot
    pub fn captures_input(&self) -> bool;                                   // any entry at all
    pub fn top_takes_input(&self) -> bool;                                  // top exists and is Open
    pub fn is_animating(&self) -> bool;
    pub fn take_events(&self) -> Vec<ModalEvent>;                           // drains the queue
}

#[derive(Clone, Debug, PartialEq)]
pub struct ModalEntryView { pub id: String, pub phase: ModalPhase, pub progress: f32, pub is_top: bool }
```

Rules (each row is a step 4 test):

| Situation | Call / condition | Result |
|---|---|---|
| id not live, depth < 4 | `push(id)` | new entry on top in `Opening` with progress 0 (or `Open` with progress 1 and `Opened` if the transition is instant); entries below are untouched and emit nothing; true |
| id already live | `push(id)` | no change, false |
| depth == 4 (Closing entries count) | `push(id)` | no change, false |
| id has an entry in `Closing` | `push(id)` | queued in `pending`, true; pushed on top when that entry emits `Closed` |
| entry `Opening` | its progress reaches 1 | `Open`, `Opened { id }` |
| top exists | `dismiss(by)` | top goes `Closing` with `by`, `Dismissed { id, by }`; progress continues down from where it was; the entry below becomes top again (it was never closed). Instant close: removed at once with `Closed`, in the same call |
| no top | `dismiss(_)` / `pop()` | no change (a second Escape while closing does nothing) |
| top exists, id not live | `replace_top(id)` | top: `Closing`, `Dismissed { by: Replaced }`; new entry pushed above it at once in `Opening`; both animate together. `Replaced` comes only from `replace_top`. Depth is checked as for `push`, counting the outgoing entry |
| no top | `replace_top(id)` | same as `push(id)` |
| any | `dismiss_all(by)` | every live entry, top first, goes `Closing` with `by` and emits `Dismissed`; `pending` is cleared |
| entry `Closing` | its progress reaches 0 | removed, `Closed { id, by }`; then any `pending` push for that id |
| any | `set_progress(id, p)` | that entry's `progress = clamp(p)`; the "reaches" rows apply only in the matching phase (raising progress while `Closing` just moves it) |

`advance(dt)`: every `Timed` entry in `Opening` gets `progress += dt /
open_secs`, every one in `Closing` `progress -= dt / close_secs`; then the
"reaches" rows apply. `Instant` and `Driven` entries ignore it, so a game can
drive the book's opening while a `Timed` confirm opens over it. A `Driven`
entry in `Closing` stays visible until the app drives it to 0. Negative or
non-finite `dt` is treated as 0. Every event carries its entry's id.

### 6.3 `ModalView`, `ModalFrame`, `PanelFrame`

```rust
/// Everything a frame painter needs to draw one frame of one modal.
#[derive(Clone, Debug, PartialEq)]
pub struct ModalView<'s> {
    pub id: &'s str,
    pub phase: ModalPhase,
    pub progress: f32,
    pub viewport: Rect,
    pub frame: Rect,
    pub content: Rect,
    pub depth: usize,   // 0 = bottom of the stack
    pub is_top: bool,
}

/// The app-drawn look of a modal: its background, its animation, its shape.
pub trait ModalFrame {
    /// Draw the frame (parchment, open book, panel) under the content.
    fn draw_frame(&self, list: &mut DrawList, ctx: &DrawContext, view: &ModalView);

    /// Draw anything over the content (a page-curl shadow, a ribbon). Default: nothing.
    fn draw_over_content(&self, _list: &mut DrawList, _ctx: &DrawContext, _view: &ModalView) {}

    /// Whether a point is on the modal for tap-outside purposes. Default: view.frame.contains(pos).
    fn contains(&self, pos: [f32; 2], view: &ModalView) -> bool {
        view.frame.contains(pos)
    }
}

impl<F: ModalFrame + ?Sized> ModalFrame for &F { /* forwards */ }

/// The built-in frame: a rounded box with an optional border, in theme colours.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct PanelFrame {
    pub bg_color: Option<[f32; 4]>,      // None: ctx.theme.bg_color
    pub border_color: Option<[f32; 4]>,  // None: ctx.theme.accent_color
    pub border_thickness: f32,           // default 1.0; 0 draws no border
    pub corner_radius: Option<f32>,      // None: ctx.theme.corner_radius
}
```

`PanelFrame::draw_frame` pushes the same `RoundedBox`/`Outline` instances as
`Panel::draw`, every colour's alpha multiplied by `view.progress`. A game that
wants a book implements `ModalFrame` on its own type and animates from
`view.progress`, `view.phase` and `view.is_top` (for example, a lower book
drawn closed but visible).

### 6.4 Backdrop, reveal, dismiss rules

```rust
/// The dim drawn over everything beneath a modal.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Backdrop {
    pub color: [f32; 4],          // default [0.0, 0.0, 0.0, 0.45]
    pub fade_with_progress: bool, // default true: alpha *= progress
}

/// How the content controls appear during a transition.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum ContentReveal {
    /// Drawn only when Open (a book whose pages appear once it has opened).
    WhenOpen,
    /// Drawn in every visible phase with alpha multiplied by progress.
    #[default]
    Fade,
    /// Drawn unchanged in every visible phase; the frame painter does the rest.
    Always,
}

/// Which user actions dismiss the top modal. Close controls are listed on Modal.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct DismissOn {
    pub outside_tap: bool, // default true
    pub escape: bool,      // default true
    pub back: bool,        // default true
}
```

**Fade.** `ContentReveal::Fade` remembers `start = list.instances.len()`
before drawing the content and afterwards multiplies `color[3]` of
`list.instances[start..]` by `progress`. This is correct for every instance
kind, animated ones included: in libmsdf's shader (rev 69e6c5c,
`sdf_render.wgsl`) shape fills use `cmd.color.a * anim_alpha * ombre_alpha`,
MSDF glyphs `cmd.color.a * alpha`, and textures `color_sample.a *
cmd.color.a`. Two known limits:

- Fading each instance is not a group alpha: overlapping translucent content
  shows through itself while it fades.
- Raised (elevation) shadows have a fixed alpha (0.15) that does not follow
  `cmd.color.a`, so a raised box in the content leaves its shadow at full
  strength during a fade. No libcontrols control uses elevation.

The fade stays in libcontrols because libcontrols draws the content; the app
fades its own frame from `view.progress`. There is no scale or "pop"
animation (libmsdf has no scale); a slide is available to the frame painter
through `push_translate`, and to content only through
`ContentReveal::Always`.

### 6.5 `Modal<'a>` (built with the host)

```rust
pub struct Modal<'a> {
    state: Arc<ModalState>,
    id: String,                           // the stack entry this modal is built for
    viewport: Rect,
    frame_rect: Rect,
    content_rect: Rect,
    frame: Box<dyn ModalFrame + 'a>,      // default Box::new(PanelFrame::default())
    backdrop: Option<Backdrop>,           // default Some(Backdrop::default())
    dismiss: DismissOn,                   // default DismissOn::default()
    hold_open: bool,                      // default false (8.2)
    close_controls: Vec<String>,          // ids that dismiss with DismissReason::Control(id)
    reveal: ContentReveal,                // default Fade
    clip_content: bool,                   // default true: push_clip(content) around the content's draw pass
    initial_focus: Option<ControlKey>,
    children: RefCell<ControlHost<'a>>,   // the content controls
}

impl<'a> Modal<'a> {
    pub fn new(state: Arc<ModalState>, id: impl Into<String>, viewport: Rect, frame: Rect, content: Rect) -> Self;
    /// frame centred in viewport at `size`; content = frame.pad(-padding).
    pub fn centered(state: Arc<ModalState>, id: impl Into<String>, viewport: Rect, size: [f32; 2], padding: f32) -> Self;

    pub fn with_frame<F: ModalFrame + 'a>(self, frame: F) -> Self;
    pub fn with_backdrop(self, backdrop: Option<Backdrop>) -> Self;
    pub fn with_dismiss(self, dismiss: DismissOn) -> Self;
    pub fn with_hold_open(self, hold: bool) -> Self;
    pub fn with_close_control(self, id: impl Into<String>) -> Self;   // may be called more than once
    pub fn with_reveal(self, reveal: ContentReveal) -> Self;
    pub fn with_clip_content(self, clip: bool) -> Self;
    pub fn with_initial_focus(self, key: ControlKey) -> Self;
    pub fn with_control<C: Control + 'a>(self, control: ControlHandle<'a, C>) -> Self;
    pub fn with_named_control<C: Control + 'a>(self, id: impl Into<String>, control: ControlHandle<'a, C>) -> Self;
    pub fn with_custom_control<H: Control + 'a>(self, control: H) -> Self;

    pub fn id(&self) -> &str;
    pub fn state(&self) -> &Arc<ModalState>;
    pub fn frame_rect(&self) -> Rect;
    pub fn content_rect(&self) -> Rect;
    /// None when the state has no entry for this id.
    pub fn view(&self) -> Option<ModalView<'_>>;

    /// Route one event to this modal as the top (section 8). ModalStack calls it;
    /// public so a modal can be tested on its own.
    pub fn deliver_event(&self, event: InputEvent) -> EventResponse;
}

impl DrawControl for Modal<'_> {
    /// Backdrop, frame, content, content overlays (top only), frame-over-content (section 9).
    /// Draws nothing when the state has no entry for this id.
    fn draw(&self, list: &mut DrawList, ctx: &DrawContext);
}
```

A `Modal` whose id has no entry draws nothing and takes no input. The app
builds one `Modal` per entry in `state.stack()`, choosing each one's content
from its id.

## 7. `ModalStack` (a hosted control)

```rust
pub struct ModalStack<'a> {
    state: Arc<ModalState>,
    viewport: Rect,
    modals: Vec<Modal<'a>>,   // any order; routed and drawn in the state's stack order
}

impl<'a> ModalStack<'a> {
    pub fn new(state: Arc<ModalState>, viewport: Rect) -> Self;
    /// One per stack entry; a second Modal for the same id replaces the first.
    pub fn with_modal(self, modal: Modal<'a>) -> Self;
    pub fn state(&self) -> &Arc<ModalState>;
    pub fn modals(&self) -> &[Modal<'a>];
    pub fn top_modal(&self) -> Option<&Modal<'a>>;    // the Modal built for state.top()
    pub fn handle(&self) -> ControlHandle<'_, Self>;
}
```

The app adds the stack to the host **last**, so that it is above every base
control for both filtering and drawing, and builds it on every host build,
even when the state is empty (an empty stack passes everything and draws
nothing). An app whose UI is only modals, like Noble's Keep, builds a host
holding just the stack.

`HostedControl for ModalStack`:

- `bounds()`: the viewport.
- `hit_test(pos)`: `Some((Body, 0.0))` while `state.captures_input()`, else
  `None`, so an empty stack is transparent to normal routing.
- `intercept(_, _)`: `Capture` while `state.captures_input()`, else `Pass`.
  This is the whole trap: base controls (level 0) receive nothing, not even
  modifiers, while any modal exists.
- `handle_mouse`, `handle_keyboard`, `handle_gesture`: if the top has a built
  `Modal`, `top.deliver_event(event)` and return its `action`; otherwise
  `None` (the event is still captured, so it reaches nothing). On every mouse
  event, the content of every lower modal gets `clear_hover()`.
- `set_modifiers(m)`: to the top modal's content host only.
- `set_hover(None)`: `clear_hover()` on the top modal's content host.

Lower levels miss modifier changes made while a modal is above them; nothing
is replayed when they become top again. No libcontrols control uses
modifiers today.

`EventResponse` from the base host names the stack (its index and id) and
carries the inner control's `action`, as with `Panel`. The app learns which
inner control acted from `ModalState::take_events` (`Dismissed { by:
Control(id) }`) and from its control state (`take_click`), as tinytrowel
already does.

`DrawControl for ModalStack`: `draw` draws nothing; `draw_overlay` draws
every entry's `Modal` bottom to top (section 9). Because the base host draws
every control's `draw` and then every control's `draw_overlay`, and the stack
is last, the modals are above every base control and every base overlay. A
base tooltip visible when a modal opens by keyboard stays under the backdrop
until the next pointer event clears base hover. An app that draws its own
scene with libmsdf (Noble's Keep's tabletop) draws it first and the host
last.

## 8. Input inside the top modal

### 8.1 `Modal::deliver_event(event)`

1. Load the content host's focus and drag from this modal's entry:
   `children.set_focused_key(entry.focus)`, `children.set_active_drag(entry.drag)`.
   If `!entry.focus_seeded`, use `initial_focus` instead and set
   `focus_seeded = true`.
2. **Escape** (key down, `Key::Escape`) with the entry `Opening` or `Open`
   and `dismiss.escape`: `request(Escape)`, return `consumed()`. With
   `dismiss.escape == false` it falls through to step 5 (the content host then
   clears its own focus, as 0.2.0 does).
   **Back** (key down, `Key::Back`) likewise with `dismiss.back` and
   `request(Back)`; disabled, it falls through to step 5 and goes to the
   focused content control. Both apply even when a content `Edit` has focus.
3. **Not Open** (`Opening` or `Closing`): return `consumed()`. Nothing reaches
   the content.
4. **Tap outside** (mouse, `Open`, no content drag in progress):
   - `Down { pos, button }` with `!frame.contains(pos, view)`: set
     `outside_press = Some(button)`, return `consumed()`.
   - `Up { pos, button }` with `outside_press == Some(button)`: clear it; if
     `!frame.contains(pos, view)` and `dismiss.outside_tap`,
     `request(OutsideTap)`. Return `consumed()`.
   - `Click { pos, .. }` outside with `outside_press == None` and
     `dismiss.outside_tap`: `request(OutsideTap)`, return `consumed()`. (For
     shells that send only `Click`. A shell that sends `Down`, `Up` and
     `Click` dismisses on the `Up`; the `Click` then arrives in `Closing` and
     is swallowed by step 3.)
   - Any other mouse event outside the frame: `consumed()`.

   Requiring the press and the release outside means the release of the tap
   on the icon that opened the modal (handled by the app on `Down`) cannot
   close it, and neither can a drag that starts on a slider and ends outside.
   A tap on a lower modal's frame is outside the top.
5. **Content**: `resp = children.deliver_event(event)`. If `resp.target_id`
   is in `close_controls` and `resp.action` is
   `Some(ControlAction::Activated { .. })`, `request(Control(id))`. Return
   `resp` unchanged.
6. Store `children.focused_key()` and `children.active_drag()` back into the
   entry.

`consumed()` is `EventResponse { handled: true, target_index: None,
target_id: None, target_part: None, action: None, focused_index: None }`.

Tab and BackTab cycle focus among the top's content controls only, because
the content host is separate and the stack captures Tab before the base host
sees it: the focus trap needs no extra code. Focus is kept per entry, so
focus in a lower modal is where it was when the modal above it closes.

### 8.2 Requests and the hold-open veto

`request(by)` is how every dismissal a user starts (`OutsideTap`, `Escape`,
`Back`, `Control(id)`) reaches the state:

- `hold_open == false`: `state.dismiss(by)`.
- `hold_open == true`: the modal stays as it is and the state queues
  `DismissRefused { id, by }`.

An app with unsaved edits builds its modal with `with_hold_open(true)` while
it has them, and answers `DismissRefused` by, for example, pushing a
"Discard changes?" confirm. Dismissals the app starts (`dismiss`, `pop`,
`close`, `dismiss_all`, `replace_top`) are never held. `hold_open` is a
per-build flag rather than a callback because libcontrols stores no closures
and the app rebuilds every frame, so it knows at build time whether the modal
may close. A `DismissOn` field that is false means the action does nothing,
not that it is refused: no `DismissRefused` is queued.

## 9. Drawing a modal

`Modal::draw(list, ctx)`, when its entry exists:

1. `view = self.view()`.
2. Backdrop: if `Some(b)`, push one `SdfKind::Box` covering `viewport` with
   `color = b.color`, alpha multiplied by `progress` when
   `b.fade_with_progress`. Each level draws its own, so each new level dims
   everything beneath it, compounding.
3. `frame.draw_frame(list, ctx, &view)`.
4. Content, unless `reveal == WhenOpen` and the phase is not `Open`:
   `start = list.instances.len()`; if `clip_content`,
   `list.push_clip(content.pos(), content.size())`;
   `children.draw_controls(list, ctx)`; `push_clip_end()`; if `view.is_top`,
   `children.draw_control_overlays(list, ctx)` (outside the clip, so tooltips
   may leave the content rect; content controls should set
   `TooltipStyle::bounds` to the frame or viewport); if `reveal == Fade`,
   multiply `color[3]` of `list.instances[start..]` by `progress`.
5. `frame.draw_over_content(list, ctx, &view)`.

## 10. Panel

### 10.1 Hover forwarding (always on)

`Panel::intercept` returns `Capture` for `InputEvent::Mouse(MouseEvent::Move
{ .. })` when `ctx.hit && !ctx.drag_elsewhere`. The captured `Move` arrives in
`Panel::handle_mouse`, which already forwards every mouse event to the child
host, whose `deliver_mouse` updates hover. `Panel::set_hover(None)` calls
`children.borrow().clear_hover()`, so leaving the panel clears its children's
hover. A hovered child now highlights on plain moves and clears on leaving,
as it does outside panels. It is a fix, so it is not optional.

### 10.2 Focus traversal (opt-in)

```rust
impl<'a, S> Panel<'a, S> {
    /// Let Tab and BackTab walk into and out of the children, and Escape clear
    /// the children's focus before the host's. Default: false.
    pub fn with_focus_traversal(self, on: bool) -> Self;
}
```

A new private field `focus_traversal: bool`. With it on, and only when
`ctx.focused` and the event is a key down, `intercept` answers:

| Key | Child focus (`n` children) | Answer and effect |
|---|---|---|
| Tab | none, `n > 0` | `Capture`; child host focuses child 0 |
| Tab | child `k < n - 1` | `Capture`; child host focuses `k + 1` |
| Tab | child `n - 1` | `Pass`; clear child focus; the outer host moves on |
| BackTab | none, `n > 0` | `Capture`; child host focuses child `n - 1` |
| BackTab | child `k > 0` | `Capture`; child host focuses `k - 1` |
| BackTab | child 0 | `Pass`; clear child focus; the outer host moves back |
| Escape | some child | `Capture`; child host clears its focus |
| Escape | none | `Pass`; the outer host clears its focus |

`Panel::handle_keyboard` already forwards keys to the child host, whose own
Tab, BackTab and Escape handling makes the moves (it wraps, but the table
never asks it to). Nested panels work the same way at each level. The panel
itself stays a focus stop: Tab first focuses the panel, the next Tab its
first child. Off by default because it changes the focus order of existing
apps that use panels and Tab.

## 11. How Noble's Keep uses it (informative)

```rust
// App state, once:
let modals = Arc::new(ModalState::new().with_transition(ModalTransition::Driven));

// The gear icon is the game's own libmsdf drawing and hit test:
if gear_tapped { modals.push("settings"); }

// Every frame:
book_anim.step(dt);                                  // the game's own book animation
modals.set_progress("settings", book_anim.openness());
modals.advance(dt);                                  // Timed entries (a confirm)
for ev in modals.take_events() { /* Closed { id: "settings", .. } => persist settings */ }

// Input: everything goes to the host; the stack captures while any modal exists.
if modals.captures_input() { host.deliver_event(ev); } else { tabletop.input(ev); }

// Building the host (per event / per frame), one Modal per stack entry:
let mut stack = ModalStack::new(modals.clone(), viewport);
for entry in modals.stack() {
    stack = stack.with_modal(match entry.id.as_str() {
        "settings" => Modal::new(modals.clone(), "settings", viewport, book_rect, page_rect)
            .with_frame(BookFrame { .. })            // game's ModalFrame: cover, pages, curl
            .with_reveal(ContentReveal::WhenOpen)
            .with_close_control("close")
            .with_named_control("volume", volume.handle())
            .with_named_control("close", close.handle()),
        "quit" => Modal::centered(modals.clone(), "quit", viewport, [320.0, 160.0], 16.0)
            .with_named_control("yes", yes.handle())
            .with_close_control("no")
            .with_named_control("no", no.handle()),
        _ => continue,
    });
}
let host = ControlHost::builder().with_custom_control(stack).build();
// Draw: tabletop first with libmsdf, then host.draw(&mut list, &ctx) last.
```

The shell sends Android Back as `Key::Back`; it dismisses the top modal like
Escape.

## 12. Tabbed containers (informative; no tabbed control exists)

Apps build tabs from a `Radio` strip (radio.rs:16, "matching the panel tab
strip") and a `Panel` whose children the app swaps by the selected index on
each build. Pages not shown are not built, so they receive nothing without
any filtering. This plan gives that arrangement Tab traversal from the strip
into the page and out again (`with_focus_traversal(true)` on the page panel)
and working hover inside the page (10.1).

A future `TabView` container would use the filter to switch pages with
Ctrl+Tab, Ctrl+Shift+Tab, Ctrl+PageDown or Ctrl+PageUp while focus is
anywhere inside it: `intercept` captures those key downs when `ctx.focused
&& ctx.modifiers.ctrl`, before the host takes Tab. Left and Right on the
strip already work through `Radio::handle_keyboard`.

## 13. Compatibility

- **Breaking (0.3.0):** `Key` gains `Back` and becomes `#[non_exhaustive]`
  (section 5).
- Everything else is additive: `HostedControl` gains a defaulted method;
  `ControlHost` gains `clear_hover`, `draw_controls` and
  `draw_control_overlays`; `Panel` gains a builder method and a private
  field; the filter and modal types are new. `ControlAction` and
  `EventResponse` do not change.
- With no control overriding `intercept`, routing is 0.2.0's, and
  `ControlHost::draw` emits the same instances in the same order.
- `Panel` with traversal off differs from 0.2.0 only in hover (10.1).
- `tests/events.rs` and the unit tests in `src/lib.rs` and `src/button.rs`
  pass unmodified (step 0 appends to `tests/events.rs`).

## 14. Steps

Each step is one commit touching only this crate, passing
`cargo fmt -p libcontrols -- --check`, `cargo clippy -p libcontrols
--all-targets` with no new warnings, and `cargo test -p libcontrols`
(Rule 3.4). New tests go in `tests/filter.rs` (steps 1 to 3) and
`tests/modal.rs` (steps 4 to 7), in the style of `tests/events.rs`: small
recorder controls logging every `handle_*`, `set_hover`, `set_modifiers` and
`intercept` call into a `RefCell<Vec<_>>`, `ControlHost::builder()`, plain
asserts with messages saying what must hold.

0. **`Key::Back`, `#[non_exhaustive]`, 0.3.0.** Add `Key::Back`, mark `Key`
   `#[non_exhaustive]`, bump `Cargo.toml` to `0.3.0`, and confirm every
   in-crate `match` on `Key` has a wildcard. Tests (appended to
   `tests/events.rs`): `back_goes_to_the_focused_control_and_keeps_focus`,
   `escape_still_clears_focus`. In a scratch copy of tinytrowel (not
   committed), replace `deps/libcontrols/src` and `Cargo.toml` with this
   crate's and run `cargo check -p trowel-game`; if it fails, the fix is
   proposed to tinytrowel's owner, not made here.
   **Done when:** the two tests and all existing tests pass unmodified, and
   `trowel-game` checks clean against 0.3.0 (or the needed tinytrowel change
   is written up).

1. **The filter hook and pass.** Add `Intercept`, `InterceptContext`,
   `HostedControl::intercept` with its `ControlHandle` forwarding,
   `ControlHost::clear_hover`, `draw_controls`, `draw_control_overlays`, the
   filter pass (4.2) and `deliver_captured` (4.3).
   Tests: `nothing_changes_when_every_control_passes`,
   `the_topmost_capturing_control_wins`,
   `a_capturing_control_gets_events_that_miss_it`,
   `a_captured_key_skips_the_hosts_tab_and_escape`,
   `captured_modifiers_go_to_the_capturer_only`,
   `capture_clears_other_hover_and_drops_other_drags`,
   `a_captured_press_leaves_host_focus_and_grab_alone`,
   `the_context_reports_focus_hit_and_drag`,
   `split_draw_passes_match_draw`.
   **Done when:** those pass, existing tests pass unmodified, and a recorder
   that always captures receives every mouse, keyboard, gesture and modifier
   event while a recorder beneath it receives none.

2. **Panel hover forwarding** (10.1).
   Tests: `a_child_hover_follows_plain_moves_inside_a_panel`,
   `a_child_hover_clears_when_the_pointer_leaves_the_panel`,
   `a_panel_does_not_take_moves_under_an_overlapping_sibling`,
   `a_panel_does_not_steal_a_siblings_drag`.
   **Done when:** a `Button` inside a `Panel` reports hover after a single
   `Move` over it and none after a `Move` outside the panel, and the existing
   panel tests in `src/lib.rs` pass.

3. **Panel focus traversal** (10.2).
   Tests: one per row of the 10.2 table, plus `traversal_is_off_by_default`
   (Tab steps over the children, as 0.2.0) and
   `nested_panels_traverse_depth_first`.
   **Done when:** in a host of `[Button, Panel(traversal on, 2 Buttons),
   Button]`, eight Tabs from no focus visit outer 0, panel, child 0, child 1,
   outer 2, then wrap to outer 0, panel, child 0; BackTab walks the same
   order backwards; and two Escapes from child 1 clear the child focus, then
   the outer focus.

4. **`ModalState`.** Add `src/modal.rs` with the 6.1 and 6.2 types and
   re-export them. No host changes.
   Tests: `a_push_opens_over_the_top_and_leaves_the_lower_one_alone`,
   `a_duplicate_push_is_refused`, `a_fifth_modal_is_refused`,
   `dismiss_closes_only_the_top_and_the_one_below_becomes_top`,
   `replace_top_reports_replaced_and_opens_at_once`,
   `replaced_is_reported_only_by_replace_top`,
   `dismiss_all_closes_top_first`,
   `pushing_an_id_that_is_closing_waits_for_its_close`,
   `timed_and_driven_entries_move_independently`,
   `an_instant_modal_is_open_at_once`,
   `every_dismissed_is_followed_by_one_closed_with_the_same_id`,
   `dismissing_with_no_top_does_nothing`,
   `events_are_taken_once`, `advance_ignores_negative_and_nan_dt`.
   **Done when:** every row of the 6.2 table is covered by a passing test and
   event order is exactly as stated there.

5. **`Modal`, `ModalFrame`, `PanelFrame`, `Backdrop`, `ContentReveal`, and
   drawing** (6.3 to 6.5, section 9). `Modal::deliver_event` routes straight
   to the content host for now.
   Tests: `a_modal_with_no_entry_draws_nothing`,
   `the_backdrop_covers_the_viewport_and_fades_with_progress`,
   `no_backdrop_means_no_backdrop_instance`,
   `the_frame_painter_is_called_with_the_view` (a recording `ModalFrame` sees
   id, phase, progress, frame, content, depth and is_top),
   `when_open_content_is_hidden_while_opening`,
   `fade_scales_content_alpha_by_progress`,
   `content_is_clipped_and_its_tooltips_are_not`,
   `only_the_top_draws_content_tooltips`.
   **Done when:** a modal at progress 0.5 with `Fade` emits backdrop, frame,
   then content instances, in that order, with content alpha halved, and a
   modal with no entry emits nothing.

6. **Input and dismissal inside `Modal::deliver_event`** (section 8).
   Tests: `escape_dismisses_and_a_second_escape_does_nothing`,
   `back_dismisses_with_back`, `escape_and_back_dismiss_with_an_edit_focused`,
   `escape_reaches_the_content_when_disabled`, `back_can_be_turned_off`,
   `a_press_and_release_outside_dismisses`,
   `a_release_outside_without_a_press_outside_does_not_dismiss`,
   `a_drag_from_a_slider_to_outside_does_not_dismiss`,
   `a_click_outside_dismisses_for_click_only_shells`,
   `outside_tap_can_be_turned_off`,
   `a_close_control_dismisses_with_its_id`,
   `hold_open_refuses_user_dismissals_and_reports_them`,
   `hold_open_does_not_stop_app_dismissals`,
   `a_disabled_action_is_not_reported_as_refused`,
   `content_receives_nothing_while_opening_or_closing`,
   `focus_inside_the_modal_survives_a_rebuild`,
   `initial_focus_is_applied_once_per_push`,
   `a_frame_shape_decides_what_is_outside`.
   **Done when:** every user dismissal path produces exactly one `Dismissed`
   with the right `DismissReason`, or exactly one `DismissRefused` when held
   open, and no event reaches a content control outside `Open`.

7. **`ModalStack`** (section 7).
   Tests: `an_empty_stack_passes_everything_and_draws_nothing`,
   `base_controls_receive_nothing_while_any_modal_exists` (no `handle_*`,
   no `set_modifiers`, in each of Opening, Open and Closing),
   `base_hover_is_cleared_and_base_drags_dropped_when_capture_begins`,
   `base_focus_is_unchanged_after_a_modal_closes`,
   `lower_modals_receive_nothing`,
   `escape_and_back_close_only_the_top`,
   `a_tap_on_a_lower_modal_is_outside_the_top`,
   `focus_is_kept_per_entry`,
   `modals_are_ordered_by_the_stack_not_by_build_order`,
   `input_is_consumed_when_the_top_has_no_modal_built`,
   `each_level_draws_its_own_backdrop_bottom_to_top`,
   `the_stack_draws_after_every_base_overlay`,
   `a_host_with_only_a_stack_works`.
   **Done when:** with two entries at progress 1.0 over a base `Button` with
   a tooltip, the list holds the base control, its overlay, then backdrop,
   frame and content of the lower modal, then backdrop, frame and content of
   the upper one, in that order; and a base recorder logs no `handle_*` or
   `set_modifiers` call while either entry exists.

8. **Documentation and compatibility check.** Add "Input filters" and
   "Modals" sections to `README.md` above `## Building`: the filter hook, the
   Panel behaviour, and a two-level modal example (settings, then a confirm
   pushed over it) in neutral terms. In a scratch copy of tinytrowel (not
   committed), replace `deps/libcontrols/src` and `Cargo.toml` with this
   crate's and run `cargo check -p trowel-game`.
   **Done when:** the README examples compile as doc tests (or are mirrored in
   `tests/filter.rs` and `tests/modal.rs`), and `trowel-game` checks clean
   with no changes to tinytrowel.

## 15. Decisions carried from the modal plan's open questions

| Modal plan question | Decision |
|---|---|
| 1. One modal or a stack | A stack of up to four (6.2). |
| 2. Platform back key | `Key::Back`, breaking, 0.3.0 (section 5). |
| 3. Modal events in `ControlAction` | No: events only through `take_events`. |
| 4. Modifiers beneath a modal | Full trap: lower levels, base included, get nothing; no resync (section 7). |
| 5. Escape with a focused text field | Escape and Back dismiss even then; `with_hold_open` protects edits (5.4, 8.2). |
| 6. Vetoing a dismissal | `with_hold_open` and `ModalEvent::DismissRefused` (8.2). |
| 7. Scrolling content | A separate plan; out of scope here. |
| 8. Version and delivery | 0.3.0; how an app consumes the crate is not libcontrols' concern. |
| 9. Group alpha | Not needed: post-hoc alpha fades every instance kind; limits in 6.4. |
| 10. Default transition timing | `Timed { 0.18, 0.12 }` stays. |

## 16. Open questions

1. **Traversal default.** `Panel` focus traversal is off by default to keep
   0.2.0's focus order. Since 0.3.0 is a breaking release anyway, should it
   default to on?
2. **State changes inside `intercept`.** Panel clears its child focus from
   `intercept` when Tab or BackTab leaves it, because no control is told when
   it loses the host's focus. The alternative is a second defaulted method,
   `set_focused(&self, focused: bool)`, called by the host whenever focus
   changes. Is a side effect in `intercept` acceptable, or should focus
   changes get their own hook?
3. **Panel child overlays.** `Panel::draw` draws its child host whole,
   overlays included, inside the outer host's first pass, and `Panel` has no
   `draw_overlay`, so a child's tooltip can be drawn under a control added
   after the panel. With `draw_controls` and `draw_control_overlays` from
   step 1 the fix is two lines. Add it as a step here, or a separate change?
4. **A `TabView` container.** Wanted as its own plan (section 12)?
