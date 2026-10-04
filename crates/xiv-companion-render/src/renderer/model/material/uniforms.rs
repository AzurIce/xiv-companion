use super::*;

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct SurfaceOverlayUniform {
    pub(crate) aura_params: [[f32; 4]; 16],
}

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
    camera_uniform_and_vfx_view(center, radius, viewport, yaw, pitch, zoom, pan, options).0
}

pub(crate) fn camera_uniform_and_binder(
    center: [f32; 3],
    radius: f32,
    viewport: [u32; 2],
    yaw: f32,
    pitch: f32,
    zoom: f32,
    pan: [f32; 2],
    options: ModelRenderOptions,
) -> (CameraUniform, xiv_companion_data::VfxBinderCameraSnapshot) {
    let (uniform, view) =
        camera_uniform_and_vfx_view(center, radius, viewport, yaw, pitch, zoom, pan, options);
    (uniform, view.camera)
}

pub(crate) fn camera_uniform_and_vfx_view(
    center: [f32; 3],
    radius: f32,
    viewport: [u32; 2],
    yaw: f32,
    pitch: f32,
    zoom: f32,
    pan: [f32; 2],
    options: ModelRenderOptions,
) -> (CameraUniform, xiv_companion_data::VfxCameraViewSnapshot) {
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
    let (right, up) = if options.camera_roll != 0.0 {
        let (sin_roll, cos_roll) = options.camera_roll.sin_cos();
        (
            right * cos_roll + up * sin_roll,
            up * cos_roll - right * sin_roll,
        )
    } else {
        (right, up)
    };
    let target = glam::Vec3::from(center) + (right * pan[0] + up * pan[1]) * radius;
    let eye = target + eye_offset;
    let view = glam::Mat4::look_at_rh(eye, target, up);
    let projection = glam::Mat4::perspective_rh(
        45_f32.to_radians(),
        aspect.max(0.1),
        0.01,
        distance + radius * 6.0,
    );
    let light =
        (right * PREVIEW_KEY_RIGHT + up * PREVIEW_KEY_UP + view_dir * PREVIEW_KEY_VIEW).normalize();

    let view_proj = projection * view;
    let binder = xiv_companion_data::VfxCameraViewSnapshot::from_view_matrices(
        view.to_cols_array(),
        view.inverse().to_cols_array(),
        viewport[1].max(1),
    );
    let uniform = CameraUniform {
        view_proj: view_proj.to_cols_array_2d(),
        inverse_view_proj: view_proj.inverse().to_cols_array_2d(),
        light_dir: [light.x, light.y, light.z, 0.0],
        view_dir: [view_dir.x, view_dir.y, view_dir.z, 0.0],
        camera_position: [eye.x, eye.y, eye.z, 1.0],
        view_right: [right.x, right.y, right.z, 0.0],
        view_up: [up.x, up.y, up.z, 0.0],
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
        instance_env_parameter: [options.instance_env_parameter_w, 0.0, 0.0, 0.0],
        viewport: [
            viewport[0].max(1) as f32,
            viewport[1].max(1) as f32,
            0.0,
            0.0,
        ],
    };
    (uniform, binder)
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct CameraUniform {
    pub(crate) view_proj: [[f32; 4]; 4],
    pub(crate) inverse_view_proj: [[f32; 4]; 4],
    pub(crate) light_dir: [f32; 4],
    pub(crate) view_dir: [f32; 4],
    pub(crate) camera_position: [f32; 4],
    pub(crate) options: [f32; 4],
    pub(crate) dynamic_emissive_color: [f32; 4],
    pub(crate) instance_env_parameter: [f32; 4],
    pub(crate) viewport: [f32; 4],
    pub(crate) view_right: [f32; 4],
    pub(crate) view_up: [f32; 4],
}

impl CameraUniform {
    pub(crate) fn up(&self) -> [f32; 3] {
        [self.view_up[0], self.view_up[1], self.view_up[2]]
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binder_camera_uses_view_rows_horizontal_direction_and_inverse_translation() {
        let (yaw, pitch, roll) = (0.65_f32, -0.3_f32, 0.7_f32);
        let (uniform, camera) = camera_uniform_and_binder(
            [2.0, -3.0, 1.0],
            2.0,
            [91, 67],
            yaw,
            pitch,
            3.0,
            [0.25, -0.4],
            ModelRenderOptions {
                camera_roll: roll,
                ..Default::default()
            },
        );
        // Analytic orbit axes, independent of look_at and matrix inversion.
        let z = glam::Vec3::new(
            yaw.sin() * pitch.cos(),
            pitch.sin(),
            yaw.cos() * pitch.cos(),
        );
        let right = glam::Vec3::new(yaw.cos(), 0.0, -yaw.sin());
        let up = glam::Vec3::new(
            -yaw.sin() * pitch.sin(),
            pitch.cos(),
            -yaw.cos() * pitch.sin(),
        );
        let r = right * roll.cos() + up * roll.sin();
        let u = up * roll.cos() - right * roll.sin();
        for (actual, expected) in
            camera
                .basis
                .into_iter()
                .zip([r.to_array(), u.to_array(), z.to_array()])
        {
            for (a, b) in actual.into_iter().zip(expected) {
                assert!((a - b).abs() < 1e-6);
            }
        }
        assert_eq!(camera.parallel_direction[1].to_bits(), 0);
        assert!((camera.parallel_direction[0] - z.x).abs() < 1e-6);
        assert!((camera.parallel_direction[2] - z.z).abs() < 1e-6);
        let eye = glam::Vec3::new(2.0, -3.0, 1.0) + (r * 0.25 + u * (-0.4)) * 2.0 + z * 6.0;
        for ((a, b), gpu) in camera
            .position
            .into_iter()
            .zip(eye.to_array())
            .zip(uniform.camera_position)
        {
            assert!((a - b).abs() < 2e-6);
            assert!((a - gpu).abs() < 2e-6);
        }
        assert_ne!(
            z.y, 0.0,
            "pitched camera must discriminate full vs horizontal direction"
        );
    }

    #[test]
    fn complete_camera_view_inverse_reconstructs_gpu_view_for_roll_pan_and_resize() {
        for viewport in [[91, 67], [91, 5120], [0, 0]] {
            let (uniform, view) = camera_uniform_and_vfx_view(
                [2.0, -3.0, 1.0],
                2.0,
                viewport,
                0.65,
                -0.3,
                3.0,
                [0.25, -0.4],
                ModelRenderOptions {
                    camera_roll: 0.7,
                    ..Default::default()
                },
            );
            view.validate().unwrap();
            assert_eq!(view.screen_height, viewport[1].max(1));
            let inverse = glam::Mat4::from_cols(
                glam::Vec3::from(view.inverse_view.basis[0]).extend(0.0),
                glam::Vec3::from(view.inverse_view.basis[1]).extend(0.0),
                glam::Vec3::from(view.inverse_view.basis[2]).extend(0.0),
                glam::Vec3::from(view.inverse_view.position).extend(1.0),
            );
            // Recover the affine view from the GPU's projective matrix using
            // an independently constructed projection, not the RoTp fields.
            let aspect = if viewport[1] == 0 {
                1.0
            } else {
                viewport[0] as f32 / viewport[1] as f32
            };
            let projection =
                glam::Mat4::perspective_rh(45_f32.to_radians(), aspect.max(0.1), 0.01, 18.0);
            let gpu_view =
                projection.inverse() * glam::Mat4::from_cols_array_2d(&uniform.view_proj);
            for (actual, expected) in (gpu_view * inverse)
                .to_cols_array()
                .into_iter()
                .zip(glam::Mat4::IDENTITY.to_cols_array())
            {
                assert!(
                    (actual - expected).abs() < 2e-4,
                    "viewport={viewport:?}: {actual} != {expected}"
                );
            }
            for (a, b) in view
                .inverse_view
                .position
                .into_iter()
                .zip(uniform.camera_position)
            {
                assert!((a - b).abs() < 2e-6);
            }
        }
    }

    #[test]
    fn compiled_shader_camera_layouts_match_cpu_uniform() {
        let offsets = [
            (
                "view_right",
                std::mem::offset_of!(CameraUniform, view_right) as u32,
            ),
            (
                "view_up",
                std::mem::offset_of!(CameraUniform, view_up) as u32,
            ),
            (
                "view_proj",
                std::mem::offset_of!(CameraUniform, view_proj) as u32,
            ),
            (
                "inverse_view_proj",
                std::mem::offset_of!(CameraUniform, inverse_view_proj) as u32,
            ),
            (
                "light_dir",
                std::mem::offset_of!(CameraUniform, light_dir) as u32,
            ),
            (
                "view_dir",
                std::mem::offset_of!(CameraUniform, view_dir) as u32,
            ),
            (
                "camera_position",
                std::mem::offset_of!(CameraUniform, camera_position) as u32,
            ),
            (
                "options",
                std::mem::offset_of!(CameraUniform, options) as u32,
            ),
            (
                "dynamic_emissive_color",
                std::mem::offset_of!(CameraUniform, dynamic_emissive_color) as u32,
            ),
            (
                "instance_env_parameter",
                std::mem::offset_of!(CameraUniform, instance_env_parameter) as u32,
            ),
            (
                "viewport",
                std::mem::offset_of!(CameraUniform, viewport) as u32,
            ),
        ];
        for (name, shader) in [
            (
                "model",
                include_str!(concat!(env!("OUT_DIR"), "/model.wgsl")),
            ),
            ("vfx", include_str!(concat!(env!("OUT_DIR"), "/vfx.wgsl"))),
            (
                "decal 1x",
                include_str!(concat!(env!("OUT_DIR"), "/vfx_decal_1x.wgsl")),
            ),
            (
                "decal 4x",
                include_str!(concat!(env!("OUT_DIR"), "/vfx_decal_4x.wgsl")),
            ),
        ] {
            let module = wgpu::naga::front::wgsl::parse_str(shader).unwrap();
            let camera = module
                .types
                .iter()
                .find_map(|(_, ty)| (ty.name.as_deref() == Some("Camera")).then_some(&ty.inner))
                .expect("compiled shader must contain Camera");
            let wgpu::naga::TypeInner::Struct { members, span } = camera else {
                panic!("{name}: Camera must be a struct");
            };
            assert_eq!(
                *span as usize,
                std::mem::size_of::<CameraUniform>(),
                "{name}"
            );
            for (field, offset) in offsets {
                let member = members
                    .iter()
                    .find(|m| m.name.as_deref() == Some(field))
                    .unwrap_or_else(|| panic!("{name}: missing Camera.{field}"));
                assert_eq!(member.offset, offset, "{name}: Camera.{field}");
            }
        }
    }

    #[test]
    fn camera_up_is_independent_of_projection_scale() {
        for radius in [0.1, 1.0, 10000.0] {
            let uniform = camera_uniform(
                [0.0; 3],
                radius,
                [64, 64],
                0.0,
                0.0,
                3.0,
                [0.0; 2],
                ModelRenderOptions {
                    camera_roll: std::f32::consts::FRAC_PI_2,
                    ..Default::default()
                },
            );
            assert!((glam::Vec3::from_array(uniform.up()) + glam::Vec3::X).length() < 1e-6);
        }
    }

    #[test]
    fn pitched_camera_up_matches_the_view_rotation_without_projection_error() {
        let (yaw, pitch) = (0.65_f32, -0.3_f32);
        let uniform = camera_uniform(
            [0.0; 3],
            1.0,
            [64, 64],
            yaw,
            pitch,
            3.0,
            [0.0; 2],
            ModelRenderOptions::default(),
        );
        let expected = glam::Vec3::new(
            -yaw.sin() * pitch.sin(),
            pitch.cos(),
            -yaw.cos() * pitch.sin(),
        );
        let error = (glam::Vec3::from_array(uniform.up()) - expected).length();
        assert!(error < 1e-6, "camera up rotation error {error}");
    }

    #[test]
    fn preview_camera_roll_rotates_view_axes_without_moving_the_eye() {
        let make = |camera_roll| {
            camera_uniform(
                [0.0; 3],
                1.0,
                [64, 64],
                0.0,
                0.0,
                3.0,
                [0.0; 2],
                ModelRenderOptions {
                    camera_roll,
                    ..Default::default()
                },
            )
        };
        let level = make(0.0);
        let rolled = make(std::f32::consts::FRAC_PI_2);
        assert_eq!(level.camera_position, rolled.camera_position);
        assert_eq!(level.view_dir, rolled.view_dir);
        assert!((glam::Vec3::from_array(level.up()) - glam::Vec3::Y).length() < 1.0e-5);
        assert!((glam::Vec3::from_array(rolled.up()) + glam::Vec3::X).length() < 1.0e-5);
        assert_eq!(make(f32::NAN).view_proj, level.view_proj);
    }
}
