use wgpu::util::DeviceExt;

use crate::{
    MaterialAlphaMode, MaterialDrawDepthMode, MaterialLightingMode, MaterialRenderMode,
    ModelMaterial, ModelMeshDrawRole, ModelRenderData, ModelTexture, ModelTextureKind,
    PreparedAlphaSource, PreparedMaterial, PreparedRenderPass, PreparedTextureAddressMode,
    PreparedTextureFilter, PreparedTextureSampling, PreparedUvSource, prepare_model_for_render,
};

const POST_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const DEFAULT_BLOOM_STRENGTH: f32 = 0.68;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ModelDebugMode {
    #[default]
    Final,
    BaseColor,
    Normal,
    Mask,
    MaterialProperties,
    Specular,
    Emissive,
    Alpha,
    Uv0,
    Uv1,
    Uv2,
    Uv3,
    VertexColor,
    MeshRole,
    ColorTableIndex,
    MaterialMap,
    MultiMap,
    TileProperties,
    SheenProperties,
    SphereProperties,
    TileMatrix,
    TileNormalArray,
    TileOrbArray,
    DetailDiffuseArray,
    DetailNormalArray,
}

impl ModelDebugMode {
    fn shader_value(self) -> f32 {
        match self {
            ModelDebugMode::Final => 0.0,
            ModelDebugMode::BaseColor => 1.0,
            ModelDebugMode::Normal => 2.0,
            ModelDebugMode::Mask => 3.0,
            ModelDebugMode::MaterialProperties => 4.0,
            ModelDebugMode::Specular => 5.0,
            ModelDebugMode::Emissive => 6.0,
            ModelDebugMode::Alpha => 7.0,
            ModelDebugMode::Uv0 => 8.0,
            ModelDebugMode::Uv1 => 9.0,
            ModelDebugMode::Uv2 => 10.0,
            ModelDebugMode::Uv3 => 11.0,
            ModelDebugMode::VertexColor => 12.0,
            ModelDebugMode::MeshRole => 13.0,
            ModelDebugMode::ColorTableIndex => 14.0,
            ModelDebugMode::MaterialMap => 15.0,
            ModelDebugMode::MultiMap => 16.0,
            ModelDebugMode::TileProperties => 17.0,
            ModelDebugMode::SheenProperties => 18.0,
            ModelDebugMode::SphereProperties => 19.0,
            ModelDebugMode::TileMatrix => 20.0,
            ModelDebugMode::TileNormalArray => 21.0,
            ModelDebugMode::TileOrbArray => 22.0,
            ModelDebugMode::DetailDiffuseArray => 23.0,
            ModelDebugMode::DetailNormalArray => 24.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ModelGlassBlendMode {
    #[default]
    Multiply,
    Additive,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ModelRenderOptions {
    pub normal_mapping: bool,
    pub normal_y_sign: f32,
    pub bloom: bool,
    pub bloom_strength: f32,
    pub uv_scroll_time: f32,
    pub debug_mode: ModelDebugMode,
    pub glass_blend_mode: ModelGlassBlendMode,
}

impl Default for ModelRenderOptions {
    fn default() -> Self {
        Self {
            normal_mapping: true,
            normal_y_sign: -1.0,
            bloom: true,
            bloom_strength: DEFAULT_BLOOM_STRENGTH,
            uv_scroll_time: 0.0,
            debug_mode: ModelDebugMode::Final,
            glass_blend_mode: ModelGlassBlendMode::Multiply,
        }
    }
}

impl ModelRenderOptions {
    fn normalized(self) -> Self {
        Self {
            normal_mapping: self.normal_mapping,
            normal_y_sign: if self.normal_y_sign < 0.0 { -1.0 } else { 1.0 },
            bloom: self.bloom,
            bloom_strength: self.bloom_strength.clamp(0.0, 2.0),
            uv_scroll_time: if self.uv_scroll_time.is_finite() {
                self.uv_scroll_time
            } else {
                0.0
            },
            debug_mode: self.debug_mode,
            glass_blend_mode: self.glass_blend_mode,
        }
    }

    fn bloom_strength(self) -> f32 {
        let normalized = self.normalized();
        if normalized.bloom {
            normalized.bloom_strength
        } else {
            0.0
        }
    }
}

pub struct ModelRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    culled_pipeline: wgpu::RenderPipeline,
    cutout_pipeline: wgpu::RenderPipeline,
    cutout_culled_pipeline: wgpu::RenderPipeline,
    dither_depth_pipeline: wgpu::RenderPipeline,
    dither_depth_culled_pipeline: wgpu::RenderPipeline,
    outline_pipeline: wgpu::RenderPipeline,
    transparent_pipeline: wgpu::RenderPipeline,
    transparent_culled_pipeline: wgpu::RenderPipeline,
    glass_pipeline: wgpu::RenderPipeline,
    glass_culled_pipeline: wgpu::RenderPipeline,
    additive_pipeline: wgpu::RenderPipeline,
    additive_culled_pipeline: wgpu::RenderPipeline,
    blur_pipeline: wgpu::RenderPipeline,
    compose_pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    draw_batches: Vec<DrawBatch>,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    material_bind_groups: Vec<wgpu::BindGroup>,
    post_sampler: wgpu::Sampler,
    compose_uniform_buffer: wgpu::Buffer,
    blur_bind_group_layout: wgpu::BindGroupLayout,
    compose_bind_group_layout: wgpu::BindGroupLayout,
    post_process: Option<PostProcessState>,
    format: wgpu::TextureFormat,
    bounds_center: [f32; 3],
    bounds_radius: f32,
}

impl ModelRenderer {
    pub fn new<M: ModelRenderData + ?Sized>(
        device: wgpu::Device,
        queue: wgpu::Queue,
        format: wgpu::TextureFormat,
        model: &M,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("model shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("model.wgsl").into()),
        });
        let post_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("model postprocess shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("postprocess.wgsl").into()),
        });

        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("weapon camera uniform"),
            size: std::mem::size_of::<CameraUniform>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("weapon camera bind group layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("weapon camera bind group"),
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });

        let material_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("weapon material bind group layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 4,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 5,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 6,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 7,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 8,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 9,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 10,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 11,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 12,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 13,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 14,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 15,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 16,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 17,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 18,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                ],
            });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("weapon pipeline layout"),
            bind_group_layouts: &[
                Some(&camera_bind_group_layout),
                Some(&material_bind_group_layout),
            ],
            immediate_size: 0,
        });

        let blur_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("weapon-bloom-pass bind group layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                ],
            });
        let compose_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("weapon compose bind group layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                ],
            });

        let blur_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("weapon-bloom-pass pipeline layout"),
            bind_group_layouts: &[Some(&blur_bind_group_layout)],
            immediate_size: 0,
        });
        let compose_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("weapon compose pipeline layout"),
                bind_group_layouts: &[Some(&compose_bind_group_layout)],
                immediate_size: 0,
            });

        let pipeline = create_model_pipeline(
            &device,
            &shader,
            &pipeline_layout,
            "weapon model pipeline",
            ModelPipelineBlend::Opaque,
            false,
        );
        let culled_pipeline = create_model_pipeline(
            &device,
            &shader,
            &pipeline_layout,
            "weapon culled model pipeline",
            ModelPipelineBlend::Opaque,
            true,
        );
        let cutout_pipeline = create_model_pipeline(
            &device,
            &shader,
            &pipeline_layout,
            "weapon cutout model pipeline",
            ModelPipelineBlend::Opaque,
            false,
        );
        let cutout_culled_pipeline = create_model_pipeline(
            &device,
            &shader,
            &pipeline_layout,
            "weapon cutout culled model pipeline",
            ModelPipelineBlend::Opaque,
            true,
        );
        let dither_depth_pipeline = create_model_pipeline(
            &device,
            &shader,
            &pipeline_layout,
            "weapon dither depth pipeline",
            ModelPipelineBlend::DitherDepth,
            false,
        );
        let dither_depth_culled_pipeline = create_model_pipeline(
            &device,
            &shader,
            &pipeline_layout,
            "weapon dither depth culled pipeline",
            ModelPipelineBlend::DitherDepth,
            true,
        );
        let outline_pipeline = create_outline_pipeline(
            &device,
            &shader,
            &pipeline_layout,
            "weapon outline pipeline",
        );
        let transparent_pipeline = create_model_pipeline(
            &device,
            &shader,
            &pipeline_layout,
            "weapon transparent model pipeline",
            ModelPipelineBlend::Alpha,
            false,
        );
        let transparent_culled_pipeline = create_model_pipeline(
            &device,
            &shader,
            &pipeline_layout,
            "weapon transparent culled model pipeline",
            ModelPipelineBlend::Alpha,
            true,
        );
        let glass_pipeline = create_model_pipeline(
            &device,
            &shader,
            &pipeline_layout,
            "weapon glass model pipeline",
            ModelPipelineBlend::Alpha,
            false,
        );
        let glass_culled_pipeline = create_model_pipeline(
            &device,
            &shader,
            &pipeline_layout,
            "weapon glass culled model pipeline",
            ModelPipelineBlend::Alpha,
            true,
        );
        let additive_pipeline = create_model_pipeline(
            &device,
            &shader,
            &pipeline_layout,
            "weapon additive model pipeline",
            ModelPipelineBlend::Additive,
            false,
        );
        let additive_culled_pipeline = create_model_pipeline(
            &device,
            &shader,
            &pipeline_layout,
            "weapon additive culled model pipeline",
            ModelPipelineBlend::Additive,
            true,
        );

        let blur_pipeline = create_post_pipeline(
            &device,
            &post_shader,
            &blur_pipeline_layout,
            "weapon-bloom-pass pipeline",
            "blur_fs",
            POST_FORMAT,
        );
        let compose_pipeline = create_post_pipeline(
            &device,
            &post_shader,
            &compose_pipeline_layout,
            "weapon compose pipeline",
            "compose_fs",
            format,
        );

        let (vertices, indices, draw_batches) = flatten_model(model);
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("weapon vertex buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("weapon index buffer"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        let material_bind_groups = create_material_bind_groups(
            &device,
            &queue,
            &material_bind_group_layout,
            model,
            &draw_batches,
        );
        let post_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("weapon postprocess sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
        let compose_uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("weapon compose uniform"),
            contents: bytemuck::bytes_of(&PostUniform {
                params: [DEFAULT_BLOOM_STRENGTH, 0.0, 0.0, 0.0],
            }),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        Self {
            device,
            queue,
            pipeline,
            culled_pipeline,
            cutout_pipeline,
            cutout_culled_pipeline,
            dither_depth_pipeline,
            dither_depth_culled_pipeline,
            outline_pipeline,
            transparent_pipeline,
            transparent_culled_pipeline,
            glass_pipeline,
            glass_culled_pipeline,
            additive_pipeline,
            additive_culled_pipeline,
            blur_pipeline,
            compose_pipeline,
            vertex_buffer,
            index_buffer,
            draw_batches,
            camera_buffer,
            camera_bind_group,
            material_bind_groups,
            post_sampler,
            compose_uniform_buffer,
            blur_bind_group_layout,
            compose_bind_group_layout,
            post_process: None,
            format,
            bounds_center: model.bounds().center,
            bounds_radius: model.bounds().radius,
        }
    }

    pub fn render_to(
        &mut self,
        target_view: &wgpu::TextureView,
        depth_view: &wgpu::TextureView,
        viewport: [u32; 2],
        yaw: f32,
        pitch: f32,
        zoom: f32,
        pan: [f32; 2],
        options: ModelRenderOptions,
    ) {
        let uniform = camera_uniform(
            self.bounds_center,
            self.bounds_radius,
            viewport,
            yaw,
            pitch,
            zoom,
            pan,
            options,
        );
        self.queue
            .write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&uniform));
        self.queue.write_buffer(
            &self.compose_uniform_buffer,
            0,
            bytemuck::bytes_of(&PostUniform {
                params: [options.bloom_strength(), 0.0, 0.0, 0.0],
            }),
        );
        let viewport = [viewport[0].max(1), viewport[1].max(1)];
        self.ensure_post_process_targets(viewport);
        let post = self
            .post_process
            .as_ref()
            .expect("post process targets are initialized");

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("weapon render encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("weapon scene render pass"),
                color_attachments: &[
                    Some(wgpu::RenderPassColorAttachment {
                        view: &post.scene_view,
                        resolve_target: None,
                        depth_slice: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: 0.055,
                                g: 0.061,
                                b: 0.067,
                                a: 1.0,
                            }),
                            store: wgpu::StoreOp::Store,
                        },
                    }),
                    Some(wgpu::RenderPassColorAttachment {
                        view: &post.bright_view,
                        resolve_target: None,
                        depth_slice: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                    }),
                ],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                occlusion_query_set: None,
                timestamp_writes: None,
                multiview_mask: None,
            });

            render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);

            for batch in self
                .draw_batches
                .iter()
                .filter(|batch| batch.pass() == PreparedRenderPass::Opaque)
            {
                render_pass.set_pipeline(if batch.render_backfaces() {
                    &self.pipeline
                } else {
                    &self.culled_pipeline
                });
                draw_model_batch(&mut render_pass, &self.material_bind_groups, batch);
            }

            for batch in self
                .draw_batches
                .iter()
                .filter(|batch| batch.pass() == PreparedRenderPass::Cutout)
            {
                render_pass.set_pipeline(if batch.render_backfaces() {
                    &self.cutout_pipeline
                } else {
                    &self.cutout_culled_pipeline
                });
                draw_model_batch(&mut render_pass, &self.material_bind_groups, batch);
            }

            for batch in self
                .draw_batches
                .iter()
                .filter(|batch| batch.uses_dither_depth_prepass())
            {
                render_pass.set_pipeline(if batch.render_backfaces() {
                    &self.dither_depth_pipeline
                } else {
                    &self.dither_depth_culled_pipeline
                });
                draw_model_batch(&mut render_pass, &self.material_bind_groups, batch);
            }

            render_pass.set_pipeline(&self.outline_pipeline);
            for batch in self
                .draw_batches
                .iter()
                .filter(|batch| batch.uses_outline_pass())
            {
                draw_model_batch(&mut render_pass, &self.material_bind_groups, batch);
            }

            let sorted_transparent_batches =
                sorted_transparent_batches(&self.draw_batches, yaw, pitch);
            for batch in sorted_transparent_batches {
                let pipeline = if batch.pass() == PreparedRenderPass::Glass {
                    if batch.uses_additive_glass_pipeline(options.glass_blend_mode) {
                        if batch.render_backfaces() {
                            &self.additive_pipeline
                        } else {
                            &self.additive_culled_pipeline
                        }
                    } else if batch.render_backfaces() {
                        &self.glass_pipeline
                    } else {
                        &self.glass_culled_pipeline
                    }
                } else if batch.render_backfaces() {
                    &self.transparent_pipeline
                } else {
                    &self.transparent_culled_pipeline
                };
                render_pass.set_pipeline(pipeline);
                draw_model_batch(&mut render_pass, &self.material_bind_groups, batch);
            }

            for batch in self
                .draw_batches
                .iter()
                .filter(|batch| batch.pass().uses_additive_pipeline())
            {
                render_pass.set_pipeline(if batch.render_backfaces() {
                    &self.additive_pipeline
                } else {
                    &self.additive_culled_pipeline
                });
                draw_model_batch(&mut render_pass, &self.material_bind_groups, batch);
            }
        }

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("weapon bloom horizontal pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &post.blur_a_view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
                multiview_mask: None,
            });
            render_pass.set_pipeline(&self.blur_pipeline);
            render_pass.set_bind_group(0, &post.blur_bright_bind_group, &[]);
            render_pass.draw(0..3, 0..1);
        }

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("weapon bloom vertical pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &post.blur_b_view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
                multiview_mask: None,
            });
            render_pass.set_pipeline(&self.blur_pipeline);
            render_pass.set_bind_group(0, &post.blur_a_bind_group, &[]);
            render_pass.draw(0..3, 0..1);
        }

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("weapon compose pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target_view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
                multiview_mask: None,
            });
            render_pass.set_pipeline(&self.compose_pipeline);
            render_pass.set_bind_group(0, &post.compose_bind_group, &[]);
            render_pass.draw(0..3, 0..1);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
    }

    fn ensure_post_process_targets(&mut self, viewport: [u32; 2]) {
        let width = viewport[0].max(1);
        let height = viewport[1].max(1);
        let needs_recreate = self
            .post_process
            .as_ref()
            .map(|targets| targets.width != width || targets.height != height)
            .unwrap_or(true);

        if needs_recreate {
            self.post_process = Some(PostProcessState::new(
                &self.device,
                &self.blur_bind_group_layout,
                &self.compose_bind_group_layout,
                &self.post_sampler,
                &self.compose_uniform_buffer,
                width,
                height,
            ));
        }
    }

    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    pub fn format(&self) -> wgpu::TextureFormat {
        self.format
    }
}

