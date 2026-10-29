//! Sectioned content as an index: an ordered list of sections, each with a
//! stable [`SectionId`] and a height, and the bookmarks added on them
//! (docs/design/20261028_08_SCROLLBAR_PLAN.md, decisions 7, 8 and 11).
//!
//! In this release a [`SectionedContent`] is an index only (the plan's
//! decision 9 fallback): a [`Scrollbar`](crate::Scrollbar) reads its
//! sections' tops to place bookmarks and to name the section and bookmark of
//! a scrub. Sections drawn by a scroll pane through a source wait for
//! libmsdf's scoped append (the plan's steps 6 and 7).

use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex};

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
    /// Inline Markdown. Drawn as plain text, exactly as written, in this
    /// release (the plan's decision 6, without the `markdown` feature).
    Markdown(String),
}

impl BookmarkText {
    /// The text as drawn: in this release, the string as written.
    pub(crate) fn plain(&self) -> &str {
        match self {
            Self::Text(s) | Self::Markdown(s) => s,
        }
    }
}

#[derive(Default)]
struct Inner {
    sections: Vec<Section>,
    /// `tops[i]` is the offset of `sections[i]`; one more entry, the total.
    tops: Vec<f32>,
    index: HashMap<SectionId, usize>,
    bookmarks: HashMap<SectionId, BookmarkText>,
}

impl Inner {
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

/// Sections and their bookmarks, shared (`Arc`) between the app and its
/// scrollbars. Bookmarks are kept by id, independent of the list: one added
/// before its section exists is shown once a section with that id is there.
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
                .finish()
        })
    }
}

impl SectionedContent {
    /// An empty index.
    pub fn new() -> Arc<Self> {
        let mut inner = Inner::default();
        inner.rebuild();
        Arc::new(Self {
            inner: Mutex::new(inner),
        })
    }

    fn with<R>(&self, f: impl FnOnce(&mut Inner) -> R) -> R {
        f(&mut self.inner.lock().unwrap_or_else(|e| e.into_inner()))
    }

    fn edit<R>(&self, f: impl FnOnce(&mut Inner) -> R) -> R {
        self.with(|s| {
            let r = f(s);
            s.rebuild();
            r
        })
    }

    /// Replace the list. Bookmarks are kept.
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

    /// Remove the section `id`, returning it. Its bookmark is kept.
    pub fn remove(&self, id: SectionId) -> Option<Section> {
        self.edit(|s| {
            let i = s.index.get(&id).copied()?;
            Some(s.sections.remove(i))
        })
    }

    /// Set a section's height (negative counts as 0), clearing its estimate
    /// flag. Returns whether the section exists.
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

    /// The bookmarks whose section exists, with that section's top.
    pub(crate) fn placed_bookmarks(&self) -> Vec<(SectionId, f32, String)> {
        self.with(|s| {
            let mut v: Vec<(SectionId, f32, String)> = s
                .bookmarks
                .iter()
                .filter_map(|(&id, t)| s.offset_of(id).map(|top| (id, top, t.plain().to_owned())))
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
