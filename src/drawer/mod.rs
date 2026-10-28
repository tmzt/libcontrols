//! A drawer: a sheet on one edge of a main view, driven by calls, composited
//! from offscreen layers while visible
//! (docs/design/20261028_01_DRAWER_PLAN.md with its amendments, and its
//! parent docs/design/20261028_04_DRAWER_LAYOUT_PLAN.md).

mod composite;
mod layers;
mod layout;
mod state;

use std::cell::Cell;
use std::sync::Arc;

use libmsdf::PaneMark;
use libmsdf::drawlist::{DrawList, Elevation, SdfInstance, SdfKind};

use crate::ease::CubicBezier;
use crate::event::{InputEvent, MouseEvent};
use crate::rect::Rect;
use crate::router::{EventResponse, NodeId, Router};
use crate::theme::ControlTheme;
use crate::traits::DrawContext;

pub(crate) use composite::CompositeParams;
pub use layers::{DrawerLayers, LayerPlan};
pub use layout::DrawerLayout;
pub use state::{DrawerEvent, DrawerPhase, DrawerReason, DrawerState};

/// The edge a drawer's sheet comes in from. One per drawer, fixed when it is
/// made.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum DrawerEdge {
    #[default]
    Left,
    Right,
    Top,
    Bottom,
}

impl DrawerEdge {
    /// Left and Right: the sheet's extent is a width.
    pub fn is_horizontal(self) -> bool {
        matches!(self, Self::Left | Self::Right)
    }

    /// 0 left, 1 right, 2 top, 3 bottom (the shader's numbering).
    pub(crate) fn index(self) -> u32 {
        match self {
            Self::Left => 0,
            Self::Right => 1,
            Self::Top => 2,
            Self::Bottom => 3,
        }
    }

    /// The unit vector from the edge into the viewport (the way the sheet
    /// moves as it opens).
    pub(crate) fn dir(self) -> [f32; 2] {
        match self {
            Self::Left => [1.0, 0.0],
            Self::Right => [-1.0, 0.0],
            Self::Top => [0.0, 1.0],
            Self::Bottom => [0.0, -1.0],
        }
    }

    /// Per-corner radii `[tl, tr, br, bl]` rounding only the open edge's
    /// corners (the side facing the main view).
    pub(crate) fn corner_radii(self, r: f32) -> [f32; 4] {
        match self {
            Self::Left => [0.0, r, r, 0.0],
            Self::Right => [r, 0.0, 0.0, r],
            Self::Top => [0.0, 0.0, r, r],
            Self::Bottom => [r, r, 0.0, 0.0],
        }
    }
}

/// The sheet's size across its edge.
#[derive(Copy, Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum DrawerSize {
    /// A fixed size in router pixels, at most the viewport's.
    Px(f32),
    /// A share of the viewport across the edge, at most `max` pixels.
    Share { share: f32, max: f32 },
}

impl DrawerSize {
    /// The extent for a viewport `across` pixels across the edge.
    pub fn extent(self, across: f32) -> f32 {
        let across = across.max(0.0);
        let e = match self {
            Self::Px(px) => px,
            Self::Share { share, max } => (share * across).min(max),
        };
        if e.is_finite() {
            e.clamp(0.0, across)
        } else {
            across
        }
    }
}

/// Keep the main layer live while the drawer is visible, or freeze it.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum MainUpdates {
    #[default]
    Live,
    Frozen,
}

/// The defaults every drawer of an app starts from, in `ControlTheme::drawer`
/// (the theming standard, spec section 8.5).
#[derive(Copy, Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct DrawerTheme {
    /// The sheet's extent for a Left or Right drawer.
    pub size: DrawerSize,
    /// The sheet's extent for a Top or Bottom drawer.
    pub size_vertical: DrawerSize,
    /// The scrim over the main view at t = 1; its alpha scales with t.
    pub scrim: [f32; 4],
    /// `None`: the theme's `bg_color`.
    pub sheet_color: Option<[f32; 4]>,
    /// The radius of the sheet's two corners on its open edge.
    pub corner_radius: f32,
    /// Shadow beyond the sheet's open edge: reach in px and alpha at t = 1.
    pub shadow: (f32, f32),
    pub open_duration: f32,
    pub close_duration: f32,
    pub open_curve: CubicBezier,
    pub close_curve: CubicBezier,
    pub main_updates: MainUpdates,
    /// Keep the layer textures this long (s) after the drawer closes; 0
    /// frees them on the frame it reaches Closed.
    pub keep_warm: f32,
}

