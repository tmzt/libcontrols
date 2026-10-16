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
