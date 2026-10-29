//! A scroll pane: a router node whose children lie on a content plane
//! larger than its viewport (docs/design/20261023_02_SCROLL_PANE_PLAN.md).
//!
//! The pane itself only holds its viewport, its options and its
//! [`ScrollState`]. The router does the rest: it clips and translates the
//! children, skips the ones out of view, maps hits and events through the
//! offset, tells a tap from a scroll, and moves flings on
//! [`Router::advance`](crate::Router::advance).

use std::cell::Cell;
use std::fmt;
use std::sync::{Arc, Mutex};

use libmsdf::drawlist::{DrawList, SdfInstance, SdfKind};

use crate::control::HostedControl;
use crate::event::{ControlAction, ControlPart, EventContext, KeyboardEvent, MouseEvent};
use crate::flags::NodeFlags;
use crate::rect::Rect;
use crate::router::NodeId;
use crate::scrollbar::{ScrollbarOptions, ScrollbarState, ScrollbarTheme};
use crate::sections::{SectionId, SectionedContent};
use crate::theme::ScrollTheme;
use crate::traits::{DrawContext, DrawControl};

/// The axis a pane scrolls along.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ScrollAxis {
    #[default]
    Vertical,
    Horizontal,
}

impl ScrollAxis {
    /// The component of `v` along this axis.
    pub(crate) fn of(self, v: [f32; 2]) -> f32 {
        match self {
            Self::Vertical => v[1],
            Self::Horizontal => v[0],
        }
    }

    /// A vector of `len` along this axis.
    pub(crate) fn vector(self, len: f32) -> [f32; 2] {
        match self {
            Self::Vertical => [0.0, len],
            Self::Horizontal => [len, 0.0],
        }
    }

    /// A rectangle's leading edge and length along this axis.
    pub(crate) fn span(self, r: Rect) -> (f32, f32) {
        match self {
            Self::Vertical => (r.y, r.h),
            Self::Horizontal => (r.x, r.w),
        }
    }
}

/// The end a sticky pane follows.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ScrollEdge {
    /// The start: newest content first.
    Start,
    /// The end: a log or a chat.
    End,
}

/// How far a finger may pull the content past either end.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub enum Overscroll {
    /// Stops at the ends.
    #[default]
    None,
    /// Up to this many pixels past either end.
    Px(f32),
    /// Up to this share of the viewport's length past either end.
    Fraction(f32),
}

/// How long the finger's recent samples are kept for its speed.
const SAMPLE_WINDOW: f64 = 0.100;
/// Within this of its edge, a sticky pane is at it.
const AT_EDGE: f32 = 1.0;
/// The overscroll snaps to the end once this close.
const SNAP: f32 = 0.5;

/// A finger holding a pane.
#[derive(Clone, Debug, Default)]
struct DragTrack {
    /// The finger's last position along the axis, on the screen.
    last: f32,
    /// Router time and position of the recent samples.
    samples: Vec<(f64, f32)>,
    /// When the finger last moved.
    last_move: f64,
}

#[derive(Debug)]
pub(crate) struct Inner {
    axis: ScrollAxis,
    offset: f32,
    wanted: f32,
    max: f32,
    /// The content's length along the axis.
    content: f32,
    /// Drawn past the end: negative before the start, positive past the end.
    over: f32,
    /// How far the finger is past the end, before resistance.
    over_raw: f32,
    /// The overscroll spring's speed (px/s).
    over_vel: f32,
    sticky: Option<ScrollEdge>,
    following: bool,
    /// The finger's speed of a fling (px/s; positive: the finger moved
    /// towards the end of the axis, scrolling back).
    fling: f32,
    drag: Option<DragTrack>,
    /// The children in view at the last range update, from the leading
    /// edge, with their leading edges then.
    pub(crate) anchors: Vec<(NodeId, f32)>,
    /// The sections in view at the last range update, from the leading
    /// edge, with their tops then (a pane with sections).
    pub(crate) section_anchors: Vec<(SectionId, f32)>,
    /// The anchoring shift held back while the scrollbar's thumb is held,
    /// applied on release.
    pub(crate) held_shift: f32,
    /// When it last moved, on router time.
    last_moved: f64,
}

