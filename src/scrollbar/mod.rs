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

use std::cell::Cell;
use std::fmt;
use std::sync::Arc;

use libmsdf::drawlist::{DrawList, SdfInstance, SdfKind};

use crate::control::HostedControl;
use crate::event::{ControlAction, ControlPart, EventContext, KeyboardEvent, MouseEvent};
use crate::flags::NodeFlags;
use crate::rect::Rect;
use crate::sections::SectionId;
use crate::theme::ControlTheme;
use crate::traits::{DrawContext, DrawControl};

pub use bookmarks::BookmarkPill;
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
    let (thumb, alpha, r, pills, fade) = state.with(|s| {
        let pills = pills_of(s, ctx);
        (s.thumb(), s.alpha(), s.resolved, pills, s.bookmarks_alpha())
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
    if pills.is_empty() {
        return;
    }
    if let Some(c) = clip {
        list.push_clip(c.pos(), c.size());
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
        self.state.with(|s| {
            s.track = track;
            s.visibility = visibility;
            s.bookmarks_on = on;
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
}
