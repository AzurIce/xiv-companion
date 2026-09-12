#![cfg(feature = "game-data")]

//! 骨骼动画解析层的本地真实数据批量验证（需 XIV_GAME_DIR 指向游戏安装目录）。
//!
//! 覆盖四条链路，全部走 vendored 的 `xiv_companion_data::vendor::physis_havok`
//!（derived from physis, MIT）：
//! - sklb：角色骨架 c0101..c1801 + 怪物/半人骨架批量解析，断言 bone 数一致、
//!   层级无环、reference pose 有限值，并统计 bone 数分布；
//! - pap：动画名表 + 内嵌 hkaAnimationContainer 解码，统计 animations/bindings
//!   长度、量化分布、duration，并推断名表 `havok_index` 的指向语义；
//! - bone 窗口：demihuman d0001e0001_top mesh0 的 submesh bone 窗口经
//!   bone_table 映射回 c0101 骨骼名，验证窗口链路自洽；
//! - 采样冒烟：joy.pap 首动画在 t=0/500/1500ms 采样，断言输出长度与取值有限。

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use physis::resource::{Resource, SqPackResource};
use xiv_companion_data::mdl_metadata_from_mdl_bytes;
use xiv_companion_data::vendor::physis_havok::{
    HavokAnimationContainer, HavokBinaryTagFileReader, HavokSplineCompressedAnimation,
    animation_binding::HavokAnimationBlendHint, object::HavokObject,
};

fn game_dir() -> String {
    std::env::var("XIV_GAME_DIR").unwrap_or_else(|_| r"E:\_ff14\game".to_string())
}

// ---------------------------------------------------------------------------
// sklb / pap 容器头解析（最小实现，布局对齐 physis skeleton.rs / pap.rs 的结构定义）
// ---------------------------------------------------------------------------

const TAGFILE_MAGIC: u32 = 0xCAB0_0D1E;

/// 返回 (版本, havok_offset)。未知版本时在文件头范围内回退扫描 tagfile magic。
fn sklb_havok_offset(bytes: &[u8]) -> Result<(u32, usize, bool), String> {
    // 真实文件的 magic 按字节是 "blks"（physis 的 0x736B6C62 即其 LE 读取值），
    // 部分文档写作 "sklb"；两种都接受。
    if bytes.len() < 16 || (&bytes[0..4] != b"blks" && &bytes[0..4] != b"sklb") {
        let head = bytes
            .iter()
            .take(16)
            .map(|byte| format!("{byte:02x}"))
            .collect::<Vec<_>>()
            .join(" ");
        return Err(format!("bad sklb magic (len={}, head={head})", bytes.len()));
    }
    let version = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
    let (offset, scanned) = match version {
        // physis SklbV1 / SklbV2 布局
        0x3132_3030 => (u16::from_le_bytes([bytes[10], bytes[11]]) as usize, false),
        0x3133_3030 | 0x3133_3031 => (
            u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]) as usize,
            false,
        ),
        other => {
            let pos = bytes
                .iter()
                .take(128)
                .position(|b| *b == 0x1e)
                .filter(|pos| {
                    *pos + 4 <= bytes.len()
                        && u32::from_le_bytes([
                            bytes[*pos],
                            bytes[*pos + 1],
                            bytes[*pos + 2],
                            bytes[*pos + 3],
                        ]) == TAGFILE_MAGIC
                })
                .ok_or_else(|| format!("unknown sklb version {other:#010x}, no tagfile magic"))?;
            println!("    (未知 sklb 版本 {other:#010x}，回退扫描到 tagfile @ {pos})");
            (pos, true)
        }
    };
    if offset + 4 > bytes.len()
        || u32::from_le_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
        ]) != TAGFILE_MAGIC
    {
        return Err(format!("tagfile magic missing at offset {offset}"));
    }
    Ok((version, offset, scanned))
}

struct PapEntry {
    name: String,
    animation_type: u16,
    havok_index: i16,
    face: bool,
}

struct PapHeader {
    entries: Vec<PapEntry>,
    havok_position: usize,
    tmb_offset: usize,
}

