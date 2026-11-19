---
title: A ribbon of sections, two-finger flings and a label on the scroll thumb
date: 2026-11-19
status: Draft
covers: "flankr's agent-view ribbon generalised onto SectionedContent (placeholder sections of a fixed height, batched edits, a window by index reported to the app for fetching, a pending length past the end), a two-finger drag and fling on ScrollPane fed by a TouchTracker that turns touches into the router's pointer events, an optional fling that lands on a section boundary, and a label on the scrollbar thumb while scrolling, from the bookmark of the section at the window's top, with optional snapping to bookmarks while scrubbing. Additive: libcontrols 0.11.1."
sources:
  - "The owner (2026-11-19), in ShotToss's session: two-finger flick for rapid scrolling; the full listing is dynamic so there is no fixed ribbon size, and months start as fixed size sections that expand as data comes in; while scrolling the month dividers become scroll thumb labels; the ribbon/card based scrolling is in ~/src/flankr and might still need moving into libcontrols; then: write the plan for libcontrols, using the flankr code from a subagent or worktree."
  - "flankr at 349ed92, apps/companion/src/agent.rs: Slot, ribbon_top, refresh_blocks, ribbon_height, anchors, keep_anchors, hold, place, note_window, take_window, estimate_height, Drag (fingers, pos, pair_lifted), TWO_FINGER, KEEP_SCREENS, FORGET_SCREENS, BAR_HOLD, BAR_FADE, SPEED_WINDOW"
  - "flankr at 349ed92, crates/flankr-companion/src/view.rs: ViewSection, section_index, ViewWindow (shown, close, near, keep, scrolled_back)"
  - "flankr at 349ed92, docs/design/20261024_01_SESSION_OUTLINE_PLAN.md: the outline's Slot (index, serial, kind, lines, chars, rev), estimated heights, contents by index range"
  - "flankr at 349ed92, docs/design/20261022_03_HARNESS_PROTOCOL_PLAN.md: amendments of 2026-10-23 23:54 EDT (the ribbon) and 2026-10-24 11:09 EDT (two-finger scroll at 5x, the proportional scrollbar)"
  - "libcontrols 0.11.0 (01275d7): src/sections.rs, src/scroll.rs, src/scrollbar/mod.rs, src/scrollbar/state.rs, src/scrollbar/bookmarks.rs, src/event.rs, src/theme.rs, src/router.rs"
