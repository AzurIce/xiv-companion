#![cfg(feature = "render-test-support")]

//! 骨骼 rest pose + GPU 蒙皮管线的本地真实数据冒烟（需 XIV_GAME_DIR 指向游戏
//! 安装目录，`--features "render-test-support,game-data" --ignored`）。
//!
//! 覆盖两条链路：
//! - 陆行鸟（demihuman d0001，坐骑）：`load_chara_model_with_skeleton_from_resource`
//!   随模型加载 sklb；rest pose 蒙皮渲染与非蒙皮渲染逐像素一致（identity 蒙皮
//!   = 恒等变换的证明）；j_kubi（颈）旋转 30° 后头部肉眼可见偏移。
//! - 中原男角色装配：`load_character_assembly_with_skeleton_from_resource` 按
//!   race code 101 加载 skl_c0101b0001；rest pose 蒙皮渲染同样逐像素一致。

#[cfg(feature = "game-data")]
mod installed {
    use physis::resource::SqPackResource;
    use xiv_companion_data::{
        CharaModelKind, CharaModelLoadRequest, CharaModelType, CharacterAssemblyLoadRequest,
        PackedCharaModelId, SkeletonPose, default_customize_for_race_code,
        load_chara_model_with_skeleton_from_resource,
        load_character_assembly_with_skeleton_from_resource, quat_from_axis_angle,
    };
    use xiv_companion_render::test_support::{
        ModelSnapshotOptions, render_model_snapshot_with_skeleton_and_pose,
    };

    fn game_dir() -> String {
        std::env::var("XIV_GAME_DIR").unwrap_or_else(|_| r"E:\_ff14\game".to_string())
    }

