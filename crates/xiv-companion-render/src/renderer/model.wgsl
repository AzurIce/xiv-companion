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
    alpha_policy_params: vec4<f32>, // x: source 0=opaque 1=base alpha 2=normal blue 3=material transparency 4=normal alpha, y: lighting, z: dither depth, w: prepared pass
    water_deep_color: vec4<f32>,
    water_refraction_color: vec4<f32>,
    water_whitecap_color: vec4<f32>,
    glass_params: vec4<f32>, // x: IOR, y: max thickness
    extra_properties: vec4<f32>, // x: tile, y: sheen, z: sphere, w: tile matrix
    shader_params: vec4<f32>, // x: normal, y: multi normal, z: detail normal, w: multi detail normal
    tile_params: vec4<f32>, // x: tile index, y: tile alpha, zw: tile repeat uv
    toon_sheen_params: vec4<f32>, // x: toon index, y: toon light scale, z: sheen rate, w: sheen tint rate
    toon_params: vec4<f32>, // x: light spec aperture, y: reflection scale, z: spec index, w: prepared toon
    sheen_sphere_params: vec4<f32>, // x: sheen aperture, y: sphere map index
    detail_params: vec4<f32>, // x: detail id, y: multi detail id, z: GetMultiValues detail blend
    array_params: vec4<f32>, // x: tile layers, y: detail layers, z: has tile pair, w: has detail pair
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
    secondary_map_params: vec4<f32>, // xyz: secondary color/normal/specular present, w: GetMultiValues blend
    draw_role_params: vec4<f32>, // x: lightshaft, y: transparent crest fallback, z: base material fallback
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
    @location(7) flow0: vec4<f32>,
    @location(8) flow1: vec4<f32>,
    @location(9) normal1: vec3<f32>,
    @location(10) bitangent1: vec4<f32>,
    @location(11) color1: vec4<f32>,
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
    out.flow0 = input.flow0;
    out.flow1 = input.flow1;
    out.normal1 = normalize(input.normal1);
    out.bitangent1 = input.bitangent1;
    out.color1 = input.color1;
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
    return out;
}

