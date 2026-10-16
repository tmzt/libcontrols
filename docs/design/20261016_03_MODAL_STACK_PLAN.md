---
title: Modal stack
date: 2026-10-16
status: Draft
covers: >
  A modal stack for libcontrols built on the input routing of plan
  20261016_02: a persistent ModalState (a stack of up to four modals with
  per-entry phase, transition, focus, drag and hover, and a queue of
  ModalEvents), a ModalFilter the app registers once that redirects every
  event to a hosted ModalStack control while any modal exists, per-build
  Modals with app-drawn frames, backdrops, content reveal, dismiss rules,
  close controls and a hold-open veto, Escape and Key::Back dismissal, the
  two draw passes of ControlHost split for clipping, how tower defense game uses
  it, the 0.3.0 version cut and VERSIONS.md entry, tests.
supersedes: docs/design/20261016_01_MODALS_AND_INPUT_FILTERS_PLAN.md (its modal design, sections 3, 5 to 9, 11 and 15; its filter design goes to 20261016_02_INPUT_ROUTING_AND_FILTERS_PLAN.md)
depends: docs/design/20261016_02_INPUT_ROUTING_AND_FILTERS_PLAN.md (Symbol, EventContext, FocusEvent, hover carrying, InputFilter and Filtered::Redirect, accepts_focus)
sources:
  - docs/design/20261016_01_MODALS_AND_INPUT_FILTERS_PLAN.md (Modals and input filters; superseded by this plan together with 20261016_02)
  - docs/design/20261015_01_MODAL_PLAN.md (Modal layer, with its amendment "2026-10-15 23:31 EDT: Key::Back and a modal stack"; superseded earlier; the origin of the state machine and the fade analysis)
  - "Owner (tower defense game): \"any UI will be limited to modals, and only if invoked via icons on the screen, such as interlocking gears on the bottom row opening the settings.\""
  - "Owner: \"modal needs to be modal, it should trap input events, if we want a non-modal dialog we can add that later.\""
  - "Owner: \"web usage usually has back/esc dismiss a modal, and we now have veto support if we want to protect changes\""
  - "Owner: \"Q8 isn't libcontrols concern at all. I think the base UI is more like level 0 ... I don't see why we need to resync or add events to reset the input state.\""
  - "Owner: \"the ModalStack registering a filter would be to deliver a private event to the ModalStack (which is a global concept, so has a handle) via a spsc type mechanism or by injecting an event (whichever makes sense)\" (decided as injection through Filtered::Redirect, section 6)"
  - "Owner: \"the control isn't registered as a filter itself\""
  - "Owner: \"except it shouldn't use strings, it should use the interned string/symbols\""
  - "Owner: \"rethink and have fable ... write a clean plan, split into filters and modal, supercede the current one\""
  - src/host.rs at d193afd (ControlHost::draw, the two passes), src/panel.rs (Panel::draw draws its child host whole), src/traits.rs (DrawControl::draw_overlay), src/button.rs, src/options.rs, src/radio.rs, src/tooltip.rs (draw_overlay tooltips, TooltipStyle::bounds, ButtonState::take_click)
  - ~/src/tinytrowel/deps/libmsdf at rev 69e6c5c (src/drawlist/mod.rs: painter order, DrawList::instances, push_clip/push_clip_end, instance_clip, push_translate, no push_scale; src/gpu/sdf_render.wgsl: how instance alpha, animation alpha and elevation shadows combine)
  - ~/src/tinytrowel/crates/trowel-game/src/ui.rs (per-event host rebuild, Arc state, take_click)
related:
  - docs/design/20261016_02_INPUT_ROUTING_AND_FILTERS_PLAN.md (Input routing and filters; this plan depends on it and ships with it as 0.3.0)
  - ~/src/NoblesKeep/docs/design/20261015_01_INITIAL_UI_PLAN.md (tower defense game initial UI plan; the main consumer)
---

# Modal stack

## 1. Purpose and scope

tower defense game shows no free-standing UI: "any UI will be limited to modals,
and only if invoked via icons on the screen, such as interlocking gears on
the bottom row opening the settings." The game draws its tabletop with
`libmsdf`; libcontrols draws and routes the controls inside a modal.

libcontrols 0.2.0 has no modals. Plan 02 gives a host a filter stack whose
`Filtered::Redirect(key)` sends an event to one control alone. This plan
builds modals on that, with nothing modal-specific in `ControlHost`:

- **`ModalState`**: app-owned, `Arc`-shared, persistent across rebuilds. A
  stack of up to four modals, each with its phase, progress, transition,
  dismiss reason, and the focus, drag and hover inside its content; a queue
  of `ModalEvent`s taken once.
- **`ModalFilter`**: a small object the app creates once from the state and
  registers on every host build with `with_filter`. While any modal exists
  it answers `Redirect(ControlKey::Named(target))`; otherwise `Pass`. It is
  the whole input trap: "modal needs to be modal, it should trap input
  events".
- **`ModalStack<'a>`**: an ordinary hosted control, added to the host by
  name, last, never registered as a filter. It receives the redirected
  events through its normal handlers and routes them to the top `Modal`;
  it draws every modal in `draw_overlay`, above every base control and
  overlay.
- **`Modal<'a>`**: built per host build like `Panel`: content controls, frame
  and content rects, the app's `ModalFrame`, backdrop, reveal, dismiss rules,
  close controls, hold-open veto.
