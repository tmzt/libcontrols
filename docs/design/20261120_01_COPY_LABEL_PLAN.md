---
title: CopyLabel, a label with a copy glyph
date: 2026-11-20
status: Implemented
covers: "A new control, CopyLabel: a one-line label, cut with an ellipsis when too long, with the usual copy glyph (two overlapping squares drawn from line segments) at its right end. A click or tap on the glyph, or Enter or Space while it is focused, sets a one-shot flag on its shared CopyLabelState that the app takes with take_copy and answers with its own platform clipboard; libcontrols has no clipboard. Sized and coloured from EditableLabel's display mode and theme values; no new theme type. Additive: 0.12.1."
sources:
  - "The owner (2026-11-20), in ShotToss's session, for its image details dialog: the filename shown \"as copyable label (label with copy glyph)\"; the request is the owner's approval of this plan and its build."
  - "The lead's brief for this plan (2026-11-20): src/copy_label.rs exporting CopyLabel and CopyLabelState; EditableLabel's display mode as the model (padding, the right-hand button, button_width, icon_size, icon_thickness, the pencil's colour resolution); the glyph from push_line segments; text cut with an ellipsis, alignment and font size settable; take_copy an AtomicBool like ButtonState's click; CopyLabel::COPY a ControlPart::Custom; a click on the text does nothing; addable to a Router and a ModalDialog's content; tests for the glyph click, the text click and the draw list; additive, 0.12.1."
  - "libcontrols at bb58be1 (0.12.0): src/editable_label.rs (PAD_H 4, parts, draw_pencil, the PENCIL part 0x454C_0001, the focus ring), src/label.rs (alignment, LINE_BOX_RATIO centring), src/button.rs (ButtonState::click and take_click on an AtomicBool), src/scrollbar/bookmarks.rs (cut_to_fit, the ellipsis cut PopupMenu's columns reuse), src/control.rs (DrawControl + HostedControl is all a Router or a ModalDialog's content needs)."
related:
  - "docs/design/20261028_02_LIBCONTROLS_SPEC.md (the living spec; section 6 gains 6.21, sections 10 and 11 are extended; amended when this is implemented)"
  - "docs/design/20261028_05_EDITABLE_LABEL_PLAN.md (the display mode and theme values this control reuses)"
  - "ShotToss docs/design/20261120_02_DETECTION_CORRECTIONS_PLAN.md (the image view's details dialog, the first user)"
depends_on: []
living_sections: []
---

# CopyLabel, a label with a copy glyph

## 1. What it is

ShotToss's image details dialog shows an image's filename. The owner wants
it copyable: a label with a copy glyph. `CopyLabel` is that control: one
line of text and, at its right end, the usual copy icon. Pressing the icon
asks for the text to be copied. libcontrols has no clipboard and does not
gain one: the control only reports the request, and the app, which owns
the platform (Android's `ClipboardManager` through libmorph, a desktop
clipboard), copies the text.

## 2. Decisions

1. **One node, one state.** `CopyLabel` is a single router node over a
   shared `Arc<CopyLabelState>` holding the text and a copy flag. It
   implements `DrawControl` and `HostedControl`, so it can be added to any
   `Router`, including a `ModalDialog`'s content router.
2. **EditableLabel's display mode is the model.** The glyph sits in a
   button of `button_width` at the right, the full height of the control;
   the text takes the rest, less `gap`, with `Label`'s 4 px horizontal
   padding and centred vertically on `LINE_BOX_RATIO`. The values come
   from `ControlTheme::editable_label` (`button_width` 32, `icon_size` 14,
   `icon_thickness` 1.6, `gap` 4); the glyph's colour is
   `editable_label.pencil_color`, else the theme's `muted_text_color`, as
   the pencil's is. Both icons then look alike in one dialog. No
   `CopyLabelTheme` is added: the control's only own overrides are the
   font size, the alignment, the text colour and the glyph colour, set on
   the builder.
