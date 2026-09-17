use super::*;

pub(crate) const DEFAULT_EXPOSURE: f32 = 1.0;

// Stable camera-relative preview-lighting contract. These coefficients define
// the key direction in the camera basis and intentionally live outside
// material semantics: shader/material fixes must not compensate by silently
// moving the preview light. The matching color/intensity/environment constants
// are centralized as `PREVIEW_*` constants in `model.wgsl`.
pub(crate) const PREVIEW_KEY_RIGHT: f32 = -0.45;
pub(crate) const PREVIEW_KEY_UP: f32 = 0.65;
pub(crate) const PREVIEW_KEY_VIEW: f32 = 0.65;

pub(crate) fn camera_uniform(
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
    let light =
        (right * PREVIEW_KEY_RIGHT + up * PREVIEW_KEY_UP + view_dir * PREVIEW_KEY_VIEW).normalize();

    CameraUniform {
        view_proj: (projection * view).to_cols_array_2d(),
        light_dir: [light.x, light.y, light.z, 0.0],
        view_dir: [view_dir.x, view_dir.y, view_dir.z, 0.0],
        camera_position: [eye.x, eye.y, eye.z, 1.0],
        options: [
            if options.normal_mapping { 1.0 } else { 0.0 },
            options.normal_y_sign,
            options.uv_scroll_time,
            options.debug_mode.shader_value(),
        ],
        dynamic_emissive_color: [
            options.dynamic_emissive_color[0],
            options.dynamic_emissive_color[1],
            options.dynamic_emissive_color[2],
            0.0,
        ],
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct CameraUniform {
    pub(crate) view_proj: [[f32; 4]; 4],
    pub(crate) light_dir: [f32; 4],
    pub(crate) view_dir: [f32; 4],
    pub(crate) camera_position: [f32; 4],
    pub(crate) options: [f32; 4],
    pub(crate) dynamic_emissive_color: [f32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct PostUniform {
    pub(crate) params: [f32; 4],
}

/// joint storage buffer 头：实际 joint 数（0 = 实例无骨架，shader 走非蒙皮
/// 旧分支）+ 对齐填充。
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct JointStorageHeader {
    pub(crate) count: u32,
    pub(crate) _pad: [u32; 3],
}

/// Compose uniform: bloom strength, exposure, and whether the shader must
/// encode sRGB itself because the target surface is not an sRGB format.
/// sRGB targets encode on write, so the shader must skip its own encode to
/// keep output encoding happening exactly once.
pub(crate) fn compose_post_params(
    bloom_strength: f32,
    target_format: wgpu::TextureFormat,
) -> [f32; 4] {
    [
        bloom_strength,
        DEFAULT_EXPOSURE,
        if target_format.is_srgb() { 0.0 } else { 1.0 },
        0.0,
    ]
}
