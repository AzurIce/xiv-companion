//! VFX 粒子批次：实例缓冲 + 按（贴图组 × 合成模式 × 边界模式 × 混合）分组的
//! bind group，配合 `vfx.wesl` 管线在 HDR 场景 pass 中绘制四边形
//! （billboard/定向）与 DrawModel 网格粒子。粒子数据来自数据层确定性采样器
//! （`xiv_companion_data::avfx_sim`）。着色为四层贴图合成：TC1（可为 TLst
//! 形状遮罩）+ TC2..TC4 按各自 TCCT/TCAT 合成模式叠加，TD 扭曲贴图先抖动 UV。
//! Powder / Windmill 只启用 TC1，并使用独立的颜色钳制顺序。

use super::*;
use bytemuck::Zeroable;
use glam::{Mat3, Mat4, Quat, Vec3, Vec4};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(usize)]
pub(crate) enum VfxBlendMode {
    Blend,
    Multiply,
    Add,
    Subtract,
    Screen,
    Reverse,
    Min,
    Max,
    Opacity,
}

#[cfg(test)]
mod aura_array_tests {
    use super::*;
    use xiv_companion_data::{VfxAuraTextureArrayRgba, VfxTextureMipRgba, VfxTextureRgba};

    fn texture(value: u8) -> VfxTextureRgba {
        VfxTextureRgba {
            width: 4,
            height: 2,
            rgba: vec![value; 32],
            mips: vec![
                VfxTextureMipRgba {
                    width: 2,
                    height: 1,
                    rgba: vec![value + 1; 8],
                },
                VfxTextureMipRgba {
                    width: 1,
                    height: 1,
                    rgba: vec![value + 2; 4],
                },
            ],
            source_mip_count: 3,
            ..Default::default()
        }
    }

    fn packed() -> VfxAuraTextureArrayRgba {
        let tc2 = texture(10);
        let td = texture(20);
        VfxAuraTextureArrayRgba::pack([Some(&tc2), None, Some(&td)]).unwrap()
    }

    #[test]
    fn aura_upload_validates_slot_order_mips_and_device_limits() {
        let valid = packed();
        let limits = wgpu::Limits::downlevel_defaults();
        assert_eq!(validate_vfx_aura_texture_array(&valid, &limits), Ok(2));

        let mut invalid = valid.clone();
        invalid.slot_layers = [Some(1), None, Some(0)];
        assert_eq!(
            validate_vfx_aura_texture_array(&invalid, &limits),
            Err(VfxAuraTextureUploadError::InvalidLayout)
        );
        invalid = valid.clone();
        invalid.mips[1].rgba.pop();
        assert_eq!(
            validate_vfx_aura_texture_array(&invalid, &limits),
            Err(VfxAuraTextureUploadError::InvalidLayout)
        );
        let mut limited = limits;
        limited.max_texture_dimension_2d = 2;
        assert_eq!(
            validate_vfx_aura_texture_array(&valid, &limited),
            Err(VfxAuraTextureUploadError::DeviceLimitExceeded)
        );
    }

    #[cfg(all(feature = "test-support", not(target_arch = "wasm32")))]
    #[test]
    #[ignore = "requires a native GPU adapter"]
    fn aura_upload_preserves_each_layer_and_authored_mip() {
        crate::test_support::require_gpu_test_opt_in().expect("explicit native GPU test opt-in");
        pollster::block_on(async {
            let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
                backends: wgpu::Backends::PRIMARY,
                ..wgpu::InstanceDescriptor::new_without_display_handle()
            });
            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions::default())
                .await
                .expect("native adapter");
            let (device, queue) = adapter
                .request_device(&wgpu::DeviceDescriptor {
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::downlevel_defaults()
                        .using_resolution(adapter.limits()),
                    memory_hints: wgpu::MemoryHints::Performance,
                    ..Default::default()
                })
                .await
                .expect("native device");
            let packed = packed();
            let uploaded = create_vfx_aura_texture_array(&device, &queue, &packed).unwrap();
            assert_eq!(uploaded.slot_layers, [Some(0), None, Some(2)]);
            for (mip, level) in packed.mips.iter().enumerate() {
                let layer_size = (level.width * level.height * 4) as usize;
                for layer in 0..3 {
                    let readback = device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("Aura layer readback"),
                        size: 256 * level.height as u64,
                        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                        mapped_at_creation: false,
                    });
                    let mut encoder =
                        device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("Aura layer readback encoder"),
                        });
                    encoder.copy_texture_to_buffer(
                        wgpu::TexelCopyTextureInfo {
                            texture: &uploaded.texture,
                            mip_level: mip as u32,
                            origin: wgpu::Origin3d {
                                x: 0,
                                y: 0,
                                z: layer,
                            },
                            aspect: wgpu::TextureAspect::All,
                        },
                        wgpu::TexelCopyBufferInfo {
                            buffer: &readback,
                            layout: wgpu::TexelCopyBufferLayout {
                                offset: 0,
                                bytes_per_row: Some(256),
                                rows_per_image: Some(level.height),
                            },
                        },
                        wgpu::Extent3d {
                            width: level.width,
                            height: level.height,
                            depth_or_array_layers: 1,
                        },
                    );
                    let submission = queue.submit(std::iter::once(encoder.finish()));
                    let slice = readback.slice(..);
                    let (sender, receiver) = std::sync::mpsc::channel();
                    slice.map_async(wgpu::MapMode::Read, move |result| {
                        let _ = sender.send(result);
                    });
                    device
                        .poll(wgpu::PollType::Wait {
                            submission_index: Some(submission),
                            timeout: None,
                        })
                        .expect("readback poll");
                    receiver
                        .recv()
                        .expect("readback callback")
                        .expect("map readback");
                    let mapped = slice.get_mapped_range();
                    for row in 0..level.height as usize {
                        let source_layer = match layer {
                            0 => Some(0),
                            1 => None,
                            _ => Some(1),
                        };
                        let source = source_layer.map(|source_layer| {
                            &level.rgba[source_layer * layer_size + row * level.width as usize * 4
                                ..source_layer * layer_size + (row + 1) * level.width as usize * 4]
                        });
                        let target = &mapped[row * 256..row * 256 + level.width as usize * 4];
                        if let Some(source) = source {
                            assert_eq!(target, source, "mip {mip}, layer {layer}, row {row}");
                        } else {
                            assert!(target.iter().all(|value| *value == 255));
                        }
                    }
                    drop(mapped);
                    readback.unmap();
                }
            }

            let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Aura array sampling test"),
                source: wgpu::ShaderSource::Wgsl(
                    r#"
@group(0) @binding(0) var aura: texture_2d_array<f32>;
@group(0) @binding(1) var aura_sampler: sampler;
@group(0) @binding(2) var<storage, read_write> result: array<vec4<f32>, 9>;

@compute @workgroup_size(1)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let layer = i32(id.x % 3u);
    let mip = f32(id.x / 3u);
    result[id.x] = textureSampleLevel(aura, aura_sampler, vec2<f32>(0.5, 0.5), layer, mip);
}
"#
                    .into(),
                ),
            });
            let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("Aura array sampling test pipeline"),
                layout: None,
                module: &shader,
                entry_point: Some("main"),
                compilation_options: Default::default(),
                cache: None,
            });
            let output = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Aura array sampling output"),
                size: 9 * 16,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            });
            let readback = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Aura array sampling readback"),
                size: 9 * 16,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            let sampler = device.create_sampler(&wgpu::SamplerDescriptor::default());
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Aura array sampling test bind group"),
                layout: &pipeline.get_bind_group_layout(0),
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&uploaded.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: output.as_entire_binding(),
                    },
                ],
            });
            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Aura array sampling test encoder"),
            });
            {
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("Aura array sampling test pass"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(&pipeline);
                pass.set_bind_group(0, &bind_group, &[]);
                pass.dispatch_workgroups(9, 1, 1);
            }
            encoder.copy_buffer_to_buffer(&output, 0, &readback, 0, 9 * 16);
            let submission = queue.submit(std::iter::once(encoder.finish()));
            let slice = readback.slice(..);
            let (sender, receiver) = std::sync::mpsc::channel();
            slice.map_async(wgpu::MapMode::Read, move |result| {
                let _ = sender.send(result);
            });
            device
                .poll(wgpu::PollType::Wait {
                    submission_index: Some(submission),
                    timeout: None,
                })
                .expect("sample poll");
            receiver
                .recv()
                .expect("sample callback")
                .expect("map samples");
            let samples = slice.get_mapped_range();
            for mip in 0..3 {
                for layer in 0..3 {
                    let offset = (mip * 3 + layer) * 16;
                    let value = f32::from_le_bytes(samples[offset..offset + 4].try_into().unwrap());
                    let expected = if layer == 1 {
                        1.0
                    } else {
                        (if layer == 0 { 10 } else { 20 } + mip) as f32 / 255.0
                    };
                    assert!((value - expected).abs() < 1e-6, "mip {mip}, layer {layer}");
                }
            }
            drop(samples);
            readback.unmap();

            let face_texture = create_mipped_rgba_texture(
                &device,
                &queue,
                "face decal array-slot test",
                1,
                1,
                &[128, 64, 32, 255],
                RgbaMipSemantic::SrgbColor,
            );
            let face_view = face_texture.create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::D2Array),
                ..Default::default()
            });
            let face_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("face decal array-slot test bind group"),
                layout: &pipeline.get_bind_group_layout(0),
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&face_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: output.as_entire_binding(),
                    },
                ],
            });
            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("face decal array-slot test encoder"),
            });
            {
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("face decal array-slot test pass"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(&pipeline);
                pass.set_bind_group(0, &face_group, &[]);
                pass.dispatch_workgroups(1, 1, 1);
            }
            encoder.copy_buffer_to_buffer(&output, 0, &readback, 0, 16);
            let submission = queue.submit(std::iter::once(encoder.finish()));
            let slice = readback.slice(..16);
            let (sender, receiver) = std::sync::mpsc::channel();
            slice.map_async(wgpu::MapMode::Read, move |result| {
                let _ = sender.send(result);
            });
            device
                .poll(wgpu::PollType::Wait {
                    submission_index: Some(submission),
                    timeout: None,
                })
                .expect("face decal sample poll");
            receiver
                .recv()
                .expect("face decal sample callback")
                .expect("map face decal sample");
            let samples = slice.get_mapped_range();
            for (channel, srgb) in [128.0_f32, 64.0, 32.0].into_iter().enumerate() {
                let value =
                    f32::from_le_bytes(samples[channel * 4..channel * 4 + 4].try_into().unwrap());
                let encoded = srgb / 255.0;
                let expected = if encoded <= 0.04045 {
                    encoded / 12.92
                } else {
                    ((encoded + 0.055) / 1.055).powf(2.4)
                };
                assert!(
                    (value - expected).abs() < 1e-4,
                    "face decal channel {channel}: sampled {value}, expected {expected}"
                );
            }
            drop(samples);
            readback.unmap();
        });
    }
}

impl VfxBlendMode {
    pub const ALL: [Self; 9] = [
        Self::Blend,
        Self::Multiply,
        Self::Add,
        Self::Subtract,
        Self::Screen,
        Self::Reverse,
        Self::Min,
        Self::Max,
        Self::Opacity,
    ];

    fn from_draw_mode(draw_mode: i32) -> Self {
        use xiv_companion_data::avfx::*;
        match draw_mode {
            DRAW_MODE_BLEND => Self::Blend,
            DRAW_MODE_MULTIPLY | 9 => Self::Multiply,
            DRAW_MODE_SUBTRACT | 11 => Self::Subtract,
            DRAW_MODE_SCREEN | 12 => Self::Screen,
            DRAW_MODE_REVERSE => Self::Reverse,
            DRAW_MODE_MIN => Self::Min,
            DRAW_MODE_MAX => Self::Max,
            DRAW_MODE_OPACITY => Self::Opacity,
            DRAW_MODE_ADD | 10 => Self::Add,
            // Unsupported RMT values retain their raw value in CPU instances and
            // are diagnosed by the parser. Keep the existing Add fallback.
            _ => Self::Add,
        }
    }

    pub fn fragment_entry(self) -> &'static str {
        match self {
            Self::Multiply => "fs_multiply",
            Self::Screen => "fs_screen",
            _ => "fs_blend",
        }
    }

    pub fn soft_fragment_entry(self, msaa: bool) -> &'static str {
        match (self, msaa) {
            (Self::Multiply, false) => "soft_multiply_1x",
            (Self::Screen, false) => "soft_screen_1x",
            (Self::Multiply, true) => "soft_multiply_4x",
            (Self::Screen, true) => "soft_screen_4x",
            (_, false) => "soft_blend_1x",
            (_, true) => "soft_blend_4x",
        }
    }

    pub fn decal_fragment_entry(self) -> &'static str {
        match self {
            Self::Multiply => "fs_decal_multiply",
            Self::Screen => "fs_decal_screen",
            _ => "fs_decal_blend",
        }
    }

    pub fn blend_state(self) -> Option<wgpu::BlendState> {
        use wgpu::{BlendComponent, BlendFactor, BlendOperation};
        let (src_factor, dst_factor, operation) = match self {
            Self::Blend => (
                BlendFactor::SrcAlpha,
                BlendFactor::OneMinusSrcAlpha,
                BlendOperation::Add,
            ),
            Self::Multiply => (BlendFactor::Zero, BlendFactor::Src, BlendOperation::Add),
            Self::Add => (BlendFactor::SrcAlpha, BlendFactor::One, BlendOperation::Add),
            Self::Subtract => (
                BlendFactor::SrcAlpha,
                BlendFactor::One,
                BlendOperation::ReverseSubtract,
            ),
            Self::Screen => (
                BlendFactor::One,
                BlendFactor::OneMinusSrc,
                BlendOperation::Add,
            ),
            Self::Reverse => (
                BlendFactor::OneMinusDst,
                BlendFactor::Zero,
                BlendOperation::Add,
            ),
            Self::Min => (BlendFactor::One, BlendFactor::One, BlendOperation::Min),
            Self::Max => (BlendFactor::One, BlendFactor::One, BlendOperation::Max),
            Self::Opacity => return None,
        };
        Some(wgpu::BlendState {
            color: BlendComponent {
                src_factor,
                dst_factor,
                operation,
            },
            // Preview coverage alpha. Apricot's separate scene-alpha path is
            // not consumed by our RGB-only bloom and opaque postprocess output.
            alpha: BlendComponent {
                src_factor: if self == Self::Blend {
                    BlendFactor::One
                } else {
                    BlendFactor::Zero
                },
                dst_factor: if self == Self::Blend {
                    BlendFactor::OneMinusSrcAlpha
                } else {
                    BlendFactor::One
                },
                operation: BlendOperation::Add,
            },
        })
    }
}

/// GPU procedural particle instance (256 bytes, at the 16-attribute WebGPU limit).
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuVfxQuad {
    /// xyz 位置；w 原始 RBDT（数值传递）。
    pub position_facing: [f32; 4],
    /// xyz 完整变换后的半宽向量；w TD 的 UV 旋转。
    pub axis_x: [f32; 4],
    /// Line/Disc store SPFR and DsSp in xy; their evaluated colors use the endpoint slots.
    pub color: [f32; 4],
    /// 四层贴图的 UV（原点 + 缩放）。
    pub uv: [[f32; 4]; 4],
    /// TD 扭曲贴图采样用 UV。
    pub uvd: [f32; 4],
    /// x：TD 强度的浮点位模式；y：低 4 位目标掩码、高 8 位 TP 偏移；zw：UV 旋转。
    pub distortion: [u32; 4],
    /// xyz 完整变换后的半高向量。
    pub axis_y: [f32; 4],
    /// xyz 枢轴平移；w：0 Quad、1 非 Smpl Powder、2 Smpl、3/4 Windmill、5 Disc、6 Laser、7 Line。
    pub pivot_offset: [f32; 4],
    /// Disc: full Z basis and packed PrtC / PCnU / PCnV. Laser: full Z basis.
    pub disc_axis_z: [f32; 4],
    /// Disc RB / RE / WB / WE. Laser: Len / scaled Wdt.
    pub disc_dimensions: [f32; 4],
    /// Disc HBI / HEI / HBO / HEO.
    pub disc_heights: [f32; 4],
    pub disc_color_inner: [f32; 4],
    /// Disc/Line endpoint color; other shapes store SPFR, DsSp, DpOf, DOTy.
    pub disc_color_outer: [f32; 4],
}

