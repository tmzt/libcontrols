---
title: EditableLabel, a label edited in place
date: 2026-10-28
status: Draft
covers: >
  A new control, `EditableLabel`: a one-line label with a pencil icon
  button at its right; the pencil (and, when enabled, the label) switches
  it to editing, where the text is a single-line `Edit` field and the
  pencil is replaced by a check (confirm) and an optional X (clear). Enter
  confirms, Escape and Back cancel. Its shared `EditableLabelState` holds
  the committed text, the draft and an event queue (`Started`,
  `Confirmed(String)`, `Cancelled`, `Cleared`), and gives the app what it
  needs to drive a soft keyboard. Icons drawn from line primitives. Theme
  per the spec's section 8.5: `ControlTheme::editable_label:
  EditableLabelTheme` defaults, `EditableLabelOptions` overrides,
  `with_theme`. Additive: 0.10.2. Steps and tests.
sources:
  - >
    The owner (2026-10-28), verbatim: "nickname should use an EditableLabel
    if that doesn't exist, start a subagent to add to libcontrols, it's has
    a label with pencil icon which switches to check and optional X
    (clear)".
  - >
    The lead's brief for this plan (2026-10-28): first user is the flankr
    companion's remote-details modal (a remote's nickname, shown as a
    label, edited in place, cleared back to a default); Enter confirms,
    Escape cancels; X only when enabled (`with_clear(true)`) and the field
    has text; placeholder for empty; a read-only mode; theme per 8.5;
    additive, 0.10.2.
  - >
    libcontrols at 6f42272 (0.10.1): src/label.rs (`Label`, the label
    look: left pad 4 px, text centred on `LINE_BOX_RATIO`), src/edit.rs
    (`Edit` and `EditState`: cursor, UTF-8 moves, `char_index_at_x` from
    the layout cached in the state at draw, `claims_drag`, `bg_color`
    `[0.10, 0.13, 0.18, 0.85]` and border `[0.15, 0.20, 0.28, 0.60]` set
    in `Edit::new`), src/router.rs (`deliver_keyboard` 972-989: the router
    takes Escape and clears the focus, so the focused control is told
    `FocusEvent::Lost`; `deliver_mouse` 874-887: a Down or Click on a
    `FOCUSABLE` node focuses it, one on nothing clears the focus),
    src/drawer/mod.rs (`DrawerTheme`, `DrawerOptions`, `setter!`,
    `with_theme`, `DrawerResolved`: the 8.5 pattern), src/symbol.rs
    (interned names only; libcontrols has no icon atlas), libmsdf
    `DrawList::push_line` (a segment between two absolute points).
  - >
    flankr companion (apps/companion/src/lib.rs `sync_keyboard` 1128-1172,
    `on_text_input` 3106-3125): the app, not libcontrols, starts and stops
    the soft keyboard (`MorphContext::start_text_input`) and receives the
    whole text back (`TextInputEvent::Changed(text)`, `Action`,
    `Dismissed`). libcontrols has no keyboard or IME hook; `Edit` only
    takes `KeyboardEvent`s.
related:
  - docs/design/20261028_02_LIBCONTROLS_SPEC.md (the living spec; section
    4 the router and focus, 6.1 Label, 6.10 Edit, 8.5 the theming
    standard; amended when this is implemented)
  - docs/design/20261025_01_EDIT_CURSOR_AND_INSERTION_POINT_PLAN.md (the
    `Edit` cursor this control's field reuses)
  - docs/design/20261028_01_DRAWER_PLAN.md (the first 8.5 control; its
    11:28 EDT amendment records that a `ControlTheme` field is additive)
---

# EditableLabel, a label edited in place

## 1. What it is

A label that the user can edit in place: it reads as text with a small
pencil at its right. Tapping the pencil (or, when enabled, the text) turns
the text into a one-line edit field holding the same text, and the pencil
into a check; when clearing is enabled and the field has text, an X sits
left of the check. Check or Enter keeps the edit; Escape, Back, or the
focus leaving throws it away; X empties the field and leaves it editing.
The app reads what happened from the state's event queue and decides what
an empty value means (the companion's nickname: "use the default name").

It is one router node. It composes the existing `Edit` for the field, so
cursor, UTF-8 moves, typing and click-to-place behave exactly as `Edit`.

## 2. Decisions

1. **One node, two modes.** `Display` (label + pencil) and `Editing`
   (field + optional X + check), switched by the shared state, not by
   adding and removing nodes. The node is `FOCUSABLE` (a Tab stop; Enter
   or Space in display mode starts editing), except read-only, which is
   `NodeFlags::NONE` like `Label`.
2. **Layout, right to left.** The trailing icon button is `button_width`
   wide and the rect's full height, at the rect's right edge. In editing,
   the X button (same size) sits immediately left of the check when shown.
   The text or field takes what is left, minus `gap` before the buttons.
   Read-only: no button; the text takes the whole rect.
3. **State.** `EditableLabelState` (shared `Arc`) holds the committed
   `text`, an `Arc<EditState>` used as the draft while editing, an
   `editing` flag and an event queue. The draft is copied from the text
   when editing starts (cursor at the end) and only written back on
   confirm, so cancel needs no saved copy.
4. **Events.** `EditableLabelEvent` (`#[non_exhaustive]`): `Started`,
   `Confirmed(String)` (the new text), `Cancelled`, `Cleared`. Queued by
   the state, drained with `take_events()`, like `ModalState` and
   `DrawerState`. The control also returns `ControlAction::Activated {
   part }` from the input that caused it (`part` the pencil, check, clear
   or field part, below), so `EventResponse::action` shows it was used.
