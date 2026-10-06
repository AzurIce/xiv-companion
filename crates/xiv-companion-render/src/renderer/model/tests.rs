use super::*;

use crate::{
    MaterialShaderFamily, ModelMeshDrawRole, PreparedMaterialFeatureFlags,
    PreparedMaterialResourceAvailability, PreparedMaterialRuntimeFallbacks,
    PreparedMaterialRuntimeInputRequirements, PreparedMaterialUnsupportedInputs,
    PreparedMaterialUvSources, PreparedTextureAddressMode, PreparedTextureBindings,
    PreparedTextureColorSpace, PreparedTextureFilter, PreparedTextureSampling,
    PreparedTextureSamplingSet, PreparedTextureScrollSet, PreparedTextureUvSources,
    PreparedUvSource, prepare_material_for_draw_role,
};

/// Extract a single function's text (from its `fn` marker to the next
/// top-level `fn`) from the linked WGSL. Robust against declaration
/// reordering introduced by WESL linking.
fn shader_fn_body<'a>(shader: &'a str, name: &str) -> &'a str {
    shader
        .split_once(&format!("fn {name}"))
        .and_then(|(_, rest)| rest.split_once("\nfn "))
        .map(|(body, _)| body)
        .unwrap_or_else(|| panic!("shader is missing fn {name}"))
}

struct ComponentTestModel {
    data: crate::ModelData,
    components: Vec<u16>,
}

impl ModelRenderData for ComponentTestModel {
    fn bounds(&self) -> &crate::ModelBounds {
        &self.data.bounds
    }

    fn materials(&self) -> &[crate::ModelMaterial] {
        &self.data.materials
    }

    fn textures(&self) -> &[crate::ModelTexture] {
        &self.data.textures
    }

    fn meshes(&self) -> &[crate::ModelMesh] {
        &self.data.meshes
    }

    fn mesh_component_index(&self, mesh_index: usize) -> u16 {
        self.components.get(mesh_index).copied().unwrap_or_default()
    }
}

#[test]
fn model_shader_keeps_surface_pipeline_stages_separate() {
    let shader = include_str!(concat!(env!("OUT_DIR"), "/model.wgsl"));
    let fs_main = shader_fn_body(shader, "fs_main");

    for stage in [
        "resolve_surface_samples(input, color_table_blend)",
        "resolve_surface_state(",
        "resolve_surface_output(",
    ] {
        assert!(fs_main.contains(stage), "fs_main must dispatch {stage}");
    }
    for implementation_detail in ["textureSample(", "smoothstep"] {
        assert!(
            !fs_main.contains(implementation_detail),
            "fs_main must not inline {implementation_detail}"
        );
    }
    assert!(
        fs_main.lines().count() < 70,
        "fs_main grew back into a monolith"
    );
    for unsupported_glass_formula in ["glass_params", "glass_tint", "resolve_glass_factors"] {
        assert!(
            !shader.contains(unsupported_glass_formula),
            "shader reintroduced unsupported glass formula {unsupported_glass_formula}"
        );
    }
}

#[test]
fn model_shader_keeps_modern_colortable_shaping_explicit() {
    let shader = include_str!(concat!(env!("OUT_DIR"), "/model.wgsl"));
    assert!(
        shader.contains("let base_weight = 1.0 - clamp(source_blend, 0.0, 1.0);"),
        "ColorTable blend must retain the installed inverted index weight"
    );
    assert!(
        shader.contains("let view_term = clamp(1.0 - abs(dot(primary_world, view)), 0.0, 1.0);"),
        "modern shaping must use the primary world-normal/view term"
    );
    assert!(
        shader.contains("(1.0 - pow(view_term, 1.0 / shaping_anisotropy)) * base_weight"),
        "modern shaping must preserve the installed anisotropy exponent"
    );
    assert!(
        shader.contains(
            "let shaping_anisotropy = max(load_packed_specular(specular_uv, 0u).a, 0.0);",
        ) && shader.contains("1.0 / shaping_anisotropy"),
        "modern shaping exponent must come from installed A-row specular alpha"
    );
    assert!(
        shader.contains("material.tile_lod_params.z <= 0.5"),
        "modern shaping must remain an explicit material/package capability"
    );
}

#[test]
fn model_shader_keeps_unverified_vertex_rgb_out_of_surface_composition() {
    let shader = include_str!(concat!(env!("OUT_DIR"), "/model.wgsl"));
    let surface_state = shader
        .split_once("fn resolve_surface_state")
        .and_then(|(_, rest)| rest.split_once("fn fresnel_schlick"))
        .map(|(section, _)| section)
        .expect("surface-state section");

    assert!(
        !surface_state.contains("input.color.rgb"),
        "ApplyVertexColor lacks a verified RGB composition formula and must not tint Final"
    );
    assert!(
        shader.contains("color = input.color.rgb;"),
        "direct VertexColor debug must remain available"
    );
}

#[test]
fn model_shader_keeps_unverified_specular_color_mask_out_of_final() {
    let shader = include_str!(concat!(env!("OUT_DIR"), "/model.wgsl"));
    assert!(
        shader.contains("specular_color_mask: vec4<f32>"),
        "the parsed constant must remain available in the material uniform"
    );
    assert!(
        !shader.contains("material.specular_color_mask"),
        "neither RGB multiplication nor a fabricated Alpha scalar has verified composition evidence"
    );
}

#[test]
fn model_shader_limits_texture_mip_bias_to_verified_character_samplers() {
    let shader = include_str!(concat!(env!("OUT_DIR"), "/model.wgsl"));
    let samples = shader
        .split_once("fn resolve_surface_samples")
        .and_then(|(_, rest)| rest.split_once("struct SurfaceState"))
        .map(|(section, _)| section)
        .expect("surface sample section");
    let dither = shader
        .split_once("fn fs_dither_depth")
        .and_then(|(_, rest)| rest.split_once("@fragment\nfn fs_main"))
        .map(|(section, _)| section)
        .expect("dither depth section");
    let lightshaft = shader_fn_body(shader, "fs_lightshaft");
    let properties = shader
        .split_once("fn resolve_material_properties")
        .and_then(|(_, rest)| rest.split_once("fn resolve_specular_mask_factor"))
        .map(|(section, _)| section)
        .expect("material properties section");

    for required in [
        "out.normal = textureSampleBias(normal_texture",
        "return textureSampleBias(base_color_texture",
        "return textureSampleBias(mask_texture",
        "return select(0.0, clamp(material.surface_params.y, -16.0, 15.99), material.surface_params.w > 0.5)",
    ] {
        assert!(
            shader.contains(required),
            "verified character mip-bias path must preserve {required}"
        );
    }
    for unverified in [
        "out.secondary_normal = textureSampleBias",
        "out.specular = textureSampleBias",
        "out.secondary_specular = textureSampleBias",
        "out.secondary_base = textureSampleBias",
        "out.emissive = textureSampleBias",
    ] {
        assert!(
            !samples.contains(unverified),
            "g_TextureMipBias must not expand to unverified sampler role {unverified}"
        );
    }
    assert!(!dither.contains("sampled_secondary_base = textureSampleBias"));
    assert!(!lightshaft.contains("textureSampleBias"));
    assert!(!properties.contains("textureSampleBias"));
}

#[test]
fn model_shader_composes_colorset_diffuse_at_source_resolution() {
    let shader = include_str!(concat!(env!("OUT_DIR"), "/model.wgsl"));
    let compose = shader
        .split_once("fn sample_color_table_base")
        .and_then(|(_, rest)| rest.split_once("fn sample_color_table_specular"))
        .map(|(section, _)| section)
        .expect("color table base sample section");

    for required in [
        "material.colorset_params.x > 0.5",
        "textureSampleBias(base_color_texture, base_color_sampler, uv, mip_bias)",
        "textureDimensions(colorset_diffuse_texture)",
        "textureLoad(colorset_diffuse_texture, packed_ramp_texel(uv, dimensions, 0u), 0)",
        "textureLoad(colorset_diffuse_texture, packed_ramp_texel(uv, dimensions, 1u), 0)",
        "return diffuse * row_color;",
    ] {
        assert!(
            compose.contains(required),
            "Compatibility base × colorset must compose in the shader: {required}"
        );
    }
    // The A/B ramp texel addressing stays the deliberate floor + mip 0 form.
    assert!(shader.contains("fn packed_ramp_texel"));
    assert!(
        !compose.contains("textureLoad(base_color_texture"),
        "the full-resolution diffuse must use the filtered sampler path"
    );
}

#[test]
fn model_shader_stays_within_material_binding_budget() {
    let shader = include_str!(concat!(env!("OUT_DIR"), "/model.wgsl"));
    let material_bindings = shader.matches("@group(1) @binding").count();
    let overlay_bindings = shader.matches("@group(3) @binding").count();
    let texture_bindings = shader
        .lines()
        .filter(|line| line.contains("var ") && line.contains("texture_2d"))
        .count();
    let sampler_bindings = shader
        .lines()
        .filter(|line| line.contains("var ") && line.contains(": sampler"))
        .count();
    assert_eq!(
        material_bindings + overlay_bindings,
        texture_bindings + sampler_bindings + 2,
        "model material and overlay bindings must include exactly two uniform buffers"
    );
    assert_eq!(overlay_bindings, 3);
    // Self-imposed conservative budget (formerly the WebGL2 limits; the
    // renderer now requires WebGPU everywhere but keeps the ceiling to
    // stop bind-group sprawl).
    assert!(
        texture_bindings <= 16,
        "material bind group budget allows at most 16 sampled textures per stage, found {texture_bindings}"
    );
    assert!(
        sampler_bindings <= 16,
        "material bind group budget allows at most 16 samplers per stage, found {sampler_bindings}"
    );
    // The freed material-map slot now carries the colorset diffuse ramp.
    assert!(
        shader.contains("var colorset_diffuse_texture: texture_2d<f32>;")
            && !shader.contains("material_map_texture")
            && !shader.contains("material_map_sampler"),
        "binding 15 must be the colorset diffuse ramp and the material map binding must be gone"
    );
}

#[test]
fn model_shader_keeps_face_decal_on_array_layer_zero() {
    let shader = include_str!(concat!(env!("OUT_DIR"), "/model.wgsl"));
    assert!(shader.contains("var surface_overlay_texture: texture_2d_array<f32>;"));
    assert!(
        shader.contains(
            "textureSample(surface_overlay_texture, surface_overlay_sampler, decal_uv, 0)"
        )
    );
}

#[test]
fn aura_targets_only_matching_model_surfaces_without_face_decal() {
    for shader_package in ["skin.shpk", "character.shpk", "characterlegacy.shpk"] {
        let mut material = fallback_material();
        material.shader_package_name = Some(shader_package.to_string());
        let mut mesh = test_mesh("normal", 0.0);
        mesh.path = "chara/weapon/w0001/model/main.mdl#mesh0".to_string();
        let mut model = crate::ModelData {
            bounds: crate::ModelBounds::default(),
            materials: vec![material],
            textures: Vec::new(),
            meshes: vec![mesh],
        };
        let (_, _, batches) = flatten_model(&model);
        assert_eq!(batches[0].model_path, "chara/weapon/w0001/model/main.mdl");
        assert!(batches[0].accepts_aura_target("chara/weapon/w0001/model/main.mdl"));
        assert!(!batches[0].accepts_aura_target("chara/weapon/w0001/model/offhand.mdl"));

        model.materials[0].character_colors = Some(crate::ModelMaterialCharacterColors {
            colors: crate::CharacterAppearanceColors::default(),
            decal_texture: Some(0),
        });
        let (_, _, batches) = flatten_model(&model);
        assert!(!batches[0].accepts_aura_target("chara/weapon/w0001/model/main.mdl"));
    }

    let mut material = fallback_material();
    material.shader_package_name = Some("hair.shpk".to_string());
    let model = crate::ModelData {
        bounds: crate::ModelBounds::default(),
        materials: vec![material],
        textures: Vec::new(),
        meshes: vec![test_mesh("normal", 0.0)],
    };
    let (_, _, batches) = flatten_model(&model);
    assert!(!batches[0].accepts_aura_target("test/normal.mdl"));
}

#[cfg(all(feature = "test-support", not(target_arch = "wasm32")))]
#[test]
#[ignore = "requires a native GPU adapter"]
fn face_decal_survives_array_slot_in_model_pipeline() {
    use crate::test_support::{ModelSnapshotOptions, render_model_snapshot_with_options};

    let mut material = fallback_material();
    material.shader_package_name = Some("skin.shpk".to_string());
    material.path =
        Some("chara/human/c0101/obj/face/f0001/material/mt_c0101f0001_fac_a.mtrl".to_string());
    material.character_colors = Some(crate::ModelMaterialCharacterColors {
        colors: crate::CharacterAppearanceColors {
            skin: [1.0; 4],
            decal: [1.0, 0.0, 0.0, 1.0],
            ..Default::default()
        },
        decal_texture: Some(0),
    });
    let mut mesh = test_mesh("normal", 0.0);
    for (vertex, position) in
        mesh.vertices
            .iter_mut()
            .zip([[-0.5, -0.5, 0.0], [0.5, -0.5, 0.0], [0.0, 0.5, 0.0]])
    {
        vertex.position = position;
        vertex.normal = [0.0, 0.0, 1.0];
    }
    let mut decal = test_texture(crate::ModelTextureKind::Other);
    decal.rgba = vec![255, 0, 0, 255];
    let mut model = crate::ModelData {
        bounds: crate::ModelBounds::default(),
        materials: vec![material],
        textures: vec![decal],
        meshes: vec![mesh],
    };
    let options = ModelSnapshotOptions::new("face-decal-array-slot-on")
        .with_output_dir("target/weapon-vfx-audit/model-skin-shaders")
        .with_viewport(96, 96)
        .with_hdr_scene_capture();
    let with_decal = render_model_snapshot_with_options(options.clone(), &model)
        .expect("render face decal through the model pipeline")
        .hdr_scene_rgba
        .expect("HDR face decal snapshot");
    model.materials[0]
        .character_colors
        .as_mut()
        .unwrap()
        .decal_texture = None;
    let without_decal = render_model_snapshot_with_options(
        ModelSnapshotOptions {
            name: "face-decal-array-slot-off".to_string(),
            ..options
        },
        &model,
    )
    .expect("render face without decal through the model pipeline")
    .hdr_scene_rgba
    .expect("HDR face baseline snapshot");
    assert!(
        with_decal
            .iter()
            .zip(&without_decal)
            .any(|(on, off)| { on[0] > off[0] + 0.02 && on[1] < off[1] - 0.02 }),
        "the face decal must change actual model pixels"
    );
}

