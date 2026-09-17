use super::*;

// HDR intermediate format for the scene/bloom attachments. `Rgba16Float`
// is color-renderable, blendable, and filterable in core WebGPU, and keeps
// metal highlights and emissive values above 1.0 intact until tone mapping.
pub(crate) const POST_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

/// Scene-linear bloom threshold: only HDR highlights (above display white)
/// contribute to the bright pass.
pub(crate) const BLOOM_THRESHOLD: f32 = 1.0;

pub(crate) struct PostProcessState {
    pub(crate) width: u32,
    pub(crate) height: u32,
    #[cfg(all(feature = "test-support", not(target_arch = "wasm32")))]
    pub(crate) scene_texture: wgpu::Texture,
    pub(crate) scene_view: wgpu::TextureView,
    /// MSAA > 1 时的多重采样场景目标：场景 pass 渲到它并 resolve 进
    /// `scene_view`（bloom/compose 仍读单采样 `scene_view`）。
    pub(crate) msaa_scene_view: Option<wgpu::TextureView>,
    pub(crate) blur_a_view: wgpu::TextureView,
    pub(crate) blur_b_view: wgpu::TextureView,
    pub(crate) blur_scene_bind_group: wgpu::BindGroup,
    pub(crate) blur_a_bind_group: wgpu::BindGroup,
    pub(crate) compose_bind_group: wgpu::BindGroup,
}

impl PostProcessState {
    pub(crate) fn new(
        device: &wgpu::Device,
        blur_layout: &wgpu::BindGroupLayout,
        compose_layout: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
        compose_uniform_buffer: &wgpu::Buffer,
        width: u32,
        height: u32,
        msaa_samples: u32,
    ) -> Self {
        let scene_texture = create_post_texture(
            device,
            "weapon scene texture",
            width,
            height,
            wgpu::TextureUsages::COPY_SRC,
        );
        let scene_view = scene_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let msaa_scene_view = (msaa_samples > 1).then(|| {
            device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some("weapon msaa scene texture"),
                    size: wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: msaa_samples,
                    dimension: wgpu::TextureDimension::D2,
                    format: POST_FORMAT,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                    view_formats: &[],
                })
                .create_view(&wgpu::TextureViewDescriptor::default())
        });
        let blur_a_view = create_post_texture_view(device, "weapon-bloom-pass a", width, height);
        let blur_b_view = create_post_texture_view(device, "weapon-bloom-pass b", width, height);

        let blur_horizontal_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("weapon-bloom-pass horizontal uniform"),
            contents: bytemuck::bytes_of(&PostUniform {
                params: [1.0 / width as f32, 0.0, BLOOM_THRESHOLD, 1.0],
            }),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let blur_vertical_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("weapon-bloom-pass vertical uniform"),
            contents: bytemuck::bytes_of(&PostUniform {
                params: [0.0, 1.0 / height as f32, 0.0, 0.0],
            }),
            usage: wgpu::BufferUsages::UNIFORM,
        });

        let blur_scene_bind_group = create_post_bind_group(
            device,
            blur_layout,
            &scene_view,
            sampler,
            &blur_horizontal_buffer,
            &scene_view,
            "weapon-bloom-pass scene bind group",
        );
        let blur_a_bind_group = create_post_bind_group(
            device,
            blur_layout,
            &blur_a_view,
            sampler,
            &blur_vertical_buffer,
            &blur_a_view,
            "weapon-bloom-pass a bind group",
        );
        let compose_bind_group = create_post_bind_group(
            device,
            compose_layout,
            &scene_view,
            sampler,
            compose_uniform_buffer,
            &blur_b_view,
            "weapon compose bind group",
        );

        Self {
            width,
            height,
            #[cfg(all(feature = "test-support", not(target_arch = "wasm32")))]
            scene_texture,
            scene_view,
            msaa_scene_view,
            blur_a_view,
            blur_b_view,
            blur_scene_bind_group,
            blur_a_bind_group,
            compose_bind_group,
        }
    }

    /// 场景 pass 的颜色附件：(渲染目标, resolve 目标)。MSAA 关闭时直接渲进
    /// 单采样 `scene_view`。
    pub(crate) fn scene_color_target(&self) -> (&wgpu::TextureView, Option<&wgpu::TextureView>) {
        match &self.msaa_scene_view {
            Some(msaa_view) => (msaa_view, Some(&self.scene_view)),
            None => (&self.scene_view, None),
        }
    }
}

pub(crate) fn create_post_pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    layout: &wgpu::PipelineLayout,
    label: &str,
    fragment_entry: &str,
    format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            buffers: &[],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some(fragment_entry),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            polygon_mode: wgpu::PolygonMode::Fill,
            unclipped_depth: false,
            conservative: false,
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}

pub(crate) fn create_post_texture_view(
    device: &wgpu::Device,
    label: &str,
    width: u32,
    height: u32,
) -> wgpu::TextureView {
    create_post_texture(device, label, width, height, wgpu::TextureUsages::empty())
        .create_view(&wgpu::TextureViewDescriptor::default())
}

pub(crate) fn create_post_texture(
    device: &wgpu::Device,
    label: &str,
    width: u32,
    height: u32,
    extra_usage: wgpu::TextureUsages,
) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: POST_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | extra_usage,
        view_formats: &[],
    })
}

pub(crate) fn create_post_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    source_view: &wgpu::TextureView,
    sampler: &wgpu::Sampler,
    uniform_buffer: &wgpu::Buffer,
    bloom_view: &wgpu::TextureView,
    label: &str,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(label),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(source_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: uniform_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::TextureView(bloom_view),
            },
        ],
    })
}