- **Escape and `Key::Back`** dismiss the top modal, even with an `Edit`
  focused, "as web usage usually has".

Out of scope: non-modal dialogs (later; `Redirect` supports them), scrolling
content by wheel or finger (its own plan; until then modal content fits its
content rect or the app pages it), tabbed containers, game-specific looks,
and how an app consumes the crate.

## 2. Constraints from the code

Beyond plan 02's section 2 (read at `d193afd`):

1. **Controls report through state and actions.** `ButtonState::take_click`
   is take-once; `EventResponse::action` carries a `ControlAction`. Modal
   events are take-once too (`ModalState::take_events`) because `Opened`
   and `Closed` come from time passing, not from an input event.
   `ControlAction` gains no modal variant.
2. **Draw order is painter's order.** `ControlHost::draw` (`host.rs:518`)
   draws every control's `draw`, then every control's `draw_overlay`.
   `DrawList` has no z; `DrawList::instances` is public and each
   `SdfInstance` carries a straight-alpha `color`. `push_clip` snapshots
   the resolved clip onto every instance pushed under it
   (`DrawList::instance_clip`); `push_translate` exists; there is no
   `push_scale` and no group alpha.
3. **The library has no clock.** Transitions are advanced by the app
   (`ModalState::advance(dt)`) or driven outright (`set_progress`).
4. **`Panel::draw` draws its child host whole** (`panel.rs:445`), overlays
   included. A modal clips its content but not its content's tooltips, so
   it needs the two passes separately (section 11.2).
5. **After plan 02**, a redirected pointer event moves the host's hover off
   any control that is not the target and drops a grab the target does not
   hold (plan 02, 9.4); no control is ever told about modifier changes, and
   every event carries the modifiers held (plan 02, section 6); a control
   can refuse the focus (`accepts_focus`, plan 02, 7.2). The base host's
   `EventResponse` for a redirected event names the target.

## 3. Concepts

- **Level**: the base host's own controls are level 0; each modal on the
  stack is one level above. Only the top level receives input.
- **Viewport**: the surface the host draws to, in input-event pixels. Each
  modal's backdrop covers it.
- **Frame rect**: the area a modal visibly occupies. A press outside it is
  a tap outside. The app's `ModalFrame` may refine this shape.
- **Content rect**: where the modal's controls are laid out (absolute
  coordinates, as for `Panel` children).
- **Phase**: `Closed`, `Opening`, `Open`, `Closing`, per entry.
- **Progress**: `0.0` closed to `1.0` open, per entry.
- **Top**: the uppermost entry in `Opening` or `Open`. Entries in `Closing`
  are on their way out and are never the top.
- **Trap**: while the stack holds any entry, `ModalFilter` redirects every
  event to the `ModalStack`. Lower levels, base included, receive nothing;
  only the top's content receives input, and only while the top is `Open`.

## 4. Phase, transition, reasons, events

All modal code is in a new module `src/modal.rs`, re-exported from
`src/lib.rs`:

```rust
pub use modal::{
    Backdrop, ContentReveal, DismissOn, DismissReason, Modal, ModalEntryView, ModalEvent,
    ModalFilter, ModalFrame, ModalPhase, ModalStack, ModalState, ModalTransition, ModalView,
    PanelFrame, MAX_MODAL_DEPTH,
};
```

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

/// Why a modal began to close. Copy: it names controls by Symbol.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum DismissReason {
    /// A press and release both outside the top's frame.
    OutsideTap,
    /// Key::Escape.
    Escape,
    /// Key::Back, the platform back action.
    Back,
    /// A content control registered with Modal::with_close_control was
    /// activated; carries its name (sym!("close"), sym!("apply"), ...).
    Control(Symbol),
    /// The app called pop(), close(id), dismiss(DismissReason::App) or dismiss_all(App).
    App,
    /// The app called replace_top; the outgoing top closes with this reason.
    Replaced,
}

/// Something the modal layer reports. Taken once with ModalState::take_events.
#[derive(Copy, Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum ModalEvent {
    /// The opening transition finished; content now takes input.
    Opened { id: Symbol },
    /// The modal began to close, and why.
    Dismissed { id: Symbol, by: DismissReason },
    /// The closing transition finished; the entry is gone.
    /// Every Dismissed is followed by exactly one Closed with the same id and reason.
    Closed { id: Symbol, by: DismissReason },
    /// A user dismissal was refused because the modal is held open (10.2).
    /// The modal stays as it was.
    DismissRefused { id: Symbol, by: DismissReason },
}
```

## 5. `ModalState` (persistent, app-owned)

### 5.1 Layout

```rust
pub const MAX_MODAL_DEPTH: usize = 4;

/// The modal stack, shared between the app and every rebuild of the host.
#[derive(Debug, Default)]
pub struct ModalState {
    inner: Mutex<ModalInner>,
    /// `!inner.entries.is_empty()`, kept in step with the lock (5.4).
    occupied: AtomicBool,
}

#[derive(Debug, Default)]
struct ModalInner {
    default_transition: ModalTransition,
    entries: Vec<ModalEntry>,                 // bottom..top, at most MAX_MODAL_DEPTH
    pending: Vec<(Symbol, ModalTransition)>,  // pushes waiting for a same-id entry to finish closing
    events: Vec<ModalEvent>,
}

