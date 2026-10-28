//! `DrawerLayers`: the drawer's GPU half (plan 01, sections 3.7 and 4, with
//! the 11:24 EDT amendment's public handles). The textures exist only while
//! the drawer is visible (or kept warm); while `t` is 0 the plan is
//! `direct` and the drawer records no pass and touches no texture.

use libmsdf::GpuSdfRenderer;
use libmsdf::drawlist::{AtlasSet, DrawList};

use super::composite::Composite;
use super::{Drawer, DrawerPhase, MainUpdates};

/// What the app draws this frame.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct LayerPlan {
    /// `t` is 0: draw the main view straight to the surface, exactly as
    /// without a drawer. No texture is touched, no composite runs.
    pub direct: bool,
    /// Draw the main view into `main_view()` this frame.
    pub draw_main: bool,
    /// Draw the sheet with `render_sheet` this frame.
    pub draw_sheet: bool,
}

impl LayerPlan {
    const DIRECT: Self = Self {
        direct: true,
        draw_main: false,
        draw_sheet: false,
    };
}

/// What `prepare` sees of the drawer, as plain values.
#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) struct Frame {
    pub phase: DrawerPhase,
    pub t: f32,
    pub main_rev: u64,
    pub content_rev: u64,
    pub main_updates: MainUpdates,
    pub keep_warm: f32,
    /// Physical pixels.
    pub main_size: (u32, u32),
    pub sheet_size: (u32, u32),
}

impl Frame {
    pub fn of(drawer: &Drawer, surface: (u32, u32), scale: f32) -> Self {
        let r = drawer.resolved();
        let open = drawer.sheet_open();
        let px = |v: f32| ((v * scale).ceil().max(1.0)) as u32;
        Self {
            phase: drawer.state().phase(),
            t: drawer.state().progress(),
            main_rev: drawer.main_mark().revision(),
            content_rev: drawer.content_mark().revision(),
            main_updates: r.main_updates,
            keep_warm: r.keep_warm,
            main_size: (surface.0.max(1), surface.1.max(1)),
            sheet_size: (px(open.w), px(open.h)),
        }
    }
}

/// The texture bookkeeping, with no device: which textures should exist at
/// which size, and which layers must be drawn. `DrawerLayers` makes the
/// GPU match it.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Book {
    pub main: Option<(u32, u32)>,
    pub sheet: Option<(u32, u32)>,
    pub main_drawn_at: Option<u64>,
    pub sheet_drawn_at: Option<u64>,
    pub closed_for: f32,
}

impl Book {
    fn free(&mut self) {
        *self = Self::default();
    }

    /// Size the textures for `f` (a resize recreates one, losing what it
    /// held).
    pub fn allocate(&mut self, f: &Frame) {
        if self.main != Some(f.main_size) {
            self.main = Some(f.main_size);
            self.main_drawn_at = None;
        }
        if self.sheet != Some(f.sheet_size) {
            self.sheet = Some(f.sheet_size);
            self.sheet_drawn_at = None;
        }
    }

    pub fn plan(&mut self, f: &Frame) -> LayerPlan {
        if f.t <= 0.0 {
            // The main view goes to the surface; what the main texture held
            // is stale from now on.
            self.main_drawn_at = None;
            if f.phase == DrawerPhase::Closed && f.keep_warm <= 0.0 {
                self.free();
            }
            return LayerPlan::DIRECT;
        }
        self.closed_for = 0.0;
        self.allocate(f);
        let draw_main = match (f.main_updates, self.main_drawn_at) {
            (MainUpdates::Frozen, Some(_)) => false,
            (_, at) => at != Some(f.main_rev),
        };
        let draw_sheet = self.sheet_drawn_at != Some(f.content_rev);
        if draw_main {
            self.main_drawn_at = Some(f.main_rev);
        }
        if draw_sheet {
            self.sheet_drawn_at = Some(f.content_rev);
        }
        LayerPlan {
            direct: false,
            draw_main,
            draw_sheet,
        }
    }

    /// Count time closed; free after `keep_warm`.
    pub fn advance(&mut self, dt: f32, phase: DrawerPhase, keep_warm: f32) {
        if phase != DrawerPhase::Closed || (self.main.is_none() && self.sheet.is_none()) {
            return;
        }
        self.closed_for += dt.max(0.0);
        if self.closed_for >= keep_warm {
            self.free();
        }
    }
}

struct Layer {
    // Held so the view stays valid.
    #[allow(dead_code)]
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    size: (u32, u32),
}

/// The drawer's two offscreen layers and its composite pass, on the app's
/// wgpu device.
pub struct DrawerLayers {
    format: wgpu::TextureFormat,
    composite: Composite,
    book: Book,
    main: Option<Layer>,
    sheet: Option<Layer>,
    bind_group: Option<wgpu::BindGroup>,
    scale: f32,
}

