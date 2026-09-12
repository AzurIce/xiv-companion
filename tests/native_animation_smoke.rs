#![cfg(feature = "render-test-support")]

//! PAP 骨骼动画采样 + 蒙皮渲染的本地真实数据冒烟（需 XIV_GAME_DIR 指向游戏
//! 安装目录，`--features "render-test-support,game-data" --ignored`）。
//!
//! 覆盖三条链路：
//! - 陆行鸟（demihuman d0001，坐骑）：mount.pap t=0 为骑乘站姿（与 bind 不
//!   同，差数万像素），t=40% 与 t=0 逐像素明显不同（动画确实在驱动骨骼），
//!   循环收尾衔接（样条 knot 窗口回归点：上游 bug 时中段 quaternion 爆成
//!   非单位值、姿态翻转）；
//! - 古菩（monster m0054）：同测（mount/idle pap 探测命中其一）；
//! - 中原男角色装配（race code 101）：action.pap 任选一动画 t=50% 渲染。

#[cfg(feature = "game-data")]
mod installed {
    use physis::resource::{Resource, SqPackResource};
    use xiv_companion_data::{
        animation_joint_matrices, default_customize_for_race_code,
        load_animation_set_from_pap_bytes, load_chara_model_with_skeleton_from_resource,
        load_character_assembly_with_skeleton_from_resource, pap_path_candidates,
        AnimationSourceKind, CharaModelKind, CharaModelLoadRequest, CharaModelType,
        CharacterAssemblyLoadRequest, ModelAnimationSet, PackedCharaModelId,
    };
    use xiv_companion_render::test_support::{
        render_model_snapshot_with_skeleton_and_pose, ModelSnapshotOptions,
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

    fn differing_pixels(a: &[u8], b: &[u8]) -> usize {
        assert_eq!(a.len(), b.len(), "pixel buffer length mismatch");
        a.chunks_exact(4)
            .zip(b.chunks_exact(4))
            .filter(|(x, y)| x != y)
            .count()
    }

    fn render(
        name: &str,
        model: &xiv_companion_data::WeaponModelData,
        skeleton: &xiv_companion_data::ModelSkeleton,
        pose: &xiv_companion_data::SkeletonPose,
    ) -> xiv_companion_render::test_support::ModelSnapshot {
        // 原位布局（component_preview_layout=false）：平铺预览的偏移量在顶点
        // 缓冲里与蒙皮前位置相加，rest 下无碍，动画驱动 joint 后偏移会随关节
        // 旋转——动画目验一律原位渲染。
        let options = xiv_companion_data::PreparedModelOptions::default()
            .with_component_preview_layout(false)
            .with_enabled_attribute_mask(0x1);
        let snapshot = render_model_snapshot_with_skeleton_and_pose(
            ModelSnapshotOptions::new(name)
                .with_viewport(640, 640)
                .with_prepared_model_options(options),
            model,
            Some(skeleton),
            Some(pose),
        )
        .unwrap_or_else(|error| panic!("render {name}: {error}"));
        eprintln!("png: {}", snapshot.png_path.display());
        snapshot
    }

    /// 逐个探测 pap 候选（打印命中率明细），返回第一个解析成功的动画集。
    fn load_animation_set(
        resource: &mut SqPackResource,
        kind: AnimationSourceKind,
        skeleton: &xiv_companion_data::ModelSkeleton,
        label: &str,
    ) -> ModelAnimationSet {
        let mut first_usable = None;
        let mut hits = 0usize;
        for path in pap_path_candidates(kind) {
            let Some(bytes) = resource.read(&path) else {
                continue;
            };
            hits += 1;
            match load_animation_set_from_pap_bytes(&bytes, skeleton) {
                Ok(set) => {
                    eprintln!(
                        "  {label}: hit {path} ({} animations)",
                        set.animations.len()
                    );
                    if first_usable.is_none() {
                        first_usable = Some(set);
                    }
                }
                Err(error) => eprintln!("  {label}: {path}: {error}"),
            }
        }
        eprintln!("  {label}: {hits} pap file(s) present");
        first_usable.unwrap_or_else(|| panic!("{label}: no usable animations"))
    }

    /// 坐骑/宠物动画驱动冒烟：t=40% 与 t=0 逐像素明显不同（动画在驱动
    /// 骨骼）；循环收尾衔接（t=100% ≈ t=0，样条全局一致性，正是上游
    /// knot 窗口 bug 的回归点）；姿态数值合理（躯干/头高度有界）。
    /// 注意：mount 站立循环的起始帧是骑乘站姿，与 bind 姿势不同
    /// （实测差数万像素），"起始帧≈bind" 对 mount/idle 数据不成立。
    fn assert_mount_animation_moves(
        model: &xiv_companion_data::WeaponModelData,
        skeleton: &xiv_companion_data::ModelSkeleton,
        animations: &ModelAnimationSet,
        label: &str,
    ) {
        let index = 0;
        let animation = &animations.animations[index];
        eprintln!(
            "  {label}: animation[0] = {:?} duration {:.0}ms",
            animation.name, animation.duration_ms
        );

        let t0_pose = xiv_companion_data::sample_animation_pose(animations, index, 0.0, skeleton);
        let t0 = render(
            &format!("animation-smoke-{label}-t0"),
            model,
            skeleton,
            &t0_pose,
        );
        let rest_pose = xiv_companion_data::SkeletonPose::rest_pose(skeleton);
        let rest = render(
            &format!("animation-smoke-{label}-rest"),
            model,
            skeleton,
            &rest_pose,
        );

        let t0_pixels = png_pixels(&t0.png_path);
        let rest_pixels = png_pixels(&rest.png_path);
        let start_diff = differing_pixels(&t0_pixels, &rest_pixels);
        eprintln!("  {label}: t0-vs-rest differing pixels = {start_diff} (mount stance vs bind)");
        assert!(
            start_diff > 1000,
            "{label}: animation pose should differ from bind pose ({start_diff} px)"
        );

        let t40_pose = xiv_companion_data::sample_animation_pose(
            animations,
            index,
            animation.duration_ms * 0.4,
            skeleton,
        );
        let t40 = render(
            &format!("animation-smoke-{label}-t40"),
            model,
            skeleton,
            &t40_pose,
        );
        let t40_pixels = png_pixels(&t40.png_path);
        let anim_diff = differing_pixels(&t0_pixels, &t40_pixels);
        eprintln!("  {label}: t40-vs-t0 differing pixels = {anim_diff}");
        assert!(
            anim_diff > 1000,
            "{label}: animation must visibly move the skeleton (only {anim_diff} px diff)"
        );

        // 循环衔接：t=最后一帧附近与 t=0 的局部 TRS 应接近（样条首尾相接；
        // 上游 knot 窗口 bug 时中段 quaternion 会爆成非单位值，这里直接回归）。
        // 采样点取 duration-1ms（贴尾帧，避开最后一个 blend 段中段）。
        let wrap_pose = xiv_companion_data::sample_animation_pose(
            animations,
            index,
            animation.duration_ms - 1.0,
            skeleton,
        );
        let mut max_delta = 0f32;
        for bone in 0..skeleton.bone_count() {
            let a = t0_pose.transform(bone).unwrap();
            let b = wrap_pose.transform(bone).unwrap();
            for i in 0..4 {
                max_delta = max_delta.max((a.rotation[i] - b.rotation[i]).abs());
            }
        }
        eprintln!("  {label}: loop wrap max quat delta = {max_delta:.4}");
        assert!(
            max_delta < 0.05,
            "{label}: animation loop should wrap cleanly (max quat delta {max_delta})"
        );

        // 样条求值回归签名：全程采样所有四元数模长不离谱（上游 knot 窗口
        // bug 的直出症状是分量爆到 12、|q| 偏离 1 数倍；分量式 NURBS 在
        // 控制点符号翻转处会插值出 ~0.3 的模长，渲染端 trs_to_mat4 归一化
        // 吸收，属可接受的已知限制）。阈值 0.75 远松于 bug 特征、远紧于正常。
        for step in 0..16 {
            let pose = xiv_companion_data::sample_animation_pose(
                animations,
                index,
                animation.duration_ms * step as f32 / 16.0,
                skeleton,
            );
            for bone in 0..skeleton.bone_count() {
                let q = pose.transform(bone).unwrap().rotation;
                let norm = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
                assert!(
                    (norm - 1.0).abs() <= 0.75,
                    "{label}: quat norm {norm} off unit at step {step} bone {bone}"
                );
            }
        }

        // 姿态数值合理：全程躯干高度有界（翻转/爆炸姿态会飞出书外）。
        for fraction in [0.0f32, 0.2, 0.4, 0.6, 0.8] {
            let pose = xiv_companion_data::sample_animation_pose(
                animations,
                index,
                animation.duration_ms * fraction,
                skeleton,
            );
            let world = xiv_companion_data::world_matrices(skeleton, &pose);
            let belly = skeleton
                .bone_index("n_hara")
                .or_else(|| skeleton.bone_index("n_root"))
                .expect("belly/root bone");
            assert!(
                world[belly][13] > 0.2 && world[belly][13] < 5.0,
                "{label}: belly world height {} out of range at {fraction}",
                world[belly][13]
            );
        }
    }

    /// 陆行鸟（专属陆行鸟坐骑，d0001e0001）：mount.pap t=0 ≈ bind，
    /// t=40% 明显摆动。
    #[test]
    #[ignore = "renders installed animation smoke snapshots to target/weapon-render-snapshots; requires XIV_GAME_DIR"]
    fn chocobo_mount_animation_moves_skeleton() {
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
        let animations = load_animation_set(
            &mut resource,
            AnimationSourceKind::Chara {
                model: request.model,
            },
            &skeleton,
            "chocobo",
        );
        assert_mount_animation_moves(&model, &skeleton, &animations, "chocobo");
    }

    /// 古菩（monster m0054）：mount/idle pap 探测命中其一，同测动画驱动。
    #[test]
    #[ignore = "renders installed animation smoke snapshots to target/weapon-render-snapshots; requires XIV_GAME_DIR"]
    fn monster_m0054_animation_moves_skeleton() {
        let mut resource = SqPackResource::from_existing(&game_dir());
        let request = CharaModelLoadRequest {
            item_id: 54,
            item_name: "古菩".to_string(),
            kind: CharaModelKind::Mount,
            model: PackedCharaModelId {
                model_id: 54,
                base_id: 1,
                variant_id: 1,
                chara_type: CharaModelType::Monster,
            },
        };
        let (model, skeleton) =
            load_chara_model_with_skeleton_from_resource(&mut resource, &request)
                .unwrap_or_else(|error| panic!("load m0054: {error:#}"));
        let skeleton = skeleton.expect("m0054 skeleton (skl_m0054b0001)");
        let animations = load_animation_set(
            &mut resource,
            AnimationSourceKind::Chara {
                model: request.model,
            },
            &skeleton,
            "m0054",
        );
        assert_mount_animation_moves(&model, &skeleton, &animations, "m0054");
    }

    /// 中原男角色装配（race code 101，skl_c0101b0001）：action.pap 任选一
    /// 动画 t=50% 渲染（人形动作姿态目验），并校验关节矩阵直出辅助的输出。
    #[test]
    #[ignore = "renders installed animation smoke snapshots to target/weapon-render-snapshots; requires XIV_GAME_DIR"]
    fn hyur_action_animation_mid_frame_renders() {
        let make = load_make_package();
        let mut resource = SqPackResource::from_existing(&game_dir());
        let customize =
            default_customize_for_race_code(&make, 101).expect("hyur male default customize");
        let request = CharacterAssemblyLoadRequest::new(customize, "animation-smoke-hyur");
        let (model, skeleton) =
            load_character_assembly_with_skeleton_from_resource(&mut resource, &request)
                .unwrap_or_else(|error| panic!("load hyur assembly: {error:#}"));
        let skeleton = skeleton.expect("hyur skeleton (skl_c0101b0001)");
        let animations = load_animation_set(
            &mut resource,
            AnimationSourceKind::Character { race_code: 101 },
            &skeleton,
            "hyur",
        );

        // 选一条时长适中的动画（跳过过短的抖动类），t=50% 渲染单帧。
        let index = animations
            .animations
            .iter()
            .position(|animation| animation.duration_ms >= 1000.0)
            .unwrap_or(0);
        let animation = &animations.animations[index];
        eprintln!(
            "  hyur: animation[{index}] = {:?} duration {:.0}ms",
            animation.name, animation.duration_ms
        );
        let pose = xiv_companion_data::sample_animation_pose(
            &animations,
            index,
            animation.duration_ms * 0.5,
            &skeleton,
        );
        let options = xiv_companion_data::PreparedModelOptions::default()
            .with_component_preview_layout(false)
            .with_enabled_attribute_names(
                xiv_companion::character_enabled_attribute_names(&customize, &model),
            );
        let snapshot = render_model_snapshot_with_skeleton_and_pose(
            ModelSnapshotOptions::new("animation-smoke-hyur-action-t50")
                .with_viewport(640, 640)
                .with_prepared_model_options(options),
            &model,
            Some(&skeleton),
            Some(&pose),
        )
        .unwrap_or_else(|error| panic!("render hyur action: {error}"));
        eprintln!("png: {}", snapshot.png_path.display());

        // 人形姿态数值合理：t=50% 时头在骨盆上方（翻转/乱飞会击穿）。
        let world = xiv_companion_data::world_matrices(&skeleton, &pose);
        let head = skeleton.bone_index("j_kao").expect("head bone");
        let pelvis = skeleton.bone_index("j_kosi").expect("pelvis bone");
        assert!(
            world[head][13] > world[pelvis][13],
            "hyur: head ({}) should be above pelvis ({})",
            world[head][13],
            world[pelvis][13]
        );

        // 关节矩阵直出辅助：与 SkeletonPose 同采样点的输出长度/有限性校验。
        let joint_names: Vec<String> = skeleton.bone_names.iter().cloned().collect();
        let mut cache = xiv_companion_data::SkeletonInverseBindCache::new();
        let matrices = animation_joint_matrices(
            &animations,
            index,
            animation.duration_ms * 0.5,
            &skeleton,
            &joint_names,
            &mut cache,
        );
        assert_eq!(matrices.len(), skeleton.bone_count());
        assert!(
            matrices.iter().flatten().all(|value| value.is_finite()),
            "joint matrices must be finite"
        );
    }
}

#[cfg(not(feature = "game-data"))]
#[test]
fn animation_smoke_requires_game_data_feature() {
    panic!("enable the `game-data` feature to run animation smoke tests");
}
