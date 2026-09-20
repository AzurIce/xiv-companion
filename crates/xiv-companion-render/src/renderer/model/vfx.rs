//! VFX 粒子批次：实例缓冲 + 按（贴图组 × 合成模式 × 边界模式 × 混合）分组的
//! bind group，配合 `vfx.wesl` 管线在 HDR 场景 pass 中绘制四边形
//! （billboard/定向）与 DrawModel 网格粒子。粒子数据来自数据层确定性采样器
//! （`xiv_companion_data::avfx_sim`）。着色为四层贴图合成：TC1（可为 TLst
//! 形状遮罩）+ TC2..TC4 按各自 TCCT/TCAT 合成模式叠加，TD 扭曲贴图先抖动 UV。

use super::*;
use std::collections::HashMap;

/// GPU 四边形实例（160 字节，与 `vfx.wesl` 的 `VfxInstanceInput` 布局一致）。
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuVfxQuad {
    /// xyz 位置；w billboard 面内旋转。
    pub position_rotation: [f32; 4],
    /// xy 半宽/半高；z flags（bit0 = billboard）；w 预留。
    pub size_flags: [f32; 4],
    pub color: [f32; 4],
    /// 四层贴图的 UV（原点 + 缩放）。
    pub uv: [[f32; 4]; 4],
    /// TD 扭曲贴图采样用 UV。
    pub uvd: [f32; 4],
    /// x：TD 强度（DPow）；y：扭曲目标位（bit0..3 = uv1..4）。
    pub distortion: [f32; 4],
    /// 非 billboard 朝向（四元数 xyzw）。
    pub orientation: [f32; 4],
}

impl GpuVfxQuad {
    const ATTRIBUTES: [wgpu::VertexAttribute; 10] = {
        let mut attributes = [
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 0,
                shader_location: 0,
            };
            10
        ];
        let mut index = 0;
        while index < 10 {
            attributes[index].offset = (index * 16) as wgpu::BufferAddress;
            attributes[index].shader_location = index as u32;
            index += 1;
        }
        attributes
    };

    pub const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &Self::ATTRIBUTES,
    };
}

impl From<&xiv_companion_data::VfxQuad> for GpuVfxQuad {
    fn from(quad: &xiv_companion_data::VfxQuad) -> Self {
        let mut uv = [[0.0; 4]; 4];
        for (i, slot) in uv.iter_mut().enumerate() {
            *slot = [
                quad.uv_origins[i][0],
                quad.uv_origins[i][1],
                quad.uv_scales[i][0],
                quad.uv_scales[i][1],
            ];
        }
        Self {
            position_rotation: [
                quad.position[0],
                quad.position[1],
                quad.position[2],
                quad.rotation,
            ],
            size_flags: [
                quad.size[0],
                quad.size[1],
                f32::from_bits(quad.billboard as u32),
                0.0,
            ],
            color: quad.color,
            uv,
            uvd: [
                quad.uvd_origin[0],
                quad.uvd_origin[1],
                quad.uvd_scale[0],
                quad.uvd_scale[1],
            ],
            distortion: [
                quad.distortion_power,
                f32::from_bits(quad.distortion_targets),
                0.0,
                0.0,
            ],
            orientation: quad.orientation,
        }
    }
}

/// 一次待上传的 VFX 贴图（RGBA8）；`None` 时使用内置径向光点回退贴图。
#[derive(Clone, Debug)]
pub struct VfxTextureInput {
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

/// 网格静态顶点（52 字节）：DrawModel 顶点（位置 + 两组 UV + 顶点色）。
/// 顶点只带首组 UV（模型本身只有一组有意义，UvSet 动画在实例侧）。
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuVfxMeshVertex {
    pub position: [f32; 3],
    pub uv: [f32; 2],
    pub color: [f32; 4],
}

impl GpuVfxMeshVertex {
    pub const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x3,
                offset: 0,
                shader_location: 0,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 12,
                shader_location: 1,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 20,
                shader_location: 2,
            },
        ],
    };
}

