//! Sectioned content: an ordered list of sections, each with a stable
//! [`SectionId`] and a height, the bookmarks added on them, and the drawing
//! cache a scroll pane draws them from
//! (docs/design/20261028_08_SCROLLBAR_PLAN.md, decisions 7-11).
//!
//! Without a source a [`SectionedContent`] is an index: a
//! [`Scrollbar`](crate::Scrollbar) reads its sections' tops to place
//! bookmarks and to name the section and bookmark of a scrub. With a
//! [`SectionSource`] and [`ScrollPane::with_sections`](crate::ScrollPane::with_sections)
//! the pane draws it: it asks the source for the sections in or near its
//! window, keeps each [`DrawList`] per id until it is invalidated or goes
//! stale, and places it with libmsdf's `DrawList::append_scoped`.

use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::sync::{Arc, Mutex};

use libmsdf::drawlist::DrawList;

use crate::theme::ControlTheme;
use crate::traits::DrawContext;

/// The section index id: one name for a section, for its bookmark, a scrub
/// and a scroll. Stable for as long as the section means the same content.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SectionId(pub u64);

/// One section: its id and its length along the scroller's axis.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct Section {
    pub id: SectionId,
    /// Length along the axis, in router pixels.
    pub height: f32,
    /// Whether `height` is an estimate.
    pub estimated: bool,
}

impl Section {
    /// A section of an exact `height` (negative counts as 0).
    pub fn new(id: SectionId, height: f32) -> Self {
        Self {
            id,
            height: height.max(0.0),
            estimated: false,
        }
    }

    /// Mark the height as an estimate (or not).
    #[must_use]
    pub fn estimated(mut self, on: bool) -> Self {
        self.estimated = on;
        self
    }
}

/// A bookmark's text.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum BookmarkText {
    /// Plain text.
    Text(String),
    /// Inline Markdown (bold, italic, code). With the `markdown` feature it
    /// is drawn as spans, its markers gone; without it, as plain text,
    /// exactly as written (the Scrollbar plan's decision 6).
    Markdown(String),
}

impl BookmarkText {
    /// The string as written.
    pub(crate) fn plain(&self) -> &str {
        match self {
            Self::Text(s) | Self::Markdown(s) => s,
        }
    }
}

/// What a section is drawn with: its id, its place in the list, the width
/// it is drawn at, and the context (atlas, shaper, theme) of the frame.
#[non_exhaustive]
pub struct SectionContext<'a> {
    id: SectionId,
    index: usize,
    width: f32,
    height: f32,
    draw: &'a DrawContext<'a>,
    reported: Cell<Option<f32>>,
}

impl fmt::Debug for SectionContext<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SectionContext")
            .field("id", &self.id)
            .field("index", &self.index)
            .field("width", &self.width)
            .field("height", &self.height)
            .field("reported", &self.reported.get())
            .finish()
    }
}

impl<'a> SectionContext<'a> {
    pub(crate) fn new(
        id: SectionId,
        index: usize,
        width: f32,
        height: f32,
        draw: &'a DrawContext<'a>,
    ) -> Self {
        Self {
            id,
            index,
            width,
            height,
            draw,
            reported: Cell::new(None),
        }
    }

    /// The section being drawn.
    pub fn id(&self) -> SectionId {
        self.id
    }

    /// Its place in the list.
    pub fn index(&self) -> usize {
        self.index
    }

    /// The width to draw at: the viewport's width.
    pub fn width(&self) -> f32 {
        self.width
    }

    /// Its height as the list has it.
    pub fn height(&self) -> f32 {
        self.height
    }

    /// The frame's drawing context: atlas, shaper, theme.
    pub fn draw(&self) -> &DrawContext<'a> {
        self.draw
    }

    /// The section's real height, measured while drawing it. When it
    /// differs from [`height`](Self::height) by more than 0.5 the list takes
    /// it and the estimate flag clears (decision 10). The last report wins.
    pub fn report_height(&self, h: f32) {
        if h.is_finite() {
            self.reported.set(Some(h.max(0.0)));
        }
    }
}

/// The caller's drawing of its sections.
///
/// A source is state the app also holds (the parsed file, the transcript).
/// Any `Fn(SectionId, &SectionContext) -> DrawList` is a source too.
pub trait SectionSource {
    /// Section `id` in section coordinates: its top-left at `[0, 0]`,
    /// `cx.width()` wide and `cx.height()` tall. It may push clips of its
    /// own, and may report its real height with
    /// [`SectionContext::report_height`]. Every scope it opens must be
    /// closed before it returns.
    fn render_section(&self, id: SectionId, cx: &SectionContext) -> DrawList;
}

