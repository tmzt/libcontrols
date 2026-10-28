//! The composite pipeline and its uniform (plan 01, section 3.9, with the
//! 11:21 EDT amendment's scrim only and the 11:24 EDT amendment's private
//! handles).

pub(crate) const SHADER_SOURCE: &str = include_str!("composite.wgsl");

/// The composite's uniform; mirrors `Uniforms` in composite.wgsl field for
/// field.
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct CompositeParams {
    /// [target_w, target_h, t, edge (0 left, 1 right, 2 top, 3 bottom)]
    pub frame: [f32; 4],
    /// The main view's rectangle, physical px: [x, y, w, h].
    pub viewport: [f32; 4],
    /// The sheet at t = 1, physical px: [x, y, w, h].
    pub sheet_open: [f32; 4],
    /// Scrim colour, alpha at t = 1.
    pub scrim: [f32; 4],
    /// [shadow reach px, shadow alpha at t = 1, 0, 0]
    pub shadow: [f32; 4],
}

/// The pipeline, sampler, uniform buffer and bind group layout.
pub(crate) struct Composite {
    pipeline: wgpu::RenderPipeline,
    pub layout: wgpu::BindGroupLayout,
    pub sampler: wgpu::Sampler,
    pub uniforms: wgpu::Buffer,
}

impl Composite {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("drawer_composite_shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER_SOURCE.into()),
        });
        let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("drawer_composite_uniforms"),
            size: std::mem::size_of::<CompositeParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("drawer_composite_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let tex = |binding: u32| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("drawer_composite_layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                tex(1),
                tex(2),
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("drawer_composite_pipeline_layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("drawer_composite_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        Self {
            pipeline,
            layout,
            sampler,
            uniforms,
        }
    }

    /// The bind group for a pair of layer views (made when a texture is
    /// (re)created, not per frame).
    pub fn bind_group(
        &self,
        device: &wgpu::Device,
        main: &wgpu::TextureView,
        sheet: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("drawer_composite_bind_group"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.uniforms.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(main),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(sheet),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        })
    }

    /// One `write_buffer` of the uniform and one pass of three vertices into
    /// `target`, in its own encoder, submitted.
    pub fn run(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: &wgpu::TextureView,
        bind_group: &wgpu::BindGroup,
        params: &CompositeParams,
    ) {
        queue.write_buffer(&self.uniforms, 0, bytemuck::bytes_of(params));
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("drawer_composite_encoder"),
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("drawer_composite_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
        queue.submit(std::iter::once(encoder.finish()));
    }
}

/// What the composite shader computes for pixel centre `p`, on the CPU, for
/// tests of the uniform's math. `main` and `sheet` sample the layers
/// (`sheet` with straight alpha, at a point relative to the sheet's corner).
#[cfg(test)]
pub(crate) fn shade(
    u: &CompositeParams,
    p: [f32; 2],
    main: impl Fn([f32; 2]) -> [f32; 3],
    sheet: impl Fn([f32; 2]) -> [f32; 4],
) -> [f32; 3] {
    let t = u.frame[2];
    let edge = (u.frame[3] + 0.5) as u32;
    let mut c = main(p);
    let vp = u.viewport;
    let in_vp = p[0] >= vp[0] && p[1] >= vp[1] && p[0] < vp[0] + vp[2] && p[1] < vp[1] + vp[3];
    if in_vp {
        for (ch, s) in c.iter_mut().zip(u.scrim) {
            *ch += (s - *ch) * u.scrim[3] * t;
        }
    }
    let dir = match edge {
        1 => [-1.0, 0.0],
        2 => [0.0, 1.0],
        3 => [0.0, -1.0],
        _ => [1.0, 0.0],
    };
    let size = [u.sheet_open[2], u.sheet_open[3]];
    let extent = if edge >= 2 { size[1] } else { size[0] };
    let o = [
        u.sheet_open[0] - dir[0] * extent * (1.0 - t),
        u.sheet_open[1] - dir[1] * extent * (1.0 - t),
    ];
    let rel = [p[0] - o[0], p[1] - o[1]];
    let in_x = rel[0] >= 0.0 && rel[0] < size[0];
    let in_y = rel[1] >= 0.0 && rel[1] < size[1];
    let (e, along) = match edge {
        1 => (o[0] - p[0], in_y),
        2 => (p[1] - (o[1] + size[1]), in_x),
        3 => (o[1] - p[1], in_x),
        _ => (p[0] - (o[0] + size[0]), in_y),
    };
    let reach = u.shadow[0];
    if in_vp && along && reach > 0.0 && (0.0..reach).contains(&e) {
        let k = 1.0 - e / reach;
        for ch in &mut c {
            *ch *= 1.0 - u.shadow[1] * t * k * k;
        }
    }
    if in_vp && in_x && in_y {
        let s = sheet(rel);
        for (i, ch) in c.iter_mut().enumerate() {
            *ch += (s[i] - *ch) * s[3];
        }
    }
    c
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_uniform_is_80_bytes_and_the_shader_validates() {
        assert_eq!(std::mem::size_of::<CompositeParams>(), 80);
        let module = naga::front::wgsl::parse_str(SHADER_SOURCE).expect("composite.wgsl parses");
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        )
        .validate(&module)
        .expect("composite.wgsl validates");
    }
}
