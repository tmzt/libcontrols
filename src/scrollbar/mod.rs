//! A scrollbar backported from flankr's agent view
//! (docs/design/20261028_08_SCROLLBAR_PLAN.md): a thin proportional thumb at
//! the track's right edge, a band around it that takes a press and scrubs,
//! `None`/`OnScroll`/`Always` visibility, and bookmarks drawn while the thumb
//! is held.
//!
//! Call-driven: on a pane ([`ScrollPane::with_scrollbar`]) the router feeds
//! it, routes its band, applies its scrubs and draws it; standalone, a
//! [`Scrollbar`] node is fed with [`ScrollbarState::set_metrics`] and
//! [`ScrollbarState::set_scrolling`] and reports its scrubs as
//! [`ScrollbarEvent`]s.
//!
//! [`ScrollPane::with_scrollbar`]: crate::ScrollPane::with_scrollbar

mod bookmarks;
mod state;

pub(crate) use bookmarks::cut_to_fit;

use std::cell::Cell;
use std::fmt;
use std::sync::Arc;

use libmsdf::drawlist::{DrawList, SdfInstance, SdfKind};

use crate::control::HostedControl;
use crate::event::{ControlAction, ControlPart, EventContext, KeyboardEvent, MouseEvent};
use crate::flags::NodeFlags;
use crate::rect::Rect;
use crate::sections::{BookmarkScale, BookmarkSize, SectionId};
use crate::theme::ControlTheme;
use crate::traits::{DrawContext, DrawControl};

pub use bookmarks::BookmarkPill;
pub(crate) use state::LabelFlight;
pub use state::ScrollbarState;

/// When the thumb is shown (the app's three settings).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum ScrollbarVisibility {
    /// No thumb and no band: presses there go to the content.
    None,
    /// While scrolling and `hold` after, then a `fade`; the band always.
    #[default]
    OnScroll,
    /// The thumb always; the band always.
    Always,
}

/// The defaults every scrollbar starts from (spec 8.5), in router pixels
/// and seconds: flankr's agent-view numbers.
#[derive(Copy, Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct ScrollbarTheme {
    /// The thumb's thickness.
    pub width: f32,
    /// The thumb's shortest length.
    pub min_length: f32,
    /// The band's slop either side of the thumb; also the thumb's inset
    /// from the track's right edge.
    pub slop: f32,
    /// The thumb's colour (`None`: the theme's `text_color`).
    pub color: Option<[f32; 4]>,
    /// The thumb's opacity when shown, times its colour's.
    pub alpha: f32,
    /// How long (s) the thumb and the bookmarks stay after scrolling or a
    /// release.
    pub hold: f32,
    /// How long (s) they take to fade.
    pub fade: f32,
    /// The bookmarks' colour (`None`: the theme's `text_color`).
    pub bookmark_color: Option<[f32; 4]>,
    pub bookmark_bg_alpha: f32,
    pub bookmark_text_alpha: f32,
    /// The pill nearest the thumb.
    pub bookmark_near_bg_alpha: f32,
    pub bookmark_near_text_alpha: f32,
    pub bookmark_font_size: f32,
    /// Times the font size.
    pub bookmark_line_height: f32,
    pub bookmark_pad_x: f32,
    pub bookmark_pad_y: f32,
    /// The text is cut to the track's width less twice this and twice
    /// `bookmark_pad_x`.
    pub bookmark_side_margin: f32,
    /// The least space between two pills shown together.
    pub bookmark_gap: f32,
    /// A Markdown bookmark's code chip's padding either side, drawn without
    /// faces (the `markdown` feature; unused without it).
    pub bookmark_code_pad: f32,
    /// That chip's corner radius.
    pub bookmark_code_radius: f32,
    /// The thumb label's font size (`Medium`).
    pub thumb_label_font_size: f32,
    /// The thumb label pill's padding across.
    pub thumb_label_pad_x: f32,
    /// The thumb label pill's padding along.
    pub thumb_label_pad_y: f32,
    /// From the label pill's right edge to the thumb's left edge.
    pub thumb_label_gap: f32,
    /// The label pill's background alpha, times its colour's.
    pub thumb_label_bg_alpha: f32,
    /// The label's text alpha, times its colour's.
    pub thumb_label_text_alpha: f32,
    /// `BookmarkScale::Small`, times `bookmark_font_size` (0.5-1).
    pub bookmark_scale_small: f32,
    /// `BookmarkScale::Large`, times `bookmark_font_size` (1-2.5).
    pub bookmark_scale_large: f32,
    /// The label pill's background (`None`: the theme's `bg_color`).
    pub thumb_label_bg: Option<[f32; 4]>,
    /// The label's text colour (`None`: the theme's `text_color`).
    pub thumb_label_color: Option<[f32; 4]>,
    /// While scrubbing with `snap_to_bookmarks`, the thumb's centre this
    /// close (track pixels) to a bookmark pill's centre snaps to it.
    pub snap_distance: f32,
}

