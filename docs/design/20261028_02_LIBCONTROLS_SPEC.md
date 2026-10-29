---
title: "libcontrols: the library specification"
date: 2026-10-28
status: Implemented
covers: >
  What libcontrols 0.9.1 is and does at commit d53ef99: its purpose and scope,
  dependencies, rendering model, the router (tree, layers, routing, focus,
  hit testing, presses, scroll panes), the event model, the control traits,
  every control, the theme as it works today, geometry, public-API stability,
  testing, and the gaps visible in the code. A description of what exists, not
  a plan; it has no steps.
sources:
  - >
    The owner's request (verbatim): "capture the system prompt in a plan, use
    10m fable run to rewrite it as a spec for the library based on existing
    code, then add them theming support based on this discussion", clarified
    as "analyze the library as it exists and capture as a spec".
  - "The AGENTS.md heading line (section 1 below)."
  - "The README's introduction and controls list (section 1 below)."
  - "The initial commit ae7c381 (2026-10-02) and its message (section 1 below)."
  - "The code at HEAD, commit d53ef99 (src/, tests/); file:line references are to it."
related:
  - "20261015_01_MODAL_PLAN.md — first modal design (superseded); nothing of it is in the code."
  - "20261016_01_MODALS_AND_INPUT_FILTERS_PLAN.md — proposed a per-control intercept (superseded); nothing of it is in the code."
  - "20261016_02_INPUT_ROUTING_AND_FILTERS_PLAN.md — the InputFilter stack, area routing with a grab, FocusEvent, hover as enter/leave, EventContext modifiers, Symbol, Key::Back (sections 4, 5)."
  - "20261016_03_MODAL_STACK_PLAN.md — ModalState, ModalStack, Modal, layer filters, hold-open, dismissal (section 4.6)."
  - "20261016_04_NESTED_CONTAINERS_PLAN.md — superseded by the persistent router; nothing of it is in the code."
  - "20261016_05_PERSISTENT_ROUTER_PLAN.md — the Router tree, NodeId, NodeFlags/NodeState, nested routers as layers (section 4)."
  - "20261023_01_PRESS_FEEDBACK_PLAN.md — Draft; NodeState::PRESSED and DrawContext::press are not in the code (section 11)."
  - "20261023_02_SCROLL_PANE_PLAN.md — ScrollPane, ScrollState, ScrollTheme, the pane drag hand-off, Router::advance (sections 4.7, 6.12, 8.2)."
  - "20261025_01_EDIT_CURSOR_AND_INSERTION_POINT_PLAN.md — the cursor in EditState and Edit (section 6.10)."
  - "20261025_02_TEXT_AREA_CONTROL_PLAN.md — TextArea (section 6.11)."
  - "20261028_01_DRAWER_PLAN.md — Draft; a Drawer and wgpu compositing, not in the code; its amendments settle that a ControlTheme field is additive and propose ControlTheme defaults, options overrides and with_theme (sections 8, 10)."
living sections:
  - "6. The controls — a new control is appended as an amendment adding a subsection."
  - "8. The theme today — theming changes (the owner's theming amendment and later ones) extend this section by amendment."
---

# libcontrols: the library specification

This document describes libcontrols as it exists at commit `d53ef99`
(version 0.9.1, `Cargo.toml:3`). Everything below is what the code does, with
the file and line it does it at. It is not a plan and has no steps; the open
questions at the end are for the owner.

## 1. Starting point

The owner chose the library's own descriptions as the starting point, in
place of a system prompt. They are quoted here so later readers see what the
spec was written from.

- `AGENTS.md:1`: `# AGENTS.md — libcontrols: immediate-mode UI controls drawn as MSDF primitives`
- `README.md:1-5`: "UI controls drawn as MSDF primitives, owned by a
  persistent router. `libcontrols` provides a set of reusable UI controls
  (labels, buttons, sliders, radios, option grids, panels, dialogs) that
  render directly into `libmsdf` draw lists. A `Router` owns the controls as
  a tree for as long as the app keeps it, routes input to them, and draws
  them every frame; the app edits the tree only when its UI changes." Its
  `## Controls` list names Label, Button, Slider, SliderNumeric, ArrowLabel,
  SelectableLabel, Radio, Options, Panel, Edit and ScrollPane.
- Commit `ae7c381` (2026-10-02): "Initial release: immediate-mode UI controls
  drawn as MSDF primitives. Labels, buttons, sliders, numeric sliders, arrow
  labels, selectable labels, radio strips, option grids, panels and edit
  fields, with a ControlHost for focus and event routing and handles for
  reading values. MIT licensed." (23 files, 7027 lines; `src/host.rs` held
  the `ControlHost`.)

Where these disagree with the code, section 11 says so.

## 2. Purpose and scope

libcontrols is a Rust library crate (edition 2024, MIT, `Cargo.toml:1-7`)
of UI controls that draw themselves into a libmsdf `DrawList` and are owned,
drawn and given input by a `Router`. It is **not** immediate-mode at HEAD:
the `Router` is a retained tree of boxed controls (`src/router.rs:129-150`,
`nodes: Vec<Option<Node>>` with a free list, id and alias maps and root
list), kept across frames; the app adds and removes nodes when its UI
changes and calls `render` every frame. Control *values* live in `Arc`
state objects the app can keep (`Label.state: Arc<LabelState>`,
`src/label.rs:66`, and likewise for every control), so a control may be
rebuilt without losing its value.

The crate does no windowing, no GPU work and no text shaping of its own:
it takes an optional `FontAtlas` and `TextShaper` from libmsdf in the
`DrawContext` (`src/traits.rs:24-29`) and pushes SDF primitives and shaped
text into the `DrawList`. Presenting the list is the host's job, through
libmsdf.

## 3. Dependencies

- `libmsdf`, git `https://github.com/tmzt/libmsdf`, pinned to rev
  `4de319ce57aea99297de0a805b899276dd90b411` (`Cargo.toml:9-10`). It is the
  only dependency. It provides `DrawList`, `SdfKind`, `FontAtlas`,
  `ShapedRun`, `TextShaper`, `push_shaped_text`, `push_clip`/`push_clip_end`
  and `push_translate`/`push_translate_end` (`src/router.rs:672-691`).
- libcontrols has no direct `wgpu` dependency; the GPU is reached only
  through whatever consumes the `DrawList`. The Drawer plan
  (`20261028_01`) proposes adding wgpu; it is Draft.

## 4. The rendering model and the router

### 4.1 Drawing

`DrawControl` (`src/traits.rs:13-20`) has `draw(&self, &mut DrawList,
&DrawContext)` and a default-empty `draw_overlay` for what a control puts
over its neighbours. `Router::render` (`src/router.rs:583-586`) is
`self.draw(list, ctx)` then `self.draw_overlay(list, ctx)`: two passes,
each pre-order over the roots (`:589-600`, `:602-657`). `draw` first stores
`ctx.theme.scroll` in `scroll_theme` for input to read (`:595`) and brings
every pane's content range up to date (`update_panes`, `:596`).

`draw_node` (`:553-578`) halves the alpha of everything a node pushed when
it or an ancestor is `DISABLED`: the one disabled look for every control.
A `HIDDEN` node and its subtree are not drawn (`src/flags.rs:103-104`).

`DrawContext<'a>` (`src/traits.rs:24-39`): `atlas: Option<&FontAtlas>`,
`shaper: Option<&TextShaper>`, `theme: ControlTheme` (owned, cloned per
context), `px_range: f32`, `flags: NodeFlags`, `state: NodeState`,
`layer: Option<LayerView>`. The router fills `flags`, `state` and `layer`
per node. Helpers: `new`, `with_theme`, `with_px_range`, `font_size(opt)`,
`text_color(opt)`, `shape_text`, `text_advance` (falls back to
`chars * size * 0.55` without a shaper, `:103-109`), `draw_text` (pushes
shaped text with atlas and shaper, otherwise returns the estimated
advance, `:112-128`). `Default` gives no atlas, no shaper,
`ControlTheme::default()` and `px_range 6.0` (`:54-66`).

### 4.2 Nodes, ids and aliases

`NodeId(u64)` (`src/router.rs:35`), displayed `@17`, parsed with
`FromStr` (`:73`, error `ParseNodeIdError`). `Router::add(parent, control)`
(`:279`) and `add_named(parent, alias, control) -> Result<NodeId,
NameError>` (`:284`); `NameError` is `#[non_exhaustive]` with `Taken(NodeId)`
and `Reserved` (`:90-97`): aliases starting with `@` or `libcontrols/`
(`LIBRARY_PREFIX`, `:27`) are refused. The library's own nodes are roots
under `libcontrols/` kept above every app root (`:301-305`); the modal
stack is `libcontrols/ModalStack` (`src/modal.rs:1282`). `remove` (`:375`),
`find(&str)` (`:440`), `find_alias(Symbol)` (`:451`), typed `get::<T>` and
`get_mut::<T>` by `Any` downcast (`:456-470`), `parent`, `children`,
`contains`, `len`, `is_empty` (`:471-502`).

