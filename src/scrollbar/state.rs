//! A scrollbar's shared state: its feed, its clock, its grab and its events.

use std::fmt;
use std::sync::{Arc, Mutex};

use crate::rect::Rect;
use crate::sections::{SectionId, SectionedContent};

use super::{ScrollbarEvent, ScrollbarResolved, ScrollbarTheme, ScrollbarVisibility};

/// The thumb on the track for these metrics, when there is one (decision 2).
/// Lengths on the screen are the track's: with the window as tall as the
/// track, as on a pane, they are the plan's formulas exactly.
pub(crate) fn thumb_rect(
    track: Rect,
    r: &ScrollbarResolved,
    offset: f32,
    content: f32,
    window: f32,
) -> Option<Rect> {
    let range = content - window;
    if range <= 0.5 || window <= 0.0 || track.h <= 0.0 {
        return None;
    }
    let len = (window / content * track.h).max(r.min_length).min(track.h);
    let u = (offset / range).clamp(0.0, 1.0);
    let top = track.y + u * (track.h - len);
    let x = track.right() - r.slop - r.width;
    Some(Rect::new(x, top, r.width, len))
}

/// The band around a thumb: from its left less the slop to the track's
/// right edge, the track's full height.
pub(crate) fn band_rect(track: Rect, r: &ScrollbarResolved, thumb: Rect) -> Rect {
    let x = thumb.x - r.slop;
    Rect::new(x, track.y, (track.right() - x).max(0.0), track.h)
}

/// `hold` whole, then a linear fade over `fade`.
fn hold_then_fade(since: f64, hold: f32, fade: f32) -> f32 {
    let since = since as f32;
    if since.is_nan() || since < 0.0 {
        return 1.0;
    }
    if since < hold {
        1.0
    } else if since < hold + fade {
        1.0 - (since - hold) / fade.max(f32::EPSILON)
    } else {
        0.0
    }
}

#[derive(Debug)]
pub(crate) struct Inner {
    pub(crate) track: Rect,
    pub(crate) resolved: ScrollbarResolved,
    pub(crate) visibility: ScrollbarVisibility,
    pub(crate) bookmarks_on: bool,
    pub(crate) offset: f32,
    pub(crate) content: f32,
    pub(crate) window: f32,
    /// Its own clock, the sum of every `advance`.
    now: f64,
    scrolling: bool,
    /// When it last scrolled or was let go of, on its clock.
    last_active: f64,
    /// The grab: the press's y less the thumb's top.
    grab: Option<f32>,
    /// When the thumb was last let go of, on its clock.
    released_at: f64,
    /// A scrub not yet applied by the router.
    pending: Option<f32>,
    events: Vec<ScrollbarEvent>,
    pub(crate) sections: Option<Arc<SectionedContent>>,
}

impl Default for Inner {
    fn default() -> Self {
        Self {
            track: Rect::default(),
            resolved: super::ScrollbarOptions::default().with_theme(&ScrollbarTheme::default()),
            visibility: ScrollbarVisibility::default(),
            bookmarks_on: true,
            offset: 0.0,
            content: 0.0,
            window: 0.0,
            now: 0.0,
            scrolling: false,
            last_active: f64::NEG_INFINITY,
            grab: None,
            released_at: f64::NEG_INFINITY,
            pending: None,
            events: Vec::new(),
            sections: None,
        }
    }
}

impl Inner {
    fn range(&self) -> f32 {
        (self.content - self.window).max(0.0)
    }

    pub(crate) fn thumb(&self) -> Option<Rect> {
        thumb_rect(
            self.track,
            &self.resolved,
            self.offset,
            self.content,
            self.window,
        )
    }

    pub(crate) fn band(&self) -> Option<Rect> {
        if self.visibility == ScrollbarVisibility::None {
            return None;
        }
        self.thumb()
            .map(|t| band_rect(self.track, &self.resolved, t))
    }

    fn in_band(&self, pos: [f32; 2]) -> bool {
        self.band().is_some_and(|b| b.contains(pos))
    }

    /// How visible the thumb is, 0 to the resolved alpha.
    pub(crate) fn alpha(&self) -> f32 {
        let full = self.resolved.alpha;
        match self.visibility {
            ScrollbarVisibility::None => 0.0,
            ScrollbarVisibility::Always => full,
            ScrollbarVisibility::OnScroll => {
                if self.grab.is_some() || self.scrolling {
                    full
                } else {
                    full * hold_then_fade(
                        self.now - self.last_active,
                        self.resolved.hold,
                        self.resolved.fade,
                    )
                }
            }
        }
    }

