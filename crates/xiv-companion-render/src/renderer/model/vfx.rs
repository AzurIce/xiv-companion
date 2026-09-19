//! VFX 粒子批次：实例缓冲 + 贴图 bind group，配合 `vfx.wesl` 管线在 HDR
//! 场景 pass 中绘制相机朝向的加色四边形。粒子数据来自数据层确定性采样器
//! （`xiv_companion_data::avfx_sim`）。

use super::*;

/// GPU 实例（64 字节，与 `vfx.wesl` 的 `VfxInstanceInput` 布局一致）。
/// 60 字节实例布局（wgpu 顶点属性末位偏移 ≤ stride-4，故 texture_index
/// 以 f32 装入 location 1 的 w 空槽，索引值 < 2^24 在 f32 中精确）。
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuVfxQuad {
    pub position: [f32; 3],
    pub size_x: f32,
    pub size_y: f32,
    pub rotation: f32,
    pub texture_index: f32,
    pub color: [f32; 4],
    pub uv_origin: [f32; 2],
    pub uv_scale: [f32; 2],
}

impl GpuVfxQuad {
    pub const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &[
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x3,
                offset: 0,
                shader_location: 0,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 12,
                shader_location: 1,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 28,
                shader_location: 2,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 44,
                shader_location: 3,
            },
        ],
    };
}

impl From<&xiv_companion_data::VfxQuad> for GpuVfxQuad {
    fn from(quad: &xiv_companion_data::VfxQuad) -> Self {
        Self {
            position: quad.position,
            size_x: quad.size[0],
            size_y: quad.size[1],
            rotation: quad.rotation,
            texture_index: quad.texture_index as f32,
            color: quad.color,
            uv_origin: quad.uv_origin,
            uv_scale: quad.uv_scale,
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

/// 网格实例（32 字节）：实例原点 + 缩放 + HDR 颜色。
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuVfxMeshInstance {
    pub position: [f32; 3],
    pub scale: f32,
    pub color: [f32; 4],
}

impl GpuVfxMeshInstance {
    pub const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &[
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x3,
                offset: 0,
                shader_location: 3,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32,
                offset: 12,
                shader_location: 4,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 16,
                shader_location: 5,
            },
        ],
    };
}

/// 单个网格的 GPU 资源（静态顶点/索引缓冲）。
pub(crate) struct GpuVfxMesh {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub index_count: u32,
}

/// 常驻 VFX 粒子批次：由 `ModelRenderContext::create_vfx_particles` 构建，
/// 每帧 `update` 后随 `render(..., vfx: Some(&batch))` 绘制。粒子按目标
/// 贴图分组（每组一个 bind group，一次绘制），换贴图即换组。
pub struct VfxParticles {
    pub(crate) instance_buffer: wgpu::Buffer,
    pub(crate) texture_bind_groups: Vec<wgpu::BindGroup>,
    /// 稳定排序后的绘制段：(bind group 下标, 实例起点, 实例数)。
    pub(crate) draw_ranges: Vec<(usize, u32, u32)>,
    pub(crate) capacity: usize,
    pub(crate) count: usize,
    /// 网格粒子：每模型的静态网格资源 + 每帧实例缓冲与绘制段。
    pub(crate) meshes: Vec<GpuVfxMesh>,
    pub(crate) mesh_instance_buffer: wgpu::Buffer,
    pub(crate) mesh_instance_count: usize,
    pub(crate) mesh_draw_ranges: Vec<(usize, usize, u32, u32)>, // (mesh, group, start, count)
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

    /// 增量上传实例数据（每帧调用；超过容量截断）。按 `texture_index`
    /// 稳定排序分组成绘制段；未命中任何贴图的粒子落入回退组（0 号）。
    pub fn update(&mut self, context: &ModelRenderContext, quads: &[xiv_companion_data::VfxQuad]) {
        self.count = quads.len().min(self.capacity);
        self.draw_ranges.clear();
        if self.count == 0 {
            return;
        }
        let mut ordered: Vec<GpuVfxQuad> =
            quads[..self.count].iter().map(GpuVfxQuad::from).collect();
        ordered.sort_by_key(|quad| quad.texture_index as i32);
        // texture_index 恒为整数，用 i32 排序即可。
        context
            .queue()
            .write_buffer(&self.instance_buffer, 0, bytemuck::cast_slice(&ordered));

        let mut ranges: Vec<(i32, u32, u32)> = Vec::new();
        for quad in &ordered {
            match ranges.last_mut() {
                Some((index, _, count)) if *index == quad.texture_index as i32 => *count += 1,
                _ => ranges.push((quad.texture_index as i32, 0, 1)),
            }
        }
        let mut start = 0_u32;
        self.draw_ranges = ranges
            .into_iter()
            .map(|(texture_index, _, count)| {
                let group = self.group_index_for(texture_index);
                let range = (group, start, count);
                start += count;
                range
            })
            .collect();
    }

    /// 贴图序号 → bind group 下标（0 号固定为回退贴图）。
    fn group_index_for(&self, texture_index: i32) -> usize {
        if texture_index < 0 {
            return 0;
        }
        (texture_index as usize + 1).min(self.texture_bind_groups.len() - 1)
    }

    /// 增量上传网格粒子实例并按（网格, 贴图）分组记录绘制段。
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
        let mut ordered: Vec<(xiv_companion_data::VfxMeshInstance, GpuVfxMeshInstance)> = instances
            [..self.mesh_instance_count]
            .iter()
            .map(|instance| {
                (
                    *instance,
                    GpuVfxMeshInstance {
                        position: instance.position,
                        scale: instance.scale,
                        color: instance.color,
                    },
                )
            })
            .collect();
        ordered.sort_by_key(|(instance, _)| (instance.model_index, instance.texture_index));
        context.queue().write_buffer(
            &self.mesh_instance_buffer,
            0,
            bytemuck::cast_slice(&ordered.iter().map(|(_, gpu)| *gpu).collect::<Vec<_>>()),
        );
        for (slot, (instance, _)) in ordered.iter().enumerate() {
            let group = self.group_index_for(instance.texture_index);
            match self.mesh_draw_ranges.last_mut() {
                Some((mesh, last_group, _, count))
                    if *mesh == instance.model_index && *last_group == group =>
                {
                    *count += 1
                }
                _ => self
                    .mesh_draw_ranges
                    .push((instance.model_index, group, slot as u32, 1)),
            }
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
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
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

/// 内置回退贴图：64×64 白色径向光点（无 atex 时的兜底形状）。
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
