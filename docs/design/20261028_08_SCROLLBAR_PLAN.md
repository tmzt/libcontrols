---
title: Scrollbar, sectioned content and bookmarks
date: 2026-10-28
status: In implementation
covers: >
  A `Scrollbar` control backported from flankr's agent view: a 4 px thumb
  at least 24 px long with a 4 px touch slop either side, whose band takes
  a press (on the thumb it grabs it, elsewhere the thumb comes to the
  finger) and whose drag scrubs; visibility `None`, `OnScroll` (shown
  while scrolling, held 2 s, faded over 300 ms) and `Always`; optional
  bookmarks drawn while the thumb is held as translucent pills centred
  across the content, the one nearest the thumb brighter, crowded ones
  giving way to the nearest, fading with the thumb. Sectioned content: a
  scroll pane's content given as an ordered list of sections, each with a
  stable section index id (`SectionId`) and a height, drawn by the
  caller's `render_section(id) -> DrawList` only for the sections in or
  near the window and cached per id until invalidated; a section's height
  changing keeps what is in view in place (anchoring); scrolling to a
  section by id. Bookmarks are added on a section's index id with
  `add_bookmark(id, BookmarkText)` (plain text or inline Markdown) and
  removed with `remove_bookmark(id)`; section boundaries are not
  bookmarks, and most sections carry none. Call-driven: the scrollbar is
  fed offset, content and window from a `ScrollPane`'s `ScrollState` or the
  caller's own scroller and reports scrub requests as events; it has no
  gestures beyond the thumb drag. `ScrollPane::with_scrollbar` and
  `ScrollPane::with_sections` attach both to a pane in one call each; the
  plain indicator stays the default. Theme per spec 8.5
  (`ControlTheme::scrollbar`). Needs one additive libmsdf call (a scoped
  append, libmsdf 0.6.1). Additive: libcontrols 0.10.5. Steps and tests.