impl Default for Inner {
    fn default() -> Self {
        Self {
            axis: ScrollAxis::Vertical,
            offset: 0.0,
            wanted: 0.0,
            max: 0.0,
            content: 0.0,
            over: 0.0,
            over_raw: 0.0,
            over_vel: 0.0,
            sticky: None,
            following: false,
            fling: 0.0,
            drag: None,
            anchors: Vec::new(),
            section_anchors: Vec::new(),
            held_shift: 0.0,
            last_moved: f64::NEG_INFINITY,
        }
    }
}

/// The iOS resistance: `x` pixels of finger past the end draw this far past
/// it, never reaching `limit`.
fn rubber_band(x: f32, limit: f32) -> f32 {
    if limit <= 0.0 {
        return 0.0;
    }
    x.signum() * limit * (1.0 - 1.0 / (x.abs() * 0.55 / limit + 1.0))
}

impl Inner {
    fn edge_pos(&self) -> Option<f32> {
        self.sticky.map(|e| match e {
            ScrollEdge::Start => 0.0,
            ScrollEdge::End => self.max,
        })
    }

    fn update_following(&mut self) {
        if let Some(edge) = self.edge_pos() {
            self.following = (self.offset - edge).abs() <= AT_EDGE;
        }
    }

    fn moved(&mut self, now: f64) {
        self.last_moved = now;
    }

    pub(crate) fn offset(&self) -> f32 {
        self.offset
    }

    pub(crate) fn max(&self) -> f32 {
        self.max
    }

    pub(crate) fn last_moved(&self) -> f64 {
        self.last_moved
    }

    pub(crate) fn is_following(&self) -> bool {
        self.following
    }

    pub(crate) fn is_dragged(&self) -> bool {
        self.drag.is_some()
    }

    /// The offset drawn: the offset and the overscroll.
    pub(crate) fn drawn(&self) -> f32 {
        self.offset + self.over
    }

    /// Whether a fling or a spring is moving it.
    pub(crate) fn is_coasting(&self, theme: &ScrollTheme) -> bool {
        self.fling.abs() >= theme.min_fling || self.over != 0.0 || self.over_vel != 0.0
    }

    /// The range: the content's length and the largest offset. A following
    /// pane goes to its edge; the others return towards `wanted`.
    pub(crate) fn set_range(&mut self, content: f32, max: f32) {
        self.content = content;
        self.max = max;
        if self.following
            && let Some(edge) = self.edge_pos()
        {
            self.wanted = edge;
        }
        self.offset = self.wanted.clamp(0.0, self.max);
    }

    /// Content above the reader moved by `d`: so does the offset.
    pub(crate) fn shift(&mut self, d: f32) {
        self.offset += d;
        self.wanted += d;
    }

    /// Put it at `offset` as the user or the app asked, stopping a fling
    /// and any overscroll.
    pub(crate) fn scroll_to(&mut self, offset: f32, now: f64) {
        let before = self.offset;
        self.fling = 0.0;
        self.over = 0.0;
        self.over_raw = 0.0;
        self.over_vel = 0.0;
        self.offset = offset.clamp(0.0, self.max);
        self.wanted = self.offset;
        self.update_following();
        if self.offset != before {
            self.moved(now);
        }
    }

    /// Whether moving the content by `delta` (positive: towards the end)
    /// would move it.
    pub(crate) fn can_move(&self, delta: f32) -> bool {
        (delta > 0.0 && self.offset < self.max) || (delta < 0.0 && self.offset > 0.0)
    }

