// VFX particle shader, standalone package module compiled to OUT_DIR/vfx.wgsl
// by the build script. Draws avfx particle quads (billboard or oriented) and
// DrawModel mesh particles with TC1 as the base, TC2..TC4 combine modes,
// per-layer color-to-alpha and per-group draw mode, fed by the CPU avfx sampler
// (`xiv_companion_data::avfx_sim`).

import package::camera::Camera;

struct VfxGroupParams {
    // TC2/TC3/TC4 的颜色合成（TCCT：0 乘、1 加、2 减、3 max、4 min；
    // 5 不合成；未知值也跳过）。
    combine_color: vec3<u32>,
    // TC2/TC3/TC4 的 alpha 合成（TCAT：0 乘、1 max、2 min、3 不合成）。
    combine_alpha: vec3<u32>,
    texture1_channels: vec2<u32>,
    // 各层 bC2A 位掩码。
    color_to_alpha: u32,
    // bit1..4: 各层有贴图；bit5: TD；bit6: TC1；bit7: Powder；bit8: TP。
    flags: u32,
    // 各层 UvSet 序号（2bit × TC1..TC4）+ TD（bit8-9）（网格模式按它选
    // 逐顶点基底 UV；四边形基底恒为角点）。
    uv_set_sel: u32,
    // bit0..3: TC1..TC4 use normalized fragment pixel position; bit4: TD.
    pixel_uv_mask: u32,
    auxiliary_lod_bias: vec4<f32>,
    texture_lod_bias: vec4<f32>,
};

@group(0) @binding(0) var<uniform> camera: Camera;
@group(1) @binding(0) var vfx_texture1: texture_2d<f32>;
@group(1) @binding(1) var vfx_texture2: texture_2d<f32>;
@group(1) @binding(2) var vfx_texture3: texture_2d<f32>;
@group(1) @binding(3) var vfx_texture4: texture_2d<f32>;
@group(1) @binding(4) var vfx_texture_d: texture_2d<f32>;
@group(1) @binding(5) var<uniform> vfx_params: VfxGroupParams;
@group(1) @binding(6) var vfx_sampler1: sampler;
@group(1) @binding(7) var vfx_sampler2: sampler;
@group(1) @binding(8) var vfx_sampler3: sampler;
@group(1) @binding(9) var vfx_sampler4: sampler;
@group(1) @binding(10) var vfx_sampler_d: sampler;
@group(1) @binding(12) var vfx_texture_p: texture_2d<f32>;
@group(1) @binding(13) var vfx_sampler_p: sampler;
@group(1) @binding(15) var vfx_texture_n: texture_2d<f32>;
@group(1) @binding(16) var vfx_sampler_n: sampler;

struct VfxPolylineVertex {
    // xyz center point; w signed width (-Wd, 0, +Wd).
    center_width: vec4<f32>,
    // xyz previous center point; w PnED distortion multiplier.
    previous: vec4<f32>,
    next: vec4<f32>,
    // xyz NBBA-selected instance axis; w enables the non-billboard path.
    width_axis: vec4<f32>,
    color: vec4<f32>,
    uv1: vec4<f32>,
    uv2: vec4<f32>,
    uv3: vec4<f32>,
    uv4: vec4<f32>,
    uvd: vec4<f32>,
    distortion: vec4<u32>,
    // xy base UV; z TD rotation; w UVPT.
    base_uv: vec4<f32>,
    soft_particle_params: vec4<f32>,
};

@group(1) @binding(14) var<storage, read> polyline_vertices: array<VfxPolylineVertex>;

// Soft-particle variants use one of these depth resources in a later pass;
// keeping both declarations in the shared module lets the 1x/4x pipelines
// select the matching binding without changing the ordinary VFX layout.
@group(2) @binding(0) var soft_scene_depth_1x: texture_depth_2d;
@group(2) @binding(1) var soft_scene_depth_4x: texture_depth_multisampled_2d;

@vertex
fn vs_depth_copy(@builtin(vertex_index) vertex_index: u32) -> @builtin(position) vec4<f32> {
    let positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    return vec4<f32>(positions[vertex_index], 0.0, 1.0);
}

@fragment
fn fs_depth_copy_1x(@builtin(position) position: vec4<f32>) -> @builtin(frag_depth) f32 {
    return textureLoad(soft_scene_depth_1x, vec2<i32>(position.xy), 0);
}

@fragment
fn fs_depth_copy_4x(
    @builtin(position) position: vec4<f32>,
    @builtin(sample_index) sample_index: u32,
) -> @builtin(frag_depth) f32 {
    return textureLoad(soft_scene_depth_4x, vec2<i32>(position.xy), sample_index);
}
@group(2) @binding(2) var vfx_screen_copy: texture_2d<f32>;
@group(2) @binding(3) var vfx_previous_scene: texture_2d<f32>;
@group(2) @binding(4) var vfx_reflection_cube: texture_cube<f32>;
@group(2) @binding(5) var vfx_reflection_sampler: sampler;
@group(1) @binding(17) var vfx_file_reflection_cube: texture_cube<f32>;
@group(1) @binding(18) var vfx_file_reflection_sampler: sampler;
@group(2) @binding(6) var vfx_portrait: texture_2d<f32>;
@group(2) @binding(7) var vfx_portrait_sampler: sampler;

fn polyline_uv(uv: vec2<f32>, precision_scale: f32) -> vec2<f32> {
    let packed = vec2<i32>(trunc(uv * precision_scale));
    let signed_low_word = (packed << vec2<u32>(16u)) >> vec2<u32>(16u);
    return vec2<f32>(signed_low_word) / max(precision_scale, 1.0);
}

struct VfxInstanceInput {
    // xyz: 位置；w: 原始 RBDT。
    @location(0) position_facing: vec4<f32>,
    // xyz: 完整变换后的半宽向量；w: TD 的 UV 旋转。
    @location(1) axis_x: vec4<f32>,
    // HDR 颜色（rgb = Col.RGB × Bri；a = Col.A 包络 × SclA）。
    @location(2) color: vec4<f32>,
    // 四层贴图的 UV（原点 + 缩放）。
    @location(3) uv1: vec4<f32>,
    @location(4) uv2: vec4<f32>,
    @location(5) uv3: vec4<f32>,
    @location(6) uv4: vec4<f32>,
    // TD 扭曲贴图采样 UV。
    @location(7) uvd: vec4<f32>,
    // x：TD 强度位模式；y：低 4 位 TD 目标、bit8..15 TP 偏移；zw：UV 旋转。
    @location(8) distortion: vec4<u32>,
    // xyz: 完整变换后的半高向量。
    @location(9) axis_y: vec4<f32>,
    // xyz: pivot offset; w: 0 Quad, 1 single Powder, 2 Smpl, 3/4 Windmill.
    @location(10) pivot_offset: vec4<f32>,
    @location(11) disc_axis_z: vec4<f32>,
    @location(12) disc_dimensions: vec4<f32>,
    @location(13) disc_heights: vec4<f32>,
    @location(14) disc_color_inner: vec4<f32>,
    @location(15) disc_color_outer: vec4<f32>,
};

struct VfxVertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv1: vec2<f32>,
    @location(1) uv2: vec2<f32>,
    @location(2) uv3: vec2<f32>,
    @location(3) uv4: vec2<f32>,
    @location(4) uvd: vec2<f32>,
    @location(5) distortion_power: f32,
    @location(6) color: vec4<f32>,
    @location(7) @interpolate(flat) distortion_targets: u32,
    @location(8) normal: vec3<f32>,
    @location(9) fresnel_direction_power: vec4<f32>,
    @location(10) @interpolate(flat) fresnel_color_begin: vec4<f32>,
    @location(11) @interpolate(flat) fresnel_color_end: vec4<f32>,
    @location(12) @interpolate(flat) fresnel_enabled: u32,
    // x = AvfxMain.SPFR, y = Ptcl.DsSp. Ordinary paths keep both at zero.
    @location(13) @interpolate(flat) soft_particle_params: vec4<f32>,
};

