---
title: Input routing and filters
date: 2026-10-16
status: Implemented
covers: >
  How a ControlHost routes input in 0.3.0. A filter stack that sees every
  event first (InputFilter, Filtered::{Pass, Consumed, Redirect}, registered
  on the builder, never a hosted control); area-based routing for pointer
  events and gestures with a press grab; focus-based routing for keys; focus
  changes delivered as events (FocusEvent, HostedControl::handle_focus);
  hover as enter and leave of an area, with the hovered key carried across
  rebuilds; modifiers carried on every event in an EventContext instead of
  being broadcast (HostedControl::set_modifiers removed); interned Symbols in
  place of String control names (Symbol, sym!); Key::Back. The per-event
  cost, the 0.3.0 compatibility break, when the version is cut, and what
  moves to a later "focus manager: nested containers" plan.
supersedes: docs/design/20261016_01_MODALS_AND_INPUT_FILTERS_PLAN.md (its filter design, sections 4, 10, 12 and 13; its modal design goes to 20261016_03_MODAL_STACK_PLAN.md)
sources:
  - docs/design/20261016_01_MODALS_AND_INPUT_FILTERS_PLAN.md (Modals and input filters; superseded by this plan together with 20261016_03_MODAL_STACK_PLAN.md)
  - "Owner: \"rethink and have fable ... write a clean plan, split into filters and modal, supercede the current one\""
  - "Owner: \"losing focus is a valid event, the focus manager needs to send those events, and a trait method is the way we write the handlers for them. as far as intercept asking every control, I wanted to prevent that, this is running inside a gpu render loop. the point of the focus manager and filter stack is to prevent the common UI deep event delivery problem.\""
  - "Owner: \"filter stack gets global events, controls have an area they receive area-based events for (mouse) and key events based on focus\""
  - "Owner: \"except it shouldn't use strings, it should use the interned string/symbols\""
  - "Owner: \"trait objects added to the filter stack have a cost, the code only does it if there's a reason, so it can intercept any key if it choses to. yes, modifiers shouldn't be separate key events in most cases (maybe that's opt-in?) a standard libcontrols user doesn't need to know when the shift key is pressed\""
  - "Owner: \"modifiers should also be passed with mouse events, even though they are keyboard events, since modified drags are common gestures\""
  - "Owner: \"the control isn't registered as a filter itself\""
  - "Owner: \"the ModalStack registering a filter would be to deliver a private event to the ModalStack (which is a global concept, so has a handle) via a spsc type mechanism or by injecting an event (whichever makes sense)\" (decided as injection through Filtered::Redirect, section 9.4)"
  - src/host.rs at d193afd (HostedControl, ControlHandle forwarding, ControlHostBuilder, ControlKey, DragGrab, ControlHost routing: update_hover, deliver_modifiers, deliver_mouse, deliver_keyboard, resolve/find_by_id)
  - src/event.rs (InputEvent, MouseEvent, KeyboardEvent, Key, Modifiers, ControlPart, ControlAction, EventResponse)
  - src/panel.rs (Panel forwarding to a child ControlHost), src/slider.rs (the Move arm of Slider::handle_mouse), src/slider_numeric.rs (a control composed of controls), src/handle.rs, src/lib.rs (unit tests that exercise routing)
  - tests/events.rs (test style; the Legacy and Recorder controls)
  - ~/src/tinytrowel/crates/trowel-game/src/ui.rs (per-event host rebuild; line 211 `.with_named_control("speed", ...)`, line 264 `resp.target_id.as_deref() == Some("speed")`)
related:
  - docs/design/20261016_03_MODAL_STACK_PLAN.md (Modal stack; depends on this plan and ships with it as 0.3.0; it cuts the version, section 13)
  - docs/design/20261015_01_MODAL_PLAN.md (Modal layer; superseded earlier; background only)
  - A future "focus manager: nested containers" plan (section 12)
---

# Input routing and filters

## 1. Purpose and scope

libcontrols runs inside a GPU render loop. Every event the shell delivers is
handled on the render thread between frames, and hosts are rebuilt per event
or per frame (tinytrowel builds a `ControlHost` for every pointer event). The
routing in 0.2.0 does work proportional to the number of controls on every
event: it tells every control where the pointer is, broadcasts every modifier
change to every control, hit-tests twice, and resolves named keys by string
comparison. The superseded plan would have added one more per-control call
per event (`HostedControl::intercept`). The owner's direction is the
opposite: "the point of the focus manager and filter stack is to prevent the
common UI deep event delivery problem."

This plan fixes the routing contract of a single host:

- A **filter stack** sees every event first. Only registered filter objects
  are consulted; controls are never asked. With no filter registered the
  stack costs one length check.
- **Area-based events** (pointer and gestures) go to the one control whose
  area contains the pointer, or to the control a press grabbed.
- **Focus-based events** (keys) go to the focused control; Tab, BackTab and
  Escape belong to the focus manager.
- **Focus changes are events** a control handles in a trait method.
- **Hover is enter and leave**: at most two controls hear about a pointer
  move.
- **Modifiers ride on every event** in an `EventContext`; no control is told
  that Shift went down.
- **Symbols replace strings** for control names, keys and response ids.
- **`Key::Back`** exists.

Out of scope, named in section 12: routing through nested containers,
`Panel`'s hover and tab-order defects, tabbed containers, scrolling
containers, rewriting events in filters. Modals are the subject of
`20261016_03_MODAL_STACK_PLAN.md`, which is built on this plan's
`Filtered::Redirect` and ships with it as 0.3.0.

## 2. Constraints from the code

Read at `d193afd`; line numbers refer to that commit.

1. **Hosts are rebuilt per event and per frame.** `ControlHost<'a>` borrows
   its controls; tinytrowel builds and drops one per pointer event
   (`ui.rs:210` to `248`). State that must survive a rebuild lives in `Arc`
   state objects or is carried by the app through `focused_key()` /
   `set_focused_key()` and `active_drag()` / `set_active_drag()`
   (`host.rs:256`, `261`, `329`, `334`). Anything new that must survive
   follows that pattern.
2. **Hover is a walk.** `update_hover` (`host.rs:381`) hit-tests, then calls
   `set_hover` on every control, on every mouse event while no drag is
   active. `deliver_mouse` then hit-tests again (`host.rs:438`).