`Symbol` (`src/symbol.rs:14`) is an interned `u32` name: `intern`,
`lookup`, `as_str` (`&'static str`), `id`, `From<&str>`/`String`,
`Deref<str>`, `Display`.

### 4.3 Flags and state

`NodeFlags` (`src/flags.rs:95-110`), declared by the control's
`default_flags` when added and changed only by `Router::set_flags(id,
flags, on)` (`src/router.rs:511`): `FOCUSABLE`, `DISABLED`, `HIDDEN`,
`TAB_THROUGH`. `NodeState` (`src/flags.rs:112-127`), written only by the
router and read with `Router::state(id)` (`:536`): `FOCUSED`,
`FOCUS_WITHIN`, `HOVERED`, `HOVER_WITHIN`, `GRABBED`. Both are `u16` bit
sets with `contains`, `intersects`, `union`, `difference`, `with`,
`is_empty` and the bit operators (`:7-93`). There is no `PRESSED` state.

### 4.4 Input routing

All routing goes through `&self` methods with interior mutability (`Cell`
and `RefCell` fields, `src/router.rs:129-150`). Entry points:
`deliver_event(InputEvent)` (`:778`), `deliver_modifiers` (`:788`),
`deliver_mouse` (`:801`), `deliver_keyboard` (`:956`), `deliver_gesture`
(`:995`). Each returns an `EventResponse` (`#[non_exhaustive]`, `:205-216`):
`handled`, `target: Option<NodeId>`, `target_part: Option<ControlPart>`,
`action: Option<ControlAction>`, `focused: Option<NodeId>`.

Order per event:

1. The root's `FilterStack`, if set with `with_filter_stack` (`:712`).
   `FilterStack` (`src/filter.rs:54`) is `Rc<Chain>`; `add` and `push`
   (`:78-95`); `run` asks filters last-pushed first (`:116`). A filter
   returns `Filtered` (`#[non_exhaustive]`, `:12-17`): `Pass`,
   `Consumed(Option<ControlAction>)`, `Redirect(NodeId)`. It sees a
   `FilterContext` (`#[non_exhaustive]`, `:26-36`): `pointer`, `modifiers`,
   `focused`, `grab`.
2. The modal stack's top layer filter, when a stack is installed and holds a
   layer (`:913`, `:1068-1071`, `LayerOutcome::{Consumed, Handled}`).
3. Routing. Pointer events and gestures go to the innermost node whose
   `hit_test` accepts the point (`hit_path`, `:1263`); a `Down` grabs that
   node (`Grab { index, button, start_pos, press_modifiers, part, kept }`,
   `:177-185`) until `Up`. Keys go to the focused node; `Tab`, `BackTab` and
   `Escape` are taken by the router (`:963-971`): Escape clears focus, Tab
   and BackTab step the tab order (`tab_order`, rebuilt when `tab_stale`).
   `Key::Back` is an ordinary key.
4. Hover is enter/leave along the parent chain (`hover: Cell<Option<(usize,
   ControlPart)>>`, `:140`; controls get `set_hover(Some(part))`/`None`).
   Focus changes reach a control as `handle_focus(FocusEvent::Gained |
   Lost)` only when the focus moves.

The router also keeps `presses: RefCell<Vec<Press>>` with an `Owner` of
`App` or `Layer(serial)` (`:155-176`) so a press that began under one owner
is released to the same one, and `drop_click` (`:150`) to swallow the
`Click` after a pane has taken a drag.

Queries: `focused`, `set_focus` (`:723-741`), `hovered`, `clear_hover`,
`active_drag -> Option<DragGrab>` (`#[non_exhaustive]`: `node`,
`start_pos`, `button`, `press_modifiers`, `:191-201`), `pointer`,
`modifiers`, `set_modifiers` (`:762-776`).

### 4.5 Nested routers

`Router` implements `DrawControl` (`:589`) and `HostedControl` (`:1530`),
so a router is a control. Nested, it is one Tab stop that keeps Tab inside
it, or, flagged `TAB_THROUGH`, its stops join the parent's.

### 4.6 The modal stack

`ModalState` (`src/modal.rs:267`, `Arc`-shared): `new`, `with_transition`,
`default_transition`, `dismiss(by)`, `pop`, `close(id)`, `dismiss_all`,
`advance(dt) -> bool`, `set_progress`, `set_top_progress`, `top`, `depth`,
`contains`, `phase`, `progress`, `stack() -> Vec<ModalEntryView>`,
`captures_input`, `top_takes_input`, `is_animating`, `take_events()`
(`:276-528`). `ModalTransition`: `Instant`, `Timed { open_secs, close_secs
}`, `Driven` (`:43-51`). `ModalPhase` (non-exhaustive): `Closed`, `Opening`,
`Open`, `Closing`. `DismissReason` (non-exhaustive): `OutsideTap`, `Escape`,
`Back`, `Control(NodeId)`, `Requested`, `App`, `Replaced`. `ModalEvent`
(non-exhaustive): `Opened`, `Dismissed`, `Closed`, `DismissRefused`, each
with `id: Symbol` (`:107-119`).

`ModalStack::install(&mut router, Arc<ModalState>, viewport)` (`:1286`)
adds the `libcontrols/ModalStack` root; `Router::push_modal(id, modal)`,
`push_modal_with`, `replace_top_modal` (`:1615-1650`); `ModalError`
(non-exhaustive): `NoStack`, `AlreadyInstalled`, `AlreadyOpen` (`:1241-1249`).
The `Modal` trait (`:767-787`, `Control + Any`) has defaults for `options()`,
`filter_layer(&LayerContext) -> Box<dyn LayerFilter>` (a `ModalLayerFilter`
over `bounds()`), and `covered()` (clears hover). `LayerFilter: InputFilter`
adds `drawn(list, range)` and `reset` (`:789-797`); `LayerShape` (`:799`) is
implemented for `Rect` and by `LayerMask` (`drawn(inside)` or `texture(rect,
w, h, alpha, threshold)`, `:940-960`); `ShapedLayerFilter =
ModalLayerFilter<LayerMask>` (`:923`). `ModalOptions` (non-exhaustive,
`:699-741`): `backdrop: Option<Backdrop>`, `dismiss: DismissOn { outside_tap,
escape, back }`, `hold_open`, with `with_*` setters. `LayerView` (`:686-695`)
is what a control sees in its contexts: `id`, `phase`, `progress`, `depth`,
`is_top`.

`ModalDialog` (`:1019-1120`): `new(viewport, frame, content)`,
`centered(viewport, size, padding)`, `with_frame(impl ModalFrame)`,
`with_options`, `with_reveal(ContentReveal::{WhenOpen, Fade, Always})`,
`with_clip_content`, `with_close_control(NodeId)`, `with_initial_focus`,
`content()`/`content_mut()` (its own `Router`), `frame_rect`,
`content_rect`. `ModalFrame` (`:543-550`): `draw_frame`,
`draw_over_content`, `contains`; `PanelFrame` (`:573-581`: `bg_color`,
`border_color`, `border_thickness`, `corner_radius`, all optional) is the
built-in frame; `Backdrop { color, fade_with_progress }` defaults to
`[0, 0, 0, 0.45]` (`:635-645`).

### 4.7 Scroll panes in the router

When drawing a `ScrollPane` node's children the router pushes a clip of the
viewport and a translate of `-offset` (`src/router.rs:672-674`), culls
children out of view, and pops both (`:689-691`). Hits and events are
mapped into content coordinates. A press that moves past
`ScrollTheme::touch_slop` along the pane's axis is handed to the pane unless
the control's `claims_drag(part)` is true (`src/control.rs:49-52`;
`Slider`, `SliderNumeric`, `Edit`, `TextArea` claim it). `Router::advance(dt)
-> bool` (`:1603`) moves flings, springs and indicators in this router and
nested ones and reports whether anything is still moving. `scroll_to`,
`scroll_by`, `scroll_into_view`, `to_content`, `visible_bounds`
(`:1632-1680`).

## 5. The event model (`src/event.rs`)

- `MouseButton`: `Left`, `Right`, `Middle`, `Other(u16)` (`:9-14`).
- `MouseEvent`: `Move`, `Down`, `Up`, `Click`, `Scroll { delta }` (notches),
  `ScrollPixels { delta }`, `Drag { start_pos, current_pos, delta, button }`
  (`:18-38`); `pos()`, `button()`.
- `Key` (`#[non_exhaustive]`, `:70-92`): `Left`, `Right`, `Up`, `Down`,
  `Tab`, `BackTab`, `Enter`, `Escape`, `Back`, `Space`, `Home`, `End`,
  `PageUp`, `PageDown`, `Backspace`, `Delete`, `Char(char)`, `Other(u32)`.
