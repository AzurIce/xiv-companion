use super::*;

/// 实例 joint 表上限（bone storage buffer 预分配 256×64B）。FFXIV 模型骨骼数
/// 远小于此（角色骨架约 200、怪物几十），超出截断并记日志。
pub(crate) const MAX_JOINTS: usize = 256;
/// joint storage buffer 头（u32 joint 数 + 12 字节对齐填充）大小。
pub(crate) const JOINT_STORAGE_HEADER_SIZE: wgpu::BufferAddress = 16;
/// joint storage buffer 总大小：头 + 256 个 mat4。
pub(crate) const JOINT_STORAGE_BUFFER_SIZE: wgpu::BufferAddress = 16 + (MAX_JOINTS as u64) * 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum VfxSoftGeometry {
    Quad,
    Line,
    Polyline,
    Mesh,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct VfxSoftPipelineKey {
    geometry: VfxSoftGeometry,
    blend_mode: VfxBlendMode,
    cull_index: usize,
    depth_mode: usize,
}

#[derive(Clone, Copy)]
enum VfxDrawItem {
    Quad(VfxDrawRange),
    Polyline(VfxPolylineDrawRange),
    Mesh(VfxMeshDrawRange),
    Decal(VfxDecalDrawRange),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum VfxDrawSegment {
    Regular,
    Soft,
    Decal,
}

// Client 3910b0: Context pass and TLS local-key base for each root group.
// These are ordering stages, not SHPK slot indexes or render-target identities.
fn ordinary_vfx_scene_stage(authored_layer: i32) -> Option<(u8, u32)> {
    const STAGES: [(u8, u32); 12] = [
        (13, 0x800000),
        (11, 0x808000),
        (11, 0x804000),
        (11, 0x800000),
        (6, 0xffbfff),
        (7, 0x40001),
        (7, 0x3c000),
        (7, 4),
        (15, 0),
        (8, 3),
        (9, 0xffbfff),
        (10, 0xffbffe),
    ];
    STAGES
        .get(usize::from(xiv_companion_data::avfx_sim::draw_layer_group(
            authored_layer,
        )))
        .copied()
}

// The preview scene has no pass-10 water receiver marked with stencil 0x40.
// Client DDTT 0/1/2 select forward pass masks 1/2/3 respectively, so only
// DDTT 0 and 2 have a draw in this scene's ordinary forward pass.
fn preview_decal_uses_forward_scene(depth_type: i32) -> bool {
    matches!(depth_type, 0 | 2)
}

impl VfxDrawItem {
    fn scene_stage(self) -> Option<(u8, u32)> {
        let layer = match self {
            Self::Quad(range) => range.draw_layer,
            Self::Polyline(range) => range.draw_layer,
            Self::Mesh(range) => range.draw_layer,
            Self::Decal(range) if preview_decal_uses_forward_scene(range.depth_type) => {
                return Some((6, 0x1fffff));
            }
            Self::Decal(_) => return None,
        };
        ordinary_vfx_scene_stage(layer)
    }

    fn priority(self) -> i32 {
        match self {
            Self::Quad(range) => range.priority,
            Self::Polyline(range) => range.priority,
            Self::Mesh(range) => range.priority,
            Self::Decal(range) => range.priority,
        }
    }

    fn draw_order(self) -> Option<u64> {
        match self {
            Self::Quad(range) => range.draw_order,
            Self::Polyline(range) => range.draw_order,
            Self::Mesh(range) => range.draw_order,
            Self::Decal(range) => range.draw_order,
        }
    }

    fn segment(self) -> VfxDrawSegment {
        match self {
            Self::Quad(range) => {
                if range.soft_particle {
                    VfxDrawSegment::Soft
                } else {
                    VfxDrawSegment::Regular
                }
            }
            Self::Polyline(range) => {
                if range.soft_particle {
                    VfxDrawSegment::Soft
                } else {
                    VfxDrawSegment::Regular
                }
            }
            Self::Mesh(range) => {
                if range.soft_particle {
                    VfxDrawSegment::Soft
                } else {
                    VfxDrawSegment::Regular
                }
            }
            Self::Decal(_) => VfxDrawSegment::Decal,
        }
    }
}

fn preview_camera_depth_row(uniform: &CameraUniform) -> [f32; 4] {
    // camera_uniform builds a right-handed look-at view: its Z row is the
    // eye-to-target outward direction and the negative eye-space dot product.
    [
        uniform.view_dir[0],
        uniform.view_dir[1],
        uniform.view_dir[2],
        -(uniform.view_dir[0] * uniform.camera_position[0]
            + uniform.view_dir[1] * uniform.camera_position[1]
            + uniform.view_dir[2] * uniform.camera_position[2]),
    ]
}

fn document_sort_ranks(
    ranges: &[VfxDocumentSortRange],
    camera_depth_row: [f32; 4],
) -> Option<Vec<usize>> {
    let keys = ranges
        .iter()
        .map(|range| {
            xiv_companion_data::avfx_sim::document_sort_key(
                range.position,
                camera_depth_row,
                range.soft_key_offset,
                range.registration_serial,
            )
        })
        .collect::<Vec<_>>();
    // The client's unordered-float tie behavior is not established. Avoid
    // inventing a total order for malformed/nonfinite source values.
    if keys.iter().any(|key| !key.is_finite()) {
        return None;
    }
    let mut indexes = (0..ranges.len()).collect::<Vec<_>>();
    indexes.sort_by(|&a, &b| keys[a].partial_cmp(&keys[b]).unwrap());
    let mut ranks = vec![0; ranges.len()];
    for (rank, index) in indexes.into_iter().enumerate() {
        ranks[index] = rank;
    }
    Some(ranks)
}

fn document_rank_for_order(
    order: Option<u64>,
    ranges: &[VfxDocumentSortRange],
    ranks: &[usize],
) -> Option<usize> {
    let order = order?;
    ranges
        .iter()
        .position(|range| (range.order_start..range.order_end).contains(&order))
        .map(|index| ranks[index])
}

fn ordered_vfx_draws(vfx: &VfxParticles, camera_depth_row: [f32; 4]) -> Vec<VfxDrawItem> {
    let mut draws = Vec::with_capacity(
        vfx.draw_ranges.len()
            + vfx.polyline_draw_ranges.len()
            + vfx.mesh_draw_ranges.len()
            + vfx.decal_draw_ranges.len(),
    );
    draws.extend(vfx.draw_ranges.iter().copied().map(VfxDrawItem::Quad));
    draws.extend(
        vfx.polyline_draw_ranges
            .iter()
            .copied()
            .map(VfxDrawItem::Polyline),
    );
    draws.extend(vfx.mesh_draw_ranges.iter().copied().map(VfxDrawItem::Mesh));
    draws.extend(
        vfx.decal_draw_ranges
            .iter()
            .copied()
            .filter(|range| preview_decal_uses_forward_scene(range.depth_type))
            .map(VfxDrawItem::Decal),
    );
    sort_vfx_draw_items(&mut draws, &vfx.document_sort_ranges, camera_depth_row);
    draws
}

fn sort_vfx_draw_items(
    draws: &mut [VfxDrawItem],
    document_ranges: &[VfxDocumentSortRange],
    camera_depth_row: [f32; 4],
) {
    if draws.iter().all(|item| item.scene_stage().is_some()) {
        let ranks = document_sort_ranks(document_ranges, camera_depth_row);
        if let Some(ranks) = ranks.filter(|ranks| {
            !ranks.is_empty()
                && draws.iter().all(|item| {
                    document_rank_for_order(item.draw_order(), document_ranges, ranks).is_some()
                })
        }) {
            draws.sort_by_key(|item| {
                (
                    item.scene_stage(),
                    document_rank_for_order(item.draw_order(), document_ranges, &ranks),
                    if matches!(item, VfxDrawItem::Decal(_)) {
                        0
                    } else {
                        item.priority()
                    },
                    item.draw_order(),
                )
            });
        } else {
            draws.sort_by_key(|item| {
                (
                    item.scene_stage(),
                    if matches!(item, VfxDrawItem::Decal(_)) {
                        0
                    } else {
                        item.priority()
                    },
                    item.draw_order(),
                )
            });
        }
    } else {
        // Unknown ordinary scene layers have no verified stage mapping.
        draws.sort_by_key(|item| (item.priority(), item.draw_order()));
    }
}

#[cfg(test)]
mod document_sort_tests {
    use super::*;

    fn quad(order: u64, priority: i32) -> VfxDrawItem {
        VfxDrawItem::Quad(VfxDrawRange {
            draw_layer: 2,
            priority,
            draw_order: Some(order),
            last_draw_order: Some(order),
            blend_mode: VfxBlendMode::Blend,
            depth_mode: 0,
            cull_mode: 1,
            double_group: None,
            line_list: false,
            soft_particle: false,
            screen_copy: false,
            vertex_count: 6,
            group: 0,
            start: 0,
            count: 1,
        })
    }

    fn mesh(order: u64, priority: i32) -> VfxDrawItem {
        VfxDrawItem::Mesh(VfxMeshDrawRange {
            draw_layer: 2,
            priority,
            draw_order: Some(order),
            last_draw_order: Some(order),
            mesh: 0,
            blend_mode: VfxBlendMode::Blend,
            depth_mode: 0,
            cull_mode: 1,
            soft_particle: false,
            screen_copy: false,
            group: 0,
            start: 0,
            count: 1,
        })
    }

    #[test]
    fn camera_and_sko_order_documents_before_intra_document_priority() {
        let mut documents = [
            VfxDocumentSortRange {
                order_start: 0,
                order_end: 2,
                position: [0.0, 0.0, 1.0],
                soft_key_offset: 0.0,
                registration_serial: 1,
            },
            VfxDocumentSortRange {
                order_start: 3,
                order_end: 5,
                position: [0.0, 0.0, -1.0],
                soft_key_offset: 0.0,
                registration_serial: 2,
            },
        ];
        let input = [quad(0, -10), mesh(3, 10), mesh(1, 5), quad(4, -5)];
        let sorted = |row, documents: &[VfxDocumentSortRange]| {
            let mut draws = input;
            sort_vfx_draw_items(&mut draws, documents, row);
            draws.map(|item| item.draw_order().unwrap())
        };
        assert_eq!(sorted([0.0, 0.0, 1.0, 0.0], &documents), [4, 3, 0, 1]);
        assert_eq!(sorted([0.0, 0.0, -1.0, 0.0], &documents), [0, 1, 4, 3]);
        documents[1].soft_key_offset = 3.0;
        assert_eq!(sorted([0.0, 0.0, 1.0, 0.0], &documents), [0, 1, 4, 3]);
    }

    #[test]
    fn preview_camera_depth_row_reverses_document_order_with_yaw() {
        let front = camera_uniform(
            [0.0; 3],
            1.0,
            [128, 128],
            0.0,
            0.0,
            3.0,
            [0.0; 2],
            ModelRenderOptions::default(),
        );
        let back = camera_uniform(
            [0.0; 3],
            1.0,
            [128, 128],
            std::f32::consts::PI,
            0.0,
            3.0,
            [0.0; 2],
            ModelRenderOptions::default(),
        );
        let documents = [
            VfxDocumentSortRange {
                order_start: 0,
                order_end: 1,
                position: [0.0, 0.0, -1.0],
                soft_key_offset: 0.0,
                registration_serial: 1,
            },
            VfxDocumentSortRange {
                order_start: 2,
                order_end: 3,
                position: [0.0, 0.0, 1.0],
                soft_key_offset: 0.0,
                registration_serial: 2,
            },
        ];
        assert_eq!(
            document_sort_ranks(&documents, preview_camera_depth_row(&front)),
            Some(vec![0, 1])
        );
        assert_eq!(
            document_sort_ranks(&documents, preview_camera_depth_row(&back)),
            Some(vec![1, 0])
        );
    }
}

fn vfx_depth_state(depth_mode: usize) -> wgpu::DepthStencilState {
    wgpu::DepthStencilState {
        format: wgpu::TextureFormat::Depth24Plus,
        // DsDt=false suppresses writes even when DsDw=true.
        depth_write_enabled: Some(depth_mode == 1),
        depth_compare: Some(if depth_mode == 2 {
            wgpu::CompareFunction::Always
        } else {
            wgpu::CompareFunction::LessEqual
        }),
        stencil: wgpu::StencilState::default(),
        bias: wgpu::DepthBiasState::default(),
    }
}

#[cfg(test)]
mod depth_state_tests {
    use super::*;

    #[test]
    fn normal_and_soft_particles_share_all_depth_modes() {
        let tested = vfx_depth_state(0);
        assert_eq!(tested.depth_compare, Some(wgpu::CompareFunction::LessEqual));
        assert_eq!(tested.depth_write_enabled, Some(false));

        let written = vfx_depth_state(1);
        assert_eq!(
            written.depth_compare,
            Some(wgpu::CompareFunction::LessEqual)
        );
        assert_eq!(written.depth_write_enabled, Some(true));

        let disabled = vfx_depth_state(2);
        assert_eq!(disabled.depth_compare, Some(wgpu::CompareFunction::Always));
        assert_eq!(disabled.depth_write_enabled, Some(false));
    }
}

fn vfx_scene_texture_layout_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

fn vfx_scene_cube_layout_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::Cube,
            multisampled: false,
        },
        count: None,
    }
}