#[cfg(all(feature = "test-support", not(target_arch = "wasm32")))]
#[test]
#[ignore = "requires a native GPU adapter"]
fn aura_bind_group_can_draw_matching_model_surface() {
    use crate::test_support::{ModelSnapshotOptions, render_model_snapshot_with_options};
    use xiv_companion_data::{
        AvfxColorCurve, AvfxCurve, AvfxCurveKey, AvfxEmitter, AvfxEmitterItem, AvfxFile,
        AvfxParticle, AvfxParticleData, AvfxParticleDataModelSkin, AvfxParticleTexture,
        AvfxTimeline, AvfxTimelineItem, AvfxUvSet, EmitterType, ParticleType, VfxPlayback,
        VfxTextureRgba, WeaponVfxAttachment, WeaponVfxAttachments, WeaponVfxData,
        WeaponVfxModelTargets,
    };

    let constant = |value| AvfxCurve {
        keys: vec![AvfxCurveKey {
            time: 0,
            interpolation: 1,
            x: 0.0,
            y: 0.0,
            z: value,
        }],
        ..Default::default()
    };

    let mut material = fallback_material();
    material.shader_package_name = Some("character.shpk".into());
    let mut mesh = test_mesh("normal", 0.0);
    mesh.path = "target.mdl#mesh0".into();
    for (vertex, position) in
        mesh.vertices
            .iter_mut()
            .zip([[-0.5, -0.5, 0.0], [0.5, -0.5, 0.0], [0.0, 0.5, 0.0]])
    {
        vertex.position = position;
        vertex.normal = [0.0, 0.0, 1.0];
    }
    let mut model = crate::ModelData {
        bounds: crate::ModelBounds::default(),
        materials: vec![material],
        textures: Vec::new(),
        meshes: vec![mesh],
    };
    let mounts = WeaponVfxAttachments {
        attachments: vec![WeaponVfxAttachment {
            model_path: "target.mdl".into(),
            data: WeaponVfxData {
                avfx_path: "aura.avfx".into(),
                file: AvfxFile {
                    timelines: vec![AvfxTimeline {
                        binder_index: -1,
                        items: vec![AvfxTimelineItem {
                            enabled: true,
                            start_time: 0,
                            end_time: -1,
                            binder_index: -1,
                            effector_index: -1,
                            emitter_index: 0,
                            platform: 0,
                            clip_index: -1,
                        }],
                        ..Default::default()
                    }],
                    emitters: vec![AvfxEmitter {
                        emitter_type: Some(EmitterType::Point),
                        effector_index: -1,
                        create_count: constant(1.0),
                        create_interval: constant(100.0),
                        particle_items: vec![AvfxEmitterItem {
                            enabled: true,
                            target_index: 0,
                            parameter_link: -1,
                            create_probability: 100,
                            create_count: 1,
                            ..Default::default()
                        }],
                        ..Default::default()
                    }],
                    particles: vec![AvfxParticle {
                        particle_type: Some(ParticleType::ModelSkin),
                        collision_type: -1,
                        data: AvfxParticleData::ModelSkin(AvfxParticleDataModelSkin {
                            fresnel_type: 1,
                            aura_target: 2,
                            sem: constant(1.0),
                            eem: constant(1.0),
                            color_begin: AvfxColorCurve {
                                alpha: Some(constant(0.5)),
                                ..Default::default()
                            },
                            color_end: AvfxColorCurve {
                                alpha: Some(constant(0.5)),
                                ..Default::default()
                            },
                            ..Default::default()
                        }),
                        uv_sets: vec![AvfxUvSet::default()],
                        texture_color2: Some(AvfxParticleTexture {
                            enabled: true,
                            texture_index: 0,
                            ..Default::default()
                        }),
                        ..Default::default()
                    }],
                    texture_paths: vec!["aura.atex".into()],
                    ..Default::default()
                },
                textures: vec![Some(VfxTextureRgba {
                    width: 1,
                    height: 1,
                    rgba: vec![255, 0, 0, 255],
                    source_mip_count: 1,
                    ..Default::default()
                })],
                ..Default::default()
            },
        }],
        model_skin_targets: WeaponVfxModelTargets {
            weapon: Some("target.mdl".into()),
            ..Default::default()
        },
        ..Default::default()
    };
    let mut playback = VfxPlayback::new_with_model_skin(mounts.attachments[0].data.runtime(), &[0]);
    assert_eq!(playback.fallback_reason(), None);
    assert!(playback.update_to(0.1));
    assert_eq!(playback.model_skin_instances().len(), 1);
    for family in ["skin", "character", "characterlegacy"] {
        model.materials[0].shader_package_name = Some(format!("{family}.shpk"));
        let options = ModelSnapshotOptions::new(format!("aura-{family}-off"))
            .with_output_dir("target/weapon-vfx-audit/model-skin-shaders")
            .with_viewport(96, 96)
            .with_hdr_scene_capture()
            .with_weapon_vfx(mounts.clone(), 0.1);
        let baseline = render_model_snapshot_with_options(options.clone(), &model)
            .expect("draw model with default overlay bind group")
            .hdr_scene_rgba
            .unwrap();
        let bound = render_model_snapshot_with_options(
            ModelSnapshotOptions {
                name: format!("aura-{family}-on"),
                ..options.clone().with_aura_definition(0, 0)
            },
            &model,
        )
        .expect("draw model with target Aura bind group")
        .hdr_scene_rgba
        .unwrap();
        assert!(baseline.iter().any(|pixel| pixel[0] > 0.1));
        let changed = baseline
            .iter()
            .zip(&bound)
            .filter(|(before, after)| (before[1] - after[1]).abs() > 0.05)
            .count();
        assert!(changed > 20, "active red Aura must change {family} pixels");
        assert!(
            baseline
                .iter()
                .zip(&bound)
                .all(|(before, after)| (before[3] - after[3]).abs() < 1e-3)
        );
        let emissive = render_model_snapshot_with_options(
            ModelSnapshotOptions {
                name: format!("aura-{family}-emissive"),
                ..options
                    .with_aura_definition(0, 0)
                    .with_render_options(ModelRenderOptions {
                        instance_env_parameter_w: 1.0,
                        ..Default::default()
                    })
            },
            &model,
        )
        .expect("draw Aura with the runtime emissive gate")
        .hdr_scene_rgba
        .unwrap();
        let brighter = bound
            .iter()
            .zip(&emissive)
            .filter(|(off, on)| on[0] > off[0] + 0.05)
            .count();
        assert!(
            brighter > 20,
            "SEM/EEM must brighten {family} pixels only when enabled"
        );
        assert!(
            bound
                .iter()
                .zip(&emissive)
                .all(|(off, on)| (off[3] - on[3]).abs() < 1e-3)
        );
    }

    let mut dual_model = model.clone();
    let mut offhand_mesh = dual_model.meshes[0].clone();
    offhand_mesh.path = "offhand.mdl#mesh0".into();
    for vertex in &mut dual_model.meshes[0].vertices {
        vertex.position[0] -= 0.55;
    }
    for vertex in &mut offhand_mesh.vertices {
        vertex.position[0] += 0.55;
    }
    dual_model.meshes.push(offhand_mesh);
    let mut dual_mounts = mounts.clone();
    dual_mounts.model_skin_targets.off_hand = Some("offhand.mdl".into());
    let attachment = &mut dual_mounts.attachments[0].data;
    let mut offhand_particle = attachment.file.particles[0].clone();
    if let AvfxParticleData::ModelSkin(data) = &mut offhand_particle.data {
        data.aura_target = 4;
    }
    offhand_particle
        .texture_color2
        .as_mut()
        .unwrap()
        .texture_index = 1;
    attachment.file.particles.push(offhand_particle);
    let mut offhand_item = attachment.file.emitters[0].particle_items[0];
    offhand_item.target_index = 1;
    attachment.file.emitters[0]
        .particle_items
        .push(offhand_item);
    attachment
        .file
        .texture_paths
        .push("offhand-aura.atex".into());
    attachment.textures.push(Some(VfxTextureRgba {
        width: 1,
        height: 1,
        rgba: vec![0, 0, 255, 255],
        source_mip_count: 1,
        ..Default::default()
    }));
    let dual_options = ModelSnapshotOptions::new("aura-independent-targets-off")
        .with_output_dir("target/weapon-vfx-audit/model-skin-shaders")
        .with_viewport(128, 96)
        .with_camera(0.0, 0.0, 3.2, [0.0; 2])
        .with_hdr_scene_capture()
        .with_weapon_vfx(dual_mounts, 0.1);
    let dual_baseline = render_model_snapshot_with_options(dual_options.clone(), &dual_model)
        .expect("draw both model targets without Aura")
        .hdr_scene_rgba
        .unwrap();
    let dual_bound = render_model_snapshot_with_options(
        ModelSnapshotOptions {
            name: "aura-independent-targets-on".into(),
            ..dual_options.with_all_unambiguous_auras()
        },
        &dual_model,
    )
    .expect("draw both independent Aura targets")
    .hdr_scene_rgba
    .unwrap();
    for half in 0..2 {
        let changed = dual_baseline
            .iter()
            .zip(&dual_bound)
            .enumerate()
            .filter(|(index, (before, after))| {
                (index % 128) / 64 == half
                    && (before[0] - after[0]).abs() + (before[2] - after[2]).abs() > 0.05
            })
            .count();
        assert!(changed > 20, "Aura must change model pixels in half {half}");
    }

    model.materials[0].shader_package_name = Some("character.shpk".into());
    let render_mode3 = |angle| {
        let mut attachment = mounts.clone();
        let particle = &mut attachment.attachments[0].data.file.particles[0];
        particle.rotation.y = Some(constant(angle));
        let AvfxParticleData::ModelSkin(data) = &mut particle.data else {
            unreachable!()
        };
        data.fresnel_type = 3;
        data.fresnel_curve = constant(1.0);
        data.color_begin.scale_rgb = Some(xiv_companion_data::AvfxColorScaleRgb {
            r: Some(constant(0.0)),
            ..Default::default()
        });
        render_model_snapshot_with_options(
            ModelSnapshotOptions::new(format!("aura-mode3-angle-{angle}"))
                .with_output_dir("target/weapon-vfx-audit/model-skin-shaders")
                .with_viewport(96, 96)
                .with_hdr_scene_capture()
                .with_weapon_vfx(attachment, 0.1)
                .with_aura_definition(0, 0),
            &model,
        )
        .expect("draw ModelSkin with instance-relative Fresnel direction")
        .hdr_scene_rgba
        .unwrap()
    };
    let mode3_aligned = render_mode3(0.0);
    let mode3_turned = render_mode3(std::f32::consts::FRAC_PI_2);
    assert!(
        mode3_aligned
            .iter()
            .zip(&mode3_turned)
            .filter(|(aligned, turned)| aligned[0] > turned[0] + 0.05)
            .count()
            > 20,
        "instance-relative FrsT=3 must change model pixels when the particle rotates"
    );

    let mut roll_model = model.clone();
    for vertex in &mut roll_model.meshes[0].vertices {
        vertex.normal = [1.0, 0.0, 0.0];
    }
    let render_billboard_aura = |mode, axis: &str, roll, msaa| {
        let mut attachment = mounts.clone();
        let particle = &mut attachment.attachments[0].data.file.particles[0];
        particle.rotation_direction_base = mode;
        let AvfxParticleData::ModelSkin(data) = &mut particle.data else {
            unreachable!()
        };
        data.fresnel_type = 3;
        data.fresnel_curve = constant(1.0);
        match axis {
            "x" => data.fresnel_rotation.y = Some(constant(std::f32::consts::FRAC_PI_2)),
            "y" => data.fresnel_rotation.x = Some(constant(std::f32::consts::FRAC_PI_2)),
            _ => unreachable!(),
        }
        data.color_begin.scale_rgb = Some(xiv_companion_data::AvfxColorScaleRgb {
            r: Some(constant(0.0)),
            ..Default::default()
        });
        render_model_snapshot_with_options(
            ModelSnapshotOptions::new(format!(
                "aura-mode3-rbdt-{mode}-{axis}-roll-{roll}-msaa-{msaa}"
            ))
            .with_output_dir("target/weapon-vfx-audit/model-skin-shaders")
            .with_viewport(96, 96)
            .with_camera(0.0, 0.0, 3.2, [0.0; 2])
            .with_render_options(ModelRenderOptions {
                camera_roll: roll,
                msaa_samples: msaa,
                ..Default::default()
            })
            .with_hdr_scene_capture()
            .with_weapon_vfx(attachment, 0.1)
            .with_aura_definition(0, 0),
            &roll_model,
        )
        .expect("draw ModelSkin with a rolled camera and instance-relative Fresnel")
        .hdr_scene_rgba
        .unwrap()
    };
    for msaa in [1, 4] {
        let rolled_screen = render_billboard_aura(5, "x", std::f32::consts::FRAC_PI_2, msaa);
        let rolled_camera = render_billboard_aura(6, "x", std::f32::consts::FRAC_PI_2, msaa);
        let rolled_y = render_billboard_aura(0, "y", std::f32::consts::FRAC_PI_2, msaa);
        let rolled_world_up = render_billboard_aura(0, "x", std::f32::consts::FRAC_PI_2, msaa);
        for actual in [&rolled_screen, &rolled_camera] {
            assert!(
                actual
                    .iter()
                    .zip(&rolled_y)
                    .all(|(a, b)| { a.iter().zip(b).all(|(a, b)| (a - b).abs() < 1e-3) }),
                "RBDT camera basis must match the fixed Fresnel axis at {msaa}x MSAA"
            );
        }
        for actual in [&rolled_screen, &rolled_camera] {
            assert!(
                actual
                    .iter()
                    .zip(&rolled_world_up)
                    .filter(|(a, b)| (a[0] - b[0]).abs() > 0.05)
                    .count()
                    > 20,
                "rolled Aura must differ from the world-up direction at {msaa}x MSAA"
            );
        }
    }

    let render_uvpd = |x, y, msaa| {
        let mut attachment = mounts.clone();
        let AvfxParticleData::ModelSkin(data) =
            &mut attachment.attachments[0].data.file.particles[0].data
        else {
            unreachable!()
        };
        data.fresnel_type = 0;
        data.uv_point_density.x = Some(constant(x));
        data.uv_point_density.y = Some(constant(y));
        render_model_snapshot_with_options(
            ModelSnapshotOptions::new(format!("aura-uvpd-{x}-{y}-msaa-{msaa}"))
                .with_output_dir("target/weapon-vfx-audit/model-skin-shaders")
                .with_viewport(96, 96)
                .with_camera(0.0, 0.0, 3.2, [0.0; 2])
                .with_render_options(ModelRenderOptions {
                    msaa_samples: msaa,
                    ..Default::default()
                })
                .with_hdr_scene_capture()
                .with_weapon_vfx(attachment, 0.1)
                .with_aura_definition(0, 0),
            &model,
        )
        .expect("draw ModelSkin with authored triplanar weights")
        .hdr_scene_rgba
        .unwrap()
    };
    for msaa in [1, 4] {
        let axial = render_uvpd(1.0, 0.0, msaa);
        let diagonal = render_uvpd(0.6, 0.8, msaa);
        assert!(
            axial
                .iter()
                .zip(&diagonal)
                .filter(|(a, b)| a[0] > 0.4 && (b[0] / a[0] - 1.4).abs() < 0.08)
                .count()
                > 20,
            "UVPD diagonal weights must use Euclidean normalization at {msaa}x MSAA"
        );
    }
    let render_normal_weights = |normal: [f32; 3], name| {
        let mut normal_model = model.clone();
        for vertex in &mut normal_model.meshes[0].vertices {
            vertex.normal = normal;
        }
        let mut attachment = mounts.clone();
        let AvfxParticleData::ModelSkin(data) =
            &mut attachment.attachments[0].data.file.particles[0].data
        else {
            unreachable!()
        };
        data.fresnel_type = 0;
        render_model_snapshot_with_options(
            ModelSnapshotOptions::new(name)
                .with_output_dir("target/weapon-vfx-audit/model-skin-shaders")
                .with_viewport(96, 96)
                .with_camera(0.0, 0.0, 3.2, [0.0; 2])
                .with_hdr_scene_capture()
                .with_weapon_vfx(attachment, 0.1)
                .with_aura_definition(0, 0),
            &normal_model,
        )
        .expect("draw ModelSkin with normal-derived triplanar weights")
        .hdr_scene_rgba
        .unwrap()
    };
    let normal_axial = render_normal_weights([1.0, 0.0, 0.0], "aura-normal-axial");
    let normal_diagonal = render_normal_weights([0.6, 0.8, 0.0], "aura-normal-diagonal");
    assert!(
        normal_axial
            .iter()
            .zip(&normal_diagonal)
            .filter(|(a, b)| a[0] > 0.4 && (b[0] / a[0] - 1.4).abs() < 0.08)
            .count()
            > 20,
        "normal-derived triplanar weights must match the client L2 normalization"
    );

    model.materials[0].color_table_rows = Some(vec![crate::ColorTableRowColors::default()]);
    model.materials[0].emissive_texture = Some(0);
    model.materials[0].material_properties_texture = Some(1);
    let mut emissive_texture = test_texture(crate::ModelTextureKind::Emissive);
    emissive_texture.rgba = vec![0, 180, 0, 255];
    let mut properties_texture = test_texture(crate::ModelTextureKind::MaterialProperties);
    properties_texture.rgba = vec![0, 128, 128, 255];
    model.textures = vec![emissive_texture, properties_texture];
    let render_mode = |cm, enabled, dynamic_green| {
        let mut attachment = mounts.clone();
        let AvfxParticleData::ModelSkin(data) =
            &mut attachment.attachments[0].data.file.particles[0].data
        else {
            unreachable!()
        };
        data.cm = cm;
        render_model_snapshot_with_options(
            ModelSnapshotOptions::new(format!(
                "aura-cm-{cm}-gate-{enabled}-dynamic-{dynamic_green}"
            ))
            .with_output_dir("target/weapon-vfx-audit/model-skin-shaders")
            .with_viewport(96, 96)
            .with_hdr_scene_capture()
            .with_weapon_vfx(attachment, 0.1)
            .with_aura_definition(0, 0)
            .with_render_options(ModelRenderOptions {
                instance_env_parameter_w: f32::from(enabled),
                dynamic_emissive_color: [1.0, dynamic_green, 1.0],
                ..Default::default()
            }),
            &model,
        )
        .expect("draw Character Aura emissive combine mode")
        .hdr_scene_rgba
        .unwrap()
    };
    let replace_off = render_mode(0, false, 1.0);
    let add_off = render_mode(1, false, 1.0);
    assert!(
        replace_off
            .iter()
            .zip(&add_off)
            .all(|(a, b)| { a.iter().zip(b).all(|(a, b)| (a - b).abs() < 1e-3) })
    );
    let replace_on = render_mode(0, true, 1.0);
    let add_on = render_mode(1, true, 1.0);
    let brighter = replace_on
        .iter()
        .zip(&add_on)
        .filter(|(replace, add)| add[1] > replace[1] + 0.05)
        .count();
    assert!(
        brighter > 20,
        "bCM=1 must add existing emissive instead of replacing it"
    );
    let replace_dynamic = render_mode(0, true, 2.0);
    assert!(
        replace_on
            .iter()
            .zip(&replace_dynamic)
            .all(|(a, b)| { a.iter().zip(b).all(|(a, b)| (a - b).abs() < 1e-3) }),
        "bCM=0 at full strength must not retain dynamically scaled source emissive"
    );
}

#[cfg(all(feature = "test-support", not(target_arch = "wasm32")))]
#[test]
#[ignore = "requires a native GPU adapter"]
fn aura_first_plane_uses_zy_model_position() {
    use crate::test_support::{ModelSnapshotOptions, render_model_snapshot_with_options};
    use xiv_companion_data::{
        AvfxColorCurve, AvfxCurve, AvfxCurveKey, AvfxEmitter, AvfxEmitterItem, AvfxFile,
        AvfxParticle, AvfxParticleData, AvfxParticleDataModelSkin, AvfxParticleTexture,
        AvfxTimeline, AvfxTimelineItem, AvfxUvSet, EmitterType, ParticleType, VfxTextureRgba,
        WeaponVfxAttachment, WeaponVfxAttachments, WeaponVfxData, WeaponVfxModelTargets,
    };

    let constant = |value| AvfxCurve {
        keys: vec![AvfxCurveKey {
            time: 0,
            interpolation: 1,
            x: 0.0,
            y: 0.0,
            z: value,
        }],
        ..Default::default()
    };
    let mut material = fallback_material();
    material.shader_package_name = Some("character.shpk".into());
    let mut mesh = test_mesh("normal", 0.0);
    mesh.path = "target.mdl#mesh0".into();
    for (vertex, position) in
        mesh.vertices
            .iter_mut()
            .zip([[-0.5, -0.5, 0.0], [0.5, -0.5, 0.0], [0.0, 0.5, 0.0]])
    {
        vertex.position = position;
        vertex.normal = [0.0, 0.0, 1.0];
    }
    let model = crate::ModelData {
        bounds: crate::ModelBounds::default(),
        materials: vec![material],
        textures: Vec::new(),
        meshes: vec![mesh],
    };
    let mut mounts = WeaponVfxAttachments {
        attachments: vec![WeaponVfxAttachment {
            model_path: "target.mdl".into(),
            data: WeaponVfxData {
                avfx_path: "aura-zy.avfx".into(),
                file: AvfxFile {
                    timelines: vec![AvfxTimeline {
                        binder_index: -1,
                        items: vec![AvfxTimelineItem {
                            enabled: true,
                            start_time: 0,
                            end_time: -1,
                            binder_index: -1,
                            effector_index: -1,
                            emitter_index: 0,
                            platform: 0,
                            clip_index: -1,
                        }],
                        ..Default::default()
                    }],
                    emitters: vec![AvfxEmitter {
                        emitter_type: Some(EmitterType::Point),
                        effector_index: -1,
                        create_count: constant(1.0),
                        create_interval: constant(100.0),
                        particle_items: vec![AvfxEmitterItem {
                            enabled: true,
                            target_index: 0,
                            parameter_link: -1,
                            create_probability: 100,
                            create_count: 1,
                            ..Default::default()
                        }],
                        ..Default::default()
                    }],
                    particles: vec![AvfxParticle {
                        particle_type: Some(ParticleType::ModelSkin),
                        collision_type: -1,
                        data: AvfxParticleData::ModelSkin(AvfxParticleDataModelSkin {
                            fresnel_type: 1,
                            aura_target: 2,
                            color_begin: AvfxColorCurve {
                                alpha: Some(constant(0.5)),
                                ..Default::default()
                            },
                            color_end: AvfxColorCurve {
                                alpha: Some(constant(0.5)),
                                ..Default::default()
                            },
                            ..Default::default()
                        }),
                        uv_sets: vec![AvfxUvSet::default()],
                        texture_color2: Some(AvfxParticleTexture {
                            enabled: true,
                            texture_index: 0,
                            ..Default::default()
                        }),
                        ..Default::default()
                    }],
                    texture_paths: vec!["aura-zy.atex".into()],
                    ..Default::default()
                },
                textures: vec![None],
                ..Default::default()
            },
        }],
        model_skin_targets: WeaponVfxModelTargets {
            weapon: Some("target.mdl".into()),
            ..Default::default()
        },
        ..Default::default()
    };
    let AvfxParticleData::ModelSkin(data) = &mut mounts.attachments[0].data.file.particles[0].data
    else {
        unreachable!()
    };
    data.uv_point_density.x = Some(constant(1.0));

    let render = |name, left_red: u8, right_red: u8| {
        let mut attachment = mounts.clone();
        attachment.attachments[0].data.textures[0] = Some(VfxTextureRgba {
            width: 2,
            height: 2,
            rgba: [
                [left_red, 0, 0, 255],
                [right_red, 0, 0, 255],
                [left_red, 0, 0, 255],
                [right_red, 0, 0, 255],
            ]
            .into_iter()
            .flatten()
            .collect(),
            source_mip_count: 1,
            ..Default::default()
        });
        render_model_snapshot_with_options(
            ModelSnapshotOptions::new(name)
                .with_output_dir("target/weapon-vfx-audit/model-skin-shaders")
                .with_viewport(96, 96)
                .with_hdr_scene_capture()
                .with_weapon_vfx(attachment, 0.1)
                .with_aura_definition(0, 0),
            &model,
        )
        .expect("draw ModelSkin with an asymmetric first-plane texture")
        .hdr_scene_rgba
        .unwrap()
    };
    let gradient = render("aura-zy-gradient", 0, 255);
    let constant = render("aura-zy-constant", 0, 0);
    let control = render("aura-zy-control", 255, 255);
    assert!(
        constant
            .iter()
            .zip(&control)
            .filter(|(a, b)| (a[0] - b[0]).abs() > 0.05)
            .count()
            > 20,
        "the asymmetric texture fixture must affect visible Aura pixels"
    );
    assert!(
        gradient
            .iter()
            .zip(&constant)
            .all(|(a, b)| a.iter().zip(b).all(|(a, b)| (a - b).abs() < 1e-3)),
        "the first Aura plane must use (z, y), so constant z cannot sample the right column"
    );
}

#[test]
fn model_aura_uses_unskinned_position_in_projection() {
    let shader = include_str!(concat!(env!("OUT_DIR"), "/model.wgsl"));
    assert!(shader.contains("out.model_position = input.position;"));
    let aura = shader_fn_body(shader, "resolve_aura_surface_color");
    assert!(aura.contains("let position = input.model_position;"));
    assert!(aura.contains("bitcast<u32>(p[14].x)"));
    assert!(aura.contains("bitcast<u32>(p[14].z)"));
}

#[test]
fn model_aura_triplanar_weights_use_euclidean_length() {
    let shader = include_str!(concat!(env!("OUT_DIR"), "/model.wgsl"));
    let aura = shader_fn_body(shader, "resolve_aura_surface_color");
    assert!(aura.contains("let weight_length = length(source_weights);"));
    assert!(aura.contains("source_weights / max(weight_length, 1e-6)"));
}

#[test]
fn model_aura_projection_uses_client_plane_axis_order() {
    let shader = include_str!(concat!(env!("OUT_DIR"), "/model.wgsl"));
    let projection = shader_fn_body(shader, "aura_triplanar");
    for plane in ["position.zy", "position.xz", "position.xy"] {
        assert!(
            projection.contains(plane),
            "the client Aura projection must preserve {plane}"
        );
    }
}

#[test]
fn model_shader_applies_verified_tile_mip_bias_only_to_tile_arrays() {
    let shader = include_str!(concat!(env!("OUT_DIR"), "/model.wgsl"));
    let tile = shader
        .split_once("fn resolve_tile_array")
        .and_then(|(_, rest)| rest.split_once("fn tile_array_layer"))
        .map(|(section, _)| section)
        .expect("tile array section");
    let detail = shader
        .split_once("fn resolve_detail_array")
        .and_then(|(_, rest)| rest.split_once("struct PairAtlasCoordinates"))
        .map(|(section, _)| section)
        .expect("detail array section");

    for required in [
        "min(length(tile_matrix.xz), length(tile_matrix.yw)) * 0.0078125",
        "max(log2(max(matrix_scale, 1e-8)), 0.0)",
        "matrix_bias + material.tile_lod_params.x",
        "normal_coordinates_a.ddx * exp2(resolve_tile_lod_bias(extra.tile_matrix_a))",
        "normal_coordinates_b.ddx * exp2(resolve_tile_lod_bias(extra.tile_matrix_b))",
        "orb_coordinates_a.ddx * exp2(resolve_tile_lod_bias(extra.tile_matrix_a))",
        "orb_coordinates_b.ddx * exp2(resolve_tile_lod_bias(extra.tile_matrix_b))",
        "out.normal = normalize(mix(normal_a, normal_b, ramp_blend))",
        "out.orb = mix(",
    ] {
        assert!(
            shader.contains(required),
            "verified TileMipBiasOffset path must preserve {required}"
        );
    }
    assert!(tile.contains("textureSampleGrad"));
    assert!(
        !detail.contains("resolve_tile_lod_bias") && !detail.contains("material.tile_lod_params"),
        "TileMipBiasOffset must not expand to the BG detail array samplers"
    );
}

#[test]
fn model_shader_uses_camera_aware_energy_conserving_metal_lighting() {
    let shader = include_str!(concat!(env!("OUT_DIR"), "/model.wgsl"));
    let surface_output = shader_fn_body(shader, "resolve_surface_output");

    for required in [
        "let view = resolve_view_direction(input.world_position);",
        "let f0 = mix(dielectric_f0",
        "* (1.0 - metalness)",
        "studio_environment(reflection, roughness)",
        "hemisphere_irradiance(normal, view)",
    ] {
        assert!(
            surface_output.contains(required),
            "surface lighting must preserve {required}"
        );
    }
    assert!(
        !surface_output.contains("light + vec3<f32>(0.0, 0.0, 1.0)"),
        "surface lighting must not use a fixed camera direction"
    );
}

#[test]
fn model_shader_applies_normal_light_once_per_direct_term() {
    let shader = include_str!(concat!(env!("OUT_DIR"), "/model.wgsl"));
    let surface_output = shader_fn_body(shader, "resolve_surface_output");
    let direct_diffuse = surface_output
        .split_once("let direct_diffuse")
        .and_then(|(_, rest)| rest.split_once("let ggx_direct_specular"))
        .map(|(section, _)| section)
        .expect("direct diffuse section");
    let direct_specular = surface_output
        .split_once("let ggx_direct_specular")
        .and_then(|(_, rest)| rest.split_once("let ambient_diffuse"))
        .map(|(section, _)| section)
        .expect("direct specular section");
    assert_eq!(
        direct_diffuse.matches("normal_light").count(),
        1,
        "direct diffuse must consume NdotL exactly once"
    );
    assert_eq!(
        direct_specular.matches("normal_light").count(),
        1,
        "direct specular must multiply the GGX BRDF by NdotL exactly once"
    );
}