5. **Cancel on Escape, Back and focus loss.** The router takes Escape
   itself and clears the focus (`src/router.rs:983-985`), so the control
   sees Escape as `FocusEvent::Lost`; a press on another node or on
   nothing also moves the focus away. All of these cancel: losing the
   focus while editing reverts. `Key::Escape` and `Key::Back` reaching
   the control directly (through a filter's `Redirect`, or Back, which
   the router passes on) cancel too. A press on the check or X is on the
   same node, so the focus stays and they work.
6. **Clear keeps editing.** X empties the draft (cursor 0), queues
   `Cleared` and stays in editing, so the user may type a new value or
   confirm the empty one; confirming then queues `Confirmed("")`. The X
   is shown, and hit, only when clearing is enabled and the draft is not
   empty.
7. **Soft keyboard: the app's.** libcontrols has no IME hook (`Edit` only
   takes `KeyboardEvent`s; the companion starts and stops the keyboard
   itself and gets the whole text back). The state gives the app what
   that needs: `is_editing()` and `draft()` to know when to start the
   keyboard and with what, `set_draft(text)` for each
   `TextInputEvent::Changed` (cursor to the end), `confirm()` for the
   keyboard's Done, `cancel()` for `Dismissed` if the app wants that. No
   new router or trait API.
8. **Programmatic editing.** `begin_edit`, `confirm`, `cancel`, `clear`
   on the state do what the taps do and queue the same events. `begin_edit`
   from code does not move the router's focus; the app calls
   `Router::set_focus(id)` too so keys reach the field.
9. **Icons from primitives.** libcontrols has no icon atlas (`Symbol` is
   interned names). Each icon is `push_line` segments in a square of
   `icon_size` centred in its button, stroked at `icon_thickness`:
   pencil 6 segments (two body sides, the top cap, a ferrule, two tip
   sides meeting at the point, bottom-left); check 2; X 2.