sources:
  - >
    The owner (2026-10-28), verbatim: "we should backport the advanced
    scrollbar to libcontrols and use it for the file viewer, with the
    optional features like labels"; then "Yes, write the libcontrols plan
    and the binary files amendment". That answer is only to write this plan
    as a Draft; it is not an approval to build it, and the status stays
    `Draft` until the owner approves it.
  - >
    The owner (2026-10-28), verbatim, on the drawing: "it could use
    render_section returning drawlist and the section index id".
  - >
    The owner (2026-10-28), verbatim, correcting the section model: "agent
    view has more sections than just the bookends/bookmarks".
  - >
    The owner (2026-10-28), verbatim, naming the label API:
    "add_bookmark(index_id, text/markdown (enum))".
  - >
    The owner (2026-10-28), verbatim, on the Markdown drawing: "we could
    move the markdown render as well, if it doesn't have heavy callbacks";
    "but yes, it should be basic formatting only"; "feature based with
    cmark as a further feature" (the Markdown view plan, 20261028_09, takes
    these; here they decide how `BookmarkText::Markdown` is drawn).
  - >
    The agreed proposal (2026-10-28, through flankr's lead): a `Scrollbar`
    control (thumb 4 wide, at least 24 long, 4 slop either side, dragging
    scrubs, length from content and window); visibility None, OnScroll
    (2 s hold, 300 ms fade), Always; optional section labels (here
    bookmarks) drawn while the thumb is dragged as translucent pills
    centred across the content in the scrollbar's grey shades, the nearest
    brighter, overlapping ones giving way to the nearest, fading with the
    thumb, at the app's exact numbers; call-driven, fed from a
    `ScrollState` or the caller's own scroller, reporting scrub requests;
    theming per spec 8.5; `ScrollPane::with_scrollbar(ScrollbarOptions)`,
    the plain indicator staying the default; a "Later" note for flankr's
    agent view; router-level tests as the PopupMenu plan's.
  - >
    flankr's agent view, ~/src/flankr/apps/companion/src/agent.rs,
    read-only: `BAR_W` 4.0, `BAR_MIN` 24.0, `BAR_SLOP` 4.0, `BAR_ALPHA`
    0.35 (the text colour), `BAR_HOLD` 2 s, `BAR_FADE` 300 ms (lines
    1950-1962); `thumb_track` (length `window / total * window`, at least
    `BAR_MIN`, at most the window; top `v.y + u * (v.h - len)` with `u =
    offset / range`; x `v.x + v.w - (BAR_SLOP + BAR_W)`; none when
    `range <= 0.5`), `band` (the thumb's x less `BAR_SLOP`, `BAR_W + 2 *
    BAR_SLOP` wide, the window's height; none when `Disabled`),
    `bar_alpha`, `drag_thumb` (`u = (y - grab - v.y) / (v.h - len)`,
    offset `u * range`), the band press (`grab` the touch less the thumb's
    top on the thumb, else `len / 2` and an immediate scrub; letting go
    leaves the view there, no fling) (lines 4015-4110, 4396-4420,
    4471-4485), `step_bar` (shown while scrolling, hold then fade; labels
    whole while the thumb is held, then hold and fade) (4854-4877),
    `ScrollbarVisibility { Disabled, Hidden (default), Always }` (2103),
    `SectionLabel`, `LABEL_BG` 0.10, `LABEL_TEXT` 0.60, `LABEL_NEAR_BG`
    0.20, `LABEL_NEAR_TEXT` 0.90 (2075-2087), `section_labels` (each at `y
    = v.y + (top / total) * v.h`, pill `h = 13 * 1.35 + 8`, `w = text +
    2 * 12`, centred on the screen's width, text cut to the width less
    `6 * 12`; sorted by distance from the thumb's centre, placed greedily
    when clear of those placed by 2, the first placed nearest; drawn
    top-down) and `labels_alpha` (3812-3880), the drawing (5016-5040: the
    thumb a rounded box of radius `w / 2`; each pill radius `h / 2`).
  - >
    flankr's harness protocol plan,
    ~/src/flankr/docs/design/20261022_03_HARNESS_PROTOCOL_PLAN.md,
    read-only: the amendments of 2026-10-24 11:09 EDT (the proportional
    thumb, 2 s then 0.3 s, at least 24 pt, the 4 pt slop, proportional
    drag, no fling), 11:32 EDT (the band works even when invisible; the
    visibility setting Disabled, Hidden, Always) and 12:27 EDT (section
    labels at the Finished cards on the scrollbar's scale, centred, 10% and
    60%, nearest 20% and 90%, crowding, "Done · time").
  - >
    flankr's settings plan,
    ~/src/flankr/docs/design/20261024_02_SETTINGS_SCREEN_PLAN.md,
    read-only: its 11:32 EDT amendment (the Display section's Scrollbar
    `ArrowLabel`), and flankr's binary files plan,
    ~/src/flankr/docs/design/20261028_05_BINARY_FILES_PLAN.md, its
    23:00 EDT amendment (the preview's bodies in a `ScrollPane` with this
    `Scrollbar`, visibility None, On Scroll, Always; labels by body).
  - >
    libcontrols at 022668c (0.10.3): src/scroll.rs (`ScrollState`'s
    private `Inner` with `content`, `max`, `last_moved`, `shift`,
    `indicator_alpha`; `ScrollPane::with_indicator`, `draw_indicator`),
    src/router.rs (`draw_subtree` pushes the pane's clip and
    `push_translate(-offset)` around its children, `:684-704`, and draws
    the indicator after the clip ends; `advance` downcasts panes and
    routers, `:1619`; `update_pane` and its node anchors, `:1797-1851`;
    hits are found last sibling first, `:1284`), src/theme.rs
    (`ControlTheme` ends with `popup_menu`), the spec
    docs/design/20261028_02_LIBCONTROLS_SPEC.md (4.7, 6.12, 8.2, 8.5, and
    the 19:58 EDT PopupMenu amendment), tests/popup_menu.rs (router-level
    fixtures).
  - >
    libmsdf at 4de319c (0.6.0, the pinned rev): `DrawList::append_from`
    copies another list's instances with their own recorded clip,
    transform and effects tags and `debug_assert!`s that no scope is open
    in the destination (src/drawlist/mod.rs:731); `instance_clips` and
    the transform bank are private, so a kept list cannot be placed under
    an open clip or translate from outside libmsdf. `push_translate`
    composes onto the open transform (:964). `GlyphStyle { Regular, Bold,
    Italic, BoldItalic }` exists, but a `DrawContext` carries one
    `TextShaper` (libcontrols src/traits.rs:28).
related:
  - docs/design/20261023_02_SCROLL_PANE_PLAN.md (the pane this extends;
    Implemented in 0.8.0; not changed by this plan)
  - docs/design/20261028_02_LIBCONTROLS_SPEC.md (the living spec; sections
    4.7, 6.12, 8 and 8.5; amended when this is implemented)
  - docs/design/20261028_07_POPUP_MENU_RADIO_PLAN.md (Draft, 0.10.4; the
    two are independent and may land in either order)
  - ~/src/flankr/docs/design/20261028_05_BINARY_FILES_PLAN.md (the first
    user: the file preview, its 23:00 EDT amendment)
  - ~/src/flankr/docs/design/20261022_03_HARNESS_PROTOCOL_PLAN.md (the
    agent view whose scrollbar this backports; it moves onto it later)
  - docs/design/20261028_09_MARKDOWN_VIEW_PLAN.md (Draft; the `markdown`
    feature whose inline parser draws `BookmarkText::Markdown`, decision 6)
  - libmsdf (~/src/libmsdf): the scoped append this needs, by a libmsdf
    plan of its own (decision 9, open question 1)
---

# Scrollbar, sectioned content and bookmarks

## 1. What it is

flankr's agent view has a scrollbar the owner wants in libcontrols, for the
file preview first: a thin proportional thumb at the right edge that can be
dragged to scrub through the content, and, while it is held, bookmarks
(the agent view calls them section labels) laid over the content at their
places on the scrollbar's scale, so the reader sees where they are going.

Two things come with it, because the preview shows files of 20 000 lines
and the agent view long transcripts:

- **Sectioned content.** The content of a pane is an ordered list of
  sections. Each has a stable section index id, `SectionId`, and a height.
  The caller draws a section when asked, `render_section(id) -> DrawList`;
  the pane asks only for the sections in or near the window and keeps each
  list until the caller invalidates that id. Long content draws only what
  shows.
- **Bookmarks.** A bookmark is a short text added on a section's id,
  `add_bookmark(id, BookmarkText)`, and shown at that section's top. Section
  boundaries are not bookmarks: a section is the unit of drawing, and most
  sections carry no bookmark. In the agent view every card or entry is a
  section and only the Finished cards are bookmarked; in a Markdown preview
  each heading starts a bookmarked section and a long section is split
  further into unbookmarked chunks for drawing.

The section index id is the one name for a section everywhere: what a
bookmark is added on, what a scrub reports, what `scroll_to_section` takes,
and what `invalidate` names.

## 2. Decisions

1. **One `Scrollbar` control, used two ways.**
   - **On a pane:** `ScrollPane::with_scrollbar(ScrollbarOptions)` gives the
     pane a scrollbar in one call. The router feeds it from the pane's
     `ScrollState`, routes presses in its band to it, applies its scrubs to
     the pane, draws it where the indicator was drawn, and advances it. The
     indicator is then not drawn for that pane. Without the call nothing
     changes: the plain indicator stays the default.
   - **Standalone:** a `Scrollbar` node over a scroller the caller drives
     itself (a terminal, the agent view's ribbon). The caller feeds it with
     `ScrollbarState::set_metrics(offset, content, window)` and
     `set_scrolling(bool)` and reads its scrubs from `take_events`. A caller
     that is not a router at all can drive the same state with `press`,
     `drag_to` and `release`.
   It is call-driven like `Drawer` and `PopupMenu`: no gesture of its own
   beyond a press in its band and the drag that follows. Wheel, keys and
   finger scrolling stay the pane's (or the caller's).
2. **Geometry: the app's numbers, in the router's pixels.** A caller that
   works in points scales the theme, as with every other value.
   - **Thumb:** `width` 4 wide. Its length is the window's share of the
     content, `window / content * window`, at least `min_length` 24 and at
     most the window. Its top is `track.y + u * (window - length)`, with
     `u = offset / range`, `range = content - window`. Its right edge sits
     `slop` (4) in from the track's right edge, so the band ends at the
     edge. It is a rounded box of radius `width / 2`.
   - **No thumb** when `range <= 0.5` or the window is empty.
   - **Band:** from the thumb's left less `slop` to the track's right edge
     (`width + 2 * slop`, 12 wide), the track's full height. It exists
     whenever there is a thumb and the visibility is not `None`, drawn or
     not (flankr's 11:32 EDT amendment: "touching the scrollbar band should
     work as a scrollbar even when invisible").
   - **Vertical only in this release.** On a horizontal pane
     `with_scrollbar` keeps the indicator and draws no scrollbar (open
     question 3).
3. **The drag.**
   - **A press in the band** grabs the thumb. On the thumb, the grab is the
     press's y less the thumb's top. Elsewhere in the band the grab is half
     the thumb's length and the thumb scrubs to the press at once (centred
     under the finger).
   - **Moving** scrubs: `u = clamp((y - grab - track.y) / (window - length),
     0, 1)`, target offset `u * range`. A scroller's fling stops when a
     grab begins.
   - **Release** leaves the content where it is, with no fling, and starts
     the hold.
   - **Cancel** (the router's press cancelled, a layer opened) ends the
     drag as a release does.
   - **Events** (`ScrollbarEvent`, `#[non_exhaustive]`, each variant
     `#[non_exhaustive]`): `Grabbed { offset }`, `Scrub { offset, section,
     bookmark }` (every move that changes the target, and the jump of an
     off-thumb press), `Released { offset, section, bookmark }`. `section`
     is the section at the target offset's top edge and `bookmark` the
     nearest bookmark's section, when sections are given; `None`
     otherwise. On a pane the router has already applied the scrub with
     `scroll_to`; the events are for the app (a preview that loads where
     the thumb stops, as the agent view's ribbon does).
4. **Visibility** (`ScrollbarVisibility`, `#[non_exhaustive]`), the app's
   three settings:
   - `None`: no thumb and no band; presses there go to the content.
   - `OnScroll` (the default): the thumb is drawn at full opacity while
     scrolling and `hold` (2.0 s) after it stops, then fades to nothing
     over `fade` (0.3 s). The band works throughout, drawn or not.
   - `Always`: the thumb is always drawn at full opacity.
   - "Full opacity" is `alpha` 0.35 of the thumb colour (`color` `None` =
     the theme's `text_color`), as the app's thumb is the text colour at
     35%.
   - "Scrolling" is: the thumb held; on a pane, the pane's `ScrollState`
     dragged, flinging, springing, or moved by a call (its last-move time,
     as the indicator uses); standalone, `set_scrolling(true)`. Content
     growing under a sticky pane does not count, as it does not in the app.
     The hold starts again on each stop.
5. **Bookmarks drawn while the thumb is held.** The app's rules exactly,
   only where libcontrols measures text with its shaper instead of the
   app's half-em estimate:
   - **When:** whole while the thumb is held and `hold` after release, then
     fading over `fade`, whatever the visibility (`Always` keeps the thumb
     but the bookmarks still fade). Not drawn while not held otherwise. Off
     with `ScrollbarOptions::bookmarks(false)`.
   - **Where:** each bookmark's centre is at `y = track.y + (top / content)
     * window`, where `top` is its section's offset: its share of the
     content down the whole track (the app's rule; see open question 2).
     Horizontally the pill is centred on the track's width (the content's
     width, "centered on the main screen").
   - **Pill:** height `font_size * line_height + 2 * pad_y` (13 * 1.35 + 2
     * 4 = 25.55), width the text's advance plus `2 * pad_x` (12 each side),
     corner radius half the height. The text is cut to one line with "…"
     to fit `track.w - 2 * side_margin - 2 * pad_x` (24 and 12: the width
     less 72, the app's `6 * PAD`).
   - **Colour:** the bookmark colour (`bookmark_color` `None` = the
     theme's `text_color`) at alpha 0.10 for the pill and 0.60 for its
     text; the bookmark nearest the thumb at 0.20 and 0.90. Every alpha is
     multiplied by the bookmarks' fade. Nothing else is drawn: the content
     shows beneath.
   - **Crowding:** bookmarks are sorted by the distance from their centre
     to the thumb's centre. Each is placed in that order when its pill is
     clear of every pill already placed by `gap` (2) above and below;
     otherwise it is not drawn this frame. The first placed is the nearest.
     They are drawn top to bottom. As the thumb moves, the hidden ones show
     again.
   - **Drawn last:** over the content and over the thumb, inside nothing
     but the track's clip.
6. **Bookmark text is plain or inline Markdown.** `BookmarkText`
   (`#[non_exhaustive]`): `Text(String)` and `Markdown(String)`. The
   `Scrollbar` is unconditional; how a `Markdown` bookmark is drawn
   depends on the Markdown view plan
   (`docs/design/20261028_09_MARKDOWN_VIEW_PLAN.md`, Draft), which adds a
   `markdown` cargo feature with an inline-only entry point for bookmark
   pills:
   - **without the `markdown` feature** (or before that plan is built),
     a `Markdown` bookmark is drawn as plain text, exactly as written,
     like `Text`;
   - **with `markdown`**, its text goes through that plan's inline parser
     (bold, italic, code only; anything else literal) and is drawn in the
     pill as spans, the markers removed and the cut to one line counting
     the visible text. With faces given
     (`ScrollbarState::set_faces(Option<Arc<MarkdownFaces>>)`, which exists
     only with the feature), bold, italic and code use those faces. Without
     faces, every span uses the context's face: bold at the nearest
     bookmark's text alpha (0.90) instead of 0.60, italic as regular, and
     code over a rounded chip at the pill's alpha again (2 px padding,
     radius 3).
   - **Why not pre-rendered spans from the caller:** the pill's measuring
     and cutting must be libcontrols' own, and a Markdown string lets the
     drawing improve (faces, the feature) with no change to the caller.
   This plan builds the plain-text path. The span drawing is built by
   whichever of the two plans lands second.
7. **Sections: the caller draws, the pane culls and caches.**
   - **`SectionId(pub u64)`**, the section index id: stable for as long as
     the section means the same content. A preview uses ordinals; the agent
     view its outline slots' serials. Two sections in one list never share
     an id.
   - **`Section`** (`#[non_exhaustive]`, built with `Section::new(id,
     height)` and `.estimated(bool)`): an id and a height along the pane's
     axis, and whether that height is an estimate. There is no label field:
     bookmarks are added on the id (decision 8).
   - **`SectionSource`**, the caller's drawing, is a trait with one
     required method, `fn render_section(&self, id: SectionId, cx:
     &SectionContext) -> DrawList`. A blanket impl makes any `Fn(SectionId,
     &SectionContext) -> DrawList` a source, so a closure works too.
     **Why a trait:** a source is state (the parsed file, the transcript)
     the app also holds and changes; a trait names the contract and can
     later gain provided methods (say, a measure without drawing) as a
     patch release; the blanket impl keeps the closure's convenience.
   - **The list** a section's `render_section` returns is in section
     coordinates: its top-left at `[0, 0]`, its width `cx.width()` (the
     viewport's width), its height the section's. The source may push
     clips of its own inside it. It may report the section's real height
     with `cx.report_height(h)` (decision 10).
   - **`SectionedContent`** (shared `Arc`, a `Mutex` inside, like
     `ScrollState`) holds the list of sections, their tops (prefix sums,
     recomputed when a height changes), an id-to-index map, the bookmarks,
     and the cache: one `DrawList` per id with the width and the theme it
     was drawn with. `SectionedContent::new()` is an index only (the
     standalone scrollbar's use, the agent view's later);
     `.with_source(Arc<dyn SectionSource>)` lets a pane draw it.
   - **`ScrollPane::with_sections(Arc<SectionedContent>)`.** The pane's
     content length is the sections' total height, or the furthest child's
     edge when that is further (children may still be added, drawn over the
     sections), unless `with_content_size` gives it.
   - **Which sections are drawn:** those overlapping the window, extended
     by `prefetch` (default 1.0 window lengths) on each side. Every visible
     section with no fresh cache entry is rendered in the frame it shows.
     At most `prefetch_per_frame` (default 2) sections outside the window
     are rendered per frame, nearest first, so a fast scrub does not stall
     on prefetch.
   - **Eviction:** after a frame, entries further than `keep` (default 3.0
     window lengths) from the window are dropped, and beyond `max_cached`
     (default 128) entries the furthest go first.
   - **Freshness:** an entry is used when its width and theme equal the
     frame's (`DrawContext.theme` compared with `PartialEq`). A width or
     theme change makes every entry stale; stale entries are re-rendered as
     they come into the drawn range.
   - **Placement:** for each drawn section the router, inside the pane's
     clip and translate, pushes `push_translate(vp.x, vp.y + top)` and
     appends the kept list with libmsdf's scoped append (decision 9), then
     ends the translate. Sections are drawn before the pane's children.
   - **No lock across the call:** the router collects the ids to render,
     releases the content's lock, calls `render_section`, then stores the
     results. A source may call `invalidate` from inside `render_section`;
     an id invalidated while being rendered is stored stale and rendered
     again next frame.
8. **Bookmarks are added on a section's id.**
   - `add_bookmark(id, text: BookmarkText)` on `SectionedContent`. A
     section has at most one bookmark: adding again on the same id
     **replaces** it.
   - `remove_bookmark(id) -> Option<BookmarkText>` returns the one removed;
     `clear_bookmarks()`; `bookmarks() -> Vec<(SectionId, BookmarkText)>`
     in section order.
   - Bookmarks are kept by id, independent of the section list: one may be
     added before its section is set, and is drawn only while a section
     with that id exists, at that section's top. `set_sections` does not
     drop them; `remove_bookmark` and `clear_bookmarks` do.
   - **Raw offset marks are not kept.** Every bookmark names a section, so a
     scrub can report it and an invalidation or a scroll can name it. A
     caller with its own scroller (the agent view) gives a
     `SectionedContent` used as an index, sections with heights and no
     source, to `ScrollbarState::with_sections`, and the bookmarks' places
     come from those sections' tops (its tops must add up to the same
     content length it gives `set_metrics`).
9. **libmsdf: a scoped append (libmsdf 0.6.1, by its own plan).** A kept
   `DrawList` must be placed under the pane's open clip and translate.
   libmsdf's `append_from` keeps the source's own tags and asserts that no
   scope is open, and the tags are private, so libcontrols cannot do it.
   libcontrols needs one additive call, here called
   `DrawList::append_scoped(&mut self, other: &DrawList)`: append `other`
   as if each of its instances were pushed now, under the open scopes. Its
   transform is the open transform composed with its own, its clip the
   open clip intersected with its own (moved by the open translation when
   bounded), its effects the open ones where it has none. The banks are
   rebased as `append_from` rebases them. It is a libmsdf patch (0.6.0 →
   0.6.1). libcontrols moving its pin to it is a semver-compatible move, so
   Rule 5.5 does not make this release breaking. flankr must pin the same
   libmsdf rev in the same move, because two git revs are two crates and a
   `DrawList` from one is not the other's. Until it exists, steps 1-5 (the
   scrollbar) can land and steps 6-7 (sections) wait (open question 1).
10. **Heights, measuring and anchoring.**
    - **Given:** each `Section` carries its height. An exact height (laid
      out by the caller) is `estimated(false)`, the default. An estimate (a
      line count times a line height, the agent view's outline estimates)
      is `estimated(true)`.
    - **Measured on render:** `render_section` may call
      `cx.report_height(h)`. When `h` differs from the section's height by
      more than 0.5, the height becomes `h` and the estimate flag clears. A
      height may also be set from outside with `set_height(id, h)`.
    - **Anchoring, by section:** `update_pane` keeps, as it keeps node
      anchors, the first section overlapping the window and its top. On the
      next update, when that section's top has moved by `d` because
      sections before it changed height (or were inserted or removed), the
      offset shifts by `d`, so what is in view stays where it is. This
      happens under the same conditions as node anchoring: `with_anchoring`
      on, not following a sticky edge, not dragged, offset above 0. A change
      to the anchor section itself or anything after it moves nothing. A
      removed anchor section falls back to the next overlapping one.
    - **While the thumb is held**, anchoring waits until release, so
      estimates corrected under a scrub do not move the content against
      the thumb. Each move scrubs against the current total.
    - **Editing the list:** `set_sections(Vec<Section>)` replaces it; it
      keeps the cache entries of ids still present, so a caller that
      changed a section's content also invalidates it. `push(Section)`,
      `insert(index, Section)` and `remove(id)` edit it in place (the agent
      view's new entries). `invalidate(id)` drops one entry;
      `invalidate_all()` drops all (a new atlas, a reloaded file).
11. **Scrolling to a section.** `SectionedContent::offset_of(id) ->
    Option<f32>` gives a section's top, and `section_at(offset) ->
    Option<SectionId>` the section containing an offset (binary search on
    the tops). `Router::scroll_to_section(pane, id) -> bool` scrolls the
    pane so the section's top is at the window's top (clamped to the range,
    stopping a fling, as `scroll_to`). It returns `false` when the pane has
    no sections or no such id.
12. **Theme per spec 8.5.** `ControlTheme` gains `scrollbar: ScrollbarTheme`
    (appended after `popup_menu`). `ScrollbarOptions` holds an `Option` per
    theme value with a setter each, plus the per-instance `visibility` and
    `bookmarks` settings. `ScrollbarOptions::with_theme(&ScrollbarTheme) ->
    ScrollbarResolved` resolves them; the control keeps its options and
    resolves against the theme it is drawn with. The section cache's
    numbers (`prefetch`, `prefetch_per_frame`, `keep`, `max_cached`) are
    per-instance settings on `SectionedContent`, not theme values.
13. **Version.** All additive (section 7): 0.10.5.

## 3. Interfaces

New module `src/scrollbar/` (`mod.rs`, `state.rs`, `bookmarks.rs`) and
`src/sections.rs`. Everything public is re-exported from
the crate root.

```rust
// ---- src/sections.rs ----

/// The section index id: one name for a section, for its drawing, its
/// bookmark, a scrub and a scroll.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SectionId(pub u64);

#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct Section {
    pub id: SectionId,
    /// Length along the pane's axis, in router pixels.
    pub height: f32,
    /// Whether `height` is an estimate a render may correct.
    pub estimated: bool,
}
impl Section {
    pub fn new(id: SectionId, height: f32) -> Self; // estimated: false
    #[must_use] pub fn estimated(self, on: bool) -> Self;
}

/// What a section is drawn with.
#[non_exhaustive]
pub struct SectionContext<'a> { /* private fields */ }
impl<'a> SectionContext<'a> {
    pub fn id(&self) -> SectionId;
    pub fn index(&self) -> usize;         // its place in the list
    pub fn width(&self) -> f32;           // the viewport's width
    pub fn height(&self) -> f32;          // as the list has it
    pub fn draw(&self) -> &DrawContext<'a>; // atlas, shaper, theme
    pub fn report_height(&self, h: f32);  // decision 10
}

/// The caller's drawing of its sections.
pub trait SectionSource {
    /// Section `id` in section coordinates: top-left [0, 0], `cx.width()`
    /// wide.
    fn render_section(&self, id: SectionId, cx: &SectionContext) -> DrawList;
}
impl<F: Fn(SectionId, &SectionContext) -> DrawList> SectionSource for F { .. }

/// A bookmark's text.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum BookmarkText {
    Text(String),
    /// Inline Markdown: bold, italic, code with the `markdown` feature;
    /// plain text without it (decision 6).
    Markdown(String),
}

/// Sections, their bookmarks and the drawing cache, shared (`Arc`)
/// between the app, its panes and its scrollbars.
pub struct SectionedContent { /* Mutex<Inner> */ }
impl SectionedContent {
    pub fn new() -> Arc<Self>;                    // an index, no drawing
    pub fn with_source(source: Arc<dyn SectionSource>) -> Arc<Self>;
    pub fn set_source(&self, source: Option<Arc<dyn SectionSource>>);
    // the list
    pub fn set_sections(&self, sections: Vec<Section>);
    pub fn push(&self, section: Section);
    pub fn insert(&self, index: usize, section: Section);
    pub fn remove(&self, id: SectionId) -> Option<Section>;
    pub fn set_height(&self, id: SectionId, height: f32) -> bool;
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
    pub fn sections(&self) -> Vec<Section>;
    pub fn total_height(&self) -> f32;
    pub fn offset_of(&self, id: SectionId) -> Option<f32>;
    pub fn section_at(&self, offset: f32) -> Option<SectionId>;
    // drawing cache
    pub fn invalidate(&self, id: SectionId);
    pub fn invalidate_all(&self);
    pub fn is_cached(&self, id: SectionId) -> bool;
    pub fn set_prefetch(&self, windows: f32);        // 1.0
    pub fn set_prefetch_per_frame(&self, n: usize);  // 2
    pub fn set_keep(&self, windows: f32);            // 3.0
    pub fn set_max_cached(&self, n: usize);          // 128
    // bookmarks (decision 8)
    pub fn add_bookmark(&self, id: SectionId, text: BookmarkText); // replaces
    pub fn remove_bookmark(&self, id: SectionId) -> Option<BookmarkText>;
    pub fn clear_bookmarks(&self);
    pub fn bookmarks(&self) -> Vec<(SectionId, BookmarkText)>;
}

// ---- src/scrollbar/ ----

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum ScrollbarVisibility {
    /// No thumb, no band.
    None,
    /// While scrolling and `hold` after, then a `fade`; the band always.
    #[default]
    OnScroll,
    /// The thumb always; the band always.
    Always,
}

#[derive(Copy, Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct ScrollbarTheme {
    pub width: f32,                          // 4.0
    pub min_length: f32,                     // 24.0
    pub slop: f32,                           // 4.0, either side; also the inset
    pub color: Option<[f32; 4]>,             // None: theme text_color
    pub alpha: f32,                          // 0.35
    pub hold: f32,                           // 2.0 s
    pub fade: f32,                           // 0.3 s
    pub bookmark_color: Option<[f32; 4]>,    // None: theme text_color
    pub bookmark_bg_alpha: f32,              // 0.10
    pub bookmark_text_alpha: f32,            // 0.60
    pub bookmark_near_bg_alpha: f32,         // 0.20
    pub bookmark_near_text_alpha: f32,       // 0.90
    pub bookmark_font_size: f32,             // 13.0
    pub bookmark_line_height: f32,           // 1.35 (times the font size)
    pub bookmark_pad_x: f32,                 // 12.0
    pub bookmark_pad_y: f32,                 // 4.0
    pub bookmark_side_margin: f32,           // 24.0
    pub bookmark_gap: f32,                   // 2.0
    pub bookmark_code_pad: f32,              // 2.0
    pub bookmark_code_radius: f32,           // 3.0
}

