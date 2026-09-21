// VFX particle shader, standalone package module compiled to OUT_DIR/vfx.wgsl
// by the build script. Draws avfx particle quads (billboard or oriented) and
// DrawModel mesh particles with four-layer texture composition (TC1 base —
// optionally a TLst shape mask driving alpha — × TC2..TC4 per-layer combine
// modes) and per-group additive/alpha blend, fed by the CPU avfx sampler
// (`xiv_companion_data::avfx_sim`).

struct Camera {
    view_proj: mat4x4<f32>,
    light_dir: vec4<f32>,
    view_dir: vec4<f32>,
    camera_position: vec4<f32>,
    options: vec4<f32>,
    dynamic_emissive_color: vec4<f32>,
};

struct VfxGroupParams {
    // TC2/TC3/TC4 的颜色合成（TCCT：0 乘、1 加、2 减、3 max、4 min）。
    combine_color: vec3<u32>,
    // 各层 bC2A 位掩码 + TC2..TC4 的 alpha 合成（TCAT：0 乘、1 max、2 min、
    // 3 不合成）。
    combine_alpha: vec3<u32>,
    color_to_alpha: u32,
    // bit0: TC1 为形状遮罩（亮度→alpha）；bit1..4: 各层有贴图；bit5: 有 TD。
    flags: u32,
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

struct VfxInstanceInput {
    // xyz: 位置；w: billboard 面内旋转（弧度）。
    @location(0) position_rotation: vec4<f32>,
    // xy: 半宽/半高；z: flags（bit0 = billboard）；w: 预留。
    @location(1) size_flags: vec4<f32>,
    // HDR 颜色（rgb = Col.RGB × Bri；a = Col.A 包络 × SclA）。
    @location(2) color: vec4<f32>,
    // 四层贴图的 UV（原点 + 缩放）。
    @location(3) uv1: vec4<f32>,
    @location(4) uv2: vec4<f32>,
    @location(5) uv3: vec4<f32>,
    @location(6) uv4: vec4<f32>,
    // TD 扭曲贴图采样 UV。
    @location(7) uvd: vec4<f32>,
    // x：TD 强度；y：扭曲目标位（bit0..3 = uv1..4）。
    @location(8) distortion: vec4<f32>,
    // 非 billboard 朝向（四元数 xyzw）。
    @location(9) orientation: vec4<f32>,
    // xy: 面内旋转枢轴（半尺寸单位，Smpl PvtX/PvtY）；zw 预留。
    @location(10) pivot: vec4<f32>,
};

struct VfxVertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv1: vec2<f32>,
    @location(1) uv2: vec2<f32>,
    @location(2) uv3: vec2<f32>,
    @location(3) uv4: vec2<f32>,
    @location(4) uvd: vec2<f32>,
    // xy：TD 强度与扭曲目标位。
    @location(5) distortion: vec2<f32>,
    @location(6) color: vec4<f32>,
};

const CORNERS: array<vec2<f32>, 6> = array<vec2<f32>, 6>(
    vec2<f32>(-1.0, -1.0),
    vec2<f32>(1.0, -1.0),
    vec2<f32>(1.0, 1.0),
    vec2<f32>(-1.0, -1.0),
    vec2<f32>(1.0, 1.0),
    vec2<f32>(-1.0, 1.0),
);

fn quat_rotate(q: vec4<f32>, v: vec3<f32>) -> vec3<f32> {
    let t = 2.0 * cross(q.xyz, v);
    return v + q.w * t + cross(q.xyz, t);
}

