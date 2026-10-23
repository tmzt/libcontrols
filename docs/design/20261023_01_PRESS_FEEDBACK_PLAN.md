---
title: Press feedback, after Material 3
date: 2026-10-23
status: Draft
covers: >
  A visible press for touch and pointer input, kept separate from a
  button's latched or toggled state: the router tracks a transient press per
  node (NodeState::PRESSED) from touch-down to lift, cancels it when the
  pointer moves past a touch slop or the host cancels the press, keeps it
  visible for a minimum time after a quick tap, and fades it out, driven by
  a host clock (Router::advance). Controls read how pressed they are from
  the draw context. Buttons draw Material 3's pressed state layer and move
  down a little ("depress"); plain-text tap targets shift their text colour
  slightly within the same hue. Theme values, shared drawing helpers,
  tests, and the 0.8.0 release.
sources:
  - >
    Owner, relayed by another session: "a visual effect like pressing it
    should have like a depress effect or something like that", then
    "that's better to recommend to libcontrols so it can plan the feature".
  - >
    Owner, in this session: "it should follow material/m3 concepts where
    possible. the plain text targets should use a slight color shift in the
    same hue".
  - >
    What a consuming app tried and reverted so that it could come from here:
    while a touch that began on a button has not moved past a 10 pt slop,
    draw the button 2 pt lower with its background at 70%; a drag past the
    slop or a touch cancel ends it; a quick tap's press stays visible about
    120 ms after lift so a fast tap shows at least one frame; plain-text tap
    targets darkened and dropped 2 pt.
  - >
    Material 3: state layers (an overlay of the content colour over the
    container: hover 8%, focus 10%, pressed 10%, dragged 16%); button
    elevation levels; motion duration tokens (short1 50 ms, short2 100 ms,
    short3 150 ms, short4 200 ms) with the standard easing; the platform
    touch slop of 8 dp.
  - >
    src/router.rs (`deliver_mouse`, `set_grab`, `Grab::start_pos`,
    `node_context`), src/flags.rs (`NodeState`, bits 0-4 used),
    src/traits.rs (`DrawContext`, public fields, not `#[non_exhaustive]`),
    src/theme.rs (`ControlTheme`), src/button.rs (`ButtonState::is_pressed`
    is the latched state; `draw` brightens the background 1.5x when
    latched), src/label.rs (`handle_mouse` reports `Activated` on a left
    press or click), src/event.rs (`MouseEvent` and `InputEvent` are
    exhaustive; there is no cancel event), src/modal.rs
    (`ModalState::advance(dt_secs)`, the precedent for a host clock), at
    5cad90f (0.7.0).
  - >
    libmsdf 0.6.0 `drawlist::Elevation` (`Flat`, `Raised`).
related:
  - docs/design/20261016_05_PERSISTENT_ROUTER_PLAN.md (the router, NodeState
    and the press grab this builds on)
---

# Press feedback, after Material 3

## 1. Goal

A finger or pointer pressing a control should see it pressed at once, for
as long as the press lasts, and briefly even when the tap is too quick for a
frame. A press that turns into a drag or is cancelled should stop looking
pressed. This is separate from a button that is latched or toggled on, which
`ButtonState::is_pressed` already shows.

After this plan:

- the router marks the node a press began on with `NodeState::PRESSED`
  while the press lasts, and gives each control a press amount from 0 to 1
  in its draw context, which holds and fades as Material 3's pressed state
  does;
- a `Button` draws M3's pressed state layer over its container and moves
  down by a theme offset;
- a `Label` used as a tap target shifts its text colour within the same
  hue;
- any other control, including an app's own, can do the same through two
  public helpers.

## 2. Material 3, and where this departs

Followed:

- **State layers.** M3 shows hover, focus and press by drawing the content
  colour (the "on" colour: the label or icon colour) over the container at a
  fixed opacity: hover 8%, focus 10%, pressed 10%, dragged 16%. A pressed
  `Button` draws its text colour over its background at 10% times the press
  amount, inside the same rounded rectangle. A hovered one draws 8%.
- **Durations.** The press appears at once. After release it stays at full
  strength until it has been visible for M3's `short2`, 100 ms, then fades
  out over `short3`, 150 ms, with the standard easing. A cancelled press
  fades at once, with no hold.
- **Touch slop.** A press that moves more than 8 logical pixels from where
  it began is a drag, not a press, as on the platform (8 dp).
- **Keyboard.** Enter or Space on a focused button shows the same press as a
  tap, for as long as the key is down, with the same hold and fade.

Departures, each a theme value so that an app can turn it off:

- **The depress offset.** M3 moves nothing on press; it changes elevation,
  and only on hover for elevated buttons. libmsdf has two elevations
  (`Flat` and `Raised`), and the owner asked for a "depress effect". A
  pressed `Button` therefore moves down by `PressTheme::offset` (default
  2.0 logical pixels) times the press amount: container, border, text and
  focus ring together.
