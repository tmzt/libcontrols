//! The drawer's GPU path on a headless wgpu device
//! (docs/design/20261028_01_DRAWER_PLAN.md step 8, driven from the
//! DrawerLayout of docs/design/20261028_04_DRAWER_LAYOUT_PLAN.md).
//!
//! Skips when no adapter is found. With `LIBCONTROLS_EVIDENCE=<dir>` set it
//! also writes each composite as a PNG into that directory.

use libcontrols::{
    ControlAction, ControlPart, DrawContext, DrawControl, DrawerEdge, DrawerLayers, DrawerLayout,
    DrawerOptions, DrawerSize, EventContext, HostedControl, KeyboardEvent, LayerPlan, MouseEvent,
    Rect,
};
use libmsdf::GpuSdfRenderer;
use libmsdf::drawlist::{DrawList, SdfInstance, SdfKind};

const FMT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const SCALE: f32 = 2.0;
/// Logical viewport; the surface is twice this.
const VP: Rect = Rect::new(0.0, 0.0, 300.0, 200.0);
const W: u32 = 600;
const H: u32 = 400;
const EXTENT: f32 = 120.0;

/// A filled box: something to see without a font atlas.
struct Fill(Rect, [f32; 4]);

impl DrawControl for Fill {
    fn draw(&self, list: &mut DrawList, _ctx: &DrawContext) {
        list.push(SdfInstance {
            kind: SdfKind::Box,
            position: self.0.pos(),
            size: self.0.size(),
            color: self.1,
            anim: 0,
        });
    }
}

impl HostedControl for Fill {
    fn bounds(&self) -> Rect {
        self.0
    }
    fn hit_test(&self, pos: [f32; 2]) -> Option<(ControlPart, f32)> {
        self.0.contains(pos).then_some((ControlPart::Body, 0.0))
    }
    fn handle_mouse(&self, _: &MouseEvent, _: &EventContext) -> Option<ControlAction> {
        None
    }
    fn handle_keyboard(&self, _: &KeyboardEvent, _: &EventContext) -> Option<ControlAction> {
        None
    }
}

struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    renderer: GpuSdfRenderer,
}

fn gpu() -> Option<Gpu> {
    let instance = wgpu::Instance::default();
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        compatible_surface: None,
        ..Default::default()
    }))
    .ok()?;
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).ok()?;
    let renderer = GpuSdfRenderer::new(&device, FMT);
    Some(Gpu {
        device,
        queue,
        renderer,
    })
}

fn target(g: &Gpu, w: u32, h: u32) -> wgpu::Texture {
    g.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("test-target"),
        size: wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: FMT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    })
}

struct Image {
    w: u32,
    h: u32,
    px: Vec<u8>,
}

impl Image {
    fn at(&self, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * self.w + x) * 4) as usize;
        [self.px[i], self.px[i + 1], self.px[i + 2], self.px[i + 3]]
    }

    fn save(&self, name: &str) {
        let Ok(dir) = std::env::var("LIBCONTROLS_EVIDENCE") else {
            return;
        };
        std::fs::create_dir_all(&dir).unwrap();
        let file = std::fs::File::create(format!("{dir}/{name}.png")).unwrap();
        let mut enc = png::Encoder::new(std::io::BufWriter::new(file), self.w, self.h);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header()
            .unwrap()
            .write_image_data(&self.px)
            .unwrap();
    }
}

fn readback(g: &Gpu, tex: &wgpu::Texture) -> Image {
    let (w, h) = (tex.width(), tex.height());
    let padded = (w * 4).next_multiple_of(256);
    let buf = g.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: u64::from(padded * h),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut enc = g.device.create_command_encoder(&Default::default());
    enc.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: tex,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buf,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded),
                rows_per_image: None,
            },
        },
        wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
    );
    g.queue.submit(std::iter::once(enc.finish()));
    let slice = buf.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |r| {
        let _ = tx.send(r);
    });
    g.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    rx.recv().unwrap().unwrap();
    let data = slice.get_mapped_range().unwrap();
    let mut px = Vec::with_capacity((w * h * 4) as usize);
    for row in 0..h {
        let start = (row * padded) as usize;
        px.extend_from_slice(&data[start..start + (w * 4) as usize]);
    }
    drop(data);
    buf.unmap();
    Image { w, h, px }
}