/// Dedicated projection-volume instance for Decal / DecalRing. The client
/// submits a local [-0.5, 0.5] cube and reconstructs scene positions in that
/// volume, so this keeps all three affine axes and their inverse intact.
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuVfxDecalInstance {
    pub world: [[f32; 4]; 4],
    pub inverse_world: [[f32; 4]; 4],
    pub color: [f32; 4],
    pub uv: [[f32; 4]; 4],
    pub uvd: [f32; 4],
    pub distortion: [u32; 4],
    /// Raw SS, distance-corrected WID + WIDR, RF, and 1 for DecalRing (0 for Decal).
    pub projection: [f32; 4],
    /// DDTT followed by reserved fields for the dedicated draw-state path.
    pub metadata: [i32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct GpuVfxDecalVertex {
    pub position: [f32; 4],
}

impl GpuVfxDecalVertex {
    pub const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[wgpu::VertexAttribute {
            format: wgpu::VertexFormat::Float32x4,
            offset: 0,
            shader_location: 0,
        }],
    };
}

pub(crate) const VFX_DECAL_CUBE_VERTICES: [GpuVfxDecalVertex; 8] = [
    GpuVfxDecalVertex {
        position: [-0.5, -0.5, -0.5, 1.0],
    },
    GpuVfxDecalVertex {
        position: [0.5, -0.5, -0.5, 1.0],
    },
    GpuVfxDecalVertex {
        position: [0.5, 0.5, -0.5, 1.0],
    },
    GpuVfxDecalVertex {
        position: [-0.5, 0.5, -0.5, 1.0],
    },
    GpuVfxDecalVertex {
        position: [-0.5, -0.5, 0.5, 1.0],
    },
    GpuVfxDecalVertex {
        position: [0.5, -0.5, 0.5, 1.0],
    },
    GpuVfxDecalVertex {
        position: [0.5, 0.5, 0.5, 1.0],
    },
    GpuVfxDecalVertex {
        position: [-0.5, 0.5, 0.5, 1.0],
    },
];

pub(crate) const VFX_DECAL_CUBE_INDICES: [u16; 36] = [
    0, 2, 1, 0, 3, 2, 4, 5, 6, 4, 6, 7, 0, 1, 5, 0, 5, 4, 3, 7, 6, 3, 6, 2, 0, 4, 7, 0, 7, 3, 1, 2,
    6, 1, 6, 5,
];

impl GpuVfxDecalInstance {
    fn from_quad(quad: &xiv_companion_data::VfxQuad) -> Option<Self> {
        let decal = quad.decal?;
        if !decal.uses_forward_target() {
            return None;
        }
        let mut linear = Mat3::from_cols_array_2d(&quad.parent_basis)
            * Mat3::from_quat(Quat::from_array(quad.orientation))
            * Mat3::from_diagonal(Vec3::new(
                quad.size[0] * 2.0,
                quad.size[1] * 2.0,
                decal.scale_z,
            ));
        let mut ring_width = decal.ring_width;
        if decal.scaling_scale < 1.0 {
            let distance = (linear.x_axis.length() + linear.z_axis.length()) * 0.5;
            if decal.ring {
                ring_width = decal.ring_width
                    * (decal.scaling_scale + (1.0 - decal.scaling_scale) / distance);
            } else {
                let correction = decal.scaling_scale + (1.0 - decal.scaling_scale) / distance;
                linear.x_axis *= correction;
                linear.z_axis *= correction;
            }
        }
        let world = glam::Mat4::from_cols(
            linear.x_axis.extend(0.0),
            linear.y_axis.extend(0.0),
            linear.z_axis.extend(0.0),
            Vec3::from_array(quad.position).extend(1.0),
        );
        let inverse_world = world.inverse();
        if !world.is_finite() || !inverse_world.is_finite() {
            return None;
        }
        let mut uv = [[0.0; 4]; 4];
        for (index, value) in uv.iter_mut().enumerate() {
            *value = [
                quad.uv_origins[index][0],
                quad.uv_origins[index][1],
                quad.uv_scales[index][0],
                quad.uv_scales[index][1],
            ];
        }
        Some(Self {
            world: world.to_cols_array_2d(),
            inverse_world: inverse_world.to_cols_array_2d(),
            color: quad.color,
            uv,
            uvd: [
                quad.uvd_origin[0],
                quad.uvd_origin[1],
                quad.uvd_scale[0],
                quad.uvd_scale[1],
            ],
            distortion: [
                quad.distortion_power.to_bits(),
                quad.distortion_targets | ((palette_offset_byte(quad.palette_offset) as u32) << 8),
                pack_uv_rot(quad.uv_rotations[0], quad.uv_rotations[1]),
                pack_uv_rot(quad.uv_rotations[2], quad.uv_rotations[3]),
            ],
            projection: [
                decal.scaling_scale,
                ring_width,
                decal.ring_fan,
                u8::from(decal.ring) as f32,
            ],
            metadata: [decal.depth_type, 0, 0, 0],
        })
    }
}

impl GpuVfxQuad {
    const ATTRIBUTES: [wgpu::VertexAttribute; 16] = {
        let mut attributes = [wgpu::VertexAttribute {
            format: wgpu::VertexFormat::Float32x4,
            offset: 0,
            shader_location: 0,
        }; 16];
        let mut index = 0;
        while index < 16 {
            attributes[index].offset = (index * 16) as wgpu::BufferAddress;
            attributes[index].shader_location = index as u32;
            index += 1;
        }
        attributes[8].format = wgpu::VertexFormat::Uint32x4;
        attributes
    };

    pub const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &Self::ATTRIBUTES,
    };
}

/// One expanded triangle-list vertex for the camera-facing Polyline path.
/// Stored in a storage buffer so the fixed quad instance layout stays within
/// WebGPU's 16 vertex-attribute limit.
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuVfxPolylineVertex {
    pub center_width: [f32; 4],
    pub previous: [f32; 4],
    pub next: [f32; 4],
    pub width_axis: [f32; 4],
    pub color: [f32; 4],
    pub uv: [[f32; 4]; 4],
    pub uvd: [f32; 4],
    pub distortion: [u32; 4],
    pub base_uv: [f32; 4],
    pub soft_particle_params: [f32; 4],
}

impl GpuVfxPolylineVertex {
    fn new(
        quad: &xiv_companion_data::VfxQuad,
        center: [f32; 3],
        previous: [f32; 3],
        next: [f32; 3],
        not_billboard_axis: Option<[f32; 3]>,
        width_side: f32,
        color: [f32; 4],
        base_uv: [f32; 2],
        uv_precision: i32,
        end_distortion: u8,
    ) -> Self {
        let mut uv = [[0.0; 4]; 4];
        for (index, value) in uv.iter_mut().enumerate() {
            *value = [
                quad.uv_origins[index][0],
                quad.uv_origins[index][1],
                quad.uv_scales[index][0],
                quad.uv_scales[index][1],
            ];
        }
        Self {
            center_width: [center[0], center[1], center[2], width_side],
            previous: [
                previous[0],
                previous[1],
                previous[2],
                f32::from(end_distortion) / 255.0,
            ],
            next: [next[0], next[1], next[2], 0.0],
            width_axis: not_billboard_axis
                .map(|axis| [axis[0], axis[1], axis[2], 1.0])
                .unwrap_or([0.0; 4]),
            color,
            uv,
            uvd: [
                quad.uvd_origin[0],
                quad.uvd_origin[1],
                quad.uvd_scale[0],
                quad.uvd_scale[1],
            ],
            distortion: [
                quad.distortion_power.to_bits(),
                quad.distortion_targets | ((palette_offset_byte(quad.palette_offset) as u32) << 8),
                pack_uv_rot(quad.uv_rotations[0], quad.uv_rotations[1]),
                pack_uv_rot(quad.uv_rotations[2], quad.uv_rotations[3]),
            ],
            base_uv: [
                base_uv[0],
                base_uv[1],
                quad.uvd_rotation,
                uv_precision as f32,
            ],
            soft_particle_params: [
                quad.soft_particle_fade_range,
                f32::from(quad.soft_particle),
                0.0,
                0.0,
            ],
        }
    }
}

impl From<&xiv_companion_data::VfxQuad> for GpuVfxQuad {
    fn from(quad: &xiv_companion_data::VfxQuad) -> Self {
        let parent = Mat3::from_cols_array_2d(&quad.parent_basis);
        let rotation = Mat3::from_quat(Quat::from_array(quad.orientation));
        let linear = if matches!(quad.rotation_direction_base, 3 | 7) {
            rotation
        } else {
            parent * rotation
        };
        let pivot = Vec3::new(
            quad.pivot[0] * quad.size[0],
            quad.pivot[1] * quad.size[1],
            0.0,
        );
        let powder = quad.particle_type == Some(xiv_companion_data::avfx::ParticleType::Powder);
        let pivot_offset = if powder && !quad.powder_single {
            linear * pivot
        } else {
            parent * (pivot - rotation * pivot)
        };
        let geometry = if quad.polygon.is_some() {
            8.0
        } else if quad.line.is_some() {
            7.0
        } else if quad.laser.is_some() {
            6.0
        } else if quad.particle_type == Some(xiv_companion_data::avfx::ParticleType::Windmill) {
            3.0 + f32::from(quad.windmill_uv_type)
        } else if quad.powder_single {
            1.0
        } else if powder {
            2.0
        } else {
            0.0
        };
        let mut uv = [[0.0; 4]; 4];
        for (i, slot) in uv.iter_mut().enumerate() {
            *slot = [
                quad.uv_origins[i][0],
                quad.uv_origins[i][1],
                quad.uv_scales[i][0],
                quad.uv_scales[i][1],
            ];
        }
        let mut result = Self {
            position_facing: [
                quad.position[0],
                quad.position[1],
                quad.position[2],
                quad.rotation_direction_base as f32,
            ],
            axis_x: (linear.x_axis * quad.size[0])
                .extend(quad.uvd_rotation)
                .to_array(),
            color: quad.color,
            uv,
            uvd: [
                quad.uvd_origin[0],
                quad.uvd_origin[1],
                quad.uvd_scale[0],
                quad.uvd_scale[1],
            ],
            distortion: [
                quad.distortion_power.to_bits(),
                quad.distortion_targets | ((palette_offset_byte(quad.palette_offset) as u32) << 8),
                pack_uv_rot(quad.uv_rotations[0], quad.uv_rotations[1]),
                pack_uv_rot(quad.uv_rotations[2], quad.uv_rotations[3]),
            ],
            axis_y: (linear.y_axis * quad.size[1]).extend(0.0).to_array(),
            pivot_offset: pivot_offset.extend(geometry).to_array(),
            disc_axis_z: [0.0; 4],
            disc_dimensions: [0.0; 4],
            disc_heights: [0.0; 4],
            disc_color_inner: [0.0; 4],
            disc_color_outer: [0.0; 4],
        };
        if geometry == 0.0 && matches!(quad.rotation_direction_base, 3 | 7) {
            let facing_parent = Mat3::from_cols_array_2d(&quad.facing_parent_basis);
            result.disc_axis_z = Vec3::from_array(quad.movement_direction)
                .extend(0.0)
                .to_array();
            result.disc_dimensions = facing_parent.y_axis.extend(0.0).to_array();
            result.disc_heights = facing_parent.z_axis.extend(0.0).to_array();
        }
        if let Some(polygon) = quad.polygon {
            result.axis_x = (linear.x_axis * quad.size[0])
                .extend(quad.uvd_rotation)
                .to_array();
            result.axis_y = (linear.y_axis * quad.size[1]).extend(0.0).to_array();
            result.disc_axis_z = (linear.z_axis * (0.5 * polygon.scale_z))
                .extend(polygon.count as f32)
                .to_array();
        } else if let Some(line) = quad.line {
            let (axes, length, facing) = if let Some(offset) = line.endpoint_offset {
                ([Vec3::from_array(offset), Vec3::ZERO, Vec3::ZERO], 1.0, 0.0)
            } else {
                (
                    [
                        linear.x_axis * line.scale[0],
                        linear.y_axis * line.scale[1],
                        linear.z_axis * line.scale[2],
                    ],
                    line.length,
                    quad.rotation_direction_base as f32,
                )
            };
            result.position_facing[3] = facing;
            result.axis_x = axes[0].extend(quad.uvd_rotation).to_array();
            result.axis_y = axes[1].extend(0.0).to_array();
            result.disc_axis_z = axes[2].extend(0.0).to_array();
            result.disc_dimensions = [length, 0.0, 0.0, 0.0];
            result.disc_color_inner = line.color_begin;
            result.disc_color_outer = line.color_end;
        } else if let Some(disc) = quad.disc {
            let axis_x = linear.x_axis * (quad.size[0] * 2.0);
            let axis_y = linear.y_axis * (quad.size[1] * 2.0);
            let axis_z = linear.z_axis * disc.scale_z;
            let average_scale = (axis_x.length() + axis_z.length()) * 0.5;
            // The client reverses radial endpoints in the compensated branch.
            // Collapsed geometry uses a finite local fallback.
            let width_factor = if disc.scaling_scale < 1.0 {
                if average_scale > 0.0 {
                    -(average_scale.recip() + (1.0 - average_scale.recip()) * disc.scaling_scale)
                } else {
                    0.0
                }
            } else {
                1.0
            };
            let packed = u32::from(disc.counts[0])
                | (u32::from(disc.counts[1]) << 8)
                | (u32::from(disc.counts[2]) << 16);
            result.axis_x = axis_x.extend(quad.uvd_rotation).to_array();
            result.axis_y = axis_y.extend(disc.angle).to_array();
            result.pivot_offset = [disc.point_interval_factor, width_factor, 0.0, 5.0];
            result.disc_axis_z = axis_z.extend(packed as f32).to_array();
            result.disc_dimensions = [disc.radius[0], disc.radius[1], disc.width[0], disc.width[1]];
            result.disc_heights = [
                disc.height_inner[0],
                disc.height_inner[1],
                disc.height_outer[0],
                disc.height_outer[1],
            ];
            result.disc_color_inner = std::array::from_fn(|i| quad.color[i] * disc.color_inner[i]);
            result.disc_color_outer = std::array::from_fn(|i| quad.color[i] * disc.color_outer[i]);
        } else if let Some(laser) = quad.laser {
            let laser_linear = linear * Mat3::from_diagonal(Vec3::from_array(laser.scale));
            let axes = [
                laser_linear.x_axis,
                laser_linear.y_axis,
                laser_linear.z_axis,
            ];
            let width_scale = match quad.rotation_direction_base {
                0 => (axes[1].length() + axes[2].length()) * 0.5,
                1 => (axes[0].length() + axes[2].length()) * 0.5,
                2 => (axes[0].length() + axes[1].length()) * 0.5,
                _ => 1.0,
            };
            result.axis_x = axes[0].extend(quad.uvd_rotation).to_array();
            result.axis_y = axes[1].extend(0.0).to_array();
            result.disc_axis_z = axes[2].extend(0.0).to_array();
            result.disc_dimensions = [laser.length, laser.width * width_scale, 0.0, 0.0];
        }
        // Disc/Line read color from their endpoint slots, leaving color.xy
        // available for soft-particle metadata without changing the 16-attribute layout.
        if quad.disc.is_some() || quad.line.is_some() {
            result.color = [
                quad.soft_particle_fade_range,
                f32::from(quad.soft_particle),
                0.0,
                0.0,
            ];
        } else {
            let depth_offset = if matches!(
                quad.particle_type,
                Some(xiv_companion_data::avfx::ParticleType::Quad)
                    | Some(xiv_companion_data::avfx::ParticleType::Powder)
            ) && quad.polygon.is_none()
                && quad.laser.is_none()
            {
                depth_offset_lanes(quad.depth_offset_type, quad.depth_offset)
            } else {
                [0.0; 2]
            };
            result.disc_color_outer = [
                quad.soft_particle_fade_range,
                f32::from(quad.soft_particle),
                depth_offset[0],
                depth_offset[1],
            ];
        }
        result
    }
}