impl Default for DrawerTheme {
    fn default() -> Self {
        Self {
            size: DrawerSize::Share {
                share: 0.85,
                max: 360.0,
            },
            size_vertical: DrawerSize::Share {
                share: 0.6,
                max: f32::INFINITY,
            },
            scrim: [0.0, 0.0, 0.0, 0.32],
            sheet_color: None,
            corner_radius: 16.0,
            shadow: (16.0, 0.25),
            open_duration: 0.300,
            close_duration: 0.250,
            open_curve: CubicBezier::EMPHASIZED_DECELERATE,
            close_curve: CubicBezier::EMPHASIZED_ACCELERATE,
            main_updates: MainUpdates::Live,
            keep_warm: 0.0,
        }
    }
}

/// One drawer's overrides of its theme: `None` takes the theme's value.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct DrawerOptions {
    /// The extent, whichever the edge.
    pub size: Option<DrawerSize>,
    pub scrim: Option<[f32; 4]>,
    pub sheet_color: Option<[f32; 4]>,
    pub corner_radius: Option<f32>,
    pub shadow: Option<(f32, f32)>,
    pub open_duration: Option<f32>,
    pub close_duration: Option<f32>,
    pub open_curve: Option<CubicBezier>,
    pub close_curve: Option<CubicBezier>,
    pub main_updates: Option<MainUpdates>,
    pub keep_warm: Option<f32>,
}

macro_rules! setter {
    ($name:ident: $t:ty) => {
        #[doc = concat!("Override the theme's `", stringify!($name), "`.")]
        pub fn $name(mut self, value: $t) -> Self {
            self.$name = Some(value);
            self
        }
    };
}

impl DrawerOptions {
    /// Overrides nothing.
    pub fn new() -> Self {
        Self::default()
    }

    setter!(size: DrawerSize);
    setter!(scrim: [f32; 4]);
    setter!(sheet_color: [f32; 4]);
    setter!(corner_radius: f32);
    setter!(shadow: (f32, f32));
    setter!(open_duration: f32);
    setter!(close_duration: f32);
    setter!(open_curve: CubicBezier);
    setter!(close_curve: CubicBezier);
    setter!(main_updates: MainUpdates);
    setter!(keep_warm: f32);

    /// Each `Some` wins; each `None` takes the theme's value.
    pub fn with_theme(&self, theme: &DrawerTheme) -> DrawerResolved {
        DrawerResolved {
            size: self.size.unwrap_or(theme.size),
            size_vertical: self.size.unwrap_or(theme.size_vertical),
            scrim: self.scrim.unwrap_or(theme.scrim),
            sheet_color: self.sheet_color.or(theme.sheet_color),
            corner_radius: self.corner_radius.unwrap_or(theme.corner_radius),
            shadow: self.shadow.unwrap_or(theme.shadow),
            open_duration: self.open_duration.unwrap_or(theme.open_duration),
            close_duration: self.close_duration.unwrap_or(theme.close_duration),
            open_curve: self.open_curve.unwrap_or(theme.open_curve),
            close_curve: self.close_curve.unwrap_or(theme.close_curve),
            main_updates: self.main_updates.unwrap_or(theme.main_updates),
            keep_warm: self.keep_warm.unwrap_or(theme.keep_warm),
        }
    }
}

