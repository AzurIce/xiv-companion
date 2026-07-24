struct Camera {
    view_proj: mat4x4<f32>,
    light_dir: vec4<f32>,
    view_dir: vec4<f32>,
    camera_position: vec4<f32>,
    options: vec4<f32>, // x: normal mapping, y: normal y sign, z: uv scroll time, w: debug mode
    dynamic_emissive_color: vec4<f32>, // runtime material dynamic emissive multiplier
};

// Stable preview-lighting contract. These values describe presentation only;
// they are not FFXIV material inputs and must not be tuned to hide texture,
// normal, ColorTable, or shader-family semantic errors. The key direction is
// camera-relative and is defined by the matching PREVIEW_KEY_* coefficients in
// model.rs. Scene values stay linear until the postprocess tone-map pass.
const PREVIEW_KEY_COLOR: vec3<f32> = vec3<f32>(1.0, 0.95, 0.88);
const PREVIEW_DIRECT_DIFFUSE_SCALE: f32 = 2.20;
const PREVIEW_DIRECT_SPECULAR_SCALE: f32 = 1.45;
const PREVIEW_AMBIENT_GROUND: vec3<f32> = vec3<f32>(0.12, 0.10, 0.085);
const PREVIEW_AMBIENT_SKY: vec3<f32> = vec3<f32>(0.30, 0.35, 0.42);
const PREVIEW_AMBIENT_VIEW_FILL: vec3<f32> = vec3<f32>(0.24, 0.215, 0.19);
const PREVIEW_AMBIENT_SCALE: f32 = 0.42;
const PREVIEW_ENV_GROUND: vec3<f32> = vec3<f32>(0.075, 0.065, 0.055);
const PREVIEW_ENV_SKY: vec3<f32> = vec3<f32>(0.34, 0.40, 0.48);
const PREVIEW_ENV_BASE_SCALE: f32 = 0.60;
const PREVIEW_ENV_HORIZON_COLOR: vec3<f32> = vec3<f32>(0.34, 0.27, 0.20);
const PREVIEW_ENV_HORIZON_SCALE: f32 = 0.55;
const PREVIEW_ENV_KEY_COLOR: vec3<f32> = vec3<f32>(1.0, 0.82, 0.64);
const PREVIEW_ENV_KEY_SCALE: f32 = 1.25;
const PREVIEW_ENV_FILL_COLOR: vec3<f32> = vec3<f32>(0.48, 0.62, 0.86);
const PREVIEW_ENV_FILL_SCALE: f32 = 0.55;
const PREVIEW_RIM_SCALE: f32 = 0.08;