    /// A finger took hold at `finger` (along the axis, on the screen).
    pub(crate) fn begin_drag(&mut self, finger: f32, now: f64, limit: Option<f32>) {
        self.fling = 0.0;
        self.over_vel = 0.0;
        // A finger that catches a springing overscroll holds it where it is:
        // the finger travel that would draw it, by the inverse resistance.
        self.over_raw = match limit {
            Some(d) if d > 0.0 && self.over != 0.0 => {
                let f = (self.over.abs() / d).min(0.99);
                self.over.signum() * (d / 0.55) * (1.0 / (1.0 - f) - 1.0)
            }
            _ => 0.0,
        };
        if self.over_raw == 0.0 {
            self.over = 0.0;
        }
        self.drag = Some(DragTrack {
            last: finger,
            samples: vec![(now, finger)],
            last_move: now,
        });
        self.moved(now);
    }

    /// The finger moved to `finger`.
    pub(crate) fn drag_to(&mut self, finger: f32, now: f64, limit: Option<f32>) {
        let Some(mut drag) = self.drag.take() else {
            return;
        };
        let delta = finger - drag.last;
        drag.last = finger;
        if delta != 0.0 {
            drag.last_move = now;
        }
        drag.samples.push((now, finger));
        drag.samples.retain(|&(t, _)| t >= now - SAMPLE_WINDOW);
        self.drag = Some(drag);
        if delta == 0.0 {
            return;
        }
        // The finger moving towards the end of the axis moves the content
        // back towards its start.
        let total = self.offset + self.over_raw - delta;
        self.offset = total.clamp(0.0, self.max);
        let mut raw = total - self.offset;
        match self.sticky {
            Some(ScrollEdge::End) if raw > 0.0 => raw = 0.0,
            Some(ScrollEdge::Start) if raw < 0.0 => raw = 0.0,
            _ => {}
        }
        match limit {
            Some(d) if d > 0.0 => {
                self.over_raw = raw;
                self.over = rubber_band(raw, d);
            }
            _ => {
                self.over_raw = 0.0;
                self.over = 0.0;
            }
        }
        self.wanted = self.offset;
        self.update_following();
        self.moved(now);
    }

    /// The finger lifted. It flings when it was moving, springs back when
    /// past an end; `clocked` says whether the router is given time at all
    /// (without it, the content returns to the end at once).
    pub(crate) fn release(&mut self, now: f64, theme: &ScrollTheme, clocked: bool) {
        let Some(drag) = self.drag.take() else {
            return;
        };
        self.over_raw = 0.0;
        if self.over != 0.0 {
            if !clocked {
                self.over = 0.0;
            }
            self.over_vel = 0.0;
            self.moved(now);
            return;
        }
        let rested = now - drag.last_move > f64::from(theme.rest);
        let (first, last) = (drag.samples.first(), drag.samples.last());
        let velocity = match (first, last) {
            (Some(&(t0, p0)), Some(&(t1, p1))) if t1 > t0 => (p1 - p0) / (t1 - t0) as f32,
            _ => 0.0,
        };
        if !rested && velocity.abs() > theme.min_fling {
            self.fling = velocity.clamp(-theme.max_fling, theme.max_fling);
        }
        self.moved(now);
    }

    /// The finger went away without lifting (a layer opened, the pane was
    /// removed): no fling, and any overscroll springs back.
    pub(crate) fn cancel_drag(&mut self) {
        self.drag = None;
        self.over_raw = 0.0;
    }

    /// Carry a fling or a spring on by `dt`. Returns whether it moved.
    pub(crate) fn tick(
        &mut self,
        dt: f32,
        now: f64,
        theme: &ScrollTheme,
        limit: Option<f32>,
    ) -> bool {
        if self.drag.is_some() {
            return false;
        }
        let dt = dt.clamp(0.0, 0.1);
        if dt == 0.0 {
            return false;
        }
        if self.over != 0.0 || self.over_vel != 0.0 {
            // A critically damped spring towards the end, solved exactly.
            let w = theme.overscroll_spring.max(0.1);
            let (x0, v0) = (self.over, self.over_vel);
            let e = (-w * dt).exp();
            let c = v0 + w * x0;
            let mut x = (x0 + c * dt) * e;
            let mut v = (v0 - w * c * dt) * e;
            if let Some(d) = limit {
                x = x.clamp(-d, d);
            }
            let crossed = x != 0.0 && x0 != 0.0 && x.signum() != x0.signum();
            if crossed || (x.abs() < SNAP && v.abs() < theme.min_fling) {
                x = 0.0;
                v = 0.0;
            }
            self.over = x;
            self.over_vel = v;
            self.moved(now);
            return true;
        }
        if self.fling == 0.0 {
            return false;
        }
        let before = self.offset;
        let target = self.offset - self.fling * dt;
        self.offset = target.clamp(0.0, self.max);
        self.wanted = self.offset;
        self.update_following();
        let past_end = target != self.offset;
        if past_end && limit.is_some() && !self.following {
            // Into the end with speed left: it carries into the spring.
            self.over_vel = -self.fling;
            self.fling = 0.0;
            self.moved(now);
            return true;
        }
        self.fling *= (-theme.fling_decay * dt).exp();
        if self.offset == before || self.fling.abs() < theme.min_fling || self.following {
            self.fling = 0.0;
        }
        if self.offset != before {
            self.moved(now);
        }
        self.offset != before
    }