/// 网格实例（176 字节）：位置 + 朝向四元数 + 三轴缩放 + HDR 颜色 +
/// 四层 UV + TD 扭曲参数。
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuVfxMeshInstance {
    /// xyz 位置；w flags（预留）。
    pub position_flags: [f32; 4],
    pub orientation: [f32; 4],
    pub scale: [f32; 4],
    pub color: [f32; 4],
    pub uv: [[f32; 4]; 4],
    pub uvd: [f32; 4],
    /// x：TD 强度；y：扭曲目标位。
    pub distortion: [f32; 4],
}

impl GpuVfxMeshInstance {
    const ATTRIBUTES: [wgpu::VertexAttribute; 10] = {
        let mut attributes = [
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 0,
                shader_location: 3,
            };
            10
        ];
        let mut index = 0;
        while index < 10 {
            attributes[index].offset = (index * 16) as wgpu::BufferAddress;
            attributes[index].shader_location = (index + 3) as u32;
            index += 1;
        }
        attributes
    };

    pub const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &Self::ATTRIBUTES,
    };
}

impl From<&xiv_companion_data::VfxMeshInstance> for GpuVfxMeshInstance {
    fn from(instance: &xiv_companion_data::VfxMeshInstance) -> Self {
        let mut uv = [[0.0; 4]; 4];
        for (i, slot) in uv.iter_mut().enumerate() {
            *slot = [
                instance.uv_origins[i][0],
                instance.uv_origins[i][1],
                instance.uv_scales[i][0],
                instance.uv_scales[i][1],
            ];
        }
        Self {
            position_flags: [
                instance.position[0],
                instance.position[1],
                instance.position[2],
                0.0,
            ],
            orientation: instance.orientation,
            scale: [instance.scale[0], instance.scale[1], instance.scale[2], 0.0],
            color: instance.color,
            uv,
            uvd: [
                instance.uvd_origin[0],
                instance.uvd_origin[1],
                instance.uvd_scale[0],
                instance.uvd_scale[1],
            ],
            distortion: [
                instance.distortion_power,
                f32::from_bits(instance.distortion_targets),
                0.0,
                0.0,
            ],
        }
    }
}

/// 单个网格的 GPU 资源（静态顶点/索引缓冲）。
pub(crate) struct GpuVfxMesh {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub index_count: u32,
}

/// 贴图合成组键：四层贴图序号 + TD + 各层合成模式/边界/bC2A + TC1 遮罩。
/// 同组粒子共享一个 bind group（贴图 × 采样器 × 参数 uniform）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct VfxGroupKey {
    textures: [i32; 4],
    texture_d: i32,
    combine_modes: [[i32; 2]; 3],
    color_to_alpha: [bool; 4],
    texture_borders: [[i32; 2]; 4],
    distortion_borders: [i32; 2],
    texture1_is_shape_mask: bool,
}

impl VfxGroupKey {
    fn of_quad(quad: &xiv_companion_data::VfxQuad) -> Self {
        Self {
            textures: quad.texture_indexes,
            texture_d: quad.texture_distortion_index,
            combine_modes: quad.combine_modes,
            color_to_alpha: quad.color_to_alpha,
            texture_borders: quad.texture_borders,
            distortion_borders: quad.distortion_borders,
            texture1_is_shape_mask: quad.texture1_is_shape_mask,
        }
    }

    fn of_mesh(instance: &xiv_companion_data::VfxMeshInstance) -> Self {
        Self {
            textures: instance.texture_indexes,
            texture_d: instance.texture_distortion_index,
            combine_modes: instance.combine_modes,
            color_to_alpha: instance.color_to_alpha,
            texture_borders: instance.texture_borders,
            distortion_borders: instance.distortion_borders,
            texture1_is_shape_mask: instance.texture1_is_shape_mask,
        }
    }
}

