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

use crate::fling::{Boundaries, Stop, Target};
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
    /// No height of its own: counted and drawn at the content's
    /// [placeholder height](SectionedContent::set_placeholder_height)
    /// until it is given one (`height` is then unused).
    pub placeholder: bool,
    /// The level of the boundary at this section's top, 0 the most
    /// significant, at most 7. An ordering the app assigns; libcontrols
    /// reads it only for [`Boundaries::AtMost`](crate::Boundaries::AtMost).
    pub level: u8,
}

/// The highest level a section's top may have.
pub const MAX_LEVEL: u8 = 7;

impl Section {
    /// A section of an exact `height` (negative counts as 0), at level 0.
    pub fn new(id: SectionId, height: f32) -> Self {
        Self {
            id,
            height: height.max(0.0),
            estimated: false,
            placeholder: false,
            level: 0,
        }
    }

    /// A section whose height is not known and has no estimate: drawn and
    /// counted at its content's placeholder height until it is given one.
    /// It is `estimated`.
    pub fn placeholder(id: SectionId) -> Self {
        Self {
            estimated: true,
            placeholder: true,
            ..Self::new(id, 0.0)
        }
    }

    /// The level of the boundary at its top (0-7; higher counts as 7).
    #[must_use]
    pub fn level(mut self, level: u8) -> Self {
        self.level = level.min(MAX_LEVEL);
        self
    }

    /// Its length in a list whose placeholder height is `placeholder`.
    pub(crate) fn length(&self, placeholder: f32) -> f32 {
        if self.placeholder {
            placeholder
        } else {
            self.height
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

/// The default placeholder height, in router pixels.
pub const DEFAULT_PLACEHOLDER_HEIGHT: f32 = 96.0;

/// The placeholder height (a newtype for its default).
#[derive(Copy, Clone, Debug)]
struct PlaceholderHeight(f32);

impl Default for PlaceholderHeight {
    fn default() -> Self {
        Self(DEFAULT_PLACEHOLDER_HEIGHT)
    }
}

/// Where a split or merged section's place went: the section now holding
/// it and how far into that section its top was.
type Replaced = HashMap<SectionId, (SectionId, f32)>;

/// Most replacements remembered before the oldest are forgotten.
const MAX_REPLACED: usize = 4096;

/// The sections in and near a pane's window, by index into the list
/// (flankr's `ViewWindow`), reported to the app for fetching its data
/// ([`SectionedContent::take_window`]).
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
    /// The pane is following its sticky end
    /// ([`ScrollState::is_following`](crate::ScrollState::is_following)).
    pub following: bool,
}

/// The window's margins, in window lengths each side (flankr's).
#[derive(Copy, Clone, Debug)]
struct Margins {
    close: f32,
    near: f32,
    keep: f32,
}

impl Default for Margins {
    fn default() -> Self {
        Self {
            close: 1.0,
            near: 5.0,
            keep: 20.0,
        }
    }
}

#[derive(Default)]
struct Inner {
    sections: Vec<Section>,
    /// The window last worked out, the one last taken, and whether a
    /// scrollbar thumb holds the pane.
    window: Option<SectionWindow>,
    taken: Option<SectionWindow>,
    window_held: bool,
    margins: Margins,
    /// Length known past the last section, counted by a scrollbar only.
    pending: f32,
    placeholder_height: PlaceholderHeight,
    /// How many times the tops were rebuilt (for tests and counting).
    rebuilds: u64,
    /// Split and merged ids, for anchoring and flights.
    replaced: Replaced,
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
        self.rebuilds += 1;
        self.tops.clear();
        self.index.clear();
        let ph = self.placeholder_height.0;
        let mut at = 0.0;
        for (i, s) in self.sections.iter().enumerate() {
            self.tops.push(at);
            self.index.entry(s.id).or_insert(i);
            at += s.length(ph);
        }
        self.tops.push(at);
    }

    /// The inclusive index range of the sections overlapping `start..end`;
    /// when none does, the nearest one. The list is not empty.
    fn span(&self, start: f32, end: f32) -> (usize, usize) {
        let r = self.overlapping(start, end);
        if r.is_empty() {
            let k = r.start.min(self.sections.len() - 1);
            (k, k)
        } else {
            (r.start, r.end - 1)
        }
    }

    /// The window `start..start + len` and its margins.
    fn window_at(&self, start: f32, len: f32, following: bool) -> Option<SectionWindow> {
        if self.sections.is_empty() {
            return None;
        }
        let len = len.max(f32::MIN_POSITIVE);
        let end = start + len;
        let around = |m: f32| self.span(start - m * len, end + m * len);
        let shown = self.span(start, end);
        Some(SectionWindow {
            shown,
            close: around(self.margins.close),
            near: around(self.margins.near),
            keep: around(self.margins.keep),
            shown_ids: (self.sections[shown.0].id, self.sections[shown.1].id),
            following,
        })
    }

    /// The id index alone, mid-batch.
    fn reindex(&mut self) {
        self.index.clear();
        for (i, s) in self.sections.iter().enumerate() {
            self.index.entry(s.id).or_insert(i);
        }
    }

    /// Remember that `old`'s place is now `off` into `new`, carrying on
    /// any earlier replacement that led to `old`.
    fn note_replaced(&mut self, old: SectionId, new: SectionId, off: f32) {
        if old == new {
            return;
        }
        if self.replaced.len() >= MAX_REPLACED {
            self.replaced.clear();
        }
        for v in self.replaced.values_mut() {
            if v.0 == old {
                *v = (new, v.1 + off);
            }
        }
        self.replaced.insert(old, (new, off));
    }

    /// Where `id`'s top is now: its own, or for a split or merged id, the
    /// place it went to.
    fn resolve_top(&self, id: SectionId) -> Option<f32> {
        if let Some(t) = self.offset_of(id) {
            return Some(t);
        }
        let &(new, off) = self.replaced.get(&id)?;
        self.offset_of(new).map(|t| t + off)
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

/// The locked list, in a batch ([`SectionedContent::edit`]).
pub struct SectionEdit<'a> {
    inner: &'a mut Inner,
    /// The id index is behind the list: rebuilt on the next lookup.
    index_dirty: bool,
}

impl fmt::Debug for SectionEdit<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SectionEdit")
            .field("sections", &self.inner.sections.len())
            .finish()
    }
}