- `KeyState`: `Pressed`, `Released`. `KeyboardEvent`: `Key { key, state }`,
  `KeyDown`, `KeyUp`, `Char { ch }` (`:102-111`); `is_down()`, `key()`.
- `GestureEvent`: `Pinch { delta }`, `DoubleTap`, `Rotate { delta }`.
- `Modifiers { shift, alt, ctrl, meta }` (`:148-153`).
- `InputEvent`: `Mouse`, `Keyboard`, `Gesture`, `Modifiers` (`:157-163`).
- `ControlPart` (`:168-179`, default `Body`): `Body`, `Label`, `Value`,
  `LeftArrow`, `RightArrow`, `Track`, `Thumb`, `NumericBox`, `Custom(u32)`.
- `EventContext` (`#[non_exhaustive]`, `:195-211`): `part`, `modifiers`,
  `press_modifiers: Option<Modifiers>`, `flags`, `state`, `layer`; `new`,
  `with_part`, `with_modifiers`, `with_press_modifiers`.
- `FocusEvent` (`#[non_exhaustive]`): `Gained`, `Lost`.
- `ControlAction` (`#[non_exhaustive]`, `:257-280`): `Activated { part }`,
  `StepLeft`, `StepRight`, `ValueChange { new_value, fraction }`,
  `TextInput { ch }`, `Custom(String)`, `Dismiss { control: Option<NodeId> }`.

## 6. The controls

This is a living section. Common shape of every control: a `pub struct`
with a `pub state: Arc<XState>`, a `pub rect: Rect`, optional style
overrides (`Option<[f32; 4]>` colours, `Option<f32>` sizes, `Option<GlowRingStyle>`), builder `with_*` setters, `with_state(Arc)`, `handle() ->
ControlHandle<Self>`, and `ReadValue<T>` for typed reads. State objects use
`Mutex`/atomics so an `Arc` can be read from anywhere. `Control` is the
blanket trait `DrawControl + HostedControl` (`src/control.rs:12-14`).

`HostedControl` (`src/control.rs:17-53`): `bounds`, `hit_test(pos) ->
Option<(ControlPart, f32)>`, `handle_mouse`, `handle_keyboard`, and
defaults for `set_hover`, `handle_gesture`, `handle_focus` (must tolerate
repeated `Gained`), `default_flags` (`FOCUSABLE`), `claims_drag` (false).
`ControlHandle<'a, C>` (`src/handle.rs:11`) forwards both traits
(`src/control.rs:55-131`).

### 6.1 Label (`src/label.rs`)
`LabelState { text: Mutex<String>, is_active: AtomicBool }` (`:21-25`).
`Label { state, rect, align: TextAlign, font_size, color, bg_color,
corner_radius, glow_style }` (`:64-80`); setters `with_align`,
`with_font_size`, `with_color`, `with_bg`, `with_active`,
`with_glow_style`. Draws an optional rounded background (radius
`corner_radius` or `theme.corner_radius`, `:168`), the text, and when active
a glowing ring (`:201`). `ReadValue<String>`. Default flags: see `:208`.

### 6.2 Button (`src/button.rs`)
`ButtonState { is_pressed, is_active, was_clicked, is_momentary,
is_hovered }` (all `AtomicBool`, `:21-31`); `new(pressed)`,
`momentary(pressed)`, `click`, `take_click`, `toggle`, setters. `Button {
state, text, rect, align, font_size, text_color, bg_color, border_color,
border_thickness, corner_radius, glow_style, tooltip_style }` (`:123-147`).
Draws background (`default_bg [0.08,0.12,0.18,0.90]`, pressed
`[0.16,0.32,0.52,0.95]`, border `[0.18,0.65,0.92,0.60]` when not overridden,
`:277-302`), text from `split_label` (abbreviation before `" - "`), the
active ring (`:338`), and in `draw_overlay` the tooltip (`:348`).
`shown_text`, `tooltip`. `ReadValue<String>` and `ReadValue<bool>`.
Activates on the `Click`, so a scroll never presses it.

### 6.3 Slider (`src/slider.rs`)
`SliderState { value: Mutex<f32>, is_active, is_dragging }` (`:17-23`).
`Slider { state, min, max, rect, track_height, thumb_size, thumb_style:
ThumbStyle, track_bg_color, track_fill_color, thumb_color,
thumb_border_color, glow_style }` (`:73-97`); `normalized_fraction`,
`thumb_rect`, `with_track_height`, `with_thumb_size`. Colours fall back to
`theme.track_bg_color`, `track_fill_color`, `thumb_color`,
`thumb_border_color` (`:196-201`); active ring around the thumb (`:278`).
`claims_drag` is true (`:290`). `ReadValue<f32>`. Emits `ValueChange`.

### 6.4 SliderNumeric (`src/slider_numeric.rs`)
Composes a `Slider`, an `Edit` and an optional `Label` (`:124-144`) over
`SliderNumericState { slider: Arc<SliderState>, edit: Arc<EditState>,
active_part: Mutex<SliderNumericActive> }` (`:67-73`). `NumericFormat`
(`:21`, `format_value`), `with_label`, `with_label_width`,
`with_numeric_width`, `with_format`, `with_active(SliderNumericActive)`,
`sub_rects() -> (Option<Rect>, Rect, Rect)`. The label is drawn in
`theme.muted_text_color` (`:308`). `claims_drag` true (`:341`).
`ReadValue<f32>` and `ReadValue<String>`.

### 6.5 ArrowLabel (`src/arrow_label.rs`)
`ArrowLabelState { text, active_part: Mutex<ArrowLabelActive> }` (`:35-39`).
`ArrowLabel { state, rect, arrow_width, chevron_thickness, font_size,
text_color, arrow_color, bg_color, left_enabled, right_enabled, glow_style }`
(`:78-100`); `sub_rects() -> (left, centre, right)`. Chevrons in
`arrow_color` or `theme.arrow_color` (`:227`), radius `theme.corner_radius`
(`:231`), a ring on the active part (`:297-306`). Emits `StepLeft`/`StepRight`.
`ReadValue<String>`.

### 6.6 SelectableLabel (`src/selectable_label.rs`)
Generic `SelectableLabel<T: 'static>` over `options: &'static [T]` with
`SelectableLabelState<T> { selected_index: AtomicUsize, active_part }`
(`:19-23`); `from_static(&[&str], rect)`, `with_selected_index`,
`with_format_fn(fn(&T) -> String)`, `selected_value`, `display_string`.
`HostedControl` is implemented for every `T` (`:379`) but `DrawControl`
only for `SelectableLabel<&'static str>` (`:284`), so only that
instantiation is a `Control`. `ReadValue<T>` for `T: Clone`.

### 6.7 Radio (`src/radio.rs`)
`RadioState { selected_index, is_active, hovered_index }` (`:37-43`;
`NO_CELL = usize::MAX`, `:33`). `Radio { state, labels: &'static [&'static
str], rect, cell_width (30), cell_gap (3), font_size, text_color,
selected_bg, unselected_bg, selected_border, unselected_border,
border_thickness, corner_radius, glow_style, tooltip_style, abbreviate }`
(`:99-132`); cell colours default to module constants (`:24-30`), not the
theme. `cell_rect`, `cell_rects`, `packed_width`, `cell_at`,
`with_abbreviate`. Ring on the selected cell (`:426`); tooltip in the
overlay pass (`:436`). `ReadValue<usize>` and `ReadValue<&'static str>`.

### 6.8 Options (`src/options.rs`)
Up to `MAX_OPTIONS = 32` toggles as a `u32` bit set (`OptionsState { bits:
AtomicU32, cursor, is_active, hovered_index }`, `:43-51`), laid out
`per_row` (default 5) with `row_gap` (3.0). Fields `:120-163` add
`on_bg/off_bg/on_border/off_border`, `muted: u32`, `muted_bg/muted_border`,
defaults from module constants (`:29-39`). `with_bits`, `with_rows`,
`with_muted`, `toggle(index)`, `mask`, `bit(index)`, `rows`,
`grid_height`, `cell_at`. `ReadValue<u32>`, `<usize>`, `<&'static str>`.

### 6.9 Panel (`src/panel.rs`)
`Panel<S = ()>` with `PanelState { title, subtitle: Mutex<Option<String>>,
is_active }` (`:22-28`) and an optional typed `Arc<PanelData<S>>`
(`read`/`write`/`set`/`get`, `:76-110`; `with_data`, `read_data`,
`write_data`). Fields `:113-137`: `corner_radius`, `border_thickness`,
`border_color`, `bg_color`, `title_color`, `subtitle_color`, title and
subtitle font sizes (default `theme.default_font_size * 1.1` and `* 0.85`,
`:355`, `:365`), `padding`, `glow_style`; subtitle colour default
`[0.65,0.78,0.90,0.90]` (`:366`). `content_rect(font_size_base)`. Children
are router nodes, not panel members. `ReadValue<String>`.