/// 组参数 uniform（与 `vfx.wesl` 的 `VfxGroupParams` 布局一致；
/// WGSL vec3<u32> 按 16 字节对齐，Rust 侧显式补齐到 48 字节）。
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct VfxGroupParams {
    /// TC2/TC3/TC4 的颜色合成模式。
    combine_color: [u32; 3],
    _pad0: u32,
    /// TC2/TC3/TC4 的 alpha 合成模式。
    combine_alpha: [u32; 3],
    _pad1: u32,
    /// 各层 bC2A 位掩码。
    color_to_alpha: u32,
    /// bit0: TC1 为形状遮罩；bit1..4: 各层有贴图；bit5: 有 TD 贴图。
    flags: u32,
    _pad2: [u32; 2],
}

/// 一条绘制段：同一混合模式 + 同一贴图组的连续实例区间。
#[derive(Clone, Copy, Debug)]
pub(crate) struct VfxDrawRange {
    pub blend_add: bool,
    pub group: usize,
    pub start: u32,
    pub count: u32,
}

/// 网格绘制段：额外携带网格序号与剔除模式（0/3 双面、1 剔正面、2 剔背面）。
#[derive(Clone, Copy, Debug)]
pub(crate) struct VfxMeshDrawRange {
    pub mesh: usize,
    pub blend_add: bool,
    pub cull_mode: i32,
    pub group: usize,
    pub start: u32,
    pub count: u32,
}

/// 常驻 VFX 粒子批次：由 `ModelRenderContext::create_vfx_particles` 构建，
/// 每帧 `update`/`update_mesh` 后随 `render(..., vfx: Some(&batch))` 绘制。
/// bind group 按组键懒创建并缓存。
pub struct VfxParticles {
    pub(crate) instance_buffer: wgpu::Buffer,
    /// 贴图视图表：0 号为内置回退光点，其后按文件 `Tex` 顺序（序号+1）。
    pub(crate) texture_views: Vec<wgpu::TextureView>,
    pub(crate) group_cache: HashMap<VfxGroupKey, usize>,
    pub(crate) bind_groups: Vec<wgpu::BindGroup>,
    pub(crate) draw_ranges: Vec<VfxDrawRange>,
    pub(crate) capacity: usize,
    pub(crate) count: usize,
    /// 网格粒子：每模型的静态网格资源 + 每帧实例缓冲与绘制段。
    pub(crate) meshes: Vec<GpuVfxMesh>,
    pub(crate) mesh_instance_buffer: wgpu::Buffer,
    pub(crate) mesh_instance_count: usize,
    pub(crate) mesh_draw_ranges: Vec<VfxMeshDrawRange>,
}

impl VfxParticles {
    pub fn count(&self) -> usize {
        self.count
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub(crate) fn instance_slice(&self) -> wgpu::BufferSlice<'_> {
        self.instance_buffer.slice(..)
    }