const SHEET: [f32; 4] = [0.85, 0.30, 0.10, 1.0];
const CONTENT: [f32; 4] = [0.10, 0.80, 0.30, 1.0];

/// A main view of coloured bands and a sheet with two boxes.
fn layout(edge: DrawerEdge, options: DrawerOptions) -> DrawerLayout {
    let mut l = DrawerLayout::new(
        VP,
        edge,
        options
            .size(DrawerSize::Px(EXTENT))
            .sheet_color(SHEET)
            .corner_radius(12.0),
    );
    let bands = [
        [0.20, 0.35, 0.80, 1.0],
        [0.95, 0.95, 0.95, 1.0],
        [0.60, 0.20, 0.70, 1.0],
    ];
    l.main_mut().add(None, Fill(VP, [0.12, 0.12, 0.14, 1.0]));
    for (i, c) in bands.into_iter().enumerate() {
        let y = 20.0 + i as f32 * 60.0;
        l.main_mut()
            .add(None, Fill(Rect::new(20.0, y, 260.0, 40.0), c));
    }
    let size = l.drawer().sheet_open().size();
    let (w, h) = (size[0], size[1]);
    l.content_mut()
        .add(None, Fill(Rect::new(10.0, 10.0, w - 20.0, 30.0), CONTENT));
    l.content_mut().add(
        None,
        Fill(Rect::new(10.0, h - 40.0, w - 20.0, 30.0), CONTENT),
    );
    l
}

/// One frame as an app draws it.
fn frame(
    g: &Gpu,
    l: &DrawerLayout,
    layers: &mut DrawerLayers,
    out: &wgpu::TextureView,
) -> LayerPlan {
    let ctx = DrawContext::default();
    let plan = layers.prepare(&g.device, l.drawer(), (W, H), SCALE);
    let main_list = || {
        let mut list = DrawList::new();
        l.render_main(&mut list, &ctx);
        list
    };
    if plan.direct {
        g.renderer
            .render_draw_list(&g.device, &g.queue, out, W, H, SCALE, &main_list(), 0.0);
        return plan;
    }
    if plan.draw_main {
        let view = layers.main_view().expect("a main layer while visible");
        g.renderer
            .render_draw_list(&g.device, &g.queue, view, W, H, SCALE, &main_list(), 0.0);
    }
    if plan.draw_sheet {
        let mut sheet = DrawList::new();
        l.render_sheet(&mut sheet, &ctx);
        layers.render_sheet(&g.device, &g.queue, &g.renderer, &sheet, None);
    }
    layers.composite(&g.device, &g.queue, out, l.drawer(), SCALE);
    plan
}

fn shot(g: &Gpu, l: &DrawerLayout, layers: &mut DrawerLayers) -> (LayerPlan, Image) {
    let out = target(g, W, H);
    let plan = frame(g, l, layers, &out.create_view(&Default::default()));
    (plan, readback(g, &out))
}

/// The main view drawn straight to a target, with no drawer involved.
fn plain_main(g: &Gpu, l: &DrawerLayout) -> Image {
    let out = target(g, W, H);
    let mut list = DrawList::new();
    l.render_main(&mut list, &DrawContext::default());
    g.renderer.render_draw_list(
        &g.device,
        &g.queue,
        &out.create_view(&Default::default()),
        W,
        H,
        SCALE,
        &list,
        0.0,
    );
    readback(g, &out)
}