// AVFXTools InitQuad faces local -Z; culling uses counterclockwise front faces.
const CORNERS: array<vec2<f32>, 6> = array<vec2<f32>, 6>(
    vec2<f32>(-1.0, -1.0),
    vec2<f32>(1.0, 1.0),
    vec2<f32>(1.0, -1.0),
    vec2<f32>(-1.0, -1.0),
    vec2<f32>(-1.0, 1.0),
    vec2<f32>(1.0, 1.0),
);

// Client Powder 0x1403fc030: corners TL/TR/BL/BR, indices 3/1/0, 0/2/3.
const POWDER_CORNERS: array<vec2<f32>, 6> = array<vec2<f32>, 6>(
    vec2<f32>(1.0, -1.0),
    vec2<f32>(1.0, 1.0),
    vec2<f32>(-1.0, 1.0),
    vec2<f32>(-1.0, 1.0),
    vec2<f32>(-1.0, -1.0),
    vec2<f32>(1.0, -1.0),
);

fn direction_basis(
    direction: vec3<f32>,
    up: vec3<f32>,
    retry_up: vec3<f32>,
) -> mat3x3<f32> {
    if dot(direction, direction) < 1.0e-4 {
        return mat3x3<f32>();
    }
    let forward = normalize(direction);
    var right = vec3<f32>(0.0);
    if dot(up, up) > 0.0 {
        let normalized_up = normalize(up);
        if abs(dot(normalized_up, forward)) <= 0.99 {
            right = cross(normalized_up, forward);
        }
    }
    if dot(right, right) == 0.0 && dot(retry_up, retry_up) > 0.0 {
        let normalized_retry = normalize(retry_up);
        if abs(dot(normalized_retry, forward)) <= 0.99 {
            right = cross(normalized_retry, forward);
        }
    }
    if dot(right, right) == 0.0 {
        return mat3x3<f32>();
    }
    right = normalize(right);
    return mat3x3<f32>(right, cross(forward, right), forward);
}

fn tree_billboard_linear(old: mat3x3<f32>, position: vec3<f32>) -> mat3x3<f32> {
    // Client 0x1403cd8c2 replaces the old matrix while preserving its Y axis
    // and the length of each column. A parallel camera ray has no valid basis.
    let old_y = old[1];
    let raw_x = cross(position - camera.camera_position.xyz, old_y);
    let y_length = length(old_y);
    let x_length = length(raw_x);
    if y_length == 0.0 || x_length == 0.0 {
        return mat3x3<f32>();
    }
    let x = raw_x / x_length;
    let y = old_y / y_length;
    let z = normalize(cross(x, old_y));
    return mat3x3<f32>(x * length(old[0]), y * y_length, z * length(old[2]));
}

fn camera_axes() -> mat3x3<f32> {
    // Use the original view rotation, including roll. Recovering these axes
    // from inverse_view_proj adds inversion error amplified at texture seams.
    return mat3x3<f32>(camera.view_right.xyz, camera.view_up.xyz, camera.view_dir.xyz);
}

// RBDT 3/7 use instance +0x174. Modes 4/8 constrain camera facing to world Y.
fn facing_basis(
    mode: i32,
    position: vec3<f32>,
    movement_direction: vec3<f32>,
    parent_y: vec3<f32>,
    parent_z: vec3<f32>,
) -> mat3x3<f32> {
    if mode == 3 {
        let basis = direction_basis(movement_direction, parent_y, -parent_z);
        if dot(basis[0], basis[0]) > 0.0 {
            return basis;
        }
        return mat3x3<f32>(
            vec3<f32>(1.0, 0.0, 0.0),
            vec3<f32>(0.0, 1.0, 0.0),
            vec3<f32>(0.0, 0.0, 1.0),
        );
    }
    if mode == 7 {
        let camera_up = camera.camera_position.xyz - position;
        var basis = direction_basis(movement_direction, camera_up, parent_y);
        if dot(basis[0], basis[0]) == 0.0 {
            basis = direction_basis(movement_direction, parent_y, -parent_z);
        }
        if dot(basis[0], basis[0]) > 0.0 {
            return basis;
        }
        return mat3x3<f32>(
            vec3<f32>(1.0, 0.0, 0.0),
            vec3<f32>(0.0, 1.0, 0.0),
            vec3<f32>(0.0, 0.0, 1.0),
        );
    }
    if mode == 6 {
        // Client 0x1403cde99 uses the camera position as target and the
        // inverse-view Y axis as up, retrying a +Z target on failure.
        let camera_up = camera_axes()[1];
        var basis = direction_basis(camera.camera_position.xyz - position, camera_up, vec3<f32>(0.0));
        if dot(basis[0], basis[0]) == 0.0 {
            basis = direction_basis(vec3<f32>(0.0, 0.0, 1.0), camera_up, vec3<f32>(0.0));
        }
        if dot(basis[0], basis[0]) > 0.0 {
            return basis;
        }
        return mat3x3<f32>(
            vec3<f32>(1.0, 0.0, 0.0),
            vec3<f32>(0.0, 1.0, 0.0),
            vec3<f32>(0.0, 0.0, 1.0),
        );
    }
    if mode == 5 {
        let axes = camera_axes();
        return mat3x3<f32>(-axes[0], axes[1], -axes[2]);
    }
    if mode < 4 || mode > 8 {
        return mat3x3<f32>(
            vec3<f32>(1.0, 0.0, 0.0),
            vec3<f32>(0.0, 1.0, 0.0),
            vec3<f32>(0.0, 0.0, 1.0),
        );
    }
    var forward = position - camera.camera_position.xyz;
    if mode == 4 {
        forward = -camera.view_dir.xyz;
    }
    if mode == 8 {
        // Client 0x1403cd843 targets the camera's X/Z position from the particle.
        forward = camera.camera_position.xyz - position;
    }
    if mode == 4 || mode == 8 {
        forward.y = 0.0;
        // Client 0x1403cd693/0x1403cd843 retries a degenerate constrained
        // target with +Z before composing the billboard matrix.
        if dot(forward, forward) < 1.0e-4 {
            forward = vec3<f32>(0.0, 0.0, 1.0);
        }
    }
    // System.Numerics CreateBillboard uses -cameraForward when positions coincide.
    // camera.view_dir points from the scene toward the eye.
    if dot(forward, forward) < 1.0e-4 {
        forward = camera.view_dir.xyz;
    }
    forward = normalize(forward);
    var right = cross(vec3<f32>(0.0, 1.0, 0.0), forward);
    if dot(right, right) < 1.0e-8 {
        right = vec3<f32>(1.0, 0.0, 0.0);
    }
    right = normalize(right);
    let up = cross(forward, right);
    return mat3x3<f32>(right, up, forward);
}

// 基底 UV 绕 (0.5,0.5) 旋转后缩放加滚动（AVFXTools `MoveUV`：
// scroll + 0.5 + scale·(rot(base)−0.5)）。
fn move_uv(transform: vec4<f32>, base: vec2<f32>, rot: f32) -> vec2<f32> {
    let c = cos(rot);
    let s = sin(rot);
    let rel = base - vec2<f32>(0.5, 0.5);
    let rotated = vec2<f32>(c * rel.x + s * rel.y, c * rel.y - s * rel.x)
        + vec2<f32>(0.5, 0.5);
    return transform.xy + vec2<f32>(0.5, 0.5) + transform.zw * (rotated - vec2<f32>(0.5, 0.5));
}