impl Default for ScrollbarTheme {
    fn default() -> Self {
        Self {
            width: 4.0,
            min_length: 24.0,
            slop: 4.0,
            color: None,
            alpha: 0.35,
            hold: 2.0,
            fade: 0.3,
            bookmark_color: None,
            bookmark_bg_alpha: 0.10,
            bookmark_text_alpha: 0.60,
            bookmark_near_bg_alpha: 0.20,
            bookmark_near_text_alpha: 0.90,
            bookmark_font_size: 13.0,
            bookmark_line_height: 1.35,
            bookmark_pad_x: 12.0,
            bookmark_pad_y: 4.0,
            bookmark_side_margin: 24.0,
            bookmark_gap: 2.0,
            bookmark_code_pad: 2.0,
            bookmark_code_radius: 3.0,
            thumb_label_font_size: 13.0,
            thumb_label_pad_x: 12.0,
            thumb_label_pad_y: 4.0,
            thumb_label_gap: 8.0,
            thumb_label_bg_alpha: 0.85,
            thumb_label_text_alpha: 1.0,
            bookmark_scale_small: 0.85,
            bookmark_scale_large: 1.30,
            thumb_label_bg: None,
            thumb_label_color: None,
            snap_distance: 16.0,
        }
    }
}

/// One scrollbar's overrides of its theme (`None` takes the theme's value)
/// and its own settings.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct ScrollbarOptions {
    pub width: Option<f32>,
    pub min_length: Option<f32>,
    pub slop: Option<f32>,
    pub color: Option<[f32; 4]>,
    pub alpha: Option<f32>,
    pub hold: Option<f32>,
    pub fade: Option<f32>,
    pub bookmark_color: Option<[f32; 4]>,
    pub bookmark_bg_alpha: Option<f32>,
    pub bookmark_text_alpha: Option<f32>,
    pub bookmark_near_bg_alpha: Option<f32>,
    pub bookmark_near_text_alpha: Option<f32>,
    pub bookmark_font_size: Option<f32>,
    pub bookmark_line_height: Option<f32>,
    pub bookmark_pad_x: Option<f32>,
    pub bookmark_pad_y: Option<f32>,
    pub bookmark_side_margin: Option<f32>,
    pub bookmark_gap: Option<f32>,
    pub bookmark_code_pad: Option<f32>,
    pub bookmark_code_radius: Option<f32>,
    /// Per instance: `OnScroll` unless set.
    pub visibility: ScrollbarVisibility,
    /// Per instance: bookmarks drawn while held, `true` unless set.
    pub bookmarks: Option<bool>,
    pub thumb_label_font_size: Option<f32>,
    pub thumb_label_pad_x: Option<f32>,
    pub thumb_label_pad_y: Option<f32>,
    pub thumb_label_gap: Option<f32>,
    pub thumb_label_bg_alpha: Option<f32>,
    pub thumb_label_text_alpha: Option<f32>,
    pub bookmark_scale_small: Option<f32>,
    pub bookmark_scale_large: Option<f32>,
    pub thumb_label_bg: Option<[f32; 4]>,
    pub thumb_label_color: Option<[f32; 4]>,
    pub snap_distance: Option<f32>,
    /// Per instance: a scrub snaps to a bookmark within `snap_distance`
    /// (`false` unless set).
    pub snap_to_bookmarks: bool,
    /// Per instance: a label on the thumb while scrolling (`false` unless
    /// set).
    pub thumb_label: bool,
    /// Per instance: during a counted flight, the boundaries still to pass
    /// after the label ("↓ 3"; `false` unless set).
    pub skip_count: bool,
}

