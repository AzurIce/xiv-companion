#![cfg(feature = "render-test-support")]

//! 着装角色（dressed character）真实数据渲染快照（需 XIV_GAME_DIR 指向游戏
//! 安装目录，`--features "render-test-support,game-data" --ignored`）。
//!
//! 用例（敖龙女，同 `load_dressed_au_ra_with_e0908_set_from_installed_game`
//! 探针的捏脸；骨骼 rest pose 蒙皮，全身正面）：
//! - A：e0908 总冠军制敌五件（met/top/glv/dwn/sho，全部无染色）。
//! - B：同套，上衣染宝石红（stain 86）与未染色对照，断言染色落地到画面。
//! - C：弦月睡袍（长袍）不穿腿部件 → 袍 EQP 遮蔽腿肤/小衣；附袍+弦月睡裤
//!   配对变体对照。
//!
//! 可见性已在加载时按网格过滤落地（EQP 遮蔽身体、IMC 裁剪装备变体），渲染
//! 侧不再做隐藏；attribute 按名启用仅用于脸部特征（atr_fv_*）等装配内变体。

#[cfg(feature = "game-data")]
mod installed {
    use physis::resource::SqPackResource;
    use xiv_companion::{
        CharacterPalettePackage, appearance_colors_from_palette, character_enabled_attribute_names,
        load_dressed_character_scene_from_resource, plan_dressed_concealment,
    };
    use xiv_companion_data::{
        AnimationSourceKind, CharacterCustomize, DressedCharacterData, DressedCharacterLoadRequest,
        DressedEquipmentPiece, ModelSkeleton, PreparedModelOptions,
        load_animation_set_from_pap_bytes, load_dressed_character_with_skeleton_from_resource,
        pap_path_candidates,
    };
    use xiv_companion_render::test_support::{
        ModelSnapshot, ModelSnapshotOptions, SceneSnapshotEntry,
        render_model_snapshot_with_skeleton_and_pose, render_scene_snapshot,
    };

    fn game_dir() -> String {
        std::env::var("XIV_GAME_DIR").unwrap_or_else(|_| r"E:\_ff14\game".to_string())
    }

