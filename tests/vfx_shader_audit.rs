#![cfg(feature = "game-data")]

use std::{collections::BTreeMap, fs, path::PathBuf, process::Command};

use binrw::BinRead;
use physis::{
    resource::{Resource, SqPackResource},
    shpk::{Key, ResourceParameter, ResourceParameterBinReadArgs, Shader, ShaderBinReadArgs},
};
use serde_json::{Value, json};

const MODEL_SKIN_PATHS: [&str; 7] = [
    "chara/weapon/w5101/obj/body/b0001/vfx/eff/vw0025.avfx",
    "chara/weapon/w5741/obj/body/b0003/vfx/eff/vw0002.avfx",
    "chara/weapon/w2901/obj/body/b0031/vfx/eff/vw0002.avfx",
    "chara/weapon/w0527/obj/body/b0004/vfx/eff/vw0002.avfx",
    "chara/weapon/w2601/obj/body/b0083/vfx/eff/vw0001.avfx",
    "chara/weapon/w0601/obj/body/b0063/vfx/eff/vw0001.avfx",
    "chara/weapon/w2651/obj/body/b0083/vfx/eff/vw0001.avfx",
];

#[test]
#[ignore = "checks installed ModelSkin UV references; requires XIV_GAME_DIR"]
fn audit_installed_model_skin_uv_references() {
    let raw_dir = PathBuf::from(std::env::var_os("XIV_GAME_DIR").expect("XIV_GAME_DIR"));
    let game_dir = xiv_companion::game_data::normalize_game_dir(&raw_dir).expect("game directory");
    let mut resource = SqPackResource::from_existing(game_dir.to_str().expect("UTF-8 game path"));
    let mut definitions = 0;
    let mut enabled_layers = 0;
    for path in MODEL_SKIN_PATHS {
        let bytes = resource.read(path).unwrap_or_else(|| panic!("read {path}"));
        let file = xiv_companion::AvfxFile::parse(&bytes)
            .unwrap_or_else(|error| panic!("parse {path}: {error}"));
        for (particle_index, particle) in file.particles.iter().enumerate() {
            if !matches!(particle.data, xiv_companion::AvfxParticleData::ModelSkin(_)) {
                continue;
            }
            definitions += 1;
            assert!(
                particle.uv_sets.len() <= 4,
                "{path} Ptcl[{particle_index}]: UvSt count={}",
                particle.uv_sets.len(),
            );
            let indexes = [
                particle
                    .texture_color2
                    .as_ref()
                    .filter(|layer| layer.enabled)
                    .map(|layer| layer.uv_set_index),
                particle
                    .texture_color3
                    .as_ref()
                    .filter(|layer| layer.enabled)
                    .map(|layer| layer.uv_set_index),
                particle
                    .texture_distortion
                    .as_ref()
                    .filter(|layer| layer.enabled)
                    .map(|layer| layer.uv_set_index),
            ];
            for (layer, index) in indexes.into_iter().enumerate() {
                let Some(index) = index else { continue };
                enabled_layers += 1;
                let slot = (index & 7) as usize;
                assert!(
                    slot < particle.uv_sets.len() && slot < 4,
                    "{path} Ptcl[{particle_index}] layer {layer}: UvSN={index}, UvSt count={}",
                    particle.uv_sets.len(),
                );
            }
        }
    }
    assert_eq!(definitions, 8);
    assert_eq!(enabled_layers, 15);
}

#[test]
#[ignore = "checks installed ModelSkin staged lifetimes; requires XIV_GAME_DIR"]
fn audit_installed_model_skin_staged_lifetimes() {
    let raw_dir = PathBuf::from(std::env::var_os("XIV_GAME_DIR").expect("XIV_GAME_DIR"));
    let game_dir = xiv_companion::game_data::normalize_game_dir(&raw_dir).expect("game directory");
    let mut resource = SqPackResource::from_existing(game_dir.to_str().expect("UTF-8 game path"));
    let mut observed = 0_usize;
    let mut draw_age_differences = 0_usize;
    for path in MODEL_SKIN_PATHS {
        let bytes = resource.read(path).unwrap_or_else(|| panic!("read {path}"));
        let file = xiv_companion::AvfxFile::parse(&bytes)
            .unwrap_or_else(|error| panic!("parse {path}: {error}"));
        let indexes = file
            .particles
            .iter()
            .enumerate()
            .filter_map(|(index, particle)| {
                matches!(particle.data, xiv_companion::AvfxParticleData::ModelSkin(_))
                    .then_some(index)
            })
            .collect::<Vec<_>>();
        for &index in &indexes {
            let xiv_companion::AvfxParticleData::ModelSkin(data) = &file.particles[index].data
            else {
                unreachable!()
            };
            assert_eq!(data.fresnel_type & 3, 1, "{path} Ptcl[{index}]");
            assert!(
                data.color_begin.random.iter().all(Option::is_none)
                    && data.color_end.random.iter().all(Option::is_none),
                "{path} Ptcl[{index}] has random Aura color curves"
            );
        }
        let mut playback = xiv_companion::VfxPlayback::new_with_model_skin(
            xiv_companion::VfxRuntime::new(&file),
            &indexes,
        );
        assert!(
            playback.fallback_reason().is_none(),
            "{path}: {:?}",
            playback.fallback_reason()
        );
        let mut peak = 0_usize;
        for update in 0..=300 {
            if update > 0 {
                playback
                    .advance([1.0 / 30.0, 1.0 / 60.0, 2.0 / 30.0][update % 3])
                    .expect("finite Framework input");
            }
            assert!(
                playback.fallback_reason().is_none(),
                "{path} update {update}: {:?}",
                playback.fallback_reason()
            );
            let active = playback.model_skin_instances();
            assert!(active.iter().all(|instance| {
                indexes.contains(&instance.particle_index)
                    && instance.age.is_finite()
                    && instance.total_age.is_finite()
                    && instance.curve_age.is_finite()
                    && instance.curve_total_age.is_finite()
                    && instance.color.iter().all(|channel| channel.is_finite())
                    && instance
                        .aura_color_begin
                        .iter()
                        .all(|channel| channel.is_finite())
                    && instance
                        .aura_color_end
                        .iter()
                        .all(|channel| channel.is_finite())
                    && instance
                        .aura_fresnel
                        .iter()
                        .all(|channel| channel.is_finite())
                    && instance.aura_fresnel_selector.is_finite()
                    && instance.aura_sem.is_finite()
                    && instance.aura_eem.is_finite()
                    && instance.aura_distortion_power.is_finite()
                    && instance
                        .aura_texture_uv
                        .iter()
                        .flatten()
                        .flatten()
                        .all(|value| value.is_finite())
                    && instance
                        .aura_color_combine
                        .iter()
                        .all(|value| value.is_finite())
                    && instance
                        .aura_material_combine
                        .iter()
                        .all(|value| value.is_finite())
                    && instance
                        .aura_combine_extras
                        .iter()
                        .all(|value| value.is_finite())
                    && instance.aura_tc2_alpha_enabled.is_finite()
                    && instance.aura_tc2_color_enabled.is_finite()
                    && instance
                        .aura_uv_point_density
                        .iter()
                        .all(|channel| channel.is_finite())
                    && instance.aura_color_mix.is_finite()
            }));
            for instance in &active {
                let xiv_companion::AvfxParticleData::ModelSkin(data) =
                    &file.particles[instance.particle_index].data
                else {
                    unreachable!()
                };
                for (name, curve, random, actual) in [
                    ("SEM", &data.sem, &data.sem_random, instance.aura_sem),
                    ("EEM", &data.eem, &data.eem_random, instance.aura_eem),
                ] {
                    if !random.keys.is_empty() {
                        continue;
                    }
                    let expected = curve.value_at(instance.age, instance.total_age, 0.0);
                    let cached = curve.value_at(instance.curve_age, instance.curve_total_age, 0.0);
                    assert!(
                        (actual - expected).abs() < 1e-5,
                        "{path} update {update} {name}: {actual} != {expected}"
                    );
                    draw_age_differences += usize::from((expected - cached).abs() > 1e-5);
                }
            }
            peak = peak.max(active.len());
            observed += active.len();
        }
        assert!(peak > 0, "{path} produced no live ModelSkin instance");
        eprintln!("{path}: peak={peak}");
    }
    eprintln!("ModelSkin draw-age values differing from cached-age values: {draw_age_differences}");
    assert!(
        draw_age_differences > 0,
        "installed curves must distinguish current from cached age"
    );
    assert!(
        observed > 0,
        "installed ModelSkin produced no live instances"
    );
}

