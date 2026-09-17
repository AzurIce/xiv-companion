// Model shader module, part of the `package` WESL package
// (compiled to OUT_DIR/model.wgsl by the build script).
import package::arrays::{DetailArraySample, TileArraySample};
import package::bindings::{FragmentOutput, VertexOutput, color_table_index_texture, index_sampler, material, multi_map_sampler, multi_map_texture, sheen_properties_texture, sheen_sampler, sphere_properties_texture, sphere_sampler};
import package::shading::{resolve_view_direction};
import package::surface::{ExtraProperties, resolve_uv};

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
        let multi_map_uv = resolve_uv(input, material.uv_sources1.x, material.uv_scroll_masks1.x);
        color = textureSample(multi_map_texture, multi_map_sampler, multi_map_uv).rgb;
    } else if mode < 16.5 {
        color = mix(extra.tile_a, extra.tile_b, extra.tile_blend).rgb;
    } else if mode < 17.5 {
        let sheen_uv = resolve_uv(input, material.uv_sources2.y, material.uv_scroll_masks2.y);
        color = textureSample(sheen_properties_texture, sheen_sampler, sheen_uv).rgb;
    } else if mode < 18.5 {
        let sphere_uv = resolve_uv(input, material.uv_sources2.z, material.uv_scroll_masks2.z);
        color = textureSample(sphere_properties_texture, sphere_sampler, sphere_uv).rgb;
    } else if mode < 19.5 {
        color = mix(extra.tile_matrix_a, extra.tile_matrix_b, extra.tile_blend).rgb;
    } else if mode < 20.5 {
        color = tile_array.normal * 0.5 + vec3<f32>(0.5);
    } else if mode < 21.5 {
        color = tile_array.orb;
    } else if mode < 22.5 {
        color = detail_array.diffuse;
    } else if mode < 23.5 {
        color = detail_array.normal * 0.5 + vec3<f32>(0.5);
    } else if mode < 24.5 {
        color = input.color1.rgb;
    } else if mode < 25.5 {
        color = normalize(input.normal1) * 0.5 + vec3<f32>(0.5);
    } else if mode < 26.5 {
        color = input.flow0.xyz * 0.5 + vec3<f32>(0.5);
    } else if mode < 27.5 {
        color = input.flow1.xyz * 0.5 + vec3<f32>(0.5);
    } else if mode < 28.5 {
        color = material.unsupported_color.rgb;
    } else {
        color = resolve_view_direction(input.world_position) * 0.5 + vec3<f32>(0.5);
    }

    var out: FragmentOutput;
    let preserves_hdr_color = mode < 1.5
        || (mode >= 3.5 && mode < 6.5)
        || (mode >= 16.5 && mode < 17.5);
    let debug_color = select(clamp(color, vec3<f32>(0.0), vec3<f32>(1.0)), max(color, vec3<f32>(0.0)), preserves_hdr_color);
    out.color = vec4<f32>(debug_color, 1.0);
    return out;
}

fn uv_debug_color(uv: vec2<f32>) -> vec3<f32> {
    return vec3<f32>(fract(uv.x), fract(uv.y), 0.5);
}