fn fill_output(
    world: vec3<f32>,
    uv_transforms: array<vec4<f32>, 4>,
    base_uv: vec2<f32>,
    uvd: vec4<f32>,
    distortion: vec4<f32>,
    color: vec4<f32>,
) -> VfxVertexOutput {
    var output: VfxVertexOutput;
    output.clip_position = camera.view_proj * vec4<f32>(world, 1.0);
    output.uv1 = uv_transforms[0].xy + base_uv * uv_transforms[0].zw;
    output.uv2 = uv_transforms[1].xy + base_uv * uv_transforms[1].zw;
    output.uv3 = uv_transforms[2].xy + base_uv * uv_transforms[2].zw;
    output.uv4 = uv_transforms[3].xy + base_uv * uv_transforms[3].zw;
    output.uvd = uvd.xy + base_uv * uvd.zw;
    output.distortion = distortion.xy;
    output.color = color;
    return output;
}

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32,
    instance: VfxInstanceInput,
) -> VfxVertexOutput {
    let corner = CORNERS[vertex_index];
    let position = instance.position_rotation.xyz;
    let rotation = instance.position_rotation.w;
    let size = instance.size_flags.xy;
    let is_billboard = (bitcast<u32>(instance.size_flags.z) & 1u) != 0u;

    var right: vec3<f32>;
    var up: vec3<f32>;
    var in_plane = corner;
    if is_billboard {
        // 面向相机的基向量 + 面内旋转（绕枢轴：corner′ = pivot + R·(corner−pivot)）。
        let cos_rotation = cos(rotation);
        let sin_rotation = sin(rotation);
        let rel = corner - instance.pivot.xy;
        in_plane = vec2<f32>(
            rel.x * cos_rotation - rel.y * sin_rotation,
            rel.x * sin_rotation + rel.y * cos_rotation,
        ) + instance.pivot.xy;
        let to_particle = position - camera.camera_position.xyz;
        var forward = vec3<f32>(0.0, 0.0, -1.0);
        if dot(to_particle, to_particle) > 1.0e-8 {
            forward = normalize(to_particle);
        }
        right = cross(vec3<f32>(0.0, 1.0, 0.0), forward);
        if dot(right, right) < 1.0e-8 {
            right = vec3<f32>(1.0, 0.0, 0.0);
        }
        right = normalize(right);
        up = cross(forward, right);
    } else {
        right = quat_rotate(instance.orientation, vec3<f32>(1.0, 0.0, 0.0));
        up = quat_rotate(instance.orientation, vec3<f32>(0.0, 1.0, 0.0));
    }

    let world = position + right * (in_plane.x * size.x) + up * (in_plane.y * size.y);
    return fill_output(
        world,
        array<vec4<f32>, 4>(instance.uv1, instance.uv2, instance.uv3, instance.uv4),
        corner * 0.5 + vec2<f32>(0.5, 0.5),
        instance.uvd,
        instance.distortion,
        instance.color,
    );
}

// ===== 网格粒子路径（DrawModel）=====

struct VfxMeshVertexInput {
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
};

struct VfxMeshInstanceInput {
    // xyz: 位置；w: flags（预留）。
    @location(3) position_flags: vec4<f32>,
    @location(4) orientation: vec4<f32>,
    // xyz: 三轴缩放。
    @location(5) scale: vec4<f32>,
    @location(6) color: vec4<f32>,
    @location(7) uv1: vec4<f32>,
    @location(8) uv2: vec4<f32>,
    @location(9) uv3: vec4<f32>,
    @location(10) uv4: vec4<f32>,
    @location(11) uvd: vec4<f32>,
    @location(12) distortion: vec4<f32>,
};

@vertex
fn vs_mesh(
    vertex: VfxMeshVertexInput,
    instance: VfxMeshInstanceInput,
) -> VfxVertexOutput {
    let scaled = vertex.position * instance.scale.xyz;
    let world = instance.position_flags.xyz + quat_rotate(instance.orientation, scaled);
    // 顶点色全通道参与：rgb 染色，alpha 为羽化遮罩（火舌翼缘 alpha=0
    // 消隐；实心壳体模型 alpha 恒 255 不受影响）。
    return fill_output(
        world,
        array<vec4<f32>, 4>(instance.uv1, instance.uv2, instance.uv3, instance.uv4),
        vertex.uv,
        instance.uvd,
        instance.distortion,
        instance.color * vertex.color,
    );
}