// Meddle.Formats/Files/ShpkFile.cs: 0x0E01 adds a u32 after the three shader counts.
// Physis does not read that field yet; reuse its per-shader and resource readers.
#[binrw::binread]
#[br(little, magic = b"ShPk")]
struct AuditShaderPackage {
    #[br(assert(version == 0x0D01 || version == 0x0E01))]
    version: u32,
    #[br(temp, assert(format == *b"DX11"))]
    format: [u8; 4],
    #[br(temp)]
    file_length: u32,
    #[br(temp)]
    shader_data_offset: u32,
    #[br(temp)]
    strings_offset: u32,
    #[br(temp)]
    vertex_count: u32,
    #[br(temp)]
    pixel_count: u32,
    #[br(temp)]
    material_size: u32,
    #[br(temp)]
    material_count: u16,
    #[br(temp)]
    has_material_defaults: u16,
    #[br(temp)]
    constant_count: u32,
    #[br(temp)]
    sampler_count: u16,
    #[br(temp)]
    texture_count: u16,
    #[br(temp)]
    uav_count: u32,
    #[br(temp)]
    system_count: u32,
    #[br(temp)]
    scene_count: u32,
    #[br(temp)]
    material_key_count: u32,
    #[br(temp)]
    node_count: u32,
    #[br(temp)]
    alias_count: u32,
    #[br(temp, assert(extra_shader_counts == [0; 3]))]
    extra_shader_counts: [u32; 3],
    #[br(if(version >= 0x0E01))]
    extra_header: Option<u32>,
    #[br(args { count: vertex_count as usize, inner: ShaderBinReadArgs::builder().version(version).shader_data_offset(shader_data_offset).strings_offset(strings_offset).is_vertex(true).finalize() })]
    vertex_shaders: Vec<Shader>,
    #[br(args { count: pixel_count as usize, inner: ShaderBinReadArgs::builder().version(version).shader_data_offset(shader_data_offset).strings_offset(strings_offset).is_vertex(false).finalize() })]
    pixel_shaders: Vec<Shader>,
    #[br(pad_before = material_count as u64 * 8 + if has_material_defaults != 0 { material_size as u64 } else { 0 })]
    #[br(args { count: constant_count as usize, inner: ResourceParameterBinReadArgs::builder().strings_offset(strings_offset).finalize() })]
    scalar_parameters: Vec<ResourceParameter>,
    #[br(args { count: sampler_count as usize, inner: ResourceParameterBinReadArgs::builder().strings_offset(strings_offset).finalize() })]
    sampler_parameters: Vec<ResourceParameter>,
    #[br(args { count: texture_count as usize, inner: ResourceParameterBinReadArgs::builder().strings_offset(strings_offset).finalize() })]
    texture_parameters: Vec<ResourceParameter>,
    #[br(temp, args { count: uav_count as usize, inner: ResourceParameterBinReadArgs::builder().strings_offset(strings_offset).finalize() })]
    uavs: Vec<ResourceParameter>,
    #[br(count = system_count)]
    system_keys: Vec<Key>,
    #[br(count = scene_count)]
    scene_keys: Vec<Key>,
    #[br(count = material_key_count)]
    material_keys: Vec<Key>,
    subview_defaults: [u32; 2],
    #[br(count = node_count, args { inner: (system_count, scene_count, material_key_count) })]
    nodes: Vec<AuditNode>,
}

#[binrw::binread]
#[br(import(system_count: u32, scene_count: u32, material_count: u32))]
struct AuditNode {
    selector: u32,
    #[br(temp)]
    pass_count: u32,
    // Meddle ShpkFile.Node: 16 slot indices, then two reserved u32 values.
    #[br(pad_after = 8)]
    pass_indices: [u8; 16],
    #[br(count = system_count)]
    system_keys: Vec<u32>,
    #[br(count = scene_count)]
    scene_keys: Vec<u32>,
    #[br(count = material_count)]
    material_keys: Vec<u32>,
    subview_keys: [u32; 2],
    #[br(count = pass_count)]
    passes: Vec<AuditPass>,
}

#[derive(BinRead, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct AuditPass {
    id: u32,
    vertex_shader: u32,
    #[br(pad_after = 12)]
    pixel_shader: u32,
}

