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