// ===== 片元：TD 扭曲 + 四层贴图合成 =====

fn sample_layer(
    texture: texture_2d<f32>,
    tex_sampler: sampler,
    uv: vec2<f32>,
    is_shape_mask: bool,
    color_to_alpha: bool,
) -> vec4<f32> {
    let tex = textureSample(texture, tex_sampler, uv);
    if is_shape_mask {
        // 形状遮罩（TLst）：亮度→alpha，rgb 不改（颜色交给 Col/TC2+）。
        let lum = 0.3 * tex.r + 0.59 * tex.g + 0.11 * tex.b;
        return vec4<f32>(1.0, 1.0, 1.0, lum);
    }
    if color_to_alpha {
        return vec4<f32>(tex.rgb, tex.r);
    }
    return tex;
}

fn combine_color(rgb: vec3<f32>, layer: vec3<f32>, mode: u32) -> vec3<f32> {
    switch mode {
        case 1u: { return rgb + layer; }
        case 2u: { return rgb - layer; }
        case 3u: { return max(rgb, layer); }
        case 4u: { return min(rgb, layer); }
        // 0 = Multiply；未知值（如 5）按 Multiply 处理。
        default: { return rgb * layer; }
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

fn compose_color(input: VfxVertexOutput) -> vec4<f32> {
    var uvs = array<vec2<f32>, 4>(input.uv1, input.uv2, input.uv3, input.uv4);
    // TD：扭曲贴图 rg − 0.5 乘强度偏移目标 UV（光罩/流光的有机抖动）。
    if (vfx_params.flags & 32u) != 0u {
        let td = textureSample(vfx_texture_d, vfx_sampler_d, input.uvd);
        let offset = input.distortion.x * (td.rg - vec2<f32>(0.5, 0.5));
        let targets = bitcast<u32>(input.distortion.y);
        for (var i = 0; i < 4; i++) {
            if (targets & (1u << u32(i))) != 0u {
                uvs[i] += offset;
            }
        }
    }
    // TC1 基准层。
    // TC1 恒采样（无贴图时绑定内置回退光点——光点是粒子的形状）。
    var tex_color = sample_layer(
        vfx_texture1,
        vfx_sampler1,
        uvs[0],
        (vfx_params.flags & 1u) != 0u,
        (vfx_params.color_to_alpha & 1u) != 0u,
    );
    // TC2..TC4 逐层合成（WGSL 不允许纹理数组，显式展开）。
    if (vfx_params.flags & 4u) != 0u {
        let layer = sample_layer(
            vfx_texture2,
            vfx_sampler2,
            uvs[1],
            false,
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
            false,
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
            false,
            (vfx_params.color_to_alpha & 8u) != 0u,
        );
        tex_color = vec4<f32>(
            combine_color(tex_color.rgb, layer.rgb, vfx_params.combine_color[2]),
            combine_alpha(tex_color.a, layer.a, vfx_params.combine_alpha[2]),
        );
    }
    let rgb = tex_color.rgb * input.color.rgb;
    let alpha = tex_color.a * input.color.a;
    return vec4<f32>(rgb, alpha);
}

// 加色（RMT=Add 系）：alpha 作为强度折进 rgb，One/One 混合由管线给出。
@fragment
fn fs_add(input: VfxVertexOutput) -> @location(0) vec4<f32> {
    let color = compose_color(input);
    return vec4<f32>(color.rgb * color.a, 0.0);
}

// 普通 alpha 混合（RMT=Blend 系）：SrcAlpha/InvSrcAlpha 由管线给出。
@fragment
fn fs_blend(input: VfxVertexOutput) -> @location(0) vec4<f32> {
    return compose_color(input);
}