macro_rules! setter {
    ($name:ident: $t:ty) => {
        #[doc = concat!("Override the theme's `", stringify!($name), "`.")]
        #[must_use]
        pub fn $name(mut self, value: $t) -> Self {
            self.$name = Some(value);
            self
        }
    };
}

impl ScrollbarOptions {
    /// Overrides nothing.
    pub fn new() -> Self {
        Self::default()
    }

    setter!(width: f32);
    setter!(min_length: f32);
    setter!(slop: f32);
    setter!(color: [f32; 4]);
    setter!(alpha: f32);
    setter!(hold: f32);
    setter!(fade: f32);
    setter!(bookmark_color: [f32; 4]);
    setter!(bookmark_bg_alpha: f32);
    setter!(bookmark_text_alpha: f32);
    setter!(bookmark_near_bg_alpha: f32);
    setter!(bookmark_near_text_alpha: f32);
    setter!(bookmark_font_size: f32);
    setter!(bookmark_line_height: f32);
    setter!(bookmark_pad_x: f32);
    setter!(bookmark_pad_y: f32);
    setter!(bookmark_side_margin: f32);
    setter!(bookmark_gap: f32);
    setter!(bookmark_code_pad: f32);
    setter!(bookmark_code_radius: f32);
    setter!(thumb_label_font_size: f32);
    setter!(thumb_label_pad_x: f32);
    setter!(thumb_label_pad_y: f32);
    setter!(thumb_label_gap: f32);
    setter!(thumb_label_bg_alpha: f32);
    setter!(thumb_label_text_alpha: f32);
    setter!(bookmark_scale_small: f32);
    setter!(bookmark_scale_large: f32);
    setter!(thumb_label_bg: [f32; 4]);
    setter!(thumb_label_color: [f32; 4]);
    setter!(snap_distance: f32);

    /// While scrubbing, snap to a bookmark when the thumb's centre comes
    /// within `snap_distance` of its pill's centre: the scrub's offset is
    /// that bookmark's section top, and `ScrollbarEvent::Snapped` is sent
    /// the first time each snap takes.
    #[must_use]
    pub fn snap_to_bookmarks(mut self, on: bool) -> Self {
        self.snap_to_bookmarks = on;
        self
    }

    /// Show a label on the thumb while scrolling: the bookmark of the
    /// section at the window's top, or the app's formatter's text
    /// ([`SectionedContent::set_label_format`](crate::SectionedContent::set_label_format)).
    #[must_use]
    pub fn thumb_label(mut self, on: bool) -> Self {
        self.thumb_label = on;
        self
    }

    /// During a counted flight, show the boundaries still to pass after
    /// the label, with the direction ("↓ 3").
    #[must_use]
    pub fn skip_count(mut self, on: bool) -> Self {
        self.skip_count = on;
        self
    }

    /// When the thumb is shown.
    #[must_use]
    pub fn visibility(mut self, visibility: ScrollbarVisibility) -> Self {
        self.visibility = visibility;
        self
    }

    /// Whether bookmarks are drawn while the thumb is held.
    #[must_use]
    pub fn bookmarks(mut self, on: bool) -> Self {
        self.bookmarks = Some(on);
        self
    }

    pub(crate) fn bookmarks_on(&self) -> bool {
        self.bookmarks.unwrap_or(true)
    }