impl<F: Fn(SectionId, &SectionContext) -> DrawList> SectionSource for F {
    fn render_section(&self, id: SectionId, cx: &SectionContext) -> DrawList {
        self(id, cx)
    }
}

/// One kept drawing.
struct CacheEntry {
    list: DrawList,
    /// The width-and-theme generation it was drawn for.
    generation: u64,
    /// Invalidated while it was being drawn: drawn this frame, drawn again
    /// on the next.
    stale: bool,
}

/// The drawing cache's numbers (decision 7).
#[derive(Copy, Clone, Debug)]
struct CacheSettings {
    prefetch: f32,
    prefetch_per_frame: usize,
    keep: f32,
    max_cached: usize,
}

impl Default for CacheSettings {
    fn default() -> Self {
        Self {
            prefetch: 1.0,
            prefetch_per_frame: 2,
            keep: 3.0,
            max_cached: 128,
        }
    }
}

/// A section to draw: what the pane asks the source for, outside the lock.
pub(crate) struct Job {
    pub(crate) id: SectionId,
    index: usize,
    height: f32,
    /// Outside the window: a prefetch.
    pub(crate) ahead: bool,
}

#[derive(Default)]
struct Inner {
    sections: Vec<Section>,
    /// `tops[i]` is the offset of `sections[i]`; one more entry, the total.
    tops: Vec<f32>,
    index: HashMap<SectionId, usize>,
    bookmarks: HashMap<SectionId, BookmarkText>,
    source: Option<Arc<dyn SectionSource + Send + Sync>>,
    cache: HashMap<SectionId, CacheEntry>,
    settings: CacheSettings,
    /// The width and theme the cache is drawn for; their generation.
    drawn_for: Option<(f32, ControlTheme)>,
    generation: u64,
    /// Ids being drawn now, and those of them invalidated meanwhile.
    rendering: HashSet<SectionId>,
    invalidated_while_rendering: HashSet<SectionId>,
}

impl Inner {
    /// Drop the cache entries of ids no longer in the list.
    fn prune_cache(&mut self) {
        let index = &self.index;
        self.cache.retain(|id, _| index.contains_key(id));
    }

    /// An entry drawn for this width and theme (fresh, or invalidated
    /// while it was drawn).
    fn usable(&self, id: SectionId) -> bool {
        self.cache
            .get(&id)
            .is_some_and(|e| e.generation == self.generation)
    }

    /// An entry that needs no drawing again.
    fn fresh(&self, id: SectionId) -> bool {
        self.cache
            .get(&id)
            .is_some_and(|e| e.generation == self.generation && !e.stale)
    }

    /// The indices of the sections overlapping `start..end`.
    fn overlapping(&self, start: f32, end: f32) -> std::ops::Range<usize> {
        let n = self.sections.len();
        // The first whose bottom is past `start`.
        let first = self.tops[1..=n].partition_point(|&b| b <= start);
        // The first whose top is at or past `end`.
        let last = self.tops[..n].partition_point(|&t| t < end);
        first..last.max(first)
    }

    /// How far section `i` is from `start..end`: 0 when it overlaps.
    fn distance(&self, i: usize, start: f32, end: f32) -> f32 {
        let (top, bottom) = (self.tops[i], self.tops[i + 1]);
        if bottom <= start {
            start - bottom
        } else if top >= end {
            top - end
        } else {
            0.0
        }
    }

    fn rebuild(&mut self) {
        self.tops.clear();
        self.index.clear();
        let mut at = 0.0;
        for (i, s) in self.sections.iter().enumerate() {
            self.tops.push(at);
            self.index.entry(s.id).or_insert(i);
            at += s.height;
        }
        self.tops.push(at);
    }

    fn total(&self) -> f32 {
        self.tops.last().copied().unwrap_or(0.0)
    }

    fn offset_of(&self, id: SectionId) -> Option<f32> {
        self.index.get(&id).map(|&i| self.tops[i])
    }

    fn section_at(&self, offset: f32) -> Option<SectionId> {
        if self.sections.is_empty() || !(0.0..self.total()).contains(&offset) {
            return None;
        }
        // The last section whose top is at or before the offset.
        let i = self.tops[..self.sections.len()].partition_point(|&t| t <= offset);
        // Empty sections share their top with the next one; the one holding
        // the offset is the last of them.
        Some(self.sections[i.saturating_sub(1)].id)
    }
}