### 6.10 Edit (`src/edit.rs`)
`EditState { text: Mutex<String>, cursor: Mutex<usize>, is_active }`
(`:77-83`) with UTF-8-boundary cursor moves, `insert_char`, `insert_text`,
`delete_backwards`, `delete_forwards` (`:132-240`). `Edit { state, rect,
align, font_size, text_color, bg_color, border_color, border_thickness,
corner_radius, glow_style }` (`:258-278`); `new` sets `bg_color` and
`border_color` to literals (`:290-291`). `char_index_at_x` hit-tests a
click to the nearest character (`:390`); the cursor bar is drawn in
`theme.accent_color` (`:511`). `claims_drag` true (`:523`). No selection
range exists in the state. `ReadValue<String>`.

### 6.11 TextArea (`src/text_area.rs`)
Multi-line edit over `TextAreaState` with soft wrapping (`LineLayout {
start, end, next_start, width }`, `:44-52`). Fields `:227-251` add
`padding: [f32; 2]`, `line_height`, `placeholder` (drawn in
`theme.muted_text_color`, `:480`). `available_width`,
`char_index_at_pos`. Cursor in `theme.accent_color` (`:520`);
`claims_drag` true (`:532`). `ReadValue<String>`.

### 6.12 ScrollPane (`src/scroll.rs`)
`ScrollAxis { Vertical, Horizontal }`, `ScrollEdge { Start, End }`,
`Overscroll { None, Px(f32), Fraction(f32) }` (all `#[non_exhaustive]`,
`:26-82`). `ScrollState` (`:422`, `Arc`): `new()`, `sticky(edge)`,
`offset`, `max`, `overscroll`, `is_following`, `is_moving`. `ScrollPane`
(`:498`): `new(viewport, axis)`, `with_state`, `with_content_size`,
`with_anchoring`, `with_indicator`, `with_overscroll`, `with_background`,
`state`, `viewport`, `axis`, `set_viewport`, `set_content_size`. Children
are router nodes laid out on the content plane from the pane's top-left at
offset zero; section 4.7 describes the router's part.

### 6.13 Helpers that are not nodes
`GlowRingStyle` (`src/ring.rs:10-23`: `padding 2.0`, `corner_radius 1.0`,
`inner_thickness 1.2`, `outer_thickness 2.8`, optional inner/outer colours;
`compact()`, `sharp()`) and `draw_glowing_ring(list, rect, style, theme)`
(`:61`), which falls back to `theme.active_ring_color` and
`theme.active_glow_color`. `TooltipStyle` (`src/tooltip.rs:29-51`) with
`with_*` setters, `within(bounds)`, `place(anchor, size)`;
`draw_tooltip` (`:169`); `LABEL_SEPARATOR = " - "` and `split_label`
(`:9-13`). `Rect` and `TextAlign` (section 9).

## 7. Values and handles

`ReadValue<T>` and `ControlHandle<'a, C>` (`src/handle.rs:4-36`) give a
typed read of a borrowed control; the `Arc` state is the way to read a value
without the router (README usage, `mute.state.clone()`).

## 8. The theme today

This is a living section describing theming as the code does it; the
owner's theming amendment extends it.

### 8.1 `ControlTheme` (`src/theme.rs:4-37`)
An exhaustive `Clone + Debug + PartialEq` struct with public fields:
`bg_color`, `text_color`, `muted_text_color`, `accent_color`,
`track_bg_color`, `track_fill_color`, `thumb_color`, `thumb_border_color`,
`arrow_color`, `active_ring_color`, `active_glow_color` (all `[f32; 4]`),
`default_font_size` (14.0), `px_range` (6.0), `corner_radius` (2.0),
`scroll: ScrollTheme`. Defaults at `:99-117` (a dark cyan palette).
Presets: `dark_fantasy()` (= default) and `solar_gold()` (gold accents,
`:120-134`). There are no `with_*` setters on `ControlTheme`; it is built by
struct update syntax.

### 8.2 `ScrollTheme` (`src/theme.rs:39-96`)
`#[non_exhaustive]`, `Copy`: `touch_slop 8.0`, `wheel_line 48.0`,
`fling_decay 3.2`, `min_fling 30.0`, `max_fling 8000.0`, `rest 0.100`,
`overscroll_spring 12.0`, `indicator_width 4.0`, `indicator_inset 2.0`,
`indicator_min_length 24.0`, `indicator_color [0.55,0.62,0.70,0.5]`,
`indicator_idle 0.5`, `indicator_fade 0.150`. Input reads the values of
the theme the router was last drawn with (`src/router.rs:595`).

### 8.3 How a theme reaches a control
The theme travels only in `DrawContext.theme`, cloned per context; the
router passes the same context to every node. A control reads it at draw
time through `unwrap_or(ctx.theme.x)` on its own `Option` override field.
Which theme fields each control reads: Label `corner_radius`,
`text_color`, `default_font_size`; Button `corner_radius`, `text_color`,
`default_font_size`; Slider the four track/thumb colours; SliderNumeric
`muted_text_color`; ArrowLabel and SelectableLabel `arrow_color`,
`corner_radius`; Radio and Options `corner_radius`; Panel `corner_radius`,
`default_font_size`; Edit and TextArea `corner_radius`, `text_color`,
`accent_color` (cursor), TextArea also `muted_text_color`; every active
ring `active_ring_color`/`active_glow_color` through `draw_glowing_ring`;
ScrollPane everything in `ScrollTheme`.

### 8.4 Values not in the theme
These are per-control defaults or module constants, not theme fields:
Button background, pressed and border colours (`src/button.rs:277-302`);
Radio cell colours (`src/radio.rs:24-30`); Options on/off/muted cell
colours and the cursor colour (`src/options.rs:29-39`, `:539`); Edit and
TextArea background and border (`src/edit.rs:290-291`,
`src/text_area.rs:263-264`); Panel subtitle colour (`src/panel.rs:366`);
`TooltipStyle` colours (`src/tooltip.rs:54-70`); `GlowRingStyle`
geometry (`src/ring.rs:25-36`); `Backdrop` colour (`src/modal.rs:645`);
`PanelFrame` colours (`src/modal.rs:573-581`); every cell width, gap, arrow
width and chevron thickness. `ControlTheme.bg_color` is defined but the
controls read it nowhere in `src/` (`grep theme.bg_color` finds no use).

## 9. Geometry (`src/rect.rs`)

`Rect { x, y, w, h }` (`:5-10`), `Copy`: `new`, `pos`, `size`, `right`,
`bottom`, `center`, `pad`, `pad_xy`, `split_h(left_width, gap)`,
`split_fraction(frac, gap)`, `contains(pt)`, `normalized_x(x)` (`:21-113`).
`TextAlign` (`:14`). All coordinates are absolute logical pixels; a scroll
pane's children are absolute in the pane's content plane.

## 10. Versioning and public-API stability

Version 0.9.1 (`Cargo.toml:3`), tagged; `VERSIONS.md` lists 0.7.0, 0.8.0,
0.9.0 and 0.9.1 at `:185-248`. `#[non_exhaustive]` at HEAD: `Key`,
`EventContext`, `FocusEvent`, `ControlAction` (`src/event.rs`); `Filtered`,
`FilterContext` (`src/filter.rs`); `NameError`, `DragGrab`,
`EventResponse` (`src/router.rs`); `ScrollTheme` (`src/theme.rs`);
`ScrollAxis`, `ScrollEdge`, `Overscroll` (`src/scroll.rs`); `ModalPhase`,
`DismissReason`, `ModalEvent`, `ModalOptions`, `ModalError`
(`src/modal.rs`). Exhaustive with public fields: `ControlTheme`,
`DrawContext`, `Rect`, `Modifiers`, `MouseEvent`, `InputEvent`,
`ControlPart`, `GlowRingStyle`, `TooltipStyle`, every control struct and
state struct. The Drawer plan's amendment (`709290b`) records the owner's
convention that appending a `ControlTheme` field is additive.

## 11. Testing

49 unit tests in `src/` (`src/lib.rs` 36, `src/scroll.rs` 10,
`src/button.rs` 3) and 312 integration tests in `tests/`: `adversarial.rs`
39, `adversarial2.rs` 27, `router.rs` 46, `modal.rs` 65, `modal_layers.rs`
23, `routing.rs` 31, `scroll.rs` 30, `filter.rs` 18, `text_area.rs` 9,
`events.rs` 7, `edit.rs` 6, `symbol.rs` 11. Tests drive a `Router` with
`deliver_*` and inspect the `DrawList` (`SdfKind`, pushed items) with a
`DrawContext::default()` (no atlas or shaper, so text is measured by the
`0.55 em` estimate). `tests/common/mod.rs` has a control that journals every
host call and `take_focus_journal()` (`:1`, `:21`). `cargo test` runs
without a GPU.

## 12. Gaps and inconsistencies visible in the code

- `AGENTS.md:1` and commit `ae7c381` call the library immediate-mode; the
  code is a retained tree (section 2). The README is right.