/// A drawer's options resolved against its theme: what the drawer reads.
#[derive(Copy, Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct DrawerResolved {
    pub size: DrawerSize,
    pub size_vertical: DrawerSize,
    pub scrim: [f32; 4],
    /// `None`: the `ControlTheme`'s `bg_color`, read at draw.
    pub sheet_color: Option<[f32; 4]>,
    pub corner_radius: f32,
    pub shadow: (f32, f32),
    pub open_duration: f32,
    pub close_duration: f32,
    pub open_curve: CubicBezier,
    pub close_curve: CubicBezier,
    pub main_updates: MainUpdates,
    pub keep_warm: f32,
}

impl DrawerResolved {
    /// The size for a drawer on `edge`.
    pub fn size_for(&self, edge: DrawerEdge) -> DrawerSize {
        if edge.is_horizontal() {
            self.size
        } else {
            self.size_vertical
        }
    }

    fn motion(&self) -> state::Motion {
        state::Motion {
            open_duration: self.open_duration,
            close_duration: self.close_duration,
            open_curve: self.open_curve,
            close_curve: self.close_curve,
        }
    }
}

/// Where an event went.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum DrawerRoute {
    /// The main view's: from [`DrawerLayout::deliver`] its response; from
    /// [`Drawer::deliver`] `None` (give it to the main view).
    Main(Option<EventResponse>),
    /// The sheet's content took it; its response.
    Drawer(Option<EventResponse>),
    /// A point on the scrim while the drawer is visible. Nothing took it;
    /// the app may call `close()`.
    Scrim,
}

/// Which side a press belongs to, from its `Down` to the next `Down`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Side {
    Main,
    Content,
    Scrim,
}

/// A sheet on one edge of a viewport, with a content `Router` laid out in
/// the sheet's own coordinates. Normally held by a [`DrawerLayout`].
pub struct Drawer {
    state: Arc<DrawerState>,
    edge: DrawerEdge,
    options: DrawerOptions,
    theme: Cell<DrawerTheme>,
    viewport: Cell<Rect>,
    content: Router,
    content_mark: PaneMark,
    main_mark: PaneMark,
    initial_focus: Option<NodeId>,
    press: Cell<Option<Side>>,
    hover: Cell<Option<Side>>,
    seen_phase: Cell<DrawerPhase>,
}

impl std::fmt::Debug for Drawer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Drawer")
            .field("edge", &self.edge)
            .field("viewport", &self.viewport.get())
            .field("phase", &self.state.phase())
            .field("progress", &self.state.progress())
            .finish_non_exhaustive()
    }
}

impl Drawer {
    pub fn new(viewport: Rect, edge: DrawerEdge, options: DrawerOptions) -> Self {
        let state = DrawerState::new();
        let d = Self {
            seen_phase: Cell::new(state.phase()),
            state,
            edge,
            options,
            theme: Cell::new(DrawerTheme::default()),
            viewport: Cell::new(viewport),
            content: Router::new(),
            content_mark: PaneMark::new(),
            main_mark: PaneMark::new(),
            initial_focus: None,
            press: Cell::new(None),
            hover: Cell::new(None),
        };
        d.state.set_motion(d.resolved().motion());
        d
    }

    /// Share a state (it survives the drawer being rebuilt).
    pub fn with_state(mut self, state: Arc<DrawerState>) -> Self {
        state.set_motion(self.resolved().motion());
        self.seen_phase.set(state.phase());
        self.state = state;
        self
    }

    pub fn with_options(mut self, options: DrawerOptions) -> Self {
        self.options = options;
        self.state.set_motion(self.resolved().motion());
        self.content_mark.mark();
        self
    }

    /// The node of the content focused when the drawer reaches Open.
    pub fn with_initial_focus(mut self, id: NodeId) -> Self {
        self.initial_focus = Some(id);
        self
    }

    pub fn state(&self) -> &Arc<DrawerState> {
        &self.state
    }

    pub fn options(&self) -> &DrawerOptions {
        &self.options
    }

    pub fn edge(&self) -> DrawerEdge {
        self.edge
    }

