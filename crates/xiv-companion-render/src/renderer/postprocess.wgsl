struct PostUniform {
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

@fragment
fn blur_fs(input: VertexOutput) -> @location(0) vec4<f32> {
    let offset = post.params.xy;
    var color = textureSample(source_texture, source_sampler, input.uv).rgb * 0.227027;
    color += textureSample(source_texture, source_sampler, input.uv + offset * 1.384615).rgb * 0.316216;
    color += textureSample(source_texture, source_sampler, input.uv - offset * 1.384615).rgb * 0.316216;
    color += textureSample(source_texture, source_sampler, input.uv + offset * 3.230769).rgb * 0.070270;
    color += textureSample(source_texture, source_sampler, input.uv - offset * 3.230769).rgb * 0.070270;
    return vec4<f32>(color, 1.0);
}

@fragment
fn compose_fs(input: VertexOutput) -> @location(0) vec4<f32> {
    let scene = textureSample(source_texture, source_sampler, input.uv).rgb;
    let bloom = textureSample(bloom_texture, source_sampler, input.uv).rgb;
    let strength = post.params.x;
    let color = scene + bloom * strength;
    return vec4<f32>(color, 1.0);
}