/// 两个 UV 旋转角（弧度）按 2π 周期量化到 u16×2 打包（UvSet `Rot`/`RotR`）。
pub(crate) fn pack_uv_rot(a: f32, b: f32) -> u32 {
    let q = |v: f32| {
        let wrapped = v.rem_euclid(std::f32::consts::TAU);
        (wrapped / std::f32::consts::TAU * 65535.0).round() as u32
    };
    q(a) | (q(b) << 16)
}

/// 一次待上传的 VFX 贴图（RGBA8）；`None` 时使用内置径向光点回退贴图。
#[derive(Clone, Debug)]
pub struct VfxTextureInput {
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
    /// `Some` 表示源 TEX 的精确 mip 链（包含 base level）；`None` 用于
    /// 程序化/合成输入，由渲染器生成完整线性 mip 链。
    pub authored_mips: Option<Vec<VfxTextureMipInput>>,
    /// BC6H_SF16 / R16G16B16A16F 的 source-authored RGBA16F 二维链，优先于 RGBA8 兼容视图。
    pub rgba16f_mips: Option<Vec<VfxTextureMipRgba16fInput>>,
    /// Source-authored cube levels, kept separately from the 2D face-zero view.
    pub cube_mips: Option<Vec<VfxTextureCubeMipInput>>,
    /// Byte layout of every cube face; BC6H uses RGBA16F rather than RGBA8.
    pub cube_format: xiv_companion_data::VfxTextureCubeFormat,
}