fn fill_output(
    world: vec4<f32>,
    uv_transforms: array<vec4<f32>, 4>,
    base_uvs: array<vec2<f32>, 4>,
    uvd: vec4<f32>,
    uvd_rotation: f32,
    distortion: vec4<u32>,
    color: vec4<f32>,
) -> VfxVertexOutput {
    var output: VfxVertexOutput;
    output.clip_position = camera.view_proj * world;
    // 每层基底 UV 按 UvSN 从 4 组基底中选择（网格粒子：逐顶点四组 UV；
    // 四边形：四角恒同）。distortion.zw 打包了四层 UvSet 旋转角
    // （u16×2 量化，见 Rust 侧 pack_uv_rot）。
    let sel1 = (vfx_params.uv_set_sel) & 3u;
    let sel2 = (vfx_params.uv_set_sel >> 2u) & 3u;
    let sel3 = (vfx_params.uv_set_sel >> 4u) & 3u;
    let sel4 = (vfx_params.uv_set_sel >> 6u) & 3u;
    let seld = (vfx_params.uv_set_sel >> 8u) & 3u;
    let rots_packed01 = distortion.z;
    let rots_packed23 = distortion.w;
    let rots01 = vec2<f32>(
        f32(rots_packed01 & 0xFFFFu) / 65535.0,
        f32(rots_packed01 >> 16u) / 65535.0,
    ) * (2.0 * 3.14159265);
    let rots23 = vec2<f32>(
        f32(rots_packed23 & 0xFFFFu) / 65535.0,
        f32(rots_packed23 >> 16u) / 65535.0,
    ) * (2.0 * 3.14159265);
    output.uv1 = move_uv(uv_transforms[0], base_uvs[sel1], rots01.x);
    output.uv2 = move_uv(uv_transforms[1], base_uvs[sel2], rots01.y);
    output.uv3 = move_uv(uv_transforms[2], base_uvs[sel3], rots23.x);
    output.uv4 = move_uv(uv_transforms[3], base_uvs[sel4], rots23.y);
    output.uvd = move_uv(uvd, base_uvs[seld], uvd_rotation);
    output.distortion_power = bitcast<f32>(distortion.x);
    output.distortion_targets = distortion.y;
    output.color = color;
    output.soft_particle_params = vec4<f32>(0.0);
    return output;
}

// Apricot's Legacy and FixedIntervalNDC variants use offset / w and offset * w
// respectively, then clamp positive-w vertices to the far clip plane.
fn apply_depth_offset(clip: vec4<f32>, depth_offset: f32, offset_type: f32) -> vec4<f32> {
    var adjusted = clip;
    if (clip.w > 0.0) {
        let delta = select(depth_offset / clip.w, depth_offset * clip.w, offset_type > 0.5);
        adjusted.z = min(clip.w, clip.z + delta);
    }
    return adjusted;
}

@vertex
fn vs_polyline(@builtin(vertex_index) vertex_index: u32) -> VfxVertexOutput {
    let vertex = polyline_vertices[vertex_index];
    var direction = vec3<f32>(0.0);
    if vertex.width_axis.w != 0.0 {
        let length_squared = dot(vertex.width_axis.xyz, vertex.width_axis.xyz);
        direction = vertex.width_axis.xyz * inverseSqrt(length_squared);
    } else {
        let tangent = vertex.previous.xyz - vertex.next.xyz;
        let camera_ray = vertex.center_width.xyz - camera.camera_position.xyz;
        let perpendicular = cross(tangent, camera_ray);
        let length_squared = dot(perpendicular, perpendicular);
        if length_squared >= 1.0e-10 {
            direction = perpendicular * inverseSqrt(length_squared);
        }
    }
    let world = vertex.center_width.xyz + direction * vertex.center_width.w;
    let base_uvs = array<vec2<f32>, 4>(
        vertex.base_uv.xy,
        vertex.base_uv.xy,
        vertex.base_uv.xy,
        vertex.base_uv.xy,
    );
    var output = fill_output(
        vec4<f32>(world, 1.0),
        array<vec4<f32>, 4>(vertex.uv1, vertex.uv2, vertex.uv3, vertex.uv4),
        base_uvs,
        vertex.uvd,
        vertex.base_uv.z,
        vertex.distortion,
        vertex.color,
    );
    output.distortion_power *= vertex.previous.w;
    let precision_index = u32(i32(vertex.base_uv.w)) & 3u;
    let precision_scale = array<f32, 4>(1000.0, 200.0, 100.0, 0.0)[precision_index];
    output.uv1 = polyline_uv(output.uv1, precision_scale);
    output.uv2 = polyline_uv(output.uv2, precision_scale);
    output.uv3 = polyline_uv(output.uv3, precision_scale);
    output.uv4 = polyline_uv(output.uv4, precision_scale);
    output.uvd = polyline_uv(output.uvd, precision_scale);
    output.soft_particle_params = vertex.soft_particle_params;
    return output;
}

// Disc streams signed short UVs in thousandths after applying UvSt.
fn disc_uv(uv: vec2<f32>) -> vec2<f32> {
    let packed = vec2<i32>(trunc(uv * 1000.0));
    return vec2<f32>((packed << vec2<u32>(16u)) >> vec2<u32>(16u)) * 0.001;
}

fn disc_vertex(vertex_index: u32, instance: VfxInstanceInput) -> VfxVertexOutput {
    let counts = u32(instance.disc_axis_z.w);
    let parts = counts & 255u;
    let count_u = (counts >> 8u) & 255u;
    let count_v = (counts >> 16u) & 255u;
    let cells = (count_u - 1u) * (count_v - 1u);
    let part = vertex_index / (cells * 6u);
    let cell = (vertex_index / 6u) % cells;
    // Client 0x1403e9b30: a,b,c / c,b,d in a U-major grid.
    let offsets = array<vec2<u32>, 6>(
        vec2<u32>(0u, 0u), vec2<u32>(1u, 0u), vec2<u32>(0u, 1u),
        vec2<u32>(0u, 1u), vec2<u32>(1u, 0u), vec2<u32>(1u, 1u),
    );
    let point = vec2<u32>(cell % (count_u - 1u), cell / (count_u - 1u))
        + offsets[vertex_index % 6u];
    let uv = vec2<f32>(point) / vec2<f32>(f32(count_u - 1u), f32(count_v - 1u));
    let fourth = uv.x * uv.x * uv.x * uv.x;
    let radial = uv.x + (fourth - uv.x) * instance.pivot_offset.x;
    let radius = mix(instance.disc_dimensions.x, instance.disc_dimensions.y, uv.y);
    let width = mix(instance.disc_dimensions.z, instance.disc_dimensions.w, uv.y) * instance.pivot_offset.y;
    let height_inner = mix(instance.disc_heights.x, instance.disc_heights.y, uv.y);
    let height_outer = mix(instance.disc_heights.z, instance.disc_heights.w, uv.y);
    let angle = f32(part) * (2.0 * 3.14159265) / f32(parts) + uv.y * instance.axis_y.w;
    let distance = mix(radius - width, radius + width, radial);
    let point_local = vec3<f32>(cos(angle) * distance, mix(height_inner, height_outer, radial), -sin(angle) * distance);
    let position = instance.position_facing.xyz;
    let mode = i32(instance.position_facing.w);
    let old = mat3x3<f32>(instance.axis_x.xyz, instance.axis_y.xyz, instance.disc_axis_z.xyz);
    var linear = facing_basis(mode, position, instance.disc_axis_z.xyz,
        instance.disc_dimensions.xyz, instance.disc_heights.xyz) * old;
    if mode == 9 {
        linear = tree_billboard_linear(old, position);
    }
    let world = position + linear * point_local;
    // Geometry uses the warped fraction; color and UV use the regular U grid.
    let color = floor(clamp(mix(instance.disc_color_inner, instance.disc_color_outer, uv.x), vec4<f32>(0.0), vec4<f32>(10.0)) * 1000.0) * 0.001;
    var output = fill_output(
        vec4<f32>(world, 1.0),
        array<vec4<f32>, 4>(instance.uv1, instance.uv2, instance.uv3, instance.uv4),
        array<vec2<f32>, 4>(uv, uv, uv, uv),
        instance.uvd,
        instance.axis_x.w,
        instance.distortion,
        color,
    );
    output.uv1 = disc_uv(output.uv1);
    output.uv2 = disc_uv(output.uv2);
    output.uv3 = disc_uv(output.uv3);
    output.uv4 = disc_uv(output.uv4);
    output.uvd = disc_uv(output.uvd);
    output.soft_particle_params = vec4<f32>(instance.color.xy, 0.0, 0.0);
    return output;
}