// Installed client 0x1417ef635..6c7 copies file slots, then relocates them
// using the 13 (runtime slot, pass ID) entries at 0x1428c1900.
// This is a client-version-specific audit, not a SHPK format invariant.
const CLIENT_PASS_ID_SLOTS: [(usize, u32); 13] = [
    (0, 0x76303d61),
    (1, 0xe412a2d4),
    (2, 0x03ac862e),
    (5, 0xfbde0a8f),
    (6, 0x955c0b73),
    (9, 0x8ef40d56),
    (10, 0x24cdf1ea),
    (4, 0x6006067f),
    (5, 0x1f197698),
    (11, 0xc885bbd3),
    (8, 0xf21a038f),
    (11, 0x2d0c1a37),
    (15, 0xea06f2f7),
];

fn client_runtime_pass_indices(file: &[u8; 16], passes: &[AuditPass]) -> [u8; 16] {
    let mut runtime = *file;
    let mut written = 0u16;
    // Read the immutable file slots: earlier moves must not become new input.
    for (source, &index) in file.iter().enumerate() {
        if (index as i8) < 0 {
            continue;
        }
        let Some(pass) = passes.get(index as usize) else {
            continue;
        };
        let Some(&(target, _)) = CLIENT_PASS_ID_SLOTS.iter().find(|(_, id)| *id == pass.id) else {
            continue;
        };
        if target == source {
            continue;
        }
        runtime[target] = index;
        written |= 1 << target;
        if written & (1 << source) == 0 {
            runtime[source] = 0xff;
        }
    }
    runtime
}

#[test]
fn runtime_pass_relocation_preserves_overlapping_destinations() {
    let passes = [
        AuditPass {
            id: 0x24cdf1ea,
            vertex_shader: 0,
            pixel_shader: 0,
        },
        AuditPass {
            id: 0x8ef40d56,
            vertex_shader: 0,
            pixel_shader: 0,
        },
        AuditPass {
            id: 0xf21a038f,
            vertex_shader: 0,
            pixel_shader: 0,
        },
        AuditPass {
            id: 0xdeadbeef,
            vertex_shader: 0,
            pixel_shader: 0,
        },
    ];
    let mut file = [0xff; 16];
    file[8] = 0; // water pass ID moves to 10
    file[9] = 1; // already in its destination
    file[10] = 2; // must use original index 2 and preserve index 0 written here
    file[12] = 3; // unknown IDs stay in their file slot
    file[13] = 99; // out-of-range positive index is untouched by the loader
    file[14] = 0x80; // all signed-negative entries are absent
    let mut expected = file;
    expected[8] = 2;
    expected[10] = 0;
    assert_eq!(client_runtime_pass_indices(&file, &passes), expected);
    file[10] = 0xff;
    let relocated = client_runtime_pass_indices(&file, &passes);
    assert_eq!(relocated[8], 0xff);
    assert_eq!(relocated[10], 0);
}

#[test]
fn shader_node_keeps_pass_slots_separate_from_pass_ids() {
    let mut bytes = Vec::new();
    let put = |bytes: &mut Vec<u8>, value: u32| bytes.extend(value.to_le_bytes());
    put(&mut bytes, 0x12345678);
    put(&mut bytes, 2);
    let mut slots = [0xff; 16];
    slots[6] = 1;
    slots[10] = 0;
    bytes.extend(slots);
    put(&mut bytes, 0x11223344); // reserved fields must not shift key parsing
    put(&mut bytes, 0x55667788);
    for value in [101, 102, 103, 104, 105] {
        put(&mut bytes, value);
    }
    for (id, vs, ps) in [(0xc5a00001, 7, 9), (0xc5a00002, 11, 13)] {
        for value in [id, vs, ps, 0, 0, 0] {
            put(&mut bytes, value);
        }
    }
    let mut cursor = std::io::Cursor::new(&bytes);
    let node = AuditNode::read_options(&mut cursor, binrw::Endian::Little, (1, 1, 1))
        .expect("node with sparse pass slots");
    assert_eq!(cursor.position() as usize, bytes.len());
    assert_eq!(node.pass_indices, slots);
    assert_eq!(node.system_keys, [101]);
    assert_eq!(node.scene_keys, [102]);
    assert_eq!(node.material_keys, [103]);
    assert_eq!(node.subview_keys, [104, 105]);
    assert_eq!(node.passes[node.pass_indices[10] as usize].id, 0xc5a00001);
    assert_eq!(node.passes[node.pass_indices[6] as usize].pixel_shader, 13);
}

