struct Camera {
    view_proj: mat4x4<f32>,
    light_dir: vec4<f32>,
};

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec3<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) color: vec3<f32>,
};

@group(0) @binding(0)
var<uniform> camera: Camera;

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = camera.view_proj * vec4<f32>(input.position, 1.0);
    out.normal = normalize(input.normal);
    out.color = input.color;
    return out;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let normal = normalize(input.normal);
    let light = normalize(camera.light_dir.xyz);
    let diffuse = max(dot(normal, light), 0.0);
    let rim = pow(1.0 - max(normal.z, 0.0), 2.0) * 0.18;
    let shaded = input.color * (0.25 + diffuse * 0.72) + vec3<f32>(rim);
    return vec4<f32>(shaded, 1.0);
}
