import package::vfx::{VfxVertexOutput, camera, compose_color, move_uv};

struct DecalInstance {
    world: mat4x4<f32>,
    inverse_world: mat4x4<f32>,
    color: vec4<f32>,
    uv1: vec4<f32>,
    uv2: vec4<f32>,
    uv3: vec4<f32>,
    uv4: vec4<f32>,
    uvd: vec4<f32>,
    distortion: vec4<u32>,
    // x: raw SS, y: distance-corrected WID + WIDR, z: RF, w: ring flag.
    projection: vec4<f32>,
    // x: DDTT.
    metadata: vec4<i32>,
};

struct DecalVertexInput {
    @location(0) position: vec4<f32>,
};

struct DecalVertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    // Client forwards clip x/y/w and divides in the pixel shader.
    @location(0) projected: vec3<f32>,
    @location(1) @interpolate(flat) instance_index: u32,
};

@group(2) @binding(0) var<storage, read> decal_instances: array<DecalInstance>;

@vertex
fn vs_decal(vertex: DecalVertexInput, @builtin(instance_index) instance_index: u32) -> DecalVertexOutput {
    let instance = decal_instances[instance_index];
    let clip = camera.view_proj * instance.world * vertex.position;
    var output: DecalVertexOutput;
    output.clip_position = clip;
    output.projected = vec3<f32>(clip.x, clip.y, clip.w);
    output.instance_index = instance_index;
    return output;
}

fn unpack_rotation(packed: u32, high: bool) -> f32 {
    let value = select(packed & 0xFFFFu, packed >> 16u, high);
    return f32(value) / 65535.0 * (2.0 * 3.14159265);
}

fn decal_compose(input: DecalVertexOutput, depth: f32) -> vec4<f32> {
    let instance = decal_instances[input.instance_index];
    let ndc = input.projected.xy / input.projected.z;
    let world_h = camera.inverse_view_proj * vec4<f32>(ndc, depth, 1.0);
    if abs(world_h.w) < 1.0e-8 {
        discard;
    }
    let local_h = instance.inverse_world * vec4<f32>(world_h.xyz / world_h.w, 1.0);
    if abs(local_h.w) < 1.0e-8 {
        discard;
    }
    let local = local_h.xyz / local_h.w;
    let edge = min(5.0 * (0.5 - abs(local.y)), 1.0);
    if edge < 0.0 {
        discard;
    }
    if instance.projection.w < 0.5 {
        if abs(local.x) >= 0.5 || abs(local.z) >= 0.5 {
            discard;
        }
    } else {
        let radius = length(local.xz);
        let width = instance.projection.y;
        if radius > 0.5 + width || radius < 0.5 - width {
            discard;
        }
        let normalized_z = select(0.0, local.z / radius, radius > 1.0e-8);
        if normalized_z + 2.0 * instance.projection.z < 1.0 {
            discard;
        }
    }

    // Decal shaders use (-local.x, -local.z, 1) as the source for each UV
    // transform, equivalent to this normalized projection coordinate.
    let base_uv = vec2<f32>(0.5 - local.x, 0.5 - local.z);
    var particle: VfxVertexOutput;
    particle.clip_position = input.clip_position;
    particle.uv1 = move_uv(instance.uv1, base_uv, unpack_rotation(instance.distortion.z, false));
    particle.uv2 = move_uv(instance.uv2, base_uv, unpack_rotation(instance.distortion.z, true));
    particle.uv3 = move_uv(instance.uv3, base_uv, unpack_rotation(instance.distortion.w, false));
    particle.uv4 = move_uv(instance.uv4, base_uv, unpack_rotation(instance.distortion.w, true));
    particle.uvd = move_uv(instance.uvd, base_uv, 0.0);
    particle.distortion_power = bitcast<f32>(instance.distortion.x);
    particle.color = vec4<f32>(instance.color.rgb, instance.color.a * edge);
    particle.distortion_targets = instance.distortion.y;
    particle.normal = vec3<f32>(0.0);
    particle.fresnel_direction_power = vec4<f32>(0.0);
    particle.fresnel_color_begin = vec4<f32>(1.0);
    particle.fresnel_color_end = vec4<f32>(1.0);
    particle.fresnel_enabled = 0u;
    return compose_color(particle);
}

fn decal_multiply(input: DecalVertexOutput, depth: f32) -> vec4<f32> {
    let color = decal_compose(input, depth);
    let alpha = min(color.a, 1.0);
    return vec4<f32>(mix(vec3<f32>(1.0), clamp(color.rgb, vec3<f32>(0.0), vec3<f32>(1.0)), alpha), alpha);
}

fn decal_screen(input: DecalVertexOutput, depth: f32) -> vec4<f32> {
    let color = decal_compose(input, depth);
    return vec4<f32>(clamp(color.rgb * color.a, vec3<f32>(0.0), vec3<f32>(1.0)), min(color.a, 1.0));
}

fn decal_blend(input: DecalVertexOutput, depth: f32) -> vec4<f32> {
    let color = decal_compose(input, depth);
    return vec4<f32>(color.rgb, min(color.a, 1.0));
}