#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct ScrollbarOptions {
    /* one Option per ScrollbarTheme field, same names */
    /// Per instance: `OnScroll` unless set.
    pub visibility: ScrollbarVisibility,
    /// Per instance: bookmarks drawn while held, `true` unless set.
    pub bookmarks: Option<bool>,
}
impl ScrollbarOptions {
    // setter!(x: T) for each theme field, as PopupMenuOptions
    pub fn visibility(self, v: ScrollbarVisibility) -> Self; // #[must_use]
    pub fn bookmarks(self, on: bool) -> Self;                 // #[must_use]
    pub fn with_theme(&self, theme: &ScrollbarTheme) -> ScrollbarResolved;
}

#[derive(Copy, Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct ScrollbarResolved { /* the theme's fields, resolved; the two
    colours stay Option, read against ControlTheme::text_color at draw */ }

#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum ScrollbarEvent {
    #[non_exhaustive]
    Grabbed { offset: f32 },
    #[non_exhaustive]
    Scrub { offset: f32, section: Option<SectionId>, bookmark: Option<SectionId> },
    #[non_exhaustive]
    Released { offset: f32, section: Option<SectionId>, bookmark: Option<SectionId> },
}

/// One scrollbar's state, shared (`Arc`) between its control and the app.
pub struct ScrollbarState { /* Mutex<Inner> */ }
impl ScrollbarState {
    pub fn new() -> Arc<Self>;
    pub fn with_sections(sections: Arc<SectionedContent>) -> Arc<Self>;
    pub fn set_sections(&self, sections: Option<Arc<SectionedContent>>);
    /// Standalone feed; on a pane the router feeds it.
    pub fn set_metrics(&self, offset: f32, content: f32, window: f32);
    pub fn set_scrolling(&self, scrolling: bool);
    /// Standalone clock; on a pane `Router::advance` drives it. Returns
    /// whether it still needs frames (shown or fading).
    pub fn advance(&self, dt_secs: f32) -> bool;
    pub fn take_events(&self) -> Vec<ScrollbarEvent>;
    pub fn is_held(&self) -> bool;
    pub fn alpha(&self) -> f32;            // the thumb's, 0..=resolved alpha
    pub fn bookmarks_alpha(&self) -> f32;  // 0..=1
    /// Input for a caller that is not a router, in the track's coordinates.
    pub fn press(&self, pos: [f32; 2]) -> bool; // true: in the band, grabbed
    pub fn drag_to(&self, pos: [f32; 2]);
    pub fn release(&self);
    pub fn cancel(&self);
}

