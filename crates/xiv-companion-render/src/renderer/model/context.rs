use super::*;

/// 实例 joint 表上限（bone storage buffer 预分配 256×64B）。FFXIV 模型骨骼数
/// 远小于此（角色骨架约 200、怪物几十），超出截断并记日志。
pub(crate) const MAX_JOINTS: usize = 256;
/// joint storage buffer 头（u32 joint 数 + 12 字节对齐填充）大小。
pub(crate) const JOINT_STORAGE_HEADER_SIZE: wgpu::BufferAddress = 16;
/// joint storage buffer 总大小：头 + 256 个 mat4。
pub(crate) const JOINT_STORAGE_BUFFER_SIZE: wgpu::BufferAddress = 16 + (MAX_JOINTS as u64) * 64;

/// 模型无关的 GPU 渲染上下文：device/queue、全部渲染管线、shader module、
/// bind group layout、相机 uniform 与后处理状态。创建代价集中在管线编译，
/// 应在同一渲染目标上跨模型复用；模型相关状态由 [`ModelRenderContext::create_model`]
/// 生成 [`ModelInstance`]，切换模型时只重建 instance。
pub struct ModelRenderContext {
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
    lightshaft_pipeline: wgpu::RenderPipeline,
    lightshaft_culled_pipeline: wgpu::RenderPipeline,
    blur_pipeline: wgpu::RenderPipeline,
    compose_pipeline: wgpu::RenderPipeline,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    material_bind_group_layout: wgpu::BindGroupLayout,
    joint_bind_group_layout: wgpu::BindGroupLayout,
    post_sampler: wgpu::Sampler,
    compose_uniform_buffer: wgpu::Buffer,
    blur_bind_group_layout: wgpu::BindGroupLayout,
    compose_bind_group_layout: wgpu::BindGroupLayout,
    vfx_pipeline_add: wgpu::RenderPipeline,
    vfx_pipeline_blend: wgpu::RenderPipeline,
    vfx_mesh_pipeline_add: wgpu::RenderPipeline,
    vfx_mesh_pipeline_blend: wgpu::RenderPipeline,
    vfx_bind_group_layout: wgpu::BindGroupLayout,
    vfx_sampler: wgpu::Sampler,
    post_process: Option<PostProcessState>,
    format: wgpu::TextureFormat,
    msaa_samples: u32,
}

/// 单个模型的 GPU 实例：顶点/索引缓冲、透明索引缓冲、绘制批次、材质 bind
/// group（含按批次去重的纹理缓存，随实例生命周期）与模型包围盒。由
/// [`ModelRenderContext::create_model`] 同步创建，切换模型时整体替换。
/// 蒙皮 joint 矩阵在实例级 group(2) storage buffer（`update_joint_matrices`
/// 增量更新，材质重建不影响）。
pub struct ModelInstance {
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    transparent_index_buffer: wgpu::Buffer,
    draw_batches: Vec<DrawBatch>,
    material_bind_groups: Vec<wgpu::BindGroup>,
    bounds_center: [f32; 3],
    bounds_radius: f32,
    joint_buffer: wgpu::Buffer,
    joint_bind_group: wgpu::BindGroup,
    joint_count: usize,
    joint_names: Vec<String>,
}

pub struct ModelRenderer {
    context: ModelRenderContext,
    instance: ModelInstance,
}

impl ModelRenderer {
    pub fn new<M: ModelRenderData + ?Sized>(
        device: wgpu::Device,
        queue: wgpu::Queue,
        format: wgpu::TextureFormat,
        model: &M,
    ) -> Self {
        Self::new_with_prepared_options(
            device,
            queue,
            format,
            model,
            PreparedModelOptions::default(),
        )
    }

    pub fn new_with_prepared_options<M: ModelRenderData + ?Sized>(
        device: wgpu::Device,
        queue: wgpu::Queue,
        format: wgpu::TextureFormat,
        model: &M,
        prepared_options: PreparedModelOptions,
    ) -> Self {
        let context = ModelRenderContext::new(device, queue, format);
        Self::from_context(context, model, prepared_options)
    }

