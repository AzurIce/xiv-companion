// Model shader module, part of the `package` WESL package
// (compiled to OUT_DIR/model.wgsl by the build script).
import package::alpha::{resolve_material_alpha};
import package::arrays::{TileArraySample};
import package::bindings::{VertexOutput, camera, color_table_index_texture, index_sampler, mask_sampler, mask_texture, material, material_properties_sampler, material_properties_texture, normal_sampler, normal_texture, sheen_properties_texture, sheen_sampler, sphere_properties_texture, sphere_sampler, tile_matrix_sampler, tile_matrix_texture, tile_properties_texture, tile_sampler};
import package::color_table::{load_packed_material_properties, load_packed_sheen, load_packed_specular, load_packed_sphere, load_packed_tile_matrix, load_packed_tile_properties, sample_color_table_base, sample_color_table_emissive, sample_color_table_specular};
import package::shading::{orient_geometric_normal_toward_viewer, resolve_character_family_base, resolve_emissive, resolve_lightshaft_color, resolve_shader_diffuse_tint, resolve_view_direction};

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
    ring_emission: vec3<f32>,
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
    var ring_emission = vec3<f32>(0.0);
    out.base = resolve_character_family_base(input, out.base, samples, &ring_emission);
    out.ring_emission = ring_emission;

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

    // 皮肤族（family_params.z）：皮肤/毛发是介电质，mask.b 是族别数据通道
    // （皮肤次表面强度等），不当金属度——否则敖龙 mask.b≈0.61 渲染成铜金属。
    let mask_metalness = mask.b * material.params.w * select(1.0, 0.0, material.family_params.z > 0.5);
    let metalness = clamp(max(material.params.y, mask_metalness), 0.0, 1.0);
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