fn vfx_scene_sampler_layout_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
        count: None,
    }
}

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
    surface_overlay_bind_group_layout: wgpu::BindGroupLayout,
    joint_bind_group_layout: wgpu::BindGroupLayout,
    post_sampler: wgpu::Sampler,
    compose_uniform_buffer: wgpu::Buffer,
    blur_bind_group_layout: wgpu::BindGroupLayout,
    compose_bind_group_layout: wgpu::BindGroupLayout,
    /// [blend_mode][cull: none/front/back][depth_mode]。
    vfx_quad_pipelines: [[[wgpu::RenderPipeline; 3]; 3]; VfxBlendMode::ALL.len()],
    vfx_depth_copy_pipeline: wgpu::RenderPipeline,
    vfx_shader: wgpu::ShaderModule,
    vfx_soft_pipeline_layout: wgpu::PipelineLayout,
    vfx_soft_pipelines: HashMap<VfxSoftPipelineKey, wgpu::RenderPipeline>,
    /// Non-Smpl Line uses the client's two-vertex line-list submission.
    vfx_line_pipelines: [[wgpu::RenderPipeline; 3]; VfxBlendMode::ALL.len()],
    /// Camera-facing no-Binder edge Polyline triangle stream.
    vfx_polyline_pipelines: [[[wgpu::RenderPipeline; 3]; 3]; VfxBlendMode::ALL.len()],
    /// [blend_mode][cull: none/front/back][depth_mode]。
    vfx_mesh_pipelines: [[[wgpu::RenderPipeline; 3]; 3]; VfxBlendMode::ALL.len()],
    vfx_decal_pipelines: [wgpu::RenderPipeline; VfxBlendMode::ALL.len()],
    vfx_bind_group_layout: wgpu::BindGroupLayout,
    vfx_scene_bind_group_layout: wgpu::BindGroupLayout,
    vfx_soft_depth_bind_group_layout: wgpu::BindGroupLayout,
    vfx_decal_bind_group_layout: wgpu::BindGroupLayout,
    vfx_decal_vertex_buffer: wgpu::Buffer,
    vfx_decal_index_buffer: wgpu::Buffer,
    vfx_sampler: wgpu::Sampler,
    vfx_reflection_cube_view: wgpu::TextureView,
    vfx_reflection_sampler: wgpu::Sampler,
    vfx_portrait_view: wgpu::TextureView,
    vfx_portrait_sampler: wgpu::Sampler,
    vfx_portrait_available: bool,
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
    surface_overlay_bind_groups: Vec<wgpu::BindGroup>,
    bounds_center: [f32; 3],
    bounds_radius: f32,
    joint_buffer: wgpu::Buffer,
    joint_bind_group: wgpu::BindGroup,
    joint_count: usize,
    joint_names: Vec<String>,
    mdl_preview_offsets: HashMap<String, [f32; 3]>,
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

    /// Set the reflection fallback for TR draws without a valid file cube.
    /// The view must be a filterable cube texture.
    pub fn set_vfx_reflection_provider(
        &mut self,
        cube_view: wgpu::TextureView,
        sampler: wgpu::Sampler,
    ) {
        self.context.set_vfx_reflection_provider(cube_view, sampler);
    }

    /// Provide the character portrait selected by TC1 `bUOS` (`-5`). Set this
    /// before creating the VFX particle batch so its group flags can enable
    /// the source.
    pub fn set_vfx_portrait_provider(
        &mut self,
        portrait_view: wgpu::TextureView,
        sampler: wgpu::Sampler,
    ) {
        self.context
            .set_vfx_portrait_provider(portrait_view, sampler);
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

    /// Advance and sample all VFX mounts against the preview offsets of this
    /// renderer's current model instance.
    pub fn update_weapon_vfx_particles(&self, particles: &mut WeaponVfxParticles, time: f32) {
        particles.advance_to(time);
        particles.update(&self.context, &self.instance, time);
    }

    pub fn create_weapon_vfx_particles(
        &self,
        data: &xiv_companion_data::WeaponVfxAttachments,
    ) -> WeaponVfxParticles {
        self.context
            .create_weapon_vfx_particles(&self.instance, data)
    }

    pub fn binder_camera_snapshot(
        &self,
        viewport: [u32; 2],
        yaw: f32,
        pitch: f32,
        zoom: f32,
        pan: [f32; 2],
        options: ModelRenderOptions,
    ) -> xiv_companion_data::VfxBinderCameraSnapshot {
        self.context.binder_camera_snapshot(
            &self.instance,
            viewport,
            yaw,
            pitch,
            zoom,
            pan,
            options,
        )
    }

    pub fn vfx_camera_view_snapshot(
        &self,
        viewport: [u32; 2],
        yaw: f32,
        pitch: f32,
        zoom: f32,
        pan: [f32; 2],
        options: ModelRenderOptions,
    ) -> xiv_companion_data::VfxCameraViewSnapshot {
        self.context.vfx_camera_view_snapshot(
            &self.instance,
            viewport,
            yaw,
            pitch,
            zoom,
            pan,
            options,
        )
    }

    pub fn create_weapon_vfx_particles_with_camera(
        &self,
        data: &xiv_companion_data::WeaponVfxAttachments,
        camera: xiv_companion_data::VfxBinderCameraSnapshot,
    ) -> Result<WeaponVfxParticles, String> {
        self.context
            .create_weapon_vfx_particles_with_camera(&self.instance, data, camera)
    }

    pub fn set_weapon_vfx_camera(
        &self,
        particles: &mut WeaponVfxParticles,
        camera: xiv_companion_data::VfxBinderCameraSnapshot,
    ) -> Result<(), String> {
        particles.set_camera(&self.instance, camera)
    }

    pub fn create_weapon_vfx_particles_with_camera_view(
        &self,
        data: &xiv_companion_data::WeaponVfxAttachments,
        camera: xiv_companion_data::VfxCameraViewSnapshot,
    ) -> Result<WeaponVfxParticles, String> {
        self.context
            .create_weapon_vfx_particles_with_camera_view(&self.instance, data, camera)
    }

    pub fn set_weapon_vfx_camera_view(
        &self,
        particles: &mut WeaponVfxParticles,
        camera: xiv_companion_data::VfxCameraViewSnapshot,
    ) -> Result<(), String> {
        particles.set_camera_view(&self.instance, camera)
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
        self.render_to_with_aura(
            target_view,
            depth_view,
            viewport,
            yaw,
            pitch,
            zoom,
            pan,
            options,
            vfx,
            None,
        );
    }

    /// Shade a prepared Aura on compatible batches of its target model.
    pub fn render_to_with_aura(
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
        aura: Option<&WeaponVfxAuraResource>,
    ) {
        let auras = aura.into_iter().collect::<Vec<_>>();
        self.render_to_with_auras(
            target_view,
            depth_view,
            viewport,
            yaw,
            pitch,
            zoom,
            pan,
            options,
            vfx,
            &auras,
        );
    }

    pub fn render_to_with_auras(
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
        auras: &[&WeaponVfxAuraResource],
    ) {
        self.context.render_with_auras(
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
            auras,
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
    /// Minimum device limits for all model and VFX pipelines and their fixed
    /// capacity buffers. WebGPU devices expose requested limits, rather than
    /// every limit supported by the adapter.
    pub fn required_limits(adapter_limits: wgpu::Limits) -> wgpu::Limits {
        let mut limits = wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter_limits);
        // The decal layout also includes the mesh and Polyline bindings:
        // two storage bindings in VFX group 1, plus decal instances in group 2.
        limits.max_storage_buffers_per_shader_stage = 3;
        let largest_binding = JOINT_STORAGE_BUFFER_SIZE
            .max((Self::VFX_PARTICLE_CAPACITY * std::mem::size_of::<GpuVfxMeshInstance>()) as u64)
            .max(
                (Self::VFX_POLYLINE_VERTEX_CAPACITY * std::mem::size_of::<GpuVfxPolylineVertex>())
                    as u64,
            )
            .max((Self::VFX_PARTICLE_CAPACITY * std::mem::size_of::<GpuVfxDecalInstance>()) as u64);
        limits.max_storage_buffer_binding_size = largest_binding;
        limits.max_vertex_buffer_array_stride = 2_048;
        limits
    }

    /// Replace the fallback reflection cube used when TR has no file cube.
    pub fn set_vfx_reflection_provider(
        &mut self,
        cube_view: wgpu::TextureView,
        sampler: wgpu::Sampler,
    ) {
        self.vfx_reflection_cube_view = cube_view;
        self.vfx_reflection_sampler = sampler;
    }

    pub(crate) fn vfx_reflection_cube_view(&self) -> &wgpu::TextureView {
        &self.vfx_reflection_cube_view
    }

    pub fn set_vfx_portrait_provider(
        &mut self,
        portrait_view: wgpu::TextureView,
        sampler: wgpu::Sampler,
    ) {
        self.vfx_portrait_view = portrait_view;
        self.vfx_portrait_sampler = sampler;
        self.vfx_portrait_available = true;
    }

    pub(crate) fn vfx_portrait_available(&self) -> bool {
        self.vfx_portrait_available
    }

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

        let surface_overlay_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("model surface overlay bind group layout"),
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
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2Array,
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
                ],
            });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("weapon pipeline layout"),
            bind_group_layouts: &[
                Some(&camera_bind_group_layout),
                Some(&material_bind_group_layout),
                Some(&joint_bind_group_layout),
                Some(&surface_overlay_bind_group_layout),
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
        // 每组粒子一个 bind group：0..4 TC1..TC4/TD，5 参数，6..10 sampler，
        // 11 网格实例 storage，12/13 TP，14 Polyline storage，15/16 TN。
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
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
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
                        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 6,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 7,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
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
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 10,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 11,
                        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
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
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
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
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    vfx_scene_cube_layout_entry(17),
                    vfx_scene_sampler_layout_entry(18),
                ],
            });
        let vfx_scene_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("weapon vfx scene-copy bind group layout"),
                entries: &[
                    vfx_scene_texture_layout_entry(2),
                    vfx_scene_texture_layout_entry(3),
                    vfx_scene_cube_layout_entry(4),
                    vfx_scene_sampler_layout_entry(5),
                    vfx_scene_texture_layout_entry(6),
                    vfx_scene_sampler_layout_entry(7),
                ],
            });
        let vfx_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("weapon vfx particle pipeline layout"),
            bind_group_layouts: &[
                Some(&camera_bind_group_layout),
                Some(&vfx_bind_group_layout),
                Some(&vfx_scene_bind_group_layout),
            ],
            immediate_size: 0,
        });
        let create_vfx_pipeline = |label: &str,
                                   vertex_entry: &str,
                                   buffers: &[wgpu::VertexBufferLayout<'static>],
                                   blend_mode: VfxBlendMode,
                                   cull_mode: Option<wgpu::Face>,
                                   depth_mode: usize,
                                   topology: wgpu::PrimitiveTopology|
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
                    entry_point: Some(blend_mode.fragment_entry()),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: POST_FORMAT,
                        blend: blend_mode.blend_state(),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology,
                    cull_mode,
                    ..Default::default()
                },
                depth_stencil: Some(vfx_depth_state(depth_mode)),
                multisample: wgpu::MultisampleState {
                    count: msaa_samples,
                    ..Default::default()
                },
                multiview_mask: None,
                cache: None,
            })
        };
        let vfx_soft_depth_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("weapon vfx soft-particle depth bind group layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: if msaa_samples > 1 { 1 } else { 0 },
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Depth,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: msaa_samples > 1,
                        },
                        count: None,
                    },
                    vfx_scene_texture_layout_entry(2),
                    vfx_scene_texture_layout_entry(3),
                    vfx_scene_cube_layout_entry(4),
                    vfx_scene_sampler_layout_entry(5),
                    vfx_scene_texture_layout_entry(6),
                    vfx_scene_sampler_layout_entry(7),
                ],
            });
        let vfx_soft_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("weapon vfx soft-particle pipeline layout"),
                bind_group_layouts: &[
                    Some(&camera_bind_group_layout),
                    Some(&vfx_bind_group_layout),
                    Some(&vfx_soft_depth_bind_group_layout),
                ],
                immediate_size: 0,
            });
        let vfx_depth_copy_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("weapon VFX soft depth snapshot pipeline"),
                layout: Some(&vfx_soft_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &vfx_shader,
                    entry_point: Some("vs_depth_copy"),
                    buffers: &[],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &vfx_shader,
                    entry_point: Some(if msaa_samples > 1 {
                        "fs_depth_copy_4x"
                    } else {
                        "fs_depth_copy_1x"
                    }),
                    targets: &[],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth24Plus,
                    depth_write_enabled: Some(true),
                    depth_compare: Some(wgpu::CompareFunction::Always),
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState {
                    count: msaa_samples,
                    ..Default::default()
                },
                multiview_mask: None,
                cache: None,
            });
        // 四边形管线：[blend_mode][cull][depth_mode]。
        let vfx_quad_pipelines = VfxBlendMode::ALL.map(|blend_mode| {
            [None, Some(wgpu::Face::Front), Some(wgpu::Face::Back)].map(|cull| {
                [0_usize, 1, 2].map(|depth_mode| {
                    create_vfx_pipeline(
                        &format!(
                            "weapon vfx particle pipeline ({blend_mode:?}/{cull:?}/{depth_mode})"
                        ),
                        "vs_main",
                        &[GpuVfxQuad::LAYOUT],
                        blend_mode,
                        cull,
                        depth_mode,
                        wgpu::PrimitiveTopology::TriangleList,
                    )
                })
            })
        });
        let vfx_line_pipelines = VfxBlendMode::ALL.map(|blend_mode| {
            [0_usize, 1, 2].map(|depth_mode| {
                create_vfx_pipeline(
                    &format!("weapon vfx line pipeline ({blend_mode:?}/{depth_mode})"),
                    "vs_main",
                    &[GpuVfxQuad::LAYOUT],
                    blend_mode,
                    None,
                    depth_mode,
                    wgpu::PrimitiveTopology::LineList,
                )
            })
        });
        let vfx_polyline_pipelines = VfxBlendMode::ALL.map(|blend_mode| {
            [None, Some(wgpu::Face::Front), Some(wgpu::Face::Back)].map(|cull| {
                [0_usize, 1, 2].map(|depth_mode| {
                    create_vfx_pipeline(
                        &format!(
                            "weapon vfx polyline pipeline ({blend_mode:?}/{cull:?}/{depth_mode})"
                        ),
                        "vs_polyline",
                        &[],
                        blend_mode,
                        cull,
                        depth_mode,
                        wgpu::PrimitiveTopology::TriangleList,
                    )
                })
            })
        });
        // 网格粒子管线：[blend_mode][cull][depth_mode]（壳体膜常用剔正面）。
        let vfx_mesh_pipelines = VfxBlendMode::ALL.map(|blend_mode| {
            [None, Some(wgpu::Face::Front), Some(wgpu::Face::Back)].map(|cull| {
                [0_usize, 1, 2].map(|depth_mode| {
                    create_vfx_pipeline(
                        &format!("weapon vfx mesh pipeline ({blend_mode:?}/{cull:?}/{depth_mode})"),
                        "vs_mesh",
                        &[GpuVfxMeshVertex::LAYOUT],
                        blend_mode,
                        cull,
                        depth_mode,
                        wgpu::PrimitiveTopology::TriangleList,
                    )
                })
            })
        });
        let vfx_decal_shader_source = if msaa_samples > 1 {
            include_str!(concat!(env!("OUT_DIR"), "/vfx_decal_4x.wgsl"))
        } else {
            include_str!(concat!(env!("OUT_DIR"), "/vfx_decal_1x.wgsl"))
        };
        let vfx_decal_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("weapon vfx decal shader"),
            source: wgpu::ShaderSource::Wgsl(vfx_decal_shader_source.into()),
        });
        let vfx_decal_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("weapon vfx decal projection bind group layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Depth,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: msaa_samples > 1,
                        },
                        count: None,
                    },
                    vfx_scene_texture_layout_entry(2),
                    vfx_scene_texture_layout_entry(3),
                    vfx_scene_cube_layout_entry(4),
                    vfx_scene_sampler_layout_entry(5),
                    vfx_scene_texture_layout_entry(6),
                    vfx_scene_sampler_layout_entry(7),
                ],
            });
        let vfx_decal_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("weapon vfx decal pipeline layout"),
                bind_group_layouts: &[
                    Some(&camera_bind_group_layout),
                    Some(&vfx_bind_group_layout),
                    Some(&vfx_decal_bind_group_layout),
                ],
                immediate_size: 0,
            });
        let vfx_decal_pipelines = VfxBlendMode::ALL.map(|blend_mode| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(&format!("weapon vfx decal pipeline ({blend_mode:?})")),
                layout: Some(&vfx_decal_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &vfx_decal_shader,
                    entry_point: Some("vs_decal"),
                    buffers: &[GpuVfxDecalVertex::LAYOUT],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &vfx_decal_shader,
                    entry_point: Some(blend_mode.decal_fragment_entry()),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: POST_FORMAT,
                        blend: blend_mode.blend_state(),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    cull_mode: Some(wgpu::Face::Back),
                    ..Default::default()
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState {
                    count: msaa_samples,
                    ..Default::default()
                },
                multiview_mask: None,
                cache: None,
            })
        });
        let vfx_decal_vertex_buffer =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("weapon vfx decal cube vertices"),
                contents: bytemuck::cast_slice(&VFX_DECAL_CUBE_VERTICES),
                usage: wgpu::BufferUsages::VERTEX,
            });
        let vfx_decal_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("weapon vfx decal cube indices"),
            contents: bytemuck::cast_slice(&VFX_DECAL_CUBE_INDICES),
            usage: wgpu::BufferUsages::INDEX,
        });
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
        // Keep a deterministic cube for TR draws without a valid file cube or
        // an application-supplied scene reflection provider.
        let reflection_cube_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("weapon vfx preview reflection cube"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 6,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let reflection_faces: [[u8; 4]; 6] = [
            [180, 120, 96, 255],
            [72, 104, 160, 255],
            [112, 156, 208, 255],
            [28, 36, 56, 255],
            [128, 144, 176, 255],
            [52, 68, 92, 255],
        ];
        for (layer, face) in reflection_faces.iter().enumerate() {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &reflection_cube_texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: 0,
                        y: 0,
                        z: layer as u32,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                face,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: None,
                    rows_per_image: None,
                },
                wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
            );
        }
        let vfx_reflection_cube_view =
            reflection_cube_texture.create_view(&wgpu::TextureViewDescriptor {
                label: Some("weapon vfx preview reflection cube view"),
                dimension: Some(wgpu::TextureViewDimension::Cube),
                ..Default::default()
            });
        let vfx_reflection_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("weapon vfx reflection cube sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
        let portrait_fallback = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("weapon vfx portrait fallback"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &portrait_fallback,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &[255, 255, 255, 255],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: None,
                rows_per_image: None,
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        let vfx_portrait_view = portrait_fallback.create_view(&Default::default());
        let vfx_portrait_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("weapon vfx portrait sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
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
            surface_overlay_bind_group_layout,
            joint_bind_group_layout,
            post_sampler,
            compose_uniform_buffer,
            blur_bind_group_layout,
            compose_bind_group_layout,
            vfx_quad_pipelines,
            vfx_depth_copy_pipeline,
            vfx_shader,
            vfx_soft_pipeline_layout,
            vfx_soft_pipelines: HashMap::new(),
            vfx_line_pipelines,
            vfx_polyline_pipelines,
            vfx_mesh_pipelines,
            vfx_decal_pipelines,
            vfx_bind_group_layout,
            vfx_scene_bind_group_layout,
            vfx_soft_depth_bind_group_layout,
            vfx_decal_bind_group_layout,
            vfx_decal_vertex_buffer,
            vfx_decal_index_buffer,
            vfx_sampler,
            vfx_reflection_cube_view,
            vfx_reflection_sampler,
            vfx_portrait_view,
            vfx_portrait_sampler,
            vfx_portrait_available: false,
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
        let FlattenedModel {
            vertices,
            indices,
            draw_batches,
            joint_names,
            mdl_preview_offsets,
        } = flatten_model_with_options_and_skeleton(model, prepared_options, skeleton);
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
        let (material_bind_groups, surface_overlay_bind_groups) = create_material_bind_groups(
            &self.device,
            &self.queue,
            &self.material_bind_group_layout,
            &self.surface_overlay_bind_group_layout,
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
            surface_overlay_bind_groups,
            bounds_center,
            bounds_radius,
            joint_buffer,
            joint_bind_group,
            joint_count,
            joint_names,
            mdl_preview_offsets,
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

    fn prepare_soft_vfx_pipelines(&mut self, vfx: &VfxParticles) {
        let mut required = Vec::new();
        for range in vfx.draw_ranges.iter().filter(|range| range.soft_particle) {
            let geometry = if range.line_list {
                VfxSoftGeometry::Line
            } else {
                VfxSoftGeometry::Quad
            };
            let single_cull = if range.line_list {
                0
            } else {
                vfx_cull_index(range.cull_mode)
            };
            let culls: &[usize] = if range.double_group.is_some() {
                &[1, 2]
            } else {
                std::slice::from_ref(&single_cull)
            };
            for &cull_index in culls {
                required.push(VfxSoftPipelineKey {
                    geometry,
                    blend_mode: range.blend_mode,
                    cull_index,
                    depth_mode: range.depth_mode,
                });
            }
        }
        for range in vfx
            .polyline_draw_ranges
            .iter()
            .filter(|range| range.soft_particle)
        {
            required.push(VfxSoftPipelineKey {
                geometry: VfxSoftGeometry::Polyline,
                blend_mode: range.blend_mode,
                cull_index: vfx_cull_index(range.cull_mode),
                depth_mode: range.depth_mode,
            });
        }
        for range in vfx
            .mesh_draw_ranges
            .iter()
            .filter(|range| range.soft_particle)
        {
            let single_cull = vfx_cull_index(range.cull_mode);
            let culls: &[usize] = if range.cull_mode == 3 {
                &[1, 2]
            } else {
                std::slice::from_ref(&single_cull)
            };
            for &cull_index in culls {
                required.push(VfxSoftPipelineKey {
                    geometry: VfxSoftGeometry::Mesh,
                    blend_mode: range.blend_mode,
                    cull_index,
                    depth_mode: range.depth_mode,
                });
            }
        }
        for key in required {
            if !self.vfx_soft_pipelines.contains_key(&key) {
                let pipeline = self.create_soft_vfx_pipeline(key);
                self.vfx_soft_pipelines.insert(key, pipeline);
            }
        }
    }

    fn create_soft_vfx_pipeline(&self, key: VfxSoftPipelineKey) -> wgpu::RenderPipeline {
        let quad_buffers = [GpuVfxQuad::LAYOUT];
        let mesh_buffers = [GpuVfxMeshVertex::LAYOUT];
        let (vertex_entry, buffers, topology) = match key.geometry {
            VfxSoftGeometry::Quad => (
                "vs_main",
                &quad_buffers[..],
                wgpu::PrimitiveTopology::TriangleList,
            ),
            VfxSoftGeometry::Line => (
                "vs_main",
                &quad_buffers[..],
                wgpu::PrimitiveTopology::LineList,
            ),
            VfxSoftGeometry::Polyline => (
                "vs_polyline",
                &[][..],
                wgpu::PrimitiveTopology::TriangleList,
            ),
            VfxSoftGeometry::Mesh => (
                "vs_mesh",
                &mesh_buffers[..],
                wgpu::PrimitiveTopology::TriangleList,
            ),
        };
        let cull_mode = match key.cull_index {
            1 => Some(wgpu::Face::Front),
            2 => Some(wgpu::Face::Back),
            _ => None,
        };
        self.device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(&format!("weapon VFX soft pipeline {key:?}")),
                layout: Some(&self.vfx_soft_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &self.vfx_shader,
                    entry_point: Some(vertex_entry),
                    buffers,
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &self.vfx_shader,
                    entry_point: Some(key.blend_mode.soft_fragment_entry(self.msaa_samples > 1)),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: POST_FORMAT,
                        blend: key.blend_mode.blend_state(),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology,
                    cull_mode,
                    ..Default::default()
                },
                depth_stencil: Some(vfx_depth_state(key.depth_mode)),
                multisample: wgpu::MultisampleState {
                    count: self.msaa_samples,
                    ..Default::default()
                },
                multiview_mask: None,
                cache: None,
            })
    }

    fn soft_vfx_pipeline(&self, key: VfxSoftPipelineKey) -> &wgpu::RenderPipeline {
        self.vfx_soft_pipelines
            .get(&key)
            .expect("soft VFX pipelines are prepared before rendering")
    }

    fn draw_vfx_item<'pass>(
        &'pass self,
        render_pass: &mut wgpu::RenderPass<'pass>,
        vfx: &'pass VfxParticles,
        item: VfxDrawItem,
    ) {
        match item {
            VfxDrawItem::Quad(range) => {
                render_pass.set_vertex_buffer(0, vfx.instance_slice());
                render_pass.set_bind_group(1, &vfx.bind_groups[range.group], &[]);
                if range.line_list {
                    let pipeline = if range.soft_particle {
                        self.soft_vfx_pipeline(VfxSoftPipelineKey {
                            geometry: VfxSoftGeometry::Line,
                            blend_mode: range.blend_mode,
                            cull_index: 0,
                            depth_mode: range.depth_mode,
                        })
                    } else {
                        &self.vfx_line_pipelines[range.blend_mode as usize][range.depth_mode]
                    };
                    render_pass.set_pipeline(pipeline);
                    render_pass.draw(
                        0..range.vertex_count,
                        range.start..range.start + range.count,
                    );
                } else {
                    let pipeline = |cull_index| {
                        if range.soft_particle {
                            self.soft_vfx_pipeline(VfxSoftPipelineKey {
                                geometry: VfxSoftGeometry::Quad,
                                blend_mode: range.blend_mode,
                                cull_index,
                                depth_mode: range.depth_mode,
                            })
                        } else {
                            &self.vfx_quad_pipelines[range.blend_mode as usize][cull_index]
                                [range.depth_mode]
                        }
                    };
                    if range.double_group.is_some() {
                        for cull_index in [1, 2] {
                            render_pass.set_pipeline(pipeline(cull_index));
                            render_pass.draw(
                                0..range.vertex_count,
                                range.start..range.start + range.count,
                            );
                        }
                    } else {
                        render_pass.set_pipeline(pipeline(vfx_cull_index(range.cull_mode)));
                        render_pass.draw(
                            0..range.vertex_count,
                            range.start..range.start + range.count,
                        );
                    }
                }
            }
            VfxDrawItem::Polyline(range) => {
                let pipeline = if range.soft_particle {
                    self.soft_vfx_pipeline(VfxSoftPipelineKey {
                        geometry: VfxSoftGeometry::Polyline,
                        blend_mode: range.blend_mode,
                        cull_index: vfx_cull_index(range.cull_mode),
                        depth_mode: range.depth_mode,
                    })
                } else {
                    &self.vfx_polyline_pipelines[range.blend_mode as usize]
                        [vfx_cull_index(range.cull_mode)][range.depth_mode]
                };
                render_pass.set_pipeline(pipeline);
                render_pass.set_bind_group(1, &vfx.bind_groups[range.group], &[]);
                render_pass.draw(range.start..range.start + range.count, 0..1);
            }
            VfxDrawItem::Mesh(range) => {
                let Some(gpu_mesh) = vfx.meshes.get(range.mesh) else {
                    return;
                };
                if gpu_mesh.index_count == 0 {
                    return;
                }
                render_pass.set_vertex_buffer(0, gpu_mesh.vertex_buffer.slice(..));
                render_pass
                    .set_index_buffer(gpu_mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                render_pass.set_bind_group(1, &vfx.bind_groups[range.group], &[]);
                let pipeline = |cull_index| {
                    if range.soft_particle {
                        self.soft_vfx_pipeline(VfxSoftPipelineKey {
                            geometry: VfxSoftGeometry::Mesh,
                            blend_mode: range.blend_mode,
                            cull_index,
                            depth_mode: range.depth_mode,
                        })
                    } else {
                        &self.vfx_mesh_pipelines[range.blend_mode as usize][cull_index]
                            [range.depth_mode]
                    }
                };
                if range.cull_mode == 3 {
                    for instance in range.start..range.start + range.count {
                        for cull_index in [1, 2] {
                            render_pass.set_pipeline(pipeline(cull_index));
                            render_pass.draw_indexed(
                                0..gpu_mesh.index_count,
                                0,
                                instance..instance + 1,
                            );
                        }
                    }
                } else {
                    render_pass.set_pipeline(pipeline(vfx_cull_index(range.cull_mode)));
                    render_pass.draw_indexed(
                        0..gpu_mesh.index_count,
                        0,
                        range.start..range.start + range.count,
                    );
                }
            }
            VfxDrawItem::Decal(_) => unreachable!("decal requires a depth-sampling pass"),
        }
    }

    fn draw_nonsoft_vfx<'pass>(
        &'pass self,
        render_pass: &mut wgpu::RenderPass<'pass>,
        vfx: &'pass VfxParticles,
        camera_depth_row: [f32; 4],
    ) {
        render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
        for item in ordered_vfx_draws(vfx, camera_depth_row) {
            if item.segment() == VfxDrawSegment::Regular {
                self.draw_vfx_item(render_pass, vfx, item);
            }
        }
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
        self.render_with_aura(
            model,
            target_view,
            depth_view,
            viewport,
            yaw,
            pitch,
            zoom,
            pan,
            options,
            vfx,
            None,
        );
    }

    /// Shade a prepared Aura on compatible batches of its target model.
    pub fn render_with_aura(
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
        aura: Option<&WeaponVfxAuraResource>,
    ) {
        let auras = aura.into_iter().collect::<Vec<_>>();
        self.render_with_auras(
            model,
            target_view,
            depth_view,
            viewport,
            yaw,
            pitch,
            zoom,
            pan,
            options,
            vfx,
            &auras,
        );
    }

    /// Shade independent Aura targets without resolving competing instances
    /// on the same model.
    pub fn render_with_auras(
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
        auras: &[&WeaponVfxAuraResource],
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
        let camera_depth_row = preview_camera_depth_row(&uniform);
        self.queue
            .write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&uniform));
        for aura in auras {
            if let Some(aura_uniform) = aura.uniform_with_camera_up(
                [
                    uniform.camera_position[0],
                    uniform.camera_position[1],
                    uniform.camera_position[2],
                ],
                [
                    uniform.view_dir[0],
                    uniform.view_dir[1],
                    uniform.view_dir[2],
                ],
                Some(uniform.up()),
            ) {
                self.queue
                    .write_buffer(&aura.uniform_buffer, 0, bytemuck::bytes_of(&aura_uniform));
            }
        }
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
        let has_soft_vfx = vfx.is_some_and(|vfx| {
            vfx.draw_ranges.iter().any(|range| range.soft_particle)
                || vfx
                    .polyline_draw_ranges
                    .iter()
                    .any(|range| range.soft_particle)
                || vfx.mesh_draw_ranges.iter().any(|range| range.soft_particle)
        });
        if has_soft_vfx {
            self.prepare_soft_vfx_pipelines(vfx.expect("soft ranges require a VFX batch"));
            self.post_process
                .as_mut()
                .expect("post process targets are initialized")
                .ensure_soft_depth_target(&self.device, self.msaa_samples);
        }
        let post = self
            .post_process
            .as_ref()
            .expect("post process targets are initialized");
        let has_screen_copy = vfx.is_some_and(VfxParticles::uses_screen_copy);
        let scene_copy_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("weapon vfx scene-copy bind group"),
            layout: &self.vfx_scene_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&post.screen_copy_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&post.previous_scene_view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&self.vfx_reflection_cube_view),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::Sampler(&self.vfx_reflection_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::TextureView(&self.vfx_portrait_view),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: wgpu::BindingResource::Sampler(&self.vfx_portrait_sampler),
                },
            ],
        });
        let has_decals = vfx.is_some_and(|particles| {
            particles
                .decal_draw_ranges
                .iter()
                .any(|range| preview_decal_uses_forward_scene(range.depth_type))
        });
        let has_segmented_vfx = has_soft_vfx || has_decals;
        let decal_bind_group = has_decals.then(|| {
            let particles = vfx.expect("decal instances require a VFX batch");
            self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("weapon vfx decal projection bind group"),
                layout: &self.vfx_decal_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: particles.decal_instance_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(depth_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(&post.screen_copy_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::TextureView(&post.previous_scene_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: wgpu::BindingResource::TextureView(
                            &self.vfx_reflection_cube_view,
                        ),
                    },
                    wgpu::BindGroupEntry {
                        binding: 5,
                        resource: wgpu::BindingResource::Sampler(&self.vfx_reflection_sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 6,
                        resource: wgpu::BindingResource::TextureView(&self.vfx_portrait_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 7,
                        resource: wgpu::BindingResource::Sampler(&self.vfx_portrait_sampler),
                    },
                ],
            })
        });

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
                    resolve_target: if has_segmented_vfx && !has_screen_copy {
                        None
                    } else {
                        scene_resolve_target
                    },
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
                draw_model_batch(
                    &mut render_pass,
                    &model.material_bind_groups,
                    &model.surface_overlay_bind_groups,
                    auras,
                    batch,
                );
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
                draw_model_batch(
                    &mut render_pass,
                    &model.material_bind_groups,
                    &model.surface_overlay_bind_groups,
                    auras,
                    batch,
                );
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
                draw_model_batch(
                    &mut render_pass,
                    &model.material_bind_groups,
                    &model.surface_overlay_bind_groups,
                    auras,
                    batch,
                );
            }

            render_pass.set_pipeline(&self.outline_pipeline);
            for batch in model
                .draw_batches
                .iter()
                .filter(|batch| batch.uses_outline_pass())
            {
                draw_model_batch(
                    &mut render_pass,
                    &model.material_bind_groups,
                    &model.surface_overlay_bind_groups,
                    auras,
                    batch,
                );
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
                    &model.surface_overlay_bind_groups,
                    auras,
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
                draw_model_batch(
                    &mut render_pass,
                    &model.material_bind_groups,
                    &model.surface_overlay_bind_groups,
                    auras,
                    batch,
                );
            }

            if let Some(vfx) = vfx.filter(|_| !has_screen_copy && !has_segmented_vfx) {
                render_pass.set_bind_group(2, &scene_copy_bind_group, &[]);
                self.draw_nonsoft_vfx(&mut render_pass, vfx, camera_depth_row);
            }
        }

        if has_screen_copy {
            encoder.copy_texture_to_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &post.scene_texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyTextureInfo {
                    texture: &post.screen_copy_texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::Extent3d {
                    width: post.width,
                    height: post.height,
                    depth_or_array_layers: 1,
                },
            );
        }
        if has_screen_copy && !has_segmented_vfx {
            let (scene_target, scene_resolve_target) = post.scene_color_target();
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("weapon VFX screen-copy scene pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: scene_target,
                    resolve_target: if has_decals {
                        None
                    } else {
                        scene_resolve_target
                    },
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                occlusion_query_set: None,
                timestamp_writes: None,
                multiview_mask: None,
            });
            render_pass.set_bind_group(2, &scene_copy_bind_group, &[]);
            self.draw_nonsoft_vfx(
                &mut render_pass,
                vfx.expect("screen-copy ranges require a VFX batch"),
                camera_depth_row,
            );
        }

        // Decal samples the scene depth without attaching it; regular and soft
        // segments attach it for DsDt/DsDw. Soft needs a fresh snapshot before
        // each segment because the original depth may have changed.
        if let Some(vfx) = vfx {
            if has_segmented_vfx {
                let create_depth_bind_group = |sampled_depth: &wgpu::TextureView, label| {
                    self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some(label),
                        layout: &self.vfx_soft_depth_bind_group_layout,
                        entries: &[
                            wgpu::BindGroupEntry {
                                binding: if self.msaa_samples > 1 { 1 } else { 0 },
                                resource: wgpu::BindingResource::TextureView(sampled_depth),
                            },
                            wgpu::BindGroupEntry {
                                binding: 2,
                                resource: wgpu::BindingResource::TextureView(
                                    &post.screen_copy_view,
                                ),
                            },
                            wgpu::BindGroupEntry {
                                binding: 3,
                                resource: wgpu::BindingResource::TextureView(
                                    &post.previous_scene_view,
                                ),
                            },
                            wgpu::BindGroupEntry {
                                binding: 4,
                                resource: wgpu::BindingResource::TextureView(
                                    &self.vfx_reflection_cube_view,
                                ),
                            },
                            wgpu::BindGroupEntry {
                                binding: 5,
                                resource: wgpu::BindingResource::Sampler(
                                    &self.vfx_reflection_sampler,
                                ),
                            },
                            wgpu::BindGroupEntry {
                                binding: 6,
                                resource: wgpu::BindingResource::TextureView(
                                    &self.vfx_portrait_view,
                                ),
                            },
                            wgpu::BindGroupEntry {
                                binding: 7,
                                resource: wgpu::BindingResource::Sampler(
                                    &self.vfx_portrait_sampler,
                                ),
                            },
                        ],
                    })
                };
                let soft_depth_view = has_soft_vfx.then(|| {
                    post.soft_depth_view
                        .as_ref()
                        .expect("soft depth target is initialized")
                });
                let source_depth_bind_group = soft_depth_view.map(|_| {
                    create_depth_bind_group(depth_view, "weapon VFX source depth bind group")
                });
                let soft_depth_bind_group = soft_depth_view.map(|view| {
                    create_depth_bind_group(view, "weapon VFX soft-particle depth bind group")
                });
                let draws = ordered_vfx_draws(vfx, camera_depth_row);

                let mut start = 0;
                while start < draws.len() {
                    let segment = draws[start].segment();
                    let mut end = start + 1;
                    while end < draws.len() && draws[end].segment() == segment {
                        end += 1;
                    }
                    if segment == VfxDrawSegment::Soft {
                        let mut depth_pass =
                            encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                                label: Some("weapon VFX soft depth snapshot pass"),
                                color_attachments: &[],
                                depth_stencil_attachment: Some(
                                    wgpu::RenderPassDepthStencilAttachment {
                                        view: soft_depth_view
                                            .expect("soft segment requires a depth snapshot"),
                                        depth_ops: Some(wgpu::Operations {
                                            load: wgpu::LoadOp::Clear(1.0),
                                            store: wgpu::StoreOp::Store,
                                        }),
                                        stencil_ops: None,
                                    },
                                ),
                                occlusion_query_set: None,
                                timestamp_writes: None,
                                multiview_mask: None,
                            });
                        depth_pass.set_pipeline(&self.vfx_depth_copy_pipeline);
                        depth_pass.set_bind_group(0, &self.camera_bind_group, &[]);
                        depth_pass.set_bind_group(1, &vfx.bind_groups[0], &[]);
                        depth_pass.set_bind_group(
                            2,
                            source_depth_bind_group
                                .as_ref()
                                .expect("soft segment requires source depth binding"),
                            &[],
                        );
                        depth_pass.draw(0..3, 0..1);
                    }
                    let (scene_target, scene_resolve_target) = post.scene_color_target();
                    let scene_resolve_target = if end == draws.len() {
                        scene_resolve_target
                    } else {
                        None
                    };
                    if segment == VfxDrawSegment::Decal {
                        let mut render_pass =
                            encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                                label: Some("weapon VFX decal scene-stage pass"),
                                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                    view: scene_target,
                                    resolve_target: scene_resolve_target,
                                    depth_slice: None,
                                    ops: wgpu::Operations {
                                        load: wgpu::LoadOp::Load,
                                        store: wgpu::StoreOp::Store,
                                    },
                                })],
                                depth_stencil_attachment: None,
                                occlusion_query_set: None,
                                timestamp_writes: None,
                                multiview_mask: None,
                            });
                        render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
                        render_pass.set_bind_group(
                            2,
                            decal_bind_group
                                .as_ref()
                                .expect("decal segment requires a depth-sampling binding"),
                            &[],
                        );
                        render_pass.set_vertex_buffer(0, self.vfx_decal_vertex_buffer.slice(..));
                        render_pass.set_index_buffer(
                            self.vfx_decal_index_buffer.slice(..),
                            wgpu::IndexFormat::Uint16,
                        );
                        for &item in &draws[start..end] {
                            let VfxDrawItem::Decal(range) = item else {
                                unreachable!("decal segment contains ordinary geometry");
                            };
                            render_pass
                                .set_pipeline(&self.vfx_decal_pipelines[range.blend_mode as usize]);
                            render_pass.set_bind_group(1, &vfx.bind_groups[range.group], &[]);
                            render_pass.draw_indexed(
                                0..VFX_DECAL_CUBE_INDICES.len() as u32,
                                0,
                                range.start..range.start + range.count,
                            );
                        }
                        start = end;
                        continue;
                    }
                    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("weapon VFX priority segment pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: scene_target,
                            resolve_target: scene_resolve_target,
                            depth_slice: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Load,
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                            view: depth_view,
                            depth_ops: Some(wgpu::Operations {
                                load: wgpu::LoadOp::Load,
                                store: wgpu::StoreOp::Store,
                            }),
                            stencil_ops: None,
                        }),
                        occlusion_query_set: None,
                        timestamp_writes: None,
                        multiview_mask: None,
                    });
                    render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
                    render_pass.set_bind_group(
                        2,
                        if segment == VfxDrawSegment::Soft {
                            soft_depth_bind_group
                                .as_ref()
                                .expect("soft segment requires a depth-sampling binding")
                        } else {
                            &scene_copy_bind_group
                        },
                        &[],
                    );
                    for &item in &draws[start..end] {
                        self.draw_vfx_item(&mut render_pass, vfx, item);
                    }
                    start = end;
                }
            }
        }

        encoder.copy_texture_to_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &post.scene_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyTextureInfo {
                texture: &post.previous_scene_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::Extent3d {
                width: post.width,
                height: post.height,
                depth_or_array_layers: 1,
            },
        );

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

    pub(crate) fn surface_overlay_bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.surface_overlay_bind_group_layout
    }

    #[cfg(all(feature = "test-support", not(target_arch = "wasm32")))]
    pub(crate) fn hdr_scene_texture(&self) -> Option<&wgpu::Texture> {
        self.post_process.as_ref().map(|post| &post.scene_texture)
    }
}