impl std::fmt::Debug for DrawerLayers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DrawerLayers")
            .field("format", &self.format)
            .field("main", &self.main.as_ref().map(|l| l.size))
            .field("sheet", &self.sheet.as_ref().map(|l| l.size))
            .finish_non_exhaustive()
    }
}

impl DrawerLayers {
    /// The pipeline for `format`, the surface's. No texture yet.
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        Self {
            format,
            composite: Composite::new(device, format),
            book: Book::default(),
            main: None,
            sheet: None,
            bind_group: None,
            scale: 1.0,
        }
    }

    /// Decide this frame. Creates or recreates textures as needed (or frees
    /// them once the drawer is Closed), and records the revisions it asks
    /// the app to draw at; the app then draws what the plan says before
    /// `composite`.
    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        drawer: &Drawer,
        surface: (u32, u32),
        scale: f32,
    ) -> LayerPlan {
        self.scale = scale;
        let plan = self.book.plan(&Frame::of(drawer, surface, scale));
        self.sync(device);
        plan
    }

    /// Create the textures ahead of the first visible frame (at an `open()`
    /// or the start of the app's gesture), so the allocation is not on a
    /// frame being shown. Nothing is drawn.
    pub fn prewarm(
        &mut self,
        device: &wgpu::Device,
        drawer: &Drawer,
        surface: (u32, u32),
        scale: f32,
    ) {
        self.scale = scale;
        self.book.closed_for = 0.0;
        self.book.allocate(&Frame::of(drawer, surface, scale));
        self.sync(device);
    }

    /// Count time closed; free the textures after `keep_warm`.
    pub fn advance(&mut self, dt_secs: f32, drawer: &Drawer) {
        self.book
            .advance(dt_secs, drawer.state().phase(), drawer.resolved().keep_warm);
        if self.book.main.is_none() {
            self.main = None;
            self.sheet = None;
            self.bind_group = None;
        }
    }

    /// The main layer, to draw the main view into on a frame whose plan
    /// says `draw_main`.
    pub fn main_view(&self) -> Option<&wgpu::TextureView> {
        self.main.as_ref().map(|l| &l.view)
    }

    /// The sheet texture's size in physical pixels, (0, 0) when none.
    pub fn sheet_size(&self) -> (u32, u32) {
        self.sheet.as_ref().map_or((0, 0), |l| l.size)
    }

    /// Whether any layer texture exists now.
    pub fn has_textures(&self) -> bool {
        self.main.is_some() || self.sheet.is_some()
    }

    /// Bytes the layer textures hold (4 per pixel).
    pub fn texture_bytes(&self) -> u64 {
        [&self.main, &self.sheet]
            .iter()
            .filter_map(|l| l.as_ref())
            .map(|l| u64::from(l.size.0) * u64::from(l.size.1) * 4)
            .sum()
    }

    /// Draw `list` (from `DrawerLayout::render_sheet`) into the sheet
    /// texture, cleared to transparent, with `renderer`: lowered against
    /// `set` when given, uploaded, rendered into its own encoder and
    /// submitted.
    pub fn render_sheet(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        renderer: &GpuSdfRenderer,
        list: &DrawList,
        set: Option<&AtlasSet>,
    ) {
        let Some(sheet) = &self.sheet else {
            return;
        };
        let frame = match set {
            Some(set) => list.lower_with(set),
            None => list.lower(),
        };
        renderer.upload_chars(queue, &frame.char_buffer);
        renderer.upload_params(queue, &frame.param_bank);
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("drawer_sheet_encoder"),
        });
        renderer.render_full_scaled_clear_into(
            &mut encoder,
            queue,
            &sheet.view,
            sheet.size.0,
            sheet.size.1,
            self.scale,
            &frame.draws,
            0.0,
            &[0; 16],
            &[],
            None,
        );
        queue.submit(std::iter::once(encoder.finish()));
    }

    /// The one composite pass into the frame's `target` (the surface's
    /// size). Nothing without textures (a `direct` frame).
    pub fn composite(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: &wgpu::TextureView,
        drawer: &Drawer,
        scale: f32,
    ) {
        let (Some(main), Some(bind_group)) = (&self.main, &self.bind_group) else {
            return;
        };
        let params = drawer.composite_params(scale, main.size);
        self.composite
            .run(device, queue, target, bind_group, &params);
    }

    /// Make the textures match the book.
    fn sync(&mut self, device: &wgpu::Device) {
        let mut changed = false;
        let format = self.format;
        let mut fit = |slot: &mut Option<Layer>, want: Option<(u32, u32)>, label: &str| {
            if slot.as_ref().map(|l| l.size) == want {
                return;
            }
            changed = true;
            *slot = want.map(|size| {
                let texture = device.create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d {
                        width: size.0,
                        height: size.1,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                });
                let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
                Layer {
                    texture,
                    view,
                    size,
                }
            });
        };
        fit(&mut self.main, self.book.main, "drawer_main_layer");
        fit(&mut self.sheet, self.book.sheet, "drawer_sheet_layer");
        if changed {
            self.bind_group = match (&self.main, &self.sheet) {
                (Some(m), Some(s)) => Some(self.composite.bind_group(device, &m.view, &s.view)),
                _ => None,
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(phase: DrawerPhase, t: f32) -> Frame {
        Frame {
            phase,
            t,
            main_rev: 0,
            content_rev: 0,
            main_updates: MainUpdates::Live,
            keep_warm: 0.0,
            main_size: (1080, 2340),
            sheet_size: (918, 2340),
        }
    }

    #[test]
    fn closed_is_direct_and_asks_for_no_texture() {
        let mut b = Book::default();
        let p = b.plan(&frame(DrawerPhase::Closed, 0.0));
        assert_eq!(p, LayerPlan::DIRECT);
        assert_eq!(b, Book::default());
    }

    #[test]
    fn t_zero_while_moving_is_direct_too() {
        let mut b = Book::default();
        assert!(b.plan(&frame(DrawerPhase::Opening, 0.0)).direct);
        assert!(b.plan(&frame(DrawerPhase::Dragging, 0.0)).direct);
        assert_eq!(b.main, None);
    }

    #[test]
    fn the_first_visible_frame_draws_both_then_neither() {
        let mut b = Book::default();
        let f = frame(DrawerPhase::Opening, 0.1);
        let p = b.plan(&f);
        assert!(!p.direct && p.draw_main && p.draw_sheet);
        assert_eq!(b.main, Some((1080, 2340)));
        assert_eq!(b.sheet, Some((918, 2340)));
        let p = b.plan(&frame(DrawerPhase::Opening, 0.5));
        assert!(!p.direct && !p.draw_main && !p.draw_sheet);
    }

    #[test]
    fn a_mark_redraws_its_layer_only() {
        let mut b = Book::default();
        let mut f = frame(DrawerPhase::Open, 1.0);
        b.plan(&f);
        f.content_rev = 1;
        let p = b.plan(&f);
        assert!(!p.draw_main && p.draw_sheet);
        f.main_rev = 1;
        let p = b.plan(&f);
        assert!(p.draw_main && !p.draw_sheet);
    }

    #[test]
    fn a_resize_draws_both() {
        let mut b = Book::default();
        let mut f = frame(DrawerPhase::Open, 1.0);
        b.plan(&f);
        f.main_size = (2340, 1080);
        f.sheet_size = (945, 1080);
        let p = b.plan(&f);
        assert!(p.draw_main && p.draw_sheet);
        assert_eq!(b.main, Some((2340, 1080)));
    }

    #[test]
    fn frozen_main_is_drawn_once_per_showing() {
        let mut b = Book::default();
        let mut f = frame(DrawerPhase::Opening, 0.2);
        f.main_updates = MainUpdates::Frozen;
        assert!(b.plan(&f).draw_main);
        f.main_rev = 5;
        assert!(!b.plan(&f).draw_main);
        f.phase = DrawerPhase::Closed;
        f.t = 0.0;
        b.plan(&f);
        f.phase = DrawerPhase::Opening;
        f.t = 0.1;
        assert!(b.plan(&f).draw_main);
    }

    #[test]
    fn reaching_closed_frees_or_keeps_warm() {
        let mut b = Book::default();
        b.plan(&frame(DrawerPhase::Closing, 0.3));
        assert!(b.plan(&frame(DrawerPhase::Closed, 0.0)).direct);
        assert_eq!(b.main, None, "freed on the frame t reaches 0");

        let mut b = Book::default();
        let mut f = frame(DrawerPhase::Closing, 0.3);
        f.keep_warm = 2.0;
        b.plan(&f);
        f.phase = DrawerPhase::Closed;
        f.t = 0.0;
        assert!(b.plan(&f).direct);
        assert!(b.main.is_some(), "kept warm");
        assert_eq!(b.main_drawn_at, None, "but its contents are stale");
        b.advance(1.5, DrawerPhase::Closed, 2.0);
        assert!(b.main.is_some());
        b.advance(0.6, DrawerPhase::Closed, 2.0);
        assert_eq!(b.main, None);
        // The sheet's contents survive a warm reopen.
        let mut b = Book::default();
        b.plan(&f);
        f.phase = DrawerPhase::Opening;
        f.t = 0.3;
        b.plan(&f);
        f.phase = DrawerPhase::Closed;
        f.t = 0.0;
        b.plan(&f);
        f.phase = DrawerPhase::Opening;
        f.t = 0.1;
        let p = b.plan(&f);
        assert!(p.draw_main && !p.draw_sheet);
    }
}