    /// 组键 → bind group 下标（懒创建）。
    pub(crate) fn group_for(&mut self, context: &ModelRenderContext, key: VfxGroupKey) -> usize {
        if let Some(index) = self.group_cache.get(&key) {
            return *index;
        }
        let view_for = |index: i32| -> &wgpu::TextureView {
            let slot = if index < 0 {
                0usize
            } else {
                (index as usize + 1).min(self.texture_views.len().saturating_sub(1))
            };
            &self.texture_views[slot.max(0)]
        };
        let mut flags = key.texture1_is_shape_mask as u32;
        for (i, texture) in key.textures.iter().enumerate() {
            flags |= ((*texture >= 0) as u32) << (i + 1);
        }
        flags |= ((key.texture_d >= 0) as u32) << 5;
        let params = VfxGroupParams {
            combine_color: key.combine_modes.map(|m| m[0].max(0) as u32),
            _pad0: 0,
            combine_alpha: key.combine_modes.map(|m| m[1].max(0) as u32),
            _pad1: 0,
            color_to_alpha: key
                .color_to_alpha
                .iter()
                .enumerate()
                .fold(0u32, |acc, (i, c)| acc | ((*c as u32) << i)),
            flags,
            _pad2: [0; 2],
        };
        let uniform = context
            .device()
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("weapon vfx group params"),
                contents: bytemuck::bytes_of(&params),
                usage: wgpu::BufferUsages::UNIFORM,
            });
        // 每层贴图各自的边界模式 sampler（渐变贴图 Clamp 防越界回绕出
        // 异色条纹，UVSet 滚动贴图 Repeat）。
        let address_mode = |mode: i32| match mode {
            1 => wgpu::AddressMode::ClampToEdge,
            2 => wgpu::AddressMode::MirrorRepeat,
            _ => wgpu::AddressMode::Repeat,
        };
        let make_sampler = |modes: [i32; 2]| {
            context.device().create_sampler(&wgpu::SamplerDescriptor {
                label: Some("weapon vfx group sampler"),
                address_mode_u: address_mode(modes[0]),
                address_mode_v: address_mode(modes[1]),
                address_mode_w: wgpu::AddressMode::Repeat,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::MipmapFilterMode::Nearest,
                ..Default::default()
            })
        };
        let samplers: Vec<wgpu::Sampler> = key
            .texture_borders
            .map(make_sampler)
            .into_iter()
            .chain(std::iter::once(make_sampler(key.distortion_borders)))
            .collect();
        let mut entries = vec![
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(view_for(key.textures[0])),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(view_for(key.textures[1])),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::TextureView(view_for(key.textures[2])),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::TextureView(view_for(key.textures[3])),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: wgpu::BindingResource::TextureView(view_for(key.texture_d)),
            },
            wgpu::BindGroupEntry {
                binding: 5,
                resource: uniform.as_entire_binding(),
            },
        ];
        for (i, sampler) in samplers.iter().enumerate() {
            entries.push(wgpu::BindGroupEntry {
                binding: (6 + i) as u32,
                resource: wgpu::BindingResource::Sampler(sampler),
            });
        }
        let bind_group = context.device().create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("weapon vfx particle bind group"),
            layout: context.vfx_bind_group_layout(),
            entries: &entries,
        });
        let index = self.bind_groups.len();
        self.bind_groups.push(bind_group);
        // sampler 生命周期：wgpu 句柄引用计数，bind group 持有即存活。
        self.group_cache.insert(key, index);
        index
    }

    /// 增量上传实例数据（每帧调用；超过容量截断）。按（混合, 组键）稳定
    /// 排序分组成绘制段。
    pub fn update(&mut self, context: &ModelRenderContext, quads: &[xiv_companion_data::VfxQuad]) {
        self.count = quads.len().min(self.capacity);
        self.draw_ranges.clear();
        if self.count == 0 {
            return;
        }
        let mut ordered: Vec<(bool, VfxGroupKey, GpuVfxQuad)> = quads[..self.count]
            .iter()
            .map(|quad| (quad.blend_add, VfxGroupKey::of_quad(quad), GpuVfxQuad::from(quad)))
            .collect();
        ordered.sort_by_key(|(blend_add, key, _)| (*blend_add, *key));
        context.queue().write_buffer(
            &self.instance_buffer,
            0,
            bytemuck::cast_slice(
                &ordered
                    .iter()
                    .map(|(_, _, gpu)| *gpu)
                    .collect::<Vec<_>>(),
            ),
        );
        let mut start = 0_u32;
        for (blend_add, key, _) in &ordered {
            let group = self.group_for(context, *key);
            match self.draw_ranges.last_mut() {
                Some(range)
                    if range.blend_add == *blend_add
                        && range.group == group
                        && range.start + range.count == start =>
                {
                    range.count += 1;
                }
                _ => self.draw_ranges.push(VfxDrawRange {
                    blend_add: *blend_add,
                    group,
                    start,
                    count: 1,
                }),
            }
            start += 1;
        }
    }

    /// 增量上传网格粒子实例并按（网格, 混合, 剔除, 组键）分组记录绘制段。
    pub fn update_mesh(
        &mut self,
        context: &ModelRenderContext,
        instances: &[xiv_companion_data::VfxMeshInstance],
    ) {
        self.mesh_instance_count = instances.len().min(self.capacity);
        self.mesh_draw_ranges.clear();
        if self.mesh_instance_count == 0 {
            return;
        }
        let mut ordered: Vec<(usize, bool, i32, VfxGroupKey, GpuVfxMeshInstance)> = instances
            [..self.mesh_instance_count]
            .iter()
            .map(|instance| {
                (
                    instance.model_index,
                    instance.blend_add,
                    instance.cull_mode,
                    VfxGroupKey::of_mesh(instance),
                    GpuVfxMeshInstance::from(instance),
                )
            })
            .collect();
        ordered.sort_by_key(|(mesh, blend_add, cull, key, _)| (*mesh, *blend_add, *cull, *key));
        context.queue().write_buffer(
            &self.mesh_instance_buffer,
            0,
            bytemuck::cast_slice(
                &ordered
                    .iter()
                    .map(|(_, _, _, _, gpu)| *gpu)
                    .collect::<Vec<_>>(),
            ),
        );
        let mut start = 0_u32;
        for (mesh, blend_add, cull_mode, key, _) in &ordered {
            let group = self.group_for(context, *key);
            match self.mesh_draw_ranges.last_mut() {
                Some(range)
                    if range.mesh == *mesh
                        && range.blend_add == *blend_add
                        && range.cull_mode == *cull_mode
                        && range.group == group
                        && range.start + range.count == start =>
                {
                    range.count += 1;
                }
                _ => self.mesh_draw_ranges.push(VfxMeshDrawRange {
                    mesh: *mesh,
                    blend_add: *blend_add,
                    cull_mode: *cull_mode,
                    group,
                    start,
                    count: 1,
                }),
            }
            start += 1;
        }
    }
}