    /// The options resolved against the theme last given (`set_theme`, or
    /// the context's at the last draw).
    pub fn resolved(&self) -> DrawerResolved {
        self.options.with_theme(&self.theme.get())
    }

    /// Resolve against `theme` from now on. A change that alters what the
    /// sheet draws bumps the content mark.
    pub fn set_theme(&self, theme: &ControlTheme) {
        if self.theme.get() != theme.drawer {
            self.theme.set(theme.drawer);
            self.state.set_motion(self.resolved().motion());
            self.content_mark.mark();
        }
    }

    pub fn set_viewport(&self, viewport: Rect) {
        if self.viewport.get() != viewport {
            self.viewport.set(viewport);
            self.content_mark.mark();
        }
    }

    pub fn viewport(&self) -> Rect {
        self.viewport.get()
    }

    /// The sheet's size across its edge, in router pixels.
    pub fn extent(&self) -> f32 {
        let vp = self.viewport.get();
        let across = if self.edge.is_horizontal() {
            vp.w
        } else {
            vp.h
        };
        self.resolved().size_for(self.edge).extent(across)
    }

    /// The sheet when open, in viewport pixels.
    pub fn sheet_open(&self) -> Rect {
        let vp = self.viewport.get();
        let e = self.extent();
        match self.edge {
            DrawerEdge::Left => Rect::new(vp.x, vp.y, e, vp.h),
            DrawerEdge::Right => Rect::new(vp.right() - e, vp.y, e, vp.h),
            DrawerEdge::Top => Rect::new(vp.x, vp.y, vp.w, e),
            DrawerEdge::Bottom => Rect::new(vp.x, vp.bottom() - e, vp.w, e),
        }
    }

    /// The sheet at progress `t`: `sheet_open` moved outward by
    /// `(1 - t) * extent`.
    pub fn sheet_at(&self, t: f32) -> Rect {
        let open = self.sheet_open();
        let away = (1.0 - t.clamp(0.0, 1.0)) * self.extent();
        let d = self.edge.dir();
        Rect::new(open.x - d[0] * away, open.y - d[1] * away, open.w, open.h)
    }

    /// The sheet at the current progress.
    pub fn sheet(&self) -> Rect {
        self.sheet_at(self.state.progress())
    }

    /// Visible: not Closed and `t > 0`.
    pub fn is_visible(&self) -> bool {
        self.state.phase() != DrawerPhase::Closed && self.state.progress() > 0.0
    }

    /// The content router, laid out from (0, 0) at the sheet's size.
    pub fn content(&self) -> &Router {
        &self.content
    }

    pub fn content_mut(&mut self) -> &mut Router {
        self.content_mark.mark();
        &mut self.content
    }

    /// Bumped by anything that changes what the sheet draws: input the
    /// content took, its animations, `set_viewport`, a theme change; the app
    /// bumps it for anything else.
    pub fn content_mark(&self) -> &PaneMark {
        &self.content_mark
    }

    /// Bumped by the app (or its `DrawerLayout`) when the main view's
    /// drawing changes.
    pub fn main_mark(&self) -> &PaneMark {
        &self.main_mark
    }

    // The calls, forwarded to the state.

    pub fn open(&self) {
        self.state.open();
    }

    pub fn close(&self) {
        self.state.close();
    }

    pub fn toggle(&self) {
        self.state.toggle();
    }

    pub fn set_open_now(&self, open: bool) {
        self.state.set_open_now(open);
    }

    pub fn set_progress(&self, t: f32) {
        self.state.set_progress(t);
    }

    pub fn settle(&self, open: bool, velocity: f32) {
        self.state.settle(open, velocity);
    }

    /// Notice phase changes made through the shared state: focus the initial
    /// node on reaching Open, drop the content's hover and grab on Closed.
    fn sync_phase(&self) {
        let phase = self.state.phase();
        let was = self.seen_phase.replace(phase);
        if phase == was {
            return;
        }
        match phase {
            DrawerPhase::Open => {
                if let Some(id) = self.initial_focus {
                    self.content.set_focus(Some(id));
                    self.content_mark.mark();
                }
            }
            DrawerPhase::Closed => {
                self.content.clear_hover();
                self.content.cancel_grab();
                self.hover.set(None);
                self.content_mark.mark();
            }
            _ => {}
        }
    }

