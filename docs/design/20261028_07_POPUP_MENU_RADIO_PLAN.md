---
title: A radio style for PopupMenu's mark column
date: 2026-10-28
status: Draft
covers: >
  A second style for the `PopupMenu`'s mark column, chosen per menu:
  `PopupMenuOptions::marks(PopupMarkStyle::Radio)` draws every item that
  has a check state as a radio mark instead of a check, a ring with a
  filled centre dot (like ⦿) for `checked: Some(true)` and a ring with a
  small centre dot (like ⊙) for `checked: Some(false)`, both drawn by the
  control from SDF primitives (an `Outline` ring and a `Circle` dot)
  because the menu's labels are in Roboto, which has neither glyph. The
  default style stays `Check`, so every existing menu draws as today.
  `PopupItem` does not change. New theme values for the ring and the two
  dots, colours defaulting to the menu's text colour (no accent). Layout,
  hit testing and input are unchanged. Additive: 0.10.4. Steps and tests.
sources:
  - >
    The owner (2026-10-28, through flankr's lead, verbatim), in flankr's
    subagents plan: "no color, it just echoes the agent selector in
    Claude, the empty dot is the menu icon then the dot is used as a radio
    selector in the menu"; then "yes" to the amendment's wording.
  - >
    flankr's subagents plan,
    ~/src/flankr/docs/design/20261023_07_SUBAGENTS_PLAN.md, read-only: its
    20:51 EDT amendment, item 2 ("Every item of the menu has a radio mark:
    ⦿ (filled) on the session being viewed, ⊙ (empty) on the others.
    There is no check mark.") and item 4 ("The menu's labels are in
    Roboto, which has neither dot, so the marks are drawn by the
    `PopupMenu` itself: a radio style for its mark column, a ring with a
    filled dot for the selected item, drawn as its check is now (an
    additive libcontrols release, 0.10.4, with its own plan in that
    repository)").
  - >
    The owner (2026-10-28), "yes" to writing this plan as a Draft. That is
    not an approval to build it; the status stays `Draft` until the owner
    approves it.
  - >
    libcontrols at 05e61c6 (0.10.3): src/popup_menu.rs (`PopupItem`, a
    plain struct with public fields and no `Default`; `PopupItemKind`,
    `#[non_exhaustive]`; `PopupMenuTheme`, `PopupMenuOptions` and
    `PopupMenuResolved`, all `#[non_exhaustive]`; the check column
    reserved by `checks = items.iter().any(|i| i.checked.is_some())`;
    `draw_check`, two `push_line` segments centred at `rect.x +
    item_padding / 2 + check_width / 2`; `check_color` `None` =
    `accent_color`), tests/popup_menu.rs (the fixture and the `lines`
    counter), src/slider.rs (`ThumbStyle::Circle`: a `SdfKind::Circle`
    with an `Outline` of radius `diameter / 2` over it, the only circle
    drawn in the library today), src/radio.rs (`Radio` is a strip of
    cells, not a ring and dot; nothing to reuse).
  - >
    libmsdf at 4de319c (the pinned rev): `SdfKind::Circle` fills the
    circle inscribed in `size` (radius `min(w, h) / 2`) and casts no
    shadow unless the instance is elevated; `SdfKind::Outline { radius,
    thickness }` strokes `abs(sd_rounded_box) - thickness / 2`, so the
    stroke is centred on the rect's edge, and with `radius = side / 2` on a
    square it is a ring.
related:
  - docs/design/20261028_06_POPUP_MENU_PLAN.md (the menu this extends; In
    implementation, released in 0.10.3; not changed by this plan)
  - docs/design/20261028_02_LIBCONTROLS_SPEC.md (the living spec; section
    6.16 the menu and 8.5 the theming standard; amended when this is
    implemented)
  - ~/src/flankr/docs/design/20261023_07_SUBAGENTS_PLAN.md (the first
    user: flankr's subagents dropdown, its 20:51 EDT amendment)
---

# A radio style for PopupMenu's mark column

## 1. What it is

flankr's subagents dropdown lists the sessions of a pane (main and its
subagents) and marks the one being viewed. The owner asked for radio
marks, not checks, and no colour: ⦿ on the selected item, ⊙ on the
others. The labels are drawn in Roboto, which has neither glyph, so the
`PopupMenu` draws the marks itself, in the column where it draws checks
today.

A menu chooses its mark style once, in its options. The items say what
they say today: `checked: Some(true)` is the selected item,
`checked: Some(false)` an unselected one, `None` no mark. With the
default `Check` style nothing changes; with `Radio`, `Some(true)` draws
⦿ and `Some(false)` draws ⊙.

## 2. Decisions

1. **The style is per menu, in `PopupMenuOptions`, not per item.**
   `PopupMenuOptions` gains `pub marks: PopupMarkStyle` (default
   `Check`) and a setter `.marks(PopupMarkStyle)`, a per-instance setting
   like `dismiss` (spec 8.5 item 2: a setting with no theme default lives
   in the options).
   Why not a field on `PopupItem`:
   - A menu's marks are one kind of choice (several independent toggles,
     or one of a set); mixing checks and radios in one menu is not asked
     for and would read badly.
   - `PopupItem` is a plain struct with public fields and **no
     `Default`**, so a caller that builds one as a struct literal cannot
     write `..Default::default()`; an appended field breaks that literal.
     The owner's ruling (appending a struct field is additive) would still
     call it a patch, but there is no need to test that ruling here when
     the menu-level setting needs no change to `PopupItem` at all.
   - `checked: Option<bool>` already carries exactly the radio's three
     states (selected, unselected, no mark), so no new item data is
     needed.
   The cost: the style is fixed for a menu instance, set when it is
   attached. An app that wants checks in one menu and radios in another
   attaches two menus under two entry ids (flankr's ⋮ menu and its
   subagents dropdown are two triggers, so two menus), or re-attaches the
   same id with new options before the next open (attaching again
   replaces the menu, plan 06 decision 2).
2. **`PopupMarkStyle` is a new `#[non_exhaustive]` enum**, `Copy, Clone,
   Debug, Default, PartialEq, Eq`, with `Check` (`#[default]`) and
   `Radio`. Being open, more styles (a plain ring, a switch) can be added
   later as patch releases.
3. **The column is unchanged.** It is reserved, as today, when any item
   has `checked: Some(_)`, with the same `check_width` (24), and the mark
   is centred at the same point, `[rect.x + item_padding / 2 +
   check_width / 2, rect.center()[1]]`. Width, height, placement, item
   rects, `hit_test`, `item_at` and input are untouched; a mark takes no
   input of its own (a tap anywhere on the row chooses the item, as
   today).
4. **The radio mark**, for an item with `checked: Some(selected)` in a
   `Radio` menu, centre `c`, from the resolved values `R =
   radio_radius`, `t = radio_thickness`:
   - **the ring**: one `SdfInstance` of kind `SdfKind::Outline { radius:
     R - t / 2, thickness: t }`, `position = [c[0] - (R - t / 2), c[1] -
     (R - t / 2)]`, `size = [2R - t, 2R - t]`. The stroke is centred on
     that square's edge, so it covers radii `R - t ..= R`: `R` is the
     ring's outer radius;
   - **the dot**: radius `d = radio_dot_radius` when selected (⦿),
     `radio_empty_dot_radius` when not (⊙), clamped to at most `R - t`
     (inside the ring); one `SdfInstance` of kind `SdfKind::Circle`,
     `position = [c[0] - d, c[1] - d]`, `size = [2d, 2d]`. A dot whose
     radius is 0 or less is not drawn, so `radio_empty_dot_radius(0.0)`
     gives a plain ring ○ for unselected items;
   - colours: the ring in `radio_ring_color`, the dot in
     `radio_dot_color`, each `None` meaning the colour the item's label is
     drawn in (the menu's text colour for an action, its heading colour
     for a heading), so the marks echo the labels and carry no accent;
     alpha multiplied by `disabled_alpha` for a disabled action, as the
     label and the check are.
   No `Line` instances are drawn for a radio mark, and no check is drawn
   in a `Radio` menu. `Some(false)` in a `Check` menu still draws nothing.
   `anim` is 0 and the instances are not elevated, so the circle casts no
   shadow.
5. **The radio does not enforce one selection.** The menu draws what the
   items say; marking exactly one item `Some(true)` is the app's job, as
   the choice that follows (`Chosen(id)`) is the app's to act on. Choosing
   still closes the menu and queues `Chosen(id)`; the menu does not move
   the mark itself (the next `open` carries the new state).
6. **Theme per section 8.5.** The sizes and colours are appended to
   `PopupMenuTheme`, with a matching `Option` and setter in
   `PopupMenuOptions` and a field in `PopupMenuResolved`; `with_theme`
   resolves them as the others. All three structs are already
   `#[non_exhaustive]`, so appending to them is additive.
7. **Version.** All additive: a new open enum, fields appended to
   `#[non_exhaustive]` structs (and, through the theme, nothing new on
   `ControlTheme`), one new setter, and no change to any existing default
   or drawing (`marks` defaults to `Check`). 0.10.4.

## 3. Interfaces

In `src/popup_menu.rs`; `PopupMarkStyle` is re-exported from the crate
root with the other `popup_menu` types.

```rust
/// How a menu draws its items' check states.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum PopupMarkStyle {
    /// `Some(true)`: a check mark; `Some(false)`: nothing. (As 0.10.3.)
    #[default]
    Check,
    /// `Some(true)`: a ring with a filled dot (⦿); `Some(false)`: a ring
    /// with a small dot (⊙).
    Radio,
}

// PopupMenuTheme, appended after `margin` (defaults):
    pub radio_radius: f32,                 // 6.0, the ring's outer radius
    pub radio_thickness: f32,              // 1.5, the ring's stroke
    pub radio_dot_radius: f32,             // 3.0, the selected item's dot (⦿)
    pub radio_empty_dot_radius: f32,       // 1.25, the others' dot (⊙); 0: none
    pub radio_ring_color: Option<[f32; 4]>, // None: the item label's colour
    pub radio_dot_color: Option<[f32; 4]>,  // None: the item label's colour

// PopupMenuOptions, appended:
    pub radio_radius: Option<f32>,
    pub radio_thickness: Option<f32>,
    pub radio_dot_radius: Option<f32>,
    pub radio_empty_dot_radius: Option<f32>,
    pub radio_ring_color: Option<[f32; 4]>,
    pub radio_dot_color: Option<[f32; 4]>,
    /// Per instance, no theme default: `Check` unless set.
    pub marks: PopupMarkStyle,
impl PopupMenuOptions {
    setter!(radio_radius: f32);
    setter!(radio_thickness: f32);
    setter!(radio_dot_radius: f32);
    setter!(radio_empty_dot_radius: f32);
    setter!(radio_ring_color: [f32; 4]);
    setter!(radio_dot_color: [f32; 4]);
    pub fn marks(self, marks: PopupMarkStyle) -> Self; // #[must_use]
}

// PopupMenuResolved, appended:
    pub radio_radius: f32,
    pub radio_thickness: f32,
    pub radio_dot_radius: f32,
    pub radio_empty_dot_radius: f32,
    pub radio_ring_color: Option<[f32; 4]>, // None read at draw
    pub radio_dot_color: Option<[f32; 4]>,  // None read at draw
```

`with_theme` resolves the four sizes with `unwrap_or(theme.x)` and the
two colours with `.or(theme.x)`, as the existing fields. `marks` is not
resolved (it is read from `self.options.marks` at draw, as `dismiss` is
read in `Modal::options`).

The sizes fit the column: the ring is 12 across, the size of today's
check (`check_size` 12), inside the 24-wide column; the ring's inner
radius is 4.5, leaving 1.5 px between it and the selected dot.

A private helper beside `draw_check`:

```rust
/// A radio mark centred on `c`: a ring of outer radius `r` and stroke
/// `t`, and a dot of radius `dot` (clamped inside the ring; none if <= 0).
fn draw_radio(list: &mut DrawList, c: [f32; 2], r: f32, t: f32, dot: f32,
              ring_col: [f32; 4], dot_col: [f32; 4]);
```

No other signature changes. `PopupItem`, `PopupItemKind`,
`PopupMenuState`, `PopupMenuLayout`, `PopupMenu` and
`Router::attach_popup_menu` are as in 0.10.3.

## 4. Steps

1. **Types, theme, options, resolution.** `PopupMarkStyle`, the six
   theme fields with the defaults of section 3, the six overrides and
   setters, `marks` and its setter, the six resolved fields and their
   resolution in `with_theme`; the crate-root re-export.
   **Done when:** a unit test resolves options with `radio_radius(8.0)`
   against a theme with `radio_dot_radius` changed to 2.0 and gets both;
   `PopupMenuOptions::default().marks` is `PopupMarkStyle::Check`; the
   existing tests pass unchanged.
2. **Drawing.** `draw_radio` (decision 4) and, in `draw`, the mark for
   `checked: Some(_)` chosen by `self.options.marks`: `Check` as today,
   `Radio` with `draw_radio` and the label's colour as the default ring
   and dot colour.
   **Done when:** router tests in `tests/popup_menu.rs`, with a fixture
   whose options set `.marks(PopupMarkStyle::Radio)` and items main
   (`Some(true)`), a heading, two actions (`Some(false)`), one of them
   disabled, show:
   - one `Outline` ring per marked item (3), each `size` `[10.5, 10.5]`
     (`2 * 6 - 1.5`) and centred on its row's mark centre, and no `Line`
     instances for marks (the separator's line, if any, only);
   - one `Circle` of `size` `[6, 6]` (the selected dot) and two of
     `[2.5, 2.5]` (the empty dots), each centred on its ring;
   - with the default theme, the ring and dot colours equal the theme's
     `text_color` (the disabled item's with alpha times 0.4), not
     `accent_color`;
   - `radio_empty_dot_radius(0.0)` draws no circle for unselected items;
   - `radio_ring_color` and `radio_dot_color` overrides reach the
     instances;
   - the default (`Check`) menu with the same items draws 2 `Line`
     segments for the `Some(true)` item and no `Circle` (today's
     drawing);
   - the frame, the item rects and a tap on an unselected item
     (`Chosen(id)`) are the same in `Radio` and `Check` menus.
3. **Spec, README, version.** The spec amendment of section 5; the
   README's decision-table row moved to the plan's new status;
   `Cargo.toml` 0.10.4, the `VERSIONS.md` entry of section 6, the tag.
   **Done when:** `cargo fmt -p libcontrols -- --check`, `cargo clippy -p
   libcontrols --all-targets` with no new warnings and `cargo test -p
   libcontrols` pass; the lead commits and tags `v0.10.4`.

## 5. The spec amendment (drafted; appended when step 3 lands)

```markdown
## Amendment — <date from the clock>: PopupMenu radio marks (sections 6.16, 8, 10, 11)

Plan `docs/design/20261028_07_POPUP_MENU_RADIO_PLAN.md`, release 0.10.4
(`v0.10.4`). This extends sections 6.16, 8, 10 and 11 as amended at
19:58 EDT; it replaces no earlier text except the version in section 10
and the test counts in 11.

1. **6.16 `PopupMenu`:** `PopupMenuOptions::marks(PopupMarkStyle)`
   (`#[non_exhaustive]`: `Check`, the default, and `Radio`) chooses how
   the mark column draws `checked`. `Check` is as before. `Radio` draws
   `Some(true)` as a ring with a filled centre dot (⦿) and `Some(false)`
   as a ring with a small centre dot (⊙): an `Outline` ring of outer
   radius `radio_radius` and stroke `radio_thickness`, and a `Circle`
   dot clamped inside it (none when its radius is 0), centred where the
   check is, in the item label's colour unless overridden, at the
   label's alpha. Layout, hit testing and input do not depend on the
   style; the menu does not enforce a single selection.
2. **8, theme:** `PopupMenuTheme` gains `radio_radius` 6,
   `radio_thickness` 1.5, `radio_dot_radius` 3, `radio_empty_dot_radius`
   1.25, `radio_ring_color` and `radio_dot_color` `None` = the item
   label's colour; `PopupMenuOptions` an override and setter for each and
   the per-instance `marks`; `PopupMenuResolved` the six values.
3. **10:** the crate is 0.10.4. `PopupMarkStyle` is `#[non_exhaustive]`.
4. **11:** `tests/popup_menu.rs` adds <n> router tests.
```

## 6. Versioning

0.10.4, a patch release: every change is additive (a new open enum,
fields appended to `#[non_exhaustive]` structs, new setters; no default
or drawing changes for an existing menu). `PopupItem` is not changed.

Draft `VERSIONS.md` entry (the short commit filled in when committed):

```markdown
## 0.10.4 — 2026-10-28 (<short commit>, tag `v0.10.4`)

PopupMenu radio marks (docs/design/20261028_07_POPUP_MENU_RADIO_PLAN.md).
Additive: a new open enum, and fields appended to `#[non_exhaustive]`
structs; menus that do not set `marks` draw as before.

- Added: `PopupMarkStyle` (`Check`, the default; `Radio`).
- Added: `PopupMenuOptions::marks` (field and setter).
- Added: `PopupMenuTheme`, `PopupMenuOptions` (with setters) and
  `PopupMenuResolved`: `radio_radius`, `radio_thickness`,
  `radio_dot_radius`, `radio_empty_dot_radius`, `radio_ring_color`,
  `radio_dot_color`.
```

Draft `CHANGES.md` addendum (appended, timestamped from the clock, when
the owner approves this plan and implementation starts):

```markdown
## <date from the clock> — PopupMenu radio marks (plan 20261028_07_POPUP_MENU_RADIO_PLAN)

New work: a radio style for the `PopupMenu`'s mark column, asked of
libcontrols by flankr's subagents plan
(~/src/flankr/docs/design/20261023_07_SUBAGENTS_PLAN.md, its 20:51 EDT
amendment, items 2 and 4) for its subagents dropdown, owner's words "the
empty dot is the menu icon then the dot is used as a radio selector in
the menu", no colour. Plan docs/design/20261028_07_POPUP_MENU_RADIO_PLAN.md;
additive, 0.10.4 (tag `v0.10.4`).
```

## 7. Open questions

1. flankr's title-bar icon is ⊙ from the terminal face, drawn as a plain
   ring until that face has U+2299 (its 20:51 EDT amendment, item 1).
   libcontrols could export the mark's drawing (`draw_radio` made public,
   or a small `RadioMark` control) so the icon and the menu's marks are
   the same shape; not covered here, and additive if added later.
2. A per-item style (checks and radios in one menu) is not covered; if it
   is ever needed, an open `PopupMarkStyle` on `PopupItem` would need
   `PopupItem` to gain a `Default` first.
3. A dot that animates between items (a fade on re-open) is not covered;
   opening is instant (plan 06, open question 4).

## Amendment — 2026-10-28 23:47 EDT: approved; built as 0.10.7 after the toggles and columns

The owner approved the plan on 2026-10-28 ("approve the radio style plan
too"). It is approved as written, its open questions left as the body
leaves them (no exported `draw_radio`, no per-item style, no animation;
each additive if wanted later). Its version follows release order: 0.10.4
went to the Scrollbar, 0.10.5 to the MarkdownView and 0.10.6 to the toggle
items and columns (plan 20261028_10, being built), so this is **0.10.7**,
built after 0.10.6 lands; its theme fields are appended after 0.10.6's,
and its marks keep drawing from `checked`, so toggle items take radio
marks in a `Radio` menu. The status stays `Draft` until the first
implementing commit (Rule 1.5).

## Amendment — 2026-10-28 23:49 EDT: toggle items keep the check mark

The owner (2026-10-28): "the toggles should be checkmark glyph instead
(checked/unchecked)". This replaces the previous amendment's "so toggle
items take radio marks in a `Radio` menu": a toggle item
(`PopupItemKind::Toggle`, plan 20261028_10) always draws the check mark
of today's check column, whatever the menu's `PopupMarkStyle`: the check
when `checked: Some(true)`, the empty column when `Some(false)`. The
radio marks of `PopupMarkStyle::Radio` apply only to `Action` items with a
check state. So flankr's volume menu (toggles) shows checks, and its
subagent dropdown (actions, `Radio`) shows ⦿ and ⊙. Tests add: a toggle in
a `Radio` menu draws the check, not a ring.