related:
  - docs/design/20261028_02_LIBCONTROLS_SPEC.md (sections 4.7, 5, 6.12, 6.17, 6.18, 8, 10, 11 are extended)
  - docs/design/20261028_08_SCROLLBAR_PLAN.md (sections, bookmarks and the scrollbar this builds on)
  - docs/design/20261023_02_SCROLL_PANE_PLAN.md (the pane's drag and fling)
  - "ShotToss docs/design/20261119_01_SHOTTOSS_GALLERY_PLAN.md (first user: month sections of a photo grid)"
  - "flankr docs/design/20261028_05_BINARY_FILES_PLAN.md (its note that the agent view moves onto libcontrols' Scrollbar later)"
depends_on: []
living_sections: []
---

# A ribbon of sections, two-finger flings and a label on the scroll thumb

## 1. Why

ShotToss (`dev.tmzt.shottoss`) shows a photo library as months of thumbnails
in one scroller. Its catalogue streams in: months it has not yet read start
at a fixed size and grow as their photos arrive, possibly far above the
window. The owner wants a two-finger flick for rapid scrolling, and the
month dividers to show as a label on the scroll thumb while scrolling.

flankr's agent view solved most of this in app code, as its "ribbon"
(the harness protocol plan's 2026-10-23 23:54 EDT amendment): the
scrollback is sections end to end, each with an index and a height,
estimated until laid out; the place is held by the section at the window's
top; the app reports a window by index (`ViewWindow`) and fetches the
contents of that range; and two fingers drag and fling at 5× (the
2026-10-24 11:09 EDT amendment). libcontrols already has the lower half
of it, from the scrollbar plan: `SectionedContent` (ids, heights, the
`estimated` flag, bookmarks), a pane drawing sections through a
`SectionSource`, anchoring by section, and `Scrollbar` with bookmarks
drawn while held.

This plan moves the rest into libcontrols, so ShotToss uses it first and
flankr can move onto it second.

## 2. Decisions

1. **The ribbon builds on `SectionedContent`; it does not replace it.**
   flankr's `Slot { index, serial, height }` is libcontrols' `Section { id,
   height, estimated }`: the id is the stable identity (flankr's serial,
   ShotToss's month key) and the place in the list is its index. Anchoring
   by section (spec 4.7 as amended 2026-10-29 00:07 EDT) is flankr's item 3
   already: the first section still present among those that overlapped
   the window keeps its place on screen. No new container type is added.
   What the ribbon adds is a placeholder height, batched edits, a window
   reported to the app, and a pending length (sections 3.1 to 3.4).
2. **Two fingers reach the pane as a finger count, not as touches.** The
   router's input is the mouse model (`MouseEvent` with one pointer), and
   `InputEvent` and `GestureEvent` are exhaustive enums, so a touch event
   type would be a breaking change across every control. Instead the host
   tells the router how many fingers are down (`Router::set_fingers`), and
   keeps sending the fingers' mean position as the pointer, as flankr's
   `Drag::pos` does: a finger joining or leaving does not move it. A
   `TouchTracker` helper does that conversion for a host that has touch
   ids (libmorph's `Touch`). This keeps every existing control unchanged
   and the release additive.
3. **The factor is flankr's 5×.** flankr's owner chose 5× for both the
   drag and the fling (`TWO_FINGER = 5.0`), and libcontrols takes that as
   the theme default. ShotToss's proposal of 4× is a per-pane override
   (`ScrollPane::with_two_finger_factor(4.0)`), if the owner keeps it.
4. **The thumb label is the bookmark of the section at the top.**
   Bookmarks are already kept by section id and placed from the section's
   current top on every draw, so they move when heights change above
   them. The label needs no new data: it is the bookmark of the last
   bookmarked section whose top is at or above the window's top. In
   ShotToss every month section has a bookmark ("November 2026"), and
   flankr's Finished-card labels are bookmarks today.
5. **Additive, 0.11.1.** Every addition is a new method, a new type, or a
   field appended to a `#[non_exhaustive]` struct (`Section`,
   `ScrollTheme`, `ScrollbarTheme`, `ScrollbarOptions`, `EventContext`), or
   a variant on a `#[non_exhaustive]` enum (`ScrollbarEvent`). Nothing
   changes for a host that never calls `set_fingers` and a pane or
   scrollbar that opts into nothing. Theme fields follow spec 8.5 item 4.

## 3. The ribbon (`src/sections.rs`, `src/scroll.rs`)

### 3.1 Placeholder sections

```rust
impl Section {
    /// A section whose height is not known and has no estimate: drawn and
    /// counted at its content's placeholder height until it is given one.
    pub fn placeholder(id: SectionId) -> Self;
}
pub struct Section {
    pub id: SectionId,
    pub height: f32,
    pub estimated: bool,
    /// Appended: no height of its own; the content's placeholder height.
    pub placeholder: bool,
}
impl SectionedContent {
    pub fn set_placeholder_height(&self, h: f32);   // default 96.0 router pixels
    pub fn placeholder_height(&self) -> f32;
}
```

- A placeholder section counts as `placeholder_height` in the tops, the
  total, `section_at`, the window and the scrollbar, and is `estimated`.
- `set_height(id, h)` on a placeholder clears `placeholder` (and keeps
  `estimated` as it is set by `Section::estimated` or by the call below);
  `set_estimate(id, h)` sets the height and keeps `estimated = true`;
  `report_height` from a `SectionContext` clears both, as it does today.
- Changing `placeholder_height` rebuilds the tops once; anchoring keeps the
  window's content in place as for any height change.

### 3.2 Batched edits

ShotToss's catalogue posts a count change per month per batch of 256
files; each `set_height` today rebuilds every top (`Inner::rebuild`,
O(n)). A batch rebuilds once:

```rust
impl SectionedContent {
    pub fn edit<R>(&self, f: impl FnOnce(&mut SectionEdit<'_>) -> R) -> R;
}
pub struct SectionEdit<'a> { /* the locked list */ }
impl SectionEdit<'_> {
    pub fn push(&mut self, section: Section);
    pub fn insert(&mut self, index: usize, section: Section);
    pub fn remove(&mut self, id: SectionId) -> Option<Section>;
    pub fn set_height(&mut self, id: SectionId, h: f32) -> bool;
    pub fn set_estimate(&mut self, id: SectionId, h: f32) -> bool;
    pub fn index_of(&self, id: SectionId) -> Option<usize>;
    pub fn len(&self) -> usize;
}
```

The tops and the id index are rebuilt once when `f` returns, drawings of
removed ids are dropped, and a pane's next range update applies anchoring
once for the whole batch. The existing single calls stay, each a batch of
one.

### 3.3 The window, reported by index

flankr's `note_window` and `take_window`, generalised. A pane with
sections works out, on each range update, which sections are in and near
its window, and the app takes it when it changed:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct SectionWindow {
    /// Inclusive index ranges into the list: in the window; within
    /// `close` window lengths of it; within `near`; within `keep`.
    pub shown: (usize, usize),
    pub close: (usize, usize),
    pub near: (usize, usize),
    pub keep: (usize, usize),
    /// The ids at the ends of `shown`, for an app that keys by id.
    pub shown_ids: (SectionId, SectionId),
    /// The pane is following its sticky end (ScrollState::is_following).
    pub following: bool,
}

impl SectionedContent {
    /// The window when it changed since last taken; `None` while the
    /// scrollbar thumb is held or the list is empty.
    pub fn take_window(&self) -> Option<SectionWindow>;
    pub fn window(&self) -> Option<SectionWindow>;            // last worked out
    pub fn set_window_margins(&self, close: f32, near: f32, keep: f32);
}
```

- Margins default to flankr's: `close` 1.0, `near` 5.0 (`KEEP_SCREENS`),
  `keep` 20.0 (`FORGET_SCREENS`), in window lengths each side.
- An app fetches data for `near` and may forget what is outside `keep`;
  under memory pressure it may use `close` instead, as flankr does by its
  `Pressure` grade. libcontrols only reports the ranges.
- While the thumb is held, nothing is reported (flankr: "contents load
  where it stops"), and the window at release is reported on the next
  update.
- These are independent of the drawing cache's `set_prefetch` and
  `set_keep`, which govern `SectionSource` drawings, not the app's data.
  A section whose data has not arrived is drawn by the app's source as a
  placeholder.

### 3.4 Length known past the end

flankr counts the sections of a newer outline held aside in the
scrollbar's length, so the thumb stops short of the bottom while the view
is scrolled back:

```rust
impl SectionedContent {
    /// Extra length counted by a scrollbar after the last section, not
    /// scrollable into: content known to exist past the end. Default 0.
    pub fn set_pending_length(&self, px: f32);
}
```

The pane's range is unchanged by it; the scrollbar's content length is
`total_height + pending_length`.

## 4. Two fingers (`src/touch.rs`, `src/router.rs`, `src/scroll.rs`)

### 4.1 The finger count

```rust
impl Router {
    /// How many fingers are down now; the host calls it when the count
    /// changes. 0 or 1 is the plain pointer.
    pub fn set_fingers(&self, count: u8);
}
pub struct EventContext {
    // existing fields …
    /// Appended: the fingers down when the event was delivered.
    pub fingers: u8,
    /// Appended: the router clock's time when the count last went from 2
    /// or more to fewer, if it has.
    pub pair_lifted_at: Option<f64>,
}
```

The router keeps the count and `pair_lifted_at` and passes them in every
`EventContext`. The router's clock is the one `Router::advance` moves.

### 4.2 `TouchTracker`, for a host with touch ids

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TouchPhase { Started, Moved, Ended, Cancelled }

#[derive(Default)]
pub struct TouchTracker { /* fingers by id, the summed mean position */ }
impl TouchTracker {
    pub fn new() -> Self;
    /// One touch; returns the router events to deliver, in order, and the
    /// finger count when it changed.
    pub fn touch(&mut self, id: u64, phase: TouchPhase, pos: [f32; 2]) -> TouchOutput;
}
#[non_exhaustive]
pub struct TouchOutput { pub events: Vec<InputEvent>, pub fingers: Option<u8> }
```

- The first finger down gives `MouseEvent::Down { button: Left }` at its
  position. While fingers are down, a move of any of them adds the mean
  of all the fingers' movement since their last positions to a summed
  position, and gives `MouseEvent::Drag` there, so a finger joining or
  leaving does not jump the pointer (flankr's `Drag::pos`).
- The last finger up gives `MouseEvent::Up` at the summed position;
  `Cancelled` on the last finger gives `Up` too, after the router's
  cancel rules.
- `fingers` is `Some(n)` whenever the count changes; the host calls
  `router.set_fingers(n)` before delivering that output's events.
- libmorph's `Touch { id, phase, .. }` maps one to one onto it; the
  host's existing mouse mapping is replaced by it, not added to it.

### 4.3 What the pane does

New theme values (appended to `ScrollTheme`) and per-pane overrides:

| Value | Theme field | Default | Override |
| --- | --- | --- | --- |
| Drag and fling factor with 2+ fingers | `two_finger_factor` | 5.0 | `ScrollPane::with_two_finger_factor(f32)` |
| Fingers lifted together within | `two_finger_lift` | 0.100 s | — |
| Fastest two-finger fling | `max_two_finger_fling` | 16000.0 px/s | — |

```rust
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum FlingStop { #[default] Free, TwoFinger, Always }
impl ScrollPane { pub fn with_fling_stop(self, stop: FlingStop) -> Self; }
```

- **Drag.** While `ctx.fingers >= 2`, a drag moves the offset by
  `two_finger_factor` times the pointer's movement. When the count drops
  to 1 the drag goes on at 1× from where it is (flankr's 11:09 EDT item 1).
- **Fling.** On `Up`, when `pair_lifted_at` is within `two_finger_lift`
  of now, the release speed (measured as today) is multiplied by
  `two_finger_factor` and capped at `max_two_finger_fling` (flankr's
  `MAX_FLING_OUT`) instead of `max_fling`. A fling's decay, rest rule and
  minimum are unchanged.
- **Landing on a boundary.** With `FlingStop::TwoFinger` (two-finger
  flings only) or `Always` (every fling), on a pane with sections: the
  fling's natural landing point is `offset + v / fling_decay`. The target
  is the first section top at or past it in the fling's direction (the
  end of the range if there is none), clamped to the range; the starting
  speed becomes `(target - offset) * fling_decay`, so the exponential
  decay ends on the target. If that speed is over the cap, the target is
  the last section top within the capped distance. The fling stops
  within 0.5 px of the target and the offset is set to it exactly. A
  section whose height changes during the fling moves the target with
  anchoring.
- A tap is never made by two fingers: a press that saw 2 or more
  fingers sends no `Click` (flankr's `Drag::multi`).

## 5. The label on the thumb (`src/scrollbar/`)

New theme values (appended to `ScrollbarTheme`) and overrides (appended
to `ScrollbarOptions`, one setter each, resolved into `ScrollbarResolved`):

| Value | Field | Default |
| --- | --- | --- |
| Label font size | `thumb_label_font_size` | 13.0 |
| Label padding | `thumb_label_pad_x`, `thumb_label_pad_y` | 12.0, 4.0 |
| Gap from the thumb's left edge | `thumb_label_gap` | 8.0 |
| Background and text alpha | `thumb_label_bg_alpha`, `thumb_label_text_alpha` | 0.85, 1.0 |
| Background colour | `thumb_label_bg` | `None` = the theme's `bg_color` |
| Text colour | `thumb_label_color` | `None` = `text_color` |
| Snap distance while scrubbing | `snap_distance` | 16.0 track pixels |

Per instance (no theme default):

```rust
impl ScrollbarOptions {
    pub fn thumb_label(self, on: bool) -> Self;        // default false
    pub fn snap_to_bookmarks(self, on: bool) -> Self;  // default false
}
impl ScrollbarState {
    /// The bookmark the label shows now, if any.
    pub fn thumb_label(&self) -> Option<(SectionId, BookmarkText)>;
}
```

- **What it shows.** The bookmark of the last section with a bookmark
  whose top is at or above the window's top (`offset + 0.5`). Above the
  first bookmarked section there is no label.
- **When.** While the thumb is shown and not held: while scrolling and for
  `hold` after, fading over `fade` with the thumb (2 s and 300 ms by
  default), at the thumb's alpha divided by the resolved `alpha`, so it is
  fully opaque when the thumb is fully shown. With `Always` visibility it
  shows only while scrolling and `hold` after, then fades, while the
  thumb stays. While the thumb is held, the label is not drawn and the
  bookmarks are drawn as today, the nearest the brightest.
- **Where.** A pill to the left of the thumb, its right edge
  `thumb_label_gap` left of the thumb's left edge, vertically centred on
  the thumb and kept inside the track; radius half its height; the text
  cut with "…" to half the track's width less the padding.
  `BookmarkText::Markdown` is drawn as the bookmarks draw it.
- **Switching.** It changes text when the top of the next bookmarked
  section passes the window's top, in either direction, with no
  animation.
- **Snapping.** With `snap_to_bookmarks`, while scrubbing, when the
  thumb's centre is within `snap_distance` of a bookmark pill's centre the
  scrub offset is that bookmark's section top; `ScrollbarEvent::Scrub`
  and `Released` report it with `bookmark: Some(id)` as today, plus a new
  variant `ScrollbarEvent::Snapped { id, offset }` the first time each
  snap takes.
- **Moving bookmarks.** Bookmarks already follow their section's current
  top; the label reads the same tops, so a placeholder section growing
  above the window moves no label and no thumb jump beyond what anchoring
  gives.

## 6. Steps

1. **Placeholders and batched edits** (3.1, 3.2), with unit tests in
   `src/sections.rs`.
   **Done when:** tests pass for a placeholder counted at the content's
   height and turning exact on `set_height`; `set_placeholder_height`
   rebuilding once; an `edit` of 1 000 `set_height` calls rebuilding the
   tops once (counted); removed ids' drawings dropped; and a router test
   in which a batch growing ten sections above the window moves nothing
   in the window.

2. **The window and pending length** (3.3, 3.4).
   **Done when:** router tests in `tests/sections.rs` pass for the four
   ranges at the top, the middle and the end of a list of 500 sections of
   mixed heights; `take_window` returning `None` until the window changes
   and while the thumb is held, then the release window once; margins
   changed by `set_window_margins`; `following` true at a sticky end; and
   `pending_length` lengthening the thumb's content without changing the
   pane's range.

3. **The finger count and `TouchTracker`** (4.1, 4.2).
   **Done when:** unit tests in `src/touch.rs` pass for one finger down,
   moving and up giving `Down`, `Drag`, `Up`; a second finger joining and
   leaving not moving the pointer; the mean movement of two fingers;
   `Cancelled`; and the count reported only on change; and a router test
   shows `ctx.fingers` and `pair_lifted_at` reaching a control.

4. **Two-finger drag and fling** (4.3, without `FlingStop`).
   **Done when:** router tests in `tests/scroll.rs` pass for a two-finger
   drag moving 5×, dropping to 1× when one finger lifts, a pair lifted
   within 100 ms flinging at 5× the one-finger speed under the 16 000 px/s
   cap, a pair lifted 200 ms apart flinging at 1×, an override of 4.0, and
   no `Click` from a two-finger press; and one-finger behaviour is
   unchanged (the existing scroll tests pass untouched).

5. **Landing on a section boundary** (4.3, `FlingStop`).
   **Done when:** tests pass for a `TwoFinger` fling landing exactly on
   the first section top past its natural landing point, in both
   directions; a capped fling landing on the last top within reach; a
   one-finger fling free under `TwoFinger` and landing under `Always`; the
   end of the range as target past the last section; and a height change
   above the window during the fling still landing on the same section's
   top.

6. **The thumb label** (section 5, without snapping).
   **Done when:** tests in `src/scrollbar/` and `tests/scrollbar.rs` pass
   for the label's text at, just above and just below a section boundary;
   none above the first bookmark; shown while scrolling, held 2 s and
   faded over 0.3 s; not drawn while held; under `Always`, shown only
   while scrolling and `hold` after; its pill's rect at the top and the
   bottom of the track; text cut with "…"; and a GPU-free draw test that
   the label's `DrawList` holds one rounded box and its text.

7. **Snapping while scrubbing** (section 5).
   **Done when:** tests pass for a scrub within 16 px of a bookmark
   setting the offset to its section's top, `Snapped` sent once per snap,
   a scrub further away not snapping, and `snap_to_bookmarks(false)`
   unchanged from 0.11.0.

8. **Spec amendment, version and release.** One amendment to
   `docs/design/20261028_02_LIBCONTROLS_SPEC.md` (sections 4.7, 5, 6.12,
   6.17, 6.18, 8, 10, 11), `Cargo.toml` 0.11.1 with its `VERSIONS.md`
   entry, the tag `v0.11.1`, and this plan's implementation amendment.
   **Done when:** `cargo fmt -- --check`, `cargo clippy --all-targets` with
   no new warnings and `cargo test` pass with the default features and
   with `markdown` and `cmark`; the tag is on the version commit; and
   ShotToss builds against the tag with a two-finger fling and a month
   label checked on the owner's S23.

## 7. How flankr would adopt it (a later flankr change, not this plan)

- `AgentScreen::slots`, `tops` and `slot_index` become a `SectionedContent`
  with the serial as `SectionId`; the marker row becomes the first
  section, of a fixed height; a pinned card's section gets height 0, as it
  has today.
- `note_window` and `take_window` become `SectionedContent::take_window`,
  mapping positions back to the daemon's indexes (flankr keys by the
  daemon's `u64` index, which can have gaps the outline detects; the
  library's indexes are positions in the list).
- `held_newer` becomes `set_pending_length`.
- `Drag`'s fingers, `pos`, `pair_lifted` and `TWO_FINGER` become
  `TouchTracker`, `Router::set_fingers` and the pane's two-finger factor.
- flankr's blocks keep their own layout and drawing; it can use the index,
  window and anchoring without a `SectionSource`.

## 8. What of flankr's ribbon does not move

- **The fling's feel.** flankr's fling curve (`FLING_KNEE` 1000 and
  `FLING_POWER` 1.5), the boost of a flick into a running fling
  (`FLING_BOOST_TOUCH` 400 ms) and the frame-time cap (`FLING_DT_MAX`
  1/30 s) differ from `ScrollPane`'s fling and are left to their own plan.
- **The jump scroll** (2 s ease-in-out to the end), the jump line, the
  pull to refresh and the placeholders' shimmer are flankr's views.
- **Memory grades.** flankr picks `close`, `near` or `keep` by its
  `Pressure`; libcontrols reports the ranges and leaves the choice to the
  app.
- **Its block layout.** flankr's `refresh_blocks` (summary contexts,
  compaction neighbours, notes after cards) is content-specific; only its
  rule that a change touches one block and the running tops after it is
  carried, by batched edits.

## 9. Open questions for the owner

1. The two-finger factor: flankr's 5× as the default (this plan), with
   ShotToss at 4× by override, or one value for both?
2. Whether the thumb label should also show while the thumb is held, in
   place of the nearest bookmark's highlight (this plan hides it then).
3. The placeholder height's default (96 px) is arbitrary; ShotToss will set
   its own (a divider and three rows of tiles).

## Amendment — 2026-11-19 18:31 EST: a customizable fling profile, counted flicks and levelled sections (sections 2, 3, 4.3, 5, 6, 8, 9)

The owner (2026-11-19), relayed by the ShotToss coordinator: "the fling
should be customizable, with flankr's as a good starting point, adjusted
to the month-based view"; "the user would get the feel for how many months
they are skipping, or if they applied just the flick, jump one month";
"for very large collections the smallest granularity might change to week
or day"; "two-finger is 4x the one-finger (which is a clear flick gesture
not a scroll)"; "the pill should be the determined granularity, same as
the label, it could use a larger pill for month/year combinations (or even
year past a large number of unloaded months)"; then approval of the proposal (parts A to G) with version
0.12.0, flankr's fling as the default, and the finest unit where the view
is as the unit counted.

This amendment replaces:

- section 2, decision 3 (one 5× factor for drag and fling, ShotToss's 4× an
  override) and decision 5 (additive, 0.11.1); the version in the front
  matter's `covers` ("Additive: libcontrols 0.11.1") is superseded by
  item 9 below;
- in section 4.3: the table of theme values, `ScrollPane::with_two_finger_factor`,
  the `FlingStop` enum and `ScrollPane::with_fling_stop`, the **Fling** and
  **Landing on a boundary** bullets, and the sentence "A fling's decay,
  rest rule and minimum are unchanged";
- in section 5: the **What it shows**, **When** and **Switching** bullets,
  for the cases items 6 and 7 below cover, and "while held, all bookmarks
  show as today" with the nearest-first placement it keeps (item 7's pills
  replace both);
- steps 4, 5 and 8 of section 6 (steps 1 and 6 are extended);
- the first bullet of section 8 ("The fling's feel …"), which is no longer
  left to a later plan;
- open question 1 of section 9, now answered.

Everything else stands.

### 1. `FlingProfile`

```rust
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct FlingProfile { /* the fields below, all pub */ }
impl FlingProfile {
    pub fn flankr() -> Self;    // the default: flankr 349ed92's values
    pub fn classic() -> Self;   // libcontrols 0.11.0's pane fling
    // one builder setter per field, clamping to its bounds
}
impl Default for FlingProfile { fn default() -> Self { Self::flankr() } }

#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub enum SpeedFit { Endpoints, WeightedLine { oldest_weight: f32 } }
```

Units are the router's pixels and seconds, as for `ScrollTheme` (flankr's
constants are points times its scale; the same numbers at scale 1).

| Field | Unit | `flankr()` (default) | `classic()` | Bounds | Meaning |
|---|---|---|---|---|---|
| `speed_window` | s | 0.100 | 0.100 | 0.016–0.5 | the release speed is measured over this span back from the last move |
| `speed_span_min` | s | 0.016 | 0 | 0–`speed_window` | a shorter span gives no speed |
| `speed_fit` | `SpeedFit` | `WeightedLine { oldest_weight: 0.5 }` | `Endpoints` | weight 0–1 | flankr's least-squares line weighted 1 (newest) to 0.5 (oldest); 0.11.0's first and last sample |
| `max_release_speed` | px/s | 8000 | 8000 | > 0 | the measured finger speed's cap |
| `curve_knee` | px/s | 1000 | 1000 | > 0 | below it a fling follows the finger exactly |
| `curve_power` | — | 1.5 | 1.0 | 1.0–3.0 | above the knee, `knee · (v / knee)^power` |
| `max_speed` | px/s | 16000 | 8000 | ≥ `min_speed` | the cap after the curve, the boost and two fingers |
| `decay` | 1/s | 2.0 | 3.2 | 0.1–20 | the speed falls to `e^-decay` of itself each second |
| `min_speed` | px/s | 30 | 30 | ≥ 0 | slower, a fling stops or does not start; also the overscroll spring's snap |
| `rest` | s | 0.100 | 0.100 | 0–1 | a finger still this long before lifting does not fling |
| `dt_max` | s | 1/30 | 0.1 | 0.001–0.1 | the longest frame a fling advances by |
| `boost_window` | s | 0.400 | 0 | 0–2 | a gesture lifted within this long of its touch-down, the same way as the fling running at touch-down, adds to it; 0 is off |
| `boost_factor` | — | 1.0 | 0 | 0–2 | the share of the running fling's speed added (distance flings) |
| `two_finger_factor` | — | 5.0 | 5.0 | 1–20 | two-finger drag movement and distance-fling speed, times this |
| `two_finger_lift` | s | 0.100 | 0.100 | 0–0.5 | the fingers count as lifted together within this |
| `flick_max_duration` | s | 0.200 | 0.200 | 0.05–0.5 | touch-down to the last lift, at most, for a flick |
| `flick_max_travel` | px | 320 | 320 | 16–2000 | the pointer's movement (the fingers' mean, before any factor), at most, for a flick |
| `flick_min_speed` | px/s | 600 | 600 | > `min_speed` | the measured release speed, at least, for a flick |
| `stop` | `FlingStop` | `Free` | `Free` | — | where a fling ends (item 3) |

A distance fling's release: the speed by `speed_fit` over `speed_window`,
capped at `max_release_speed`; the curve; times `two_finger_factor` for a
pair lifted within `two_finger_lift`; plus `boost_factor` times the
running fling under the boost rule; capped at `max_speed`; no fling if
`rest` passed or the speed is at most `min_speed`. Each tick advances by
`min(dt, dt_max)` and multiplies the speed by `e^(-decay · dt)`.

### 2. Theme and overrides (spec 8.5)

`ScrollTheme::fling: FlingProfile` is appended, `FlingProfile::flankr()` by
default. `ScrollTheme`'s `fling_decay`, `min_fling`, `max_fling` and `rest`
are removed (8.5: a theme does not repeat a value); they are
`fling.decay`, `fling.min_speed`, `fling.max_release_speed` and
`fling.rest`, and the 0.11.0 `tick` clamp of `dt` to 0.1 is
`fling.dt_max`. `FlingOptions` has the same fields as `Option`s with one
setter each and a `Default` that overrides nothing;
`FlingOptions::from(FlingProfile)` overrides every field (a whole preset);
`FlingOptions::with_theme(&self, &FlingProfile) -> FlingProfile` resolves
them. The resolved type is the profile itself, since its values are plain:
a departure from 8.5's separate `XResolved`, recorded here. Per pane,
`ScrollPane::with_fling(FlingOptions)`.

### 3. Where a fling ends: `FlingStop`

```rust
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[non_exhaustive]
pub enum FlingStop {
    /// A distance fling, ending where its decay takes it.
    #[default]
    Free,
    /// A distance fling whose end moves to the first boundary at or past
    /// its natural landing point.
    #[non_exhaustive]
    Boundary { boundaries: Boundaries },
    /// A flick counts boundaries (item 4); any other fling is `Free`.
    #[non_exhaustive]
    Count { boundaries: Boundaries, skip: SkipMapping },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum Boundaries {
    /// Every section's top: one unit of the finest level where the view is.
    #[default]
    Sections,
    /// Tops whose level (item 8) is at most n: units of level n everywhere.
    AtMost(u8),
    /// Tops of sections that have a bookmark.
    Bookmarked,
}
```

There is no per-finger selector (the proposal's `StopFingers` is not
built): what counts is the gesture, a flick, with one finger or two, by
the classifier of item 1. An exact `Level(n)` is not offered, since
counting months by level 1 alone would skip every January (level 0).
`Boundary` lands as section 4.3's former landing rule did: the starting
speed becomes `(target - offset) · decay`, and when that is over
`max_speed` the target is the last boundary within reach. On a pane
without sections every stop is `Free`.

### 4. Counted flicks

**The classifier.** On the last lift, the gesture is a flick when its
time from touch-down to that lift is at most `flick_max_duration`, the
pointer's travel is at most `flick_max_travel`, the measured release speed
`v_r` (by `speed_fit`, capped at `max_release_speed`, before the curve and
any factor) is at least `flick_min_speed`, and `rest` did not pass. Two
fingers are a two-finger flick when they lifted as a pair within
`two_finger_lift`. Anything else that ends with momentum (a scroll, a drag
longer than the flick window) is a distance fling, as in flankr.

```rust
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct SkipMapping {
    pub skip_threshold: f32,          // px/s, default 1500, > flick_min_speed
    pub per_doubling: f32,            // default 2.0, 0.5–8
    pub max: u32,                     // one-finger cap, default 12, ≥ 1
    pub two_finger_count_factor: u32, // default 4, 1–16
    pub two_finger_max: u32,          // default 48, ≥ max
    pub duration_base: f32,           // s, default 0.30, 0.05–2
    pub duration_per: f32,            // s per boundary after the first, default 0.05, 0–0.5
    pub duration_max: f32,            // s, default 0.80, ≥ duration_base
}
```

**The count**, with `T = skip_threshold`, `k = per_doubling`, `M = max`:

```
n₁ = 1                                   if v_r < T        (a light flick: the next unit)
n₁ = min(M, 2 + floor(k · log2(v_r / T))) if v_r ≥ T
n  = n₁                                   one finger
n  = min(two_finger_max, n₁ · two_finger_count_factor)    a two-finger flick
```

With the defaults:

| `v_r` (px/s) | 600–1499 | 1500 | 2121 | 3000 | 4243 | 6000 | 8000 (cap) |
|---|---|---|---|---|---|---|---|
| One finger | 1 | 2 | 3 | 4 | 5 | 6 | 6 |
| Two fingers | 4 | 8 | 12 | 16 | 20 | 24 | 24 |

The mapping is stepped, monotonic and logarithmic in the hand's speed,
and reads nothing else, so the same flick passes the same number of
boundaries every time; two fingers pass four times as many. A distance
fling instead travels about `v / decay` pixels, so how many sections it
crosses depends on their heights. `two_finger_factor` (5×) stays the
two-finger drag's and distance fling's; the count's multiplier is the
separate `two_finger_count_factor`, applied to `n`, not to the speed.

**The target.** On release: the `n`th boundary of the chosen set after
the section at the window's top, in the flick's direction (the end of the
range if there are fewer), held by its `SectionId`, not by an offset.
Placeholder sections count as one each (under `AtMost(n)`, when their
level is at most n; under `Bookmarked`, when bookmarked).

**The flight.** An ease-out (cubic) over `duration(n) = min(duration_max,
duration_base + duration_per · (n - 1))` in place of the decay. Each tick
the offset is `top(target) - (1 - ease(t / duration)) · d₀`, `d₀` being
the signed distance from the release offset to the target's top at
release and `top(target)` its top this frame, so a height change, an
insertion, a split or a merge (item 8) during the flight still lands it
exactly. A removed target becomes the next section still present in the
same direction, or the end. At `t = duration` the offset is set to
`top(target)` exactly. A touch-down stops the flight, as it stops a fling.

**Flicks add up.** A flick lifted within `boost_window` of its touch-down,
the same way as a counted flight running at touch-down, adds its own `n`
(a two-finger flick's already multiplied) to the boundaries still to pass;
the total remaining is capped at `two_finger_max`, and the flight restarts
from the current offset over `duration(remaining)`. A flick the other way,
or later, counts afresh from the section at the top. A distance fling
after a counted flight is unaffected by it.

### 5. Events

`ScrollState::take_fling_events() -> Vec<FlingEvent>` (`#[non_exhaustive]`,
as are its variants): `Counted { target: SectionId, n: u32 }` on release,
`Passed { id: SectionId, remaining: u32 }` at each boundary crossed,
`Retargeted { target: SectionId }` (a removed, split or merged target, or a
flick added), and `Landed { id: SectionId }`. libcontrols has no haptics;
an app may turn `Passed` into a tick.

### 6. The thumb label during a counted flight

From release to landing the label shows the **target**'s label, at full
opacity under every visibility, so the user sees where the flick will land
and learns the mapping from it; passing labels would change up to dozens
of times in 0.8 s, unreadably. After landing, section 5's rule resumes
(the section at the window's top, now the target), held 2 s and faded over
300 ms. `ScrollbarOptions::skip_count(bool)` (default false) adds the
boundaries still to pass after the label, with the direction ("↓ 3"),
counting down as each is crossed, hidden for a one-boundary flight.

### 7. Labels built from levels

Each section's bookmark is the label of its own unit only ("2026",
"November", "week 3", "Mon 17"). For a reference point (the window's top
while scrolling, the target during a counted flight) libcontrols gathers,
for each level from 0 to the finest present, the latest bookmarked
boundary of that level at or above the point, keeping a part only when it
is at or after the latest boundary of every coarser level (so October's
"week 3" is dropped once November begins). The app formats them:

```rust
#[non_exhaustive]
pub struct LabelParts<'a> {
    /// Coarsest first: (level, section, its bookmark).
    pub parts: &'a [(u8, SectionId, &'a BookmarkText)],
    pub purpose: LabelPurpose,   // Top, Target or Pill, #[non_exhaustive]
}
impl SectionedContent {
    pub fn set_label_format(
        &self,
        f: Option<Arc<dyn Fn(&LabelParts<'_>) -> BookmarkText + Send + Sync>>,
    );
}
```

The default joins the two finest parts, coarsest first, with " · ". The
formatter lives on `SectionedContent`, which already holds an `Arc`
source, because `ScrollbarOptions` is `PartialEq` data a closure cannot
be. `ScrollbarState::thumb_label` returns the formatted text. Without
levels (all 0) the label is the one bookmark, as section 5 says.

**Pills while the thumb is held follow the same granularity.** A pill is
drawn for each bookmarked section's top: the sections the app determined
for each range, the same ones the label names and counted flicks count.
Its text comes from the same formatter, called with `LabelPurpose::Pill`
and the parts gathered at that section's top, so the app may shorten it
("Nov", "wk 3"). The bookmark's own text is used only when no formatter is
set.

**Larger pills for coarser boundaries.** Appended to `ScrollbarTheme`, with
`ScrollbarOptions` overrides and `ScrollbarResolved` values per spec 8.5:

| Value | Field | Default | Bounds |
|---|---|---|---|
| Scale per level step | `pill_level_scale` | 1.15 | 1.0–2.0 |
| Largest scale | `pill_scale_max` | 1.6 | 1.0–3.0 |
| Collapse a placeholder run longer than | `collapse_placeholders_after` | 6 sections | 0 (never) or ≥ 2 |

A pill's step is the finest level among the pills on the track less its
own level (a month start among weeks is 1 step, a year start among weeks
2); its font size, padding and height are multiplied by
`min(pill_scale_max, pill_level_scale^step)`. Crowding is resolved coarser
first: pills are placed in order of level, coarsest first, and within a
level nearest the thumb first, each only where it clears those already
placed by `bookmark_gap`, so finer pills give way before coarser ones. The
pill nearest the thumb is still drawn at the `near` alphas.

**Collapsed runs of placeholders.** A run of consecutive placeholder
sections longer than `collapse_placeholders_after` draws pills only at the
coarsest level found among the run's boundaries (a "2019" year pill past
many unloaded months); when no boundary in the run is coarser than its
first, one pill is drawn at the run's first section. As sections load the
run shortens, and once it is no longer than the threshold its pills are
drawn at the determined granularity again. Collapsing is drawing only:
counted flicks still count every placeholder as one section, since a
count that changed as data loaded, even mid-flight, would no longer be
learnable, and the target is held by id either way.

`ScrollbarOptions::bookmark_levels(max: u8)` stays as a generic option,
default 7 (every level), for an app whose sections are finer than it wants
pills for; it only drops pills of a level finer than `max`, before
crowding. ShotToss does not use it.

### 8. Levels on a flat list, and splitting and merging

```rust
pub struct Section {
    // id, height, estimated, placeholder …
    /// Appended: the level of the boundary at this section's top, 0 the
    /// coarsest. Default 0.
    pub level: u8,
}
impl Section { pub fn level(self, level: u8) -> Self; }   // 0–7, higher clamps to 7

impl SectionEdit<'_> {
    /// Replace `id` with `pieces` (new ids, in order); false if absent.
    pub fn split(&mut self, id: SectionId, pieces: Vec<Section>) -> bool;
    /// Replace the consecutive run `first..=last` with `merged`.
    pub fn merge(&mut self, first: SectionId, last: SectionId, merged: Section) -> bool;
}
```

The list stays flat, one section per finest unit the app chose for that
range, and a section's level is that of the coarsest unit starting at its
top (ShotToss: a split month's first week carries the month's level 1, its
other weeks level 2). A tree was not chosen because every pane, window,
anchoring and scrollbar path reads a flat list of tops. Replaced ids lose
their drawings and bookmarks; the app adds the new ids' bookmarks in the
same `edit`. Anchoring by run: when the section anchoring the window is
split, the window keeps its distance from the top of the replacing run;
when merged, its distance from the old run's first top, now inside the
merged section; nothing on screen moves. A counted flight's target that is
split becomes the first piece, and one merged the merged section's top.

### 9. Steps and version

Step 1's **Done when:** adds: `Section::level` defaulting to 0 and a
level-0 list behaving as before; `split` and `merge` in one `edit`
rebuilding once; a split or merge of the anchoring section leaving the
window's content in place; replaced ids' drawings and bookmarks dropped.

Step 4 is replaced by:

4. **The fling profile and two fingers** (items 1 and 2, and 4.3's
   two-finger drag).
   **Done when:** release-pipeline tests pass against flankr's numbers
   (knee 1000, power 1.5, the weighted-line fit equal to flankr's
   `release_speed` on its sample sets, the boost added within 400 ms of
   touch-down and not after); `classic()` passes the existing
   `tests/scroll.rs` flings unchanged; the default profile's fling covers
   the same distance at 1/60 s and 1/10 s frames; a two-finger drag
   moves 5× and drops to 1× when one finger lifts; a pair lifted within
   100 ms flings 5× and one 200 ms apart 1×; the cap at `max_speed`; no
   `Click` from a two-finger press; `with_fling` overrides resolving
   against the theme.

Step 5 is replaced by:

5. **Stops, counted flicks and their feedback** (items 3 to 6).
   **Done when:** the classifier passes a 150 ms, 200 px, 2000 px/s
   gesture as a flick and rejects one of 300 ms, of 400 px, of 400 px/s,
   or with a rest; `Boundary` lands on the first boundary past the
   natural landing point in both directions; `Count` passes the table's
   counts for one finger and two at each speed in it, and the same flick
   passes the same count over sections of 10 px and of 10 000 px; a
   non-flick fling under `Count` is a distance fling; placeholders count
   as one each; a target growing, removed, split or merged mid-flight
   lands as item 4 says; flicks adding up within 400 ms, capped at 48; the
   flight's length by `duration(n)`; the label showing the target from
   release to landing, then the top section; `skip_count` counting down;
   `FlingEvent`s in order.

Step 6's **Done when:** adds: the label's parts at a month, week and day
boundary and just after a coarser one (the stale finer part dropped); the
default join and an app formatter; one pill per bookmarked section of the
determined granularity, its text from the formatter with
`LabelPurpose::Pill`; a month start among weeks drawn at 1.15× and a year
start at 1.3225×, never above 1.6×; under crowding, week pills giving way
before month pills and month pills before year pills; a run of 7
placeholder months drawing only its year pills, or one pill when it holds
no year start, and drawing month pills again once 2 of them load; a
counted flick over that run counting each placeholder as one;
`bookmark_levels` dropping finer pills; `AtMost(1)` counting the same
months whether or not some are split, and `Sections` counting weeks in a
split month and months elsewhere.

Step 8 is as before but at **0.12.0** and tag `v0.12.0`, its `VERSIONS.md`
entry marking **Breaking** the removal of the four `ScrollTheme` fields and
flankr's fling as the default; an app that wants 0.11.0's feel sets
`ScrollTheme { fling: FlingProfile::classic(), ..ScrollTheme::default() }`.
Everything else is additive (fields appended to `#[non_exhaustive]`
structs, new types and methods). Its **Done when:** also requires flankr's
scroll tests to pass on `FlingProfile::flankr()` when flankr takes 0.12.0.

Section 8's other bullets (the jump scroll, the jump line, pull to
refresh, the shimmer, the memory grades, the block layout) stand. Open
question 1 is answered: `two_finger_factor` 5× for dragging and distance
flings, `two_finger_count_factor` 4× for counted flicks.

## Amendment — 2026-11-19 18:34 EST: bookmarks carry their own size; libcontrols infers nothing from levels (amends the 2026-11-19 18:31 EST amendment, items 3, 7 and 9)

The owner (2026-11-19), relayed by the ShotToss coordinator: "libcontrols
doesn't know anything about months or years, just bookmarks, if size is
going to be a setting on the bookmark that's fine (could small, medium,
large) or a point size, following the libcontrols convention (usually an
enum that switched between exact and themed)". Those words are the
agreement for this amendment. It amends the 18:31 EST amendment as stated
in each item below; everything else in it stands.

