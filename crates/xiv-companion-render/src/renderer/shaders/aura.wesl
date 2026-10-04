import package::bindings::{VertexOutput, camera, surface_overlay, surface_overlay_sampler, surface_overlay_texture};

struct AuraSurfaceColor {
    main: vec3<f32>,
    emissive: vec3<f32>,
};

fn aura_projection(position: vec2<f32>, rows: array<vec4<f32>, 2>, offset: vec2<f32>) -> vec2<f32> {
    let point = vec3<f32>(position, 1.0);
    return vec2<f32>(dot(rows[0].xyz, point), dot(rows[1].xyz, point)) + offset;
}

fn aura_triplanar(
    position: vec3<f32>,
    weights: vec3<f32>,
    rows: array<vec4<f32>, 2>,
    offset: vec2<f32>,
    layer: i32,
) -> vec4<f32> {
    let yz = textureSample(surface_overlay_texture, surface_overlay_sampler,
        aura_projection(position.zy, rows, offset), layer);
    let xz = textureSample(surface_overlay_texture, surface_overlay_sampler,
        aura_projection(position.xz, rows, offset), layer);
    let xy = textureSample(surface_overlay_texture, surface_overlay_sampler,
        aura_projection(position.xy, rows, offset), layer);
    return yz * weights.x + xz * weights.y + xy * weights.z;
}

fn aura_color_to_alpha(sampled: vec4<f32>, enabled: bool) -> vec4<f32> {
    if enabled {
        return vec4<f32>(1.0, 1.0, 1.0,
            dot(sampled.rgb, vec3<f32>(0.298912, 0.586611, 0.114478)));
    }
    return sampled;
}

fn resolve_aura_surface_color(
    input: VertexOutput,
    normal: vec3<f32>,
    base: vec3<f32>,
    original_emissive: vec3<f32>,
) -> AuraSurfaceColor {
    let p = surface_overlay.aura_params;
    if p[15].w < 0.5 {
        return AuraSurfaceColor(base, original_emissive);
    }

    let source_weights = select(abs(normal), p[7].xyz, p[7].w > 0.0);
    let weight_length = length(source_weights);
    let weights = select(vec3<f32>(1.0 / 3.0), source_weights / max(weight_length, 1e-6), weight_length > 1e-6);
    let position = input.model_position;
    var distortion = vec2<f32>(0.0);
    if p[3].w > 0.0 {
        let td = aura_triplanar(position, weights, array<vec4<f32>, 2>(p[12], p[13]), vec2<f32>(0.0), 2);
        distortion = td.rg * p[3].w;
    }
    let tc2 = aura_color_to_alpha(
        aura_triplanar(position, weights, array<vec4<f32>, 2>(p[8], p[9]), distortion * p[6].z, 0),
        bitcast<u32>(p[14].x) != 0u,
    );
    let tc3 = aura_color_to_alpha(
        aura_triplanar(position, weights, array<vec4<f32>, 2>(p[10], p[11]), distortion * p[6].w, 1),
        bitcast<u32>(p[14].y) != 0u,
    );
    let tc2_color = select(vec3<f32>(1.0), tc2.rgb, p[15].y != 0.0);
    let tc2_alpha = select(1.0, tc2.a, bitcast<u32>(p[14].w) != 0u);
    let c = p[4];
    let e = p[6];
    var aura_texture_color = tc2_color * tc3.rgb * c.x
        + (tc2_color + tc3.rgb) * c.y
        + (tc3.rgb - tc2_color) * c.z
        + max(tc2_color, tc3.rgb) * c.w
        + min(tc2_color, tc3.rgb) * e.x;
    if c.x + c.y + c.z + c.w + e.x < 1.0 {
        aura_texture_color += tc2_color;
    }
    var aura_texture_alpha = tc2_alpha * tc3.a;
    let alpha_mode = bitcast<u32>(p[14].z);
    if alpha_mode == 1u {
        aura_texture_alpha = max(tc2_alpha, tc3.a);
    } else if alpha_mode == 2u {
        aura_texture_alpha = min(tc2_alpha, tc3.a);
    } else if alpha_mode != 0u {
        aura_texture_alpha = tc2_alpha;
    }

    let direction_source = select(p[2].xyz, p[2].xyz - input.world_position, p[3].x > 0.5);
    let direction = direction_source / max(length(direction_source), 1e-6);
    let fresnel = min(pow(max(abs(dot(normal, direction)), 1e-5), p[2].w), 1.0);
    let aura_color = mix(p[0], p[1], fresnel);
    let aura_rgb = aura_texture_color * aura_color.rgb;
    let alpha = clamp(aura_texture_alpha * aura_color.a, 0.0, 1.0);

    let m = p[5];
    var combined = mix(base, aura_rgb, alpha) * m.x
        + mix(base, base * aura_rgb, alpha) * m.y
        + (base + aura_rgb * alpha) * m.z
        + (base - aura_rgb * alpha) * m.w
        + (base * (vec3<f32>(1.0) - aura_rgb * alpha) + aura_rgb * alpha) * e.y;
    if m.x + m.y + m.z + m.w + e.y < 1.0 {
        combined += base;
    }
    var emissive = original_emissive;
    if camera.instance_env_parameter.x > 0.0 {
        let emission_strength = mix(p[3].y, p[3].z, fresnel) * p[15].x;
        emissive = select(
            mix(original_emissive, aura_rgb, emission_strength),
            original_emissive + aura_rgb * emission_strength,
            i32(p[15].z) != 0,
        );
    }
    return AuraSurfaceColor(mix(base, combined, p[15].x), emissive);
}