3. **Modifiers are a broadcast.** `deliver_modifiers` (`host.rs:372`) calls
   `set_modifiers` on every control. No built-in control reads modifiers;
   `Panel` forwards them to its child host (`panel.rs:495`), which broadcasts
   again.
4. **Named keys resolve by string.** `resolve` (`host.rs:242`) calls
   `find_by_id`, a linear scan comparing `String`s, and `focused_index()`
   (`host.rs:250`) resolves on every call; `deliver_mouse` and
   `deliver_keyboard` call it several times per event.
5. **`Move` must not reach a control outside a grab.** `Slider::handle_mouse`
   treats `MouseEvent::Move { pos }` as a value change (`slider.rs:313`), so
   `deliver_mouse` returns early for `Move` when nothing is grabbed
   (`host.rs:433`); the test
   `sweeping_across_sliders_with_nothing_grabbed_moves_none_of_them` guards
   it. This plan keeps that rule.
6. **The grab.** A `Down` on a hit control records a `DragGrab`
   (`host.rs:446`). While it stands, `Move`, `Drag` and the `Up` of the
   grabbing button go to the grabbed control wherever the pointer is; other
   pointer events route by area as usual (`host.rs:398` to `431`).
7. **Tab, BackTab and Escape** are taken by the host on key down
   (`host.rs:467` to `506`); their key up falls through to the focused
   control. Nothing tells a control that it gained or lost focus.
8. **Control names are `String`s** in `ControlHostBuilder::control_ids`,
   `ControlHost::control_ids`, `ControlKey::Named`, `find_by_id(&str)` and
   `EventResponse::target_id: Option<String>`; every response clones the
   id (`host.rs:366`, `405`, `419`, `440`, `509`).
