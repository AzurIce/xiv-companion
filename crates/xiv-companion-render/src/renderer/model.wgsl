struct Camera {
    view_proj: mat4x4<f32>,
    light_dir: vec4<f32>,
    options: vec4<f32>, // x: normal mapping, y: normal y sign
};

struct Material {
    diffuse_color: vec4<f32>,
    emissive_color: vec4<f32>, // a: has emissive texture
    specular_color: vec4<f32>,
    params: vec4<f32>, // x: has base, y: metalness, z: has normal, w: has mask
    properties: vec4<f32>, // x: has ColorTable material properties texture, y: has specular texture, z: apply vertex color
    render: vec4<f32>, // x: render mode, y: opacity, z: alpha mode 0=opaque 1=mask 2=blend 3=glass, w: alpha threshold
};

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv0: vec2<f32>,
    @location(3) bitangent: vec4<f32>,
    @location(4) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) uv0: vec2<f32>,
    @location(2) bitangent: vec4<f32>,
    @location(3) color: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> camera: Camera;

@group(1) @binding(0)
var<uniform> material: Material;

@group(1) @binding(1)
var base_color_texture: texture_2d<f32>;

@group(1) @binding(2)
var base_color_sampler: sampler;

@group(1) @binding(3)
var normal_texture: texture_2d<f32>;

@group(1) @binding(4)
var mask_texture: texture_2d<f32>;

@group(1) @binding(5)
var emissive_texture: texture_2d<f32>;

@group(1) @binding(6)
var material_properties_texture: texture_2d<f32>;

@group(1) @binding(7)
var specular_texture: texture_2d<f32>;

struct FragmentOutput {
    @location(0) color: vec4<f32>,
    @location(1) bright: vec4<f32>,
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = camera.view_proj * vec4<f32>(input.position, 1.0);
    out.normal = normalize(input.normal);
    out.uv0 = input.uv0;
    out.bitangent = input.bitangent;
    out.color = input.color;
    return out;
}

@fragment
fn fs_main(input: VertexOutput, @builtin(front_facing) front_facing: bool) -> FragmentOutput {
    let normal = resolve_normal(input, front_facing);
    let light = normalize(camera.light_dir.xyz);
    let diffuse = max(dot(normal, light), 0.0);
    let half_dir = normalize(light + vec3<f32>(0.0, 0.0, 1.0));
    let mask = resolve_mask(input.uv0);
    let properties = resolve_material_properties(input.uv0);
    let metalness = clamp(properties.x, 0.0, 1.0);
    let roughness = clamp(properties.y, 0.08, 1.0);
    let gloss_strength = clamp(properties.z, 0.0, 1.0);
    let specular_strength = clamp(properties.w, 0.0, 1.0);
    let specular_scale = specular_strength * mix(1.0, mask.r * 1.35, material.params.w);
    let specular_power = mix(12.0, 96.0, gloss_strength) * (1.0 - roughness * 0.55);
    let specular = pow(max(dot(normal, half_dir), 0.0), specular_power);
    let sampled_base = textureSample(base_color_texture, base_color_sampler, input.uv0);
    let sampled_specular = textureSample(specular_texture, base_color_sampler, input.uv0).rgb;
    let texture_mix = select(vec3<f32>(1.0), sampled_base.rgb, material.params.x > 0.5);
    let texture_alpha = select(1.0, sampled_base.a, material.params.x > 0.5);
    let material_specular = select(material.specular_color.rgb, sampled_specular, material.properties.y > 0.5);
    let vertex_tint = select(vec3<f32>(1.0), input.color.rgb, material.properties.z > 0.5);
    let is_mask = material.render.z > 0.5 && material.render.z < 1.5;
    let is_blend = material.render.z > 1.5 && material.render.z < 2.5;
    let is_glass = material.render.z > 2.5 || material.render.x > 1.5;
    let uses_alpha = is_mask || is_blend || is_glass || material.render.x > 0.5;
    let base = material.diffuse_color.rgb * texture_mix * vertex_tint;
    var alpha = select(1.0, clamp(material.diffuse_color.a * texture_alpha * input.color.a, 0.0, 1.0), uses_alpha);
    if is_glass {
        alpha = clamp(material.render.y * texture_alpha * input.color.a, 0.05, 0.55);
    }
    if is_mask && alpha < material.render.w {
        discard;
    }
    if uses_alpha && alpha < 0.01 {
        discard;
    }
    let rim = pow(1.0 - max(normal.z, 0.0), 2.0) * select(0.16, 0.58, is_glass);
    let specular_tint = mix(material_specular, base, metalness * 0.35);
    let glass_tint = mix(base, vec3<f32>(0.70, 0.93, 1.0), 0.55);
    let opaque_lit = base * (0.22 + diffuse * 0.74)
        + specular_tint * specular * specular_scale * 0.24
        + vec3<f32>(rim);
    let glass_lit = glass_tint * (0.10 + diffuse * 0.18)
        + material_specular * specular * 0.65
        + vec3<f32>(rim) * vec3<f32>(0.60, 0.85, 1.0);
    let lit = select(opaque_lit, glass_lit, is_glass);
    let emissive_tex = textureSample(emissive_texture, base_color_sampler, input.uv0).rgb;
    let emissive = resolve_emissive(emissive_tex, input.color.a, mask);
    let color = lit + emissive;
    let luma = dot(color, vec3<f32>(0.2126, 0.7152, 0.0722));
    let highlight = max(color - vec3<f32>(0.72), vec3<f32>(0.0)) * smoothstep(0.72, 1.0, luma);

    var out: FragmentOutput;
    out.color = vec4<f32>(color, alpha);
    out.bright = vec4<f32>(emissive * 1.15 + highlight * 0.65, 1.0);
    return out;
}