impl ModelInstance {
    /// The exact layout translation applied to this MDL's preview vertices.
    /// Missing paths identify attachments belonging to a different model.
    pub fn mdl_preview_offset(&self, path: &str) -> Option<[f32; 3]> {
        self.mdl_preview_offsets.get(path).copied()
    }

    pub(crate) fn accepts_aura_target(&self, path: &str) -> bool {
        self.draw_batches
            .iter()
            .any(|batch| batch.accepts_aura_target(path))
    }

    /// 按当前模型数据重建材质 bind group（染色等材质增量更新路径）。
    /// 纹理去重缓存随本次重建重新填充，批次共享语义与创建时一致。
    /// joint 绑定在 group(2)，本操作不触碰。
    pub fn update_materials<M: ModelRenderData + ?Sized>(
        &mut self,
        context: &ModelRenderContext,
        model: &M,
    ) {
        for batch in &mut self.draw_batches {
            batch.aura_surface_compatible = model
                .materials()
                .get(batch.material_slot)
                .is_some_and(material_accepts_aura);
        }
        (self.material_bind_groups, self.surface_overlay_bind_groups) = create_material_bind_groups(
            &context.device,
            &context.queue,
            &context.material_bind_group_layout,
            &context.surface_overlay_bind_group_layout,
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
    pub const VFX_POLYLINE_VERTEX_CAPACITY: usize = Self::VFX_PARTICLE_CAPACITY * 12;

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
        let cube_texture_views = textures
            .iter()
            .map(|texture| {
                texture.as_ref().and_then(|texture| {
                    create_vfx_cube_texture_view(&self.device, &self.queue, texture)
                })
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
                        position: [
                            vertex.position[0],
                            vertex.position[1],
                            vertex.position[2],
                            vertex.position_w,
                        ],
                        normal: vertex.normal.map(|value| value as f32 / 255.0 - 0.5),
                        tangent: vertex.tangent.map(|value| value as f32 / 255.0 - 0.5),
                        // VDrw stores UVs around zero; MoveUV uses [0,1] coordinates.
                        uvs: vertex.uvs.map(|uv| uv.map(|value| value + 0.5)),
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
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let polyline_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("weapon vfx polyline vertices"),
            size: (Self::VFX_POLYLINE_VERTEX_CAPACITY * std::mem::size_of::<GpuVfxPolylineVertex>())
                as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let decal_instance_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("weapon vfx decal instances"),
            size: (Self::VFX_PARTICLE_CAPACITY * std::mem::size_of::<GpuVfxDecalInstance>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        VfxParticles {
            instance_buffer,
            texture_views,
            cube_texture_views,
            group_cache: HashMap::new(),
            bind_groups: Vec::new(),
            draw_ranges: Vec::new(),
            capacity: Self::VFX_PARTICLE_CAPACITY,
            count: 0,
            meshes,
            mesh_instance_buffer,
            mesh_instance_count: 0,
            mesh_draw_ranges: Vec::new(),
            polyline_buffer,
            polyline_draw_ranges: Vec::new(),
            polyline_vertex_count: 0,
            decal_instance_buffer,
            decal_instance_count: 0,
            decal_draw_ranges: Vec::new(),
            document_sort_ranges: Vec::new(),
        }
    }
}

impl ModelRenderContext {
    /// Derive Binder inputs from the same orbit view used by the GPU camera.
    pub fn binder_camera_snapshot(
        &self,
        model: &ModelInstance,
        viewport: [u32; 2],
        yaw: f32,
        pitch: f32,
        zoom: f32,
        pan: [f32; 2],
        options: super::ModelRenderOptions,
    ) -> xiv_companion_data::VfxBinderCameraSnapshot {
        super::camera_uniform_and_binder(
            model.bounds_center,
            model.bounds_radius,
            viewport,
            yaw,
            pitch,
            zoom,
            pan,
            options,
        )
        .1
    }

    /// Derive Binder inputs from the same orbit view used by the GPU camera.
    pub fn vfx_camera_view_snapshot(
        &self,
        model: &ModelInstance,
        viewport: [u32; 2],
        yaw: f32,
        pitch: f32,
        zoom: f32,
        pan: [f32; 2],
        options: super::ModelRenderOptions,
    ) -> xiv_companion_data::VfxCameraViewSnapshot {
        super::camera_uniform_and_vfx_view(
            model.bounds_center,
            model.bounds_radius,
            viewport,
            yaw,
            pitch,
            zoom,
            pan,
            options,
        )
        .1
    }
}