    /// How visible the indicator is at `now`, 0 to 1.
    pub(crate) fn indicator_alpha(&self, now: f64, theme: &ScrollTheme) -> f32 {
        if self.drag.is_some() || self.is_coasting(theme) {
            return 1.0;
        }
        let since = (now - self.last_moved) as f32;
        if since < theme.indicator_idle {
            1.0
        } else if since < theme.indicator_idle + theme.indicator_fade {
            1.0 - (since - theme.indicator_idle) / theme.indicator_fade.max(f32::EPSILON)
        } else {
            0.0
        }
    }
}

/// One pane's place, shared (`Arc`) between the pane and the app, so the
/// place survives the pane being removed and added again.
pub struct ScrollState {
    inner: Mutex<Inner>,
}

impl fmt::Debug for ScrollState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.with(|s| {
            f.debug_struct("ScrollState")
                .field("offset", &s.offset)
                .field("max", &s.max)
                .field("overscroll", &s.over)
                .field("following", &s.following)
                .finish()
        })
    }
}

impl Default for ScrollState {
    fn default() -> Self {
        Self {
            inner: Mutex::new(Inner::default()),
        }
    }
}

impl ScrollState {
    /// At the start.
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Follows `edge` while it is at it: at the end, a log or a chat.
    pub fn sticky(edge: ScrollEdge) -> Arc<Self> {
        let state = Self::default();
        state.with(|s| {
            s.sticky = Some(edge);
            s.following = true;
        });
        Arc::new(state)
    }

    pub(crate) fn with<R>(&self, f: impl FnOnce(&mut Inner) -> R) -> R {
        f(&mut self.inner.lock().unwrap_or_else(|e| e.into_inner()))
    }

    /// The offset on each axis, without any overscroll.
    pub fn offset(&self) -> [f32; 2] {
        self.with(|s| s.axis.vector(s.offset))
    }

    /// The largest offset on each axis: the content's size less the
    /// viewport's, at least 0.
    pub fn max(&self) -> [f32; 2] {
        self.with(|s| s.axis.vector(s.max))
    }

    /// How far the content is drawn past an end: negative before the start,
    /// positive past the end.
    pub fn overscroll(&self) -> f32 {
        self.with(|s| s.over)
    }

    /// Whether it follows its sticky edge.
    pub fn is_following(&self) -> bool {
        self.with(|s| s.following)
    }

    /// Whether a drag, a fling or an overscroll spring is moving it.
    pub fn is_moving(&self) -> bool {
        self.with(|s| s.drag.is_some() || s.fling != 0.0 || s.over != 0.0 || s.over_vel != 0.0)
    }

    /// The content's length along the axis, as the last range update had it.
    pub fn content_length(&self) -> f32 {
        self.with(|s| s.content)
    }
}

/// A node whose children are laid out on a content plane, from the
/// viewport's top-left corner at offset zero, and shown through the
/// viewport at the state's offset.
pub struct ScrollPane {
    viewport: Cell<Rect>,
    axis: ScrollAxis,
    state: Arc<ScrollState>,
    content_size: Cell<Option<[f32; 2]>>,
    anchoring: bool,
    indicator: bool,
    overscroll: Overscroll,
    background: Option<[f32; 4]>,
    /// A scrollbar in place of the indicator (vertical panes only).
    scrollbar: Option<(ScrollbarOptions, Arc<ScrollbarState>)>,
    /// Its content as sections drawn by their source.
    sections: Option<Arc<SectionedContent>>,
}