#[test]
#[ignore = "inspects installed shader pass slots; requires XIV_GAME_DIR"]
fn audit_installed_decal_pass_slots() {
    let raw_dir = PathBuf::from(std::env::var_os("XIV_GAME_DIR").expect("XIV_GAME_DIR"));
    let game_dir = xiv_companion::game_data::normalize_game_dir(&raw_dir).expect("game directory");
    let mut resource = SqPackResource::from_existing(game_dir.to_str().expect("UTF-8 game path"));
    let mut packages = Vec::new();
    for name in [
        "apricot_decal",
        "apricot_decal_ring",
        "apricot_gbuffer_decal",
        "apricot_gbuffer_decal_ring",
        // The 0x40 stencil writer at client 0x140299a70 belongs to
        // WaterRenderer (Manager+0x2e390), iterating LOD water meshes.
        // Character packages below are comparison coverage, not evidence
        // that ordinary model surfaces participate in that writer.
        "water",
        "river",
        "skin",
        "character",
        "characterlegacy",
    ] {
        let path = format!("shader/sm5/shpk/{name}.shpk");
        let bytes = resource
            .read(&path)
            .unwrap_or_else(|| panic!("read {path}"));
        let package = AuditShaderPackage::read_options(
            &mut std::io::Cursor::new(&bytes),
            binrw::Endian::Little,
            (),
        )
        .unwrap_or_else(|error| panic!("parse {path}: {error}"));
        let mut counts = [0usize; 16];
        let mut runtime_counts = [0usize; 16];
        for node in &package.nodes {
            assert!(node.passes.len() <= u8::MAX as usize);
            let runtime = client_runtime_pass_indices(&node.pass_indices, &node.passes);
            if matches!(name, "water" | "river") {
                assert!((node.pass_indices[10] as i8) < 0);
                assert_eq!(runtime[8], 0xff);
                assert_eq!(runtime[9], node.pass_indices[8]);
                assert_eq!(runtime[10], node.pass_indices[9]);
                assert_eq!(node.passes[runtime[9] as usize].id, 0x8ef40d56);
                assert_eq!(node.passes[runtime[10] as usize].id, 0x24cdf1ea);
            }
            for (slot, &index) in runtime.iter().enumerate() {
                if (index as i8) >= 0 {
                    assert!((index as usize) < node.passes.len());
                    runtime_counts[slot] += 1;
                }
            }
            if name.starts_with("apricot_") {
                // Client 0x140386595..65d5 selects the first pass directly,
                // then 0x14037e6d0 binds VS/PS without Context pass lookup.
                assert_eq!(
                    node.passes.len(),
                    1,
                    "{name}: first-pass binding assumption"
                );
                assert_eq!(node.pass_indices[0], 0, "{name}: file slot zero");
                assert!(
                    node.pass_indices[1..]
                        .iter()
                        .all(|&index| (index as i8) < 0)
                );
            }
            for (slot, &index) in node.pass_indices.iter().enumerate() {
                if (index as i8) < 0 {
                    continue;
                }
                let pass = node.passes.get(index as usize).unwrap_or_else(|| {
                    panic!(
                        "{name} node {} slot {slot}: invalid index {index}",
                        node.selector
                    )
                });
                assert!((pass.vertex_shader as usize) < package.vertex_shaders.len());
                assert!((pass.pixel_shader as usize) < package.pixel_shaders.len());
                counts[slot] += 1;
            }
        }
        println!(
            "{name}: {} nodes, file slots {counts:?}, runtime slots {runtime_counts:?}",
            package.nodes.len()
        );
        packages.push(
            json!({"path": path, "slotCoverage": counts, "runtimeSlotCoverage": runtime_counts,
            "nodes": package.nodes.iter().map(|node| json!({
                "selector": node.selector, "passIndices": node.pass_indices, "passes": node.passes,
                "runtimePassIndices": client_runtime_pass_indices(&node.pass_indices, &node.passes),
                "systemKeys": node.system_keys, "sceneKeys": node.scene_keys,
                "materialKeys": node.material_keys, "subviewKeys": node.subview_keys,
            })).collect::<Vec<_>>() }),
        );
    }
    let output = PathBuf::from("target/weapon-vfx-audit/decal-pass-slots.json");
    fs::create_dir_all(output.parent().unwrap()).expect("create audit directory");
    fs::write(
        output,
        serde_json::to_vec_pretty(&json!({
            "gameVersion": fs::read_to_string(game_dir.join("ffxivgame.ver")).ok(),
            "runtimePassIdSlots": CLIENT_PASS_ID_SLOTS,
            "packages": packages,
        }))
        .unwrap(),
    )
    .expect("write pass slot report");
}

fn parameters(parameters: &[ResourceParameter]) -> Vec<Value> {
    parameters
        .iter()
        .map(|parameter| {
            json!({ "name": parameter.name, "slot": parameter.slot, "size": parameter.size })
        })
        .collect()
}

#[test]
#[ignore = "inspects installed ModelSkin Aura texture inputs; requires XIV_GAME_DIR"]
fn audit_installed_model_skin_aura_textures() {
    let raw_dir = PathBuf::from(std::env::var_os("XIV_GAME_DIR").expect("XIV_GAME_DIR"));
    let game_dir = xiv_companion::game_data::normalize_game_dir(&raw_dir).expect("game directory");
    let mut resource = SqPackResource::from_existing(game_dir.to_str().expect("UTF-8 game path"));
    let mut rows = Vec::new();
    let mut enabled_counts = [0_usize; 3];
    let mut target_counts = [0_usize; 2];
    for path in MODEL_SKIN_PATHS {
        let bytes = resource.read(path).unwrap_or_else(|| panic!("read {path}"));
        let file = xiv_companion::AvfxFile::parse(&bytes)
            .unwrap_or_else(|error| panic!("parse {path}: {error}"));
        let texture_path = |index: i32| {
            usize::try_from(index)
                .ok()
                .and_then(|index| file.texture_paths.get(index))
        };
        for (particle_index, particle) in file.particles.iter().enumerate() {
            let xiv_companion::AvfxParticleData::ModelSkin(data) = &particle.data else {
                continue;
            };
            let target_role = match data.aura_target {
                2 => {
                    target_counts[0] += 1;
                    "Weapon"
                }
                4 => {
                    target_counts[1] += 1;
                    "OffHand"
                }
                target => panic!("unexpected Aura target {target} in {path}"),
            };
            let sources = [
                particle
                    .texture_color2
                    .as_ref()
                    .map(|texture| (texture.enabled, texture.texture_index)),
                particle
                    .texture_color3
                    .as_ref()
                    .map(|texture| (texture.enabled, texture.texture_index)),
                particle
                    .texture_distortion
                    .as_ref()
                    .map(|texture| (texture.enabled, texture.texture_index)),
            ];
            let mut decoded_sources = [None, None, None];
            let mut decoded_textures = vec![None; file.texture_paths.len()];
            for (name, texture) in [
                ("TC2", particle.texture_color2.as_ref()),
                ("TC3", particle.texture_color3.as_ref()),
            ] {
                if let Some(texture) = texture.filter(|texture| texture.enabled) {
                    assert!(
                        !texture.use_screen_copy && !texture.use_chara_portrait,
                        "{path} {name} uses an Aura texture source other than file Tex"
                    );
                }
            }
            for (slot, source) in sources.into_iter().enumerate() {
                let Some((true, index)) = source else {
                    continue;
                };
                enabled_counts[slot] += 1;
                let texture_path = texture_path(index)
                    .unwrap_or_else(|| panic!("missing Aura texture index {index} in {path}"));
                assert!(
                    texture_path.ends_with(".atex"),
                    "not an ATEX: {texture_path}"
                );
                let bytes = resource
                    .read(texture_path)
                    .unwrap_or_else(|| panic!("unreadable Aura texture {texture_path} in {path}"));
                let decoded = xiv_companion_data::avfx::decode_atex_rgba(&bytes)
                    .unwrap_or_else(|| panic!("undecodable Aura texture {texture_path} in {path}"));
                let is_hdr = !decoded.rgba16f_mips.is_empty();
                decoded_sources[slot] = Some(json!({
                    "width": decoded.width,
                    "height": decoded.height,
                    "format": if is_hdr { "rgba16float" } else { "rgba8unorm" },
                    "sourceMipCount": decoded.source_mip_count,
                    "decodedMipCount": decoded.decoded_mip_count(),
                    "completeSourceMips": decoded.has_complete_source_mips(),
                    "isCube": decoded.is_cube,
                    "sourceType": decoded.source_type,
                }));
                decoded_textures[usize::try_from(index).expect("nonnegative texture index")] =
                    Some(decoded);
            }
            let packed = xiv_companion_data::VfxAuraTextureArrayRgba::from_model_skin_particle(
                particle,
                &decoded_textures,
            )
            .unwrap_or_else(|error| panic!("unpackable Aura layers in {path}: {error:?}"));
            let color = |slot: usize, texture: Option<&xiv_companion::AvfxParticleTexture>| {
                texture.map(|texture| {
                    json!({
                        "enabled": texture.enabled,
                        "textureIndex": texture.texture_index,
                        "texturePath": texture_path(texture.texture_index),
                        "useScreenCopy": texture.use_screen_copy,
                        "useCharaPortrait": texture.use_chara_portrait,
                        "decoded": decoded_sources[slot].as_ref(),
                    })
                })
            };
            let distortion = particle.texture_distortion.as_ref().map(|texture| {
                json!({
                    "enabled": texture.enabled,
                    "textureIndex": texture.texture_index,
                    "texturePath": texture_path(texture.texture_index),
                    "decoded": decoded_sources[2].as_ref(),
                })
            });
            rows.push(json!({
                "path": path,
                "particle": particle_index,
                "auraTarget": data.aura_target,
                "targetRole": target_role,
                "packable2DArrayWithoutResampling": true,
                "packedArray": {
                    "width": packed.width,
                    "height": packed.height,
                    "layerCount": packed.layer_count(),
                    "mipCount": packed.mips.len(),
                    "slotLayers": packed.slot_layers,
                },
                "auraTextures": {
                    "slot0Tc2": color(0, particle.texture_color2.as_ref()),
                    "slot1Tc3": color(1, particle.texture_color3.as_ref()),
                    "slot2Td": distortion,
                },
            }));
        }
    }
    assert_eq!(rows.len(), 8, "installed ModelSkin definition count");
    assert_eq!(target_counts, [7, 1], "installed Aura target distribution");
    assert_eq!(enabled_counts, [8, 4, 3], "installed Aura texture inputs");
    assert!(
        rows.iter()
            .all(|row| row["packable2DArrayWithoutResampling"] == true),
        "installed Aura layers require an array layout without resampling"
    );
    let output_dir = PathBuf::from("target/weapon-vfx-audit/model-skin-shaders");
    fs::create_dir_all(&output_dir).expect("create ModelSkin audit directory");
    fs::write(
        output_dir.join("installed-aura-textures.json"),
        serde_json::to_vec_pretty(&rows).expect("serialize ModelSkin Aura textures"),
    )
    .expect("write ModelSkin Aura texture report");
}

