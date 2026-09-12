#![cfg(feature = "game-data")]

//! 角色拼装的本地真实数据验证（需 XIV_GAME_DIR 指向游戏安装目录）。
//!
//! 加载默认捏脸（assets/character-make.json），为各代表（部族, 性别）组装
//! 完整角色并打印 per-part 摘要；探针测试固化编号基数/cmp 布局/共享发分布
//! 的逆向结论（对齐 `chara_assemble`/`character_make` 模块文档）。

use physis::resource::{Resource, SqPackResource};
use xiv_companion::{
    CharacterAssemblyLoadRequest, CharacterCustomize, character_assembly_attribute_options,
    character_part_paths, load_character_assembly_from_resource,
};

fn game_dir() -> String {
    std::env::var("XIV_GAME_DIR").unwrap_or_else(|_| r"E:\_ff14\game".to_string())
}

fn load_make_package() -> xiv_companion::CharacterMakePackage {
    let path =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/character-make.json");
    let bytes = std::fs::read(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
    serde_json::from_slice(&bytes).expect("decode character-make.json")
}

/// 默认捏脸全组加载：10 个代表组覆盖皮肤回退（鲁加女）、发共享特例（硌狮/
/// 共享发）、兔耳（维埃拉）、尾（猫魅/敖龙）、高地借脸、拉拉女借男裸模。
#[test]
#[ignore = "loads default character assemblies from the installed game; requires XIV_GAME_DIR"]
fn load_default_character_assemblies_from_installed_game() {
    let make = load_make_package();
    let mut resource = SqPackResource::from_existing(&game_dir());
    let race_codes = [101u16, 201, 401, 501, 801, 1001, 1201, 1301, 1501, 1801];
    let mut failures = Vec::new();
    for race_code in race_codes {
        let Some(customize) = xiv_companion::default_customize_for_race_code(&make, race_code)
        else {
            failures.push(format!("race code {race_code}: no default customize"));
            continue;
        };
        let request =
            CharacterAssemblyLoadRequest::new(customize, format!("race-code-{race_code}"));
        match load_character_assembly_from_resource(&mut resource, &request) {
            Ok(data) => {
                if race_code == 101 {
                    let out = std::path::PathBuf::from("target/character-assembly-101.json");
                    std::fs::write(&out, serde_json::to_vec(&data).expect("serialize character assembly"))
                        .expect("write character assembly export");
                    println!("wrote {}", out.display());
                }
                let mut parts: Vec<String> = Vec::new();
                for path in &data.loaded_paths {
                    if path.ends_with(".mdl") {
                        let obj = path.split('/').nth(3).unwrap_or("?");
                        parts.push(format!("{obj}:{}", path.rsplit('/').next().unwrap_or("?")));
                    }
                }
                println!(
                    "RACE {race_code}: meshes={} materials={} textures={} diagnostics={} parts=[{}]",
                    data.meshes.len(),
                    data.materials.len(),
                    data.textures.len(),
                    data.load_diagnostics.len(),
                    parts.join(", ")
                );
                if data.meshes.is_empty() || data.materials.is_empty() {
                    failures.push(format!("race code {race_code}: empty meshes/materials"));
                }
            }
            Err(error) => failures.push(format!("race code {race_code}: {error:#}")),
        }
    }
    assert!(
        failures.is_empty(),
        "assembly failures:\n{}",
        failures.join("\n")
    );
}

#[test]
#[ignore = "loads the user's Au Ra preset from FFXIV_CHARA_02.dat"]
fn export_user_au_ra_preset() {
    let make = load_make_package();
    let root = std::path::PathBuf::from(std::env::var("XIV_CONFIG_DIR").unwrap());
    let bytes = std::fs::read(root.join("FFXIV_CHARA_02.dat")).expect("read preset");
    let mut customize = CharacterCustomize::from_bytes(&bytes[16..42]).expect("decode customize");
    customize.head = customize.head.saturating_sub(1);
    assert_eq!(customize.race_code(), 1401);
    let request = CharacterAssemblyLoadRequest::new(customize, "user-au-ra");
    let mut resource = SqPackResource::from_existing(&game_dir());
    let data = load_character_assembly_from_resource(&mut resource, &request).expect("load");
    std::fs::write("target/character-assembly-user.json", serde_json::to_vec(&data).unwrap()).unwrap();
}

/// 共享发装配：维埃拉女换 h0116（模型回退中原男根 c0101，材质按模型路径
/// 种族走共享表）。
#[test]
#[ignore = "loads a shared-hair assembly from the installed game; requires XIV_GAME_DIR"]
fn load_shared_hair_character_assembly_from_installed_game() {
    let make = load_make_package();
    let mut resource = SqPackResource::from_existing(&game_dir());
    let mut customize =
        xiv_companion::default_customize_for_race_code(&make, 1801).expect("viera female default");
    customize.hair = 116;
    let request = CharacterAssemblyLoadRequest::new(customize, "viera-shared-hair-116");
    let data = load_character_assembly_from_resource(&mut resource, &request)
        .expect("load shared-hair assembly");
    assert!(!data.meshes.is_empty() && !data.materials.is_empty());
    let hair_material = data
        .materials
        .iter()
        .find(|material| {
            material
                .path
                .as_deref()
                .is_some_and(|p| p.contains("h0116"))
        })
        .expect("hair material");
    println!("shared hair material path: {:?}", hair_material.path);
    assert!(
        hair_material
            .path
            .as_deref()
            .unwrap_or_default()
            .starts_with("chara/human/c0101/obj/hair/h0116/")
    );
}

/// 脸部 attribute 变体选项（眉/眼/鼻/嘴/轮廓特征件 submesh 位）。
#[test]
#[ignore = "lists face attribute options from the installed game; requires XIV_GAME_DIR"]
fn face_attribute_options_from_installed_game() {
    let make = load_make_package();
    let mut resource = SqPackResource::from_existing(&game_dir());
    let customize = xiv_companion::default_customize_for_race_code(&make, 101).expect("hyur male");
    let request = CharacterAssemblyLoadRequest::new(customize, "race-code-101");
    let data = load_character_assembly_from_resource(&mut resource, &request)
        .expect("load hyur male assembly");
    let options = character_assembly_attribute_options(&data);
    println!("attribute options ({}):", options.len());
    for option in &options {
        println!("  bit {:#04x}: {}", option.bit, option.names.join(", "));
    }
    assert!(
        !options.is_empty(),
        "hyur male face has attribute submeshes"
    );
}

/// 编号基数/cmp 布局/裸装部件探针：固化以下结论（详见 `chara_assemble`
/// 模块文档）：
/// - 脸/尾/兔耳文件 1 基而字节 0 基（f0000/t0000/z0000 不存在）；
/// - 发型 1 基直连（h0000 不存在），InitVal 是 CharaMakeCustomize 块序的
///   0 基选项索引（中原男 index 51 → h0148）；
/// - 裸装身体 = 小衣 e0001（top/dwn 全族恒有；sho 仅中原男/女、鲁加男、
///   拉拉男）+ 裸肤 e0000（全槽仅中原男/女、拉拉男有文件）；硌狮/维埃拉
///   无自身 e0001 文件；
/// - human.cmp = 182272 字节色板区 + 4480 字节尾部（32 × 140 字节 RGSP
///   缩放记录）。
#[test]
#[ignore = "probes installed game data; requires XIV_GAME_DIR"]
fn probe_character_file_numbering_and_cmp_layout() {
    let mut resource = SqPackResource::from_existing(&game_dir());
    let paths = [
        (
            "face 0-based",
            "chara/human/c0101/obj/face/f0000/model/c0101f0000_fac.mdl",
            false,
        ),
        (
            "face 1-based",
            "chara/human/c0101/obj/face/f0004/model/c0101f0004_fac.mdl",
            true,
        ),
        (
            "tail 0-based",
            "chara/human/c1301/obj/tail/t0000/model/c1301t0000_til.mdl",
            false,
        ),
        (
            "tail 1-based",
            "chara/human/c1301/obj/tail/t0001/model/c1301t0001_til.mdl",
            true,
        ),
        (
            "zear 1-based",
            "chara/human/c1701/obj/zear/z0004/model/c1701z0004_zer.mdl",
            true,
        ),
        (
            "hair 1-based",
            "chara/human/c0101/obj/hair/h0051/model/c0101h0051_hir.mdl",
            true,
        ),
        (
            "hair 0-based",
            "chara/human/c0101/obj/hair/h0000/model/c0101h0000_hir.mdl",
            false,
        ),
        (
            "default hair opt51",
            "chara/human/c0101/obj/hair/h0148/model/c0101h0148_hir.mdl",
            true,
        ),
        (
            "miqo fem default",
            "chara/human/c0801/obj/hair/h0168/model/c0801h0168_hir.mdl",
            true,
        ),
        (
            "highlander face",
            "chara/human/c0401/obj/face/f0001/model/c0401f0001_fac.mdl",
            false,
        ),
        (
            "midlander face",
            "chara/human/c0201/obj/face/f0001/model/c0201f0001_fac.mdl",
            true,
        ),
        (
            "lala fem smallclothes",
            "chara/equipment/e0001/model/c1201e0001_top.mdl",
            true,
        ),
        (
            "hrothgar smallclothes",
            "chara/equipment/e0001/model/c1501e0001_top.mdl",
            false,
        ),
        (
            "midlander fem bare hands",
            "chara/equipment/e0000/model/c0201e0000_glv.mdl",
            true,
        ),
        (
            "miqote fem bare hands",
            "chara/equipment/e0000/model/c0801e0000_glv.mdl",
            false,
        ),
        (
            "midlander male smallclothes shoes",
            "chara/equipment/e0001/model/c0101e0001_sho.mdl",
            true,
        ),
        (
            "au-ra fem smallclothes shoes",
            "chara/equipment/e0001/model/c1401e0001_sho.mdl",
            false,
        ),
        (
            "skin material",
            "chara/human/c0101/obj/body/b0001/material/v0001/mt_c0101b0001_a.mtrl",
            true,
        ),
        (
            "face material",
            "chara/human/c0101/obj/face/f0004/material/mt_c0101f0004_fac_a.mtrl",
            true,
        ),
        (
            "shared hair model",
            "chara/human/c0101/obj/hair/h0116/model/c0101h0116_hir.mdl",
            true,
        ),
        (
            "shared hair own",
            "chara/human/c0201/obj/hair/h0116/model/c0201h0116_hir.mdl",
            false,
        ),
    ];
    for (label, path, expected) in paths {
        let exists = resource.read(path).is_some();
        println!("EXISTS {exists} (expected {expected}) {label} = {path}");
        assert_eq!(exists, expected, "{label}: {path}");
    }

    let cmp = resource
        .read("chara/xls/charamake/human.cmp")
        .expect("read human.cmp");
    println!(
        "CMP bytes = {} (colors {} + {} tail)",
        cmp.len(),
        cmp.len() / 4,
        cmp.len() - 182272
    );
    assert_eq!(
        cmp.len(),
        186752,
        "human.cmp = 182272 palette + 4480 RGSP tail"
    );
    // 尾部按 32 条 × 140 字节解析为 35 个 float 的记录（RGSP 缩放参数）。
    let tail_float = |record: usize, index: usize| -> f32 {
        let at = 182272 + record * 140 + index * 4;
        f32::from_le_bytes([cmp[at], cmp[at + 1], cmp[at + 2], cmp[at + 3]])
    };
    println!(
        "CMP rgsp record0 floats: {:?}",
        (0..4).map(|i| tail_float(0, i)).collect::<Vec<_>>()
    );
    // 色板锚点：眼色块 0 首项、肤色 tribe1 男块 (18+3)*256 首项。
    let color = |index: usize| {
        [
            cmp[index * 4],
            cmp[index * 4 + 1],
            cmp[index * 4 + 2],
            cmp[index * 4 + 3],
        ]
    };
    println!("CMP eye[0] = {:?}", color(0));
    println!("CMP skin tribe1 male [0] = {:?}", color((18 + 3) * 256));
    assert_eq!(color(0)[3], 255, "eye colors are opaque");
}

/// 部件 attribute 名表探针：按 MDL 分组打印每个 submesh 的掩码与其 zip 对应
/// 的 attribute 名（名随掩码置位升序 zip 对应；位是 MDL 本地表序，跨模型
/// 数值不可比）。
#[test]
#[ignore = "dumps per-part attribute tables from the installed game; requires XIV_GAME_DIR"]
fn dump_part_attribute_tables_from_installed_game() {
    let make = load_make_package();
    let mut resource = SqPackResource::from_existing(&game_dir());
    for race_code in [101u16, 801, 1401] {
        let customize = xiv_companion::default_customize_for_race_code(&make, race_code)
            .expect("default customize");
        let request = CharacterAssemblyLoadRequest::new(customize, format!("race-code-{race_code}"));
        let data = load_character_assembly_from_resource(&mut resource, &request)
            .expect("load assembly");
        println!("== race code {race_code} ==");
        let mut last_path = String::new();
        for mesh in &data.meshes {
            if mesh.path != last_path {
                last_path = mesh.path.clone();
                println!("  MDL {last_path}");
            }
            if let Some(submesh) = &mesh.submesh {
                println!(
                    "    mesh mat={} mask={} names(zip)={:?}",
                    mesh.material_name,
                    submesh.attribute_index_mask_hex,
                    submesh.attribute_names,
                );
            } else {
                println!("    mesh mat={} (no submesh info)", mesh.material_name);
            }
        }
    }
}

/// 尾巴/头皮纹理探针：导出敖龙女尾巴 base 与头发 mask 头皮区 PNG。
#[test]
#[ignore = "dumps tail/scalp textures from the installed game; requires XIV_GAME_DIR"]
fn dump_tail_and_scalp_textures_from_installed_game() {
    let make = load_make_package();
    let mut resource = SqPackResource::from_existing(&game_dir());
    let customize = xiv_companion::default_customize_for_race_code(&make, 1401).unwrap();
    let request = CharacterAssemblyLoadRequest::new(customize, "au-ra-female");
    let data = load_character_assembly_from_resource(&mut resource, &request).expect("load");
    let out_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target/tmp/texture-probe");
    std::fs::create_dir_all(&out_dir).unwrap();
    for texture in &data.textures {
        if !(texture.path.contains("t0001") || texture.path.contains("h0001_hir")) {
            continue;
        }
        let name = texture.path.replace('/', "_");
        let layer_len = (usize::from(texture.width) * usize::from(texture.height)) * 4;
        let rgba = if texture.rgba.len() >= layer_len {
            &texture.rgba[..layer_len]
        } else {
            &texture.rgba
        };
        let path = out_dir.join(format!("{name}.png"));
        image::save_buffer_with_format(
            &path,
            rgba,
            u32::from(texture.width),
            u32::from(texture.height),
            image::ColorType::Rgba8,
            image::ImageFormat::Png,
        )
        .unwrap_or_else(|error| panic!("write {path:?}: {error}"));
        println!("png: {}", path.display());
    }
}

/// 尾巴网格几何探针：敖龙女尾巴 mesh 的绑定姿势边界/蒙皮数据。
#[test]
#[ignore = "dumps tail mesh geometry from the installed game; requires XIV_GAME_DIR"]
fn dump_tail_mesh_geometry_from_installed_game() {
    let make = load_make_package();
    let mut resource = SqPackResource::from_existing(&game_dir());
    let customize = xiv_companion::default_customize_for_race_code(&make, 1401).unwrap();
    let request = CharacterAssemblyLoadRequest::new(customize, "au-ra-female");
    let data = load_character_assembly_from_resource(&mut resource, &request).expect("load");
    for (index, mesh) in data.meshes.iter().enumerate() {
        if !mesh.path.contains("tail") {
            continue;
        }
        let mut min = [f32::INFINITY; 3];
        let mut max = [f32::NEG_INFINITY; 3];
        for vertex in &mesh.vertices {
            for axis in 0..3 {
                min[axis] = min[axis].min(vertex.position[axis]);
                max[axis] = max[axis].max(vertex.position[axis]);
            }
        }
        let skinned = mesh
            .vertices
            .iter()
            .filter(|v| v.blend_weights.is_some() && v.blend_indices.is_some())
            .count();
        println!(
            "mesh[{index}] {} verts={} skinned={} min={:?} max={:?} bone_table={:?}",
            mesh.path,
            mesh.vertices.len(),
            skinned,
            min,
            max,
            mesh.bone_table.as_ref().map(|t| t.bone_names.iter().flatten().take(8).cloned().collect::<Vec<_>>()),
        );
        // 顶点 y/z 分布直方（看是否成段分离）。
        let mut histogram = [0usize; 10];
        for vertex in &mesh.vertices {
            let bucket = ((vertex.position[1] * 10.0) as i32).clamp(0, 9) as usize;
            histogram[bucket] += 1;
        }
        println!("    y-histogram(0.1): {histogram:?}");
    }
}

/// 发型子网格构成探针：h0001（敖龙）与 h0148（中原男）各 submesh 顶点数/边界。
#[test]
#[ignore = "dumps hair submesh composition from the installed game; requires XIV_GAME_DIR"]
fn dump_hair_submesh_composition_from_installed_game() {
    let make = load_make_package();
    let mut resource = SqPackResource::from_existing(&game_dir());
    for race_code in [1401u16, 101] {
        let customize = xiv_companion::default_customize_for_race_code(&make, race_code).unwrap();
        let request = CharacterAssemblyLoadRequest::new(customize, format!("race-{race_code}"));
        let data = load_character_assembly_from_resource(&mut resource, &request).expect("load");
        println!("== race {race_code} hair ==");
        for mesh in &data.meshes {
            if !mesh.path.contains("_hir.mdl") {
                continue;
            }
            let mut min = [f32::INFINITY; 3];
            let mut max = [f32::NEG_INFINITY; 3];
            for vertex in &mesh.vertices {
                for axis in 0..3 {
                    min[axis] = min[axis].min(vertex.position[axis]);
                    max[axis] = max[axis].max(vertex.position[axis]);
                }
            }
            let (mask, names) = mesh
                .submesh
                .as_ref()
                .map(|s| (s.attribute_index_mask_hex.clone(), s.attribute_names.clone()))
                .unwrap_or_default();
            println!(
                "  {} verts={} mask={} names={:?} mat={} min={:?} max={:?}",
                mesh.path.rsplit('/').next().unwrap_or(&mesh.path),
                mesh.vertices.len(),
                mask,
                names,
                mesh.material_name,
                min,
                max,
            );
        }
    }
}

/// 手部骨变形守护：敖龙女装配中 e0000 glv（中原女回退件）经 `RaceDeform`
/// 烘焙后位置应向敖龙骨架对齐（相对中原原始位置下移/内收），手腕与敖龙小臂
/// 衔接（races 骨变形烘焙的端到端实证）。
#[test]
#[ignore = "asserts baked hand deform from the installed game; requires XIV_GAME_DIR"]
fn baked_hand_mesh_deforms_to_au_ra_wrist() {
    let make = load_make_package();
    let mut resource = SqPackResource::from_existing(&game_dir());
    let customize = xiv_companion::default_customize_for_race_code(&make, 1401).unwrap();
    let request = CharacterAssemblyLoadRequest::new(customize, "au-ra-female");
    let data = load_character_assembly_from_resource(&mut resource, &request).expect("load");
    let mesh = data
        .meshes
        .iter()
        .find(|mesh| mesh.path.contains("e0000_glv"))
        .expect("baked glv in assembly");
    let bounds = |mesh: &xiv_companion_data::ModelMesh| {
        let mut min = [f32::INFINITY; 3];
        let mut max = [f32::NEG_INFINITY; 3];
        for vertex in &mesh.vertices {
            for axis in 0..3 {
                min[axis] = min[axis].min(vertex.position[axis]);
                max[axis] = max[axis].max(vertex.position[axis]);
            }
        }
        (min, max)
    };
    let (_baked_min, baked_max) = bounds(mesh);
    // 未烘焙参照：中原女装配（本族文件不触发骨变形）中的同名 glv 网格。
    let midlander_customize =
        xiv_companion::default_customize_for_race_code(&make, 201).expect("midlander default");
    let midlander = load_character_assembly_from_resource(
        &mut resource,
        &CharacterAssemblyLoadRequest::new(midlander_customize, "midlander-female"),
    )
    .expect("midlander assembly");
    let raw_mesh = midlander
        .meshes
        .iter()
        .find(|mesh| mesh.path.contains("e0000_glv"))
        .expect("midlander glv");
    let (_raw_min, raw_max) = bounds(raw_mesh);
    // 骨变形生效：中原手（y≈0.886..1.027）烘焙到敖龙骨架后明显下移/内收。
    assert!(
        baked_max[1] < raw_max[1] - 0.01,
        "baked hand should move down toward au-ra wrist (baked {baked_max:?} raw {raw_max:?})"
    );
    assert!(
        baked_max[0].abs() < raw_max[0].abs(),
        "baked hand should pull inward (baked {baked_max:?} raw {raw_max:?})"
    );
    // 与敖龙小臂腕部衔接：手覆盖到 y≈0.99 附近（敖龙小臂腕端 ~0.97）。
    assert!(
        (baked_max[1] - 0.99).abs() < 0.03,
        "baked hand wrist should meet au-ra forearm (baked {baked_max:?})"
    );
}

/// 同步/异步加载等价性：敖龙女装配经两条路径加载，网格与纹理数据必须一致
/// （app 只走异步路径，native 快照走同步——分叉排查守护）。
#[test]
#[ignore = "compares sync vs async assembly loads; requires XIV_GAME_DIR"]
fn sync_and_async_assembly_loads_are_identical() {
    use futures_executor::block_on;
    use physis::resource::Resource;
    use xiv_companion_data::{AsyncGameResource, load_character_assembly_with_skeleton_from_async_resource};

    struct SyncAsAsync(SqPackResource);
    impl AsyncGameResource for SyncAsAsync {
        type Error = String;
        type ReadFuture<'a> = std::future::Ready<Result<Vec<u8>, String>>;
        fn read<'a>(&'a mut self, path: &'a str) -> Self::ReadFuture<'a> {
            std::future::ready(
                self.0
                    .read(path)
                    .ok_or_else(|| format!("missing {path}")),
            )
        }
        fn platform(&self) -> physis::Platform {
            self.0.platform()
        }
    }

    let make = load_make_package();
    let customize = xiv_companion::default_customize_for_race_code(&make, 1401).unwrap();
    let request = CharacterAssemblyLoadRequest::new(customize, "au-ra-female");
    let (sync_data, sync_skeleton) = {
        let mut resource = SqPackResource::from_existing(&game_dir());
        xiv_companion::load_character_assembly_with_skeleton_from_resource(&mut resource, &request)
            .expect("sync load")
    };
    let (async_data, async_skeleton) = block_on(async {
        let mut resource = SyncAsAsync(SqPackResource::from_existing(&game_dir()));
        load_character_assembly_with_skeleton_from_async_resource(&mut resource, &request).await
    })
    .expect("async load");

    assert_eq!(sync_data.meshes.len(), async_data.meshes.len(), "mesh count");
    assert_eq!(sync_data.materials.len(), async_data.materials.len(), "materials");
    assert_eq!(sync_data.textures.len(), async_data.textures.len(), "textures");
    assert_eq!(sync_skeleton.is_some(), async_skeleton.is_some(), "skeleton");
    for (index, (a, b)) in sync_data.meshes.iter().zip(&async_data.meshes).enumerate() {
        assert_eq!(a.path, b.path, "mesh[{index}] path");
        assert_eq!(a.vertices.len(), b.vertices.len(), "mesh[{index}] verts");
        assert_eq!(a.indices, b.indices, "mesh[{index}] indices");
    }
    for (index, (a, b)) in sync_data.textures.iter().zip(&async_data.textures).enumerate() {
        assert_eq!(a.path, b.path, "texture[{index}] path");
        assert_eq!(a.rgba, b.rgba, "texture[{index}] rgba");
    }
    // 关键抽查：头发法线 alpha 与尾巴 base 的前若干字节（异步解码链一致性）。
    let hair = sync_data
        .textures
        .iter()
        .position(|t| t.path.contains("h0001_hir_norm"))
        .expect("hair normal");
    assert_eq!(
        &sync_data.textures[hair].rgba[..256],
        &async_data.textures[hair].rgba[..256],
        "hair normal alpha decode"
    );
}

/// 组件核对：CharacterCustomize 从默认表往返 26 字节。
#[test]
fn default_customize_bytes_round_trip() {
    let make = load_make_package();
    let customize =
        xiv_companion::default_customize_for_race_code(&make, 101).expect("hyur male default");
    let bytes = customize.to_bytes();
    assert_eq!(bytes.len(), 26);
    assert_eq!(CharacterCustomize::from_bytes(&bytes).unwrap(), customize);
}

/// 全部 18 个 c 编码的真实发型文件集合（h0001..=h0220），一次探测进程级
/// 缓存；32 组默认发型 id 必须落在对应集合内（CharaMakeCustomize 槽映射
/// 的 ground truth 守护）。
fn all_race_hair_files() -> &'static Vec<std::collections::HashSet<u16>> {
    static HAIR_FILES: std::sync::OnceLock<Vec<std::collections::HashSet<u16>>> =
        std::sync::OnceLock::new();
    HAIR_FILES.get_or_init(|| {
        let mut resource = SqPackResource::from_existing(&game_dir());
        xiv_companion::CHARA_MAKE_CUSTOMIZE_SLOT_RACE_CODES
            .iter()
            .map(|race_code| {
                let race = format!("c{race_code:04}");
                let mut found = std::collections::HashSet::new();
                for hair in 1..=220u16 {
                    let path = format!(
                        "chara/human/{race}/obj/hair/h{hair:04}/model/{race}h{hair:04}_hir.mdl"
                    );
                    if resource.read(&path).is_some() {
                        found.insert(hair);
                    }
                }
                found
            })
            .collect()
    })
}