9. **`Key` is exhaustive**, so adding `Back` is breaking. `tinytrowel` does
   not name `libcontrols::Key` (its `Key::Escape` in
   `trowel-desktop/src/main.rs:496` is winit's).
10. **The only `HostedControl` implementations** are the eleven controls in
    `src/` (`ArrowLabel`, `Button`, `Edit`, `Label`, `Options`, `Panel`,
    `Radio`, `SelectableLabel`, `Slider`, `SliderNumeric`, and
    `ControlHandle`'s forwarding in `host.rs:58`), and `Legacy` and
    `Recorder` in `tests/events.rs`. tinytrowel and Noble's Keep implement
    none, so changing the trait's signatures costs those thirteen edits.
11. **`SliderNumeric` composes controls** by calling their `handle_mouse`
    directly with a part it chooses (`slider_numeric.rs:370` on). A context
    passed to handlers must be cheap to re-derive with another part.

## 3. The routing contract

For one `ControlHost`. Events enter through `deliver_event` or one of
`deliver_mouse`, `deliver_keyboard`, `deliver_gesture`, `deliver_modifiers`;
all five follow this contract.

1. **Record.** A mouse event sets the host's `pointer`; a modifiers event
   sets its `modifiers`. This happens before anything else, so the filter
   context and every `EventContext` describe the state including this event.
2. **Filters first.** The registered filters are asked in order, the last
   registered first. Each answers `Pass`, `Consumed(action)` or
   `Redirect(key)` (section 9). The first answer that is not `Pass` ends the
   pass. Only registered filter objects are consulted; no hosted control is
   ever asked. With no filters registered nothing happens here.
3. **Area-based events.** A pointer event (`Move`, `Down`, `Up`, `Click`,
   `Scroll`, `ScrollPixels`, `Drag`) or a gesture goes to the one control
   whose `hit_test` accepts the pointer, searching from the last-added
   control down and stopping at the first hit, with these exceptions:
   - a `Down` on a hit control **grabs** it (`DragGrab`); while the grab
     stands, `Move`, `Drag` and the `Up` of the grabbing button go to that
     control wherever the pointer is, and the `Up` releases the grab;
   - a `Move` outside a grab is a hover event only (section 8) and reaches
     no handler (constraint 5);
   - a `Down` or `Click` on a hit control gives it the focus (section 7); on
     no control it clears the focus.
4. **Focus-based events.** A keyboard event goes to the focused control.
   `Key::Tab` and `Key::BackTab` move the focus and `Key::Escape` clears it;
   those three, down and up, reach no control unless a filter takes them.
   `Key::Back` is an ordinary key.
5. **Modifier changes reach no control.** A modifiers event is recorded,
   shown to the filters, and dropped.
6. **Focus changes are events.** Whenever the host's focused control
   changes, the control that had the focus is told `FocusEvent::Lost` and
   the one that gets it `FocusEvent::Gained` (section 7).
7. **Hover is enter and leave.** On a pointer event, only the control the
   pointer left and the control it entered are told, plus the same control
   when its hovered part changes (section 8).

Nested containers (a `Panel` holding a child host) are out of scope: the
contract applies to each host separately, and a container forwards what it
receives to its child host as `Panel` does today. Section 12 names the plan
that addresses the levels together.

## 4. Symbols

A control's name is interned once and carried as a `Copy` integer.

### 4.1 `Symbol`

New module `src/symbol.rs`, re-exported as `libcontrols::Symbol`; the macro
is exported at the crate root as `libcontrols::sym!`.

```rust
/// An interned string. Equal symbols are the same name; comparing two is an
/// integer compare. Names are never freed: the UI vocabulary is small.
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Symbol(u32);

impl Symbol {
    /// The symbol for `name`, interning it on first sight.
    pub fn intern(name: &str) -> Symbol;
    /// The name. Lives as long as the process.
    pub fn as_str(self) -> &'static str;
    /// The symbol's index in the interner: dense from 0, in order of first
    /// interning. Stable within a process only.
    pub const fn id(self) -> u32;
}

impl From<&str> for Symbol;           // interns on every call
impl From<String> for Symbol;
impl From<&String> for Symbol;
impl std::ops::Deref for Symbol { type Target = str; }   // so Option<Symbol>::as_deref() is Option<&str>
impl AsRef<str> for Symbol;
impl PartialEq<str> for Symbol;
impl PartialEq<&str> for Symbol;
impl std::fmt::Display for Symbol;    // the name
impl std::fmt::Debug for Symbol;      // the name, quoted: "speed"

/// The symbol for a string literal, interned once per call site.
#[macro_export]
macro_rules! sym {
    ($name:literal) => {{
        static SYM: ::std::sync::OnceLock<$crate::Symbol> = ::std::sync::OnceLock::new();
        *SYM.get_or_init(|| $crate::Symbol::intern($name))
    }};
}
```

`Ord` is interning order, for use as a map key, not for display. `Symbol`
does **not** implement `Borrow<str>`: its `Hash` is the integer's, which
would disagree with `str`'s hash in a `HashMap`.

### 4.2 The interner

One process-wide interner:

```rust
struct Interner {
    by_name: HashMap<&'static str, u32>,
    names: Vec<&'static str>,            // index = Symbol.0
}
static INTERNER: OnceLock<RwLock<Interner>> = OnceLock::new();
```

`intern(name)`: take the read lock and look `name` up; on a miss take the
write lock, look it up again (another thread may have added it), and if it
is still absent `Box::leak(name.to_owned().into_boxed_str())`, push it on
`names` and insert it in `by_name`. `as_str(self)`: read lock, `names[id]`.
`Symbol` is `Send + Sync` (it is a `u32`). `sym!` interns once per call site
through a `static OnceLock`, so a hot path pays one atomic load and no hash
or lock. `Symbol::from(&str)` hashes and takes the read lock on every call;
it is kept so that `"speed"` still compiles where a name is expected.
`as_str` takes the read lock too, so a hot comparison should compare symbols
(`resp.target_id == Some(sym!("speed"))`), not strings; the string form still
works.

### 4.3 Where symbols replace strings

| 0.2.0 | 0.3.0 |
|---|---|
| `ControlKey::Named(String)` | `ControlKey::Named(Symbol)`; `ControlKey` becomes `Copy` |
| `ControlKey::named(impl Into<String>)` | `ControlKey::named(impl Into<Symbol>)` |
| `ControlKey::id(&self) -> Option<&str>` | `ControlKey::id(self) -> Option<Symbol>` |
| `ControlHostBuilder::with_named_control(impl Into<String>, ..)` | `impl Into<Symbol>` |
| `ControlHost::add_named_control(impl Into<String>, ..)` | `impl Into<Symbol>` |
| `ControlHost::find_by_id(&str)` | `find_by_id(impl Into<Symbol>)` |
| `ControlHost::control_ids: Vec<Option<String>>` (private) | `Vec<Option<Symbol>>` |
| `EventResponse::target_id: Option<String>` | `Option<Symbol>` |
| `EventResponse::handled(.., target_id: Option<String>, ..)` | `Option<Symbol>` |
| `Panel::with_named_control(impl Into<String>, ..)` | `impl Into<Symbol>` |
| `DragGrab` (`Clone`) | `Copy` (its key is) |

Nothing clones a `String` per event any more. `ControlAction::Custom(String)`
stays: it is an app payload, not a name.

tinytrowel's two lines compile unchanged: `with_named_control("speed", ..)`
takes `impl Into<Symbol>` and `&str` converts; `resp.target_id.as_deref()`
is `Option<&str>` through `Deref`, compared with `Some("speed")`. Step 8
verifies it.

## 5. `Key::Back` (breaking)

1. `Key` gains `Back`: "the platform back action (Android Back, a browser's
   back)". Shells send it as `KeyboardEvent::KeyDown { key: Key::Back }`,
   no longer as `Escape`.
2. `Key` becomes `#[non_exhaustive]`, so later keys are not breaking. Every
   in-crate `match` on `Key` already ends in `_` (`arrow_label.rs:376`,
   `options.rs:603`, `radio.rs:480`, `selectable_label.rs:447`,
   `slider.rs:358`, and the `if let Some(Key::Enter | Key::Space)` forms).
3. Without a modal `Back` is not special: it goes to the focused control
   like any other key and does not clear focus. Only the modal stack (plan
   03) gives it a meaning.

## 6. Modifiers and the `EventContext`

### 6.1 No modifier events for controls

`HostedControl::set_modifiers` is **removed**, not kept as a dead default: a
defaulted method that the host never calls would silently disable any
override. `InputEvent::Modifiers(Modifiers)` stays as host input. The host
records it (`ControlHost::modifiers()` still reads it back) and shows it to
the filters, which are the opt-in; no control is told. A redirected
modifiers event is consumed (section 9.4).

A container seeds its child host rather than forwarding a change it never
hears about:

```rust
impl ControlHost<'_> {
    /// Record the modifier keys held without telling any filter: for a
    /// container seeding its child host from an EventContext, or an app
    /// seeding a rebuilt host from the previous one's modifiers().
    pub fn set_modifiers(&mut self, modifiers: Modifiers);
}
```

A host rebuilt per event starts with no modifiers held. An app that rebuilds
per event and wants modified drags carries `modifiers()` across the rebuild
as it carries `focused_key()`; one that delivers `InputEvent::Modifiers` to
each new host gets the same effect.

### 6.2 `EventContext`

Modifiers are passed with every pointer event, gesture and key, "since
modified drags are common gestures". The bare `hit_part` argument becomes a
context:

```rust
/// What the host knows about an event when it hands it to a control.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct EventContext {
    /// The part of the control the event is for. Body for keys.
    pub part: ControlPart,
    /// The modifier keys held at this event. A Shift pressed mid-drag shows
    /// here on the next Drag.
    pub modifiers: Modifiers,
    /// The modifiers held at the press that started the grab this control
    /// holds; None when this control holds no grab.
    pub press_modifiers: Option<Modifiers>,
}

impl EventContext {
    pub fn new(part: ControlPart) -> Self;                       // modifiers default, no press
    pub fn with_part(self, part: ControlPart) -> Self;           // for composed controls (SliderNumeric)
    pub fn with_modifiers(self, modifiers: Modifiers) -> Self;
    pub fn with_press_modifiers(self, press: Option<Modifiers>) -> Self;
}
```

`ControlPart` gains `#[derive(Default)]` with `Body` as the default, so
`EventContext: Default`. Outside the crate (`tests/`, apps composing
controls) a context is built with `EventContext::new(part)` and the `with_*`
methods, or `EventContext::default()` followed by field assignment; the
struct literal is closed by `#[non_exhaustive]`.

`press_modifiers` comes from the grab, which now records them:

```rust
/// A drag in progress, in a form that can be carried across a rebuild.
#[derive(Copy, Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct DragGrab {
    pub key: ControlKey,
    pub start_pos: [f32; 2],
    pub button: MouseButton,
    /// The modifiers held at the press.
    pub press_modifiers: Modifiers,
}

impl DragGrab {
    pub fn new(key: ControlKey, start_pos: [f32; 2], button: MouseButton) -> Self;  // no modifiers
    pub fn with_press_modifiers(self, modifiers: Modifiers) -> Self;
}
```

`DragGrab` becomes `#[non_exhaustive]` for the same reason `Key` does; the
in-crate tests that build one as a literal (`lib.rs:626`, `700`) use
`DragGrab::new`.

### 6.3 The handler signatures

```rust
pub trait HostedControl {
    fn bounds(&self) -> Rect;
    fn hit_test(&self, pos: [f32; 2]) -> Option<(ControlPart, f32)>;
    fn handle_mouse(&self, event: &MouseEvent, ctx: &EventContext) -> Option<ControlAction>;
    fn handle_keyboard(&self, event: &KeyboardEvent, ctx: &EventContext) -> Option<ControlAction>;
    fn handle_gesture(&self, _event: &GestureEvent, _ctx: &EventContext) -> Option<ControlAction> { None }
    fn set_hover(&self, _hover: Option<ControlPart>) {}
    fn handle_focus(&self, _event: FocusEvent) {}                 // section 7
    fn accepts_focus(&self) -> bool { true }                      // section 7
}
```

`ControlHandle`'s forwarding gains the two new methods and loses
`set_modifiers`. What the host puts in the context:

| Delivery | `part` | `modifiers` | `press_modifiers` |
|---|---|---|---|
| area-routed pointer event | the hit part | host's | `Some` if this control holds the grab |
| grabbed pointer event | the control's `hit_test` at the pointer, else `Body` | host's | `Some(grab.press_modifiers)` |
| key to the focused control | `Body` | host's | `Some` if the focused control holds the grab |
| gesture | the hit part at the last pointer | host's | `Some` if this control holds the grab |
| redirected (9.4) | as the row above for its kind | host's | `Some` only if the target holds the grab |

`SliderNumeric` passes `ctx.with_part(..)` to the controls it composes.
`Panel` seeds its child host (`children.set_modifiers(ctx.modifiers)`) before
forwarding each event, so children see the modifiers the parent saw.

## 7. Focus events and focus stops

### 7.1 `FocusEvent`

```rust
/// The host's focus arrived at this control or left it.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum FocusEvent {
    Gained,
    Lost,
}
```

The host sends them on **every change of its focused control**, from
whatever cause: `Tab`, `BackTab`, `Escape`, a `Down` or `Click` on a
control or on nothing, `clear_focus`, `set_focused_index`,
`set_focused_key`, `focus_next`, `focus_prev`, and `build()` applying
`with_initial_focus`. Order: `Lost` to the control that had it, then
`Gained` to the one that gets it; either is skipped when there is no such
control. Setting the focus to the control that already has it (same index)
sends nothing.

**A rebuilt host.** A fresh host's focus is `None`. Restoring carried focus
(`set_focused_key(Some(key))` on it) is a change from `None` from that
host's point of view: the control is told `Gained` and nobody is told
`Lost`. The host cannot know whether the control already held the focus in
a previous host, and a first `with_initial_focus` genuinely is news, so the
rule is the same for both. Consequences, which handlers must tolerate:

- a control in an app that rebuilds per event receives one `Gained` per
  build while it holds the focus: handlers are idempotent;
- a `Gained` may arrive without a prior `Lost`, and a control that is no
  longer built when the focus moves (its key fails to resolve) receives no
  `Lost`: a control that commits something on `Lost` cannot rely on it
  across a rebuild that drops it;
- a `set_focused_key` whose key does not resolve leaves the focus `None` and
  sends nothing.

Cost: at most two `handle_focus` calls per focus change, one per build for a
per-build app; none while the focus stays put.

No built-in control overrides `handle_focus` in this plan. The `Recorder`
test control logs it. `Edit` (commit on blur) and `Panel` (child focus) are
expected to in later plans; see section 12.

### 7.2 `accepts_focus`

`HostedControl::accepts_focus(&self) -> bool`, default `true`. `focus_next`
and `focus_prev` skip controls that return `false` (at most one call per
control per Tab, and only until a taker is found; if every control refuses,
the focus becomes `None`). A `Down` or `Click` on a control that refuses
focus leaves the focus where it was, rather than taking or clearing it.
Programmatic sets (`set_focused_index`, `set_focused_key`,
`with_initial_focus`) are honoured regardless; the app knows what it is
doing. No built-in control overrides it in this plan; plan 03's `ModalStack`
is the first to return `false`, so Tab in the base host never stops on an
invisible control.

## 8. Hover as enter and leave

### 8.1 The rule

On every pointer event that reaches routing (not consumed by a filter) while
no grab stands, the host hit-tests once and compares the result with the
hovered control it remembers:

| Remembered | Hit now | Calls |
|---|---|---|
| none | none | none |
| none | `(j, p)` | `controls[j].set_hover(Some(p))` |
| `(i, p)` | `(i, p)` | none |
| `(i, p)` | `(i, q)`, `q != p` | `controls[i].set_hover(Some(q))` |
| `(i, _)` | `(j, p)`, `j != i` | `controls[i].set_hover(None)`, then `controls[j].set_hover(Some(p))` |
| `(i, _)` | none | `controls[i].set_hover(None)` |

Then it remembers the new hit. While a grab stands hover is frozen, as in
0.2.0 (the grabbed control keeps whatever hover it had until the release and
the next pointer event). The hit from this test is reused for routing, so a
pointer event hit-tests once, not twice.

Part changes matter because `Radio` and `Options` hover per cell
(`ControlPart::Custom(index)`) and `Slider` per `Track`/`Thumb`.

### 8.2 Carrying hover across a rebuild

A fresh host remembers no hover, so without help the control the pointer was
over in the previous build would never be told it left. Following the
focus and drag pattern:

```rust
impl ControlHost<'_> {
    /// The hovered control as a key, to be carried to the next build.
    pub fn hovered_key(&self) -> Option<ControlKey>;
    /// Put the hover back on the control it was on before this rebuild.
    /// Tells no control anything; the part is re-sent on the next pointer event.
    pub fn set_hovered_key(&mut self, key: Option<ControlKey>);
    /// Tell the hovered control, if any, that the pointer left, and forget it.
    pub fn clear_hover(&mut self);
}

impl<'a, S> Panel<'a, S> {
    pub fn with_hovered_key(self, key: Option<ControlKey>) -> Self;
    pub fn hovered_key(&self) -> Option<ControlKey>;
}
```

`set_hovered_key` resolves the key as `set_focused_key` does (a key that no
longer resolves is dropped) and records the part as unknown, so the first
pointer event after a rebuild re-sends `set_hover(Some(part))` to the same
control once (row four of the table, with the unknown part counting as
different). `clear_hover` is one call, not a walk.

**Compatibility note.** In 0.2.0 the walk cleared stale hover on every event.
An app that rebuilds its host per event *and* delivers `Move` must now carry
`hovered_key()` into the next build or a control the pointer has left keeps
its hover (and its tooltip). tinytrowel delivers no `Move`
(`PointerPhase::Hover => return None`, `ui.rs:227`), so nothing changes for
it. An app that keeps its host across events needs nothing.

### 8.3 Hover and filters

A filter that consumes pointer events does not touch hover; a filter has no
host. An app that wants stale base hover cleared while a filter holds the
pointer calls `host.clear_hover()`. A redirected pointer event moves the
hover off any control that is not the target (9.4), which is what the modal
stack relies on.

## 9. Filters

### 9.1 Types

In `src/event.rs` (`Filtered`, `FilterContext`) and `src/host.rs`
(`InputFilter`, registration), all re-exported:

```rust
/// A filter's answer for one event.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Filtered {
    /// Not mine: ask the next filter down, then route as usual.
    Pass,
    /// Spent: no control sees it. The action, if any, is returned to the app.
    Consumed(Option<ControlAction>),
    /// Deliver it to this control alone, through its normal handlers (9.4).
    Redirect(ControlKey),
}

/// What the host knows when it asks a filter. Nothing per control.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct FilterContext {
    /// The pointer, after recording this event.
    pub pointer: Option<[f32; 2]>,
    /// The modifiers held, after recording this event.
    pub modifiers: Modifiers,
    /// The focused control, if any.
    pub focused: Option<ControlKey>,
    /// The control holding the grab, if any.
    pub grab: Option<ControlKey>,
}

/// Sees every event before routing. App-owned and persistent: created once,
/// borrowed by each host build.
pub trait InputFilter {
    fn filter(&self, event: &InputEvent, ctx: &FilterContext) -> Filtered;
}

impl<'a> ControlHostBuilder<'a> {
    /// Register a filter. The last added is asked first.
    pub fn with_filter(self, filter: &'a dyn InputFilter) -> Self;
}
```

Outside the crate a `FilterContext` is `FilterContext::default()` with
fields assigned. `InputFilter` is a separate trait, not a method on
`HostedControl`, and **a control is never registered as a filter itself**:
a global feature owns a small filter object that routes to its control by
name. Filters are trait objects, so "the code only does it if there's a
reason". They are borrowed, not boxed: the host holds
`Vec<&'a dyn InputFilter>`, and a per-build app allocates nothing for them.

### 9.2 What a filter sees

Every event the host is given, after the host has recorded it: pointer
events, keys including `Tab`, `BackTab`, `Escape` and `Back`, gestures, and
modifier changes. A filter "can intercept any key if it chooses to". It may
keep its own state through interior mutability (`Cell`, atomics, a
`Mutex`); the host calls it through `&self`.

### 9.3 The pass

At the start of each `deliver_*`, after recording:

```text
for filter in filters.iter().rev():
    match filter.filter(&event, &ctx):
        Pass            => continue
        Consumed(action) => return EventResponse::consumed(action, focused_index)
        Redirect(key)   => return deliver_redirected(key, event)
route as section 3
```

```rust
impl EventResponse {
    /// Handled, naming no control.
    pub fn consumed(action: Option<ControlAction>, focused_index: Option<usize>) -> Self;
}
```

Cost: `k` virtual calls per event for `k` registered filters; a length
check with none. The order is registration order reversed, so an app that
registers a shortcut layer and then a modal filter has the modal asked
first.

### 9.4 Redirect

`Redirect(key)` is generic: a modal stack, a non-modal dialog layer, a
drag-and-drop layer or a shortcut layer can each route an event to the one
control that implements it, through that control's ordinary `handle_*`
methods. It was chosen over a side channel because delivery is synchronous
on the render thread, must produce an `EventResponse` for the app, and the
target's content only exists during the build; a queue belongs between a
shell's input thread and the render loop, outside libcontrols.

`deliver_redirected(key, event)` (private):

1. Resolve `key`. If it names no control in this host, return
   `EventResponse::consumed(None, focused_index)`: the input stays trapped
   rather than falling through to area or focus routing.
2. Never change the focus, never start a grab, never handle `Tab`,
   `BackTab` or `Escape`. Drop a grab held by any control other than the
   target (`active_drag = None`).
3. By kind:
   - **Mouse.** Hover: if the remembered hovered control is not the target,
     tell it `set_hover(None)` and forget it; then apply the 8.1 rule to the
     target alone, with `target.hit_test(pos)` as the hit (so a target whose
     `hit_test` returns `None` is never hovered and the host remembers no
     hover). `part = target.hit_test(pos).map(|(p, _)| p).unwrap_or(Body)`.
     Call `handle_mouse(&event, &ctx)`; return
     `EventResponse::handled(idx, id, part, action, focused_index)`.
   - **Keyboard.** `handle_keyboard(&event, &ctx)` with `part = Body`;
     return `handled(idx, id, Body, action, focused_index)`.
   - **Gesture.** `part` from `target.hit_test` at the host's pointer, else
     `Body`; `handle_gesture`; return `handled(..)`.
   - **Modifiers.** Already recorded; there is no handler to call. Return
     `consumed(None, focused_index)`.

A redirected pointer event never grabs, so the target's `press_modifiers`
is `Some` only if it already held the host's grab (it will not, in
practice; a redirect target runs its own child host and keeps its own
grabs there).