    /// How visible the bookmarks are, 0 to 1: whole while held and `hold`
    /// after, then fading.
    pub(crate) fn bookmarks_alpha(&self) -> f32 {
        if self.grab.is_some() {
            return 1.0;
        }
        hold_then_fade(
            self.now - self.released_at,
            self.resolved.hold,
            self.resolved.fade,
        )
    }

    pub(crate) fn busy(&self) -> bool {
        (self.visibility == ScrollbarVisibility::OnScroll && self.alpha() > 0.0)
            || (self.bookmarks_on && self.bookmarks_alpha() > 0.0)
    }

    /// The section at `offset` and the bookmark nearest the thumb, when
    /// sections are given.
    fn names(&self) -> (Option<SectionId>, Option<SectionId>) {
        let Some(sections) = &self.sections else {
            return (None, None);
        };
        let section = sections.section_at(self.offset);
        let centre = self.thumb().map(|t| t.y + t.h * 0.5);
        let content = self.content.max(1.0);
        let bookmark = centre.and_then(|c| {
            sections
                .placed_bookmarks()
                .into_iter()
                .map(|(id, top, _)| {
                    let y = self.track.y + (top / content).clamp(0.0, 1.0) * self.track.h;
                    (id, (y - c).abs())
                })
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(id, _)| id)
        });
        (section, bookmark)
    }

    /// Scrub so the thumb's top is at `y` less the grab.
    fn scrub_to(&mut self, y: f32) {
        let (Some(grab), Some(thumb)) = (self.grab, self.thumb()) else {
            return;
        };
        let travel = self.track.h - thumb.h;
        let u = if travel > 0.0 {
            ((y - grab - self.track.y) / travel).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let target = u * self.range();
        if target == self.offset {
            return;
        }
        self.offset = target;
        self.pending = Some(target);
        let (section, bookmark) = self.names();
        self.events.push(ScrollbarEvent::Scrub {
            offset: target,
            section,
            bookmark,
        });
    }

    pub(crate) fn press(&mut self, pos: [f32; 2]) -> bool {
        if !self.in_band(pos) {
            return false;
        }
        let Some(thumb) = self.thumb() else {
            return false;
        };
        let on_thumb = pos[1] >= thumb.y && pos[1] <= thumb.bottom();
        self.grab = Some(if on_thumb {
            pos[1] - thumb.y
        } else {
            thumb.h * 0.5
        });
        self.last_active = self.now;
        self.events.push(ScrollbarEvent::Grabbed {
            offset: self.offset,
        });
        if !on_thumb {
            self.scrub_to(pos[1]);
        }
        true
    }

    pub(crate) fn drag_to(&mut self, pos: [f32; 2]) {
        if self.grab.is_some() {
            self.last_active = self.now;
            self.scrub_to(pos[1]);
        }
    }

    pub(crate) fn release(&mut self) {
        if self.grab.take().is_none() {
            return;
        }
        self.last_active = self.now;
        self.released_at = self.now;
        let (section, bookmark) = self.names();
        self.events.push(ScrollbarEvent::Released {
            offset: self.offset,
            section,
            bookmark,
        });
    }

    pub(crate) fn is_held(&self) -> bool {
        self.grab.is_some()
    }

    pub(crate) fn take_pending(&mut self) -> Option<f32> {
        self.pending.take()
    }

    /// The feed from a pane: its metrics, whether it is being dragged or
    /// coasting, and how long ago (router seconds) it last moved.
    pub(crate) fn feed_pane(
        &mut self,
        track: Rect,
        offset: f32,
        content: f32,
        moving: bool,
        since_moved: f64,
    ) {
        self.track = track;
        self.offset = offset;
        self.content = content;
        self.window = track.h;
        self.scrolling = moving;
        if moving {
            self.last_active = self.now;
        } else if since_moved.is_finite() {
            self.last_active = self.last_active.max(self.now - since_moved);
        }
    }

    pub(crate) fn tick(&mut self, dt: f32) -> bool {
        self.now += f64::from(dt.max(0.0));
        if self.scrolling || self.grab.is_some() {
            self.last_active = self.now;
        }
        self.busy()
    }
}