/// Sections, their bookmarks and their drawing cache, shared (`Arc`)
/// between the app, its panes and its scrollbars. Bookmarks are kept by id,
/// independent of the list: one added before its section exists is shown
/// once a section with that id is there.
pub struct SectionedContent {
    inner: Mutex<Inner>,
}

impl fmt::Debug for SectionedContent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.with(|s| {
            f.debug_struct("SectionedContent")
                .field("sections", &s.sections.len())
                .field("total_height", &s.total())
                .field("bookmarks", &s.bookmarks.len())
                .field("has_source", &s.source.is_some())
                .field("cached", &s.cache.len())
                .finish()
        })
    }
}

impl SectionedContent {
    /// An empty index: no source, so a pane given it draws nothing of it.
    pub fn new() -> Arc<Self> {
        let mut inner = Inner::default();
        inner.rebuild();
        Arc::new(Self {
            inner: Mutex::new(inner),
        })
    }

    /// Empty, drawn by `source` on a pane
    /// ([`ScrollPane::with_sections`](crate::ScrollPane::with_sections)).
    pub fn with_source(source: Arc<dyn SectionSource + Send + Sync>) -> Arc<Self> {
        let content = Self::new();
        content.set_source(Some(source));
        content
    }

    /// Change the source, or (`None`) make it an index again. Every kept
    /// drawing is dropped.
    pub fn set_source(&self, source: Option<Arc<dyn SectionSource + Send + Sync>>) {
        self.with(|s| {
            s.source = source;
            s.cache.clear();
            s.invalidated_while_rendering
                .extend(s.rendering.iter().copied());
        });
    }

    fn with<R>(&self, f: impl FnOnce(&mut Inner) -> R) -> R {
        f(&mut self.inner.lock().unwrap_or_else(|e| e.into_inner()))
    }

    fn edit<R>(&self, f: impl FnOnce(&mut Inner) -> R) -> R {
        self.with(|s| {
            let r = f(s);
            s.rebuild();
            s.prune_cache();
            r
        })
    }

    /// Replace the list. Bookmarks are kept, and so are the kept drawings
    /// of ids still present: invalidate those whose content changed.
    pub fn set_sections(&self, sections: Vec<Section>) {
        self.edit(|s| s.sections = sections);
    }

    /// Add a section at the end.
    pub fn push(&self, section: Section) {
        self.edit(|s| s.sections.push(section));
    }

    /// Insert a section at `index` (clamped to the length).
    pub fn insert(&self, index: usize, section: Section) {
        self.edit(|s| {
            let at = index.min(s.sections.len());
            s.sections.insert(at, section);
        });
    }

    /// Remove the section `id`, returning it. Its bookmark is kept; its kept
    /// drawing is dropped.
    pub fn remove(&self, id: SectionId) -> Option<Section> {
        self.edit(|s| {
            let i = s.index.get(&id).copied()?;
            Some(s.sections.remove(i))
        })
    }

    /// Set a section's height (negative counts as 0), clearing its estimate
    /// flag. Returns whether the section exists. Its kept drawing stays.
    pub fn set_height(&self, id: SectionId, height: f32) -> bool {
        self.edit(|s| {
            let Some(&i) = s.index.get(&id) else {
                return false;
            };
            s.sections[i].height = height.max(0.0);
            s.sections[i].estimated = false;
            true
        })
    }

    /// How many sections there are.
    pub fn len(&self) -> usize {
        self.with(|s| s.sections.len())
    }

    /// Whether there are none.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The sections, in order.
    pub fn sections(&self) -> Vec<Section> {
        self.with(|s| s.sections.clone())
    }

    /// The sum of the heights.
    pub fn total_height(&self) -> f32 {
        self.with(|s| s.total())
    }

    /// A section's top: the heights of the sections before it.
    pub fn offset_of(&self, id: SectionId) -> Option<f32> {
        self.with(|s| s.offset_of(id))
    }

    /// The section containing `offset` (from its top, inclusive, to its
    /// bottom, exclusive); `None` before 0 or at or past the total.
    pub fn section_at(&self, offset: f32) -> Option<SectionId> {
        self.with(|s| s.section_at(offset))
    }

    /// Drop the kept drawing of section `id`: it is drawn again when next
    /// in range. Invalidated while it is being drawn, the new drawing is
    /// kept stale and drawn again on the next frame.
    pub fn invalidate(&self, id: SectionId) {
        self.with(|s| {
            s.cache.remove(&id);
            if s.rendering.contains(&id) {
                s.invalidated_while_rendering.insert(id);
            }
        });
    }