    fn load_palette_package() -> CharacterPalettePackage {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("assets/character-palette.json");
        let bytes = std::fs::read(&path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
        serde_json::from_slice(&bytes).expect("decode character-palette.json")
    }

    /// 与探针一致的敖龙女捏脸（敖龙女有效取值：tribe 11/12、head 0-3、hair
    /// 命中 character-make.json hairOptions；palette 外观色随捏脸索引）。
    fn au_ra_female_customize() -> CharacterCustomize {
        CharacterCustomize {
            race: 6,
            gender: 1,
            age: 1,
            height: 50,
            tribe: 11,
            head: 1,
            hair: 1,
            ..Default::default()
        }
    }

    /// 鲁加男捏脸（c0901；鲁加族部落 7 海狼/8 红焰，head 0-3）。
    fn roe_male_customize() -> CharacterCustomize {
        CharacterCustomize {
            race: 5,
            gender: 0,
            age: 1,
            height: 50,
            tribe: 7,
            head: 1,
            hair: 1,
            ..Default::default()
        }
    }

    /// 女仆装（e6016，model_main 0x11780 = set 6016 + IMC 子集 1）五件。
    fn maid_set() -> Vec<DressedEquipmentPiece> {
        let piece = |item_id: u32, name: &str, equip_slot_category: u32| DressedEquipmentPiece {
            item_id,
            item_name: name.to_string(),
            model_main: 0x1_1780,
            model_sub: 0,
            equip_slot_category,
            stain_ids: [0, 0],
        };
        vec![
            piece(14972, "女仆发带", 3),
            piece(14973, "女仆围裙装", 4),
            piece(14974, "女仆腕带", 5),
            piece(14975, "女仆蓬松裤", 7),
            piece(14976, "女仆礼鞋", 8),
        ]
    }

    fn e0908_piece(
        item_id: u32,
        name: &str,
        equip_slot_category: u32,
        stain_ids: [u8; 2],
    ) -> DressedEquipmentPiece {
        DressedEquipmentPiece {
            item_id,
            item_name: name.to_string(),
            model_main: 0x2_038C,
            model_sub: 0,
            equip_slot_category,
            stain_ids,
        }
    }

    fn e0908_set(top_stain_ids: [u8; 2]) -> Vec<DressedEquipmentPiece> {
        vec![
            e0908_piece(49685, "总冠军制敌头甲", 3, [0, 0]),
            e0908_piece(49686, "总冠军制敌上衣", 4, top_stain_ids),
            e0908_piece(49687, "总冠军制敌手套", 5, [0, 0]),
            e0908_piece(49688, "总冠军制敌马裤", 7, [0, 0]),
            e0908_piece(49689, "总冠军制敌矮靴", 8, [0, 0]),
        ]
    }

    /// 着装装配加载 + 诊断打印（网格/材质/贴图数、材质区间、遮蔽条目、
    /// 加载诊断）。骨骼缺失直接失败：蒙皮渲染是本的用例前提。
    fn load_dressed(
        resource: &mut SqPackResource,
        name: &str,
        equipment: Vec<DressedEquipmentPiece>,
    ) -> (CharacterCustomize, DressedCharacterData, ModelSkeleton) {
        load_dressed_as(resource, au_ra_female_customize(), name, equipment)
    }

    /// [`load_dressed`] 的自定义体型版（指定捏脸数据）。
    fn load_dressed_as(
        resource: &mut SqPackResource,
        customize: CharacterCustomize,
        name: &str,
        equipment: Vec<DressedEquipmentPiece>,
    ) -> (CharacterCustomize, DressedCharacterData, ModelSkeleton) {
        let palette = load_palette_package();
        let appearance = appearance_colors_from_palette(&customize, &palette.palette);
        let request = DressedCharacterLoadRequest::new(customize, name)
            .with_appearance(appearance)
            .with_equipment(equipment);
        let (data, skeleton) =
            load_dressed_character_with_skeleton_from_resource(resource, &request)
                .unwrap_or_else(|error| panic!("load dressed {name}: {error:#}"));
        let skeleton = skeleton.unwrap_or_else(|| panic!("{name}: missing human skeleton"));
        eprintln!(
            "{name}: meshes={} materials={} textures={} bones={} ranges={:?}",
            data.model.meshes.len(),
            data.model.materials.len(),
            data.model.textures.len(),
            skeleton.bone_count(),
            data.equipment_material_ranges,
        );
        eprintln!("{name}: hidden={:?}", data.hidden_body_attributes);
        for diagnostic in &data.model.load_diagnostics {
            eprintln!("{name}: diagnostic: {}", diagnostic.error);
        }
        (customize, data, skeleton)
    }

    /// 全身正面 rest pose 蒙皮渲染（相机对齐 native_character_render 的敖龙女
    /// 全身机位），PNG 落 target/weapon-render-snapshots。
    fn render_dressed_front(
        name: &str,
        customize: &CharacterCustomize,
        data: &DressedCharacterData,
        skeleton: &ModelSkeleton,
    ) -> ModelSnapshot {
        render_dressed_front_zoom(name, customize, data, skeleton, 2.6)
    }

    /// [`render_dressed_front`] 的可调距离版（大体型种族需要更远机位）。
    fn render_dressed_front_zoom(
        name: &str,
        customize: &CharacterCustomize,
        data: &DressedCharacterData,
        skeleton: &ModelSkeleton,
        zoom: f32,
    ) -> ModelSnapshot {
        let snapshot = render_model_snapshot_with_skeleton_and_pose(
            ModelSnapshotOptions::new(name)
                .with_viewport(720, 900)
                .with_camera(0.35, 0.12, zoom, [0.0, 0.0])
                .with_prepared_model_options(
                    PreparedModelOptions::default()
                        .with_component_preview_layout(false)
                        .with_enabled_attribute_names(character_enabled_attribute_names(
                            customize,
                            &data.model,
                        )),
                ),
            &data.model,
            Some(skeleton),
            None,
        )
        .unwrap_or_else(|error| panic!("render {name}: {error}"));
        eprintln!("png: {}", snapshot.png_path.display());
        snapshot
    }

    /// 非空非均匀断言：左上为背景，角色应占画面相当比例且有亮度层次（排除
    /// 黑图/纯色图/空渲染）。返回像素供跨渲染差异比较。
    fn assert_non_uniform_render(snapshot: &ModelSnapshot, label: &str) -> Vec<u8> {
        let pixels = image::open(&snapshot.png_path)
            .unwrap_or_else(|error| panic!("decode {}: {error}", snapshot.png_path.display()))
            .to_rgba8()
            .into_raw();
        let pixel_count = (snapshot.width * snapshot.height) as usize;
        assert_eq!(pixels.len(), pixel_count * 4, "{label}: pixel buffer size");
        let background = &pixels[..4];
        let foreground = pixels
            .chunks_exact(4)
            .filter(|pixel| (0..3).any(|channel| pixel[channel].abs_diff(background[channel]) > 8))
            .count();
        eprintln!("{label}: foreground {foreground}/{pixel_count} pixels");
        assert!(
            foreground > pixel_count / 20,
            "{label}: render looks empty (foreground {foreground}/{pixel_count})"
        );
        let mut luminance_min = u8::MAX;
        let mut luminance_max = u8::MIN;
        for pixel in pixels.chunks_exact(4) {
            let luminance =
                ((u16::from(pixel[0]) + u16::from(pixel[1]) + u16::from(pixel[2])) / 3) as u8;
            luminance_min = luminance_min.min(luminance);
            luminance_max = luminance_max.max(luminance);
        }
        assert!(
            luminance_max > 96,
            "{label}: render looks black (max luminance {luminance_max})"
        );
        assert!(
            luminance_max - luminance_min > 48,
            "{label}: render looks uniform (luminance {luminance_min}..{luminance_max})"
        );
        pixels
    }

    fn rgb_difference(left: &[u8], right: &[u8]) -> u64 {
        assert_eq!(left.len(), right.len(), "pixel buffer length mismatch");
        left.chunks_exact(4)
            .zip(right.chunks_exact(4))
            .map(|(left, right)| {
                (0..3)
                    .map(|channel| left[channel].abs_diff(right[channel]) as u64)
                    .sum::<u64>()
            })
            .sum()
    }

    /// Case A：敖龙女 + e0908 总冠军制敌五件（无染色）全身正面。
    #[test]
    #[ignore = "renders dressed au-ra e0908 set to target/weapon-render-snapshots; requires XIV_GAME_DIR"]
    fn render_dressed_au_ra_e0908_set_front() {
        let mut resource = SqPackResource::from_existing(&game_dir());
        let (customize, data, skeleton) =
            load_dressed(&mut resource, "dressed-au-ra-e0908", e0908_set([0, 0]));
        let snapshot = render_dressed_front(
            "dressed-au-ra-e0908-set-front",
            &customize,
            &data,
            &skeleton,
        );
        assert_non_uniform_render(&snapshot, "e0908-set-front");
    }

    /// Case B：同套，上衣（49686）染宝石红（stain 86）；与未染色渲染逐像素
    /// 对比，断言染色在画面上可见，且上衣材质切片内有染色落地记录。
    #[test]
    #[ignore = "renders dressed au-ra e0908 top stain contrast; requires XIV_GAME_DIR"]
    fn render_dressed_au_ra_e0908_top_stain_contrast() {
        let mut resource = SqPackResource::from_existing(&game_dir());
        let (customize, unstained, skeleton) =
            load_dressed(&mut resource, "dressed-au-ra-e0908", e0908_set([0, 0]));
        let (_, stained, stained_skeleton) =
            load_dressed(&mut resource, "dressed-au-ra-e0908", e0908_set([86, 0]));

        let top_range = stained
            .equipment_material_ranges
            .iter()
            .find(|range| range.item_id == 49686)
            .expect("top material range");
        assert!(
            stained.model.materials[top_range.material_start..top_range.material_end]
                .iter()
                .any(|material| material.staining_application.is_some()),
            "stained top materials should record a staining application"
        );

        let reference = render_dressed_front(
            "dressed-au-ra-e0908-top-unstained",
            &customize,
            &unstained,
            &skeleton,
        );
        let stained_snapshot = render_dressed_front(
            "dressed-au-ra-e0908-top-stained-gem-red",
            &customize,
            &stained,
            &stained_skeleton,
        );
        let reference_pixels = assert_non_uniform_render(&reference, "e0908-top-unstained");
        let stained_pixels = assert_non_uniform_render(&stained_snapshot, "e0908-top-stained");
        let difference = rgb_difference(&reference_pixels, &stained_pixels);
        eprintln!("top stain RGB difference: {difference}");
        assert!(
            difference > 100_000,
            "staining the top with 宝石红 must visibly change the render"
        );
    }

    /// Case C：弦月睡袍（长袍，8559）不穿腿部件——袍 EQP 应遮蔽腿部皮肤与
    /// 小衣布料；配对变体加弦月睡裤（8560，同套装同 model_main）。
    #[test]
    #[ignore = "renders dressed au-ra crescent robe variants; requires XIV_GAME_DIR"]
    fn render_dressed_au_ra_crescent_robe_leg_hiding() {
        let robe = |item_id: u32, name: &str, equip_slot_category: u32| DressedEquipmentPiece {
            item_id,
            item_name: name.to_string(),
            model_main: 0x1_001A,
            model_sub: 0,
            equip_slot_category,
            stain_ids: [0, 0],
        };
        let mut resource = SqPackResource::from_existing(&game_dir());

        let (customize, top_only, skeleton) = load_dressed(
            &mut resource,
            "dressed-au-ra-crescent-robe",
            vec![robe(8559, "弦月睡袍", 4)],
        );
        assert!(
            top_only
                .hidden_body_attributes
                .iter()
                .any(|entry| entry == "top:cloth"),
            "robe top must hide the smallclothes torso: {:?}",
            top_only.hidden_body_attributes
        );
        let snapshot = render_dressed_front(
            "dressed-au-ra-crescent-robe-top-only",
            &customize,
            &top_only,
            &skeleton,
        );
        assert_non_uniform_render(&snapshot, "crescent-robe-top-only");

        let (customize, with_dwn, skeleton) = load_dressed(
            &mut resource,
            "dressed-au-ra-crescent-robe",
            vec![robe(8559, "弦月睡袍", 4), robe(8560, "弦月睡裤", 7)],
        );
        assert!(
            with_dwn
                .hidden_body_attributes
                .iter()
                .any(|entry| entry == "dwn:cloth"),
            "robe + bottoms must hide the smallclothes legs: {:?}",
            with_dwn.hidden_body_attributes
        );
        let snapshot = render_dressed_front(
            "dressed-au-ra-crescent-robe-with-dwn",
            &customize,
            &with_dwn,
            &skeleton,
        );
        assert_non_uniform_render(&snapshot, "crescent-robe-with-dwn");
    }

    /// Case D：敖龙女 + 女仆装（e6016）五件全身正面。e6016 仅 met 有敖龙女
    /// 自有模型，top/glv/dwn/sho 走种族骨变形树回退（c1401→c0201→c0101）。
    #[test]
    #[ignore = "renders dressed au-ra maid set to target/weapon-render-snapshots; requires XIV_GAME_DIR"]
    fn render_dressed_au_ra_maid_front() {
        let mut resource = SqPackResource::from_existing(&game_dir());
        let (customize, data, skeleton) =
            load_dressed(&mut resource, "dressed-au-ra-maid", maid_set());
        let snapshot =
            render_dressed_front("dressed-au-ra-maid-front", &customize, &data, &skeleton);
        assert_non_uniform_render(&snapshot, "au-ra-maid-front");
    }

    /// Case E：鲁加男 + 女仆装（对照组：c0901 有全套自有模型，不经回退）。
    #[test]
    #[ignore = "renders dressed roegadyn male maid set to target/weapon-render-snapshots; requires XIV_GAME_DIR"]
    fn render_dressed_roe_m_maid_front() {
        let mut resource = SqPackResource::from_existing(&game_dir());
        let (customize, data, skeleton) = load_dressed_as(
            &mut resource,
            roe_male_customize(),
            "dressed-roe-m-maid",
            maid_set(),
        );
        let snapshot = render_dressed_front_zoom(
            "dressed-roe-m-maid-front",
            &customize,
            &data,
            &skeleton,
            3.4,
        );
        assert_non_uniform_render(&snapshot, "roe-m-maid-front");
    }

    /// 模型的可见网格签名（路径 + 顶点数多重集，渲染谓词同 flatten：整网格
    /// 隐藏跳过、attribute 隐藏经按名启用在 prepare 落地、加色网格保留）。
    fn visible_mesh_signature(
        model: &xiv_companion_data::CharacterAssemblyData,
        options: &PreparedModelOptions,
    ) -> Vec<(String, usize)> {
        let prepared =
            xiv_companion_data::prepare_model_for_render_with_options(model, options.clone());
        let mut signature = Vec::new();
        for mesh in &prepared.meshes {
            if mesh.mesh_hidden {
                continue;
            }
            if !mesh.renders_in_main_pass
                && !mesh.prepared_material.render_pass.uses_additive_pipeline()
            {
                continue;
            }
            let source = &model.meshes[mesh.mesh_index];
            signature.push((source.path.clone(), source.vertices.len()));
        }
        signature.sort();
        signature
    }

    fn model_has_attribute_submeshes(model: &xiv_companion_data::CharacterAssemblyData) -> bool {
        model.meshes.iter().any(|mesh| {
            mesh.submesh
                .as_ref()
                .is_some_and(|submesh| submesh.attribute_index_mask != 0)
        })
    }

    /// Case F：逐件场景（scene 加载 + 遮蔽计划隐藏标签）与合并版（加载期网格
    /// 过滤）的可见网格完全一致（敖龙女 + e0908 五件，无染色）。这是逐件化
    /// 重构的行为等价锚：件级缓存/增量换装路径渲染的网格集合与合并路径一致。
    #[test]
    #[ignore = "compares per-piece scene loading against the merged loader; requires XIV_GAME_DIR"]
    fn dressed_scene_pieces_match_merged_visibility() {
        let mut resource = SqPackResource::from_existing(&game_dir());
        let customize = au_ra_female_customize();
        let palette = load_palette_package();
        let appearance = appearance_colors_from_palette(&customize, &palette.palette);

        let merged_request = DressedCharacterLoadRequest::new(customize, "scene-parity-merged")
            .with_appearance(appearance)
            .with_equipment(e0908_set([0, 0]));
        let (merged, merged_skeleton) =
            load_dressed_character_with_skeleton_from_resource(&mut resource, &merged_request)
                .unwrap_or_else(|error| panic!("merged load: {error:#}"));

        let scene_request = DressedCharacterLoadRequest::new(customize, "scene-parity-scene")
            .with_equipment(e0908_set([0, 0]));
        let scene = load_dressed_character_scene_from_resource(&mut resource, &scene_request)
            .unwrap_or_else(|error| panic!("scene load: {error:#}"));

        let skeleton = scene
            .skeleton
            .as_ref()
            .unwrap_or_else(|| panic!("scene: missing skeleton"));
        assert_eq!(
            skeleton.bone_count(),
            merged_skeleton
                .as_ref()
                .map(|skeleton| skeleton.bone_count())
                .unwrap_or(0),
            "scene skeleton must match the merged loader skeleton"
        );
        assert_eq!(scene.pieces.len(), 5, "scene: all five pieces load");

        // 合并版可见签名：加载期已过滤网格 + 按名启用（捏脸特征件）。
        let merged_signature = visible_mesh_signature(
            &merged.model,
            &PreparedModelOptions::default()
                .with_component_preview_layout(false)
                .with_enabled_attribute_names(character_enabled_attribute_names(
                    &customize,
                    &merged.model,
                )),
        );

        // 场景版可见签名：身体（隐藏标签 + 默认启用名 − 遮蔽名）+ 各件（IMC
        // 变体名 + 跨件规则），选项组装语义与幻化页一致。
        let plan = plan_dressed_concealment(&customize, &scene.body, &scene.pieces);
        let mut body_names = character_enabled_attribute_names(&customize, &scene.body);
        for hidden in &plan.body_hidden_attributes {
            body_names.retain(|name| name != hidden);
        }
        let mut scene_signature = visible_mesh_signature(
            &scene.body,
            &PreparedModelOptions::default()
                .with_component_preview_layout(false)
                .with_hidden_mesh_indices(plan.body_hidden_meshes.clone())
                .with_enabled_attribute_names(body_names),
        );
        for piece in &scene.pieces {
            if plan
                .hidden_pieces
                .contains(&(piece.equip_slot_category, piece.item_id))
            {
                continue;
            }
            let mut options = PreparedModelOptions::default().with_component_preview_layout(false);
            if model_has_attribute_submeshes(&piece.model) {
                if let Some((_, names)) =
                    plan.piece_enabled_attributes
                        .iter()
                        .find(|((slot, item_id), _)| {
                            *slot == piece.equip_slot_category && *item_id == piece.item_id
                        })
                {
                    options = options.with_enabled_attribute_names(names.clone());
                }
            }
            scene_signature.extend(visible_mesh_signature(&piece.model, &options));
        }
        scene_signature.sort();

        eprintln!(
            "merged visible meshes: {} / scene visible meshes: {} / concealment notes: {:?}",
            merged_signature.len(),
            scene_signature.len(),
            plan.hidden_notes
        );
        assert_eq!(
            merged_signature, scene_signature,
            "per-piece scene with concealment tags must render exactly the merged loader's visible meshes"
        );
    }

    /// Case F2：头部/尾部新规则的合并版↔逐件版一致性锚——幽灵套装（137，
    /// 按 top 槽位语义加载）+ 耳坠（7215）：脸/发/角/尾隐藏 + 耳饰件整件隐藏，
    /// 两路径可见网格集合必须一致。
    #[test]
    #[ignore = "compares per-piece scene loading against the merged loader for head/tail EQP rules; requires XIV_GAME_DIR"]
    fn dressed_scene_pieces_match_merged_visibility_head_rules() {
        let mut resource = SqPackResource::from_existing(&game_dir());
        let customize = au_ra_female_customize();
        let palette = load_palette_package();
        let appearance = appearance_colors_from_palette(&customize, &palette.palette);
        let equipment = || {
            vec![
                gear_piece(6107, "尖啸幽灵套装", 4, 0x1_0089),
                gear_piece(7215, "亚拉戈高位咏咒耳坠", 9, 0x1_002B),
            ]
        };

        let merged_request = DressedCharacterLoadRequest::new(customize, "scene-parity-merged")
            .with_appearance(appearance)
            .with_equipment(equipment());
        let (merged, merged_skeleton) =
            load_dressed_character_with_skeleton_from_resource(&mut resource, &merged_request)
                .unwrap_or_else(|error| panic!("merged load: {error:#}"));

        let scene_request = DressedCharacterLoadRequest::new(customize, "scene-parity-scene")
            .with_equipment(equipment());
        let scene = load_dressed_character_scene_from_resource(&mut resource, &scene_request)
            .unwrap_or_else(|error| panic!("scene load: {error:#}"));
        assert!(scene.skeleton.is_some());
        assert!(merged_skeleton.is_some());
        assert_eq!(scene.pieces.len(), 2, "scene: ghost top + earring load");

        let merged_signature = visible_mesh_signature(
            &merged.model,
            &PreparedModelOptions::default()
                .with_component_preview_layout(false)
                .with_enabled_attribute_names(character_enabled_attribute_names(
                    &customize,
                    &merged.model,
                )),
        );

        let plan = plan_dressed_concealment(&customize, &scene.body, &scene.pieces);
        assert!(
            plan.hidden_pieces.contains(&(9, 7215)),
            "earring hidden in the scene plan: {:?}",
            plan.hidden_pieces
        );
        let mut body_names = character_enabled_attribute_names(&customize, &scene.body);
        for hidden in &plan.body_hidden_attributes {
            body_names.retain(|name| name != hidden);
        }
        let mut scene_signature = visible_mesh_signature(
            &scene.body,
            &PreparedModelOptions::default()
                .with_component_preview_layout(false)
                .with_hidden_mesh_indices(plan.body_hidden_meshes.clone())
                .with_enabled_attribute_names(body_names),
        );
        for piece in &scene.pieces {
            if plan
                .hidden_pieces
                .contains(&(piece.equip_slot_category, piece.item_id))
            {
                continue;
            }
            let mut options = PreparedModelOptions::default().with_component_preview_layout(false);
            if model_has_attribute_submeshes(&piece.model) {
                if let Some((_, names)) =
                    plan.piece_enabled_attributes
                        .iter()
                        .find(|((slot, item_id), _)| {
                            *slot == piece.equip_slot_category && *item_id == piece.item_id
                        })
                {
                    options = options.with_enabled_attribute_names(names.clone());
                }
            }
            scene_signature.extend(visible_mesh_signature(&piece.model, &options));
        }
        scene_signature.sort();

        eprintln!(
            "merged visible meshes: {} / scene visible meshes: {} / concealment notes: {:?}",
            merged_signature.len(),
            scene_signature.len(),
            plan.hidden_notes
        );
        assert_eq!(
            merged_signature, scene_signature,
            "per-piece scene with concealment tags must render exactly the merged loader's visible meshes"
        );
    }

    // -----------------------------------------------------------------------
    // 武器挂接（主手/副手武器进着装预览）
    // -----------------------------------------------------------------------

    /// 武器件：item 目录行（id + EquipSlotCategory + Model{Main,Sub} 原始值）。
    /// 丰水长剑/丰水之盾/亥伯龙的目录数据见 assets/collection-catalog.json。
    fn weapon_piece(
        item_id: u32,
        name: &str,
        equip_slot_category: u32,
        model_main: u64,
        model_sub: u64,
    ) -> DressedEquipmentPiece {
        DressedEquipmentPiece {
            item_id,
            item_name: name.to_string(),
            model_main,
            model_sub,
            equip_slot_category,
            stain_ids: [0, 0],
        }
    }

    /// 敖龙女 + e0908 五件 + 单手剑主手（24643 丰水长剑，cat 1）+ 盾副手
    /// （24658 丰水之盾，cat 2）。
    fn e0908_sword_shield_equipment() -> Vec<DressedEquipmentPiece> {
        let mut equipment = e0908_set([0, 0]);
        equipment.push(weapon_piece(24643, "丰水长剑", 1, 0x2006A_00C9, 0));
        equipment.push(weapon_piece(24658, "丰水之盾", 2, 0x20049_0065, 0));
        equipment
    }

    /// 敖龙女 + e0908 五件 + 双手大剑（34057 亥伯龙，cat 13，只挂右手）。
    fn e0908_two_handed_equipment() -> Vec<DressedEquipmentPiece> {
        let mut equipment = e0908_set([0, 0]);
        equipment.push(weapon_piece(34057, "亥伯龙", 13, 0x1002C_09C5, 0));
        equipment
    }

    /// 敖龙女 + e0908 五件 + 刀（21328 一贯斋，cat 13）：主模型刀身挂右手、
    /// 次模型（刀鞘）挂左手——次模型挂接路径用例。
    fn e0908_katana_equipment() -> Vec<DressedEquipmentPiece> {
        let mut equipment = e0908_set([0, 0]);
        equipment.push(weapon_piece(
            21328,
            "一贯斋",
            13,
            0x10034_07D1,
            0x10001_0833,
        ));
        equipment
    }

    /// 逐件场景加载（武器件走挂点骨单骨烘焙），打印件级诊断。骨架缺失直接
    /// 失败（挂接前提）。
    fn load_dressed_scene(
        resource: &mut SqPackResource,
        name: &str,
        equipment: Vec<DressedEquipmentPiece>,
    ) -> (CharacterCustomize, xiv_companion::DressedCharacterScene) {
        let customize = au_ra_female_customize();
        let palette = load_palette_package();
        let appearance = appearance_colors_from_palette(&customize, &palette.palette);
        let request = DressedCharacterLoadRequest::new(customize, name)
            .with_appearance(appearance)
            .with_equipment(equipment);
        let scene = load_dressed_character_scene_from_resource(resource, &request)
            .unwrap_or_else(|error| panic!("load scene {name}: {error:#}"));
        assert!(
            scene.skeleton.is_some(),
            "{name}: missing human skeleton (weapon attach requires it)"
        );
        eprintln!(
            "{name}: pieces={} bones={}",
            scene.pieces.len(),
            scene
                .skeleton
                .as_ref()
                .map_or(0, |skeleton| skeleton.bone_count()),
        );
        for piece in &scene.pieces {
            eprintln!(
                "{name}: piece {} slot {} attach={} meshes={}",
                piece.item_name,
                piece.equip_slot_category,
                piece.attach.is_some(),
                piece.model.meshes.len(),
            );
        }
        for diagnostic in &scene.load_diagnostics {
            eprintln!("{name}: diagnostic: {}", diagnostic.error);
        }
        (customize, scene)
    }

    /// 件模型的实例 joint 名表（各网格 bone table 名并集，首见顺序——与
    /// 渲染器 `register_mesh_joint_names` 的建表顺序一致）。
    fn piece_joint_names(model: &xiv_companion_data::WeaponModelData) -> Vec<String> {
        let mut names: Vec<String> = Vec::new();
        for mesh in &model.meshes {
            let Some(bone_table) = &mesh.bone_table else {
                continue;
            };
            for name in bone_table.bone_names.iter().flatten() {
                if !names.contains(name) {
                    names.push(name.clone());
                }
            }
        }
        names
    }

    /// 场景 → 快照条目（与幻化页同语义：身体隐藏标签 + 按名启用、件 IMC
    /// 名单；武器件关节矩阵按挂接规则现算 `pose` 下的矩阵并覆盖上传）。
    fn scene_snapshot_entries<'a>(
        customize: &CharacterCustomize,
        scene: &'a xiv_companion::DressedCharacterScene,
        pose: &xiv_companion_data::SkeletonPose,
    ) -> Vec<SceneSnapshotEntry<'a>> {
        let skeleton = scene.skeleton.as_ref().expect("scene skeleton");
        let plan = plan_dressed_concealment(customize, &scene.body, &scene.pieces);
        let mut entries = Vec::new();
        let mut body_names = character_enabled_attribute_names(customize, &scene.body);
        for hidden in &plan.body_hidden_attributes {
            body_names.retain(|name| name != hidden);
        }
        let body_options = PreparedModelOptions::default()
            .with_component_preview_layout(false)
            .with_hidden_mesh_indices(plan.body_hidden_meshes.clone())
            .with_enabled_attribute_names(body_names);
        entries.push(SceneSnapshotEntry::new(&scene.body).with_prepared_options(body_options));
        for piece in &scene.pieces {
            // 整件隐藏的件（如耳饰位门控的耳饰件）不建条目（与幻化页一致）。
            if plan
                .hidden_pieces
                .contains(&(piece.equip_slot_category, piece.item_id))
            {
                continue;
            }
            let mut options = PreparedModelOptions::default().with_component_preview_layout(false);
            if let Some((_, names)) =
                plan.piece_enabled_attributes
                    .iter()
                    .find(|((slot, item_id), _)| {
                        *slot == piece.equip_slot_category && *item_id == piece.item_id
                    })
            {
                options = options.with_enabled_attribute_names(names.clone());
            }
            let mut entry =
                SceneSnapshotEntry::new(piece.model.as_ref()).with_prepared_options(options);
            if let Some(attach) = &piece.attach {
                let joint_names = piece_joint_names(&piece.model);
                assert!(
                    !joint_names.is_empty(),
                    "weapon piece {} has no attach joints",
                    piece.item_name
                );
                entry =
                    entry.with_joint_matrices(xiv_companion_data::weapon_attach_joint_matrices(
                        skeleton,
                        pose,
                        &joint_names,
                        attach.correction,
                    ));
            }
            entries.push(entry);
        }
        entries
    }

    /// 渲染场景快照（多实例路径）：非武器件按 `pose` 出世界 × inverse bind
    /// 关节矩阵，武器件按挂接规则现算（见 `scene_snapshot_entries`）。
    fn render_dressed_scene(
        name: &str,
        customize: &CharacterCustomize,
        scene: &xiv_companion::DressedCharacterScene,
        pose: &xiv_companion_data::SkeletonPose,
    ) -> ModelSnapshot {
        let skeleton = scene.skeleton.as_ref().expect("scene skeleton");
        let entries = scene_snapshot_entries(customize, scene, pose);
        let snapshot = render_scene_snapshot(
            ModelSnapshotOptions::new(name)
                .with_viewport(720, 900)
                .with_camera(0.35, 0.12, 2.6, [0.0, 0.0]),
            &entries,
            Some(skeleton),
            Some(pose),
        )
        .unwrap_or_else(|error| panic!("render {name}: {error}"));
        eprintln!("png: {}", snapshot.png_path.display());
        snapshot
    }

    /// 加载角色 action.pap（第一个可用候选），返回动画集。
    fn load_character_animations(
        resource: &mut SqPackResource,
        customize: &CharacterCustomize,
        skeleton: &ModelSkeleton,
    ) -> xiv_companion_data::ModelAnimationSet {
        use physis::resource::Resource;
        for path in pap_path_candidates(AnimationSourceKind::Character {
            race_code: customize.race_code(),
        }) {
            let Some(bytes) = resource.read(&path) else {
                continue;
            };
            if let Ok(set) = load_animation_set_from_pap_bytes(&bytes, skeleton) {
                eprintln!("animations from {path}: {} clips", set.animations.len());
                return set;
            }
        }
        panic!("no usable character animations (action.pap)")
    }

    /// 武器跟随动画的数值锚：挂点骨姿势世界原点在 rest/采样姿势下应有
    /// 可测位移（动画确实在驱动手骨）。
    fn assert_attach_follows_pose(
        scene: &xiv_companion::DressedCharacterScene,
        pose: &xiv_companion_data::SkeletonPose,
        label: &str,
    ) {
        let skeleton = scene.skeleton.as_ref().expect("scene skeleton");
        let rest = skeleton.scaled_rest_pose();
        for piece in scene.pieces.iter().filter(|piece| piece.attach.is_some()) {
            let attach = piece.attach.as_ref().unwrap();
            let joint_names = piece_joint_names(&piece.model);
            let rest_matrices = xiv_companion_data::weapon_attach_joint_matrices(
                skeleton,
                &rest,
                &joint_names,
                attach.correction,
            );
            let posed_matrices = xiv_companion_data::weapon_attach_joint_matrices(
                skeleton,
                pose,
                &joint_names,
                attach.correction,
            );
            for (rest_matrix, posed_matrix) in rest_matrices.iter().zip(&posed_matrices) {
                let rest_origin = xiv_companion_data::mat4_transform_point(*rest_matrix, [0.0; 3]);
                let posed_origin =
                    xiv_companion_data::mat4_transform_point(*posed_matrix, [0.0; 3]);
                let moved = (0..3)
                    .map(|axis| (posed_origin[axis] - rest_origin[axis]).abs())
                    .sum::<f32>();
                eprintln!(
                    "{label}: {} attach origin rest={rest_origin:?} posed={posed_origin:?} moved={moved}",
                    piece.item_name
                );
                assert!(
                    moved > 0.01,
                    "{label}: {} attach joint must follow the animated hand bone",
                    piece.item_name
                );
            }
        }
    }

    /// Case G：敖龙女 + e0908 五件 + 主手剑/副手盾。rest 与 action.pap
    /// 动画帧各渲一张；武器应落在手中（挂点骨），动画帧里随手移动。
    #[test]
    #[ignore = "renders dressed au-ra with sword+shield to target/weapon-render-snapshots; requires XIV_GAME_DIR"]
    fn render_dressed_au_ra_e0908_sword_shield() {
        let mut resource = SqPackResource::from_existing(&game_dir());
        let (customize, scene) = load_dressed_scene(
            &mut resource,
            "dressed-au-ra-e0908-sword-shield",
            e0908_sword_shield_equipment(),
        );
        let skeleton = scene.skeleton.as_ref().unwrap().clone();

        let rest_pose = skeleton.scaled_rest_pose();
        let rest = render_dressed_scene(
            "dressed-au-ra-e0908-sword-shield-rest",
            &customize,
            &scene,
            &rest_pose,
        );
        let rest_pixels = assert_non_uniform_render(&rest, "sword-shield-rest");

        let animations = load_character_animations(&mut resource, &customize, &skeleton);
        let index = 0;
        let duration = animations.animations[index].duration_ms.max(1.0);
        let pose = xiv_companion_data::sample_animation_pose(
            &animations,
            index,
            duration * 0.4,
            &skeleton,
        );
        assert_attach_follows_pose(&scene, &pose, "sword-shield");
        let animated = render_dressed_scene(
            "dressed-au-ra-e0908-sword-shield-animated",
            &customize,
            &scene,
            &pose,
        );
        let animated_pixels = assert_non_uniform_render(&animated, "sword-shield-animated");
        assert!(
            rgb_difference(&rest_pixels, &animated_pixels) > 100_000,
            "the animated frame must visibly differ from rest (weapon follows the hand)"
        );
    }

    /// Case H：敖龙女 + e0908 五件 + 双手大剑（cat 13 只挂右手骨）。
    #[test]
    #[ignore = "renders dressed au-ra with a two-handed weapon; requires XIV_GAME_DIR"]
    fn render_dressed_au_ra_e0908_two_handed() {
        let mut resource = SqPackResource::from_existing(&game_dir());
        let (customize, scene) = load_dressed_scene(
            &mut resource,
            "dressed-au-ra-e0908-two-handed",
            e0908_two_handed_equipment(),
        );
        let skeleton = scene.skeleton.as_ref().unwrap().clone();
        // 双手武器：只有右手挂点（主模型），joint 表恰为 [n_buki_r]。
        let weapon = scene
            .pieces
            .iter()
            .find(|piece| piece.equip_slot_category == 13)
            .expect("two-handed weapon piece");
        assert_eq!(
            piece_joint_names(&weapon.model),
            vec![xiv_companion_data::WEAPON_ATTACH_BONE_MAIN_HAND.to_string()],
            "two-handed weapon attaches to the right hand bone only"
        );

        let rest_pose = skeleton.scaled_rest_pose();
        let rest = render_dressed_scene(
            "dressed-au-ra-e0908-two-handed-rest",
            &customize,
            &scene,
            &rest_pose,
        );
        let rest_pixels = assert_non_uniform_render(&rest, "two-handed-rest");

        let animations = load_character_animations(&mut resource, &customize, &skeleton);
        let index = 0;
        let duration = animations.animations[index].duration_ms.max(1.0);
        let pose = xiv_companion_data::sample_animation_pose(
            &animations,
            index,
            duration * 0.4,
            &skeleton,
        );
        assert_attach_follows_pose(&scene, &pose, "two-handed");
        let animated = render_dressed_scene(
            "dressed-au-ra-e0908-two-handed-animated",
            &customize,
            &scene,
            &pose,
        );
        let animated_pixels = assert_non_uniform_render(&animated, "two-handed-animated");
        assert!(
            rgb_difference(&rest_pixels, &animated_pixels) > 100_000,
            "the animated frame must visibly differ from rest (weapon follows the hand)"
        );
    }

    /// Case I：敖龙女 + e0908 五件 + 刀（主模型刀身 → n_buki_r，次模型刀鞘
    /// → n_buki_l）。次模型挂接（成对武器/刀鞘）路径用例。
    #[test]
    #[ignore = "renders dressed au-ra with a katana + scabbard; requires XIV_GAME_DIR"]
    fn render_dressed_au_ra_e0908_katana() {
        let mut resource = SqPackResource::from_existing(&game_dir());
        let (customize, scene) = load_dressed_scene(
            &mut resource,
            "dressed-au-ra-e0908-katana",
            e0908_katana_equipment(),
        );
        let skeleton = scene.skeleton.as_ref().unwrap().clone();
        // 刀：joint 表 = [n_buki_r, n_buki_l]（主模型刀身 + 次模型刀鞘）。
        let weapon = scene
            .pieces
            .iter()
            .find(|piece| piece.equip_slot_category == 13)
            .expect("katana piece");
        assert_eq!(
            piece_joint_names(&weapon.model),
            vec![
                xiv_companion_data::WEAPON_ATTACH_BONE_MAIN_HAND.to_string(),
                xiv_companion_data::WEAPON_ATTACH_BONE_OFF_HAND.to_string(),
            ],
            "katana attaches blade to the right hand and scabbard to the left"
        );

        let rest_pose = skeleton.scaled_rest_pose();
        let rest = render_dressed_scene(
            "dressed-au-ra-e0908-katana-rest",
            &customize,
            &scene,
            &rest_pose,
        );
        let rest_pixels = assert_non_uniform_render(&rest, "katana-rest");

        let animations = load_character_animations(&mut resource, &customize, &skeleton);
        let index = 0;
        let duration = animations.animations[index].duration_ms.max(1.0);
        let pose = xiv_companion_data::sample_animation_pose(
            &animations,
            index,
            duration * 0.4,
            &skeleton,
        );
        assert_attach_follows_pose(&scene, &pose, "katana");
        let animated = render_dressed_scene(
            "dressed-au-ra-e0908-katana-animated",
            &customize,
            &scene,
            &pose,
        );
        let animated_pixels = assert_non_uniform_render(&animated, "katana-animated");
        assert!(
            rgb_difference(&rest_pixels, &animated_pixels) > 100_000,
            "the animated frame must visibly differ from rest (weapon follows the hand)"
        );
    }

    // -----------------------------------------------------------------------
    // EQP 头部/尾部显隐规则（耳饰种族组位、耳/角几何、全身套装遮头、遮尾）
    // -----------------------------------------------------------------------

    /// 装备件快捷构造（model_main 原始值）。
    fn gear_piece(
        item_id: u32,
        name: &str,
        equip_slot_category: u32,
        model_main: u64,
    ) -> DressedEquipmentPiece {
        DressedEquipmentPiece {
            item_id,
            item_name: name.to_string(),
            model_main,
            model_sub: 0,
            equip_slot_category,
            stain_ids: [0, 0],
        }
    }

    /// 头部近景渲染（rest pose；相机拉近并对准头部，便于核对角/耳饰显隐）。
    fn render_dressed_scene_head(
        name: &str,
        customize: &CharacterCustomize,
        scene: &xiv_companion::DressedCharacterScene,
    ) -> ModelSnapshot {
        let skeleton = scene.skeleton.as_ref().expect("scene skeleton");
        let pose = skeleton.scaled_rest_pose();
        let entries = scene_snapshot_entries(customize, scene, &pose);
        let snapshot = render_scene_snapshot(
            ModelSnapshotOptions::new(name)
                .with_viewport(720, 900)
                .with_camera(0.15, 0.05, 1.15, [0.0, 0.85]),
            &entries,
            Some(skeleton),
            Some(&pose),
        )
        .unwrap_or_else(|error| panic!("render {name}: {error}"));
        eprintln!("png: {}", snapshot.png_path.display());
        snapshot
    }

    /// Case J：开面帽的耳/角/耳饰显隐。敖龙女 + 耳坠（7215，set 43）：
    /// - C1战术兜帽（41544，set 836）：敖龙组耳饰位（49）与角位（52）置位 →
    ///   角与耳坠显示；
    /// - 黯云制敌头盔（44610，set 871）：46-53 全清 → 角隐藏（脸部
    ///   atr_hrn）、耳坠整件隐藏、头发保留（41/42 清）。
    #[test]
    #[ignore = "renders au-ra with open helmets gating horns/earrings; requires XIV_GAME_DIR"]
    fn render_dressed_au_ra_head_eqp_horn_and_earring_gating() {
        let mut resource = SqPackResource::from_existing(&game_dir());
        let earring = gear_piece(7215, "亚拉戈高位咏咒耳坠", 9, 0x1_002B);
        let hood = gear_piece(41544, "C1战术兜帽", 3, 0x1_0344);
        let kabuto = gear_piece(44610, "黯云制敌头盔", 3, 0x2_0367);

        let (customize, hood_scene) = load_dressed_scene(
            &mut resource,
            "dressed-au-ra-hood-earring",
            vec![earring.clone(), hood],
        );
        let hood_plan = plan_dressed_concealment(&customize, &hood_scene.body, &hood_scene.pieces);
        assert!(
            hood_plan.hidden_pieces.is_empty(),
            "hood keeps the earring for au ra: {:?}",
            hood_plan.hidden_pieces
        );
        assert!(
            !hood_plan
                .body_hidden_attributes
                .iter()
                .any(|name| name == "atr_hrn"),
            "hood keeps au-ra horns: {:?}",
            hood_plan.hidden_notes
        );
        let hood_shot = render_dressed_scene(
            "dressed-au-ra-hood-earring",
            &customize,
            &hood_scene,
            &hood_scene.skeleton.as_ref().unwrap().scaled_rest_pose(),
        );
        let hood_pixels = assert_non_uniform_render(&hood_shot, "au-ra-hood-earring");
        let hood_head =
            render_dressed_scene_head("dressed-au-ra-hood-earring-head", &customize, &hood_scene);
        assert_non_uniform_render(&hood_head, "au-ra-hood-earring-head");

        let (customize, kabuto_scene) = load_dressed_scene(
            &mut resource,
            "dressed-au-ra-kabuto-earring",
            vec![earring, kabuto],
        );
        let kabuto_plan =
            plan_dressed_concealment(&customize, &kabuto_scene.body, &kabuto_scene.pieces);
        assert!(
            kabuto_plan.hidden_pieces.contains(&(9, 7215)),
            "kabuto hides the earring for au ra: {:?}",
            kabuto_plan.hidden_pieces
        );
        assert!(
            kabuto_plan
                .body_hidden_attributes
                .iter()
                .any(|name| name == "atr_hrn"),
            "kabuto hides au-ra horns: {:?}",
            kabuto_plan.hidden_notes
        );
        let kabuto_shot = render_dressed_scene(
            "dressed-au-ra-kabuto-earring",
            &customize,
            &kabuto_scene,
            &kabuto_scene.skeleton.as_ref().unwrap().scaled_rest_pose(),
        );
        let kabuto_pixels = assert_non_uniform_render(&kabuto_shot, "au-ra-kabuto-earring");
        let kabuto_head = render_dressed_scene_head(
            "dressed-au-ra-kabuto-earring-head",
            &customize,
            &kabuto_scene,
        );
        assert_non_uniform_render(&kabuto_head, "au-ra-kabuto-earring-head");
        assert!(
            rgb_difference(&hood_pixels, &kabuto_pixels) > 100_000,
            "horn/earring gating must visibly differ between hood and kabuto"
        );

        // 耳坠单戴对照（无头部件 → 恒显示，鲶鱼耳饰模型大、便于目检）。
        let mameshiba = gear_piece(24612, "鲶鱼耳饰", 9, 0x1_0069);
        let (customize, earring_scene) = load_dressed_scene(
            &mut resource,
            "dressed-au-ra-earring-only",
            vec![mameshiba.clone()],
        );
        let earring_plan =
            plan_dressed_concealment(&customize, &earring_scene.body, &earring_scene.pieces);
        assert!(earring_plan.hidden_pieces.is_empty());
        let earring_head = render_dressed_scene_head(
            "dressed-au-ra-earring-only-head",
            &customize,
            &earring_scene,
        );
        assert_non_uniform_render(&earring_head, "au-ra-earring-only-head");

        // 同一耳坠 + 黯云制敌头盔 → 整件隐藏。
        let kabuto = gear_piece(44610, "黯云制敌头盔", 3, 0x2_0367);
        let (customize, hidden_scene) = load_dressed_scene(
            &mut resource,
            "dressed-au-ra-earring-hidden",
            vec![mameshiba, kabuto],
        );
        let hidden_plan =
            plan_dressed_concealment(&customize, &hidden_scene.body, &hidden_scene.pieces);
        assert!(hidden_plan.hidden_pieces.contains(&(9, 24612)));
        let hidden_head = render_dressed_scene_head(
            "dressed-au-ra-earring-hidden-head",
            &customize,
            &hidden_scene,
        );
        assert_non_uniform_render(&hidden_head, "au-ra-earring-hidden-head");
    }

    /// Case K：全身套装遮头（BodyShowHead 清）。敖龙女 + 幽灵套装（6107，
    /// set 137，游戏内组合槽 19，这里按 top 槽位语义加载）+ 耳坠：脸/发/角/
    /// 尾/耳饰全隐（头部规则改读 top 条目）。对照组：无装备裸装（全显示）。
    #[test]
    #[ignore = "renders au-ra in the ghost costume hiding the whole head; requires XIV_GAME_DIR"]
    fn render_dressed_au_ra_ghost_costume_hides_head() {
        let mut resource = SqPackResource::from_existing(&game_dir());
        let ghost = gear_piece(6107, "尖啸幽灵套装", 4, 0x1_0089);
        let earring = gear_piece(7215, "亚拉戈高位咏咒耳坠", 9, 0x1_002B);

        let (customize, ghost_scene) =
            load_dressed_scene(&mut resource, "dressed-au-ra-ghost", vec![ghost, earring]);
        let ghost_plan =
            plan_dressed_concealment(&customize, &ghost_scene.body, &ghost_scene.pieces);
        for note in ["face:all", "hair:all", "tail:all", "ear:gear"] {
            assert!(
                ghost_plan.hidden_notes.iter().any(|entry| entry == note),
                "ghost costume must record {note}: {:?}",
                ghost_plan.hidden_notes
            );
        }
        assert!(ghost_plan.hidden_pieces.contains(&(9, 7215)));
        let ghost_shot = render_dressed_scene(
            "dressed-au-ra-ghost",
            &customize,
            &ghost_scene,
            &ghost_scene.skeleton.as_ref().unwrap().scaled_rest_pose(),
        );
        let ghost_pixels = assert_non_uniform_render(&ghost_shot, "au-ra-ghost");

        // 对照：裸装（无件）全身。
        let (customize, bare_scene) =
            load_dressed_scene(&mut resource, "dressed-au-ra-bare", Vec::new());
        let bare_shot = render_dressed_scene(
            "dressed-au-ra-bare",
            &customize,
            &bare_scene,
            &bare_scene.skeleton.as_ref().unwrap().scaled_rest_pose(),
        );
        let bare_pixels = assert_non_uniform_render(&bare_shot, "au-ra-bare");
        assert!(
            rgb_difference(&ghost_pixels, &bare_pixels) > 100_000,
            "ghost costume must visibly differ from the bare body"
        );
    }

    // -----------------------------------------------------------------------
    // 体型缩放（RGSP：身高/胸围/尾长骨缩放）
    // -----------------------------------------------------------------------

    /// 探针：各网格的 bone table / 顶点权重覆盖 / 骨架缺失骨名清单。缺失骨
    /// （脸部 `j_f_*`、发件 `j_ex_h*`——partial skeleton 骨，不随装配加载）
    /// 的顶点经 [`xiv_companion_data::joint_matrices`] 的锚定回退跟随头骨
    /// （RGSP 身高缩放/头部姿势下与头一致）；此探针用于核对该回退覆盖面。
    #[test]
    #[ignore = "probes mesh skinning coverage; requires XIV_GAME_DIR"]
    fn probe_rgsp_head_skinning_coverage() {
        let mut resource = SqPackResource::from_existing(&game_dir());
        let (customize, data, skeleton) = load_dressed_as(
            &mut resource,
            au_ra_female_customize(),
            "dressed-au-ra-rgsp",
            e0908_set([0, 0]),
        );
        let _ = customize;
        eprintln!("skeleton: {} bones", skeleton.bone_count());
        for mesh in &data.model.meshes {
            let total = mesh.vertices.len();
            let weighted = mesh
                .vertices
                .iter()
                .filter(|vertex| {
                    vertex
                        .blend_weights
                        .is_some_and(|weights| weights.count > 0)
                })
                .count();
            let mut missing: Vec<String> = Vec::new();
            if let Some(bone_table) = &mesh.bone_table {
                for name in bone_table.bone_names.iter().flatten() {
                    if skeleton.bone_index(name).is_none() && !missing.contains(name) {
                        missing.push(name.clone());
                    }
                }
            }
            let short = mesh.path.rsplit('/').next().unwrap_or(&mesh.path);
            eprintln!(
                "MESH {short}: verts={total} weighted={weighted} bone_table={} missing_bones={missing:?}",
                mesh.bone_table.is_some()
            );
        }
    }

    /// 前景像素包围盒 (min_x, min_y, max_x, max_y)：背景 = 左上角像素，
    /// 容差与 [`assert_non_uniform_render`] 的前景口径一致（RGB 任一通道差 >8）。
    fn foreground_bbox(snapshot: &ModelSnapshot) -> (u32, u32, u32, u32) {
        let pixels = image::open(&snapshot.png_path)
            .unwrap_or_else(|error| panic!("decode {}: {error}", snapshot.png_path.display()))
            .to_rgba8()
            .into_raw();
        let background = &pixels[..4];
        let mut min = [u32::MAX; 2];
        let mut max = [0u32; 2];
        let mut count = 0usize;
        for (index, pixel) in pixels.chunks_exact(4).enumerate() {
            if !(0..3).any(|channel| pixel[channel].abs_diff(background[channel]) > 8) {
                continue;
            }
            count += 1;
            let x = index as u32 % snapshot.width;
            let y = index as u32 / snapshot.width;
            min[0] = min[0].min(x);
            min[1] = min[1].min(y);
            max[0] = max[0].max(x);
            max[1] = max[1].max(y);
        }
        assert!(
            count > 0,
            "{}: empty foreground",
            snapshot.png_path.display()
        );
        (min[0], min[1], max[0], max[1])
    }

    /// 敖龙女 + e0908 五件，身高 0/50/100：骨架附着 body_scaling 且数值与
    /// human.cmp 表一致（敖龙女身高段 0.93/0.97/1.01）；画面前景包围盒高度
    /// 严格递增（同一机位同一帧幅，缩放直接体现为像素高度）。PNG 全量留档
    /// 供目验。身高 50 档附胸围 0/100 极值对（断言画面可见差异）。
    #[test]
    #[ignore = "renders dressed au-ra at height/bust extremes; requires XIV_GAME_DIR"]
    fn render_dressed_au_ra_body_scaling_height_and_bust() {
        let mut resource = SqPackResource::from_existing(&game_dir());
        let mut bbox_heights = Vec::new();
        let expected_scales = [0.93_f32, 0.97, 1.01];
        for (index, height) in [0u8, 50, 100].iter().enumerate() {
            let customize = CharacterCustomize {
                height: *height,
                ..au_ra_female_customize()
            };
            let (customize, data, skeleton) = load_dressed_as(
                &mut resource,
                customize,
                "dressed-au-ra-rgsp",
                e0908_set([0, 0]),
            );
            let scaling = skeleton
                .body_scaling
                .unwrap_or_else(|| panic!("height {height}: body scaling attached"));
            assert!(
                (scaling.height - expected_scales[index]).abs() < 0.01,
                "height {height}: scale {} ≈ {}",
                scaling.height,
                expected_scales[index]
            );
            let snapshot = render_dressed_front_zoom(
                &format!("dressed-au-ra-rgsp-height-{height}"),
                &customize,
                &data,
                &skeleton,
                3.0,
            );
            assert_non_uniform_render(&snapshot, &format!("rgsp-height-{height}"));
            let (min_x, min_y, max_x, max_y) = foreground_bbox(&snapshot);
            eprintln!(
                "rgsp-height-{height}: bbox x {min_x}..{max_x} y {min_y}..{max_y} (h={})",
                max_y - min_y
            );
            bbox_heights.push(max_y - min_y);
        }
        assert!(
            bbox_heights[0] < bbox_heights[1] && bbox_heights[1] < bbox_heights[2],
            "height slider must order the rendered body height: {bbox_heights:?}"
        );

        // 胸围极值对（身高 50）：bust 0 vs 100 画面可见差异，PNG 留档目验。
        let bust_snapshots = [0u8, 100].map(|bust| {
            let customize = CharacterCustomize {
                bust,
                ..au_ra_female_customize()
            };
            let (customize, data, skeleton) = load_dressed_as(
                &mut resource,
                customize,
                "dressed-au-ra-rgsp",
                e0908_set([0, 0]),
            );
            let scaling = skeleton.body_scaling.expect("bust: body scaling attached");
            let expected = if bust == 0 {
                [0.92, 0.80, 0.816]
            } else {
                [1.08, 1.20, 1.184]
            };
            for axis in 0..3 {
                assert!(
                    (scaling.bust[axis] - expected[axis]).abs() < 0.01,
                    "bust {bust} axis {axis}: {} ≈ {}",
                    scaling.bust[axis],
                    expected[axis]
                );
            }
            let snapshot = render_dressed_front(
                &format!("dressed-au-ra-rgsp-bust-{bust}"),
                &customize,
                &data,
                &skeleton,
            );
            assert_non_uniform_render(&snapshot, &format!("rgsp-bust-{bust}"))
        });
        let difference = rgb_difference(&bust_snapshots[0], &bust_snapshots[1]);
        eprintln!("bust 0-vs-100 RGB difference: {difference}");
        assert!(
            difference > 100_000,
            "bust slider extremes must visibly change the render"
        );
    }

    /// 武器挂接随体型缩放（挂点骨世界矩阵自动跟随，无需武器侧特殊处理）：
    /// 身高 0 vs 100 的挂点原点分量比 == 身高缩放比（0.93/1.01）；动画采样
    /// 姿势同样带身高系数（n_root 局部 scale）。两张武器渲染 PNG 留档目验。
    #[test]
    #[ignore = "verifies weapon attach follows body scaling; requires XIV_GAME_DIR"]
    fn dressed_au_ra_weapon_attach_scales_with_height() {
        let mut resource = SqPackResource::from_existing(&game_dir());
        let joint_names = vec![xiv_companion_data::WEAPON_ATTACH_BONE_MAIN_HAND.to_string()];
        let mut origins = Vec::new();
        let mut scales = Vec::new();
        for height in [0u8, 100] {
            let customize = CharacterCustomize {
                height,
                ..au_ra_female_customize()
            };
            let palette = load_palette_package();
            let appearance = appearance_colors_from_palette(&customize, &palette.palette);
            let request = DressedCharacterLoadRequest::new(customize, "rgsp-attach")
                .with_appearance(appearance)
                .with_equipment(e0908_sword_shield_equipment());
            let scene = load_dressed_character_scene_from_resource(&mut resource, &request)
                .unwrap_or_else(|error| panic!("load scene height {height}: {error:#}"));
            let skeleton = scene.skeleton.as_ref().expect("scene skeleton").clone();
            let scaling = skeleton.body_scaling.expect("body scaling attached");
            let pose = skeleton.scaled_rest_pose();
            let matrices = xiv_companion_data::weapon_attach_joint_matrices(
                &skeleton,
                &pose,
                &joint_names,
                xiv_companion_data::WEAPON_ATTACH_CORRECTION,
            );
            let origin = xiv_companion_data::mat4_transform_point(matrices[0], [0.0; 3]);
            eprintln!(
                "rgsp-attach height {height}: scale={:.3} origin={origin:?}",
                scaling.height
            );
            origins.push(origin);
            scales.push(scaling.height);

            // 动画路径：action.pap 采样后 n_root 局部 scale 仍带身高系数。
            if height == 100 {
                let animations = load_character_animations(&mut resource, &customize, &skeleton);
                let duration = animations.animations[0].duration_ms.max(1.0);
                let sampled = xiv_companion_data::sample_animation_pose(
                    &animations,
                    0,
                    duration * 0.4,
                    &skeleton,
                );
                let root = skeleton
                    .bone_index(xiv_companion_data::HEIGHT_BONE)
                    .expect("n_root bone");
                let root_scale = sampled.transform(root).unwrap().scale;
                assert!(
                    (root_scale[0] - scaling.height).abs() < 1e-4,
                    "sampled pose keeps height scaling on n_root: {root_scale:?} vs {}",
                    scaling.height
                );

                let snapshot = render_dressed_scene(
                    &format!("dressed-au-ra-rgsp-height-{height}-sword"),
                    &customize,
                    &scene,
                    &pose,
                );
                assert_non_uniform_render(&snapshot, "rgsp-height-100-sword");
            }
        }
        let ratio = scales[1] / scales[0];
        for axis in 0..3 {
            let scaled = origins[0][axis] * ratio;
            assert!(
                (origins[1][axis] - scaled).abs() < 1e-3,
                "attach origin axis {axis} scales with height: {} × {ratio:.4} ≈ {}",
                origins[0][axis],
                origins[1][axis]
            );
        }
    }
}

#[cfg(not(feature = "game-data"))]
#[test]
fn dressed_character_render_requires_game_data_feature() {
    panic!("enable the `game-data` feature to run dressed character render tests");
}