## 10. Host data layout and per-event cost

### 10.1 Layout

```rust
pub struct ControlHost<'a> {
    controls: Vec<Box<dyn Control + 'a>>,
    control_ids: Vec<Option<Symbol>>,
    filters: Vec<&'a dyn InputFilter>,   // registration order; asked from the back
    focused: Option<Slot>,
    hovered: Option<Hovered>,
    active_drag: Option<(DragGrab, usize)>,
    pointer: Option<[f32; 2]>,
    modifiers: Modifiers,
}

#[derive(Copy, Clone)]
struct Slot { key: ControlKey, index: usize }

#[derive(Copy, Clone)]
struct Hovered { key: ControlKey, index: usize, part: Option<ControlPart> }  // part None after set_hovered_key
```

A key is resolved to its index **once**, when it is set, and the index is
kept beside it. That is sound because controls are only ever appended
(`add_control`, `add_named_control`); nothing removes or reorders them. The
public accessors keep their shapes: `focused_key()`, `focused_index()`,
`active_drag() -> Option<DragGrab>`, `hovered_key()`.

`clear_hover` and the hover updates mutate `hovered`, so they take
`&mut self` like every `deliver_*`; `Panel` already holds its child host in
a `RefCell` for the same reason.

`ControlHostBuilder` gains `filters: Vec<&'a dyn InputFilter>` and keeps
`#[derive(Default)]`.