10. **Theme per section 8.5.** `ControlTheme::editable_label:
    EditableLabelTheme` (appended field) holds the defaults; one
    instance's `EditableLabelOptions` overrides them; `with_theme(&self,
    &EditableLabelTheme) -> EditableLabelResolved` resolves; the control
    resolves at draw against `ctx.theme.editable_label`. Colours that
    default to a flat `ControlTheme` colour are `Option` in the theme,
    `None` meaning that flat colour, read at draw (as `DrawerTheme`'s
    `sheet_color` is).
11. **Hit testing without a theme.** `hit_test` has no `DrawContext`. The
    control keeps the resolved values of its last draw in a `Cell` and
    lays out from them; before the first draw it resolves against
    `EditableLabelTheme::default()`.
12. **Press on release.** The pencil, check and X act on `Click` (the
    release), as `Button` does, so a scroll never presses them. The field
    takes `Down`, `Click` and `Drag` as `Edit` does, and claims the drag.
    The text in display mode starts editing on `Click` only when
    `label_edits` is on (default off: the owner's "optionally").
13. **Disabled** is the router's `NodeFlags::DISABLED` (half alpha, no
    input), as for every control. **Read-only** (`read_only(true)`) is
    the control's: it draws the text (or placeholder) only, no pencil,
    and never edits; `begin_edit` on its state still works for an app
    that wants it, but the control never calls it.
14. **Version.** All additive: a new module and types, a field appended
    to `ControlTheme` (additive by the owner's ruling, a caller builds a
    theme with `..ControlTheme::default()`), no change to an existing
    signature or behaviour. 0.10.2.

## 3. Interfaces

Module `src/editable_label.rs`, exported from the crate root.

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum EditableLabelEvent { Started, Confirmed(String), Cancelled, Cleared }

#[derive(Debug)]
pub struct EditableLabelState { /* text: Mutex<String>, draft: Arc<EditState>,
                                   editing: AtomicBool, events: Mutex<Vec<_>> */ }
impl EditableLabelState {
    pub fn new(text: impl Into<String>) -> Self;
    pub fn text(&self) -> String;                 // the committed value
    pub fn set_text(&self, text: impl Into<String>); // no event; draft untouched
    pub fn is_editing(&self) -> bool;
    pub fn draft(&self) -> Option<String>;        // Some while editing
    pub fn draft_state(&self) -> &Arc<EditState>; // the field's EditState
    pub fn set_draft(&self, text: impl Into<String>); // cursor to end; ignored unless editing
    pub fn begin_edit(&self) -> bool;             // false if already editing; queues Started
    pub fn confirm(&self) -> Option<String>;      // commits the draft; queues Confirmed
    pub fn cancel(&self) -> bool;                 // queues Cancelled
    pub fn clear(&self) -> bool;                  // empties the draft; queues Cleared
    pub fn take_events(&self) -> Vec<EditableLabelEvent>;
}

#[derive(Copy, Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct EditableLabelTheme {
    pub button_width: f32,        // 32.0, each icon button's width (full rect height)
    pub icon_size: f32,           // 14.0, the icon's square
    pub icon_thickness: f32,      // 1.6
    pub gap: f32,                 // 4.0, between the text/field and the buttons
    pub pencil_color: Option<[f32; 4]>,      // None: ControlTheme::muted_text_color
    pub confirm_color: Option<[f32; 4]>,     // None: ControlTheme::accent_color
    pub clear_color: Option<[f32; 4]>,       // None: ControlTheme::muted_text_color
    pub placeholder_color: Option<[f32; 4]>, // None: ControlTheme::muted_text_color
    pub field_bg: [f32; 4],       // [0.10, 0.13, 0.18, 0.85], Edit's
    pub field_border: [f32; 4],   // [0.15, 0.20, 0.28, 0.60], Edit's
    pub field_border_thickness: f32, // 1.0
}

#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct EditableLabelOptions {
    // overrides, one setter each (`.button_width(..)` …), None = the theme's:
    pub button_width, icon_size, icon_thickness, gap: Option<f32>,
    pub pencil_color, confirm_color, clear_color, placeholder_color,
        field_bg, field_border: Option<[f32; 4]>,
    pub field_border_thickness: Option<f32>,
    // flat ControlTheme values overridden per instance:
    pub font_size: Option<f32>, pub text_color: Option<[f32; 4]>,
    // per-instance settings, no theme default:
    pub placeholder: Option<String>, pub clear: bool,
    pub read_only: bool, pub label_edits: bool,
}
impl EditableLabelOptions {
    pub fn new() -> Self;
    pub fn with_theme(&self, theme: &EditableLabelTheme) -> EditableLabelResolved;
}

#[derive(Copy, Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct EditableLabelResolved { /* the theme's fields, plain (colours that
    fall back to a flat ControlTheme colour stay Option) */ }

#[derive(Clone, Debug)]
pub struct EditableLabel { pub state: Arc<EditableLabelState>, pub rect: Rect,
                           pub options: EditableLabelOptions, /* last: Cell<Resolved> */ }
impl EditableLabel {
    pub const PENCIL: ControlPart = ControlPart::Custom(0x454C_0001);
    pub const CONFIRM: ControlPart = ControlPart::Custom(0x454C_0002);
    pub const CLEAR: ControlPart = ControlPart::Custom(0x454C_0003);
    // the field is ControlPart::NumericBox (Edit's), display text ControlPart::Label
    pub fn new(text: impl Into<String>, rect: Rect) -> Self;
    pub fn with_state(self, Arc<EditableLabelState>) -> Self;
    pub fn with_options(self, EditableLabelOptions) -> Self;
    pub fn with_clear(self, bool) -> Self;
    pub fn with_placeholder(self, impl Into<String>) -> Self;
    pub fn with_read_only(self, bool) -> Self;
    pub fn with_label_edits(self, bool) -> Self;
    pub fn with_font_size(self, f32) -> Self;
    pub fn with_text_color(self, [f32; 4]) -> Self;
    pub fn text(&self) -> String; pub fn is_editing(&self) -> bool;
    pub fn resolved(&self, theme: &ControlTheme) -> EditableLabelResolved;
    pub fn parts(&self, resolved: &EditableLabelResolved) -> EditableLabelParts;
    pub fn handle(&self) -> ControlHandle<'_, Self>;
}
#[derive(Copy, Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct EditableLabelParts { pub text: Rect, pub button: Option<Rect>, pub clear: Option<Rect> }
// DrawControl, HostedControl, ReadValue<String> (the committed text).
```