/// GPU upload of one ModelSkin TC2/TC3/TD array, with its original slot mapping.
pub struct VfxAuraGpuTexture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub slot_layers: [Option<u32>; 3],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VfxAuraTextureUploadError {
    InvalidLayout,
    DeviceLimitExceeded,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VfxTextureMipInput {
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VfxTextureMipRgba16fInput {
    pub rgba16f: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VfxTextureCubeMipInput {
    pub width: u32,
    pub height: u32,
    pub faces: [Vec<u8>; 6],
}

impl From<&xiv_companion_data::VfxTextureRgba> for VfxTextureInput {
    fn from(texture: &xiv_companion_data::VfxTextureRgba) -> Self {
        let authored_mips = std::iter::once(VfxTextureMipInput {
            rgba: texture.rgba.clone(),
            width: texture.width,
            height: texture.height,
        })
        .chain(texture.mips.iter().map(|level| VfxTextureMipInput {
            rgba: level.rgba.clone(),
            width: level.width,
            height: level.height,
        }))
        .collect();
        Self {
            rgba: texture.rgba.clone(),
            width: texture.width,
            height: texture.height,
            authored_mips: Some(authored_mips),
            rgba16f_mips: (!texture.rgba16f_mips.is_empty()).then(|| {
                texture
                    .rgba16f_mips
                    .iter()
                    .map(|mip| VfxTextureMipRgba16fInput {
                        rgba16f: mip.rgba16f.clone(),
                        width: mip.width,
                        height: mip.height,
                    })
                    .collect()
            }),
            cube_mips: texture.is_cube.then(|| {
                texture
                    .cube_mips
                    .iter()
                    .map(|mip| VfxTextureCubeMipInput {
                        width: mip.width,
                        height: mip.height,
                        faces: mip.faces.clone(),
                    })
                    .collect()
            }),
            cube_format: texture.cube_format,
        }
    }
}

/// 网格静态顶点（96 字节）：位置、四组 UV、顶点色、解码法线与切线。
/// 各贴图层按 TCn 的 `UvSN` 选用对应组基底 UV（UvSet 动画在实例侧）。
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuVfxMeshVertex {
    /// Original homogeneous position, including VDrw's half4 w.
    pub position: [f32; 4],
    /// 四组 UV。
    pub uvs: [[f32; 2]; 4],
    pub color: [f32; 4],
    pub normal: [f32; 4],
    pub tangent: [f32; 4],
}

impl GpuVfxMeshVertex {
    pub const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
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
        ],
    };
}

/// 网格实例存储缓冲（336 字节），与 WESL VfxMeshInstance 布局一致。
/// Storage avoids exceeding WebGPU's default 16 vertex attributes.
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuVfxMeshInstance {
    /// xyz 位置；w 原始 RBDT（数值传递）。
    pub position_facing: [f32; 4],
    /// xyz 变换矩阵的列；axis_x.w 为 TD 的 UV 旋转。
    pub axis_x: [f32; 4],
    /// xyz: linear column; w: FrsT (0 disables the additional color factor).
    pub axis_y: [f32; 4],
    pub axis_z: [f32; 4],
    pub color: [f32; 4],
    pub uv: [[f32; 4]; 4],
    pub uvd: [f32; 4],
    /// x：TD 强度的浮点位模式；y：目标掩码；zw：打包的 UV 旋转角。
    pub distortion: [u32; 4],
    pub fresnel_direction_power: [f32; 4],
    pub fresnel_color_begin: [f32; 4],
    pub fresnel_color_end: [f32; 4],
    /// RBDT 3/7: world movement direction and pre-facing parent Y/Z axes.
    pub movement_direction: [f32; 4],
    pub facing_parent_y: [f32; 4],
    pub facing_parent_z: [f32; 4],
    /// x = SPFR, y = DsSp, z = DpOf, w = DOTy.
    pub soft_particle_params: [f32; 4],
    /// TN UvSet transform: xy scroll/origin, zw scale.
    pub normal_uv: [f32; 4],
    /// x rotation, y NPow, z TR TxNo, w floating-point TP offset.
    pub normal_params: [f32; 4],
    /// x Rate, y RPow, z bUSC, w TR TFT filter mode.
    pub reflection_params: [f32; 4],
}

impl From<&xiv_companion_data::VfxMeshInstance> for GpuVfxMeshInstance {
    fn from(instance: &xiv_companion_data::VfxMeshInstance) -> Self {
        let rotation = Mat3::from_quat(Quat::from_array(instance.orientation));
        let scale = Mat3::from_diagonal(Vec3::from_array(instance.scale));
        let linear = if matches!(instance.rotation_direction_base, 3 | 7) {
            rotation * scale
        } else {
            Mat3::from_cols_array_2d(&instance.parent_basis) * rotation * scale
        };
        let facing_parent = Mat3::from_cols_array_2d(&instance.facing_parent_basis);
        let mut uv = [[0.0; 4]; 4];
        for (i, slot) in uv.iter_mut().enumerate() {
            *slot = [
                instance.uv_origins[i][0],
                instance.uv_origins[i][1],
                instance.uv_scales[i][0],
                instance.uv_scales[i][1],
            ];
        }
        let depth_offset = depth_offset_lanes(instance.depth_offset_type, instance.depth_offset);
        Self {
            position_facing: [
                instance.position[0],
                instance.position[1],
                instance.position[2],
                instance.rotation_direction_base as f32,
            ],
            axis_x: linear.x_axis.extend(instance.uvd_rotation).to_array(),
            axis_y: linear
                .y_axis
                .extend(instance.fresnel.map_or(0.0, |f| f.kind as f32))
                .to_array(),
            axis_z: linear.z_axis.extend(0.0).to_array(),
            color: instance.color,
            uv,
            uvd: [
                instance.uvd_origin[0],
                instance.uvd_origin[1],
                instance.uvd_scale[0],
                instance.uvd_scale[1],
            ],
            distortion: [
                instance.distortion_power.to_bits(),
                instance.distortion_targets,
                pack_uv_rot(instance.uv_rotations[0], instance.uv_rotations[1]),
                pack_uv_rot(instance.uv_rotations[2], instance.uv_rotations[3]),
            ],
            fresnel_direction_power: instance.fresnel.map_or([0.0; 4], |f| {
                [f.direction[0], f.direction[1], f.direction[2], f.exponent]
            }),
            fresnel_color_begin: instance.fresnel.map_or([1.0; 4], |f| f.color_begin),
            fresnel_color_end: instance.fresnel.map_or([1.0; 4], |f| f.color_end),
            movement_direction: Vec3::from_array(instance.movement_direction)
                .extend(0.0)
                .to_array(),
            facing_parent_y: facing_parent.y_axis.extend(0.0).to_array(),
            facing_parent_z: facing_parent.z_axis.extend(0.0).to_array(),
            soft_particle_params: [
                instance.soft_particle_fade_range,
                f32::from(instance.soft_particle),
                depth_offset[0],
                depth_offset[1],
            ],
            normal_uv: [
                instance.normal_uv_origin[0],
                instance.normal_uv_origin[1],
                instance.normal_uv_scale[0],
                instance.normal_uv_scale[1],
            ],
            normal_params: [
                instance.normal_uv_rotation,
                instance.normal_power,
                instance.reflection_texture_index as f32,
                instance.palette_offset,
            ],
            reflection_params: [
                instance.reflection_rate,
                instance.reflection_power,
                f32::from(instance.reflection_use_screen_copy),
                instance.reflection_texture_filter as f32,
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

/// 贴图合成组键：四层贴图序号 + TD + 各层合成模式/边界/bC2A +
/// 各层 UvSet 序号、TC1 启用状态和 apricot_powder 流程。同组粒子共享一个 bind group。
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct VfxGroupKey {
    powder: bool,
    textures: [i32; 4],
    texture_d: i32,
    texture_p: i32,
    texture_n: i32,
    combine_modes: [[i32; 2]; 3],
    color_to_alpha: [bool; 4],
    texture_borders: [[i32; 2]; 4],
    texture_filters: [i32; 4],
    distortion_borders: [i32; 2],
    distortion_filter: i32,
    palette_border: i32,
    palette_filter: i32,
    normal_texture_borders: [i32; 2],
    normal_texture_filter: i32,
    reflection_enabled: bool,
    reflection_use_screen_copy: bool,
    reflection_texture_index: i32,
    reflection_texture_filter: i32,
    reflection_calculate_color: i32,
    texture_uv_sets: [i32; 4],
    distortion_uv_set: i32,
    uv_by_pixel_position: [bool; 4],
    uvd_by_pixel_position: bool,
    normal_uv_by_pixel_position: bool,
    normal_uv_set: i32,
    texture1_enabled: bool,
    texture1_channels: [bool; 2],
}

impl VfxGroupKey {
    fn of_quad(quad: &xiv_companion_data::VfxQuad) -> Self {
        let line = quad.line.is_some();
        Self {
            powder: matches!(
                quad.particle_type,
                Some(
                    xiv_companion_data::avfx::ParticleType::Powder
                        | xiv_companion_data::avfx::ParticleType::Windmill
                )
            ),
            textures: if line { [-1; 4] } else { quad.texture_indexes },
            texture_d: if line {
                -1
            } else {
                quad.texture_distortion_index
            },
            texture_p: if line
                || quad.particle_type.is_some_and(|kind| {
                    matches!(
                        kind,
                        xiv_companion_data::avfx::ParticleType::Powder
                            | xiv_companion_data::avfx::ParticleType::Windmill
                    )
                }) {
                -1
            } else {
                quad.texture_palette_index
            },
            texture_n: -1,
            combine_modes: quad.combine_modes,
            color_to_alpha: quad.color_to_alpha,
            texture_borders: quad.texture_borders,
            texture_filters: quad.texture_filters,
            distortion_borders: quad.distortion_borders,
            distortion_filter: quad.distortion_filter,
            palette_border: quad.palette_border,
            palette_filter: quad.palette_filter,
            normal_texture_borders: [0; 2],
            normal_texture_filter: 1,
            reflection_enabled: false,
            reflection_use_screen_copy: false,
            reflection_texture_index: -1,
            reflection_texture_filter: 1,
            reflection_calculate_color: 0,
            texture_uv_sets: quad.texture_uv_sets,
            distortion_uv_set: quad.distortion_uv_set,
            uv_by_pixel_position: if line {
                [false; 4]
            } else {
                quad.uv_by_pixel_position
            },
            uvd_by_pixel_position: !line && quad.uvd_by_pixel_position,
            normal_uv_by_pixel_position: false,
            normal_uv_set: 0,
            texture1_enabled: !line && quad.texture1_enabled,
            texture1_channels: if line {
                [false; 2]
            } else {
                tc1_channels(quad.combine_mode_tc1, quad.texture1_use_screen_copy)
            },
        }
    }

    fn of_mesh(instance: &xiv_companion_data::VfxMeshInstance) -> Self {
        Self {
            powder: false,
            textures: instance.texture_indexes,
            texture_d: instance.texture_distortion_index,
            texture_p: instance.texture_palette_index,
            texture_n: instance.texture_normal_index,
            combine_modes: instance.combine_modes,
            color_to_alpha: instance.color_to_alpha,
            texture_borders: instance.texture_borders,
            texture_filters: instance.texture_filters,
            distortion_borders: instance.distortion_borders,
            distortion_filter: instance.distortion_filter,
            palette_border: instance.palette_border,
            palette_filter: instance.palette_filter,
            normal_texture_borders: instance.normal_texture_borders,
            normal_texture_filter: instance.normal_texture_filter,
            reflection_enabled: instance.reflection_enabled,
            reflection_use_screen_copy: instance.reflection_use_screen_copy,
            reflection_texture_index: instance.reflection_texture_index,
            reflection_texture_filter: instance.reflection_texture_filter,
            reflection_calculate_color: instance.reflection_calculate_color,
            texture_uv_sets: instance.texture_uv_sets,
            distortion_uv_set: instance.distortion_uv_set,
            uv_by_pixel_position: instance.uv_by_pixel_position,
            uvd_by_pixel_position: instance.uvd_by_pixel_position,
            normal_uv_by_pixel_position: instance.normal_uv_by_pixel_position,
            normal_uv_set: instance.normal_uv_set,
            texture1_enabled: instance.texture1_enabled,
            texture1_channels: tc1_channels(
                instance.combine_mode_tc1,
                instance.texture1_use_screen_copy,
            ),
        }
    }
}

fn tc1_channels([color, alpha]: [i32; 2], screen_copy: bool) -> [bool; 2] {
    [color & 7 != 0, alpha & 3 != 0 && !screen_copy]
}

fn key_uses_screen_copy(key: VfxGroupKey) -> bool {
    matches!(key.textures[0], -2 | -3) || key.reflection_use_screen_copy
}

fn palette_offset_byte(offset: f32) -> u8 {
    (offset * 255.0).round().clamp(0.0, 255.0) as u8
}

fn depth_offset_lanes(depth_offset_type: i32, depth_offset: f32) -> [f32; 2] {
    if matches!(depth_offset_type, 0 | 1) && depth_offset.is_finite() {
        [depth_offset, depth_offset_type as f32]
    } else {
        [0.0; 2]
    }
}

/// 组参数 uniform（与 `vfx.wesl` 的 `VfxGroupParams` 布局一致；
/// WGSL vec3<u32>/vec4<f32> 按 16 字节对齐，Rust 侧显式补齐到 96 字节）。
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct VfxGroupParams {
    /// TC2/TC3/TC4 的颜色合成模式。
    combine_color: [u32; 3],
    _pad0: u32,
    /// TC2/TC3/TC4 的 alpha 合成模式。
    combine_alpha: [u32; 3],
    _pad1: u32,
    /// TC1 RGB / alpha channel enables, independent of TC2..TC4 combine modes.
    texture1_channels: [u32; 2],
    /// 各层 bC2A 位掩码。
    color_to_alpha: u32,
    /// bit1..4: textures; bit5: TD; bit6: TC1; bit7: Powder; bit8: TP.
    flags: u32,
    /// 各层 UvSet 序号（2bit × TC1..TC4）+ TD（2bit，bit8-9）。
    uv_set_sel: u32,
    /// bit0..3 select normalized fragment pixel position for TC1..TC4; bit4 for TD.
    pixel_uv_mask: u32,
    _pad2: [u32; 2],
    /// TD / TP / TN sampler mip LOD bias; final lane is reserved.
    auxiliary_lod_bias: [f32; 4],
    /// TC1..TC4 sampler mip LOD bias.
    texture_lod_bias: [f32; 4],
}

fn vfx_texture_lod_bias(filter: i32) -> f32 {
    match filter {
        1 => -0.5,
        2 => -1.0,
        3 => -1.5,
        4 => -2.0,
        _ => 0.0,
    }
}

pub(super) fn vfx_sampler_descriptor(
    modes: [i32; 2],
    filter: i32,
) -> wgpu::SamplerDescriptor<'static> {
    let address_mode = |mode: i32| match mode {
        1 => wgpu::AddressMode::ClampToEdge,
        2 => wgpu::AddressMode::MirrorRepeat,
        _ => wgpu::AddressMode::Repeat,
    };
    let linear = filter != 0;
    let filter_mode = if linear {
        wgpu::FilterMode::Linear
    } else {
        wgpu::FilterMode::Nearest
    };
    let anisotropy_clamp = match filter {
        2 => 4,
        3 => 8,
        4 => 16,
        _ => 1,
    };
    wgpu::SamplerDescriptor {
        label: Some("weapon vfx group sampler"),
        address_mode_u: address_mode(modes[0]),
        address_mode_v: address_mode(modes[1]),
        address_mode_w: wgpu::AddressMode::Repeat,
        mag_filter: filter_mode,
        min_filter: filter_mode,
        // The client enables linear interpolation between mip levels for every
        // TFT mode, including TFT=0's point min/mag filtering.
        mipmap_filter: wgpu::MipmapFilterMode::Linear,
        anisotropy_clamp,
        ..Default::default()
    }
}

/// 一条绘制段：混合、贴图、深度及剔除状态相同的连续实例区间。
#[derive(Clone, Copy, Debug)]
pub(crate) struct VfxDrawRange {
    pub draw_layer: i32,
    pub priority: i32,
    pub draw_order: Option<u64>,
    pub last_draw_order: Option<u64>,
    pub blend_mode: VfxBlendMode,
    pub depth_mode: usize,
    pub cull_mode: i32,
    /// Quad Double batches cannot span different Ptcl definitions.
    pub double_group: Option<usize>,
    /// Whether this range uses the dedicated two-vertex line-list pipeline.
    pub line_list: bool,
    /// Quad/Powder instances that must be drawn after the scene depth pass.
    pub soft_particle: bool,
    pub screen_copy: bool,
    /// Two line-list vertices for Line, six triangle-list vertices for quads,
    /// 18 for Laser, or 24 for Windmill.
    pub vertex_count: u32,
    pub group: usize,
    pub start: u32,
    pub count: u32,
}

/// 网格绘制段：携带模型与原始 CulT（0 双面、1 留正面、2 留背面、3 先背后正）。
#[derive(Clone, Copy, Debug)]
pub(crate) struct VfxMeshDrawRange {
    pub draw_layer: i32,
    pub priority: i32,
    pub draw_order: Option<u64>,
    pub last_draw_order: Option<u64>,
    pub mesh: usize,
    pub blend_mode: VfxBlendMode,
    pub depth_mode: usize,
    pub cull_mode: i32,
    pub soft_particle: bool,
    pub screen_copy: bool,
    pub group: usize,
    pub start: u32,
    pub count: u32,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct VfxPolylineDrawRange {
    pub draw_layer: i32,
    pub priority: i32,
    pub draw_order: Option<u64>,
    pub last_draw_order: Option<u64>,
    pub blend_mode: VfxBlendMode,
    pub depth_mode: usize,
    pub cull_mode: i32,
    pub soft_particle: bool,
    pub screen_copy: bool,
    pub group: usize,
    pub start: u32,
    pub count: u32,
}

/// Prepared Decal instances are deliberately not submitted by the ordinary
/// scene pass. They require a later pass with a sampleable scene depth view.
#[derive(Clone, Copy, Debug)]
pub(crate) struct VfxDecalDrawRange {
    pub draw_layer: i32,
    pub priority: i32,
    pub draw_order: Option<u64>,
    pub last_draw_order: Option<u64>,
    pub blend_mode: VfxBlendMode,
    pub depth_type: i32,
    pub ring: bool,
    pub screen_copy: bool,
    pub group: usize,
    pub start: u32,
    pub count: u32,
}

/// Preview registration of one mounted AVFX Document. `order_end` is exclusive;
/// the gap between adjacent ranges prevents GPU batching across Documents.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct VfxDocumentSortRange {
    pub order_start: u64,
    pub order_end: u64,
    pub position: [f32; 3],
    pub soft_key_offset: f32,
    pub registration_serial: u32,
}

/// `DsDt`/`DsDw` → 深度模式：0 测不写、1 测且写、2 不测不写。
/// D3D11 DepthEnable=false also suppresses writes when DsDw is set.
pub(crate) fn vfx_depth_mode(depth_test: bool, depth_write: bool) -> usize {
    if !depth_test {
        2
    } else if depth_write {
        1
    } else {
        0
    }
}

/// Client CulT 1/2 retain the named face; pipeline slots 1/2 cull front/back.
/// Verified Double paths select their two passes at draw time.
pub(crate) fn vfx_cull_index(cull_mode: i32) -> usize {
    match cull_mode {
        1 => 2,
        2 => 1,
        _ => 0,
    }
}

fn draw_orders_share_run(previous: Option<u64>, next: Option<u64>) -> bool {
    match (previous, next) {
        // A single sampling event can emit several Powder children.
        (Some(previous), Some(next)) => next == previous || previous.checked_add(1) == Some(next),
        (None, None) => true,
        _ => false,
    }
}

/// Reproduce the client's confirmed soft-particle alpha stage.
///
/// The `apricot_powder` `ps24` shader reconstructs the scene position from
/// screen UV + sampled depth with the inverse view-projection matrix, compares
/// it with the particle position, and applies `min(distance / SPFR, 1)^2` to
/// alpha. Keeping this as a pure function makes the formula testable before a
/// separate sampleable depth pass is connected to the ordinary VFX pass.
pub(crate) fn soft_particle_alpha(
    screen_uv: [f32; 2],
    scene_depth: f32,
    particle_clip_position: [f32; 4],
    inverse_view_projection: [[f32; 4]; 4],
    fade_range: f32,
    base_alpha: f32,
) -> f32 {
    let inverse = Mat4::from_cols_array_2d(&inverse_view_projection);
    let scene_ndc = Vec4::new(
        screen_uv[0] * 2.0 - 1.0,
        screen_uv[1] * -2.0 + 1.0,
        scene_depth,
        1.0,
    );
    let scene_clip = inverse * scene_ndc;
    let scene_world = scene_clip.truncate() / scene_clip.w;
    let particle_clip = Vec4::from_array(particle_clip_position);
    let particle_world = particle_clip.truncate() / particle_clip.w;
    let normalized_distance = scene_world.distance(particle_world) / fade_range;
    let fade = normalized_distance.min(1.0).powi(2);
    (base_alpha * fade).clamp(0.0, 1.0)
}

fn vfx_quad_double_group(quad: &xiv_companion_data::VfxQuad) -> Option<usize> {
    (quad.cull_mode == 3
        && matches!(
            quad.particle_type,
            Some(
                xiv_companion_data::avfx::ParticleType::Quad
                    | xiv_companion_data::avfx::ParticleType::Laser
            )
        ))
    .then_some(quad.particle_index)
}

fn vfx_quad_vertex_count(quad: &xiv_companion_data::VfxQuad) -> u32 {
    if let Some(polygon) = quad.polygon {
        polygon.vertex_count()
    } else if quad.line.is_some() {
        2
    } else if let Some(disc) = quad.disc {
        disc.vertex_count()
    } else if quad.particle_type == Some(xiv_companion_data::avfx::ParticleType::Windmill) {
        24
    } else if quad.laser.is_some() {
        18
    } else {
        6
    }
}

fn gpu_polyline_vertices(
    quad: &xiv_companion_data::VfxQuad,
    polyline: xiv_companion_data::VfxPolyline,
) -> Vec<GpuVfxPolylineVertex> {
    // Client helper 0x1403f47a0 connects two edge/regular/edge cross-sections.
    const EDGE_INDICES: [usize; 12] = [0, 1, 4, 4, 3, 0, 1, 2, 4, 2, 5, 4];
    const NON_EDGE_INDICES: [usize; 6] = [0, 1, 3, 3, 2, 0];
    let points = polyline.points();
    let widths = polyline.point_widths();
    let colors = polyline.point_colors();
    let end_distortion = polyline.point_end_distortion();
    let columns = if polyline.use_edge { 3 } else { 2 };
    let vertices_per_segment = if polyline.use_edge {
        EDGE_INDICES.len()
    } else {
        NON_EDGE_INDICES.len()
    };
    let mut vertices = Vec::with_capacity(points.len().saturating_sub(1) * vertices_per_segment);
    for segment in 0..points.len() - 1 {
        let mut control = [GpuVfxPolylineVertex::zeroed(); 6];
        for endpoint in 0..2 {
            let output_index = segment + endpoint;
            let source_index = if polyline.reverse_points {
                points.len() - 1 - output_index
            } else {
                output_index
            };
            let source_at = |output_index: usize| {
                let source_index = if polyline.reverse_points {
                    points.len() - 1 - output_index
                } else {
                    output_index
                };
                points[source_index]
            };
            let previous = source_at(output_index.saturating_sub(1));
            let next = source_at((output_index + 1).min(points.len() - 1));
            let along = output_index as f32 / (points.len() - 1) as f32 * polyline.uv_span;
            for side in 0..columns {
                let side_factor = if polyline.use_edge {
                    side as f32 - 1.0
                } else {
                    side as f32 * 2.0 - 1.0
                };
                let color = if polyline.use_edge {
                    colors[source_index][side]
                } else {
                    colors[source_index][1]
                };
                control[endpoint * columns + side] = GpuVfxPolylineVertex::new(
                    quad,
                    points[source_index],
                    previous,
                    next,
                    polyline.not_billboard_axis,
                    widths[source_index] * side_factor,
                    color,
                    [side as f32 / (columns - 1) as f32, along],
                    polyline.uv_precision,
                    end_distortion[source_index],
                );
            }
        }
        if polyline.use_edge {
            vertices.extend(EDGE_INDICES.map(|index| control[index]));
        } else {
            vertices.extend(NON_EDGE_INDICES.map(|index| control[index]));
        }
    }
    vertices
}

/// 常驻 VFX 粒子批次：由 `ModelRenderContext::create_vfx_particles` 构建，
/// 每帧 `update`/`update_mesh` 后随 `render(..., vfx: Some(&batch))` 绘制。
/// bind group 按组键懒创建并缓存。
pub struct VfxParticles {
    pub(crate) instance_buffer: wgpu::Buffer,
    /// 贴图视图表：0 号为内置回退光点，其后按文件 `Tex` 顺序（序号+1）。
    pub(crate) texture_views: Vec<wgpu::TextureView>,
    pub(crate) cube_texture_views: Vec<Option<wgpu::TextureView>>,
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
    pub(crate) polyline_buffer: wgpu::Buffer,
    pub(crate) polyline_draw_ranges: Vec<VfxPolylineDrawRange>,
    pub(crate) polyline_vertex_count: usize,
    pub(crate) decal_instance_buffer: wgpu::Buffer,
    pub(crate) decal_instance_count: usize,
    pub(crate) decal_draw_ranges: Vec<VfxDecalDrawRange>,
    pub(crate) document_sort_ranges: Vec<VfxDocumentSortRange>,
}

impl VfxParticles {
    pub fn count(&self) -> usize {
        self.count
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Number of valid projection-volume instances prepared for the dedicated
    /// depth-sampling pass. They are not drawn through the quad fallback.
    pub fn decal_count(&self) -> usize {
        self.decal_instance_count
    }

    pub(crate) fn instance_slice(&self) -> wgpu::BufferSlice<'_> {
        self.instance_buffer.slice(..)
    }

    pub(crate) fn uses_screen_copy(&self) -> bool {
        self.draw_ranges.iter().any(|range| range.screen_copy)
            || self
                .polyline_draw_ranges
                .iter()
                .any(|range| range.screen_copy)
            || self.mesh_draw_ranges.iter().any(|range| range.screen_copy)
            || self.decal_draw_ranges.iter().any(|range| range.screen_copy)
    }

    /// 组键 → bind group 下标（懒创建）。
    pub(crate) fn group_for(&mut self, context: &ModelRenderContext, key: VfxGroupKey) -> usize {
        if let Some(index) = self.group_cache.get(&key) {
            return *index;
        }
        let view_for = |index: i32| -> &wgpu::TextureView {
            usize::try_from(index)
                .ok()
                .and_then(|index| self.texture_views.get(index + 1))
                .unwrap_or(&self.texture_views[0])
        };
        let file_cube = (!key.reflection_use_screen_copy && key.reflection_enabled)
            .then(|| usize::try_from(key.reflection_texture_index).ok())
            .flatten()
            .and_then(|index| self.cube_texture_views.get(index))
            .and_then(Option::as_ref);
        let mut flags = 0u32;
        for (i, texture) in key.textures.iter().enumerate() {
            // Apricot Powder config excludes TC2..TC4 and TD.
            if !key.powder || i == 0 {
                let builtin = i == 0
                    && (matches!(*texture, -2 | -3)
                        || (*texture == -5 && context.vfx_portrait_available()));
                flags |= ((*texture >= 0 || builtin) as u32) << (i + 1);
            }
        }
        flags |= ((!key.powder && key.texture_d >= 0) as u32) << 5;
        flags |= (key.texture1_enabled as u32) << 6;
        flags |= (key.powder as u32) << 7;
        flags |= ((!key.powder && key.texture_p >= 0) as u32) << 8;
        flags |= ((key.textures[0] == -2) as u32) << 9;
        flags |= ((key.textures[0] == -3) as u32) << 10;
        flags |= ((key.texture_n >= 0) as u32) << 11;
        flags |= (key.reflection_enabled as u32) << 12;
        // The client packs TCCT with `shr 17; and 7`: preserve the low three
        // bits, including unknown values, instead of clamping them to a mode.
        flags |= ((key.reflection_calculate_color as u32) & 7) << 13;
        flags |= ((key.textures[0] == -5 && context.vfx_portrait_available()) as u32) << 16;
        flags |= (file_cube.is_some() as u32) << 17;
        // 每层 2bit 的 UvSet 序号（钳到 0..3）；TD 在 bit8-9。
        let mut uv_set_sel = 0u32;
        for (i, set) in key.texture_uv_sets.iter().enumerate() {
            uv_set_sel |= ((*set).clamp(0, 3) as u32) << (i * 2);
        }
        uv_set_sel |= (key.distortion_uv_set.clamp(0, 3) as u32) << 8;
        uv_set_sel |= (key.normal_uv_set.clamp(0, 3) as u32) << 10;
        let pixel_uv_mask = key
            .uv_by_pixel_position
            .iter()
            .enumerate()
            .fold(0u32, |mask, (index, enabled)| {
                mask | (u32::from(*enabled) << index)
            })
            | (u32::from(key.uvd_by_pixel_position) << 4)
            | (u32::from(key.normal_uv_by_pixel_position) << 5);
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
            texture1_channels: key.texture1_channels.map(u32::from),
            uv_set_sel,
            pixel_uv_mask,
            _pad2: [0; 2],
            auxiliary_lod_bias: [
                vfx_texture_lod_bias(key.distortion_filter),
                vfx_texture_lod_bias(key.palette_filter),
                vfx_texture_lod_bias(key.normal_texture_filter),
                vfx_texture_lod_bias(key.reflection_texture_filter),
            ],
            texture_lod_bias: key.texture_filters.map(vfx_texture_lod_bias),
        };
        let uniform = context
            .device()
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("weapon vfx group params"),
                contents: bytemuck::bytes_of(&params),
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let make_sampler = |(modes, filter): ([i32; 2], i32)| {
            context
                .device()
                .create_sampler(&vfx_sampler_descriptor(modes, filter))
        };
        let samplers: Vec<wgpu::Sampler> = key
            .texture_borders
            .into_iter()
            .zip(key.texture_filters)
            .chain(std::iter::once((
                key.distortion_borders,
                key.distortion_filter,
            )))
            .chain(std::iter::once((
                [key.palette_border; 2],
                key.palette_filter,
            )))
            .chain(std::iter::once((
                key.normal_texture_borders,
                key.normal_texture_filter,
            )))
            .map(make_sampler)
            .collect();
        let reflection_sampler = make_sampler(([1, 1], key.reflection_texture_filter));
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
            wgpu::BindGroupEntry {
                binding: 11,
                resource: self.mesh_instance_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 12,
                resource: wgpu::BindingResource::TextureView(view_for(key.texture_p)),
            },
            wgpu::BindGroupEntry {
                binding: 14,
                resource: self.polyline_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 15,
                resource: wgpu::BindingResource::TextureView(view_for(key.texture_n)),
            },
            wgpu::BindGroupEntry {
                binding: 17,
                resource: wgpu::BindingResource::TextureView(
                    file_cube.unwrap_or(context.vfx_reflection_cube_view()),
                ),
            },
            wgpu::BindGroupEntry {
                binding: 18,
                resource: wgpu::BindingResource::Sampler(&reflection_sampler),
            },
        ];
        for (i, sampler) in samplers.iter().enumerate() {
            entries.push(wgpu::BindGroupEntry {
                binding: match i {
                    0..=4 => (6 + i) as u32,
                    5 => 13,
                    _ => 16,
                },
                resource: wgpu::BindingResource::Sampler(sampler),
            });
        }
        let bind_group = context
            .device()
            .create_bind_group(&wgpu::BindGroupDescriptor {
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

    /// 增量上传实例数据（每帧调用；超过容量截断）。按 DwPr 稳定排序，
    /// 同优先级保持输入顺序，只合并状态相同的相邻实例。
    pub fn update(&mut self, context: &ModelRenderContext, quads: &[xiv_companion_data::VfxQuad]) {
        self.count = quads.len().min(self.capacity);
        self.draw_ranges.clear();
        self.polyline_draw_ranges.clear();
        self.polyline_vertex_count = 0;
        self.decal_draw_ranges.clear();
        self.decal_instance_count = 0;
        if self.count == 0 {
            return;
        }
        let mut ordered: Vec<_> = quads[..self.count]
            .iter()
            .filter(|quad| quad.polyline.is_none() && quad.decal.is_none())
            .map(|quad| {
                (
                    VfxBlendMode::from_draw_mode(quad.draw_mode),
                    (quad.draw_priority, quad.draw_layer),
                    quad.draw_order,
                    vfx_depth_mode(quad.depth_test, quad.depth_write),
                    // Powder/Windmill/Disc submit one draw and pass 3
                    // directly to the state setter, which selects CullBack.
                    if quad.cull_mode == 3
                        && matches!(
                            quad.particle_type,
                            Some(
                                xiv_companion_data::avfx::ParticleType::Powder
                                    | xiv_companion_data::avfx::ParticleType::Windmill
                                    | xiv_companion_data::avfx::ParticleType::Disc
                                    | xiv_companion_data::avfx::ParticleType::Polygon
                            )
                        )
                    {
                        1
                    } else {
                        quad.cull_mode
                    },
                    vfx_quad_double_group(quad),
                    quad.line.is_some(),
                    quad.soft_particle,
                    vfx_quad_vertex_count(quad),
                    VfxGroupKey::of_quad(quad),
                    GpuVfxQuad::from(quad),
                )
            })
            .collect();
        ordered.sort_by_key(|(_, (priority, _), _, _, _, _, _, _, _, _, _)| *priority);
        context.queue().write_buffer(
            &self.instance_buffer,
            0,
            bytemuck::cast_slice(
                &ordered
                    .iter()
                    .map(|(_, _, _, _, _, _, _, _, _, _, gpu)| *gpu)
                    .collect::<Vec<_>>(),
            ),
        );
        let mut start = 0_u32;
        for (
            blend_mode,
            (priority, draw_layer),
            draw_order,
            depth_mode,
            cull_mode,
            double_group,
            line_list,
            soft_particle,
            vertex_count,
            key,
            _,
        ) in &ordered
        {
            let group = self.group_for(context, *key);
            match self.draw_ranges.last_mut() {
                Some(range)
                    if range.priority == *priority
                        && range.draw_layer == *draw_layer
                        && draw_orders_share_run(range.last_draw_order, *draw_order)
                        && range.blend_mode == *blend_mode
                        && range.depth_mode == *depth_mode
                        && range.cull_mode == *cull_mode
                        && range.double_group == *double_group
                        && range.line_list == *line_list
                        && range.soft_particle == *soft_particle
                        && range.screen_copy == key_uses_screen_copy(*key)
                        && range.vertex_count == *vertex_count
                        // Client flushes before adding the 4096th streamed quad.
                        && (double_group.is_none() || range.count < 4095)
                        && range.group == group
                        && range.start + range.count == start =>
                {
                    range.count += 1;
                    range.last_draw_order = *draw_order;
                }
                _ => self.draw_ranges.push(VfxDrawRange {
                    draw_layer: *draw_layer,
                    priority: *priority,
                    draw_order: *draw_order,
                    last_draw_order: *draw_order,
                    blend_mode: *blend_mode,
                    depth_mode: *depth_mode,
                    cull_mode: *cull_mode,
                    double_group: *double_group,
                    line_list: *line_list,
                    soft_particle: *soft_particle,
                    screen_copy: key_uses_screen_copy(*key),
                    vertex_count: *vertex_count,
                    group,
                    start,
                    count: 1,
                }),
            }
            start += 1;
        }

        let mut polylines: Vec<_> = quads[..self.count]
            .iter()
            .filter_map(|quad| {
                quad.polyline.map(|polyline| {
                    (
                        VfxBlendMode::from_draw_mode(quad.draw_mode),
                        (quad.draw_priority, quad.draw_layer),
                        quad.draw_order,
                        vfx_depth_mode(quad.depth_test, quad.depth_write),
                        quad.cull_mode,
                        quad.soft_particle,
                        VfxGroupKey::of_quad(quad),
                        quad,
                        polyline,
                    )
                })
            })
            .collect();
        polylines.sort_by_key(|(_, (priority, _), _, _, _, _, _, _, _)| *priority);
        let mut vertices = Vec::new();
        for (
            blend_mode,
            (priority, draw_layer),
            draw_order,
            depth_mode,
            cull_mode,
            soft_particle,
            key,
            quad,
            polyline,
        ) in polylines
        {
            let generated = gpu_polyline_vertices(quad, polyline);
            let required = generated.len();
            if vertices.len() + required > ModelRenderContext::VFX_POLYLINE_VERTEX_CAPACITY {
                continue;
            }
            let range_start = vertices.len() as u32;
            vertices.extend(generated);
            let range_count = vertices.len() as u32 - range_start;
            let group = self.group_for(context, key);
            match self.polyline_draw_ranges.last_mut() {
                Some(range)
                    if range.priority == priority
                        && range.draw_layer == draw_layer
                        && draw_orders_share_run(range.last_draw_order, draw_order)
                        && range.blend_mode == blend_mode
                        && range.depth_mode == depth_mode
                        && range.cull_mode == cull_mode
                        && range.soft_particle == soft_particle
                        && range.screen_copy == key_uses_screen_copy(key)
                        && range.group == group
                        && range.start + range.count == range_start =>
                {
                    range.count += range_count;
                    range.last_draw_order = draw_order;
                }
                _ => self.polyline_draw_ranges.push(VfxPolylineDrawRange {
                    draw_layer,
                    priority,
                    draw_order,
                    last_draw_order: draw_order,
                    blend_mode,
                    depth_mode,
                    cull_mode,
                    soft_particle,
                    screen_copy: key_uses_screen_copy(key),
                    group,
                    start: range_start,
                    count: range_count,
                }),
            }
        }
        self.polyline_vertex_count = vertices.len();
        if !vertices.is_empty() {
            context
                .queue()
                .write_buffer(&self.polyline_buffer, 0, bytemuck::cast_slice(&vertices));
        }

        let mut decals: Vec<_> = quads[..self.count]
            .iter()
            .filter_map(|quad| {
                let decal = quad.decal?;
                Some((
                    (quad.draw_priority, quad.draw_layer),
                    quad.draw_order,
                    VfxBlendMode::from_draw_mode(quad.draw_mode),
                    decal.depth_type,
                    decal.ring,
                    VfxGroupKey::of_quad(quad),
                    GpuVfxDecalInstance::from_quad(quad)?,
                ))
            })
            .collect();
        // Independent registration order; DwPr does not sort the Decal list.
        decals.sort_by_key(|(_, draw_order, _, _, _, _, _)| *draw_order);
        self.decal_instance_count = decals.len();
        if !decals.is_empty() {
            context.queue().write_buffer(
                &self.decal_instance_buffer,
                0,
                bytemuck::cast_slice(
                    &decals
                        .iter()
                        .map(|(_, _, _, _, _, _, instance)| *instance)
                        .collect::<Vec<_>>(),
                ),
            );
        }
        for (start, ((priority, draw_layer), draw_order, blend_mode, depth_type, ring, key, _)) in
            decals.into_iter().enumerate()
        {
            let group = self.group_for(context, key);
            match self.decal_draw_ranges.last_mut() {
                Some(range)
                    if range.priority == priority
                        && range.draw_layer == draw_layer
                        && draw_orders_share_run(range.last_draw_order, draw_order)
                        && range.blend_mode == blend_mode
                        && range.depth_type == depth_type
                        && range.ring == ring
                        && range.screen_copy == key_uses_screen_copy(key)
                        && range.group == group
                        && range.start + range.count == start as u32 =>
                {
                    range.count += 1;
                    range.last_draw_order = draw_order;
                }
                _ => self.decal_draw_ranges.push(VfxDecalDrawRange {
                    draw_layer,
                    priority,
                    draw_order,
                    last_draw_order: draw_order,
                    blend_mode,
                    depth_type,
                    ring,
                    screen_copy: key_uses_screen_copy(key),
                    group,
                    start: start as u32,
                    count: 1,
                }),
            }
        }
    }

    /// 增量上传网格粒子实例并按 DwPr 稳定排序，只合并相邻兼容实例。
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
        let mut ordered: Vec<(
            VfxBlendMode,
            (i32, i32),
            Option<u64>,
            usize,
            i32,
            bool,
            usize,
            VfxGroupKey,
            GpuVfxMeshInstance,
        )> = instances[..self.mesh_instance_count]
            .iter()
            .map(|instance| {
                (
                    VfxBlendMode::from_draw_mode(instance.draw_mode),
                    (instance.draw_priority, instance.draw_layer),
                    instance.draw_order,
                    vfx_depth_mode(instance.depth_test, instance.depth_write),
                    instance.cull_mode,
                    instance.soft_particle,
                    instance.model_index,
                    VfxGroupKey::of_mesh(instance),
                    GpuVfxMeshInstance::from(instance),
                )
            })
            .collect();
        ordered.sort_by_key(|(_, (priority, _), _, _, _, _, _, _, _)| *priority);
        context.queue().write_buffer(
            &self.mesh_instance_buffer,
            0,
            bytemuck::cast_slice(
                &ordered
                    .iter()
                    .map(|(_, _, _, _, _, _, _, _, gpu)| *gpu)
                    .collect::<Vec<_>>(),
            ),
        );
        let mut start = 0_u32;
        for (
            blend_mode,
            (priority, draw_layer),
            draw_order,
            depth_mode,
            cull_mode,
            soft_particle,
            mesh,
            key,
            _,
        ) in &ordered
        {
            let group = self.group_for(context, *key);
            match self.mesh_draw_ranges.last_mut() {
                Some(range)
                    if range.priority == *priority
                        && range.draw_layer == *draw_layer
                        && draw_orders_share_run(range.last_draw_order, *draw_order)
                        && range.mesh == *mesh
                        && range.blend_mode == *blend_mode
                        && range.depth_mode == *depth_mode
                        && range.cull_mode == *cull_mode
                        && range.soft_particle == *soft_particle
                        && range.screen_copy == key_uses_screen_copy(*key)
                        && range.group == group
                        && range.start + range.count == start =>
                {
                    range.count += 1;
                    range.last_draw_order = *draw_order;
                }
                _ => self.mesh_draw_ranges.push(VfxMeshDrawRange {
                    draw_layer: *draw_layer,
                    priority: *priority,
                    draw_order: *draw_order,
                    last_draw_order: *draw_order,
                    mesh: *mesh,
                    blend_mode: *blend_mode,
                    depth_mode: *depth_mode,
                    cull_mode: *cull_mode,
                    soft_particle: *soft_particle,
                    screen_copy: key_uses_screen_copy(*key),
                    group,
                    start,
                    count: 1,
                }),
            }
            start += 1;
        }
    }
}

/// Uploads the validated CPU array as one filterable RGBA8 2D-array resource.
impl ModelRenderContext {
    pub fn upload_vfx_aura_texture_array(
        &self,
        array: &xiv_companion_data::VfxAuraTextureArrayRgba,
    ) -> Result<VfxAuraGpuTexture, VfxAuraTextureUploadError> {
        create_vfx_aura_texture_array(self.device(), self.queue(), array)
    }
}

fn create_vfx_aura_texture_array(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    array: &xiv_companion_data::VfxAuraTextureArrayRgba,
) -> Result<VfxAuraGpuTexture, VfxAuraTextureUploadError> {
    validate_vfx_aura_texture_array(array, &device.limits())?;
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("weapon ModelSkin Aura texture array"),
        size: wgpu::Extent3d {
            width: array.width,
            height: array.height,
            depth_or_array_layers: 3,
        },
        mip_level_count: array.mips.len() as u32,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_DST
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    for (mip, level) in array.mips.iter().enumerate() {
        // Keep shader layer numbers equal to the client's TC2/TC3/TD slots.
        let layer_size = (level.width * level.height * 4) as usize;
        let mut rgba = vec![255; layer_size * 3];
        for (slot, source_layer) in array.slot_layers.iter().enumerate() {
            if let Some(source_layer) = source_layer {
                rgba[slot * layer_size..(slot + 1) * layer_size].copy_from_slice(
                    &level.rgba[*source_layer as usize * layer_size
                        ..(*source_layer as usize + 1) * layer_size],
                );
            }
        }
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: mip as u32,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(level.width * 4),
                rows_per_image: Some(level.height),
            },
            wgpu::Extent3d {
                width: level.width,
                height: level.height,
                depth_or_array_layers: 3,
            },
        );
    }
    let view = texture.create_view(&wgpu::TextureViewDescriptor {
        label: Some("weapon ModelSkin Aura texture array view"),
        dimension: Some(wgpu::TextureViewDimension::D2Array),
        ..Default::default()
    });
    Ok(VfxAuraGpuTexture {
        texture,
        view,
        slot_layers: std::array::from_fn(|slot| array.slot_layers[slot].map(|_| slot as u32)),
    })
}

fn validate_vfx_aura_texture_array(
    array: &xiv_companion_data::VfxAuraTextureArrayRgba,
    limits: &wgpu::Limits,
) -> Result<u32, VfxAuraTextureUploadError> {
    let mut layers = 0_u32;
    for layer in array.slot_layers.into_iter().flatten() {
        if layer != layers {
            return Err(VfxAuraTextureUploadError::InvalidLayout);
        }
        layers += 1;
    }
    let max_mips = u32::BITS - array.width.max(array.height).leading_zeros();
    if array.width == 0
        || array.height == 0
        || layers == 0
        || array.mips.is_empty()
        || array.mips.len() > max_mips as usize
    {
        return Err(VfxAuraTextureUploadError::InvalidLayout);
    }
    if array.width > limits.max_texture_dimension_2d
        || array.height > limits.max_texture_dimension_2d
        || 3 > limits.max_texture_array_layers
    {
        return Err(VfxAuraTextureUploadError::DeviceLimitExceeded);
    }
    for (mip, level) in array.mips.iter().enumerate() {
        let width = (array.width >> mip).max(1);
        let height = (array.height >> mip).max(1);
        let expected_len = (width as usize)
            .checked_mul(height as usize)
            .and_then(|pixels| pixels.checked_mul(4))
            .and_then(|bytes| bytes.checked_mul(layers as usize));
        if level.width != width || level.height != height || expected_len != Some(level.rgba.len())
        {
            return Err(VfxAuraTextureUploadError::InvalidLayout);
        }
    }
    Ok(layers)
}

/// 上传 VFX 贴图并返回视图。BC6H 使用 RGBA16F，其他文件输入使用 RGBA8 authored mip；
/// 程序化与合成输入才生成完整线性链。
pub(crate) fn create_vfx_texture_view(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &VfxTextureInput,
) -> wgpu::TextureView {
    if let Some(levels) = vfx_authored_rgba16f_mips(texture) {
        let gpu_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("weapon vfx HDR color texture"),
            size: wgpu::Extent3d {
                width: levels[0].width,
                height: levels[0].height,
                depth_or_array_layers: 1,
            },
            mip_level_count: levels.len() as u32,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        for (mip, level) in levels.iter().enumerate() {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &gpu_texture,
                    mip_level: mip as u32,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &level.rgba16f,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(level.width * 8),
                    rows_per_image: Some(level.height),
                },
                wgpu::Extent3d {
                    width: level.width,
                    height: level.height,
                    depth_or_array_layers: 1,
                },
            );
        }
        return gpu_texture.create_view(&wgpu::TextureViewDescriptor::default());
    }
    // VFX 贴图按线性数据处理（atex 是特效数据贴图，不走 sRGB 解码——
    // sRGB 会把暗部压近零，光罩纹理图案与火舌流光全部消失）。
    let authored = vfx_authored_mip_levels(texture);
    let gpu_texture = if let Some(levels) = authored.as_deref() {
        create_rgba_texture_from_mips(
            device,
            queue,
            "weapon vfx color texture",
            levels,
            RgbaMipSemantic::LinearData,
        )
    } else {
        create_mipped_rgba_texture(
            device,
            queue,
            "weapon vfx color texture",
            texture.width.max(1),
            texture.height.max(1),
            &texture.rgba,
            RgbaMipSemantic::LinearData,
        )
    };
    gpu_texture.create_view(&wgpu::TextureViewDescriptor::default())
}