    /// Each `Some` wins; each `None` takes the theme's value.
    pub fn with_theme(&self, theme: &ScrollbarTheme) -> ScrollbarResolved {
        ScrollbarResolved {
            width: self.width.unwrap_or(theme.width),
            min_length: self.min_length.unwrap_or(theme.min_length),
            slop: self.slop.unwrap_or(theme.slop),
            color: self.color.or(theme.color),
            alpha: self.alpha.unwrap_or(theme.alpha),
            hold: self.hold.unwrap_or(theme.hold),
            fade: self.fade.unwrap_or(theme.fade),
            bookmark_color: self.bookmark_color.or(theme.bookmark_color),
            bookmark_bg_alpha: self.bookmark_bg_alpha.unwrap_or(theme.bookmark_bg_alpha),
            bookmark_text_alpha: self
                .bookmark_text_alpha
                .unwrap_or(theme.bookmark_text_alpha),
            bookmark_near_bg_alpha: self
                .bookmark_near_bg_alpha
                .unwrap_or(theme.bookmark_near_bg_alpha),
            bookmark_near_text_alpha: self
                .bookmark_near_text_alpha
                .unwrap_or(theme.bookmark_near_text_alpha),
            bookmark_font_size: self.bookmark_font_size.unwrap_or(theme.bookmark_font_size),
            bookmark_line_height: self
                .bookmark_line_height
                .unwrap_or(theme.bookmark_line_height),
            bookmark_pad_x: self.bookmark_pad_x.unwrap_or(theme.bookmark_pad_x),
            bookmark_pad_y: self.bookmark_pad_y.unwrap_or(theme.bookmark_pad_y),
            bookmark_side_margin: self
                .bookmark_side_margin
                .unwrap_or(theme.bookmark_side_margin),
            bookmark_gap: self.bookmark_gap.unwrap_or(theme.bookmark_gap),
            bookmark_code_pad: self.bookmark_code_pad.unwrap_or(theme.bookmark_code_pad),
            bookmark_code_radius: self
                .bookmark_code_radius
                .unwrap_or(theme.bookmark_code_radius),
            thumb_label_font_size: self
                .thumb_label_font_size
                .unwrap_or(theme.thumb_label_font_size),
            thumb_label_pad_x: self.thumb_label_pad_x.unwrap_or(theme.thumb_label_pad_x),
            thumb_label_pad_y: self.thumb_label_pad_y.unwrap_or(theme.thumb_label_pad_y),
            thumb_label_gap: self.thumb_label_gap.unwrap_or(theme.thumb_label_gap),
            thumb_label_bg_alpha: self
                .thumb_label_bg_alpha
                .unwrap_or(theme.thumb_label_bg_alpha),
            thumb_label_text_alpha: self
                .thumb_label_text_alpha
                .unwrap_or(theme.thumb_label_text_alpha),
            bookmark_scale_small: self
                .bookmark_scale_small
                .unwrap_or(theme.bookmark_scale_small),
            bookmark_scale_large: self
                .bookmark_scale_large
                .unwrap_or(theme.bookmark_scale_large),
            thumb_label_bg: self.thumb_label_bg.or(theme.thumb_label_bg),
            thumb_label_color: self.thumb_label_color.or(theme.thumb_label_color),
            snap_distance: self.snap_distance.unwrap_or(theme.snap_distance),
        }
    }
}

/// A scrollbar's options resolved against its theme: what it reads. The two
/// colours left `None` are the `ControlTheme`'s `text_color`, read at draw.
#[derive(Copy, Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct ScrollbarResolved {
    pub width: f32,
    pub min_length: f32,
    pub slop: f32,
    pub color: Option<[f32; 4]>,
    pub alpha: f32,
    pub hold: f32,
    pub fade: f32,
    pub bookmark_color: Option<[f32; 4]>,
    pub bookmark_bg_alpha: f32,
    pub bookmark_text_alpha: f32,
    pub bookmark_near_bg_alpha: f32,
    pub bookmark_near_text_alpha: f32,
    pub bookmark_font_size: f32,
    pub bookmark_line_height: f32,
    pub bookmark_pad_x: f32,
    pub bookmark_pad_y: f32,
    pub bookmark_side_margin: f32,
    pub bookmark_gap: f32,
    pub bookmark_code_pad: f32,
    pub bookmark_code_radius: f32,
    pub thumb_label_font_size: f32,
    pub thumb_label_pad_x: f32,
    pub thumb_label_pad_y: f32,
    pub thumb_label_gap: f32,
    pub thumb_label_bg_alpha: f32,
    pub thumb_label_text_alpha: f32,
    pub bookmark_scale_small: f32,
    pub bookmark_scale_large: f32,
    pub thumb_label_bg: Option<[f32; 4]>,
    pub thumb_label_color: Option<[f32; 4]>,
    pub snap_distance: f32,
}

