struct Camera {
    view_proj: mat4x4<f32>,
    light_dir: vec4<f32>,
    options: vec4<f32>, // x: normal mapping, y: normal y sign, z: uv scroll time, w: debug mode
};

struct Material {
    diffuse_color: vec4<f32>,
    emissive_color: vec4<f32>, // a: has emissive texture
    specular_color: vec4<f32>,
    params: vec4<f32>, // x: has base, y: metalness, z: has normal, w: has mask
    properties: vec4<f32>, // x: has ColorTable material properties texture, y: has specular texture, z: apply vertex color
    render: vec4<f32>, // x: render mode, y: opacity, z: alpha mode 0=opaque 1=mask 2=blend 3=glass, w: alpha threshold
    alpha_params: vec4<f32>, // x: aperture, y: offset, z: shadow alpha threshold, w: transparency
    glass_params: vec4<f32>, // x: IOR, y: max thickness
    extra_properties: vec4<f32>, // x: tile, y: sheen, z: sphere, w: tile matrix
    shader_params: vec4<f32>, // x: normal, y: multi normal, z: detail normal, w: multi detail normal
    tile_params: vec4<f32>, // x: tile index, y: tile alpha, zw: tile repeat uv
    toon_sheen_params: vec4<f32>, // x: toon index, y: toon light scale, z: sheen rate, w: sheen tint rate
    sheen_sphere_params: vec4<f32>, // x: sheen aperture, y: sphere map index
    detail_params: vec4<f32>, // x: detail id, y: multi detail id
    detail_color: vec4<f32>,
    multi_detail_color: vec4<f32>,
    shader_diffuse_color: vec4<f32>,
    shader_multi_diffuse_color: vec4<f32>,
    shader_emissive_color: vec4<f32>,
    shader_multi_emissive_color: vec4<f32>,
    outline_params: vec4<f32>, // rgb: outline color, a: outline width
    specular_color_mask: vec4<f32>,
    surface_params: vec4<f32>, // x: ssao mask, y: texture mip bias, z: shadow pos offset
    detail_color_uv_scale: vec4<f32>, // xy: detail color repeat, zw: multi detail color repeat
    detail_normal_uv_scale: vec4<f32>, // xy: detail normal repeat, zw: multi detail normal repeat
    uv_scroll: vec4<f32>, // xy: uv0 scroll multiplier, zw: uv1 scroll multiplier
    lightshaft_color: vec4<f32>,
    lightshaft_tex_anim: vec4<f32>,
    lightshaft_tex_u: vec4<f32>,
    lightshaft_tex_v: vec4<f32>,
    lightshaft_ray: vec4<f32>,
    uv_sources0: vec4<f32>, // x: base, y: normal, z: mask, w: material map
    uv_sources1: vec4<f32>, // x: multi, y: specular, z: emissive, w: material properties
    uv_sources2: vec4<f32>, // x: tile, y: sheen, z: sphere, w: tile matrix
    uv_sources3: vec4<f32>, // x: ColorTable index, y: other
    draw_role_params: vec4<f32>, // x: lightshaft draw role
    debug_color: vec4<f32>, // xyz: mesh/draw-role debug color
};

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv0: vec2<f32>,
    @location(3) bitangent: vec4<f32>,
    @location(4) color: vec4<f32>,
    @location(5) uv1: vec2<f32>,
    @location(6) uv2: vec2<f32>,
    @location(7) uv3: vec2<f32>,
    @location(8) color1: vec4<f32>,
    @location(9) normal1: vec3<f32>,
    @location(10) bitangent1: vec4<f32>,
    @location(11) flow0: vec4<f32>,
    @location(12) flow1: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) uv0: vec2<f32>,
    @location(2) bitangent: vec4<f32>,
    @location(3) color: vec4<f32>,
    @location(4) uv1: vec2<f32>,
    @location(5) uv2: vec2<f32>,
    @location(6) uv3: vec2<f32>,
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

@group(1) @binding(8)
var data_sampler: sampler;

@group(1) @binding(9)
var tile_properties_texture: texture_2d<f32>;

@group(1) @binding(10)
var sheen_properties_texture: texture_2d<f32>;

@group(1) @binding(11)
var sphere_properties_texture: texture_2d<f32>;