/// pap 固定头 26 字节，后接 num_animations 条 40 字节名表项
/// （name[32] / type u16 / havok_index i16 / face i32）。
fn parse_pap_header(bytes: &[u8]) -> Result<PapHeader, String> {
    if bytes.len() < 26 || &bytes[0..4] != b"pap " {
        return Err("bad pap magic".to_string());
    }
    let num_animations = i16::from_le_bytes([bytes[8], bytes[9]]);
    if num_animations < 0 {
        return Err("negative num_animations".to_string());
    }
    let havok_position = i32::from_le_bytes([bytes[18], bytes[19], bytes[20], bytes[21]]);
    let tmb_offset = i32::from_le_bytes([bytes[22], bytes[23], bytes[24], bytes[25]]);
    if havok_position <= 0 {
        return Err(format!("bad havok_position {havok_position}"));
    }
    let table_offset = 26usize;
    let entries = (0..num_animations as usize)
        .map(|index| {
            let base = table_offset + index * 40;
            if base + 40 > bytes.len() {
                return Err(format!("animation table truncated at entry {index}"));
            }
            let raw_name = &bytes[base..base + 32];
            let name_end = raw_name
                .iter()
                .position(|b| *b == 0)
                .unwrap_or(raw_name.len());
            Ok(PapEntry {
                name: String::from_utf8_lossy(&raw_name[..name_end]).into_owned(),
                animation_type: u16::from_le_bytes([bytes[base + 32], bytes[base + 33]]),
                havok_index: i16::from_le_bytes([bytes[base + 34], bytes[base + 35]]),
                face: i32::from_le_bytes([
                    bytes[base + 36],
                    bytes[base + 37],
                    bytes[base + 38],
                    bytes[base + 39],
                ]) != 0,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(PapHeader {
        entries,
        havok_position: havok_position as usize,
        tmb_offset: tmb_offset as usize,
    })
}

fn pap_havok_bytes<'a>(bytes: &'a [u8], header: &PapHeader) -> Result<&'a [u8], String> {
    let end = if header.tmb_offset > header.havok_position && header.tmb_offset <= bytes.len() {
        header.tmb_offset
    } else {
        println!(
            "    (tmb_offset {} 异常，havok 数据取到文件尾, len={})",
            header.tmb_offset,
            bytes.len()
        );
        bytes.len()
    };
    let havok = &bytes[header.havok_position..end];
    if havok.len() < 8
        || u32::from_le_bytes([havok[0], havok[1], havok[2], havok[3]]) != TAGFILE_MAGIC
    {
        return Err("embedded tagfile magic missing".to_string());
    }
    Ok(havok)
}

// ---------------------------------------------------------------------------
// vendored havok 解码（包一层 catch_unwind：上游对不支持项直接 panic，这里先统计分布）
// ---------------------------------------------------------------------------

struct DecodedContainer {
    container: HavokAnimationContainer,
    /// hkaAnimationContainer.animations 数组（上游容器结构未保留，读原始对象）
    animation_objects: Vec<Arc<RefCell<HavokObject>>>,
    /// 每个 binding 的 animation 对象在 animations 数组中的下标（Arc 指针比对）
    binding_animation_indices: Vec<Option<usize>>,
}

impl DecodedContainer {
    fn animations_len(&self) -> usize {
        self.animation_objects.len()
    }
}

fn decode_tagfile_container(havok: &[u8]) -> Result<DecodedContainer, String> {
    catch_unwind(AssertUnwindSafe(|| {
        let root = HavokBinaryTagFileReader::read(havok);
        let container_obj = root.find_object_by_type("hkaAnimationContainer");

        let animation_objects = {
            let borrowed = container_obj.borrow();
            borrowed
                .get("animations")
                .as_array()
                .iter()
                .map(|value| value.as_object())
                .collect::<Vec<_>>()
        };

        let binding_animation_indices = {
            let borrowed = container_obj.borrow();
            borrowed
                .get("bindings")
                .as_array()
                .iter()
                .map(|value| {
                    let bound = value.as_object().borrow().get("animation").as_object();
                    animation_objects
                        .iter()
                        .position(|candidate| Arc::ptr_eq(candidate, &bound))
                })
                .collect::<Vec<_>>()
        };

        DecodedContainer {
            container: HavokAnimationContainer::new(container_obj),
            animation_objects,
            binding_animation_indices,
        }
    }))
    .map_err(|panic| {
        let message = panic
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| panic.downcast_ref::<&str>().map(|s| s.to_string()))
            .unwrap_or_else(|| "<non-string panic>".to_string());
        format!("havok decode panic: {message}")
    })
}

struct SkeletonSummary {
    bone_count: usize,
    skeleton_count: usize,
}

fn parse_skeleton_file(bytes: &[u8]) -> Result<SkeletonSummary, String> {
    let (_version, havok_offset, _scanned) = sklb_havok_offset(bytes)?;
    let decoded = decode_tagfile_container(&bytes[havok_offset..])?;
    if decoded.container.skeletons.is_empty() {
        return Err("container has no skeleton".to_string());
    }
    let skeleton = &decoded.container.skeletons[0];
    let bone_count = skeleton.bone_names.len();
    if bone_count == 0 {
        return Err("empty skeleton".to_string());
    }
    if skeleton.parent_indices.len() != bone_count || skeleton.reference_pose.len() != bone_count {
        return Err(format!(
            "array length mismatch: names={} parents={} poses={}",
            bone_count,
            skeleton.parent_indices.len(),
            skeleton.reference_pose.len()
        ));
    }
    for (index, name) in skeleton.bone_names.iter().enumerate() {
        if name.is_empty() {
            return Err(format!("bone {index} has empty name"));
        }
    }
    // 层级：parent 为 -1（usize 回转）或合法下标、无自环、沿父链走 N 步必到根（无环）
    for (index, &parent) in skeleton.parent_indices.iter().enumerate() {
        let parent = parent as i64;
        if parent == -1 {
            continue;
        }
        if parent < 0 || parent as usize >= bone_count {
            return Err(format!("bone {index} parent {parent} out of range"));
        }
        if parent as usize == index {
            return Err(format!("bone {index} is its own parent"));
        }
    }
    for start in 0..bone_count {
        let mut seen = BTreeSet::new();
        let mut cursor = start;
        loop {
            if !seen.insert(cursor) {
                return Err(format!("parent cycle involving bone {start}"));
            }
            let parent = skeleton.parent_indices[cursor] as i64;
            if parent == -1 {
                break;
            }
            cursor = parent as usize;
        }
    }
    // reference pose 有限值
    for (index, pose) in skeleton.reference_pose.iter().enumerate() {
        for value in pose
            .translation
            .iter()
            .chain(pose.rotation.iter())
            .chain(pose.scale.iter())
        {
            if !value.is_finite() {
                return Err(format!("bone {index} reference pose has non-finite value"));
            }
        }
    }
    Ok(SkeletonSummary {
        bone_count,
        skeleton_count: decoded.container.skeletons.len(),
    })
}

fn human_skeleton_path(race_code: u16) -> String {
    format!("chara/human/c{race_code:04}/skeleton/base/b0001/skl_c{race_code:04}b0001.sklb")
}

fn monster_skeleton_path(id: u16) -> String {
    format!("chara/monster/m{id:04}/skeleton/base/b0001/skl_m{id:04}b0001.sklb")
}

fn demihuman_skeleton_path(id: u16) -> String {
    format!("chara/demihuman/d{id:04}/skeleton/base/b0001/skl_d{id:04}b0001.sklb")
}

// ---------------------------------------------------------------------------
// 1. sklb 批量解析
// ---------------------------------------------------------------------------

enum ProbeOutcome {
    Missing,
    Ok,
    Failed(String),
}

/// 解析单个骨架并打印摘要；失败（含 havok 解码 panic）以 Failed 返回而不中断批量统计。
fn probe_skeleton(
    resource: &mut SqPackResource,
    label: &str,
    path: &str,
    distribution: &mut BTreeMap<usize, Vec<String>>,
) -> ProbeOutcome {
    let Some(bytes) = resource.read(path) else {
        return ProbeOutcome::Missing;
    };
    match parse_skeleton_file(&bytes) {
        Ok(summary) => {
            println!(
                "  {label}: bones={} (skeletons in container: {})",
                summary.bone_count, summary.skeleton_count
            );
            distribution
                .entry(summary.bone_count)
                .or_default()
                .push(label.to_string());
            ProbeOutcome::Ok
        }
        Err(reason) => ProbeOutcome::Failed(reason),
    }
}

fn scan_skeletons(
    resource: &mut SqPackResource,
    labels_paths: impl Iterator<Item = (String, String)>,
    distribution: &mut BTreeMap<usize, Vec<String>>,
    failures: &mut Vec<(String, String)>,
) -> (usize, usize) {
    let mut ok = 0usize;
    let mut missing = 0usize;
    for (label, path) in labels_paths {
        match probe_skeleton(resource, &label, &path, distribution) {
            ProbeOutcome::Ok => ok += 1,
            ProbeOutcome::Missing => missing += 1,
            ProbeOutcome::Failed(reason) => failures.push((label, reason)),
        }
    }
    (ok, missing)
}

#[test]
#[ignore = "parses skeleton files from the installed game; requires XIV_GAME_DIR"]
fn parse_skeleton_files_from_installed_game() {
    let mut resource = SqPackResource::from_existing(&game_dir());
    resource.preload_index_files();

    let mut distribution: BTreeMap<usize, Vec<String>> = BTreeMap::new();
    let mut failures: Vec<(String, String)> = Vec::new();

    println!("== 角色骨架 c0101..c1801 ==");
    let (race_ok, race_missing) = scan_skeletons(
        &mut resource,
        (1u16..=18).map(|race| {
            let race_code = race * 100 + 1;
            (format!("c{race_code:04}"), human_skeleton_path(race_code))
        }),
        &mut distribution,
        &mut failures,
    );

    println!("== 怪物骨架扫描 m0001..m0300 + m8001..m8010 ==");
    let (monster_ok, missing_monsters) = scan_skeletons(
        &mut resource,
        (1u16..=300)
            .chain(8001..=8010)
            .map(|id| (format!("m{id:04}"), monster_skeleton_path(id))),
        &mut distribution,
        &mut failures,
    );

    println!("== 半人骨架扫描 d0001..d0020 ==");
    let (demi_ok, demi_missing) = scan_skeletons(
        &mut resource,
        (1u16..=20).map(|id| (format!("d{id:04}"), demihuman_skeleton_path(id))),
        &mut distribution,
        &mut failures,
    );

    println!("== bone 数分布 ==");
    for (bone_count, labels) in &distribution {
        println!(
            "  {bone_count:4} bones: {} 个 ({})",
            labels.len(),
            labels.join(", ")
        );
    }
    println!(
        "== 汇总: race_ok={race_ok}/18 (miss {race_missing}), monster_ok={monster_ok} (miss {missing_monsters}), demi_ok={demi_ok} (miss {demi_missing}) =="
    );
    if !failures.is_empty() {
        println!("== 失败明细 ==");
        for (label, reason) in &failures {
            println!("  {label}: {reason}");
        }
    }

    assert!(race_ok >= 16, "角色骨架成功数 {race_ok} < 16");
    assert!(
        monster_ok + demi_ok >= 10,
        "怪物/半人骨架成功数 {} < 10",
        monster_ok + demi_ok
    );
    assert!(
        failures.is_empty(),
        "{} 个骨架解析失败（panic 风险点，见上明细）",
        failures.len()
    );
}

// ---------------------------------------------------------------------------
// 2. pap 名表 + 内嵌 tagfile 解码 + havok_index 语义推断
// ---------------------------------------------------------------------------

const PAP_SAMPLES: &[&str] = &[
    "chara/human/c0101/animation/a0001/bt_common/emote/joy.pap",
    "chara/human/c0101/animation/a0001/bt_common/resident/action.pap",
    "chara/monster/m0512/animation/a0001/bt_common/resident/mount.pap",
];

#[test]
#[ignore = "parses animation files from the installed game; requires XIV_GAME_DIR"]
fn parse_animation_files_from_installed_game() {
    let mut resource = SqPackResource::from_existing(&game_dir());

    for path in PAP_SAMPLES {
        println!("== {path} ==");
        let bytes = resource
            .read(path)
            .unwrap_or_else(|| panic!("读取失败: {path}"));
        let header = parse_pap_header(&bytes).expect("pap header");
        println!(
            "  名表 {} 条 (havok_position={}, tmb_offset={})",
            header.entries.len(),
            header.havok_position,
            header.tmb_offset
        );
        for (index, entry) in header.entries.iter().enumerate() {
            println!(
                "    [{index}] name={:?} type={} havok_index={} face={}",
                entry.name, entry.animation_type, entry.havok_index, entry.face
            );
        }

        let havok = pap_havok_bytes(&bytes, &header).expect("embedded tagfile");
        let decoded = decode_tagfile_container(havok).expect("decode container");

        println!(
            "  container: animations={}, bindings={}",
            decoded.animations_len(),
            decoded.container.bindings.len()
        );

        // 关键验证素材：名表数 vs animations 长度 vs bindings 长度
        assert_eq!(
            header.entries.len(),
            decoded.animations_len(),
            "名表数量应等于 container.animations 长度"
        );
        assert_eq!(
            header.entries.len(),
            decoded.container.bindings.len(),
            "名表数量应等于 container.bindings 长度"
        );

        // binding → animation 对象对应（Arc 指针比对），确认两数组是否平行
        let mut parallel = true;
        for (binding_index, mapped) in decoded.binding_animation_indices.iter().enumerate() {
            println!("    binding[{binding_index}] -> animation[{mapped:?}]");
            if *mapped != Some(binding_index) {
                parallel = false;
            }
        }

        // 每条 animation 的统计：duration / tracks / 量化分布
        let container_animation_count = decoded.animations_len();
        let mut total_rotation = [0u32; 16];
        let mut total_translation = [0u32; 4];
        let mut total_scale = [0u32; 4];
        for (anim_index, binding) in decoded.container.bindings.iter().enumerate() {
            // duration 经 trait（dyn HavokAnimation）读取
            let duration = binding.animation.duration();

            // 量化/tracks 经具体类型读取：binding.animation 由上游装箱，这里从
            // container.animations 同位对象重建具体类型（已验证二者一一对应）。
            let animation_object = decoded.animation_objects[anim_index].clone();
            let type_name = animation_object.borrow().object_type.name.clone();
            assert_eq!(
                &*type_name, "hkaSplineCompressedAnimation",
                "只支持 spline 压缩动画，其余分支上游会 panic"
            );
            let spline = HavokSplineCompressedAnimation::new(animation_object);
            let (rotation, translation, scale) = spline.quantization_histogram();
            println!(
                "    animation[{anim_index}]: duration={duration:.3}s tracks={} frames={} blocks={}",
                spline.number_of_transform_tracks(),
                spline.num_frames(),
                spline.num_blocks()
            );
            println!(
                "      rotation量化 POLAR32={} THREECOMP40={} THREECOMP48={} THREECOMP24={} STRAIGHT16={} UNCOMPRESSED={} 其它={}",
                rotation[0],
                rotation[1],
                rotation[2],
                rotation[3],
                rotation[4],
                rotation[5],
                rotation[6..].iter().sum::<u32>()
            );
            println!(
                "      translation量化 BITS8={} BITS16={} 其它={}",
                translation[0],
                translation[1],
                translation[2..].iter().sum::<u32>()
            );
            println!(
                "      scale量化 BITS8={} BITS16={} 其它={}",
                scale[0],
                scale[1],
                scale[2..].iter().sum::<u32>()
            );
            for i in 0..16 {
                total_rotation[i] += rotation[i];
            }
            for i in 0..4 {
                total_translation[i] += translation[i];
                total_scale[i] += scale[i];
            }

            // 采样输出长度 == track 数 == binding 的 transformTrackToBoneIndices 长度
            assert_eq!(
                spline.number_of_transform_tracks(),
                binding.transform_track_to_bone_indices.len(),
                "animation[{anim_index}] tracks 与 binding 骨骼映射长度不一致"
            );
        }
        println!(
            "  量化合计: rotation POLAR32={} THREECOMP40={} THREECOMP48={} THREECOMP24={} STRAIGHT16={} UNCOMPRESSED={} 其它={}",
            total_rotation[0],
            total_rotation[1],
            total_rotation[2],
            total_rotation[3],
            total_rotation[4],
            total_rotation[5],
            total_rotation[6..].iter().sum::<u32>()
        );
        println!(
            "  量化合计: translation BITS8={} BITS16={} 其它={}；scale BITS8={} BITS16={} 其它={}",
            total_translation[0],
            total_translation[1],
            total_translation[2..].iter().sum::<u32>(),
            total_scale[0],
            total_scale[1],
            total_scale[2..].iter().sum::<u32>()
        );

        // havok_index 指向推断
        let indices = header
            .entries
            .iter()
            .map(|entry| entry.havok_index)
            .collect::<Vec<_>>();
        let all_in_range = indices
            .iter()
            .all(|index| *index >= 0 && *index as usize <= container_animation_count);
        assert!(
            all_in_range,
            "havok_index 超出 animations 数组范围: {indices:?}"
        );
        println!("  havok_index 集合: {indices:?}；bindings 与 animations 数组平行: {parallel}");
    }
}

// ---------------------------------------------------------------------------
// 3. bone 窗口语义：submesh 窗口 → bone_table → 全局骨骼名 → 模型自身骨架名表
// ---------------------------------------------------------------------------

/// 读 sklb 并返回 (bone 数, 骨骼名集合)。
fn load_skeleton_name_set(resource: &mut SqPackResource, path: &str) -> (usize, BTreeSet<String>) {
    let bytes = resource
        .read(path)
        .unwrap_or_else(|| panic!("读取失败: {path}"));
    let summary = parse_skeleton_file(&bytes).expect("parse skeleton");
    let (_version, havok_offset, _) = sklb_havok_offset(&bytes).unwrap();
    let decoded = decode_tagfile_container(&bytes[havok_offset..]).unwrap();
    let names = decoded.container.skeletons[0]
        .bone_names
        .iter()
        .cloned()
        .collect();
    (summary.bone_count, names)
}

#[test]
#[ignore = "validates submesh bone windows against the model skeleton; requires XIV_GAME_DIR"]
fn validate_submesh_bone_window_links_to_skeleton_names() {
    let mut resource = SqPackResource::from_existing(&game_dir());

    let model_path = "chara/demihuman/d0001/obj/equipment/e0001/model/d0001e0001_top.mdl";
    let model_bytes = resource
        .read(model_path)
        .unwrap_or_else(|| panic!("读取失败: {model_path}"));
    let metadata = mdl_metadata_from_mdl_bytes(model_path, &model_bytes).expect("parse mdl");

    // demihuman 模型绑定的是自身骨架（skl_d0001b0001），不是 c0101；
    // c0101 名表仅用于交叉统计（骨骼命名与玩家骨架的重合度）。
    let own_sklb_path = demihuman_skeleton_path(1);
    let (own_bone_count, own_skeleton_names) =
        load_skeleton_name_set(&mut resource, &own_sklb_path);
    let human_sklb_path = human_skeleton_path(101);
    let (human_bone_count, human_skeleton_names) =
        load_skeleton_name_set(&mut resource, &human_sklb_path);
    println!(
        "模型 {}: meshes={}, bones={}, 自身骨架({}) bone 数={}, c0101 bone 数={}",
        model_path,
        metadata.meshes.len(),
        metadata.bones.len(),
        own_sklb_path,
        own_bone_count,
        human_bone_count
    );

    let mesh = &metadata.meshes[0];
    let table = mesh
        .bone_table
        .as_ref()
        .expect("mesh0 应携带 bone table（bone_table_index 非 255）");
    println!(
        "mesh0: bone_table_index={}, table bones={} (mesh submeshes={})",
        mesh.bone_table_index,
        table.bone_indices.len(),
        mesh.submeshes.len()
    );

    let mut checked_windows = 0usize;
    let mut human_overlap = 0usize;
    for submesh in &mesh.submeshes {
        let start = usize::from(submesh.bone_start_index);
        let count = usize::from(submesh.bone_count);
        assert!(
            start + count <= table.bone_indices.len(),
            "submesh#{} 窗口 [{start}, +{count}) 越出 bone_table 长度 {}",
            submesh.table_index,
            table.bone_indices.len()
        );
        for window_index in start..start + count {
            let global_index = usize::from(table.bone_indices[window_index]);
            assert!(
                global_index < metadata.bones.len(),
                "窗口索引 {window_index} 的全局骨骼下标 {global_index} 越界 (bones={})",
                metadata.bones.len()
            );
            let name = table.bone_names[window_index].as_ref().unwrap_or_else(|| {
                panic!("窗口索引 {window_index} 的骨骼名无法解析（全局下标 {global_index}）")
            });
            assert_eq!(
                metadata.bones[global_index].name.as_deref(),
                Some(name.as_str()),
                "bone_table.bone_names 与 bone_indices→bones 解析不一致"
            );
            assert!(
                own_skeleton_names.contains(name),
                "骨骼 {name:?}（全局下标 {global_index}）不在模型自身骨架名表中"
            );
            if human_skeleton_names.contains(name) {
                human_overlap += 1;
            }
            checked_windows += 1;
        }
    }
    println!(
        "mesh0 共校验 {checked_windows} 个窗口骨骼引用，其中 {human_overlap} 个名字同时存在于 c0101 骨架"
    );
    assert!(checked_windows > 0, "mesh0 没有任何 bone 窗口可校验");
}

// ---------------------------------------------------------------------------
// 4. 采样冒烟：joy.pap 首动画 t=0/500/1500ms
// ---------------------------------------------------------------------------

#[test]
#[ignore = "samples joy.pap spline animation from the installed game; requires XIV_GAME_DIR"]
fn sample_joy_animation_smoke() {
    let mut resource = SqPackResource::from_existing(&game_dir());
    let path = PAP_SAMPLES[0];
    let bytes = resource
        .read(path)
        .unwrap_or_else(|| panic!("读取失败: {path}"));
    let header = parse_pap_header(&bytes).expect("pap header");
    assert_eq!(
        header.entries.len(),
        1,
        "joy.pap 预期单动画，名表 {} 条",
        header.entries.len()
    );
    let havok = pap_havok_bytes(&bytes, &header).expect("embedded tagfile");
    let decoded = decode_tagfile_container(havok).expect("decode container");
    assert_eq!(
        decoded.container.bindings.len(),
        1,
        "joy.pap 预期单 binding"
    );

    let binding = &decoded.container.bindings[0];
    let expected_tracks = binding.transform_track_to_bone_indices.len();
    println!(
        "joy.pap: tracks={expected_tracks}, havok_index={}, blend_hint={}",
        header.entries[0].havok_index,
        match &binding.blend_hint {
            HavokAnimationBlendHint::Normal => "Normal",
            HavokAnimationBlendHint::Additive => "Additive",
        }
    );

    for time_ms in [0.0f32, 500.0, 1500.0] {
        let transforms = binding.animation.sample(time_ms);
        assert_eq!(
            transforms.len(),
            expected_tracks,
            "t={time_ms}ms 采样长度 {} != binding track 数 {expected_tracks}",
            transforms.len()
        );
        for (index, transform) in transforms.iter().enumerate() {
            for value in transform
                .translation
                .iter()
                .chain(transform.rotation.iter())
                .chain(transform.scale.iter())
            {
                assert!(
                    value.is_finite(),
                    "t={time_ms}ms track {index} 出现非有限值"
                );
            }
        }
        let first = &transforms[0];
        println!(
            "  t={time_ms:7.1}ms ok: {} tracks, track0 t={:?} r={:?} s={:?}",
            transforms.len(),
            &first.translation[..3],
            &first.rotation[..4],
            &first.scale[..3]
        );
    }
}
