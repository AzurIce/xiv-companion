#![cfg(feature = "render-test-support")]

//! Installed-game snapshots for the extended model domains (equipment,
//! furniture, minions/mounts). Requires XIV_GAME_DIR to point at the local
//! game directory; every test stays `#[ignore]`d like the other
//! installed-data suites.

#[cfg(feature = "game-data")]
mod installed {
    use physis::resource::SqPackResource;
    use xiv_companion::{WeaponModelLoadRequest, load_weapon_model_from_resource_request};
    use xiv_companion_data::{
        CharaModelKind, CharaModelLoadRequest, CharaModelType, EquipmentModelLoadRequest,
        FurnitureModelKind, FurnitureModelLoadRequest, PackedCharaModelId, WeaponModelData,
        load_chara_model_from_resource, load_equipment_model_from_resource,
        load_furniture_model_from_resource,
    };
    use xiv_companion_render::test_support::{
        WeaponModelSnapshotOptions, render_weapon_model_snapshot_with_options,
    };

    fn game_dir() -> String {
        std::env::var("XIV_GAME_DIR").unwrap_or_else(|_| r"E:\_ff14\game".to_string())
    }

    fn render(name: &str, model: &WeaponModelData) {
        let snapshot = render_weapon_model_snapshot_with_options(
            WeaponModelSnapshotOptions::new(name).with_viewport(640, 640),
            model,
        )
        .expect("render model domain snapshot");
        eprintln!("png: {}", snapshot.png_path.display());
        eprintln!(
            "adapter: {} ({:?})",
            snapshot.adapter_name, snapshot.adapter_backend
        );
    }

    fn mesh_summary(model: &WeaponModelData) -> String {
        let components: std::collections::BTreeSet<u16> = model
            .meshes
            .iter()
            .map(|mesh| xiv_companion_data::weapon_model_mesh_component_index(model, mesh))
            .collect();
        format!(
            "meshes={} components={} materials={}",
            model.meshes.len(),
            components.len(),
            model.materials.len()
        )
    }

    #[test]
    #[ignore = "renders installed gear/accessory snapshots to target/weapon-render-snapshots"]
    fn render_installed_equipment_domain_snapshots() {
        let mut resource = SqPackResource::from_existing(&game_dir());
        // 总冠军制敌套装（ilvl 790，set e0908 variant 2），覆盖 头/身/手/腿/脚。
        let gear = [
            (49_685u32, "总冠军制敌头甲", 3u32),
            (49_686, "总冠军制敌上衣", 4),
            (49_687, "总冠军制敌手套", 5),
            (49_688, "总冠军制敌马裤", 7),
            (49_689, "总冠军制敌矮靴", 8),
        ];
        for (item_id, item_name, slot) in gear {
            let request = EquipmentModelLoadRequest {
                item_id,
                item_name: item_name.to_string(),
                model_main: 131_980,
                model_sub: 0,
                equip_slot_category: slot,
                race_id: 101,
                stain_ids: [0, 0],
            };
            let model = load_equipment_model_from_resource(&mut resource, &request)
                .unwrap_or_else(|error| panic!("load {item_name}: {error}"));
            assert!(!model.meshes.is_empty(), "{item_name} has no meshes");
            eprintln!("{item_name}: {}", mesh_summary(&model));
            render(&format!("installed-equipment-{item_id}"), &model);
        }

        // 种族变体：同一上衣用猫魅族女性模型（c0801）。
        let race_variant = EquipmentModelLoadRequest {
            item_id: 49_686,
            item_name: "总冠军制敌上衣".to_string(),
            model_main: 131_980,
            model_sub: 0,
            equip_slot_category: 4,
            race_id: 801,
            stain_ids: [0, 0],
        };
        let model = load_equipment_model_from_resource(&mut resource, &race_variant)
            .expect("load race-variant body");
        assert!(!model.meshes.is_empty());
        render("installed-equipment-49686-race-c0801", &model);

        // 饰品：总冠军咏咒耳夹（a0182，slot 9）。
        let accessory = EquipmentModelLoadRequest {
            item_id: 49_719,
            item_name: "总冠军咏咒耳夹".to_string(),
            model_main: 65_718,
            model_sub: 0,
            equip_slot_category: 9,
            race_id: 101,
            stain_ids: [0, 0],
        };
        let model = load_equipment_model_from_resource(&mut resource, &accessory)
            .expect("load accessory");
        assert!(!model.meshes.is_empty());
        render("installed-equipment-49719-accessory", &model);
    }