#[derive(Debug)]
struct ModalEntry {
    id: Symbol,                               // unique among entries not Closing
    phase: ModalPhase,                        // Opening, Open or Closing; Closed entries are removed
    progress: f32,                            // always clamped to [0, 1]
    transition: ModalTransition,
    reason: Option<DismissReason>,            // Some only in Closing
    focus: Option<ControlKey>,                // focus inside this modal's content, across rebuilds
    drag: Option<DragGrab>,                   // drag inside this modal's content, across rebuilds
    hover: Option<ControlKey>,                // hover inside this modal's content, across rebuilds
    focus_seeded: bool,                       // Modal::with_initial_focus applied since push
    outside_press: Option<MouseButton>,       // a press outside the frame awaiting its release
}
```

`hover` is new against the superseded plan: plan 02 carries hover across
rebuilds, and the content host is rebuilt with the `Modal`.

### 5.2 API

All `&self`; the `Mutex` gives interior mutability, matching `PanelData`;
`ModalState` is `Send + Sync`. Every `id` parameter is `impl Into<Symbol>`,
so `"settings"` and `sym!("settings")` both work.

```rust
impl ModalState {
    pub fn new() -> Self;
    pub fn with_transition(self, t: ModalTransition) -> Self;               // default for pushes

    pub fn push(&self, id: impl Into<Symbol>) -> bool;                      // false: duplicate id or stack full
    pub fn push_with(&self, id: impl Into<Symbol>, t: ModalTransition) -> bool;
    pub fn replace_top(&self, id: impl Into<Symbol>) -> bool;               // top closes (Replaced), id opens
    pub fn replace_top_with(&self, id: impl Into<Symbol>, t: ModalTransition) -> bool;
    pub fn dismiss(&self, by: DismissReason);                               // closes the top only; ignores hold-open
    pub fn pop(&self);                                                      // dismiss(DismissReason::App)
    pub fn close(&self, id: impl Into<Symbol>);                             // closes that entry, wherever it is (App)
    pub fn dismiss_all(&self, by: DismissReason);                           // every live entry, top first

    pub fn advance(&self, dt_secs: f32) -> bool;                            // every Timed entry; true while any animates
    pub fn set_progress(&self, id: impl Into<Symbol>, progress: f32);       // Driven, per entry (works in any mode)
    pub fn set_top_progress(&self, progress: f32);

    pub fn top(&self) -> Option<Symbol>;
    pub fn depth(&self) -> usize;                                           // entries, Closing included
    pub fn contains(&self, id: impl Into<Symbol>) -> bool;                  // live (not Closing) entry
    pub fn phase(&self, id: impl Into<Symbol>) -> ModalPhase;               // Closed if absent
    pub fn progress(&self, id: impl Into<Symbol>) -> f32;                   // 0.0 if absent
    pub fn stack(&self) -> Vec<ModalEntryView>;                             // bottom..top snapshot
    pub fn captures_input(&self) -> bool;                                   // any entry at all; lock-free (5.4)
    pub fn top_takes_input(&self) -> bool;                                  // top exists and is Open
    pub fn is_animating(&self) -> bool;
    pub fn take_events(&self) -> Vec<ModalEvent>;                           // drains the queue