impl SectionEdit<'_> {
    fn lookup(&mut self, id: SectionId) -> Option<usize> {
        if self.index_dirty {
            self.inner.reindex();
            self.index_dirty = false;
        }
        self.inner.index.get(&id).copied()
    }

    /// Add a section at the end.
    pub fn push(&mut self, section: Section) {
        let (id, i) = (section.id, self.inner.sections.len());
        self.inner.sections.push(section);
        if !self.index_dirty {
            self.inner.index.entry(id).or_insert(i);
        }
    }

    /// Insert a section at `index` (clamped to the length).
    pub fn insert(&mut self, index: usize, section: Section) {
        let at = index.min(self.inner.sections.len());
        self.inner.sections.insert(at, section);
        self.index_dirty = true;
    }

    /// Remove the section `id`, returning it. Its bookmark is kept; its
    /// kept drawing is dropped.
    pub fn remove(&mut self, id: SectionId) -> Option<Section> {
        let i = self.lookup(id)?;
        self.index_dirty = true;
        Some(self.inner.sections.remove(i))
    }

    /// Set a section's height (negative counts as 0), clearing its estimate
    /// and placeholder flags. Returns whether the section exists.
    pub fn set_height(&mut self, id: SectionId, h: f32) -> bool {
        let Some(i) = self.lookup(id) else {
            return false;
        };
        let s = &mut self.inner.sections[i];
        s.height = h.max(0.0);
        s.estimated = false;
        s.placeholder = false;
        true
    }

    /// Set a section's height as an estimate (negative counts as 0): no
    /// longer a placeholder, still `estimated`.
    pub fn set_estimate(&mut self, id: SectionId, h: f32) -> bool {
        let Some(i) = self.lookup(id) else {
            return false;
        };
        let s = &mut self.inner.sections[i];
        s.height = h.max(0.0);
        s.estimated = true;
        s.placeholder = false;
        true
    }

    /// Where section `id` is in the list now.
    pub fn index_of(&mut self, id: SectionId) -> Option<usize> {
        self.lookup(id)
    }

    /// How many sections there are now.
    pub fn len(&self) -> usize {
        self.inner.sections.len()
    }

    /// Whether there are none now.
    pub fn is_empty(&self) -> bool {
        self.inner.sections.is_empty()
    }

    /// Drop a replaced id's drawing and, unless it is still in the list
    /// under `keep`, its bookmark.
    fn forget(&mut self, id: SectionId, kept: &[SectionId]) {
        self.inner.cache.remove(&id);
        if self.inner.rendering.contains(&id) {
            self.inner.invalidated_while_rendering.insert(id);
        }
        if !kept.contains(&id) {
            self.inner.bookmarks.remove(&id);
        }
    }

    /// Replace `id` with `pieces` (new ids, in order); false if absent.
    /// The replaced id loses its drawing and bookmark (add the pieces'
    /// bookmarks in the same batch). A pane anchored on it keeps its
    /// distance from the first piece's top; a counted flight's target that
    /// is split becomes the first piece.
    pub fn split(&mut self, id: SectionId, pieces: Vec<Section>) -> bool {
        let Some(i) = self.lookup(id) else {
            return false;
        };
        let kept: Vec<SectionId> = pieces.iter().map(|p| p.id).collect();
        let first = kept.first().copied();
        self.inner.sections.splice(i..=i, pieces);
        self.index_dirty = true;
        self.forget(id, &kept);
        if let Some(first) = first {
            self.inner.note_replaced(id, first, 0.0);
        }
        true
    }

    /// Replace the consecutive run `first..=last` with `merged`; false if
    /// either is absent or `last` is before `first`. The replaced ids lose
    /// their drawings and bookmarks. A pane anchored inside the run keeps
    /// its distance from the run's first top, now inside `merged`; a
    /// counted flight's target in the run becomes the merged section.
    pub fn merge(&mut self, first: SectionId, last: SectionId, merged: Section) -> bool {
        let (Some(i), Some(j)) = (self.lookup(first), self.lookup(last)) else {
            return false;
        };
        if j < i {
            return false;
        }
        let ph = self.inner.placeholder_height.0;
        let new = merged.id;
        let mut at = 0.0;
        let olds: Vec<(SectionId, f32)> = self.inner.sections[i..=j]
            .iter()
            .map(|s| {
                let o = (s.id, at);
                at += s.length(ph);
                o
            })
            .collect();
        self.inner.sections.splice(i..=j, std::iter::once(merged));
        self.index_dirty = true;
        for (id, off) in olds {
            self.forget(id, &[new]);
            self.inner.note_replaced(id, new, off);
        }
        true
    }

    /// Bookmark section `id` in this batch, replacing any bookmark it had.
    pub fn add_bookmark(&mut self, id: SectionId, text: BookmarkText) {
        self.inner.bookmarks.insert(id, text);
    }

    /// Remove section `id`'s bookmark in this batch, returning it.
    pub fn remove_bookmark(&mut self, id: SectionId) -> Option<BookmarkText> {
        self.inner.bookmarks.remove(&id)
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

    /// Edit the list in one batch: the tops and the id index are rebuilt
    /// once when `f` returns, the kept drawings of removed and replaced ids
    /// are dropped, and a pane's next range update anchors once for the
    /// whole batch. The single calls below are each a batch of one.
    ///
    /// `f` runs under the content's lock: it must not call this
    /// `SectionedContent`'s methods (use the [`SectionEdit`]'s).
    pub fn edit<R>(&self, f: impl FnOnce(&mut SectionEdit<'_>) -> R) -> R {
        self.with(|s| {
            let r = {
                let mut e = SectionEdit {
                    inner: s,
                    index_dirty: false,
                };
                f(&mut e)
            };
            s.rebuild();
            s.prune_cache();
            r
        })
    }

    /// Replace the list. Bookmarks are kept, and so are the kept drawings
    /// of ids still present: invalidate those whose content changed.
    pub fn set_sections(&self, sections: Vec<Section>) {
        self.edit(|e| e.inner.sections = sections);
    }

    /// Add a section at the end.
    pub fn push(&self, section: Section) {
        self.edit(|e| e.push(section));
    }

    /// Insert a section at `index` (clamped to the length).
    pub fn insert(&self, index: usize, section: Section) {
        self.edit(|e| e.insert(index, section));
    }

    /// Remove the section `id`, returning it. Its bookmark is kept; its kept
    /// drawing is dropped.
    pub fn remove(&self, id: SectionId) -> Option<Section> {
        self.edit(|e| e.remove(id))
    }

    /// Set a section's height (negative counts as 0), clearing its estimate
    /// and placeholder flags. Returns whether the section exists. Its kept
    /// drawing stays.
    pub fn set_height(&self, id: SectionId, height: f32) -> bool {
        self.edit(|e| e.set_height(id, height))
    }

    /// Set a section's height as an estimate: it is no longer a
    /// placeholder, and stays `estimated` until measured or set exactly.
    pub fn set_estimate(&self, id: SectionId, height: f32) -> bool {
        self.edit(|e| e.set_estimate(id, height))
    }

    /// The height a placeholder section counts as
    /// ([`DEFAULT_PLACEHOLDER_HEIGHT`], 96 router pixels, by default;
    /// negative counts as 0). Changing it rebuilds the tops once; a pane
    /// anchors on it as on any height change.
    pub fn set_placeholder_height(&self, h: f32) {
        self.with(|s| {
            let h = if h.is_finite() { h.max(0.0) } else { 0.0 };
            if s.placeholder_height.0 != h {
                s.placeholder_height = PlaceholderHeight(h);
                s.rebuild();
            }
        });
    }

    /// The height a placeholder section counts as.
    pub fn placeholder_height(&self) -> f32 {
        self.with(|s| s.placeholder_height.0)
    }

    /// How many times the tops have been rebuilt.
    #[cfg(test)]
    pub(crate) fn rebuilds(&self) -> u64 {
        self.with(|s| s.rebuilds)
    }

    /// The window of the pane drawing these sections when it changed since
    /// last taken; `None` while the pane's scrollbar thumb is held or the
    /// list is empty. A pane works it out on each range update; the window
    /// at a thumb's release is reported on the update after it. An app
    /// fetches data for `near`, may forget what is outside `keep`, and under
    /// memory pressure may use `close` instead.
    pub fn take_window(&self) -> Option<SectionWindow> {
        self.with(|s| {
            if s.window_held || s.window.is_none() || s.window == s.taken {
                return None;
            }
            s.taken = s.window;
            s.window
        })
    }

    /// The window last worked out, taken or not.
    pub fn window(&self) -> Option<SectionWindow> {
        self.with(|s| s.window)
    }

    /// The window's margins in window lengths each side: `close` (1.0 by
    /// default), `near` (5.0) and `keep` (20.0); negative counts as 0.
    /// They apply from the next range update.
    pub fn set_window_margins(&self, close: f32, near: f32, keep: f32) {
        self.with(|s| {
            s.margins = Margins {
                close: close.max(0.0),
                near: near.max(0.0),
                keep: keep.max(0.0),
            }
        });
    }

    /// Extra length counted by a scrollbar after the last section, not
    /// scrollable into: content known to exist past the end (0 by
    /// default; negative counts as 0). A pane's range is unchanged by it;
    /// its scrollbar's content length is `total_height + pending_length`.
    pub fn set_pending_length(&self, px: f32) {
        self.with(|s| s.pending = if px.is_finite() { px.max(0.0) } else { 0.0 });
    }

    /// The length set by [`set_pending_length`](Self::set_pending_length).
    pub fn pending_length(&self) -> f32 {
        self.with(|s| s.pending)
    }

    /// A pane's range update: its window is `start..start + len`.
    pub(crate) fn note_window(&self, start: f32, len: f32, following: bool, held: bool) {
        self.with(|s| {
            s.window_held = held;
            if !held {
                s.window = s.window_at(start, len, following);
            }
        });
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
        self.with(|s| ids.iter().map(|&id| s.resolve_top(id)).collect())
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
                height: s.sections[i].length(s.placeholder_height.0),
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
                let ph = s.placeholder_height.0;
                if let Some(h) = reported
                    && (h - s.sections[i].length(ph)).abs() > 0.5
                {
                    let sec = &mut s.sections[i];
                    sec.height = h;
                    sec.estimated = false;
                    sec.placeholder = false;
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

    /// The tops of `b`'s sections, in order: `(id, index, top)`.
    pub(crate) fn boundary_tops(&self, b: Boundaries) -> Vec<(SectionId, usize, f32)> {
        self.with(|s| {
            s.sections
                .iter()
                .enumerate()
                .filter(|(_, sec)| match b {
                    Boundaries::Sections => true,
                    Boundaries::AtMost(n) => sec.level <= n,
                    Boundaries::Bookmarked => s.bookmarks.contains_key(&sec.id),
                })
                .map(|(i, sec)| (sec.id, i, s.tops[i]))
                .collect()
        })
    }

    /// Where `target` is now, for a fling moving `dir` (+1 towards the
    /// end): a split or merged section's place goes to its replacement, a
    /// removed one's to the next section still present that way, or the
    /// end. `None` when the list is empty.
    pub(crate) fn locate(&self, target: Target, dir: f32) -> Option<Stop> {
        self.with(|s| {
            let n = s.sections.len();
            if n == 0 {
                return None;
            }
            let end = |s: &Inner, t: Target| match t {
                Target::Start => Stop {
                    target: Target::Start,
                    top: 0.0,
                    id: s.sections[0].id,
                },
                _ => Stop {
                    target: Target::End,
                    top: s.total(),
                    id: s.sections[n - 1].id,
                },
            };
            let at = |s: &Inner, i: usize| Stop {
                target: Target::Section {
                    id: s.sections[i].id,
                    index: i,
                },
                top: s.tops[i],
                id: s.sections[i].id,
            };
            Some(match target {
                Target::Start | Target::End => end(s, target),
                Target::Section { id, index } => {
                    if let Some(&i) = s.index.get(&id) {
                        at(s, i)
                    } else if let Some(&i) =
                        s.replaced.get(&id).and_then(|(new, _)| s.index.get(new))
                    {
                        at(s, i)
                    } else {
                        let k = if dir > 0.0 {
                            Some(index)
                        } else {
                            index.checked_sub(1)
                        };
                        match k.filter(|&k| k < n) {
                            Some(k) => at(s, k),
                            None if dir > 0.0 => end(s, Target::End),
                            None => end(s, Target::Start),
                        }
                    }
                }
            })
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
    // ---- Plan 20261119_01, step 1: placeholders and batched edits ----

    #[test]
    fn a_placeholder_counts_at_the_contents_height_until_given_one() {
        let c = SectionedContent::new();
        c.set_sections(vec![
            Section::new(SectionId(0), 100.0),
            Section::placeholder(SectionId(1)),
            Section::new(SectionId(2), 100.0),
        ]);
        let p = &c.sections()[1];
        assert!(p.placeholder && p.estimated);
        assert_eq!(c.placeholder_height(), 96.0);
        assert_eq!(c.total_height(), 296.0);
        assert_eq!(c.offset_of(SectionId(2)), Some(196.0));
        assert_eq!(c.section_at(150.0), Some(SectionId(1)));
        assert!(c.set_height(SectionId(1), 40.0));
        let p = &c.sections()[1];
        assert!(!p.placeholder && !p.estimated);
        assert_eq!(c.total_height(), 240.0);
        c.push(Section::placeholder(SectionId(3)));
        assert!(c.set_estimate(SectionId(3), 10.0));
        let p = &c.sections()[3];
        assert!(!p.placeholder && p.estimated);
        assert_eq!(c.total_height(), 250.0);
    }

    #[test]
    fn set_placeholder_height_rebuilds_once() {
        let c = SectionedContent::new();
        c.set_sections(
            (0..10)
                .map(|i| Section::placeholder(SectionId(i)))
                .collect(),
        );
        let before = c.rebuilds();
        c.set_placeholder_height(50.0);
        assert_eq!(c.rebuilds(), before + 1);
        assert_eq!(c.total_height(), 500.0);
        c.set_placeholder_height(50.0);
        assert_eq!(c.rebuilds(), before + 1, "unchanged: no rebuild");
    }

    #[test]
    fn an_edit_of_a_thousand_heights_rebuilds_the_tops_once() {
        let c = SectionedContent::new();
        c.set_sections(
            (0..1000)
                .map(|i| Section::new(SectionId(i), 10.0))
                .collect(),
        );
        let before = c.rebuilds();
        let n = c.edit(|e| {
            (0..1000)
                .filter(|&i| e.set_height(SectionId(i), 20.0))
                .count()
        });
        assert_eq!(n, 1000);
        assert_eq!(c.rebuilds(), before + 1);
        assert_eq!(c.total_height(), 20_000.0);
        assert_eq!(c.offset_of(SectionId(999)), Some(19_980.0));
    }

    #[test]
    fn an_edit_sees_its_own_changes_and_drops_removed_drawings() {
        let draw = |_: SectionId, _: &SectionContext| DrawList::new();
        let c = SectionedContent::with_source(Arc::new(draw));
        c.set_sections((0..10).map(|i| Section::new(SectionId(i), 100.0)).collect());
        let theme = ControlTheme::default();
        let jobs = c.prepare(400.0, &theme, 0.0, 1000.0, &HashSet::new(), 0);
        c.render(&jobs, 400.0, &crate::traits::DrawContext::default());
        assert!(c.is_cached(SectionId(3)) && c.is_cached(SectionId(4)));
        c.edit(|e| {
            e.insert(0, Section::new(SectionId(100), 5.0));
            assert_eq!(e.index_of(SectionId(3)), Some(4));
            assert!(e.remove(SectionId(3)).is_some());
            assert_eq!(e.index_of(SectionId(4)), Some(4));
            e.push(Section::new(SectionId(101), 5.0));
            assert_eq!(e.index_of(SectionId(101)), Some(10));
            assert_eq!(e.len(), 11);
        });
        assert!(!c.is_cached(SectionId(3)), "removed: dropped");
        assert!(c.is_cached(SectionId(4)), "kept");
    }

    #[test]
    fn levels_default_to_zero_and_clamp_at_seven() {
        assert_eq!(Section::new(SectionId(0), 1.0).level, 0);
        assert_eq!(Section::new(SectionId(0), 1.0).level(3).level, 3);
        assert_eq!(Section::placeholder(SectionId(0)).level(200).level, 7);
    }

    #[test]
    fn split_and_merge_in_one_edit_rebuild_once_and_drop_what_they_replace() {
        let draw = |_: SectionId, _: &SectionContext| DrawList::new();
        let c = SectionedContent::with_source(Arc::new(draw));
        c.set_sections((0..10).map(|i| Section::new(SectionId(i), 100.0)).collect());
        for i in 0..10 {
            c.add_bookmark(SectionId(i), BookmarkText::Text(format!("{i}")));
        }
        let theme = ControlTheme::default();
        let jobs = c.prepare(400.0, &theme, 0.0, 1000.0, &HashSet::new(), 0);
        c.render(&jobs, 400.0, &crate::traits::DrawContext::default());
        let before = c.rebuilds();
        let ok = c.edit(|e| {
            let a = e.split(
                SectionId(2),
                vec![
                    Section::new(SectionId(20), 30.0).level(1),
                    Section::new(SectionId(21), 70.0).level(2),
                ],
            );
            let b = e.merge(
                SectionId(5),
                SectionId(7),
                Section::new(SectionId(50), 300.0),
            );
            e.add_bookmark(SectionId(20), BookmarkText::Text("20".into()));
            let bad = e.merge(SectionId(9), SectionId(8), Section::new(SectionId(90), 1.0));
            assert!(!e.split(SectionId(99), Vec::new()));
            a && b && !bad
        });
        assert!(ok);
        assert_eq!(c.rebuilds(), before + 1);
        let ids: Vec<u64> = c.sections().iter().map(|s| s.id.0).collect();
        assert_eq!(ids, [0, 1, 20, 21, 3, 4, 50, 8, 9]);
        assert_eq!(c.total_height(), 1000.0);
        for gone in [2, 5, 6, 7] {
            assert!(!c.is_cached(SectionId(gone)));
            assert!(c.remove_bookmark(SectionId(gone)).is_none(), "{gone}");
        }
        assert!(c.is_cached(SectionId(4)));
        assert!(c.remove_bookmark(SectionId(4)).is_some());
        assert!(c.remove_bookmark(SectionId(20)).is_some());
        // A merged id resolves into the merged section at its old distance.
        c.with(|s| {
            assert_eq!(s.resolve_top(SectionId(6)), Some(600.0));
            assert_eq!(s.resolve_top(SectionId(2)), Some(200.0));
        });
    }
}