fn polygon_vertex(vertex_index: u32, instance: VfxInstanceInput) -> VfxVertexOutput {
    let count = u32(instance.disc_axis_z.w);
    let triangle = vertex_index / 3u;
    let corner = vertex_index % 3u;
    var base_uv = vec2<f32>(0.5);
    var local = vec3<f32>(0.0);
    if corner != 2u {
        let rim = select(triangle, (triangle + 1u) % count, corner == 0u);
        let angle = f32(rim) * (2.0 * 3.14159265) / f32(count) + 0.5 * 3.14159265;
        let radial = vec2<f32>(sin(angle), cos(angle));
        base_uv += radial * 0.5;
        let mode = i32(instance.position_facing.w);
        if mode == 0 {
            local = instance.disc_axis_z.xyz * radial.x - instance.axis_y.xyz * radial.y;
        } else if mode == 1 {
            local = instance.axis_x.xyz * radial.x - instance.disc_axis_z.xyz * radial.y;
        } else {
            local = instance.axis_x.xyz * radial.x - instance.axis_y.xyz * radial.y;
        }
    }
    var output = fill_output(
        vec4<f32>(instance.position_facing.xyz + local, 1.0),
        array<vec4<f32>, 4>(instance.uv1, instance.uv2, instance.uv3, instance.uv4),
        array<vec2<f32>, 4>(base_uv, base_uv, base_uv, base_uv),
        instance.uvd,
        instance.axis_x.w,
        instance.distortion,
        instance.color,
    );
    output.uv1 = disc_uv(output.uv1);
    output.uv2 = disc_uv(output.uv2);
    output.uv3 = disc_uv(output.uv3);
    output.uv4 = disc_uv(output.uv4);
    output.uvd = disc_uv(output.uvd);
    output.soft_particle_params = vec4<f32>(instance.disc_color_outer.xy, 0.0, 0.0);
    return output;
}

fn line_vertex(vertex_index: u32, instance: VfxInstanceInput) -> VfxVertexOutput {
    let axes = array<vec3<f32>, 3>(
        instance.axis_x.xyz,
        instance.axis_y.xyz,
        instance.disc_axis_z.xyz,
    );
    let axis_index = clamp(i32(instance.position_facing.w), 0, 2);
    let endpoint = min(vertex_index, 1u);
    let along = f32(endpoint);
    let world = instance.position_facing.xyz
        + axes[axis_index] * instance.disc_dimensions.x * along;
    let base_uv = vec2<f32>(along, 0.0);
    var output = fill_output(
        vec4<f32>(world, 1.0),
        array<vec4<f32>, 4>(instance.uv1, instance.uv2, instance.uv3, instance.uv4),
        array<vec2<f32>, 4>(base_uv, base_uv, base_uv, base_uv),
        instance.uvd,
        instance.axis_x.w,
        instance.distortion,
        mix(instance.disc_color_inner, instance.disc_color_outer, along),
    );
    output.soft_particle_params = vec4<f32>(instance.color.xy, 0.0, 0.0);
    return output;
}

const LASER_INDICES: array<u32, 18> = array<u32, 18>(
    0u, 1u, 3u, 3u, 2u, 0u,
    2u, 3u, 5u, 5u, 4u, 2u,
    4u, 5u, 7u, 7u, 6u, 4u,
);

fn laser_projected_direction(point: vec3<f32>, tangent: vec3<f32>) -> vec3<f32> {
    let camera_ray = point - camera.camera_position.xyz;
    let projected = cross(camera.view_dir.xyz, cross(camera_ray, tangent));
    let length_squared = dot(projected, projected);
    if length_squared > 1.0e-12 {
        return projected * inverseSqrt(length_squared);
    }
    let tangent_squared = dot(tangent, tangent);
    if tangent_squared > 1.0e-12 {
        return tangent * inverseSqrt(tangent_squared);
    }
    return vec3<f32>(0.0);
}

fn laser_width_direction(point: vec3<f32>, tangent: vec3<f32>) -> vec3<f32> {
    let camera_ray = point - camera.camera_position.xyz;
    // Client stores the negated camera-ray cross product before emitting the
    // minus/plus vertex pair.
    let perpendicular = cross(tangent, camera_ray);
    let length_squared = dot(perpendicular, perpendicular);
    if length_squared > 1.0e-12 {
        return perpendicular * inverseSqrt(length_squared);
    }
    return vec3<f32>(0.0);
}