struct PostProcessState {
    width: u32,
    height: u32,
    scene_view: wgpu::TextureView,
    bright_view: wgpu::TextureView,
    blur_a_view: wgpu::TextureView,
    blur_b_view: wgpu::TextureView,
    blur_bright_bind_group: wgpu::BindGroup,
    blur_a_bind_group: wgpu::BindGroup,
    compose_bind_group: wgpu::BindGroup,
}

impl PostProcessState {
    fn new(
        device: &wgpu::Device,
        blur_layout: &wgpu::BindGroupLayout,
        compose_layout: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
        compose_uniform_buffer: &wgpu::Buffer,
        width: u32,
        height: u32,
    ) -> Self {
        let scene_view = create_post_texture_view(device, "weapon scene texture", width, height);
        let bright_view = create_post_texture_view(device, "weapon bright texture", width, height);
        let blur_a_view = create_post_texture_view(device, "weapon-bloom-pass a", width, height);
        let blur_b_view = create_post_texture_view(device, "weapon-bloom-pass b", width, height);

        let blur_horizontal_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("weapon-bloom-pass horizontal uniform"),
            contents: bytemuck::bytes_of(&PostUniform {
                params: [1.0 / width as f32, 0.0, 0.0, 0.0],
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

        let blur_bright_bind_group = create_post_bind_group(
            device,
            blur_layout,
            &bright_view,
            sampler,
            &blur_horizontal_buffer,
            &bright_view,
            "weapon-bloom-pass bright bind group",
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
            scene_view,
            bright_view,
            blur_a_view,
            blur_b_view,
            blur_bright_bind_group,
            blur_a_bind_group,
            compose_bind_group,
        }
    }
}

fn flatten_model<M: ModelRenderData + ?Sized>(
    model: &M,
) -> (Vec<GpuVertex>, Vec<u32>, Vec<DrawBatch>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let mut draw_batches = Vec::new();

    let prepared_model = prepare_model_for_render(model);
    for prepared_mesh in &prepared_model.meshes {
        if !prepared_mesh.renders_in_main_pass
            && !prepared_mesh
                .prepared_material
                .render_pass
                .uses_additive_pipeline()
        {
            continue;
        }
        let Some(mesh) = model.meshes().get(prepared_mesh.mesh_index) else {
            continue;
        };

        let base = vertices.len() as u32;
        vertices.extend(mesh.vertices.iter().map(GpuVertex::from_model_vertex));
        let index_start = indices.len() as u32;
        indices.extend(mesh.indices.iter().map(|index| base + *index));
        draw_batches.push(DrawBatch {
            material_slot: prepared_mesh.material_slot,
            material_bind_group_index: draw_batches.len(),
            draw_role: prepared_mesh.draw_role,
            index_start,
            index_count: mesh.indices.len() as u32,
            prepared_material: prepared_mesh.prepared_material,
            center: mesh_bounds_center(mesh),
        });
    }

    (vertices, indices, draw_batches)
}

fn mesh_bounds_center(mesh: &crate::ModelMesh) -> [f32; 3] {
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for vertex in &mesh.vertices {
        for axis in 0..3 {
            min[axis] = min[axis].min(vertex.position[axis]);
            max[axis] = max[axis].max(vertex.position[axis]);
        }
    }

    if min.iter().any(|value| !value.is_finite()) || max.iter().any(|value| !value.is_finite()) {
        return [0.0; 3];
    }

    [
        (min[0] + max[0]) * 0.5,
        (min[1] + max[1]) * 0.5,
        (min[2] + max[2]) * 0.5,
    ]
}

fn sorted_transparent_batches(draw_batches: &[DrawBatch], yaw: f32, pitch: f32) -> Vec<&DrawBatch> {
    let sort_dir = transparent_sort_direction(yaw, pitch);
    let mut batches = draw_batches
        .iter()
        .filter(|batch| batch.pass().sorts_back_to_front())
        .collect::<Vec<_>>();
    batches.sort_by(|left, right| {
        let left_depth = glam::Vec3::from(left.center).dot(sort_dir);
        let right_depth = glam::Vec3::from(right.center).dot(sort_dir);
        right_depth
            .partial_cmp(&left_depth)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    batches
}

fn transparent_sort_direction(yaw: f32, pitch: f32) -> glam::Vec3 {
    let pitch = pitch.clamp(-1.35, 1.35);
    glam::Vec3::new(
        -yaw.sin() * pitch.cos(),
        -pitch.sin(),
        -yaw.cos() * pitch.cos(),
    )
    .normalize_or_zero()
}

fn draw_model_batch<'a>(
    render_pass: &mut wgpu::RenderPass<'a>,
    material_bind_groups: &'a [wgpu::BindGroup],
    batch: &DrawBatch,
) {
    if let Some(bind_group) = material_bind_groups
        .get(batch.material_bind_group_index)
        .or_else(|| material_bind_groups.first())
    {
        render_pass.set_bind_group(1, bind_group, &[]);
        render_pass.draw_indexed(
            batch.index_start..batch.index_start + batch.index_count,
            0,
            0..1,
        );
    }
}

fn create_model_pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    layout: &wgpu::PipelineLayout,
    label: &str,
    blend_mode: ModelPipelineBlend,
    cull_backfaces: bool,
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
            entry_point: Some(blend_mode.fragment_entry()),
            targets: &[
                Some(wgpu::ColorTargetState {
                    format: POST_FORMAT,
                    blend,
                    write_mask: blend_mode.color_write_mask(),
                }),
                Some(wgpu::ColorTargetState {
                    format: POST_FORMAT,
                    blend,
                    write_mask: blend_mode.color_write_mask(),
                }),
            ],
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
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}

fn create_outline_pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    layout: &wgpu::PipelineLayout,
    label: &str,
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
            targets: &[
                Some(wgpu::ColorTargetState {
                    format: POST_FORMAT,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                }),
                Some(wgpu::ColorTargetState {
                    format: POST_FORMAT,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                }),
            ],
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
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ModelPipelineBlend {
    Opaque,
    DitherDepth,
    Alpha,
    Additive,
}

impl ModelPipelineBlend {
    fn blend_state(self) -> Option<wgpu::BlendState> {
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

    fn writes_depth(self) -> bool {
        matches!(
            self,
            ModelPipelineBlend::Opaque | ModelPipelineBlend::DitherDepth
        )
    }

    fn fragment_entry(self) -> &'static str {
        match self {
            ModelPipelineBlend::DitherDepth => "fs_dither_depth",
            _ => "fs_main",
        }
    }

    fn color_write_mask(self) -> wgpu::ColorWrites {
        match self {
            ModelPipelineBlend::DitherDepth => wgpu::ColorWrites::empty(),
            _ => wgpu::ColorWrites::ALL,
        }
    }
}

fn create_post_pipeline(
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

fn create_post_texture_view(
    device: &wgpu::Device,
    label: &str,
    width: u32,
    height: u32,
) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
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
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
        .create_view(&wgpu::TextureViewDescriptor::default())
}

fn create_post_bind_group(
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

fn create_material_bind_groups<M: ModelRenderData + ?Sized>(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
    model: &M,
    draw_batches: &[DrawBatch],
) -> Vec<wgpu::BindGroup> {
    // Pair related atlases horizontally to stay below common WebGPU per-stage texture limits.
    let tile_array_pair_texture = create_array_pair_texture(
        device,
        queue,
        "weapon tile array pair",
        model_texture_pair_for_kinds(
            model,
            ModelTextureKind::TileNormalArray,
            ModelTextureKind::TileOrbArray,
        ),
        [128, 128, 255, 255],
        [255, 128, 255, 255],
    );
    let tile_array_pair_view =
        tile_array_pair_texture.create_view(&wgpu::TextureViewDescriptor::default());
    let detail_array_pair_texture = create_array_pair_texture(
        device,
        queue,
        "weapon detail array pair",
        model_texture_pair_for_kinds(
            model,
            ModelTextureKind::DetailDiffuseArray,
            ModelTextureKind::DetailNormalArray,
        ),
        [128, 128, 128, 255],
        [128, 128, 255, 255],
    );
    let detail_array_pair_view =
        detail_array_pair_texture.create_view(&wgpu::TextureViewDescriptor::default());

    draw_batches
        .iter()
        .map(|batch| {
            let fallback = fallback_material();
            let material = model
                .materials()
                .get(batch.material_slot)
                .unwrap_or(&fallback);
            create_material_bind_group(
                device,
                queue,
                layout,
                material,
                model,
                batch.prepared_material,
                batch.draw_role,
                &tile_array_pair_view,
                &detail_array_pair_view,
            )
        })
        .collect()
}

fn create_material_bind_group<M: ModelRenderData + ?Sized>(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
    material: &ModelMaterial,
    model: &M,
    prepared_material: PreparedMaterial,
    draw_role: ModelMeshDrawRole,
    tile_array_pair_view: &wgpu::TextureView,
    detail_array_pair_view: &wgpu::TextureView,
) -> wgpu::BindGroup {
    let effective_mask_texture = effective_mask_texture(material);
    let uv_sources = material_uv_source_params(prepared_material);
    let uniform = MaterialUniform {
        diffuse_color: [
            material.diffuse_color[0],
            material.diffuse_color[1],
            material.diffuse_color[2],
            material.opacity.clamp(0.0, 1.0),
        ],
        emissive_color: [
            material.emissive_color[0],
            material.emissive_color[1],
            material.emissive_color[2],
            material
                .emissive_texture
                .and_then(|index| model.textures().get(index))
                .map(|_| 1.0)
                .unwrap_or(0.0),
        ],
        specular_color: [
            material.specular_color[0],
            material.specular_color[1],
            material.specular_color[2],
            material.roughness,
        ],
        params: [
            material
                .base_color_texture
                .and_then(|index| model.textures().get(index))
                .map(|_| 1.0)
                .unwrap_or(0.0),
            material.metalness,
            material
                .normal_texture
                .and_then(|index| model.textures().get(index))
                .map(|_| 1.0)
                .unwrap_or(0.0),
            effective_mask_texture
                .and_then(|index| model.textures().get(index))
                .map(|_| 1.0)
                .unwrap_or(0.0),
        ],
        properties: [
            material
                .material_properties_texture
                .and_then(|index| model.textures().get(index))
                .map(|_| 1.0)
                .unwrap_or(0.0),
            material
                .specular_texture
                .and_then(|index| model.textures().get(index))
                .map(|_| 1.0)
                .unwrap_or(0.0),
            if material.apply_vertex_color {
                1.0
            } else {
                0.0
            },
            0.0,
        ],
        render: [
            render_mode_value(material.render_mode),
            material.opacity,
            alpha_mode_value(material.alpha_mode),
            material.alpha_threshold.clamp(0.0, 1.0),
        ],
        alpha_params: material_alpha_params(material),
        alpha_policy_params: material_alpha_policy_params(prepared_material),
        glass_params: material_glass_params(material),
        extra_properties: material_extra_texture_flags(material, model),
        shader_params: material_shader_params(material),
        tile_params: material_tile_params(material),
        toon_sheen_params: material_toon_sheen_params(material),
        toon_params: material_toon_params(material, prepared_material),
        sheen_sphere_params: material_sheen_sphere_params(material),
        detail_params: material_detail_params(material),
        array_params: material_array_params(material, model),
        detail_color: material_detail_color(material),
        multi_detail_color: material_multi_detail_color(material),
        shader_diffuse_color: material_shader_diffuse_color(material),
        shader_multi_diffuse_color: material_shader_multi_diffuse_color(material),
        shader_emissive_color: material_shader_emissive_color(material),
        shader_multi_emissive_color: material_shader_multi_emissive_color(material),
        outline_params: material_outline_params(material),
        specular_color_mask: material_specular_color_mask(material),
        surface_params: material_surface_params(material),
        detail_color_uv_scale: material_detail_color_uv_scale(material),
        detail_normal_uv_scale: material_detail_normal_uv_scale(material),
        uv_scroll: material_uv_scroll(material),
        lightshaft_color: material_lightshaft_color(material),
        lightshaft_tex_anim: material_lightshaft_tex_anim(material),
        lightshaft_tex_u: material_lightshaft_tex_u(material),
        lightshaft_tex_v: material_lightshaft_tex_v(material),
        lightshaft_ray: material_lightshaft_ray(material),
        uv_sources0: uv_sources.0,
        uv_sources1: uv_sources.1,
        uv_sources2: uv_sources.2,
        uv_sources3: uv_sources.3,
        draw_role_params: draw_role_params(draw_role),
        debug_color: draw_role_debug_color(draw_role),
    };
    let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("weapon material uniform"),
        contents: bytemuck::bytes_of(&uniform),
        usage: wgpu::BufferUsages::UNIFORM,
    });

    let texture_view = material
        .base_color_texture
        .and_then(|index| model.textures().get(index))
        .map(|texture| {
            create_rgba_texture(
                device,
                queue,
                &format!("weapon texture {}", texture.path),
                texture.width.max(1) as u32,
                texture.height.max(1) as u32,
                &texture.rgba,
                wgpu::TextureFormat::Rgba8UnormSrgb,
            )
        })
        .unwrap_or_else(|| {
            create_rgba_texture(
                device,
                queue,
                "weapon white texture",
                1,
                1,
                &[255, 255, 255, 255],
                wgpu::TextureFormat::Rgba8UnormSrgb,
            )
        })
        .create_view(&wgpu::TextureViewDescriptor::default());
    let mask_texture_view = effective_mask_texture
        .and_then(|index| model.textures().get(index))
        .map(|texture| {
            create_rgba_texture(
                device,
                queue,
                &format!("weapon mask texture {}", texture.path),
                texture.width.max(1) as u32,
                texture.height.max(1) as u32,
                &texture.rgba,
                wgpu::TextureFormat::Rgba8Unorm,
            )
        })
        .unwrap_or_else(|| {
            create_rgba_texture(
                device,
                queue,
                "weapon neutral mask texture",
                1,
                1,
                &[255, 128, 0, 255],
                wgpu::TextureFormat::Rgba8Unorm,
            )
        })
        .create_view(&wgpu::TextureViewDescriptor::default());
    let emissive_texture_view = material
        .emissive_texture
        .and_then(|index| model.textures().get(index))
        .map(|texture| {
            create_rgba_texture(
                device,
                queue,
                &format!("weapon emissive texture {}", texture.path),
                texture.width.max(1) as u32,
                texture.height.max(1) as u32,
                &texture.rgba,
                wgpu::TextureFormat::Rgba8UnormSrgb,
            )
        })
        .unwrap_or_else(|| {
            create_rgba_texture(
                device,
                queue,
                "weapon black emissive texture",
                1,
                1,
                &[0, 0, 0, 255],
                wgpu::TextureFormat::Rgba8UnormSrgb,
            )
        })
        .create_view(&wgpu::TextureViewDescriptor::default());
    let normal_texture_view = material
        .normal_texture
        .and_then(|index| model.textures().get(index))
        .map(|texture| {
            create_rgba_texture(
                device,
                queue,
                &format!("weapon normal texture {}", texture.path),
                texture.width.max(1) as u32,
                texture.height.max(1) as u32,
                &texture.rgba,
                wgpu::TextureFormat::Rgba8Unorm,
            )
        })
        .unwrap_or_else(|| {
            create_rgba_texture(
                device,
                queue,
                "weapon flat normal texture",
                1,
                1,
                &[128, 128, 255, 255],
                wgpu::TextureFormat::Rgba8Unorm,
            )
        })
        .create_view(&wgpu::TextureViewDescriptor::default());
    let material_properties_texture_view = material
        .material_properties_texture
        .and_then(|index| model.textures().get(index))
        .map(|texture| {
            create_rgba_texture(
                device,
                queue,
                &format!("weapon material properties texture {}", texture.path),
                texture.width.max(1) as u32,
                texture.height.max(1) as u32,
                &texture.rgba,
                wgpu::TextureFormat::Rgba8Unorm,
            )
        })
        .unwrap_or_else(|| {
            create_rgba_texture(
                device,
                queue,
                "weapon neutral material properties texture",
                1,
                1,
                &[
                    unorm_byte(material.metalness),
                    unorm_byte(material.roughness),
                    255,
                    255,
                ],
                wgpu::TextureFormat::Rgba8Unorm,
            )
        })
        .create_view(&wgpu::TextureViewDescriptor::default());
    let specular_texture_view = material
        .specular_texture
        .and_then(|index| model.textures().get(index))
        .map(|texture| {
            create_rgba_texture(
                device,
                queue,
                &format!("weapon specular texture {}", texture.path),
                texture.width.max(1) as u32,
                texture.height.max(1) as u32,
                &texture.rgba,
                wgpu::TextureFormat::Rgba8UnormSrgb,
            )
        })
        .unwrap_or_else(|| {
            create_rgba_texture(
                device,
                queue,
                "weapon neutral specular texture",
                1,
                1,
                &[
                    srgb_byte(material.specular_color[0]),
                    srgb_byte(material.specular_color[1]),
                    srgb_byte(material.specular_color[2]),
                    255,
                ],
                wgpu::TextureFormat::Rgba8UnormSrgb,
            )
        })
        .create_view(&wgpu::TextureViewDescriptor::default());
    let tile_properties_texture_view = material
        .tile_properties_texture
        .and_then(|index| model.textures().get(index))
        .map(|texture| {
            create_rgba_texture(
                device,
                queue,
                &format!("weapon tile properties texture {}", texture.path),
                texture.width.max(1) as u32,
                texture.height.max(1) as u32,
                &texture.rgba,
                wgpu::TextureFormat::Rgba8Unorm,
            )
        })
        .unwrap_or_else(|| {
            create_rgba_texture(
                device,
                queue,
                "weapon neutral tile properties texture",
                1,
                1,
                &[0, 255, 255, 255],
                wgpu::TextureFormat::Rgba8Unorm,
            )
        })
        .create_view(&wgpu::TextureViewDescriptor::default());
    let sheen_properties_texture_view = material
        .sheen_properties_texture
        .and_then(|index| model.textures().get(index))
        .map(|texture| {
            create_rgba_texture(
                device,
                queue,
                &format!("weapon sheen properties texture {}", texture.path),
                texture.width.max(1) as u32,
                texture.height.max(1) as u32,
                &texture.rgba,
                wgpu::TextureFormat::Rgba8Unorm,
            )
        })
        .unwrap_or_else(|| {
            create_rgba_texture(
                device,
                queue,
                "weapon neutral sheen properties texture",
                1,
                1,
                &[0, 0, 0, 255],
                wgpu::TextureFormat::Rgba8Unorm,
            )
        })
        .create_view(&wgpu::TextureViewDescriptor::default());
    let sphere_properties_texture_view = material
        .sphere_properties_texture
        .and_then(|index| model.textures().get(index))
        .map(|texture| {
            create_rgba_texture(
                device,
                queue,
                &format!("weapon sphere properties texture {}", texture.path),
                texture.width.max(1) as u32,
                texture.height.max(1) as u32,
                &texture.rgba,
                wgpu::TextureFormat::Rgba8Unorm,
            )
        })
        .unwrap_or_else(|| {
            create_rgba_texture(
                device,
                queue,
                "weapon neutral sphere properties texture",
                1,
                1,
                &[0, 0, 255, 255],
                wgpu::TextureFormat::Rgba8Unorm,
            )
        })
        .create_view(&wgpu::TextureViewDescriptor::default());
    let tile_matrix_texture_view = material
        .tile_matrix_texture
        .and_then(|index| model.textures().get(index))
        .map(|texture| {
            create_rgba_texture(
                device,
                queue,
                &format!("weapon tile matrix texture {}", texture.path),
                texture.width.max(1) as u32,
                texture.height.max(1) as u32,
                &texture.rgba,
                wgpu::TextureFormat::Rgba8Unorm,
            )
        })
        .unwrap_or_else(|| {
            create_rgba_texture(
                device,
                queue,
                "weapon neutral tile matrix texture",
                1,
                1,
                &[255, 0, 0, 255],
                wgpu::TextureFormat::Rgba8Unorm,
            )
        })
        .create_view(&wgpu::TextureViewDescriptor::default());
    let index_texture_view = material
        .index_texture
        .and_then(|index| model.textures().get(index))
        .map(|texture| {
            create_rgba_texture(
                device,
                queue,
                &format!("weapon ColorTable index texture {}", texture.path),
                texture.width.max(1) as u32,
                texture.height.max(1) as u32,
                &texture.rgba,
                wgpu::TextureFormat::Rgba8Unorm,
            )
        })
        .unwrap_or_else(|| {
            create_rgba_texture(
                device,
                queue,
                "weapon neutral ColorTable index texture",
                1,
                1,
                &[0, 0, 0, 255],
                wgpu::TextureFormat::Rgba8Unorm,
            )
        })
        .create_view(&wgpu::TextureViewDescriptor::default());
    let material_map_texture_view = material
        .material_map_texture
        .and_then(|index| model.textures().get(index))
        .map(|texture| {
            create_rgba_texture(
                device,
                queue,
                &format!("weapon material map texture {}", texture.path),
                texture.width.max(1) as u32,
                texture.height.max(1) as u32,
                &texture.rgba,
                wgpu::TextureFormat::Rgba8Unorm,
            )
        })
        .unwrap_or_else(|| {
            create_rgba_texture(
                device,
                queue,
                "weapon neutral material map texture",
                1,
                1,
                &[0, 0, 0, 255],
                wgpu::TextureFormat::Rgba8Unorm,
            )
        })
        .create_view(&wgpu::TextureViewDescriptor::default());
    let multi_map_texture_view = material
        .multi_map_texture
        .and_then(|index| model.textures().get(index))
        .map(|texture| {
            create_rgba_texture(
                device,
                queue,
                &format!("weapon multi map texture {}", texture.path),
                texture.width.max(1) as u32,
                texture.height.max(1) as u32,
                &texture.rgba,
                wgpu::TextureFormat::Rgba8Unorm,
            )
        })
        .unwrap_or_else(|| {
            create_rgba_texture(
                device,
                queue,
                "weapon neutral multi map texture",
                1,
                1,
                &[0, 0, 0, 255],
                wgpu::TextureFormat::Rgba8Unorm,
            )
        })
        .create_view(&wgpu::TextureViewDescriptor::default());

    let color_sampler = create_sampler_for_sampling(
        device,
        "weapon material color sampler",
        material_color_sampler_policy(prepared_material),
    );
    let data_sampler = create_sampler_for_sampling(
        device,
        "weapon material data sampler",
        material_data_sampler_policy(prepared_material),
    );
    let nearest_data_sampler = create_sampler_for_sampling(
        device,
        "weapon material nearest data sampler",
        material_nearest_sampler_policy(prepared_material),
    );

    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("weapon material bind group"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&texture_view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(&color_sampler),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::TextureView(&normal_texture_view),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: wgpu::BindingResource::TextureView(&mask_texture_view),
            },
            wgpu::BindGroupEntry {
                binding: 5,
                resource: wgpu::BindingResource::TextureView(&emissive_texture_view),
            },
            wgpu::BindGroupEntry {
                binding: 6,
                resource: wgpu::BindingResource::TextureView(&material_properties_texture_view),
            },
            wgpu::BindGroupEntry {
                binding: 7,
                resource: wgpu::BindingResource::TextureView(&specular_texture_view),
            },
            wgpu::BindGroupEntry {
                binding: 8,
                resource: wgpu::BindingResource::Sampler(&data_sampler),
            },
            wgpu::BindGroupEntry {
                binding: 9,
                resource: wgpu::BindingResource::TextureView(&tile_properties_texture_view),
            },
            wgpu::BindGroupEntry {
                binding: 10,
                resource: wgpu::BindingResource::TextureView(&sheen_properties_texture_view),
            },
            wgpu::BindGroupEntry {
                binding: 11,
                resource: wgpu::BindingResource::TextureView(&sphere_properties_texture_view),
            },
            wgpu::BindGroupEntry {
                binding: 12,
                resource: wgpu::BindingResource::TextureView(&tile_matrix_texture_view),
            },
            wgpu::BindGroupEntry {
                binding: 13,
                resource: wgpu::BindingResource::Sampler(&nearest_data_sampler),
            },
            wgpu::BindGroupEntry {
                binding: 14,
                resource: wgpu::BindingResource::TextureView(&index_texture_view),
            },
            wgpu::BindGroupEntry {
                binding: 15,
                resource: wgpu::BindingResource::TextureView(&material_map_texture_view),
            },
            wgpu::BindGroupEntry {
                binding: 16,
                resource: wgpu::BindingResource::TextureView(&multi_map_texture_view),
            },
            wgpu::BindGroupEntry {
                binding: 17,
                resource: wgpu::BindingResource::TextureView(tile_array_pair_view),
            },
            wgpu::BindGroupEntry {
                binding: 18,
                resource: wgpu::BindingResource::TextureView(detail_array_pair_view),
            },
        ],
    })
}

