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

## 0.6.0 — 2026-10-23 (tag `v0.6.0`)

The move to libmsdf 0.5.0. The commit is the one tagged `v0.6.0`: the entry
is written in that commit, which cannot name its own hash.

- **Breaking:** libmsdf moves from 0.4.0 to 0.5.0 (`c3cf511`, tag `v0.5.0`).
  libmsdf types appear in this crate's public API (`DrawContext::atlas` is a
  `FontAtlas`, `DrawContext::shaper` a `TextShaper`, `DrawList` in
  `DrawControl`). In libmsdf 0.5.0 `BundledAtlas` is `#[non_exhaustive]`
  with two new variants, `BakeOptions` and `FaceReport` gain fields, and a
  text run's glyphs composite as one shape, so overlapping glyph edges draw
  slightly lighter. This crate's own code is unchanged.

## 0.7.0 — 2026-10-23 (tag `v0.7.0`)

The move to libmsdf 0.6.0. The commit is the one tagged `v0.7.0`: the entry
is written in that commit, which cannot name its own hash.

- **Breaking:** libmsdf moves from 0.5.0 to 0.6.0 (`4de319c`, tag `v0.6.0`).
  libmsdf types appear in this crate's public API (`DrawContext::atlas` is a
  `FontAtlas`, `DrawContext::shaper` a `TextShaper`, `DrawList` in
  `DrawControl`). In libmsdf 0.6.0 `FontAtlas` gains the field `cell_box`, so
  struct literals of it need updating; the atlas format moves to 5:0.2.0 for
  atlases with a cell box; and the bundled Texturina and Grenze Gotisch
  atlases draw their accents whole, overhanging the line box. This crate's
  own code is unchanged.

## 0.8.0 — 2026-10-23 (tag `v0.8.0`)

Scroll panes (docs/design/20261023_02_SCROLL_PANE_PLAN.md). The commit is
the one tagged `v0.8.0`: the entry is written in that commit, which cannot
name its own hash.

- **Breaking:** `Button` and `Label` activate on a left `Click` only, no
  longer on the `Down` as well, so a press that becomes a scroll does not
  activate them. A host that sends only `Down` for a tap must send the
  `Click`.
- **Breaking:** `ControlTheme` gains the field `scroll: ScrollTheme`, so
  struct literals of it need updating (`..ControlTheme::default()` covers it).
- Added: the `scroll` module: `ScrollPane` (`new`, `with_state`,
  `with_content_size`, `with_anchoring`, `with_indicator`,
  `with_overscroll`, `with_background`, `state`, `viewport`, `axis`,
  `set_viewport`, `set_content_size`), `ScrollState` (`new`, `sticky`,
  `offset`, `max`, `overscroll`, `is_following`, `is_moving`), and the
  `#[non_exhaustive]` enums `ScrollAxis`, `ScrollEdge` and `Overscroll`.
- Added: `ScrollTheme` (`#[non_exhaustive]`, with `Default`).
- Added: `Router::advance`, `scroll_to`, `scroll_by`, `scroll_into_view`,
  `to_content` and `visible_bounds`.
- Added: `HostedControl::claims_drag`, provided (default `false`); `Slider`,
  `SliderNumeric` and `Edit` return `true`.
- Changed, for nodes inside a scroll pane only: events carry positions in
  the node's content coordinates, and `Router::set_focus` and Tab scroll the
  focused node into view.

## 0.9.0 — 2026-10-25 (tag `v0.9.0`)

Cursor and insertion point in Edit (docs/design/20261025_01_EDIT_CURSOR_AND_INSERTION_POINT_PLAN.md).
The commit is the one tagged `v0.9.0`: the entry is written in that commit,
which cannot name its own hash.

- **Breaking:** `EditState` gains the public field `cursor: Mutex<usize>`, so
  struct literals of it need updating (`EditState::new` remains unchanged).
- Added: `EditState` methods: `cursor(&self) -> usize`, `set_cursor(&self, pos: usize)`,
  `move_cursor_left(&self)`, `move_cursor_right(&self)`, `move_cursor_home(&self)`,
  `move_cursor_end(&self)`, `insert_char(&self, ch: char)`, `insert_text(&self, text: &str)`,
  `delete_backwards(&self) -> bool`, and `delete_forwards(&self) -> bool`.