    /// [`new_with_prepared_options`] 的骨架版：实例携带 rest pose 关节矩阵。
    pub fn new_with_skeleton_and_prepared_options<M: ModelRenderData + ?Sized>(
        device: wgpu::Device,
        queue: wgpu::Queue,
        format: wgpu::TextureFormat,
        model: &M,
        prepared_options: PreparedModelOptions,
        skeleton: Option<&xiv_companion_data::ModelSkeleton>,
    ) -> Self {
        let context = ModelRenderContext::new(device, queue, format);
        Self::from_context_with_skeleton(context, model, prepared_options, skeleton)
    }

    /// 在既有 context 上为 `model` 创建实例。复用调用方持有的管线与后处理
    /// 状态，避免跨模型切换时重复初始化设备与编译管线。
    pub fn from_context<M: ModelRenderData + ?Sized>(
        context: ModelRenderContext,
        model: &M,
        prepared_options: PreparedModelOptions,
    ) -> Self {
        let instance = context.create_model(model, prepared_options);
        Self { context, instance }
    }

    /// [`from_context`] 的骨架版。
    pub fn from_context_with_skeleton<M: ModelRenderData + ?Sized>(
        context: ModelRenderContext,
        model: &M,
        prepared_options: PreparedModelOptions,
        skeleton: Option<&xiv_companion_data::ModelSkeleton>,
    ) -> Self {
        let instance = context.create_model_with_skeleton(model, prepared_options, skeleton);
        Self { context, instance }
    }

    pub fn context(&self) -> &ModelRenderContext {
        &self.context
    }

    /// 复用 context 同步替换当前模型实例：重建顶点/索引缓冲、绘制批次与
    /// 材质 bind group，不触碰设备、管线与后处理状态。
    pub fn set_model<M: ModelRenderData + ?Sized>(
        &mut self,
        model: &M,
        prepared_options: PreparedModelOptions,
    ) {
        self.instance = self.context.create_model(model, prepared_options);
    }

    /// [`set_model`] 的骨架版：joint 表与 storage buffer 随实例重建。
    pub fn set_model_with_skeleton<M: ModelRenderData + ?Sized>(
        &mut self,
        model: &M,
        prepared_options: PreparedModelOptions,
        skeleton: Option<&xiv_companion_data::ModelSkeleton>,
    ) {
        self.instance = self
            .context
            .create_model_with_skeleton(model, prepared_options, skeleton);
    }

    pub fn update_materials<M: ModelRenderData + ?Sized>(&mut self, model: &M) {
        self.instance.update_materials(&self.context, model);
    }

    /// 实例 joint 名表（顶点 joints 槽位对应顺序；无骨架为空）。
    pub fn joint_names(&self) -> &[String] {
        self.instance.joint_names()
    }

    /// 覆盖实例 joint 矩阵（世界 × inverse(bind world)，数据层
    /// `joint_matrices`/`SkeletonInverseBindCache` 计算），立即生效于后续渲染。
    pub fn update_joint_matrices(&mut self, matrices: &[[f32; 16]]) {
        self.instance.update_joint_matrices(&self.context, matrices);
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
        vfx: Option<&VfxParticles>,
    ) {
        self.context.render(
            &self.instance,
            target_view,
            depth_view,
            viewport,
            yaw,
            pitch,
            zoom,
            pan,
            options,
            vfx,
        );
    }

    pub fn device(&self) -> &wgpu::Device {
        self.context.device()
    }

    pub fn queue(&self) -> &wgpu::Queue {
        self.context.queue()
    }

    pub fn format(&self) -> wgpu::TextureFormat {
        self.context.format()
    }

    #[cfg(all(feature = "test-support", not(target_arch = "wasm32")))]
    pub(crate) fn hdr_scene_texture(&self) -> Option<&wgpu::Texture> {
        self.context.hdr_scene_texture()
    }
}

impl ModelRenderContext {
    pub fn new(device: wgpu::Device, queue: wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        Self::new_with_msaa(device, queue, format, 1)
    }

