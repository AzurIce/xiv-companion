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

/// 常驻 VFX 粒子批次：由 `ModelRenderContext::create_vfx_particles` 构建，
/// 每帧 `update` 后随 `render(..., vfx: Some(&batch))` 绘制。
pub struct VfxParticles {
    pub(crate) instance_buffer: wgpu::Buffer,
    pub(crate) bind_group: wgpu::BindGroup,
    pub(crate) capacity: usize,
    pub(crate) count: usize,
}

impl VfxParticles {
    pub fn count(&self) -> usize {
        self.count
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub(crate) fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }

    pub(crate) fn instance_slice(&self) -> wgpu::BufferSlice<'_> {
        self.instance_buffer.slice(..)
    }

    /// 增量上传实例数据（每帧调用；超过容量截断）。
    pub fn update(&mut self, context: &ModelRenderContext, quads: &[xiv_companion_data::VfxQuad]) {
        self.count = quads.len().min(self.capacity);
        if self.count == 0 {
            return;
        }
        let gpu: Vec<GpuVfxQuad> = quads[..self.count].iter().map(GpuVfxQuad::from).collect();
        context
            .queue()
            .write_buffer(&self.instance_buffer, 0, bytemuck::cast_slice(&gpu));
    }
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