### 1. A bookmark's size is the app's (replaces item 7's "Larger pills for coarser boundaries", its table, and its crowding rule)

The level-derived scale (`pill_level_scale` 1.15 per level step,
`pill_scale_max` 1.6) and those two theme values are removed. The
precedent for a value that is either exact or relative is `DrawerSize`
(`src/drawer/mod.rs`): a `#[non_exhaustive]` enum with a `Px(f32)`
variant in router pixels and a variant resolved against something else.
`BookmarkSize` follows it, its other variant resolved against the theme:

```rust
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub enum BookmarkScale { Small, Medium, Large }

#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub enum BookmarkSize {
    /// One of the theme's sizes (`ControlTheme::scrollbar`).
    Themed(BookmarkScale),
    /// The pill's font size in router pixels.
    Px(f32),
}
impl Default for BookmarkSize { fn default() -> Self { Self::Themed(BookmarkScale::Medium) } }

#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct Bookmark { pub text: BookmarkText, pub size: BookmarkSize }
impl Bookmark {
    pub fn new(text: BookmarkText) -> Self;            // Medium
    pub fn size(self, size: BookmarkSize) -> Self;
}
impl SectionedContent {
    /// Bookmark `id` with a size, replacing any bookmark it had.
    pub fn add_bookmark_with(&self, id: SectionId, bookmark: Bookmark);
    /// Every bookmark with its size, in section order.
    pub fn bookmarks_with(&self) -> Vec<(SectionId, Bookmark)>;
}
```

