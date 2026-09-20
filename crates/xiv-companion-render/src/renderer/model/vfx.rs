//! VFX 粒子批次：实例缓冲 + 按（贴图对 × 合成模式 × 混合）分组的 bind group，
//! 配合 `vfx.wesl` 管线在 HDR 场景 pass 中绘制四边形（billboard/定向）与
//! DrawModel 网格粒子。粒子数据来自数据层确定性采样器
//! （`xiv_companion_data::avfx_sim`）。

use super::*;
use std::collections::HashMap;

/// GPU 四边形实例（128 字节，与 `vfx.wesl` 的 `VfxInstanceInput` 布局一致）。
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuVfxQuad {
    /// xyz 位置；w billboard 面内旋转。
    pub position_rotation: [f32; 4],
    /// xy 半宽/半高；z flags（bit0 = billboard）；w 预留。
    pub size_flags: [f32; 4],
    pub color: [f32; 4],
    /// 贴图 1：uv 原点 + 缩放。
    pub uv1: [f32; 4],
    /// 贴图 2：uv 原点 + 缩放。
    pub uv2: [f32; 4],
    /// 非 billboard 朝向四元数。
    pub orientation: [f32; 4],
    /// TD 扭曲贴图采样用 UV。
    pub uvd: [f32; 4],
    /// x：TD 强度（DPow）；y：扭曲目标位（bit0 uv1 / bit1 uv2）。
    pub distortion: [f32; 4],
}

impl GpuVfxQuad {
    pub const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &[
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 0,
                shader_location: 0,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 16,
                shader_location: 1,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 32,
                shader_location: 2,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 48,
                shader_location: 3,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 64,
                shader_location: 4,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 80,
                shader_location: 5,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 96,
                shader_location: 6,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 112,
                shader_location: 7,
            },
        ],
    };
}

impl From<&xiv_companion_data::VfxQuad> for GpuVfxQuad {
    fn from(quad: &xiv_companion_data::VfxQuad) -> Self {
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
            uv1: [
                quad.uv_origin[0],
                quad.uv_origin[1],
                quad.uv_scale[0],
                quad.uv_scale[1],
            ],
            uv2: [
                quad.uv2_origin[0],
                quad.uv2_origin[1],
                quad.uv2_scale[0],
                quad.uv2_scale[1],
            ],
            orientation: quad.orientation,
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

/// 网格静态顶点（36 字节）：DrawModel 顶点（位置/UV/顶点色）。
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

/// 网格实例（128 字节）：位置 + 朝向四元数 + 三轴缩放 + HDR 颜色 +
/// 双 UV + TD 扭曲参数。
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuVfxMeshInstance {
    /// xyz 位置；w flags（预留）。
    pub position_flags: [f32; 4],
    pub orientation: [f32; 4],
    pub scale: [f32; 4],
    pub color: [f32; 4],
    pub uv1: [f32; 4],
    pub uv2: [f32; 4],
    pub uvd: [f32; 4],
    /// x：TD 强度；y：扭曲目标位。
    pub distortion: [f32; 4],
}

impl GpuVfxMeshInstance {
    pub const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &[
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 0,
                shader_location: 3,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 16,
                shader_location: 4,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 32,
                shader_location: 5,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 48,
                shader_location: 6,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 64,
                shader_location: 7,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 80,
                shader_location: 8,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 96,
                shader_location: 9,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 112,
                shader_location: 10,
            },
        ],
    };
}

impl From<&xiv_companion_data::VfxMeshInstance> for GpuVfxMeshInstance {
    fn from(instance: &xiv_companion_data::VfxMeshInstance) -> Self {
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
            uv1: [
                instance.uv_origin[0],
                instance.uv_origin[1],
                instance.uv_scale[0],
                instance.uv_scale[1],
            ],
            uv2: [
                instance.uv2_origin[0],
                instance.uv2_origin[1],
                instance.uv2_scale[0],
                instance.uv2_scale[1],
            ],
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

/// 贴图合成组键：TC1/TC2 贴图序号 + TC2 合成模式 + TC1 的 bC2A。
/// 同组粒子共享一个 bind group（双贴图 + 参数 uniform）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct VfxGroupKey {
    texture1: i32,
    texture2: i32,
    texture_d: i32,
    combine_color: i32,
    combine_alpha: i32,
    color_to_alpha: bool,
    color_to_alpha2: bool,
    /// [TC1.u, TC1.v, TC2.u, TC2.v, TD.u, TD.v] 边界模式。
    texture_borders: [i32; 6],
}

impl VfxGroupKey {
    fn of_quad(quad: &xiv_companion_data::VfxQuad) -> Self {
        Self {
            texture1: quad.texture_index,
            texture2: quad.texture2_index,
            texture_d: quad.texture_distortion_index,
            combine_color: quad.combine_color,
            combine_alpha: quad.combine_alpha,
            color_to_alpha: quad.color_to_alpha,
            color_to_alpha2: quad.color_to_alpha2,
            texture_borders: quad.texture_borders,
        }
    }

    fn of_mesh(instance: &xiv_companion_data::VfxMeshInstance) -> Self {
        Self {
            texture1: instance.texture_index,
            texture2: instance.texture2_index,
            texture_d: instance.texture_distortion_index,
            combine_color: instance.combine_color,
            combine_alpha: instance.combine_alpha,
            color_to_alpha: instance.color_to_alpha,
            color_to_alpha2: instance.color_to_alpha2,
            texture_borders: instance.texture_borders,
        }
    }
}

/// 组参数 uniform（与 `vfx.wesl` 的 `VfxGroupParams` 布局一致）。
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct VfxGroupParams {
    combine_color: u32,
    combine_alpha: u32,
    flags: u32,
    _pad: u32,
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
    pub(crate) fn group_for(
        &mut self,
        context: &ModelRenderContext,
        key: VfxGroupKey,
    ) -> usize {
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
        let has_texture1 = key.texture1 >= 0;
        let has_texture2 = key.texture2 >= 0;
        let has_texture_d = key.texture_d >= 0;
        let params = VfxGroupParams {
            combine_color: key.combine_color.max(0) as u32,
            combine_alpha: key.combine_alpha.max(0) as u32,
            flags: (key.color_to_alpha as u32)
                | ((has_texture1 as u32) << 1)
                | ((has_texture2 as u32) << 2)
                | ((key.color_to_alpha2 as u32) << 3)
                | ((has_texture_d as u32) << 4),
            _pad: 0,
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
        let make_sampler = |u: i32, v: i32| {
            context.device().create_sampler(&wgpu::SamplerDescriptor {
                label: Some("weapon vfx group sampler"),
                address_mode_u: address_mode(u),
                address_mode_v: address_mode(v),
                address_mode_w: wgpu::AddressMode::Repeat,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::MipmapFilterMode::Nearest,
                ..Default::default()
            })
        };
        let sampler1 = make_sampler(key.texture_borders[0], key.texture_borders[1]);
        let sampler2 = make_sampler(key.texture_borders[2], key.texture_borders[3]);
        let sampler_d = make_sampler(key.texture_borders[4], key.texture_borders[5]);
        let bind_group = context.device().create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("weapon vfx particle bind group"),
            layout: context.vfx_bind_group_layout(),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(view_for(key.texture1)),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(view_for(key.texture2)),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&sampler1),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(view_for(key.texture_d)),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::Sampler(&sampler2),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::Sampler(&sampler_d),
                },
            ],
        });
        let index = self.bind_groups.len();
        self.bind_groups.push(bind_group);
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

    /// 增量上传网格粒子实例并按（网格, 混合, 组键）分组记录绘制段。
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