impl fmt::Debug for ScrollPane {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ScrollPane")
            .field("viewport", &self.viewport.get())
            .field("axis", &self.axis)
            .field("state", &self.state)
            .finish()
    }
}

impl ScrollPane {
    /// A pane over `viewport`, scrolling along `axis`, at the start.
    pub fn new(viewport: Rect, axis: ScrollAxis) -> Self {
        let pane = Self {
            viewport: Cell::new(viewport),
            axis,
            state: ScrollState::new(),
            content_size: Cell::new(None),
            anchoring: true,
            indicator: true,
            overscroll: Overscroll::None,
            background: None,
            scrollbar: None,
            sections: None,
        };
        pane.state.with(|s| s.axis = axis);
        pane
    }

    /// Keep its place in `state`, which the app holds across rebuilds.
    #[must_use]
    pub fn with_state(mut self, state: Arc<ScrollState>) -> Self {
        state.with(|s| s.axis = self.axis);
        self.state = state;
        self
    }

    /// The content's size, when the app knows it. Without it, the content
    /// reaches the far edge of the furthest child.
    #[must_use]
    pub fn with_content_size(self, size: [f32; 2]) -> Self {
        self.content_size.set(Some(size));
        self
    }

    /// Whether the content in view stays in place when content above it
    /// changes (on by default).
    #[must_use]
    pub fn with_anchoring(mut self, on: bool) -> Self {
        self.anchoring = on;
        self
    }

    /// Whether the scroll indicator is drawn (on by default).
    #[must_use]
    pub fn with_indicator(mut self, on: bool) -> Self {
        self.indicator = on;
        self
    }

    /// How far a finger may pull the content past an end (none by default).
    #[must_use]
    pub fn with_overscroll(mut self, overscroll: Overscroll) -> Self {
        self.overscroll = overscroll;
        self
    }

    /// A scrollbar instead of the indicator
    /// (docs/design/20261028_08_SCROLLBAR_PLAN.md): the router feeds it from
    /// this pane, gives it presses in its band before the children, applies
    /// its scrubs and draws it where the indicator was. Vertical panes only:
    /// on a horizontal pane this does nothing and the indicator stays.
    #[must_use]
    pub fn with_scrollbar(mut self, options: ScrollbarOptions) -> Self {
        if self.axis == ScrollAxis::Vertical {
            let state = ScrollbarState::new();
            state.with(|s| {
                s.visibility = options.visibility;
                s.bookmarks_on = options.bookmarks_on();
                s.resolved = options.with_theme(&Default::default());
                s.sections = self.sections.clone();
            });
            self.scrollbar = Some((options, state));
        }
        self
    }

    /// Its content as `sections`, drawn by their source
    /// (docs/design/20261028_08_SCROLLBAR_PLAN.md, decisions 7-11): the
    /// router draws the sections in or near the window before the children,
    /// from the content's per-id cache, and asks the source only for those
    /// with no fresh drawing. The content's length is the sections' total
    /// height, or the furthest child's edge when that is further, unless
    /// [`with_content_size`](Self::with_content_size) gives it. With
    /// anchoring on, the first section in view stays in place when sections
    /// before it change height. A scrollbar on the pane takes its bookmarks
    /// from them.
    #[must_use]
    pub fn with_sections(mut self, sections: Arc<SectionedContent>) -> Self {
        if let Some((_, bar)) = &self.scrollbar {
            bar.set_sections(Some(sections.clone()));
        }
        self.sections = Some(sections);
        self
    }

    /// Its sections, when it has them.
    pub fn sections(&self) -> Option<&Arc<SectionedContent>> {
        self.sections.as_ref()
    }

