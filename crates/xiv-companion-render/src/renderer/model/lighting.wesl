// Model shader module, part of the `package` WESL package
// (compiled to OUT_DIR/model.wgsl by the build script).
import package::bindings::{VertexOutput, camera};
import package::shading::{resolve_view_direction};

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