    /// Drop every kept drawing (a new atlas, a reloaded file).
    pub fn invalidate_all(&self) {
        self.with(|s| {
            s.cache.clear();
            s.invalidated_while_rendering
                .extend(s.rendering.iter().copied());
        });
    }

    /// Whether section `id` has a kept drawing that the next frame at the
    /// last drawn width and theme would use without drawing it again.
    pub fn is_cached(&self, id: SectionId) -> bool {
        self.with(|s| s.fresh(id))
    }

    /// Draw this many window lengths beyond the window on each side ahead
    /// of time (1.0 by default; negative counts as 0).
    pub fn set_prefetch(&self, windows: f32) {
        self.with(|s| s.settings.prefetch = windows.max(0.0));
    }

    /// Draw at most this many sections outside the window per frame (2 by
    /// default), nearest first.
    pub fn set_prefetch_per_frame(&self, n: usize) {
        self.with(|s| s.settings.prefetch_per_frame = n);
    }

    /// Drop kept drawings further than this many window lengths from the
    /// window after each frame (3.0 by default; negative counts as 0).
    pub fn set_keep(&self, windows: f32) {
        self.with(|s| s.settings.keep = windows.max(0.0));
    }

    /// Keep at most this many drawings (128 by default), the furthest from
    /// the window going first.
    pub fn set_max_cached(&self, n: usize) {
        self.with(|s| s.settings.max_cached = n);
    }

    /// Bookmark the section `id`, replacing any bookmark it had.
    pub fn add_bookmark(&self, id: SectionId, text: BookmarkText) {
        self.with(|s| {
            s.bookmarks.insert(id, text);
        });
    }

    /// Remove the section's bookmark, returning it.
    pub fn remove_bookmark(&self, id: SectionId) -> Option<BookmarkText> {
        self.with(|s| s.bookmarks.remove(&id))
    }

    /// Remove every bookmark.
    pub fn clear_bookmarks(&self) {
        self.with(|s| s.bookmarks.clear());
    }

    /// The bookmarks, in section order; those on ids with no section follow,
    /// by id.
    pub fn bookmarks(&self) -> Vec<(SectionId, BookmarkText)> {
        self.with(|s| {
            let mut all: Vec<(Option<usize>, SectionId, BookmarkText)> = s
                .bookmarks
                .iter()
                .map(|(&id, t)| (s.index.get(&id).copied(), id, t.clone()))
                .collect();
            all.sort_by(|a, b| match (a.0, b.0) {
                (Some(x), Some(y)) => x.cmp(&y),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => a.1.cmp(&b.1),
            });
            all.into_iter().map(|(_, id, t)| (id, t)).collect()
        })
    }

    /// The sections overlapping `start..end`, with their tops (a pane's
    /// anchors).
    pub(crate) fn tops_in(&self, start: f32, end: f32) -> Vec<(SectionId, f32)> {
        self.with(|s| {
            s.overlapping(start, end)
                .map(|i| (s.sections[i].id, s.tops[i]))
                .collect()
        })
    }

    /// The tops, now, of `ids` (`None` for one no longer in the list).
    pub(crate) fn tops_of(&self, ids: &[SectionId]) -> Vec<Option<f32>> {
        self.with(|s| ids.iter().map(|&id| s.offset_of(id)).collect())
    }

    /// The sections to draw for a frame at `width` and `theme` whose window
    /// is `start..start + len` along the axis: every one overlapping the
    /// window with no fresh drawing, then at most `prefetch_per_frame` less
    /// `prefetched` (those already drawn ahead this frame) more in the
    /// prefetch margin, nearest first. Those in `done` (drawn already this
    /// frame) are skipped. Marks them as being drawn. Empty without a
    /// source.
    pub(crate) fn prepare(
        &self,
        width: f32,
        theme: &ControlTheme,
        start: f32,
        len: f32,
        done: &HashSet<SectionId>,
        prefetched: usize,
    ) -> Vec<Job> {
        self.with(|s| {
            if s.source.is_none() {
                return Vec::new();
            }
            if s.drawn_for
                .as_ref()
                .is_none_or(|(w, t)| *w != width || t != theme)
            {
                s.drawn_for = Some((width, theme.clone()));
                s.generation += 1;
            }
            let end = start + len;
            let margin = s.settings.prefetch * len;
            let job = |s: &Inner, i: usize, ahead: bool| Job {
                id: s.sections[i].id,
                index: i,
                height: s.sections[i].height,
                ahead,
            };
            let wanted = |s: &Inner, i: usize| {
                let id = s.sections[i].id;
                !s.fresh(id) && !done.contains(&id)
            };
            let mut jobs: Vec<Job> = s
                .overlapping(start, end)
                .filter(|&i| wanted(s, i))
                .map(|i| job(s, i, false))
                .collect();
            let window = s.overlapping(start, end);
            let around = s.overlapping(start - margin, end + margin);
            let mut ahead: Vec<(f32, usize)> = around
                .filter(|i| !window.contains(i) && wanted(s, *i))
                .map(|i| (s.distance(i, start, end), i))
                .collect();
            ahead.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
            jobs.extend(
                ahead
                    .into_iter()
                    .take(s.settings.prefetch_per_frame.saturating_sub(prefetched))
                    .map(|(_, i)| job(s, i, true)),
            );
            for j in &jobs {
                s.rendering.insert(j.id);
            }
            jobs
        })
    }