/// 单张贴图上传为 2D 纹理并返回视图。
pub(crate) fn create_vfx_texture_view(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &VfxTextureInput,
) -> wgpu::TextureView {
    let size = wgpu::Extent3d {
        width: texture.width.max(1),
        height: texture.height.max(1),
        depth_or_array_layers: 1,
    };
    let gpu_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("weapon vfx color texture"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        // VFX 贴图按线性数据处理（atex 是特效数据贴图，不走 sRGB 解码——
        // sRGB 会把暗部压近零，光罩纹理图案与火舌流光全部消失）。
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &gpu_texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &texture.rgba,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(size.width * 4),
            rows_per_image: Some(size.height),
        },
        size,
    );
    gpu_texture.create_view(&wgpu::TextureViewDescriptor::default())
}

/// 内置回退贴图：64×64 白色径向光点（无贴图时的兜底形状）。
pub(crate) fn fallback_vfx_texture_rgba() -> VfxTextureInput {
    let size = 64_u32;
    let mut rgba = Vec::with_capacity((size * size * 4) as usize);
    let center = (size - 1) as f32 * 0.5;
    for y in 0..size {
        for x in 0..size {
            let distance = (((x as f32 - center).powi(2) + (y as f32 - center).powi(2)).sqrt()
                / center)
                .clamp(0.0, 1.0);
            // 软边光点：中心全亮，边缘平滑衰减到 0。
            let alpha = ((1.0 - distance) * (1.0 - distance) * 255.0).round() as u8;
            rgba.extend_from_slice(&[255, 255, 255, alpha]);
        }
    }
    VfxTextureInput {
        rgba,
        width: size,
        height: size,
    }
}