/// One scrollbar's state, shared (`Arc`) between its control and the app.
///
/// The track and the theme numbers it reads come from its [`Scrollbar`]
/// (`new`, `set_track`, and each draw) or, on a pane, from the router.
///
/// [`Scrollbar`]: super::Scrollbar
pub struct ScrollbarState {
    inner: Mutex<Inner>,
}

impl fmt::Debug for ScrollbarState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.with(|s| {
            f.debug_struct("ScrollbarState")
                .field("offset", &s.offset)
                .field("content", &s.content)
                .field("window", &s.window)
                .field("held", &s.grab.is_some())
                .finish()
        })
    }
}

impl Default for ScrollbarState {
    fn default() -> Self {
        Self {
            inner: Mutex::new(Inner::default()),
        }
    }
}

impl ScrollbarState {
    /// No sections; nothing to scroll until fed.
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Bookmarks and scrub names from `sections`, used as an index.
    pub fn with_sections(sections: Arc<SectionedContent>) -> Arc<Self> {
        let state = Self::default();
        state.set_sections(Some(sections));
        Arc::new(state)
    }

    pub(crate) fn with<R>(&self, f: impl FnOnce(&mut Inner) -> R) -> R {
        f(&mut self.inner.lock().unwrap_or_else(|e| e.into_inner()))
    }

    /// Give it sections (an index: their tops must add up to the content
    /// length it is fed), or take them away.
    pub fn set_sections(&self, sections: Option<Arc<SectionedContent>>) {
        self.with(|s| s.sections = sections);
    }

    /// The standalone feed: the scroller's offset, its content's length and
    /// its window's length. On a pane the router feeds it.
    pub fn set_metrics(&self, offset: f32, content: f32, window: f32) {
        self.with(|s| {
            s.offset = offset;
            s.content = content.max(0.0);
            s.window = window.max(0.0);
        });
    }

    /// The standalone feed: whether the scroller is scrolling. The hold
    /// starts again on each stop.
    pub fn set_scrolling(&self, scrolling: bool) {
        self.with(|s| {
            if scrolling || s.scrolling {
                s.last_active = s.now;
            }
            s.scrolling = scrolling;
        });
    }

    /// Move its clock on by `dt_secs`. On a pane, and for a `Scrollbar`
    /// node, `Router::advance` does this; call it only for a scrollbar no
    /// router advances. Returns whether it still needs frames (shown with
    /// `OnScroll`, or its bookmarks shown or fading).
    pub fn advance(&self, dt_secs: f32) -> bool {
        self.with(|s| s.tick(dt_secs))
    }

    /// The events since the last call: grabs, scrubs and releases.
    pub fn take_events(&self) -> Vec<ScrollbarEvent> {
        self.with(|s| std::mem::take(&mut s.events))
    }

    /// Whether the thumb is held.
    pub fn is_held(&self) -> bool {
        self.with(|s| s.is_held())
    }

    /// The thumb's opacity: 0 to the resolved `alpha`.
    pub fn alpha(&self) -> f32 {
        self.with(|s| s.alpha())
    }

    /// The bookmarks' opacity, 0 to 1.
    pub fn bookmarks_alpha(&self) -> f32 {
        self.with(|s| s.bookmarks_alpha())
    }

    /// A press at `pos`, in the track's coordinates (those its `Rect` is
    /// given in). Returns whether it was in the band and grabbed the thumb.
    pub fn press(&self, pos: [f32; 2]) -> bool {
        self.with(|s| s.press(pos))
    }

    /// The held thumb moved to `pos`: it scrubs.
    pub fn drag_to(&self, pos: [f32; 2]) {
        self.with(|s| s.drag_to(pos));
    }

    /// Let go of the thumb: the content stays where it is (no fling) and
    /// the hold starts.
    pub fn release(&self) {
        self.with(|s| s.release());
    }