`ControlTheme` gains `pub editable_label: EditableLabelTheme` (appended),
re-exported from `theme`.

## 4. Steps

1. **State and events.** `EditableLabelState` and `EditableLabelEvent` as
   in section 3.
   **Done when:** unit tests show `begin_edit` copies the text into the
   draft with the cursor at the end and queues `Started`; `confirm`
   commits and queues `Confirmed(text)`; `cancel` keeps the old text and
   queues `Cancelled`; `clear` empties the draft and queues `Cleared`;
   each is a no-op returning false/None outside editing.
2. **Theme, options, resolution.** `EditableLabelTheme` with the defaults
   of section 3, `EditableLabelOptions` with a setter per override,
   `with_theme`, `EditableLabelResolved`; `ControlTheme::editable_label`.
   **Done when:** a test resolves an options with one override against a
   theme with another changed value and gets both.
3. **The control: layout, draw, hit test.** `EditableLabel` with `parts`,
   `draw` (display: text or placeholder, pencil; editing: the field as an
   `Edit` over the draft state, placeholder over an empty field, optional
   X, check; a focus ring in display mode when focused), `hit_test`
   (parts from the last draw's resolved values).
   **Done when:** router tests show display mode draws 6 line segments
   (pencil) and no field; editing draws the field box, the cursor, 2
   segments for the check and 2 more for the X only when enabled and the
   draft is non-empty; the placeholder is drawn in the placeholder colour
   when empty (with a stub atlas-free context, the estimated text path);
   read-only draws no segments and is not focusable.
4. **Input.** Click on pencil starts editing; on the label only with
   `label_edits`; check confirms, X clears; the field takes mouse and keys
   as `Edit`; Enter confirms; Escape/Back keys cancel; focus loss cancels;
   Enter/Space in display mode starts editing; `claims_drag` for the field.
   **Done when:** router tests: pencil hit -> editing, typed chars reach
   the draft, check -> `Confirmed("…")` and the label shows it; router
   Escape -> `Cancelled` and the old text; X clears only when enabled and
   non-empty; a click on the label without `label_edits` does nothing.
5. **Spec, README, version.** Spec amendment for sections 6 (6.15) and 8;
   README controls list; `Cargo.toml` 0.10.2, `VERSIONS.md` entry, tag.
   **Done when:** `cargo fmt --check`, `cargo clippy --all-targets` with
   no warnings and `cargo test` pass; the lead commits and tags `v0.10.2`.

## 5. Open questions

1. Should losing the focus by tapping elsewhere **confirm** rather than
   cancel? This plan cancels (decision 5), since the router reports
   Escape and a tap elsewhere the same way; an option
   (`commit_on_blur`) could be added later, additively.
2. Should X confirm the empty value at once (one tap to "use the
   default") instead of leaving the field empty for typing (decision 6)?
3. Should Back cancel inside the control, or be left to the app's own
   Back handling (the companion handles Back before routing)?
4. Long text is not truncated or clipped (as `Label` today); an ellipsis
   could come with a later Label plan.