/// The sheet drawn alone into a sheet-sized transparent target.
fn plain_sheet(g: &Gpu, l: &DrawerLayout, w: u32, h: u32) -> Image {
    let out = target(g, w, h);
    let mut list = DrawList::new();
    l.render_sheet(&mut list, &DrawContext::default());
    let frame = list.lower();
    g.renderer.upload_chars(&g.queue, &frame.char_buffer);
    g.renderer.upload_params(&g.queue, &frame.param_bank);
    let mut enc = g.device.create_command_encoder(&Default::default());
    g.renderer.render_full_scaled_clear_into(
        &mut enc,
        &g.queue,
        &out.create_view(&Default::default()),
        w,
        h,
        SCALE,
        &frame.draws,
        0.0,
        &[0; 16],
        &[],
        None,
    );
    g.queue.submit(std::iter::once(enc.finish()));
    readback(g, &out)
}

fn set_t(l: &DrawerLayout, t: f32) {
    if t <= 0.0 {
        l.set_open_now(false);
    } else if t >= 1.0 {
        l.set_open_now(true);
    } else {
        l.set_progress(t);
    }
}

fn close(a: [u8; 4], b: [u8; 4], tol: i32) -> bool {
    a.iter()
        .zip(b)
        .take(3)
        .all(|(x, y)| (i32::from(*x) - i32::from(y)).abs() <= tol)
}

#[test]
fn closed_drawer_adds_no_texture_and_no_pass() {
    let Some(g) = gpu() else {
        eprintln!("drawer_gpu: SKIP — no GPU adapter");
        return;
    };
    let l = layout(DrawerEdge::Left, DrawerOptions::new());
    let mut layers = DrawerLayers::new(&g.device, FMT);
    let (plan, img) = shot(&g, &l, &mut layers);
    assert!(plan.direct && !plan.draw_main && !plan.draw_sheet);
    assert!(!layers.has_textures());
    assert_eq!(layers.texture_bytes(), 0);
    assert!(layers.main_view().is_none());
    let plain = plain_main(&g, &l);
    assert_eq!(
        img.px, plain.px,
        "t = 0: the frame is the main view drawn directly"
    );
}

#[test]
fn textures_live_only_while_visible() {
    let Some(g) = gpu() else {
        eprintln!("drawer_gpu: SKIP — no GPU adapter");
        return;
    };
    let l = layout(DrawerEdge::Left, DrawerOptions::new());
    let mut layers = DrawerLayers::new(&g.device, FMT);
    l.open();
    // t is still 0 before the first advance: still direct.
    assert!(shot(&g, &l, &mut layers).0.direct);
    assert!(!layers.has_textures());
    l.advance(0.016);
    let (plan, _) = shot(&g, &l, &mut layers);
    assert!(!plan.direct && plan.draw_main && plan.draw_sheet);
    let sheet_px = (EXTENT * SCALE) as u64 * u64::from(H) * 4;
    assert_eq!(layers.texture_bytes(), u64::from(W * H * 4) + sheet_px);
    assert_eq!(layers.sheet_size(), (240, H));
    l.advance(0.016);
    let (plan, _) = shot(&g, &l, &mut layers);
    assert!(
        !plan.direct && !plan.draw_main && !plan.draw_sheet,
        "moving: composite only"
    );
    while l.advance(0.016) {
        shot(&g, &l, &mut layers);
    }
    l.close();
    while l.advance(0.016) {
        assert!(!shot(&g, &l, &mut layers).0.direct);
    }
    let (plan, img) = shot(&g, &l, &mut layers);
    assert!(plan.direct);
    assert!(!layers.has_textures(), "freed on the frame t reaches 0");
    assert_eq!(img.px, plain_main(&g, &l).px);
}