fn laser_vertex(vertex_index: u32, instance: VfxInstanceInput) -> VfxVertexOutput {
    let axes = array<vec3<f32>, 3>(
        instance.axis_x.xyz,
        instance.axis_y.xyz,
        instance.disc_axis_z.xyz,
    );
    let axis_index = clamp(i32(instance.position_facing.w), 0, 2);
    let start = instance.position_facing.xyz;
    let end = start + axes[axis_index] * instance.disc_dimensions.x;
    let beam = end - start;
    let width = instance.disc_dimensions.y;
    let points = array<vec3<f32>, 4>(
        start - laser_projected_direction(start, beam) * width,
        start,
        end,
        end + laser_projected_direction(end, beam) * width,
    );
    let packed_index = LASER_INDICES[vertex_index];
    let point_index = packed_index / 2u;
    let side = packed_index & 1u;
    let previous_index = max(point_index, 1u) - 1u;
    let previous = points[previous_index];
    let next = points[min(point_index + 1u, 3u)];
    let offset = laser_width_direction(points[point_index], previous - next) * width;
    let world = points[point_index] + select(-offset, offset, side == 1u);
    let along = array<f32, 4>(0.0, 0.5, 0.5, 0.0)[point_index];
    let base_uv = vec2<f32>(f32(side), along);
    var output = fill_output(
        vec4<f32>(world, 1.0),
        array<vec4<f32>, 4>(instance.uv1, instance.uv2, instance.uv3, instance.uv4),
        array<vec2<f32>, 4>(base_uv, base_uv, base_uv, base_uv),
        instance.uvd,
        instance.axis_x.w,
        instance.distortion,
        instance.color,
    );
    output.uv1 = disc_uv(output.uv1);
    output.uv2 = disc_uv(output.uv2);
    output.uv3 = disc_uv(output.uv3);
    output.uv4 = disc_uv(output.uv4);
    output.uvd = disc_uv(output.uvd);
    output.soft_particle_params = vec4<f32>(instance.disc_color_outer.xy, 0.0, 0.0);
    return output;
}

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32,
    instance: VfxInstanceInput,
) -> VfxVertexOutput {
    if instance.pivot_offset.w == 8.0 {
        return polygon_vertex(vertex_index, instance);
    }
    if instance.pivot_offset.w == 7.0 {
        return line_vertex(vertex_index, instance);
    }
    if instance.pivot_offset.w == 5.0 {
        return disc_vertex(vertex_index, instance);
    }
    if instance.pivot_offset.w == 6.0 {
        return laser_vertex(vertex_index, instance);
    }
    let powder = instance.pivot_offset.w != 0.0;
    let powder_single = instance.pivot_offset.w == 1.0;
    let windmill = instance.pivot_offset.w >= 3.0;
    let fixed_uv = powder_single || windmill;
    let corner_index = vertex_index % 6u;
    var corner = CORNERS[corner_index];
    let position = instance.position_facing.xyz;
    var basis = facing_basis(
        i32(instance.position_facing.w), position, instance.disc_axis_z.xyz,
        instance.disc_dimensions.xyz, instance.disc_heights.xyz,
    );
    if powder {
        corner = POWDER_CORNERS[corner_index];
    }
    if fixed_uv || (powder && instance.position_facing.w == 5.0) {
        // Powder/Windmill use the inverse view rotation, independently of position.
        let screen = facing_basis(5, position, vec3<f32>(0.0), vec3<f32>(0.0), vec3<f32>(0.0));
        basis = mat3x3<f32>(-screen[0], screen[1], -screen[2]);
    }
    // AVFXTools InitQuad maps local -X to U=1 and local -Y to V=0.
    var base_uv = vec2<f32>(0.5 - corner.x * 0.5, 0.5 + corner.y * 0.5);
    if powder {
        base_uv = vec2<f32>(0.5 + corner.x * 0.5, 0.5 - corner.y * 0.5);
    }
    if windmill {
        // Client 0x1403ff320: four coplanar quadrants, each sampling all of TC1.
        let quadrant = vertex_index / 6u;
        let mirror = instance.pivot_offset.w == 4.0;
        let right = (quadrant & 1u) != 0u;
        let bottom = (quadrant & 2u) != 0u;
        corner = corner * 0.5 + vec2<f32>(select(-0.5, 0.5, right), select(0.5, -0.5, bottom));
        if quadrant == 1u {
            base_uv = select(vec2<f32>(base_uv.y, 1.0 - base_uv.x), vec2<f32>(1.0 - base_uv.x, base_uv.y), mirror);
        } else if quadrant == 2u {
            base_uv = select(vec2<f32>(1.0 - base_uv.y, base_uv.x), vec2<f32>(base_uv.x, 1.0 - base_uv.y), mirror);
        } else if quadrant == 3u {
            base_uv = vec2<f32>(1.0) - base_uv;
        }
    }
    let local = instance.axis_x.xyz * corner.x + instance.axis_y.xyz * corner.y
        + instance.pivot_offset.xyz;
    var world = position + basis * local;
    // Single Powder and Windmill use the fixed screen basis even when
    // the authored ordinary-particle RBDT happens to be AxisY (9).
    if !fixed_uv && i32(instance.position_facing.w) == 9 {
        let old = mat3x3<f32>(instance.axis_x.xyz, instance.axis_y.xyz, vec3<f32>(0.0));
        let tree = tree_billboard_linear(old, position);
        world = position + tree[0] * corner.x + tree[1] * corner.y + instance.pivot_offset.xyz;
    }
    var output = fill_output(
        vec4<f32>(world, 1.0),
        array<vec4<f32>, 4>(instance.uv1, instance.uv2, instance.uv3, instance.uv4),
        // 四边形基底 UV：每层同为角点（UvSet 序号不参与）。
        array<vec2<f32>, 4>(base_uv, base_uv, base_uv, base_uv),
        instance.uvd,
        instance.axis_x.w,
        instance.distortion,
        instance.color,
    );
    output.soft_particle_params = vec4<f32>(instance.disc_color_outer.xy, 0.0, 0.0);
    output.clip_position = apply_depth_offset(output.clip_position, instance.disc_color_outer.z, instance.disc_color_outer.w);
    if fixed_uv {
        // Fixed packed SNORM UVs; this path never evaluates UvSt.
        output.uv1 = base_uv;
    }
    return output;
}

// ===== 网格粒子路径（DrawModel）=====

struct VfxMeshVertexInput {
    @location(0) position: vec4<f32>,
    // Pair the four base UV sets to stay within WebGPU's vertex attribute limit.
    @location(1) uv12: vec4<f32>,
    @location(2) uv34: vec4<f32>,
    @location(3) color: vec4<f32>,
    @location(4) normal: vec4<f32>,
    @location(5) tangent: vec4<f32>,
};

struct VfxMeshInstance {
    // xyz: 位置；w: 原始 RBDT。
    position_facing: vec4<f32>,
    // xyz: 线性变换的三列；axis_x.w: TD 的 UV 旋转。
    axis_x: vec4<f32>,
    // w: FrsT.
    axis_y: vec4<f32>,
    axis_z: vec4<f32>,
    color: vec4<f32>,
    uv1: vec4<f32>,
    uv2: vec4<f32>,
    uv3: vec4<f32>,
    uv4: vec4<f32>,
    uvd: vec4<f32>,
    distortion: vec4<u32>,
    fresnel_direction_power: vec4<f32>,
    fresnel_color_begin: vec4<f32>,
    fresnel_color_end: vec4<f32>,
    movement_direction: vec4<f32>,
    facing_parent_y: vec4<f32>,
    facing_parent_z: vec4<f32>,
    soft_particle_params: vec4<f32>,
    normal_uv: vec4<f32>,
    // x TN rotation, y NPow, z TR TxNo, w raw floating-point TP offset.
    normal_params: vec4<f32>,
    reflection_params: vec4<f32>,
};

@group(1) @binding(11) var<storage, read> mesh_instances: array<VfxMeshInstance>;

@vertex
fn vs_mesh(
    vertex: VfxMeshVertexInput,
    @builtin(instance_index) instance_index: u32,
) -> VfxVertexOutput {
    let instance = mesh_instances[instance_index];
    let linear = mat3x3<f32>(instance.axis_x.xyz, instance.axis_y.xyz, instance.axis_z.xyz);
    let position = instance.position_facing.xyz;
    let basis = facing_basis(
        i32(instance.position_facing.w), position, instance.movement_direction.xyz,
        instance.facing_parent_y.xyz, instance.facing_parent_z.xyz,
    );
    var transformed = basis * linear;
    if i32(instance.position_facing.w) == 9 {
        transformed = tree_billboard_linear(linear, position);
    }
    let world = position * vertex.position.w + transformed * vertex.position.xyz;
    // 顶点色全通道参与：rgb 染色，alpha 为羽化遮罩（火舌翼缘 alpha=0
    // 消隐；实心壳体模型 alpha 恒 255 不受影响）。
    let base_uvs = array<vec2<f32>, 4>(
        vertex.uv12.xy, vertex.uv12.zw, vertex.uv34.xy, vertex.uv34.zw,
    );
    var output = fill_output(
        vec4<f32>(world, vertex.position.w),
        array<vec4<f32>, 4>(instance.uv1, instance.uv2, instance.uv3, instance.uv4),
        base_uvs,
        instance.uvd,
        instance.axis_x.w,
        instance.distortion,
        instance.color * vertex.color,
    );
    output.soft_particle_params = vec4<f32>(instance.soft_particle_params.xy, 0.0, 0.0);
    output.clip_position = apply_depth_offset(output.clip_position, instance.soft_particle_params.z, instance.soft_particle_params.w);
    // Quad/Polyline/Decal leave the upper bits clear. Model uses bit31 as a
    // marker and bit16..30 for the storage index needed by pixel-position UVs.
    output.distortion_targets |= 0x80000000u | ((instance_index & 0x7FFFu) << 16u);
    let kind = i32(instance.axis_y.w);
    if (kind >= 1 && kind <= 3) || (vfx_params.flags & 2048u) != 0u {
        // Apricot uses the model linear matrix, not its inverse transpose.
        output.normal = vec3<f32>(transformed * vertex.normal.xyz);
    }
    if (vfx_params.flags & 2048u) != 0u {
        let normal_uv_set = (vfx_params.uv_set_sel >> 10u) & 3u;
        let normal_uv = move_uv(
            instance.normal_uv,
            base_uvs[normal_uv_set],
            instance.normal_params.x,
        );
        output.fresnel_direction_power = vec4<f32>(
            transformed * vertex.tangent.xyz,
            instance.normal_params.y,
        );
        output.fresnel_color_begin = vec4<f32>(1.0);
        output.fresnel_color_end = vec4<f32>(1.0);
        output.fresnel_enabled = 1u;
    }
    if kind >= 1 && kind <= 3 {
        var direction = instance.fresnel_direction_power.xyz;
        if kind == 1 {
            direction = camera.camera_position.xyz - world;
        } else if kind == 3 {
            direction = transformed * direction;
        }
        if (vfx_params.flags & 2048u) == 0u {
            output.fresnel_direction_power = vec4<f32>(direction, instance.fresnel_direction_power.w);
        }
        output.fresnel_color_begin = instance.fresnel_color_begin;
        output.fresnel_color_end = instance.fresnel_color_end;
        output.fresnel_enabled = 1u;
    }
    return output;
}