@group(1) @binding(12)
var tile_matrix_texture: texture_2d<f32>;

@group(1) @binding(13)
var nearest_data_sampler: sampler;

@group(1) @binding(14)
var color_table_index_texture: texture_2d<f32>;

@group(1) @binding(15)
var material_map_texture: texture_2d<f32>;

@group(1) @binding(16)
var multi_map_texture: texture_2d<f32>;

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
    out.uv1 = input.uv1;
    out.uv2 = input.uv2;
    out.uv3 = input.uv3;
    return out;
}

@fragment
fn fs_main(input: VertexOutput, @builtin(front_facing) front_facing: bool) -> FragmentOutput {
    let is_lightshaft = material.draw_role_params.x > 0.5;
    var base_uv = resolve_uv(input, material.uv_sources0.x);
    if is_lightshaft {
        base_uv = resolve_lightshaft_uv(input);
    }
    let normal_uv = resolve_uv(input, material.uv_sources0.y);
    let mask_uv = resolve_uv(input, material.uv_sources0.z);
    let specular_uv = resolve_uv(input, material.uv_sources1.y);
    let emissive_uv = resolve_uv(input, material.uv_sources1.z);
    let material_properties_uv = resolve_uv(input, material.uv_sources1.w);

    let normal = resolve_normal(input, front_facing, normal_uv);
    let light = normalize(camera.light_dir.xyz);
    let diffuse = max(dot(normal, light), 0.0);
    let half_dir = normalize(light + vec3<f32>(0.0, 0.0, 1.0));
    let mask = resolve_mask(mask_uv);
    let properties = resolve_material_properties(material_properties_uv, mask);
    let metalness = clamp(properties.x, 0.0, 1.0);
    let roughness = clamp(properties.y, 0.08, 1.0);
    let gloss_strength = clamp(properties.z, 0.0, 1.0);
    let specular_strength = clamp(properties.w, 0.0, 1.0);
    let extra = resolve_extra_properties(input);
    let tile_specular_scale = resolve_tile_specular_scale(input, extra);
    let specular_color_mask = clamp(material.specular_color_mask, vec4<f32>(0.0), vec4<f32>(4.0));
    let specular_scale = specular_strength
        * mix(1.0, mask.r * 1.35, material.params.w)
        * tile_specular_scale
        * specular_color_mask.a;
    let specular_power = mix(12.0, 96.0, gloss_strength) * (1.0 - roughness * 0.55);
    let specular = pow(max(dot(normal, half_dir), 0.0), specular_power);
    let sampled_base = textureSample(base_color_texture, base_color_sampler, base_uv);
    let sampled_specular = textureSample(specular_texture, base_color_sampler, specular_uv).rgb;
    let emissive_tex = textureSample(emissive_texture, base_color_sampler, emissive_uv).rgb;
    let texture_mix = select(vec3<f32>(1.0), sampled_base.rgb, material.params.x > 0.5);
    let texture_alpha = select(1.0, sampled_base.a, material.params.x > 0.5);
    let material_specular = select(material.specular_color.rgb, sampled_specular, material.properties.y > 0.5)
        * specular_color_mask.rgb;
    let vertex_tint = select(vec3<f32>(1.0), input.color.rgb, material.properties.z > 0.5);
    let is_mask = material.render.z > 0.5 && material.render.z < 1.5;
    let is_blend = material.render.z > 1.5 && material.render.z < 2.5;
    let is_glass = material.render.z > 2.5 || material.render.x > 1.5;
    let uses_alpha = is_mask || is_blend || is_glass || is_lightshaft || material.render.x > 0.5;
    let shader_tint = resolve_shader_diffuse_tint(mask);
    let detail_tint = resolve_detail_tint(input);
    let base = material.diffuse_color.rgb * texture_mix * vertex_tint * shader_tint * detail_tint;
    var alpha = select(1.0, clamp(material.diffuse_color.a * texture_alpha * input.color.a, 0.0, 1.0), uses_alpha);
    if uses_alpha && !is_glass && !is_lightshaft {
        alpha = resolve_alpha_shaping(alpha);
    }
    if is_glass {
        alpha = clamp(material.render.y * texture_alpha * input.color.a, 0.05, 0.55);
    }
    let emissive = resolve_emissive(emissive_tex, input.color.a, mask);
    if camera.options.w > 0.5 {
        return debug_fragment_output(input, camera.options.w, base, normal, mask, properties, material_specular, emissive, alpha);
    }
    if is_mask && alpha < material.render.w {
        discard;
    }
    if uses_alpha && alpha < 0.01 {
        discard;
    }
    if is_lightshaft {
        let lightshaft = resolve_lightshaft_color(base, texture_alpha, input.color.a);
        var out: FragmentOutput;
        out.color = lightshaft;
        out.bright = vec4<f32>(lightshaft.rgb * 1.15, 1.0);
        return out;
    }
    let rim = pow(1.0 - max(normal.z, 0.0), 2.0) * select(0.16, 0.58, is_glass);
    let specular_tint = mix(material_specular, base, metalness * 0.35);
    let glass_factors = resolve_glass_factors();
    let glass_tint = mix(base, vec3<f32>(0.70, 0.93, 1.0), 0.55 + glass_factors.y * 0.18);
    let ssao_mask = clamp(material.surface_params.x, 0.0, 1.0);
    let ambient = mix(0.08, 0.22, ssao_mask);
    let glass_ambient = mix(0.06, 0.10, ssao_mask);
    let opaque_lit = base * (ambient + diffuse * 0.74)
        + specular_tint * specular * specular_scale * 0.24
        + vec3<f32>(rim);
    let glass_lit = glass_tint * (glass_ambient + diffuse * 0.18)
        + material_specular * specular * (0.65 + glass_factors.z * 0.25)
        + vec3<f32>(rim) * vec3<f32>(0.60, 0.85, 1.0) * (1.0 + glass_factors.x * 0.35);
    let lit = select(opaque_lit, glass_lit, is_glass);
    let extra_lit = resolve_extra_lighting(extra, normal, half_dir, rim, material_specular, base, is_glass);
    let outlined = resolve_outline_rim(lit + extra_lit, rim, is_glass);
    let color = outlined + emissive;
    let luma = dot(color, vec3<f32>(0.2126, 0.7152, 0.0722));
    let highlight = max(color - vec3<f32>(0.72), vec3<f32>(0.0)) * smoothstep(0.72, 1.0, luma);

    var out: FragmentOutput;
    out.color = vec4<f32>(color, alpha);
    out.bright = vec4<f32>(emissive * 1.15 + highlight * 0.65, 1.0);
    return out;
}