    /// Its scrollbar's state, when it has one. Give it sections with
    /// [`ScrollbarState::set_sections`] for bookmarks; a pane with
    /// [`with_sections`](Self::with_sections) gives it its own.
    pub fn scrollbar(&self) -> Option<&Arc<ScrollbarState>> {
        self.scrollbar.as_ref().map(|(_, s)| s)
    }

    pub(crate) fn scrollbar_parts(&self) -> Option<(&ScrollbarOptions, &Arc<ScrollbarState>)> {
        self.scrollbar.as_ref().map(|(o, s)| (o, s))
    }

    /// Fill the viewport with `color` beneath the content.
    #[must_use]
    pub fn with_background(mut self, color: [f32; 4]) -> Self {
        self.background = Some(color);
        self
    }

    /// Its place.
    pub fn state(&self) -> &Arc<ScrollState> {
        &self.state
    }

    /// Its viewport, in its parent's content coordinates.
    pub fn viewport(&self) -> Rect {
        self.viewport.get()
    }

    /// The axis it scrolls along.
    pub fn axis(&self) -> ScrollAxis {
        self.axis
    }

    /// Move or resize the viewport.
    pub fn set_viewport(&self, viewport: Rect) {
        self.viewport.set(viewport);
    }

    /// Set the content's size, or `None` to take it from the children.
    pub fn set_content_size(&self, size: Option<[f32; 2]>) {
        self.content_size.set(size);
    }

    pub(crate) fn content_size(&self) -> Option<[f32; 2]> {
        self.content_size.get()
    }

    pub(crate) fn anchoring(&self) -> bool {
        self.anchoring
    }

    /// The indicator is drawn: on, and no scrollbar in its place.
    pub(crate) fn shows_indicator(&self) -> bool {
        self.indicator && self.scrollbar.is_none()
    }

    /// The overscroll limit in pixels, when it overscrolls.
    pub(crate) fn overscroll_limit(&self) -> Option<f32> {
        let len = self.axis.span(self.viewport.get()).1;
        match self.overscroll {
            Overscroll::None => None,
            Overscroll::Px(px) => (px > 0.0).then_some(px),
            Overscroll::Fraction(f) => (f > 0.0).then_some(f * len),
        }
    }

    /// The offset drawn, overscroll included, as a vector.
    pub(crate) fn drawn_offset(&self) -> [f32; 2] {
        self.state.with(|s| self.axis.vector(s.drawn()))
    }

    /// Feed its scrollbar, if any, from the pane: the viewport as the
    /// track, the offset, the content's length, and whether and when it
    /// moved, at router time `now`.
    pub(crate) fn feed_scrollbar(&self, bar_theme: &ScrollbarTheme, theme: &ScrollTheme, now: f64) {
        let Some((options, bar)) = self.scrollbar_parts() else {
            return;
        };
        let track = self.viewport.get();
        let (offset, content, moving, since) = self.state.with(|s| {
            (
                s.offset(),
                s.content,
                s.is_dragged() || s.is_coasting(theme),
                now - s.last_moved(),
            )
        });
        let resolved = options.with_theme(bar_theme);
        bar.with(|b| {
            b.resolved = resolved;
            b.feed_pane(track, offset, content, moving, since);
        });
    }

    /// Draw the indicator along the viewport's trailing edge.
    pub(crate) fn draw_indicator(&self, list: &mut DrawList, theme: &ScrollTheme, now: f64) {
        let vp = self.viewport.get();
        let (alpha, content, max, drawn) = self
            .state
            .with(|s| (s.indicator_alpha(now, theme), s.content, s.max, s.drawn()));
        let (_, len) = self.axis.span(vp);
        if alpha <= 0.0 || max <= 0.0 || content <= len {
            return;
        }
        let inset = theme.indicator_inset;
        let track = (len - 2.0 * inset).max(0.0);
        let bar = (track * len / content)
            .max(theme.indicator_min_length)
            .min(track);
        let at = inset + (track - bar) * (drawn / max).clamp(0.0, 1.0);
        let w = theme.indicator_width;
        let (position, size) = match self.axis {
            ScrollAxis::Vertical => ([vp.right() - inset - w, vp.y + at], [w, bar]),
            ScrollAxis::Horizontal => ([vp.x + at, vp.bottom() - inset - w], [bar, w]),
        };
        let mut color = theme.indicator_color;
        color[3] *= alpha;
        list.push(SdfInstance {
            kind: SdfKind::RoundedBox { radius: w * 0.5 },
            position,
            size,
            color,
            anim: 0,
        });
    }
}