- The README's controls list omits `TextArea` (exported, `src/lib.rs:60`)
  and the modal types; its Edit line says "with selection", but `EditState`
  has no selection range (`src/edit.rs:77-83`).
- The decision table says plan 02 removed `set_modifiers`; `Router::set_modifiers`
  exists (`src/router.rs:773`) beside `deliver_modifiers` (`:788`).
- `FilterStack` is `Rc` (`src/filter.rs:54`) while control state is `Arc`
  with atomics and mutexes: the router is single-threaded, the values are
  not.
- `SelectableLabel<T>` is a `Control` only for `T = &'static str` (6.6).
- `ControlTheme.bg_color` is unused (8.4); most control backgrounds and
  borders are literals outside the theme.
- Press feedback (plan `20261023_01`) is not implemented: no
  `NodeState::PRESSED`, no `DrawContext::press`.
- `Router::set_focus` and the `deliver_*` family take `&self`; mutation of
  the tree (`add`, `remove`, `set_flags`, `get_mut`) takes `&mut self`.

## Open questions for the owner

1. Should the AGENTS.md heading drop "immediate-mode" to match the code?
2. Is `Router::set_modifiers` meant to stay, or is `deliver_modifiers` the
   one way in?
3. Should `ControlTheme.bg_color` be read by anything, or removed in the
   theming amendment?
4. Is `SelectableLabel` meant to draw for any `T` with a `format_fn`, as
   its generic `HostedControl` suggests?

## Amendment — 2026-10-28 11:38 EDT: the theming standard for themed controls (living section 8)

The owner (2026-10-28), in the drawer plan's discussion
(`docs/design/20261028_01_DRAWER_PLAN.md`, the 11:28 EDT amendment): "theme
as defaults, options as override with a helper taking the theme as input
(such with_theme)", then "that should be the standard for themed
components", and on versions: "right, it's additive, a caller can always
use ..default". This adds section 8.5. Sections 8.1-8.4 stay the record of
what the code does at `d53ef99`; 8.5 is the standard that new controls
follow and existing ones move to.

### 8.5 The standard for a themed control

For a control `X` with values an app themes:

1. **Defaults in the theme.** `ControlTheme` has a field `x: XTheme`
   holding the app-wide defaults for every `X`. `XTheme` is
   `#[non_exhaustive]`, `Clone + Debug + PartialEq` (`Copy` when its
   fields are), with a public `Default` that gives the values the control
   uses today. Values several controls share (`text_color`,
   `default_font_size`, `corner_radius`, the active ring's colours) stay
   flat `ControlTheme` fields, and `XTheme` does not repeat them.
2. **Overrides in the options.** `XOptions` has the same values as
   `Option`s, `None` meaning "the theme's", with one builder setter per
   field (`.size(..)`, `.scrim(..)` …) and a `Default` that overrides
   nothing. It also holds per-instance settings that have no theme default
   (a drawer's edge, a pane's axis).
3. **`XOptions::with_theme(&self, theme: &XTheme) -> XResolved`** resolves
   them: each `Some` wins, each `None` takes the theme's value.
   `XResolved` has plain values and is what the control reads. The control
   keeps its `XOptions` and resolves against the theme it is drawn with
   (`DrawContext.theme`, section 8.3), so a theme change reaches every
   value an instance did not override.
4. **Versions.** Appending `x: XTheme` to `ControlTheme` is additive by
   the owner's ruling (a caller builds a theme with
   `..ControlTheme::default()`), and so are new `XTheme`, `XOptions` and
   `XResolved` types and fields appended to them: a patch release.
5. **Existing controls.** Today's per-control `Option` override fields
   read through `unwrap_or(ctx.theme.x)` (section 8.3) are this pattern
   without its types. A control moves to the standard by gaining its
   `XTheme`/`XOptions`/`with_theme`, keeping its existing `with_*`
   setters as calls that set the matching override, so the move is
   additive. The values of section 8.4 that an app would theme move into
   the control's `XTheme`. Each move is its own plan.
6. **First users.** The drawer (`docs/design/20261028_01_DRAWER_PLAN.md`,
   `DrawerTheme`, `DrawerOptions`, `DrawerResolved`) is the first
   control built to it. `ScrollTheme` (section 8.2) is the theme half
   already; `ScrollPane`'s `with_overscroll`, `with_background` and
   `with_indicator` would become `ScrollOptions` overrides.

## Amendment — 2026-10-28 11:39 EDT: the living spec of the library; status Implemented

The owner (2026-10-28): "it becomes the living spec plan once finished and
should be added to the AGENTS.md". The document is finished as the
description of libcontrols at `d53ef99` with the theming standard of the
previous amendment, and becomes the library's **living spec**:

1. **Status `Implemented`.** It describes code that exists, so it has no
   steps left; it stays frozen except for its living sections.
2. **Every section that describes the code is living**, beyond the two the
   front matter names (6 and 8): when a plan's implementation changes what
   a section describes (a new control, a router or event change, a theme
   value, a version), an amendment appended here records the change, names
   the plan and its commits, and states which earlier text it replaces.
   Sections 1 (the starting point) and 12 (the gaps at `d53ef99`) are
   fixed; a gap that is closed is recorded by such an amendment.
3. **`AGENTS.md` points to it**, so every agent working on the library
   reads it first and keeps it current.
4. **The open questions stay open** for the owner; an answer is recorded
   by amendment.

## Amendment — 2026-10-28 12:22 EDT: immediate drawing, persisted controls (open question 1)