impl ScrollbarResolved {
    /// A bookmark's pill font size.
    pub fn bookmark_size(&self, size: BookmarkSize) -> f32 {
        match size {
            BookmarkSize::Px(px) if px.is_finite() => px.max(1.0),
            BookmarkSize::Themed(BookmarkScale::Small) => {
                self.bookmark_font_size * self.bookmark_scale_small.clamp(0.5, 1.0)
            }
            BookmarkSize::Themed(BookmarkScale::Large) => {
                self.bookmark_font_size * self.bookmark_scale_large.clamp(1.0, 2.5)
            }
            _ => self.bookmark_font_size,
        }
    }
}

/// What a scrollbar reports, read with [`ScrollbarState::take_events`].
/// `section` is the section at the offset's top edge and `bookmark` the
/// bookmark nearest the thumb, when the state has sections.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum ScrollbarEvent {
    /// The thumb was grabbed at `offset`.
    #[non_exhaustive]
    Grabbed { offset: f32 },
    /// The thumb scrubbed to `offset` (on a pane, already applied).
    #[non_exhaustive]
    Scrub {
        offset: f32,
        section: Option<SectionId>,
        bookmark: Option<SectionId>,
    },
    /// The thumb was let go of at `offset`.
    #[non_exhaustive]
    Released {
        offset: f32,
        section: Option<SectionId>,
        bookmark: Option<SectionId>,
    },
    /// A scrub snapped to bookmark `id`'s section top, `offset`
    /// (`snap_to_bookmarks`), sent the first time each snap takes.
    #[non_exhaustive]
    Snapped { id: SectionId, offset: f32 },
}

/// Draw the thumb and, while held or fading, the bookmarks, from the state.
/// `clip` is the track in screen pixels, when the caller knows it.
pub(crate) fn draw_bar(
    list: &mut DrawList,
    ctx: &DrawContext,
    state: &ScrollbarState,
    clip: Option<Rect>,
) {
    #[cfg(feature = "markdown")]
    let faces = state.with(|s| s.faces.clone());
    let (thumb, alpha, r, pills, fade, label) = state.with(|s| {
        let pills = pills_of(s, ctx);
        let label = label_of(s, ctx);
        (
            s.thumb(),
            s.alpha(),
            s.resolved,
            pills,
            s.bookmarks_alpha(),
            label,
        )
    });
    if let Some(t) = thumb
        && alpha > 0.0
    {
        let mut color = r.color.unwrap_or(ctx.theme.text_color);
        color[3] *= alpha;
        list.push(SdfInstance {
            kind: SdfKind::RoundedBox { radius: t.w * 0.5 },
            position: t.pos(),
            size: t.size(),
            color,
            anim: 0,
        });
    }
    if pills.is_empty() && label.is_none() {
        return;
    }
    if let Some(c) = clip {
        list.push_clip(c.pos(), c.size());
    }
    if let Some((pill, a)) = &label {
        bookmarks::draw_label(
            list,
            ctx,
            &r,
            pill,
            *a,
            #[cfg(feature = "markdown")]
            faces.as_deref(),
        );
    }
    bookmarks::draw(
        list,
        ctx,
        &r,
        &pills,
        fade,
        #[cfg(feature = "markdown")]
        faces.as_deref(),
    );
    if clip.is_some() {
        list.push_clip_end();
    }
}

