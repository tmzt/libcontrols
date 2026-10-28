// The drawer's composite (docs/design/20261028_01_DRAWER_PLAN.md, section
// 3.9, scrim only by its 11:21 EDT amendment): one full-screen triangle that
// lays the sheet texture over the main texture at the sheet's place for t,
// with the scrim and the shadow worked out from t. Mirrors
// `CompositeParams` in composite.rs field for field.

struct Uniforms {
    // [target_w, target_h, t, edge (0 left, 1 right, 2 top, 3 bottom)]
    frame: vec4<f32>,
    // The main view's rectangle, physical px: [x, y, w, h].
    viewport: vec4<f32>,
    // The sheet at t = 1, physical px: [x, y, w, h].
    sheet_open: vec4<f32>,
    // Scrim colour, alpha at t = 1 (straight).
    scrim: vec4<f32>,
    // [shadow reach px, shadow alpha at t = 1, 0, 0]
    shadow: vec4<f32>,
};

@group(0) @binding(0) var<uniform> u: Uniforms;
@group(0) @binding(1) var main_tex: texture_2d<f32>;
@group(0) @binding(2) var sheet_tex: texture_2d<f32>;
@group(0) @binding(3) var samp: sampler;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> VsOut {
    let x = f32((i << 1u) & 2u) * 2.0 - 1.0;
    let y = f32(i & 2u) * 2.0 - 1.0;
    var out: VsOut;
    out.pos = vec4<f32>(x, y, 0.0, 1.0);
    return out;
}

fn edge_dir(edge: u32) -> vec2<f32> {
    switch edge {
        case 1u: { return vec2<f32>(-1.0, 0.0); }
        case 2u: { return vec2<f32>(0.0, 1.0); }
        case 3u: { return vec2<f32>(0.0, -1.0); }
        default: { return vec2<f32>(1.0, 0.0); }
    }
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = in.pos.xy;
    let t = u.frame.z;
    let edge = u32(u.frame.w + 0.5);

    // 1. Main: the main texture where it is, unmoved.
    let main_dims = vec2<f32>(textureDimensions(main_tex));
    var c = textureSampleLevel(main_tex, samp, p / main_dims, 0.0).rgb;

    let vp0 = u.viewport.xy;
    let vp1 = u.viewport.xy + u.viewport.zw;
    let in_vp = all(p >= vp0) && all(p < vp1);

    // 2. Scrim over the main view.
    if (in_vp) {
        c = mix(c, u.scrim.rgb, u.scrim.a * t);
    }

    // The sheet's place at t.
    let dir = edge_dir(edge);
    let size = u.sheet_open.zw;
    var extent = size.x;
    if (edge >= 2u) {
        extent = size.y;
    }
    let o = u.sheet_open.xy - dir * extent * (1.0 - t);
    let rel = p - o;

    // 3. Shadow beyond the sheet's open edge, inside the viewport.
    var e = -1.0;
    var along = false;
    switch edge {
        case 1u: { e = o.x - p.x; along = rel.y >= 0.0 && rel.y < size.y; }
        case 2u: { e = p.y - (o.y + size.y); along = rel.x >= 0.0 && rel.x < size.x; }
        case 3u: { e = o.y - p.y; along = rel.x >= 0.0 && rel.x < size.x; }
        default: { e = p.x - (o.x + size.x); along = rel.y >= 0.0 && rel.y < size.y; }
    }
    let reach = u.shadow.x;
    if (in_vp && along && reach > 0.0 && e >= 0.0 && e < reach) {
        let k = 1.0 - e / reach;
        c = c * (1.0 - u.shadow.y * t * k * k);
    }

    // 4. Sheet over, by its alpha (its rounded corners are in it). libmsdf's
    // renderer writes straight, not premultiplied, alpha (blend_over in
    // sdf_render.wgsl divides by the result's alpha).
    if (in_vp && all(rel >= vec2<f32>(0.0)) && all(rel < size)) {
        let sheet_dims = vec2<f32>(textureDimensions(sheet_tex));
        let s = textureSampleLevel(sheet_tex, samp, rel / sheet_dims, 0.0);
        c = mix(c, s.rgb, s.a);
    }

    return vec4<f32>(c, 1.0);
}
