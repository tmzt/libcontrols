# VERSIONS

Each change of the crate version and the public API changes it carries
(AGENTS.md, Rule 5). Internal changes are not listed, and there is no entry
without a version change. Entries are appended, oldest first, and never
edited.

## 0.1.0 — 2026-10-02 (`ae7c381`, tag `v0.1.0`)

Initial release. Recorded retroactively on 2026-10-16 from the commit; the
tag was added on 2026-10-16.

- Controls: `ArrowLabel`, `Button`, `Edit`, `Label`, `Options`, `Panel`,
  `Radio`, `SelectableLabel`, `Slider`, `SliderNumeric`, each with its `Arc`
  state type (`ArrowLabelState`, `ButtonState`, `EditState`, `LabelState`,
  `OptionsState`, `PanelState`, `RadioState`, `SelectableLabelState`,
  `SliderState`, `SliderNumericState`), plus `PanelData`, `ThumbStyle`,
  `NumericFormat`, `ArrowLabelActive`, `SliderNumericActive`.
- Hosting: `Control`, `HostedControl`, `ControlHost`, `ControlHostBuilder`,
  `ControlKey`, `DragGrab`, `ControlHandle`, `ReadValue`.
- Events: `InputEvent` (`Mouse`, `Keyboard`), `MouseEvent`, `MouseButton`,
  `KeyboardEvent`, `Key`, `KeyState`, `ControlAction`, `ControlHit`,
  `ControlPart`, `EventResponse`.
- Drawing: `DrawControl` (`draw`, `draw_overlay`), `DrawContext`,
  `ControlTheme`, `Rect`, `TextAlign`, `GlowRingStyle`, `draw_glowing_ring`,
  `TooltipStyle`, `draw_tooltip`, `split_label`.
- Constants: `DEFAULT_PER_ROW`, `DEFAULT_ROW_GAP`, `MAX_OPTIONS`,
  `DEFAULT_CELL_GAP`, `DEFAULT_CELL_WIDTH`, `NO_CELL`, `LABEL_SEPARATOR`.

## 0.2.0 — 2026-10-02 (`88dd8c7`, tag `v0.2.0`)

Gesture, modifier and pixel-scroll events. Recorded retroactively on
2026-10-16 from the commit; the tag already existed.

- **Breaking:** `MouseEvent` gains `ScrollPixels { pos, delta }`, and
  `InputEvent` gains `Gesture(GestureEvent)` and `Modifiers(Modifiers)`.
  Both enums are exhaustive, so downstream exhaustive matches on them stop
  compiling.
- Added `GestureEvent` (`Pinch { delta }`, `DoubleTap`, `Rotate { delta }`)
  and `Modifiers` (`shift`, `alt`, `ctrl`, `meta`).
- `HostedControl` gains defaulted `handle_gesture(&self, &GestureEvent,
  ControlPart) -> Option<ControlAction>` and `set_modifiers(&self,
  Modifiers)`; `Panel` forwards both to its children.
- `ControlHost` gains `deliver_gesture`, `deliver_modifiers`, `pointer()` and
  `modifiers()`; `deliver_event` routes the new `InputEvent` variants.

## 0.3.0 — 2026-10-16 (tag `v0.3.0`)

Input routing and filters (`docs/design/20261016_02_INPUT_ROUTING_AND_FILTERS_PLAN.md`)
and the modal stack (`docs/design/20261016_03_MODAL_STACK_PLAN.md`), released
together. The commit is the one tagged `v0.3.0`: the entry is written in that
commit, which cannot name its own hash.

- **Breaking:** `ControlKey::Named` holds a `Symbol`; `ControlKey::id()`
  returns `Option<Symbol>`; `with_named_control`, `add_named_control`,
  `ControlKey::named` and `find_by_id` take `impl Into<Symbol>` (a `&str`
  still compiles); `EventResponse::target_id` is `Option<Symbol>` (its
  `as_deref()` is still `Option<&str>`).
- **Breaking:** `Key` gains `Back` (the platform back action) and is
  `#[non_exhaustive]`.
- **Breaking:** `HostedControl::handle_mouse`, `handle_keyboard` and
  `handle_gesture` take `&EventContext` in place of `ControlPart`;
  `HostedControl::set_modifiers` is removed, and
  `ControlHost::deliver_modifiers` tells no control.
- **Breaking:** `DragGrab` gains `press_modifiers` and is
  `#[non_exhaustive]` (build one with `DragGrab::new`).
- **Breaking:** Tab, BackTab and Escape key releases no longer reach the
  focused control; hover is no longer re-sent to every control on every
  pointer event (an app that rebuilds its host per event and delivers `Move`
  carries `hovered_key()` into the next build).
- Added: `Symbol` and `sym!`; `FocusEvent`, `HostedControl::handle_focus`
  and `HostedControl::accepts_focus`; `EventContext`; `InputFilter`,
  `Filtered`, `FilterContext` and `ControlHostBuilder::with_filter`;
  `ControlHost::{set_modifiers, hovered_key, set_hovered_key, clear_hover,
  draw_controls, draw_control_overlays}`; `EventResponse::consumed`;
  `Panel::{with_hovered_key, hovered_key}`; `Copy` on `ControlKey` and
  `DragGrab`; `Default` on `ControlPart`.
- Added: the `modal` module: `ModalState`, `ModalFilter`, `ModalStack`,
  `Modal`, `ModalFrame`, `PanelFrame`, `Backdrop`, `ContentReveal`,
  `DismissOn`, `DismissReason`, `ModalEvent`, `ModalPhase`,
  `ModalTransition`, `ModalView`, `ModalEntryView` and `MAX_MODAL_DEPTH`.