/// The thumb label as it is drawn now, with its alpha.
pub(crate) fn label_of(s: &state::Inner, ctx: &DrawContext) -> Option<(bookmarks::LabelPill, f32)> {
    let alpha = s.label_alpha();
    if alpha <= 0.0 {
        return None;
    }
    let thumb = s.thumb()?;
    let (id, text) = s.label()?;
    let r = s.resolved;
    let m = bookmarks::PillMeasure::new(
        ctx,
        &r,
        #[cfg(feature = "markdown")]
        s.faces.as_deref(),
    );
    Some((
        bookmarks::label_layout(s.track, thumb, &r, id, &text, &m),
        alpha,
    ))
}

/// The bookmarks as they are drawn now.
fn pills_of(s: &state::Inner, ctx: &DrawContext) -> Vec<BookmarkPill> {
    if !s.bookmarks_on || s.bookmarks_alpha() <= 0.0 {
        return Vec::new();
    }
    let (Some(sections), Some(thumb)) = (&s.sections, s.thumb()) else {
        return Vec::new();
    };
    let r = s.resolved;
    let m = bookmarks::PillMeasure::new(
        ctx,
        &r,
        #[cfg(feature = "markdown")]
        s.faces.as_deref(),
    );
    bookmarks::layout(
        s.track,
        &r,
        s.content,
        thumb.y + thumb.h * 0.5,
        sections.placed_bookmarks(),
        &m,
    )
}

/// The control: a track (the content's window on the screen) with the
/// thumb at its right edge and the bookmarks over it. Add it after the
/// content it scrolls, so it is hit first and drawn over it.
pub struct Scrollbar {
    track: Cell<Rect>,
    options: ScrollbarOptions,
    state: Arc<ScrollbarState>,
}

impl fmt::Debug for Scrollbar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Scrollbar")
            .field("track", &self.track.get())
            .field("options", &self.options)
            .field("state", &self.state)
            .finish()
    }
}

impl Scrollbar {
    /// A scrollbar over `track` with its own state.
    pub fn new(track: Rect, options: ScrollbarOptions) -> Self {
        let bar = Self {
            track: Cell::new(track),
            options,
            state: ScrollbarState::new(),
        };
        bar.sync(&ScrollbarTheme::default());
        bar
    }

    /// Use `state`, which the app holds and feeds.
    #[must_use]
    pub fn with_state(mut self, state: Arc<ScrollbarState>) -> Self {
        let resolved = self.state.with(|s| s.resolved);
        self.state = state;
        self.state.with(|s| s.resolved = resolved);
        self.sync_settings();
        self
    }

    /// Its state.
    pub fn state(&self) -> &Arc<ScrollbarState> {
        &self.state
    }

    /// Move or resize the track.
    pub fn set_track(&self, track: Rect) {
        self.track.set(track);
        self.state.with(|s| s.track = track);
    }

    /// The track.
    pub fn track(&self) -> Rect {
        self.track.get()
    }

    /// Its options resolved against `theme`.
    pub fn resolved(&self, theme: &ControlTheme) -> ScrollbarResolved {
        self.options.with_theme(&theme.scrollbar)
    }

    /// The thumb on the screen, when there is one.
    pub fn thumb_rect(&self, theme: &ControlTheme) -> Option<Rect> {
        let r = self.resolved(theme);
        let track = self.track.get();
        self.state
            .with(|s| state::thumb_rect(track, &r, s.offset, s.content, s.window))
    }

    /// The band on the screen, when there is one (a thumb, and not `None`).
    pub fn band_rect(&self, theme: &ControlTheme) -> Option<Rect> {
        if self.options.visibility == ScrollbarVisibility::None {
            return None;
        }
        let r = self.resolved(theme);
        self.thumb_rect(theme)
            .map(|t| state::band_rect(self.track.get(), &r, t))
    }

    /// The bookmarks as drawn this frame.
    pub fn bookmark_layout(&self, ctx: &DrawContext) -> Vec<BookmarkPill> {
        self.sync(&ctx.theme.scrollbar);
        self.state.with(|s| pills_of(s, ctx))
    }

