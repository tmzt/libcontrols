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

## 0.4.0 — 2026-10-16 (tag `v0.4.0`)

The persistent control tree and router layers
(`docs/design/20261016_05_PERSISTENT_ROUTER_PLAN.md`), released together with
the move to libmsdf 0.3.0. The commit is the one tagged `v0.4.0`: the entry is
written in that commit, which cannot name its own hash.

- **Breaking:** libmsdf moves from 0.2.0 to 0.3.0 (`SdfKind` gains a variant
  and is `#[non_exhaustive]`). libmsdf types appear in this crate's public
  API (`DrawList` in `DrawControl`, `SdfInstance`), so the move is breaking
  for callers.
- **Breaking:** the `host` module is removed with `ControlHost`,
  `ControlHostBuilder` and `ControlKey`. A persistent `Router` owns the
  controls in a tree of `NodeId`s and keeps focus, hover and the press grab;
  apps call `deliver_event`/`deliver_mouse`/`deliver_keyboard`/
  `deliver_gesture`/`deliver_modifiers` and `render` on it.
- **Breaking:** `Control` and `HostedControl` move to the `control` module
  (still re-exported at the root). `HostedControl` gains defaulted
  `default_flags()`.
- **Breaking:** `InputFilter`, `Filtered` and `FilterContext` move to the
  `filter` module (still re-exported at the root). `Filtered::Redirect` and
  `FilterContext::{focused, grab}` hold `NodeId` in place of `ControlKey`.
  Filters are added to a `FilterStack` given to the root router with
  `Router::with_filter_stack`, in place of `ControlHostBuilder::with_filter`.
- **Breaking:** `EventResponse` and `DragGrab` move to the `router` module
  (still re-exported at the root). `EventResponse` has `target:
  Option<NodeId>` and `focused: Option<NodeId>` in place of `target_index`,
  `target_id` and `focused_index`, and is `#[non_exhaustive]`; its
  constructors take `NodeId`s. `DragGrab` holds `node: NodeId` and `button`.
- **Breaking:** `ControlHit` is removed.
- **Breaking:** `Panel` loses its lifetime parameter and its own children:
  `with_control`, `with_named_control`, `deliver_event`, `with_active_drag`,
  `active_drag`, `with_focus`, `with_initial_focus`, `with_focus_key`,
  `with_hovered_key`, `hovered_key`, `focused_key` and `focused_index` are
  removed. A panel's children are router nodes added with the panel as
  parent.
- **Breaking:** `DrawContext` and `EventContext` gain `flags: NodeFlags`,
  `state: NodeState` and `layer: Option<LayerView>`. `EventContext` no
  longer derives `Eq` or `Hash`.
- **Breaking:** the router draws disabled nodes at half alpha; controls no
  longer draw it themselves. `Label` is not a Tab stop by default and draws
  its ring when focused.
- **Breaking:** the modal stack is rebuilt on the router. `Modal` is now a
  trait (`options`, `filter_layer`, `covered`), and the old `Modal` struct
  becomes `ModalDialog`, whose content is a nested `Router`
  (`content`/`content_mut`). `with_close_control` and `with_initial_focus`
  take a `NodeId`. Backdrop, dismissal and hold-open move to `ModalOptions`.
  `ModalStack` is installed once with `ModalStack::install(&mut Router,
  state, viewport)` under `ModalStack::NAME`. Modals are opened with
  `Router::push_modal`, `push_modal_with` and `replace_top_modal`, which
  return `Result<(), ModalError>`. `ModalStack::modal` and `modal_mut` look
  a modal up by id.
- **Breaking:** removed: `ModalFilter`, `ModalState::filter`,
  `ModalState::{push, push_with, replace_top, replace_top_with}`,
  `MAX_MODAL_DEPTH` (there is no depth limit), and
  `ModalStack::{new, with_modal, modals, top_modal, handle}`.
- **Breaking:** `ControlAction` gains `Dismiss { control: Option<NodeId> }`;
  the enum is exhaustive, so downstream exhaustive matches stop compiling.
- Added: `Router` (`new`, `add`, `add_named`, `remove`, `find`, `find_alias`,
  `get`, `get_mut`, `parent`, `children`, `contains`, `len`, `is_empty`,
  `flags`, `set_flags`, `state`, `render`, `with_filter_stack`,
  `filter_stack`, `focused`, `set_focus`, `hovered`, `clear_hover`,
  `active_drag`, `pointer`, `modifiers`, `set_modifiers` and the `deliver_*`
  methods). A `Router` is itself a `Control`, so it can be nested.
- Added: `NodeId` (`get`, `Display` as `@n`, `FromStr`), `ParseNodeIdError`
  and `NameError` (`Taken`, `Reserved`).
- Added: `NodeFlags` (`FOCUSABLE`, `DISABLED`, `HIDDEN`, `TAB_THROUGH`) and
  `NodeState` (`FOCUSED`, `FOCUS_WITHIN`, `HOVERED`, `HOVER_WITHIN`,
  `GRABBED`), with `NONE`, `contains`, `intersects`, `union`, `difference`,
  `with`, `is_empty` and the bit operators.
- Added: `FilterStack` (`new`, `add`, `push`, `len`, `is_empty`, `run`).
- Added: `ModalDialog`, `ModalOptions`, `ModalError`, `LayerContext`,
  `LayerFilter`, `LayerShape`, `ModalLayerFilter`, `ShapedLayerFilter`,
  `LayerMask` (`drawn`, `texture`), `LayerView` and
  `ModalState::default_transition`.
- Added: `Symbol::lookup`.

## 0.5.0 — 2026-10-16 (tag `v0.5.0`)

The move to libmsdf 0.4.0. The commit is the one tagged `v0.5.0`: the entry
is written in that commit, which cannot name its own hash.

- **Breaking:** libmsdf moves from 0.3.0 to 0.4.0 (`b48d066`, tag `v0.4.0`).
  libmsdf types appear in this crate's public API (`DrawContext::atlas` is a
  `FontAtlas`, `DrawContext::shaper` a `TextShaper`, `DrawList` in
  `DrawControl`). In libmsdf 0.4.0 `FontAtlas` gains fields, so struct
  literals of it need updating, and the atlas format moves to v5. This
  crate's own code is unchanged.