fn material_color_sampler_policy(prepared_material: PreparedMaterial) -> PreparedTextureSampling {
    prepared_material.texture_sampling.base_color
}

fn material_data_sampler_policy(prepared_material: PreparedMaterial) -> PreparedTextureSampling {
    prepared_material.texture_sampling.normal
}

fn material_nearest_sampler_policy(prepared_material: PreparedMaterial) -> PreparedTextureSampling {
    prepared_material.texture_sampling.index
}

fn create_sampler_for_sampling(
    device: &wgpu::Device,
    label: &'static str,
    sampling: PreparedTextureSampling,
) -> wgpu::Sampler {
    device.create_sampler(&sampler_descriptor_for_sampling(label, sampling))
}

fn sampler_descriptor_for_sampling(
    label: &'static str,
    sampling: PreparedTextureSampling,
) -> wgpu::SamplerDescriptor<'static> {
    let address_mode = sampler_address_mode(sampling.address_mode);
    let filter_mode = sampler_filter_mode(sampling.filter);
    wgpu::SamplerDescriptor {
        label: Some(label),
        address_mode_u: address_mode,
        address_mode_v: address_mode,
        address_mode_w: address_mode,
        mag_filter: filter_mode,
        min_filter: filter_mode,
        mipmap_filter: wgpu::MipmapFilterMode::Nearest,
        ..Default::default()
    }
}

fn sampler_address_mode(address_mode: PreparedTextureAddressMode) -> wgpu::AddressMode {
    match address_mode {
        PreparedTextureAddressMode::Repeat => wgpu::AddressMode::Repeat,
        PreparedTextureAddressMode::ClampToEdge | PreparedTextureAddressMode::Clip => {
            wgpu::AddressMode::ClampToEdge
        }
    }
}

fn sampler_filter_mode(filter: PreparedTextureFilter) -> wgpu::FilterMode {
    match filter {
        PreparedTextureFilter::Linear => wgpu::FilterMode::Linear,
        PreparedTextureFilter::Nearest => wgpu::FilterMode::Nearest,
    }
}

fn material_array_params<M: ModelRenderData + ?Sized>(
    material: &ModelMaterial,
    model: &M,
) -> [f32; 4] {
    let tile = material_texture_array_pair(
        material.texture_arrays.tile_normal,
        material.texture_arrays.tile_orb,
        model,
        ModelTextureKind::TileNormalArray,
        ModelTextureKind::TileOrbArray,
    );
    let detail = material_texture_array_pair(
        material.texture_arrays.detail_diffuse,
        material.texture_arrays.detail_normal,
        model,
        ModelTextureKind::DetailDiffuseArray,
        ModelTextureKind::DetailNormalArray,
    );
    [
        tile.map(|(texture, _)| f32::from(texture.array_size))
            .unwrap_or(1.0),
        detail
            .map(|(texture, _)| f32::from(texture.array_size))
            .unwrap_or(1.0),
        if tile.is_some() { 1.0 } else { 0.0 },
        if detail.is_some() { 1.0 } else { 0.0 },
    ]
}

fn material_texture_array_pair<'a, M: ModelRenderData + ?Sized>(
    first: Option<usize>,
    second: Option<usize>,
    model: &'a M,
    first_kind: ModelTextureKind,
    second_kind: ModelTextureKind,
) -> Option<(&'a ModelTexture, &'a ModelTexture)> {
    let first_index = first?;
    let second_index = second?;
    let first = model.textures().get(first_index)?;
    let second = model.textures().get(second_index)?;
    (first.kind == first_kind
        && second.kind == second_kind
        && model
            .textures()
            .iter()
            .position(|texture| texture.kind == first_kind)
            == Some(first_index)
        && model
            .textures()
            .iter()
            .position(|texture| texture.kind == second_kind)
            == Some(second_index)
        && texture_array_pair_is_compatible(first, second))
    .then_some((first, second))
}

fn model_texture_pair_for_kinds<M: ModelRenderData + ?Sized>(
    model: &M,
    first_kind: ModelTextureKind,
    second_kind: ModelTextureKind,
) -> Option<(&ModelTexture, &ModelTexture)> {
    let first = model
        .textures()
        .iter()
        .find(|texture| texture.kind == first_kind)?;
    let second = model
        .textures()
        .iter()
        .find(|texture| texture.kind == second_kind)?;
    texture_array_pair_is_compatible(first, second).then_some((first, second))
}

fn texture_array_pair_is_compatible(first: &ModelTexture, second: &ModelTexture) -> bool {
    texture_array_layout_is_valid(first)
        && texture_array_layout_is_valid(second)
        && first.width == second.width
        && first.height == second.height
        && first.array_size == second.array_size
        && first.array_layer_height == second.array_layer_height
}