struct Material {
    diffuse_color: vec4<f32>,
    emissive_color: vec4<f32>, // a: has emissive texture
    specular_color: vec4<f32>,
    params: vec4<f32>, // x: has base, y: metalness, z: has normal, w: has mask
    properties: vec4<f32>, // x: has ColorTable material properties texture, y: has specular texture, z: apply vertex color, w: legacy Compatibility specular mode
    render: vec4<f32>, // x: render mode, y: opacity, z: alpha mode 0=opaque 1=mask 2=blend 3=glass, w: alpha threshold
    alpha_params: vec4<f32>, // x: aperture, y: offset, z: shadow alpha threshold, w: transparency
    alpha_policy_params: vec4<f32>, // x: source 0=opaque 1=base alpha 2=normal blue 3=material transparency 4=normal alpha, y: lighting, z: dither depth, w: prepared pass
    alpha_composition_params: vec4<f32>, // x: 0xAD94E254, vertex alpha remap toward one
    water_deep_color: vec4<f32>,
    water_refraction_color: vec4<f32>,
    water_whitecap_color: vec4<f32>,
    extra_properties: vec4<f32>, // x: tile, y: sheen, z: sphere, w: tile matrix
    shader_params: vec4<f32>, // x: normal, y: multi normal, z: detail normal, w: multi detail normal
    tile_params: vec4<f32>, // x: tile index, y: tile alpha, zw: tile repeat uv
    toon_sheen_params: vec4<f32>, // x: toon index, y: toon light scale, z: sheen rate, w: sheen tint rate
    toon_params: vec4<f32>, // x: light spec aperture, y: reflection scale, z: spec index, w: prepared toon
    sheen_sphere_params: vec4<f32>, // x: sheen aperture, y: sphere map index, z: baked specular alpha is anisotropy
    detail_params: vec4<f32>, // x: detail id, y: multi detail id, z: GetMultiValues detail blend
    array_params: vec4<f32>, // x: tile layers, y: detail layers, z: has tile pair, w: has detail pair
    tile_lod_params: vec4<f32>, // x: tile bias, y: packed generic ramps, z: modern blend shaping, w: packed tile ramps
    detail_color: vec4<f32>,
    multi_detail_color: vec4<f32>,
    shader_diffuse_color: vec4<f32>,
    shader_multi_diffuse_color: vec4<f32>,
    shader_emissive_color: vec4<f32>,
    shader_multi_emissive_color: vec4<f32>,
    outline_params: vec4<f32>, // rgb: outline color, a: outline width
    specular_color_mask: vec4<f32>,
    surface_params: vec4<f32>, // x: ssao mask, y: texture mip bias, z: shadow pos offset, w: character mip bias
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
    uv_scroll_masks0: vec4<f32>, // base, normal, mask, material map
    uv_scroll_masks1: vec4<f32>, // multi, specular, emissive, material properties
    uv_scroll_masks2: vec4<f32>, // tile, sheen, sphere, tile matrix
    uv_scroll_masks3: vec4<f32>, // ColorTable index, other
    feature_params: vec4<f32>, // x: use flow0 tangent, y: water, z: secondary bindings, w: bg specular channels
    family_params: vec4<f32>, // x: exact characterlegacy, y: exact character/legacy Final
    secondary_map_params: vec4<f32>, // xyz: secondary color/normal/specular present, w: GetMultiValues blend
    draw_role_params: vec4<f32>, // x: lightshaft, y: transparent crest fallback, z: base material fallback
    debug_color: vec4<f32>, // xyz: mesh/draw-role debug color
    unsupported_color: vec4<f32>, // rgb: unsupported-input diagnostic color
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
    @location(7) flow0: vec4<f32>,
    @location(8) flow1: vec4<f32>,
    @location(9) normal1: vec3<f32>,
    @location(10) bitangent1: vec4<f32>,
    @location(11) color1: vec4<f32>,
    @location(12) world_position: vec3<f32>,
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
var normal_sampler: sampler;

@group(1) @binding(9)
var tile_properties_texture: texture_2d<f32>;

@group(1) @binding(10)
var sheen_properties_texture: texture_2d<f32>;

@group(1) @binding(11)
var sphere_properties_texture: texture_2d<f32>;

@group(1) @binding(12)
var tile_matrix_texture: texture_2d<f32>;

@group(1) @binding(13)
var tile_matrix_sampler: sampler;

@group(1) @binding(14)
var color_table_index_texture: texture_2d<f32>;

@group(1) @binding(15)
var material_map_texture: texture_2d<f32>;

@group(1) @binding(16)
var multi_map_texture: texture_2d<f32>;

@group(1) @binding(17)
var tile_array_pair_texture: texture_2d<f32>;

@group(1) @binding(18)
var detail_array_pair_texture: texture_2d<f32>;

@group(1) @binding(19)
var mask_sampler: sampler;

@group(1) @binding(20)
var emissive_sampler: sampler;

@group(1) @binding(21)
var material_properties_sampler: sampler;

@group(1) @binding(22)
var specular_sampler: sampler;

@group(1) @binding(23)
var tile_sampler: sampler;

@group(1) @binding(24)
var sheen_sampler: sampler;

@group(1) @binding(25)
var sphere_sampler: sampler;

@group(1) @binding(26)
var index_sampler: sampler;

@group(1) @binding(27)
var material_map_sampler: sampler;

@group(1) @binding(28)
var multi_map_sampler: sampler;

@group(1) @binding(29)
var tile_array_sampler: sampler;

@group(1) @binding(30)
var detail_array_sampler: sampler;

struct FragmentOutput {
    @location(0) color: vec4<f32>,
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
    out.flow0 = input.flow0;
    out.flow1 = input.flow1;
    out.normal1 = normalize(input.normal1);
    out.bitangent1 = input.bitangent1;
    out.color1 = input.color1;
    out.world_position = input.position;
    return out;
}

@vertex
fn vs_outline(input: VertexInput) -> VertexOutput {
    let width = clamp(material.outline_params.a, 0.0, 0.1);
    var out: VertexOutput;
    out.clip_position = camera.view_proj * vec4<f32>(input.position + normalize(input.normal) * width, 1.0);
    out.normal = normalize(input.normal);
    out.uv0 = input.uv0;
    out.bitangent = input.bitangent;
    out.color = input.color;
    out.uv1 = input.uv1;
    out.uv2 = input.uv2;
    out.uv3 = input.uv3;
    out.flow0 = input.flow0;
    out.flow1 = input.flow1;
    out.normal1 = normalize(input.normal1);
    out.bitangent1 = input.bitangent1;
    out.color1 = input.color1;
    out.world_position = input.position + normalize(input.normal) * width;
    return out;
}

@fragment
fn fs_outline() -> FragmentOutput {
    var out: FragmentOutput;
    out.color = vec4<f32>(clamp(material.outline_params.rgb, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
    return out;
}

@fragment
fn fs_dither_depth(input: VertexOutput) -> FragmentOutput {
    let base_uv = resolve_uv(input, material.uv_sources0.x, material.uv_scroll_masks0.x);
    let secondary_base_uv = resolve_uv(input, material.uv_sources2.x, material.uv_scroll_masks2.x);
    let normal_uv = resolve_uv(input, material.uv_sources0.y, material.uv_scroll_masks0.y);
    let mip_bias = resolve_texture_mip_bias();
    let sampled_normal = textureSampleBias(normal_texture, normal_sampler, normal_uv, mip_bias);
    let color_table_blend = resolve_color_table_blend(input, sampled_normal).weight;
    let sampled_base = sample_color_table_base(base_uv, mip_bias, color_table_blend);
    let sampled_secondary_base = textureSample(
        tile_properties_texture,
        tile_sampler,
        secondary_base_uv,
    );
    let primary_alpha = select(1.0, sampled_base.a, material.params.x > 0.5);
    let secondary_weight = clamp(input.color.a, 0.0, 1.0)
        * material.secondary_map_params.x
        * material.secondary_map_params.w;
    let base_texture_alpha = mix(primary_alpha, sampled_secondary_base.a, secondary_weight);
    let opacity_vertex_alpha = select(
        input.color.a,
        1.0,
        material.secondary_map_params.w > 0.5,
    );
    let alpha = resolve_material_alpha(
        opacity_vertex_alpha,
        base_texture_alpha,
        sampled_normal.b,
        sampled_normal.a,
        false,
        false,
    );
    if material.alpha_policy_params.z < 0.5 || alpha <= ordered_dither_threshold(input.clip_position.xy) {
        discard;
    }

    var out: FragmentOutput;
    out.color = vec4<f32>(0.0);
    return out;
}

@fragment
fn fs_main(input: VertexOutput) -> FragmentOutput {
    let pass_flags = resolve_surface_pass_flags();
    let blend_normal_uv = resolve_uv(input, material.uv_sources0.y, material.uv_scroll_masks0.y);
    let blend_normal = textureSampleBias(
        normal_texture,
        normal_sampler,
        blend_normal_uv,
        resolve_texture_mip_bias(),
    );
    let color_table_blend = resolve_color_table_blend(input, blend_normal).weight;
    let extra = resolve_extra_properties(input, color_table_blend);
    let tile_array = resolve_tile_array(input, extra);
    let detail_array = resolve_detail_array(input);
    let samples = resolve_surface_samples(input, color_table_blend);
    let surface = resolve_surface_state(
        input,
        pass_flags,
        samples,
        tile_array,
    );
    if camera.options.w > 0.5 {
        return debug_fragment_output(
            input,
            camera.options.w,
            surface.base,
            surface.normal,
            surface.mask,
            surface.properties,
            surface.material_specular,
            surface.emissive,
            surface.alpha,
            tile_array,
            detail_array,
            extra,
        );
    }
    if pass_flags.is_crest_fallback {
        discard;
    }
    if pass_flags.is_mask && surface.alpha < material.render.w {
        discard;
    }
    if pass_flags.uses_alpha && surface.alpha < 0.01 {
        discard;
    }
    if pass_flags.is_lightshaft {
        var out: FragmentOutput;
        out.color = surface.lightshaft;
        return out;
    }
    return resolve_surface_output(surface, input);
}

struct SurfacePassFlags {
    is_lightshaft: bool,
    is_crest_fallback: bool,
    is_mask: bool,
    is_glass: bool,
    uses_alpha: bool,
};

fn resolve_surface_pass_flags() -> SurfacePassFlags {
    var out: SurfacePassFlags;
    out.is_lightshaft = material.draw_role_params.x > 0.5;
    out.is_crest_fallback = material.draw_role_params.y > 0.5;
    out.is_mask = material.render.z > 0.5 && material.render.z < 1.5;
    let is_blend = material.alpha_policy_params.w > 0.5 && material.alpha_policy_params.w < 1.5;
    out.is_glass = material.alpha_policy_params.w > 1.5;
    out.uses_alpha = out.is_mask
        || is_blend
        || out.is_glass
        || out.is_lightshaft
        || out.is_crest_fallback
        || material.render.x > 0.5;
    return out;
}

struct SurfaceSamples {
    base: vec4<f32>,
    secondary_base: vec4<f32>,
    normal: vec4<f32>,
    secondary_normal: vec4<f32>,
    specular: vec4<f32>,
    secondary_specular: vec3<f32>,
    emissive: vec3<f32>,
    mask: vec3<f32>,
    properties: vec4<f32>,
    secondary_blend: f32,
};

struct ColorTableBlend {
    weight: f32,
}

fn resolve_color_table_blend(
    input: VertexOutput,
    normal_sample: vec4<f32>,
) -> ColorTableBlend {
    var out: ColorTableBlend;
    out.weight = 0.0;
    if material.tile_lod_params.y <= 0.5 && material.tile_lod_params.w <= 0.5 {
        return out;
    }

    var source_blend = 0.0;
    if material.tile_lod_params.w > 0.5 {
        let tile_uv = resolve_uv(input, material.uv_sources2.x, material.uv_scroll_masks2.x);
        source_blend = load_packed_tile_properties(tile_uv, 0u).z;
    } else {
        let index_uv = resolve_uv(input, material.uv_sources3.x, material.uv_scroll_masks3.x);
        source_blend = textureSampleLevel(color_table_index_texture, index_sampler, index_uv, 0.0).g;
    }
    let base_weight = 1.0 - clamp(source_blend, 0.0, 1.0);
    out.weight = base_weight;
    if material.tile_lod_params.z <= 0.5 {
        return out;
    }

    let specular_uv = resolve_uv(input, material.uv_sources1.y, material.uv_scroll_masks1.y);
    // The installed shaping sample reads texel column 4, component W. Meddle's
    // ColorTable layout identifies that channel as A-row anisotropy.
    let shaping_anisotropy = max(load_packed_specular(specular_uv, 0u).a, 0.0);
    if shaping_anisotropy <= 0.0001 {
        return out;
    }

    let primary_world = resolve_blend_primary_world_normal(input, normal_sample);
    let view = resolve_view_direction(input.world_position);
    let view_term = clamp(1.0 - abs(dot(primary_world, view)), 0.0, 1.0);
    out.weight = clamp(
        (1.0 - pow(view_term, 1.0 / shaping_anisotropy)) * base_weight,
        0.0,
        1.0,
    );
    return out;
}

fn resolve_blend_primary_world_normal(
    input: VertexOutput,
    normal_sample: vec4<f32>,
) -> vec3<f32> {
    let view = resolve_view_direction(input.world_position);
    let geometric = orient_geometric_normal_toward_viewer(input.normal, view);
    if material.params.z <= 0.5 || dot(input.bitangent.xyz, input.bitangent.xyz) <= 0.0001 {
        return geometric;
    }
    let bitangent = normalize(input.bitangent.xyz);
    let tangent_sign = select(1.0, -1.0, input.bitangent.w < 0.0);
    let tangent = normalize(cross(bitangent, geometric)) * tangent_sign;
    let sampled = decode_normal(normal_sample);
    let scale = clamp(material.shader_params.x, 0.0, 4.0);
    let mapped = normalize(vec3<f32>(sampled.xy * scale, sampled.z));
    return normalize(tangent * mapped.x + bitangent * mapped.y + geometric * mapped.z);
}

fn resolve_surface_samples(input: VertexOutput, color_table_weight: f32) -> SurfaceSamples {
    let base_uv = resolve_uv(input, material.uv_sources0.x, material.uv_scroll_masks0.x);
    let normal_uv = resolve_uv(input, material.uv_sources0.y, material.uv_scroll_masks0.y);
    let secondary_base_uv = resolve_uv(input, material.uv_sources2.x, material.uv_scroll_masks2.x);
    let secondary_normal_uv = resolve_uv(input, material.uv_sources2.y, material.uv_scroll_masks2.y);
    let secondary_specular_uv = resolve_uv(input, material.uv_sources2.z, material.uv_scroll_masks2.z);
    let mask_uv = resolve_uv(input, material.uv_sources0.z, material.uv_scroll_masks0.z);
    let specular_uv = resolve_uv(input, material.uv_sources1.y, material.uv_scroll_masks1.y);
    let emissive_uv = resolve_uv(input, material.uv_sources1.z, material.uv_scroll_masks1.z);
    let material_properties_uv = resolve_uv(input, material.uv_sources1.w, material.uv_scroll_masks1.w);
    let mip_bias = resolve_texture_mip_bias();

    var out: SurfaceSamples;
    out.normal = textureSampleBias(normal_texture, normal_sampler, normal_uv, mip_bias);
    out.secondary_normal = textureSample(
        sheen_properties_texture,
        sheen_sampler,
        secondary_normal_uv,
    );
    out.secondary_blend = clamp(input.color.a, 0.0, 1.0) * material.secondary_map_params.w;
    out.specular = sample_color_table_specular(specular_uv, color_table_weight);
    out.secondary_specular = textureSample(
        sphere_properties_texture,
        sphere_sampler,
        secondary_specular_uv,
    ).rgb;
    let effective_specular_sample = mix(
        out.specular.rgb,
        out.secondary_specular,
        out.secondary_blend * material.secondary_map_params.z,
    );
    out.mask = resolve_mask(mask_uv);
    out.properties = resolve_material_properties(material_properties_uv, out.mask, color_table_weight);
    let has_bg_specular = material.properties.y > 0.5 || material.secondary_map_params.z > 0.5;
    if material.feature_params.w > 0.5 && has_bg_specular {
        out.properties.x = effective_specular_sample.b;
        out.properties.y = effective_specular_sample.g;
    }
    out.base = sample_color_table_base(base_uv, mip_bias, color_table_weight);
    out.secondary_base = textureSample(
        tile_properties_texture,
        tile_sampler,
        secondary_base_uv,
    );
    out.emissive = sample_color_table_emissive(emissive_uv, color_table_weight);
    return out;
}

struct SurfaceState {
    base: vec3<f32>,
    normal: vec3<f32>,
    mask: vec3<f32>,
    properties: vec4<f32>,
    material_specular: vec3<f32>,
    anisotropy: f32,
    emissive: vec3<f32>,
    color_table_emissive: vec3<f32>,
    lightshaft: vec4<f32>,
    alpha: f32,
};

fn resolve_surface_state(
    input: VertexOutput,
    pass_flags: SurfacePassFlags,
    samples: SurfaceSamples,
    tile_array: TileArraySample,
) -> SurfaceState {
    var out: SurfaceState;
    out.normal = resolve_normal(
        input,
        samples.normal,
        samples.secondary_normal,
        samples.secondary_blend,
        tile_array,
    );
    out.mask = samples.mask;
    out.properties = samples.properties;
    let primary_texture = select(vec3<f32>(1.0), samples.base.rgb, material.params.x > 0.5);
    let secondary_color_weight = samples.secondary_blend * material.secondary_map_params.x;
    let scroll_texture_mix = mix(
        primary_texture * material.shader_diffuse_color.rgb,
        samples.secondary_base.rgb * material.shader_multi_diffuse_color.rgb,
        secondary_color_weight,
    );
    let texture_mix = select(
        primary_texture,
        scroll_texture_mix,
        material.secondary_map_params.w > 0.5,
    );
    let primary_alpha = select(1.0, samples.base.a, material.params.x > 0.5);
    let base_texture_alpha = mix(primary_alpha, samples.secondary_base.a, secondary_color_weight);
    out.lightshaft = resolve_lightshaft_color(
        samples.base.rgb,
        samples.secondary_base.rgb,
        input.color,
    );

    let primary_specular = select(
        material.specular_color.rgb,
        samples.specular.rgb,
        material.properties.y > 0.5,
    );
    let generic_material_specular = mix(
        primary_specular,
        samples.secondary_specular,
        samples.secondary_blend * material.secondary_map_params.z,
    );
    out.material_specular = select(
        generic_material_specular,
        material.specular_color.rgb,
        material.feature_params.w > 0.5,
    );
    out.anisotropy = select(
        0.0,
        clamp(samples.specular.a, 0.0, 1.0),
        material.sheen_sphere_params.z > 0.5,
    );

    let shader_tint = resolve_shader_diffuse_tint();
    let generic_base = material.diffuse_color.rgb
        * texture_mix
        * shader_tint
        * tile_array.color_multiplier;
    let scroll_base = texture_mix;
    let family_base = select(
        generic_base,
        scroll_base,
        material.secondary_map_params.w > 0.5,
    );
    out.base = select(
        family_base,
        material.water_deep_color.rgb,
        material.feature_params.y > 0.5,
    );

    let opacity_vertex_alpha = select(
        input.color.a,
        1.0,
        material.secondary_map_params.w > 0.5,
    );
    let surface_alpha = resolve_material_alpha(
        opacity_vertex_alpha,
        base_texture_alpha,
        samples.normal.b,
        samples.normal.a,
        pass_flags.is_lightshaft,
        pass_flags.is_crest_fallback,
    );
    out.alpha = select(surface_alpha, out.lightshaft.a, pass_flags.is_lightshaft);
    out.emissive = resolve_emissive(samples.emissive);
    out.color_table_emissive =
        samples.emissive * material.emissive_color.a * camera.dynamic_emissive_color.rgb;
    return out;
}

fn fresnel_schlick(cos_theta: f32, f0: vec3<f32>) -> vec3<f32> {
    let weight = pow(1.0 - clamp(cos_theta, 0.0, 1.0), 5.0);
    return f0 + (vec3<f32>(1.0) - f0) * weight;
}

fn legacy_camera_reflection_lobe(
    normal: vec3<f32>,
    world_position: vec3<f32>,
    gloss_strength: f32,
) -> f32 {
    // Installed characterlegacy DXBC proves this complete direct-light shape:
    // V = normalize(-camera-relative position), L = normalize(-position +
    // (0, 0.2, 0)), R = reflect(-V, N), then
    // min(3 - 3 * (1 - saturate(N.L))^2, 1) * saturate(R.L)^Gloss.
    let view = resolve_view_direction(world_position);
    let to_camera_light = camera.camera_position.xyz - world_position
        + vec3<f32>(0.0, 0.2, 0.0);
    let has_light_direction = dot(to_camera_light, to_camera_light) > 0.000001;
    let safe_light_direction = to_camera_light
        + select(vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(0.0), has_light_direction);
    let light = normalize(safe_light_direction);
    let normal_light = clamp(dot(normal, light), 0.0, 1.0);
    let one_minus_light = 1.0 - normal_light;
    let visibility = min(3.0 - 3.0 * one_minus_light * one_minus_light, 1.0);
    let reflected_view = reflect(-view, normal);
    let reflection_light = clamp(dot(reflected_view, light), 0.0, 1.0);
    return visibility * pow(reflection_light, max(gloss_strength, 0.0));
}

fn ggx_distribution(normal_half: f32, roughness: f32) -> f32 {
    let alpha = roughness * roughness;
    let alpha_squared = alpha * alpha;
    let denominator = normal_half * normal_half * (alpha_squared - 1.0) + 1.0;
    return alpha_squared / max(3.14159265 * denominator * denominator, 0.0001);
}

struct AnisotropyFrame {
    tangent: vec3<f32>,
    bitangent: vec3<f32>,
    available: bool,
};

fn resolve_anisotropy_frame(input: VertexOutput, normal: vec3<f32>) -> AnisotropyFrame {
    let source_bitangent = input.bitangent.xyz;
    let tangent_candidate = cross(source_bitangent, normal);
    let has_frame = dot(tangent_candidate, tangent_candidate) > 0.0001;
    let fallback_axis = select(
        vec3<f32>(0.0, 1.0, 0.0),
        vec3<f32>(1.0, 0.0, 0.0),
        abs(normal.x) < 0.9,
    );
    let fallback_tangent = normalize(cross(fallback_axis, normal));
    let tangent = normalize(select(fallback_tangent, tangent_candidate, has_frame));
    let bitangent = normalize(cross(normal, tangent));

    var out: AnisotropyFrame;
    out.tangent = tangent;
    out.bitangent = bitangent;
    out.available = has_frame;
    return out;
}

fn ggx_anisotropic_distribution(
    normal: vec3<f32>,
    half_dir: vec3<f32>,
    roughness: f32,
    anisotropy: f32,
    frame: AnisotropyFrame,
) -> f32 {
    let strength = clamp(anisotropy, 0.0, 1.0);
    if strength < 0.0001 || !frame.available {
        return ggx_distribution(max(dot(normal, half_dir), 0.0), roughness);
    }

    // MeddleTools proves that the ColorTable specular-ramp alpha feeds the
    // Principled anisotropy input. The preview keeps its existing GGX model and
    // uses the standard aspect-preserving anisotropic GGX NDF; the exact FFXIV
    // lobe and tangent rotation remain unknown.
    let alpha = max(roughness * roughness, 0.001);
    let aspect = sqrt(max(1.0 - 0.9 * strength, 0.1));
    let alpha_tangent = max(alpha / aspect, 0.001);
    let alpha_bitangent = max(alpha * aspect, 0.001);
    let tangent_half = dot(frame.tangent, half_dir) / alpha_tangent;
    let bitangent_half = dot(frame.bitangent, half_dir) / alpha_bitangent;
    let normal_half = max(dot(normal, half_dir), 0.0);
    let denominator = tangent_half * tangent_half
        + bitangent_half * bitangent_half
        + normal_half * normal_half;
    return 1.0 / max(
        3.14159265 * alpha_tangent * alpha_bitangent * denominator * denominator,
        0.0001,
    );
}

fn ggx_geometry_schlick(normal_dot: f32, roughness: f32) -> f32 {
    let radius = roughness + 1.0;
    let k = radius * radius * 0.125;
    return normal_dot / max(normal_dot * (1.0 - k) + k, 0.0001);
}

fn studio_environment(direction: vec3<f32>, roughness: f32) -> vec3<f32> {
    let ray = normalize(direction);
    let vertical = clamp(ray.y * 0.5 + 0.5, 0.0, 1.0);
    var color = mix(PREVIEW_ENV_GROUND, PREVIEW_ENV_SKY, vertical)
        * PREVIEW_ENV_BASE_SCALE;

    let horizon_width = mix(5.0, 1.5, roughness);
    let horizon = pow(max(1.0 - abs(ray.y), 0.0), horizon_width);
    color += PREVIEW_ENV_HORIZON_COLOR * horizon * PREVIEW_ENV_HORIZON_SCALE;

    let key_direction = normalize(vec3<f32>(-0.55, 0.45, 0.70));
    let fill_direction = normalize(vec3<f32>(0.72, 0.18, 0.67));
    let key = pow(
        max(dot(ray, key_direction), 0.0),
        mix(44.0, 5.0, roughness),
    );
    let fill = pow(
        max(dot(ray, fill_direction), 0.0),
        mix(72.0, 7.0, roughness),
    );
    color += PREVIEW_ENV_KEY_COLOR * key * PREVIEW_ENV_KEY_SCALE;
    color += PREVIEW_ENV_FILL_COLOR * fill * PREVIEW_ENV_FILL_SCALE;
    return color;
}

fn hemisphere_irradiance(normal: vec3<f32>, view: vec3<f32>) -> vec3<f32> {
    let vertical = clamp(normal.y * 0.5 + 0.5, 0.0, 1.0);
    let view_fill = sqrt(max(dot(normal, view), 0.0));
    return mix(PREVIEW_AMBIENT_GROUND, PREVIEW_AMBIENT_SKY, vertical)
        + PREVIEW_AMBIENT_VIEW_FILL * view_fill;
}

fn resolve_surface_output(
    surface: SurfaceState,
    input: VertexOutput,
) -> FragmentOutput {
    let normal = normalize(surface.normal);
    let light = normalize(camera.light_dir.xyz);
    let view = resolve_view_direction(input.world_position);
    let half_dir = normalize(light + view);
    let normal_light = max(dot(normal, light), 0.0);
    let normal_view = max(dot(normal, view), 0.001);
    let view_half = max(dot(view, half_dir), 0.0);
    let metalness = clamp(surface.properties.x, 0.0, 1.0);
    let specular_strength = max(surface.properties.w, 0.0);
    // Modern Roughness is independent from GlossStrength. Installed Legacy
    // DXBC writes exp2(-GlossStrength / 15) into the same MRT component for
    // its material-parameter path. Keep the transform here, after the float
    // ramp has incorporated staining, and retain raw GlossStrength in Z for
    // debug and the still-unsupported pass-specific consumers.
    let legacy_gloss_roughness = exp2(-max(surface.properties.z, 0.0) / 15.0);
    let roughness_source = select(
        surface.properties.y,
        legacy_gloss_roughness,
        material.properties.x > 0.5 && material.family_params.x > 0.5,
    );
    let roughness = clamp(roughness_source, 0.06, 1.0);
    let uses_legacy_colortable = material.properties.x > 0.5
        && material.family_params.x > 0.5;
    let legacy_specular_mode = material.properties.w;
    let uses_colortable_specular_mask = material.properties.x > 0.5
        && legacy_specular_mode < 0.5;
    let specular_weight = specular_strength * select(
        1.0,
        clamp(surface.mask.r, 0.0, 1.0),
        uses_colortable_specular_mask,
    );
    // Installed Legacy Final shaders do not use raw SpecularStrength as a
    // GGX/F0 scalar. They fold it into a wetness-shaped composite that
    // multiplies an environment/material/scene-light RGB branch before a
    // separate MAD. Until those runtime-owned inputs exist, keep the raw lane
    // for debug/diagnostics but do not feed the contradicted preview mapping.
    let preview_f0_specular_weight = select(
        specular_weight,
        1.0,
        uses_legacy_colortable,
    );
    let dielectric_f0 = clamp(
        surface.material_specular * (0.08 * preview_f0_specular_weight),
        vec3<f32>(0.0),
        vec3<f32>(1.0),
    );
    let f0 = mix(dielectric_f0, clamp(surface.base, vec3<f32>(0.0), vec3<f32>(1.0)), metalness);
    let fresnel = fresnel_schlick(view_half, f0);
    let anisotropy_frame = resolve_anisotropy_frame(input, normal);
    let distribution = ggx_anisotropic_distribution(
        normal,
        half_dir,
        roughness,
        surface.anisotropy,
        anisotropy_frame,
    );
    let geometry = ggx_geometry_schlick(normal_view, roughness)
        * ggx_geometry_schlick(normal_light, roughness);
    let specular_brdf = distribution * geometry * fresnel
        / max(4.0 * normal_view * normal_light, 0.001);
    let diffuse_weight = (vec3<f32>(1.0) - fresnel) * (1.0 - metalness);
    let specular_mask = clamp(resolve_specular_mask_factor(surface.mask.r), 0.0, 1.35);
    let direct_diffuse = diffuse_weight
        * surface.base
        * (normal_light * PREVIEW_DIRECT_DIFFUSE_SCALE / 3.14159265)
        * PREVIEW_KEY_COLOR;
    let ggx_direct_specular = specular_brdf
        * normal_light
        * PREVIEW_DIRECT_SPECULAR_SCALE
        * PREVIEW_KEY_COLOR
        * specular_mask;
    let legacy_direct_lobe = legacy_camera_reflection_lobe(
        normal,
        input.world_position,
        surface.properties.z,
    );
    let legacy_direct_specular = fresnel
        * legacy_direct_lobe
        * PREVIEW_DIRECT_SPECULAR_SCALE
        * PREVIEW_KEY_COLOR
        * specular_mask;
    let direct_specular = select(
        ggx_direct_specular,
        legacy_direct_specular,
        uses_legacy_colortable,
    );

    let ambient_diffuse = diffuse_weight
        * surface.base
        * hemisphere_irradiance(normal, view)
        * PREVIEW_AMBIENT_SCALE;
    let reflection = reflect(-view, normal);
    let environment_fresnel = fresnel_schlick(normal_view, f0);
    let environment_visibility = mix(0.82, 0.24, roughness);
    let environment_specular = studio_environment(reflection, roughness)
        * environment_fresnel
        * environment_visibility
        * specular_mask;
    let rim = pow(1.0 - normal_view, 2.0) * PREVIEW_RIM_SCALE;
    let rim_tint = mix(vec3<f32>(0.48, 0.58, 0.72), surface.base, metalness * 0.70);
    let opaque_lit = direct_diffuse
        + direct_specular
        + ambient_diffuse
        + environment_specular
        + rim_tint * rim;
    let lighting_enabled = material.alpha_policy_params.y > 0.5;
    let lit = select(surface.base, opaque_lit, lighting_enabled);
    // Installed Character/Legacy ColorTable Final shaders scale only the
    // ColorTable emissive term by the luminance of the already-lit RGB branch.
    // g_EmissiveColor and other shader emissive terms remain independent.
    let uses_character_colortable_emissive_scale = material.properties.x > 0.5
        && material.family_params.y > 0.5;
    let lit_luminance = dot(lit, vec3<f32>(0.298910, 0.586610, 0.114480));
    let color_table_emissive_scale = select(
        1.0,
        max(lit_luminance, 1.0),
        uses_character_colortable_emissive_scale,
    );
    let unscaled_emissive = surface.emissive - surface.color_table_emissive;
    let color = lit
        + unscaled_emissive
        + surface.color_table_emissive * color_table_emissive_scale;

    var out: FragmentOutput;
    out.color = vec4<f32>(color, surface.alpha);
    return out;
}

@fragment
fn fs_lightshaft(input: VertexOutput) -> FragmentOutput {
    let base_uv = resolve_uv(input, material.uv_sources0.x, material.uv_scroll_masks0.x);
    let secondary_base_uv = resolve_uv(input, material.uv_sources2.x, material.uv_scroll_masks2.x);
    let primary = textureSample(base_color_texture, base_color_sampler, base_uv).rgb;
    let secondary = textureSample(tile_properties_texture, tile_sampler, secondary_base_uv).rgb;
    let lightshaft = resolve_lightshaft_color(primary, secondary, input.color);
    let is_mask = material.render.z > 0.5 && material.render.z < 1.5;
    if lightshaft.a < 0.01 || (is_mask && lightshaft.a < material.render.w) {
        discard;
    }

    var out: FragmentOutput;
    out.color = lightshaft;
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
    tile_array: TileArraySample,
    detail_array: DetailArraySample,
    extra: ExtraProperties,
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
        let index_uv = resolve_uv(input, material.uv_sources3.x, material.uv_scroll_masks3.x);
        let index_sample = textureSample(color_table_index_texture, index_sampler, index_uv);
        color = vec3<f32>(index_sample.r, index_sample.g, 0.5);
    } else if mode < 15.5 {
        let material_map_uv = resolve_uv(input, material.uv_sources0.w, material.uv_scroll_masks0.w);
        color = textureSample(material_map_texture, material_map_sampler, material_map_uv).rgb;
    } else if mode < 16.5 {
        let multi_map_uv = resolve_uv(input, material.uv_sources1.x, material.uv_scroll_masks1.x);
        color = textureSample(multi_map_texture, multi_map_sampler, multi_map_uv).rgb;
    } else if mode < 17.5 {
        color = mix(extra.tile_a, extra.tile_b, extra.tile_blend).rgb;
    } else if mode < 18.5 {
        let sheen_uv = resolve_uv(input, material.uv_sources2.y, material.uv_scroll_masks2.y);
        color = textureSample(sheen_properties_texture, sheen_sampler, sheen_uv).rgb;
    } else if mode < 19.5 {
        let sphere_uv = resolve_uv(input, material.uv_sources2.z, material.uv_scroll_masks2.z);
        color = textureSample(sphere_properties_texture, sphere_sampler, sphere_uv).rgb;
    } else if mode < 20.5 {
        color = mix(extra.tile_matrix_a, extra.tile_matrix_b, extra.tile_blend).rgb;
    } else if mode < 21.5 {
        color = tile_array.normal * 0.5 + vec3<f32>(0.5);
    } else if mode < 22.5 {
        color = tile_array.orb;
    } else if mode < 23.5 {
        color = detail_array.diffuse;
    } else if mode < 24.5 {
        color = detail_array.normal * 0.5 + vec3<f32>(0.5);
    } else if mode < 25.5 {
        color = input.color1.rgb;
    } else if mode < 26.5 {
        color = normalize(input.normal1) * 0.5 + vec3<f32>(0.5);
    } else if mode < 27.5 {
        color = input.flow0.xyz * 0.5 + vec3<f32>(0.5);
    } else if mode < 28.5 {
        color = input.flow1.xyz * 0.5 + vec3<f32>(0.5);
    } else if mode < 29.5 {
        color = material.unsupported_color.rgb;
    } else {
        color = resolve_view_direction(input.world_position) * 0.5 + vec3<f32>(0.5);
    }

    var out: FragmentOutput;
    let preserves_hdr_color = mode < 1.5
        || (mode >= 3.5 && mode < 6.5)
        || (mode >= 17.5 && mode < 18.5);
    let debug_color = select(clamp(color, vec3<f32>(0.0), vec3<f32>(1.0)), max(color, vec3<f32>(0.0)), preserves_hdr_color);
    out.color = vec4<f32>(debug_color, 1.0);
    return out;
}

fn uv_debug_color(uv: vec2<f32>) -> vec3<f32> {
    return vec3<f32>(fract(uv.x), fract(uv.y), 0.5);
}

fn resolve_texture_mip_bias() -> f32 {
    return select(
        0.0,
        clamp(material.surface_params.y, -16.0, 15.99),
        material.surface_params.w > 0.5,
    );
}

fn resolve_mask(uv: vec2<f32>) -> vec3<f32> {
    if material.params.w <= 0.5 {
        return vec3<f32>(1.0, material.specular_color.a, material.params.y);
    }
    return textureSampleBias(mask_texture, mask_sampler, uv, resolve_texture_mip_bias()).rgb;
}

fn resolve_material_properties(uv: vec2<f32>, mask: vec3<f32>, color_table_weight: f32) -> vec4<f32> {
    if material.properties.x > 0.5 {
        if material.tile_lod_params.y > 0.5 {
            return mix(
                load_packed_material_properties(uv, 0u),
                load_packed_material_properties(uv, 1u),
                color_table_weight,
            );
        }
        return textureSample(material_properties_texture, material_properties_sampler, uv);
    }

    if material.properties.w > 0.5 {
        let metalness = clamp(material.params.y, 0.0, 1.0);
        let roughness = clamp(material.specular_color.a, 0.08, 1.0);
        let gloss_strength = clamp((1.0 - roughness) * 0.75 + 0.25, 0.0, 1.0);
        return vec4<f32>(metalness, roughness, gloss_strength, 1.0);
    }

    let metalness = clamp(max(material.params.y, mask.b * material.params.w), 0.0, 1.0);
    let roughness = clamp(mix(material.specular_color.a, mask.g, material.params.w), 0.08, 1.0);
    let specular_strength = mix(1.0, mask.r, material.params.w);
    let gloss_strength = clamp((1.0 - roughness) * 0.75 + 0.25, 0.0, 1.0);
    return vec4<f32>(metalness, roughness, gloss_strength, specular_strength);
}

fn resolve_specular_mask_factor(mask_red: f32) -> f32 {
    if material.properties.w > 1.5 {
        return mask_red * mask_red;
    }
    return 1.0;
}

struct TileArraySample {
    normal: vec3<f32>,
    orb: vec3<f32>,
    normal_weight: f32,
    color_multiplier: f32,
};

struct DetailArraySample {
    diffuse: vec3<f32>,
    normal: vec3<f32>,
};

fn resolve_tile_array(input: VertexOutput, extra: ExtraProperties) -> TileArraySample {
    var out: TileArraySample;
    out.normal = vec3<f32>(0.0, 0.0, 1.0);
    out.orb = vec3<f32>(1.0);
    out.normal_weight = 0.0;
    out.color_multiplier = 1.0;
    if material.array_params.z <= 0.5 {
        return out;
    }

    let layer_count = max(round(material.array_params.x), 1.0);
    let ramp_layer_a = tile_array_layer(clamp(extra.tile_a.x, 0.0, 1.0) * 64.0, layer_count);
    let ramp_layer_b = tile_array_layer(clamp(extra.tile_b.x, 0.0, 1.0) * 64.0, layer_count);
    let shader_layer = tile_array_layer(material.tile_params.x, layer_count);
    let layer_a = select(shader_layer, ramp_layer_a, extra.flags.x > 0.5);
    let layer_b = select(shader_layer, ramp_layer_b, extra.flags.x > 0.5);
    let shader_alpha = clamp(material.tile_params.y, 0.0, 1.0);
    let tile_alpha_a = select(shader_alpha, clamp(extra.tile_a.y, 0.0, 1.0), extra.flags.x > 0.5);
    let tile_alpha_b = select(shader_alpha, clamp(extra.tile_b.y, 0.0, 1.0), extra.flags.x > 0.5);
    let ramp_blend = select(0.0, clamp(extra.tile_blend, 0.0, 1.0), extra.flags.x > 0.5);
    let source_uv = resolve_uv(input, material.uv_sources2.x, material.uv_scroll_masks2.x);
    let tiled_uv_a = vec2<f32>(
        dot(extra.tile_matrix_a.xy, source_uv),
        dot(extra.tile_matrix_a.zw, source_uv),
    ) * max(abs(material.tile_params.zw), vec2<f32>(0.001));
    let tiled_uv_b = vec2<f32>(
        dot(extra.tile_matrix_b.xy, source_uv),
        dot(extra.tile_matrix_b.zw, source_uv),
    ) * max(abs(material.tile_params.zw), vec2<f32>(0.001));
    let normal_coordinates_a = pair_atlas_coordinates(tiled_uv_a, layer_a, layer_count, 0.0);
    let normal_sample_a = textureSampleGrad(
        tile_array_pair_texture,
        tile_array_sampler,
        normal_coordinates_a.uv,
        normal_coordinates_a.ddx * exp2(resolve_tile_lod_bias(extra.tile_matrix_a)),
        normal_coordinates_a.ddy * exp2(resolve_tile_lod_bias(extra.tile_matrix_a)),
    );
    let orb_coordinates_a = pair_atlas_coordinates(tiled_uv_a, layer_a, layer_count, 1.0);
    let orb_sample_a = textureSampleGrad(
        tile_array_pair_texture,
        tile_array_sampler,
        orb_coordinates_a.uv,
        orb_coordinates_a.ddx * exp2(resolve_tile_lod_bias(extra.tile_matrix_a)),
        orb_coordinates_a.ddy * exp2(resolve_tile_lod_bias(extra.tile_matrix_a)),
    );
    let normal_coordinates_b = pair_atlas_coordinates(tiled_uv_b, layer_b, layer_count, 0.0);
    let normal_sample_b = textureSampleGrad(
        tile_array_pair_texture,
        tile_array_sampler,
        normal_coordinates_b.uv,
        normal_coordinates_b.ddx * exp2(resolve_tile_lod_bias(extra.tile_matrix_b)),
        normal_coordinates_b.ddy * exp2(resolve_tile_lod_bias(extra.tile_matrix_b)),
    );
    let orb_coordinates_b = pair_atlas_coordinates(tiled_uv_b, layer_b, layer_count, 1.0);
    let orb_sample_b = textureSampleGrad(
        tile_array_pair_texture,
        tile_array_sampler,
        orb_coordinates_b.uv,
        orb_coordinates_b.ddx * exp2(resolve_tile_lod_bias(extra.tile_matrix_b)),
        orb_coordinates_b.ddy * exp2(resolve_tile_lod_bias(extra.tile_matrix_b)),
    );
    let normal_a = transform_tile_normal(decode_normal(normal_sample_a), extra.tile_matrix_a);
    let normal_b = transform_tile_normal(decode_normal(normal_sample_b), extra.tile_matrix_b);
    out.normal = normalize(mix(normal_a, normal_b, ramp_blend));
    out.orb = mix(
        vec3<f32>(1.0, 0.5, 1.0)
            + tile_alpha_a * (orb_sample_a.rgb - vec3<f32>(1.0, 0.5, 1.0)),
        vec3<f32>(1.0, 0.5, 1.0)
            + tile_alpha_b * (orb_sample_b.rgb - vec3<f32>(1.0, 0.5, 1.0)),
        ramp_blend,
    );
    out.normal_weight = mix(
        clamp(normal_sample_a.a, 0.0, 1.0) * tile_alpha_a,
        clamp(normal_sample_b.a, 0.0, 1.0) * tile_alpha_b,
        ramp_blend,
    );
    out.color_multiplier = clamp(out.orb.b, 0.0, 1.0);
    return out;
}

fn transform_tile_normal(normal: vec3<f32>, tile_matrix: vec4<f32>) -> vec3<f32> {
    let axis_u = max(length(tile_matrix.xz), 1.0e-6);
    let axis_v = max(length(tile_matrix.yw), 1.0e-6);
    let normalized_matrix = tile_matrix / vec4<f32>(axis_u, axis_v, axis_u, axis_v);
    return normalize(vec3<f32>(
        dot(normalized_matrix.xy, normal.xy),
        dot(normalized_matrix.zw, normal.xy),
        normal.z,
    ));
}

fn resolve_tile_lod_bias(tile_matrix: vec4<f32>) -> f32 {
    // Installed character/legacy DXBC normalizes these two matrix axes and
    // adds max(log2(minAxis / 128), 0) before g_TileMipBiasOffset.
    let matrix_scale = min(length(tile_matrix.xz), length(tile_matrix.yw)) * 0.0078125;
    let matrix_bias = max(log2(max(matrix_scale, 1.0e-8)), 0.0);
    return clamp(matrix_bias + material.tile_lod_params.x, -16.0, 15.99);
}

fn tile_array_layer(tile_index: f32, layer_count: f32) -> f32 {
    return clamp(floor(max(tile_index, 0.0)), 0.0, layer_count - 1.0);
}

fn resolve_detail_array(input: VertexOutput) -> DetailArraySample {
    var out: DetailArraySample;
    out.diffuse = vec3<f32>(0.5);
    out.normal = vec3<f32>(0.0, 0.0, 1.0);
    if material.array_params.w <= 0.5 {
        return out;
    }

    let layer_count = max(round(material.array_params.y), 1.0);
    let detail_layer = clamp(round(max(material.detail_params.x, 0.0)), 0.0, layer_count - 1.0);
    let multi_layer = clamp(round(max(material.detail_params.y, 0.0)), 0.0, layer_count - 1.0);
    let detail_diffuse_coordinates = pair_atlas_coordinates(
        input.uv0 * max(abs(material.detail_color_uv_scale.xy), vec2<f32>(0.001)),
        detail_layer,
        layer_count,
        0.0,
    );
    let detail_diffuse = textureSampleGrad(
        detail_array_pair_texture,
        detail_array_sampler,
        detail_diffuse_coordinates.uv,
        detail_diffuse_coordinates.ddx,
        detail_diffuse_coordinates.ddy,
    ).rgb;
    let multi_diffuse_coordinates = pair_atlas_coordinates(
        input.uv0 * max(abs(material.detail_color_uv_scale.zw), vec2<f32>(0.001)),
        multi_layer,
        layer_count,
        0.0,
    );
    let multi_diffuse = textureSampleGrad(
        detail_array_pair_texture,
        detail_array_sampler,
        multi_diffuse_coordinates.uv,
        multi_diffuse_coordinates.ddx,
        multi_diffuse_coordinates.ddy,
    ).rgb;
    let detail_normal_coordinates = pair_atlas_coordinates(
        input.uv0 * max(abs(material.detail_normal_uv_scale.xy), vec2<f32>(0.001)),
        detail_layer,
        layer_count,
        1.0,
    );
    let detail_normal = decode_normal(textureSampleGrad(
        detail_array_pair_texture,
        detail_array_sampler,
        detail_normal_coordinates.uv,
        detail_normal_coordinates.ddx,
        detail_normal_coordinates.ddy,
    ));
    let multi_normal_coordinates = pair_atlas_coordinates(
        input.uv0 * max(abs(material.detail_normal_uv_scale.zw), vec2<f32>(0.001)),
        multi_layer,
        layer_count,
        1.0,
    );
    let multi_normal = decode_normal(textureSampleGrad(
        detail_array_pair_texture,
        detail_array_sampler,
        multi_normal_coordinates.uv,
        multi_normal_coordinates.ddx,
        multi_normal_coordinates.ddy,
    ));
    let multi_blend = select(0.0, clamp(input.color.a, 0.0, 1.0), material.detail_params.z > 0.5);
    let scaled_detail_normal = normalize(vec3<f32>(
        detail_normal.xy * clamp(material.shader_params.z, 0.0, 4.0),
        detail_normal.z,
    ));
    let scaled_multi_normal = normalize(vec3<f32>(
        multi_normal.xy * clamp(material.shader_params.w, 0.0, 4.0),
        multi_normal.z,
    ));
    out.diffuse = mix(detail_diffuse, multi_diffuse, multi_blend);
    out.normal = normalize(mix(scaled_detail_normal, scaled_multi_normal, multi_blend));
    return out;
}

struct PairAtlasCoordinates {
    uv: vec2<f32>,
    ddx: vec2<f32>,
    ddy: vec2<f32>,
};

fn pair_atlas_coordinates(uv: vec2<f32>, layer: f32, layer_count: f32, side: f32) -> PairAtlasCoordinates {
    let atlas_scale = vec2<f32>(0.5, 1.0 / layer_count);
    var out: PairAtlasCoordinates;
    out.uv = vec2<f32>(
        (fract(uv.x) + side) * 0.5,
        (fract(uv.y) + layer) / layer_count,
    );
    out.ddx = dpdx(uv) * atlas_scale;
    out.ddy = dpdy(uv) * atlas_scale;
    return out;
}

fn decode_normal(sampled: vec4<f32>) -> vec3<f32> {
    let xy = sampled.rg * 2.0 - vec2<f32>(1.0);
    return normalize(vec3<f32>(xy, sqrt(max(1.0 - dot(xy, xy), 0.001))));
}

struct ExtraProperties {
    tile_a: vec4<f32>,
    tile_b: vec4<f32>,
    sheen: vec4<f32>,
    sphere: vec4<f32>,
    tile_matrix_a: vec4<f32>,
    tile_matrix_b: vec4<f32>,
    tile_blend: f32,
    flags: vec4<f32>,
};

fn resolve_extra_properties(input: VertexOutput, color_table_weight: f32) -> ExtraProperties {
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
    extra.tile_a = vec4<f32>(0.0, 1.0, 0.0, 1.0);
    extra.tile_b = vec4<f32>(0.0, 1.0, 0.0, 1.0);
    extra.sheen = vec4<f32>(0.0, 0.0, 0.0, 1.0);
    extra.sphere = vec4<f32>(0.0, 0.0, 1.0, 1.0);
    extra.tile_matrix_a = vec4<f32>(1.0, 0.0, 0.0, 1.0);
    extra.tile_matrix_b = vec4<f32>(1.0, 0.0, 0.0, 1.0);
    extra.tile_blend = 0.0;
    if has_tile {
        let tile_uv = resolve_uv(input, material.uv_sources2.x, material.uv_scroll_masks2.x);
        if material.tile_lod_params.w > 0.5 {
            extra.tile_a = load_packed_tile_properties(tile_uv, 0u);
            extra.tile_b = load_packed_tile_properties(tile_uv, 1u);
            extra.tile_blend = color_table_weight;
        } else {
            extra.tile_a = textureSample(tile_properties_texture, tile_sampler, tile_uv);
            extra.tile_b = extra.tile_a;
        }
    }
    if has_sheen {
        extra.sheen = textureSample(sheen_properties_texture, sheen_sampler, resolve_uv(input, material.uv_sources2.y, material.uv_scroll_masks2.y));
        if material.tile_lod_params.y > 0.5 {
            extra.sheen = mix(
                load_packed_sheen(resolve_uv(input, material.uv_sources2.y, material.uv_scroll_masks2.y), 0u),
                load_packed_sheen(resolve_uv(input, material.uv_sources2.y, material.uv_scroll_masks2.y), 1u),
                color_table_weight,
            );
        }
    }
    if has_sphere {
        extra.sphere = textureSample(sphere_properties_texture, sphere_sampler, resolve_uv(input, material.uv_sources2.z, material.uv_scroll_masks2.z));
        if material.tile_lod_params.y > 0.5 {
            extra.sphere = mix(
                load_packed_sphere(resolve_uv(input, material.uv_sources2.z, material.uv_scroll_masks2.z), 0u),
                load_packed_sphere(resolve_uv(input, material.uv_sources2.z, material.uv_scroll_masks2.z), 1u),
                color_table_weight,
            );
        }
    }
    if has_tile_matrix {
        let matrix_uv = resolve_uv(input, material.uv_sources2.w, material.uv_scroll_masks2.w);
        if material.tile_lod_params.w > 0.5 {
            extra.tile_matrix_a = load_packed_tile_matrix(matrix_uv, 0u);
            extra.tile_matrix_b = load_packed_tile_matrix(matrix_uv, 1u);
        } else {
            extra.tile_matrix_a = textureSample(tile_matrix_texture, tile_matrix_sampler, matrix_uv);
            extra.tile_matrix_b = extra.tile_matrix_a;
        }
    }
    return extra;
}

fn packed_ramp_texel(uv: vec2<f32>, dimensions: vec2<u32>, side: u32) -> vec2<i32> {
    let source_width = max(dimensions.x / 2u, 1u);
    let wrapped = fract(uv);
    let source_x = min(u32(floor(wrapped.x * f32(source_width))), source_width - 1u);
    let source_y = min(u32(floor(wrapped.y * f32(dimensions.y))), dimensions.y - 1u);
    return vec2<i32>(i32(source_x * 2u + side), i32(source_y));
}

fn load_packed_base(uv: vec2<f32>, side: u32) -> vec4<f32> {
    let dimensions = textureDimensions(base_color_texture);
    return textureLoad(base_color_texture, packed_ramp_texel(uv, dimensions, side), 0);
}

fn load_packed_specular(uv: vec2<f32>, side: u32) -> vec4<f32> {
    let dimensions = textureDimensions(specular_texture);
    return textureLoad(specular_texture, packed_ramp_texel(uv, dimensions, side), 0);
}

fn load_packed_material_properties(uv: vec2<f32>, side: u32) -> vec4<f32> {
    let dimensions = textureDimensions(material_properties_texture);
    return textureLoad(material_properties_texture, packed_ramp_texel(uv, dimensions, side), 0);
}

fn load_packed_sheen(uv: vec2<f32>, side: u32) -> vec4<f32> {
    let dimensions = textureDimensions(sheen_properties_texture);
    return textureLoad(sheen_properties_texture, packed_ramp_texel(uv, dimensions, side), 0);
}

fn load_packed_sphere(uv: vec2<f32>, side: u32) -> vec4<f32> {
    let dimensions = textureDimensions(sphere_properties_texture);
    return textureLoad(sphere_properties_texture, packed_ramp_texel(uv, dimensions, side), 0);
}

fn load_packed_emissive(uv: vec2<f32>, side: u32) -> vec4<f32> {
    let dimensions = textureDimensions(emissive_texture);
    return textureLoad(emissive_texture, packed_ramp_texel(uv, dimensions, side), 0);
}

fn sample_color_table_base(uv: vec2<f32>, mip_bias: f32, weight: f32) -> vec4<f32> {
    if material.tile_lod_params.y > 0.5 {
        return mix(load_packed_base(uv, 0u), load_packed_base(uv, 1u), weight);
    }
    return textureSampleBias(base_color_texture, base_color_sampler, uv, mip_bias);
}

fn sample_color_table_specular(uv: vec2<f32>, weight: f32) -> vec4<f32> {
    if material.tile_lod_params.y > 0.5 {
        return mix(load_packed_specular(uv, 0u), load_packed_specular(uv, 1u), weight);
    }
    return textureSample(specular_texture, specular_sampler, uv);
}

fn sample_color_table_emissive(uv: vec2<f32>, weight: f32) -> vec3<f32> {
    if material.tile_lod_params.y > 0.5 && material.emissive_color.a > 0.5 {
        return mix(
            load_packed_emissive(uv, 0u),
            load_packed_emissive(uv, 1u),
            weight,
        ).rgb;
    }
    return textureSample(emissive_texture, emissive_sampler, uv).rgb;
}

fn load_packed_tile_properties(uv: vec2<f32>, side: u32) -> vec4<f32> {
    let dimensions = textureDimensions(tile_properties_texture);
    return textureLoad(tile_properties_texture, packed_ramp_texel(uv, dimensions, side), 0);
}

fn load_packed_tile_matrix(uv: vec2<f32>, side: u32) -> vec4<f32> {
    let dimensions = textureDimensions(tile_matrix_texture);
    return textureLoad(tile_matrix_texture, packed_ramp_texel(uv, dimensions, side), 0);
}

fn resolve_surface_alpha(base_alpha: f32, normal_blue: f32, normal_alpha: f32) -> f32 {
    if material.alpha_policy_params.x > 3.5 {
        return clamp(normal_alpha, 0.0, 1.0);
    }
    if material.alpha_policy_params.x > 2.5 {
        return clamp(material.alpha_params.w, 0.0, 1.0);
    }
    if material.alpha_policy_params.x > 1.5 {
        return clamp(normal_blue, 0.0, 1.0);
    }
    if material.alpha_policy_params.x > 0.5 {
        return clamp(base_alpha, 0.0, 1.0);
    }
    return 1.0;
}

fn resolve_material_alpha(
    vertex_alpha: f32,
    base_texture_alpha: f32,
    normal_blue: f32,
    normal_alpha: f32,
    is_lightshaft: bool,
    is_crest_fallback: bool,
) -> f32 {
    let texture_alpha = resolve_surface_alpha(base_texture_alpha, normal_blue, normal_alpha);
    let remapped_vertex_alpha = mix(
        clamp(vertex_alpha, 0.0, 1.0),
        1.0,
        material.alpha_composition_params.x,
    );
    let opacity_vertex_alpha = select(
        1.0,
        remapped_vertex_alpha,
        material.alpha_composition_params.y > 0.5,
    );
    let is_mask = material.render.z > 0.5 && material.render.z < 1.5;
    let is_blend = material.alpha_policy_params.w > 0.5 && material.alpha_policy_params.w < 1.5;
    let is_glass = material.alpha_policy_params.w > 1.5;
    let uses_alpha = is_mask || is_blend || is_glass || is_lightshaft || is_crest_fallback || material.render.x > 0.5;
    var alpha = select(
        1.0,
        clamp(material.diffuse_color.a * texture_alpha * opacity_vertex_alpha, 0.0, 1.0),
        uses_alpha,
    );
    if material.alpha_policy_params.x > 2.5 {
        alpha = texture_alpha;
    }
    if is_glass {
        alpha = clamp(material.render.y * texture_alpha * opacity_vertex_alpha, 0.0, 1.0);
    }
    return alpha;
}

fn ordered_dither_threshold(position: vec2<f32>) -> f32 {
    const BAYER_4X4 = array<f32, 16>(
        0.0, 8.0, 2.0, 10.0,
        12.0, 4.0, 14.0, 6.0,
        3.0, 11.0, 1.0, 9.0,
        15.0, 7.0, 13.0, 5.0,
    );
    let pixel = vec2<u32>(max(position, vec2<f32>(0.0)));
    let index = (pixel.y & 3u) * 4u + (pixel.x & 3u);
    return (BAYER_4X4[index] + 0.5) / 16.0;
}

fn resolve_emissive(emissive_tex: vec3<f32>) -> vec3<f32> {
    let material_emissive = material.emissive_color.rgb;
    let shader_emissive = material.shader_emissive_color.rgb * material.shader_emissive_color.a;
    let texture_presence = material.emissive_color.a;
    // Installed Character/Legacy Final DXBC proves that ColorTable texel 2.5
    // emissive is multiplied by g_MaterialParameterDynamic.m_EmissiveColor,
    // then by max(dot(pre-emissive runtime lighting, Rec.601 luma), 1), before
    // reaching output. The offline preview has no dynamic-cbuffer provider, so
    // texture_emissive uses that multiplier's identity here. The verified
    // luminance shape is applied later to this ColorTable term using preview lit.
    let texture_emissive = emissive_tex * texture_presence;
    let fallback_emissive = material_emissive * (1.0 - texture_presence);
    return texture_emissive + fallback_emissive + shader_emissive;
}

fn resolve_shader_diffuse_tint() -> vec3<f32> {
    return material.shader_diffuse_color.rgb;
}

fn resolve_lightshaft_color(
    primary: vec3<f32>,
    secondary: vec3<f32>,
    vertex_color: vec4<f32>,
) -> vec4<f32> {
    let multiply_factor = clamp(vertex_color.b, 0.0, 1.0)
        * material.secondary_map_params.x;
    let multiplied = mix(
        primary,
        primary * secondary,
        multiply_factor,
    );
    let emission_color = multiplied * max(material.lightshaft_color.rgb, vec3<f32>(0.0));
    let emission_strength = dot(emission_color, vec3<f32>(0.2126, 0.7152, 0.0722));
    return vec4<f32>(emission_color * emission_strength, emission_strength * vertex_color.a);
}

fn resolve_view_direction(world_position: vec3<f32>) -> vec3<f32> {
    let to_camera = camera.camera_position.xyz - world_position;
    let has_direction = dot(to_camera, to_camera) > 0.000001;
    let safe_direction = to_camera + select(camera.view_dir.xyz, vec3<f32>(0.0), has_direction);
    return normalize(safe_direction);
}

fn orient_geometric_normal_toward_viewer(
    vertex_normal: vec3<f32>,
    view: vec3<f32>,
) -> vec3<f32> {
    // Two-sided surfaces must shade the visible side as front-facing. Triangle
    // winding (`front_facing`) is not a reliable orientation source because
    // imported vertex normals may disagree with it, so derive the sign from the
    // geometric relationship between the vertex normal and the viewer instead.
    // Callers rebuild the tangent frame from this oriented normal, which keeps
    // the tangent-space handedness consistent when the normal is flipped.
    let normal = normalize(vertex_normal);
    return normal * select(-1.0, 1.0, dot(normal, view) >= 0.0);
}

fn resolve_normal(
    input: VertexOutput,
    normal_sample: vec4<f32>,
    secondary_normal_sample: vec4<f32>,
    secondary_blend: f32,
    tile_array: TileArraySample,
) -> vec3<f32> {
    let view = resolve_view_direction(input.world_position);
    let geometric_normal = orient_geometric_normal_toward_viewer(input.normal, view);
    let has_primary = material.params.z > 0.5 || material.secondary_map_params.y > 0.5;
    let has_array_normal = tile_array.normal_weight > 0.001;
    let has_bitangent = dot(input.bitangent.xyz, input.bitangent.xyz) > 0.0001;
    let flow_tangent_plane = input.flow0.xyz - geometric_normal * dot(input.flow0.xyz, geometric_normal);
    let uses_flow = material.feature_params.x > 0.5 && dot(flow_tangent_plane, flow_tangent_plane) > 0.0001;
    if camera.options.x <= 0.5 || (!has_primary && !has_array_normal) || (!has_bitangent && !uses_flow) {
        return geometric_normal;
    }

    let safe_bitangent = input.bitangent.xyz + select(vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(0.0), has_bitangent);
    let primary_bitangent = normalize(safe_bitangent);
    let tangent_sign = select(1.0, -1.0, input.bitangent.w < 0.0);
    let primary_tangent = normalize(cross(primary_bitangent, geometric_normal)) * tangent_sign;
    let safe_flow_tangent = flow_tangent_plane + select(vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(0.0), uses_flow);
    let flow_tangent = normalize(safe_flow_tangent);
    let flow_bitangent_unoriented = normalize(cross(geometric_normal, flow_tangent));
    let flow_orientation = select(
        1.0,
        -1.0,
        has_bitangent && dot(flow_bitangent_unoriented, primary_bitangent) < 0.0,
    );
    let tangent = select(primary_tangent, flow_tangent, uses_flow);
    let bitangent = select(primary_bitangent, flow_bitangent_unoriented * flow_orientation, uses_flow);
    let primary_sampled = select(
        vec3<f32>(0.0, 0.0, 1.0),
        decode_normal(normal_sample),
        material.params.z > 0.5,
    );
    let primary_scale = clamp(material.shader_params.x, 0.0, 4.0);
    let secondary_scale = clamp(material.shader_params.y, 0.0, 4.0);
    let scaled_primary = normalize(vec3<f32>(
        primary_sampled.xy * primary_scale,
        primary_sampled.z,
    ));
    let decoded_secondary = decode_normal(secondary_normal_sample);
    let scaled_secondary = normalize(vec3<f32>(
        decoded_secondary.xy * secondary_scale,
        decoded_secondary.z,
    ));
    let uses_secondary_maps = material.secondary_map_params.w > 0.5;
    if uses_secondary_maps {
        let secondary_geometric_normal = orient_geometric_normal_toward_viewer(input.normal1, view);
        let has_secondary_bitangent = dot(input.bitangent1.xyz, input.bitangent1.xyz) > 0.0001;
        let safe_secondary_bitangent = input.bitangent1.xyz + select(
            vec3<f32>(1.0, 0.0, 0.0),
            vec3<f32>(0.0),
            has_secondary_bitangent,
        );
        let secondary_bitangent = normalize(safe_secondary_bitangent);
        let secondary_tangent_sign = select(1.0, -1.0, input.bitangent1.w < 0.0);
        let secondary_tangent = normalize(cross(secondary_bitangent, secondary_geometric_normal))
            * secondary_tangent_sign;
        let primary_world = normalize(
            tangent * scaled_primary.x
                + bitangent * scaled_primary.y
                + geometric_normal * scaled_primary.z,
        );
        let secondary_world = normalize(
            secondary_tangent * scaled_secondary.x
                + secondary_bitangent * scaled_secondary.y
                + secondary_geometric_normal * scaled_secondary.z,
        );
        return normalize(mix(
            primary_world,
            secondary_world,
            secondary_blend * material.secondary_map_params.y,
        ));
    }
    let mapped = normalize(vec3<f32>(
        scaled_primary.x + tile_array.normal.x * tile_array.normal_weight,
        scaled_primary.y * camera.options.y
            + tile_array.normal.y * camera.options.y * tile_array.normal_weight,
        max(
            scaled_primary.z * mix(1.0, tile_array.normal.z, tile_array.normal_weight),
            0.05,
        ),
    ));
    return normalize(tangent * mapped.x + bitangent * mapped.y + geometric_normal * mapped.z);
}

fn resolve_uv(input: VertexOutput, source: f32, scroll_enabled: f32) -> vec2<f32> {
    if source > 2.5 {
        return input.uv3;
    }
    if source > 1.5 {
        return input.uv2;
    }
    if source > 0.5 {
        return input.uv1 + material.uv_scroll.zw * camera.options.z * scroll_enabled;
    }
    return input.uv0 + material.uv_scroll.xy * camera.options.z * scroll_enabled;
}