pub(crate) fn create_vfx_cube_texture_view(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &VfxTextureInput,
) -> Option<wgpu::TextureView> {
    let levels = vfx_authored_cube_mips(texture)?;
    let (format, bytes_per_pixel) = match texture.cube_format {
        xiv_companion_data::VfxTextureCubeFormat::Rgba8Unorm => {
            (wgpu::TextureFormat::Rgba8Unorm, 4)
        }
        xiv_companion_data::VfxTextureCubeFormat::Rgba16Float => {
            (wgpu::TextureFormat::Rgba16Float, 8)
        }
    };
    let gpu_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("weapon vfx authored reflection cube"),
        size: wgpu::Extent3d {
            width: levels[0].width,
            height: levels[0].height,
            depth_or_array_layers: 6,
        },
        mip_level_count: levels.len() as u32,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    for (mip, level) in levels.iter().enumerate() {
        for (face, rgba) in level.faces.iter().enumerate() {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &gpu_texture,
                    mip_level: mip as u32,
                    origin: wgpu::Origin3d {
                        x: 0,
                        y: 0,
                        z: face as u32,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                rgba,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(level.width * bytes_per_pixel),
                    rows_per_image: Some(level.height),
                },
                wgpu::Extent3d {
                    width: level.width,
                    height: level.height,
                    depth_or_array_layers: 1,
                },
            );
        }
    }
    Some(gpu_texture.create_view(&wgpu::TextureViewDescriptor {
        label: Some("weapon vfx authored reflection cube view"),
        dimension: Some(wgpu::TextureViewDimension::Cube),
        ..Default::default()
    }))
}