/// 32 组默认装配全量守护：每组默认捏脸完整加载（meshes/materials 非空、
/// 诊断无 ERROR 级），默认发型 id ∈ 该族真实发型文件集合，脸部件候选
/// （自身或借脸）存在。
#[test]
#[cfg(feature = "game-data")]
#[ignore = "loads all 32 default character assemblies from the installed game; requires XIV_GAME_DIR"]
fn load_all_32_default_character_assemblies_from_installed_game() {
    let make = load_make_package();
    assert_eq!(
        make.groups.len(),
        32,
        "character-make.json should have 32 groups"
    );
    let mut resource = SqPackResource::from_existing(&game_dir());
    let hair_files = all_race_hair_files();
    let mut failures = Vec::new();
    for group in &make.groups {
        let race_code = group.race_code;
        let label = format!(
            "key{} c{} R{}T{}G{}",
            group.key, race_code, group.race, group.tribe, group.gender
        );
        let customize = group.default_customize;
        let race_files =
            &hair_files[xiv_companion::chara_make_customize_slot_index(race_code).unwrap()];
        // 发型守护：默认发型 id 必须落在真实文件集合内。选项列表整体不做
        // 子集断言——数据本身含少量无对应文件的菜单项（如拉拉男/鲁加女的
        // 51-55、猫魅的 13/104），游戏内由 UI 禁用或跨族共享消化。
        if !race_files.contains(&u16::from(customize.hair)) {
            failures.push(format!(
                "{label}: default hair h{:04} not in race files",
                customize.hair
            ));
        }
        // 脸部件候选存在（自身或借脸）。
        let face_part = character_part_paths(&customize)
            .into_iter()
            .find(|part| part.kind == xiv_companion::CharacterPartKind::Face)
            .expect("face part");
        let face_exists = std::iter::once(&face_part.model_path)
            .chain(face_part.alternate_model_paths.iter())
            .any(|path| resource.read(path).is_some());
        if !face_exists {
            failures.push(format!("{label}: no face model candidate exists"));
        }
        // 完整装配。
        let request = CharacterAssemblyLoadRequest::new(customize, label.clone());
        match load_character_assembly_from_resource(&mut resource, &request) {
            Ok(data) => {
                let error_diagnostics = data
                    .load_diagnostics
                    .iter()
                    .flat_map(|diagnostic| diagnostic.candidates.iter())
                    .filter(|candidate| {
                        candidate.status != xiv_companion::WeaponModelLoadCandidateStatus::Missing
                    })
                    .count();
                let face_file = data
                    .loaded_paths
                    .iter()
                    .find(|path| path.contains("_fac.mdl"))
                    .and_then(|path| path.rsplit('/').next().map(str::to_string))
                    .unwrap_or_else(|| "f????".to_string());
                println!(
                    "GROUP {label}: meshes={} materials={} textures={} diagnostics={} hair=h{:04} face={}",
                    data.meshes.len(),
                    data.materials.len(),
                    data.textures.len(),
                    data.load_diagnostics.len(),
                    customize.hair,
                    face_file.rsplit(".mdl").next().unwrap_or(&face_file),
                );
                if data.meshes.is_empty() || data.materials.is_empty() {
                    failures.push(format!("{label}: empty meshes/materials"));
                }
                if error_diagnostics != 0 {
                    failures.push(format!(
                        "{label}: {error_diagnostics} ERROR-level diagnostics"
                    ));
                }
            }
            Err(error) => failures.push(format!("{label}: {error:#}")),
        }
    }
    assert!(
        failures.is_empty(),
        "assembly failures:\n{}",
        failures.join("\n")
    );
}