fn texture_array_layout_is_valid(texture: &ModelTexture) -> bool {
    texture.width != 0
        && texture.array_size > 1
        && texture.array_layer_height != 0
        && u32::from(texture.height)
            == u32::from(texture.array_size) * u32::from(texture.array_layer_height)
        && texture.rgba.len() == usize::from(texture.width) * usize::from(texture.height) * 4
}

fn create_array_pair_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    label: &str,
    pair: Option<(&ModelTexture, &ModelTexture)>,
    fallback_first: [u8; 4],
    fallback_second: [u8; 4],
) -> wgpu::Texture {
    let Some((first, second)) = pair else {
        let rgba = [
            fallback_first[0],
            fallback_first[1],
            fallback_first[2],
            fallback_first[3],
            fallback_second[0],
            fallback_second[1],
            fallback_second[2],
            fallback_second[3],
        ];
        return create_rgba_texture(
            device,
            queue,
            label,
            2,
            1,
            &rgba,
            wgpu::TextureFormat::Rgba8Unorm,
        );
    };

    let width = u32::from(first.width);
    let height = u32::from(first.height);
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: width * 2,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let copy_layout = wgpu::TexelCopyBufferLayout {
        offset: 0,
        bytes_per_row: Some(width * 4),
        rows_per_image: Some(height),
    };
    let copy_size = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };
    for (origin_x, rgba) in [(0, first.rgba.as_slice()), (width, second.rgba.as_slice())] {
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: origin_x,
                    y: 0,
                    z: 0,
                },
                aspect: wgpu::TextureAspect::All,
            },
            rgba,
            copy_layout,
            copy_size,
        );
    }
    texture
}

fn create_rgba_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    label: &str,
    width: u32,
    height: u32,
    rgba: &[u8],
    format: wgpu::TextureFormat,
) -> wgpu::Texture {
    let expected_len = width as usize * height as usize * 4;
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    if rgba.len() >= expected_len {
        let copy_layout = if height == 1 {
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: None,
                rows_per_image: None,
            }
        } else {
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            }
        };
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &rgba[..expected_len],
            copy_layout,
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
    }
    texture
}