impl DrawControl for ScrollPane {
    /// The background; the router draws the children and the indicator.
    fn draw(&self, list: &mut DrawList, _ctx: &DrawContext) {
        if let Some(color) = self.background {
            let vp = self.viewport.get();
            list.push(SdfInstance {
                kind: SdfKind::Box,
                position: vp.pos(),
                size: vp.size(),
                color,
                anim: 0,
            });
        }
    }
}

impl HostedControl for ScrollPane {
    fn bounds(&self) -> Rect {
        self.viewport.get()
    }

    fn hit_test(&self, pos: [f32; 2]) -> Option<(ControlPart, f32)> {
        let vp = self.viewport.get();
        vp.contains(pos)
            .then(|| (ControlPart::Body, vp.normalized_x(pos[0])))
    }

    fn handle_mouse(&self, _event: &MouseEvent, _ctx: &EventContext) -> Option<ControlAction> {
        None
    }

    fn handle_keyboard(
        &self,
        _event: &KeyboardEvent,
        _ctx: &EventContext,
    ) -> Option<ControlAction> {
        None
    }

    /// Not a Tab stop: its children are.
    fn default_flags(&self) -> NodeFlags {
        NodeFlags::NONE
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(content: f32, window: f32) -> Inner {
        let mut s = Inner::default();
        s.set_range(content, (content - window).max(0.0));
        s
    }

    fn range(s: &mut Inner, content: f32, window: f32) {
        s.set_range(content, (content - window).max(0.0));
    }

    #[test]
    fn the_range_follows_the_content_and_the_window() {
        let s = state(3000.0, 1000.0);
        assert_eq!(s.max, 2000.0);
        assert_eq!(state(500.0, 1000.0).max, 0.0);
    }

    #[test]
    fn scroll_to_clamps_to_the_range() {
        let mut s = state(3000.0, 1000.0);
        s.scroll_to(5000.0, 0.0);
        assert_eq!(s.offset, 2000.0);
        s.scroll_to(-10.0, 0.0);
        assert_eq!(s.offset, 0.0);
    }

    #[test]
    fn the_place_comes_back_when_there_is_room_again() {
        let mut s = state(1000.0, 400.0);
        s.scroll_to(300.0, 0.0);
        range(&mut s, 500.0, 400.0);
        assert_eq!(s.offset, 100.0);
        range(&mut s, 1000.0, 400.0);
        assert_eq!(s.offset, 300.0);
    }

    #[test]
    fn a_sticky_end_follows_until_scrolled_back_and_again_at_the_end() {
        let mut s = Inner {
            sticky: Some(ScrollEdge::End),
            following: true,
            ..Inner::default()
        };
        range(&mut s, 500.0, 1000.0);
        assert_eq!(s.offset, 0.0);
        range(&mut s, 3000.0, 1000.0);
        assert_eq!(s.offset, 2000.0);
        s.scroll_to(1700.0, 0.0);
        assert!(!s.following);
        range(&mut s, 4000.0, 1000.0);
        assert_eq!(s.offset, 1700.0);
        s.scroll_to(3000.0, 0.0);
        assert!(s.following);
        range(&mut s, 5000.0, 1000.0);
        assert_eq!(s.offset, 4000.0);
    }

    #[test]
    fn a_drag_past_the_end_resists_and_never_reaches_the_limit() {
        let mut s = state(3000.0, 1000.0);
        s.begin_drag(500.0, 0.0, Some(100.0));
        // Down by 200 px at the start: past it.
        s.drag_to(700.0, 0.0, Some(100.0));
        assert_eq!(s.offset, 0.0);
        assert!(s.over < 0.0 && s.over > -100.0, "{}", s.over);
        assert!(s.over.abs() < 200.0);
        s.drag_to(100_000.0, 0.0, Some(100.0));
        assert!(s.over > -100.0);
        // Back up: the overscroll goes first, then the offset.
        s.drag_to(500.0, 0.0, Some(100.0));
        assert_eq!(s.over, 0.0);
        assert_eq!(s.offset, 0.0);
        s.drag_to(400.0, 0.0, Some(100.0));
        assert_eq!(s.offset, 100.0);
    }

    #[test]
    fn without_overscroll_it_stops_dead() {
        let mut s = state(3000.0, 1000.0);
        s.begin_drag(500.0, 0.0, Some(100.0));
        s.drag_to(900.0, 0.0, None);
        assert_eq!((s.offset, s.over), (0.0, 0.0));
    }

    #[test]
    fn an_overscroll_springs_back_to_the_end() {
        let theme = ScrollTheme::default();
        let mut s = state(3000.0, 1000.0);
        s.begin_drag(500.0, 0.0, Some(100.0));
        s.drag_to(700.0, 0.0, Some(100.0));
        s.release(0.0, &theme, true);
        let mut t = 0.0;
        let mut steps = 0;
        while s.tick(0.016, t, &theme, Some(100.0)) {
            t += 0.016;
            steps += 1;
            assert!(s.over <= 0.0, "a critically damped spring does not cross");
            assert!(steps < 200, "the spring never settled");
        }
        assert_eq!(s.over, 0.0);
        assert!(steps as f32 * 0.016 < 1.0, "{steps} steps");
    }

    #[test]
    fn a_fling_decays_to_a_stop_and_one_after_a_rest_does_not_start() {
        let theme = ScrollTheme::default();
        let mut s = state(20_000.0, 1000.0);
        s.begin_drag(1500.0, 0.0, None);
        for i in 1..=6 {
            s.drag_to(1500.0 - 100.0 * i as f32, 0.016 * f64::from(i), None);
        }
        s.release(0.1, &theme, true);
        assert!(s.fling < 0.0, "{}", s.fling);
        let after = s.offset;
        let mut t = 0.1;
        let mut steps = 0;
        while s.tick(0.016, t, &theme, None) {
            t += 0.016;
            steps += 1;
            assert!(steps < 10_000);
        }
        assert!(s.offset > after && s.offset <= s.max);

        let mut s = state(20_000.0, 1000.0);
        s.begin_drag(1500.0, 0.0, None);
        for i in 1..=6 {
            s.drag_to(1500.0 - 100.0 * i as f32, 0.016 * f64::from(i), None);
        }
        s.release(0.096 + 0.2, &theme, true);
        assert_eq!(s.fling, 0.0, "the finger rested before lifting");
    }

    #[test]
    fn a_fling_into_an_overscrolling_end_bounces_back() {
        let theme = ScrollTheme::default();
        let mut s = state(2000.0, 1000.0);
        s.scroll_to(900.0, 0.0);
        s.fling = -3000.0;
        let mut t = 0.0;
        let mut went_past = false;
        let mut steps = 0;
        while s.tick(0.016, t, &theme, Some(100.0)) {
            t += 0.016;
            went_past |= s.over > 0.0;
            steps += 1;
            assert!(steps < 500);
        }
        assert!(went_past);
        assert_eq!((s.offset, s.over), (1000.0, 0.0));
    }

    #[test]
    fn the_indicator_stays_then_fades() {
        let theme = ScrollTheme::default();
        let mut s = state(3000.0, 1000.0);
        assert_eq!(s.indicator_alpha(0.0, &theme), 0.0, "never moved");
        s.scroll_to(100.0, 1.0);
        assert_eq!(s.indicator_alpha(1.4, &theme), 1.0);
        let mid = s.indicator_alpha(1.0 + 0.5 + 0.075, &theme);
        assert!(mid > 0.4 && mid < 0.6, "{mid}");
        assert_eq!(s.indicator_alpha(1.0 + 0.5 + 0.2, &theme), 0.0);
    }
}
