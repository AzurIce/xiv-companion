// Model shader module, part of the `package` WESL package
// (compiled to OUT_DIR/model.wgsl by the build script).
import package::bindings::{VertexOutput, detail_array_pair_texture, detail_array_sampler, material, tile_array_pair_texture, tile_array_sampler};
import package::surface::{ExtraProperties, decode_normal, resolve_uv};

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