// ===== 片元：TD 扭曲 + 四层贴图合成 =====

fn convert_color_to_alpha(tex: vec4<f32>, color_to_alpha: bool) -> vec4<f32> {
    if color_to_alpha {
        // Client Apricot ColorToAlpha_On: replace alpha with luminance and RGB
        // with white. The sampled alpha and TLst/TxNo source do not participate.
        let luminance = dot(tex.rgb, vec3<f32>(0.298912, 0.586611, 0.114478));
        return vec4<f32>(1.0, 1.0, 1.0, luminance);
    }
    return tex;
}

fn sample_layer(
    texture: texture_2d<f32>,
    tex_sampler: sampler,
    uv: vec2<f32>,
    lod_bias: f32,
    color_to_alpha: bool,
) -> vec4<f32> {
    return convert_color_to_alpha(
        textureSampleBias(texture, tex_sampler, uv, lod_bias),
        color_to_alpha,
    );
}

fn combine_color(rgb: vec3<f32>, layer: vec3<f32>, mode: u32) -> vec3<f32> {
    switch mode {
        case 0u: { return rgb * layer; }
        case 1u: { return rgb + layer; }
        case 2u: { return rgb - layer; }
        case 3u: { return max(rgb, layer); }
        case 4u: { return min(rgb, layer); }
        // 5 = None in the client's key table; unknown values also skip.
        default: { return rgb; }
    }
}

fn combine_alpha(a: f32, layer: f32, mode: u32) -> f32 {
    switch mode {
        case 0u: { return a * layer; }
        case 1u: { return max(a, layer); }
        case 2u: { return min(a, layer); }
        default: { return a; }
    }
}

fn model_pixel_uv(
    instance: VfxMeshInstance,
    transform: vec4<f32>,
    rotation: f32,
    pixel_uv: vec2<f32>,
) -> vec2<f32> {
    let clip = camera.view_proj * vec4<f32>(instance.position_facing.xyz, 1.0);
    let center = clip.xy / clip.w * vec2<f32>(0.5, -0.5) + vec2<f32>(0.5);
    // Client Model ByPixelPosition applies the UvSet affine transform around
    // the projected particle origin, then returns to VDrw's centered UV space.
    return move_uv(transform, pixel_uv - center + vec2<f32>(0.5), rotation)
        + center - vec2<f32>(0.5);
}

