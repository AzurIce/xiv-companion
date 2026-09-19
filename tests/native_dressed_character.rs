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
        CharacterPalettePackage, appearance_colors_from_palette,
        character_enabled_attribute_names,
    };
    use xiv_companion_data::{
        CharacterCustomize, DressedCharacterData, DressedCharacterLoadRequest,
        DressedEquipmentPiece, ModelSkeleton, PreparedModelOptions,
        load_dressed_character_with_skeleton_from_resource,
    };
    use xiv_companion_render::test_support::{
        ModelSnapshot, ModelSnapshotOptions, render_model_snapshot_with_skeleton_and_pose,
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
        let (data, skeleton) = load_dressed_character_with_skeleton_from_resource(resource, &request)
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
                        .with_enabled_attribute_names(
                            character_enabled_attribute_names(customize, &data.model),
                        ),
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
        let snapshot =
            render_dressed_front("dressed-au-ra-e0908-set-front", &customize, &data, &skeleton);
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
}

#[cfg(not(feature = "game-data"))]
#[test]
fn dressed_character_render_requires_game_data_feature() {
    panic!("enable the `game-data` feature to run dressed character render tests");
}