#[test]
fn composite_places_the_sheet_and_the_scrim() {
    let Some(g) = gpu() else {
        eprintln!("drawer_gpu: SKIP — no GPU adapter");
        return;
    };
    let mut layers = DrawerLayers::new(&g.device, FMT);

    // t = 1 with no scrim and no shadow: the sheet's rectangle is the sheet
    // drawn alone, texel for texel, where it is opaque.
    let bare = layout(
        DrawerEdge::Left,
        DrawerOptions::new()
            .scrim([0.0, 0.0, 0.0, 0.0])
            .shadow((0.0, 0.0)),
    );
    set_t(&bare, 1.0);
    let (_, img) = shot(&g, &bare, &mut layers);
    let (sw, sh) = layers.sheet_size();
    let sheet = plain_sheet(&g, &bare, sw, sh);
    let mut compared = 0;
    for y in 0..sh {
        for x in 0..sw {
            let s = sheet.at(x, y);
            if s[3] == 255 {
                assert_eq!(img.at(x, y)[..3], s[..3], "texel ({x}, {y})");
                compared += 1;
            }
        }
    }
    assert!(compared > (sw * sh) as usize * 9 / 10);
    let main = plain_main(&g, &bare);
    assert_eq!(
        img.at(W - 10, 50),
        main.at(W - 10, 50),
        "no scrim: main unchanged"
    );

    // t = 0.5 with the default scrim: the sheet's leading column is at
    // extent / 2, and the main view is darkened by 0.32 * 0.5.
    let l = layout(DrawerEdge::Left, DrawerOptions::new().shadow((0.0, 0.0)));
    set_t(&l, 0.5);
    let (_, img) = shot(&g, &l, &mut layers);
    let edge = (EXTENT * 0.5 * SCALE) as u32;
    // The sheet's own colour as the renderer draws it (away from its boxes).
    let sheet_rgb = sheet.at(sw / 2, sh / 2);
    assert_eq!(sheet_rgb[3], 255);
    let y = H / 2;
    // The sheet's last column is its anti-aliased edge; the one before is
    // inside it and the one after is the main view.
    assert!(
        close(img.at(edge - 2, y), sheet_rgb, 2),
        "{:?}",
        img.at(edge - 2, y)
    );
    assert!(!close(img.at(edge + 1, y), sheet_rgb, 40));
    let main = plain_main(&g, &l);
    for (x, y) in [(W - 10, 50), (W - 10, H - 10), (edge + 30, 70)] {
        let m = main.at(x, y);
        let want = m.map(|c| (f32::from(c) * (1.0 - 0.32 * 0.5)).round() as u8);
        assert!(
            close(img.at(x, y), want, 2),
            "({x}, {y}): {:?} vs {want:?}",
            img.at(x, y)
        );
    }

    // Bottom at t = 0.5: the sheet's top row is at H - extent / 2.
    let b = layout(DrawerEdge::Bottom, DrawerOptions::new().shadow((0.0, 0.0)));
    set_t(&b, 0.5);
    let (_, img) = shot(&g, &b, &mut layers);
    let top = H - (EXTENT * 0.5 * SCALE) as u32;
    assert!(close(img.at(W / 2, top + 1), sheet_rgb, 2));
    assert!(!close(img.at(W / 2, top - 1), sheet_rgb, 40));
}

#[test]
fn composites_at_several_t_for_left_and_bottom() {
    let Some(g) = gpu() else {
        eprintln!("drawer_gpu: SKIP — no GPU adapter");
        return;
    };
    for (name, edge) in [("left", DrawerEdge::Left), ("bottom", DrawerEdge::Bottom)] {
        let l = layout(edge, DrawerOptions::new());
        let mut layers = DrawerLayers::new(&g.device, FMT);
        let mut last: Option<Vec<u8>> = None;
        for t in [0.0, 0.25, 0.5, 1.0] {
            set_t(&l, t);
            let (plan, img) = shot(&g, &l, &mut layers);
            assert_eq!(plan.direct, t == 0.0);
            assert_eq!(layers.has_textures(), t > 0.0);
            if let Some(prev) = &last {
                assert_ne!(prev, &img.px, "{name} t = {t} differs from the previous t");
            }
            img.save(&format!("{name}_t{:03}", (t * 100.0) as u32));
            last = Some(img.px);
        }
    }
}