#[test]
fn model_shader_consumes_only_baked_specular_alpha_as_anisotropy() {
    let shader = include_str!(concat!(env!("OUT_DIR"), "/model.wgsl"));
    for required in [
        "specular: vec4<f32>",
        "clamp(samples.specular.a, 0.0, 1.0)",
        "material.sheen_sphere_params.z > 0.5",
        "fn ggx_anisotropic_distribution",
        "let anisotropy_frame = resolve_anisotropy_frame(input, normal);",
    ] {
        assert!(
            shader.contains(required),
            "anisotropy path must preserve {required}"
        );
    }
}

#[test]
fn model_shader_does_not_invent_ssao_without_runtime_occlusion() {
    let shader = include_str!(concat!(env!("OUT_DIR"), "/model.wgsl"));
    let surface_output = shader_fn_body(shader, "resolve_surface_output");
    assert!(
        !surface_output.contains("material.surface_params.x")
            && !surface_output.contains("ambient_visibility"),
        "g_SSAOMask has no verified MeddleTools/runtime SSAO composition and must not silently darken Final shading"
    );
}

#[test]
fn model_shader_does_not_invent_sheen_or_sphere_lighting() {
    let shader = include_str!(concat!(env!("OUT_DIR"), "/model.wgsl"));
    let surface_output = shader_fn_body(shader, "resolve_surface_output");
    assert!(
        !shader.contains("fn resolve_extra_lighting")
            && !surface_output.contains("extra.sheen")
            && !surface_output.contains("extra.sphere"),
        "MeddleTools leaves Sheen/Sphere at a dead-end interface, so Final must not invent a lighting formula"
    );
    assert!(
        shader.contains("extra.sheen = textureSample")
            && shader.contains("extra.sphere = textureSample"),
        "Sheen/Sphere bindings must remain available to their dedicated debug views"
    );
}

#[test]
fn model_shader_does_not_invent_toon_lighting() {
    let shader = include_str!(concat!(env!("OUT_DIR"), "/model.wgsl"));
    let surface_output = shader_fn_body(shader, "resolve_surface_output");
    assert!(
        !shader.contains("fn resolve_toon_lighting")
            && !surface_output.contains("toon_sheen_params")
            && !surface_output.contains("toon_params"),
        "MeddleTools has no Toon node semantics, so Final must not invent banding or reflection formulas"
    );
    for invented_formula in ["0.6180339", "toon_diffuse", "toon_specular", "spec_band"] {
        assert!(
            !shader.contains(invented_formula),
            "shader reintroduced unsupported Toon formula {invented_formula}"
        );
    }
}

#[test]
fn model_shader_composes_colortable_lighting_fields_independently() {
    let shader = include_str!(concat!(env!("OUT_DIR"), "/model.wgsl"));
    let surface_output = shader_fn_body(shader, "resolve_surface_output");

    // MeddleTools character topology keeps modern Roughness independent
    // of GlossStrength. Installed Legacy DXBC separately proves its MRT
    // exp2(-Gloss/15) parameterization. Installed character/legacy DXBC
    // consumes raw SpecularStrength through a direct mul, so it must not
    // be clamped before composition; the final physical F0 is bounded
    // separately.
    assert!(
        !surface_output.contains("mix(1.0, 0.68, gloss_strength)"),
        "roughness must not be scaled by GlossStrength"
    );
    assert!(
        !surface_output.contains("mix(0.025, 0.10, specular_strength)"),
        "SpecularStrength must not remap to an arbitrary F0 range"
    );
    for required in [
        "let legacy_gloss_roughness = exp2(-max(surface.properties.z, 0.0) / 15.0);",
        "let roughness_source = select(surface.properties.y, legacy_gloss_roughness, material.properties.x > 0.5 && material.family_params.x > 0.5)",
        "let uses_legacy_colortable = material.properties.x > 0.5 && material.family_params.x > 0.5;",
        "let roughness = clamp(roughness_source, 0.06, 1.0);",
        "let specular_strength = max(surface.properties.w, 0.0);",
        "let specular_weight = specular_strength * select(1.0, clamp(surface.mask.r, 0.0, 1.0), uses_colortable_specular_mask)",
        "let preview_f0_specular_weight = select(specular_weight, 1.0, uses_legacy_colortable)",
        "let uses_colortable_specular_mask = material.properties.x > 0.5 && legacy_specular_mode < 0.5;",
        "surface.material_specular * (0.08 * preview_f0_specular_weight)",
        "let specular_mask = clamp(resolve_specular_mask_factor(surface.mask.r), 0.0, 1.35);",
    ] {
        assert!(
            surface_output.contains(required),
            "surface lighting must preserve the verified composition: {required}"
        );
    }
    assert!(
        !surface_output.contains("clamp(surface.properties.w, 0.0, 1.0)"),
        "installed raw SpecularStrength must survive until the final F0 bound"
    );
    let material_properties = shader
        .split_once("fn resolve_material_properties")
        .and_then(|(_, rest)| rest.split_once("fn resolve_specular_mask_factor"))
        .map(|(section, _)| section)
        .expect("material properties section");
    assert!(
        !material_properties.contains("mask.r * 1.35"),
        "specular strength must compose with mask.R directly, without an empirical factor"
    );
    assert!(
        material_properties
            .contains("let specular_strength = mix(1.0, mask.r, material.params.w);"),
        "mask fallback must keep the verified specular strength composition"
    );
    let specular_mask_factor = shader_fn_body(shader, "resolve_specular_mask_factor");
    assert!(
        specular_mask_factor.contains("return mask_red * mask_red;")
            && specular_mask_factor.contains("return 1.0;")
            && !specular_mask_factor.contains("1.35")
            && !specular_mask_factor.contains("material.params.w"),
        "only verified legacy Compatibility Mask may add a second mask-R factor"
    );
    assert!(
        !surface_output.contains(
            "select(\n            resolve_specular_mask_factor(surface.mask.r),\n            1.0,\n            material.properties.x > 0.5,"
        ),
        "ColorTable properties must not disable the verified legacy Default/Mask permutation"
    );
}

#[test]
fn model_shader_uses_proven_legacy_camera_reflection_lobe() {
    let shader = include_str!(concat!(env!("OUT_DIR"), "/model.wgsl"));
    let lobe = shader
        .split_once("fn legacy_camera_reflection_lobe")
        .and_then(|(_, rest)| rest.split_once("fn ggx_anisotropic_distribution"))
        .map(|(section, _)| section)
        .expect("Legacy camera-reflection lobe section");
    for required in [
        "+ vec3<f32>(0.0, 0.2, 0.0);",
        "let reflected_view = reflect(-view, normal);",
        "let normal_light = clamp(dot(normal, light), 0.0, 1.0);",
        "let visibility = min(3.0 - 3.0 * one_minus_light * one_minus_light, 1.0);",
        "let reflection_light = clamp(dot(reflected_view, light), 0.0, 1.0);",
        "return visibility * pow(reflection_light, max(gloss_strength, 0.0));",
    ] {
        assert!(
            lobe.contains(required),
            "Legacy lobe must retain installed DXBC formula: {required}"
        );
    }
    assert!(shader.contains("material.properties.x > 0.5 && material.family_params.x > 0.5;"));
}

#[test]
fn model_shader_applies_bg_uv_scales_only_to_bg_textures() {
    let shader = include_str!(concat!(env!("OUT_DIR"), "/model.wgsl"));
    // 六个可缩放采样点直乘 uv_scale(mask/emissive/material map 不缩放)。
    for required in [
        "resolve_uv(input, material.uv_sources0.x, material.uv_scroll_masks0.x) * material.uv_scale_a.xy",
        "resolve_uv(input, material.uv_sources0.y, material.uv_scroll_masks0.y) * material.uv_scale_b.xy",
        "resolve_uv(input, material.uv_sources2.x, material.uv_scroll_masks2.x) * material.uv_scale_a.zw",
        "resolve_uv(input, material.uv_sources2.y, material.uv_scroll_masks2.y) * material.uv_scale_b.zw",
        "resolve_uv(input, material.uv_sources2.z, material.uv_scroll_masks2.z) * material.uv_scale_c.zw",
        "resolve_uv(input, material.uv_sources1.y, material.uv_scroll_masks1.y) * material.uv_scale_c.xy",
    ] {
        assert!(
            shader.contains(required),
            "bg uv scale must apply: {required}"
        );
    }
}

#[test]
fn model_shader_irise_ring_follows_meddletools_node_semantics() {
    let shader = include_str!(concat!(env!("OUT_DIR"), "/model.wgsl"));
    let family = shader_fn_body(shader, "resolve_character_family_base");
    // 分侧：顶点色 G>0.5 选右眼色（单眼材质双眼共享）。
    for required in [
        "is_right = input.color.g > 0.5;",
        "select(material.character_left_iris.rgb, material.character_right_iris.rgb, is_right)",
        "mix(material.iris_white_eye.rgb, iris_color, clamp(samples.mask.b, 0.0, 1.0))",
    ] {
        assert!(
            family.contains(required),
            "iris eye tint must match MeddleTools nodes: {required}"
        );
    }
    // 角膜环：中心距 + radius ± fade 软环带 + 每侧 limbal 强度 × 发射强度。
    for required in [
        "let d = length(input.uv0 - vec2<f32>(0.5, 0.5));",
        "let inner = max(radius.x - fade.x, 0.0);",
        "let outer = radius.y + fade.y;",
        "let factor = clamp(ramp_in * gate_in + ramp_out * gate_out, 0.0, 1.0);",
        "select(material.iris_ring_a.w, material.iris_ring_b.z, is_right)",
        "*ring_emission += material.iris_ring_color.rgb * (factor * limbal * max(material.iris_ring_a.x, 0.0))",
    ] {
        assert!(
            family.contains(required),
            "iris ring must implement the soft band: {required}"
        );
    }
    // 环发射只进最终颜色（非光照路径），非 Iris 家族 uniform 全零不激活。
    assert!(
        shader.contains("+ surface.ring_emission;"),
        "ring emission must be additive in Final"
    );
    assert!(
        family.contains("if material.iris_ring_color.a > 0.5 {"),
        "ring must be gated by the uniform enable flag"
    );
}

#[test]
fn model_shader_uses_emissive_texture_or_fallback_without_empirical_gates() {
    let shader = include_str!(concat!(env!("OUT_DIR"), "/model.wgsl"));
    let emissive = shader
        .split_once("fn resolve_emissive")
        .and_then(|(_, rest)| rest.split_once("fn resolve_shader_diffuse_tint"))
        .map(|(section, _)| section)
        .expect("emissive resolve section");
    for required in [
        "let material_emissive = material.emissive_color.rgb;",
        "let shader_emissive = material.shader_emissive_color.rgb * material.shader_emissive_color.a;",
        "let texture_presence = material.emissive_color.a;",
        "let texture_emissive = emissive_tex * texture_presence;",
        "let fallback_emissive = material_emissive * (1.0 - texture_presence);",
        "return texture_emissive + fallback_emissive + shader_emissive;",
    ] {
        assert!(
            emissive.contains(required),
            "emissive source boundary must preserve {required}"
        );
    }
    for unsupported_formula in [
        "smoothstep",
        "mask.b",
        "vertex_alpha",
        "shader_multi_emissive",
        "texture_luma",
        "clamp",
    ] {
        assert!(
            !emissive.contains(unsupported_formula),
            "emissive must not reintroduce empirical gate {unsupported_formula}"
        );
    }
}

#[test]
fn model_shader_scales_only_character_colortable_emissive_by_lit_luminance() {
    let shader = include_str!(concat!(env!("OUT_DIR"), "/model.wgsl"));
    let surface = shader_fn_body(shader, "resolve_surface_output");
    for required in [
        "out.emissive_texture_term = samples.emissive * material.emissive_color.a;",
        "out.emissive_texture_term * camera.dynamic_emissive_color.rgb",
        "let uses_character_colortable_emissive_scale = material.properties.x > 0.5 && material.family_params.y > 0.5;",
        "let lit_luminance = dot(lit, vec3<f32>(0.29891, 0.58661, 0.11448));",
        "select(1.0, max(lit_luminance, 1.0), uses_character_colortable_emissive_scale)",
        "let unscaled_emissive = surface.emissive - surface.emissive_texture_term;",
        "select(surface.emissive_texture_term, surface.color_table_emissive, uses_character_colortable_emissive_scale)",
        "let color = lit + unscaled_emissive + surface.ring_emission;",
        "source_emissive * color_table_emissive_scale)",
        "out.color = vec4<f32>(aura.main + aura.emissive, surface.alpha);",
    ] {
        assert!(
            shader.contains(required) || surface.contains(required),
            "Character ColorTable emissive must retain installed Final formula: {required}"
        );
    }
    assert!(
        !surface.contains("surface.emissive * color_table_emissive_scale"),
        "static/shader emissive must not inherit the ColorTable-only luminance scale"
    );
}

#[test]
fn model_aura_emissive_keeps_runtime_gate_and_combine_modes() {
    let shader = include_str!(concat!(env!("OUT_DIR"), "/model.wgsl"));
    let aura = shader_fn_body(shader, "resolve_aura_surface_color");
    for required in [
        "camera.instance_env_parameter.x > 0.0",
        "mix(p[3].y, p[3].z, fresnel) * p[15].x",
        "mix(original_emissive, aura_rgb, emission_strength)",
        "original_emissive + aura_rgb * emission_strength",
        "i32(p[15].z) != 0",
    ] {
        assert!(
            aura.contains(required),
            "Aura emissive must preserve {required}"
        );
    }
    assert!(!aura.contains("aura_texture_alpha * emission_strength"));
}

#[test]
fn model_shader_does_not_invent_generic_multi_diffuse_mask_blending() {
    let shader = include_str!(concat!(env!("OUT_DIR"), "/model.wgsl"));
    let generic_tint = shader
        .split_once("fn resolve_shader_diffuse_tint")
        .and_then(|(_, rest)| rest.split_once("fn resolve_lightshaft_color"))
        .map(|(section, _)| section)
        .expect("generic shader tint section");
    assert!(
        generic_tint.contains("return material.shader_diffuse_color.rgb;")
            && !generic_tint.contains("shader_multi_diffuse_color")
            && !generic_tint.contains("clamp")
            && !generic_tint.contains("smoothstep")
            && !generic_tint.contains("mask.r")
            && !generic_tint.contains("0.35"),
        "generic Final must not invent a mask-R formula for g_MultiDiffuseColor"
    );
    assert!(
        shader.contains("samples.secondary_base.rgb * material.shader_multi_diffuse_color.rgb"),
        "verified BG secondary-base mix must retain g_MultiDiffuseColor"
    );
}

#[test]
fn model_shader_preserves_verified_water_deep_color_as_linear_hdr() {
    let shader = include_str!(concat!(env!("OUT_DIR"), "/model.wgsl"));
    let surface = shader
        .split_once("fn resolve_surface_state")
        .and_then(|(_, rest)| rest.split_once("fn fresnel_schlick"))
        .map(|(section, _)| section)
        .expect("surface state section");
    assert!(
        surface.contains("material.water_deep_color.rgb,")
            && !surface.contains("clamp(material.water_deep_color"),
        "verified water deep color must not be clipped by a preview-only range"
    );
}

#[test]
fn model_shader_keeps_unverified_detail_composition_out_of_final() {
    let shader = include_str!(concat!(env!("OUT_DIR"), "/model.wgsl"));
    let surface = shader
        .split_once("fn resolve_surface_state")
        .and_then(|(_, rest)| rest.split_once("fn resolve_surface_output"))
        .map(|(section, _)| section)
        .expect("surface state section");
    let normal = shader
        .split_once("fn resolve_normal")
        .and_then(|(_, rest)| rest.split_once("fn resolve_uv"))
        .map(|(section, _)| section)
        .expect("normal resolution section");

    assert!(
        !surface.contains("detail_array")
            && !surface.contains("detail_color")
            && !normal.contains("detail_array")
            && !shader.contains("fn resolve_detail_tint")
            && !shader.contains("fn resolve_single_detail_tint"),
        "unverified BG detail tint/normal composition must not affect Final"
    );
    for empirical_constant in ["* 0.22", "* 0.14", "0.32", "0.073", "0.65"] {
        assert!(
            !shader.contains(empirical_constant),
            "detail composition must not restore empirical constant {empirical_constant}"
        );
    }
    for debug_sample in [
        "color = detail_array.diffuse",
        "color = detail_array.normal",
    ] {
        assert!(
            shader.contains(debug_sample),
            "detail array sampling must remain directly inspectable via {debug_sample}"
        );
    }
}

#[test]
fn model_shader_keeps_unverified_alpha_shaping_out_of_final() {
    let shader = include_str!(concat!(env!("OUT_DIR"), "/model.wgsl"));
    let alpha = shader
        .split_once("fn resolve_surface_alpha")
        .and_then(|(_, rest)| rest.split_once("fn ordered_dither_threshold"))
        .map(|(section, _)| section)
        .expect("surface alpha section");
    assert!(
        !shader.contains("fn resolve_alpha_shaping")
            && !alpha.contains("material.alpha_params.x")
            && !alpha.contains("material.alpha_params.y")
            && !alpha.contains("pow("),
        "g_AlphaAperture/g_AlphaOffset must not alter Final without a verified formula"
    );
}

#[test]
fn model_shader_orients_two_sided_normals_toward_the_viewer() {
    let shader = include_str!(concat!(env!("OUT_DIR"), "/model.wgsl"));
    let resolve_normal = shader
        .split_once("fn resolve_normal")
        .and_then(|(_, rest)| rest.split_once("\nfn resolve_uv"))
        .map(|(section, _)| section)
        .expect("resolve_normal section");

    assert!(
        !resolve_normal.contains("front_facing"),
        "resolve_normal must not derive two-sided orientation from triangle winding"
    );
    assert!(
        !resolve_normal.contains("face_sign"),
        "resolve_normal must not keep a winding-based face sign"
    );
    for required in [
        "orient_geometric_normal_toward_viewer(input.normal, view)",
        "orient_geometric_normal_toward_viewer(input.normal1, view)",
    ] {
        assert!(
            resolve_normal.contains(required),
            "every world-space normal path must apply {required}"
        );
    }
    for required in [
        "fn resolve_view_direction(world_position: vec3<f32>) -> vec3<f32>",
        "camera.camera_position.xyz - world_position",
        "vertex_normal: vec3<f32>, view: vec3<f32>",
        "select(-1.0, 1.0, dot(normal, view) >= 0.0)",
    ] {
        assert!(
            shader.contains(required),
            "two-sided orientation helper must contain {required}"
        );
    }
    let fs_main = shader_fn_body(shader, "fs_main");
    assert!(
        !fs_main.contains("front_facing"),
        "fs_main must not plumb triangle winding into surface state"
    );
}

#[test]
fn post_pipeline_uses_hdr_intermediates_and_tone_mapped_compose() {
    assert_eq!(
        POST_FORMAT,
        wgpu::TextureFormat::Rgba16Float,
        "scene/bright intermediates must keep HDR values above 1.0 until composition"
    );

    let shader = include_str!(concat!(env!("OUT_DIR"), "/postprocess.wgsl"));
    let compose = shader
        .split_once("fn compose_fs")
        .and_then(|(_, rest)| rest.split_once("fn tonemap_pbr_neutral"))
        .map(|(section, _)| section)
        .expect("compose section");
    for required in [
        "let exposure = post.params.y;",
        "tonemap_pbr_neutral(color)",
        "if post.params.z > 0.5",
    ] {
        assert!(
            compose.contains(required),
            "compose pass must contain {required}"
        );
    }
    let encode_call_count = compose.matches("linear_to_srgb_channel(color.").count();
    assert_eq!(
        encode_call_count, 3,
        "compose must encode RGB exactly once inside the non-sRGB branch"
    );
    assert!(
        shader.contains("fn tonemap_pbr_neutral(input_color: vec3<f32>) -> vec3<f32>"),
        "compose must use the documented Khronos PBR Neutral operator"
    );
}

#[test]
fn bloom_uses_scene_linear_threshold_in_the_post_pass() {
    assert_eq!(
        BLOOM_THRESHOLD, 1.0,
        "bloom threshold is defined in scene-linear units (display white)"
    );
    let shader = include_str!(concat!(env!("OUT_DIR"), "/postprocess.wgsl"));
    for required in [
        "fn bloom_contribution(color: vec3<f32>, threshold: f32) -> vec3<f32>",
        "smoothstep(threshold, threshold + BLOOM_KNEE, luma)",
        "mix(color, bloom_contribution(color, post.params.z), post.params.w)",
    ] {
        assert!(
            shader.contains(required),
            "bloom pass must contain {required}"
        );
    }
    let model_shader = include_str!(concat!(env!("OUT_DIR"), "/model.wgsl"));
    assert!(
        !model_shader.contains("bright"),
        "material shader must not own bright-pass extraction anymore"
    );
    assert!(
        !model_shader.contains("highlight"),
        "material shader must not embed highlight extraction constants"
    );
}

#[test]
fn compose_post_params_encode_exactly_once_for_the_target_format() {
    let srgb = compose_post_params(0.68, wgpu::TextureFormat::Rgba8UnormSrgb);
    assert_eq!(
        srgb,
        [0.68, DEFAULT_EXPOSURE, 0.0, 0.0],
        "sRGB targets encode on write; the shader must not encode again"
    );
    let linear = compose_post_params(0.68, wgpu::TextureFormat::Rgba8Unorm);
    assert_eq!(
        linear,
        [0.68, DEFAULT_EXPOSURE, 1.0, 0.0],
        "non-sRGB targets need the shader-side sRGB encode"
    );
}