`add_bookmark(id, text)` is unchanged and adds a `Medium` bookmark;
`remove_bookmark` and `bookmarks` are unchanged and return the text.
Appended to `ScrollbarTheme` (spec 8.5, with `ScrollbarOptions` overrides
and `ScrollbarResolved` values):

| Field | Default | Bounds | Meaning |
|---|---|---|---|
| `bookmark_scale_small` | 0.85 | 0.5–1.0 | `Small`, times `bookmark_font_size` |
| `bookmark_scale_large` | 1.30 | 1.0–2.5 | `Large`, times `bookmark_font_size` |

`Medium` is `bookmark_font_size` itself (13), so an app that sets no size
draws exactly as 0.11.0. A pill's font size is `bookmark_font_size` times
its scale, or `Px`; its padding and height are scaled by the same ratio to
`bookmark_font_size`. Additive: a new type, a new struct, two new methods
and two appended theme fields.

**Crowding by size.** Pills are placed largest resolved font size first,
then, among equal sizes, nearest the thumb first, each only where it
clears those already placed by `bookmark_gap`; smaller pills give way
first. Size was chosen over a separate priority so that one setting the
app makes decides both how a pill looks and which pills survive; a
priority field can be appended to `Bookmark` later if an app needs a small
pill to win.

### 2. Pills are the bookmarks the app set (replaces item 7's "Pills while the thumb is held follow the same granularity", "Collapsed runs of placeholders" and the `bookmark_levels` paragraph)