### 10.2 Cost per event, `n` controls, `k` filters

| Event | 0.2.0 | This plan |
|---|---|---|
| `Move`, no grab | 2 hit-test scans + `n` `set_hover` | `k` filter calls + 1 scan + at most 2 `set_hover` |
| `Down` / `Click` on a control | 2 scans + `n` `set_hover` + `O(n)` key resolves + 1 handler | `k` + 1 scan + at most 2 `set_hover` + at most 2 `handle_focus` + 1 handler |
| `Up` / `Scroll` / `Drag` under a grab | `O(n)` string resolve + 1 handler | `k` + `O(1)` + 1 handler |
| key to the focused control | `O(n)` string resolve + 1 handler | `k` + `O(1)` + 1 handler |
| `Tab` / `BackTab` | `O(n)` resolve | `k` + at most 2 `handle_focus` (+ `accepts_focus` on each refuser skipped) |
| modifiers | `n` `set_modifiers` | `k` |
| gesture | 1 scan + 1 handler | `k` + 1 scan + 1 handler |

The hit-test scan is the one remaining per-control walk on a pointer event:
last-to-first, one `hit_test` per control until the first hit. With tens of
controls per host that is not measured here; section 15 asks whether a
later plan should index it.

## 11. Compatibility (0.3.0)

