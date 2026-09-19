// VFX particle shader, standalone package module compiled to OUT_DIR/vfx.wgsl
// by the build script. Draws camera-facing additive quads from an instance
// buffer produced by the CPU avfx sampler (`xiv-companion-data` avfx_sim).

struct Camera {
    view_proj: mat4x4<f32>,
    light_dir: vec4<f32>,
    view_dir: vec4<f32>,
    camera_position: vec4<f32>,
    options: vec4<f32>,
    dynamic_emissive_color: vec4<f32>,
};

@group(0) @binding(0) var<uniform> camera: Camera;
@group(1) @binding(0) var vfx_color_texture: texture_2d<f32>;
@group(1) @binding(1) var vfx_color_sampler: sampler;

struct VfxInstanceInput {
    @location(0) position: vec3<f32>,
    // xy: quad size (world units), z: in-plane rotation (radians)
    @location(1) size_rotation: vec4<f32>,
    // rgb may exceed 1.0 (HDR, picked up by bloom); a: opacity
    @location(2) color: vec4<f32>,
    // xy: uv frame origin, zw: uv scale
    @location(3) uv_params: vec4<f32>,
};

struct VfxVertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

const CORNERS: array<vec2<f32>, 6> = array<vec2<f32>, 6>(
    vec2<f32>(-1.0, -1.0),
    vec2<f32>(1.0, -1.0),
    vec2<f32>(1.0, 1.0),
    vec2<f32>(-1.0, -1.0),
    vec2<f32>(1.0, 1.0),
    vec2<f32>(-1.0, 1.0),
);

const UV_CORNERS: array<vec2<f32>, 6> = array<vec2<f32>, 6>(
    vec2<f32>(0.0, 1.0),
    vec2<f32>(1.0, 1.0),
    vec2<f32>(1.0, 0.0),
    vec2<f32>(0.0, 1.0),
    vec2<f32>(1.0, 0.0),
    vec2<f32>(0.0, 0.0),
);

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32,
    instance: VfxInstanceInput,
) -> VfxVertexOutput {
    let corner = CORNERS[vertex_index];
    let size = instance.size_rotation.xy;
    let rotation = instance.size_rotation.z;
    let cos_rotation = cos(rotation);
    let sin_rotation = sin(rotation);
    let rotated = vec2<f32>(
        corner.x * cos_rotation - corner.y * sin_rotation,
        corner.x * sin_rotation + corner.y * cos_rotation,
    ) * size;

    // Billboard basis around the particle position (view direction from the
    // orbit camera position toward the particle).
    let to_particle = instance.position - camera.camera_position.xyz;
    var forward = normalize(to_particle);
    if (dot(to_particle, to_particle) < 1.0e-8) {
        forward = vec3<f32>(0.0, 0.0, -1.0);
    }
    var right = normalize(cross(vec3<f32>(0.0, 1.0, 0.0), forward));
    if (dot(right, right) < 1.0e-8) {
        right = vec3<f32>(1.0, 0.0, 0.0);
    }
    let up = cross(forward, right);

    let world = instance.position + right * rotated.x + up * rotated.y;
    var output: VfxVertexOutput;
    output.clip_position = camera.view_proj * vec4<f32>(world, 1.0);
    // 深度偏移：贴附武器表面的粒子（发射模型顶点）深度与表面几乎相等，
    // 严格的 LessEqual 测试下会被表面剔除；把 NDC 深度向近处拉一个小量
    // （标准贴花手法，远小于场景深度差，不影响遮挡关系）。
    output.clip_position.z -= 0.002 * output.clip_position.w;
    let uv_corner = UV_CORNERS[vertex_index];
    output.uv = instance.uv_params.xy + uv_corner * instance.uv_params.zw;
    output.color = instance.color;
    return output;
}

@fragment
fn fs_main(input: VfxVertexOutput) -> @location(0) vec4<f32> {
    let tex = textureSample(vfx_color_texture, vfx_color_sampler, input.uv);
    // Additive: RGB weighted by texture alpha so masked-out regions contribute
    // nothing; the HDR target and bloom downstream pick up >1 values.
    return vec4<f32>(input.color.rgb * tex.rgb * tex.a, input.color.a);
}