/// The control: a track (the content's window on the screen) with the
/// thumb at its right edge and the bookmarks over it.
pub struct Scrollbar { /* track: Cell<Rect>, options, state */ }
impl Scrollbar {
    pub fn new(track: Rect, options: ScrollbarOptions) -> Self;
    #[must_use] pub fn with_state(self, state: Arc<ScrollbarState>) -> Self;
    pub fn state(&self) -> &Arc<ScrollbarState>;
    pub fn set_track(&self, track: Rect);
    pub fn track(&self) -> Rect;
    pub fn resolved(&self, theme: &ControlTheme) -> ScrollbarResolved;
    /// The thumb and the band on the screen, when there is a thumb.
    pub fn thumb_rect(&self, theme: &ControlTheme) -> Option<Rect>;
    pub fn band_rect(&self, theme: &ControlTheme) -> Option<Rect>;
    /// The bookmarks as drawn this frame (for tests and hit areas).
    pub fn bookmark_layout(&self, ctx: &DrawContext) -> Vec<BookmarkPill>;
}

#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct BookmarkPill {
    pub id: SectionId,
    pub rect: Rect,
    pub text: String,   // visible text, cut, markers removed
    pub nearest: bool,
}
```

`Scrollbar` is a `HostedControl`: `bounds` is the track; `hit_test` is
`Some((ControlPart::Thumb, ..))` only inside the band (so a press elsewhere
on the track falls through to the nodes beneath, the content); `Down`,
`Drag` and `Up` call `press`, `drag_to` and `release`; `claims_drag` is true
for its part; `default_flags` is `NONE` (not a Tab stop). It answers no
`ControlAction` (its events are in its state). A standalone `Scrollbar` is
added after the content it scrolls, so it is hit first (last sibling first)
and drawn over it. `Router::advance` advances every `Scrollbar` node's state
as it ticks panes.

On the pane and the router:

```rust
impl ScrollPane {
    /// A scrollbar instead of the indicator (vertical panes).
    #[must_use] pub fn with_scrollbar(self, options: ScrollbarOptions) -> Self;
    /// Its content as sections drawn by their source.
    #[must_use] pub fn with_sections(self, sections: Arc<SectionedContent>) -> Self;
    pub fn scrollbar(&self) -> Option<&Arc<ScrollbarState>>;
    pub fn sections(&self) -> Option<&Arc<SectionedContent>>;
}
impl ScrollState {
    /// The content's length along the axis, as the last range update had it.
    pub fn content_length(&self) -> f32;
}
impl Router {
    pub fn scroll_to_section(&self, pane: NodeId, id: SectionId) -> bool;
}
```

`with_scrollbar` on a pane that also has `with_sections` reads its bookmarks
from them; `ScrollPane::scrollbar()`'s state can also be given sections of
its own with `set_sections`. In the router: on a `Down`, before the children
are hit, each pane on the path from the outermost is asked whether the
point is in its scrollbar's band; the innermost that says yes takes the
press as a grab, and the children never see it. Its scrubs are applied with
the pane's `scroll_to` at the router's time (which counts as a move for
visibility). The scrollbar is drawn after the pane's clip ends, where the
indicator is drawn now, with the disabled halving applied the same way.

## 4. Steps

Tests are router-level in `tests/scrollbar.rs` and `tests/sections.rs`,
with fixtures as `tests/popup_menu.rs`'s (a `Router`, a `DrawContext`
without a shaper, so advances are `0.55 * chars * size`, and counters over
the rendered `DrawList`). The geometry has unit tests in its module.

1. **Theme, options, visibility.** `ScrollbarVisibility`, `ScrollbarTheme`
   with the defaults of section 3, `ScrollbarOptions` and setters,
   `ScrollbarResolved`, `with_theme`, `ControlTheme::scrollbar` appended
   with its default; the re-exports.
   **Done when:** a unit test resolves options with `.min_length(30.0)`
   against a theme whose `hold` is 1.0 and gets both, and the defaults
   equal the app's (4, 24, 4, 0.35, 2.0, 0.3, 0.10, 0.60, 0.20, 0.90);
   `ScrollbarOptions::default().visibility` is `OnScroll`; every existing
   test passes unchanged.
2. **The state and its geometry.** `ScrollbarState` with `set_metrics`,
   `set_scrolling`, `advance`, `press`, `drag_to`, `release`, `cancel`,
   `take_events`, `alpha`, `bookmarks_alpha`, and the thumb and band
   geometry of decision 2, the drag of decision 3 and the visibility of
   decision 4.
   **Done when:** unit tests show, for a track 1000 tall at x 0..400:
   - content 4000 and window 1000 give a thumb 250 long, 4 wide, at x
     392..396; at offset 0 its top is the track's, at 3000 its bottom is the
     track's, at 1500 it is midway;
   - content 100 000 gives the 24 minimum; content 1000.4 gives no thumb;
   - the band is x 388..400, the whole height;
   - a press on the thumb grabs at the touch and moving 75 (a quarter of the
     750 travel) scrubs to 750; a press in the band below the thumb centres
     it under the press at once with a `Scrub`; release queues `Released`
     and leaves the offset;
   - `OnScroll` is 0 before any scroll, 0.35 while scrolling and 2.0 s
     after, about half at 2.15 s, 0 at 2.3 s; `Always` is 0.35 at rest;
     `None` is 0 with no band, and `press` there returns `false`;
   - bookmarks' alpha is 1 while held, 1 until 2 s after release, 0 by
     2.3 s, in every visibility.
3. **The `Scrollbar` node.** `HostedControl` and `DrawControl` for
   `Scrollbar`: band-only hit test, mouse routing, `claims_drag`, the thumb
   drawn as a `RoundedBox` of radius 2 in the resolved colour at the
   state's alpha; `Router::advance` advancing it.
   **Done when:** router tests show a standalone `Scrollbar` added over a
   `Button` filling the track: a tap in the band does not press the button
   and queues `Grabbed`; a tap left of the band presses the button; a drag
   in the band queues `Scrub`s with offsets from step 2's rule; the thumb is
   drawn as one `RoundedBox` of size `[4, len]` with alpha `0.35 *
   text_color[3]` while held and none 2.3 s after release under
   `OnScroll`; `advance` reports busy until then and idle after.
4. **Bookmarks.** `bookmark_layout` (decision 5) and its drawing, the
   bookmarks read from the state's `SectionedContent`, `Markdown` drawn as
   plain text (decision 6).
   **Done when:** router tests, with an index-only
   `SectionedContent` of 10 sections of 400 (content 4000, window 1000) and
   bookmarks on sections 0, 4 and 5, show:
   - no pills before a press; three while held, centred on the track's
     width, each at `track.y + top / 4000 * 1000` (0, 400, 500), each 25.55
     tall with radius 12.775;
   - the pill nearest the thumb has pill alpha 0.20 and text 0.90, the
     others 0.10 and 0.60, all in `text_color`;
   - with sections 4 and 5 at 10 apart (sections of 40 near there) the
     farther of the two from the thumb is not drawn, and it shows again
     after the thumb moves past;
   - a long bookmark is cut with "…" to the width less 72;
   - a `Markdown` bookmark `"**Landed**"` is drawn as written, markers
     included, the same as `Text` with that string;
   - `add_bookmark` on an id with one replaces it (one pill),
     `remove_bookmark` returns it and the pill goes, and a bookmark on an
     id with no section draws nothing until the section is pushed;
   - `Scrub` and `Released` carry the section at the offset and the
     nearest bookmark's id;
   - pills fade with step 2's bookmark alpha and `bookmarks(false)` draws
     none.
5. **On a pane.** `ScrollPane::with_scrollbar`, `scrollbar()`,
   `ScrollState::content_length`; the router's band routing, feeding,
   scrub application, visibility from the pane's moves, drawing in the
   indicator's place.
   **Done when:** router tests over a pane of 50 buttons 48 tall (content
   2400) in a 600 viewport show: no indicator `RoundedBox` of the
   indicator's width is drawn, only the scrollbar's; a press in the band
   scrubs the pane (`ScrollState::offset` follows) and no button is
   pressed; a finger drag elsewhere scrolls as before and shows the thumb
   for 2 s; a fling keeps it shown while it runs; release after a scrub
   does not fling; a pane without `with_scrollbar` draws exactly as at
   0.10.3 (the same instances as a recorded list); `None` lets the band's
   press reach the button beneath.
6. **Sections drawn by the pane** (needs libmsdf 0.6.1, decision 9; the
   pin moves in this step's commit). `SectionId`, `Section`,
   `SectionContext`, `SectionSource` and its blanket impl,
   `SectionedContent` (list, tops, map, cache, settings),
   `ScrollPane::with_sections`, the drawn range, prefetch, eviction,
   freshness, and placement with `append_scoped`.
   **Done when:** router tests with a counting source (each render pushes
   one `Box` the section's size and counts calls by id), 2000 sections of
   100 (a 20 000-line file in 10-line sections; content 200 000) in a 1000
   viewport show:
   - the first frame renders the 10 visible sections and at most 2 more,
     and draws exactly the visible ones plus cached neighbours, each `Box`
     at its section's place within the clip;
   - a second frame renders nothing; scrolling by 100 renders at most one
     newly visible section plus prefetch;
   - `invalidate(SectionId(3))` re-renders only section 3 on the next
     frame;
   - a viewport width change or a theme change re-renders every drawn
     section once;
   - after scrolling to 100 000, no more than `max_cached` entries are held
     and none further than 3 windows from the window;
   - the pane's content length is 200 000; children added to the pane are
     drawn over the sections;
   - a closure works as a source;
   - an instance pushed by a source at `[0, 0]` lands at `[vp.x, vp.y +
     top - offset]` and carries the pane's clip.
7. **Heights, anchoring, scrolling to a section, bookmarks from a pane's
   sections.** `report_height`, `set_height`, `push`, `insert`, `remove`,
   section anchoring, `offset_of`, `section_at`,
   `Router::scroll_to_section`; a pane with both `with_sections` and
   `with_scrollbar` draws the sections' bookmarks.
   **Done when:** router tests show:
   - estimated sections of 100 whose renders report 140: scrolled to
     section 50's top, a section above growing by 40 shifts the offset by
     40 and section 50 stays at the window's top; a section below growing
     moves nothing; the anchor section growing moves nothing;
   - an `insert` above the window shifts by the inserted height, a `remove`
     above by minus it;
   - a sticky-end pane keeps following when sections are pushed;
   - while the thumb is held no anchoring shift happens, and it applies on
     release;
   - `scroll_to_section(pane, SectionId(500))` puts that section's top at
     the window's top, clamped at the end, and returns `false` for an
     unknown id or a pane without sections;
   - `section_at(offset_of(id)) == Some(id)` for every id;
   - a pane with sections and a scrollbar draws bookmarks at the sections'
     tops while its thumb is held.
8. **Spec, README, version.** The spec amendment of section 6; the README's
   row moved to the plan's new status and a "Scrollbar and sections"
   paragraph after the scroll pane's; `Cargo.toml` 0.10.5 and the libmsdf
   pin; the `VERSIONS.md` entry of section 7; the tag.
   **Done when:** `cargo fmt -p libcontrols -- --check`, `cargo clippy -p
   libcontrols --all-targets` with no new warnings and `cargo test -p
   libcontrols` pass; the lead commits and tags `v0.10.5`.

## 5. Mapping the first users onto it (for flankr's plans; not built here)

- **The file preview (flankr plan 20261028_05, its 23:00 EDT amendment)**,
  the first user. Each scrolling body is a `ScrollPane::with_sections` and
  `with_scrollbar`, and its `SectionSource` lays out and draws one section
  with flankr's own Markdown or terminal drawing. Ids are ordinals in the
  body; a reload calls `set_sections` with new ids and `invalidate_all`.
  - **Markdown:** a section starts at each `#` or `##` heading, bookmarked
    with the heading's text (`BookmarkText::Markdown`, cut to 40
    characters by the app); a leading preamble before the first heading is
    an unbookmarked section. A heading's section longer than about one
    window is split at block boundaries into further, unbookmarked
    sections for drawing.
  - **Diff and `git` output:** one section per file at its `diff --git`
    line, bookmarked with the file's name. A long file's diff is split at
    hunk boundaries into unbookmarked sections. For `git show`, the commit
    header is a first section bookmarked "commit <hash8>".
  - **Plain text:** fixed chunks of 100 lines (exact heights: lines times
    the line height), with no bookmarks; a 20 000-line file is 200
    sections.
  - **Heights:** exact where the app lays out before drawing (plain text,
    diffs); estimated and corrected by `report_height` for Markdown.
  - **Visibility:** the app's setting, mapped None → `None`, On Scroll →
    `OnScroll`, Always → `Always`; the app forces `Always` for a plain or
    diff body longer than three screens.