fn debug_fragment_output(
    input: VertexOutput,
    mode: f32,
    base: vec3<f32>,
    normal: vec3<f32>,
    mask: vec3<f32>,
    properties: vec4<f32>,
    specular: vec3<f32>,
    emissive: vec3<f32>,
    alpha: f32,
) -> FragmentOutput {
    var color = base;
    if mode < 1.5 {
        color = base;
    } else if mode < 2.5 {
        color = normal * 0.5 + vec3<f32>(0.5);
    } else if mode < 3.5 {
        color = mask;
    } else if mode < 4.5 {
        color = vec3<f32>(properties.x, properties.y, properties.w);
    } else if mode < 5.5 {
        color = specular;
    } else if mode < 6.5 {
        color = emissive;
    } else if mode < 7.5 {
        color = vec3<f32>(alpha);
    } else if mode < 8.5 {
        color = uv_debug_color(input.uv0);
    } else if mode < 9.5 {
        color = uv_debug_color(input.uv1);
    } else if mode < 10.5 {
        color = uv_debug_color(input.uv2);
    } else if mode < 11.5 {
        color = uv_debug_color(input.uv3);
    } else if mode < 12.5 {
        color = input.color.rgb;
    } else if mode < 13.5 {
        color = material.debug_color.rgb;
    } else if mode < 14.5 {
        let index_uv = resolve_uv(input, material.uv_sources3.x);
        let index_sample = textureSample(color_table_index_texture, nearest_data_sampler, index_uv);
        color = vec3<f32>(index_sample.r, index_sample.g, 0.5);
    } else if mode < 15.5 {
        let material_map_uv = resolve_uv(input, material.uv_sources0.w);
        color = textureSample(material_map_texture, data_sampler, material_map_uv).rgb;
    } else if mode < 16.5 {
        let multi_map_uv = resolve_uv(input, material.uv_sources1.x);
        color = textureSample(multi_map_texture, data_sampler, multi_map_uv).rgb;
    } else if mode < 17.5 {
        let tile_uv = resolve_uv(input, material.uv_sources2.x);
        color = textureSample(tile_properties_texture, nearest_data_sampler, tile_uv).rgb;
    } else if mode < 18.5 {
        let sheen_uv = resolve_uv(input, material.uv_sources2.y);
        color = textureSample(sheen_properties_texture, nearest_data_sampler, sheen_uv).rgb;
    } else if mode < 19.5 {
        let sphere_uv = resolve_uv(input, material.uv_sources2.z);
        color = textureSample(sphere_properties_texture, nearest_data_sampler, sphere_uv).rgb;
    } else {
        let tile_matrix_uv = resolve_uv(input, material.uv_sources2.w);
        color = textureSample(tile_matrix_texture, nearest_data_sampler, tile_matrix_uv).rgb;
    }

    var out: FragmentOutput;
    out.color = vec4<f32>(clamp(color, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
    out.bright = vec4<f32>(0.0, 0.0, 0.0, 1.0);
    return out;
}

fn uv_debug_color(uv: vec2<f32>) -> vec3<f32> {
    return vec3<f32>(fract(uv.x), fract(uv.y), 0.5);
}

fn resolve_mask(uv: vec2<f32>) -> vec3<f32> {
    if material.params.w <= 0.5 {
        return vec3<f32>(1.0, material.specular_color.a, material.params.y);
    }
    return textureSample(mask_texture, data_sampler, uv).rgb;
}

fn resolve_material_properties(uv: vec2<f32>, mask: vec3<f32>) -> vec4<f32> {
    if material.properties.x > 0.5 {
        return textureSample(material_properties_texture, data_sampler, uv);
    }

    let metalness = clamp(max(material.params.y, mask.b * material.params.w), 0.0, 1.0);
    let roughness = clamp(mix(material.specular_color.a, mask.g, material.params.w), 0.08, 1.0);
    let specular_strength = mix(1.0, mask.r * 1.35, material.params.w);
    let gloss_strength = clamp((1.0 - roughness) * 0.75 + 0.25, 0.0, 1.0);
    return vec4<f32>(metalness, roughness, gloss_strength, specular_strength);
}

struct ExtraProperties {
    tile: vec4<f32>,
    sheen: vec4<f32>,
    sphere: vec4<f32>,
    tile_matrix: vec4<f32>,
    flags: vec4<f32>,
};

fn resolve_extra_properties(input: VertexOutput) -> ExtraProperties {
    var extra: ExtraProperties;
    let has_tile = material.extra_properties.x > 0.5;
    let has_sheen = material.extra_properties.y > 0.5;
    let has_sphere = material.extra_properties.z > 0.5;
    let has_tile_matrix = material.extra_properties.w > 0.5;
    extra.flags = vec4<f32>(
        select(0.0, 1.0, has_tile),
        select(0.0, 1.0, has_sheen),
        select(0.0, 1.0, has_sphere),
        select(0.0, 1.0, has_tile_matrix),
    );
    extra.tile = vec4<f32>(0.0, 1.0, 1.0, 1.0);
    extra.sheen = vec4<f32>(0.0, 0.0, 0.0, 1.0);
    extra.sphere = vec4<f32>(0.0, 0.0, 1.0, 1.0);
    extra.tile_matrix = vec4<f32>(1.0, 0.0, 0.0, 1.0);
    if has_tile {
        extra.tile = textureSample(tile_properties_texture, nearest_data_sampler, resolve_uv(input, material.uv_sources2.x));
    }
    if has_sheen {
        extra.sheen = textureSample(sheen_properties_texture, nearest_data_sampler, resolve_uv(input, material.uv_sources2.y));
    }
    if has_sphere {
        extra.sphere = textureSample(sphere_properties_texture, nearest_data_sampler, resolve_uv(input, material.uv_sources2.z));
    }
    if has_tile_matrix {
        extra.tile_matrix = textureSample(tile_matrix_texture, nearest_data_sampler, resolve_uv(input, material.uv_sources2.w));
    }
    return extra;
}

fn resolve_extra_lighting(
    extra: ExtraProperties,
    normal: vec3<f32>,
    half_dir: vec3<f32>,
    rim: f32,
    material_specular: vec3<f32>,
    base: vec3<f32>,
    is_glass: bool,
) -> vec3<f32> {
    let ramp_sheen_rate = clamp(extra.sheen.x, 0.0, 1.0) * extra.flags.y;
    let shader_sheen_rate = clamp(material.toon_sheen_params.z, 0.0, 1.0);
    let shader_sheen_active = select(0.0, 1.0, shader_sheen_rate > 0.001);
    let sheen_rate = clamp(ramp_sheen_rate + shader_sheen_rate * (1.0 - ramp_sheen_rate), 0.0, 1.0);
    let sheen_tint = max(
        clamp(extra.sheen.y, 0.0, 1.0) * extra.flags.y,
        clamp(material.toon_sheen_params.w, 0.0, 1.0) * shader_sheen_active,
    );
    let sheen_aptitude = mix(
        clamp(extra.sheen.z, 0.0, 1.0),
        clamp(material.sheen_sphere_params.x, 0.0, 1.0),
        shader_sheen_active,
    );
    let sheen_power = mix(24.0, 160.0, sheen_aptitude);
    let sheen_term = pow(max(dot(normal, half_dir), 0.0), sheen_power) * sheen_rate;
    let sheen_color = mix(material_specular, base, sheen_tint * 0.45) * sheen_term * 0.42;

    let sphere_index = max(
        clamp(extra.sphere.x, 0.0, 1.0),
        clamp(material.sheen_sphere_params.y, 0.0, 1.0),
    );
    let sphere_mask = clamp(extra.sphere.y, 0.0, 1.0) * extra.flags.z;
    let sphere_tint = mix(vec3<f32>(0.55, 0.68, 0.82), material_specular, sphere_index);
    let sphere_term = rim * sphere_mask * select(0.18, 0.10, is_glass);

    let matrix_delta = vec4<f32>(
        extra.tile_matrix.x - 1.0,
        extra.tile_matrix.y,
        extra.tile_matrix.z,
        extra.tile_matrix.w - 1.0,
    );
    let matrix_term = clamp(length(matrix_delta) * 0.16, 0.0, 0.18) * extra.flags.w;

    return sheen_color + sphere_tint * sphere_term + material_specular * matrix_term * 0.18;
}

fn resolve_tile_specular_scale(input: VertexOutput, extra: ExtraProperties) -> f32 {
    let ramp_tile_alpha = clamp(extra.tile.y, 0.0, 1.0);
    let ramp_scale = mix(1.0, mix(0.88, 1.16, ramp_tile_alpha), extra.flags.x);

    let shader_tile_alpha = clamp(material.tile_params.y, 0.0, 1.0);
    let tile_repeat = max(abs(material.tile_params.zw), vec2<f32>(0.001));
    let repeat_delta = length((tile_repeat - vec2<f32>(16.0)) / 16.0);
    let shader_tile_enabled = select(
        0.0,
        1.0,
        abs(material.tile_params.x) > 0.001 ||
            abs(shader_tile_alpha - 1.0) > 0.001 ||
            repeat_delta > 0.001,
    );
    let tile_uv = resolve_uv(input, material.uv_sources2.x) * tile_repeat;
    let tile_phase = dot(tile_uv, vec2<f32>(1.0, 0.618)) + material.tile_params.x * 0.137;
    let tile_pattern = 0.5 + 0.5 * sin(tile_phase * 6.2831853);
    let shader_alpha_scale = mix(0.97, 1.03, shader_tile_alpha);
    let shader_pattern_scale = mix(0.98, 1.02, tile_pattern);
    return ramp_scale * mix(1.0, shader_alpha_scale * shader_pattern_scale, shader_tile_enabled);
}

fn resolve_alpha_shaping(raw_alpha: f32) -> f32 {
    let aperture = clamp(material.alpha_params.x, 0.001, 8.0);
    let offset = clamp(material.alpha_params.y, -1.0, 1.0);
    let shaping_enabled = select(
        0.0,
        1.0,
        abs(aperture - 2.0) > 0.001 || abs(offset) > 0.001,
    );
    let adjusted = clamp(raw_alpha + offset, 0.0, 1.0);
    let exponent = clamp(2.0 / aperture, 0.25, 4.0);
    let shaped = pow(adjusted, exponent);
    return mix(raw_alpha, shaped, shaping_enabled);
}

fn resolve_glass_factors() -> vec3<f32> {
    let ior_delta = clamp((clamp(material.glass_params.x, 1.0, 2.5) - 1.0) / 1.5, 0.0, 1.0);
    let thickness_delta = clamp((max(material.glass_params.y, 0.0) - 0.01) * 8.0, 0.0, 1.0);
    return vec3<f32>(ior_delta, thickness_delta, max(ior_delta, thickness_delta * 0.35));
}

fn resolve_outline_rim(color: vec3<f32>, rim: f32, is_glass: bool) -> vec3<f32> {
    let width = clamp(material.outline_params.a, 0.0, 1.0);
    let outline_enabled = select(0.0, 1.0, width > 0.001);
    let rim_limit = select(0.16, 0.58, is_glass);
    let silhouette = smoothstep(0.35, 1.0, clamp(rim / rim_limit, 0.0, 1.0));
    let outline_color = clamp(material.outline_params.rgb, vec3<f32>(0.0), vec3<f32>(4.0));
    let glass_scale = select(1.0, 0.6, is_glass);
    let strength = outline_enabled * silhouette * clamp(width * 1.5, 0.0, 0.35) * glass_scale;
    return mix(color, outline_color, strength);
}

fn resolve_emissive(emissive_tex: vec3<f32>, vertex_alpha: f32, mask: vec3<f32>) -> vec3<f32> {
    let material_emissive = clamp(material.emissive_color.rgb, vec3<f32>(0.0), vec3<f32>(4.0));
    let shader_emissive = clamp(
        material.shader_emissive_color.rgb * material.shader_emissive_color.a,
        vec3<f32>(0.0),
        vec3<f32>(8.0),
    );
    let shader_multi_emissive = clamp(
        material.shader_multi_emissive_color.rgb * material.shader_multi_emissive_color.a,
        vec3<f32>(0.0),
        vec3<f32>(8.0),
    );
    let texture_strength = material.emissive_color.a;
    let texture_luma = dot(emissive_tex, vec3<f32>(0.2126, 0.7152, 0.0722));
    let texture_gate = smoothstep(0.02, 0.28, texture_luma) * texture_strength;
    let mask_gate = smoothstep(0.88, 1.0, mask.b) * material.params.w * 0.18;
    let vertex_gate = 0.35 + clamp(vertex_alpha, 0.0, 1.0) * 0.65;
    return emissive_tex * texture_strength
        + material_emissive * (texture_gate + mask_gate) * vertex_gate
        + shader_emissive * vertex_gate
        + shader_multi_emissive * mask_gate * vertex_gate;
}

fn resolve_shader_diffuse_tint(mask: vec3<f32>) -> vec3<f32> {
    let diffuse_tint = clamp(material.shader_diffuse_color.rgb, vec3<f32>(0.0), vec3<f32>(4.0));
    let multi_tint = clamp(material.shader_multi_diffuse_color.rgb, vec3<f32>(0.0), vec3<f32>(4.0));
    let multi_gate = smoothstep(0.22, 1.0, mask.r) * material.params.w * 0.35;
    return diffuse_tint * mix(vec3<f32>(1.0), multi_tint, multi_gate);
}

fn resolve_detail_tint(input: VertexOutput) -> vec3<f32> {
    let detail = resolve_single_detail_tint(
        material.detail_params.x,
        material.detail_color,
        material.detail_color_uv_scale.xy,
        input.uv0,
        1.0,
    );
    let multi_detail = resolve_single_detail_tint(
        material.detail_params.y,
        material.multi_detail_color,
        material.detail_color_uv_scale.zw,
        input.uv0,
        0.65,
    );
    return detail * multi_detail;
}

fn resolve_single_detail_tint(
    detail_id: f32,
    detail_color: vec4<f32>,
    uv_scale: vec2<f32>,
    uv: vec2<f32>,
    strength_scale: f32,
) -> vec3<f32> {
    let tint = clamp(detail_color.rgb * 2.0, vec3<f32>(0.25), vec3<f32>(1.75));
    let color_delta = length(detail_color.rgb - vec3<f32>(0.5));
    let detail_enabled = select(0.0, 1.0, abs(detail_id) > 0.001 || color_delta > 0.001);
    let repeat = max(abs(uv_scale), vec2<f32>(0.001));
    let scaled_uv = uv * repeat;
    let wave = sin((scaled_uv.x + scaled_uv.y + detail_id * 0.073) * 6.2831853);
    let pattern = 0.5 + 0.5 * wave;
    let strength = detail_enabled * clamp(detail_color.a, 0.0, 1.0) * (0.08 + pattern * 0.12) * strength_scale;
    return mix(vec3<f32>(1.0), tint, strength);
}

fn resolve_lightshaft_uv(input: VertexOutput) -> vec2<f32> {
    let animated = input.uv0 + material.lightshaft_tex_anim.xy * camera.options.z;
    let basis = vec3<f32>(animated, 1.0);
    return vec2<f32>(
        dot(basis, material.lightshaft_tex_u.xyz),
        dot(basis, material.lightshaft_tex_v.xyz),
    );
}

fn resolve_lightshaft_color(base: vec3<f32>, texture_alpha: f32, vertex_alpha: f32) -> vec4<f32> {
    let tint = clamp(material.lightshaft_color, vec4<f32>(0.0), vec4<f32>(8.0));
    let ray_strength = max(
        max(material.lightshaft_ray.x, material.lightshaft_ray.y),
        max(material.lightshaft_ray.z, material.lightshaft_ray.w),
    );
    let intensity = max(1.0, clamp(ray_strength, 0.0, 8.0));
    let alpha = clamp(texture_alpha * vertex_alpha * tint.a, 0.0, 1.0);
    return vec4<f32>(base * tint.rgb * intensity * alpha, alpha);
}

fn resolve_normal(input: VertexOutput, front_facing: bool, uv: vec2<f32>) -> vec3<f32> {
    let face_sign = select(-1.0, 1.0, front_facing);
    let geometric_normal = normalize(input.normal) * face_sign;
    let sampled = textureSample(normal_texture, data_sampler, uv).xyz * 2.0 - vec3<f32>(1.0);
    if camera.options.x <= 0.5 || material.params.z <= 0.5 || dot(input.bitangent.xyz, input.bitangent.xyz) <= 0.0001 {
        return geometric_normal;
    }

    let bitangent = normalize(input.bitangent.xyz);
    let tangent_sign = select(1.0, -1.0, input.bitangent.w < 0.0);
    let tangent = normalize(cross(bitangent, geometric_normal)) * tangent_sign;
    let normal_scale = resolve_effective_normal_scale();
    let mapped = normalize(vec3<f32>(
        sampled.x * normal_scale,
        sampled.y * camera.options.y * normal_scale,
        sampled.z,
    ));
    return normalize(tangent * mapped.x + bitangent * mapped.y + geometric_normal * mapped.z);
}

fn resolve_effective_normal_scale() -> f32 {
    let primary = clamp(material.shader_params.x, 0.0, 4.0);
    let multi_delta = clamp(material.shader_params.y, 0.0, 4.0) - 1.0;
    let detail_delta = clamp(material.shader_params.z, 0.0, 4.0) - 1.0;
    let multi_detail_delta = clamp(material.shader_params.w, 0.0, 4.0) - 1.0;
    let fallback_delta = multi_delta * 0.08 + detail_delta * 0.12 + multi_detail_delta * 0.08;
    return clamp(primary + fallback_delta, 0.0, 4.0);
}

fn resolve_uv(input: VertexOutput, source: f32) -> vec2<f32> {
    if source > 2.5 {
        return input.uv3;
    }
    if source > 1.5 {
        return input.uv2;
    }
    if source > 0.5 {
        return input.uv1 + material.uv_scroll.zw * camera.options.z;
    }
    return input.uv0 + material.uv_scroll.xy * camera.options.z;
}
