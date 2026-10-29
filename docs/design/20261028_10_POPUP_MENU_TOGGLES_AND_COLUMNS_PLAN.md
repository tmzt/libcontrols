---
title: PopupMenu toggle items and columns
date: 2026-10-28
status: Draft
covers: >
  Two additions to the `PopupMenu`, in one release because both change
  what a menu row is and how it is chosen or drawn. (1) Toggle items:
  `PopupItemKind::Toggle`, `PopupItem::toggle(id, label, checked)`;
  choosing one (tap, Enter or Space) flips its `checked` in the state's
  items, keeps the menu open and the highlight where it is, and queues
  `PopupMenuEvent::Toggled { id, checked }` and no `Chosen`. (2) Columns:
  a row may carry cells (text, alignment, an auto, fixed or flexible
  width, dim or not, a text size scale such as 0.8) drawn in columns
  shared across the menu instead of its one label. Cells ride on a new
  `#[non_exhaustive]` `PopupRow` (an item and its cells) opened with
  `PopupMenuState::open_rows`, so `PopupItem` keeps its fields. Theme:
  `column_gap` and `dim_text_color` appended to the menu's theme,
  options and resolved values. Layout of rows, placement, hit testing
  and the existing kinds are unchanged. Additive: 0.10.6. Steps and
  tests.