    /// The press went away (a layer opened): ends the drag as a release
    /// does.
    pub fn cancel(&self) {
        self.release();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TRACK: Rect = Rect::new(0.0, 0.0, 400.0, 1000.0);

    fn fed(content: f32, offset: f32, visibility: ScrollbarVisibility) -> Inner {
        Inner {
            track: TRACK,
            visibility,
            offset,
            content,
            window: 1000.0,
            ..Inner::default()
        }
    }

    #[test]
    fn the_thumb_and_the_band() {
        let s = fed(4000.0, 0.0, ScrollbarVisibility::OnScroll);
        let t = s.thumb().unwrap();
        assert_eq!((t.x, t.w, t.y, t.h), (392.0, 4.0, 0.0, 250.0));
        assert_eq!(
            fed(4000.0, 3000.0, ScrollbarVisibility::OnScroll)
                .thumb()
                .unwrap()
                .bottom(),
            1000.0
        );
        let mid = fed(4000.0, 1500.0, ScrollbarVisibility::OnScroll)
            .thumb()
            .unwrap();
        assert_eq!(mid.y, 375.0, "midway along the 750 of travel");
        assert_eq!(
            fed(100_000.0, 0.0, ScrollbarVisibility::OnScroll)
                .thumb()
                .unwrap()
                .h,
            24.0
        );
        assert!(
            fed(1000.4, 0.0, ScrollbarVisibility::OnScroll)
                .thumb()
                .is_none()
        );
        let b = s.band().unwrap();
        assert_eq!((b.x, b.right(), b.y, b.h), (388.0, 400.0, 0.0, 1000.0));
    }

    #[test]
    fn a_press_on_the_thumb_grabs_where_it_touched() {
        let mut s = fed(4000.0, 0.0, ScrollbarVisibility::OnScroll);
        assert!(s.press([394.0, 100.0]));
        assert_eq!(s.events, [ScrollbarEvent::Grabbed { offset: 0.0 }]);
        // A quarter of the 750 of travel is a quarter of the 3000 range.
        s.drag_to([394.0, 100.0 + 187.5]);
        assert_eq!(s.offset, 750.0);
        s.release();
        assert!(matches!(
            s.events.last(),
            Some(ScrollbarEvent::Released { offset, .. }) if *offset == 750.0
        ));
        assert_eq!(s.offset, 750.0, "no fling");
    }

    #[test]
    fn a_press_below_the_thumb_centres_it_under_the_press() {
        let mut s = fed(4000.0, 0.0, ScrollbarVisibility::OnScroll);
        assert!(s.press([390.0, 500.0]));
        let t = s.thumb().unwrap();
        assert!((t.y + t.h * 0.5 - 500.0).abs() < 1e-3, "{t:?}");
        assert!(matches!(s.events[1], ScrollbarEvent::Scrub { .. }));
        assert_eq!(s.take_pending(), Some(s.offset));
        assert!(!s.press([380.0, 500.0]), "left of the band");
    }

    #[test]
    fn on_scroll_holds_then_fades_and_always_stays() {
        let mut s = fed(4000.0, 0.0, ScrollbarVisibility::OnScroll);
        assert_eq!(s.alpha(), 0.0);
        s.scrolling = true;
        s.tick(0.5);
        assert_eq!(s.alpha(), 0.35);
        s.scrolling = false;
        s.last_active = s.now;
        s.tick(2.0);
        assert_eq!(s.alpha(), 0.35);
        s.tick(0.15);
        assert!((s.alpha() - 0.175).abs() < 1e-3, "{}", s.alpha());
        s.tick(0.15);
        assert_eq!(s.alpha(), 0.0);
        assert!(!s.tick(0.0), "idle");
        let a = fed(4000.0, 0.0, ScrollbarVisibility::Always);
        assert_eq!(a.alpha(), 0.35);
        let mut n = fed(4000.0, 0.0, ScrollbarVisibility::None);
        assert_eq!(n.alpha(), 0.0);
        assert!(n.band().is_none());
        assert!(!n.press([394.0, 100.0]));
    }

    #[test]
    fn bookmarks_hold_then_fade_after_release_in_every_visibility() {
        for v in [ScrollbarVisibility::OnScroll, ScrollbarVisibility::Always] {
            let mut s = fed(4000.0, 0.0, v);
            assert_eq!(s.bookmarks_alpha(), 0.0);
            s.press([394.0, 100.0]);
            s.tick(5.0);
            assert_eq!(s.bookmarks_alpha(), 1.0, "held");
            s.release();
            s.tick(1.99);
            assert_eq!(s.bookmarks_alpha(), 1.0);
            s.tick(0.32);
            assert_eq!(s.bookmarks_alpha(), 0.0);
        }
    }
}
