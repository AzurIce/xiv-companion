// Model shader module, part of the `package` WESL package
// (compiled to OUT_DIR/model.wgsl by the build script).
import package::bindings::{material};

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
