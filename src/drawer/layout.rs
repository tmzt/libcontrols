//! `DrawerLayout`: a main view and a drawer over it
//! (docs/design/20261028_04_DRAWER_LAYOUT_PLAN.md).

use std::cell::Cell;
use std::sync::Arc;

use libmsdf::drawlist::DrawList;

use super::{Drawer, DrawerEdge, DrawerOptions, DrawerRoute, DrawerState};
use crate::event::InputEvent;
use crate::rect::Rect;
use crate::router::{NodeId, Router};
use crate::theme::ControlTheme;
use crate::traits::DrawContext;

/// A main view and a drawer over it: two `Router` children, the main view
/// (the layout's coordinates, filling its rect) and the drawer's content
/// (sheet coordinates).
#[derive(Debug)]
pub struct DrawerLayout {
    rect: Cell<Rect>,
    main: Router,
    drawer: Drawer,
}

impl DrawerLayout {
    pub fn new(rect: Rect, edge: DrawerEdge, options: DrawerOptions) -> Self {
        Self {
            rect: Cell::new(rect),
            main: Router::new(),
            drawer: Drawer::new(rect, edge, options),
        }
    }

    /// Share a `DrawerState` (it survives the layout being rebuilt).
    pub fn with_state(mut self, state: Arc<DrawerState>) -> Self {
        self.drawer = self.drawer.with_state(state);
        self
    }

    /// The node of the content focused when the drawer reaches Open.
    pub fn with_initial_focus(mut self, id: NodeId) -> Self {
        self.drawer = self.drawer.with_initial_focus(id);
        self
    }

    pub fn rect(&self) -> Rect {
        self.rect.get()
    }

    /// Moves the main view's rect and the drawer's viewport together.
    pub fn set_rect(&self, rect: Rect) {
        if self.rect.replace(rect) != rect {
            self.drawer.set_viewport(rect);
            self.drawer.main_mark().mark();
        }
    }

    pub fn main(&self) -> &Router {
        &self.main
    }

    pub fn main_mut(&mut self) -> &mut Router {
        self.drawer.main_mark().mark();
        &mut self.main
    }

    pub fn drawer(&self) -> &Drawer {
        &self.drawer
    }

    pub fn drawer_mut(&mut self) -> &mut Drawer {
        &mut self.drawer
    }

    /// The drawer's content router (sheet coordinates).
    pub fn content(&self) -> &Router {
        self.drawer.content()
    }

    pub fn content_mut(&mut self) -> &mut Router {
        self.drawer.content_mut()
    }

    pub fn state(&self) -> &Arc<DrawerState> {
        self.drawer.state()
    }

    pub fn open(&self) {
        self.drawer.open();
    }

    pub fn close(&self) {
        self.drawer.close();
    }

    pub fn toggle(&self) {
        self.drawer.toggle();
    }

    pub fn set_open_now(&self, open: bool) {
        self.drawer.set_open_now(open);
    }

    pub fn set_progress(&self, t: f32) {
        self.drawer.set_progress(t);
    }

    pub fn settle(&self, open: bool, velocity: f32) {
        self.drawer.settle(open, velocity);
    }

    /// Route one event: the drawer's content while the sheet is visible and
    /// the event is its, `Scrim` for a point on the scrim, the main view
    /// otherwise (with its response).
    pub fn deliver(&self, event: InputEvent) -> DrawerRoute {
        match self.drawer.deliver(event) {
            DrawerRoute::Main(_) => {
                let r = self.main.deliver_event(event);
                self.drawer.main_mark().mark();
                DrawerRoute::Main(Some(r))
            }
            other => {
                if matches!(event, InputEvent::Mouse(_)) && self.main.hovered().is_some() {
                    self.main.clear_hover();
                    self.drawer.main_mark().mark();
                }
                other
            }
        }
    }

    /// The drawer's animation, the content's and the main view's own.
    /// Whether anything still moves.
    pub fn advance(&self, dt_secs: f32) -> bool {
        let drawer = self.drawer.advance(dt_secs);
        let main = self.main.advance(dt_secs);
        if main {
            self.drawer.main_mark().mark();
        }
        drawer || main
    }

    /// Resolve the drawer's options against `theme` now (it is also done at
    /// every draw with the context's theme).
    pub fn set_theme(&self, theme: &ControlTheme) {
        self.drawer.set_theme(theme);
    }

    /// The direct path: the main view, then the drawer's scrim, shadow and
    /// sheet over it when it is visible.
    pub fn render(&self, list: &mut DrawList, ctx: &DrawContext) {
        self.render_main(list, ctx);
        self.drawer.draw_direct(list, ctx);
    }

    /// The main view alone: to the surface while `LayerPlan::direct`, or
    /// into `DrawerLayers::main_view()`.
    pub fn render_main(&self, list: &mut DrawList, ctx: &DrawContext) {
        self.drawer.set_theme(&ctx.theme);
        self.main.render(list, ctx);
    }

    /// The sheet alone, in sheet coordinates, for its texture.
    pub fn render_sheet(&self, list: &mut DrawList, ctx: &DrawContext) {
        self.drawer.render_sheet(list, ctx);
    }

    /// Bump the main view's revision (the app changed what it draws).
    pub fn mark_main(&self) {
        self.drawer.main_mark().mark();
    }
}