    /// Whether keys and gestures go to the content now.
    fn keys_to_content(&self) -> bool {
        match self.state.phase() {
            DrawerPhase::Opening | DrawerPhase::Open => true,
            DrawerPhase::Dragging => self.state.progress() > 0.0,
            _ => false,
        }
    }

    /// Which side a point is on now.
    fn side_at(&self, pos: [f32; 2]) -> Side {
        if !self.is_visible() {
            return Side::Main;
        }
        if self.sheet().contains(pos) {
            Side::Content
        } else if self.viewport.get().contains(pos) {
            Side::Scrim
        } else {
            Side::Main
        }
    }

    /// Route one event to the sheet's content while it is visible (plan 01,
    /// 11:22 EDT, item 3). Pointer events inside the sheet go to the content,
    /// moved into sheet coordinates; points on the scrim are `Scrim`; a press
    /// stays with the side its `Down` landed on. Everything else is
    /// `Main(None)`: give it to the main view. Modifiers reach the content
    /// and are `Main(None)` too.
    pub fn deliver(&self, event: InputEvent) -> DrawerRoute {
        self.sync_phase();
        match event {
            InputEvent::Mouse(m) => self.deliver_mouse(m),
            InputEvent::Keyboard(k) => {
                if self.keys_to_content() {
                    let r = self.content.deliver_keyboard(k);
                    self.content_mark.mark();
                    DrawerRoute::Drawer(Some(r))
                } else {
                    DrawerRoute::Main(None)
                }
            }
            InputEvent::Gesture(g) => {
                if self.keys_to_content() {
                    let r = self.content.deliver_gesture(g);
                    self.content_mark.mark();
                    DrawerRoute::Drawer(Some(r))
                } else {
                    DrawerRoute::Main(None)
                }
            }
            InputEvent::Modifiers(m) => {
                self.content.deliver_modifiers(m);
                DrawerRoute::Main(None)
            }
        }
    }

    fn deliver_mouse(&self, m: MouseEvent) -> DrawerRoute {
        let side = match m {
            MouseEvent::Down { pos, .. } => {
                let s = self.side_at(pos);
                self.press.set(Some(s));
                s
            }
            MouseEvent::Drag { .. } | MouseEvent::Up { .. } | MouseEvent::Click { .. } => {
                self.press.get().unwrap_or_else(|| self.side_at(m.pos()))
            }
            _ => self.side_at(m.pos()),
        };
        if self.hover.replace(Some(side)) == Some(Side::Content) && side != Side::Content {
            self.content.clear_hover();
            self.content_mark.mark();
        }
        match side {
            Side::Main => DrawerRoute::Main(None),
            Side::Scrim => DrawerRoute::Scrim,
            Side::Content => {
                let o = self.sheet().pos();
                let r = self.content.deliver_mouse(shift_mouse(m, [-o[0], -o[1]]));
                self.content_mark.mark();
                DrawerRoute::Drawer(Some(r))
            }
        }
    }

    /// Move time on: the state's animation and the content router's own
    /// (flings, indicators). Returns whether anything still moves.
    pub fn advance(&self, dt_secs: f32) -> bool {
        let moving = self.state.advance(dt_secs);
        self.sync_phase();
        let content = self.content.advance(dt_secs);
        if content {
            self.content_mark.mark();
        }
        moving || content
    }

    /// The sheet's colour against `theme`.
    fn sheet_color(&self, theme: &ControlTheme) -> [f32; 4] {
        self.resolved().sheet_color.unwrap_or(theme.bg_color)
    }