#[test]
fn transparent_triangles_sort_back_to_front_without_moving_opaque() {
    let batches = vec![
        test_batch(0, PreparedRenderPass::Opaque, [0.0, 0.0, 100.0]),
        test_batch(1, PreparedRenderPass::Transparent, [0.0, 0.0, -2.0]),
        test_batch(2, PreparedRenderPass::Glass, [0.0, 0.0, 3.0]),
        test_batch(3, PreparedRenderPass::Cutout, [0.0, 0.0, -100.0]),
        test_batch(4, PreparedRenderPass::AdditiveLightShaft, [0.0, 0.0, 200.0]),
    ];

    let sorted = sorted_transparent_triangles(&batches, 0.0, 0.0);

    assert_eq!(
        sorted
            .draws
            .iter()
            .map(|draw| batches[draw.batch_index].material_slot)
            .collect::<Vec<_>>(),
        vec![1, 2]
    );
    assert_eq!(sorted.indices, vec![3, 4, 5, 6, 7, 8]);
}

#[test]
fn transparent_triangles_reverse_order_with_the_camera() {
    let mut batch = test_batch(0, PreparedRenderPass::Transparent, [0.0; 3]);
    batch.transparent_triangles = vec![
        TransparentTriangle {
            indices: [0, 1, 2],
            center: [0.0, 0.0, 3.0],
        },
        TransparentTriangle {
            indices: [3, 4, 5],
            center: [0.0, 0.0, -2.0],
        },
    ];

    let front = sorted_transparent_triangles(std::slice::from_ref(&batch), 0.0, 0.0);
    let back =
        sorted_transparent_triangles(std::slice::from_ref(&batch), std::f32::consts::PI, 0.0);

    assert_eq!(front.indices, vec![3, 4, 5, 0, 1, 2]);
    assert_eq!(back.indices, vec![0, 1, 2, 3, 4, 5]);
    assert_eq!(front.draws.len(), 1);
    assert_eq!(front.draws[0].index_count, 6);
}

#[test]
fn transparent_sort_merges_only_adjacent_batch_runs() {
    let mut first = test_batch(0, PreparedRenderPass::Transparent, [0.0; 3]);
    first.transparent_triangles = vec![
        TransparentTriangle {
            indices: [0, 1, 2],
            center: [0.0, 0.0, -3.0],
        },
        TransparentTriangle {
            indices: [6, 7, 8],
            center: [0.0, 0.0, 3.0],
        },
    ];
    let mut second = test_batch(1, PreparedRenderPass::Glass, [0.0; 3]);
    second.transparent_triangles = vec![TransparentTriangle {
        indices: [3, 4, 5],
        center: [0.0, 0.0, 0.0],
    }];

    let sorted = sorted_transparent_triangles(&[first, second], 0.0, 0.0);

    assert_eq!(sorted.indices, vec![0, 1, 2, 3, 4, 5, 6, 7, 8]);
    assert_eq!(
        sorted.draws,
        vec![
            SortedTransparentDraw {
                batch_index: 0,
                index_start: 0,
                index_count: 3,
            },
            SortedTransparentDraw {
                batch_index: 1,
                index_start: 3,
                index_count: 3,
            },
            SortedTransparentDraw {
                batch_index: 0,
                index_start: 6,
                index_count: 3,
            },
        ]
    );
}

#[test]
fn transparent_triangle_metadata_uses_global_indices_and_centroids() {
    let vertices = vec![
        test_vertex([0.0, 0.0, 0.0]),
        test_vertex([3.0, 0.0, 0.0]),
        test_vertex([0.0, 6.0, 0.0]),
    ];

    assert_eq!(
        transparent_triangles(&vertices, &[0, 1, 2], 10, [0.0; 3]),
        vec![TransparentTriangle {
            indices: [10, 11, 12],
            center: [1.0, 2.0, 0.0],
        }]
    );
}

#[test]
fn scene_transparent_sort_interleaves_instances_globally_by_depth() {
    // 实例 0（身体，前景透明）与实例 1（装备，背景透明）深度交错：全局序必须
    // 跨实例穿插，而不是先画完一个实例再画另一个。
    let mut body = test_batch(0, PreparedRenderPass::Transparent, [0.0; 3]);
    body.transparent_triangles = vec![
        TransparentTriangle {
            indices: [0, 1, 2],
            center: [0.0, 0.0, -1.0],
        },
        TransparentTriangle {
            indices: [3, 4, 5],
            center: [0.0, 0.0, 3.0],
        },
    ];
    let mut gear = test_batch(1, PreparedRenderPass::Transparent, [0.0; 3]);
    gear.transparent_triangles = vec![
        TransparentTriangle {
            indices: [0, 1, 2],
            center: [0.0, 0.0, 1.0],
        },
        TransparentTriangle {
            indices: [3, 4, 5],
            center: [0.0, 0.0, -3.0],
        },
    ];

    let sorted = sorted_scene_transparent_batches(&[&[body], &[gear]], 0.0, 0.0);

    // yaw=0 时视线朝 -Z：远处（z=-3）先画，近处（z=3）后画，两实例交替。
    assert_eq!(
        sorted
            .draws
            .iter()
            .map(|draw| (draw.instance_index, draw.batch_index))
            .collect::<Vec<_>>(),
        vec![(1, 0), (0, 0), (1, 0), (0, 0)]
    );
    // 每实例索引流按全局深度序写入自己的缓冲（实例 0：z=-1 先于 z=3；
    // 实例 1：z=-3 先于 z=1）。
    assert_eq!(sorted.indices[0], vec![0, 1, 2, 3, 4, 5]);
    assert_eq!(sorted.indices[1], vec![3, 4, 5, 0, 1, 2]);
    // 全局绘制的段序与各实例索引流一一对应。
    let body_draws = sorted
        .draws
        .iter()
        .filter(|draw| draw.instance_index == 0)
        .collect::<Vec<_>>();
    assert_eq!(body_draws[0].index_start, 0);
    assert_eq!(body_draws[1].index_start, 3);
}

#[test]
fn scene_transparent_sort_skips_opaque_and_merges_adjacent_runs() {
    let mut body_opaque = test_batch(0, PreparedRenderPass::Opaque, [0.0, 0.0, -50.0]);
    body_opaque.transparent_triangles = Vec::new();
    let mut gear = test_batch(1, PreparedRenderPass::Glass, [0.0; 3]);
    gear.transparent_triangles = vec![
        TransparentTriangle {
            indices: [0, 1, 2],
            center: [0.0, 0.0, 2.0],
        },
        TransparentTriangle {
            indices: [3, 4, 5],
            center: [0.0, 0.0, 1.0],
        },
    ];

    let sorted = sorted_scene_transparent_batches(&[&[body_opaque], &[gear]], 0.0, 0.0);

    // 不透明批次不参与排序；同实例同批次的相邻段合并成一段。
    assert_eq!(sorted.indices[0], Vec::<u32>::new());
    assert_eq!(sorted.indices[1], vec![3, 4, 5, 0, 1, 2]);
    assert_eq!(
        sorted.draws,
        vec![SceneTransparentDraw {
            instance_index: 1,
            batch_index: 0,
            index_start: 0,
            index_count: 6,
        }]
    );
}

#[test]
fn scene_bounds_unwraps_all_instance_spheres() {
    // 相同中心不同半径：并集半径覆盖最大球。
    assert_eq!(
        scene_bounds_from_spheres(&[([0.0; 3], 1.0), ([0.0; 3], 3.0)]),
        ([0.0, 0.0, 0.0], 3.0)
    );
    // 分离球：中心取中点，半径覆盖两球。
    let (center, radius) =
        scene_bounds_from_spheres(&[([-2.0, 0.0, 0.0], 1.0), ([2.0, 0.0, 0.0], 1.0)]);
    assert!((center[0] - 0.0).abs() < 1e-6);
    assert!(
        (radius - 3.0).abs() < 1e-6,
        "radius must cover both spheres"
    );
    // 空场景：保守单位球。
    assert_eq!(scene_bounds_from_spheres(&[]), ([0.0; 3], 1.0));
}

#[test]
fn prepared_material_pass_maps_alpha_modes_and_draw_roles() {
    assert_eq!(
        test_prepared_render_pass(
            MaterialAlphaMode::Opaque,
            MaterialRenderMode::Opaque,
            ModelMeshDrawRole::Normal
        ),
        PreparedRenderPass::Opaque
    );
    assert_eq!(
        test_prepared_render_pass(
            MaterialAlphaMode::Mask,
            MaterialRenderMode::Opaque,
            ModelMeshDrawRole::Normal
        ),
        PreparedRenderPass::Cutout
    );
    assert_eq!(
        test_prepared_render_pass(
            MaterialAlphaMode::Blend,
            MaterialRenderMode::Transparent,
            ModelMeshDrawRole::Normal
        ),
        PreparedRenderPass::Transparent
    );
    assert_eq!(
        test_prepared_render_pass(
            MaterialAlphaMode::Glass,
            MaterialRenderMode::Glass,
            ModelMeshDrawRole::Normal
        ),
        PreparedRenderPass::Glass
    );
    assert_eq!(
        test_prepared_render_pass(
            MaterialAlphaMode::Opaque,
            MaterialRenderMode::Transparent,
            ModelMeshDrawRole::Normal
        ),
        PreparedRenderPass::Transparent
    );
    assert_eq!(
        test_prepared_render_pass(
            MaterialAlphaMode::Opaque,
            MaterialRenderMode::Opaque,
            ModelMeshDrawRole::Glass
        ),
        PreparedRenderPass::Glass
    );
    assert_eq!(
        test_prepared_render_pass(
            MaterialAlphaMode::Opaque,
            MaterialRenderMode::Opaque,
            ModelMeshDrawRole::LightShaft
        ),
        PreparedRenderPass::AdditiveLightShaft
    );
    assert_eq!(
        test_prepared_render_pass(
            MaterialAlphaMode::Opaque,
            MaterialRenderMode::Opaque,
            ModelMeshDrawRole::CrestChange
        ),
        PreparedRenderPass::Transparent
    );

    let mut stockings = fallback_material();
    stockings.shader_package_name = Some("characterstockings.shpk".to_string());
    stockings.alpha_mode = MaterialAlphaMode::Blend;
    stockings.render_mode = MaterialRenderMode::Transparent;
    let prepared = prepare_material_for_draw_role(Some(&stockings), ModelMeshDrawRole::Normal);
    assert_eq!(prepared.render_pass, PreparedRenderPass::Opaque);
    assert!(prepared.render_pass.uses_opaque_pipeline());
}

#[test]
fn prepared_material_falls_back_for_missing_material_slot() {
    assert_eq!(
        prepare_material_for_draw_role(None, ModelMeshDrawRole::Normal),
        PreparedMaterial {
            render_pass: PreparedRenderPass::Opaque,
            shader_family: MaterialShaderFamily::Unknown,
            flow_mode: MaterialFlowMode::Standard,
            value_mode: crate::MaterialValueMode::Single,
            value_mode_raw: None,
            sub_color_mode: crate::MaterialSubColorMode::None,
            decal_color_mode: crate::MaterialDecalColorMode::Off,
            decal_color_mode_raw: None,
            skin_value_mode: crate::MaterialSkinValueMode::None,
            lightshaft_type: crate::MaterialLightShaftType::None,
            lightshaft_type_raw: None,
            alpha_policy: crate::PreparedMaterialAlphaPolicy::default(),
            texture_bindings: PreparedTextureBindings::default(),
            texture_sampling: PreparedTextureSamplingSet::default(),
            uv_sources: PreparedMaterialUvSources::default(),
            feature_flags: PreparedMaterialFeatureFlags::default(),
            unsupported_inputs: PreparedMaterialUnsupportedInputs::default(),
            resource_availability: PreparedMaterialResourceAvailability::default(),
            runtime_fallbacks: PreparedMaterialRuntimeFallbacks::default(),
            runtime_input_requirements: PreparedMaterialRuntimeInputRequirements::default(),
            render_backfaces: true,
        }
    );
}

#[test]
fn prepared_render_pass_reports_pipeline_class() {
    assert!(PreparedRenderPass::Opaque.uses_opaque_pipeline());
    assert!(!PreparedRenderPass::Cutout.uses_opaque_pipeline());
    assert!(PreparedRenderPass::Cutout.uses_cutout_pipeline());
    assert!(!PreparedRenderPass::Opaque.sorts_back_to_front());
    assert!(!PreparedRenderPass::Cutout.sorts_back_to_front());
    assert!(PreparedRenderPass::Transparent.uses_transparent_pipeline());
    assert!(!PreparedRenderPass::Glass.uses_transparent_pipeline());
    assert!(PreparedRenderPass::Glass.uses_glass_pipeline());
    assert!(PreparedRenderPass::Transparent.sorts_back_to_front());
    assert!(PreparedRenderPass::Glass.sorts_back_to_front());
    assert!(PreparedRenderPass::AdditiveLightShaft.uses_additive_pipeline());
    assert!(!PreparedRenderPass::AdditiveLightShaft.uses_opaque_pipeline());
    assert!(!PreparedRenderPass::AdditiveLightShaft.uses_cutout_pipeline());
    assert!(!PreparedRenderPass::AdditiveLightShaft.uses_transparent_pipeline());
    assert!(!PreparedRenderPass::AdditiveLightShaft.uses_glass_pipeline());
    assert!(!PreparedRenderPass::AdditiveLightShaft.sorts_back_to_front());
}

#[test]
fn model_pipeline_blend_modes_report_blend_and_depth_policy() {
    assert_eq!(ModelPipelineBlend::Opaque.blend_state(), None);
    assert!(ModelPipelineBlend::Opaque.writes_depth());
    assert_eq!(ModelPipelineBlend::Opaque.fragment_entry(), "fs_main");
    assert_eq!(
        ModelPipelineBlend::Opaque.color_write_mask(),
        wgpu::ColorWrites::ALL
    );

    assert_eq!(ModelPipelineBlend::DitherDepth.blend_state(), None);
    assert!(ModelPipelineBlend::DitherDepth.writes_depth());
    assert_eq!(
        ModelPipelineBlend::DitherDepth.fragment_entry(),
        "fs_dither_depth"
    );
    assert_eq!(
        ModelPipelineBlend::DitherDepth.color_write_mask(),
        wgpu::ColorWrites::empty()
    );
    assert_eq!(
        ModelPipelineBlend::Alpha.blend_state(),
        Some(wgpu::BlendState::ALPHA_BLENDING)
    );
    assert!(!ModelPipelineBlend::Alpha.writes_depth());

    let additive = ModelPipelineBlend::Additive
        .blend_state()
        .expect("additive blend");
    assert_eq!(additive.color.src_factor, wgpu::BlendFactor::One);
    assert_eq!(additive.color.dst_factor, wgpu::BlendFactor::One);
    assert_eq!(additive.color.operation, wgpu::BlendOperation::Add);
    assert_eq!(additive.alpha.src_factor, wgpu::BlendFactor::One);
    assert_eq!(additive.alpha.dst_factor, wgpu::BlendFactor::One);
    assert!(!ModelPipelineBlend::Additive.writes_depth());
}

#[test]
fn dither_depth_prepass_only_selects_dithered_transparent_batches() {
    let mut glass = test_batch(0, PreparedRenderPass::Glass, [0.0; 3]);
    glass.prepared_material.alpha_policy.draw_depth_mode = MaterialDrawDepthMode::Dither;
    assert!(glass.uses_dither_depth_prepass());

    let mut transparent = test_batch(1, PreparedRenderPass::Transparent, [0.0; 3]);
    assert!(!transparent.uses_dither_depth_prepass());
    transparent.prepared_material.alpha_policy.draw_depth_mode = MaterialDrawDepthMode::Dither;
    assert!(transparent.uses_dither_depth_prepass());

    let mut opaque = test_batch(2, PreparedRenderPass::Opaque, [0.0; 3]);
    opaque.prepared_material.alpha_policy.draw_depth_mode = MaterialDrawDepthMode::Dither;
    assert!(!opaque.uses_dither_depth_prepass());
}

#[test]
fn glass_blend_mode_only_switches_glass_batches_to_additive() {
    assert_eq!(
        ModelRenderOptions::default().glass_blend_mode,
        ModelGlassBlendMode::Alpha
    );

    let glass = test_batch(0, PreparedRenderPass::Glass, [0.0; 3]);
    assert!(!glass.uses_additive_glass_pipeline(ModelGlassBlendMode::Alpha));
    assert!(glass.uses_additive_glass_pipeline(ModelGlassBlendMode::Additive));

    let transparent = test_batch(1, PreparedRenderPass::Transparent, [0.0; 3]);
    assert!(!transparent.uses_additive_glass_pipeline(ModelGlassBlendMode::Additive));
}

#[test]
fn outline_pass_only_selects_eligible_surface_batches() {
    let mut normal = test_batch(0, PreparedRenderPass::Opaque, [0.0; 3]);
    normal.prepared_material.feature_flags.uses_outline = true;
    normal
        .prepared_material
        .unsupported_inputs
        .outline_composition = true;
    assert!(
        !normal.uses_outline_pass(),
        "unverified static outline inputs must not automatically alter Final"
    );
    normal
        .prepared_material
        .unsupported_inputs
        .outline_composition = false;
    assert!(normal.uses_outline_pass());

    let mut lightshaft = test_batch(1, PreparedRenderPass::AdditiveLightShaft, [0.0; 3]);
    lightshaft.prepared_material.feature_flags.uses_outline = true;
    lightshaft.draw_role = ModelMeshDrawRole::LightShaft;
    assert!(!lightshaft.uses_outline_pass());

    let mut crest = test_batch(2, PreparedRenderPass::Transparent, [0.0; 3]);
    crest.prepared_material.feature_flags.uses_outline = true;
    crest.draw_role = ModelMeshDrawRole::CrestChange;
    assert!(!crest.uses_outline_pass());
}

#[test]
fn camera_uniform_preserves_finite_uv_scroll_time() {
    let mut options = ModelRenderOptions {
        uv_scroll_time: 12.5,
        debug_mode: ModelDebugMode::Mask,
        ..ModelRenderOptions::default()
    };
    let uniform = camera_uniform([0.0; 3], 1.0, [128, 64], 0.0, 0.0, 2.0, [0.0; 2], options);
    assert_eq!(uniform.options[2], 12.5);
    assert_eq!(uniform.options[3], 3.0);

    options.uv_scroll_time = f32::NAN;
    let uniform = camera_uniform([0.0; 3], 1.0, [128, 64], 0.0, 0.0, 2.0, [0.0; 2], options);
    assert_eq!(uniform.options[2], 0.0);
    assert_eq!(uniform.options[3], 3.0);
}

#[test]
fn camera_uniform_carries_dynamic_emissive_with_identity_and_finite_fallbacks() {
    let uniform = camera_uniform(
        [0.0; 3],
        1.0,
        [128, 64],
        0.0,
        0.0,
        2.0,
        [0.0; 2],
        ModelRenderOptions {
            dynamic_emissive_color: [2.0, 0.5, 3.0],
            ..ModelRenderOptions::default()
        },
    );
    assert_eq!(uniform.dynamic_emissive_color, [2.0, 0.5, 3.0, 0.0]);

    let uniform = camera_uniform(
        [0.0; 3],
        1.0,
        [128, 64],
        0.0,
        0.0,
        2.0,
        [0.0; 2],
        ModelRenderOptions {
            dynamic_emissive_color: [f32::NAN, f32::INFINITY, 0.25],
            ..ModelRenderOptions::default()
        },
    );
    assert_eq!(uniform.dynamic_emissive_color, [1.0, 1.0, 0.25, 0.0]);
}

#[test]
fn camera_uniform_carries_aura_emissive_gate_only_when_finite() {
    let make = |value| {
        camera_uniform(
            [0.0; 3],
            1.0,
            [128, 64],
            0.0,
            0.0,
            2.0,
            [0.0; 2],
            ModelRenderOptions {
                instance_env_parameter_w: value,
                ..Default::default()
            },
        )
    };
    assert_eq!(make(1.0).instance_env_parameter, [1.0, 0.0, 0.0, 0.0]);
    assert_eq!(make(f32::NAN).instance_env_parameter, [0.0; 4]);
    assert_eq!(make(f32::INFINITY).instance_env_parameter, [0.0; 4]);
}

#[test]
fn camera_uniform_rotates_view_and_studio_key_light_together() {
    let front = camera_uniform(
        [0.0; 3],
        1.0,
        [128, 128],
        0.0,
        0.0,
        2.0,
        [0.0; 2],
        ModelRenderOptions::default(),
    );
    let back = camera_uniform(
        [0.0; 3],
        1.0,
        [128, 128],
        std::f32::consts::PI,
        0.0,
        2.0,
        [0.0; 2],
        ModelRenderOptions::default(),
    );
    let light_view_dot = |uniform: &CameraUniform| {
        uniform.light_dir[0] * uniform.view_dir[0]
            + uniform.light_dir[1] * uniform.view_dir[1]
            + uniform.light_dir[2] * uniform.view_dir[2]
    };

    assert!(front.view_dir[2] > 0.99);
    assert!(back.view_dir[2] < -0.99);
    assert!(light_view_dot(&front) > 0.5);
    assert!(light_view_dot(&back) > 0.5);
}

#[test]
fn preview_lighting_contract_is_explicit_and_stable() {
    let uniform = camera_uniform(
        [0.0; 3],
        1.0,
        [128, 128],
        0.0,
        0.0,
        2.0,
        [0.0; 2],
        ModelRenderOptions::default(),
    );
    let expected = glam::Vec3::new(PREVIEW_KEY_RIGHT, PREVIEW_KEY_UP, PREVIEW_KEY_VIEW).normalize();
    let actual = glam::Vec3::from_array([
        uniform.light_dir[0],
        uniform.light_dir[1],
        uniform.light_dir[2],
    ]);
    assert!(actual.abs_diff_eq(expected, 1.0e-6));

    let shader = include_str!(concat!(env!("OUT_DIR"), "/model.wgsl"));
    for constant in [
        "PREVIEW_KEY_COLOR",
        "PREVIEW_DIRECT_DIFFUSE_SCALE",
        "PREVIEW_DIRECT_SPECULAR_SCALE",
        "PREVIEW_AMBIENT_GROUND",
        "PREVIEW_AMBIENT_SKY",
        "PREVIEW_ENV_GROUND",
        "PREVIEW_ENV_SKY",
        "PREVIEW_RIM_SCALE",
    ] {
        assert!(
            shader.contains(constant),
            "preview lighting parameter {constant} must stay named and auditable"
        );
    }
}