fn vfx_authored_cube_mips(texture: &VfxTextureInput) -> Option<&[VfxTextureCubeMipInput]> {
    let levels = texture.cube_mips.as_deref()?;
    let mut size = texture.width;
    let bytes_per_pixel = match texture.cube_format {
        xiv_companion_data::VfxTextureCubeFormat::Rgba8Unorm => 4,
        xiv_companion_data::VfxTextureCubeFormat::Rgba16Float => 8,
    };
    let max_mips = u32::BITS - size.leading_zeros();
    if size == 0 || texture.height != size || levels.is_empty() || levels.len() > max_mips as usize
    {
        return None;
    }
    for level in levels {
        let bytes = usize::try_from(size)
            .ok()?
            .checked_mul(usize::try_from(size).ok()?)?
            .checked_mul(bytes_per_pixel)?;
        if level.width != size
            || level.height != size
            || level.faces.iter().any(|face| face.len() != bytes)
        {
            return None;
        }
        size = (size / 2).max(1);
    }
    Some(levels)
}

fn vfx_authored_rgba16f_mips(texture: &VfxTextureInput) -> Option<&[VfxTextureMipRgba16fInput]> {
    let levels = texture.rgba16f_mips.as_deref()?;
    let mut width = texture.width;
    let mut height = texture.height;
    let max_mips = u32::BITS - width.max(height).leading_zeros();
    if width == 0 || height == 0 || levels.is_empty() || levels.len() > max_mips as usize {
        return None;
    }
    for level in levels {
        let bytes = usize::try_from(width)
            .ok()?
            .checked_mul(usize::try_from(height).ok()?)?
            .checked_mul(8)?;
        if level.width != width || level.height != height || level.rgba16f.len() != bytes {
            return None;
        }
        width = (width / 2).max(1);
        height = (height / 2).max(1);
    }
    Some(levels)
}

fn vfx_authored_mip_levels(texture: &VfxTextureInput) -> Option<Vec<RgbaMipLevel>> {
    let levels = texture.authored_mips.as_deref()?;
    let mut expected_width = texture.width.max(1);
    let mut expected_height = texture.height.max(1);
    let max_mip_count = (u32::BITS - expected_width.max(expected_height).leading_zeros()) as usize;
    let valid = !levels.is_empty()
        && levels.len() <= max_mip_count
        && levels.iter().all(|level| {
            let expected_len = usize::try_from(expected_width)
                .ok()
                .and_then(|width| {
                    usize::try_from(expected_height)
                        .ok()
                        .and_then(|height| width.checked_mul(height))
                })
                .and_then(|pixels| pixels.checked_mul(4));
            let valid = level.width == expected_width
                && level.height == expected_height
                && expected_len == Some(level.rgba.len());
            expected_width = (expected_width / 2).max(1);
            expected_height = (expected_height / 2).max(1);
            valid
        });
    valid.then(|| {
        levels
            .iter()
            .map(|level| RgbaMipLevel {
                width: level.width,
                height: level.height,
                rgba: level.rgba.clone(),
            })
            .collect()
    })
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
        authored_mips: None,
        rgba16f_mips: None,
        cube_mips: None,
        cube_format: xiv_companion_data::VfxTextureCubeFormat::Rgba8Unorm,
    }
}

#[cfg(test)]
mod laser_tests {
    use super::*;
    use xiv_companion_data::{
        VfxDecal, VfxDisc, VfxLaser, VfxLine, VfxPolygon, VfxPolyline, VfxQuad,
    };

    fn laser_quad(mode: i32) -> VfxQuad {
        VfxQuad {
            particle_type: Some(xiv_companion_data::avfx::ParticleType::Laser),
            particle_index: 17,
            soft_particle: false,
            soft_particle_fade_range: 1.0,
            depth_offset_type: 0,
            depth_offset: 0.0,
            powder_single: false,
            windmill_uv_type: 0,
            disc: None,
            polygon: None,
            laser: Some(VfxLaser {
                length: 5.0,
                width: 2.0,
                scale: [5.0, 6.0, 7.0],
            }),
            line: None,
            polyline: None,
            decal: None,
            position: [0.0; 3],
            size: [0.5; 2],
            orientation: [0.0, 0.0, 0.0, 1.0],
            parent_basis: [[2.0, 0.0, 0.0], [0.0, 3.0, 0.0], [0.0, 0.0, -4.0]],
            movement_direction: [0.0; 3],
            facing_parent_basis: xiv_companion_data::VFX_IDENTITY_BASIS,
            rotation_direction_base: mode,
            color: [1.0; 4],
            draw_layer: 0,
            soft_key_offset: 0.0,
            draw_priority: 0,
            draw_order: None,
            pivot: [0.0; 2],
            texture_indexes: [-1; 4],
            texture_uv_sets: [0; 4],
            uv_by_pixel_position: [false; 4],
            combine_mode_tc1: [0; 2],
            combine_modes: [[0; 2]; 3],
            color_to_alpha: [false; 4],
            uv_origins: [[0.0; 2]; 4],
            uv_scales: [[1.0; 2]; 4],
            uv_rotations: [0.0; 4],
            texture_borders: [[0; 2]; 4],
            texture_filters: [0; 4],
            texture1_is_shape_mask: false,
            texture1_enabled: false,
            texture1_use_screen_copy: false,
            draw_mode: 0,
            depth_test: true,
            depth_write: false,
            cull_mode: 3,
            texture_distortion_index: -1,
            distortion_power: 0.0,
            distortion_targets: 0,
            uvd_origin: [0.0; 2],
            uvd_scale: [1.0; 2],
            uvd_rotation: 0.0,
            distortion_uv_set: 0,
            uvd_by_pixel_position: false,
            distortion_borders: [0; 2],
            distortion_filter: 0,
            texture_palette_index: -1,
            palette_offset: 0.0,
            palette_border: 0,
            palette_filter: 0,
        }
    }