fn compose_color(input: VfxVertexOutput) -> vec4<f32> {
    let powder = (vfx_params.flags & 128u) != 0u;
    var uvs = array<vec2<f32>, 4>(input.uv1, input.uv2, input.uv3, input.uv4);
    let pixel_uv = input.clip_position.xy / camera.viewport.xy;
    let mesh = (input.distortion_targets & 0x80000000u) != 0u;
    var distortion_uv = input.uvd;
    var normal_uv = vec2<f32>(0.0);
    if mesh {
        let instance_index = (input.distortion_targets >> 16u) & 0x7FFFu;
        let instance = mesh_instances[instance_index];
        let transforms = array<vec4<f32>, 4>(
            instance.uv1, instance.uv2, instance.uv3, instance.uv4,
        );
        let rotations = array<f32, 4>(
            f32(instance.distortion.z & 0xFFFFu) / 65535.0 * (2.0 * 3.14159265),
            f32(instance.distortion.z >> 16u) / 65535.0 * (2.0 * 3.14159265),
            f32(instance.distortion.w & 0xFFFFu) / 65535.0 * (2.0 * 3.14159265),
            f32(instance.distortion.w >> 16u) / 65535.0 * (2.0 * 3.14159265),
        );
        let normal_set = (vfx_params.uv_set_sel >> 10u) & 3u;
        normal_uv = array<vec2<f32>, 4>(input.uv1, input.uv2, input.uv3, input.uv4)[normal_set];
        for (var i = 0u; i < 4u; i++) {
            if (vfx_params.pixel_uv_mask & (1u << i)) != 0u {
                uvs[i] = model_pixel_uv(instance, transforms[i], rotations[i], pixel_uv);
            }
        }
        if (vfx_params.pixel_uv_mask & 16u) != 0u {
            distortion_uv = model_pixel_uv(instance, instance.uvd, instance.axis_x.w, pixel_uv);
        }
        if (vfx_params.pixel_uv_mask & 32u) != 0u {
            normal_uv = model_pixel_uv(instance, instance.normal_uv, instance.normal_params.x, pixel_uv);
        }
    } else {
        for (var i = 0u; i < 4u; i++) {
            if (vfx_params.pixel_uv_mask & (1u << i)) != 0u {
                uvs[i] = pixel_uv;
            }
        }
        if (vfx_params.pixel_uv_mask & 16u) != 0u {
            distortion_uv = pixel_uv;
        }
    }
    // TD：扭曲贴图 rg − 0.5 乘强度偏移目标 UV（光罩/流光的有机抖动）。
    if (vfx_params.flags & 32u) != 0u {
        let td = textureSampleBias(
            vfx_texture_d,
            vfx_sampler_d,
            distortion_uv,
            vfx_params.auxiliary_lod_bias.x,
        );
        let offset = input.distortion_power * (td.rg - vec2<f32>(0.5, 0.5));
        let targets = input.distortion_targets & 15u;
        for (var i = 0; i < 4; i++) {
            if (targets & (1u << u32(i))) != 0u {
                uvs[i] += offset;
            }
        }
    }
    // TC1 modes gate RGB / alpha; TC2..TC4 select combine operations.
    var tex_color = vec4<f32>(1.0, 1.0, 1.0, 1.0);
    if (vfx_params.flags & 64u) != 0u {
        if (vfx_params.flags & 512u) != 0u {
            tex_color = textureSample(vfx_screen_copy, vfx_sampler1, pixel_uv);
        } else if (vfx_params.flags & 1024u) != 0u {
            tex_color = textureSample(vfx_previous_scene, vfx_sampler1, pixel_uv);
        } else if (vfx_params.flags & 65536u) != 0u {
            tex_color = textureSample(vfx_portrait, vfx_portrait_sampler, pixel_uv);
        } else {
            tex_color = sample_layer(
                vfx_texture1,
                vfx_sampler1,
                uvs[0],
                vfx_params.texture_lod_bias.x,
                false,
            );
        }
        tex_color = convert_color_to_alpha(tex_color, (vfx_params.color_to_alpha & 1u) != 0u);
        tex_color = vec4<f32>(
            select(vec3<f32>(1.0), tex_color.rgb, vfx_params.texture1_channels.x != 0u),
            select(1.0, tex_color.a, vfx_params.texture1_channels.y != 0u),
        );
    }
    // TP：TC1 各通道分别作为一维查表坐标，只取调色板红通道。
    if (vfx_params.flags & 256u) != 0u {
        var palette_offset = f32((input.distortion_targets >> 8u) & 255u) / 255.0;
        if mesh {
            let instance_index = (input.distortion_targets >> 16u) & 0x7FFFu;
            palette_offset = mesh_instances[instance_index].normal_params.w;
        }
        let coords = tex_color + vec4<f32>(palette_offset);
        tex_color = vec4<f32>(
            textureSampleBias(vfx_texture_p, vfx_sampler_p, vec2<f32>(coords.r), vfx_params.auxiliary_lod_bias.y).r,
            textureSampleBias(vfx_texture_p, vfx_sampler_p, vec2<f32>(coords.g), vfx_params.auxiliary_lod_bias.y).r,
            textureSampleBias(vfx_texture_p, vfx_sampler_p, vec2<f32>(coords.b), vfx_params.auxiliary_lod_bias.y).r,
            textureSampleBias(vfx_texture_p, vfx_sampler_p, vec2<f32>(coords.a), vfx_params.auxiliary_lod_bias.y).r,
        );
    }
    // TC2..TC4 逐层合成（WGSL 不允许纹理数组，显式展开）。
    if (vfx_params.flags & 4u) != 0u {
        let layer = sample_layer(
            vfx_texture2,
            vfx_sampler2,
            uvs[1],
            vfx_params.texture_lod_bias.y,
            (vfx_params.color_to_alpha & 2u) != 0u,
        );
        tex_color = vec4<f32>(
            combine_color(tex_color.rgb, layer.rgb, vfx_params.combine_color[0]),
            combine_alpha(tex_color.a, layer.a, vfx_params.combine_alpha[0]),
        );
    }
    if (vfx_params.flags & 8u) != 0u {
        let layer = sample_layer(
            vfx_texture3,
            vfx_sampler3,
            uvs[2],
            vfx_params.texture_lod_bias.z,
            (vfx_params.color_to_alpha & 4u) != 0u,
        );
        tex_color = vec4<f32>(
            combine_color(tex_color.rgb, layer.rgb, vfx_params.combine_color[1]),
            combine_alpha(tex_color.a, layer.a, vfx_params.combine_alpha[1]),
        );
    }
    if (vfx_params.flags & 16u) != 0u {
        let layer = sample_layer(
            vfx_texture4,
            vfx_sampler4,
            uvs[3],
            vfx_params.texture_lod_bias.w,
            (vfx_params.color_to_alpha & 8u) != 0u,
        );
        tex_color = vec4<f32>(
            combine_color(tex_color.rgb, layer.rgb, vfx_params.combine_color[2]),
            combine_alpha(tex_color.a, layer.a, vfx_params.combine_alpha[2]),
        );
    }
    // Powder clamps the TC1 result before vertex/document color. Its
    // NoneControl output keeps signed RGB; other types clamp after composition.
    if powder {
        tex_color = max(tex_color, vec4<f32>(0.0));
    }
    var color = tex_color * input.color;
    if (vfx_params.flags & 4096u) != 0u {
        let mesh_index = (input.distortion_targets >> 16u) & 0x7FFFu;
        let instance = mesh_instances[mesh_index];
        let ndc = vec3<f32>(
            input.clip_position.x / camera.viewport.x * 2.0 - 1.0,
            input.clip_position.y / camera.viewport.y * -2.0 + 1.0,
            input.clip_position.z,
        );
        let world_h = camera.inverse_view_proj * vec4<f32>(ndc, 1.0);
        let world = world_h.xyz / world_h.w;
        let view_ray = normalize(world - camera.camera_position.xyz);
        let normal = input.normal * inverseSqrt(max(dot(input.normal, input.normal), 1e-20));
        let reflection_dir = reflect(view_ray, normal);
        var reflection = textureSampleBias(
            vfx_reflection_cube,
            vfx_reflection_sampler,
            reflection_dir,
            vfx_params.auxiliary_lod_bias.w,
        ).rgb;
        if (vfx_params.flags & 131072u) != 0u {
            reflection = textureSampleBias(
                vfx_file_reflection_cube,
                vfx_file_reflection_sampler,
                reflection_dir,
                vfx_params.auxiliary_lod_bias.w,
            ).rgb;
        }
        // Storage-backed instance parameters are not uniform to WebGPU's
        // validator. Sample before selecting the per-instance source so the
        // implicit derivatives still execute across the fragment quad.
        let screen_uv = vec2<f32>(
            0.5 + 0.5 * reflection_dir.x,
            0.5 - 0.5 * reflection_dir.y,
        );
        let screen_reflection = textureSample(vfx_screen_copy, vfx_sampler1, screen_uv).rgb;
        reflection = select(reflection, screen_reflection, instance.reflection_params.z > 0.5);
        let power = max(instance.reflection_params.y, 1.0e-4);
        reflection = pow(max(reflection, vec3<f32>(0.0)), vec3<f32>(power));
        let rate = clamp(instance.reflection_params.x, 0.0, 1.0);
        let mode = (vfx_params.flags >> 13u) & 7u;
        switch mode {
            case 0u: { color = vec4<f32>(color.rgb * mix(vec3<f32>(1.0), reflection, rate), color.a); }
            case 1u: { color = vec4<f32>(color.rgb + reflection * rate, color.a); }
            case 2u: { color = vec4<f32>(color.rgb - reflection * rate, color.a); }
            case 3u: { color = vec4<f32>(max(color.rgb, reflection * rate), color.a); }
            case 4u: { color = vec4<f32>(min(color.rgb, reflection * rate), color.a); }
            default: { /* Unknown TCCT is a no-op in the client fallback path. */ }
        }
    }
    // A flat interpolant can vary between primitives; it is not a uniform
    // condition for implicit-LOD sampling. Keep the normal-map lookup under
    // the uniform material flag, outside the per-fragment Fresnel branch.
    var mapped_normal = vec3<f32>(0.0);
    if (vfx_params.flags & 2048u) != 0u {
        mapped_normal = textureSampleBias(
            vfx_texture_n,
            vfx_sampler_n,
            normal_uv,
            vfx_params.auxiliary_lod_bias.z,
        ).rgb - vec3<f32>(0.5);
    }
    if input.fresnel_enabled != 0u {
        var direction = input.fresnel_direction_power.xyz;
        var exponent = input.fresnel_direction_power.w;
        var fresnel_begin = input.fresnel_color_begin;
        var fresnel_end = input.fresnel_color_end;
        let mesh = (input.distortion_targets & 0x80000000u) != 0u;
        let mesh_index = (input.distortion_targets >> 16u) & 0x7FFFu;
        if mesh && (vfx_params.flags & 2048u) != 0u {
            let instance = mesh_instances[mesh_index];
            let kind = i32(instance.axis_y.w);
            exponent = instance.fresnel_direction_power.w;
            fresnel_begin = instance.fresnel_color_begin;
            fresnel_end = instance.fresnel_color_end;
            if kind == 1 {
                let ndc = vec3<f32>(
                    input.clip_position.x / camera.viewport.x * 2.0 - 1.0,
                    input.clip_position.y / camera.viewport.y * -2.0 + 1.0,
                    input.clip_position.z,
                );
                let world_h = camera.inverse_view_proj * vec4<f32>(ndc, 1.0);
                let world = world_h.xyz / world_h.w;
                direction = camera.camera_position.xyz - world;
            } else if kind == 2 {
                direction = instance.fresnel_direction_power.xyz;
            } else if kind == 3 {
                let linear = mat3x3<f32>(instance.axis_x.xyz, instance.axis_y.xyz, instance.axis_z.xyz);
                let basis = facing_basis(
                    i32(instance.position_facing.w),
                    instance.position_facing.xyz,
                    instance.movement_direction.xyz,
                    instance.facing_parent_y.xyz,
                    instance.facing_parent_z.xyz,
                );
                var transformed = basis * linear;
                if i32(instance.position_facing.w) == 9 {
                    transformed = tree_billboard_linear(linear, instance.position_facing.xyz);
                }
                direction = transformed * instance.fresnel_direction_power.xyz;
            } else {
                direction = vec3<f32>(0.0, 0.0, 1.0);
                exponent = 0.0;
            }
        }
        // Keep zero-scale meshes and coincident camera positions finite.
        var normal = input.normal * inverseSqrt(max(dot(input.normal, input.normal), 1e-20));
        if (vfx_params.flags & 2048u) != 0u {
            let tangent = input.fresnel_direction_power.xyz
                * inverseSqrt(max(dot(input.fresnel_direction_power.xyz, input.fresnel_direction_power.xyz), 1e-20));
            let bitangent = cross(normal, tangent);
            let mapped = vec3<f32>(mapped_normal.xy * input.fresnel_direction_power.w, mapped_normal.z);
            normal = tangent * mapped.x + bitangent * mapped.y + normal * mapped.z;
            normal *= inverseSqrt(max(dot(normal, normal), 1e-20));
        }
        let view = direction * inverseSqrt(max(dot(direction, direction), 1e-20));
        let factor = pow(max(abs(dot(view, normal)), 1e-5), exponent);
        color *= mix(fresnel_begin, fresnel_end, factor);
    }
    let rgb = select(max(color.rgb, vec3<f32>(0.0)), color.rgb, powder);
    let alpha = clamp(color.a, 0.0, 1.0);
    // Apricot retains HDR RGB and rejects alpha < 1/255.
    // Discard also prevents invisible fragments from writing particle depth.
    if alpha < (1.0 / 255.0) {
        discard;
    }
    // ModulateAlpha uses the nonnegative alpha before its upper clamp.
    return vec4<f32>(rgb, max(color.a, 0.0));
}