#[test]
fn model_debug_modes_have_stable_shader_values() {
    assert_eq!(ModelDebugMode::Final.shader_value(), 0.0);
    assert_eq!(ModelDebugMode::BaseColor.shader_value(), 1.0);
    assert_eq!(ModelDebugMode::Normal.shader_value(), 2.0);
    assert_eq!(ModelDebugMode::Mask.shader_value(), 3.0);
    assert_eq!(ModelDebugMode::MaterialProperties.shader_value(), 4.0);
    assert_eq!(ModelDebugMode::Specular.shader_value(), 5.0);
    assert_eq!(ModelDebugMode::Emissive.shader_value(), 6.0);
    assert_eq!(ModelDebugMode::Alpha.shader_value(), 7.0);
    assert_eq!(ModelDebugMode::Uv0.shader_value(), 8.0);
    assert_eq!(ModelDebugMode::Uv1.shader_value(), 9.0);
    assert_eq!(ModelDebugMode::Uv2.shader_value(), 10.0);
    assert_eq!(ModelDebugMode::Uv3.shader_value(), 11.0);
    assert_eq!(ModelDebugMode::VertexColor.shader_value(), 12.0);
    assert_eq!(ModelDebugMode::MeshRole.shader_value(), 13.0);
    assert_eq!(ModelDebugMode::ColorTableIndex.shader_value(), 14.0);
    assert_eq!(ModelDebugMode::MultiMap.shader_value(), 15.0);
    assert_eq!(ModelDebugMode::TileProperties.shader_value(), 16.0);
    assert_eq!(ModelDebugMode::SheenProperties.shader_value(), 17.0);
    assert_eq!(ModelDebugMode::SphereProperties.shader_value(), 18.0);
    assert_eq!(ModelDebugMode::TileMatrix.shader_value(), 19.0);
    assert_eq!(ModelDebugMode::TileNormalArray.shader_value(), 20.0);
    assert_eq!(ModelDebugMode::TileOrbArray.shader_value(), 21.0);
    assert_eq!(ModelDebugMode::DetailDiffuseArray.shader_value(), 22.0);
    assert_eq!(ModelDebugMode::DetailNormalArray.shader_value(), 23.0);
    assert_eq!(ModelDebugMode::VertexColor1.shader_value(), 24.0);
    assert_eq!(ModelDebugMode::SecondaryNormal.shader_value(), 25.0);
    assert_eq!(ModelDebugMode::Flow0.shader_value(), 26.0);
    assert_eq!(ModelDebugMode::Flow1.shader_value(), 27.0);
    assert_eq!(ModelDebugMode::UnsupportedInputs.shader_value(), 28.0);
    assert_eq!(ModelDebugMode::ViewDirection.shader_value(), 29.0);
}

#[test]
fn unsupported_inputs_diagnostic_color_prioritizes_visible_families() {
    let prepared = |patch: &dyn Fn(&mut PreparedMaterialUnsupportedInputs)| {
        let mut prepared = prepare_material_for_draw_role(None, ModelMeshDrawRole::Normal);
        patch(&mut prepared.unsupported_inputs);
        prepared
    };

    assert_eq!(
        unsupported_inputs_diagnostic_color(prepared(&|_| {})),
        [0.05, 0.22, 0.1, 1.0],
        "fully supported materials must not look like unsupported ones"
    );
    assert_eq!(
        unsupported_inputs_diagnostic_color(prepared(&|inputs| {
            inputs.lightshaft_clip = true;
        })),
        [1.0, 0.82, 0.2, 1.0]
    );
    assert_eq!(
        unsupported_inputs_diagnostic_color(prepared(&|inputs| {
            inputs.environment_mapping = true;
        })),
        [0.25, 0.5, 1.0, 1.0]
    );
    assert_eq!(
        unsupported_inputs_diagnostic_color(prepared(&|inputs| {
            inputs.character_reflection = true;
        })),
        [1.0, 0.3, 0.75, 1.0]
    );
    assert_eq!(
        unsupported_inputs_diagnostic_color(prepared(&|inputs| {
            inputs.character_scroll_variant = true;
        })),
        [1.0, 0.55, 0.1, 1.0]
    );
    assert_eq!(
        unsupported_inputs_diagnostic_color(prepared(&|inputs| {
            inputs.glass_shader_parameters = true;
            inputs.incomplete_shader_family_logic = true;
        })),
        [0.15, 0.85, 0.9, 1.0],
        "family-specific flags must win over the generic incomplete-family hue"
    );
    assert_eq!(
        unsupported_inputs_diagnostic_color(prepared(&|inputs| {
            inputs.incomplete_shader_family_logic = true;
        })),
        [0.95, 0.2, 0.2, 1.0]
    );
    assert_eq!(
        unsupported_inputs_diagnostic_color(prepared(&|inputs| {
            inputs.alpha_multi_values = true;
        })),
        [0.68, 0.28, 1.0, 1.0]
    );
    assert_eq!(
        unsupported_inputs_diagnostic_color(prepared(&|inputs| {
            inputs.multi_color_composition = true;
        })),
        [0.92, 0.36, 0.16, 1.0]
    );
    assert_eq!(
        unsupported_inputs_diagnostic_color(prepared(&|inputs| {
            inputs.detail_composition = true;
        })),
        [0.72, 0.52, 0.96, 1.0]
    );
    assert_eq!(
        unsupported_inputs_diagnostic_color(prepared(&|inputs| {
            inputs.alpha_shaping = true;
        })),
        [0.94, 0.44, 0.78, 1.0]
    );
    assert_eq!(
        unsupported_inputs_diagnostic_color(prepared(&|inputs| {
            inputs.vertex_color_composition = true;
        })),
        [0.18, 0.76, 0.54, 1.0]
    );
    assert_eq!(
        unsupported_inputs_diagnostic_color(prepared(&|inputs| {
            inputs.specular_color_mask_composition = true;
        })),
        [0.84, 0.28, 0.62, 1.0]
    );
    assert_eq!(
        unsupported_inputs_diagnostic_color(prepared(&|inputs| {
            inputs.outline_composition = true;
        })),
        [0.46, 0.82, 0.94, 1.0]
    );
    assert_eq!(
        unsupported_inputs_diagnostic_color(prepared(&|inputs| {
            inputs.toon_lighting = true;
        })),
        [0.88, 0.64, 0.22, 1.0]
    );
    assert_eq!(
        unsupported_inputs_diagnostic_color(prepared(&|inputs| {
            inputs.sheen_lighting = true;
        })),
        [0.95, 0.48, 0.62, 1.0]
    );
    assert_eq!(
        unsupported_inputs_diagnostic_color(prepared(&|inputs| {
            inputs.sphere_lighting = true;
        })),
        [0.34, 0.7, 0.92, 1.0]
    );
    assert_eq!(
        unsupported_inputs_diagnostic_color(prepared(&|inputs| {
            inputs.ambient_occlusion_mask = true;
        })),
        [0.58, 0.72, 0.16, 1.0]
    );
    assert_eq!(
        unsupported_inputs_diagnostic_color(prepared(&|inputs| {
            inputs.ssao_mask = true;
        })),
        [0.58, 0.72, 0.16, 1.0]
    );
    assert_eq!(
        unsupported_inputs_diagnostic_color(prepared(&|inputs| {
            inputs.legacy_gloss_composition = true;
        })),
        [0.76, 0.3, 0.9, 1.0]
    );
    assert_eq!(
        unsupported_inputs_diagnostic_color(prepared(&|inputs| {
            inputs.legacy_specular_type = true;
        })),
        [0.58, 0.38, 0.88, 1.0]
    );
    assert_eq!(
        unsupported_inputs_diagnostic_color(prepared(&|inputs| {
            inputs.tile_mip_bias_offset = true;
        })),
        [0.78, 0.4, 0.12, 1.0]
    );
    assert_eq!(
        unsupported_inputs_diagnostic_color(prepared(&|inputs| {
            inputs.vertex_movement_parameters = true;
        })),
        [0.32, 0.88, 0.28, 1.0]
    );
    assert_eq!(
        unsupported_inputs_diagnostic_color(prepared(&|inputs| {
            inputs.runtime_color_table = true;
        })),
        [0.6, 0.6, 0.6, 1.0],
        "any other unsupported input must still surface as unsupported"
    );
}

#[test]
fn model_shader_exposes_unsupported_inputs_debug_view() {
    let shader = include_str!(concat!(env!("OUT_DIR"), "/model.wgsl"));
    assert!(
        shader.contains("unsupported_color: vec4<f32>"),
        "material uniform must carry the unsupported-input diagnostic color"
    );
    let debug_output = shader
        .split_once("fn debug_fragment_output")
        .map(|(_, rest)| rest)
        .expect("debug fragment output section");
    assert!(
        debug_output.contains("color = material.unsupported_color.rgb;"),
        "debug output must route the unsupported-input mode to the diagnostic color"
    );
}

#[test]
fn lightshaft_dedicated_pipeline_is_only_used_for_final_rendering() {
    assert!(lightshaft_uses_dedicated_pipeline(ModelDebugMode::Final));
    assert!(!lightshaft_uses_dedicated_pipeline(
        ModelDebugMode::BaseColor
    ));
    assert!(!lightshaft_uses_dedicated_pipeline(
        ModelDebugMode::MeshRole
    ));
}

#[test]
fn material_sampler_roles_keep_independent_prepared_policies() {
    let base_sampling = test_sampling(
        PreparedTextureColorSpace::Srgb,
        PreparedTextureFilter::Linear,
        PreparedTextureAddressMode::ClampToEdge,
    );
    let emissive_sampling = test_sampling(
        PreparedTextureColorSpace::Srgb,
        PreparedTextureFilter::Linear,
        PreparedTextureAddressMode::Repeat,
    );
    let index_sampling = test_sampling(
        PreparedTextureColorSpace::NonColor,
        PreparedTextureFilter::Nearest,
        PreparedTextureAddressMode::Repeat,
    );
    let prepared = PreparedMaterial {
        render_pass: PreparedRenderPass::Opaque,
        shader_family: MaterialShaderFamily::Character,
        flow_mode: MaterialFlowMode::Standard,
        value_mode: crate::MaterialValueMode::Single,
        value_mode_raw: None,
        sub_color_mode: crate::MaterialSubColorMode::None,
        decal_color_mode: crate::MaterialDecalColorMode::Off,
        decal_color_mode_raw: None,
        skin_value_mode: crate::MaterialSkinValueMode::None,
        lightshaft_type: crate::MaterialLightShaftType::None,
        lightshaft_type_raw: None,
        alpha_policy: crate::PreparedMaterialAlphaPolicy::default(),
        texture_bindings: PreparedTextureBindings::default(),
        texture_sampling: PreparedTextureSamplingSet {
            base_color: base_sampling,
            emissive: emissive_sampling,
            index: index_sampling,
            ..PreparedTextureSamplingSet::default()
        },
        uv_sources: PreparedMaterialUvSources::default(),
        feature_flags: PreparedMaterialFeatureFlags::default(),
        unsupported_inputs: PreparedMaterialUnsupportedInputs::default(),
        resource_availability: PreparedMaterialResourceAvailability::default(),
        runtime_fallbacks: PreparedMaterialRuntimeFallbacks::default(),
        runtime_input_requirements: PreparedMaterialRuntimeInputRequirements::default(),
        render_backfaces: true,
    };

    let base = sampler_descriptor_for_sampling("base", prepared.texture_sampling.base_color);
    let emissive = sampler_descriptor_for_sampling("emissive", prepared.texture_sampling.emissive);
    let index = sampler_descriptor_for_sampling("index", prepared.texture_sampling.index);
    assert_eq!(base.address_mode_u, wgpu::AddressMode::ClampToEdge);
    assert_eq!(emissive.address_mode_u, wgpu::AddressMode::Repeat);
    assert_eq!(index.mag_filter, wgpu::FilterMode::Nearest);
}

#[test]
fn sampler_descriptor_follows_prepared_filter_and_address_policy() {
    let linear_repeat = sampler_descriptor_for_sampling(
        "linear repeat",
        test_sampling(
            PreparedTextureColorSpace::Srgb,
            PreparedTextureFilter::Linear,
            PreparedTextureAddressMode::Repeat,
        ),
    );
    assert_eq!(linear_repeat.mag_filter, wgpu::FilterMode::Linear);
    assert_eq!(linear_repeat.min_filter, wgpu::FilterMode::Linear);
    assert_eq!(linear_repeat.mipmap_filter, wgpu::MipmapFilterMode::Linear);
    assert_eq!(linear_repeat.address_mode_u, wgpu::AddressMode::Repeat);
    assert_eq!(linear_repeat.address_mode_v, wgpu::AddressMode::Repeat);

    let nearest_clip = sampler_descriptor_for_sampling(
        "nearest clip",
        test_sampling(
            PreparedTextureColorSpace::NonColor,
            PreparedTextureFilter::Nearest,
            PreparedTextureAddressMode::Clip,
        ),
    );
    assert_eq!(nearest_clip.mag_filter, wgpu::FilterMode::Nearest);
    assert_eq!(nearest_clip.min_filter, wgpu::FilterMode::Nearest);
    assert_eq!(nearest_clip.mipmap_filter, wgpu::MipmapFilterMode::Nearest);
    assert_eq!(nearest_clip.address_mode_u, wgpu::AddressMode::ClampToEdge);
    assert_eq!(nearest_clip.address_mode_v, wgpu::AddressMode::ClampToEdge);
}

#[test]
fn texture_formats_follow_prepared_color_space() {
    assert_eq!(
        mip_semantic_for_color_space(PreparedTextureColorSpace::Srgb),
        RgbaMipSemantic::SrgbColor,
        "sRGB-sampled textures need sRGB GPU decoding with linear alpha"
    );
    assert_eq!(
        mip_semantic_for_color_space(PreparedTextureColorSpace::NonColor),
        RgbaMipSemantic::LinearData,
        "Non-Color textures must stay raw linear data"
    );
    assert_eq!(
        texture_format_for_color_space(PreparedTextureColorSpace::Srgb),
        wgpu::TextureFormat::Rgba8UnormSrgb
    );
    assert_eq!(
        texture_format_for_color_space(PreparedTextureColorSpace::NonColor),
        wgpu::TextureFormat::Rgba8Unorm
    );
}

#[test]
fn draw_role_debug_colors_distinguish_visible_roles() {
    assert_eq!(
        draw_role_debug_color(ModelMeshDrawRole::Normal),
        [0.16, 0.72, 1.0, 1.0]
    );
    assert_eq!(
        draw_role_debug_color(ModelMeshDrawRole::Glass),
        [0.66, 0.92, 1.0, 1.0]
    );
    assert_eq!(
        draw_role_debug_color(ModelMeshDrawRole::MaterialChange),
        [1.0, 0.34, 0.76, 1.0]
    );
    assert_eq!(
        draw_role_debug_color(ModelMeshDrawRole::CrestChange),
        [1.0, 0.62, 0.2, 1.0]
    );
}

#[test]
fn prepared_material_preserves_culling_policy() {
    let mut material = fallback_material();
    material.render_backfaces = false;
    let model = crate::ModelData {
        bounds: crate::ModelBounds::default(),
        materials: vec![material],
        textures: Vec::new(),
        meshes: Vec::new(),
    };

    assert_eq!(
        prepare_material_for_draw_role(model.materials().first(), ModelMeshDrawRole::Normal),
        PreparedMaterial {
            render_pass: PreparedRenderPass::Opaque,
            shader_family: MaterialShaderFamily::Unknown,
            flow_mode: MaterialFlowMode::Standard,
            value_mode: crate::MaterialValueMode::Single,
            value_mode_raw: None,
            sub_color_mode: crate::MaterialSubColorMode::None,
            decal_color_mode: crate::MaterialDecalColorMode::Off,
            decal_color_mode_raw: None,
            skin_value_mode: crate::MaterialSkinValueMode::None,
            lightshaft_type: crate::MaterialLightShaftType::None,
            lightshaft_type_raw: None,
            alpha_policy: crate::PreparedMaterialAlphaPolicy::default(),
            texture_bindings: PreparedTextureBindings::default(),
            texture_sampling: PreparedTextureSamplingSet::default(),
            uv_sources: PreparedMaterialUvSources::default(),
            feature_flags: PreparedMaterialFeatureFlags::default(),
            unsupported_inputs: PreparedMaterialUnsupportedInputs::default(),
            resource_availability: PreparedMaterialResourceAvailability::default(),
            runtime_fallbacks: PreparedMaterialRuntimeFallbacks::default(),
            runtime_input_requirements: PreparedMaterialRuntimeInputRequirements::default(),
            render_backfaces: false,
        }
    );
}

#[test]
fn prepared_render_pass_uses_source_order_for_render_mode_fallbacks() {
    assert_eq!(
        test_prepared_render_pass(
            MaterialAlphaMode::Opaque,
            MaterialRenderMode::Glass,
            ModelMeshDrawRole::Normal
        ),
        PreparedRenderPass::Glass
    );
    assert_eq!(
        test_prepared_render_pass(
            MaterialAlphaMode::Opaque,
            MaterialRenderMode::Transparent,
            ModelMeshDrawRole::Normal
        ),
        PreparedRenderPass::Transparent
    );
    assert_eq!(
        test_prepared_render_pass(
            MaterialAlphaMode::Opaque,
            MaterialRenderMode::Opaque,
            ModelMeshDrawRole::Normal
        ),
        PreparedRenderPass::Opaque
    );
}

#[test]
fn effective_mask_texture_uses_only_explicit_mask_sampler() {
    let mut material = fallback_material();
    material.material_map_texture = Some(2);
    material.multi_map_texture = Some(3);

    assert_eq!(effective_mask_texture(&material), None);

    material.mask_texture = Some(1);
    assert_eq!(effective_mask_texture(&material), Some(1));
}

#[test]
fn effective_normal_texture_uses_primary_water_wave_only_for_water() {
    let mut material = fallback_material();
    material.normal_texture = Some(2);
    material.water_wave_texture = Some(7);
    material.water_wave1_texture = Some(8);
    material.water_whitecap_texture = Some(9);

    let regular = prepare_material_for_draw_role(Some(&material), ModelMeshDrawRole::Normal);
    assert_eq!(effective_normal_texture(&material, regular), Some(2));

    material.shader_package_name = Some("water.shpk".to_string());
    let water = prepare_material_for_draw_role(Some(&material), ModelMeshDrawRole::Normal);
    assert_eq!(effective_normal_texture(&material, water), Some(7));

    material.water_wave_texture = None;
    assert_eq!(effective_normal_texture(&material, water), Some(2));
}

#[test]
fn rgba_mip_chain_downsamples_each_texture_semantic() {
    let alternating = [
        0, 0, 0, 0, 255, 255, 255, 255, 255, 255, 255, 255, 0, 0, 0, 0,
    ];
    let srgb = rgba_mip_chain(2, 2, &alternating, RgbaMipSemantic::SrgbColor);
    let linear = rgba_mip_chain(2, 2, &alternating, RgbaMipSemantic::LinearData);
    assert_eq!(srgb.len(), 2);
    assert_eq!(srgb[1].rgba, [188, 188, 188, 128]);
    assert_eq!(linear[1].rgba, [128, 128, 128, 128]);

    let normals = [
        255, 128, 128, 0, 128, 255, 128, 255, 255, 128, 128, 0, 128, 255, 128, 255,
    ];
    let normal = rgba_mip_chain(2, 2, &normals, RgbaMipSemantic::PackedNormalRg);
    assert_eq!(normal[1].rgba, [218, 218, 128, 128]);
}

#[test]
fn array_pair_mips_preserve_halves_layers_and_packed_normal_payloads() {
    let texture = |kind, rgba| crate::ModelTexture {
        path: format!("synthetic/{kind:?}.tex"),
        kind,
        texel_layout: crate::ModelTextureTexelLayout::Standard,
        width: 2,
        height: 4,
        array_size: 2,
        array_layer_height: 2,
        rgba,
        rgba_f32: None,
    };
    let first = texture(
        crate::ModelTextureKind::TileNormalArray,
        vec![
            128, 128, 0, 0, 128, 128, 64, 0, 128, 128, 128, 255, 128, 128, 255, 255, 128, 128, 200,
            64, 128, 128, 200, 64, 128, 128, 200, 64, 128, 128, 200, 64,
        ],
    );
    let second = texture(
        crate::ModelTextureKind::TileOrbArray,
        vec![
            10, 0, 0, 255, 20, 0, 0, 255, 30, 0, 0, 255, 40, 0, 0, 255, 50, 0, 0, 255, 60, 0, 0,
            255, 70, 0, 0, 255, 80, 0, 0, 255,
        ],
    );

    let levels = array_pair_mip_chain(
        &first,
        &second,
        RgbaMipSemantic::PackedNormalRg,
        RgbaMipSemantic::LinearData,
    );
    assert_eq!(
        levels
            .iter()
            .map(|level| (level.width, level.height))
            .collect::<Vec<_>>(),
        [(4, 4), (2, 2)]
    );
    assert_eq!(
        levels[1]
            .rgba
            .chunks_exact(4)
            .map(|pixel| pixel.to_vec())
            .collect::<Vec<_>>(),
        [
            vec![128, 128, 112, 128],
            vec![25, 0, 0, 255],
            vec![128, 128, 200, 64],
            vec![65, 0, 0, 255],
        ]
    );
}

#[test]
fn rgba_mip_chain_reaches_one_by_one_for_odd_dimensions() {
    let levels = rgba_mip_chain(3, 5, &vec![255; 3 * 5 * 4], RgbaMipSemantic::LinearData);
    assert_eq!(
        levels
            .iter()
            .map(|level| (level.width, level.height))
            .collect::<Vec<_>>(),
        [(3, 5), (1, 2), (1, 1)]
    );
}