fn resolve_mask(uv: vec2<f32>) -> vec3<f32> {
    if material.params.w <= 0.5 {
        return vec3<f32>(1.0, material.specular_color.a, material.params.y);
    }
    return textureSample(mask_texture, base_color_sampler, uv).rgb;
}

fn resolve_material_properties(uv: vec2<f32>) -> vec4<f32> {
    if material.properties.x > 0.5 {
        return textureSample(material_properties_texture, base_color_sampler, uv);
    }

    let mask = resolve_mask(uv);
    let metalness = clamp(max(material.params.y, mask.b * material.params.w), 0.0, 1.0);
    let roughness = clamp(mix(material.specular_color.a, mask.g, material.params.w), 0.08, 1.0);
    let specular_strength = mix(1.0, mask.r * 1.35, material.params.w);
    let gloss_strength = clamp((1.0 - roughness) * 0.75 + 0.25, 0.0, 1.0);
    return vec4<f32>(metalness, roughness, gloss_strength, specular_strength);
}

fn resolve_emissive(emissive_tex: vec3<f32>, vertex_alpha: f32, mask: vec3<f32>) -> vec3<f32> {
    let material_emissive = clamp(material.emissive_color.rgb, vec3<f32>(0.0), vec3<f32>(4.0));
    let texture_strength = material.emissive_color.a;
    let texture_luma = dot(emissive_tex, vec3<f32>(0.2126, 0.7152, 0.0722));
    let texture_gate = smoothstep(0.02, 0.28, texture_luma) * texture_strength;
    let mask_gate = smoothstep(0.88, 1.0, mask.b) * material.params.w * 0.18;
    let vertex_gate = 0.35 + clamp(vertex_alpha, 0.0, 1.0) * 0.65;
    return emissive_tex * texture_strength + material_emissive * (texture_gate + mask_gate) * vertex_gate;
}

fn resolve_normal(input: VertexOutput, front_facing: bool) -> vec3<f32> {
    let face_sign = select(-1.0, 1.0, front_facing);
    let geometric_normal = normalize(input.normal) * face_sign;
    let sampled = textureSample(normal_texture, base_color_sampler, input.uv0).xyz * 2.0 - vec3<f32>(1.0);
    if camera.options.x <= 0.5 || material.params.z <= 0.5 || dot(input.bitangent.xyz, input.bitangent.xyz) <= 0.0001 {
        return geometric_normal;
    }

    let bitangent = normalize(input.bitangent.xyz);
    let tangent_sign = select(1.0, -1.0, input.bitangent.w < 0.0);
    let tangent = normalize(cross(bitangent, geometric_normal)) * tangent_sign;
    let mapped = normalize(vec3<f32>(sampled.x, sampled.y * camera.options.y, sampled.z));
    return normalize(tangent * mapped.x + bitangent * mapped.y + geometric_normal * mapped.z);
}