- **Later: flankr's agent view**, by a flankr plan or amendment of its own,
  keeping its Finished-card labels. One section per card or entry (text,
  tool call, diff, question, Finished card, the prompt card), with ids from
  the outline slots' serials and estimated heights from the outline
  (`held_newer` slots included). Only the Finished cards carry bookmarks
  ("Landed the settings screen · 12:41", "Done · 12:41" before a summary),
  so many unbookmarked sections sit between two bookmarks. It keeps its own
  scroller, two-finger speed, pull to refresh and window loading at first.
  It uses a standalone `Scrollbar` over its ribbon, fed by `set_metrics`
  and `set_scrolling`, with a `SectionedContent` as an index for the
  bookmarks' places, and loads contents on `Released`. Drawing its sections
  through `render_section` is a later step of that plan.

## 6. The spec amendment (drafted; appended when step 8 lands)

```markdown
## Amendment — <date from the clock>: Scrollbar, sectioned content and bookmarks (sections 3, 4.7, 6, 8, 10, 11)

Plan `docs/design/20261028_08_SCROLLBAR_PLAN.md`, release 0.10.5
(`v0.10.5`). This adds sections 6.17 and 6.18 and extends 3, 4.7, 6.12, 8,
10 and 11; it replaces no earlier text except the libmsdf rev in 3, the
version in 10 and the test counts in 11.

1. **3:** libmsdf is pinned at <rev> (0.6.1), for `DrawList::append_scoped`.
2. **4.7:** a pane with `with_scrollbar` takes a press in its scrollbar's
   band before its children (the innermost such pane on the path), applies
   the scrollbar's scrubs with `scroll_to`, and draws the scrollbar where
   the indicator was, the indicator then not drawn. A pane with
   `with_sections` draws, before its children, the sections overlapping
   the window and a prefetch margin, each from its `SectionedContent`'s
   cache (rendered by the source's `render_section` when missing or stale,
   placed with `push_translate` and `append_scoped`), evicts those beyond
   `keep`, takes its content length from the sections (or children, if
   further), and anchors on the first section in view. `Router::advance`
   advances `Scrollbar` nodes. `Router::scroll_to_section`.
3. **6.12:** `ScrollPane::with_scrollbar`, `with_sections`, `scrollbar`,
   `sections`; `ScrollState::content_length`.
4. **6.17 `Scrollbar` (`src/scrollbar/`):** the control, `ScrollbarState`,
   `ScrollbarVisibility`, `ScrollbarEvent`, `BookmarkPill`, with the
   geometry, drag, visibility and bookmark rules of the plan's decisions
   2-6.
5. **6.18 Sections (`src/sections.rs`):** `SectionId`, `Section`,
   `SectionContext`, `SectionSource` (and its closure impl),
   `BookmarkText`, `SectionedContent` (list, bookmarks with replace on
   re-add, cache and its settings), per the plan's decisions 7-11.
6. **8, theme:** `ControlTheme::scrollbar: ScrollbarTheme` (the plan's
   section 3 defaults), `ScrollbarOptions`, `ScrollbarResolved`, built to
   8.5.
7. **10:** the crate is 0.10.5. `ScrollbarVisibility`, `ScrollbarEvent` (and
   its variants), `ScrollbarTheme`, `ScrollbarOptions`, `ScrollbarResolved`,
   `BookmarkPill`, `BookmarkText`, `Section` and `SectionContext` are
   `#[non_exhaustive]`.
