#![cfg(feature = "render-test-support")]

//! 角色材质渲染点亮的本地真实数据验证（需 XIV_GAME_DIR 指向游戏安装目录）。
//!
//! 用默认捏脸（assets/character-make.json）+ human.cmp 调色板
//! （assets/character-palette.json）组装中原男/猫魅女/敖龙女/维埃拉女，携带
//! `CharacterAppearanceColors` 渲染快照：肤色/发色/眼色/特征色/面妆经
//! skin/hair/iris/charactertattoo shader 家族分支上色，全部件原位重叠
//! （`component_preview_layout = false`）。attribute 显隐按
//! [`character_enabled_attribute_names`] 的按名启用集合（位是 MDL 本地表序，
//! 跨模型数值掩码无意义）。

#[cfg(feature = "game-data")]
use physis::resource::SqPackResource;
#[cfg(feature = "game-data")]
use xiv_companion::{
    CharacterAssemblyLoadRequest, CharacterPalettePackage, PreparedModelOptions,
    appearance_colors_from_palette, character_enabled_attribute_names,
    default_customize_for_race_code, load_character_assembly_from_resource,
};
#[cfg(feature = "game-data")]
use xiv_companion_render::test_support::{
    ModelSnapshotOptions, render_model_snapshot_with_options,
};

#[cfg(feature = "game-data")]
fn game_dir() -> String {
    std::env::var("XIV_GAME_DIR").unwrap_or_else(|_| r"E:\_ff14\game".to_string())
}