    fn load_make_package() -> xiv_companion_data::CharacterMakePackage {
        let path =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/character-make.json");
        let bytes = std::fs::read(&path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
        serde_json::from_slice(&bytes).expect("decode character-make.json")
    }

    fn png_pixels(path: &std::path::Path) -> Vec<u8> {
        image::open(path)
            .unwrap_or_else(|error| panic!("decode {}: {error}", path.display()))
            .to_rgba8()
            .into_raw()
    }

    fn differing_pixels(a: &[u8], b: &[u8]) -> (usize, u8) {
        assert_eq!(a.len(), b.len(), "pixel buffer length mismatch");
        let mut count = 0usize;
        let mut max_diff = 0u8;
        for (x, y) in a.iter().zip(b.iter()) {
            if x != y {
                count += 1;
                max_diff = max_diff.max(x.abs_diff(*y));
            }
        }
        (count, max_diff)
    }

    /// rest pose 蒙皮（joint = world × inverse(bind world) ≈ 恒等，f32 噪声
    /// ~1e-7）与非蒙皮路径的逐像素等价断言。差异只允许是两类浮点噪声：
    /// HDR f16 量化边界上的 ±1 LSB；以及 ~1e-7 位置扰动恰好跨过纹理边界的
    /// 刀锋像素（换采相邻纹素，单通道 diff 可达十几）。任何蒙皮链路真实错误
    /// （joint 错配、权重错、漏归一）都会造成上万像素变化。实测（640×640）：
    /// 陆行鸟 9 像素（1 个刀锋）、中原男 10 通道 ±1 LSB。
    fn assert_identity_skinning_equivalent(unskinned: &[u8], rest: &[u8], label: &str) {
        assert_eq!(unskinned.len(), rest.len(), "pixel buffer length mismatch");
        let mut diff_pixels = 0usize;
        let mut knife_edge_pixels = 0usize;
        for (a, b) in unskinned.chunks_exact(4).zip(rest.chunks_exact(4)) {
            let max_diff = a
                .iter()
                .zip(b.iter())
                .map(|(x, y)| x.abs_diff(*y))
                .max()
                .unwrap_or(0);
            if max_diff > 0 {
                diff_pixels += 1;
                if max_diff > 2 {
                    knife_edge_pixels += 1;
                }
            }
        }
        eprintln!("{label}: differing pixels = {diff_pixels} (knife-edge = {knife_edge_pixels})");
        assert!(
            diff_pixels <= 64,
            "{label}: identity skinning diverged on {diff_pixels} pixels ({}% of output)",
            diff_pixels as f64 / (unskinned.len() / 4) as f64 * 100.0
        );
        assert!(
            knife_edge_pixels <= 8,
            "{label}: {knife_edge_pixels} pixels shifted by more than 2 LSB (expected f32 noise only)"
        );
    }

    fn render(
        name: &str,
        model: &xiv_companion_data::WeaponModelData,
        skeleton: Option<&xiv_companion_data::ModelSkeleton>,
        pose: Option<&SkeletonPose>,
        prepared_options: xiv_companion_data::PreparedModelOptions,
    ) -> xiv_companion_render::test_support::ModelSnapshot {
        let snapshot = render_model_snapshot_with_skeleton_and_pose(
            ModelSnapshotOptions::new(name)
                .with_viewport(640, 640)
                .with_prepared_model_options(prepared_options),
            model,
            skeleton,
            pose,
        )
        .unwrap_or_else(|error| panic!("render {name}: {error}"));
        eprintln!("png: {}", snapshot.png_path.display());
        snapshot
    }

    /// 陆行鸟（专属陆行鸟坐骑，d0001e0001）：identity 蒙皮逐像素一致 + 颈部
    /// 旋转 30° 头部偏移。
    #[test]
    #[ignore = "renders installed skeleton smoke snapshots to target/weapon-render-snapshots; requires XIV_GAME_DIR"]
    fn chocobo_rest_pose_skinning_matches_unskinned_and_neck_rotation_shifts_head() {
        let mut resource = SqPackResource::from_existing(&game_dir());
        let request = CharaModelLoadRequest {
            item_id: 1,
            item_name: "专属陆行鸟".to_string(),
            kind: CharaModelKind::Mount,
            model: PackedCharaModelId {
                model_id: 1,
                base_id: 1,
                variant_id: 1,
                chara_type: CharaModelType::Demihuman,
            },
        };
        let (model, skeleton) =
            load_chara_model_with_skeleton_from_resource(&mut resource, &request)
                .unwrap_or_else(|error| panic!("load chocobo: {error:#}"));
        let skeleton = skeleton.expect("chocobo skeleton (skl_d0001b0001)");
        eprintln!("chocobo skeleton: {} bones", skeleton.bone_count());
        assert!(
            skeleton.bone_count() >= 60,
            "chocobo skeleton should have ~86 bones, got {}",
            skeleton.bone_count()
        );
        let skinned_meshes = model
            .meshes
            .iter()
            .filter(|mesh| mesh.bone_table.is_some())
            .count();
        eprintln!(
            "chocobo skinned meshes: {}/{}",
            skinned_meshes,
            model.meshes.len()
        );
        assert!(skinned_meshes > 0, "chocobo MDL carries bone tables");

        let options = xiv_companion_data::PreparedModelOptions::default();
        let unskinned = render(
            "skeleton-smoke-chocobo-unskinned",
            &model,
            None,
            None,
            options.clone(),
        );
        let rest = render(
            "skeleton-smoke-chocobo-rest",
            &model,
            Some(&skeleton),
            None,
            options.clone(),
        );
        let unskinned_pixels = png_pixels(&unskinned.png_path);
        let rest_pixels = png_pixels(&rest.png_path);
        assert_identity_skinning_equivalent(
            &unskinned_pixels,
            &rest_pixels,
            "chocobo rest-vs-unskinned",
        );

        // 颈部链 j_kubi_a → j_kubi_b → j_kubi_c → j_kao（头）：j_kubi_a（颈根）
        // 绕 X 轴转 30°，头部/喙/缰绳网格随颈部关节明显偏移。
        let neck = skeleton
            .bone_index("j_kubi_a")
            .unwrap_or_else(|| panic!("j_kubi_a bone not found in chocobo skeleton"));
        let mut pose = SkeletonPose::rest_pose(&skeleton);
        pose.set_rotation(
            neck,
            quat_from_axis_angle([1.0, 0.0, 0.0], 30.0f32.to_radians()),
        )
        .expect("set neck rotation");
        let rotated = render(
            "skeleton-smoke-chocobo-neck-rot30",
            &model,
            Some(&skeleton),
            Some(&pose),
            options,
        );
        let rotated_pixels = png_pixels(&rotated.png_path);
        let (diff, max_diff) = differing_pixels(&unskinned_pixels, &rotated_pixels);
        eprintln!(
            "neck-rot30-vs-unskinned: differing pixels = {diff}, max channel diff = {max_diff}"
        );
        assert!(
            diff > 4_000,
            "30° neck rotation must visibly move the head (only {diff} pixels changed)"
        );
    }

    /// 中原男角色装配（race code 101，skl_c0101b0001）：rest pose 蒙皮渲染与
    /// 非蒙皮渲染逐像素一致（全部件原位重叠 + attribute 按名启用）。
    #[test]
    #[ignore = "renders installed skeleton smoke snapshots to target/weapon-render-snapshots; requires XIV_GAME_DIR"]
    fn hyur_assembly_rest_pose_skinning_matches_unskinned() {
        let make = load_make_package();
        let mut resource = SqPackResource::from_existing(&game_dir());
        let customize =
            default_customize_for_race_code(&make, 101).expect("hyur male default customize");
        let request = CharacterAssemblyLoadRequest::new(customize, "skeleton-smoke-hyur");
        let (model, skeleton) =
            load_character_assembly_with_skeleton_from_resource(&mut resource, &request)
                .unwrap_or_else(|error| panic!("load hyur assembly: {error:#}"));
        let skeleton = skeleton.expect("hyur skeleton (skl_c0101b0001)");
        eprintln!("hyur skeleton: {} bones", skeleton.bone_count());
        assert!(skeleton.bone_count() > 100, "human skeleton has ~200 bones");
        let skinned_meshes = model
            .meshes
            .iter()
            .filter(|mesh| mesh.bone_table.is_some())
            .count();
        eprintln!(
            "hyur skinned meshes: {}/{}",
            skinned_meshes,
            model.meshes.len()
        );
        assert!(skinned_meshes > 0, "character MDLs carry bone tables");

        // 与角色渲染页一致：全部件原位重叠，attribute 按名启用。
        let options = xiv_companion_data::PreparedModelOptions::default()
            .with_component_preview_layout(false)
            .with_enabled_attribute_names(
                xiv_companion::character_enabled_attribute_names(&customize, &model),
            );
        let unskinned = render(
            "skeleton-smoke-hyur-unskinned",
            &model,
            None,
            None,
            options.clone(),
        );
        let rest = render(
            "skeleton-smoke-hyur-rest",
            &model,
            Some(&skeleton),
            None,
            options.clone(),
        );
        let unskinned_pixels = png_pixels(&unskinned.png_path);
        let rest_pixels = png_pixels(&rest.png_path);
        assert_identity_skinning_equivalent(
            &unskinned_pixels,
            &rest_pixels,
            "hyur rest-vs-unskinned",
        );
    }
}

#[cfg(not(feature = "game-data"))]
#[test]
fn skeleton_smoke_requires_game_data_feature() {
    panic!("enable the `game-data` feature to run skeleton smoke tests");
}