fn unorm_byte(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn srgb_byte(linear: f32) -> u8 {
    let value = linear.clamp(0.0, 1.0);
    let srgb = if value <= 0.003_130_8 {
        value * 12.92
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    };
    unorm_byte(srgb)
}

fn fallback_material() -> ModelMaterial {
    ModelMaterial {
        slot: 0,
        material_index: 0,
        name: "fallback".to_string(),
        path: None,
        shader_package_name: None,
        render_mode: MaterialRenderMode::Opaque,
        alpha_mode: MaterialAlphaMode::Opaque,
        alpha_threshold: 0.0,
        draw_depth_mode: MaterialDrawDepthMode::None,
        lighting_mode: MaterialLightingMode::Default,
        transparency: 0.0,
        alpha_aperture: 2.0,
        alpha_offset: 0.0,
        shadow_alpha_threshold: 0.5,
        glass_ior: 1.0,
        glass_thickness_max: 0.01,
        normal_scale: 1.0,
        multi_normal_scale: 1.0,
        detail_normal_scale: 1.0,
        multi_detail_normal_scale: 1.0,
        tile_index: 0.0,
        tile_alpha: 1.0,
        tile_scale: [16.0, 16.0],
        toon_index: 0.0,
        toon_light_scale: 2.0,
        toon_light_spec_aperture: 50.0,
        toon_reflection_scale: 2.5,
        toon_spec_index: 4.0e-45,
        sheen_rate: 0.0,
        sheen_tint_rate: 0.0,
        sheen_aperture: 1.0,
        sphere_map_index: 0.0,
        detail_id: 0.0,
        multi_detail_id: 0.0,
        detail_color: [0.5, 0.5, 0.5, 1.0],
        multi_detail_color: [0.5, 0.5, 0.5, 1.0],
        shader_diffuse_color: [1.0, 1.0, 1.0, 1.0],
        shader_multi_diffuse_color: [1.0, 1.0, 1.0, 1.0],
        shader_emissive_color: [0.0, 0.0, 0.0, 1.0],
        shader_multi_emissive_color: [0.0, 0.0, 0.0, 1.0],
        outline_color: [0.0, 0.0, 0.0, 1.0],
        outline_width: 0.0,
        specular_color_mask: [1.0, 1.0, 1.0, 1.0],
        ssao_mask: 1.0,
        texture_mip_bias: 0.0,
        shadow_pos_offset: 0.0,
        detail_color_uv_scale: [4.0, 4.0, 4.0, 4.0],
        detail_normal_uv_scale: [4.0, 4.0, 4.0, 4.0],
        uv_scroll: [0.0, 0.0, 0.0, 0.0],
        lightshaft_color: [1.0, 1.0, 1.0, 1.0],
        lightshaft_tex_anim: [0.0, 0.0, 0.0, 0.0],
        lightshaft_tex_u: [1.0, 0.0, 0.0, 0.0],
        lightshaft_tex_v: [0.0, 1.0, 0.0, 0.0],
        lightshaft_ray: [0.0, 0.0, 0.0, 0.0],
        opacity: 1.0,
        render_backfaces: true,
        apply_vertex_color: false,
        has_color_dye_table: false,
        color_dye_table: None,
        staining_application: None,
        texture_arrays: crate::ModelMaterialTextureArrays::default(),
        fallback_color: [0.78, 0.72, 0.64],
        diffuse_color: [0.78, 0.72, 0.64],
        specular_color: [0.35, 0.35, 0.35],
        emissive_color: [0.0, 0.0, 0.0],
        roughness: 0.55,
        metalness: 0.0,
        texture_indices: Vec::new(),
        base_color_texture: None,
        normal_texture: None,
        mask_texture: None,
        material_map_texture: None,
        multi_map_texture: None,
        specular_texture: None,
        emissive_texture: None,
        material_properties_texture: None,
        tile_properties_texture: None,
        sheen_properties_texture: None,
        sphere_properties_texture: None,
        tile_matrix_texture: None,
        index_texture: None,
    }
}

fn effective_mask_texture(material: &ModelMaterial) -> Option<usize> {
    material.mask_texture
}

fn material_extra_texture_flags<M: ModelRenderData + ?Sized>(
    material: &ModelMaterial,
    model: &M,
) -> [f32; 4] {
    [
        texture_presence_flag(model, material.tile_properties_texture),
        texture_presence_flag(model, material.sheen_properties_texture),
        texture_presence_flag(model, material.sphere_properties_texture),
        texture_presence_flag(model, material.tile_matrix_texture),
    ]
}

fn texture_presence_flag<M: ModelRenderData + ?Sized>(
    model: &M,
    texture_index: Option<usize>,
) -> f32 {
    texture_index
        .and_then(|index| model.textures().get(index))
        .map(|_| 1.0)
        .unwrap_or(0.0)
}

fn material_glass_params(material: &ModelMaterial) -> [f32; 4] {
    [
        finite_or(material.glass_ior, 1.0),
        finite_or(material.glass_thickness_max, 0.01),
        0.0,
        0.0,
    ]
}

fn material_alpha_params(material: &ModelMaterial) -> [f32; 4] {
    [
        finite_or(material.alpha_aperture, 2.0),
        finite_or(material.alpha_offset, 0.0),
        finite_or(material.shadow_alpha_threshold, 0.5).clamp(0.0, 1.0),
        finite_or(material.transparency, 0.0).clamp(0.0, 1.0),
    ]
}

fn material_alpha_policy_params(prepared_material: PreparedMaterial) -> [f32; 4] {
    let source = match prepared_material.alpha_policy.source {
        PreparedAlphaSource::Opaque => 0.0,
        PreparedAlphaSource::BaseColorAlpha => 1.0,
        PreparedAlphaSource::NormalBlue => 2.0,
    };
    let pass = match prepared_material.render_pass {
        PreparedRenderPass::Transparent => 1.0,
        PreparedRenderPass::Glass => 2.0,
        _ => 0.0,
    };
    [
        source,
        if prepared_material.alpha_policy.lighting_enabled {
            1.0
        } else {
            0.0
        },
        if matches!(
            prepared_material.alpha_policy.draw_depth_mode,
            MaterialDrawDepthMode::Dither
        ) {
            1.0
        } else {
            0.0
        },
        pass,
    ]
}

fn material_shader_params(material: &ModelMaterial) -> [f32; 4] {
    [
        material.normal_scale.clamp(0.0, 4.0),
        material.multi_normal_scale.clamp(0.0, 4.0),
        material.detail_normal_scale.clamp(0.0, 4.0),
        material.multi_detail_normal_scale.clamp(0.0, 4.0),
    ]
}

fn material_tile_params(material: &ModelMaterial) -> [f32; 4] {
    [
        finite_or(material.tile_index, 0.0),
        finite_or(material.tile_alpha, 1.0).clamp(0.0, 1.0),
        finite_or(material.tile_scale[0], 16.0),
        finite_or(material.tile_scale[1], 16.0),
    ]
}

fn material_toon_sheen_params(material: &ModelMaterial) -> [f32; 4] {
    [
        finite_or(material.toon_index, 0.0),
        finite_or(material.toon_light_scale, 2.0),
        finite_or(material.sheen_rate, 0.0),
        finite_or(material.sheen_tint_rate, 0.0),
    ]
}

fn material_toon_params(material: &ModelMaterial, prepared_material: PreparedMaterial) -> [f32; 4] {
    [
        finite_or(material.toon_light_spec_aperture, 50.0),
        finite_or(material.toon_reflection_scale, 2.5),
        finite_or(material.toon_spec_index, 4.0e-45),
        if prepared_material.feature_flags.uses_toon {
            1.0
        } else {
            0.0
        },
    ]
}

fn material_sheen_sphere_params(material: &ModelMaterial) -> [f32; 4] {
    [
        finite_or(material.sheen_aperture, 1.0),
        finite_or(material.sphere_map_index, 0.0),
        0.0,
        0.0,
    ]
}

fn material_detail_params(material: &ModelMaterial) -> [f32; 4] {
    [
        finite_or(material.detail_id, 0.0),
        finite_or(material.multi_detail_id, 0.0),
        0.0,
        0.0,
    ]
}

fn material_detail_color(material: &ModelMaterial) -> [f32; 4] {
    finite_vec4_or(material.detail_color, [0.5, 0.5, 0.5, 1.0])
}

fn material_multi_detail_color(material: &ModelMaterial) -> [f32; 4] {
    finite_vec4_or(material.multi_detail_color, [0.5, 0.5, 0.5, 1.0])
}

fn material_shader_diffuse_color(material: &ModelMaterial) -> [f32; 4] {
    finite_vec4_or(material.shader_diffuse_color, [1.0; 4])
}

fn material_shader_multi_diffuse_color(material: &ModelMaterial) -> [f32; 4] {
    finite_vec4_or(material.shader_multi_diffuse_color, [1.0; 4])
}

fn material_shader_emissive_color(material: &ModelMaterial) -> [f32; 4] {
    finite_vec4_or(material.shader_emissive_color, [0.0, 0.0, 0.0, 1.0])
}

fn material_shader_multi_emissive_color(material: &ModelMaterial) -> [f32; 4] {
    finite_vec4_or(material.shader_multi_emissive_color, [0.0, 0.0, 0.0, 1.0])
}

fn material_outline_params(material: &ModelMaterial) -> [f32; 4] {
    let color = finite_vec4_or(material.outline_color, [0.0, 0.0, 0.0, 1.0]);
    [
        color[0],
        color[1],
        color[2],
        finite_or(material.outline_width, 0.0),
    ]
}

fn material_specular_color_mask(material: &ModelMaterial) -> [f32; 4] {
    finite_vec4_or(material.specular_color_mask, [1.0; 4])
}

fn material_surface_params(material: &ModelMaterial) -> [f32; 4] {
    [
        finite_or(material.ssao_mask, 1.0),
        finite_or(material.texture_mip_bias, 0.0),
        finite_or(material.shadow_pos_offset, 0.0),
        0.0,
    ]
}

fn material_detail_color_uv_scale(material: &ModelMaterial) -> [f32; 4] {
    finite_vec4_or(material.detail_color_uv_scale, [4.0; 4])
}

fn material_detail_normal_uv_scale(material: &ModelMaterial) -> [f32; 4] {
    finite_vec4_or(material.detail_normal_uv_scale, [4.0; 4])
}

fn material_uv_scroll(material: &ModelMaterial) -> [f32; 4] {
    finite_vec4_or(material.uv_scroll, [0.0; 4])
}

fn material_lightshaft_color(material: &ModelMaterial) -> [f32; 4] {
    finite_vec4_or(material.lightshaft_color, [1.0; 4])
}

fn material_lightshaft_tex_anim(material: &ModelMaterial) -> [f32; 4] {
    finite_vec4_or(material.lightshaft_tex_anim, [0.0; 4])
}

fn material_lightshaft_tex_u(material: &ModelMaterial) -> [f32; 4] {
    finite_vec4_or(material.lightshaft_tex_u, [1.0, 0.0, 0.0, 0.0])
}

fn material_lightshaft_tex_v(material: &ModelMaterial) -> [f32; 4] {
    finite_vec4_or(material.lightshaft_tex_v, [0.0, 1.0, 0.0, 0.0])
}

fn material_lightshaft_ray(material: &ModelMaterial) -> [f32; 4] {
    finite_vec4_or(material.lightshaft_ray, [0.0; 4])
}

fn material_uv_source_params(
    prepared_material: PreparedMaterial,
) -> ([f32; 4], [f32; 4], [f32; 4], [f32; 4]) {
    let uv_sources = prepared_material.uv_sources.textures;
    (
        [
            prepared_uv_source_value(uv_sources.base_color),
            prepared_uv_source_value(uv_sources.normal),
            prepared_uv_source_value(uv_sources.mask),
            prepared_uv_source_value(uv_sources.material_map),
        ],
        [
            prepared_uv_source_value(uv_sources.multi_map),
            prepared_uv_source_value(uv_sources.specular),
            prepared_uv_source_value(uv_sources.emissive),
            prepared_uv_source_value(uv_sources.material_properties),
        ],
        [
            prepared_uv_source_value(uv_sources.tile_properties),
            prepared_uv_source_value(uv_sources.sheen_properties),
            prepared_uv_source_value(uv_sources.sphere_properties),
            prepared_uv_source_value(uv_sources.tile_matrix),
        ],
        [
            prepared_uv_source_value(uv_sources.index),
            prepared_uv_source_value(uv_sources.other),
            0.0,
            0.0,
        ],
    )
}

fn prepared_uv_source_value(source: PreparedUvSource) -> f32 {
    match source {
        PreparedUvSource::Uv0 => 0.0,
        PreparedUvSource::Uv1 => 1.0,
        PreparedUvSource::Uv2 => 2.0,
        PreparedUvSource::Uv3 => 3.0,
    }
}

fn draw_role_debug_color(draw_role: ModelMeshDrawRole) -> [f32; 4] {
    match draw_role {
        ModelMeshDrawRole::Normal => [0.16, 0.72, 1.0, 1.0],
        ModelMeshDrawRole::Glass => [0.66, 0.92, 1.0, 1.0],
        ModelMeshDrawRole::LightShaft => [1.0, 0.82, 0.22, 1.0],
        ModelMeshDrawRole::ShadowOnly => [0.18, 0.18, 0.22, 1.0],
        ModelMeshDrawRole::Ignored => [0.55, 0.55, 0.55, 1.0],
        ModelMeshDrawRole::MaterialChange => [1.0, 0.34, 0.76, 1.0],
        ModelMeshDrawRole::CrestChange => [1.0, 0.62, 0.2, 1.0],
    }
}

fn draw_role_params(draw_role: ModelMeshDrawRole) -> [f32; 4] {
    [
        if matches!(draw_role, ModelMeshDrawRole::LightShaft) {
            1.0
        } else {
            0.0
        },
        if matches!(draw_role, ModelMeshDrawRole::CrestChange) {
            1.0
        } else {
            0.0
        },
        if matches!(draw_role, ModelMeshDrawRole::MaterialChange) {
            1.0
        } else {
            0.0
        },
        0.0,
    ]
}

fn finite_vec4_or(values: [f32; 4], default: [f32; 4]) -> [f32; 4] {
    let mut resolved = default;
    for (target, value) in resolved.iter_mut().zip(values) {
        if value.is_finite() {
            *target = value;
        }
    }
    resolved
}

fn finite_or(value: f32, default: f32) -> f32 {
    if value.is_finite() { value } else { default }
}

fn render_mode_value(mode: MaterialRenderMode) -> f32 {
    match mode {
        MaterialRenderMode::Opaque => 0.0,
        MaterialRenderMode::Transparent => 1.0,
        MaterialRenderMode::Glass => 2.0,
    }
}

fn alpha_mode_value(mode: MaterialAlphaMode) -> f32 {
    match mode {
        MaterialAlphaMode::Opaque => 0.0,
        MaterialAlphaMode::Mask => 1.0,
        MaterialAlphaMode::Blend => 2.0,
        MaterialAlphaMode::Glass => 3.0,
    }
}

fn camera_uniform(
    center: [f32; 3],
    radius: f32,
    viewport: [u32; 2],
    yaw: f32,
    pitch: f32,
    zoom: f32,
    pan: [f32; 2],
    options: ModelRenderOptions,
) -> CameraUniform {
    let options = options.normalized();
    let aspect = if viewport[1] == 0 {
        1.0
    } else {
        viewport[0] as f32 / viewport[1] as f32
    };
    let radius = radius.max(0.1);
    let distance = radius * zoom.max(1.15);
    let pitch = pitch.clamp(-1.35, 1.35);
    let eye_offset = glam::Vec3::new(
        yaw.sin() * pitch.cos() * distance,
        pitch.sin() * distance,
        yaw.cos() * pitch.cos() * distance,
    );
    let view_dir = eye_offset.normalize_or_zero();
    let right = glam::Vec3::Y
        .cross(view_dir)
        .try_normalize()
        .unwrap_or(glam::Vec3::X);
    let up = view_dir
        .cross(right)
        .try_normalize()
        .unwrap_or(glam::Vec3::Y);
    let target = glam::Vec3::from(center) + (right * pan[0] + up * pan[1]) * radius;
    let eye = target + eye_offset;
    let view = glam::Mat4::look_at_rh(eye, target, glam::Vec3::Y);
    let projection = glam::Mat4::perspective_rh(
        45_f32.to_radians(),
        aspect.max(0.1),
        0.01,
        distance + radius * 6.0,
    );
    let light = glam::Vec3::new(-0.4, 0.7, 0.55).normalize();

    CameraUniform {
        view_proj: (projection * view).to_cols_array_2d(),
        light_dir: [light.x, light.y, light.z, 0.0],
        options: [
            if options.normal_mapping { 1.0 } else { 0.0 },
            options.normal_y_sign,
            options.uv_scroll_time,
            options.debug_mode.shader_value(),
        ],
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct CameraUniform {
    view_proj: [[f32; 4]; 4],
    light_dir: [f32; 4],
    options: [f32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct MaterialUniform {
    diffuse_color: [f32; 4],
    emissive_color: [f32; 4],
    specular_color: [f32; 4],
    params: [f32; 4],
    properties: [f32; 4],
    render: [f32; 4],
    alpha_params: [f32; 4],
    alpha_policy_params: [f32; 4],
    glass_params: [f32; 4],
    extra_properties: [f32; 4],
    shader_params: [f32; 4],
    tile_params: [f32; 4],
    toon_sheen_params: [f32; 4],
    toon_params: [f32; 4],
    sheen_sphere_params: [f32; 4],
    detail_params: [f32; 4],
    array_params: [f32; 4],
    detail_color: [f32; 4],
    multi_detail_color: [f32; 4],
    shader_diffuse_color: [f32; 4],
    shader_multi_diffuse_color: [f32; 4],
    shader_emissive_color: [f32; 4],
    shader_multi_emissive_color: [f32; 4],
    outline_params: [f32; 4],
    specular_color_mask: [f32; 4],
    surface_params: [f32; 4],
    detail_color_uv_scale: [f32; 4],
    detail_normal_uv_scale: [f32; 4],
    uv_scroll: [f32; 4],
    lightshaft_color: [f32; 4],
    lightshaft_tex_anim: [f32; 4],
    lightshaft_tex_u: [f32; 4],
    lightshaft_tex_v: [f32; 4],
    lightshaft_ray: [f32; 4],
    uv_sources0: [f32; 4],
    uv_sources1: [f32; 4],
    uv_sources2: [f32; 4],
    uv_sources3: [f32; 4],
    draw_role_params: [f32; 4],
    debug_color: [f32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct PostUniform {
    params: [f32; 4],
}

#[derive(Copy, Clone, Debug)]
struct DrawBatch {
    material_slot: usize,
    material_bind_group_index: usize,
    draw_role: ModelMeshDrawRole,
    index_start: u32,
    index_count: u32,
    prepared_material: PreparedMaterial,
    center: [f32; 3],
}

impl DrawBatch {
    fn pass(&self) -> PreparedRenderPass {
        self.prepared_material.render_pass
    }

    fn render_backfaces(&self) -> bool {
        self.prepared_material.render_backfaces
    }

    fn uses_dither_depth_prepass(&self) -> bool {
        self.pass().sorts_back_to_front()
            && matches!(
                self.prepared_material.alpha_policy.draw_depth_mode,
                MaterialDrawDepthMode::Dither
            )
    }

    fn uses_additive_glass_pipeline(&self, glass_blend_mode: ModelGlassBlendMode) -> bool {
        self.pass() == PreparedRenderPass::Glass
            && matches!(glass_blend_mode, ModelGlassBlendMode::Additive)
    }

    fn uses_outline_pass(&self) -> bool {
        self.prepared_material.feature_flags.uses_outline
            && matches!(
                self.draw_role,
                ModelMeshDrawRole::Normal
                    | ModelMeshDrawRole::Glass
                    | ModelMeshDrawRole::MaterialChange
            )
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct GpuVertex {
    position: [f32; 3],
    normal: [f32; 3],
    uv0: [f32; 2],
    bitangent: [f32; 4],
    color: [f32; 4],
    uv1: [f32; 2],
    uv2: [f32; 2],
    uv3: [f32; 2],
    color1: [f32; 4],
    normal1: [f32; 3],
    bitangent1: [f32; 4],
    flow0: [f32; 4],
    flow1: [f32; 4],
}

impl GpuVertex {
    const ATTRIBUTES: [wgpu::VertexAttribute; 13] = [
        wgpu::VertexAttribute {
            offset: std::mem::offset_of!(GpuVertex, position) as wgpu::BufferAddress,
            shader_location: 0,
            format: wgpu::VertexFormat::Float32x3,
        },
        wgpu::VertexAttribute {
            offset: std::mem::offset_of!(GpuVertex, normal) as wgpu::BufferAddress,
            shader_location: 1,
            format: wgpu::VertexFormat::Float32x3,
        },
        wgpu::VertexAttribute {
            offset: std::mem::offset_of!(GpuVertex, uv0) as wgpu::BufferAddress,
            shader_location: 2,
            format: wgpu::VertexFormat::Float32x2,
        },
        wgpu::VertexAttribute {
            offset: std::mem::offset_of!(GpuVertex, bitangent) as wgpu::BufferAddress,
            shader_location: 3,
            format: wgpu::VertexFormat::Float32x4,
        },
        wgpu::VertexAttribute {
            offset: std::mem::offset_of!(GpuVertex, color) as wgpu::BufferAddress,
            shader_location: 4,
            format: wgpu::VertexFormat::Float32x4,
        },
        wgpu::VertexAttribute {
            offset: std::mem::offset_of!(GpuVertex, uv1) as wgpu::BufferAddress,
            shader_location: 5,
            format: wgpu::VertexFormat::Float32x2,
        },
        wgpu::VertexAttribute {
            offset: std::mem::offset_of!(GpuVertex, uv2) as wgpu::BufferAddress,
            shader_location: 6,
            format: wgpu::VertexFormat::Float32x2,
        },
        wgpu::VertexAttribute {
            offset: std::mem::offset_of!(GpuVertex, uv3) as wgpu::BufferAddress,
            shader_location: 7,
            format: wgpu::VertexFormat::Float32x2,
        },
        wgpu::VertexAttribute {
            offset: std::mem::offset_of!(GpuVertex, color1) as wgpu::BufferAddress,
            shader_location: 8,
            format: wgpu::VertexFormat::Float32x4,
        },
        wgpu::VertexAttribute {
            offset: std::mem::offset_of!(GpuVertex, normal1) as wgpu::BufferAddress,
            shader_location: 9,
            format: wgpu::VertexFormat::Float32x3,
        },
        wgpu::VertexAttribute {
            offset: std::mem::offset_of!(GpuVertex, bitangent1) as wgpu::BufferAddress,
            shader_location: 10,
            format: wgpu::VertexFormat::Float32x4,
        },
        wgpu::VertexAttribute {
            offset: std::mem::offset_of!(GpuVertex, flow0) as wgpu::BufferAddress,
            shader_location: 11,
            format: wgpu::VertexFormat::Float32x4,
        },
        wgpu::VertexAttribute {
            offset: std::mem::offset_of!(GpuVertex, flow1) as wgpu::BufferAddress,
            shader_location: 12,
            format: wgpu::VertexFormat::Float32x4,
        },
    ];

    fn from_model_vertex(vertex: &crate::ModelVertex) -> Self {
        Self {
            position: vertex.position,
            normal: vertex.normal,
            uv0: vertex.uv0,
            bitangent: vertex.bitangent,
            color: vertex.color,
            uv1: vertex.uv1,
            uv2: vertex.uv2,
            uv3: vertex.uv3,
            color1: vertex.color1.unwrap_or([1.0, 1.0, 1.0, 1.0]),
            normal1: vertex.normal1.unwrap_or(vertex.normal),
            bitangent1: vertex.bitangent1.unwrap_or(vertex.bitangent),
            flow0: vertex.flow0.unwrap_or([0.0; 4]),
            flow1: vertex.flow1.unwrap_or([0.0; 4]),
        }
    }

    fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<GpuVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBUTES,
        }
    }
}

pub type WeaponRenderOptions = ModelRenderOptions;
pub type WeaponRenderer = ModelRenderer;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        MaterialShaderFamily, ModelMeshDrawRole, PreparedMaterialFeatureFlags,
        PreparedMaterialResourceAvailability, PreparedMaterialRuntimeFallbacks,
        PreparedMaterialUnsupportedInputs, PreparedMaterialUvSources, PreparedTextureAddressMode,
        PreparedTextureBindings, PreparedTextureColorSpace, PreparedTextureFilter,
        PreparedTextureSampling, PreparedTextureSamplingSet, PreparedTextureUvSources,
        PreparedUvSource, prepare_material_for_draw_role,
    };

    #[test]
    fn transparent_batches_sort_back_to_front_without_moving_opaque() {
        let batches = vec![
            test_batch(0, PreparedRenderPass::Opaque, [0.0, 0.0, 100.0]),
            test_batch(1, PreparedRenderPass::Transparent, [0.0, 0.0, -2.0]),
            test_batch(2, PreparedRenderPass::Glass, [0.0, 0.0, 3.0]),
            test_batch(3, PreparedRenderPass::Cutout, [0.0, 0.0, -100.0]),
            test_batch(4, PreparedRenderPass::AdditiveLightShaft, [0.0, 0.0, 200.0]),
        ];

        let sorted = sorted_transparent_batches(&batches, 0.0, 0.0);

        assert_eq!(
            sorted
                .iter()
                .map(|batch| batch.material_slot)
                .collect::<Vec<_>>(),
            vec![1, 2]
        );
    }

    #[test]
    fn prepared_material_pass_maps_alpha_modes_and_draw_roles() {
        assert_eq!(
            test_prepared_render_pass(
                MaterialAlphaMode::Opaque,
                MaterialRenderMode::Opaque,
                ModelMeshDrawRole::Normal
            ),
            PreparedRenderPass::Opaque
        );
        assert_eq!(
            test_prepared_render_pass(
                MaterialAlphaMode::Mask,
                MaterialRenderMode::Opaque,
                ModelMeshDrawRole::Normal
            ),
            PreparedRenderPass::Cutout
        );
        assert_eq!(
            test_prepared_render_pass(
                MaterialAlphaMode::Blend,
                MaterialRenderMode::Transparent,
                ModelMeshDrawRole::Normal
            ),
            PreparedRenderPass::Transparent
        );
        assert_eq!(
            test_prepared_render_pass(
                MaterialAlphaMode::Glass,
                MaterialRenderMode::Glass,
                ModelMeshDrawRole::Normal
            ),
            PreparedRenderPass::Glass
        );
        assert_eq!(
            test_prepared_render_pass(
                MaterialAlphaMode::Opaque,
                MaterialRenderMode::Transparent,
                ModelMeshDrawRole::Normal
            ),
            PreparedRenderPass::Transparent
        );
        assert_eq!(
            test_prepared_render_pass(
                MaterialAlphaMode::Opaque,
                MaterialRenderMode::Opaque,
                ModelMeshDrawRole::Glass
            ),
            PreparedRenderPass::Glass
        );
        assert_eq!(
            test_prepared_render_pass(
                MaterialAlphaMode::Opaque,
                MaterialRenderMode::Opaque,
                ModelMeshDrawRole::LightShaft
            ),
            PreparedRenderPass::AdditiveLightShaft
        );
        assert_eq!(
            test_prepared_render_pass(
                MaterialAlphaMode::Opaque,
                MaterialRenderMode::Opaque,
                ModelMeshDrawRole::CrestChange
            ),
            PreparedRenderPass::Transparent
        );
    }

    #[test]
    fn prepared_material_falls_back_for_missing_material_slot() {
        assert_eq!(
            prepare_material_for_draw_role(None, ModelMeshDrawRole::Normal),
            PreparedMaterial {
                render_pass: PreparedRenderPass::Opaque,
                shader_family: MaterialShaderFamily::Unknown,
                alpha_policy: crate::PreparedMaterialAlphaPolicy::default(),
                texture_bindings: PreparedTextureBindings::default(),
                texture_sampling: PreparedTextureSamplingSet::default(),
                uv_sources: PreparedMaterialUvSources::default(),
                feature_flags: PreparedMaterialFeatureFlags::default(),
                unsupported_inputs: PreparedMaterialUnsupportedInputs::default(),
                resource_availability: PreparedMaterialResourceAvailability::default(),
                runtime_fallbacks: PreparedMaterialRuntimeFallbacks::default(),
                render_backfaces: true,
            }
        );
    }

    #[test]
    fn prepared_render_pass_reports_pipeline_class() {
        assert!(PreparedRenderPass::Opaque.uses_opaque_pipeline());
        assert!(!PreparedRenderPass::Cutout.uses_opaque_pipeline());
        assert!(PreparedRenderPass::Cutout.uses_cutout_pipeline());
        assert!(!PreparedRenderPass::Opaque.sorts_back_to_front());
        assert!(!PreparedRenderPass::Cutout.sorts_back_to_front());
        assert!(PreparedRenderPass::Transparent.uses_transparent_pipeline());
        assert!(!PreparedRenderPass::Glass.uses_transparent_pipeline());
        assert!(PreparedRenderPass::Glass.uses_glass_pipeline());
        assert!(PreparedRenderPass::Transparent.sorts_back_to_front());
        assert!(PreparedRenderPass::Glass.sorts_back_to_front());
        assert!(PreparedRenderPass::AdditiveLightShaft.uses_additive_pipeline());
        assert!(!PreparedRenderPass::AdditiveLightShaft.uses_opaque_pipeline());
        assert!(!PreparedRenderPass::AdditiveLightShaft.uses_cutout_pipeline());
        assert!(!PreparedRenderPass::AdditiveLightShaft.uses_transparent_pipeline());
        assert!(!PreparedRenderPass::AdditiveLightShaft.uses_glass_pipeline());
        assert!(!PreparedRenderPass::AdditiveLightShaft.sorts_back_to_front());
    }

    #[test]
    fn model_pipeline_blend_modes_report_blend_and_depth_policy() {
        assert_eq!(ModelPipelineBlend::Opaque.blend_state(), None);
        assert!(ModelPipelineBlend::Opaque.writes_depth());
        assert_eq!(ModelPipelineBlend::Opaque.fragment_entry(), "fs_main");
        assert_eq!(
            ModelPipelineBlend::Opaque.color_write_mask(),
            wgpu::ColorWrites::ALL
        );

        assert_eq!(ModelPipelineBlend::DitherDepth.blend_state(), None);
        assert!(ModelPipelineBlend::DitherDepth.writes_depth());
        assert_eq!(
            ModelPipelineBlend::DitherDepth.fragment_entry(),
            "fs_dither_depth"
        );
        assert_eq!(
            ModelPipelineBlend::DitherDepth.color_write_mask(),
            wgpu::ColorWrites::empty()
        );
        assert_eq!(
            ModelPipelineBlend::Alpha.blend_state(),
            Some(wgpu::BlendState::ALPHA_BLENDING)
        );
        assert!(!ModelPipelineBlend::Alpha.writes_depth());

        let additive = ModelPipelineBlend::Additive
            .blend_state()
            .expect("additive blend");
        assert_eq!(additive.color.src_factor, wgpu::BlendFactor::One);
        assert_eq!(additive.color.dst_factor, wgpu::BlendFactor::One);
        assert_eq!(additive.color.operation, wgpu::BlendOperation::Add);
        assert_eq!(additive.alpha.src_factor, wgpu::BlendFactor::One);
        assert_eq!(additive.alpha.dst_factor, wgpu::BlendFactor::One);
        assert!(!ModelPipelineBlend::Additive.writes_depth());
    }

    #[test]
    fn dither_depth_prepass_only_selects_dithered_transparent_batches() {
        let mut glass = test_batch(0, PreparedRenderPass::Glass, [0.0; 3]);
        glass.prepared_material.alpha_policy.draw_depth_mode = MaterialDrawDepthMode::Dither;
        assert!(glass.uses_dither_depth_prepass());

        let mut transparent = test_batch(1, PreparedRenderPass::Transparent, [0.0; 3]);
        assert!(!transparent.uses_dither_depth_prepass());
        transparent.prepared_material.alpha_policy.draw_depth_mode = MaterialDrawDepthMode::Dither;
        assert!(transparent.uses_dither_depth_prepass());

        let mut opaque = test_batch(2, PreparedRenderPass::Opaque, [0.0; 3]);
        opaque.prepared_material.alpha_policy.draw_depth_mode = MaterialDrawDepthMode::Dither;
        assert!(!opaque.uses_dither_depth_prepass());
    }

    #[test]
    fn glass_blend_mode_only_switches_glass_batches_to_additive() {
        assert_eq!(
            ModelRenderOptions::default().glass_blend_mode,
            ModelGlassBlendMode::Multiply
        );

        let glass = test_batch(0, PreparedRenderPass::Glass, [0.0; 3]);
        assert!(!glass.uses_additive_glass_pipeline(ModelGlassBlendMode::Multiply));
        assert!(glass.uses_additive_glass_pipeline(ModelGlassBlendMode::Additive));

        let transparent = test_batch(1, PreparedRenderPass::Transparent, [0.0; 3]);
        assert!(!transparent.uses_additive_glass_pipeline(ModelGlassBlendMode::Additive));
    }

    #[test]
    fn outline_pass_only_selects_eligible_surface_batches() {
        let mut normal = test_batch(0, PreparedRenderPass::Opaque, [0.0; 3]);
        normal.prepared_material.feature_flags.uses_outline = true;
        assert!(normal.uses_outline_pass());

        let mut lightshaft = test_batch(1, PreparedRenderPass::AdditiveLightShaft, [0.0; 3]);
        lightshaft.prepared_material.feature_flags.uses_outline = true;
        lightshaft.draw_role = ModelMeshDrawRole::LightShaft;
        assert!(!lightshaft.uses_outline_pass());

        let mut crest = test_batch(2, PreparedRenderPass::Transparent, [0.0; 3]);
        crest.prepared_material.feature_flags.uses_outline = true;
        crest.draw_role = ModelMeshDrawRole::CrestChange;
        assert!(!crest.uses_outline_pass());
    }

    #[test]
    fn camera_uniform_preserves_finite_uv_scroll_time() {
        let mut options = ModelRenderOptions {
            uv_scroll_time: 12.5,
            debug_mode: ModelDebugMode::Mask,
            ..ModelRenderOptions::default()
        };
        let uniform = camera_uniform([0.0; 3], 1.0, [128, 64], 0.0, 0.0, 2.0, [0.0; 2], options);
        assert_eq!(uniform.options[2], 12.5);
        assert_eq!(uniform.options[3], 3.0);

        options.uv_scroll_time = f32::NAN;
        let uniform = camera_uniform([0.0; 3], 1.0, [128, 64], 0.0, 0.0, 2.0, [0.0; 2], options);
        assert_eq!(uniform.options[2], 0.0);
        assert_eq!(uniform.options[3], 3.0);
    }

    #[test]
    fn model_debug_modes_have_stable_shader_values() {
        assert_eq!(ModelDebugMode::Final.shader_value(), 0.0);
        assert_eq!(ModelDebugMode::BaseColor.shader_value(), 1.0);
        assert_eq!(ModelDebugMode::Normal.shader_value(), 2.0);
        assert_eq!(ModelDebugMode::Mask.shader_value(), 3.0);
        assert_eq!(ModelDebugMode::MaterialProperties.shader_value(), 4.0);
        assert_eq!(ModelDebugMode::Specular.shader_value(), 5.0);
        assert_eq!(ModelDebugMode::Emissive.shader_value(), 6.0);
        assert_eq!(ModelDebugMode::Alpha.shader_value(), 7.0);
        assert_eq!(ModelDebugMode::Uv0.shader_value(), 8.0);
        assert_eq!(ModelDebugMode::Uv1.shader_value(), 9.0);
        assert_eq!(ModelDebugMode::Uv2.shader_value(), 10.0);
        assert_eq!(ModelDebugMode::Uv3.shader_value(), 11.0);
        assert_eq!(ModelDebugMode::VertexColor.shader_value(), 12.0);
        assert_eq!(ModelDebugMode::MeshRole.shader_value(), 13.0);
        assert_eq!(ModelDebugMode::ColorTableIndex.shader_value(), 14.0);
        assert_eq!(ModelDebugMode::MaterialMap.shader_value(), 15.0);
        assert_eq!(ModelDebugMode::MultiMap.shader_value(), 16.0);
        assert_eq!(ModelDebugMode::TileProperties.shader_value(), 17.0);
        assert_eq!(ModelDebugMode::SheenProperties.shader_value(), 18.0);
        assert_eq!(ModelDebugMode::SphereProperties.shader_value(), 19.0);
        assert_eq!(ModelDebugMode::TileMatrix.shader_value(), 20.0);
        assert_eq!(ModelDebugMode::TileNormalArray.shader_value(), 21.0);
        assert_eq!(ModelDebugMode::TileOrbArray.shader_value(), 22.0);
        assert_eq!(ModelDebugMode::DetailDiffuseArray.shader_value(), 23.0);
        assert_eq!(ModelDebugMode::DetailNormalArray.shader_value(), 24.0);
    }

    #[test]
    fn material_sampler_groups_use_prepared_sampling_roles() {
        let color_sampling = test_sampling(
            PreparedTextureColorSpace::Srgb,
            PreparedTextureFilter::Linear,
            PreparedTextureAddressMode::Repeat,
        );
        let data_sampling = test_sampling(
            PreparedTextureColorSpace::NonColor,
            PreparedTextureFilter::Linear,
            PreparedTextureAddressMode::Clip,
        );
        let nearest_sampling = test_sampling(
            PreparedTextureColorSpace::NonColor,
            PreparedTextureFilter::Nearest,
            PreparedTextureAddressMode::Repeat,
        );
        let prepared = PreparedMaterial {
            render_pass: PreparedRenderPass::Opaque,
            shader_family: MaterialShaderFamily::Character,
            alpha_policy: crate::PreparedMaterialAlphaPolicy::default(),
            texture_bindings: PreparedTextureBindings::default(),
            texture_sampling: PreparedTextureSamplingSet {
                base_color: color_sampling,
                normal: data_sampling,
                index: nearest_sampling,
                ..PreparedTextureSamplingSet::default()
            },
            uv_sources: PreparedMaterialUvSources::default(),
            feature_flags: PreparedMaterialFeatureFlags::default(),
            unsupported_inputs: PreparedMaterialUnsupportedInputs::default(),
            resource_availability: PreparedMaterialResourceAvailability::default(),
            runtime_fallbacks: PreparedMaterialRuntimeFallbacks::default(),
            render_backfaces: true,
        };

        assert_eq!(material_color_sampler_policy(prepared), color_sampling);
        assert_eq!(material_data_sampler_policy(prepared), data_sampling);
        assert_eq!(material_nearest_sampler_policy(prepared), nearest_sampling);
    }

    #[test]
    fn sampler_descriptor_follows_prepared_filter_and_address_policy() {
        let linear_repeat = sampler_descriptor_for_sampling(
            "linear repeat",
            test_sampling(
                PreparedTextureColorSpace::Srgb,
                PreparedTextureFilter::Linear,
                PreparedTextureAddressMode::Repeat,
            ),
        );
        assert_eq!(linear_repeat.mag_filter, wgpu::FilterMode::Linear);
        assert_eq!(linear_repeat.min_filter, wgpu::FilterMode::Linear);
        assert_eq!(linear_repeat.address_mode_u, wgpu::AddressMode::Repeat);
        assert_eq!(linear_repeat.address_mode_v, wgpu::AddressMode::Repeat);

        let nearest_clip = sampler_descriptor_for_sampling(
            "nearest clip",
            test_sampling(
                PreparedTextureColorSpace::NonColor,
                PreparedTextureFilter::Nearest,
                PreparedTextureAddressMode::Clip,
            ),
        );
        assert_eq!(nearest_clip.mag_filter, wgpu::FilterMode::Nearest);
        assert_eq!(nearest_clip.min_filter, wgpu::FilterMode::Nearest);
        assert_eq!(nearest_clip.address_mode_u, wgpu::AddressMode::ClampToEdge);
        assert_eq!(nearest_clip.address_mode_v, wgpu::AddressMode::ClampToEdge);
    }

    #[test]
    fn draw_role_debug_colors_distinguish_visible_roles() {
        assert_eq!(
            draw_role_debug_color(ModelMeshDrawRole::Normal),
            [0.16, 0.72, 1.0, 1.0]
        );
        assert_eq!(
            draw_role_debug_color(ModelMeshDrawRole::Glass),
            [0.66, 0.92, 1.0, 1.0]
        );
        assert_eq!(
            draw_role_debug_color(ModelMeshDrawRole::MaterialChange),
            [1.0, 0.34, 0.76, 1.0]
        );
        assert_eq!(
            draw_role_debug_color(ModelMeshDrawRole::CrestChange),
            [1.0, 0.62, 0.2, 1.0]
        );
    }

    #[test]
    fn prepared_material_preserves_culling_policy() {
        let mut material = fallback_material();
        material.render_backfaces = false;
        let model = crate::ModelData {
            bounds: crate::ModelBounds::default(),
            materials: vec![material],
            textures: Vec::new(),
            meshes: Vec::new(),
        };

        assert_eq!(
            prepare_material_for_draw_role(model.materials().first(), ModelMeshDrawRole::Normal),
            PreparedMaterial {
                render_pass: PreparedRenderPass::Opaque,
                shader_family: MaterialShaderFamily::Unknown,
                alpha_policy: crate::PreparedMaterialAlphaPolicy::default(),
                texture_bindings: PreparedTextureBindings::default(),
                texture_sampling: PreparedTextureSamplingSet::default(),
                uv_sources: PreparedMaterialUvSources::default(),
                feature_flags: PreparedMaterialFeatureFlags::default(),
                unsupported_inputs: PreparedMaterialUnsupportedInputs::default(),
                resource_availability: PreparedMaterialResourceAvailability::default(),
                runtime_fallbacks: PreparedMaterialRuntimeFallbacks::default(),
                render_backfaces: false,
            }
        );
    }

    #[test]
    fn prepared_render_pass_uses_source_order_for_render_mode_fallbacks() {
        assert_eq!(
            test_prepared_render_pass(
                MaterialAlphaMode::Opaque,
                MaterialRenderMode::Glass,
                ModelMeshDrawRole::Normal
            ),
            PreparedRenderPass::Glass
        );
        assert_eq!(
            test_prepared_render_pass(
                MaterialAlphaMode::Opaque,
                MaterialRenderMode::Transparent,
                ModelMeshDrawRole::Normal
            ),
            PreparedRenderPass::Transparent
        );
        assert_eq!(
            test_prepared_render_pass(
                MaterialAlphaMode::Opaque,
                MaterialRenderMode::Opaque,
                ModelMeshDrawRole::Normal
            ),
            PreparedRenderPass::Opaque
        );
    }

    #[test]
    fn effective_mask_texture_uses_only_explicit_mask_sampler() {
        let mut material = fallback_material();
        material.material_map_texture = Some(2);
        material.multi_map_texture = Some(3);

        assert_eq!(effective_mask_texture(&material), None);

        material.mask_texture = Some(1);
        assert_eq!(effective_mask_texture(&material), Some(1));
    }

    #[test]
    fn material_extra_texture_flags_require_loaded_textures() {
        let mut material = fallback_material();
        material.tile_properties_texture = Some(0);
        material.sheen_properties_texture = Some(1);
        material.sphere_properties_texture = Some(99);
        material.tile_matrix_texture = Some(2);
        let model = crate::ModelData {
            bounds: crate::ModelBounds::default(),
            materials: vec![material.clone()],
            textures: vec![
                test_texture(crate::ModelTextureKind::TileProperties),
                test_texture(crate::ModelTextureKind::SheenProperties),
                test_texture(crate::ModelTextureKind::TileMatrixProperties),
            ],
            meshes: Vec::new(),
        };

        assert_eq!(
            material_extra_texture_flags(&material, &model),
            [1.0, 1.0, 0.0, 1.0]
        );
    }

    #[test]
    fn material_array_params_require_compatible_texture_pairs() {
        let mut material = fallback_material();
        material.texture_arrays.tile_normal = Some(0);
        material.texture_arrays.tile_orb = Some(1);
        material.texture_arrays.detail_diffuse = Some(2);
        material.texture_arrays.detail_normal = Some(3);
        let mut textures = vec![
            test_array_texture(crate::ModelTextureKind::TileNormalArray, 4),
            test_array_texture(crate::ModelTextureKind::TileOrbArray, 4),
            test_array_texture(crate::ModelTextureKind::DetailDiffuseArray, 8),
            test_array_texture(crate::ModelTextureKind::DetailNormalArray, 8),
        ];
        let mut model = crate::ModelData {
            bounds: crate::ModelBounds::default(),
            materials: vec![material.clone()],
            textures: textures.clone(),
            meshes: Vec::new(),
        };

        assert_eq!(
            material_array_params(&material, &model),
            [4.0, 8.0, 1.0, 1.0]
        );

        model.textures[0].kind = crate::ModelTextureKind::BaseColor;
        assert_eq!(
            material_array_params(&material, &model),
            [1.0, 8.0, 0.0, 1.0]
        );
        model.textures[0].kind = crate::ModelTextureKind::TileNormalArray;

        let duplicate_tile_normal = model.textures[0].clone();
        let duplicate_tile_orb = model.textures[1].clone();
        model
            .textures
            .extend([duplicate_tile_normal, duplicate_tile_orb]);
        material.texture_arrays.tile_normal = Some(4);
        material.texture_arrays.tile_orb = Some(5);
        assert_eq!(
            material_array_params(&material, &model),
            [1.0, 8.0, 0.0, 1.0]
        );
        material.texture_arrays.tile_normal = Some(0);
        material.texture_arrays.tile_orb = Some(1);

        textures[1].array_size = 3;
        textures[1].height = 3;
        textures[1].rgba.resize(12, 255);
        model.textures = textures;
        assert_eq!(
            material_array_params(&material, &model),
            [1.0, 8.0, 0.0, 1.0]
        );
    }

    #[test]
    fn material_shader_params_clamp_normal_scales() {
        let mut material = fallback_material();
        assert_eq!(material_shader_params(&material), [1.0, 1.0, 1.0, 1.0]);

        material.normal_scale = 2.25;
        material.multi_normal_scale = 0.5;
        material.detail_normal_scale = 3.5;
        material.multi_detail_normal_scale = f32::INFINITY;
        assert_eq!(material_shader_params(&material), [2.25, 0.5, 3.5, 4.0]);

        material.normal_scale = 8.0;
        material.multi_normal_scale = -1.0;
        material.detail_normal_scale = 8.0;
        material.multi_detail_normal_scale = 0.25;
        assert_eq!(material_shader_params(&material), [4.0, 0.0, 4.0, 0.25]);
    }

    #[test]
    fn material_tile_params_preserve_tile_select_values() {
        let mut material = fallback_material();
        assert_eq!(material_tile_params(&material), [0.0, 1.0, 16.0, 16.0]);

        material.tile_index = 7.0;
        material.tile_alpha = 0.35;
        material.tile_scale = [24.0, 12.0];
        assert_eq!(material_tile_params(&material), [7.0, 0.35, 24.0, 12.0]);

        material.tile_index = f32::INFINITY;
        material.tile_alpha = 8.0;
        material.tile_scale = [f32::NAN, f32::NEG_INFINITY];
        assert_eq!(material_tile_params(&material), [0.0, 1.0, 16.0, 16.0]);
    }

    #[test]
    fn material_toon_sheen_sphere_params_preserve_shader_inputs() {
        let mut material = fallback_material();
        let prepared = prepare_material_for_draw_role(Some(&material), ModelMeshDrawRole::Normal);
        assert_eq!(material_toon_sheen_params(&material), [0.0, 2.0, 0.0, 0.0]);
        assert_eq!(
            material_toon_params(&material, prepared),
            [50.0, 2.5, 4.0e-45, 0.0]
        );
        assert_eq!(
            material_sheen_sphere_params(&material),
            [1.0, 0.0, 0.0, 0.0]
        );

        material.toon_index = 5.0;
        material.toon_light_scale = 1.5;
        material.toon_light_spec_aperture = 64.0;
        material.toon_reflection_scale = 3.5;
        material.toon_spec_index = 2.0;
        material.sheen_rate = 0.25;
        material.sheen_tint_rate = 0.35;
        material.sheen_aperture = 0.8;
        material.sphere_map_index = 3.0;
        assert_eq!(
            material_toon_sheen_params(&material),
            [5.0, 1.5, 0.25, 0.35]
        );
        material.shader_package_name = Some("character.shpk".to_string());
        let prepared = prepare_material_for_draw_role(Some(&material), ModelMeshDrawRole::Normal);
        assert_eq!(
            material_toon_params(&material, prepared),
            [64.0, 3.5, 2.0, 1.0]
        );
        assert_eq!(
            material_sheen_sphere_params(&material),
            [0.8, 3.0, 0.0, 0.0]
        );

        material.toon_index = f32::NAN;
        material.toon_light_scale = f32::INFINITY;
        material.toon_light_spec_aperture = f32::NAN;
        material.toon_reflection_scale = f32::INFINITY;
        material.toon_spec_index = f32::NEG_INFINITY;
        material.sheen_rate = f32::NEG_INFINITY;
        material.sheen_tint_rate = f32::NAN;
        material.sheen_aperture = f32::INFINITY;
        material.sphere_map_index = f32::NAN;
        assert_eq!(material_toon_sheen_params(&material), [0.0, 2.0, 0.0, 0.0]);
        let prepared = prepare_material_for_draw_role(Some(&material), ModelMeshDrawRole::Normal);
        assert_eq!(
            material_toon_params(&material, prepared),
            [50.0, 2.5, 4.0e-45, 1.0]
        );
        assert_eq!(
            material_sheen_sphere_params(&material),
            [1.0, 0.0, 0.0, 0.0]
        );
    }

    #[test]
    fn material_alpha_params_preserve_shader_inputs() {
        let mut material = fallback_material();
        assert_eq!(material_alpha_params(&material), [2.0, 0.0, 0.5, 0.0]);

        material.alpha_aperture = 2.5;
        material.alpha_offset = -0.2;
        material.shadow_alpha_threshold = 0.35;
        material.transparency = 0.6;
        assert_eq!(material_alpha_params(&material), [2.5, -0.2, 0.35, 0.6]);

        material.alpha_aperture = f32::NAN;
        material.alpha_offset = f32::INFINITY;
        material.shadow_alpha_threshold = 4.0;
        material.transparency = -1.0;
        assert_eq!(material_alpha_params(&material), [2.0, 0.0, 1.0, 0.0]);
    }

    #[test]
    fn material_alpha_policy_params_encode_prepared_shader_policy() {
        let mut prepared = PreparedMaterial {
            render_pass: PreparedRenderPass::Transparent,
            shader_family: MaterialShaderFamily::CharacterTransparency,
            alpha_policy: crate::PreparedMaterialAlphaPolicy {
                source: PreparedAlphaSource::NormalBlue,
                draw_depth_mode: MaterialDrawDepthMode::Dither,
                lighting_enabled: false,
            },
            texture_bindings: PreparedTextureBindings::default(),
            texture_sampling: PreparedTextureSamplingSet::default(),
            uv_sources: PreparedMaterialUvSources::default(),
            feature_flags: PreparedMaterialFeatureFlags::default(),
            unsupported_inputs: PreparedMaterialUnsupportedInputs::default(),
            resource_availability: PreparedMaterialResourceAvailability::default(),
            runtime_fallbacks: PreparedMaterialRuntimeFallbacks::default(),
            render_backfaces: true,
        };
        assert_eq!(material_alpha_policy_params(prepared), [2.0, 0.0, 1.0, 1.0]);

        prepared.render_pass = PreparedRenderPass::Glass;
        prepared.alpha_policy.source = PreparedAlphaSource::BaseColorAlpha;
        prepared.alpha_policy.draw_depth_mode = MaterialDrawDepthMode::None;
        prepared.alpha_policy.lighting_enabled = true;
        assert_eq!(material_alpha_policy_params(prepared), [1.0, 1.0, 0.0, 2.0]);
    }

    #[test]
    fn material_glass_params_preserve_shader_inputs() {
        let mut material = fallback_material();
        assert_eq!(material_glass_params(&material), [1.0, 0.01, 0.0, 0.0]);

        material.glass_ior = 1.52;
        material.glass_thickness_max = 0.125;
        assert_eq!(material_glass_params(&material), [1.52, 0.125, 0.0, 0.0]);

        material.glass_ior = f32::NAN;
        material.glass_thickness_max = f32::INFINITY;
        assert_eq!(material_glass_params(&material), [1.0, 0.01, 0.0, 0.0]);
    }

    #[test]
    fn material_detail_params_preserve_detail_uv_values() {
        let mut material = fallback_material();
        assert_eq!(material_detail_params(&material), [0.0, 0.0, 0.0, 0.0]);
        assert_eq!(material_detail_color(&material), [0.5, 0.5, 0.5, 1.0]);
        assert_eq!(material_multi_detail_color(&material), [0.5, 0.5, 0.5, 1.0]);
        assert_eq!(material_detail_color_uv_scale(&material), [4.0; 4]);
        assert_eq!(material_detail_normal_uv_scale(&material), [4.0; 4]);

        material.detail_id = 3.0;
        material.multi_detail_id = 5.0;
        material.detail_color = [0.2, 0.4, 0.6, 0.8];
        material.multi_detail_color = [0.1, 0.3, 0.5, 0.7];
        material.detail_color_uv_scale = [8.0, 6.0, 4.0, 2.0];
        material.detail_normal_uv_scale = [7.0, 5.0, 3.0, 1.0];
        assert_eq!(material_detail_params(&material), [3.0, 5.0, 0.0, 0.0]);
        assert_eq!(material_detail_color(&material), [0.2, 0.4, 0.6, 0.8]);
        assert_eq!(material_multi_detail_color(&material), [0.1, 0.3, 0.5, 0.7]);
        assert_eq!(
            material_detail_color_uv_scale(&material),
            [8.0, 6.0, 4.0, 2.0]
        );
        assert_eq!(
            material_detail_normal_uv_scale(&material),
            [7.0, 5.0, 3.0, 1.0]
        );

        material.detail_id = f32::NAN;
        material.multi_detail_id = f32::INFINITY;
        material.detail_color = [0.25, f32::NAN, f32::INFINITY, 0.5];
        material.multi_detail_color = [f32::NEG_INFINITY, 0.3, 0.5, f32::NAN];
        material.detail_color_uv_scale = [1.0, f32::NAN, f32::INFINITY, 2.0];
        material.detail_normal_uv_scale = [f32::NEG_INFINITY, 3.0, 4.0, f32::NAN];
        assert_eq!(material_detail_params(&material), [0.0, 0.0, 0.0, 0.0]);
        assert_eq!(material_detail_color(&material), [0.25, 0.5, 0.5, 0.5]);
        assert_eq!(material_multi_detail_color(&material), [0.5, 0.3, 0.5, 1.0]);
        assert_eq!(
            material_detail_color_uv_scale(&material),
            [1.0, 4.0, 4.0, 2.0]
        );
        assert_eq!(
            material_detail_normal_uv_scale(&material),
            [4.0, 3.0, 4.0, 4.0]
        );
    }

    #[test]
    fn material_shader_colors_preserve_material_constants() {
        let mut material = fallback_material();
        assert_eq!(material_shader_diffuse_color(&material), [1.0; 4]);
        assert_eq!(material_shader_multi_diffuse_color(&material), [1.0; 4]);
        assert_eq!(
            material_shader_emissive_color(&material),
            [0.0, 0.0, 0.0, 1.0]
        );
        assert_eq!(
            material_shader_multi_emissive_color(&material),
            [0.0, 0.0, 0.0, 1.0]
        );

        material.shader_diffuse_color = [0.8, 0.7, 0.6, 0.5];
        material.shader_multi_diffuse_color = [0.6, 0.7, 0.8, 0.9];
        material.shader_emissive_color = [0.1, 0.2, 0.3, 1.0];
        material.shader_multi_emissive_color = [0.4, 0.5, 0.6, 1.0];
        assert_eq!(
            material_shader_diffuse_color(&material),
            [0.8, 0.7, 0.6, 0.5]
        );
        assert_eq!(
            material_shader_multi_diffuse_color(&material),
            [0.6, 0.7, 0.8, 0.9]
        );
        assert_eq!(
            material_shader_emissive_color(&material),
            [0.1, 0.2, 0.3, 1.0]
        );
        assert_eq!(
            material_shader_multi_emissive_color(&material),
            [0.4, 0.5, 0.6, 1.0]
        );

        material.shader_diffuse_color = [0.25, f32::NAN, f32::INFINITY, 0.5];
        material.shader_emissive_color = [f32::NEG_INFINITY, 0.2, 0.3, f32::NAN];
        assert_eq!(
            material_shader_diffuse_color(&material),
            [0.25, 1.0, 1.0, 0.5]
        );
        assert_eq!(
            material_shader_emissive_color(&material),
            [0.0, 0.2, 0.3, 1.0]
        );
    }

    #[test]
    fn material_outline_specular_surface_params_preserve_shader_inputs() {
        let mut material = fallback_material();
        assert_eq!(material_outline_params(&material), [0.0, 0.0, 0.0, 0.0]);
        assert_eq!(material_specular_color_mask(&material), [1.0; 4]);
        assert_eq!(material_surface_params(&material), [1.0, 0.0, 0.0, 0.0]);

        material.outline_color = [0.1, 0.2, 0.3, 0.4];
        material.outline_width = 0.05;
        material.specular_color_mask = [0.7, 0.8, 0.9, 1.0];
        material.ssao_mask = 0.6;
        material.texture_mip_bias = -0.75;
        material.shadow_pos_offset = 0.125;
        assert_eq!(material_outline_params(&material), [0.1, 0.2, 0.3, 0.05]);
        assert_eq!(
            material_specular_color_mask(&material),
            [0.7, 0.8, 0.9, 1.0]
        );
        assert_eq!(material_surface_params(&material), [0.6, -0.75, 0.125, 0.0]);

        material.outline_color = [0.25, f32::NAN, f32::INFINITY, 0.5];
        material.outline_width = f32::NEG_INFINITY;
        material.specular_color_mask = [f32::NAN, 0.3, f32::INFINITY, 0.5];
        material.ssao_mask = f32::INFINITY;
        material.texture_mip_bias = f32::NAN;
        material.shadow_pos_offset = f32::NEG_INFINITY;
        assert_eq!(material_outline_params(&material), [0.25, 0.0, 0.0, 0.0]);
        assert_eq!(
            material_specular_color_mask(&material),
            [1.0, 0.3, 1.0, 0.5]
        );
        assert_eq!(material_surface_params(&material), [1.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn material_uv_scroll_preserves_scroll_multipliers() {
        let mut material = fallback_material();
        assert_eq!(material_uv_scroll(&material), [0.0; 4]);

        material.uv_scroll = [-10.0, 20.0, -30.0, 40.0];
        assert_eq!(material_uv_scroll(&material), [-10.0, 20.0, -30.0, 40.0]);

        material.uv_scroll = [1.0, f32::NAN, f32::INFINITY, 2.0];
        assert_eq!(material_uv_scroll(&material), [1.0, 0.0, 0.0, 2.0]);
    }

    #[test]
    fn material_lightshaft_params_preserve_shader_inputs() {
        let mut material = fallback_material();
        assert_eq!(material_lightshaft_color(&material), [1.0; 4]);
        assert_eq!(material_lightshaft_tex_anim(&material), [0.0; 4]);
        assert_eq!(material_lightshaft_tex_u(&material), [1.0, 0.0, 0.0, 0.0]);
        assert_eq!(material_lightshaft_tex_v(&material), [0.0, 1.0, 0.0, 0.0]);
        assert_eq!(material_lightshaft_ray(&material), [0.0; 4]);
        assert_eq!(draw_role_params(ModelMeshDrawRole::Normal), [0.0; 4]);
        assert_eq!(
            draw_role_params(ModelMeshDrawRole::LightShaft),
            [1.0, 0.0, 0.0, 0.0]
        );
        assert_eq!(
            draw_role_params(ModelMeshDrawRole::CrestChange),
            [0.0, 1.0, 0.0, 0.0]
        );
        assert_eq!(
            draw_role_params(ModelMeshDrawRole::MaterialChange),
            [0.0, 0.0, 1.0, 0.0]
        );

        material.lightshaft_color = [0.2, 0.4, 0.6, 0.8];
        material.lightshaft_tex_anim = [0.1, 0.2, 0.3, 0.4];
        material.lightshaft_tex_u = [1.5, 0.5, 0.25, 0.0];
        material.lightshaft_tex_v = [0.25, 1.75, 0.5, 0.0];
        material.lightshaft_ray = [2.0, 3.0, 4.0, 5.0];
        assert_eq!(material_lightshaft_color(&material), [0.2, 0.4, 0.6, 0.8]);
        assert_eq!(
            material_lightshaft_tex_anim(&material),
            [0.1, 0.2, 0.3, 0.4]
        );
        assert_eq!(material_lightshaft_tex_u(&material), [1.5, 0.5, 0.25, 0.0]);
        assert_eq!(material_lightshaft_tex_v(&material), [0.25, 1.75, 0.5, 0.0]);
        assert_eq!(material_lightshaft_ray(&material), [2.0, 3.0, 4.0, 5.0]);

        material.lightshaft_color = [0.2, f32::NAN, f32::INFINITY, 0.8];
        material.lightshaft_tex_u = [f32::NAN, 0.5, f32::INFINITY, 0.0];
        assert_eq!(material_lightshaft_color(&material), [0.2, 1.0, 1.0, 0.8]);
        assert_eq!(material_lightshaft_tex_u(&material), [1.0, 0.5, 0.0, 0.0]);
    }

    #[test]
    fn material_uv_source_params_preserve_prepared_texture_sources() {
        let prepared = PreparedMaterial {
            render_pass: PreparedRenderPass::Opaque,
            shader_family: MaterialShaderFamily::Character,
            alpha_policy: crate::PreparedMaterialAlphaPolicy::default(),
            texture_bindings: PreparedTextureBindings::default(),
            texture_sampling: PreparedTextureSamplingSet::default(),
            uv_sources: PreparedMaterialUvSources {
                textures: PreparedTextureUvSources {
                    base_color: PreparedUvSource::Uv0,
                    normal: PreparedUvSource::Uv1,
                    mask: PreparedUvSource::Uv2,
                    material_map: PreparedUvSource::Uv3,
                    multi_map: PreparedUvSource::Uv3,
                    specular: PreparedUvSource::Uv2,
                    emissive: PreparedUvSource::Uv1,
                    material_properties: PreparedUvSource::Uv0,
                    tile_properties: PreparedUvSource::Uv1,
                    sheen_properties: PreparedUvSource::Uv2,
                    sphere_properties: PreparedUvSource::Uv3,
                    tile_matrix: PreparedUvSource::Uv0,
                    index: PreparedUvSource::Uv1,
                    other: PreparedUvSource::Uv2,
                },
                uv0_scroll: PreparedUvSource::Uv0,
                uv1_scroll: PreparedUvSource::Uv1,
            },
            feature_flags: PreparedMaterialFeatureFlags::default(),
            unsupported_inputs: PreparedMaterialUnsupportedInputs::default(),
            resource_availability: PreparedMaterialResourceAvailability::default(),
            runtime_fallbacks: PreparedMaterialRuntimeFallbacks::default(),
            render_backfaces: true,
        };

        assert_eq!(
            material_uv_source_params(prepared),
            (
                [0.0, 1.0, 2.0, 3.0],
                [3.0, 2.0, 1.0, 0.0],
                [1.0, 2.0, 3.0, 0.0],
                [1.0, 2.0, 0.0, 0.0]
            )
        );
    }

    #[test]
    fn gpu_vertex_layout_exposes_extended_model_channels() {
        let layout = GpuVertex::layout();

        assert_eq!(
            layout.array_stride,
            std::mem::size_of::<GpuVertex>() as wgpu::BufferAddress
        );
        assert_eq!(layout.step_mode, wgpu::VertexStepMode::Vertex);
        assert_eq!(
            layout
                .attributes
                .iter()
                .map(|attribute| (
                    attribute.shader_location,
                    attribute.offset,
                    attribute.format
                ))
                .collect::<Vec<_>>(),
            vec![
                (
                    0,
                    std::mem::offset_of!(GpuVertex, position) as wgpu::BufferAddress,
                    wgpu::VertexFormat::Float32x3
                ),
                (
                    1,
                    std::mem::offset_of!(GpuVertex, normal) as wgpu::BufferAddress,
                    wgpu::VertexFormat::Float32x3
                ),
                (
                    2,
                    std::mem::offset_of!(GpuVertex, uv0) as wgpu::BufferAddress,
                    wgpu::VertexFormat::Float32x2
                ),
                (
                    3,
                    std::mem::offset_of!(GpuVertex, bitangent) as wgpu::BufferAddress,
                    wgpu::VertexFormat::Float32x4
                ),
                (
                    4,
                    std::mem::offset_of!(GpuVertex, color) as wgpu::BufferAddress,
                    wgpu::VertexFormat::Float32x4
                ),
                (
                    5,
                    std::mem::offset_of!(GpuVertex, uv1) as wgpu::BufferAddress,
                    wgpu::VertexFormat::Float32x2
                ),
                (
                    6,
                    std::mem::offset_of!(GpuVertex, uv2) as wgpu::BufferAddress,
                    wgpu::VertexFormat::Float32x2
                ),
                (
                    7,
                    std::mem::offset_of!(GpuVertex, uv3) as wgpu::BufferAddress,
                    wgpu::VertexFormat::Float32x2
                ),
                (
                    8,
                    std::mem::offset_of!(GpuVertex, color1) as wgpu::BufferAddress,
                    wgpu::VertexFormat::Float32x4
                ),
                (
                    9,
                    std::mem::offset_of!(GpuVertex, normal1) as wgpu::BufferAddress,
                    wgpu::VertexFormat::Float32x3
                ),
                (
                    10,
                    std::mem::offset_of!(GpuVertex, bitangent1) as wgpu::BufferAddress,
                    wgpu::VertexFormat::Float32x4
                ),
                (
                    11,
                    std::mem::offset_of!(GpuVertex, flow0) as wgpu::BufferAddress,
                    wgpu::VertexFormat::Float32x4
                ),
                (
                    12,
                    std::mem::offset_of!(GpuVertex, flow1) as wgpu::BufferAddress,
                    wgpu::VertexFormat::Float32x4
                ),
            ]
        );
    }

    #[test]
    fn flatten_model_filters_non_surface_roles_but_keeps_additive_lightshafts() {
        let model = crate::ModelData {
            bounds: crate::ModelBounds::default(),
            materials: vec![fallback_material()],
            textures: Vec::new(),
            meshes: vec![
                test_mesh("normal", 0.0),
                test_mesh("shadow", 1.0),
                test_mesh("terrainShadow", 2.0),
                test_mesh("verticalFog", 3.0),
                test_mesh("lightShaft", 4.0),
                test_mesh("materialChange", 5.0),
                test_mesh("glass", 6.0),
            ],
        };

        let (vertices, indices, batches) = flatten_model(&model);

        assert_eq!(vertices.len(), 12);
        assert_eq!(indices.len(), 12);
        assert_eq!(batches.len(), 4);
        assert_eq!(
            batches
                .iter()
                .map(|batch| batch.center[0])
                .collect::<Vec<_>>(),
            vec![0.5, 4.5, 5.5, 6.5]
        );
        assert_eq!(batches[0].pass(), PreparedRenderPass::Opaque);
        assert_eq!(batches[1].pass(), PreparedRenderPass::AdditiveLightShaft);
        assert_eq!(batches[2].pass(), PreparedRenderPass::Opaque);
        assert_eq!(batches[3].pass(), PreparedRenderPass::Glass);
        assert_eq!(
            batches
                .iter()
                .map(|batch| batch.draw_role)
                .collect::<Vec<_>>(),
            vec![
                ModelMeshDrawRole::Normal,
                ModelMeshDrawRole::LightShaft,
                ModelMeshDrawRole::MaterialChange,
                ModelMeshDrawRole::Glass
            ]
        );
    }

    #[test]
    fn flatten_model_preserves_extended_vertex_channels() {
        let mut mesh = test_mesh("normal", 0.0);
        mesh.vertices[0].uv1 = [0.1, 0.2];
        mesh.vertices[0].uv2 = [0.3, 0.4];
        mesh.vertices[0].uv3 = [0.5, 0.6];
        mesh.vertices[0].color1 = Some([0.7, 0.8, 0.9, 1.0]);
        mesh.vertices[0].normal1 = Some([1.0, 0.0, 0.0]);
        mesh.vertices[0].bitangent1 = Some([0.0, 1.0, 0.0, -1.0]);
        mesh.vertices[0].flow0 = Some([0.11, 0.22, 0.33, 0.44]);
        mesh.vertices[0].flow1 = Some([0.55, 0.66, 0.77, 0.88]);
        let model = crate::ModelData {
            bounds: crate::ModelBounds::default(),
            materials: vec![fallback_material()],
            textures: Vec::new(),
            meshes: vec![mesh],
        };

        let (vertices, _, _) = flatten_model(&model);

        assert_eq!(vertices[0].uv1, [0.1, 0.2]);
        assert_eq!(vertices[0].uv2, [0.3, 0.4]);
        assert_eq!(vertices[0].uv3, [0.5, 0.6]);
        assert_eq!(vertices[0].color1, [0.7, 0.8, 0.9, 1.0]);
        assert_eq!(vertices[0].normal1, [1.0, 0.0, 0.0]);
        assert_eq!(vertices[0].bitangent1, [0.0, 1.0, 0.0, -1.0]);
        assert_eq!(vertices[0].flow0, [0.11, 0.22, 0.33, 0.44]);
        assert_eq!(vertices[0].flow1, [0.55, 0.66, 0.77, 0.88]);
        assert_eq!(vertices[1].color1, [1.0, 1.0, 1.0, 1.0]);
        assert_eq!(vertices[1].normal1, [0.0, 1.0, 0.0]);
        assert_eq!(vertices[1].bitangent1, [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(vertices[1].flow0, [0.0; 4]);
        assert_eq!(vertices[1].flow1, [0.0; 4]);
    }

    fn test_prepared_render_pass(
        alpha_mode: MaterialAlphaMode,
        render_mode: MaterialRenderMode,
        draw_role: ModelMeshDrawRole,
    ) -> PreparedRenderPass {
        let mut material = fallback_material();
        material.alpha_mode = alpha_mode;
        material.render_mode = render_mode;
        let model = crate::ModelData {
            bounds: crate::ModelBounds::default(),
            materials: vec![material],
            textures: Vec::new(),
            meshes: Vec::new(),
        };
        prepare_material_for_draw_role(model.materials().first(), draw_role).render_pass
    }

    fn test_batch(material_slot: usize, pass: PreparedRenderPass, center: [f32; 3]) -> DrawBatch {
        DrawBatch {
            material_slot,
            material_bind_group_index: material_slot,
            draw_role: ModelMeshDrawRole::Normal,
            index_start: 0,
            index_count: 3,
            prepared_material: PreparedMaterial {
                render_pass: pass,
                shader_family: MaterialShaderFamily::Unknown,
                alpha_policy: crate::PreparedMaterialAlphaPolicy::default(),
                texture_bindings: PreparedTextureBindings::default(),
                texture_sampling: PreparedTextureSamplingSet::default(),
                uv_sources: PreparedMaterialUvSources::default(),
                feature_flags: PreparedMaterialFeatureFlags::default(),
                unsupported_inputs: PreparedMaterialUnsupportedInputs::default(),
                resource_availability: PreparedMaterialResourceAvailability::default(),
                runtime_fallbacks: PreparedMaterialRuntimeFallbacks::default(),
                render_backfaces: true,
            },
            center,
        }
    }

    fn test_sampling(
        color_space: PreparedTextureColorSpace,
        filter: PreparedTextureFilter,
        address_mode: PreparedTextureAddressMode,
    ) -> PreparedTextureSampling {
        PreparedTextureSampling {
            color_space,
            filter,
            address_mode,
        }
    }

    fn test_mesh(category: &str, x: f32) -> crate::ModelMesh {
        crate::ModelMesh {
            path: format!("test/{category}.mdl"),
            part_index: 0,
            mesh_category: Some(category.to_string()),
            submesh: None,
            shape_influences: Vec::new(),
            material_index: 0,
            material_slot: 0,
            material_name: "test".to_string(),
            color: [1.0, 1.0, 1.0],
            bone_table: None,
            vertices: vec![
                test_vertex([x, 0.0, 0.0]),
                test_vertex([x + 1.0, 0.0, 0.0]),
                test_vertex([x, 1.0, 0.0]),
            ],
            indices: vec![0, 1, 2],
        }
    }

    fn test_vertex(position: [f32; 3]) -> crate::ModelVertex {
        crate::ModelVertex {
            position,
            blend_weights: None,
            blend_indices: None,
            normal: [0.0, 1.0, 0.0],
            uv0: [0.0, 0.0],
            uv1: [0.0, 0.0],
            uv2: [0.0, 0.0],
            uv3: [0.0, 0.0],
            bitangent: [1.0, 0.0, 0.0, 1.0],
            normal1: None,
            bitangent1: None,
            color: [1.0, 1.0, 1.0, 1.0],
            color1: None,
            flow0: None,
            flow1: None,
        }
    }

    fn test_texture(kind: crate::ModelTextureKind) -> crate::ModelTexture {
        crate::ModelTexture {
            path: "test.tex".to_string(),
            kind,
            width: 1,
            height: 1,
            array_size: 1,
            array_layer_height: 1,
            rgba: vec![0, 0, 0, 255],
            rgba_f32: None,
        }
    }

    fn test_array_texture(kind: crate::ModelTextureKind, array_size: u16) -> crate::ModelTexture {
        crate::ModelTexture {
            path: "array.tex".to_string(),
            kind,
            width: 1,
            height: array_size,
            array_size,
            array_layer_height: 1,
            rgba: vec![128; usize::from(array_size) * 4],
            rgba_f32: None,
        }
    }
}