- **Plain text.** M3's text buttons have a state layer behind the label.
  The owner asked for plain-text tap targets to shift colour within the
  same hue instead, with no container (section 4.4).
- **No ripple.** M3 on Android adds a ripple from the touch point. Not in
  this plan (open question 3).

## 3. Findings (read 2026-10-23, at 5cad90f)

- The router already holds a press grab: `deliver_mouse` sets `GRABBED` on
  the node a `Down` lands on, records `start_pos`, and clears it on the
  matching `Up`. Moves and drags go to the grabbed node. That is the
  transient press, minus the slop, a cancel and the timing.
- `NodeState` is a bit set with bits 0 to 4 in use; a sixth bit is additive.
- Controls get `NodeState` through `DrawContext::state`, set by
  `Router::node_context` for each node drawn.
- There is no clock. `ModalState::advance(dt_secs) -> bool` (true while
  anything is still moving) is how a host already drives time; the router
  would do the same.
- There is no cancel event, and `MouseEvent` and `InputEvent` are exhaustive
  enums, so a new variant would break every match on them.
- `Button::draw` shows the latched state by brightening the background 1.5x
  and recolouring the border and text; it has no transient press.
- `Label::handle_mouse` reports `Activated` on a left press or click, so a
  label is already a tap target.
- A `Click` is routed by hit test, not to the grabbed node, so a press that
  became a drag and then lifted over a button still activates it today.

## 4. Design

### 4.1 The router

- **`NodeState::PRESSED`** (bit 5): set on the node a left-button `Down`
  lands on, when that node's control asks for press feedback (below);
  cleared on the matching `Up`, on `Router::cancel_press`, and when a `Drag`
  of that press moves more than `PressTheme::touch_slop` from the grab's
  `start_pos`. `GRABBED` is unchanged: a press that became a drag stays
  grabbed, so a slider keeps its drag.
- **`HostedControl::press_feedback(&self) -> bool`**, default `false`.
  `Button` and `Label` return `true`. A control that does not ask gets no
  `PRESSED`, no press amount and no change to its `Click`: sliders, edits
  and panels behave exactly as before.
- **The press amount.** For each node the router keeps
  `(amount: f32, held: f32, released: bool)`. A press sets the amount to 1
  and `held` to 0. `Router::advance(dt_secs: f32) -> bool` adds `dt_secs` to
  `held`. After release the amount stays at 1 until `held` reaches
  `min_visible`, then falls to 0 over `fade`, eased. A cancel starts the
  fade at once. `advance` returns true while any node's amount is above 0,
  so the host keeps drawing frames until the press has faded. A host that
  never calls `advance` sees a press that ends at release with no hold or
  fade, which is today's behaviour plus the press.
- **`Router::cancel_press()`**: the host's touch cancel (the platform took
  the touch, a scroll began, the window lost focus). It clears `PRESSED`
  and starts the fade. It is a method rather than a new `MouseEvent`
  variant, so that no match on the input enums breaks.
- **A cancelled press does not activate.** For a press-feedback node whose
  press was cancelled by the slop or by `cancel_press`, the router drops the
  `Click` that ends that press instead of routing it, so what the user sees
  (no longer pressed) matches what happens (no activation). Other nodes'
  clicks are routed as today.
- **Keyboard.** A `KeyDown` of Enter or Space delivered to a focused
  press-feedback node sets `PRESSED`, and its `KeyUp` releases it, with the
  same amount, hold and fade.
- Nested routers: each router tracks the presses of its own nodes; a press
  on a nested router's node is that router's, and the outer router's
  `advance` calls the nested routers' `advance`.

### 4.2 The draw context and the theme

- **`DrawContext::press: f32`**, the press amount of the node being drawn
  (0 when not pressed), set by `Router::node_context` with `state`.
- **`ControlTheme::press: PressTheme`**:

  ```rust
  pub struct PressTheme {
      /// M3 pressed state layer opacity. 0.10.
      pub pressed_layer: f32,
      /// M3 hover state layer opacity. 0.08.
      pub hover_layer: f32,
      /// How far a pressed button moves down, logical pixels. 2.0; 0 is M3's
      /// "no movement".
      pub offset: f32,
      /// How long a press stays at full strength after a quick tap,
      /// seconds. 0.100 (M3 short2).
      pub min_visible: f32,
      /// How long a released press takes to fade, seconds. 0.150 (M3 short3).
      pub fade: f32,
      /// How far a press may move before it is a drag, logical pixels. 8.0.
      pub touch_slop: f32,
      /// How far plain text moves toward the background in OKLab
      /// lightness when pressed. 0.10.
      pub text_shift: f32,
  }
  ```

### 4.3 Drawing helpers

- **`draw_state_layer(list, rect, radius, content: [f32; 4], opacity: f32)`**:
  one rounded box of `content` at `content[3] * opacity`, the M3 state
  layer.