A pill is drawn for each bookmark, with the bookmark's text, as in 0.11.0.
Which sections carry bookmarks, at what size and with what text is the
app's decision; libcontrols reads no level to choose them.

- `LabelPurpose::Pill` is removed: a pill's text is its bookmark's text.
- `collapse_placeholders_after` is removed. Which bookmarks a long run of
  placeholder sections should keep is meaning only the app has; the app
  sets and removes bookmarks as its data arrives (it already edits the
  list on each batch). Example, ShotToss's use only: over a long run of
  unloaded months it bookmarks only each year start, `Large`, and adds the
  month bookmarks as the months load.
- `ScrollbarOptions::bookmark_levels` is removed: an app that wants fewer
  pills sets fewer bookmarks.

### 3. The label is the app's (replaces item 7's gathering of parts by level and `LabelParts`)

libcontrols no longer gathers one part per level or drops "stale finer
parts"; that rule assumed a hierarchy only the app knows. The formatter is
told only where the label is for:

```rust
#[non_exhaustive]
pub struct LabelRequest {
    pub section: SectionId,       // the section the label is for
    pub index: usize,             // its place in the list
    pub purpose: LabelPurpose,    // Top or Target, #[non_exhaustive]
}
impl SectionedContent {
    pub fn set_label_format(
        &self,
        f: Option<Arc<dyn Fn(&LabelRequest) -> BookmarkText + Send + Sync>>,
    );
}
```