#[test]
#[ignore = "loads installed ModelSkin weapon surfaces; requires XIV_GAME_DIR"]
fn audit_installed_model_skin_surface_targets() {
    use xiv_companion::game_data::{
        export_weapon_catalog_from_resource, game_version, normalize_game_dir,
    };
    use xiv_companion::{
        load_weapon_model_from_resource, load_weapon_vfx_attachments_from_resource,
        weapon_vfx_imc_path,
    };
    use xiv_companion_data::imc::ImcFile;

    let raw_dir = PathBuf::from(std::env::var_os("XIV_GAME_DIR").expect("XIV_GAME_DIR"));
    let game_dir = normalize_game_dir(&raw_dir).expect("game directory");
    let game_path = game_dir.to_str().expect("UTF-8 game path");
    let catalog = export_weapon_catalog_from_resource(
        SqPackResource::from_existing(game_path),
        game_dir.display().to_string(),
        game_version(&game_dir),
        "model-skin-target-audit".to_string(),
    )
    .expect("weapon catalog");
    let mut resource = SqPackResource::from_existing(game_path);
    let mut rows = Vec::new();
    for path in MODEL_SKIN_PATHS {
        let parts = path.split('/').collect::<Vec<_>>();
        let model_id = parts[2][1..].parse::<u16>().expect("weapon model id");
        let body_id = parts[5][1..].parse::<u16>().expect("weapon body id");
        let vfx_id = parts[8][2..6].parse::<u8>().expect("VfxId");
        let imc_path = weapon_vfx_imc_path(model_id, body_id);
        let imc = ImcFile::parse(
            &resource
                .read(&imc_path)
                .unwrap_or_else(|| panic!("read {imc_path}")),
        )
        .unwrap_or_else(|error| panic!("parse {imc_path}: {error}"));
        let imc_variants = imc
            .subsets
            .iter()
            .enumerate()
            .filter_map(|(index, entries)| (entries[0].vfx == vfx_id).then_some(index + 1))
            .collect::<Vec<_>>();
        let catalog_candidates = catalog
            .items
            .iter()
            .filter(|item| {
                [Some(item.primary_model()), item.secondary_model()]
                    .into_iter()
                    .flatten()
                    .any(|packed| packed.model_id == model_id && packed.body_id == body_id)
            })
            .collect::<Vec<_>>();
        let item = catalog_candidates.iter().copied().find(|item| {
            [Some(item.primary_model()), item.secondary_model()]
                .into_iter()
                .flatten()
                .any(|packed| {
                    packed.model_id == model_id
                        && packed.body_id == body_id
                        && imc.subset_for_variant(packed.variant_id)[0].vfx == vfx_id
                })
        });
        let Some(item) = item else {
            rows.push(json!({
                "avfx": path,
                "status": "noCatalogMount",
                "imcVariants": imc_variants,
                "catalogCandidates": catalog_candidates.len(),
            }));
            continue;
        };
        let model = load_weapon_model_from_resource(&mut resource, item)
            .unwrap_or_else(|error| panic!("load item {}: {error:#}", item.id));
        let attachments = load_weapon_vfx_attachments_from_resource(&mut resource, &model)
            .unwrap_or_else(|error| panic!("load effects for item {}: {error:#}", item.id))
            .unwrap_or_else(|| panic!("item {} has no effects", item.id));
        let mount = attachments
            .attachments
            .iter()
            .find(|mount| mount.data.avfx_path == path)
            .unwrap_or_else(|| panic!("item {} did not mount {path}", item.id));
        let targets = mount
            .data
            .file
            .particles
            .iter()
            .filter_map(|particle| match &particle.data {
                xiv_companion::AvfxParticleData::ModelSkin(data) => Some(data.aura_target),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(!targets.is_empty(), "{path} has no ModelSkin");
        assert!(
            targets.iter().all(|target| *target == targets[0]),
            "{path} has mixed targets"
        );
        let target_model = match targets[0] {
            2 => model.model_main,
            4 => model.model_sub.expect("Off_Hand requires a secondary MDL"),
            target => panic!("unsupported installed Aura target {target}"),
        };
        let target_path = model
            .loaded_paths
            .iter()
            .find(|path| {
                path.ends_with(".mdl")
                    && xiv_companion::weapon_model_candidate_paths(target_model).contains(path)
            })
            .unwrap_or_else(|| panic!("item {} did not load its Aura target", item.id));
        assert_eq!(
            attachments.model_skin_target_path(targets[0]),
            Some(target_path.as_str()),
            "{path} Aura target differs from the loaded model"
        );
        let surface_meshes = model
            .meshes
            .iter()
            .filter(|mesh| mesh.path.split('#').next() == Some(target_path.as_str()))
            .map(|mesh| {
                let material = &model.materials[mesh.material_slot];
                json!({
                    "mesh": mesh.path,
                    "material": material.path,
                    "shaderPackage": material.shader_package_name,
                    "faceDecalTexture": material.character_colors.as_ref().and_then(|colors| colors.decal_texture),
                })
            })
            .collect::<Vec<_>>();
        assert!(
            !surface_meshes.is_empty(),
            "{target_path} has no surface meshes"
        );
        assert!(
            surface_meshes
                .iter()
                .all(|mesh| mesh["faceDecalTexture"].is_null()),
            "{target_path} needs both a face decal and an Aura texture slot"
        );
        let mut shader_packages = BTreeMap::new();
        for mesh in model
            .meshes
            .iter()
            .filter(|mesh| mesh.path.split('#').next() == Some(target_path.as_str()))
        {
            let Some(name) = model.materials[mesh.material_slot]
                .shader_package_name
                .as_deref()
            else {
                continue;
            };
            if shader_packages.contains_key(name) {
                continue;
            }
            let shader_path = format!("shader/sm5/shpk/{name}");
            let bytes = resource
                .read(&shader_path)
                .unwrap_or_else(|| panic!("read {shader_path}"));
            let package = AuditShaderPackage::read_options(
                &mut std::io::Cursor::new(&bytes),
                binrw::Endian::Little,
                (),
            )
            .unwrap_or_else(|error| panic!("parse {shader_path}: {error}"));
            let aura_key = physis::shpk::ShaderPackage::crc("ApplyAuraColor");
            let aura_on = physis::shpk::ShaderPackage::crc("ApplyAuraColorOn");
            let aura_off = physis::shpk::ShaderPackage::crc("ApplyAuraColorOff");
            let scene_key = package.scene_keys.iter().position(|key| key.id == aura_key);
            let on_nodes = scene_key.map_or(0, |index| {
                package
                    .nodes
                    .iter()
                    .filter(|node| node.scene_keys[index] == aura_on)
                    .count()
            });
            let off_nodes = scene_key.map_or(0, |index| {
                package
                    .nodes
                    .iter()
                    .filter(|node| node.scene_keys[index] == aura_off)
                    .count()
            });
            assert!(
                scene_key.is_some() && on_nodes > 0,
                "{shader_path} has no Aura-on path"
            );
            assert!(
                off_nodes >= on_nodes,
                "{shader_path} has too few Aura-off nodes"
            );
            shader_packages.insert(
                name.to_string(),
                json!({
                    "sceneAuraKey": scene_key.is_some(),
                    "onNodes": on_nodes,
                    "offNodes": off_nodes,
                }),
            );
        }
        rows.push(json!({
            "avfx": path,
            "status": "loaded",
            "itemId": item.id,
            "itemName": item.name,
            "auraTarget": targets[0],
            "mountMdl": mount.model_path,
            "targetMdl": target_path,
            "targetMatchesMount": target_path == &mount.model_path,
            "imcVariants": imc_variants,
            "surfaceMeshes": surface_meshes,
            "shaderPackages": shader_packages,
        }));
    }
    assert_eq!(rows.len(), MODEL_SKIN_PATHS.len());
    assert_eq!(
        rows.iter().filter(|row| row["status"] == "loaded").count(),
        6,
        "installed ModelSkin files mounted by catalog items"
    );
    assert_eq!(
        rows.iter()
            .filter(|row| row["status"] == "noCatalogMount")
            .count(),
        1,
        "installed ModelSkin files without a catalog mount"
    );
    let output_dir = PathBuf::from("target/weapon-vfx-audit/model-skin-shaders");
    fs::create_dir_all(&output_dir).expect("create ModelSkin audit directory");
    fs::write(
        output_dir.join("installed-surface-targets.json"),
        serde_json::to_vec_pretty(&rows).expect("serialize ModelSkin surface targets"),
    )
    .expect("write ModelSkin surface target report");
}

#[test]
#[ignore = "inspects installed model-surface Aura shader variants; requires XIV_GAME_DIR"]
fn audit_installed_model_skin_aura_shaders() {
    let raw_dir = PathBuf::from(std::env::var_os("XIV_GAME_DIR").expect("XIV_GAME_DIR"));
    let game_dir = xiv_companion::game_data::normalize_game_dir(&raw_dir).expect("game directory");
    let mut resource = SqPackResource::from_existing(game_dir.to_str().expect("UTF-8 game path"));
    let output_dir = PathBuf::from("target/weapon-vfx-audit/model-skin-shaders");
    fs::create_dir_all(&output_dir).expect("create model-skin shader audit directory");
    let aura_key = physis::shpk::ShaderPackage::crc("ApplyAuraColor");
    let aura_on = physis::shpk::ShaderPackage::crc("ApplyAuraColorOn");
    let aura_off = physis::shpk::ShaderPackage::crc("ApplyAuraColorOff");
    let mut report = Vec::new();
    for name in ["skin", "character", "characterlegacy"] {
        let path = format!("shader/sm5/shpk/{name}.shpk");
        let bytes = resource
            .read(&path)
            .unwrap_or_else(|| panic!("read {path}"));
        let package = AuditShaderPackage::read_options(
            &mut std::io::Cursor::new(&bytes),
            binrw::Endian::Little,
            (),
        )
        .unwrap_or_else(|error| panic!("parse {path}: {error}"));
        let system_index = package
            .system_keys
            .iter()
            .position(|key| key.id == aura_key);
        let scene_index = package.scene_keys.iter().position(|key| key.id == aura_key);
        let material_index = package
            .material_keys
            .iter()
            .position(|key| key.id == aura_key);
        let signature = |node: &AuditNode| {
            let mut values = node.system_keys.clone();
            values.extend(&node.scene_keys);
            values.extend(&node.material_keys);
            values.extend(node.subview_keys);
            let system_count = package.system_keys.len();
            let scene_count = package.scene_keys.len();
            if let Some(index) = system_index {
                values[index] = 0;
            }
            if let Some(index) = scene_index {
                values[system_count + index] = 0;
            }
            if let Some(index) = material_index {
                values[system_count + scene_count + index] = 0;
            }
            values
        };
        let aura_value = |node: &AuditNode| {
            system_index
                .and_then(|index| node.system_keys.get(index))
                .or_else(|| scene_index.and_then(|index| node.scene_keys.get(index)))
                .or_else(|| material_index.and_then(|index| node.material_keys.get(index)))
                .copied()
        };
        let off_nodes = package
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| aura_value(node) == Some(aura_off))
            .map(|(index, node)| (signature(node), index))
            .collect::<BTreeMap<_, _>>();
        let on_nodes = package
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| aura_value(node) == Some(aura_on))
            .collect::<Vec<_>>();
        let pairs = on_nodes
            .iter()
            .filter_map(|(on_index, on)| {
                off_nodes
                    .get(&signature(on))
                    .map(|off_index| (*on_index, *off_index))
            })
            .collect::<Vec<_>>();
        assert_eq!(
            pairs.len(),
            on_nodes.len(),
            "{path} Aura-on nodes without off pairs"
        );
        let mut aura_passes = 0;
        for (on_index, off_index) in &pairs {
            let on = &package.nodes[*on_index];
            let off = &package.nodes[*off_index];
            for pass in &on.passes {
                let Some(off_pass) = off.passes.iter().find(|other| other.id == pass.id) else {
                    continue;
                };
                let on_shader = &package.pixel_shaders[pass.pixel_shader as usize];
                let off_shader = &package.pixel_shaders[off_pass.pixel_shader as usize];
                let has_aura = |shader: &Shader| {
                    shader
                        .scalar_parameters
                        .iter()
                        .any(|parameter| parameter.name == "g_AuraParam")
                };
                if has_aura(on_shader) {
                    aura_passes += 1;
                    assert!(
                        on_shader
                            .scalar_parameters
                            .iter()
                            .any(
                                |parameter| parameter.name == "g_AuraParam" && parameter.size == 16
                            ),
                        "{path} pass {} Aura constant block is not 16 registers",
                        pass.id
                    );
                    assert!(!has_aura(off_shader), "{path} pass {} Aura off", pass.id);
                    for texture in [
                        "g_SamplerAuraTexture",
                        "g_SamplerAuraTexture1",
                        "g_SamplerAuraTexture2",
                    ] {
                        assert!(
                            on_shader
                                .texture_parameters
                                .iter()
                                .any(|parameter| parameter.name == texture),
                            "{path} pass {} missing {texture}",
                            pass.id
                        );
                        assert!(
                            !off_shader
                                .texture_parameters
                                .iter()
                                .any(|parameter| parameter.name == texture),
                            "{path} pass {} Aura off includes {texture}",
                            pass.id
                        );
                    }
                }
            }
        }
        assert!(aura_passes > 0, "{path} has no paired Aura pixel pass");
        let mut variants = Vec::new();
        for (pair_index, (on_index, off_index)) in pairs.iter().take(3).enumerate() {
            let on = &package.nodes[*on_index];
            let off = &package.nodes[*off_index];
            for (state, node) in [("on", on), ("off", off)] {
                let passes = node
                    .passes
                    .iter()
                    .map(|pass| {
                        let shader = &package.pixel_shaders[pass.pixel_shader as usize];
                        let vertex = &package.vertex_shaders[pass.vertex_shader as usize];
                        let declared =
                            u32::from_le_bytes(shader.bytecode[24..28].try_into().unwrap())
                                as usize;
                        let file_name =
                            format!("{name}-{pair_index}-{state}-ps{}.dxbc", pass.pixel_shader);
                        fs::write(output_dir.join(&file_name), &shader.bytecode[..declared])
                            .expect("write Aura pixel shader");
                        let vertex_declared =
                            u32::from_le_bytes(vertex.bytecode[24..28].try_into().unwrap())
                                as usize;
                        let vertex_name =
                            format!("{name}-{pair_index}-{state}-vs{}.dxbc", pass.vertex_shader);
                        fs::write(
                            output_dir.join(&vertex_name),
                            &vertex.bytecode[..vertex_declared],
                        )
                        .expect("write Aura vertex shader");
                        json!({
                            "passId": pass.id,
                            "vertexShader": pass.vertex_shader,
                            "vertexFile": vertex_name,
                            "pixelShader": pass.pixel_shader,
                            "file": file_name,
                            "constants": parameters(&shader.scalar_parameters),
                            "samplers": parameters(&shader.resource_parameters),
                            "textures": parameters(&shader.texture_parameters),
                        })
                    })
                    .collect::<Vec<_>>();
                variants.push(json!({
                    "pair": pair_index,
                    "state": state,
                    "node": if state == "on" { *on_index } else { *off_index },
                    "passIndices": node.pass_indices,
                    "passes": passes,
                }));
            }
        }
        report.push(json!({
            "path": path,
            "vertexShaders": package.vertex_shaders.len(),
            "pixelShaders": package.pixel_shaders.len(),
            "nodes": package.nodes.len(),
            "auraKey": { "system": system_index, "scene": scene_index, "material": material_index },
            "auraOnNodes": on_nodes.len(),
            "auraOffNodes": off_nodes.len(),
            "matchedPairs": pairs.len(),
            "pairedAuraPixelPasses": aura_passes,
            "variants": variants,
        }));
    }
    assert!(
        report
            .iter()
            .any(|entry| entry["matchedPairs"].as_u64().unwrap_or(0) > 0)
    );
    let report = json!({ "packages": report });
    fs::write(
        output_dir.join("report.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .expect("write Aura shader report");
    for package in report["packages"].as_array().unwrap() {
        println!(
            "{}: {}/{} Aura-on nodes paired, {} Aura pixel passes",
            package["path"].as_str().unwrap(),
            package["matchedPairs"],
            package["auraOnNodes"],
            package["pairedAuraPixelPasses"]
        );
    }
}

#[test]
#[ignore = "exports installed client VFX shaders; requires XIV_GAME_DIR, optionally VKD3D_COMPILER"]
fn export_installed_vfx_shaders() {
    let raw_dir = PathBuf::from(std::env::var_os("XIV_GAME_DIR").expect("XIV_GAME_DIR"));
    let game_dir = xiv_companion::game_data::normalize_game_dir(&raw_dir).expect("game directory");
    let mut resource = SqPackResource::from_existing(game_dir.to_str().expect("UTF-8 game path"));
    let output_dir = PathBuf::from("target/weapon-vfx-audit/shaders");
    fs::create_dir_all(&output_dir).expect("create shader audit directory");
    let compiler = std::env::var_os("VKD3D_COMPILER");
    let compiler_version = compiler.as_ref().map(|compiler| {
        let result = Command::new(compiler)
            .arg("--version")
            .output()
            .expect("vkd3d version");
        assert!(result.status.success(), "vkd3d version failed");
        String::from_utf8_lossy(&result.stdout).trim().to_owned()
    });
    let mut key_names = BTreeMap::new();
    if let Ok(executable) = fs::read(game_dir.join("ffxiv_dx11.exe")) {
        for candidate in executable.split(|byte| !(0x20..=0x7e).contains(byte)) {
            if (4..=256).contains(&candidate.len()) {
                let name = std::str::from_utf8(candidate).expect("ASCII candidate");
                key_names.insert(physis::shpk::ShaderPackage::crc(name), name.to_owned());
            }
        }
    }
    let mut packages = Vec::new();
    for name in [
        "apricot_decal",
        "apricot_decal_ring",
        "apricot_gbuffer_decal",
        "apricot_gbuffer_decal_ring",
        "apricot_powder",
        "apricot_shape",
        "apricot_model",
        "apricot_model_morph",
        "apricot_lightmodel",
        "apricot_fogModel",
    ] {
        let path = format!("shader/sm5/shpk/{name}.shpk");
        let bytes = resource
            .read(&path)
            .unwrap_or_else(|| panic!("read {path}"));
        fs::write(output_dir.join(format!("{name}.shpk")), &bytes).expect("write package");
        let package = AuditShaderPackage::read_options(
            &mut std::io::Cursor::new(&bytes),
            binrw::Endian::Little,
            (),
        )
        .unwrap_or_else(|error| panic!("parse {path}: {error}"));
        let package_dir = output_dir.join(name);
        fs::create_dir_all(&package_dir).expect("create package directory");
        for pass in package.nodes.iter().flat_map(|node| &node.passes) {
            assert!(
                (pass.vertex_shader as usize) < package.vertex_shaders.len(),
                "{path} vertex index"
            );
            assert!(
                (pass.pixel_shader as usize) < package.pixel_shaders.len(),
                "{path} pixel index"
            );
        }
        let mut shaders = Vec::new();
        for (stage, entries) in [
            ("vs", &package.vertex_shaders),
            ("ps", &package.pixel_shaders),
        ] {
            for (index, shader) in entries.iter().enumerate() {
                // Physis includes the vertex header's eight trailing bytes in data_size.
                let bytecode = &shader.bytecode;
                assert!(
                    bytecode.len() >= 32 && bytecode.starts_with(b"DXBC"),
                    "{path} {stage}{index}"
                );
                let declared = u32::from_le_bytes(bytecode[24..28].try_into().unwrap()) as usize;
                assert!(
                    (32..=bytecode.len()).contains(&declared),
                    "{path} {stage}{index} size"
                );
                let file_name = format!("{stage}{index}.dxbc");
                let shader_path = package_dir.join(&file_name);
                fs::write(&shader_path, &bytecode[..declared]).expect("write DXBC");
                if let Some(compiler) = &compiler {
                    let result = Command::new(compiler)
                        .args(["-x", "dxbc-tpf", "-b", "d3d-asm", "--formatting=signatures"])
                        .arg(&shader_path)
                        .arg("-o")
                        .arg(shader_path.with_extension("asm"))
                        .output()
                        .expect("run vkd3d-compiler");
                    assert!(
                        result.status.success(),
                        "{path} {stage}{index}: {}",
                        String::from_utf8_lossy(&result.stderr)
                    );
                }
                shaders.push(json!({
                    "stage": stage,
                    "index": index,
                    "file": file_name,
                    "bytes": declared,
                    "dxbcChecksum": bytecode[4..20].iter().map(|byte| format!("{byte:02x}")).collect::<String>(),
                    "constants": parameters(&shader.scalar_parameters),
                    "samplers": parameters(&shader.resource_parameters),
                    "textures": parameters(&shader.texture_parameters),
                }));
            }
        }
        let keys = |keys: &[physis::shpk::Key]| {
            keys.iter()
                .map(|key| json!({ "id": key.id, "name": key_names.get(&key.id), "default": key.default_value, "defaultName": key_names.get(&key.default_value) }))
                .collect::<Vec<_>>()
        };
        let metadata = json!({
            "path": path,
            "version": package.version,
            "extraHeader": package.extra_header,
            "subviewDefaults": package.subview_defaults,
            "constants": parameters(&package.scalar_parameters),
            "samplers": parameters(&package.sampler_parameters),
            "textures": parameters(&package.texture_parameters),
            "systemKeys": keys(&package.system_keys),
            "sceneKeys": keys(&package.scene_keys),
            "materialKeys": keys(&package.material_keys),
            "nodes": package.nodes.iter().map(|node| json!({
                "selector": node.selector,
                "systemKeys": node.system_keys,
                "sceneKeys": node.scene_keys,
                "sceneKeyNames": node.scene_keys.iter().map(|value| key_names.get(value)).collect::<Vec<_>>(),
                "materialKeys": node.material_keys,
                "subviewKeys": node.subview_keys,
                "passIndices": node.pass_indices,
                "passes": node.passes,
            })).collect::<Vec<_>>(),
            "shaders": shaders,
        });
        fs::write(
            package_dir.join("metadata.json"),
            serde_json::to_vec_pretty(&metadata).unwrap(),
        )
        .expect("write package metadata");
        packages.push(json!({
            "path": path,
            "vertexShaders": package.vertex_shaders.len(),
            "pixelShaders": package.pixel_shaders.len(),
            "nodes": package.nodes.len(),
        }));
    }
    let report = json!({
        "gameVersion": fs::read_to_string(game_dir.join("ffxivgame.ver")).ok(),
        "disassembler": compiler_version,
        "packages": packages,
    });
    fs::write(
        output_dir.join("report.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .expect("write shader audit report");
    println!("{report:#}");
}
