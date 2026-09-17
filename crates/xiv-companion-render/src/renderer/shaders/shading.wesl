// Model shader module, part of the `package` WESL package
// (compiled to OUT_DIR/model.wgsl by the build script).
import package::bindings::{FragmentOutput, VertexOutput, camera, decal_sampler, decal_texture, mask_sampler, mask_texture, material};
import package::lighting::{PREVIEW_AMBIENT_SCALE, PREVIEW_DIRECT_DIFFUSE_SCALE, PREVIEW_DIRECT_SPECULAR_SCALE, PREVIEW_KEY_COLOR, PREVIEW_RIM_SCALE, fresnel_schlick, ggx_anisotropic_distribution, ggx_geometry_schlick, hemisphere_irradiance, legacy_camera_reflection_lobe, resolve_anisotropy_frame, studio_environment};
import package::surface::{SurfaceSamples, SurfaceState, resolve_specular_mask_factor, resolve_texture_mip_bias, resolve_uv};

fn resolve_character_family_base(
    input: VertexOutput,
    base: vec3<f32>,
    samples: SurfaceSamples,
    ring_emission: ptr<function, vec3<f32>>,
) -> vec3<f32> {
    // MeddleTools 已连节点（node_configs.py skin/hair/iris/charactertattoo）的第一版
    // 落地；无节点证据的输入（唇形/挑染遮罩通道语义、双眼分侧、g_WhiteEyeColor、
    // FacePaintUVMultiplier/Offset 运行态值）按近似处理并保留 unsupported 摘要。
    var color = base;
    // skin family（Face/Body/BodyJJM 同路径）：diffuse × SkinColor。
    if material.character_skin.a > 0.5 {
        color = color * material.character_skin.rgb;
        // LipColor：按 lipstick 开关与唇形遮罩混入。遮罩无节点证据，第一版用脸
        // 漫反射 alpha（唇区 alpha 非 0），仅脸部件材质由渲染侧填该通道。
        if material.character_lip.a > 0.5 {
            let lip_weight = clamp(material.character_lip.a * samples.base.a, 0.0, 1.0);
            color = mix(color, material.character_lip.rgb, lip_weight);
        }
        // 面妆 decal：UV multiplier/offset 运行态值离线不可得，按恒等 UV（uv0）
        // 采样；缺失时数据层不挂贴图（w = 0），此处不激活。
        if material.character_decal.a > 0.0 && material.character_decal_uv.w > 0.5 {
            let decal_uv = vec2<f32>(
                input.uv0.x * material.character_decal_uv.x + material.character_decal_uv.y,
                input.uv0.y,
            );
            let decal_sample = textureSample(decal_texture, decal_sampler, decal_uv);
            color = mix(
                color,
                decal_sample.rgb * material.character_decal.rgb,
                clamp(decal_sample.a * material.character_decal.a, 0.0, 1.0),
            );
        }
    }
    // hair family：MainColor 乘 diffuse；头发/尾/兔耳无 base 纹理，mask **R 通道**
    // 是发丝明暗渐变（AO，真实纹理解码验证：R=渐变、G=挑染区域、B≈常量、
    // A=R 副本）作明暗细节；脸部 hair 材质（眉毛/睫毛）的 mask R 同为明暗
    // 细节图（眉发丝纹理 + 睫毛区压暗），平色发色会亮成白粉点（敖龙女实证）。
    // MeshColor 挑染区域用 **G 通道**。
    if material.character_main.a > 0.5 {
        let use_mask_detail = material.character_params.x > 0.5 && material.params.w > 0.5;
        let hair_detail = select(vec3<f32>(1.0), vec3<f32>(samples.mask.r), use_mask_detail);
        var hair_color = hair_detail * material.character_main.rgb;
        if material.character_mesh.a > 0.5 && material.params.w > 0.5 {
            let mask_uv = resolve_uv(input, material.uv_sources0.z, material.uv_scroll_masks0.z);
            let hair_mask = textureSampleBias(
                mask_texture,
                mask_sampler,
                mask_uv,
                resolve_texture_mip_bias(),
            );
            hair_color = mix(
                hair_color,
                hair_detail * material.character_mesh.rgb,
                clamp(hair_mask.g * material.character_mesh.a, 0.0, 1.0),
            );
        }
        color = hair_color;
    }
    // iris family（MeddleTools iris.shpk 节点组）：单眼材质双眼共享，顶点色
    // G>0.5 选右眼色；diffuse × mix(眼白 g_WhiteEyeColor, 虹膜色, mask.B)。
    // 角膜环：d = |uv0 - (0.5,0.5)|，环带在 radius.xy ± fade.xy 内软过渡
    // （内缘 ramp 上、外缘 ramp 下），发射 = 环色 × factor × 每侧 limbal
    // 强度（虹膜色 alpha）× g_IrisRingEmissiveIntensity。
    var is_right = false;
    if material.character_left_iris.a > 0.5 {
        is_right = input.color.g > 0.5;
        let iris_color = select(
            material.character_left_iris.rgb,
            material.character_right_iris.rgb,
            is_right,
        );
        let eye_tint = mix(
            material.iris_white_eye.rgb,
            iris_color,
            clamp(samples.mask.b, 0.0, 1.0),
        );
        color = color * eye_tint;
    }
    if material.iris_ring_color.a > 0.5 {
        let d = length(input.uv0 - vec2<f32>(0.5, 0.5));
        let radius = material.iris_ring_a.yz;
        let fade = material.iris_ring_b.xy;
        let inner = max(radius.x - fade.x, 0.0);
        let outer = radius.y + fade.y;
        let ramp_in = clamp((d - inner) / max(radius.x - inner, 1e-5), 0.0, 1.0);
        let ramp_out = clamp((outer - d) / max(outer - radius.y, 1e-5), 0.0, 1.0);
        let gate_in = select(0.0, 1.0, d < radius.y);
        let gate_out = select(0.0, 1.0, d > radius.x);
        let factor = clamp(ramp_in * gate_in + ramp_out * gate_out, 0.0, 1.0);
        let limbal = select(material.iris_ring_a.w, material.iris_ring_b.z, is_right);
        *ring_emission += material.iris_ring_color.rgb
            * (factor * limbal * max(material.iris_ring_a.x, 0.0));
    }
    // charactertattoo family：OptionColor 平铺（形状由法线贴图 alpha 的透明策略给）。
    if material.character_option.a > 0.5 {
        color = material.character_option.rgb;
    }
    return color;
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
        + surface.color_table_emissive * color_table_emissive_scale
        + surface.ring_emission;

    var out: FragmentOutput;
    out.color = vec4<f32>(color, surface.alpha);
    return out;
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