3. **The glyph.** Two overlapping squares of side `0.52 * icon_size`: the
   back one offset up and right, the front one down and left, by
   `0.24 * icon_size`. The front square is four `push_line` segments; the
   back one is only the part the front one does not cover, four more (its
   top and right edges, its left edge down to the front square's top, its
   bottom edge from the front square's right). Eight segments in all,
   centred on the button.
4. **The text** is the state's text cut to fit the text area less its
   padding with `cut_to_fit` (the ellipsis cut the scrollbar's pills and
   `PopupMenu`'s columns use): its first line only, ending in "…" when
   cut. It is placed Left (default), Center or Right in the text area.
5. **Copy requests.** A left `Click` on the glyph, or `Enter`/`Space`
   down while the node is focused, sets the state's flag and returns
   `Activated { part: CopyLabel::COPY }`. `take_copy` returns the flag and
   clears it (an `AtomicBool` swap, like `ButtonState::take_click`), so a
   request is seen once. A click on the text, or anywhere else in the
   rect, does nothing and returns `None`.
6. **Parts.** `CopyLabel::COPY` is `ControlPart::Custom(0x434C_0001)`
   ("CL"); `CopyLabel::TEXT` is `ControlPart::Label`; the gap is `Body`.
7. **Focus.** `FOCUSABLE` by default, so keyboard users can copy; the
   focused control draws the glowing ring, as `EditableLabel` does.
8. **Versions.** A new module and new types only: additive, 0.12.1.

## 3. Interfaces

```rust
// src/copy_label.rs; lib.rs: pub mod copy_label; pub use copy_label::{CopyLabel, CopyLabelState};

pub struct CopyLabelState { /* text: Mutex<String>, copy: AtomicBool */ }
impl CopyLabelState {
    pub fn new(text: impl Into<String>) -> Self;
    pub fn text(&self) -> String;
    pub fn set_text(&self, text: impl Into<String>);
    /// True once after a copy was asked for; clears the request.
    pub fn take_copy(&self) -> bool;
}

#[derive(Clone, Debug)]
pub struct CopyLabel {
    pub state: Arc<CopyLabelState>,
    pub rect: Rect,
    pub align: TextAlign,                 // Left
    pub font_size: Option<f32>,           // None: theme default_font_size
    pub text_color: Option<[f32; 4]>,     // None: theme text_color
    pub glyph_color: Option<[f32; 4]>,    // None: editable_label.pencil_color, else muted_text_color
}
impl CopyLabel {
    pub const COPY: ControlPart = ControlPart::Custom(0x434C_0001);
    pub const TEXT: ControlPart = ControlPart::Label;
    pub fn new(text: impl Into<String>, rect: Rect) -> Self;
    pub fn with_state(self, state: Arc<CopyLabelState>) -> Self;
    pub fn with_font_size(self, size: f32) -> Self;
    pub fn with_align(self, align: TextAlign) -> Self;
    pub fn with_text_color(self, color: [f32; 4]) -> Self;
    pub fn with_glyph_color(self, color: [f32; 4]) -> Self;
    pub fn text(&self) -> String;
    /// The text as drawn with `ctx`: its first line, cut with "…" to fit.
    pub fn shown_text(&self, ctx: &DrawContext) -> String;
}
impl DrawControl for CopyLabel;
impl HostedControl for CopyLabel;
impl ReadValue<String> for CopyLabel;
```

The control keeps the `editable_label` values of its last draw in a
`Cell`, as `EditableLabel` does, so `hit_test` places the glyph where it
was drawn.

## 4. Steps

1. **The control.** `src/copy_label.rs` with the state, the builder, the
   layout, the glyph, the text cut and the input of section 2; exported
   from `lib.rs`.
   **Done when:** `cargo fmt -p libcontrols -- --check`, `cargo clippy -p
   libcontrols --all-targets` (no new warnings) and `cargo test -p
   libcontrols` pass.
2. **Tests,** `tests/copy_label.rs` through a `Router`: a click on the
   glyph makes `take_copy` true once, then false; a click on the text
   leaves it false; Enter on the focused label sets it; a draw does not
   panic and its list holds the glyph's eight line segments inside the
   button, in the muted colour or the set glyph colour; a long text is cut
   with "…" and a short one is not; the label is added to a
   `ModalDialog`'s content and a click on its glyph there sets the flag.
   **Done when:** those tests pass.
3. **Spec, README, version.** An amendment to the spec adding section
   6.21 and extending 10 and 11; this plan's README row; `version`
   0.12.1 with its `VERSIONS.md` entry; tag `v0.12.1`.
   **Done when:** the tag names the release commit and `tpt check` reports
   no error in this plan.

## 5. Open questions

None. A themed `CopyLabelTheme`, a "copied" confirmation state and a
copyable multi-line text are left for a later plan if an app asks.

## Amendment — 2026-11-20 12:11 EST: approved; implementation started, the document is now immutable

The owner's request for ShotToss's details dialog is the approval of this
plan. Implementation starts with this amendment, on `main` from `51b8fd3`:
`status` is `In implementation`, and the body above is frozen (AGENTS.md
Rule 1.5). The scope is every step, 1-3, released as **0.12.1**.
Departures, if any, are recorded in `CHANGES.md`.

## Amendment — 2026-11-20 12:11 EST: implemented

Every step is done and its **Done when:** holds, in ecc5a87 (steps 1-3:
`src/copy_label.rs` and its exports, `tests/copy_label.rs` with 8 tests,
the spec's amendment, the README row and 0.12.1, tag `v0.12.1`), after
the plan (ae9dded) and its CHANGES.md addendum (51b8fd3). No departures.
Status `Implemented`.