8. **11:** `tests/scrollbar.rs` and `tests/sections.rs` add <n> router
   tests.
```

## 7. Versioning

0.10.5, a patch release. Every public change is additive under the rules
and the owner's rulings:

- New types only: `Scrollbar`, `ScrollbarState`, `SectionedContent`,
  `SectionId` (a new tuple struct; nothing existing changes), and the
  `#[non_exhaustive]` `ScrollbarVisibility`, `ScrollbarEvent` (variants
  also `#[non_exhaustive]`, so fields can be added later),
  `ScrollbarTheme`, `ScrollbarOptions`, `ScrollbarResolved`, `BookmarkPill`,
  `BookmarkText`, `Section`, `SectionContext`.
- A new trait, `SectionSource`, with a blanket impl for closures. New
  provided methods later are additive. No existing type implements it.
- `ControlTheme::scrollbar` appended: additive by the owner's ruling
  (spec 8.5 item 4), though `ControlTheme` is exhaustive.
- New methods on `ScrollPane`, `ScrollState` and `Router`; no signature
  changes.
- No observable change for existing callers: a pane without
  `with_scrollbar` or `with_sections` routes, draws and anchors exactly as
  in 0.10.3 (step 5's recorded-list test). Router changes are internal and
  gated on those calls.
- The libmsdf pin moves from 0.6.0 to 0.6.1, a semver-compatible version,
  so Rule 5.5 does not apply. flankr moves its own libmsdf pin to the same
  rev with this release (decision 9).

The two Draft releases follow release order: whichever of this plan and the
radio plan (0.10.4) lands first takes 0.10.4, and the other 0.10.5. The
numbers here assume the radio plan lands first. If the sections must wait
for libmsdf (open question 1), the scrollbar (steps 1-5, 8) may ship alone
as a patch and the sections (steps 6-7) follow as the next patch.

Draft `VERSIONS.md` entry (the short commit filled in when committed):

```markdown
## 0.10.5 — <date> (<short commit>, tag `v0.10.5`)

Scrollbar, sectioned content and bookmarks
(docs/design/20261028_08_SCROLLBAR_PLAN.md). Additive: new types, a new
trait, new methods, and `ControlTheme::scrollbar` appended; panes that do
not opt in behave as before. libmsdf moves from 0.6.0 to 0.6.1
(compatible).

- Added: `Scrollbar`, `ScrollbarState`, `ScrollbarVisibility` (`None`,
  `OnScroll`, the default, `Always`), `ScrollbarEvent` (`Grabbed`, `Scrub`,
  `Released`), `BookmarkPill`.
- Added: `ScrollbarTheme`, `ScrollbarOptions` (with setters, `visibility`,
  `bookmarks`, `with_theme`), `ScrollbarResolved`;
  `ControlTheme::scrollbar`.
- Added: `SectionId`, `Section`, `SectionContext`, `SectionSource` (and its
  impl for closures), `BookmarkText` (`Text`, `Markdown`),
  `SectionedContent` (sections, `add_bookmark`/`remove_bookmark`, cache).
- Added: `ScrollPane::with_scrollbar`, `with_sections`, `scrollbar`,
  `sections`; `ScrollState::content_length`; `Router::scroll_to_section`.
```

Draft `CHANGES.md` addendum (appended, timestamped from the clock, when the
owner approves this plan and implementation starts):

```markdown
## <date from the clock> — Scrollbar, sections and bookmarks (plan 20261028_08_SCROLLBAR_PLAN)

New work: flankr's agent-view scrollbar backported as a `Scrollbar`
control, with sectioned content drawn by `render_section(id) -> DrawList`
and cached per section, and bookmarks added on a section's id, asked by
the owner ("we should backport the advanced scrollbar to libcontrols and
use it for the file viewer, with the optional features like labels") for
flankr's file preview (~/src/flankr/docs/design/20261028_05_BINARY_FILES_PLAN.md,
its 23:00 EDT amendment). Plan docs/design/20261028_08_SCROLLBAR_PLAN.md;
additive, 0.10.5 (tag `v0.10.5`); needs libmsdf 0.6.1's scoped append.
```

## 8. Open questions

1. **The libmsdf scoped append.** Who plans and releases it, under what name,
   and when? It is the only way to place a kept `DrawList` under a pane's
   clip. Without it, the sections cannot cache. Should the scrollbar ship
   alone first (steps 1-5) if it is late?
2. **Bookmarks' vertical scale.** The app places a bookmark at `top /
   content` of the whole track. The thumb's top travels `offset / range`
   of `window - length`. So scrubbing to a bookmarked section leaves the
   thumb's centre off its bookmark, by up to half a thumb length near the
   ends. This plan matches the app. Should both instead align the bookmark
   with the thumb's centre at that section's offset?
3. **Horizontal panes.** The scrollbar is vertical only here (decision 2).
   Is a horizontal thumb (at the bottom edge, no bookmarks) wanted now?
4. **`SectionSource` and threads.** The trait has no `Send + Sync` bound, so
   a `SectionedContent` with a source is `!Send` unless its source is. This
   matches single-threaded drawing. Is a bound wanted?
5. **Markdown bookmarks' look without faces.** With the `markdown` feature
   and no faces given, bold is drawn brighter and italic as regular
   (decision 6). Is that stand-in right, or should a caller without faces
   get plain text?
6. **The cache's numbers** (1 window of prefetch, 2 prefetches a frame,
   3 windows kept, 128 entries) are first guesses for a phone. They should
   be checked on the S23 with the 20 000-line case.
7. **A tap on the band off the thumb jumps** (the app's rule, decision 3).
   Should a mouse click there page instead, as desktop scrollbars do?
8. **Should a bookmark pill take a tap** (to scroll to its section) while
   the bookmarks fade after release? Not covered here, and additive later.

## Amendment — 2026-10-28 23:13 EDT: approved; steps 1-5 and 8 now, 6-7 after libmsdf's scoped append

The owner approved this plan on 2026-10-28 ("approve and build", given for
this plan and plan 20261028_09). Steps 1-5 (the scrollbar, its bookmarks,
on a pane) are built now, with step 8's spec, README and version for that
release. Steps 6 and 7 (sections drawn by the pane, heights, anchoring)
need `DrawList::append_scoped` in libmsdf (decision 9), which needs a
libmsdf plan of its own that the owner has not yet seen; they follow in a
later patch release once that is approved and released. Until then
`SectionedContent` serves only as the index of sections and bookmarks
(decision 9's fallback for a caller with its own scroller).

The version follows release order (decision of section 5): this release
is built before the radio-style plan (20261028_07, a Draft), so it is
**0.10.4**, not the drafted 0.10.5. The status stays `Draft` until the
first implementing commit, which sets `In implementation` (Rule 1.5).

## Amendment — 2026-10-28 23:25 EDT: implementation started; the document is now immutable

Implementation starts with this amendment, in the worktree
`.claude/worktrees/scrollbar` on the branch `wt-scrollbar` from
`e7a345c`: `status` is `In implementation`, and the body above is frozen
(AGENTS.md Rule 1.5). The scope is the 23:13 EDT amendment's: steps 1-5
and step 8, released as 0.10.4. Steps 6 and 7 are not built; until
libmsdf's scoped append is planned and released, `SectionedContent` is
only the index of sections and bookmarks (decision 9's fallback), with no
source, no drawing cache and no `ScrollPane::with_sections`. The open
questions of section 8 stay open for the owner; the code follows the
decisions as written. Departures are recorded in `CHANGES.md`.