// Client `apricot_powder` ps24: reconstruct scene position from depth, then
// multiply alpha by min(distance / SPFR, 1)^2. The two fragment entry families
// below select the 1x or multisampled depth binding in the later soft pass.
fn soft_particle_fade(input: VfxVertexOutput, scene_depth: f32) -> f32 {
    let soft_screen_uv = input.clip_position.xy / camera.viewport.xy;
    // CameraUniform uses perspective_rh: both sampled depth and fragment z
    // already occupy the inverse matrix's 0..1 NDC range.
    let ndc = vec3<f32>(
        soft_screen_uv.x * 2.0 - 1.0,
        soft_screen_uv.y * -2.0 + 1.0,
        scene_depth,
    );
    let scene_h = camera.inverse_view_proj * vec4<f32>(ndc, 1.0);
    let scene_world = scene_h.xyz / scene_h.w;
    let particle_h = camera.inverse_view_proj * vec4<f32>(
        ndc.x,
        ndc.y,
        input.clip_position.z,
        1.0,
    );
    let particle_world = particle_h.xyz / particle_h.w;
    let distance = length(scene_world - particle_world);
    let range = input.soft_particle_params.x;
    let denominator = select(range, 1.0e-6, range == 0.0);
    let normalized = min(distance / denominator, 1.0);
    return normalized * normalized;
}

fn soft_particle_color(input: VfxVertexOutput, scene_depth: f32) -> vec4<f32> {
    let color = compose_color(input);
    if input.soft_particle_params.y < 0.5 {
        return color;
    }
    return vec4<f32>(color.rgb, color.a * soft_particle_fade(input, scene_depth));
}

// ComputeFinalColorType_LerpWhite: clamp RGB before interpolating toward white.
@fragment
fn fs_multiply(input: VfxVertexOutput) -> @location(0) vec4<f32> {
    let color = compose_color(input);
    let alpha = min(color.a, 1.0);
    return vec4<f32>(mix(vec3<f32>(1.0), clamp(color.rgb, vec3<f32>(0.0), vec3<f32>(1.0)), alpha), alpha);
}

// ComputeFinalColorType_ModulateAlpha: clamp the RGB * raw alpha product.
@fragment
fn fs_screen(input: VfxVertexOutput) -> @location(0) vec4<f32> {
    let color = compose_color(input);
    return vec4<f32>(clamp(color.rgb * color.a, vec3<f32>(0.0), vec3<f32>(1.0)), min(color.a, 1.0));
}

// ComputeFinalColorType_NoneControl: preserve HDR RGB for the hardware blend.
@fragment
fn fs_blend(input: VfxVertexOutput) -> @location(0) vec4<f32> {
    let color = compose_color(input);
    return vec4<f32>(color.rgb, min(color.a, 1.0));
}

fn soft_alpha(input: VfxVertexOutput, scene_depth: f32) -> vec4<f32> {
    let color = soft_particle_color(input, scene_depth);
    if color.a < (1.0 / 255.0) {
        discard;
    }
    return color;
}

fn soft_multiply_color(input: VfxVertexOutput, depth: f32) -> vec4<f32> {
    let color = soft_alpha(input, depth);
    let alpha = min(color.a, 1.0);
    return vec4<f32>(mix(vec3<f32>(1.0), clamp(color.rgb, vec3<f32>(0.0), vec3<f32>(1.0)), alpha), alpha);
}

fn soft_screen_color(input: VfxVertexOutput, depth: f32) -> vec4<f32> {
    let color = soft_alpha(input, depth);
    return vec4<f32>(clamp(color.rgb * color.a, vec3<f32>(0.0), vec3<f32>(1.0)), min(color.a, 1.0));
}

fn soft_blend_color(input: VfxVertexOutput, depth: f32) -> vec4<f32> {
    let color = soft_alpha(input, depth);
    return vec4<f32>(color.rgb, min(color.a, 1.0));
}

@fragment
fn soft_multiply_1x(input: VfxVertexOutput) -> @location(0) vec4<f32> { return soft_multiply_color(input, textureLoad(soft_scene_depth_1x, vec2<i32>(input.clip_position.xy), 0)); }
@fragment
fn soft_screen_1x(input: VfxVertexOutput) -> @location(0) vec4<f32> { return soft_screen_color(input, textureLoad(soft_scene_depth_1x, vec2<i32>(input.clip_position.xy), 0)); }
@fragment
fn soft_blend_1x(input: VfxVertexOutput) -> @location(0) vec4<f32> { return soft_blend_color(input, textureLoad(soft_scene_depth_1x, vec2<i32>(input.clip_position.xy), 0)); }
@fragment
fn soft_multiply_4x(input: VfxVertexOutput, @builtin(sample_index) sample_index: u32) -> @location(0) vec4<f32> { return soft_multiply_color(input, textureLoad(soft_scene_depth_4x, vec2<i32>(input.clip_position.xy), sample_index)); }
@fragment
fn soft_screen_4x(input: VfxVertexOutput, @builtin(sample_index) sample_index: u32) -> @location(0) vec4<f32> { return soft_screen_color(input, textureLoad(soft_scene_depth_4x, vec2<i32>(input.clip_position.xy), sample_index)); }
@fragment
fn soft_blend_4x(input: VfxVertexOutput, @builtin(sample_index) sample_index: u32) -> @location(0) vec4<f32> { return soft_blend_color(input, textureLoad(soft_scene_depth_4x, vec2<i32>(input.clip_position.xy), sample_index)); }