    /// MSAA 变体：`msaa_samples` 为场景 HDR 目标与全部场景管线的采样数
    /// （1 = 关闭，4 = 4x 多重采样，bloom/compose 前 resolve 到单采样纹理）。
    pub fn new_with_msaa(
        device: wgpu::Device,
        queue: wgpu::Queue,
        format: wgpu::TextureFormat,
        msaa_samples: u32,
    ) -> Self {
        let msaa_samples = if msaa_samples >= 3 { 4 } else { 1 };
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("model shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!(concat!(env!("OUT_DIR"), "/model.wgsl")).into(),
            ),
        });
        let post_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("model postprocess shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!(concat!(env!("OUT_DIR"), "/postprocess.wgsl")).into(),
            ),
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
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 13,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
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
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
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
                    wgpu::BindGroupLayoutEntry {
                        binding: 19,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 20,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 21,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 22,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 23,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 24,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 25,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 26,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 28,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 29,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 30,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 31,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 32,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let joint_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("model joint bind group layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("weapon pipeline layout"),
            bind_group_layouts: &[
                Some(&camera_bind_group_layout),
                Some(&material_bind_group_layout),
                Some(&joint_bind_group_layout),
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
            msaa_samples,
        );
        let culled_pipeline = create_model_pipeline(
            &device,
            &shader,
            &pipeline_layout,
            "weapon culled model pipeline",
            ModelPipelineBlend::Opaque,
            true,
            msaa_samples,
        );
        let cutout_pipeline = create_model_pipeline(
            &device,
            &shader,
            &pipeline_layout,
            "weapon cutout model pipeline",
            ModelPipelineBlend::Opaque,
            false,
            msaa_samples,
        );
        let cutout_culled_pipeline = create_model_pipeline(
            &device,
            &shader,
            &pipeline_layout,
            "weapon cutout culled model pipeline",
            ModelPipelineBlend::Opaque,
            true,
            msaa_samples,
        );
        let dither_depth_pipeline = create_model_pipeline(
            &device,
            &shader,
            &pipeline_layout,
            "weapon dither depth pipeline",
            ModelPipelineBlend::DitherDepth,
            false,
            msaa_samples,
        );
        let dither_depth_culled_pipeline = create_model_pipeline(
            &device,
            &shader,
            &pipeline_layout,
            "weapon dither depth culled pipeline",
            ModelPipelineBlend::DitherDepth,
            true,
            msaa_samples,
        );
        let outline_pipeline = create_outline_pipeline(
            &device,
            &shader,
            &pipeline_layout,
            "weapon outline pipeline",
            msaa_samples,
        );
        let transparent_pipeline = create_model_pipeline(
            &device,
            &shader,
            &pipeline_layout,
            "weapon transparent model pipeline",
            ModelPipelineBlend::Alpha,
            false,
            msaa_samples,
        );
        let transparent_culled_pipeline = create_model_pipeline(
            &device,
            &shader,
            &pipeline_layout,
            "weapon transparent culled model pipeline",
            ModelPipelineBlend::Alpha,
            true,
            msaa_samples,
        );
        let glass_pipeline = create_model_pipeline(
            &device,
            &shader,
            &pipeline_layout,
            "weapon glass model pipeline",
            ModelPipelineBlend::Alpha,
            false,
            msaa_samples,
        );
        let glass_culled_pipeline = create_model_pipeline(
            &device,
            &shader,
            &pipeline_layout,
            "weapon glass culled model pipeline",
            ModelPipelineBlend::Alpha,
            true,
            msaa_samples,
        );
        let additive_pipeline = create_model_pipeline(
            &device,
            &shader,
            &pipeline_layout,
            "weapon additive model pipeline",
            ModelPipelineBlend::Additive,
            false,
            msaa_samples,
        );
        let additive_culled_pipeline = create_model_pipeline(
            &device,
            &shader,
            &pipeline_layout,
            "weapon additive culled model pipeline",
            ModelPipelineBlend::Additive,
            true,
            msaa_samples,
        );
        let lightshaft_pipeline = create_model_pipeline_with_fragment_entry(
            &device,
            &shader,
            &pipeline_layout,
            "weapon lightshaft pipeline",
            ModelPipelineBlend::Additive,
            "fs_lightshaft",
            false,
            msaa_samples,
        );
        let lightshaft_culled_pipeline = create_model_pipeline_with_fragment_entry(
            &device,
            &shader,
            &pipeline_layout,
            "weapon lightshaft culled pipeline",
            ModelPipelineBlend::Additive,
            "fs_lightshaft",
            true,
            msaa_samples,
        );

        let vfx_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("weapon vfx particle shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!(concat!(env!("OUT_DIR"), "/vfx.wgsl")).into(),
            ),
        });
        // 每组粒子（TC1×TC2 贴图对 + 合成模式）一个 bind group：双贴图 +
        // 共享 sampler + 组参数 uniform。
        let vfx_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("weapon vfx particle bind group layout"),
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
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });
        let vfx_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("weapon vfx particle pipeline layout"),
            bind_group_layouts: &[
                Some(&camera_bind_group_layout),
                Some(&vfx_bind_group_layout),
            ],
            immediate_size: 0,
        });
        /// 加色（One/One，alpha 不写入）与 alpha 混合（SrcAlpha/InvSrcAlpha）。
        fn vfx_blend_state(blend_add: bool) -> wgpu::BlendState {
            if blend_add {
                wgpu::BlendState {
                    color: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::One,
                        dst_factor: wgpu::BlendFactor::One,
                        operation: wgpu::BlendOperation::Add,
                    },
                    alpha: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::Zero,
                        dst_factor: wgpu::BlendFactor::One,
                        operation: wgpu::BlendOperation::Add,
                    },
                }
            } else {
                wgpu::BlendState {
                    color: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::SrcAlpha,
                        dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                        operation: wgpu::BlendOperation::Add,
                    },
                    alpha: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::One,
                        dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                        operation: wgpu::BlendOperation::Add,
                    },
                }
            }
        }
        let create_vfx_pipeline = |label: &str,
                                   vertex_entry: &str,
                                   fragment_entry: &str,
                                   buffers: &[wgpu::VertexBufferLayout<'static>],
                                   blend_add: bool|
         -> wgpu::RenderPipeline {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&vfx_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &vfx_shader,
                    entry_point: Some(vertex_entry),
                    buffers,
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &vfx_shader,
                    entry_point: Some(fragment_entry),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: POST_FORMAT,
                        blend: Some(vfx_blend_state(blend_add)),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth24Plus,
                    depth_write_enabled: Some(false),
                    depth_compare: Some(wgpu::CompareFunction::LessEqual),
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState {
                    count: msaa_samples,
                    ..Default::default()
                },
                multiview_mask: None,
                cache: None,
            })
        };
        let vfx_pipeline_add = create_vfx_pipeline(
            "weapon vfx particle pipeline (add)",
            "vs_main",
            "fs_add",
            &[GpuVfxQuad::LAYOUT],
            true,
        );
        let vfx_pipeline_blend = create_vfx_pipeline(
            "weapon vfx particle pipeline (blend)",
            "vs_main",
            "fs_blend",
            &[GpuVfxQuad::LAYOUT],
            false,
        );
        let vfx_mesh_pipeline_add = create_vfx_pipeline(
            "weapon vfx mesh pipeline (add)",
            "vs_mesh",
            "fs_add",
            &[GpuVfxMeshVertex::LAYOUT, GpuVfxMeshInstance::LAYOUT],
            true,
        );
        let vfx_mesh_pipeline_blend = create_vfx_pipeline(
            "weapon vfx mesh pipeline (blend)",
            "vs_mesh",
            "fs_blend",
            &[GpuVfxMeshVertex::LAYOUT, GpuVfxMeshInstance::LAYOUT],
            false,
        );
        let vfx_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("weapon vfx particle sampler"),
            // UVSet scroll 环绕采样。
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

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
                params: compose_post_params(DEFAULT_BLOOM_STRENGTH, format),
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
            lightshaft_pipeline,
            lightshaft_culled_pipeline,
            blur_pipeline,
            compose_pipeline,
            camera_buffer,
            camera_bind_group,
            material_bind_group_layout,
            joint_bind_group_layout,
            post_sampler,
            compose_uniform_buffer,
            blur_bind_group_layout,
            compose_bind_group_layout,
            vfx_pipeline_add,
            vfx_pipeline_blend,
            vfx_mesh_pipeline_add,
            vfx_mesh_pipeline_blend,
            vfx_bind_group_layout,
            vfx_sampler,
            post_process: None,
            format,
            msaa_samples,
        }
    }

    /// 为 `model` 同步创建 GPU 实例：展平几何、上传顶点/索引缓冲并构建全部
    /// 材质 bind group（批次间共享去重纹理，缓存随实例生命周期）。只使用
    /// context 中的 device/queue 与 bind group layout，不编译任何管线，
    /// 可重复调用来切换模型。
    pub fn create_model<M: ModelRenderData + ?Sized>(
        &self,
        model: &M,
        prepared_options: PreparedModelOptions,
    ) -> ModelInstance {
        self.create_model_with_skeleton(model, prepared_options, None)
    }

    /// [`ModelRenderContext::create_model`] 的骨架版：`skeleton` 为 Some 时
    /// 构建实例 joint 表（全部渲染 mesh bone_table 名并集，按名→骨骼索引），
    /// 顶点 blend 索引重映射为实例 joint 下标，预分配 joint storage buffer 并
    /// 上传 rest pose 关节矩阵（world × inverse(bind world)，rest 输出恒等）。
    /// 无骨架时 joint 数 0，shader 走原路径。bone storage 上限 256 个 mat4，
    /// 超出截断并记日志。材质更新（`ModelInstance::update_materials`）只重建
    /// group(1)，joint 绑定在 group(2) 不受影响。
    pub fn create_model_with_skeleton<M: ModelRenderData + ?Sized>(
        &self,
        model: &M,
        prepared_options: PreparedModelOptions,
        skeleton: Option<&xiv_companion_data::ModelSkeleton>,
    ) -> ModelInstance {
        let (vertices, indices, draw_batches, joint_names) =
            flatten_model_with_options_and_skeleton(model, prepared_options, skeleton);
        let (bounds_center, bounds_radius) = gpu_vertices_bounds(&vertices)
            .unwrap_or((model.bounds().center, model.bounds().radius));
        let vertex_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("weapon vertex buffer"),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
        let index_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("weapon index buffer"),
                contents: bytemuck::cast_slice(&indices),
                usage: wgpu::BufferUsages::INDEX,
            });
        let transparent_index_count = draw_batches
            .iter()
            .map(|batch| batch.transparent_triangles.len() * 3)
            .sum::<usize>();
        let transparent_index_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("weapon transparent index buffer"),
            size: (transparent_index_count.max(1) * std::mem::size_of::<u32>())
                as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let material_bind_groups = create_material_bind_groups(
            &self.device,
            &self.queue,
            &self.material_bind_group_layout,
            model,
            &draw_batches,
        );
        let (joint_buffer, joint_bind_group, joint_count) =
            self.create_joint_binding(skeleton, &joint_names);
        ModelInstance {
            vertex_buffer,
            index_buffer,
            transparent_index_buffer,
            draw_batches,
            material_bind_groups,
            bounds_center,
            bounds_radius,
            joint_buffer,
            joint_bind_group,
            joint_count,
            joint_names,
        }
    }

    /// 预分配 joint storage buffer（STORAGE|COPY_DST，头 + 256×64B）并构建
    /// group(2) bind group。有骨架时上传 rest pose 关节矩阵；无骨架 joint 数 0。
    pub(crate) fn create_joint_binding(
        &self,
        skeleton: Option<&xiv_companion_data::ModelSkeleton>,
        joint_names: &[String],
    ) -> (wgpu::Buffer, wgpu::BindGroup, usize) {
        let joint_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("model joint storage buffer"),
            size: JOINT_STORAGE_BUFFER_SIZE,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let joint_count = match skeleton {
            Some(_) => joint_names.len().min(MAX_JOINTS),
            None => 0,
        };
        if joint_names.len() > MAX_JOINTS {
            eprintln!(
                "model skinning: joint table truncated from {} to {MAX_JOINTS}",
                joint_names.len()
            );
        }
        self.queue.write_buffer(
            &joint_buffer,
            0,
            bytemuck::bytes_of(&JointStorageHeader {
                count: joint_count as u32,
                _pad: [0; 3],
            }),
        );
        if let Some(skeleton) = skeleton.filter(|_| joint_count > 0) {
            let rest_pose = xiv_companion_data::SkeletonPose::rest_pose(skeleton);
            let matrices = xiv_companion_data::joint_matrices(
                skeleton,
                &rest_pose,
                &joint_names[..joint_count],
            );
            self.queue.write_buffer(
                &joint_buffer,
                JOINT_STORAGE_HEADER_SIZE,
                bytemuck::cast_slice(&matrices),
            );
        }
        let joint_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("model joint bind group"),
            layout: &self.joint_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: joint_buffer.as_entire_binding(),
            }],
        });
        (joint_buffer, joint_bind_group, joint_count)
    }

    pub fn render(
        &mut self,
        model: &ModelInstance,
        target_view: &wgpu::TextureView,
        depth_view: &wgpu::TextureView,
        viewport: [u32; 2],
        yaw: f32,
        pitch: f32,
        zoom: f32,
        pan: [f32; 2],
        options: ModelRenderOptions,
        vfx: Option<&VfxParticles>,
    ) {
        let uniform = camera_uniform(
            model.bounds_center,
            model.bounds_radius,
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
                params: compose_post_params(options.bloom_strength(), self.format),
            }),
        );
        let sorted_transparent = sorted_transparent_triangles(&model.draw_batches, yaw, pitch);
        if !sorted_transparent.indices.is_empty() {
            self.queue.write_buffer(
                &model.transparent_index_buffer,
                0,
                bytemuck::cast_slice(&sorted_transparent.indices),
            );
        }
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
            let (scene_target, scene_resolve_target) = post.scene_color_target();
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("weapon scene render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: scene_target,
                    resolve_target: scene_resolve_target,
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
                })],
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
            render_pass.set_bind_group(2, &model.joint_bind_group, &[]);
            render_pass.set_vertex_buffer(0, model.vertex_buffer.slice(..));
            render_pass.set_index_buffer(model.index_buffer.slice(..), wgpu::IndexFormat::Uint32);

            for batch in model
                .draw_batches
                .iter()
                .filter(|batch| batch.pass() == PreparedRenderPass::Opaque)
            {
                render_pass.set_pipeline(if batch.render_backfaces() {
                    &self.pipeline
                } else {
                    &self.culled_pipeline
                });
                draw_model_batch(&mut render_pass, &model.material_bind_groups, batch);
            }

            for batch in model
                .draw_batches
                .iter()
                .filter(|batch| batch.pass() == PreparedRenderPass::Cutout)
            {
                render_pass.set_pipeline(if batch.render_backfaces() {
                    &self.cutout_pipeline
                } else {
                    &self.cutout_culled_pipeline
                });
                draw_model_batch(&mut render_pass, &model.material_bind_groups, batch);
            }

            for batch in model
                .draw_batches
                .iter()
                .filter(|batch| batch.uses_dither_depth_prepass())
            {
                render_pass.set_pipeline(if batch.render_backfaces() {
                    &self.dither_depth_pipeline
                } else {
                    &self.dither_depth_culled_pipeline
                });
                draw_model_batch(&mut render_pass, &model.material_bind_groups, batch);
            }

            render_pass.set_pipeline(&self.outline_pipeline);
            for batch in model
                .draw_batches
                .iter()
                .filter(|batch| batch.uses_outline_pass())
            {
                draw_model_batch(&mut render_pass, &model.material_bind_groups, batch);
            }

            render_pass.set_index_buffer(
                model.transparent_index_buffer.slice(..),
                wgpu::IndexFormat::Uint32,
            );
            for draw in &sorted_transparent.draws {
                let batch = &model.draw_batches[draw.batch_index];
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
                draw_model_batch_range(
                    &mut render_pass,
                    &model.material_bind_groups,
                    batch,
                    draw.index_start,
                    draw.index_count,
                );
            }

            render_pass.set_index_buffer(model.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            for batch in model
                .draw_batches
                .iter()
                .filter(|batch| batch.pass().uses_additive_pipeline())
            {
                let dedicated = lightshaft_uses_dedicated_pipeline(options.debug_mode);
                render_pass.set_pipeline(if dedicated && batch.render_backfaces() {
                    &self.lightshaft_pipeline
                } else if dedicated {
                    &self.lightshaft_culled_pipeline
                } else if batch.render_backfaces() {
                    &self.additive_pipeline
                } else {
                    &self.additive_culled_pipeline
                });
                draw_model_batch(&mut render_pass, &model.material_bind_groups, batch);
            }

            // VFX 粒子最后画：按绘制段选加色/混合管线，深度只测不写
            // （武器遮挡身后的粒子，粒子不遮挡后续无）。
            if let Some(vfx) = vfx {
                if vfx.count() > 0 {
                    render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
                    render_pass.set_vertex_buffer(0, vfx.instance_slice());
                    for range in &vfx.draw_ranges {
                        render_pass.set_pipeline(if range.blend_add {
                            &self.vfx_pipeline_add
                        } else {
                            &self.vfx_pipeline_blend
                        });
                        render_pass.set_bind_group(1, &vfx.bind_groups[range.group], &[]);
                        render_pass.draw(0..6, range.start..range.start + range.count);
                    }
                }
                // 网格粒子：Model/LightModel 的本体渲染路径（按网格 + 混合 +
                // 贴图组分段 draw_indexed）。
                if vfx.mesh_instance_count > 0 {
                    render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
                    render_pass.set_vertex_buffer(1, vfx.mesh_instance_buffer.slice(..));
                    for range in &vfx.mesh_draw_ranges {
                        let Some(gpu_mesh) = vfx.meshes.get(range.mesh) else {
                            continue;
                        };
                        render_pass.set_pipeline(if range.blend_add {
                            &self.vfx_mesh_pipeline_add
                        } else {
                            &self.vfx_mesh_pipeline_blend
                        });
                        render_pass.set_vertex_buffer(0, gpu_mesh.vertex_buffer.slice(..));
                        render_pass.set_index_buffer(
                            gpu_mesh.index_buffer.slice(..),
                            wgpu::IndexFormat::Uint32,
                        );
                        render_pass.set_bind_group(1, &vfx.bind_groups[range.group], &[]);
                        render_pass.draw_indexed(
                            0..gpu_mesh.index_count,
                            0,
                            range.start..range.start + range.count,
                        );
                    }
                }
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
            render_pass.set_bind_group(0, &post.blur_scene_bind_group, &[]);
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

    pub(crate) fn ensure_post_process_targets(&mut self, viewport: [u32; 2]) {
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
                self.msaa_samples,
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

    #[cfg(all(feature = "test-support", not(target_arch = "wasm32")))]
    pub(crate) fn hdr_scene_texture(&self) -> Option<&wgpu::Texture> {
        self.post_process.as_ref().map(|post| &post.scene_texture)
    }
}

impl ModelInstance {
    /// 按当前模型数据重建材质 bind group（染色等材质增量更新路径）。
    /// 纹理去重缓存随本次重建重新填充，批次共享语义与创建时一致。
    /// joint 绑定在 group(2)，本操作不触碰。
    pub fn update_materials<M: ModelRenderData + ?Sized>(
        &mut self,
        context: &ModelRenderContext,
        model: &M,
    ) {
        self.material_bind_groups = create_material_bind_groups(
            &context.device,
            &context.queue,
            &context.material_bind_group_layout,
            model,
            &self.draw_batches,
        );
    }

    /// 蒙皮实例的 joint 名表（与顶点 joints 槽位一一对应）；无骨架实例为空。
    pub fn joint_names(&self) -> &[String] {
        &self.joint_names
    }

    /// 当前生效的 joint 数（≤256；无骨架为 0）。
    pub fn joint_count(&self) -> usize {
        self.joint_count
    }

    /// 覆盖 joint 矩阵（世界 × inverse(bind world)，由数据层
    /// `joint_matrices`/`SkeletonInverseBindCache` 计算）。长度按实例 joint
    /// 数截断，经 queue.write_buffer 写进 storage buffer。
    pub fn update_joint_matrices(&mut self, context: &ModelRenderContext, matrices: &[[f32; 16]]) {
        let count = matrices.len().min(self.joint_count).min(MAX_JOINTS);
        if count == 0 {
            return;
        }
        context.queue.write_buffer(
            &self.joint_buffer,
            JOINT_STORAGE_HEADER_SIZE,
            bytemuck::cast_slice(&matrices[..count]),
        );
    }
}

/// VFX 粒子批次创建（独立 impl 块，vfx 模块协作）。
impl ModelRenderContext {
    /// 粒子批次容量上限（超过由采样器截断）。
    pub const VFX_PARTICLE_CAPACITY: usize = 4096;

    pub(crate) fn vfx_bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.vfx_bind_group_layout
    }

    pub(crate) fn vfx_sampler(&self) -> &wgpu::Sampler {
        &self.vfx_sampler
    }

    /// 创建 VFX 粒子批次：固定容量实例缓冲 + 贴图视图表（0 号固定为内置
    /// 径向光点回退，其后按文件 `Tex` 顺序对应贴图序号+1；bind group 按
    /// 粒子实际使用的贴图对/合成模式在 update 时懒创建）。
    pub fn create_vfx_particles(
        &self,
        textures: &[Option<VfxTextureInput>],
        meshes: &[xiv_companion_data::VfxDrawModel],
    ) -> VfxParticles {
        let instance_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("weapon vfx particle instances"),
            size: (Self::VFX_PARTICLE_CAPACITY * std::mem::size_of::<GpuVfxQuad>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let fallback = fallback_vfx_texture_rgba();
        let texture_views: Vec<wgpu::TextureView> = std::iter::once(&None)
            .chain(textures.iter())
            .map(|texture| {
                create_vfx_texture_view(
                    &self.device,
                    &self.queue,
                    texture.as_ref().unwrap_or(&fallback),
                )
            })
            .collect();
        // 保留文件序号对齐（无绘制数据的模型给空缓冲，实例不会引用）。
        let meshes: Vec<GpuVfxMesh> = meshes
            .iter()
            .map(|mesh| {
                let vertices: Vec<GpuVfxMeshVertex> = mesh
                    .vertices
                    .iter()
                    .map(|vertex| GpuVfxMeshVertex {
                        position: vertex.position,
                        uv: vertex.uv,
                        // 顶点色：rgb 染色 + alpha 作为羽化遮罩（火舌翼缘
                        // 等 alpha=0 区域消隐；实心壳体模型 alpha 恒 255）。
                        color: [
                            vertex.color[0] as f32 / 255.0,
                            vertex.color[1] as f32 / 255.0,
                            vertex.color[2] as f32 / 255.0,
                            vertex.color[3] as f32 / 255.0,
                        ],
                    })
                    .collect();
                let vertex_buffer =
                    self.device
                        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("weapon vfx mesh vertices"),
                            contents: bytemuck::cast_slice(&vertices),
                            usage: wgpu::BufferUsages::VERTEX,
                        });
                let index_buffer =
                    self.device
                        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("weapon vfx mesh indices"),
                            contents: bytemuck::cast_slice(&mesh.indices),
                            usage: wgpu::BufferUsages::INDEX,
                        });
                GpuVfxMesh {
                    vertex_buffer,
                    index_buffer,
                    index_count: mesh.indices.len() as u32,
                }
            })
            .collect();
        let mesh_instance_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("weapon vfx mesh instances"),
            size: (Self::VFX_PARTICLE_CAPACITY * std::mem::size_of::<GpuVfxMeshInstance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        VfxParticles {
            instance_buffer,
            texture_views,
            group_cache: HashMap::new(),
            bind_groups: Vec::new(),
            draw_ranges: Vec::new(),
            capacity: Self::VFX_PARTICLE_CAPACITY,
            count: 0,
            meshes,
            mesh_instance_buffer,
            mesh_instance_count: 0,
            mesh_draw_ranges: Vec::new(),
        }
    }
}