    #[test]
    #[ignore = "renders installed furniture/yard snapshots to target/weapon-render-snapshots"]
    fn render_installed_furniture_domain_snapshots() {
        let mut resource = SqPackResource::from_existing(&game_dir());
        let cases = [
            (6_601u32, "海滨圆桌", FurnitureModelKind::Indoor, 1u16),
            (6_475, "莫古信箱", FurnitureModelKind::Outdoor, 4),
        ];
        for (item_id, item_name, kind, model_key) in cases {
            let request = FurnitureModelLoadRequest {
                item_id,
                item_name: item_name.to_string(),
                kind,
                model_key,
            };
            let model = load_furniture_model_from_resource(&mut resource, &request)
                .unwrap_or_else(|error| panic!("load {item_name}: {error}"));
            assert!(!model.meshes.is_empty(), "{item_name} has no meshes");
            eprintln!("{item_name}: {}", mesh_summary(&model));
            render(&format!("installed-furniture-{item_id}"), &model);
        }
    }

    #[test]
    #[ignore = "renders installed minion/mount snapshots to target/weapon-render-snapshots"]
    fn render_installed_chara_domain_snapshots() {
        let mut resource = SqPackResource::from_existing(&game_dir());
        let cases = [
            (
                6_003u32,
                "爆弹仔",
                CharaModelKind::Minion,
                PackedCharaModelId {
                    model_id: 8_003,
                    base_id: 1,
                    variant_id: 1,
                    chara_type: CharaModelType::Monster,
                },
            ),
            (
                6_002,
                "古菩角笛",
                CharaModelKind::Mount,
                PackedCharaModelId {
                    model_id: 54,
                    base_id: 2,
                    variant_id: 1,
                    chara_type: CharaModelType::Monster,
                },
            ),
            (
                6_001,
                "陆行鸟笛",
                CharaModelKind::Mount,
                PackedCharaModelId {
                    model_id: 1,
                    base_id: 1,
                    variant_id: 1,
                    chara_type: CharaModelType::Demihuman,
                },
            ),
        ];
        for (item_id, item_name, kind, model_id) in cases {
            let request = CharaModelLoadRequest {
                item_id,
                item_name: item_name.to_string(),
                kind,
                model: model_id,
            };
            let model = load_chara_model_from_resource(&mut resource, &request)
                .unwrap_or_else(|error| panic!("load {item_name}: {error}"));
            assert!(!model.meshes.is_empty(), "{item_name} has no meshes");
            eprintln!("{item_name}: {}", mesh_summary(&model));
            render(&format!("installed-chara-{item_id}"), &model);
        }
    }

    #[test]
    #[ignore = "renders a current-patch weapon as regression for the extended domains"]
    fn render_installed_weapon_regression_snapshot() {
        let mut resource = SqPackResource::from_existing(&game_dir());
        let request = WeaponModelLoadRequest {
            item_id: 52_311,
            item_name: "帕拉佐钻石六分仪".to_string(),
            model_main: 8_593_868_853,
            model_sub: 0,
            stain_ids: [0, 0],
        };
        let model = load_weapon_model_from_resource_request(&mut resource, &request)
            .expect("load weapon regression");
        assert!(!model.meshes.is_empty());
        eprintln!("weapon: {}", mesh_summary(&model));
        render("installed-weapon-52311-regression", &model);
    }
}
