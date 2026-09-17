use super::*;

pub(crate) fn create_model_pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    layout: &wgpu::PipelineLayout,
    label: &str,
    blend_mode: ModelPipelineBlend,
    cull_backfaces: bool,
    sample_count: u32,
) -> wgpu::RenderPipeline {
    create_model_pipeline_with_fragment_entry(
        device,
        shader,
        layout,
        label,
        blend_mode,
        blend_mode.fragment_entry(),
        cull_backfaces,
        sample_count,
    )
}

pub(crate) fn create_model_pipeline_with_fragment_entry(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    layout: &wgpu::PipelineLayout,
    label: &str,
    blend_mode: ModelPipelineBlend,
    fragment_entry: &str,
    cull_backfaces: bool,
    sample_count: u32,
) -> wgpu::RenderPipeline {
    let blend = blend_mode.blend_state();
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            buffers: &[GpuVertex::layout()],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some(fragment_entry),
            targets: &[Some(wgpu::ColorTargetState {
                format: POST_FORMAT,
                blend,
                write_mask: blend_mode.color_write_mask(),
            })],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: cull_backfaces.then_some(wgpu::Face::Back),
            polygon_mode: wgpu::PolygonMode::Fill,
            unclipped_depth: false,
            conservative: false,
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth24Plus,
            depth_write_enabled: Some(blend_mode.writes_depth()),
            depth_compare: Some(wgpu::CompareFunction::LessEqual),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState {
            count: sample_count,
            ..Default::default()
        },
        multiview_mask: None,
        cache: None,
    })
}

pub(crate) fn create_outline_pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    layout: &wgpu::PipelineLayout,
    label: &str,
    sample_count: u32,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_outline"),
            buffers: &[GpuVertex::layout()],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("fs_outline"),
            targets: &[Some(wgpu::ColorTargetState {
                format: POST_FORMAT,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: Some(wgpu::Face::Front),
            polygon_mode: wgpu::PolygonMode::Fill,
            unclipped_depth: false,
            conservative: false,
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth24Plus,
            depth_write_enabled: Some(false),
            depth_compare: Some(wgpu::CompareFunction::LessEqual),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState {
            count: sample_count,
            ..Default::default()
        },
        multiview_mask: None,
        cache: None,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ModelPipelineBlend {
    Opaque,
    DitherDepth,
    Alpha,
    Additive,
}

impl ModelPipelineBlend {
    pub(crate) fn blend_state(self) -> Option<wgpu::BlendState> {
        match self {
            ModelPipelineBlend::Opaque | ModelPipelineBlend::DitherDepth => None,
            ModelPipelineBlend::Alpha => Some(wgpu::BlendState::ALPHA_BLENDING),
            ModelPipelineBlend::Additive => Some(wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::One,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::One,
                    operation: wgpu::BlendOperation::Add,
                },
            }),
        }
    }

    pub(crate) fn writes_depth(self) -> bool {
        matches!(
            self,
            ModelPipelineBlend::Opaque | ModelPipelineBlend::DitherDepth
        )
    }

    pub(crate) fn fragment_entry(self) -> &'static str {
        match self {
            ModelPipelineBlend::DitherDepth => "fs_dither_depth",
            _ => "fs_main",
        }
    }

    pub(crate) fn color_write_mask(self) -> wgpu::ColorWrites {
        match self {
            ModelPipelineBlend::DitherDepth => wgpu::ColorWrites::empty(),
            _ => wgpu::ColorWrites::ALL,
        }
    }
}
