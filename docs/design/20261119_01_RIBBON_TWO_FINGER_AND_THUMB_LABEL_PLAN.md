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
