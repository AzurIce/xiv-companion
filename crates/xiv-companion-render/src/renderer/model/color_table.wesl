// Model shader module, part of the `package` WESL package
// (compiled to OUT_DIR/model.wgsl by the build script).
import package::bindings::{base_color_sampler, base_color_texture, colorset_diffuse_texture, emissive_sampler, emissive_texture, material, material_properties_texture, sheen_properties_texture, specular_sampler, specular_texture, sphere_properties_texture, tile_matrix_texture, tile_properties_texture};

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
    if material.colorset_params.x > 0.5 {
        // Compatibility `base × colorset`: the game multiplies per pixel, so the
        // diffuse keeps its own filtered full-resolution sample and only the
        // ColorTable row color comes from the unfiltered A/B ramp texels.
        let diffuse = textureSampleBias(base_color_texture, base_color_sampler, uv, mip_bias);
        let dimensions = textureDimensions(colorset_diffuse_texture);
        let row_color = mix(
            textureLoad(colorset_diffuse_texture, packed_ramp_texel(uv, dimensions, 0u), 0),
            textureLoad(colorset_diffuse_texture, packed_ramp_texel(uv, dimensions, 1u), 0),
            weight,
        );
        // The baked ramp alpha is opaque, so the composed alpha is the diffuse alpha.
        return diffuse * row_color;
    }
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