    #[cfg(all(feature = "test-support", not(target_arch = "wasm32")))]
    #[test]
    #[ignore = "requires explicit native GPU opt-in"]
    fn browser_device_limits_support_all_vfx_pipelines_and_buffers() {
        crate::test_support::require_gpu_test_opt_in().expect("explicit native GPU test opt-in");
        pollster::block_on(async {
            let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
                backends: wgpu::Backends::VULKAN,
                ..wgpu::InstanceDescriptor::new_without_display_handle()
            });
            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions::default())
                .await
                .expect("native Vulkan adapter");
            eprintln!("browser-limit regression adapter: {:?}", adapter.get_info());
            let mut old_limits =
                wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits());
            old_limits.max_storage_buffers_per_shader_stage = 1;
            old_limits.max_storage_buffer_binding_size = JOINT_STORAGE_BUFFER_SIZE;
            old_limits.max_vertex_buffer_array_stride = 2_048;
            let (old_device, old_queue) = adapter
                .request_device(&wgpu::DeviceDescriptor {
                    required_limits: old_limits,
                    ..Default::default()
                })
                .await
                .expect("old browser device limits");
            let scope = old_device.push_error_scope(wgpu::ErrorFilter::Validation);
            let old_context = ModelRenderContext::new_with_msaa(
                old_device,
                old_queue,
                wgpu::TextureFormat::Rgba8UnormSrgb,
                1,
            );
            let error = scope
                .pop()
                .await
                .expect("old browser limits reject VFX pipelines");
            let message = error.to_string();
            assert!(message.contains("StorageBuffers"), "{message}");
            eprintln!("old browser limits reproduced: {message}");
            drop(old_context);

            let (device, queue) = adapter
                .request_device(&wgpu::DeviceDescriptor {
                    required_limits: ModelRenderContext::required_limits(adapter.limits()),
                    ..Default::default()
                })
                .await
                .expect("shared browser/native device limits");
            for msaa in [1, 4] {
                let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
                let context = ModelRenderContext::new_with_msaa(
                    device.clone(),
                    queue.clone(),
                    wgpu::TextureFormat::Rgba8UnormSrgb,
                    msaa,
                );
                let mut batch = context.create_vfx_particles(&[], &[]);
                batch.update(&context, &[laser_quad(0)]);
                assert!(!batch.bind_groups.is_empty());
                assert!(
                    scope.pop().await.is_none(),
                    "MSAA {msaa}: invalid browser limits"
                );
            }
        });
    }

    #[cfg(all(feature = "test-support", not(target_arch = "wasm32")))]
    #[test]
    #[ignore = "requires a native GPU adapter"]
    fn draw_batches_preserve_layer_boundaries() {
        crate::test_support::require_gpu_test_opt_in().expect("explicit native GPU test opt-in");
        pollster::block_on(async {
            let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
                backends: wgpu::Backends::PRIMARY,
                ..wgpu::InstanceDescriptor::new_without_display_handle()
            });
            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions::default())
                .await
                .expect("native adapter");
            let (device, queue) = adapter
                .request_device(&wgpu::DeviceDescriptor {
                    required_limits: wgpu::Limits::downlevel_defaults()
                        .using_resolution(adapter.limits()),
                    ..Default::default()
                })
                .await
                .expect("native device");
            let context =
                ModelRenderContext::new(device, queue, wgpu::TextureFormat::Rgba8UnormSrgb);
            let mut batch = context.create_vfx_particles(&[], &[]);
            for decal in [false, true] {
                let quads: Vec<_> = [4, 4, 10, 4]
                    .into_iter()
                    .enumerate()
                    .map(|(order, layer)| {
                        let mut quad = laser_quad(0);
                        quad.laser = None;
                        quad.particle_type = Some(xiv_companion_data::avfx::ParticleType::Quad);
                        quad.draw_layer = layer;
                        quad.draw_order = Some(order as u64);
                        if decal {
                            quad.decal = Some(VfxDecal {
                                ring: false,
                                scaling_scale: 1.0,
                                depth_type: 0,
                                ring_width: 0.0,
                                ring_fan: 0.0,
                                scale_z: 1.0,
                            });
                        }
                        quad
                    })
                    .collect();
                batch.update(&context, &quads);
                let ranges: Vec<_> = if decal {
                    batch
                        .decal_draw_ranges
                        .iter()
                        .map(|r| (r.draw_layer, r.start, r.count))
                        .collect()
                } else {
                    batch
                        .draw_ranges
                        .iter()
                        .map(|r| (r.draw_layer, r.start, r.count))
                        .collect()
                };
                assert_eq!(ranges, [(4, 0, 2), (10, 2, 1), (4, 3, 1)]);
            }
        });
    }

    #[test]
    fn soft_particle_alpha_matches_inverse_projection_distance_fade() {
        let identity = Mat4::IDENTITY.to_cols_array_2d();
        let alpha = soft_particle_alpha([0.5, 0.5], 0.75, [0.0, 0.0, 0.5, 1.0], identity, 0.5, 1.0);
        // distance=.25, range=.5, then the client squares the normalized fade.
        assert!((alpha - 0.25).abs() < 1.0e-6);
    }

    #[test]
    fn soft_particle_alpha_saturates_beyond_fade_range_and_clamps_base_alpha() {
        let identity = Mat4::IDENTITY.to_cols_array_2d();
        assert_eq!(
            soft_particle_alpha([0.5, 0.5], 2.0, [0.0, 0.0, 0.0, 1.0], identity, 0.5, 2.0,),
            1.0
        );
        assert_eq!(
            soft_particle_alpha([0.5, 0.5], 0.0, [0.0, 0.0, 0.0, 1.0], identity, 0.5, -1.0,),
            0.0
        );
    }

    #[test]
    fn soft_particle_metadata_uses_reserved_quad_attribute_slot() {
        let mut quad = laser_quad(0);
        quad.particle_type = Some(xiv_companion_data::avfx::ParticleType::Quad);
        quad.laser = None;
        quad.soft_particle = true;
        quad.soft_particle_fade_range = 2.5;
        let gpu = GpuVfxQuad::from(&quad);
        assert_eq!(gpu.disc_color_outer, [2.5, 1.0, 0.0, 0.0]);
    }

    #[test]
    fn special_geometry_soft_metadata_keeps_evaluated_colors() {
        let mut particle = laser_quad(0);
        particle.soft_particle = true;
        particle.soft_particle_fade_range = 2.5;
        let laser = GpuVfxQuad::from(&particle);
        assert_eq!(laser.disc_color_outer[..2], [2.5, 1.0]);

        particle.particle_type = Some(xiv_companion_data::avfx::ParticleType::Line);
        particle.laser = None;
        particle.line = Some(VfxLine {
            scale: [1.0; 3],
            length: 1.0,
            endpoint_offset: None,
            color_begin: [1.0, 0.0, 0.0, 1.0],
            color_end: [0.0, 0.0, 1.0, 1.0],
        });
        let line = GpuVfxQuad::from(&particle);
        assert_eq!(line.color, [2.5, 1.0, 0.0, 0.0]);
        assert_eq!(line.disc_color_inner, particle.line.unwrap().color_begin);
        assert_eq!(line.disc_color_outer, particle.line.unwrap().color_end);

        particle.particle_type = Some(xiv_companion_data::avfx::ParticleType::Disc);
        particle.line = None;
        particle.disc = Some(VfxDisc {
            counts: [1, 2, 3],
            angle: 0.0,
            radius: [1.0; 2],
            width: [0.2; 2],
            height_inner: [0.0; 2],
            height_outer: [0.0; 2],
            color_inner: [1.0, 0.0, 0.0, 1.0],
            color_outer: [0.0, 0.0, 1.0, 1.0],
            point_interval_factor: 0.0,
            scaling_scale: 1.0,
            scale_z: 1.0,
        });
        let disc = GpuVfxQuad::from(&particle);
        assert_eq!(disc.color, [2.5, 1.0, 0.0, 0.0]);
        assert_eq!(disc.disc_color_inner, particle.disc.unwrap().color_inner);
        assert_eq!(disc.disc_color_outer, particle.disc.unwrap().color_outer);
    }

    #[test]
    fn decal_gpu_instance_keeps_affine_axes_inverse_and_projection_parameters() {
        let mut quad = laser_quad(0);
        quad.particle_type = Some(xiv_companion_data::avfx::ParticleType::DecalRing);
        quad.laser = None;
        quad.decal = Some(VfxDecal {
            ring: true,
            scaling_scale: 0.75,
            depth_type: 2,
            ring_width: 0.125,
            ring_fan: 0.5,
            scale_z: -5.0,
        });
        quad.position = [7.0, 8.0, 9.0];
        quad.size = [2.0, -3.0];
        quad.parent_basis = [[2.0, 0.0, 0.0], [1.0, 3.0, 0.0], [0.0, 0.0, 4.0]];

        let gpu = GpuVfxDecalInstance::from_quad(&quad).unwrap();
        assert_eq!(gpu.world[0], [8.0, 0.0, 0.0, 0.0]);
        assert_eq!(gpu.world[1], [-6.0, -18.0, 0.0, 0.0]);
        assert_eq!(gpu.world[2], [0.0, 0.0, -20.0, 0.0]);
        assert_eq!(gpu.world[3], [7.0, 8.0, 9.0, 1.0]);
        let correction = 0.75 + 0.25 / 14.0;
        assert_eq!(gpu.projection, [0.75, 0.125 * correction, 0.5, 1.0]);
        assert_eq!(gpu.metadata[0], 2);
        let identity = glam::Mat4::from_cols_array_2d(&gpu.world)
            * glam::Mat4::from_cols_array_2d(&gpu.inverse_world);
        assert!(identity.abs_diff_eq(glam::Mat4::IDENTITY, 1.0e-6));
    }

    #[test]
    fn decal_gpu_instance_applies_client_ss_correction_to_local_xz() {
        let mut quad = laser_quad(0);
        quad.particle_type = Some(xiv_companion_data::avfx::ParticleType::Decal);
        quad.laser = None;
        quad.decal = Some(VfxDecal {
            ring: false,
            scaling_scale: 0.5,
            depth_type: 0,
            ring_width: 0.0,
            ring_fan: 0.0,
            scale_z: 4.0,
        });
        quad.size = [1.0, 1.5];
        quad.parent_basis = xiv_companion_data::VFX_IDENTITY_BASIS;

        let gpu = GpuVfxDecalInstance::from_quad(&quad).unwrap();
        // Mean X/Z axis length is 3, so SS + (1 - SS) / distance = 2/3.
        assert!((gpu.world[0][0] - 4.0 / 3.0).abs() < 1.0e-6);
        assert_eq!(gpu.world[1], [0.0, 3.0, 0.0, 0.0]);
        assert!((gpu.world[2][2] - 8.0 / 3.0).abs() < 1.0e-6);
        assert_eq!(gpu.projection, [0.5, 0.0, 0.0, 0.0]);
        let identity = glam::Mat4::from_cols_array_2d(&gpu.world)
            * glam::Mat4::from_cols_array_2d(&gpu.inverse_world);
        assert!(identity.abs_diff_eq(glam::Mat4::IDENTITY, 1.0e-6));
    }

    #[test]
    fn decal_gpu_instance_rejects_noninvertible_projection_volume() {
        let mut quad = laser_quad(0);
        quad.laser = None;
        quad.decal = Some(VfxDecal {
            ring: false,
            scaling_scale: 1.0,
            depth_type: 0,
            ring_width: 0.0,
            ring_fan: 0.0,
            scale_z: 0.0,
        });
        assert!(GpuVfxDecalInstance::from_quad(&quad).is_none());
    }

    #[test]
    fn decal_gpu_instance_rejects_deferred_and_unknown_ddtt_targets() {
        let mut quad = laser_quad(0);
        quad.laser = None;
        for depth_type in [3, 4, 5, -1, 6] {
            quad.decal = Some(VfxDecal {
                ring: false,
                scaling_scale: 1.0,
                depth_type,
                ring_width: 0.0,
                ring_fan: 0.0,
                scale_z: 1.0,
            });
            assert!(
                GpuVfxDecalInstance::from_quad(&quad).is_none(),
                "DDTT={depth_type}"
            );
        }
    }

    #[test]
    fn decal_cube_matches_client_vertex_and_index_counts() {
        assert_eq!(VFX_DECAL_CUBE_VERTICES.len(), 8);
        assert_eq!(VFX_DECAL_CUBE_INDICES.len(), 36);
        assert!(VFX_DECAL_CUBE_VERTICES.iter().all(|vertex| {
            vertex.position[3] == 1.0 && vertex.position[..3].iter().all(|value| value.abs() == 0.5)
        }));
        assert!(
            VFX_DECAL_CUBE_INDICES
                .iter()
                .all(|index| usize::from(*index) < VFX_DECAL_CUBE_VERTICES.len())
        );
        for triangle in VFX_DECAL_CUBE_INDICES.chunks_exact(3) {
            let positions = [triangle[0], triangle[1], triangle[2]].map(|index| {
                let [x, y, z, _] = VFX_DECAL_CUBE_VERTICES[usize::from(index)].position;
                Vec3::new(x, y, z)
            });
            let normal = (positions[1] - positions[0]).cross(positions[2] - positions[0]);
            assert!(
                normal.dot(positions[0]) > 0.0,
                "triangle {triangle:?} points inward"
            );
        }
    }

    #[test]
    fn decal_shaders_keep_single_and_per_sample_depth_paths() {
        let single = include_str!(concat!(env!("OUT_DIR"), "/vfx_decal_1x.wgsl"));
        let multisample = include_str!(concat!(env!("OUT_DIR"), "/vfx_decal_4x.wgsl"));
        let common = include_str!(concat!(env!("OUT_DIR"), "/vfx_decal_1x.wgsl"));
        assert!(single.contains("texture_depth_2d"));
        assert!(single.contains("fn fs_decal_multiply(input: DecalVertexOutput)"));
        assert!(multisample.contains("texture_depth_multisampled_2d"));
        assert!(multisample.contains("@builtin(sample_index)"));
        assert!(common.contains("inverse_view_proj"));
        assert!(common.contains("0.5 - local.x"));
        assert!(common.contains("normalized_z + 2.0 * instance.projection.z"));
    }

    #[test]
    fn laser_gpu_instance_keeps_three_axes_and_perpendicular_width_scale() {
        for (mode, expected_width) in [(0, 46.0), (1, 38.0), (2, 28.0)] {
            let gpu = GpuVfxQuad::from(&laser_quad(mode));
            assert_eq!(gpu.axis_x[..3], [10.0, 0.0, 0.0]);
            assert_eq!(gpu.axis_y[..3], [0.0, 18.0, 0.0]);
            assert_eq!(gpu.disc_axis_z[..3], [0.0, 0.0, -28.0]);
            assert_eq!(gpu.disc_dimensions, [5.0, expected_width, 0.0, 0.0]);
            assert_eq!(gpu.pivot_offset[3], 6.0);
        }
    }

    #[test]
    fn laser_uses_eighteen_vertices_and_quad_double_grouping() {
        let quad = laser_quad(1);
        assert_eq!(vfx_quad_vertex_count(&quad), 18);
        assert_eq!(vfx_quad_double_group(&quad), Some(17));

        let mut unsupported = quad;
        unsupported.laser = None;
        assert_eq!(vfx_quad_vertex_count(&unsupported), 6);
    }

    #[test]
    fn polyline_stream_expands_each_segment_to_two_edge_quads() {
        let mut quad = laser_quad(1);
        quad.particle_type = Some(xiv_companion_data::avfx::ParticleType::Polyline);
        quad.laser = None;
        let polyline = VfxPolyline {
            point_count: 3,
            point_count_center: 1,
            uv_precision: 2,
            use_edge: true,
            not_billboard_axis: None,
            reverse_points: false,
            uv_span: 1.0,
            positions: {
                let mut points = [[0.0; 3]; 64];
                points[..3].copy_from_slice(&[[0.0, 0.0, 0.0], [1.0, 1.0, 0.0], [2.0, 0.0, 0.0]]);
                points
            },
            end_distortion: {
                let mut values = [u8::MAX; 64];
                values[..3].copy_from_slice(&[0, 127, 255]);
                values
            },
            widths: [1.0, 2.0, 3.0],
            colors: [[1.0, 0.0, 0.0, 1.0]; 3],
            edge_colors: [[0.0, 0.0, 1.0, 0.5]; 3],
        };
        quad.polyline = Some(polyline);
        let vertices = gpu_polyline_vertices(&quad, polyline);
        assert_eq!(vertices.len(), 24);
        assert_eq!(
            vertices[..12]
                .iter()
                .map(|vertex| vertex.center_width)
                .collect::<Vec<_>>(),
            vec![
                [0.0, 0.0, 0.0, -1.0],
                [0.0, 0.0, 0.0, 0.0],
                [1.0, 1.0, 0.0, 0.0],
                [1.0, 1.0, 0.0, 0.0],
                [1.0, 1.0, 0.0, -2.0],
                [0.0, 0.0, 0.0, -1.0],
                [0.0, 0.0, 0.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
                [1.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
                [1.0, 1.0, 0.0, 2.0],
                [1.0, 1.0, 0.0, 0.0],
            ]
        );
        assert_eq!(vertices[0].base_uv[..2], [0.0, 0.0]);
        assert_eq!(vertices[0].base_uv[3], 2.0);
        assert_eq!(vertices[0].previous[3], 0.0);
        assert_eq!(vertices[2].previous[3], 127.0 / 255.0);
        assert_eq!(vertices[1].base_uv[..2], [0.5, 0.0]);
        assert_eq!(vertices[2].base_uv[..2], [0.5, 0.5]);
        assert_eq!(vertices[0].color, [0.0, 0.0, 1.0, 0.5]);
        assert_eq!(vertices[1].color, [1.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn local_spline_stream_reverses_all_point_attributes_and_negates_uv_span() {
        let mut quad = laser_quad(1);
        quad.particle_type = Some(xiv_companion_data::avfx::ParticleType::Polyline);
        quad.laser = None;
        let polyline = VfxPolyline {
            point_count: 3,
            point_count_center: 1,
            uv_precision: 0,
            use_edge: false,
            not_billboard_axis: None,
            reverse_points: true,
            uv_span: -0.5,
            positions: {
                let mut points = [[0.0; 3]; 64];
                points[..3].copy_from_slice(&[[0.0, 0.0, 0.0], [1.0, 2.0, 0.0], [3.0, 4.0, 0.0]]);
                points
            },
            end_distortion: {
                let mut values = [u8::MAX; 64];
                values[..3].copy_from_slice(&[17, 127, 239]);
                values
            },
            widths: [1.0, 2.0, 3.0],
            colors: [
                [1.0, 0.0, 0.0, 1.0],
                [0.0, 1.0, 0.0, 1.0],
                [0.0, 0.0, 1.0, 1.0],
            ],
            edge_colors: [[0.0; 4]; 3],
        };

        let vertices = gpu_polyline_vertices(&quad, polyline);
        assert_eq!(vertices[0].center_width, [3.0, 4.0, 0.0, -3.0]);
        assert_eq!(vertices[0].previous[..3], [3.0, 4.0, 0.0]);
        assert_eq!(vertices[0].next[..3], [1.0, 2.0, 0.0]);
        assert_eq!(vertices[0].previous[3], 239.0 / 255.0);
        assert_eq!(vertices[0].color, [0.0, 0.0, 1.0, 1.0]);
        assert_eq!(vertices[0].base_uv[..2], [0.0, -0.0]);
        assert_eq!(vertices[2].center_width, [1.0, 2.0, 0.0, 2.0]);
        assert_eq!(vertices[2].previous[3], 127.0 / 255.0);
        assert_eq!(vertices[2].color, [0.0, 1.0, 0.0, 1.0]);
        assert_eq!(vertices[2].base_uv[..2], [1.0, -0.25]);
    }

    #[test]
    fn non_edge_polyline_stream_uses_two_main_color_columns() {
        assert_eq!(std::mem::size_of::<GpuVfxPolylineVertex>(), 208);
        assert_eq!(
            std::mem::offset_of!(GpuVfxPolylineVertex, soft_particle_params),
            192
        );
        let mut quad = laser_quad(1);
        quad.particle_type = Some(xiv_companion_data::avfx::ParticleType::Polyline);
        quad.laser = None;
        quad.soft_particle = true;
        quad.soft_particle_fade_range = 2.75;
        let polyline = VfxPolyline {
            point_count: 2,
            point_count_center: 1,
            uv_precision: 0,
            use_edge: false,
            not_billboard_axis: Some([0.0, 3.0, 4.0]),
            reverse_points: false,
            uv_span: 1.0,
            positions: {
                let mut points = [[0.0; 3]; 64];
                points[..2].copy_from_slice(&[[0.0; 3], [1.0, 0.0, 0.0]]);
                points
            },
            end_distortion: [u8::MAX; 64],
            widths: [2.0; 3],
            colors: [[1.0, 0.0, 0.0, 1.0]; 3],
            edge_colors: [[0.0, 0.0, 1.0, 1.0]; 3],
        };
        let vertices = gpu_polyline_vertices(&quad, polyline);
        assert_eq!(vertices.len(), 6);
        assert!(
            vertices
                .iter()
                .all(|vertex| vertex.width_axis == [0.0, 3.0, 4.0, 1.0])
        );
        assert_eq!(
            vertices
                .iter()
                .map(|vertex| vertex.center_width[3])
                .collect::<Vec<_>>(),
            [-2.0, 2.0, 2.0, 2.0, -2.0, -2.0]
        );
        assert!(
            vertices
                .iter()
                .all(|vertex| vertex.color == [1.0, 0.0, 0.0, 1.0])
        );
        assert_eq!(vertices[0].base_uv[..2], [0.0, 0.0]);
        assert_eq!(vertices[1].base_uv[..2], [1.0, 0.0]);
        assert_eq!(vertices[2].base_uv[..2], [1.0, 1.0]);
        assert!(
            vertices
                .iter()
                .all(|vertex| vertex.soft_particle_params == [2.75, 1.0, 0.0, 0.0])
        );
    }

    #[test]
    fn palette_state_is_grouped_and_packed_without_changing_td_targets() {
        let mut quad = laser_quad(1);
        quad.particle_type = Some(xiv_companion_data::avfx::ParticleType::Quad);
        quad.texture_palette_index = 3;
        quad.palette_offset = 63.0 / 255.0;
        quad.palette_border = 2;
        quad.palette_filter = 1;
        quad.distortion_targets = 5;
        let key = VfxGroupKey::of_quad(&quad);
        assert_eq!(key.texture_p, 3);
        assert_eq!(key.palette_border, 2);
        assert_eq!(key.palette_filter, 1);
        let gpu = GpuVfxQuad::from(&quad);
        assert_eq!(gpu.distortion[1] & 15, 5);
        assert_eq!((gpu.distortion[1] >> 8) & 255, 63);

        quad.particle_type = Some(xiv_companion_data::avfx::ParticleType::Powder);
        assert_eq!(VfxGroupKey::of_quad(&quad).texture_p, -1);
        quad.particle_type = Some(xiv_companion_data::avfx::ParticleType::Line);
        quad.line = Some(VfxLine {
            scale: [1.0; 3],
            length: 1.0,
            endpoint_offset: None,
            color_begin: [1.0; 4],
            color_end: [1.0; 4],
        });
        assert_eq!(VfxGroupKey::of_quad(&quad).texture_p, -1);
    }

    #[test]
    fn shader_applies_palette_after_tc1_gating_and_before_tc2() {
        let compiled = include_str!(concat!(env!("OUT_DIR"), "/vfx.wgsl"));
        let module = wgpu::naga::front::wgsl::parse_str(compiled).unwrap();
        wgpu::naga::valid::Validator::new(
            wgpu::naga::valid::ValidationFlags::all(),
            wgpu::naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .unwrap();
        let layout = module
            .types
            .iter()
            .find_map(|(_, ty)| {
                (ty.name.as_deref() == Some("VfxMeshInstance")).then_some(&ty.inner)
            })
            .unwrap();
        let wgpu::naga::TypeInner::Struct { members, span } = layout else {
            panic!("mesh storage must be a struct");
        };
        assert_eq!(*span as usize, std::mem::size_of::<GpuVfxMeshInstance>());
        let texture_params = members
            .iter()
            .find(|m| m.name.as_deref() == Some("normal_params"))
            .unwrap();
        assert_eq!(
            texture_params.offset as usize,
            std::mem::offset_of!(GpuVfxMeshInstance, normal_params)
        );
        let source = include_str!("../shaders/vfx.wesl");
        let compose = &source[source.find("fn compose_color").unwrap()..];
        let gate = compose
            .find("select(vec3<f32>(1.0), tex_color.rgb")
            .unwrap();
        let palette = compose.find("textureSampleBias(vfx_texture_p").unwrap();
        let tc2 = compose[palette..].find("vfx_texture2,").unwrap() + palette;
        assert!(gate < palette && palette < tc2);
    }

    #[test]
    fn shader_selects_current_and_previous_scene_sources_before_tc1_gating() {
        let source = include_str!("../shaders/vfx.wesl");
        let compose = &source[source.find("fn compose_color").unwrap()..];
        let current = compose.find("textureSample(vfx_screen_copy").unwrap();
        let previous = compose.find("textureSample(vfx_previous_scene").unwrap();
        let file_texture = compose
            .find("sample_layer(\n                vfx_texture1")
            .unwrap();
        let gate = compose
            .find("select(vec3<f32>(1.0), tex_color.rgb")
            .unwrap();
        assert!(current < previous && previous < file_texture && file_texture < gate);
    }

    #[test]
    fn shader_selects_file_or_provider_cube_with_tft_bias() {
        let source = include_str!("../shaders/vfx.wesl");
        assert!(source.contains("var vfx_reflection_cube: texture_cube<f32>"));
        assert!(source.contains("var vfx_reflection_sampler: sampler"));
        assert!(source.contains("var vfx_file_reflection_cube: texture_cube<f32>"));
        assert!(source.contains("textureSampleBias(\n                vfx_file_reflection_cube"));
        assert!(source.contains("vfx_params.auxiliary_lod_bias.w"));
    }

    #[test]
    fn client_texture_filter_modes_map_to_sampler_and_lod_bias() {
        let expected = [
            (wgpu::FilterMode::Nearest, wgpu::FilterMode::Nearest, 1, 0.0),
            (wgpu::FilterMode::Linear, wgpu::FilterMode::Linear, 1, -0.5),
            (wgpu::FilterMode::Linear, wgpu::FilterMode::Linear, 4, -1.0),
            (wgpu::FilterMode::Linear, wgpu::FilterMode::Linear, 8, -1.5),
            (wgpu::FilterMode::Linear, wgpu::FilterMode::Linear, 16, -2.0),
        ];
        for (filter, (mag, min, anisotropy, bias)) in expected.into_iter().enumerate() {
            let descriptor = vfx_sampler_descriptor([0, 1], filter as i32);
            assert_eq!(descriptor.mag_filter, mag);
            assert_eq!(descriptor.min_filter, min);
            assert_eq!(descriptor.mipmap_filter, wgpu::MipmapFilterMode::Linear);
            assert_eq!(descriptor.anisotropy_clamp, anisotropy);
            assert_eq!(vfx_texture_lod_bias(filter as i32), bias);
            assert_eq!(descriptor.address_mode_u, wgpu::AddressMode::Repeat);
            assert_eq!(descriptor.address_mode_v, wgpu::AddressMode::ClampToEdge);
        }
    }

    #[test]
    fn source_authored_vfx_mips_preserve_levels_and_reject_invalid_layouts() {
        let mut texture = VfxTextureInput {
            rgba: vec![0; 4 * 4 * 4],
            width: 4,
            height: 4,
            authored_mips: Some(vec![
                VfxTextureMipInput {
                    rgba: vec![10; 4 * 4 * 4],
                    width: 4,
                    height: 4,
                },
                VfxTextureMipInput {
                    rgba: vec![20; 2 * 2 * 4],
                    width: 2,
                    height: 2,
                },
                VfxTextureMipInput {
                    rgba: vec![30; 4],
                    width: 1,
                    height: 1,
                },
            ]),
            rgba16f_mips: None,
            cube_mips: None,
            cube_format: xiv_companion_data::VfxTextureCubeFormat::Rgba8Unorm,
        };
        let levels = vfx_authored_mip_levels(&texture).expect("valid authored levels");
        assert_eq!(
            levels.iter().map(|level| level.width).collect::<Vec<_>>(),
            [4, 2, 1]
        );
        assert_eq!(levels[0].rgba, [10; 64]);
        assert_eq!(levels[1].rgba, [20; 16]);
        assert_eq!(levels[2].rgba, [30; 4]);

        texture.authored_mips.as_mut().unwrap()[1].width = 3;
        assert!(vfx_authored_mip_levels(&texture).is_none());
        texture.authored_mips.as_mut().unwrap()[1].width = 2;
        texture
            .authored_mips
            .as_mut()
            .unwrap()
            .push(VfxTextureMipInput {
                rgba: vec![40; 4],
                width: 1,
                height: 1,
            });
        assert!(vfx_authored_mip_levels(&texture).is_none());
    }

    #[test]
    fn authored_cube_requires_six_complete_square_faces_per_mip() {
        let mut texture = VfxTextureInput {
            rgba: vec![0; 64],
            width: 4,
            height: 4,
            authored_mips: None,
            rgba16f_mips: None,
            cube_mips: Some(vec![
                VfxTextureCubeMipInput {
                    width: 4,
                    height: 4,
                    faces: std::array::from_fn(|_| vec![10; 64]),
                },
                VfxTextureCubeMipInput {
                    width: 2,
                    height: 2,
                    faces: std::array::from_fn(|_| vec![20; 16]),
                },
            ]),
            cube_format: xiv_companion_data::VfxTextureCubeFormat::Rgba8Unorm,
        };
        assert_eq!(vfx_authored_cube_mips(&texture).unwrap().len(), 2);
        texture.cube_mips.as_mut().unwrap()[1].faces[5].pop();
        assert!(vfx_authored_cube_mips(&texture).is_none());
        texture.cube_mips.as_mut().unwrap()[1].faces[5].push(20);
        texture.cube_mips.as_mut().unwrap()[1].width = 3;
        assert!(vfx_authored_cube_mips(&texture).is_none());
        texture.cube_mips.as_mut().unwrap()[1].width = 2;
        texture.height = 2;
        assert!(vfx_authored_cube_mips(&texture).is_none());

        texture.height = 4;
        texture.cube_format = xiv_companion_data::VfxTextureCubeFormat::Rgba16Float;
        assert!(vfx_authored_cube_mips(&texture).is_none());
        for level in texture.cube_mips.as_mut().unwrap() {
            for face in &mut level.faces {
                face.resize((level.width * level.height * 8) as usize, 0);
            }
        }
        assert_eq!(vfx_authored_cube_mips(&texture).unwrap().len(), 2);
    }

    #[test]
    fn authored_hdr_2d_requires_complete_half_float_mips() {
        let mut texture = fallback_vfx_texture_rgba();
        texture.width = 4;
        texture.height = 2;
        texture.rgba16f_mips = Some(vec![
            VfxTextureMipRgba16fInput {
                rgba16f: vec![0; 4 * 2 * 8],
                width: 4,
                height: 2,
            },
            VfxTextureMipRgba16fInput {
                rgba16f: vec![0; 2 * 8],
                width: 2,
                height: 1,
            },
        ]);
        assert_eq!(vfx_authored_rgba16f_mips(&texture).unwrap().len(), 2);
        texture.rgba16f_mips.as_mut().unwrap()[1].rgba16f.pop();
        assert!(vfx_authored_rgba16f_mips(&texture).is_none());
        texture.rgba16f_mips.as_mut().unwrap()[1].rgba16f.push(0);
        texture.rgba16f_mips.as_mut().unwrap()[1].width = 3;
        assert!(vfx_authored_rgba16f_mips(&texture).is_none());
    }

    #[test]
    fn decoded_hdr_cube_retains_format_through_render_input() {
        let face_zero = [0x4000_u16, 0, 0, 0x3c00]
            .into_iter()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        let source = xiv_companion_data::VfxTextureRgba {
            width: 1,
            height: 1,
            rgba: vec![255, 0, 0, 255],
            rgba16f_mips: vec![xiv_companion_data::VfxTextureMipRgba16f {
                width: 1,
                height: 1,
                rgba16f: face_zero.clone(),
            }],
            is_cube: true,
            cube_format: xiv_companion_data::VfxTextureCubeFormat::Rgba16Float,
            cube_mips: vec![xiv_companion_data::VfxTextureCubeMipRgba {
                width: 1,
                height: 1,
                faces: std::array::from_fn(|_| face_zero.clone()),
            }],
            ..Default::default()
        };
        let input = VfxTextureInput::from(&source);
        assert_eq!(
            input.cube_format,
            xiv_companion_data::VfxTextureCubeFormat::Rgba16Float
        );
        assert_eq!(vfx_authored_cube_mips(&input).unwrap()[0].faces[0].len(), 8);
        assert_eq!(
            vfx_authored_rgba16f_mips(&input).unwrap()[0].rgba16f,
            face_zero
        );
    }

    #[test]
    fn decoded_hdr_2d_retains_half_float_mips_through_render_input() {
        let source = xiv_companion_data::VfxTextureRgba {
            width: 1,
            height: 1,
            rgba: vec![255, 0, 0, 255],
            rgba16f_mips: vec![xiv_companion_data::VfxTextureMipRgba16f {
                width: 1,
                height: 1,
                rgba16f: [0x4000_u16, 0, 0, 0x3c00]
                    .into_iter()
                    .flat_map(u16::to_le_bytes)
                    .collect(),
            }],
            ..Default::default()
        };
        let input = VfxTextureInput::from(&source);
        assert!(input.cube_mips.is_none());
        assert_eq!(
            vfx_authored_rgba16f_mips(&input).unwrap()[0].rgba16f[0..2],
            0x4000_u16.to_le_bytes()
        );
    }

    #[test]
    fn vfx_group_params_keeps_shader_uniform_layout() {
        assert_eq!(std::mem::size_of::<VfxGroupParams>(), 96);
        assert_eq!(std::mem::align_of::<VfxGroupParams>(), 4);
    }

    #[test]
    fn polyline_shader_applies_per_point_end_distortion() {
        let source = include_str!("../shaders/vfx.wesl");
        let polyline = &source[source.find("fn vs_polyline").unwrap()..];
        assert!(polyline.contains("output.distortion_power *= vertex.previous.w;"));
    }

    #[test]
    fn line_gpu_instance_keeps_axes_endpoint_colors_and_line_topology() {
        let mut quad = laser_quad(2);
        quad.particle_type = Some(xiv_companion_data::avfx::ParticleType::Line);
        quad.laser = None;
        quad.line = Some(VfxLine {
            scale: [-2.0, 0.5, 3.0],
            length: -3.0,
            endpoint_offset: None,
            color_begin: [1.0, 2.0, 3.0, 4.0],
            color_end: [5.0, 6.0, 7.0, 8.0],
        });
        let gpu = GpuVfxQuad::from(&quad);
        assert_eq!(gpu.axis_x[..3], [-4.0, 0.0, 0.0]);
        assert_eq!(gpu.axis_y[..3], [0.0, 1.5, 0.0]);
        assert_eq!(gpu.disc_axis_z[..3], [0.0, 0.0, -12.0]);
        assert_eq!(gpu.disc_dimensions, [-3.0, 0.0, 0.0, 0.0]);
        assert_eq!(gpu.disc_color_inner, [1.0, 2.0, 3.0, 4.0]);
        assert_eq!(gpu.disc_color_outer, [5.0, 6.0, 7.0, 8.0]);
        assert_eq!(gpu.pivot_offset[3], 7.0);
        assert_eq!(vfx_quad_vertex_count(&quad), 2);
        assert_eq!(vfx_quad_double_group(&quad), None);
        let key = VfxGroupKey::of_quad(&quad);
        assert_eq!(key.textures, [-1; 4]);
        assert_eq!(key.texture_d, -1);
        assert!(!key.texture1_enabled);
        assert_eq!(key.texture1_channels, [false; 2]);
    }

    #[test]
    fn polygon_gpu_instance_keeps_three_scaled_axes_and_fan_count() {
        let mut quad = laser_quad(1);
        quad.particle_type = Some(xiv_companion_data::avfx::ParticleType::Polygon);
        quad.laser = None;
        quad.polygon = Some(VfxPolygon {
            count: 7,
            scale_z: 5.0,
        });
        let gpu = GpuVfxQuad::from(&quad);
        assert_eq!(gpu.axis_x[..3], [1.0, 0.0, 0.0]);
        assert_eq!(gpu.axis_y[..3], [0.0, 1.5, 0.0]);
        assert_eq!(gpu.disc_axis_z, [0.0, 0.0, -10.0, 7.0]);
        assert_eq!(gpu.pivot_offset[3], 8.0);
        assert_eq!(vfx_quad_vertex_count(&quad), 21);
        assert_eq!(vfx_quad_double_group(&quad), None);
    }

    #[test]
    fn line_gpu_instance_accepts_world_space_endpoint_offset() {
        let mut quad = laser_quad(2);
        quad.particle_type = Some(xiv_companion_data::avfx::ParticleType::Line);
        quad.laser = None;
        quad.line = Some(VfxLine {
            scale: [-2.0, 0.5, 3.0],
            length: 99.0,
            endpoint_offset: Some([1.25, -2.5, 3.75]),
            color_begin: [1.0; 4],
            color_end: [0.5; 4],
        });

        let gpu = GpuVfxQuad::from(&quad);
        assert_eq!(gpu.position_facing[3], 0.0);
        assert_eq!(gpu.axis_x[..3], [1.25, -2.5, 3.75]);
        assert_eq!(gpu.axis_y[..3], [0.0; 3]);
        assert_eq!(gpu.disc_axis_z[..3], [0.0; 3]);
        assert_eq!(gpu.disc_dimensions, [1.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn depth_offset_uses_reserved_gpu_lanes_and_rejects_unknown_types() {
        let mut quad = laser_quad(0);
        quad.laser = None;
        quad.particle_type = Some(xiv_companion_data::avfx::ParticleType::Quad);
        quad.depth_offset = 0.125;
        let gpu = GpuVfxQuad::from(&quad);
        assert_eq!(gpu.disc_color_outer[2..], [0.125, 0.0]);

        quad.depth_offset_type = 1;
        let gpu = GpuVfxQuad::from(&quad);
        assert_eq!(gpu.disc_color_outer[2..], [0.125, 1.0]);

        quad.depth_offset_type = 2;
        assert_eq!(GpuVfxQuad::from(&quad).disc_color_outer[2..], [0.0, 0.0]);
        quad.depth_offset_type = 0;
        quad.particle_type = Some(xiv_companion_data::avfx::ParticleType::Windmill);
        assert_eq!(GpuVfxQuad::from(&quad).disc_color_outer[2..], [0.0, 0.0]);
    }
}