- Added: `Edit` methods: `cursor(&self) -> usize`, `set_cursor(&self, pos: usize)`,
  `with_cursor(self, pos: usize) -> Self`, `char_index_at_x(&self, click_x: f32) -> usize`,
  and `set_cursor_from_x(&self, click_x: f32)`.
- Added: `Edit` rendering draws an SDF vertical cursor bar (`SdfKind::Box`, 1.5 px width)
  in `ctx.theme.accent_color` when focused or active.
- Added: `Edit::handle_mouse` sets the cursor to the nearest character boundary on
  left-click, down, or drag.
- Added: `Edit::handle_keyboard` supports navigation (Left, Right, Home, End), deletion
  (Backspace, Delete), Space, and character input at the insertion cursor.

## 0.9.1 — 2026-10-25 (tag `v0.9.1`)

TextArea multi-line text edit control (docs/design/20261025_02_TEXT_AREA_CONTROL_PLAN.md).
The commit is the one tagged `v0.9.1`: the entry is written in that commit,
which cannot name its own hash.

- Added: `TextArea` multi-line text edit control (`src/text_area.rs`) with soft
  line wrapping, padding, font size, text/border/bg styling, line height,
  placeholder text, cursor tracking, and active glow ring.
- Added: `TextAreaState` type alias for `EditState`.
- Added: `wrap_text`, `LineLayout`, and `cursor_line_and_col` layout helpers.
- Added: `HostedControl` implementation on `TextArea` supporting mouse click and
  drag cursor positioning to line and character boundaries, and multi-line
  keyboard navigation (Up, Down, Left, Right, Home, End) and editing (Enter,
  Backspace, Delete, character input).
- Added: `ReadValue<String>` implementation for `TextArea`.


## 0.10.0 — 2026-10-28 (tag `v0.10.0`)

SelectableLabel for any Display type, with format_fn and an opt-in
abbreviation (docs/design/20261028_03_SELECTABLE_LABEL_FORMAT_AND_ABBREVIATION_PLAN.md).
The commit is the one tagged `v0.10.0`.

- **Breaking:** `Button`, `Radio` and `Options` now default to drawing a
  `"XX - Name"` label as written, with no tooltip (`abbreviate: false`).
  Migration: add `.with_abbreviate(true)` to every `Button`, `Radio` and
  `Options` that relied on the abbreviation and its tooltip. `.with_abbreviate(false)` now draws the whole
  label (`"XX - Name"`) rather than the name alone. The abbreviation was added
  for one minimalist UI and is not what a general UI library is expected to
  do, so it is opt-in per control.
- Added: `SelectableLabel<T>` draws for every `T: Display` (it drew only for
  `&'static str`): its text is `format_fn`'s, else `Display`'s, then split by
  the `"XX - Name"` convention only with `with_abbreviate(true)`.
- Added: `Button::abbreviate` and `with_abbreviate`.
- Added: `SelectableLabel::abbreviate`, `with_abbreviate`, `tooltip_style`,
  `with_tooltip_style`, `shown_text`, `tooltip`, `is_hovered`;
  `SelectableLabelState::is_hovered` (field), `is_hovered`, `set_hovered`.

## 0.10.1 — 2026-10-28 (tag `v0.10.1`)

Drawer and DrawerLayout (docs/design/20261028_01_DRAWER_PLAN.md,
docs/design/20261028_04_DRAWER_LAYOUT_PLAN.md). Additive: new modules and
types, a field appended to `ControlTheme` (additive by the owner's ruling: a
caller builds a theme with `..ControlTheme::default()`), and `wgpu` 30.0.1
and `bytemuck` 1 as direct dependencies at the versions libmsdf already
uses. The commit is the one tagged `v0.10.1`.

- Added: `ease` (`CubicBezier` with `STANDARD`, `EMPHASIZED_DECELERATE`,
  `EMPHASIZED_ACCELERATE`; `hermite_ease`).
