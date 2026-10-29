---
title: PopupMenu, a call-driven popup menu of actions
date: 2026-10-28
status: In implementation
covers: >
  A new control, `PopupMenu`: a small menu of actions anchored to a rect
  (an app's ⋮ button), opened and closed by calls on its shared
  `PopupMenuState` (no gestures of its own, like the `Drawer`), shown as
  a layer of the router's modal stack with no backdrop, so it takes input
  above the screen and an outside tap, Escape or Back dismisses it. Items
  are actions, headings and separators, each with an id, a label, an
  enabled flag and an optional check mark. It opens below its anchor,
  right-aligned to it, and flips above or shifts to stay inside its
  viewport. Pointer and keyboard (Up/Down/Home/End, Enter/Space) input;
  the outcome is queued as `Chosen(id)` or `Dismissed`. Theme per the
  spec's section 8.5: `ControlTheme::popup_menu: PopupMenuTheme`
  defaults, `PopupMenuOptions` overrides, `with_theme`. Additive: 0.10.3.
  Steps and tests.
sources:
  - >
    The owner (2026-10-28, through flankr's lead, verbatim), in flankr's
    plugin plan: "the plugin will contribute a menu item (but we'll need a
    popup menu with actions, maybe in the title bar?)".
  - >
    flankr's plugin plan,
    ~/src/flankr/docs/design/20261028_04_PLUGINS_PLAN.md (on the branch
    main-plugins), read-only: section 3.6 (the ⋮ popup menu; item 3, the
    interface asked of libcontrols), step 4 (built and released here as
    0.10.3 under this repository's rules, "or an agreed variant, recorded
    in CHANGES.md" there) and its 19:47 EDT amendment (approved by the
    owner: "approve and impl in worktree, new branch (main-plugins)").
  - >
    The lead's brief for this plan (2026-10-28): a ⋮ in the title bar on
    every screen opening a popup menu of actions, built as a small
    call-driven control in libcontrols (open and close by call, items with
    a label and an enabled flag, the chosen item returned; no gestures of
    its own), anchored to a rect, opening below or above it to fit,
    keyboard and pointer, a tap outside closes it (as a layer of the
    modal stack), events `Chosen(id)` and `Dismissed`, a shared
    `PopupMenuState`, theme per 8.5; additive, 0.10.3.
  - >
    libcontrols at 0bfd5db (0.10.2): src/modal.rs (`ModalState`, its
    `push_with`, `close`, `contains` and the `Instant` transition that
    opens and closes at once; `ModalStack::sync`, which makes the layer of
    a live entry from a queued modal; `Modal::options` and `filter_layer`;
    `ModalLayerFilter<S: LayerShape>`, which passes events inside its
    shape, consumes them outside and requests `OutsideTap`, `Escape` and
    `Back` dismissals by `DismissOn`; `ModalOptions::with_backdrop`),
    src/router.rs (`run_layers`: while a layer exists the top layer's
    filter and modal see every event before the app's nodes; keys reach
    the top modal before the router's own Tab and Escape;
    `press_belongs_elsewhere`: the rest of a press that began under
    another owner is swallowed), src/editable_label.rs and
    src/drawer/mod.rs (the 8.5 pattern: `setter!`, `with_theme`, the
    resolved values kept for hit testing), src/button.rs (act on `Click`).
related:
  - docs/design/20261028_02_LIBCONTROLS_SPEC.md (the living spec; section
    4.6 the modal stack, 6 the controls, 8.5 the theming standard; amended
    when this is implemented)
  - docs/design/20261016_03_MODAL_STACK_PLAN.md (the modal stack this
    menu is a layer of)
  - docs/design/20261028_01_DRAWER_PLAN.md (the call-driven style; the
    first 8.5 control)
  - docs/design/20261028_05_EDITABLE_LABEL_PLAN.md (the line-segment
    check mark this menu reuses)
  - ~/src/flankr/docs/design/20261028_04_PLUGINS_PLAN.md (the first user:
    flankr's ⋮ menu, `apps/companion/src/menu.rs`)
---

# PopupMenu, a call-driven popup menu of actions

## 1. What it is

A small floating menu of actions. The app draws its own trigger (in
flankr, a ⋮ at the right of each title bar), and when it is tapped calls
`open(anchor, items)` on the menu's shared state with the trigger's rect
and the items for that screen. The menu appears just below the trigger,
right-aligned to it (above it when there is no room below), over
everything else. A tap on an enabled action closes it and queues
`Chosen(id)`; a tap anywhere outside it, Escape or Back closes it and
queues `Dismissed`. The app reads the queue each frame and acts.

It has no gestures of its own: nothing in the library opens it, only the
app's call. It is drawn and given input as a layer of the router's modal
stack, so it needs `ModalStack::install` once, as `ModalDialog` does.

## 2. Decisions

1. **The interface is flankr's** (its plugin plan, 3.6 item 3), a
   superset of the brief's `{ id, label, enabled, separator? }`: an item
   has `id: Symbol`, `label: String`, `enabled: bool`, `checked:
   Option<bool>` and `kind: PopupItemKind` (`Action`, `Heading`,
   `Separator`; `#[non_exhaustive]`). `PopupItem` is a plain struct with
   public fields, as asked, plus constructors `action(id, label)`,
   `heading(label)`, `separator()` and setters `with_enabled`,
   `with_checked`.
2. **Opened by a call on the state.** `PopupMenuState::open(&self,
   anchor, items)` takes `&self`, so the app can call it from an input
   handler without `&mut Router`. To make that work the menu is
   **attached** once to the router's modal stack:
   `Router::attach_popup_menu(&mut self, id, menu) -> Result<(),
   ModalError>` (`NoStack` without a stack) binds the state to the
   stack's `ModalState` under the entry id `id` and gives the stack the
   menu to make its layer from. `open` then pushes entry `id` with
   `ModalTransition::Instant`; the stack makes the layer (a clone of the
   attached menu, sharing its state) the next time it syncs, which is at
   the start of every routed event and every draw. Attaching the same id
   again replaces the attached menu (a new viewport or options) for the
   next open. This one setup call is the only addition to flankr's
   interface. A state that was never attached records `open` (and
   `is_open` is true) but nothing is shown.
3. **A layer with no backdrop.** `Modal::options` is
   `ModalOptions::default().with_backdrop(None).with_dismiss(dismiss)`,
   `dismiss` from the options (default: outside tap, Escape and Back all
   dismiss). Its `filter_layer` is the standard `ModalLayerFilter` over a
   shape that is the menu's frame as last laid out, so a tap outside is
   consumed (it does not reach the screen beneath, including the ⋮
   itself, so a second tap on ⋮ closes the menu) and requests
   `OutsideTap`; Escape and Back request `Escape` and `Back`. Since the
   layer is the top of the stack, keys reach the menu before the router's
   own Tab and Escape handling.
4. **Events and reconciliation.** `PopupMenuEvent` (`#[non_exhaustive]`):
   `Chosen(Symbol)` and `Dismissed`, queued by the state and drained with
   `take_events()`. A choice clears the open flag, queues `Chosen(id)` and
   closes the entry. `close()` (the app's) clears the flag and closes the
   entry and queues nothing. Any other end of the entry (the filter's
   outside tap, Escape or Back; the app's `ModalState::dismiss_all` or a
   `replace_top_modal` over it) is noticed by the state when
   `is_open()` or `take_events()` next runs: the flag is set but the
   entry is gone, so the flag is cleared and `Dismissed` queued. Every
   open therefore ends in exactly one of `Chosen`, `Dismissed` or the
   app's own `close()`. The `ModalState`'s own `Opened`, `Dismissed` and
   `Closed` events for the entry id are also queued there, as for any
   modal.
5. **Re-open replaces.** `open` while open replaces the anchor and items
   in place (the entry stays; no event), clears the highlight and lays
   out again.
6. **Placement.** With the resolved values (section 3) and `viewport` the
   menu's `rect`:
   - width: the widest label's advance (`DrawContext::text_advance`) plus
     `2 * item_padding`, plus `check_width` when any item has `checked:
     Some(_)`, clamped to `min_width..=max_width` and to `viewport.w - 2 *
     margin`;
   - height: `2 * padding` plus `item_height` for each action and heading
     and `separator_height` for each separator;
   - x: `anchor.right() - width` (right-aligned), then moved right to at
     least `viewport.x + margin` and left to at most `viewport.right() -
     margin - width`;
   - y: below, `anchor.bottom() + offset`, if it fits above
     `viewport.bottom() - margin`; else above, `anchor.y - offset -
     height`, if that is at least `viewport.y + margin`; else on the side
     with more room, clamped into the viewport (a menu taller than the
     viewport is not scrolled in this plan).
   - item rects: inset by `padding` from the frame on every side, stacked
     top to bottom.
   `PopupMenu::layout(&DrawContext) -> PopupMenuLayout { frame, items:
   Vec<Rect>, above: bool }` computes it; each draw stores it in the state
   (`PopupMenuState::layout()`), and hit tests and the layer's shape read
   that. Before the first draw they lay out with `DrawContext::default()`
   (the default theme and the 0.55 em text estimate).
7. **Input.** Pointer, in screen coordinates (the stack does not
   translate layers): `Move` and `Drag` highlight the enabled action under
   the pointer (none over anything else); `Down` on an enabled action
   records it as pressed and highlights it; `Click` on an enabled action
   chooses it when it is the pressed one or no press was recorded (a
   shell that sends only `Click`), so a press that slides from one item
   to another chooses nothing; `Up` does nothing (act on release, as
   `Button`). Headings, separators and disabled actions take no input.
   Keys, on press: `Down`/`Up` move the highlight to the next/previous
   enabled action, wrapping (from none, `Down` goes to the first and `Up`
   to the last); `Home`/`End` to the first/last; `Enter` and `Space`
   choose the highlighted action. The pointer leaving the menu
   (`set_hover(None)`) clears the highlight. A choice returns
   `ControlAction::Activated { part: ControlPart::Custom(index) }`, the
   item's index, as the routed response's action.
8. **Drawing**, in `draw` (the stack draws a layer's both passes): an
   optional shadow (a rounded box in `shadow`, moved down by
   `shadow_offset`; none when its alpha is 0), the background (rounded
   box, `bg`, `corner_radius`), the border (outline, `border`,
   `border_thickness`; none when 0), then each item: the highlight under
   a highlighted action (rounded box, `highlight`), the label at the
   item's left plus `item_padding` (plus `check_width` when the menu has
   checks), vertically centred, in `text_color` (headings in
   `heading_color`), alpha multiplied by `disabled_alpha` when disabled;
   a check mark (two `push_line` segments, as `EditableLabel`'s) centred
   in the check column for `checked: Some(true)`; a separator as a
   `separator_thickness` line across the item inset by `item_padding`, in
   `separator_color`.
9. **Theme per section 8.5.** `ControlTheme::popup_menu: PopupMenuTheme`
   (appended field) holds the defaults; one menu's `PopupMenuOptions`
   overrides them; `with_theme(&self, &PopupMenuTheme) ->
   PopupMenuResolved` resolves; the control resolves at draw against
   `ctx.theme.popup_menu`. Colours that default to a flat `ControlTheme`
   colour (`text_color`, `heading_color`, `check_color`) are `Option` in
   the theme, `None` meaning that flat colour, read at draw; so is
   `font_size`.
10. **Modal stack change, internal only.** `ModalStack` gains a private
    list of attached menus (entry id and the menu to clone), and `sync`
    makes the layer of a live entry with no layer from that list when no
    queued push has one. No public modal type changes.
11. **Version.** All additive: a new module and types, a field appended
    to `ControlTheme` (additive by the owner's ruling, spec 8.5 item 4), a
    new `Router` method, no change to an existing signature or behaviour.
    0.10.3.

## 3. Interfaces

Module `src/popup_menu.rs`, exported from the crate root.

```rust
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum PopupItemKind { Action, Heading, Separator }

#[derive(Clone, Debug, PartialEq)]
pub struct PopupItem {
    pub id: Symbol, pub label: String, pub enabled: bool,
    pub checked: Option<bool>, pub kind: PopupItemKind,
}
impl PopupItem {
    pub fn action(id: impl Into<Symbol>, label: impl Into<String>) -> Self; // enabled
    pub fn heading(label: impl Into<String>) -> Self;  // id ""
    pub fn separator() -> Self;                        // id "", label ""
    pub fn with_enabled(self, bool) -> Self;
    pub fn with_checked(self, Option<bool>) -> Self;
    pub fn is_choosable(&self) -> bool;                // an enabled Action
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum PopupMenuEvent { Chosen(Symbol), Dismissed }

#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct PopupMenuLayout { pub frame: Rect, pub items: Vec<Rect>, pub above: bool }

#[derive(Debug, Default)]
pub struct PopupMenuState { /* Mutex<inner>: open, anchor, items, highlight,
                               pressed, events, layout, binding */ }
impl PopupMenuState {
    pub fn new() -> Self;
    pub fn open(&self, anchor: Rect, items: Vec<PopupItem>);
    pub fn close(&self);
    pub fn is_open(&self) -> bool;
    pub fn take_events(&self) -> Vec<PopupMenuEvent>;
    pub fn anchor(&self) -> Rect;
    pub fn items(&self) -> Vec<PopupItem>;
    pub fn highlighted(&self) -> Option<usize>;
    pub fn layout(&self) -> Option<PopupMenuLayout>;   // as last drawn
}

#[derive(Copy, Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct PopupMenuTheme {
    pub bg: [f32; 4],                 // [0.08, 0.11, 0.16, 0.98]
    pub border: [f32; 4],             // [0.18, 0.65, 0.92, 0.35]
    pub border_thickness: f32,        // 1.0
    pub corner_radius: f32,           // 6.0
    pub item_height: f32,             // 40.0
    pub padding: f32,                 // 6.0, frame to items, every side
    pub item_padding: f32,            // 14.0, item edge to label
    pub min_width: f32,               // 160.0
    pub max_width: f32,               // 320.0
    pub separator_height: f32,        // 9.0
    pub separator_thickness: f32,     // 1.0
    pub separator_color: [f32; 4],    // [0.30, 0.36, 0.44, 0.60]
    pub text_color: Option<[f32; 4]>,    // None: ControlTheme::text_color
    pub heading_color: Option<[f32; 4]>, // None: ControlTheme::muted_text_color
    pub check_color: Option<[f32; 4]>,   // None: ControlTheme::accent_color
    pub font_size: Option<f32>,          // None: ControlTheme::default_font_size
    pub highlight: [f32; 4],          // [0.18, 0.65, 0.92, 0.20]
    pub check_width: f32,             // 24.0
    pub check_size: f32,              // 12.0
    pub check_thickness: f32,         // 1.6
    pub disabled_alpha: f32,          // 0.4
    pub shadow: [f32; 4],             // [0.0, 0.0, 0.0, 0.35]
    pub shadow_offset: f32,           // 3.0
    pub offset: f32,                  // 4.0, anchor to menu
    pub margin: f32,                  // 8.0, menu to viewport edge
}

#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct PopupMenuOptions {
    // one Option per theme field, one setter each (`.bg(..)` …), None = the theme's
    // per-instance: pub dismiss: DismissOn (default: all), setter `.dismiss(..)`
}
impl PopupMenuOptions {
    pub fn new() -> Self;
    pub fn with_theme(&self, theme: &PopupMenuTheme) -> PopupMenuResolved;
}

#[derive(Copy, Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct PopupMenuResolved { /* the theme's fields, plain (the Option
    colours and font size stay Option, read at draw) */ }

#[derive(Clone, Debug)]
pub struct PopupMenu { pub state: Arc<PopupMenuState>, pub rect: Rect,
                       pub options: PopupMenuOptions }
impl PopupMenu {
    pub fn new(viewport: Rect) -> Self;
    pub fn with_state(self, Arc<PopupMenuState>) -> Self;
    pub fn with_options(self, PopupMenuOptions) -> Self;
    pub fn resolved(&self, theme: &ControlTheme) -> PopupMenuResolved;
    pub fn layout(&self, ctx: &DrawContext) -> PopupMenuLayout;
    pub fn item_at(&self, pos: [f32; 2]) -> Option<usize>; // by the last layout
}
// DrawControl, HostedControl, Modal.

impl Router {
    pub fn attach_popup_menu(&mut self, id: impl Into<Symbol>, menu: PopupMenu)
        -> Result<(), ModalError>;
}
```

`ControlTheme` gains `pub popup_menu: PopupMenuTheme` (appended),
re-exported from `theme`.

## 4. Steps

1. **Items, state and events.** `PopupItem`, `PopupItemKind`,
   `PopupMenuEvent`, `PopupMenuState` (open, close, reconciliation,
   highlight moves, choose) as in sections 2 and 3.
   **Done when:** unit tests show `open` records the anchor and items and
   `is_open` is true; `close` queues nothing; a choice of a disabled item,
   a heading or a separator does nothing; highlight moves skip them and
   wrap.
2. **Theme, options, resolution.** `PopupMenuTheme` with the defaults of
   section 3, `PopupMenuOptions` with a setter per override, `with_theme`,
   `PopupMenuResolved`; `ControlTheme::popup_menu`.
   **Done when:** a test resolves options with one override against a
   theme with another changed value and gets both.
3. **Layout and draw.** `PopupMenu::layout` (decision 6) and `draw`
   (decision 8).
   **Done when:** tests show the menu below and right-aligned to an anchor
   near the top right, above an anchor near the bottom, shifted inside the
   viewport for an anchor at the left edge; a drawn open menu pushes its
   background in `bg` with the frame's rect, a highlight only under a
   highlighted action, 2 line segments for a checked item and a
   separator line.
4. **The layer.** `Modal` for `PopupMenu` (decision 3), the stack's
   attached menus (decision 10), `Router::attach_popup_menu`, `open` and
   `close` pushing and closing the entry.
   **Done when:** router tests show `open` by call puts the menu on the
   stack and its next render draws it with no backdrop; `close` removes
   it; `attach_popup_menu` without a stack is `NoStack`.
5. **Input.** Decision 7 through the router.
   **Done when:** router tests show a tap on an action queues
   `Chosen(id)` and closes the menu, a tap on a disabled action, a heading
   or a separator does nothing and keeps it open, keyboard Down/Down/Enter
   chooses the second enabled action (skipping a disabled one), a tap
   outside queues `Dismissed`, closes the menu and does not reach a button
   beneath it, Escape queues `Dismissed`, and a theme override (item
   height) moves the items.
6. **Spec, README, version.** Spec amendment for sections 4.6, 6 (6.16),
   8, 10 and 11; the README's controls list; `Cargo.toml` 0.10.3,
   `VERSIONS.md` entry, tag.
   **Done when:** `cargo fmt --check`, `cargo clippy --all-targets` with
   no warnings and `cargo test` pass; the lead commits and tags `v0.10.3`.

## 5. Open questions

1. A menu taller than its viewport is clamped, not scrolled; a
   `ScrollPane` inside it could come later.
2. Alignment is right-aligned to the anchor only (flankr's ⋮ is at the
   right); a left-aligned option (for a menu under a left trigger) can be
   added to `PopupMenuOptions`, additively.
3. Submenus, item icons and keyboard type-ahead are not covered.
4. Opening and closing are instant; a fade or scale could use the modal
   stack's `Timed` transitions later.

## Amendment — 2026-10-28 19:52 EDT: approved by the plugin plan

flankr's plugin plan (~/src/flankr/docs/design/20261028_04_PLUGINS_PLAN.md,
section 3.6 item 3 and step 4) asks libcontrols for this control with this
interface, and the owner approved that plan as written ("approve and impl
in worktree, new branch (main-plugins)", its 19:47 EDT amendment). This
plan is the libcontrols plan that step 4 names, and it is approved by that
approval. Its one addition to the requested interface,
`Router::attach_popup_menu` (decision 2), is recorded for flankr's
`CHANGES.md` as the agreed variant its step 4 allows. The status stays
`Draft` until the first implementing commit, which sets it to `In
implementation` and appends the amendment saying the document is now
immutable.

## Amendment — 2026-10-28 19:54 EDT: implementation started; the document is now immutable

Implementation starts with this amendment, in the worktree
`.claude/worktrees/popup-menu` on the branch `wt-popup-menu` from
`0bfd5db`: `status` is `In implementation`, and the body above is frozen
(AGENTS.md Rule 1.5). The open questions of section 5 stay open for the
owner; the code follows the decisions as written. Departures are recorded
in `CHANGES.md`.