#[cfg(feature = "game-data")]
fn load_make_package() -> xiv_companion::CharacterMakePackage {
    let path =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/character-make.json");
    let bytes = std::fs::read(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
    serde_json::from_slice(&bytes).expect("decode character-make.json")
}

#[cfg(feature = "game-data")]
fn load_palette_package() -> CharacterPalettePackage {
    let path =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/character-palette.json");
    let bytes = std::fs::read(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
    serde_json::from_slice(&bytes).expect("decode character-palette.json")
}

/// 敖龙女调试渲染：BaseColor/Normal/Mask 通道分离，定位皮肤/头发颜色问题。
#[test]
#[cfg(feature = "game-data")]
#[ignore = "renders au-ra debug channels; requires XIV_GAME_DIR"]
fn render_au_ra_debug_channels() {
    use xiv_companion_render::renderer::ModelDebugMode;
    let make = load_make_package();
    let palette = load_palette_package();
    let mut resource = SqPackResource::from_existing(&game_dir());
    let customize = default_customize_for_race_code(&make, 1401).expect("default");
    let appearance = appearance_colors_from_palette(&customize, &palette.palette);
    let request =
        CharacterAssemblyLoadRequest::new(customize, "au-ra-female").with_appearance(appearance);
    let model = load_character_assembly_from_resource(&mut resource, &request)
        .expect("load assembly");
    let names = character_enabled_attribute_names(&customize, &model);
    for mode in [ModelDebugMode::BaseColor, ModelDebugMode::Mask] {
        let mut render_options = xiv_companion_render::renderer::ModelRenderOptions::default();
        render_options.debug_mode = mode;
        let snapshot = render_model_snapshot_with_options(
            ModelSnapshotOptions::new(format!("debug-au-ra-{mode:?}"))
                .with_viewport(720, 900)
                .with_camera(0.35, 0.12, 2.6, [0.0, 0.0])
                .with_render_options(render_options)
                .with_prepared_model_options(
                    PreparedModelOptions::default()
                        .with_component_preview_layout(false)
                        .with_enabled_attribute_names(names.clone()),
                ),
            &model,
        )
        .expect("render debug");
        eprintln!("png: {}", snapshot.png_path.display());
    }
}

/// 敖龙女背面视角（检查尾巴颜色/形态）。
#[test]
#[cfg(feature = "game-data")]
#[ignore = "renders au-ra rear view; requires XIV_GAME_DIR"]
fn render_au_ra_rear_view() {
    let make = load_make_package();
    let palette = load_palette_package();
    let mut resource = SqPackResource::from_existing(&game_dir());
    let customize = default_customize_for_race_code(&make, 1401).expect("default");
    let appearance = appearance_colors_from_palette(&customize, &palette.palette);
    let request =
        CharacterAssemblyLoadRequest::new(customize, "au-ra-female").with_appearance(appearance);
    let model = load_character_assembly_from_resource(&mut resource, &request)
        .expect("load assembly");
    let names = character_enabled_attribute_names(&customize, &model);
    let snapshot = render_model_snapshot_with_options(
        ModelSnapshotOptions::new("debug-au-ra-rear")
            .with_viewport(720, 900)
            .with_camera(2.6, 0.15, 2.2, [0.0, 0.1])
            .with_prepared_model_options(
                PreparedModelOptions::default()
                    .with_component_preview_layout(false)
                    .with_enabled_attribute_names(names),
            ),
        &model,
    )
    .expect("render rear");
    eprintln!("png: {}", snapshot.png_path.display());
}

/// 敖龙女头部特写（复现 app 视角的发丝/头顶细节）。
#[test]
#[cfg(feature = "game-data")]
#[ignore = "renders au-ra head close-up; requires XIV_GAME_DIR"]
fn render_au_ra_head_closeup() {
    let make = load_make_package();
    let palette = load_palette_package();
    let mut resource = SqPackResource::from_existing(&game_dir());
    let customize = default_customize_for_race_code(&make, 1401).expect("default");
    let appearance = appearance_colors_from_palette(&customize, &palette.palette);
    let request =
        CharacterAssemblyLoadRequest::new(customize, "au-ra-female").with_appearance(appearance);
    let model = load_character_assembly_from_resource(&mut resource, &request)
        .expect("load assembly");
    let names = character_enabled_attribute_names(&customize, &model);
    let snapshot = render_model_snapshot_with_options(
        ModelSnapshotOptions::new("debug-au-ra-head-closeup")
            .with_viewport(720, 720)
            .with_camera(0.35, 0.10, 0.80, [0.0, 0.55])
            .with_prepared_model_options(
                PreparedModelOptions::default()
                    .with_component_preview_layout(false)
                    .with_enabled_attribute_names(names),
            ),
        &model,
    )
    .expect("render closeup");
    eprintln!("png: {}", snapshot.png_path.display());
}

/// 敖龙女眼部/睫毛渲染探针：脸部网格的材质/纹理/透明模式摘要 + 眼部特写
/// （仅留脸部网格重算 bounds 以获得近景）与 BaseColor/Alpha 调试通道。
#[test]
#[cfg(feature = "game-data")]
#[ignore = "probes au-ra eye/lash rendering; requires XIV_GAME_DIR"]
fn probe_au_ra_eye_lash() {
    use xiv_companion_render::renderer::ModelDebugMode;
    let make = load_make_package();
    let palette = load_palette_package();
    let mut resource = SqPackResource::from_existing(&game_dir());
    let customize = default_customize_for_race_code(&make, 1401).expect("default");
    let appearance = appearance_colors_from_palette(&customize, &palette.palette);
    let request =
        CharacterAssemblyLoadRequest::new(customize, "au-ra-female").with_appearance(appearance);
    let model = load_character_assembly_from_resource(&mut resource, &request)
        .expect("load assembly");
    let names = character_enabled_attribute_names(&customize, &model);

    let texture_path = |model: &xiv_companion_data::WeaponModelData, index: Option<usize>| {
        index
            .and_then(|index| model.textures.get(index))
            .map(|texture| texture.path.as_str())
            .unwrap_or("<none>")
            .to_string()
    };
    for (index, mesh) in model.meshes.iter().enumerate() {
        if !mesh.path.contains("_fac.mdl") {
            continue;
        }
        let material = &model.materials[mesh.material_slot];
        println!(
            "FACE mesh[{index}] {} mat={} resolved={:?} shader={:?} alpha_mode={:?} alpha_thr={:.3} render_mode={:?} transparency={:.2}",
            mesh.path.rsplit('/').next().unwrap_or(&mesh.path),
            mesh.material_name,
            material.path,
            material.shader_package_name,
            material.alpha_mode,
            material.alpha_threshold,
            material.render_mode,
            material.transparency,
        );
        println!(
            "    base={} normal={} mask={} specular={}",
            texture_path(&model, material.base_color_texture),
            texture_path(&model, material.normal_texture),
            texture_path(&model, material.mask_texture),
            texture_path(&model, material.specular_texture),
        );
        println!(
            "    sub_color_mode={:?} diffuse={:?} multi_diffuse={:?} emissive={:?} aperture={:.3} offset={:.3}",
            material.sub_color_mode,
            material.shader_diffuse_color,
            material.shader_multi_diffuse_color,
            material.shader_emissive_color,
            material.alpha_aperture,
            material.alpha_offset,
        );
        // 顶点颜色分布（睫毛网格是否靠顶点色压暗）。
        let mut color_min = [f32::INFINITY; 4];
        let mut color_max = [f32::NEG_INFINITY; 4];
        for vertex in &mesh.vertices {
            for lane in 0..4 {
                color_min[lane] = color_min[lane].min(vertex.color[lane]);
                color_max[lane] = color_max[lane].max(vertex.color[lane]);
            }
        }
        println!("    vertex_color min={color_min:?} max={color_max:?}");
        // UV 分布：定位网格落在贴图哪个区域。
        let mut uv_min = [f32::INFINITY; 2];
        let mut uv_max = [f32::NEG_INFINITY; 2];
        for vertex in &mesh.vertices {
            for lane in 0..2 {
                uv_min[lane] = uv_min[lane].min(vertex.uv0[lane]);
                uv_max[lane] = uv_max[lane].max(vertex.uv0[lane]);
            }
        }
        println!("    uv0 min={uv_min:?} max={uv_max:?} verts={}", mesh.vertices.len());
    }

    // 导出睫毛/眉毛材质的贴图通道供检查。
    let out_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target/tmp/lash-probe");
    std::fs::create_dir_all(&out_dir).unwrap();
    for texture in &model.textures {
        if !texture.path.contains("f0002_etc") {
            continue;
        }
        let name = texture.path.rsplit('/').next().unwrap_or(&texture.path);
        let layer_len = (usize::from(texture.width) * usize::from(texture.height)) * 4;
        let rgba = if texture.rgba.len() >= layer_len {
            &texture.rgba[..layer_len]
        } else {
            &texture.rgba
        };
        // 整体 + 各通道分离。
        for channel in ["rgba", "r", "g", "b", "a"] {
            let path = out_dir.join(format!("{name}.{channel}.png"));
            if channel == "rgba" {
                image::save_buffer_with_format(
                    &path,
                    rgba,
                    u32::from(texture.width),
                    u32::from(texture.height),
                    image::ColorType::Rgba8,
                    image::ImageFormat::Png,
                )
                .unwrap();
            } else {
                let offset = match channel {
                    "r" => 0,
                    "g" => 1,
                    "b" => 2,
                    _ => 3,
                };
                let gray: Vec<u8> = rgba
                    .chunks_exact(4)
                    .flat_map(|px| {
                        let v = px[offset];
                        [v, v, v, 255]
                    })
                    .collect();
                image::save_buffer_with_format(
                    &path,
                    &gray,
                    u32::from(texture.width),
                    u32::from(texture.height),
                    image::ColorType::Rgba8,
                    image::ImageFormat::Png,
                )
                .unwrap();
            }
        }
        println!("png: {}", out_dir.join(name).display());
    }

    // 仅留脸部网格重算 bounds → 眼部近景。
    let mut face = model.clone();
    face.meshes.retain(|mesh| mesh.path.contains("_fac.mdl"));
    face.bounds = xiv_companion_data::calculate_model_bounds(&face.meshes);
    let prepared = || {
        PreparedModelOptions::default()
            .with_component_preview_layout(false)
            .with_enabled_attribute_names(names.clone())
    };
    // 仅睫毛/眉毛网格（etc_a）：隔离点状伪影的来源。
    let mut lash_only = face.clone();
    lash_only
        .meshes
        .retain(|mesh| mesh.material_name.contains("etc_a"));
    lash_only.bounds = xiv_companion_data::calculate_model_bounds(&lash_only.meshes);
    // prepared 验证：etc_a 的 render pass 与深度模式。
    for mesh in &lash_only.meshes {
        let material = &lash_only.materials[mesh.material_slot];
        let prepared_material = xiv_companion_data::prepare_material_for_draw_role(
            Some(material),
            xiv_companion_data::ModelMeshDrawRole::Normal,
        );
        println!(
            "PREPARED {} pass={:?} depth={:?} source={:?}",
            mesh.material_name,
            prepared_material.render_pass,
            prepared_material.alpha_policy.draw_depth_mode,
            prepared_material.alpha_policy.source,
        );
    }
    // 逐项排除：bloom / normal_mapping 对点状伪影的影响。
    for (label, bloom, normal_mapping) in [
        ("nobloom", false, true),
        ("nonormal", false, false),
    ] {
        let mut render_options = xiv_companion_render::renderer::ModelRenderOptions::default();
        render_options.bloom = bloom;
        render_options.normal_mapping = normal_mapping;
        let snapshot = render_model_snapshot_with_options(
            ModelSnapshotOptions::new(format!("probe-au-ra-lash-{label}"))
                .with_viewport(900, 720)
                .with_camera(0.15, 0.0, 1.15, [0.05, 0.10])
                .with_render_options(render_options)
                .with_prepared_model_options(prepared()),
            &lash_only,
        )
        .expect("render lash variant");
        eprintln!("png: {}", snapshot.png_path.display());
    }
    let snapshot = render_model_snapshot_with_options(
        ModelSnapshotOptions::new("probe-au-ra-lash-only")
            .with_viewport(900, 720)
            .with_camera(0.15, 0.0, 1.15, [0.05, 0.10])
            .with_prepared_model_options(prepared()),
        &lash_only,
    )
    .expect("render lash only");
    eprintln!("png: {}", snapshot.png_path.display());
    for (label, mode) in [
        ("alpha", ModelDebugMode::Alpha),
        ("basecolor", ModelDebugMode::BaseColor),
    ] {
        let mut render_options = xiv_companion_render::renderer::ModelRenderOptions::default();
        render_options.debug_mode = mode;
        let snapshot = render_model_snapshot_with_options(
            ModelSnapshotOptions::new(format!("probe-au-ra-lash-{label}"))
                .with_viewport(900, 720)
                .with_camera(0.15, 0.0, 1.15, [0.05, 0.10])
                .with_render_options(render_options)
                .with_prepared_model_options(prepared()),
            &lash_only,
        )
        .expect("render lash debug");
        eprintln!("png: {}", snapshot.png_path.display());
    }
    for (label, mode) in [
        ("final", ModelDebugMode::Final),
        ("basecolor", ModelDebugMode::BaseColor),
        ("alpha", ModelDebugMode::Alpha),
        ("mask", ModelDebugMode::Mask),
    ] {
        let mut render_options = xiv_companion_render::renderer::ModelRenderOptions::default();
        render_options.debug_mode = mode;
        let snapshot = render_model_snapshot_with_options(
            ModelSnapshotOptions::new(format!("probe-au-ra-eye-{label}"))
                .with_viewport(900, 720)
                .with_camera(0.15, 0.0, 1.15, [0.05, 0.10])
                .with_render_options(render_options)
                .with_prepared_model_options(prepared()),
            &face,
        )
        .expect("render eye probe");
        eprintln!("png: {}", snapshot.png_path.display());
    }
    // MSAA 4x 对照。
    let mut msaa_options = xiv_companion_render::renderer::ModelRenderOptions::default();
    msaa_options.msaa_samples = 4;
    let snapshot = render_model_snapshot_with_options(
        ModelSnapshotOptions::new("probe-au-ra-eye-final-msaa4")
            .with_viewport(900, 720)
            .with_camera(0.15, 0.0, 1.15, [0.05, 0.10])
            .with_render_options(msaa_options)
            .with_prepared_model_options(prepared()),
        &face,
    )
    .expect("render eye msaa probe");
    eprintln!("png: {}", snapshot.png_path.display());
}

/// 敖龙女四肢拼接探针：打印手/足/臂/腿网格的内嵌材质名、解析后材质路径与
/// 顶点范围（对照中原女原生配对），渲染脚踝/手腕特写。用于核对拼接闭合
/// （`close_bare_limb_junctions`/`snap_bare_hand_cuff_to_forearm`）的几何事实。
#[test]
#[cfg(feature = "game-data")]
#[ignore = "probes au-ra limb junction seams; requires XIV_GAME_DIR"]
fn probe_au_ra_limb_junction_seams() {
    let make = load_make_package();
    let palette = load_palette_package();
    let mut resource = SqPackResource::from_existing(&game_dir());
    let customize = default_customize_for_race_code(&make, 1401).expect("default");
    let appearance = appearance_colors_from_palette(&customize, &palette.palette);
    let request =
        CharacterAssemblyLoadRequest::new(customize, "au-ra-female").with_appearance(appearance);
    let model = load_character_assembly_from_resource(&mut resource, &request)
        .expect("load assembly");
    let names = character_enabled_attribute_names(&customize, &model);

    // 逐网格摘要：四肢相关文件的材质名与顶点 y/x 范围。对照中原女（原生无骨
    // 变形），区分"设计缺口"与"骨变形下移"。
    for probe_race in [1401u16, 201] {
        let probe_customize = default_customize_for_race_code(&make, probe_race).expect("default");
        let probe_model = load_character_assembly_from_resource(
            &mut resource,
            &CharacterAssemblyLoadRequest::new(probe_customize, format!("race-{probe_race}")),
        )
        .expect("load probe assembly");
        println!("== race {probe_race} ==");
        for mesh in &probe_model.meshes {
            let limb = ["e0001_dwn", "e0000_sho", "e0001_top", "e0000_glv"]
                .iter()
                .any(|key| mesh.path.contains(key));
            if !limb {
                continue;
            }
            let resolved = probe_model
                .materials
                .get(mesh.material_slot)
                .and_then(|material| material.path.as_deref())
                .unwrap_or("<none>");
            let mut min = [f32::INFINITY; 3];
            let mut max = [f32::NEG_INFINITY; 3];
            for vertex in &mesh.vertices {
                for axis in 0..3 {
                    min[axis] = min[axis].min(vertex.position[axis]);
                    max[axis] = max[axis].max(vertex.position[axis]);
                }
            }
            println!(
                "MESH {} mat={} resolved={} verts={} ymin={:.4} ymax={:.4} xmin={:.4} xmax={:.4}",
                mesh.path.rsplit('/').next().unwrap_or(&mesh.path),
                mesh.material_name,
                resolved,
                mesh.vertices.len(),
                min[1],
                max[1],
                min[0],
                max[0],
            );
        }
    }

    let prepared = || {
        PreparedModelOptions::default()
            .with_component_preview_layout(false)
            .with_enabled_attribute_names(names.clone())
    };
    // 脚踝特写（右侧）与手腕特写（右侧，世界坐标约 x≈0.45、y≈0.97）。
    for (label, camera) in [
        ("ankle-full", (0.35, 0.0, 1.15, [0.10, -0.81])),
        ("wrist-full", (0.35, 0.0, 1.15, [0.55, 0.33])),
    ] {
        let (yaw, pitch, zoom, pan) = camera;
        let snapshot = render_model_snapshot_with_options(
            ModelSnapshotOptions::new(format!("probe-au-ra-{label}"))
                .with_viewport(720, 900)
                .with_camera(yaw, pitch, zoom, pan)
                .with_prepared_model_options(prepared()),
            &model,
        )
        .expect("render probe");
        eprintln!("png: {}", snapshot.png_path.display());
    }

    // 叠加带绘制次序实验：把 glv 网格移到 top 之前（前臂后画），看叠加带
    // 显示前臂图案还是手部图案对接缝观感的影响。
    let mut reordered = model.clone();
    reordered.meshes.sort_by_key(|mesh| {
        if mesh.path.contains("e0000_glv") {
            0
        } else if mesh.path.contains("e0001_top") {
            1
        } else {
            2
        }
    });
    let snapshot = render_model_snapshot_with_options(
        ModelSnapshotOptions::new("probe-au-ra-wrist-forearm-wins")
            .with_viewport(720, 900)
            .with_camera(0.35, 0.0, 1.15, [0.55, 0.33])
            .with_prepared_model_options(prepared()),
        &reordered,
    )
    .expect("render wrist forearm-wins");
    eprintln!("png: {}", snapshot.png_path.display());

    // 中原女原生配对对照：同一渲染器下原生手腕/脚踝接缝长什么样。
    let mid_customize = default_customize_for_race_code(&make, 201).expect("default");
    let mid_appearance = appearance_colors_from_palette(&mid_customize, &palette.palette);
    let mid_model = load_character_assembly_from_resource(
        &mut resource,
        &CharacterAssemblyLoadRequest::new(mid_customize, "midlander-female")
            .with_appearance(mid_appearance),
    )
    .expect("load midlander assembly");
    let mid_names = character_enabled_attribute_names(&mid_customize, &mid_model);
    let mid_prepared = || {
        PreparedModelOptions::default()
            .with_component_preview_layout(false)
            .with_enabled_attribute_names(mid_names.clone())
    };
    for (label, camera) in [
        // 中原女腕骨 j_te 在 |x|≈0.47、y≈1.00；脚踝 y≈0.14。
        ("wrist-full", (0.35, 0.0, 1.15, [0.60, 0.36])),
        ("ankle-full", (0.35, 0.0, 1.15, [0.10, -0.76])),
    ] {
        let (yaw, pitch, zoom, pan) = camera;
        let snapshot = render_model_snapshot_with_options(
            ModelSnapshotOptions::new(format!("probe-midlander-{label}"))
                .with_viewport(720, 900)
                .with_camera(yaw, pitch, zoom, pan)
                .with_prepared_model_options(mid_prepared()),
            &mid_model,
        )
        .expect("render midlander probe");
        eprintln!("png: {}", snapshot.png_path.display());
    }
}

/// 默认捏脸 + 调色板的四族代表渲染快照（中原男/猫魅女/敖龙女/维埃拉女）。
#[test]
#[cfg(feature = "game-data")]
#[ignore = "renders installed character assemblies to target/weapon-render-snapshots; requires XIV_GAME_DIR"]
fn render_installed_character_snapshots_with_appearance_colors() {
    let make = load_make_package();
    let palette = load_palette_package();
    let mut resource = SqPackResource::from_existing(&game_dir());
    for (race_code, label, camera) in [
        (
            101u16,
            "hyur-midlander-male",
            (0.35, 0.12, 2.6, [0.0, 0.05]),
        ),
        (801, "miqote-female", (0.35, 0.12, 2.7, [0.0, 0.15])),
        (1401, "au-ra-female", (0.35, 0.12, 2.6, [0.0, 0.0])),
        (1801, "viera-female", (0.35, 0.12, 2.6, [0.0, -0.1])),
    ] {
        let customize = default_customize_for_race_code(&make, race_code)
            .unwrap_or_else(|| panic!("default customize for race code {race_code}"));
        let appearance = appearance_colors_from_palette(&customize, &palette.palette);
        let request =
            CharacterAssemblyLoadRequest::new(customize, label).with_appearance(appearance);
        let model = load_character_assembly_from_resource(&mut resource, &request)
            .unwrap_or_else(|error| panic!("load assembly {label}: {error:#}"));
        let enabled_attribute_names = character_enabled_attribute_names(&customize, &model);
        let decal_loaded = model.materials.iter().any(|material| {
            material
                .character_colors
                .is_some_and(|c| c.decal_texture.is_some())
        });
        eprintln!(
            "RACE {race_code} ({label}): meshes={} materials={} decal_textures={} attributes={:?}",
            model.meshes.len(),
            model.materials.len(),
            decal_loaded,
            enabled_attribute_names,
        );
        let (yaw, pitch, zoom, pan) = camera;
        let snapshot = render_model_snapshot_with_options(
            ModelSnapshotOptions::new(format!("installed-character-{label}"))
                .with_viewport(720, 900)
                .with_camera(yaw, pitch, zoom, pan)
                .with_prepared_model_options(
                    PreparedModelOptions::default()
                        .with_component_preview_layout(false)
                        .with_enabled_attribute_names(enabled_attribute_names),
                ),
            &model,
        )
        .unwrap_or_else(|error| panic!("render {label}: {error}"));
        eprintln!("png: {}", snapshot.png_path.display());
        eprintln!(
            "adapter: {} ({:?})",
            snapshot.adapter_name, snapshot.adapter_backend
        );
    }
}