#[test]
fn material_extra_texture_flags_require_loaded_textures() {
    let mut material = fallback_material();
    material.tile_properties_texture = Some(0);
    material.sheen_properties_texture = Some(1);
    material.sphere_properties_texture = Some(99);
    material.tile_matrix_texture = Some(2);
    let model = crate::ModelData {
        bounds: crate::ModelBounds::default(),
        materials: vec![material.clone()],
        textures: vec![
            test_texture(crate::ModelTextureKind::TileProperties),
            test_texture(crate::ModelTextureKind::SheenProperties),
            test_texture(crate::ModelTextureKind::TileMatrixProperties),
        ],
        meshes: Vec::new(),
    };

    assert_eq!(
        material_extra_texture_flags(
            &material,
            &model,
            prepare_material_for_draw_role(Some(&material), ModelMeshDrawRole::Normal),
        ),
        [1.0, 1.0, 0.0, 1.0]
    );
}

#[test]
fn material_array_params_encode_prepared_resources() {
    let mut prepared = prepare_material_for_draw_role(None, ModelMeshDrawRole::Normal);
    prepared.resource_availability.tile_array = crate::PreparedTextureArrayResource {
        status: crate::PreparedTextureArrayStatus::Ready,
        layer_count: Some(4),
    };
    prepared.resource_availability.detail_array = crate::PreparedTextureArrayResource {
        status: crate::PreparedTextureArrayStatus::Ready,
        layer_count: Some(8),
    };
    assert_eq!(material_array_params(prepared), [4.0, 8.0, 1.0, 1.0]);

    prepared.resource_availability.tile_array.status =
        crate::PreparedTextureArrayStatus::InvalidLayout;
    prepared.resource_availability.tile_array.layer_count = None;
    assert_eq!(material_array_params(prepared), [1.0, 8.0, 0.0, 1.0]);
}

#[test]
fn material_tile_lod_params_require_a_ready_tile_array() {
    let mut material = fallback_material();
    let mut model = crate::ModelData {
        bounds: crate::ModelBounds::default(),
        materials: Vec::new(),
        textures: Vec::new(),
        meshes: Vec::new(),
    };
    material.tile_mip_bias_offset = -1.0;
    let mut prepared = prepare_material_for_draw_role(Some(&material), ModelMeshDrawRole::Normal);
    assert_eq!(
        material_tile_lod_params(&material, &model, prepared),
        [0.0; 4]
    );

    prepared.resource_availability.tile_array = crate::PreparedTextureArrayResource {
        status: crate::PreparedTextureArrayStatus::Ready,
        layer_count: Some(64),
    };
    assert_eq!(
        material_tile_lod_params(&material, &model, prepared),
        [-1.0, 0.0, 0.0, 0.0]
    );

    material.tile_mip_bias_offset = f32::INFINITY;
    assert_eq!(
        material_tile_lod_params(&material, &model, prepared),
        [0.0; 4]
    );

    material.tile_properties_texture = Some(0);
    material.tile_matrix_texture = Some(1);
    let mut tile = test_texture(crate::ModelTextureKind::TileProperties);
    let mut matrix = test_texture(crate::ModelTextureKind::TileMatrixProperties);
    tile.path = "renamed://tile-properties".to_string();
    matrix.path = "renamed://tile-matrix".to_string();
    model.textures = vec![tile.clone(), matrix.clone()];
    assert_eq!(
        material_tile_lod_params(&material, &model, prepared),
        [0.0, 0.0, 0.0, 0.0]
    );

    tile.texel_layout = crate::ModelTextureTexelLayout::ColorTableTileRampAb;
    model.textures = vec![tile.clone(), matrix.clone()];
    assert_eq!(
        material_tile_lod_params(&material, &model, prepared),
        [0.0, 0.0, 0.0, 0.0]
    );

    matrix.texel_layout = crate::ModelTextureTexelLayout::ColorTableTileRampAb;
    model.textures = vec![tile.clone(), matrix.clone()];
    assert_eq!(
        material_tile_lod_params(&material, &model, prepared),
        [0.0, 0.0, 0.0, 1.0]
    );

    material.shader_package_name = Some("character.shpk".to_string());
    material.base_color_texture = Some(2);
    material.specular_texture = Some(3);
    material.material_properties_texture = Some(4);
    material.sheen_properties_texture = Some(5);
    material.sphere_properties_texture = Some(6);
    let generic_kinds = [
        crate::ModelTextureKind::BaseColor,
        crate::ModelTextureKind::Specular,
        crate::ModelTextureKind::MaterialProperties,
        crate::ModelTextureKind::SheenProperties,
        crate::ModelTextureKind::SphereProperties,
    ];
    model.textures.extend(generic_kinds.into_iter().map(|kind| {
        let mut texture = test_texture(kind);
        texture.texel_layout = crate::ModelTextureTexelLayout::ColorTableRampAb;
        texture
    }));
    assert_eq!(
        material_tile_lod_params(&material, &model, prepared),
        [0.0, 1.0, 1.0, 1.0]
    );

    // A full-resolution base diffuse is not an A/B ramp: without the side-car
    // colorset ramp the packed base lookup stays off and the shader
    // composition flag stays clear.
    model.textures[2].texel_layout = crate::ModelTextureTexelLayout::Standard;
    assert_eq!(
        material_tile_lod_params(&material, &model, prepared),
        [0.0, 0.0, 0.0, 1.0]
    );
    assert_eq!(material_colorset_params(&material, &model), [0.0; 4]);
    // The colorset-diffuse ramp moves the A/B base lookup off the diffuse and
    // selects the shader composition branch.
    material.colorset_diffuse_texture = Some(2);
    model.textures[2].texel_layout = crate::ModelTextureTexelLayout::ColorTableRampAb;
    assert_eq!(
        material_tile_lod_params(&material, &model, prepared),
        [0.0, 1.0, 1.0, 1.0]
    );
    assert_eq!(
        material_colorset_params(&material, &model),
        [1.0, 0.0, 0.0, 0.0]
    );
    material.colorset_diffuse_texture = None;

    material.shader_package_name = Some("characterlegacy.shpk".to_string());
    assert_eq!(
        material_tile_lod_params(&material, &model, prepared),
        [0.0, 1.0, 0.0, 1.0]
    );

    matrix.kind = crate::ModelTextureKind::SheenProperties;
    model.textures = vec![tile, matrix];
    assert_eq!(
        material_tile_lod_params(&material, &model, prepared),
        [0.0, 0.0, 0.0, 0.0]
    );
}

#[test]
fn tile_matrix_texture_pixels_preserve_float_channels_and_fallbacks() {
    let mut texture = test_texture(crate::ModelTextureKind::TileMatrixProperties);
    texture.rgba = vec![255, 64, 128, 191];
    texture.rgba_f32 = Some(vec![[2.0, -0.5, 0.25, 1.5]]);
    assert_eq!(
        tile_matrix_texture_pixels(Some(&texture)),
        (1, 1, vec![[2.0, -0.5, 0.25, 1.5]])
    );

    texture.rgba_f32 = Some(vec![[f32::NAN, -0.5, f32::INFINITY, 1.5]]);
    let (_, _, pixels) = tile_matrix_texture_pixels(Some(&texture));
    assert_eq!(pixels[0], [1.0, -0.5, 128.0 / 255.0, 1.5]);

    texture.rgba_f32 = Some(Vec::new());
    let (_, _, pixels) = tile_matrix_texture_pixels(Some(&texture));
    assert_eq!(pixels[0], [1.0, 64.0 / 255.0, 128.0 / 255.0, 191.0 / 255.0]);
    assert_eq!(
        tile_matrix_texture_pixels(None),
        (1, 1, vec![[1.0, 0.0, 0.0, 1.0]])
    );
}

#[test]
fn float_ramp_texture_pixels_preserve_hdr_channels_and_fallbacks() {
    let mut texture = test_texture(crate::ModelTextureKind::SheenProperties);
    texture.rgba = vec![26, 51, 255, 255];
    texture.rgba_f32 = Some(vec![[0.1, 0.2, 4.0, 1.0]]);
    assert_eq!(
        float_ramp_texture_pixels(Some(&texture), [0.0, 0.0, 0.0, 1.0]),
        (1, 1, vec![[0.1, 0.2, 4.0, 1.0]])
    );

    texture.rgba_f32 = Some(vec![[f32::NAN, 0.2, f32::INFINITY, 1.0]]);
    let (_, _, pixels) = float_ramp_texture_pixels(Some(&texture), [0.0, 0.0, 0.0, 1.0]);
    assert_eq!(pixels[0], [26.0 / 255.0, 0.2, 1.0, 1.0]);

    texture.rgba_f32 = Some(Vec::new());
    let (_, _, pixels) = float_ramp_texture_pixels(Some(&texture), [0.0, 0.0, 0.0, 1.0]);
    assert_eq!(pixels[0], [26.0 / 255.0, 51.0 / 255.0, 1.0, 1.0]);
    assert_eq!(
        float_ramp_texture_pixels(None, [0.0, 0.0, 0.0, 1.0]),
        (1, 1, vec![[0.0, 0.0, 0.0, 1.0]])
    );
}

#[test]
fn material_shader_params_clamp_normal_scales() {
    let mut material = fallback_material();
    assert_eq!(material_shader_params(&material), [1.0, 1.0, 1.0, 1.0]);

    material.normal_scale = 2.25;
    material.multi_normal_scale = 0.5;
    material.detail_normal_scale = 3.5;
    material.multi_detail_normal_scale = f32::INFINITY;
    assert_eq!(material_shader_params(&material), [2.25, 0.5, 3.5, 4.0]);

    material.normal_scale = 8.0;
    material.multi_normal_scale = -1.0;
    material.detail_normal_scale = 8.0;
    material.multi_detail_normal_scale = 0.25;
    assert_eq!(material_shader_params(&material), [4.0, 0.0, 4.0, 0.25]);
}

#[test]
fn material_tile_params_preserve_tile_select_values() {
    let mut material = fallback_material();
    assert_eq!(material_tile_params(&material), [0.0, 1.0, 16.0, 16.0]);

    material.tile_index = 7.0;
    material.tile_alpha = 0.35;
    material.tile_scale = [24.0, 12.0];
    assert_eq!(material_tile_params(&material), [7.0, 0.35, 24.0, 12.0]);

    material.tile_index = f32::INFINITY;
    material.tile_alpha = 8.0;
    material.tile_scale = [f32::NAN, f32::NEG_INFINITY];
    assert_eq!(material_tile_params(&material), [0.0, 1.0, 16.0, 16.0]);
}

#[test]
fn material_toon_sheen_sphere_params_preserve_shader_inputs() {
    let mut material = fallback_material();
    let prepared = prepare_material_for_draw_role(Some(&material), ModelMeshDrawRole::Normal);
    assert_eq!(material_toon_sheen_params(&material), [0.0, 2.0, 0.0, 0.0]);
    assert_eq!(
        material_toon_params(&material, prepared),
        [50.0, 2.5, 4.0e-45, 0.0]
    );
    assert_eq!(
        material_sheen_sphere_params(&material, prepared),
        [1.0, 0.0, 0.0, 0.0]
    );

    material.toon_index = 5.0;
    material.toon_light_scale = 1.5;
    material.toon_light_spec_aperture = 64.0;
    material.toon_reflection_scale = 3.5;
    material.toon_spec_index = 2.0;
    material.sheen_rate = 0.25;
    material.sheen_tint_rate = 0.35;
    material.sheen_aperture = 0.8;
    material.sphere_map_index = 3.0;
    assert_eq!(
        material_toon_sheen_params(&material),
        [5.0, 1.5, 0.25, 0.35]
    );
    material.shader_package_name = Some("character.shpk".to_string());
    let prepared = prepare_material_for_draw_role(Some(&material), ModelMeshDrawRole::Normal);
    assert_eq!(
        material_toon_params(&material, prepared),
        [64.0, 3.5, 2.0, 1.0]
    );
    assert_eq!(
        material_sheen_sphere_params(&material, prepared),
        [0.8, 3.0, 0.0, 0.0]
    );

    material.toon_index = f32::NAN;
    material.toon_light_scale = f32::INFINITY;
    material.toon_light_spec_aperture = f32::NAN;
    material.toon_reflection_scale = f32::INFINITY;
    material.toon_spec_index = f32::NEG_INFINITY;
    material.sheen_rate = f32::NEG_INFINITY;
    material.sheen_tint_rate = f32::NAN;
    material.sheen_aperture = f32::INFINITY;
    material.sphere_map_index = f32::NAN;
    assert_eq!(material_toon_sheen_params(&material), [0.0, 2.0, 0.0, 0.0]);
    let prepared = prepare_material_for_draw_role(Some(&material), ModelMeshDrawRole::Normal);
    assert_eq!(
        material_toon_params(&material, prepared),
        [50.0, 2.5, 4.0e-45, 1.0]
    );
    assert_eq!(
        material_sheen_sphere_params(&material, prepared),
        [1.0, 0.0, 0.0, 0.0]
    );

    let mut baked_anisotropy = prepared;
    baked_anisotropy.texture_sampling.specular.color_space = PreparedTextureColorSpace::Srgb;
    assert_eq!(
        material_sheen_sphere_params(&material, baked_anisotropy),
        [1.0, 0.0, 1.0, 0.0],
        "only a baked ColorTable specular ramp may expose alpha as anisotropy"
    );
}

#[test]
fn material_alpha_params_preserve_shader_inputs() {
    let mut material = fallback_material();
    assert_eq!(material_alpha_params(&material), [2.0, 0.0, 0.5, 0.0]);

    material.alpha_aperture = 2.5;
    material.alpha_offset = -0.2;
    material.shadow_alpha_threshold = 0.35;
    material.transparency = 0.6;
    assert_eq!(material_alpha_params(&material), [2.5, -0.2, 0.35, 0.6]);

    material.alpha_aperture = f32::NAN;
    material.alpha_offset = f32::INFINITY;
    material.shadow_alpha_threshold = 4.0;
    material.transparency = -1.0;
    assert_eq!(material_alpha_params(&material), [2.0, 0.0, 1.0, 0.0]);
}

#[test]
fn material_alpha_composition_params_scope_vertex_alpha_remap_to_dxbc_packages() {
    let mut material = fallback_material();
    material.vertex_alpha_to_one = 0.01;
    assert_eq!(
        material_alpha_composition_params(&material),
        [0.01, 0.0, 0.0, 0.0]
    );

    for package in [
        "character.shpk",
        "characterlegacy.shpk",
        "characterglass.shpk",
    ] {
        material.shader_package_name = Some(package.to_string());
        assert_eq!(
            material_alpha_composition_params(&material),
            [0.01, 1.0, 0.0, 0.0]
        );
    }

    material.shader_package_name = Some("charactertattoo.shpk".to_string());
    assert_eq!(
        material_alpha_composition_params(&material),
        [0.01, 0.0, 0.0, 0.0]
    );
    material.vertex_alpha_to_one = f32::NAN;
    assert_eq!(
        material_alpha_composition_params(&material),
        [0.0, 0.0, 0.0, 0.0]
    );
}

#[test]
fn material_alpha_policy_params_encode_prepared_shader_policy() {
    let mut prepared = PreparedMaterial {
        render_pass: PreparedRenderPass::Transparent,
        shader_family: MaterialShaderFamily::CharacterTransparency,
        flow_mode: MaterialFlowMode::Standard,
        value_mode: crate::MaterialValueMode::Single,
        value_mode_raw: None,
        sub_color_mode: crate::MaterialSubColorMode::None,
        decal_color_mode: crate::MaterialDecalColorMode::Off,
        decal_color_mode_raw: None,
        skin_value_mode: crate::MaterialSkinValueMode::None,
        lightshaft_type: crate::MaterialLightShaftType::None,
        lightshaft_type_raw: None,
        alpha_policy: crate::PreparedMaterialAlphaPolicy {
            source: PreparedAlphaSource::NormalBlue,
            draw_depth_mode: MaterialDrawDepthMode::Dither,
            lighting_enabled: false,
        },
        texture_bindings: PreparedTextureBindings::default(),
        texture_sampling: PreparedTextureSamplingSet::default(),
        uv_sources: PreparedMaterialUvSources::default(),
        feature_flags: PreparedMaterialFeatureFlags::default(),
        unsupported_inputs: PreparedMaterialUnsupportedInputs::default(),
        resource_availability: PreparedMaterialResourceAvailability::default(),
        runtime_fallbacks: PreparedMaterialRuntimeFallbacks::default(),
        runtime_input_requirements: PreparedMaterialRuntimeInputRequirements::default(),
        render_backfaces: true,
    };
    assert_eq!(material_alpha_policy_params(prepared), [2.0, 0.0, 1.0, 1.0]);

    prepared.render_pass = PreparedRenderPass::Glass;
    prepared.alpha_policy.source = PreparedAlphaSource::BaseColorAlpha;
    prepared.alpha_policy.draw_depth_mode = MaterialDrawDepthMode::None;
    prepared.alpha_policy.lighting_enabled = true;
    assert_eq!(material_alpha_policy_params(prepared), [1.0, 1.0, 0.0, 2.0]);

    prepared.render_pass = PreparedRenderPass::Transparent;
    prepared.alpha_policy.source = PreparedAlphaSource::MaterialTransparency;
    assert_eq!(material_alpha_policy_params(prepared), [3.0, 1.0, 0.0, 1.0]);

    prepared.alpha_policy.source = PreparedAlphaSource::NormalAlpha;
    assert_eq!(material_alpha_policy_params(prepared), [4.0, 1.0, 0.0, 1.0]);
}

#[test]
fn material_water_colors_preserve_finite_shader_inputs() {
    let mut material = fallback_material();
    material.water_deep_color = [0.1, 0.2, 0.3, 0.4];
    material.water_refraction_color = [0.5, 0.6, 0.7, 0.8];
    material.water_whitecap_color = [0.9, 1.0, 1.1, 0.25];
    assert_eq!(material_water_deep_color(&material), [0.1, 0.2, 0.3, 0.4]);
    assert_eq!(
        material_water_refraction_color(&material),
        [0.5, 0.6, 0.7, 0.8]
    );
    assert_eq!(
        material_water_whitecap_color(&material),
        [0.9, 1.0, 1.1, 0.25]
    );

    material.water_deep_color = [f32::NAN, 0.2, f32::INFINITY, 0.4];
    assert_eq!(
        material_water_deep_color(&material),
        [0.3529, 0.2, 0.3921, 0.4]
    );
}

#[test]
fn material_detail_params_preserve_detail_uv_values() {
    let mut material = fallback_material();
    let prepared = prepare_material_for_draw_role(Some(&material), ModelMeshDrawRole::Normal);
    assert_eq!(
        material_detail_params(&material, prepared),
        [0.0, 0.0, 0.0, 0.0]
    );
    assert_eq!(material_detail_color(&material), [0.5, 0.5, 0.5, 1.0]);
    assert_eq!(material_multi_detail_color(&material), [0.5, 0.5, 0.5, 1.0]);
    assert_eq!(material_detail_color_uv_scale(&material), [4.0; 4]);
    assert_eq!(material_detail_normal_uv_scale(&material), [4.0; 4]);

    material.detail_id = 3.0;
    material.multi_detail_id = 5.0;
    material.detail_color = [0.2, 0.4, 0.6, 0.8];
    material.multi_detail_color = [0.1, 0.3, 0.5, 0.7];
    material.detail_color_uv_scale = [8.0, 6.0, 4.0, 2.0];
    material.detail_normal_uv_scale = [7.0, 5.0, 3.0, 1.0];
    material.shader_package_name = Some("bg.shpk".to_string());
    material.value_mode = MaterialValueMode::Multi;
    let prepared = prepare_material_for_draw_role(Some(&material), ModelMeshDrawRole::Normal);
    assert_eq!(
        material_detail_params(&material, prepared),
        [3.0, 5.0, 1.0, 0.0]
    );
    material.value_mode = MaterialValueMode::Single;
    let prepared = prepare_material_for_draw_role(Some(&material), ModelMeshDrawRole::Normal);
    assert_eq!(
        material_detail_params(&material, prepared),
        [3.0, 5.0, 0.0, 0.0]
    );
    material.value_mode = MaterialValueMode::Multi;
    assert_eq!(material_detail_color(&material), [0.2, 0.4, 0.6, 0.8]);
    assert_eq!(material_multi_detail_color(&material), [0.1, 0.3, 0.5, 0.7]);
    assert_eq!(
        material_detail_color_uv_scale(&material),
        [8.0, 6.0, 4.0, 2.0]
    );
    assert_eq!(
        material_detail_normal_uv_scale(&material),
        [7.0, 5.0, 3.0, 1.0]
    );

    material.detail_id = f32::NAN;
    material.multi_detail_id = f32::INFINITY;
    material.detail_color = [0.25, f32::NAN, f32::INFINITY, 0.5];
    material.multi_detail_color = [f32::NEG_INFINITY, 0.3, 0.5, f32::NAN];
    material.detail_color_uv_scale = [1.0, f32::NAN, f32::INFINITY, 2.0];
    material.detail_normal_uv_scale = [f32::NEG_INFINITY, 3.0, 4.0, f32::NAN];
    let prepared = prepare_material_for_draw_role(Some(&material), ModelMeshDrawRole::Normal);
    assert_eq!(
        material_detail_params(&material, prepared),
        [0.0, 0.0, 1.0, 0.0]
    );
    assert_eq!(material_detail_color(&material), [0.25, 0.5, 0.5, 0.5]);
    assert_eq!(material_multi_detail_color(&material), [0.5, 0.3, 0.5, 1.0]);
    assert_eq!(
        material_detail_color_uv_scale(&material),
        [1.0, 4.0, 4.0, 2.0]
    );
    assert_eq!(
        material_detail_normal_uv_scale(&material),
        [4.0, 3.0, 4.0, 4.0]
    );
}