`section` is the section at the window's top while scrolling (`Top`), or
the counted flight's target (`Target`). Without a formatter the label is
the bookmark of the last bookmarked section at or above that section, as
section 5 says; the label is drawn at `Medium` size unless
`ScrollbarOptions` overrides it. Example, ShotToss's use only: its
formatter returns "November 2026 · week 3" for a week section, from its
own catalogue.

### 4. Levels are an ordering the app assigns (amends items 3 and 8)

`Section::level` stays, as a generic number the app gives each section's
top boundary: 0 the most significant, higher less. libcontrols uses it in
one place, `Boundaries::AtMost(n)`, which counts the tops whose level is at
most n; it attaches no other meaning to it and draws nothing from it.
Item 3's reason for offering no exact `Level(n)` is restated without
dates: an exact-level count would skip every top where a more significant
boundary starts at the same place. Item 8's text stands; its example of a
split range is ShotToss's use, not libcontrols' behaviour.
`Boundaries::Sections` (every top) and `Bookmarked` read no level.

### 5. Steps (amends item 9)

Step 6's added **Done when:** is replaced by: the label from a formatter
with `Top` and `Target`, and without one the last bookmark at or above; a
pill per bookmark with its own text; `Small`, `Medium` and `Large` drawn
at 0.85×, 1× and 1.3× of `bookmark_font_size`, and `Px(20.0)` at 20 px
with padding scaled to match; `Medium` and `add_bookmark` drawing exactly
as 0.11.0; under crowding, smaller pills giving way before larger ones,
and nearest first among equals; `AtMost(n)` counting the same tops
whether or not other sections are split, and `Sections` counting every
top. Step 1's and step 5's additions stand. The version stays 0.12.0; this
amendment's changes are additive over the 18:31 EST amendment.