    fn sync_settings(&self) {
        let track = self.track.get();
        let visibility = self.options.visibility;
        let on = self.options.bookmarks_on();
        let (label, skip) = (self.options.thumb_label, self.options.skip_count);
        let snap = self.options.snap_to_bookmarks;
        self.state.with(|s| {
            s.snap_on = snap;
            s.track = track;
            s.visibility = visibility;
            s.bookmarks_on = on;
            s.thumb_label_on = label;
            s.skip_count_on = skip;
        });
    }

    fn sync(&self, theme: &crate::scrollbar::ScrollbarTheme) {
        self.sync_settings();
        let r = self.options.with_theme(theme);
        self.state.with(|s| s.resolved = r);
    }
}

impl DrawControl for Scrollbar {
    fn draw(&self, list: &mut DrawList, ctx: &DrawContext) {
        self.sync(&ctx.theme.scrollbar);
        draw_bar(list, ctx, &self.state, None);
    }
}

impl HostedControl for Scrollbar {
    fn bounds(&self) -> Rect {
        self.track.get()
    }

    /// Only the band: a press elsewhere on the track reaches the content.
    fn hit_test(&self, pos: [f32; 2]) -> Option<(ControlPart, f32)> {
        let band = self.state.with(|s| s.band())?;
        band.contains(pos)
            .then(|| (ControlPart::Thumb, self.track.get().normalized_x(pos[0])))
    }

    fn handle_mouse(&self, event: &MouseEvent, _ctx: &EventContext) -> Option<ControlAction> {
        match *event {
            MouseEvent::Down { pos, .. } => {
                self.state.press(pos);
            }
            MouseEvent::Drag { current_pos, .. } => self.state.drag_to(current_pos),
            MouseEvent::Up { .. } => self.state.release(),
            _ => {}
        }
        None
    }

    fn handle_keyboard(
        &self,
        _event: &KeyboardEvent,
        _ctx: &EventContext,
    ) -> Option<ControlAction> {
        None
    }

    /// Not a Tab stop.
    fn default_flags(&self) -> NodeFlags {
        NodeFlags::NONE
    }

