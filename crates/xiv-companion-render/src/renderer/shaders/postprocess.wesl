struct PostUniform {
    // blur: xy: texel offset, z: bloom threshold (scene-linear), w: extract bright-pass flag
    // compose: x: bloom strength, y: exposure, z: encode sRGB in shader (non-sRGB target), w: unused
    params: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@group(0) @binding(0)
var source_texture: texture_2d<f32>;

@group(0) @binding(1)
var source_sampler: sampler;

@group(0) @binding(2)
var<uniform> post: PostUniform;

@group(0) @binding(3)
var bloom_texture: texture_2d<f32>;

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );

    let position = positions[vertex_index];
    var out: VertexOutput;
    out.clip_position = vec4<f32>(position, 0.0, 1.0);
    out.uv = vec2<f32>(position.x * 0.5 + 0.5, 0.5 - position.y * 0.5);
    return out;
}

// Scene-linear bright-pass: only HDR highlights above the threshold bloom.
// The knee smooths the transition so highlights near the threshold do not pop.
const BLOOM_KNEE = 0.5;

fn bloom_contribution(color: vec3<f32>, threshold: f32) -> vec3<f32> {
    let luma = dot(color, vec3<f32>(0.2126, 0.7152, 0.0722));
    let weight = smoothstep(threshold, threshold + BLOOM_KNEE, luma);
    return color * weight;
}

fn bloom_tap(uv: vec2<f32>) -> vec3<f32> {
    let color = textureSample(source_texture, source_sampler, uv).rgb;
    return mix(color, bloom_contribution(color, post.params.z), post.params.w);
}

@fragment
fn blur_fs(input: VertexOutput) -> @location(0) vec4<f32> {
    let offset = post.params.xy;
    var color = bloom_tap(input.uv) * 0.227027;
    color += bloom_tap(input.uv + offset * 1.384615) * 0.316216;
    color += bloom_tap(input.uv - offset * 1.384615) * 0.316216;
    color += bloom_tap(input.uv + offset * 3.230769) * 0.070270;
    color += bloom_tap(input.uv - offset * 3.230769) * 0.070270;
    return vec4<f32>(color, 1.0);
}

@fragment
fn compose_fs(input: VertexOutput) -> @location(0) vec4<f32> {
    let scene = textureSample(source_texture, source_sampler, input.uv).rgb;
    let bloom = textureSample(bloom_texture, source_sampler, input.uv).rgb;
    let strength = post.params.x;
    let exposure = post.params.y;
    var color = (scene + bloom * strength) * exposure;
    color = tonemap_pbr_neutral(color);
    if post.params.z > 0.5 {
        color = vec3<f32>(
            linear_to_srgb_channel(color.r),
            linear_to_srgb_channel(color.g),
            linear_to_srgb_channel(color.b),
        );
    }
    return vec4<f32>(color, 1.0);
}

// Khronos PBR Neutral tone mapper (https://github.com/KhronosGroup/ToneMapping,
// PBRNeutral.glsl). Identity for peak values below 0.8 - 0.04, smooth
// compression up to the 1.2 - 0.04 desaturated range for HDR highlights.
fn tonemap_pbr_neutral(input_color: vec3<f32>) -> vec3<f32> {
    const start_compression = 0.76;
    const desaturation = 0.15;

    var color = input_color;
    let x = min(color.r, min(color.g, color.b));
    let offset = select(0.04, x - 6.25 * x * x, x < 0.08);
    color -= vec3<f32>(offset);

    let peak = max(color.r, max(color.g, color.b));
    if peak < start_compression {
        return color;
    }

    let d = 1.0 - start_compression;
    let new_peak = 1.0 - d * d / (peak + d - start_compression);
    color *= new_peak / peak;
    let g = 1.0 - 1.0 / (desaturation * (peak - new_peak) + 1.0);
    return mix(color, vec3<f32>(new_peak), g);
}

fn linear_to_srgb_channel(value: f32) -> f32 {
    let clamped = clamp(value, 0.0, 1.0);
    let low = clamped * 12.92;
    let high = 1.055 * pow(clamped, 1.0 / 2.4) - 0.055;
    return select(high, low, clamped <= 0.0031308);
}
