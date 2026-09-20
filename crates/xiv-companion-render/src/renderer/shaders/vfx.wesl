// VFX particle shader, standalone package module compiled to OUT_DIR/vfx.wgsl
// by the build script. Draws avfx particle quads (billboard or oriented) and
// DrawModel mesh particles with dual-texture composition (TC1 base × TC2
// combine) and per-group additive/alpha blend, fed by the CPU avfx sampler
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
    // TC2 颜色合成（TCCT：0 乘、1 加、2 减、3 max、4 min）。
    combine_color: u32,
    // TC2 alpha 合成（TCAT：0 乘、1 max、2 min、3 不合成）。
    combine_alpha: u32,
    // bit0: TC1 bC2A；bit1: 有 TC1 贴图；bit2: 有 TC2 贴图；bit3: TC2 bC2A。
    flags: u32,
    _pad: u32,
};

@group(0) @binding(0) var<uniform> camera: Camera;
@group(1) @binding(0) var vfx_texture1: texture_2d<f32>;
@group(1) @binding(1) var vfx_texture2: texture_2d<f32>;
@group(1) @binding(2) var vfx_sampler: sampler;
@group(1) @binding(3) var<uniform> vfx_params: VfxGroupParams;

struct VfxInstanceInput {
    // xyz: 位置；w: billboard 面内旋转（弧度）。
    @location(0) position_rotation: vec4<f32>,
    // xy: 半宽/半高；z: flags（bit0 = billboard）；w: 预留。
    @location(1) size_flags: vec4<f32>,
    // HDR 颜色（rgb = Col.RGB × Bri；a = Col.A 包络）。
    @location(2) color: vec4<f32>,
    // 贴图 1 的 UV 原点 + 缩放。
    @location(3) uv1: vec4<f32>,
    // 贴图 2 的 UV 原点 + 缩放。
    @location(4) uv2: vec4<f32>,
    // 非 billboard 朝向（四元数 xyzw）。
    @location(5) orientation: vec4<f32>,
};

struct VfxVertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv1: vec2<f32>,
    @location(1) uv2: vec2<f32>,
    @location(2) color: vec4<f32>,
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

// 贴附武器表面的粒子深度与表面几乎相等：把 NDC 深度向近处拉一个小量
// （标准贴花手法，远小于场景深度差，不影响遮挡关系）。
fn vfx_depth_bias(clip: vec4<f32>) -> vec4<f32> {
    var out = clip;
    out.z -= 0.002 * out.w;
    return out;
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
        // 面向相机的基向量 + 面内旋转。
        let cos_rotation = cos(rotation);
        let sin_rotation = sin(rotation);
        in_plane = vec2<f32>(
            corner.x * cos_rotation - corner.y * sin_rotation,
            corner.x * sin_rotation + corner.y * cos_rotation,
        );
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
    var output: VfxVertexOutput;
    output.clip_position = vfx_depth_bias(camera.view_proj * vec4<f32>(world, 1.0));
    let uv_corner = corner * 0.5 + vec2<f32>(0.5, 0.5);
    output.uv1 = instance.uv1.xy + uv_corner * instance.uv1.zw;
    output.uv2 = instance.uv2.xy + uv_corner * instance.uv2.zw;
    output.color = instance.color;
    return output;
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
};

@vertex
fn vs_mesh(
    vertex: VfxMeshVertexInput,
    instance: VfxMeshInstanceInput,
    @builtin(vertex_index) vertex_index: u32,
) -> VfxVertexOutput {
    var output: VfxVertexOutput;
    let scaled = vertex.position * instance.scale.xyz;
    let world = instance.position_flags.xyz + quat_rotate(instance.orientation, scaled);
    output.clip_position = vfx_depth_bias(camera.view_proj * vec4<f32>(world, 1.0));
    output.uv1 = instance.uv1.xy + vertex.uv * instance.uv1.zw;
    output.uv2 = instance.uv2.xy + vertex.uv * instance.uv2.zw;
    // 顶点色全通道参与：rgb 染色，alpha 为羽化遮罩（火舌翼缘 alpha=0
    // 消隐；实心壳体模型 alpha 恒 255 不受影响）。
    output.color = instance.color * vertex.color;
    return output;
}

// ===== 片元：双贴图合成 =====

fn compose_color(uv1: vec2<f32>, uv2: vec2<f32>, instance_color: vec4<f32>) -> vec4<f32> {
    var tex_color = vec4<f32>(1.0, 1.0, 1.0, 1.0);
    if (vfx_params.flags & 2u) != 0u {
        tex_color = textureSample(vfx_texture1, vfx_sampler, uv1);
        if (vfx_params.flags & 1u) != 0u {
            tex_color = vec4<f32>(tex_color.rgb, tex_color.r);
        }
    }
    if (vfx_params.flags & 4u) != 0u {
        var c2 = textureSample(vfx_texture2, vfx_sampler, uv2);
        if (vfx_params.flags & 8u) != 0u {
            c2 = vec4<f32>(c2.rgb, c2.r);
        }
        // TCCT：0 乘、1 加、2 减、3 max、4 min；其余（未知 5 等）不改变颜色。
        switch vfx_params.combine_color {
            case 0u: { tex_color = vec4<f32>(tex_color.rgb * c2.rgb, tex_color.a); }
            case 1u: { tex_color = vec4<f32>(tex_color.rgb + c2.rgb, tex_color.a); }
            case 2u: { tex_color = vec4<f32>(tex_color.rgb - c2.rgb, tex_color.a); }
            case 3u: { tex_color = vec4<f32>(max(tex_color.rgb, c2.rgb), tex_color.a); }
            case 4u: { tex_color = vec4<f32>(min(tex_color.rgb, c2.rgb), tex_color.a); }
            default: { }
        }
        // TCAT：0 乘、1 max、2 min；3（None）与其他值不改变 alpha。
        switch vfx_params.combine_alpha {
            case 0u: { tex_color = vec4<f32>(tex_color.rgb, tex_color.a * c2.a); }
            case 1u: { tex_color = vec4<f32>(tex_color.rgb, max(tex_color.a, c2.a)); }
            case 2u: { tex_color = vec4<f32>(tex_color.rgb, min(tex_color.a, c2.a)); }
            default: { }
        }
    }
    let rgb = tex_color.rgb * instance_color.rgb;
    let alpha = tex_color.a * instance_color.a;
    return vec4<f32>(rgb, alpha);
}

// 加色（RMT=Add 系）：alpha 作为强度折进 rgb，One/One 混合由管线给出。
@fragment
fn fs_add(input: VfxVertexOutput) -> @location(0) vec4<f32> {
    let color = compose_color(input.uv1, input.uv2, input.color);
    return vec4<f32>(color.rgb * color.a, 0.0);
}

// 普通 alpha 混合（RMT=Blend 系）：SrcAlpha/InvSrcAlpha 由管线给出。
@fragment
fn fs_blend(input: VfxVertexOutput) -> @location(0) vec4<f32> {
    return compose_color(input.uv1, input.uv2, input.color);
}