@fragment
fn fs_outline() -> FragmentOutput {
    var out: FragmentOutput;
    out.color = vec4<f32>(clamp(material.outline_params.rgb, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
    out.bright = vec4<f32>(0.0, 0.0, 0.0, 1.0);
    return out;
}

@fragment
fn fs_dither_depth(input: VertexOutput) -> FragmentOutput {
    let base_uv = resolve_uv(input, material.uv_sources0.x, material.uv_scroll_masks0.x);
    let secondary_base_uv = resolve_uv(input, material.uv_sources2.x, material.uv_scroll_masks2.x);
    let normal_uv = resolve_uv(input, material.uv_sources0.y, material.uv_scroll_masks0.y);
    let mip_bias = resolve_texture_mip_bias();
    let sampled_base = textureSampleBias(base_color_texture, base_color_sampler, base_uv, mip_bias);
    let sampled_secondary_base = textureSampleBias(
        tile_properties_texture,
        tile_sampler,
        secondary_base_uv,
        mip_bias,
    );
    let sampled_normal = textureSampleBias(normal_texture, normal_sampler, normal_uv, mip_bias);
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
    out.bright = vec4<f32>(0.0);
    return out;
}

@fragment
fn fs_main(input: VertexOutput, @builtin(front_facing) front_facing: bool) -> FragmentOutput {
    let is_lightshaft = material.draw_role_params.x > 0.5;
    let is_crest_fallback = material.draw_role_params.y > 0.5;
    var base_uv = resolve_uv(input, material.uv_sources0.x, material.uv_scroll_masks0.x);
    if is_lightshaft {
        base_uv = resolve_lightshaft_uv(input);
    }
    let normal_uv = resolve_uv(input, material.uv_sources0.y, material.uv_scroll_masks0.y);
    let secondary_base_uv = resolve_uv(input, material.uv_sources2.x, material.uv_scroll_masks2.x);
    let secondary_normal_uv = resolve_uv(input, material.uv_sources2.y, material.uv_scroll_masks2.y);
    let secondary_specular_uv = resolve_uv(input, material.uv_sources2.z, material.uv_scroll_masks2.z);
    let mask_uv = resolve_uv(input, material.uv_sources0.z, material.uv_scroll_masks0.z);
    let specular_uv = resolve_uv(input, material.uv_sources1.y, material.uv_scroll_masks1.y);
    let emissive_uv = resolve_uv(input, material.uv_sources1.z, material.uv_scroll_masks1.z);
    let material_properties_uv = resolve_uv(input, material.uv_sources1.w, material.uv_scroll_masks1.w);
    let mip_bias = resolve_texture_mip_bias();

    let extra = resolve_extra_properties(input);
    let tile_array = resolve_tile_array(input, extra);
    let detail_array = resolve_detail_array(input);
    let sampled_normal = textureSampleBias(normal_texture, normal_sampler, normal_uv, mip_bias);
    let sampled_secondary_normal = textureSampleBias(
        sheen_properties_texture,
        sheen_sampler,
        secondary_normal_uv,
        mip_bias,
    );
    let secondary_blend = clamp(input.color.a, 0.0, 1.0) * material.secondary_map_params.w;
    let sampled_specular = textureSampleBias(specular_texture, specular_sampler, specular_uv, mip_bias).rgb;
    let sampled_secondary_specular = textureSampleBias(
        sphere_properties_texture,
        sphere_sampler,
        secondary_specular_uv,
        mip_bias,
    ).rgb;
    let effective_specular_sample = mix(
        sampled_specular,
        sampled_secondary_specular,
        secondary_blend * material.secondary_map_params.z,
    );
    let normal = resolve_normal(
        input,
        front_facing,
        sampled_normal,
        sampled_secondary_normal,
        secondary_blend,
        tile_array,
        detail_array,
    );
    let light = normalize(camera.light_dir.xyz);
    let diffuse = max(dot(normal, light), 0.0);
    let half_dir = normalize(light + vec3<f32>(0.0, 0.0, 1.0));
    let mask = resolve_mask(mask_uv);
    var properties = resolve_material_properties(material_properties_uv, mask);
    let has_bg_specular = material.properties.y > 0.5 || material.secondary_map_params.z > 0.5;
    if material.feature_params.w > 0.5 && has_bg_specular {
        properties.x = effective_specular_sample.b;
        properties.y = effective_specular_sample.g;
    }
    let metalness = clamp(properties.x, 0.0, 1.0);
    let roughness = clamp(properties.y, 0.08, 1.0);
    let gloss_strength = clamp(properties.z, 0.0, 1.0);
    let specular_strength = clamp(properties.w, 0.0, 1.0);
    let specular_color_mask = clamp(material.specular_color_mask, vec4<f32>(0.0), vec4<f32>(4.0));
    let specular_scale = specular_strength
        * mix(1.0, mask.r * 1.35, material.params.w)
        * specular_color_mask.a;
    let specular_power = mix(12.0, 96.0, gloss_strength) * (1.0 - roughness * 0.55);
    let normal_half = max(dot(normal, half_dir), 0.0);
    let specular = pow(normal_half, specular_power);
    let toon_lighting = resolve_toon_lighting(diffuse, normal_half, specular);
    let sampled_base = textureSampleBias(base_color_texture, base_color_sampler, base_uv, mip_bias);
    let sampled_secondary_base = textureSampleBias(
        tile_properties_texture,
        tile_sampler,
        secondary_base_uv,
        mip_bias,
    );
    let emissive_tex = textureSampleBias(
        emissive_texture,
        emissive_sampler,
        emissive_uv,
        mip_bias,
    ).rgb;
    let primary_texture = select(vec3<f32>(1.0), sampled_base.rgb, material.params.x > 0.5);
    let secondary_color_weight = secondary_blend * material.secondary_map_params.x;
    let scroll_texture_mix = mix(
        primary_texture * clamp(material.shader_diffuse_color.rgb, vec3<f32>(0.0), vec3<f32>(4.0)),
        sampled_secondary_base.rgb * clamp(material.shader_multi_diffuse_color.rgb, vec3<f32>(0.0), vec3<f32>(4.0)),
        secondary_color_weight,
    );
    let texture_mix = select(primary_texture, scroll_texture_mix, material.secondary_map_params.w > 0.5);
    let primary_alpha = select(1.0, sampled_base.a, material.params.x > 0.5);
    let base_texture_alpha = mix(primary_alpha, sampled_secondary_base.a, secondary_color_weight);
    let primary_specular = select(
        material.specular_color.rgb,
        sampled_specular,
        material.properties.y > 0.5,
    );
    let generic_material_specular = mix(
        primary_specular,
        sampled_secondary_specular,
        secondary_blend * material.secondary_map_params.z,
    );
    let material_specular = select(
        generic_material_specular,
        material.specular_color.rgb,
        material.feature_params.w > 0.5,
    ) * specular_color_mask.rgb;
    let vertex_tint = select(vec3<f32>(1.0), input.color.rgb, material.properties.z > 0.5);
    let is_mask = material.render.z > 0.5 && material.render.z < 1.5;
    let is_blend = material.alpha_policy_params.w > 0.5 && material.alpha_policy_params.w < 1.5;
    let is_glass = material.alpha_policy_params.w > 1.5;
    let uses_alpha = is_mask || is_blend || is_glass || is_lightshaft || is_crest_fallback || material.render.x > 0.5;
    let shader_tint = resolve_shader_diffuse_tint(mask);
    let detail_tint = resolve_detail_tint(input, detail_array);
    let generic_base = material.diffuse_color.rgb
        * texture_mix
        * vertex_tint
        * shader_tint
        * detail_tint
        * tile_array.color_multiplier;
    let scroll_base = texture_mix * vertex_tint;
    let family_base = select(generic_base, scroll_base, material.secondary_map_params.w > 0.5);
    let is_water = material.feature_params.y > 0.5;
    let base = select(
        family_base,
        clamp(material.water_deep_color.rgb, vec3<f32>(0.0), vec3<f32>(4.0)),
        is_water,
    );
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
        is_lightshaft,
        is_crest_fallback,
    );
    let emissive = resolve_emissive(emissive_tex, input.color.a, mask);
    if camera.options.w > 0.5 {
        return debug_fragment_output(
            input,
            camera.options.w,
            base,
            normal,
            mask,
            properties,
            material_specular,
            emissive,
            alpha,
            tile_array,
            detail_array,
        );
    }
    if is_crest_fallback {
        discard;
    }
    if is_mask && alpha < material.render.w {
        discard;
    }
    if uses_alpha && alpha < 0.01 {
        discard;
    }
    if is_lightshaft {
        let lightshaft = resolve_lightshaft_color(base, base_texture_alpha, input.color.a);
        var out: FragmentOutput;
        out.color = lightshaft;
        out.bright = vec4<f32>(lightshaft.rgb * 1.15, 1.0);
        return out;
    }
    let rim = pow(1.0 - max(normal.z, 0.0), 2.0)
        * select(0.16, 0.58, is_glass)
        * toon_lighting.z;
    let specular_tint = mix(material_specular, base, metalness * 0.35);
    let glass_factors = resolve_glass_factors();
    let glass_tint = mix(
        vec3<f32>(0.82, 0.94, 1.0),
        clamp(base, vec3<f32>(0.0), vec3<f32>(2.0)),
        0.18 + glass_factors.y * 0.22,
    );
    let ssao_mask = clamp(material.surface_params.x, 0.0, 1.0);
    let ambient = mix(0.08, 0.22, ssao_mask);
    let glass_ambient = mix(0.52, 0.62, ssao_mask);
    let opaque_lit = base * (ambient + toon_lighting.x * 0.74)
        + specular_tint * toon_lighting.y * specular_scale * 0.24
        + vec3<f32>(rim);
    let glass_lit = glass_tint * (glass_ambient + toon_lighting.x * 0.12)
        + material_specular * toon_lighting.y * (0.65 + glass_factors.z * 0.25)
        + vec3<f32>(rim) * vec3<f32>(0.60, 0.85, 1.0) * (1.0 + glass_factors.x * 0.35);
    let lighting_enabled = material.alpha_policy_params.y > 0.5;
    let surface_lit = select(base, opaque_lit, lighting_enabled);
    let lit = select(surface_lit, glass_lit, is_glass);
    let extra_lit = select(
        vec3<f32>(0.0),
        resolve_extra_lighting(extra, normal, half_dir, rim, material_specular, base, is_glass),
        lighting_enabled,
    );
    let color = lit + extra_lit + emissive;
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
    tile_array: TileArraySample,
    detail_array: DetailArraySample,
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
        let tile_uv = resolve_uv(input, material.uv_sources2.x, material.uv_scroll_masks2.x);
        color = textureSample(tile_properties_texture, tile_sampler, tile_uv).rgb;
    } else if mode < 18.5 {
        let sheen_uv = resolve_uv(input, material.uv_sources2.y, material.uv_scroll_masks2.y);
        color = textureSample(sheen_properties_texture, sheen_sampler, sheen_uv).rgb;
    } else if mode < 19.5 {
        let sphere_uv = resolve_uv(input, material.uv_sources2.z, material.uv_scroll_masks2.z);
        color = textureSample(sphere_properties_texture, sphere_sampler, sphere_uv).rgb;
    } else if mode < 20.5 {
        let tile_matrix_uv = resolve_uv(input, material.uv_sources2.w, material.uv_scroll_masks2.w);
        color = textureSample(tile_matrix_texture, tile_matrix_sampler, tile_matrix_uv).rgb;
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
    } else {
        color = input.flow1.xyz * 0.5 + vec3<f32>(0.5);
    }

    var out: FragmentOutput;
    out.color = vec4<f32>(clamp(color, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
    out.bright = vec4<f32>(0.0, 0.0, 0.0, 1.0);
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

fn resolve_material_properties(uv: vec2<f32>, mask: vec3<f32>) -> vec4<f32> {
    if material.properties.x > 0.5 {
        return textureSampleBias(
            material_properties_texture,
            material_properties_sampler,
            uv,
            resolve_texture_mip_bias(),
        );
    }

    let metalness = clamp(max(material.params.y, mask.b * material.params.w), 0.0, 1.0);
    let roughness = clamp(mix(material.specular_color.a, mask.g, material.params.w), 0.08, 1.0);
    let specular_strength = mix(1.0, mask.r * 1.35, material.params.w);
    let gloss_strength = clamp((1.0 - roughness) * 0.75 + 0.25, 0.0, 1.0);
    return vec4<f32>(metalness, roughness, gloss_strength, specular_strength);
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
    tint: vec3<f32>,
    normal_weight: f32,
    available: f32,
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
    let ramp_layer = round(clamp(extra.tile.x, 0.0, 1.0) * 64.0);
    let shader_layer = round(max(material.tile_params.x, 0.0));
    let layer = clamp(select(shader_layer, ramp_layer, extra.flags.x > 0.5), 0.0, layer_count - 1.0);
    let shader_alpha = clamp(material.tile_params.y, 0.0, 1.0);
    let tile_alpha = select(shader_alpha, clamp(extra.tile.y, 0.0, 1.0), extra.flags.x > 0.5);
    let source_uv = resolve_uv(input, material.uv_sources2.x, material.uv_scroll_masks2.x);
    let transformed_uv = vec2<f32>(
        dot(extra.tile_matrix.xy, source_uv),
        dot(extra.tile_matrix.zw, source_uv),
    );
    let tiled_uv = transformed_uv * max(abs(material.tile_params.zw), vec2<f32>(0.001));
    let normal_sample = textureSample(
        tile_array_pair_texture,
        tile_array_sampler,
        pair_atlas_uv(tiled_uv, layer, layer_count, 0.0),
    );
    let orb_sample = textureSample(
        tile_array_pair_texture,
        tile_array_sampler,
        pair_atlas_uv(tiled_uv, layer, layer_count, 1.0),
    );
    out.normal = decode_normal(normal_sample);
    out.orb = orb_sample.rgb;
    out.normal_weight = clamp(normal_sample.a, 0.0, 1.0) * tile_alpha;
    out.color_multiplier = clamp(orb_sample.b, 0.0, 1.0);
    return out;
}

fn resolve_detail_array(input: VertexOutput) -> DetailArraySample {
    var out: DetailArraySample;
    out.diffuse = vec3<f32>(0.5);
    out.normal = vec3<f32>(0.0, 0.0, 1.0);
    out.tint = vec3<f32>(1.0);
    out.normal_weight = 0.0;
    out.available = 0.0;
    if material.array_params.w <= 0.5 {
        return out;
    }

    let layer_count = max(round(material.array_params.y), 1.0);
    let detail_layer = clamp(round(max(material.detail_params.x, 0.0)), 0.0, layer_count - 1.0);
    let multi_layer = clamp(round(max(material.detail_params.y, 0.0)), 0.0, layer_count - 1.0);
    let detail_diffuse = textureSample(
        detail_array_pair_texture,
        detail_array_sampler,
        pair_atlas_uv(input.uv0 * max(abs(material.detail_color_uv_scale.xy), vec2<f32>(0.001)), detail_layer, layer_count, 0.0),
    ).rgb;
    let multi_diffuse = textureSample(
        detail_array_pair_texture,
        detail_array_sampler,
        pair_atlas_uv(input.uv0 * max(abs(material.detail_color_uv_scale.zw), vec2<f32>(0.001)), multi_layer, layer_count, 0.0),
    ).rgb;
    let detail_normal = decode_normal(textureSample(
        detail_array_pair_texture,
        detail_array_sampler,
        pair_atlas_uv(input.uv0 * max(abs(material.detail_normal_uv_scale.xy), vec2<f32>(0.001)), detail_layer, layer_count, 1.0),
    ));
    let multi_normal = decode_normal(textureSample(
        detail_array_pair_texture,
        detail_array_sampler,
        pair_atlas_uv(input.uv0 * max(abs(material.detail_normal_uv_scale.zw), vec2<f32>(0.001)), multi_layer, layer_count, 1.0),
    ));
    let detail_tint = clamp(detail_diffuse * 2.0 * material.detail_color.rgb * 2.0, vec3<f32>(0.25), vec3<f32>(1.75));
    let multi_tint = clamp(multi_diffuse * 2.0 * material.multi_detail_color.rgb * 2.0, vec3<f32>(0.25), vec3<f32>(1.75));
    let detail_weight = clamp(material.detail_color.a, 0.0, 1.0) * 0.22;
    let multi_weight = clamp(material.multi_detail_color.a, 0.0, 1.0) * 0.14;
    let multi_blend = select(0.0, clamp(input.color.a, 0.0, 1.0), material.detail_params.z > 0.5);
    let scaled_detail_normal = normalize(vec3<f32>(
        detail_normal.xy * clamp(material.shader_params.z, 0.0, 4.0),
        detail_normal.z,
    ));
    let scaled_multi_normal = normalize(vec3<f32>(
        multi_normal.xy * clamp(material.shader_params.w, 0.0, 4.0),
        multi_normal.z,
    ));
    let primary_tint = mix(vec3<f32>(1.0), detail_tint, detail_weight);
    let secondary_tint = mix(vec3<f32>(1.0), multi_tint, multi_weight);
    out.diffuse = mix(detail_diffuse, multi_diffuse, multi_blend);
    out.normal = normalize(mix(scaled_detail_normal, scaled_multi_normal, multi_blend));
    out.tint = mix(primary_tint, secondary_tint, multi_blend);
    out.normal_weight = 0.32;
    out.available = 1.0;
    return out;
}

fn pair_atlas_uv(uv: vec2<f32>, layer: f32, layer_count: f32, side: f32) -> vec2<f32> {
    return vec2<f32>(
        (fract(uv.x) + side) * 0.5,
        (fract(uv.y) + layer) / layer_count,
    );
}

fn decode_normal(sampled: vec4<f32>) -> vec3<f32> {
    let xy = sampled.rg * 2.0 - vec2<f32>(1.0);
    return normalize(vec3<f32>(xy, sqrt(max(1.0 - dot(xy, xy), 0.001))));
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
        extra.tile = textureSample(tile_properties_texture, tile_sampler, resolve_uv(input, material.uv_sources2.x, material.uv_scroll_masks2.x));
    }
    if has_sheen {
        extra.sheen = textureSample(sheen_properties_texture, sheen_sampler, resolve_uv(input, material.uv_sources2.y, material.uv_scroll_masks2.y));
    }
    if has_sphere {
        extra.sphere = textureSample(sphere_properties_texture, sphere_sampler, resolve_uv(input, material.uv_sources2.z, material.uv_scroll_masks2.z));
    }
    if has_tile_matrix {
        extra.tile_matrix = textureSample(tile_matrix_texture, tile_matrix_sampler, resolve_uv(input, material.uv_sources2.w, material.uv_scroll_masks2.w));
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

    return sheen_color + sphere_tint * sphere_term;
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
    let is_mask = material.render.z > 0.5 && material.render.z < 1.5;
    let is_blend = material.alpha_policy_params.w > 0.5 && material.alpha_policy_params.w < 1.5;
    let is_glass = material.alpha_policy_params.w > 1.5;
    let uses_alpha = is_mask || is_blend || is_glass || is_lightshaft || is_crest_fallback || material.render.x > 0.5;
    var alpha = select(
        1.0,
        clamp(material.diffuse_color.a * texture_alpha * vertex_alpha, 0.0, 1.0),
        uses_alpha,
    );
    if material.alpha_policy_params.x > 2.5 {
        alpha = texture_alpha;
    } else if uses_alpha && !is_glass && !is_lightshaft {
        alpha = resolve_alpha_shaping(alpha);
    }
    if is_glass {
        alpha = clamp(material.render.y * texture_alpha * vertex_alpha, 0.0, 1.0);
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

fn resolve_glass_factors() -> vec3<f32> {
    let ior_delta = clamp((clamp(material.glass_params.x, 1.0, 2.5) - 1.0) / 1.5, 0.0, 1.0);
    let thickness_delta = clamp((max(material.glass_params.y, 0.0) - 0.01) * 8.0, 0.0, 1.0);
    return vec3<f32>(ior_delta, thickness_delta, max(ior_delta, thickness_delta * 0.35));
}

fn resolve_toon_lighting(raw_diffuse: f32, normal_half: f32, raw_specular: f32) -> vec3<f32> {
    if material.toon_params.w < 0.5 {
        return vec3<f32>(raw_diffuse, raw_specular, 1.0);
    }

    let toon_index_phase = fract(abs(material.toon_sheen_params.x) * 0.6180339);
    let scaled_diffuse = clamp(
        raw_diffuse * clamp(material.toon_sheen_params.y, 0.0, 8.0) * 0.5,
        0.0,
        1.0,
    );
    let diffuse_threshold = 0.45 + (toon_index_phase - 0.5) * 0.18;
    let shadow_level = 0.40 + toon_index_phase * 0.10;
    let diffuse_band = mix(
        shadow_level,
        1.0,
        smoothstep(diffuse_threshold - 0.12, diffuse_threshold + 0.12, scaled_diffuse),
    );
    let toon_diffuse = mix(scaled_diffuse, diffuse_band, 0.35);

    let spec_index_phase = fract(abs(material.toon_params.z) * 0.6180339);
    let spec_aperture = clamp(material.toon_params.x, 1.0, 256.0);
    let spec_signal = pow(clamp(normal_half, 0.0, 1.0), spec_aperture);
    let spec_threshold = 0.35 + spec_index_phase * 0.30;
    let spec_band = smoothstep(spec_threshold - 0.08, spec_threshold + 0.08, spec_signal);
    let toon_specular = mix(raw_specular, max(raw_specular, spec_band), 0.40);
    let reflection_scale = clamp(material.toon_params.y / 2.5, 0.25, 3.0);

    return vec3<f32>(toon_diffuse, toon_specular, reflection_scale);
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

fn resolve_detail_tint(input: VertexOutput, detail_array: DetailArraySample) -> vec3<f32> {
    if detail_array.available > 0.5 {
        return detail_array.tint;
    }
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
    let multi_blend = select(0.0, clamp(input.color.a, 0.0, 1.0), material.detail_params.z > 0.5);
    return mix(detail, multi_detail, multi_blend);
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

fn resolve_normal(
    input: VertexOutput,
    front_facing: bool,
    normal_sample: vec4<f32>,
    secondary_normal_sample: vec4<f32>,
    secondary_blend: f32,
    tile_array: TileArraySample,
    detail_array: DetailArraySample,
) -> vec3<f32> {
    let face_sign = select(-1.0, 1.0, front_facing);
    let geometric_normal = normalize(input.normal) * face_sign;
    let has_primary = material.params.z > 0.5 || material.secondary_map_params.y > 0.5;
    let has_array_normal = tile_array.normal_weight > 0.001 || detail_array.normal_weight > 0.001;
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
        let secondary_geometric_normal = normalize(input.normal1) * face_sign;
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
    let sampled = primary_sampled;
    let normal_scale = select(
        resolve_effective_normal_scale(detail_array.available > 0.5),
        1.0,
        false,
    );
    let mapped = normalize(vec3<f32>(
        sampled.x * normal_scale
            + tile_array.normal.x * tile_array.normal_weight
            + detail_array.normal.x * detail_array.normal_weight,
        sampled.y * camera.options.y * normal_scale
            + tile_array.normal.y * camera.options.y * tile_array.normal_weight
            + detail_array.normal.y * camera.options.y * detail_array.normal_weight,
        max(
            sampled.z
                * mix(1.0, tile_array.normal.z, tile_array.normal_weight)
                * detail_array.normal.z,
            0.05,
        ),
    ));
    return normalize(tangent * mapped.x + bitangent * mapped.y + geometric_normal * mapped.z);
}

fn resolve_effective_normal_scale(has_detail_array: bool) -> f32 {
    let primary = clamp(material.shader_params.x, 0.0, 4.0);
    let multi_delta = clamp(material.shader_params.y, 0.0, 4.0) - 1.0;
    let detail_delta = clamp(material.shader_params.z, 0.0, 4.0) - 1.0;
    let multi_detail_delta = clamp(material.shader_params.w, 0.0, 4.0) - 1.0;
    let fallback_detail = select(detail_delta * 0.12 + multi_detail_delta * 0.08, 0.0, has_detail_array);
    let fallback_delta = multi_delta * 0.08 + fallback_detail;
    return clamp(primary + fallback_delta, 0.0, 4.0);
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