**Breaking:**

- `ControlKey::Named(Symbol)`; `ControlKey::id()` returns `Option<Symbol>`;
  `with_named_control`, `add_named_control`, `ControlKey::named` and
  `find_by_id` take `impl Into<Symbol>` (an `&str` still compiles);
  `EventResponse::target_id: Option<Symbol>` (`as_deref()` still gives
  `Option<&str>`).
- `Key::Back`; `Key` is `#[non_exhaustive]`.
- `HostedControl::handle_mouse`, `handle_keyboard` and `handle_gesture`
  take `&EventContext` in place of `ControlPart`.
- `HostedControl::set_modifiers` is removed; `ControlHost::deliver_modifiers`
  tells no control.
- `DragGrab` gains `press_modifiers` and is `#[non_exhaustive]`
  (`DragGrab::new`).
- `Tab`, `BackTab` and `Escape` key **releases** no longer reach the
  focused control (their presses never did).
- Hover is no longer re-sent to every control on every pointer event
  (8.2's note for apps that rebuild per event and deliver `Move`).

**Additive:** `Symbol`, `sym!`, `FocusEvent`, `HostedControl::handle_focus`
and `accepts_focus`, `EventContext`, `InputFilter`, `Filtered`,
`FilterContext`, `ControlHostBuilder::with_filter`,
`ControlHost::{set_modifiers, hovered_key, set_hovered_key, clear_hover}`,
`EventResponse::consumed`, `Panel::{with_hovered_key, hovered_key}`,
`Copy` on `ControlKey` and `DragGrab`, `Default` on `ControlPart`.

**Downstream:**

- tinytrowel implements no `HostedControl` and names no `Key`; its two
  lines (`ui.rs:211`, `264`) compile unchanged. Step 8 checks the crate.
- Noble's Keep has no code yet that uses libcontrols.
- `tests/events.rs` needs **signature updates only**: `Legacy` and
  `Recorder` take `&EventContext`; `Recorder` loses `set_modifiers` and
  gains `handle_focus` logging; the test
  `modifiers_are_recorded_and_told_to_every_control` becomes
  `modifiers_are_recorded_and_told_to_no_control`. Every assertion about
  behaviour stands.
- The unit tests in `src/lib.rs` change only where they build a `DragGrab`
  literal (`DragGrab::new`).

**Unchanged:** `DrawControl`, `ControlHost::draw` and its instance order,
`ControlAction`, `EventResponse`'s shape, `InputEvent`, `MouseEvent`,
`KeyboardEvent`, `GestureEvent`, the grab rules of constraint 6, every
control's visible behaviour.

## 12. Out of scope: the "focus manager: nested containers" plan

This plan routes within one host. A later plan, to be named
"focus manager: nested containers", takes the levels together. It covers,
and this plan deliberately does not touch:

- addressing focus, hover and drag as a **symbol path** (for example
  `[panel, control]`) so an event goes straight to the innermost control
  without each level running a whole child `ControlHost` pass;
- **Tab order across nesting** (into a `Panel`'s children and out again),
  and `Panel::handle_focus` clearing child focus on `Lost`;
- `Panel`'s **existing hover defects**: `Move` never reaches a panel's
  children (constraint 5 stops it at the outer host), and `Panel` does not
  clear its children's hover when the pointer leaves it;
- `Panel` child **tooltips drawn in the first pass** (`Panel::draw` draws its
  child host whole, overlays included);
- filters on nested hosts, which today see no modifier changes because the
  container is not told of them (6.1).

Also out of scope here: a `TabView` container (deferred), wheel and finger
scrolling containers (their own plan), and filters that rewrite events
(`Filtered` has no `Replace`).

## 13. When 0.3.0 is cut

Plans 02 and 03 ship together as **0.3.0**, and the version is cut
**once, in the last step of plan 03** (`20261016_03_MODAL_STACK_PLAN.md`,
its final step), which bumps `Cargo.toml`, writes the single `VERSIONS.md`
entry covering both plans, and is tagged `v0.3.0` (AGENTS.md Rule 5).

This plan bumps nothing. From its step 2 until that tag, `main` carries
`version = "0.2.0"` with an API that is no longer 0.2.0's. That is accepted
because: a tag must mark a commit whose API is the one its `VERSIONS.md`
entry describes, and the first breaking step is not such a commit; the
entry cannot be written before the changes it lists exist, and entries are
never edited once appended; and nothing consumes `main` by version
(tinytrowel vendors a chosen commit by path). If plan 03 is put on `Hold`
or superseded before its last step, a `CHANGES.md` addendum names a new
final step of this plan that cuts 0.3.0 for this plan alone; until then
the rule is one bump, in plan 03.

## 14. Steps

Each step is one commit touching only this crate and passing
`cargo fmt -p libcontrols -- --check`, `cargo clippy -p libcontrols
--all-targets` with no new warnings, and `cargo test -p libcontrols`
(Rule 3.4). New tests go in `tests/symbol.rs` (steps 1 and 2),
`tests/routing.rs` (steps 4 to 6) and `tests/filter.rs` (step 7), in the
style of `tests/events.rs`: a `Recorder` control logging every
`handle_mouse`, `handle_keyboard`, `handle_gesture`, `set_hover` and
`handle_focus` call (with its `EventContext`) into a `RefCell<Vec<_>>`,
`ControlHost::builder()`, plain asserts whose messages say what must hold.

1. **`Symbol` and `sym!`.** Add `src/symbol.rs` (4.1, 4.2), re-export
   `Symbol`, export `sym!`. Nothing else changes.
   Tests: `interning_the_same_name_gives_the_same_symbol`,
   `different_names_are_different_symbols`,
   `a_symbol_reads_back_as_its_name` (`as_str`, `Deref`, `Display`,
   `Debug` quoted), `a_symbol_compares_with_a_str`,
   `sym_interns_once_per_call_site` (two evaluations of one `sym!` in a
   loop equal each other and `Symbol::intern` of the same literal),
   `interning_from_many_threads_agrees` (eight threads interning the same
   twenty names get identical symbols),
   `symbol_order_is_interning_order`.
   **Done when:** those pass and `Symbol` is `Copy + Eq + Hash + Ord + Send
   + Sync` (asserted with a `fn assert<T: ..>()` in the test).

2. **Symbols for control names** (4.3). `ControlKey::Named(Symbol)` and
   `Copy`, the host's ids, `find_by_id`, `EventResponse::target_id`,
   `Panel::with_named_control`, `DragGrab: Copy`.
   Tests: `a_named_control_is_found_by_str_and_by_symbol`,
   `the_response_names_the_control_as_a_symbol` (both
   `as_deref() == Some("speed")` and `== Some(sym!("speed"))` hold),
   `a_control_key_is_copy_and_names_its_symbol`.
   **Done when:** those pass, and `tests/events.rs` and the `src/lib.rs`
   tests pass unmodified (they use `&str` names and `as_deref()`).

3. **`Key::Back`** (section 5). Add the variant, mark `Key`
   `#[non_exhaustive]`, confirm every in-crate `match` has a wildcard.
   Tests (appended to `tests/events.rs`):
   `back_goes_to_the_focused_control_and_keeps_focus`,
   `escape_still_clears_focus`.
   **Done when:** the two tests and all existing tests pass unmodified.

4. **`EventContext` and modifiers on every handler; `set_modifiers`
   removed** (section 6). Change the three signatures, add `handle_focus`
   and `accepts_focus` as defaulted no-ops (their behaviour is step 5),
   `DragGrab::press_modifiers`, `ControlHost::set_modifiers`,
   `EventResponse::consumed`, `Default` on `ControlPart`; update the eleven
   controls, `ControlHandle`, `Panel` (seeding, no `set_modifiers`
   override) and `tests/events.rs` (signatures only; the modifiers test
   renamed as in section 11).
   Tests: `every_pointer_event_carries_the_modifiers_held`,
   `a_shift_pressed_mid_drag_shows_on_the_next_drag_event`,
   `press_modifiers_are_those_held_at_the_press`,
   `press_modifiers_are_none_outside_a_grab`,
   `keys_and_gestures_carry_the_modifiers_too`,
   `a_modifier_change_reaches_no_control` (a `Recorder` logs nothing for
   `InputEvent::Modifiers`, and `host.modifiers()` reads the new value),
   `a_panel_passes_the_modifiers_it_saw_to_its_children`,
   `a_context_is_built_outside_the_crate_with_new_and_with_methods`,
   `a_rebuilt_host_is_seeded_with_the_last_modifiers`.
   **Done when:** those pass; a `Recorder` under a `Down` with Shift held,
   then `Modifiers` adding Ctrl, then a `Drag`, logs `press_modifiers ==
   Some(shift)` and `modifiers == shift+ctrl` on the `Drag`.

5. **Focus events and focus stops** (section 7), with the resolved-index
   cache of 10.1 for focus and drag.
   Tests: `tab_sends_lost_then_gained`,
   `backtab_sends_them_the_other_way`,
   `a_press_on_a_control_moves_the_focus_with_events`,
   `escape_and_a_press_on_nothing_send_lost`,
   `restoring_focus_on_a_rebuilt_host_sends_gained_only`,
   `setting_the_focus_it_already_has_sends_nothing`,
   `a_carried_key_that_no_longer_resolves_sends_nothing`,
   `initial_focus_sends_gained_at_build`,
   `tab_skips_a_control_that_refuses_focus`,
   `a_press_on_a_control_that_refuses_focus_leaves_focus_alone`,
   `when_every_control_refuses_tab_clears_the_focus`,
   `tab_and_escape_releases_reach_no_control`.
   **Done when:** in a host of three `Recorder`s, Tab from none logs
   `[Gained(0)]`, Tab again `[Lost(0), Gained(1)]`, Escape `[Lost(1)]`;
   and with control 1 refusing focus, Tab from 0 lands on 2.

6. **Hover as enter and leave** (section 8). Replace `update_hover`; one
   hit-test per pointer event reused for routing; `hovered_key`,
   `set_hovered_key`, `clear_hover`; `Panel::with_hovered_key` and
   `hovered_key`.
   Tests: `only_the_control_entered_and_the_one_left_are_told`,
   `a_part_change_tells_the_same_control_once`,
   `staying_put_tells_nobody`,
   `leaving_everything_tells_only_the_one_left`,
   `a_grab_freezes_hover_until_the_release`,
   `hover_is_carried_across_a_rebuild_so_leaving_still_clears`,
   `the_part_is_resent_once_after_a_rebuild`,
   `clear_hover_tells_only_the_hovered_control`,
   `a_hovered_key_that_no_longer_resolves_is_dropped`,
   `a_panel_carries_its_childrens_hover`.
   **Done when:** a host of eight `Recorder`s logs at most two `set_hover`
   calls per pointer event over any sequence of moves, and the hover tests
   in `src/lib.rs` (`a_hovered_cell_is_the_one_under_the_pointer_and_nothing_else`,
   `hovering_one_control_clears_the_hover_on_another`,
   `a_tooltip_is_drawn_over_the_controls_that_follow_it`) pass unmodified.

7. **The filter stack** (section 9). `InputFilter`, `Filtered`,
   `FilterContext`, `with_filter`, the pass, `deliver_redirected`.
   Tests: `with_no_filter_registered_routing_is_unchanged` (a `Recorder`
   host logs the same calls with and without an empty filter list),
   `the_last_filter_added_is_asked_first`,
   `a_passing_filter_leaves_routing_alone`,
   `a_consumed_event_reaches_no_control_and_reports_handled`,
   `a_consumed_action_is_returned_to_the_app`,
   `a_filter_sees_tab_escape_back_and_modifier_changes`,
   `the_context_reports_pointer_modifiers_focus_and_grab`,
   `a_redirected_mouse_event_reaches_only_the_target_even_off_its_area`,
   `a_redirected_key_skips_the_hosts_tab_and_escape`,
   `a_redirected_modifier_change_is_consumed`,
   `a_redirect_to_a_missing_control_is_consumed`,
   `a_redirect_leaves_focus_alone_and_starts_no_grab`,
   `a_redirect_moves_hover_off_the_hovered_control`,
   `a_redirect_drops_another_controls_grab`,
   `a_target_whose_hit_test_misses_is_never_hovered`,
   `one_filter_object_serves_two_builds` (a filter with a `Cell` counter is
   borrowed by two successive hosts and counts both events).
   **Done when:** those pass; a filter answering
   `Redirect(ControlKey::named("sink"))` for everything sends every mouse,
   keyboard and gesture event to the `sink` `Recorder` alone, consumes
   modifier changes, and a `Recorder` beneath it logs nothing but one
   `set_hover(None)`.

8. **README and the compatibility check.** Add an "Input routing" section
   to `README.md` above `## Building`: area and focus routing, the grab,
   `Symbol` and `sym!`, `EventContext`, `handle_focus`, carrying
   `focused_key`, `active_drag`, `hovered_key` and `modifiers` across a
   rebuild, and a ten-line `InputFilter` example that consumes `Ctrl+S`.
   In a scratch copy of tinytrowel (not committed), replace
   `deps/libcontrols/src` and its `Cargo.toml` with this crate's and run
   `cargo check -p trowel-game`; if it fails, the fix is proposed to
   tinytrowel's owner, not made here. No version bump (section 13).
   **Done when:** the README example is mirrored by a test in
   `tests/filter.rs` (`the_readme_shortcut_filter_consumes_ctrl_s`), and
   `trowel-game` checks clean with no change to tinytrowel (or the needed
   change is written up).

## 15. Open questions

1. **A shared crate for `Symbol`.** The interner lives in libcontrols so
   that `sym!` and `ControlKey` need no second dependency. tinytrowel and
   Noble's Keep may want the same symbols for their own ids. Move it to a
   small shared crate later (re-exported here), or keep it in libcontrols?
2. **Hit-test indexing.** After this plan the last-to-first `hit_test` scan
   is the only per-control walk on a pointer event. Is it worth a bucketed
   or sorted index in the nested-containers plan, or do hosts stay small
   enough that it never matters?
3. **Display-only focus stops.** `Label` takes the focus on Tab today.
   `accepts_focus` makes it a one-line change to skip labels, but that
   changes the Tab order of existing apps. Do it in 0.3.0 (already
   breaking) or leave it?
4. **`EventResponse` and `#[non_exhaustive]`.** Its fields are public and
   it has constructors. Should it be closed now, while 0.3.0 is breaking
   anyway, so later fields (a focus path, a filter name) are not?
5. **`Edit` on `FocusEvent::Lost`.** `Edit` has no caret or commit today.
   When it grows editing, is commit-on-blur the right default given that a
   per-build app can lose the `Lost` (7.1)?

## Amendment — 2026-10-16 01:15 EDT: implementation started

Status changes from `Draft` to `In implementation`. The owner approved
this plan as written ("approve and commit both") and asked for the work to
start. Implementation begins with step 1 on the branch `impl/0.3.0`, which
is squash-merged into `main` once this plan and
`20261016_03_MODAL_STACK_PLAN.md` are both complete. From this amendment on
the document is immutable; departures are recorded in `CHANGES.md`
(AGENTS.md, Rule 1.7).

## Amendment — 2026-10-16 01:37 EDT: implemented

Status changes from `In implementation` to `Implemented`. Every step is
done and its **Done when:** holds (`cargo fmt`, `cargo clippy` with no
warnings and `cargo test` pass at each step; the tinytrowel check passes).
Each step was one commit on the branch `impl/0.3.0`; all of them reached
`main` together in the squash commit `8c34296`, tagged `v0.3.0`
(`CHANGES.md`, 2026-10-16 01:37 EDT addendum):

- Step 1 (Symbol and sym!): branch commit `1714da2`.
- Step 2 (symbols for control names): branch commit `035f066`.
- Step 3 (Key::Back): branch commit `31059cd`.
- Step 4 (EventContext on every handler; set_modifiers removed): branch commit `36a2cc6`.
- Step 5 (focus events and focus stops): branch commit `deca05e`.
- Step 6 (hover as enter and leave): branch commit `09517b0`.
- Step 7 (the filter stack): branch commit `8e7bea5`.
- Step 8 (README input routing section; tinytrowel check): branch commit `8dc9d73`.