sources:
  - >
    The owner (2026-10-28, through flankr's lead, verbatim): "plan those
    in appropriate repos with subagent, then impl", of what flankr's
    bottom bar plan and plugin plan need from libcontrols.
  - >
    flankr's bottom bar plan,
    ~/src/flankr/docs/design/20261028_06_BOTTOM_BAR_UPDATE_PLAN.md
    (Draft), read-only: decision 3 (the volume menu's two speak items
    "toggle in place and keep the menu open"; `PopupItemKind::Toggle`,
    `PopupItem::toggle(id, label, checked: bool)`, choosing flips
    `checked` in the state's items, keeps the highlight, leaves the menu
    open and queues `PopupMenuEvent::Toggled { id: Symbol, checked: bool
    }`, not `Chosen`; "it composes with the Draft radio-mark style"),
    step 1 (tests: a toggle flips, stays open with its highlight, queues
    one `Toggled` and no `Chosen`; existing menu tests unchanged) and its
    versioning table (named 0.10.5 there; 0.10.6 here, see section 6).
  - >
    flankr's plugin plan,
    ~/src/flankr/docs/design/20261028_04_PLUGINS_PLAN.md, read-only: its
    23:14 EDT amendment "the plan menu is a table, by last update" (item
    1: four columns, NN, mm/dd, title cut with … to the column's width,
    status in the dim colour; NN and mm/dd right-aligned in fixed-width
    columns so the rows line up; item 3: "an item may carry cells (text,
    alignment, a fixed or flexible width, dim or not) instead of one
    label, laid out in shared columns across the menu", asked of
    libcontrols by a plan of its own), its 23:14 EDT amendment on
    abbreviated statuses, its 23:15 EDT amendment (the status column at
    0.8 of the row's text size, vertically centred on the row) and its
    23:16 EDT amendment (NN, mm/dd and status at 0.8, the title at full
    size).
  - >
    libcontrols at 7f82af4 (0.10.4): src/popup_menu.rs (`PopupItem`, a
    plain struct with public fields `id`, `label`, `enabled`, `checked`,
    `kind`, no `#[non_exhaustive]` and no `Default`; `PopupItemKind` and
    `PopupMenuEvent`, `#[non_exhaustive]`; `PopupMenuLayout`,
    `PopupMenuTheme`, `PopupMenuOptions`, `PopupMenuResolved`,
    `#[non_exhaustive]`; `PopupMenuState::choose`, which always closes and
    queues `Chosen`; `open` while open replaces the items and clears the
    highlight; `PopupMenu::layout` measuring each label with
    `DrawContext::text_advance`; labels drawn uncut), src/rect.rs
    (`TextAlign { Left, Center, Right }`, re-exported), src/scrollbar/
    bookmarks.rs (`cut_to_fit`, `pub(crate)` in the private module
    `scrollbar::bookmarks`), tests/popup_menu.rs (the fixture; no shaper,
    so text is 0.55 em per char).
  - >
    The MarkdownView plan, docs/design/20261028_09_MARKDOWN_VIEW_PLAN.md,
    approved at 23:13 EDT as 0.10.5 and being built in the worktree
    `.claude/worktrees/markdown` (it changes src/theme.rs, src/lib.rs and
    src/scrollbar/; this plan is built after it lands).
related:
  - docs/design/20261028_06_POPUP_MENU_PLAN.md (the menu this extends; In
    implementation, released in 0.10.3; not changed by this plan)
  - docs/design/20261028_07_POPUP_MENU_RADIO_PLAN.md (Draft; the radio
    style for the mark column; compatible, see decision 4)
  - docs/design/20261028_02_LIBCONTROLS_SPEC.md (the living spec; section
    6.16 the menu, 8.5 the theming standard; amended when this is
    implemented)
  - ~/src/flankr/docs/design/20261028_06_BOTTOM_BAR_UPDATE_PLAN.md (first
    user of toggles: the volume menu)
  - ~/src/flankr/docs/design/20261028_04_PLUGINS_PLAN.md (first user of
    columns: the ☑ plan table)
---

# PopupMenu toggle items and columns

## 1. What it is

flankr asks two things of the `PopupMenu`:

- **Toggle items.** The bottom bar's volume menu holds two on/off items
  (*Speak coding responses*, *Speak agent (Gemini) responses*). Tapping
  one flips it and the menu stays open, so the user can flip both and
  then tap outside. Today every choice closes the menu and queues
  `Chosen`.
- **Columns.** The plugin plan's ☑ menu lists plans as a table: the NN,
  the date as mm/dd, the title, and the status, the numbers right-aligned
  so they line up, the title cut with … to its column, the status dim,
  and everything but the title at 0.8 of the row's text size. Today a
  row has one label.

Both change what a row is and how it is drawn or chosen, they touch the
same file and the same tests, and flankr needs them in one pin, so they
are one plan and one release. Two plans would each have to describe the
other's changes to `choose`, `layout` and `draw` to stay compatible.

## 2. Decisions

### Toggles

1. **A new kind, `PopupItemKind::Toggle`** (the enum is
   `#[non_exhaustive]`, so this is additive), made by
   `PopupItem::toggle(id, label, checked: bool)`: `enabled: true`,
   `checked: Some(checked)`, `kind: Toggle`. `with_enabled` and
   `with_checked` work on it as on an action. A toggle whose `checked`
   is `None` (set by `with_checked(None)` or a struct literal) is treated
   as off: it draws no mark, and choosing it makes it `Some(true)`.
2. **Choosable like an action.** `PopupItem::is_choosable` becomes "an
   enabled action or toggle". Hover, press, the Up/Down/Home/End
   highlight and the highlight's drawing treat a toggle exactly as an
   action; a disabled toggle is drawn at `disabled_alpha` and takes no
   input, as a disabled action. For the existing kinds nothing changes.
3. **Choosing a toggle** (a `Click` that passes today's pressed-item
   rule, or Enter or Space on the highlighted toggle), in the private
   `PopupMenuState::choose` (renamed `activate`):
   - flips `items[i].checked` in the state: `Some(b)` becomes
     `Some(!b)`, `None` becomes `Some(true)`;
   - leaves `open` true and the modal entry on the stack;
   - keeps `highlight` as it is (the toggled item, after a key or a
     pointer that is still over it) and clears `pressed`;
   - does not touch the layout: the check column is already reserved
     (the toggle's `checked` is `Some`), so the width cannot change;
   - queues `PopupMenuEvent::Toggled { id, checked }` with the new
     state, and no `Chosen`;
   - answers `ControlAction::Activated { part: ControlPart::Custom(i) }`,
     as a choice does.
   Choosing an action is unchanged: `Chosen(id)`, closed.
   `PopupMenuState::items()` (and `rows()`, decision 6) return the
   flipped state, so an app may read it instead of the events.
4. **Drawing is today's.** A toggle draws its mark from `checked` like
   any item: with the default check style a check for `Some(true)` and
   nothing for `Some(false)`. The Draft radio plan (20261028_07) chooses
   the mark per menu from `checked` and not from the kind, so a toggle
   in a `Radio` menu draws ⦿ or ⊙ with no change to either plan; that
   plan's decision 5 (the menu does not enforce one selection) holds for
   toggles too. Whichever of the two lands first, the other appends its
   theme fields after the first's.
5. **Events in order, and the app's part.** A menu may queue several
   `Toggled` events and then `Dismissed` (an outside tap, Escape, Back)
   or `Chosen` (an action in the same menu). The app drains them with
   `take_events`, applies each `Toggled` to its own settings, and, if it
   then replaces the items while the menu is open (`open` on an open
   menu, as flankr does when Skip comes and goes), builds them from those
   settings so they carry the flipped states. `open` on an open menu
   still clears the highlight (unchanged; see open question 1).
   `PopupMenuEvent`'s doc becomes "What happened in a menu", since a
   toggle does not end its opening.

### Columns

6. **Cells ride on a new `PopupRow`, not on `PopupItem`.** `PopupItem`
   is a plain struct with public fields and no `Default`: a field
   appended to it breaks every struct literal of it, which Rule 5.4 as
   written counts as not strictly additive. (No dependent writes one
   today: flankr, highbay and NoblesKeep build items only with the
   constructors. The owner's ruling that appending a struct field is
   additive would make it a patch anyway; this plan does not need to
   rely on it.) So:
   - `PopupRow { pub item: PopupItem, pub cells: Vec<PopupCell> }`,
     `#[non_exhaustive]`, `Clone, Debug, PartialEq`: `PopupRow::new(item)`
     (no cells), `.with_cells(cells)`, `From<PopupItem>`, and
     `PopupItem::with_cells(cells) -> PopupRow` so a row reads
     `PopupItem::action("plan/10", "10 10/28 Toggles…").with_cells(..)`.
     Being `#[non_exhaustive]`, it is where later per-row data (an icon,
     a second line) can be appended without a break.
   - `PopupMenuState::open_rows(anchor, rows: Vec<PopupRow>)`, the same
     as `open` (replacing the rows while open); `open(anchor, items)`
     becomes `open_rows` of `PopupRow::from` each item.
   - `PopupMenuState::rows() -> Vec<PopupRow>`; `items()` keeps returning
     `Vec<PopupItem>`, the rows' items.
   - The state keeps `Vec<PopupRow>` inside; `choosable`, `step`,
     `activate` and the highlight read `rows[i].item`.
   A row with no cells is drawn and measured exactly as today. A row
   with cells draws its cells instead of its label; the label is not
   drawn or measured, and stays the row's text for the app (for
   flankr's fallback, a search or a screen reader later); writing it as
   the cells joined by spaces is the recommended convention.
   Cells are used on `Action`, `Toggle` and `Heading` rows (a heading row
   of cells is a column header); a `Separator` row's cells are ignored.
7. **A cell** is `PopupCell { pub text: String, pub align: TextAlign,
   pub width: PopupCellWidth, pub dim: bool, pub scale: f32 }`,
   `#[non_exhaustive]`, `Clone, Debug, PartialEq`, made by
   `PopupCell::new(text)` (`Left`, `Auto`, not dim, scale 1.0) and the
   builders `.align(TextAlign)`, `.width(PopupCellWidth)`, `.fixed(w)`,
   `.flex()`, `.dim(bool)`, `.scale(f32)`. `align` reuses the crate's
   `TextAlign` (`Left`, `Center`, `Right`); nothing new is added to that
   closed enum.
   - `text` is one line: anything from the first `\n` on is cut with …,
     as `cut_to_fit` does.
   - `scale` multiplies the menu's font size (`ctx.font_size(r.font_size)`)
     for this cell; it is clamped to `0.25..=4.0` at use. flankr's NN,
     mm/dd and status are `.scale(0.8)`.
   - `dim` draws the cell in the dim colour (decision 10) instead of the
     row's text colour.
8. **Widths: `PopupCellWidth`**, `#[non_exhaustive]`, `Copy, Clone,
   Debug, Default, PartialEq`: `Auto` (`#[default]`, the column is as
   wide as its widest cell), `Fixed(f32)` (that many px), `Flex` (takes
   the room left over, and gives it back first when there is too
   little). Columns are shared across the menu: column `k` is the `k`th
   cell of every row that has cells. A column's kind is `Fixed(max w)`
   if any of its cells is `Fixed`, else `Flex` if any is `Flex`, else
   `Auto`; alignment, dim and scale stay per cell. A row with fewer cells
   than the menu has columns leaves the rest empty.
9. **Column layout**, computed in `PopupMenu::layout` with the same `ctx`
   that measures labels, `font_size` the menu's, `g` = `column_gap`:
   - the natural width `N_k` of column `k`: `w` for `Fixed(w)`; otherwise
     the widest `ctx.text_advance(cell.text, font_size * scale)` of its
     cells;
   - the cells' content width `C = Σ N_k + g · (n - 1)`; the menu's width
     is today's formula with `max(label_w, C)` in place of `label_w`
     (label rows still measured by their labels), within `min_width`,
     `max_width` and the viewport less `margin` as today;
   - the room for cells `A = inner_w - 2 · item_padding - check_w`
     (`inner_w`, `check_w` as today);
   - if `C <= A`, `w_k = N_k`, and the extra `A - C` is shared equally by
     the `Flex` columns, or, with none, given to the last column (so a
     right-aligned last column ends at the row's text edge);
   - if `C > A`, the excess `C - A` is taken from the `Flex` columns in
     proportion to their `N_k`, each down to 0; any excess left is taken
     from the columns from the last to the first, each down to 0;
   - columns start at `x0 = row.x + item_padding + check_w`, column `k`
     at `x0 + Σ_{j<k} (w_j + g)`; the result is appended to
     `PopupMenuLayout` as `columns: Vec<[f32; 2]>` (each column's screen
     `x` and width; empty when no row has cells).
   A cell wider than its column is cut with … (`cut_to_fit`, measured at
   the cell's size) to `w_k`; if `w_k` is narrower than "…" itself,
   nothing is drawn. `Fixed` columns are cut only by that last-resort
   rule. Row rects, heights, placement, `item_at` and `hit_test` are as
   today: cells take no input of their own.
10. **Drawing a cell.** For column `k` at `[x, w]`, a cell's size
    `s = font_size * scale`, its drawn text `t` (cut) of advance `a`:
    pen x is `x` (`Left`), `x + (w - a) / 2` (`Center`) or `x + w - a`
    (`Right`); pen y is `row.y + (row.h - s · LINE_BOX_RATIO) / 2`, so
    every cell is centred on the row whatever its size (the plugin plan's
    23:15 and 23:16 EDT amendments). Colour: `dim_text_color` (`None` =
    the theme's `muted_text_color`) when `dim`, else the row's label
    colour today (text colour for actions and toggles, heading colour for
    headings); alpha times `disabled_alpha` for a disabled action or
    toggle. Marks, highlight, separators and the frame are drawn as
    today.
11. **Theme per section 8.5.** Appended to `PopupMenuTheme`, with a
    matching `Option` and setter in `PopupMenuOptions` and a field in
    `PopupMenuResolved`, resolved in `with_theme` as the others:
    `column_gap: f32` (12.0) and `dim_text_color: Option<[f32; 4]>`
    (`None`: the theme's `muted_text_color`, read at draw). All three
    structs are `#[non_exhaustive]`. `ControlTheme` does not change.
12. **One helper shared.** `cut_to_fit` stays in
    `src/scrollbar/bookmarks.rs`; `src/scrollbar/mod.rs` gains
    `pub(crate) use bookmarks::cut_to_fit;` so `popup_menu` can call it.
    (Both files are changed by the 0.10.5 build; this line is added
    after that lands.)
13. **Version.** Every change is additive under Rule 5.4 as written:
    variants on two `#[non_exhaustive]` enums, new types (`PopupRow`,
    `PopupCell`, `PopupCellWidth`, all `#[non_exhaustive]`), new
    functions and methods, fields appended to `#[non_exhaustive]`
    structs; `PopupItem`'s fields and every existing signature are
    unchanged, and a menu with no toggles and no cells lays out, draws and
    behaves as in 0.10.4. 0.10.6, after MarkdownView's 0.10.5.

## 3. Interfaces

In `src/popup_menu.rs`; `PopupRow`, `PopupCell` and `PopupCellWidth`
are re-exported from the crate root with the other `popup_menu` types.

```rust
#[non_exhaustive]
pub enum PopupItemKind {
    Action,
    Heading,
    Separator,
    /// Chosen like an action, but flips `checked` and keeps the menu open.
    Toggle,
}

impl PopupItem {
    /// An enabled toggle, on or off.
    pub fn toggle(id: impl Into<Symbol>, label: impl Into<String>, checked: bool) -> Self;
    /// This item with cells, as a row.
    pub fn with_cells(self, cells: Vec<PopupCell>) -> PopupRow; // #[must_use]
    /// An enabled action or toggle.
    pub fn is_choosable(&self) -> bool;
}

#[non_exhaustive]
pub enum PopupMenuEvent {
    Chosen(Symbol),
    Dismissed,
    /// A toggle was chosen; the menu stays open. `checked` is its new state.
    Toggled { id: Symbol, checked: bool },
}

/// How wide a cell's column is.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub enum PopupCellWidth {
    /// As wide as the column's widest cell.
    #[default]
    Auto,
    /// This many px.
    Fixed(f32),
    /// The room left over; shrinks first.
    Flex,
}

/// One cell of a row, drawn in its column.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct PopupCell {
    pub text: String,
    pub align: TextAlign,        // Left
    pub width: PopupCellWidth,   // Auto
    /// Drawn in `dim_text_color`.
    pub dim: bool,               // false
    /// Times the menu's font size.
    pub scale: f32,              // 1.0
}

impl PopupCell {
    pub fn new(text: impl Into<String>) -> Self;
    pub fn align(self, align: TextAlign) -> Self;       // #[must_use], each
    pub fn width(self, width: PopupCellWidth) -> Self;
    pub fn fixed(self, w: f32) -> Self;                  // width(Fixed(w))
    pub fn flex(self) -> Self;                           // width(Flex)
    pub fn dim(self, dim: bool) -> Self;
    pub fn scale(self, scale: f32) -> Self;
}

/// A menu row: an item and, optionally, cells drawn instead of its label.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct PopupRow {
    pub item: PopupItem,
    /// Empty: the label is drawn, as for a plain item.
    pub cells: Vec<PopupCell>,
}

impl PopupRow {
    pub fn new(item: PopupItem) -> Self;
    pub fn with_cells(self, cells: Vec<PopupCell>) -> Self; // #[must_use]
}
impl From<PopupItem> for PopupRow;

impl PopupMenuState {
    /// As `open`, with rows.
    pub fn open_rows(&self, anchor: Rect, rows: Vec<PopupRow>);
    /// The rows it was last opened with, toggles as flipped since.
    pub fn rows(&self) -> Vec<PopupRow>;
    // open(anchor, items), items(): signatures unchanged.
}

// PopupMenuLayout, appended:
    /// Each column's screen x and width; empty when no row has cells.
    pub columns: Vec<[f32; 2]>,

// PopupMenuTheme, appended (defaults):
    pub column_gap: f32,                    // 12.0
    pub dim_text_color: Option<[f32; 4]>,   // None: muted_text_color

// PopupMenuOptions, appended, with setter!(column_gap: f32) and
// setter!(dim_text_color: [f32; 4]):
    pub column_gap: Option<f32>,
    pub dim_text_color: Option<[f32; 4]>,

// PopupMenuResolved, appended:
    pub column_gap: f32,
    pub dim_text_color: Option<[f32; 4]>,   // None read at draw
```

flankr's plan row, for illustration:

```rust
PopupItem::action(id, "10 10/28 PopupMenu toggle items and columns Draft")
    .with_cells(vec![
        PopupCell::new("10").align(TextAlign::Right).scale(0.8),
        PopupCell::new("10/28").align(TextAlign::Right).scale(0.8),
        PopupCell::new("PopupMenu toggle items and columns").flex(),
        PopupCell::new("Draft").dim(true).scale(0.8),
    ])
```

## 4. Steps

All in this crate; each step passes `cargo fmt -p libcontrols --
--check`, `cargo clippy -p libcontrols --all-targets` with no new
warnings and `cargo test -p libcontrols`. Built after 0.10.5 lands on
`main`, in a new worktree under `.claude/worktrees/`.

1. **Toggles.** `PopupItemKind::Toggle`, `PopupItem::toggle`,
   `is_choosable` (decision 2), `PopupMenuEvent::Toggled`, `activate`
   (decision 3), the disabled alpha for toggles; the event enum's doc.
   **Done when:** router tests in `tests/popup_menu.rs`, with items
   `toggle("t/a", "A", false)`, `toggle("t/b", "B", true)`, a disabled
   `toggle("t/c", "C", true)` and `action("a/x", "X")`, show:
   - a tap on A: the menu is still open, the stack still has the entry,
     `take_events()` is exactly `[Toggled { id: "t/a", checked: true }]`
     (no `Chosen`), `items()[0].checked` is `Some(true)`, and the next
     draw has two more `Line` instances than before (A's check);
   - a second tap on A queues `Toggled { checked: false }`;
   - Down, Down, Enter: B is highlighted, flips to off, the highlight is
     still B (`highlighted() == Some(1)`) and is drawn; Space flips it
     back on;
   - the disabled C takes no tap and is skipped by Down;
   - a tap on X afterwards queues `Chosen("a/x")` and closes;
   - toggles then an outside tap queue `[Toggled…, Dismissed]` in order;
   - a toggle made with `with_checked(None)` flips to `Some(true)`;
   - the frame and item rects are the same before and after a flip;
   - every existing test in the file passes unchanged.
2. **Rows and cells: types, theme, layout.** `PopupCellWidth`,
   `PopupCell`, `PopupRow` and their builders, `PopupItem::with_cells`,
   `open_rows`, `rows`, the state holding rows (decision 6); the two
   theme values with options, setters and resolution (decision 11);
   `cut_to_fit` reachable (decision 12); column layout and
   `PopupMenuLayout::columns` (decisions 8 and 9); the crate-root
   re-exports.
   **Done when:** unit and router tests show, with the default theme, the
   fixture's viewport and anchor (`max_width` 320, so the frame is at x
   72, rows at 78..386, `x0` 92, `A` 280) and two rows of cells
   `["7" Right 0.8, "10/28" Right 0.8, "Short" Flex, "Draft" dim 0.8]`
   and `["10" Right 0.8, "10/27" Right 0.8, "A much longer plan title
   that will not fit" Flex, "InImpl" dim 0.8]` (no shaper: 6.16 px per
   char at 0.8, 7.7 at 1.0):
   - `columns` is `[[92, 12.32], [116.32, 30.8], [159.12, 163.92],
     [335.04, 36.96]]` (to 0.01): the title column took the excess, the
     last column ends at 372;
   - with short titles (`"Short"`, `"Other"`) the frame is 194.58 wide
     (`C` 154.58 plus paddings) and nothing is cut; with options
     `min_width(260.0)` the 65.42 extra goes to the Flex column;
   - with no Flex cell the extra goes to the last column;
   - a `Fixed(40)` cell makes its column 40 wide in every row, whatever
     its texts;
   - a menu of plain items has empty `columns` and the same frame and
     rects as 0.10.4 (the existing layout tests unchanged);
   - `open(anchor, items)` and `open_rows` of the same items as rows give
     equal layouts, and `items()` and `rows()` agree;
   - options with `column_gap(4.0)` and a theme with `dim_text_color`
     set both resolve.
3. **Drawing cells.** Decision 10, in `draw`.
   **Done when:** router tests on the step 2 menu show:
   - one text run per cell (no label text for those rows), the long
     title's run ending in "…" and fitting in 163.92, the short one
     uncut;
   - "7" drawn at x `92 + 12.32 - 6.16` (`Right`), "Short" at 159.12
     (`Left`), a `Center` cell centred in its column;
   - the 0.8 cells' pen y is `row.y + (40 - 11.2 · LINE_BOX_RATIO) / 2`
     and the title's `row.y + (40 - 14 · LINE_BOX_RATIO) / 2`;
   - the dim cells are drawn in `muted_text_color` by default and in an
     overridden `dim_text_color`, the others in `text_color`; a disabled
     row's cells at alpha times 0.4;
   - a toggle row with cells flips and draws its check as in step 1;
   - a heading row of cells is drawn in the heading colour and takes no
     input; a separator row's cells are not drawn.
4. **Spec, README, version.** The spec amendment of section 5; the
   README's row moved to the plan's new status; `Cargo.toml` 0.10.6, the
   `VERSIONS.md` entry of section 6, the `CHANGES.md` addendum.
   **Done when:** the checks above pass; the lead commits and tags
   `v0.10.6`.

The first implementing commit sets `status: In implementation` and
appends the immutable marker (Rule 1.5).

## 5. The spec amendment (drafted; appended when step 4 lands)

```markdown
## Amendment — <date from the clock>: PopupMenu toggle items and columns (sections 6.16, 8, 10, 11)

Plan `docs/design/20261028_10_POPUP_MENU_TOGGLES_AND_COLUMNS_PLAN.md`,
release 0.10.6 (`v0.10.6`). This extends sections 6.16, 8, 10 and 11 as
amended; it replaces no earlier text except, in 6.16, "a choice; the
menu closes" now holding for actions only, and the version in 10 and
the test counts in 11.

1. **6.16 `PopupMenu`:** `PopupItemKind::Toggle` (`PopupItem::toggle(id,
   label, checked)`) is chosen like an action (`is_choosable`: an
   enabled action or toggle) but flips its `checked` in the state, keeps
   the menu open and the highlight, and queues `PopupMenuEvent::Toggled {
   id, checked }` with the new state, not `Chosen`. A row may carry cells:
   `PopupRow { item, cells }` (`PopupRow::new`, `with_cells`,
   `From<PopupItem>`, `PopupItem::with_cells`), opened with
   `PopupMenuState::open_rows` and read with `rows`; `open` and `items`
   are unchanged. A `PopupCell { text, align: TextAlign, width:
   PopupCellWidth (Auto, Fixed(px), Flex), dim, scale }` is drawn in
   column k, shared by the k-th cells of all rows: Fixed if any cell is,
   else Flex if any is, else Auto; natural widths at each cell's size,
   gaps of `column_gap`; extra room to the Flex columns (else the last),
   too little taken from the Flex columns in proportion, then from the
   last column back; cells cut with … to their column, centred
   vertically on the row, dim ones in `dim_text_color`. A row with cells
   draws them instead of its label. `PopupMenuLayout::columns` gives each
   column's x and width.
2. **8, theme:** `PopupMenuTheme` gains `column_gap` 12 and
   `dim_text_color` `None` = `muted_text_color`; `PopupMenuOptions` an
   override and setter for each; `PopupMenuResolved` both.
3. **10:** the crate is 0.10.6. `PopupRow`, `PopupCell` and
   `PopupCellWidth` are `#[non_exhaustive]`; `PopupItem` is unchanged.
4. **11:** `tests/popup_menu.rs` adds <n> router tests.
```

## 6. Versioning

0.10.6, a patch release (decision 13). The numbering follows release
order: 0.10.4 is the Scrollbar, 0.10.5 the MarkdownView now being built.
flankr's bottom bar plan names this release 0.10.5; flankr pins 0.10.6
(or later) instead.

Draft `VERSIONS.md` entry (the short commit filled in when committed):

```markdown
## 0.10.6 — <date> (<short commit>, tag `v0.10.6`)

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
```

Draft `CHANGES.md` addendum (appended, timestamped from the clock, when
implementation starts):

```markdown
## <date from the clock> — PopupMenu toggle items and columns (plan 20261028_10_POPUP_MENU_TOGGLES_AND_COLUMNS_PLAN)

New work: toggle items that keep the menu open, asked by flankr's bottom
bar plan (~/src/flankr/docs/design/20261028_06_BOTTOM_BAR_UPDATE_PLAN.md,
decision 3) for its volume menu, and cells in shared columns, asked by
flankr's plugin plan (~/src/flankr/docs/design/20261028_04_PLUGINS_PLAN.md,
its 23:14-23:16 EDT amendments) for its plan table. Plan
docs/design/20261028_10_POPUP_MENU_TOGGLES_AND_COLUMNS_PLAN.md;
additive, 0.10.6 (tag `v0.10.6`).
```

## 7. Open questions

1. **Replacing rows while open clears the highlight.** flankr replaces
   the volume menu's items when speaking starts or stops (Skip comes and
   goes); with a keyboard, the highlight is lost then. An additive
   `PopupMenuState::replace_rows(rows)` could keep the highlight on the
   row with the same id. Not covered; recommended only if flankr's
   desktop build needs it.
2. **Plain labels are still drawn uncut.** A label longer than
   `max_width` overflows the frame today. Cutting it with … would be a
   visible change to existing menus (a minor bump under Rule 5.4), so it
   is left as is; a plain row can be given one `Flex` cell to get the
   cut.
3. **`PopupItem` itself.** If the owner would rather have
   `PopupItem::with_cells` return an item (a `cells` field appended to
   `PopupItem`, a patch by the owner's ruling, a minor bump by Rule 5.4
   as written), `PopupRow` would not be needed; this plan keeps
   `PopupItem` unchanged and gives later per-row data a
   `#[non_exhaustive]` home instead.
4. **flankr's All plans… modal** uses the same columns (the plugin
   plan's 23:14 EDT amendment, item 4). It is flankr's own view; if it
   wants the same arithmetic, the column layout (decision 9) could be
   exported as a pure function later, additively. Not covered.

## Amendment — 2026-10-28 23:41 EDT: approved; built after MarkdownView's 0.10.5

The owner approved planning and building this on 2026-10-28, through
flankr's lead, of what flankr's bottom bar plan and plugin plan need from
libcontrols: "plan those in appropriate repos with subagent, then impl".
It is built after MarkdownView's 0.10.5 lands on `main`, as 0.10.6. The
status stays `Draft` until the first implementing commit (Rule 1.5).