- **`pressed_text_color(text: [f32; 4], background: [f32; 4], amount: f32,
  theme: &PressTheme) -> [f32; 4]`**: converts `text` to OKLab, moves its
  lightness toward `background`'s by `text_shift * amount`, keeps its hue
  angle and chroma, and converts back, keeping alpha. On a dark theme light
  text dims slightly; on a light theme dark text lightens slightly; a
  coloured link stays the same hue.

### 4.4 Controls

- **`Button`**: with `ctx.press` above 0, every instance it draws moves down
  by `offset * press`, and after the container it draws the state layer of
  its text colour at `pressed_layer * press`. When hovered and not pressed
  it draws the hover layer at `hover_layer`. The latched look
  (`is_pressed`) is unchanged and combines with these: a latched button can
  be pressed too.
- **`Label`**: with `ctx.press` above 0, its text is drawn in
  `pressed_text_color(text, background, press, ..)`, where the background is
  the label's own `bg_color` or, without one, the theme's `bg_color`. No
  container, no offset.

## 5. Tests

- **Router** (src/router.rs tests): `Down` on a button sets `PRESSED` and
  `GRABBED`; `Up` clears `PRESSED`; a drag of 8 px keeps it and of 9 px
  clears it but keeps `GRABBED`; `cancel_press` clears it; the `Click` after
  a slop cancel or a `cancel_press` is not routed, and after a plain press
  it is; a slider (no press feedback) gets neither `PRESSED` nor a dropped
  click; Enter and Space press a focused button. The amount: 1 while held;
  after a release at 30 ms it stays 1 until 100 ms, is about half at 175
  ms, 0 at 250 ms; a cancel fades from the cancel; `advance` is true until
  the amount reaches 0; with no `advance` the amount is 0 after release.
- **Helpers**: `pressed_text_color` keeps the OKLab hue angle within 0.5
  degrees and the chroma within 1%, moves lightness toward the background
  by `text_shift * amount`, and is the identity at amount 0.
- **Drawing** (instance level): a pressed `Button` at amount 1 has every
  instance 2 px lower than unpressed and one more instance (the state layer)
  of its text colour at 10% alpha; at amount 0.5, 1 px and 5%; a hovered one
  has the 8% layer; a pressed `Label` draws its text in the shifted colour
  and moves nothing.
- Renders of a pressed and unpressed button and label at 1x and 2x, under
  `out/controls/<YYYYMMDD_NN>_press_feedback/`, logged in that area's log.

## 6. Versioning (Rule 5)

Not strictly additive, so **0.8.0**:

- **Breaking:** `DrawContext` gains the public field `press`, and
  `ControlTheme` the public field `press`: struct literals of either need
  them.
- **Breaking (behaviour):** buttons and labels look different when pressed
  and when hovered; a button's `Click` after a press cancelled by the slop
  or `cancel_press` no longer activates it.
- Added: `NodeState::PRESSED`, `HostedControl::press_feedback`,
  `Router::advance`, `Router::cancel_press`, `PressTheme`,
  `draw_state_layer`, `pressed_text_color`.

No dependency changes version.

## 7. Steps

1. **The router's press.** `NodeState::PRESSED`, `press_feedback`, the
   slop, `cancel_press`, the keyboard press, the dropped click, the press
   amount and `Router::advance`, with `PressTheme`'s timing and slop.
   **Done when:** the router tests in section 5 pass, and every existing
   test passes unchanged.
2. **The draw context and helpers.** `DrawContext::press`, the rest of
   `PressTheme`, `draw_state_layer` and `pressed_text_color`.
   **Done when:** the helper tests pass.
3. **Button and Label.** Section 4.4.
   **Done when:** the drawing tests pass, the renders are logged, and a
   button that is never pressed or hovered draws exactly the instances it
   drew before.
4. **Release.** 0.8.0 with its VERSIONS.md entry and tag, on the owner's
   approval; the plan marked Implemented.
   **Done when:** fmt, clippy with no new warnings and the tests pass at the
   tagged commit.

## 8. Open questions

1. **The offset.** 2.0 px as the app tried, 1.0 px, or 0 (pure M3, the state
   layer alone)?
2. **The timing.** M3's 100 ms hold and 150 ms fade, or the app's 120 ms
   hold with no fade?
3. **Ripple.** M3's touch-point ripple on Android, as a later option?
4. **M3 Expressive shape change.** M3 Expressive buttons square their
   corners a little while pressed. Add it as a theme value (default off)?
5. **Other controls.** `Options` and `Radio` cells, `ArrowLabel` arrows and
   the slider thumb (M3 narrows a pressed slider handle) could take press
   feedback in the same way. In this plan, or after?
6. **Hover on touch.** A touch device has no hover; should the router skip
   the hover layer for presses that came from touch? The input events do not
   say which device a press came from today.