The owner (2026-10-28), answering open question 1: "immediate-drawing,
persisted controls". libcontrols draws immediately (the router redraws
every node into a fresh `DrawList` each frame, section 4.1) and keeps its
controls persisted (the router's retained tree, section 4.2), so neither
"immediate-mode" alone nor "retained" alone describes it. Section 2's
characterisation is read as this. `AGENTS.md`'s heading now reads
"libcontrols: immediate-drawing, persisted UI controls as MSDF primitives"
(commit after `c1daa98`); the initial commit's wording (section 1) stays
as the record.

## Amendment — 2026-10-28 12:28 EDT: open question 4 answered; open question 3 clarified

The owner (2026-10-28):

1. **Open question 3** (`ControlTheme.bg_color`): asked "bg_color defaults
   to black?"; it defaults to `[0.08, 0.10, 0.14, 0.85]` (`src/theme.rs:97`),
   a near-black navy at 85% alpha, and no control reads it. Whether it
   becomes the shared background a control falls back to under the theming
   standard (section 8.5) or is removed is still open.
2. **Open question 4** (`SelectableLabel` for any `T`): "chain them, but
   split should be enabled via setter, it was added for a minimalist ui but
   this is a common UI library". `format_fn` (or `Display`) makes the
   text, and the `"XX - Name"` split applies to it only when enabled by a
   setter. The work is
   `docs/design/20261028_03_SELECTABLE_LABEL_FORMAT_AND_ABBREVIATION_PLAN.md`
   (Draft); this spec's section 6.6 and the section 12 gap are amended when
   it is implemented.

## Amendment — 2026-10-28 12:37 EDT: SelectableLabel for any Display type; the abbreviation opt-in (sections 6.6, 6.7, 6.8, 12)

Plan `docs/design/20261028_03_SELECTABLE_LABEL_FORMAT_AND_ABBREVIATION_PLAN.md`,
commits `1d2fd40` and `4d8530e`, release 0.10.0 (`v0.10.0`). This replaces what sections
6.2 and 6.6-6.8 say about drawing labels and closes section 12's `SelectableLabel`
gap.

1. **6.6 `SelectableLabel`:** `DrawControl` is implemented for every `T:
   Display + 'static`. The centre text is `shown_text()`: `format_fn`'s
   text, else `Display`'s, cut by `split_label` only when `abbreviate`
   (default `false`, `with_abbreviate`) is on; `tooltip()` is the name,
   drawn on hover with `tooltip_style` (`with_tooltip_style`).
   `SelectableLabelState` has `is_hovered`.
2. **6.7 `Radio`, 6.8 `Options`:** `abbreviate` defaults to `false`; off, a
   label is drawn as written (`"XX - Name"` included) with no tooltip; on,
   as before.
3. **12:** the `SelectableLabel` gap (`HostedControl` for all `T`,
   `DrawControl` only for `&'static str`) is closed.
5. **6.2 `Button`:** `abbreviate` (default `false`, `with_abbreviate`): off,
   `shown_text` is the label as written and `tooltip` is `None`; on, split
   as before (commit `4d8530e`).
4. **10:** the crate is 0.10.0.

## Amendment — 2026-10-28 12:50 EDT: Drawer and DrawerLayout (sections 3, 6, 8, 10)

Plans `docs/design/20261028_01_DRAWER_PLAN.md` and
`docs/design/20261028_04_DRAWER_LAYOUT_PLAN.md`, commit `cf12470`, release
0.10.1 (`v0.10.1`); the departures are in `CHANGES.md` (this time).

1. **3, dependencies:** `wgpu` 30.0.1 and `bytemuck` 1 are direct
   dependencies (the versions libmsdf uses). This replaces "libmsdf is the
   only dependency".
2. **6.14 `DrawerLayout` and `Drawer`** (`src/drawer/`): a layout with two
   `Router` children, main filling its rect and the drawer's content in
   sheet coordinates, placed by one `DrawerEdge` (Left by default) and its
   size. The `Drawer` is driven by calls only (`open`, `close`, `toggle`,
   `set_open_now`, `set_progress`, `settle`), with no gestures; its
   progress `t` eases with Material 3's emphasized curves. `deliver` routes
   a press to the side its Down landed on, keys to the content while it is
   showing, and reports scrim points as `DrawerRoute::Scrim`.
   `DrawerLayers` draws each side into a wgpu texture only while `t > 0`
   and composites them in one pass (slide, scrim, shadow); at `t = 0` it
   creates nothing and the main view draws straight to the surface.
   `src/ease.rs` holds the curves. Only the device, queue, target view and
   format are taken from the app, and only `main_view()` is given back.
3. **8, theme:** `ControlTheme::drawer: DrawerTheme` (`size`,
   `size_vertical`, scrim, sheet colour, corner radius, shadow, durations,
   curves, `main_updates`, `keep_warm`), overridden per drawer by
   `DrawerOptions` and resolved by `with_theme`: the first control built to
   section 8.5's standard.
4. **10:** the crate is 0.10.1.

## Amendment — 2026-10-28 18:53 EDT: EditableLabel (sections 6, 8, 10, 11)

Plan `docs/design/20261028_05_EDITABLE_LABEL_PLAN.md`, release 0.10.2
(`v0.10.2`); the commit is the one that adds `src/editable_label.rs`. This
adds section 6.15 and extends sections 8, 10 and 11; it replaces no
earlier text except the version in section 10 and the test counts in 11.

1. **6.15 `EditableLabel` (`src/editable_label.rs`).** One node with two
   modes over a shared `EditableLabelState` (the committed `text`, a draft
   `Arc<EditState>`, an `editing` flag and an event queue):
   `new`, `text`, `set_text`, `is_editing`, `draft`, `draft_state`,
   `set_draft` (a soft keyboard's whole-text update, cursor to the end),
   `begin_edit`, `confirm`, `cancel`, `clear`, `take_events` of
   `EditableLabelEvent` (`#[non_exhaustive]`: `Started`,
   `Confirmed(String)`, `Cancelled`, `Cleared`). Display mode draws the
   text (or the placeholder in the placeholder colour) from the left with
   4 px padding and a pencil (six `push_line` segments) in a
   `button_width` button at the right; editing draws an `Edit` over the
   draft (align Left) in what is left minus `gap`, the placeholder over an
   empty field, a check (two segments) in place of the pencil and, when
   `clear` is on and the draft is not empty, an X (two segments) left of
   it. Parts: `EditableLabel::PENCIL`, `CONFIRM`, `CLEAR`
   (`ControlPart::Custom`), `TEXT` (`Label`), `FIELD` (`NumericBox`,
   `Edit`'s); `parts(&resolved) -> EditableLabelParts`, `shown_text`,
   `shows_placeholder`, `clear_shown`, `resolved(&ControlTheme)`. Buttons
   act on `Click`; the text starts editing on `Click` only with
   `label_edits`; the field takes `Down`/`Click`/`Drag` and keys as
   `Edit`, and claims the drag. Enter confirms; `Key::Escape` and
   `Key::Back` cancel; `FocusEvent::Lost` while editing cancels (so the
   router's own Escape, which clears the focus, cancels, as does a press
   elsewhere); Enter or Space in display mode start editing. `FOCUSABLE`
   by default, `NodeFlags::NONE` when `read_only` (text only, no pencil,
   never edits). The control keeps the resolved values of its last draw in
   a `Cell` for `hit_test`. No keyboard or IME hook was added: the app
   starts the soft keyboard from `is_editing`/`draft` and feeds it back
   with `set_draft` and `confirm`. `ReadValue<String>` is the committed
   text.
2. **8, theme:** `ControlTheme::editable_label: EditableLabelTheme`
   (`button_width` 32, `icon_size` 14, `icon_thickness` 1.6, `gap` 4,
   `pencil_color`/`clear_color`/`placeholder_color` `None` =
   `muted_text_color`, `confirm_color` `None` = `accent_color`,
   `field_bg [0.10, 0.13, 0.18, 0.85]`, `field_border [0.15, 0.20, 0.28,
   0.60]`, `field_border_thickness` 1), overridden per label by
   `EditableLabelOptions` (one setter per value, plus `font_size`,
   `text_color` and the per-instance `placeholder`, `clear`, `read_only`,
   `label_edits`) and resolved by `with_theme` into
   `EditableLabelResolved`: built to section 8.5's standard.
3. **10:** the crate is 0.10.2. `EditableLabelEvent`,
   `EditableLabelTheme`, `EditableLabelOptions`, `EditableLabelResolved`
   and `EditableLabelParts` are `#[non_exhaustive]`.
4. **11:** `tests/editable_label.rs` adds 13 router tests.

## Amendment — 2026-10-28 19:58 EDT: PopupMenu (sections 4.6, 6, 8, 10, 11)

Plan `docs/design/20261028_06_POPUP_MENU_PLAN.md`, release 0.10.3
(`v0.10.3`); the commit is the one that adds `src/popup_menu.rs`. This
adds section 6.16 and extends sections 4.6, 8, 10 and 11; it replaces no
earlier text except the version in section 10 and the test counts in 11.

1. **4.6, the modal stack:** `ModalStack` also keeps **attached** modals
   (a private list of an entry id and a function making the modal). When
   `sync` finds a live entry with no layer and no queued push for it, it
   makes the layer from the attached modal of that id. An entry pushed on
   the `ModalState` alone therefore gets a layer at the stack's next
   routed event or draw. The only way in is `Router::attach_popup_menu`;
   no public modal type changed.
2. **6.16 `PopupMenu` (`src/popup_menu.rs`).** A menu of actions anchored
   to a rect, opened and closed by calls on its shared `PopupMenuState`
   (`open(anchor, items)`, which replaces the items while open; `close`;
   `is_open`; `take_events`; `anchor`, `items`, `highlighted`, `layout`),
   with no gestures of its own. `Router::attach_popup_menu(id, menu)`
   (`ModalError::NoStack` without a stack; attaching an id again replaces
   the menu for the next open) binds the state to the stack's
   `ModalState`; `open` then pushes entry `id` with
   `ModalTransition::Instant` and the stack draws a clone of the menu as
   that entry's layer. The layer has no backdrop and is dismissed by
   `PopupMenuOptions::dismiss` (`DismissOn`, default all) through the
   standard `ModalLayerFilter` over the menu's last laid-out frame: an
   outside tap is consumed (nothing beneath sees it) and dismisses, as do
   Escape and Back. Items are `PopupItem { id, label, enabled, checked:
   Option<bool>, kind }` (`PopupItemKind`: `Action`, `Heading`,
   `Separator`; `action`, `heading`, `separator`, `with_enabled`,
   `with_checked`, `is_choosable`). `PopupMenuEvent`: `Chosen(Symbol)`
   (a choice; the menu closes) and `Dismissed` (the entry ended by
   anything but a choice or `close`, noticed by `is_open` or
   `take_events`); `close` queues nothing. Placement
   (`PopupMenu::layout(&DrawContext) -> PopupMenuLayout { frame, items,
   above }`): width from the widest label plus paddings and the check
   column, within `min_width..=max_width` and the viewport less
   `margin`; right-aligned to the anchor and moved inside the viewport;
   below the anchor by `offset` when it fits, else above when that fits,
   else on the roomier side, clamped. Each draw stores the layout in the
   state; hit tests and the layer's shape read it (the default theme
   before the first draw). Input: `Move`/`Drag` highlight the enabled
   action under the pointer; `Down` records it as pressed; `Click`
   chooses it when it is the pressed one or no press was recorded; Up and
   Down keys move the highlight over enabled actions, wrapping, Home and
   End to the first and last, Enter and Space choose; the pointer leaving
   clears the highlight. A choice answers `ControlAction::Activated {
   part: ControlPart::Custom(index) }`; `hit_test` gives an item as
   `ControlPart::Custom(index)`. Draw: shadow, background, outline,
   highlight, labels (headings in the heading colour, disabled actions at
   `disabled_alpha`), check marks (two `push_line` segments), separators
   (one line). `item_at`, `resolved`, `with_state`, `with_options`.
3. **8, theme:** `ControlTheme::popup_menu: PopupMenuTheme` (`bg [0.08,
   0.11, 0.16, 0.98]`, `border [0.18, 0.65, 0.92, 0.35]`,
   `border_thickness` 1, `corner_radius` 6, `item_height` 40, `padding` 6,
   `item_padding` 14, `min_width` 160, `max_width` 320,
   `separator_height` 9, `separator_thickness` 1, `separator_color [0.30,
   0.36, 0.44, 0.60]`, `text_color`/`heading_color`/`check_color`/
   `font_size` `None` = `text_color`/`muted_text_color`/`accent_color`/
   `default_font_size`, `highlight [0.18, 0.65, 0.92, 0.20]`,
   `check_width` 24, `check_size` 12, `check_thickness` 1.6,
   `disabled_alpha` 0.4, `shadow [0, 0, 0, 0.35]`, `shadow_offset` 3,
   `offset` 4, `margin` 8), overridden per menu by `PopupMenuOptions` (one
   setter per value, plus the per-instance `dismiss`) and resolved by
   `with_theme` into `PopupMenuResolved`: built to section 8.5's
   standard.
4. **10:** the crate is 0.10.3. `PopupItemKind`, `PopupMenuEvent`,
   `PopupMenuLayout`, `PopupMenuTheme`, `PopupMenuOptions` and
   `PopupMenuResolved` are `#[non_exhaustive]`; `PopupItem` is a plain
   struct with public fields.
5. **11:** `tests/popup_menu.rs` adds 20 router tests.

## Amendment — 2026-10-28 23:26 EDT: Scrollbar and the index of sections (sections 4.7, 6, 8, 10, 11)

Plan `docs/design/20261028_08_SCROLLBAR_PLAN.md`, steps 1-5 and 8, release
0.10.4 (`v0.10.4`); the commit is the one that adds `src/scrollbar/`.
This adds sections 6.17 and 6.18 and extends 4.7, 6.12, 8, 10 and 11; it
replaces no earlier text except the version in section 10 and the test
counts in 11. The plan's steps 6 and 7 (sections drawn by a pane through
`render_section`, their cache, anchoring and `Router::scroll_to_section`)
are not built: they need a libmsdf scoped append, and the libmsdf pin in
section 3 is unchanged.

1. **4.7:** a pane with `with_scrollbar` takes a press in its scrollbar's
   band before its children (the innermost such pane on the hit path):
   the press grabs the thumb, stops a fling, and its drag and release go
   to the scrollbar, whose scrubs the router applies with the pane's
   `scroll_to`; the `Click` after it is dropped. A grab that goes away (a
   layer opened, the grab moved) ends as a release. The router feeds the
   scrollbar from the pane on each range update (the viewport as the
   track, the offset, the content's length, whether the pane is dragged
   or coasting and when it last moved) and draws it after the pane's clip
   ends, where the indicator was, with the disabled halving; the
   indicator is then not drawn. The bookmarks are drawn inside a clip of
   the viewport in screen pixels. `Router::advance` moves the clocks of
   panes' scrollbars and of `Scrollbar` nodes and reports them busy while
   an `OnScroll` thumb is shown or bookmarks are shown or fading. The
   router keeps the `ScrollbarTheme` of the theme it was last drawn with
   for input, as it keeps the `ScrollTheme`. A pane without
   `with_scrollbar` routes, draws and anchors as before.
2. **6.12:** `ScrollPane::with_scrollbar(ScrollbarOptions)` (vertical panes
   only; on a horizontal pane it does nothing and `scrollbar()` is
   `None`) and `ScrollPane::scrollbar() -> Option<&Arc<ScrollbarState>>`;
   `ScrollState::content_length`.
3. **6.17 `Scrollbar` (`src/scrollbar/`).** `ScrollbarState` (`Arc`):
   `new`, `with_sections`, `set_sections`, `set_metrics(offset, content,
   window)`, `set_scrolling`, `advance(dt) -> bool`, `take_events`,
   `is_held`, `alpha`, `bookmarks_alpha`, `press`, `drag_to`, `release`,
   `cancel`. The thumb is `width` wide, its length the window's share of
   the content times the track's height, at least `min_length` and at
   most the track, its top `track.y + offset / range * (track.h -
   length)`, its right edge `slop` in from the track's; none when the
   range is 0.5 or less or the window is empty. The band is the thumb's
   left less `slop` to the track's right edge, the track's height, and
   exists whenever there is a thumb and the visibility is not `None`. A
   press on the thumb grabs at the touch; elsewhere in the band the grab
   is half the thumb and it scrubs there at once. A move scrubs to
   `clamp((y - grab - track.y) / (track.h - length), 0, 1) * range`;
   release leaves the offset. `ScrollbarVisibility` (`None`, `OnScroll`
   the default, `Always`): `OnScroll` is the resolved `alpha` while held or
   scrolling and `hold` after, then fades over `fade`. `ScrollbarEvent`
   (`Grabbed { offset }`, `Scrub { offset, section, bookmark }`,
   `Released { offset, section, bookmark }`; `section` is the section at
   the offset and `bookmark` the bookmark nearest the thumb, when the
   state has sections). Bookmarks are drawn while held and `hold` after,
   then fading, in every visibility: each pill centred across the track at
   `track.y + top / content * track.h`, `font_size * line_height + 2 *
   pad_y` tall, the text's advance plus `2 * pad_x` wide, radius half its
   height, the text cut with "…" to the track's width less `2 *
   side_margin + 2 * pad_x`; at `bookmark_bg_alpha`/`bookmark_text_alpha`
   of the bookmark colour, the nearest at the `near` alphas, all times the
   fade; placed nearest first when clear of those placed by `gap`, and
   drawn top to bottom. `BookmarkText::Markdown` is drawn as written. The
   `Scrollbar` node (`new(track, options)`, `with_state`, `state`,
   `set_track`, `track`, `resolved`, `thumb_rect`, `band_rect`,
   `bookmark_layout -> Vec<BookmarkPill>`) hit-tests only its band
   (`ControlPart::Thumb`), routes `Down`, `Drag` and `Up` to the state,
   claims its drag, is not a Tab stop, answers no `ControlAction`, and
   draws its thumb (a `RoundedBox` of radius `width / 2` in the resolved
   colour at the state's alpha) and its bookmarks without a clip.
4. **6.18 Sections (`src/sections.rs`), an index only:** `SectionId(pub
   u64)`, `Section { id, height, estimated }` (`new`, `estimated`),
   `BookmarkText` (`Text`, `Markdown`) and `SectionedContent` (`Arc`):
   `new`, `set_sections`, `push`, `insert`, `remove`, `set_height`, `len`,
   `is_empty`, `sections`, `total_height`, `offset_of`, `section_at`,
   `add_bookmark` (replaces), `remove_bookmark`, `clear_bookmarks`,
   `bookmarks` (in section order). Bookmarks are kept by id and shown only
   while a section with that id exists. There is no `SectionSource`,
   `SectionContext` or drawing cache in this release.
5. **8, theme:** `ControlTheme::scrollbar: ScrollbarTheme` (`width` 4,
   `min_length` 24, `slop` 4, `color` `None` = `text_color`, `alpha`
   0.35, `hold` 2.0, `fade` 0.3, `bookmark_color` `None` = `text_color`,
   `bookmark_bg_alpha` 0.10, `bookmark_text_alpha` 0.60,
   `bookmark_near_bg_alpha` 0.20, `bookmark_near_text_alpha` 0.90,
   `bookmark_font_size` 13, `bookmark_line_height` 1.35,
   `bookmark_pad_x` 12, `bookmark_pad_y` 4, `bookmark_side_margin` 24,
   `bookmark_gap` 2, and `bookmark_code_pad` 2 and `bookmark_code_radius`
   3, which nothing reads yet), overridden per scrollbar by
   `ScrollbarOptions` (one setter per value, plus the per-instance
   `visibility` and `bookmarks`) and resolved by `with_theme` into
   `ScrollbarResolved`: built to section 8.5's standard.
6. **10:** the crate is 0.10.4. `ScrollbarVisibility`, `ScrollbarEvent`
   (and its variants), `ScrollbarTheme`, `ScrollbarOptions`,
   `ScrollbarResolved`, `BookmarkPill`, `BookmarkText` and `Section` are
   `#[non_exhaustive]`.
7. **11:** `src/scrollbar/` and `src/sections.rs` add 11 unit tests, and
   `tests/scrollbar.rs` adds 21 router tests.

## Amendment — 2026-10-28 23:41 EDT: Markdown, behind the `markdown` and `cmark` features (sections 3, 6, 8, 10, 11)

Plan `docs/design/20261028_09_MARKDOWN_VIEW_PLAN.md`, release 0.10.5
(`v0.10.5`); the commit is the one that adds `src/markdown/`. This adds
section 6.19 and extends 3, 6.17, 8, 10 and 11; it replaces no earlier
text except the version in 10, the test counts in 11, and, in the
23:26 EDT amendment, "`BookmarkText::Markdown` is drawn as written" (its
item 3) and "which nothing reads yet" of `bookmark_code_pad` and
`bookmark_code_radius` (its item 5), which now hold only without the
`markdown` feature.

1. **3:** two cargo features, none on by default: `markdown` (the
   `markdown` module, no new dependency) and `cmark` (`markdown` plus
   pulldown-cmark 0.13, default features off, optional). The theme types
   and the Scrollbar are built in every configuration. If any crate in a
   build turns `cmark` on, every `parse` and `MarkdownView` in it uses
   CommonMark.
2. **6.19 Markdown (`src/markdown/`, `markdown`):** `parse(md) ->
   MdDocument` (without `cmark` the minimal parser: paragraphs split at
   blank lines, single newlines as spaces, `***x***` bold italic, `**x**`
   and `__x__` bold, `*x*` and `_x_` italic by CommonMark's flanking rules
   (an `_` inside a word is literal), code spans with nothing formatted
   inside, escapes of ``\ * _ ` ``, bare `http(s)://` URLs as links trimmed
   of trailing punctuation, everything else literal; with `cmark`
   CommonMark with no extensions: headings bold, quotes dim and indented
   `quote_indent_chars` spaces a level, code blocks in `Code` keeping
   their lines, list items "• " or "N. " with a hanging indent, HTML
   literal, images as their alt text linked to their URL, links and bare
   URLs in order); `parse_inline(text) -> Vec<MdRun>` (the minimal
   parser's inline pass in both builds, newlines as spaces, URLs plain);
   the model `MdStyle` (`Regular`, `Bold`, `Italic`, `BoldItalic`,
   `Code`), `MdRun` (`text`, `style`, `link`, `dim`; `new`), `MdBlock`
   (`runs`, `list_depth`, `outer_markers`, `quote`, `marker`, `pre`,
   `gap`; a `'\n'` in a run is a hard break), `MdDocument` (`blocks`,
   `links`), `MdLine` (`runs`, `indent`, `gap_before`), `MdLaid`
   (`lines`, `links`); `MarkdownMeasure::new(faces, px, &MarkdownResolved)`
   with `px` and `width(text, style)` (with faces each face's shaper, code
   the terminal face's cell a character; without, 0.55 a character and
   code `code_cell_em`); `lay_out(doc, width, &measure) -> MdLaid` (words
   wrap at spaces, a word wider than the line breaks by character);
   `MarkdownFaces` (`regular`, `bold`, `italic` as `Face { id, atlas,
   shaper }`, `code` as `CodeFace { shaper, text, symbols }`, `px_range`;
   `metrics`; `BoldItalic` in `bold`); and `MarkdownView`: `new(md,
   origin, width, options)`, `with_faces`, `set_text`, `set_width`,
   `height`, `rect`, `px`, `source`, `text`, `links`, `move_to`, `center`,
   `link_at`, `link_rect`. It parses and lays out at construction and at
   `with_faces`, `set_text` and `set_width`, with the `markdown` theme and
   default font size of `ControlTheme::default()` until first drawn, then
   of the theme it was last drawn with; lines are `px * line_height`
   apart, a line's gap times that above it. It draws each run in its face
   (without faces, `DrawContext::draw_text`), links in the link colour
   with an underline box `max(px * underline_thickness, 1)` thick whose
   bottom is at `underline_at` of the line, quotes in the dim colour, code
   in the code colour, prose in the text colour, all resolved against the
   theme it is drawn with. A link's tap area is its run, from
   `link_tap_above` of a line above it, `link_tap_height` lines tall. It
   hit-tests its rect (`Body`); a left `Click` on a link answers
   `Activated { part: Custom(index) }`, anything else nothing, and it
   takes no keys. With `cmark`: `score_markdown`, `is_markdown`,
   `MarkdownScore` and `DEFAULT_MARKDOWN_THRESHOLD_PCT` (15.0), moved from
   flankr-markdown unchanged.
3. **6.17, with `markdown`:** `ScrollbarState::set_faces(Option<Arc<
   MarkdownFaces>>)`. A `BookmarkText::Markdown` bookmark's first line goes
   through `parse_inline` and is drawn as spans, the markers gone, the cut
   to fit counting the visible text (`BookmarkPill::text` is that text).
   With faces, bold, italic and code are drawn and measured in them.
   Without, every span is in the context's face: bold and bold italic at
   `bookmark_near_text_alpha` (times the colour and the fade) instead of
   the pill's text alpha, italic as regular, and code over a rounded chip
   of radius `bookmark_code_radius` in the pill's background colour, the
   line's height and `bookmark_code_pad` wider either side (counted in the
   pill's width). Without the feature a `Markdown` bookmark is drawn as
   written.
4. **8, theme:** `ControlTheme::markdown: MarkdownTheme` (`text_color`
   `None` = `text_color`, `link_color` `None` = `accent_color`,
   `dim_color` `None` = `muted_text_color`, `code_color` `None` = the text
   colour, `font_size` `None` = `default_font_size`, `line_height` 1.35,
   `block_gap` 0.5, `item_gap` 0.15, `quote_indent_chars` 4,
   `underline_thickness` 0.07, `underline_at` 0.92, `link_tap_above`
   0.15, `link_tap_height` 1.3, `code_cell_em` 0.6), overridden per view
   by `MarkdownOptions` (one setter per value, plus the per-instance
   `centered`, which centres each line at every layout) and resolved by
   `with_theme` into `MarkdownResolved`, whose colours and size stay
   `Option` and are read against a `ControlTheme` with `font_size_in`,
   `text_color_in`, `link_color_in`, `dim_color_in` and `code_color_in`:
   built to section 8.5's standard, in every configuration.
5. **10:** the crate is 0.10.5. `MdStyle`, `MdRun`, `MdBlock`,
   `MdDocument`, `MdLine`, `MdLaid`, `MarkdownTheme`, `MarkdownOptions`
   and `MarkdownResolved` are `#[non_exhaustive]`.
6. **11:** `src/theme.rs` adds 1 unit test in every build; with
   `markdown`, `src/markdown/` and `src/scrollbar/` add 14 more, and with
   `cmark` 17 more again. `tests/markdown.rs` adds 11 router tests in
   either build (10 shared and one for the parser built), and
   `tests/scrollbar.rs`'s test of a Markdown bookmark drawn as written
   runs only without `markdown`. The crate is tested with no features,
   `--features markdown` and `--features cmark`.

## Amendment — 2026-10-28 23:50 EDT: PopupMenu toggle items and columns (sections 6.16, 8, 10, 11)

Plan `docs/design/20261028_10_POPUP_MENU_TOGGLES_AND_COLUMNS_PLAN.md`,
release 0.10.6 (`v0.10.6`). This extends sections 6.16, 8, 10 and 11 as
amended; it replaces no earlier text except, in 6.16, "a choice; the
menu closes" now holding for actions only, and the version in 10 and
the test counts in 11.

1. **6.16 `PopupMenu`:** `PopupItemKind::Toggle` (`PopupItem::toggle(id,
   label, checked)`) is chosen like an action (`is_choosable`: an
   enabled action or toggle) but flips its `checked` in the state, keeps
   the menu open and the highlight, and queues `PopupMenuEvent::Toggled {
   id, checked }` with the new state, not `Chosen`. A toggle whose
   `checked` is `None` is off and flips to `Some(true)`; any toggle
   reserves the check column, so a flip never changes the layout. A
   disabled toggle is drawn at `disabled_alpha` and takes no input. A row
   may carry cells: `PopupRow { item, cells }` (`PopupRow::new`,
   `with_cells`, `From<PopupItem>`, `PopupItem::with_cells`), opened
   with `PopupMenuState::open_rows` and read with `rows`; `open` and
   `items` are unchanged. A `PopupCell { text, align: TextAlign, width:
   PopupCellWidth (Auto, Fixed(px), Flex), dim, scale }` is drawn in
   column k, shared by the k-th cells of all rows: Fixed if any cell is,
   else Flex if any is, else Auto; natural widths at each cell's size
   (`scale` clamped to 0.25..=4.0), gaps of `column_gap`; extra room to
   the Flex columns (else the last), too little taken from the Flex
   columns in proportion, then from the last column back; cells cut with
   … to their column (one line; nothing drawn in a column narrower than
   "…"), centred vertically on the row, dim ones in `dim_text_color`. A
   row with cells draws them instead of its label; a separator's cells
   are ignored. `PopupMenuLayout::columns` gives each column's x and
   width.
2. **8, theme:** `PopupMenuTheme` gains `column_gap` 12 and
   `dim_text_color` `None` = `muted_text_color`; `PopupMenuOptions` an
   override and setter for each; `PopupMenuResolved` both.
3. **10:** the crate is 0.10.6. `PopupRow`, `PopupCell` and
   `PopupCellWidth` are `#[non_exhaustive]`; `PopupItem` is unchanged.
4. **11:** `tests/popup_menu.rs` adds 15 router tests (35 in all), and
   `src/popup_menu.rs` adds 7 unit tests of the cells as drawn and of the
   column arithmetic, in every build.