#[test]
fn material_shader_colors_preserve_material_constants() {
    let mut material = fallback_material();
    assert_eq!(material_shader_diffuse_color(&material), [1.0; 4]);
    assert_eq!(material_shader_multi_diffuse_color(&material), [1.0; 4]);
    assert_eq!(
        material_shader_emissive_color(&material),
        [0.0, 0.0, 0.0, 1.0]
    );
    assert_eq!(
        material_shader_multi_emissive_color(&material),
        [0.0, 0.0, 0.0, 1.0]
    );

    material.shader_diffuse_color = [0.8, 0.7, 0.6, 0.5];
    material.shader_multi_diffuse_color = [0.6, 0.7, 0.8, 0.9];
    material.shader_emissive_color = [0.1, 0.2, 0.3, 1.0];
    material.shader_multi_emissive_color = [0.4, 0.5, 0.6, 1.0];
    assert_eq!(
        material_shader_diffuse_color(&material),
        [0.8, 0.7, 0.6, 0.5]
    );
    assert_eq!(
        material_shader_multi_diffuse_color(&material),
        [0.6, 0.7, 0.8, 0.9]
    );
    assert_eq!(
        material_shader_emissive_color(&material),
        [0.1, 0.2, 0.3, 1.0]
    );
    assert_eq!(
        material_shader_multi_emissive_color(&material),
        [0.4, 0.5, 0.6, 1.0]
    );

    material.shader_diffuse_color = [0.25, f32::NAN, f32::INFINITY, 0.5];
    material.shader_emissive_color = [f32::NEG_INFINITY, 0.2, 0.3, f32::NAN];
    assert_eq!(
        material_shader_diffuse_color(&material),
        [0.25, 1.0, 1.0, 0.5]
    );
    assert_eq!(
        material_shader_emissive_color(&material),
        [0.0, 0.2, 0.3, 1.0]
    );
}

#[test]
fn material_outline_specular_surface_params_preserve_shader_inputs() {
    let mut material = fallback_material();
    let unknown = prepare_material_for_draw_role(Some(&material), ModelMeshDrawRole::Normal);
    assert_eq!(material_outline_params(&material), [0.0, 0.0, 0.0, 0.0]);
    assert_eq!(material_specular_color_mask(&material), [1.0; 4]);
    assert_eq!(
        material_surface_params(&material, unknown),
        [1.0, 0.0, 0.0, 0.0]
    );

    material.shader_package_name = Some("character.shpk".to_string());
    material.outline_color = [0.1, 0.2, 0.3, 0.4];
    material.outline_width = 0.05;
    material.specular_color_mask = [0.7, 0.8, 0.9, 1.0];
    material.ssao_mask = 0.6;
    material.texture_mip_bias = -0.75;
    material.shadow_pos_offset = 0.125;
    assert_eq!(material_outline_params(&material), [0.1, 0.2, 0.3, 0.05]);
    assert_eq!(
        material_specular_color_mask(&material),
        [0.7, 0.8, 0.9, 1.0]
    );
    let character = prepare_material_for_draw_role(Some(&material), ModelMeshDrawRole::Normal);
    assert_eq!(
        material_surface_params(&material, character),
        [0.6, -0.75, 0.125, 1.0]
    );

    material.outline_color = [0.25, f32::NAN, f32::INFINITY, 0.5];
    material.outline_width = f32::NEG_INFINITY;
    material.specular_color_mask = [f32::NAN, 0.3, f32::INFINITY, 0.5];
    material.ssao_mask = f32::INFINITY;
    material.texture_mip_bias = f32::NAN;
    material.shadow_pos_offset = f32::NEG_INFINITY;
    assert_eq!(material_outline_params(&material), [0.25, 0.0, 0.0, 0.0]);
    assert_eq!(
        material_specular_color_mask(&material),
        [1.0, 0.3, 1.0, 0.5]
    );
    assert_eq!(
        material_surface_params(&material, character),
        [1.0, 0.0, 0.0, 1.0]
    );
}

#[test]
fn legacy_compatibility_specular_mode_is_key_aware() {
    let mut material = fallback_material();
    assert!(!material_is_character_legacy(&material));
    assert_eq!(material_family_params(&material), [0.0; 4]);
    assert_eq!(material_legacy_specular_mode(&material), 0.0);

    material.shader_package_name = Some("character.shpk".to_string());
    assert!(!material_is_character_legacy(&material));
    assert_eq!(material_family_params(&material), [0.0; 4]);
    material.color_table_rows = Some(vec![crate::ColorTableRowColors::default()]);
    assert_eq!(material_family_params(&material), [0.0, 1.0, 0.0, 0.0]);

    material.shader_package_name = Some("characterlegacy.shpk".to_string());
    assert!(material_is_character_legacy(&material));
    assert_eq!(material_family_params(&material), [1.0, 1.0, 0.0, 0.0]);
    material.value_mode = crate::MaterialValueMode::Compatibility;
    assert_eq!(material_legacy_specular_mode(&material), 1.0);

    material.specular_type = MaterialSpecularType::Mask;
    assert_eq!(material_legacy_specular_mode(&material), 2.0);

    material.specular_type = MaterialSpecularType::Unknown;
    assert_eq!(material_legacy_specular_mode(&material), 0.0);

    material.specular_type = MaterialSpecularType::Mask;
    material.value_mode = crate::MaterialValueMode::MultiMaterial;
    assert_eq!(material_legacy_specular_mode(&material), 0.0);

    material.shader_package_name = Some("CharacterLegacy.SHPK".to_string());
    assert!(material_is_character_legacy(&material));
    material.shader_package_name = Some("character.shpk".to_string());
    assert!(!material_is_character_legacy(&material));
}

#[test]
fn material_uv_scroll_preserves_scroll_multipliers() {
    let mut material = fallback_material();
    assert_eq!(material_uv_scroll(&material), [0.0; 4]);

    material.uv_scroll = [-10.0, 20.0, -30.0, 40.0];
    assert_eq!(material_uv_scroll(&material), [-10.0, 20.0, -30.0, 40.0]);

    material.uv_scroll = [1.0, f32::NAN, f32::INFINITY, 2.0];
    assert_eq!(material_uv_scroll(&material), [1.0, 0.0, 0.0, 2.0]);
}

#[test]
fn material_lightshaft_params_preserve_shader_inputs() {
    let mut material = fallback_material();
    assert_eq!(material_lightshaft_color(&material), [1.0; 4]);
    assert_eq!(material_lightshaft_tex_anim(&material), [0.0; 4]);
    assert_eq!(material_lightshaft_tex_u(&material), [1.0, 0.0, 0.0, 0.0]);
    assert_eq!(material_lightshaft_tex_v(&material), [0.0, 1.0, 0.0, 0.0]);
    assert_eq!(material_lightshaft_ray(&material), [0.0; 4]);
    assert_eq!(draw_role_params(ModelMeshDrawRole::Normal), [0.0; 4]);
    assert_eq!(
        draw_role_params(ModelMeshDrawRole::LightShaft),
        [1.0, 0.0, 0.0, 0.0]
    );
    assert_eq!(
        draw_role_params(ModelMeshDrawRole::CrestChange),
        [0.0, 1.0, 0.0, 0.0]
    );
    assert_eq!(
        draw_role_params(ModelMeshDrawRole::MaterialChange),
        [0.0, 0.0, 1.0, 0.0]
    );

    material.lightshaft_color = [0.2, 0.4, 0.6, 0.8];
    material.lightshaft_tex_anim = [0.1, 0.2, 0.3, 0.4];
    material.lightshaft_tex_u = [1.5, 0.5, 0.25, 0.0];
    material.lightshaft_tex_v = [0.25, 1.75, 0.5, 0.0];
    material.lightshaft_ray = [2.0, 3.0, 4.0, 5.0];
    assert_eq!(material_lightshaft_color(&material), [0.2, 0.4, 0.6, 0.8]);
    assert_eq!(
        material_lightshaft_tex_anim(&material),
        [0.1, 0.2, 0.3, 0.4]
    );
    assert_eq!(material_lightshaft_tex_u(&material), [1.5, 0.5, 0.25, 0.0]);
    assert_eq!(material_lightshaft_tex_v(&material), [0.25, 1.75, 0.5, 0.0]);
    assert_eq!(material_lightshaft_ray(&material), [2.0, 3.0, 4.0, 5.0]);

    material.lightshaft_color = [0.2, f32::NAN, f32::INFINITY, 0.8];
    material.lightshaft_tex_u = [f32::NAN, 0.5, f32::INFINITY, 0.0];
    assert_eq!(material_lightshaft_color(&material), [0.2, 1.0, 1.0, 0.8]);
    assert_eq!(material_lightshaft_tex_u(&material), [1.0, 0.5, 0.0, 0.0]);
}

#[test]
fn material_uv_source_params_preserve_prepared_texture_sources() {
    let prepared = PreparedMaterial {
        render_pass: PreparedRenderPass::Opaque,
        shader_family: MaterialShaderFamily::Character,
        flow_mode: MaterialFlowMode::Standard,
        value_mode: crate::MaterialValueMode::Single,
        value_mode_raw: None,
        sub_color_mode: crate::MaterialSubColorMode::None,
        decal_color_mode: crate::MaterialDecalColorMode::Off,
        decal_color_mode_raw: None,
        skin_value_mode: crate::MaterialSkinValueMode::None,
        lightshaft_type: crate::MaterialLightShaftType::None,
        lightshaft_type_raw: None,
        alpha_policy: crate::PreparedMaterialAlphaPolicy::default(),
        texture_bindings: PreparedTextureBindings::default(),
        texture_sampling: PreparedTextureSamplingSet::default(),
        uv_sources: PreparedMaterialUvSources {
            textures: PreparedTextureUvSources {
                base_color: PreparedUvSource::Uv0,
                secondary_base_color: PreparedUvSource::Uv0,
                normal: PreparedUvSource::Uv1,
                secondary_normal: PreparedUvSource::Uv0,
                mask: PreparedUvSource::Uv2,
                skin_diffuse: PreparedUvSource::Uv2,
                skin_normal: PreparedUvSource::Uv2,
                skin_mask: PreparedUvSource::Uv2,
                material_map: PreparedUvSource::Uv3,
                multi_map: PreparedUvSource::Uv3,
                specular: PreparedUvSource::Uv2,
                secondary_specular: PreparedUvSource::Uv0,
                emissive: PreparedUvSource::Uv1,
                environment: PreparedUvSource::Uv0,
                material_properties: PreparedUvSource::Uv0,
                tile_properties: PreparedUvSource::Uv1,
                sheen_properties: PreparedUvSource::Uv2,
                sphere_properties: PreparedUvSource::Uv3,
                tile_matrix: PreparedUvSource::Uv0,
                index: PreparedUvSource::Uv1,
                other: PreparedUvSource::Uv2,
            },
            scroll: PreparedTextureScrollSet {
                base_color: true,
                normal: true,
                specular: true,
                ..PreparedTextureScrollSet::default()
            },
            uv0_scroll: PreparedUvSource::Uv0,
            uv1_scroll: PreparedUvSource::Uv1,
        },
        feature_flags: PreparedMaterialFeatureFlags::default(),
        unsupported_inputs: PreparedMaterialUnsupportedInputs::default(),
        resource_availability: PreparedMaterialResourceAvailability::default(),
        runtime_fallbacks: PreparedMaterialRuntimeFallbacks::default(),
        runtime_input_requirements: PreparedMaterialRuntimeInputRequirements::default(),
        render_backfaces: true,
    };

    assert_eq!(
        material_uv_source_params(prepared),
        (
            [0.0, 1.0, 2.0, 3.0],
            [3.0, 2.0, 1.0, 0.0],
            [1.0, 2.0, 3.0, 0.0],
            [1.0, 2.0, 0.0, 0.0]
        )
    );
    assert_eq!(
        material_uv_scroll_mask_params(prepared),
        (
            [1.0, 1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0; 4],
            [0.0; 4],
        )
    );
}

#[test]
fn material_feature_params_encode_prepared_flow_policy() {
    let mut prepared = prepare_material_for_draw_role(None, ModelMeshDrawRole::Normal);
    assert_eq!(material_feature_params(prepared), [0.0; 4]);

    prepared.feature_flags.uses_flow = true;
    assert_eq!(material_feature_params(prepared), [1.0, 0.0, 0.0, 0.0]);

    prepared.shader_family = MaterialShaderFamily::Water;
    assert_eq!(material_feature_params(prepared), [1.0, 1.0, 0.0, 0.0]);
}

#[test]
fn secondary_scroll_maps_reuse_extra_bindings_with_presence_flags() {
    let mut material = fallback_material();
    material.shader_package_name = Some("bguvscroll.shpk".to_string());
    material.value_mode = crate::MaterialValueMode::Multi;
    material.secondary_base_color_texture = Some(0);
    material.secondary_normal_texture = Some(1);
    material.secondary_specular_texture = Some(99);
    let model = crate::ModelData {
        bounds: crate::ModelBounds::default(),
        materials: vec![material.clone()],
        textures: vec![
            test_texture(crate::ModelTextureKind::SecondaryBaseColor),
            test_texture(crate::ModelTextureKind::SecondaryNormal),
        ],
        meshes: Vec::new(),
    };
    let prepared = prepare_material_for_draw_role(Some(&material), ModelMeshDrawRole::Normal);

    assert!(prepared.feature_flags.uses_secondary_maps);
    assert_eq!(material_feature_params(prepared), [0.0, 0.0, 1.0, 1.0]);
    assert_eq!(
        material_secondary_map_params(&material, &model, prepared),
        [1.0, 1.0, 0.0, 1.0]
    );
    assert_eq!(
        material_extra_texture_flags(&material, &model, prepared),
        [0.0; 4]
    );
    assert_eq!(material_uv_source_params(prepared).2, [1.0, 1.0, 1.0, 0.0]);
    assert_eq!(
        material_uv_scroll_mask_params(prepared).2,
        [1.0, 1.0, 1.0, 0.0]
    );
}

#[test]
fn lightshaft_sampler1_reuses_only_the_secondary_color_binding() {
    let mut material = fallback_material();
    material.shader_package_name = Some("lightshaft.shpk".to_string());
    material.secondary_base_color_texture = Some(0);
    material.secondary_normal_texture = Some(1);
    let model = crate::ModelData {
        bounds: crate::ModelBounds::default(),
        materials: vec![material.clone()],
        textures: vec![
            test_texture(crate::ModelTextureKind::SecondaryBaseColor),
            test_texture(crate::ModelTextureKind::SecondaryNormal),
        ],
        meshes: Vec::new(),
    };
    let prepared = prepare_material_for_draw_role(Some(&material), ModelMeshDrawRole::LightShaft);

    assert!(prepared.feature_flags.uses_secondary_maps);
    assert!(!prepared.feature_flags.uses_scroll);
    assert_eq!(material_feature_params(prepared), [0.0, 0.0, 1.0, 0.0]);
    assert_eq!(
        material_secondary_map_params(&material, &model, prepared),
        [1.0, 0.0, 0.0, 0.0]
    );
}

#[test]
fn gpu_vertex_layout_exposes_extended_model_channels() {
    let layout = GpuVertex::layout();

    assert_eq!(
        layout.array_stride,
        std::mem::size_of::<GpuVertex>() as wgpu::BufferAddress
    );
    assert_eq!(layout.step_mode, wgpu::VertexStepMode::Vertex);
    assert_eq!(
        layout
            .attributes
            .iter()
            .map(|attribute| (
                attribute.shader_location,
                attribute.offset,
                attribute.format
            ))
            .collect::<Vec<_>>(),
        vec![
            (
                0,
                std::mem::offset_of!(GpuVertex, position) as wgpu::BufferAddress,
                wgpu::VertexFormat::Float32x3
            ),
            (
                1,
                std::mem::offset_of!(GpuVertex, normal) as wgpu::BufferAddress,
                wgpu::VertexFormat::Float32x3
            ),
            (
                2,
                std::mem::offset_of!(GpuVertex, uv0) as wgpu::BufferAddress,
                wgpu::VertexFormat::Float32x2
            ),
            (
                3,
                std::mem::offset_of!(GpuVertex, bitangent) as wgpu::BufferAddress,
                wgpu::VertexFormat::Float32x4
            ),
            (
                4,
                std::mem::offset_of!(GpuVertex, color) as wgpu::BufferAddress,
                wgpu::VertexFormat::Float32x4
            ),
            (
                5,
                std::mem::offset_of!(GpuVertex, uv1) as wgpu::BufferAddress,
                wgpu::VertexFormat::Float32x2
            ),
            (
                6,
                std::mem::offset_of!(GpuVertex, uv2) as wgpu::BufferAddress,
                wgpu::VertexFormat::Float32x2
            ),
            (
                7,
                std::mem::offset_of!(GpuVertex, uv3) as wgpu::BufferAddress,
                wgpu::VertexFormat::Float32x2
            ),
            (
                8,
                std::mem::offset_of!(GpuVertex, color1) as wgpu::BufferAddress,
                wgpu::VertexFormat::Float32x4
            ),
            (
                9,
                std::mem::offset_of!(GpuVertex, normal1) as wgpu::BufferAddress,
                wgpu::VertexFormat::Float32x3
            ),
            (
                10,
                std::mem::offset_of!(GpuVertex, bitangent1) as wgpu::BufferAddress,
                wgpu::VertexFormat::Float32x4
            ),
            (
                11,
                std::mem::offset_of!(GpuVertex, flow0) as wgpu::BufferAddress,
                wgpu::VertexFormat::Float32x4
            ),
            (
                12,
                std::mem::offset_of!(GpuVertex, flow1) as wgpu::BufferAddress,
                wgpu::VertexFormat::Float32x4
            ),
            (
                13,
                std::mem::offset_of!(GpuVertex, joints) as wgpu::BufferAddress,
                wgpu::VertexFormat::Uint32x2
            ),
            (
                14,
                std::mem::offset_of!(GpuVertex, weights0) as wgpu::BufferAddress,
                wgpu::VertexFormat::Float32x4
            ),
            (
                15,
                std::mem::offset_of!(GpuVertex, weights1) as wgpu::BufferAddress,
                wgpu::VertexFormat::Float32x4
            ),
        ]
    );
    assert_eq!(
        std::mem::size_of::<GpuVertex>(),
        204,
        "u32 槽位排在 f32 前，全部字段 4 字节对齐、无填充"
    );
}

#[test]
fn flatten_model_filters_non_surface_roles_but_keeps_additive_lightshafts() {
    let model = crate::ModelData {
        bounds: crate::ModelBounds::default(),
        materials: vec![fallback_material()],
        textures: Vec::new(),
        meshes: vec![
            test_mesh("normal", 0.0),
            test_mesh("shadow", 1.0),
            test_mesh("terrainShadow", 2.0),
            test_mesh("verticalFog", 3.0),
            test_mesh("lightShaft", 4.0),
            test_mesh("materialChange", 5.0),
            test_mesh("glass", 6.0),
        ],
    };

    let (vertices, indices, batches) = flatten_model(&model);

    assert_eq!(vertices.len(), 12);
    assert_eq!(indices.len(), 12);
    assert_eq!(batches.len(), 4);
    assert_eq!(batches[0].pass(), PreparedRenderPass::Opaque);
    assert_eq!(batches[1].pass(), PreparedRenderPass::AdditiveLightShaft);
    assert_eq!(batches[2].pass(), PreparedRenderPass::Opaque);
    assert_eq!(batches[3].pass(), PreparedRenderPass::Glass);
    assert_eq!(
        batches
            .iter()
            .map(|batch| batch.draw_role)
            .collect::<Vec<_>>(),
        vec![
            ModelMeshDrawRole::Normal,
            ModelMeshDrawRole::LightShaft,
            ModelMeshDrawRole::MaterialChange,
            ModelMeshDrawRole::Glass
        ]
    );
}

#[test]
fn flatten_model_skips_hidden_meshes_entirely() {
    let model = crate::ModelData {
        bounds: crate::ModelBounds::default(),
        materials: vec![fallback_material()],
        textures: Vec::new(),
        meshes: vec![
            test_mesh("normal", 0.0),
            test_mesh("glass", 3.0),
            test_mesh("normal", 6.0),
        ],
    };

    // 隐藏下标 1（glass）：连同透明批次一并跳过，其余网格照常展平。
    let (vertices, indices, batches) = flatten_model_with_options(
        &model,
        PreparedModelOptions::default().with_hidden_mesh_indices(vec![1]),
    );

    assert_eq!(vertices.len(), 6);
    assert_eq!(indices.len(), 6);
    assert_eq!(batches.len(), 2);
    assert!(
        batches
            .iter()
            .all(|batch| batch.pass() == PreparedRenderPass::Opaque)
    );
    // 不隐藏时 glass 批次在场（对照，确保隐藏是标签生效而非数据差异）。
    let (_, _, all_batches) = flatten_model(&model);
    assert_eq!(all_batches.len(), 3);
}

#[test]
fn flatten_model_preserves_extended_vertex_channels() {
    let mut mesh = test_mesh("normal", 0.0);
    mesh.vertices[0].uv1 = [0.1, 0.2];
    mesh.vertices[0].uv2 = [0.3, 0.4];
    mesh.vertices[0].uv3 = [0.5, 0.6];
    mesh.vertices[0].color1 = Some([0.7, 0.8, 0.9, 1.0]);
    mesh.vertices[0].normal1 = Some([1.0, 0.0, 0.0]);
    mesh.vertices[0].bitangent1 = Some([0.0, 1.0, 0.0, -1.0]);
    mesh.vertices[0].flow0 = Some([0.11, 0.22, 0.33, 0.44]);
    mesh.vertices[0].flow1 = Some([0.55, 0.66, 0.77, 0.88]);
    let model = crate::ModelData {
        bounds: crate::ModelBounds::default(),
        materials: vec![fallback_material()],
        textures: Vec::new(),
        meshes: vec![mesh],
    };

    let (vertices, _, _) = flatten_model(&model);

    assert_eq!(vertices[0].uv1, [0.1, 0.2]);
    assert_eq!(vertices[0].uv2, [0.3, 0.4]);
    assert_eq!(vertices[0].uv3, [0.5, 0.6]);
    assert_eq!(vertices[0].color1, [0.7, 0.8, 0.9, 1.0]);
    assert_eq!(vertices[0].normal1, [1.0, 0.0, 0.0]);
    assert_eq!(vertices[0].bitangent1, [0.0, 1.0, 0.0, -1.0]);
    assert_eq!(vertices[0].flow0, [0.11, 0.22, 0.33, 0.44]);
    assert_eq!(vertices[0].flow1, [0.55, 0.66, 0.77, 0.88]);
    assert_eq!(vertices[1].color1, [1.0, 1.0, 1.0, 1.0]);
    assert_eq!(vertices[1].normal1, [0.0, 1.0, 0.0]);
    assert_eq!(vertices[1].bitangent1, [1.0, 0.0, 0.0, 1.0]);
    assert_eq!(vertices[1].flow0, [0.0; 4]);
    assert_eq!(vertices[1].flow1, [0.0; 4]);
    // 无蒙皮数据的默认槽位：joint 0 + 全重 1.0 在 slot0（joint 数 0 的旧
    // shader 分支下不读取，渲染与非蒙皮路径逐位一致）。
    assert_eq!(vertices[0].joints, [0; 2]);
    assert_eq!(vertices[0].weights0, [1.0, 0.0, 0.0, 0.0]);
    assert_eq!(vertices[0].weights1, [0.0; 4]);
}