    fn claims_drag(&self, part: ControlPart) -> bool {
        part == ControlPart::Thumb
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn options_resolve_against_the_theme_and_the_defaults_are_the_apps() {
        let theme = ScrollbarTheme {
            hold: 1.0,
            ..ScrollbarTheme::default()
        };
        let r = ScrollbarOptions::default()
            .min_length(30.0)
            .with_theme(&theme);
        assert_eq!((r.min_length, r.hold), (30.0, 1.0));
        let d = ScrollbarTheme::default();
        assert_eq!(
            (d.width, d.min_length, d.slop, d.alpha, d.hold, d.fade),
            (4.0, 24.0, 4.0, 0.35, 2.0, 0.3)
        );
        assert_eq!(
            (
                d.bookmark_bg_alpha,
                d.bookmark_text_alpha,
                d.bookmark_near_bg_alpha,
                d.bookmark_near_text_alpha
            ),
            (0.10, 0.60, 0.20, 0.90)
        );
        assert_eq!(
            ScrollbarOptions::default().visibility,
            ScrollbarVisibility::OnScroll
        );
        assert!(ScrollbarOptions::default().bookmarks_on());
        assert!(!ScrollbarOptions::default().bookmarks(false).bookmarks_on());
        assert_eq!(ControlTheme::default().scrollbar, ScrollbarTheme::default());
    }

    // ---- Plan 20261119_01, step 6: the thumb label ----

    use crate::sections::{BookmarkText, Section, SectionId, SectionedContent};

    fn labelled(content: f32, offset: f32, mark: &str) -> state::Inner {
        let c = SectionedContent::new();
        c.set_sections(vec![Section::new(SectionId(0), content)]);
        c.add_bookmark(SectionId(0), BookmarkText::Text(mark.into()));
        let mut s = state::Inner::default();
        s.thumb_label_on = true;
        s.sections = Some(c);
        s.feed_pane(
            Rect::new(0.0, 0.0, 400.0, 1000.0),
            offset,
            content,
            true,
            0.0,
        );
        s
    }

    #[test]
    fn the_label_pill_sits_left_of_the_thumb_inside_the_track() {
        let ctx = DrawContext::default();
        // A 24 px thumb at the top: centred on it would leave the track.
        let s = labelled(100_000.0, 0.0, "November 2026");
        let (pill, alpha) = label_of(&s, &ctx).expect("a label");
        assert_eq!(alpha, 1.0);
        let thumb = s.thumb().unwrap();
        assert_eq!(pill.rect.right(), thumb.x - 8.0);
        assert_eq!(pill.rect.y, 0.0, "kept in the track");
        assert!((pill.rect.h - 25.55).abs() < 1e-3);
        let s = labelled(100_000.0, 99_000.0, "November 2026");
        let (pill, _) = label_of(&s, &ctx).unwrap();
        assert!((pill.rect.bottom() - 1000.0).abs() < 1e-3);
        // Mid-track: centred on the thumb.
        let s = labelled(4000.0, 1500.0, "x");
        let (pill, _) = label_of(&s, &ctx).unwrap();
        let t = s.thumb().unwrap();
        assert!((pill.rect.y + pill.rect.h * 0.5 - (t.y + t.h * 0.5)).abs() < 1e-3);
    }

    #[test]
    fn the_label_is_cut_to_half_the_track_less_its_padding() {
        let ctx = DrawContext::default();
        let s = labelled(4000.0, 0.0, &"y".repeat(100));
        let (pill, _) = label_of(&s, &ctx).unwrap();
        assert!(pill.text.ends_with('\u{2026}'), "{}", pill.text);
        assert!(ctx.text_advance(&pill.text, 13.0) <= 200.0 - 24.0);
        let s = labelled(4000.0, 0.0, "short");
        assert_eq!(label_of(&s, &ctx).unwrap().0.text, "short");
    }

    #[test]
    fn the_label_draws_one_rounded_box_and_its_text() {
        let ctx = DrawContext::default();
        let s = labelled(4000.0, 0.0, "November 2026");
        let (pill, alpha) = label_of(&s, &ctx).unwrap();
        let mut list = DrawList::new();
        bookmarks::draw_label(
            &mut list,
            &ctx,
            &s.resolved,
            &pill,
            alpha,
            #[cfg(feature = "markdown")]
            None,
        );
        // Without an atlas the text is measured, not drawn: one box.
        assert_eq!(list.instances.len(), 1);
        let b = list.instances[0];
        assert!(matches!(b.kind, SdfKind::RoundedBox { radius } if radius == pill.rect.h * 0.5));
        assert_eq!(b.position, pill.rect.pos());
        let bg = ctx.theme.bg_color;
        assert!((b.color[3] - bg[3] * 0.85).abs() < 1e-6);
        assert_eq!(pill.text, "November 2026");
    }

    #[test]
    fn bookmark_sizes_resolve_against_the_theme() {
        use crate::sections::{BookmarkScale, BookmarkSize};
        let r = ScrollbarOptions::default().with_theme(&ScrollbarTheme::default());
        let size = |s| r.bookmark_size(s);
        assert!((size(BookmarkSize::Themed(BookmarkScale::Small)) - 13.0 * 0.85).abs() < 1e-5);
        assert_eq!(size(BookmarkSize::Themed(BookmarkScale::Medium)), 13.0);
        assert!((size(BookmarkSize::Themed(BookmarkScale::Large)) - 13.0 * 1.3).abs() < 1e-5);
        assert_eq!(size(BookmarkSize::Px(20.0)), 20.0);
        let d = ScrollbarTheme::default();
        assert_eq!(
            (
                d.thumb_label_font_size,
                d.thumb_label_pad_x,
                d.thumb_label_pad_y,
                d.thumb_label_gap,
                d.thumb_label_bg_alpha,
                d.thumb_label_text_alpha
            ),
            (13.0, 12.0, 4.0, 8.0, 0.85, 1.0)
        );
        assert_eq!((d.thumb_label_bg, d.thumb_label_color), (None, None));
        assert_eq!(
            (d.bookmark_scale_small, d.bookmark_scale_large),
            (0.85, 1.30)
        );
        let o = ScrollbarOptions::default();
        assert!(!o.thumb_label && !o.skip_count);
    }
}
