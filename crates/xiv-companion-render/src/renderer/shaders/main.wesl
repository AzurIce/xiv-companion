import package::alpha::{ordered_dither_threshold, resolve_material_alpha};
import package::arrays::{resolve_detail_array, resolve_tile_array};
import package::bindings::{FragmentOutput, VertexInput, VertexOutput, base_color_sampler, base_color_texture, camera, material, normal_sampler, normal_texture, tile_properties_texture, tile_sampler};
import package::color_table::{sample_color_table_base};
import package::debug::{debug_fragment_output};
import package::shading::{resolve_lightshaft_color, resolve_surface_output};
import package::skinning::{joint_data, skin_vertex};
import package::surface::{resolve_color_table_blend, resolve_extra_properties, resolve_surface_pass_flags, resolve_surface_samples, resolve_surface_state, resolve_texture_mip_bias, resolve_uv};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var position = vec4<f32>(input.position, 1.0);
    var normal = input.normal;
    if joint_data.count > 0u {
        let skinned = skin_vertex(
            position,
            normal,
            input.joints,
            input.weights0,
            input.weights1,
        );
        position = skinned.position;
        normal = skinned.normal;
    }
    var out: VertexOutput;
    out.clip_position = camera.view_proj * position;
    out.normal = normalize(normal);
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
    out.world_position = position.xyz;
    out.model_position = input.position;
    return out;
}

@vertex
fn vs_outline(input: VertexInput) -> VertexOutput {
    let width = clamp(material.outline_params.a, 0.0, 0.1);
    var position = vec4<f32>(input.position, 1.0);
    var normal = input.normal;
    if joint_data.count > 0u {
        let skinned = skin_vertex(
            position,
            normal,
            input.joints,
            input.weights0,
            input.weights1,
        );
        position = skinned.position;
        normal = skinned.normal;
    }
    var out: VertexOutput;
    out.clip_position = camera.view_proj * vec4<f32>(position.xyz + normalize(normal) * width, 1.0);
    out.normal = normalize(normal);
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
    out.world_position = position.xyz + normalize(normal) * width;
    out.model_position = input.position;
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
        if material.family_params.w > 0.5 {
            // hair.shpk：alpha-to-coverage 近似——固定阈值会把发丝边缘渐变带
            // 整片削碎（敖龙刘海实证），按 4x4 screen-door 抖动保留软边缘。
            if surface.alpha <= ordered_dither_threshold(input.clip_position.xy) {
                discard;
            }
        } else {
            discard;
        }
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