    /// Draw `jobs` with the source, outside the lock, and keep the
    /// results. Returns whether a reported height changed the list.
    pub(crate) fn render(&self, jobs: &[Job], width: f32, ctx: &DrawContext) -> bool {
        let Some(source) = self.with(|s| s.source.clone()) else {
            return false;
        };
        let mut drawn = Vec::with_capacity(jobs.len());
        for j in jobs {
            let cx = SectionContext::new(j.id, j.index, width, j.height, ctx);
            let list = source.render_section(j.id, &cx);
            drawn.push((j.id, list, cx.reported.get()));
        }
        self.with(|s| {
            let mut changed = false;
            for (id, list, reported) in drawn {
                s.rendering.remove(&id);
                let stale = s.invalidated_while_rendering.remove(&id);
                let Some(&i) = s.index.get(&id) else {
                    continue;
                };
                if let Some(h) = reported
                    && (h - s.sections[i].height).abs() > 0.5
                {
                    s.sections[i].height = h;
                    s.sections[i].estimated = false;
                    changed = true;
                }
                let generation = s.generation;
                s.cache.insert(
                    id,
                    CacheEntry {
                        list,
                        generation,
                        stale,
                    },
                );
            }
            if changed {
                s.rebuild();
            }
            changed
        })
    }

    /// Append the kept drawing of every section in the drawn range (the
    /// window and its prefetch margin) to `list`, each moved to `place(top)`
    /// under the scopes open on `list`. Returns how many it drew.
    pub(crate) fn draw_into(
        &self,
        list: &mut DrawList,
        start: f32,
        len: f32,
        place: impl Fn(f32) -> [f32; 2],
    ) -> usize {
        self.with(|s| {
            if s.source.is_none() {
                return 0;
            }
            let margin = s.settings.prefetch * len;
            let mut n = 0;
            for i in s.overlapping(start - margin, start + len + margin) {
                let id = s.sections[i].id;
                if !s.usable(id) {
                    continue;
                }
                if let Some(entry) = s.cache.get(&id) {
                    list.append_scoped(&entry.list, place(s.tops[i]));
                    n += 1;
                }
            }
            n
        })
    }

    /// After a frame: drop the drawings further than `keep` windows from
    /// the window, then the furthest beyond `max_cached`.
    pub(crate) fn evict(&self, start: f32, len: f32) {
        self.with(|s| {
            let end = start + len;
            let keep = s.settings.keep * len;
            let mut held: Vec<(f32, SectionId)> = s
                .cache
                .keys()
                .filter_map(|&id| {
                    let i = *s.index.get(&id)?;
                    Some((s.distance(i, start, end), id))
                })
                .collect();
            held.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
            let max = s.settings.max_cached;
            let drop: Vec<SectionId> = held
                .iter()
                .enumerate()
                .filter(|&(k, &(d, _))| d > keep || k >= max)
                .map(|(_, &(_, id))| id)
                .collect();
            for id in drop {
                s.cache.remove(&id);
            }
            s.prune_cache();
        })
    }