- Added: `drawer`: `DrawerState` (`open`, `close`, `toggle`,
  `set_open_now`, `set_progress`, `settle`, `advance`, `phase`,
  `progress`, `take_events` …), `DrawerPhase`, `DrawerEvent`,
  `DrawerReason`, `DrawerEdge` (default `Left`), `DrawerSize`,
  `MainUpdates`, `DrawerTheme`, `DrawerOptions` (overrides, with
  `with_theme`), `DrawerResolved`, `DrawerRoute`, `Drawer`, `DrawerLayout`,
  `DrawerLayers` (`prepare`, `prewarm`, `main_view`, `render_sheet`,
  `composite`, `has_textures`, `texture_bytes` …), `LayerPlan`.
- Added: `ControlTheme::drawer: DrawerTheme`.

## 0.10.2 — 2026-10-28 (tag `v0.10.2`)

EditableLabel (docs/design/20261028_05_EDITABLE_LABEL_PLAN.md). Additive: a
new module and types, and a field appended to `ControlTheme` (additive by
the owner's ruling: a caller builds a theme with `..ControlTheme::default()`).

- Added: `editable_label`: `EditableLabel` (`PENCIL`, `CONFIRM`, `CLEAR`,
  `TEXT`, `FIELD`; `with_state`, `with_options`, `with_clear`,
  `with_placeholder`, `with_read_only`, `with_label_edits`,
  `with_font_size`, `with_text_color`, `text`, `is_editing`, `shown_text`,
  `shows_placeholder`, `clear_shown`, `resolved`, `parts`, `handle`),
  `EditableLabelState` (`begin_edit`, `confirm`, `cancel`, `clear`,
  `draft`, `set_draft`, `draft_state`, `take_events` …),
  `EditableLabelEvent`, `EditableLabelTheme`, `EditableLabelOptions`
  (overrides, with `with_theme`), `EditableLabelResolved`,
  `EditableLabelParts`.
- Added: `ControlTheme::editable_label: EditableLabelTheme`.

## 0.10.3 — 2026-10-28 (tag `v0.10.3`)

PopupMenu (docs/design/20261028_06_POPUP_MENU_PLAN.md). Additive: a new
module and types, a `Router` method, and a field appended to `ControlTheme`
(additive by the owner's ruling: a caller builds a theme with
`..ControlTheme::default()`).

- Added: `popup_menu`: `PopupMenu` (`new`, `with_state`, `with_options`,
  `resolved`, `layout`, `item_at`; `Modal`), `PopupMenuState` (`open`,
  `close`, `is_open`, `take_events`, `anchor`, `items`, `highlighted`,
  `layout`), `PopupItem` (`action`, `heading`, `separator`, `with_enabled`,
  `with_checked`, `is_choosable`), `PopupItemKind`, `PopupMenuEvent`,
  `PopupMenuLayout`, `PopupMenuTheme`, `PopupMenuOptions` (overrides and
  `dismiss`, with `with_theme`), `PopupMenuResolved`.
- Added: `Router::attach_popup_menu`.
- Added: `ControlTheme::popup_menu: PopupMenuTheme`.

## 0.10.4 — 2026-10-28 (tag `v0.10.4`)

Scrollbar and bookmarks (docs/design/20261028_08_SCROLLBAR_PLAN.md, steps
1-5). Additive: new modules and types, new methods, and a field appended
to `ControlTheme` (additive by the owner's ruling: a caller builds a theme
with `..ControlTheme::default()`); panes that do not opt in behave as
before. Sections drawn by a pane wait for a later release.

- Added: `scrollbar`: `Scrollbar` (`new`, `with_state`, `state`,
  `set_track`, `track`, `resolved`, `thumb_rect`, `band_rect`,
  `bookmark_layout`), `ScrollbarState` (`new`, `with_sections`,
  `set_sections`, `set_metrics`, `set_scrolling`, `advance`,
  `take_events`, `is_held`, `alpha`, `bookmarks_alpha`, `press`,
  `drag_to`, `release`, `cancel`), `ScrollbarVisibility` (`None`,
  `OnScroll`, the default, `Always`), `ScrollbarEvent` (`Grabbed`,
  `Scrub`, `Released`), `BookmarkPill`.
- Added: `ScrollbarTheme`, `ScrollbarOptions` (with setters, `visibility`,
  `bookmarks`, `with_theme`), `ScrollbarResolved`;
  `ControlTheme::scrollbar`.
- Added: `sections`: `SectionId`, `Section` (`new`, `estimated`),
  `BookmarkText` (`Text`, `Markdown`), `SectionedContent` (`new`,
  `set_sections`, `push`, `insert`, `remove`, `set_height`, `len`,
  `is_empty`, `sections`, `total_height`, `offset_of`, `section_at`,
  `add_bookmark`, `remove_bookmark`, `clear_bookmarks`, `bookmarks`).
- Added: `ScrollPane::with_scrollbar`, `ScrollPane::scrollbar`;
  `ScrollState::content_length`.

## 0.10.5 — 2026-10-28 (tag `v0.10.5`)

Markdown (docs/design/20261028_09_MARKDOWN_VIEW_PLAN.md). Additive: two
new features, none on by default, a new optional dependency, a new module,
new theme types and `ControlTheme::markdown` appended (additive by the
owner's ruling: a caller builds a theme with `..ControlTheme::default()`).
A build without the features is 0.10.4's apart from the theme types.

- Added: features `markdown` and `cmark` (pulldown-cmark 0.13, default
  features off, optional).
- Added (`markdown`): `markdown`: `parse`, `parse_inline`, `lay_out`,
  `MarkdownMeasure` (`new`, `px`, `width`), `MdStyle`, `MdRun` (`new`),
  `MdBlock`, `MdDocument`, `MdLine`, `MdLaid`, `MarkdownFaces`
  (`metrics`), `Face`, `CodeFace`, `MarkdownView` (`new`, `with_faces`,
  `set_text`, `set_width`, `height`, `rect`, `px`, `source`, `text`,
  `links`, `move_to`, `center`, `link_at`, `link_rect`; `DrawControl`,
  `HostedControl`); `ScrollbarState::set_faces`.
- Added (`markdown`): a `BookmarkText::Markdown` bookmark is drawn as
  inline spans, its markers gone (the Scrollbar plan's decision 6);
  without the feature it is drawn as written, as in 0.10.4.
- Added (`cmark`): `score_markdown`, `is_markdown`, `MarkdownScore`,
  `DEFAULT_MARKDOWN_THRESHOLD_PCT`.
- Added: `MarkdownTheme`, `MarkdownOptions` (a setter per value,
  `centered`, `with_theme`), `MarkdownResolved` (`font_size_in`,
  `text_color_in`, `link_color_in`, `dim_color_in`, `code_color_in`);
  `ControlTheme::markdown`.

## 0.10.6 — 2026-10-28 (tag `v0.10.6`)

PopupMenu toggle items and columns
(docs/design/20261028_10_POPUP_MENU_TOGGLES_AND_COLUMNS_PLAN.md).
Additive: variants on `#[non_exhaustive]` enums, new types and methods,
fields appended to `#[non_exhaustive]` structs; `PopupItem` is
unchanged, and menus without toggles or cells behave as before.

- Added: `PopupItemKind::Toggle`, `PopupItem::toggle`,
  `PopupMenuEvent::Toggled { id, checked }`; `PopupItem::is_choosable`
  is true for an enabled toggle.
- Added: `PopupRow` (`new`, `with_cells`, `From<PopupItem>`),
  `PopupItem::with_cells`, `PopupCell` (`new`, `align`, `width`,
  `fixed`, `flex`, `dim`, `scale`), `PopupCellWidth` (`Auto`, the
  default, `Fixed`, `Flex`).
- Added: `PopupMenuState::open_rows`, `PopupMenuState::rows`.
- Added: `PopupMenuLayout::columns`.
- Added: `PopupMenuTheme`, `PopupMenuOptions` (with setters) and
  `PopupMenuResolved`: `column_gap`, `dim_text_color`.