fn skinned_test_mesh() -> crate::ModelMesh {
    let mut mesh = test_mesh("normal", 0.0);
    mesh.submesh = Some(xiv_companion_data::ModelSubmeshInfo {
        index: 0,
        table_index: 0,
        attribute_index_mask: 0,
        attribute_index_mask_hex: "0x0".to_string(),
        attribute_names: Vec::new(),
        bone_start_index: 0,
        bone_count: 2,
    });
    mesh.bone_table = Some(xiv_companion_data::ModelBoneTable {
        index: 0,
        bone_count: 2,
        bone_indices: vec![0, 1],
        bone_names: vec![Some("n_root".to_string()), Some("n_spine".to_string())],
    });
    mesh.vertices[0].blend_weights = Some(xiv_companion_data::ModelBlendWeights {
        count: 2,
        values: [0.8, 0.2, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    });
    mesh.vertices[0].blend_indices = Some(xiv_companion_data::ModelBlendIndices {
        count: 2,
        values: [0, 1, 0, 0, 0, 0, 0, 0],
    });
    mesh.vertices[1].blend_weights = Some(xiv_companion_data::ModelBlendWeights {
        count: 1,
        values: [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    });
    mesh.vertices[1].blend_indices = Some(xiv_companion_data::ModelBlendIndices {
        count: 1,
        values: [1, 0, 0, 0, 0, 0, 0, 0],
    });
    mesh
}

fn skinned_test_skeleton() -> xiv_companion_data::ModelSkeleton {
    xiv_companion_data::ModelSkeleton {
        bone_names: vec!["n_root".to_string(), "n_spine".to_string()],
        parent_indices: vec![-1, 0],
        rest_pose: vec![xiv_companion_data::BoneTransform::IDENTITY; 2],
        body_scaling: None,
    }
}

#[test]
fn flatten_model_with_skeleton_builds_joint_table_and_remaps_vertices() {
    let model = crate::ModelData {
        bounds: crate::ModelBounds::default(),
        materials: vec![fallback_material()],
        textures: Vec::new(),
        meshes: vec![skinned_test_mesh()],
    };
    let skeleton = skinned_test_skeleton();

    let FlattenedModel {
        vertices,
        joint_names,
        ..
    } = flatten_model_with_options_and_skeleton(
        &model,
        PreparedModelOptions::default(),
        Some(&skeleton),
    );

    assert_eq!(joint_names, ["n_root", "n_spine"]);
    // 顶点 0：双骨权重按 bone_table 绝对下标 0/1 映射到 joint 0/1，
    // 原始量化权重原样传递（GPU 按和归一）；8 个 u8 索引低字节在前打包。
    assert_eq!(
        vertices[0].joints,
        [
            u32::from_le_bytes([0, 1, 0, 0]),
            u32::from_le_bytes([0, 0, 0, 0])
        ]
    );
    assert_eq!(vertices[0].weights0, [0.8, 0.2, 0.0, 0.0]);
    // 顶点 1：单骨 n_spine → joint 1。
    assert_eq!(vertices[1].joints, [u32::from_le_bytes([1, 0, 0, 0]), 0]);
    assert_eq!(vertices[1].weights0, [1.0, 0.0, 0.0, 0.0]);
    // 顶点 2 无 blend 数据 → 默认槽位。
    assert_eq!(vertices[2].joints, [0; 2]);
    assert_eq!(vertices[2].weights0, [1.0, 0.0, 0.0, 0.0]);
}

#[test]
fn flatten_model_with_skeleton_merges_bone_names_across_meshes() {
    let mut second = skinned_test_mesh();
    second.bone_table = Some(xiv_companion_data::ModelBoneTable {
        index: 0,
        bone_count: 1,
        bone_indices: vec![1],
        bone_names: vec![Some("n_spine".to_string())],
    });
    let model = crate::ModelData {
        bounds: crate::ModelBounds::default(),
        materials: vec![fallback_material()],
        textures: Vec::new(),
        meshes: vec![skinned_test_mesh(), second],
    };
    let skeleton = skinned_test_skeleton();

    let FlattenedModel {
        vertices,
        joint_names,
        ..
    } = flatten_model_with_options_and_skeleton(
        &model,
        PreparedModelOptions::default(),
        Some(&skeleton),
    );

    // 跨 mesh 同名骨骼只进一次 joint 表；第二个 mesh 的 n_spine 引用映射到 joint 1。
    assert_eq!(joint_names, ["n_root", "n_spine"]);
    assert_eq!(vertices[3].joints, [u32::from_le_bytes([1, 0, 0, 0]), 0]);
}

#[test]
fn remap_vertex_skinning_uses_absolute_bone_table_index() {
    let mut vertex = test_vertex([0.0; 3]);
    vertex.blend_weights = Some(xiv_companion_data::ModelBlendWeights {
        count: 1,
        values: [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    });
    vertex.blend_indices = Some(xiv_companion_data::ModelBlendIndices {
        count: 1,
        values: [2, 0, 0, 0, 0, 0, 0, 0],
    });
    let bone_table = xiv_companion_data::ModelBoneTable {
        index: 0,
        bone_count: 4,
        bone_indices: vec![0, 1, 2, 3],
        bone_names: vec![
            Some("a".to_string()),
            Some("b".to_string()),
            Some("c".to_string()),
            Some("d".to_string()),
        ],
    };
    // blend 索引是 bone_table 的绝对下标（d0001e0001_dwn 实测：submesh
    // 窗口基址 bone_start_index 不参与重映射，按窗口偏移会把网格撕碎）。
    // 索引 2 → bone_table[2] = "c"，与 submesh 声明的窗口无关。
    let joint_names = vec![
        "x".to_string(),
        "y".to_string(),
        "c".to_string(),
        "z".to_string(),
    ];
    let (joints, weights) = remap_vertex_skinning(&vertex, &bone_table, &joint_names);
    assert_eq!(joints[0], 2);
    assert_eq!(weights[0], 1.0);
}

#[test]
fn flatten_model_separates_independent_preview_components() {
    let model = std::rc::Rc::new(ComponentTestModel {
        data: crate::ModelData {
            bounds: crate::ModelBounds::default(),
            materials: vec![fallback_material()],
            textures: Vec::new(),
            meshes: vec![test_mesh("normal", 0.0), test_mesh("normal", 0.0)],
        },
        components: vec![0, 1],
    });

    let (vertices, _, _) = flatten_model(&model);
    let primary_max = vertices[..3]
        .iter()
        .map(|vertex| vertex.position[0])
        .fold(f32::NEG_INFINITY, f32::max);
    let secondary_min = vertices[3..]
        .iter()
        .map(|vertex| vertex.position[0])
        .fold(f32::INFINITY, f32::min);

    assert!(
        secondary_min - primary_max >= 0.04,
        "components must keep a visible preview gap"
    );
}

#[test]
fn flatten_model_overlaps_components_when_preview_layout_disabled() {
    // 角色拼装：component_preview_layout=false 时全部件原位重叠（全零偏移）。
    let model = std::rc::Rc::new(ComponentTestModel {
        data: crate::ModelData {
            bounds: crate::ModelBounds::default(),
            materials: vec![fallback_material()],
            textures: Vec::new(),
            meshes: vec![test_mesh("normal", 0.0), test_mesh("normal", 5.0)],
        },
        components: vec![0, 1],
    });

    let (vertices, _, _) = flatten_model_with_options(
        &model,
        PreparedModelOptions::default().with_component_preview_layout(false),
    );
    assert_eq!(vertices[0].position, [0.0, 0.0, 0.0]);
    assert_eq!(vertices[3].position, [5.0, 0.0, 0.0]);

    let (vertices, _, _) = flatten_model_with_options(
        &model,
        PreparedModelOptions::default().with_component_preview_layout(true),
    );
    assert!(
        (vertices[3].position[0] - 5.0).abs() > 0.04,
        "layout enabled must spread components apart from their source positions"
    );
}

#[test]
fn mdl_attachment_offsets_match_flattened_vertices_and_shape_layout() {
    let mut primary = test_mesh("normal", 0.0);
    primary.path = "main.mdl#part-0".into();
    let shape = crate::ModelShapeInfo {
        index: 0,
        name: Some("extended".into()),
        shape_index_mask: 1,
        shape_index_mask_hex: "0x1".into(),
        shape_mesh_index: 0,
        shape_value_count: 1,
    };
    primary.shape_influences.push(shape.clone());
    primary.shape_targets.push(crate::ModelShapeTarget {
        shape,
        vertex_deltas: vec![crate::ModelShapeVertexDelta {
            vertex_index: 0,
            position: [5.0, 0.0, 0.0],
            normal: [0.0; 3],
        }],
    });
    let mut secondary = test_mesh("normal", 4.0);
    secondary.path = "sub.mdl#part-0".into();
    let model = ComponentTestModel {
        data: crate::ModelData {
            bounds: Default::default(),
            materials: vec![fallback_material()],
            textures: Vec::new(),
            meshes: vec![primary, secondary],
        },
        components: vec![0, 1],
    };
    let mut primary_offsets = Vec::new();
    for (layout, shape) in [(true, 0), (true, 1), (false, 1)] {
        let flattened = flatten_model_with_options_and_skeleton(
            &model,
            PreparedModelOptions::default()
                .with_component_preview_layout(layout)
                .with_enabled_shape_mask(shape),
            None,
        );
        for (mesh_index, path) in ["main.mdl", "sub.mdl"].into_iter().enumerate() {
            let offset = flattened.mdl_preview_offsets[path];
            assert_eq!(flattened.draw_batches[mesh_index].model_path, path);
            if !layout {
                assert_eq!(offset, [0.0; 3]);
            }
            let source =
                model_mesh_vertices_with_shape_mask(&model.data.meshes[mesh_index], Some(shape));
            for (index, vertex) in source.iter().enumerate() {
                let actual = flattened.vertices[mesh_index * 3 + index].position;
                for axis in 0..3 {
                    assert!((actual[axis] - vertex.position[axis] - offset[axis]).abs() < 1e-6);
                }
            }
        }
        primary_offsets.push(flattened.mdl_preview_offsets["main.mdl"]);
        assert!(!flattened.mdl_preview_offsets.contains_key("unrelated.mdl"));
    }
    assert_ne!(
        primary_offsets[0], primary_offsets[1],
        "shape changes must update attachment placement"
    );
}

#[test]
fn material_character_color_channels_follow_shader_family() {
    let appearance = crate::CharacterAppearanceColors {
        skin: [0.60, 0.45, 0.35, 1.0],
        lip: [0.70, 0.30, 0.30, 0.80],
        main: [0.20, 0.15, 0.10, 1.0],
        mesh: [0.90, 0.85, 0.70, 1.0],
        left_iris: [0.10, 0.50, 0.20, 1.0],
        right_iris: [0.50, 0.20, 0.60, 1.0],
        option: [0.40, 0.60, 0.80, 1.0],
        decal: [0.30, 0.30, 0.35, 0.70],
        lipstick: true,
        highlights: true,
        face_paint_reversed: true,
    };
    let mut material = fallback_material();
    assert_eq!(
        material_character_color_channels(&material),
        CharacterColorChannels::default(),
        "materials without character colors keep every channel zeroed"
    );

    material.character_colors = Some(crate::ModelMaterialCharacterColors {
        colors: appearance,
        decal_texture: Some(3),
    });
    // 非消费 family（character）不写通道。
    material.shader_package_name = Some("character.shpk".to_string());
    assert_eq!(
        material_character_color_channels(&material),
        CharacterColorChannels::default()
    );

    // skin family 全收：脸材质吃唇妆与面妆。
    material.shader_package_name = Some("skin.shpk".to_string());
    material.path =
        Some("chara/human/c0101/obj/face/f0001/material/mt_c0101f0001_fac_a.mtrl".to_string());
    let channels = material_character_color_channels(&material);
    assert_eq!(channels.skin, [0.60, 0.45, 0.35, 1.0]);
    assert_eq!(channels.lip, [0.70, 0.30, 0.30, 0.80]);
    assert_eq!(channels.decal, [0.30, 0.30, 0.35, 0.70]);
    assert_eq!(channels.decal_uv, [1.0, 0.0, 1.0, 1.0]);
    assert_eq!(channels.main, [0.0; 4]);
    assert_eq!(channels.option, [0.0; 4]);
    // 身体 skin 材质不吃唇妆（无唇形遮罩）。
    material.path =
        Some("chara/human/c0101/obj/body/b0001/material/v0001/mt_c0101b0001_a.mtrl".to_string());
    let channels = material_character_color_channels(&material);
    assert_eq!(
        channels.lip, [0.0; 4],
        "body skin must not consume lip color"
    );
    assert_eq!(channels.skin, [0.60, 0.45, 0.35, 1.0]);
    // lipstick 关闭：脸材质也不吃唇妆。
    material.path =
        Some("chara/human/c0101/obj/face/f0001/material/mt_c0101f0001_fac_a.mtrl".to_string());
    material.character_colors = Some(crate::ModelMaterialCharacterColors {
        colors: crate::CharacterAppearanceColors {
            lipstick: false,
            ..appearance
        },
        decal_texture: None,
    });
    let channels = material_character_color_channels(&material);
    assert_eq!(channels.lip, [0.0; 4]);
    assert_eq!(
        channels.decal, [0.0; 4],
        "missing decal texture keeps decal inactive"
    );
    assert_eq!(channels.decal_uv, [0.0; 4]);

    // hair family：收发色与挑染；mask 细节标记按材质路径（脸部 hair 材质关闭）。
    material.shader_package_name = Some("hair.shpk".to_string());
    material.path = Some(
        "chara/human/c1801/obj/hair/h0002/material/v0001/mt_c1801h0002_hir_a.mtrl".to_string(),
    );
    material.character_colors = Some(crate::ModelMaterialCharacterColors {
        colors: appearance,
        decal_texture: None,
    });
    let channels = material_character_color_channels(&material);
    assert_eq!(channels.main, [0.20, 0.15, 0.10, 1.0]);
    assert_eq!(channels.mesh, [0.90, 0.85, 0.70, 1.0]);
    assert_eq!(channels.skin, [0.0; 4]);
    assert_eq!(
        channels.params,
        [1.0, 0.0, 0.0, 0.0],
        "hair part uses mask R channel as AO detail"
    );
    // 脸部 hair 材质（眉毛/睫毛）：mask R 同样是明暗细节图（眉发丝纹理 +
    // 睫毛区压暗），与头发一致走 mask 细节。
    material.path =
        Some("chara/human/c1801/obj/face/f0001/material/mt_c1801f0001_etc_a.mtrl".to_string());
    assert_eq!(
        material_character_color_channels(&material).params,
        [1.0, 0.0, 0.0, 0.0]
    );
    // highlights 关闭：挑染通道不激活。
    material.character_colors = Some(crate::ModelMaterialCharacterColors {
        colors: crate::CharacterAppearanceColors {
            highlights: false,
            ..appearance
        },
        decal_texture: None,
    });
    assert_eq!(material_character_color_channels(&material).mesh, [0.0; 4]);

    // iris family：左右眼色，激活标记取角膜环强度。
    material.shader_package_name = Some("iris.shpk".to_string());
    material.character_colors = Some(crate::ModelMaterialCharacterColors {
        colors: appearance,
        decal_texture: None,
    });
    let channels = material_character_color_channels(&material);
    assert_eq!(channels.left_iris, [0.10, 0.50, 0.20, 1.0]);
    assert_eq!(channels.right_iris, [0.50, 0.20, 0.60, 1.0]);

    // charactertattoo family：收特征色。
    material.shader_package_name = Some("charactertattoo.shpk".to_string());
    let channels = material_character_color_channels(&material);
    assert_eq!(channels.option, [0.40, 0.60, 0.80, 1.0]);
}

#[test]
fn flatten_model_applies_only_explicitly_enabled_shape_targets() {
    let mut mesh = test_mesh("normal", 0.0);
    let shape = crate::ModelShapeInfo {
        index: 0,
        name: Some("shape_a".to_string()),
        shape_index_mask: 1,
        shape_index_mask_hex: "0x00000001".to_string(),
        shape_mesh_index: 0,
        shape_value_count: 1,
    };
    mesh.shape_influences.push(shape.clone());
    mesh.shape_targets.push(crate::ModelShapeTarget {
        shape,
        vertex_deltas: vec![crate::ModelShapeVertexDelta {
            vertex_index: 0,
            position: [5.0, 0.0, 0.0],
            normal: [0.0, 0.0, 0.0],
        }],
    });
    let model = crate::ModelData {
        bounds: crate::ModelBounds::default(),
        materials: vec![fallback_material()],
        textures: Vec::new(),
        meshes: vec![mesh],
    };

    let (base_vertices, _, _) = flatten_model(&model);
    let (shaped_vertices, _, batches) = flatten_model_with_options(
        &model,
        PreparedModelOptions::default().with_enabled_shape_mask(1),
    );

    assert_eq!(base_vertices[0].position, [0.0, 0.0, 0.0]);
    assert_eq!(shaped_vertices[0].position, [5.0, 0.0, 0.0]);
    assert_eq!(batches.len(), 1);
}

fn test_prepared_render_pass(
    alpha_mode: MaterialAlphaMode,
    render_mode: MaterialRenderMode,
    draw_role: ModelMeshDrawRole,
) -> PreparedRenderPass {
    let mut material = fallback_material();
    material.alpha_mode = alpha_mode;
    material.render_mode = render_mode;
    let model = crate::ModelData {
        bounds: crate::ModelBounds::default(),
        materials: vec![material],
        textures: Vec::new(),
        meshes: Vec::new(),
    };
    prepare_material_for_draw_role(model.materials().first(), draw_role).render_pass
}

fn test_batch(material_slot: usize, pass: PreparedRenderPass, center: [f32; 3]) -> DrawBatch {
    let transparent_triangles = pass.sorts_back_to_front().then(|| TransparentTriangle {
        indices: [
            material_slot as u32 * 3,
            material_slot as u32 * 3 + 1,
            material_slot as u32 * 3 + 2,
        ],
        center,
    });
    DrawBatch {
        model_path: "test.mdl".to_string(),
        material_slot,
        material_bind_group_index: material_slot,
        aura_surface_compatible: false,
        draw_role: ModelMeshDrawRole::Normal,
        index_start: 0,
        index_count: 3,
        prepared_material: PreparedMaterial {
            render_pass: pass,
            shader_family: MaterialShaderFamily::Unknown,
            flow_mode: MaterialFlowMode::Standard,
            value_mode: crate::MaterialValueMode::Single,
            value_mode_raw: None,
            sub_color_mode: crate::MaterialSubColorMode::None,
            decal_color_mode: crate::MaterialDecalColorMode::Off,
            decal_color_mode_raw: None,
            skin_value_mode: crate::MaterialSkinValueMode::None,
            lightshaft_type: crate::MaterialLightShaftType::None,
            lightshaft_type_raw: None,
            alpha_policy: crate::PreparedMaterialAlphaPolicy::default(),
            texture_bindings: PreparedTextureBindings::default(),
            texture_sampling: PreparedTextureSamplingSet::default(),
            uv_sources: PreparedMaterialUvSources::default(),
            feature_flags: PreparedMaterialFeatureFlags::default(),
            unsupported_inputs: PreparedMaterialUnsupportedInputs::default(),
            resource_availability: PreparedMaterialResourceAvailability::default(),
            runtime_fallbacks: PreparedMaterialRuntimeFallbacks::default(),
            runtime_input_requirements: PreparedMaterialRuntimeInputRequirements::default(),
            render_backfaces: true,
        },
        transparent_triangles: transparent_triangles.into_iter().collect(),
    }
}

fn test_sampling(
    color_space: PreparedTextureColorSpace,
    filter: PreparedTextureFilter,
    address_mode: PreparedTextureAddressMode,
) -> PreparedTextureSampling {
    PreparedTextureSampling {
        color_space,
        filter,
        address_mode,
    }
}

pub(super) fn test_mesh(category: &str, x: f32) -> crate::ModelMesh {
    crate::ModelMesh {
        path: format!("test/{category}.mdl"),
        part_index: 0,
        mesh_category: Some(category.to_string()),
        submesh: None,
        shape_influences: Vec::new(),
        shape_targets: Vec::new(),
        material_index: 0,
        material_slot: 0,
        material_name: "test".to_string(),
        color: [1.0, 1.0, 1.0],
        bone_table: None,
        vertices: vec![
            test_vertex([x, 0.0, 0.0]),
            test_vertex([x + 1.0, 0.0, 0.0]),
            test_vertex([x, 1.0, 0.0]),
        ],
        indices: vec![0, 1, 2],
    }
}

fn test_vertex(position: [f32; 3]) -> crate::ModelVertex {
    crate::ModelVertex {
        position,
        blend_weights: None,
        blend_indices: None,
        normal: [0.0, 1.0, 0.0],
        uv0: [0.0, 0.0],
        uv1: [0.0, 0.0],
        uv2: [0.0, 0.0],
        uv3: [0.0, 0.0],
        bitangent: [1.0, 0.0, 0.0, 1.0],
        normal1: None,
        bitangent1: None,
        color: [1.0, 1.0, 1.0, 1.0],
        color1: None,
        flow0: None,
        flow1: None,
    }
}

fn test_texture(kind: crate::ModelTextureKind) -> crate::ModelTexture {
    crate::ModelTexture {
        path: "test.tex".to_string(),
        kind,
        texel_layout: crate::ModelTextureTexelLayout::Standard,
        width: 1,
        height: 1,
        array_size: 1,
        array_layer_height: 1,
        rgba: vec![0, 0, 0, 255],
        rgba_f32: None,
    }
}