    /// The bookmarks whose section exists, with that section's top.
    pub(crate) fn placed_bookmarks(&self) -> Vec<(SectionId, f32, BookmarkText)> {
        self.with(|s| {
            let mut v: Vec<(SectionId, f32, BookmarkText)> = s
                .bookmarks
                .iter()
                .filter_map(|(&id, t)| s.offset_of(id).map(|top| (id, top, t.clone())))
                .collect();
            v.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
            v
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ten() -> Arc<SectionedContent> {
        let c = SectionedContent::new();
        c.set_sections((0..10).map(|i| Section::new(SectionId(i), 400.0)).collect());
        c
    }

    #[test]
    fn tops_and_the_section_at_an_offset() {
        let c = ten();
        assert_eq!(c.total_height(), 4000.0);
        assert_eq!(c.offset_of(SectionId(4)), Some(1600.0));
        assert_eq!(c.offset_of(SectionId(99)), None);
        for i in 0..10 {
            let id = SectionId(i);
            assert_eq!(c.section_at(c.offset_of(id).unwrap()), Some(id));
        }
        assert_eq!(c.section_at(399.9), Some(SectionId(0)));
        assert_eq!(c.section_at(4000.0), None);
        assert_eq!(c.section_at(-1.0), None);
    }

    #[test]
    fn editing_the_list_moves_the_tops() {
        let c = ten();
        c.insert(0, Section::new(SectionId(100), 50.0));
        assert_eq!(c.offset_of(SectionId(0)), Some(50.0));
        assert!(c.set_height(SectionId(100), 10.0));
        assert_eq!(c.offset_of(SectionId(0)), Some(10.0));
        assert_eq!(c.remove(SectionId(100)).map(|s| s.height), Some(10.0));
        assert_eq!(c.offset_of(SectionId(0)), Some(0.0));
        assert!(!c.set_height(SectionId(100), 10.0));
        c.push(Section::new(SectionId(10), 1.0).estimated(true));
        assert_eq!(c.len(), 11);
        assert!(c.sections()[10].estimated);
    }

    #[test]
    fn the_sections_overlapping_a_span_and_their_distance() {
        let c = ten();
        c.with(|s| {
            assert_eq!(s.overlapping(0.0, 1000.0), 0..3);
            assert_eq!(s.overlapping(400.0, 800.0), 1..2);
            assert_eq!(s.overlapping(-500.0, 0.0), 0..0);
            assert_eq!(s.overlapping(3900.0, 9000.0), 9..10);
            assert_eq!(s.distance(0, 400.0, 800.0), 0.0);
            assert_eq!(s.distance(3, 400.0, 800.0), 400.0);
        });
        c.set_sections(Vec::new());
        c.with(|s| assert_eq!(s.overlapping(0.0, 100.0), 0..0));
    }

    #[test]
    fn prefetch_is_nearest_first_and_limited_per_frame() {
        let draw = |_: SectionId, _: &SectionContext| DrawList::new();
        let c = SectionedContent::with_source(Arc::new(draw));
        c.set_sections((0..30).map(|i| Section::new(SectionId(i), 100.0)).collect());
        let theme = ControlTheme::default();
        let jobs = c.prepare(400.0, &theme, 1000.0, 1000.0, &HashSet::new(), 0);
        let visible: Vec<u64> = jobs.iter().filter(|j| !j.ahead).map(|j| j.id.0).collect();
        let ahead: Vec<u64> = jobs.iter().filter(|j| j.ahead).map(|j| j.id.0).collect();
        assert_eq!(visible, (10..20).collect::<Vec<_>>());
        assert_eq!(ahead, [9, 20], "the two nearest, either side");
        let more = c.prepare(400.0, &theme, 1000.0, 1000.0, &HashSet::new(), 1);
        assert_eq!(more.iter().filter(|j| j.ahead).count(), 1);
    }

    #[test]
    fn bookmarks_replace_and_outlive_their_sections() {
        let c = ten();
        c.add_bookmark(SectionId(5), BookmarkText::Text("five".into()));
        c.add_bookmark(SectionId(5), BookmarkText::Text("FIVE".into()));
        c.add_bookmark(SectionId(1), BookmarkText::Markdown("**one**".into()));
        c.add_bookmark(SectionId(20), BookmarkText::Text("later".into()));
        let ids: Vec<_> = c.bookmarks().into_iter().map(|(id, _)| id).collect();
        assert_eq!(ids, [SectionId(1), SectionId(5), SectionId(20)]);
        assert_eq!(c.placed_bookmarks().len(), 2);
        c.set_sections(Vec::new());
        assert_eq!(c.bookmarks().len(), 3, "set_sections keeps bookmarks");
        assert_eq!(
            c.remove_bookmark(SectionId(5)),
            Some(BookmarkText::Text("FIVE".into()))
        );
        c.clear_bookmarks();
        assert!(c.bookmarks().is_empty());
    }
}