    pub fn filter(self: &Arc<Self>, target: impl Into<Symbol>) -> ModalFilter;   // section 6
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ModalEntryView { pub id: Symbol, pub phase: ModalPhase, pub progress: f32, pub is_top: bool }
```

### 5.3 Rules

Each row is a step 1 test.

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

### 5.4 The `occupied` flag

`captures_input()` is asked on **every event** by the `ModalFilter`, for
every host build. It reads `occupied` with `Ordering::Acquire` and takes no
lock. Every method that can change `entries.len()` (`push`, `push_with`,
`replace_top`, `replace_top_with`, `dismiss`, `pop`, `close`, `dismiss_all`,
`advance`, `set_progress`, `set_top_progress`) ends, still under the lock,
with `occupied.store(!entries.is_empty(), Ordering::Release)`.

Why an atomic rather than the lock: an uncontended `Mutex` is cheap and the
state is only ever touched from the render thread, so either would work;
the atomic keeps the filter's cost fixed and independent of the lock, and
lets a shell ask `captures_input()` from its input thread (to decide whether
an event is the game's or the UI's) without ever blocking the render thread.
It costs one store per state change. `top_takes_input()` and the other
queries lock as before.

## 6. `ModalFilter`

The owner's framing was that "the ModalStack registering a filter would be
to deliver a private event to the ModalStack (which is a global concept, so
has a handle) via a spsc type mechanism or by injecting an event (whichever
makes sense)". Injection through `Filtered::Redirect` is what makes sense
here: delivery is synchronous on the render thread, must return an
`EventResponse`, and the modal's content only exists during the build; a
queue belongs between a shell's input thread and the render loop, outside
libcontrols. And "the control isn't registered as a filter itself": the
filter is a separate object that names the control.

```rust
/// Traps input while any modal exists by redirecting every event to the
/// hosted ModalStack registered under `target`. Created once from the state
/// and kept in app state beside the Arc; borrowed by every host build.
#[derive(Clone, Debug)]
pub struct ModalFilter {
    state: Arc<ModalState>,
    target: Symbol,
}

impl ModalFilter {
    pub fn new(state: Arc<ModalState>, target: impl Into<Symbol>) -> Self;
    pub fn state(&self) -> &Arc<ModalState>;
    pub fn target(&self) -> Symbol;
}

impl InputFilter for ModalFilter {
    fn filter(&self, _event: &InputEvent, _ctx: &FilterContext) -> Filtered {
        if self.state.captures_input() {
            Filtered::Redirect(ControlKey::Named(self.target))
        } else {
            Filtered::Pass
        }
    }
}
```

`ModalState::filter(target)` is `ModalFilter::new(Arc::clone(self), target)`.
The filter is registered with `with_filter(&modal_filter)` on every build;
registered last, it is asked first, so nothing else sees an event while a
modal exists. With no entry it answers `Pass` for the cost of one atomic
load. If the host has no control named `target` the redirect is consumed by
the host (plan 02, 9.4 step 1): input stays trapped and nothing is drawn,
which is a programming error the step 2 test makes visible.

## 7. Frames, backdrop, reveal, dismiss rules

### 7.1 `ModalView`, `ModalFrame`, `PanelFrame`

```rust
/// Everything a frame painter needs to draw one frame of one modal.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ModalView {
    pub id: Symbol,
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

`ModalView` has no lifetime now that `id` is a `Symbol`. `PanelFrame::draw_frame`
pushes the same `RoundedBox`/`Outline` instances as `Panel::draw`, every
colour's alpha multiplied by `view.progress`. A game that wants a book
implements `ModalFrame` on its own type and animates from `view.progress`,
`view.phase` and `view.is_top` (for example, a lower book drawn closed but
visible).

### 7.2 `Backdrop`, `ContentReveal`, `DismissOn`

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

### 7.3 The fade

`ContentReveal::Fade` remembers `start = list.instances.len()` before
drawing the content and afterwards multiplies `color[3]` of
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

## 8. `Modal<'a>` (built with the host)

```rust
pub struct Modal<'a> {
    state: Arc<ModalState>,
    id: Symbol,                           // the stack entry this modal is built for
    viewport: Rect,
    frame_rect: Rect,
    content_rect: Rect,
    frame: Box<dyn ModalFrame + 'a>,      // default Box::new(PanelFrame::default())
    backdrop: Option<Backdrop>,           // default Some(Backdrop::default())
    dismiss: DismissOn,                   // default DismissOn::default()
    hold_open: bool,                      // default false (10.2)
    close_controls: Vec<Symbol>,          // names that dismiss with DismissReason::Control(name)
    reveal: ContentReveal,                // default Fade
    clip_content: bool,                   // default true: push_clip(content) around the content's draw pass
    initial_focus: Option<ControlKey>,
    children: RefCell<ControlHost<'a>>,   // the content controls
}

impl<'a> Modal<'a> {
    pub fn new(state: Arc<ModalState>, id: impl Into<Symbol>, viewport: Rect, frame: Rect, content: Rect) -> Self;
    /// frame centred in viewport at `size`; content = frame.pad(-padding).
    pub fn centered(state: Arc<ModalState>, id: impl Into<Symbol>, viewport: Rect, size: [f32; 2], padding: f32) -> Self;

    pub fn with_frame<F: ModalFrame + 'a>(self, frame: F) -> Self;
    pub fn with_backdrop(self, backdrop: Option<Backdrop>) -> Self;
    pub fn with_dismiss(self, dismiss: DismissOn) -> Self;
    pub fn with_hold_open(self, hold: bool) -> Self;
    pub fn with_close_control(self, id: impl Into<Symbol>) -> Self;   // may be called more than once
    pub fn with_reveal(self, reveal: ContentReveal) -> Self;
    pub fn with_clip_content(self, clip: bool) -> Self;
    pub fn with_initial_focus(self, key: ControlKey) -> Self;
    pub fn with_control<C: Control + 'a>(self, control: ControlHandle<'a, C>) -> Self;
    pub fn with_named_control<C: Control + 'a>(self, id: impl Into<Symbol>, control: ControlHandle<'a, C>) -> Self;
    pub fn with_custom_control<H: Control + 'a>(self, control: H) -> Self;

    pub fn id(&self) -> Symbol;
    pub fn state(&self) -> &Arc<ModalState>;
    pub fn frame_rect(&self) -> Rect;
    pub fn content_rect(&self) -> Rect;
    /// None when the state has no entry for this id.
    pub fn view(&self) -> Option<ModalView>;

    /// Route one event to this modal as the top (section 10), with the
    /// modifiers the outer host saw. ModalStack calls it; public so a modal
    /// can be tested on its own.
    pub fn deliver_event(&self, event: InputEvent, modifiers: Modifiers) -> EventResponse;

    /// Tell the hovered content control, if any, that the pointer left,
    /// and record no hover for this entry. ModalStack calls it on every
    /// modal that is not the top.
    pub fn clear_hover(&self);
}

impl DrawControl for Modal<'_> {
    /// Backdrop, frame, content, content overlays (top only), frame-over-content (section 11).
    /// Draws nothing when the state has no entry for this id.
    fn draw(&self, list: &mut DrawList, ctx: &DrawContext);
}
```

A `Modal` whose id has no entry draws nothing and takes no input. The app
builds one `Modal` per entry in `state.stack()`, choosing each one's content
from its id.

## 9. `ModalStack` (a hosted control)

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

The app adds the stack to the host **by name, last**:
`.with_named_control(sym!("modals"), stack.handle())` with the same name the
`ModalFilter` was created with, after every base control so that it draws
last. It builds the stack on every host build, even when the state is empty
(an empty stack is transparent and draws nothing). An app whose UI is only
modals, like tower defense game, builds a host holding just the stack and the
filter. The stack is **never registered as a filter**.

`HostedControl for ModalStack`:

- `bounds()`: the viewport.
- `hit_test(pos)`: always `None`. The stack is never hit by area routing,
  never hovered, never focused by a press. It receives input only by
  redirect.
- `accepts_focus()`: `false`, so Tab in the base host never stops on it.
- `handle_mouse(event, ctx)`: for every built `Modal` that is not the top,
  `clear_hover()`; then if the top entry has a built `Modal`,
  `top.deliver_event(InputEvent::Mouse(*event), ctx.modifiers).action`,
  else `None`. (The event is still redirected, so it reaches nothing else;
  the host reports it handled by the stack.)
- `handle_keyboard(event, ctx)` and `handle_gesture(event, ctx)`: the same
  without the hover clearing.
- `handle_focus`, `set_hover`: nothing (neither is ever called in practice).

Everything that happens to the base host when the trap closes is plan 02's
redirect rule (9.4): on the first redirected pointer event the base control
that was hovered is told `set_hover(None)`, a base grab is dropped, and the
base focus is left exactly where it was, so when the last modal closes the
base focus is where it was when the first opened. Base controls (level 0)
receive no `handle_*` call while any entry exists, and no modifier
information either; since plan 02 no control stores modifier state (each
event carries its own), there is nothing to resync when the base becomes
top again. "I don't see why we need to resync or add events to reset the
input state."

`EventResponse` from the base host names the stack (its index and
`sym!("modals")`) and carries the inner control's `action`, as with `Panel`.
The app learns which inner control acted from `ModalState::take_events`
(`Dismissed { by: Control(id) }`) and from its control state (`take_click`),
as tinytrowel already does.

`DrawControl for ModalStack`: `draw` draws nothing; `draw_overlay` draws
every entry's `Modal` bottom to top (section 11). Because the base host
draws every control's `draw` and then every control's `draw_overlay`, and
the stack is last, the modals are above every base control and every base
overlay. A base tooltip visible when a modal opens by keyboard stays under
the backdrop until the next pointer event clears base hover (or the app
calls `host.clear_hover()` after the push). An app that draws its own scene
with libmsdf (tower defense game's tabletop) draws it first and the host last.

## 10. Input inside the top modal

### 10.1 `Modal::deliver_event(event, modifiers)`

1. Load the content host from this modal's entry:
   `children.set_focused_key(entry.focus)`,
   `children.set_active_drag(entry.drag)`,
   `children.set_hovered_key(entry.hover)`,
   `children.set_modifiers(modifiers)`. If `!entry.focus_seeded`, use
   `initial_focus` instead of `entry.focus` and set `focus_seeded = true`.
   (Per plan 02, 7.1, the restore sends `FocusEvent::Gained` to the focused
   content control once per fresh `Modal`; a second event through the same
   `Modal` sends nothing.)
2. **Escape** (key down, `Key::Escape`) with the entry `Opening` or `Open`
   and `dismiss.escape`: `request(Escape)`, return `consumed()`. With
   `dismiss.escape == false` it falls through to step 5 (the content host
   then clears its own focus, as the base host does).
   **Back** (key down, `Key::Back`) likewise with `dismiss.back` and
   `request(Back)`; disabled, it falls through to step 5 and goes to the
   focused content control. Both apply even when a content `Edit` has
   focus: "web usage usually has back/esc dismiss a modal, and we now have
   veto support if we want to protect changes".
3. **Not Open** (`Opening` or `Closing`): return `consumed()`. Nothing
   reaches the content.
4. **Tap outside** (mouse, `Open`, no content drag in progress):
   - `Down { pos, button }` with `!frame.contains(pos, view)`: set
     `outside_press = Some(button)`, return `consumed()`.
   - `Up { pos, button }` with `outside_press == Some(button)`: clear it; if
     `!frame.contains(pos, view)` and `dismiss.outside_tap`,
     `request(OutsideTap)`. Return `consumed()`.
   - `Click { pos, .. }` outside with `outside_press == None` and
     `dismiss.outside_tap`: `request(OutsideTap)`, return `consumed()`. (For
     shells that send only `Click`. A shell that sends `Down`, `Up` and
     `Click` dismisses on the `Up`; the `Click` then arrives in `Closing`
     and is swallowed by step 3.)
   - Any other mouse event outside the frame: `consumed()`.

   Requiring the press and the release outside means the release of the tap
   on the icon that opened the modal (handled by the app on `Down`) cannot
   close it, and neither can a drag that starts on a slider and ends
   outside. A tap on a lower modal's frame is outside the top.
5. **Content**: `resp = children.deliver_event(event)`. If `resp.target_id`
   is in `close_controls` and `resp.action` is
   `Some(ControlAction::Activated { .. })`, `request(Control(id))`. Return
   `resp` unchanged.
6. Store `children.focused_key()`, `children.active_drag()` and
   `children.hovered_key()` back into the entry.

`consumed()` is `EventResponse::consumed(None, None)` from plan 02.

Tab and BackTab cycle focus among the top's content controls only, because
the content host is separate and the filter redirects Tab before the base
host handles it: the focus trap needs no extra code. Focus is kept per
entry, so focus in a lower modal is where it was when the modal above it
closes.

`Modal::clear_hover()`: `children.set_hovered_key(entry.hover)`,
`children.clear_hover()`, store `None` in the entry. One call at most.

### 10.2 Requests and the hold-open veto

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

## 11. Drawing

### 11.1 `Modal::draw(list, ctx)`, when its entry exists

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

### 11.2 The two passes of `ControlHost`

```rust
impl ControlHost<'_> {
    /// Pass 1 only: every control's draw.
    pub fn draw_controls(&self, list: &mut DrawList, ctx: &DrawContext);
    /// Pass 2 only: every control's draw_overlay.
    pub fn draw_control_overlays(&self, list: &mut DrawList, ctx: &DrawContext);
}
```

`impl DrawControl for ControlHost` becomes `draw_controls` then
`draw_control_overlays`, which is what it does today, so its instance order
does not change. This small host addition lives in this plan (step 3)
rather than plan 02 because plan 02 touches no drawing and the modal is the
only caller; `Panel`'s child tooltips (plan 02, section 12) will use it in
the nested-containers plan.

## 12. `Key::Back` and Escape in a modal

`Key::Back` itself is plan 02's (its section 5). Here: in a modal, Escape
and Back both dismiss the top (10.1), reported as `DismissReason::Escape`
and `DismissReason::Back`, even when a content `Edit` has focus, as dialogs
on the web do. An app that must protect unsaved edits uses
`Modal::with_hold_open`. Without a modal, Back reaches the focused control
and does nothing special.

## 13. How tower defense game uses it (informative)

```rust
// App state, once:
let modals = Arc::new(ModalState::new().with_transition(ModalTransition::Driven));
let modal_filter = modals.filter(sym!("modals"));   // once; borrowed by every build

// The gear icon is the game's own libmsdf drawing and hit test:
if gear_tapped { modals.push(sym!("settings")); }

// Every frame:
book_anim.step(dt);                                  // the game's own book animation
modals.set_progress(sym!("settings"), book_anim.openness());
modals.advance(dt);                                  // Timed entries (a confirm)
for ev in modals.take_events() {
    match ev {
        ModalEvent::Closed { id, .. } if id == sym!("settings") => { /* persist settings */ }
        ModalEvent::DismissRefused { .. } => { modals.push(sym!("discard")); }
        _ => {}
    }
}

// Input: everything goes to the host; the filter traps while any modal exists.
if modals.captures_input() { host.deliver_event(ev); } else { tabletop.input(ev); }

// Building the host (per event / per frame), one Modal per stack entry:
let mut stack = ModalStack::new(modals.clone(), viewport);
for entry in modals.stack() {
    stack = stack.with_modal(match entry.id {
        id if id == sym!("settings") => Modal::new(modals.clone(), id, viewport, book_rect, page_rect)
            .with_frame(BookFrame { .. })            // game's ModalFrame: cover, pages, curl
            .with_reveal(ContentReveal::WhenOpen)
            .with_hold_open(settings_dirty)
            .with_close_control(sym!("close"))
            .with_named_control(sym!("volume"), volume.handle())
            .with_named_control(sym!("close"), close.handle()),
        id if id == sym!("quit") => Modal::centered(modals.clone(), id, viewport, [320.0, 160.0], 16.0)
            .with_named_control(sym!("yes"), yes.handle())
            .with_close_control(sym!("no"))
            .with_named_control(sym!("no"), no.handle()),
        _ => continue,
    });
}
let host = ControlHost::builder()
    .with_named_control(sym!("modals"), stack.handle())
    .with_filter(&modal_filter)
    .build();
// Draw: tabletop first with libmsdf, then host.draw(&mut list, &ctx) last.
```

The shell sends Android Back as `Key::Back`; it dismisses the top modal like
Escape.

## 14. Compatibility and version 0.3.0

Everything in this plan is **additive** on top of plan 02: the `modal`
module and its types, `ControlHost::draw_controls` and
`draw_control_overlays`. `ControlAction`, `EventResponse` and `HostedControl`
do not change here. With no `ModalFilter` registered and no `ModalStack`
hosted, nothing in a host behaves differently. `tests/events.rs` is
untouched by this plan.

**The version is cut here.** Plans 02 and 03 ship together as 0.3.0, once,
in this plan's last step (plan 02, section 13): `Cargo.toml` goes from
`0.2.0` to `0.3.0`, `VERSIONS.md` gains one entry in the same commit
(AGENTS.md Rule 5.3), and the commit is tagged `v0.3.0` as soon as it is
committed (Rule 5.1). The entry, headed
`## 0.3.0 — YYYY-MM-DD (<short commit>, tag v0.3.0)`, lists the public API
changes of both plans:

- **Breaking:** `ControlKey::Named` holds a `Symbol`; `ControlKey::id()`
  returns `Option<Symbol>`; `with_named_control`, `add_named_control`,
  `ControlKey::named` and `find_by_id` take `impl Into<Symbol>`;
  `EventResponse::target_id` is `Option<Symbol>`.
- **Breaking:** `Key` gains `Back` and is `#[non_exhaustive]`.
- **Breaking:** `HostedControl::handle_mouse`, `handle_keyboard` and
  `handle_gesture` take `&EventContext` in place of `ControlPart`;
  `HostedControl::set_modifiers` is removed, and
  `ControlHost::deliver_modifiers` tells no control.
- **Breaking:** `DragGrab` gains `press_modifiers` and is
  `#[non_exhaustive]` (`DragGrab::new`).
- **Breaking:** Tab, BackTab and Escape key releases no longer reach the
  focused control; hover is no longer re-sent to every control on every
  pointer event (apps that rebuild per event and deliver `Move` carry
  `hovered_key()`).
- Added: `Symbol` and `sym!`; `FocusEvent` and `HostedControl::handle_focus`,
  `accepts_focus`; `EventContext`; `InputFilter`, `Filtered`,
  `FilterContext`, `ControlHostBuilder::with_filter`;
  `ControlHost::{set_modifiers, hovered_key, set_hovered_key, clear_hover,
  draw_controls, draw_control_overlays}`; `EventResponse::consumed`;
  `Panel::{with_hovered_key, hovered_key}`; `Copy` on `ControlKey` and
  `DragGrab`; `Default` on `ControlPart`.
- Added: the `modal` module: `ModalState`, `ModalFilter`, `ModalStack`,
  `Modal`, `ModalFrame`, `PanelFrame`, `Backdrop`, `ContentReveal`,
  `DismissOn`, `DismissReason`, `ModalEvent`, `ModalPhase`,
  `ModalTransition`, `ModalView`, `ModalEntryView`, `MAX_MODAL_DEPTH`.

## 15. Steps

Each step is one commit touching only this crate and passing
`cargo fmt -p libcontrols -- --check`, `cargo clippy -p libcontrols
--all-targets` with no new warnings, and `cargo test -p libcontrols`
(Rule 3.4). Plan 02 is implemented first; this plan's first commit may land
only after plan 02's step 7. New tests go in `tests/modal.rs`, in the style
of `tests/events.rs`: small recorder controls logging every `handle_*`,
`set_hover` and `handle_focus` call into a `RefCell<Vec<_>>`, a recording
`ModalFrame`, `ControlHost::builder()`, plain asserts with messages saying
what must hold.

1. **`ModalState`.** Add `src/modal.rs` with the section 4 types,
   `ModalState`, `ModalEntryView` and the `occupied` flag (5.1 to 5.4);
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
   `events_are_taken_once`, `advance_ignores_negative_and_nan_dt`,
   `captures_input_follows_the_entries_without_a_lock` (true from the push
   through `Closing`, false the moment the last entry is removed),
   `ids_are_symbols_and_events_are_copy`.
   **Done when:** every row of the 5.3 table is covered by a passing test
   and event order is exactly as stated there.

2. **`ModalFilter`** (section 6).
   Tests: `the_filter_passes_while_the_state_is_empty`,
   `the_filter_redirects_to_its_target_while_any_modal_exists` (a recorder
   named `modals` receives every mouse, keyboard and gesture event in
   `Opening`, `Open` and `Closing`; a recorder beneath it receives nothing),
   `a_modifier_change_is_consumed_while_a_modal_exists`,
   `a_filter_whose_target_is_not_hosted_still_traps_input`,
   `one_filter_serves_many_builds`.
   **Done when:** those pass with the filter created once and borrowed by
   three successive hosts.

3. **`Modal`, `ModalFrame`, `PanelFrame`, `Backdrop`, `ContentReveal`,
   drawing, and the split draw passes** (sections 7, 8, 11).
   `Modal::deliver_event` routes straight to the content host for now.
   Tests: `a_modal_with_no_entry_draws_nothing`,
   `the_backdrop_covers_the_viewport_and_fades_with_progress`,
   `no_backdrop_means_no_backdrop_instance`,
   `the_frame_painter_is_called_with_the_view` (a recording `ModalFrame`
   sees id, phase, progress, frame, content, depth and is_top),
   `when_open_content_is_hidden_while_opening`,
   `fade_scales_content_alpha_by_progress`,
   `content_is_clipped_and_its_tooltips_are_not` (checks
   `DrawList::instance_clip` for a content instance and a tooltip instance),
   `only_the_top_draws_content_tooltips`,
   `split_draw_passes_match_draw` (a host's `draw` equals `draw_controls`
   then `draw_control_overlays`, instance for instance).
   **Done when:** a modal at progress 0.5 with `Fade` emits backdrop, frame,
   then content instances, in that order, with content alpha halved, and a
   modal with no entry emits nothing.

4. **Input and dismissal inside `Modal::deliver_event`** (section 10).
   Tests: `escape_dismisses_and_a_second_escape_does_nothing`,
   `back_dismisses_with_back`, `escape_and_back_dismiss_with_an_edit_focused`,
   `escape_reaches_the_content_when_disabled`, `back_can_be_turned_off`,
   `a_press_and_release_outside_dismisses`,
   `a_release_outside_without_a_press_outside_does_not_dismiss`,
   `a_drag_from_a_slider_to_outside_does_not_dismiss`,
   `a_click_outside_dismisses_for_click_only_shells`,
   `outside_tap_can_be_turned_off`,
   `a_close_control_dismisses_with_its_symbol`,
   `hold_open_refuses_user_dismissals_and_reports_them`,
   `hold_open_does_not_stop_app_dismissals`,
   `a_disabled_action_is_not_reported_as_refused`,
   `content_receives_nothing_while_opening_or_closing`,
   `focus_drag_and_hover_inside_the_modal_survive_a_rebuild`,
   `initial_focus_is_applied_once_per_push`,
   `the_content_sees_the_modifiers_the_outer_host_saw`,
   `tab_cycles_the_content_only`,
   `a_frame_shape_decides_what_is_outside`.
   **Done when:** every user dismissal path produces exactly one `Dismissed`
   with the right `DismissReason`, or exactly one `DismissRefused` when held
   open, and no event reaches a content control outside `Open`.

5. **`ModalStack`** (section 9).
   Tests: `an_empty_stack_is_transparent_and_draws_nothing`,
   `tab_in_the_base_skips_the_stack`,
   `base_controls_receive_nothing_while_any_modal_exists` (no `handle_*`
   in each of Opening, Open and Closing),
   `base_hover_is_left_and_a_base_grab_dropped_on_the_first_redirected_event`,
   `base_focus_is_unchanged_after_a_modal_closes`,
   `lower_modals_receive_nothing_and_lose_their_hover`,
   `escape_and_back_close_only_the_top`,
   `a_tap_on_a_lower_modal_is_outside_the_top`,
   `focus_is_kept_per_entry`,
   `modals_are_ordered_by_the_stack_not_by_build_order`,
   `input_is_consumed_when_the_top_has_no_modal_built`,
   `the_response_names_the_stack_and_carries_the_inner_action`,
   `each_level_draws_its_own_backdrop_bottom_to_top`,
   `the_stack_draws_after_every_base_overlay`,
   `a_host_with_only_a_stack_and_its_filter_works`.
   **Done when:** with two entries at progress 1.0 over a base `Button` with
   a tooltip, the list holds the base control, its overlay, then backdrop,
   frame and content of the lower modal, then backdrop, frame and content of
   the upper one, in that order; and a base recorder logs no `handle_*` call
   and exactly one `set_hover(None)` while either entry exists.

6. **README, 0.3.0, compatibility check.** Add a "Modals" section to
   `README.md` above `## Building`: the filter and the stack, a two-level
   example (settings, then a confirm pushed over it) in neutral terms, the
   veto. Bump `Cargo.toml` to `0.3.0`, append the section 14 entry to
   `VERSIONS.md` in the same commit, and tag the commit `v0.3.0`. In a
   scratch copy of tinytrowel (not committed), replace `deps/libcontrols/src`
   and its `Cargo.toml` with this crate's and run `cargo check -p
   trowel-game`; if it fails, the fix is proposed to tinytrowel's owner, not
   made here.
   **Done when:** the README example is mirrored by a test in
   `tests/modal.rs` (`the_readme_two_level_example_runs`), `trowel-game`
   checks clean with no change to tinytrowel (or the needed change is
   written up), and `git tag` lists `v0.3.0` on the bumping commit.

## 16. Decisions carried from the modal plan's open questions

| Modal plan question | Decision |
|---|---|
| 1. One modal or a stack | A stack of up to four (5.1). |
| 2. Platform back key | `Key::Back`, breaking, 0.3.0 (plan 02, section 5; here section 12). |
| 3. Modal events in `ControlAction` | No: events only through `take_events`. |
| 4. Modifiers beneath a modal | Full trap: lower levels, base included, get nothing; no resync. Since plan 02 no control stores modifier state, so nothing is stale (section 9). |
| 5. Escape with a focused text field | Escape and Back dismiss even then; `with_hold_open` protects edits (10.1, 10.2). |
| 6. Vetoing a dismissal | `with_hold_open` and `ModalEvent::DismissRefused` (10.2). |
| 7. Scrolling content | A separate plan; out of scope here. |
| 8. Version and delivery | 0.3.0, cut in this plan's step 6; how an app consumes the crate is not libcontrols' concern. |
| 9. Group alpha | Not needed: post-hoc alpha fades every instance kind; limits in 7.3. |
| 10. Default transition timing | `Timed { 0.18, 0.12 }` stays. |

## 17. Open questions

1. **Clearing base hover on a keyboard push.** A modal pushed from a key
   (no pointer event follows) leaves a base tooltip drawn under the
   backdrop until the pointer moves. The app can call `host.clear_hover()`
   after the push; should `ModalStack` do it instead, for example from its
   `handle_keyboard` on the first redirected event, or is the app's call
   enough?
2. **Filters inside a modal.** `Modal` exposes no `with_filter` for its
   content host. A shortcut layer scoped to one modal (Ctrl+S in a settings
   book) would want one. Add it now, or when the first such filter exists?
3. **Default tooltip bounds for content.** Content tooltips are drawn
   outside the clip and may leave the frame. Should `Modal` set
   `TooltipStyle::bounds` to the frame for content controls that leave it
   `None`, or stay out of the controls' styles?
4. **`DismissReason::Control` for non-`Activated` actions.** A close
   control counts only when its action is `Activated`. A `Radio` cell or an
   `Options` toggle also report `Activated`; should a close control be
   allowed to name a part, so only one cell of a strip closes the modal?
5. **Backdrop compounding.** Four levels at 0.45 alpha each leave 9% of the
   tabletop visible. Keep compounding (each level is independent), or let
   an app ask for one backdrop under the top only?