    fn sheet_background(&self, at: [f32; 2], theme: &ControlTheme) -> SdfInstance {
        let size = self.sheet_open().size();
        SdfInstance {
            kind: SdfKind::RoundedBoxPerCorner {
                radii: self.edge.corner_radii(self.resolved().corner_radius),
            },
            position: at,
            size,
            color: self.sheet_color(theme),
            anim: 0,
        }
    }

    /// Draw the sheet (background and content) in sheet coordinates, for
    /// its texture.
    pub fn render_sheet(&self, list: &mut DrawList, ctx: &DrawContext) {
        self.set_theme(&ctx.theme);
        list.push(self.sheet_background([0.0, 0.0], &ctx.theme));
        self.content.render(list, ctx);
    }

    /// The fallback: draw the scrim, the sheet's shadowed background and its
    /// content over a main view already in `list`, with a clip and a
    /// translate at `sheet()`. Nothing while not visible.
    pub fn draw_direct(&self, list: &mut DrawList, ctx: &DrawContext) {
        self.set_theme(&ctx.theme);
        if !self.is_visible() {
            return;
        }
        let t = self.state.progress();
        let r = self.resolved();
        let vp = self.viewport.get();
        let mut scrim = r.scrim;
        scrim[3] *= t;
        list.push(SdfInstance {
            kind: SdfKind::Box,
            position: vp.pos(),
            size: vp.size(),
            color: scrim,
            anim: 0,
        });
        let sheet = self.sheet();
        let o = sheet.pos();
        list.push_translate(o[0], o[1]);
        list.push_fill(
            self.sheet_background([0.0, 0.0], &ctx.theme),
            Elevation::Raised,
        );
        list.push_translate_end();
        let clip = intersect(sheet, vp);
        list.push_clip(clip.pos(), clip.size());
        list.push_translate(o[0], o[1]);
        self.content.render_at(list, ctx, o);
        list.push_translate_end();
        list.push_clip_end();
    }

    /// The composite's uniform for this frame, in physical pixels.
    pub(crate) fn composite_params(&self, scale: f32, target: (u32, u32)) -> CompositeParams {
        let r = self.resolved();
        let s = scale;
        let open = self.sheet_open();
        let vp = self.viewport.get();
        CompositeParams {
            frame: [
                target.0 as f32,
                target.1 as f32,
                self.state.progress(),
                self.edge.index() as f32,
            ],
            viewport: [vp.x * s, vp.y * s, vp.w * s, vp.h * s],
            sheet_open: [open.x * s, open.y * s, open.w * s, open.h * s],
            scrim: r.scrim,
            shadow: [r.shadow.0 * s, r.shadow.1, 0.0, 0.0],
        }
    }
}

fn intersect(a: Rect, b: Rect) -> Rect {
    let x = a.x.max(b.x);
    let y = a.y.max(b.y);
    Rect::new(
        x,
        y,
        (a.right().min(b.right()) - x).max(0.0),
        (a.bottom().min(b.bottom()) - y).max(0.0),
    )
}

fn shift_mouse(m: MouseEvent, d: [f32; 2]) -> MouseEvent {
    let s = |p: [f32; 2]| [p[0] + d[0], p[1] + d[1]];
    match m {
        MouseEvent::Move { pos } => MouseEvent::Move { pos: s(pos) },
        MouseEvent::Down { pos, button } => MouseEvent::Down {
            pos: s(pos),
            button,
        },
        MouseEvent::Up { pos, button } => MouseEvent::Up {
            pos: s(pos),
            button,
        },
        MouseEvent::Click { pos, button } => MouseEvent::Click {
            pos: s(pos),
            button,
        },
        MouseEvent::Scroll { pos, delta } => MouseEvent::Scroll { pos: s(pos), delta },
        MouseEvent::ScrollPixels { pos, delta } => MouseEvent::ScrollPixels { pos: s(pos), delta },
        MouseEvent::Drag {
            start_pos,
            current_pos,
            delta,
            button,
        } => MouseEvent::Drag {
            start_pos: s(start_pos),
            current_pos: s(current_pos),
            delta,
            button,
        },
    }
}

#[cfg(test)]
mod tests;
