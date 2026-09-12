//! PAP 骨骼动画：容器头解析、havok 关联、采样到 `SkeletonPose`。
//!
//! 语义来源（均已对安装版真实数据验证，见 `tests/native_havok_validation.rs`）：
//! - pap 固定头 26 字节（magic `pap `/version/num_animations/.../havok_position/
//!   tmb_offset）+ 每条 40 字节名表项（name[32]/type u16/havok_index i16/face i32）；
//! - 内嵌 havok 字节 = `[havok_position, tmb_offset)`（tmb_offset 异常时取到文件尾），
//!   头是 tagfile magic（0xCAB0_0D1E），根对象 `hkaAnimationContainer`；
//! - container 的 `animations` 与 `bindings` 严格平行等长，名表 `havok_index`
//!   即数组下标；`binding.transform_track_to_bone_indices` 把 transform track
//!   映射到骨架 bone 下标，`number_of_transform_tracks == binding 表长`；
//! - FFXIV 实测只有 `hkaSplineCompressedAnimation`，量化仅 POLAR32/THREECOMP40/
//!   THREECOMP48（rotation）+ BITS8/16（translation/scale）——vendored 采样器对
//!   其余量化与未知类型直接 panic，加载时经量化直方图预检把这类动画剔出
//!   （加载即保证采样路径不 panic，对齐 skeleton.rs 的“宁加载失败不炸页面”）。
//!
//! 采样时间入参是**毫秒**（vendored `sample(time_ms)` 内部除 1000 转秒）。
//! 与 skeleton 一样刻意不实现 serde——动画集随用随加载，不进 IndexedDB/快照。

use std::fmt;
use std::panic::{AssertUnwindSafe, catch_unwind};

use crate::chara_models::{CharaModelType, PackedCharaModelId};
use crate::skeleton::{BoneTransform, ModelSkeleton, SkeletonInverseBindCache, SkeletonPose};
use crate::vendor::physis_havok::animation_binding::HavokAnimationBinding;
use crate::vendor::physis_havok::object::HavokObject;
use crate::vendor::physis_havok::spline_compressed_animation::HavokSplineCompressedAnimation;
use crate::vendor::physis_havok::{HavokAnimation, HavokBinaryTagFileReader};

const TAGFILE_MAGIC: u32 = 0xCAB0_0D1E;
const PAP_HEADER_LEN: usize = 26;
const PAP_ENTRY_LEN: usize = 40;

/// 常用 emote pap 探测名（存在性探测，缺失静默跳过）。
const EMOTE_PAP_PROBES: &[&str] = &[
    "joy", "wave", "beckon", "dance", "salute", "point", "laugh", "doze",
];

/// 单条动画的展示元数据（`ModelAnimationSet.animations` 元素）。
#[derive(Clone, Debug, PartialEq)]
pub struct ModelAnimation {
    pub name: String,
    /// 时长（毫秒）；采样时间按 `[0, duration_ms]` 钳制。
    pub duration_ms: f32,
}

/// 一个（或合并多个 pap 文件的）动画集。`animations` 公开供 UI 列表；
/// 采样上下文（binding + 预解析的 track→bone 映射）私有，随加载内存存活。
pub struct ModelAnimationSet {
    pub animations: Vec<ModelAnimation>,
    /// 按 havok_index 存放的 binding 解析产物；`None` = 该 binding 构造或
    /// 量化预检失败（名表指向它的条目不进入 `animations`）。
    bindings: Vec<Option<HavokAnimationBinding>>,
    /// `bindings[i]` 的 track→bone 解析表（`None` = track 越界已丢弃）。
    track_bone_maps: Vec<Vec<Option<usize>>>,
    /// `animations[i]` 对应的 bindings 下标（= 名表 havok_index）。
    animation_bindings: Vec<usize>,
}

impl fmt::Debug for ModelAnimationSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ModelAnimationSet")
            .field("animations", &self.animations)
            .field("bindings", &self.bindings.len())
            .finish()
    }
}

impl ModelAnimationSet {
    fn empty() -> Self {
        Self {
            animations: Vec::new(),
            bindings: Vec::new(),
            track_bone_maps: Vec::new(),
            animation_bindings: Vec::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.animations.is_empty()
    }

    /// 合并另一个解析结果（多 pap 候选探测命中多个文件时累加）。
    pub fn append(&mut self, other: ModelAnimationSet) {
        let offset = self.bindings.len();
        self.animations.extend(other.animations);
        self.bindings.extend(other.bindings);
        self.track_bone_maps.extend(other.track_bone_maps);
        self.animation_bindings
            .extend(other.animation_bindings.iter().map(|index| index + offset));
    }
}

/// 动画来源模型：决定 pap 路径探测的目录模板。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnimationSourceKind {
    /// 玩家角色（chara/human）：race code（如 101 = 中原人男）。
    Character { race_code: u16 },
    /// 宠物/坐骑（chara/monster、chara/demihuman）。
    Chara { model: PackedCharaModelId },
}

/// pap 候选路径（按优先级排序）。多候选由调用方逐个探测存在性，全部
/// 缺失返回空集、不报错（动画是 best-effort 增强，不动模型加载成败）。
pub fn pap_path_candidates(kind: AnimationSourceKind) -> Vec<String> {
    match kind {
        AnimationSourceKind::Character { race_code } => {
            let base = format!("chara/human/c{race_code:04}/animation/a0001/bt_common");
            let mut paths = vec![format!("{base}/resident/action.pap")];
            for emote in EMOTE_PAP_PROBES {
                paths.push(format!("{base}/emote/{emote}.pap"));
            }
            paths
        }
        AnimationSourceKind::Chara { model } => {
            if model.model_id == 0 {
                return Vec::new();
            }
            let (category, prefix) = match model.chara_type {
                CharaModelType::Monster => ("monster", "m"),
                CharaModelType::Demihuman => ("demihuman", "d"),
            };
            let base = format!(
                "chara/{category}/{prefix}{:04}/animation/a0001/bt_common/resident",
                model.model_id
            );
            vec![format!("{base}/mount.pap"), format!("{base}/idle.pap")]
        }
    }
}

/// pap 加载/解析错误。vendored 解析器对不支持项直接 panic，对外统一经
/// `catch_unwind` 转成 `HavokPanic`（wasm 下 panic=abort，宁可加载失败不可炸页面）。
#[derive(Clone, Debug, PartialEq)]
pub enum AnimationError {
    Truncated {
        len: usize,
    },
    BadMagic,
    NegativeAnimationCount {
        count: i16,
    },
    BadHavokPosition {
        position: i32,
    },
    TableTruncated {
        entry: usize,
    },
    HavokBytesOutOfRange {
        position: usize,
        len: usize,
    },
    TagfileMagicMissing,
    HavokPanic(String),
    HavokIndexOutOfRange {
        entry: usize,
        index: i16,
        bindings: usize,
    },
    NoAnimations,
}

impl fmt::Display for AnimationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated { len } => write!(f, "pap file too short (len={len})"),
            Self::BadMagic => write!(f, "bad pap magic (expected \"pap \")"),
            Self::NegativeAnimationCount { count } => {
                write!(f, "negative num_animations {count}")
            }
            Self::BadHavokPosition { position } => {
                write!(f, "bad havok_position {position}")
            }
            Self::TableTruncated { entry } => {
                write!(f, "animation name table truncated at entry {entry}")
            }
            Self::HavokBytesOutOfRange { position, len } => {
                write!(f, "havok_position {position} out of range (file len {len})")
            }
            Self::TagfileMagicMissing => write!(f, "embedded tagfile magic missing"),
            Self::HavokPanic(message) => write!(f, "havok decode panic: {message}"),
            Self::HavokIndexOutOfRange {
                entry,
                index,
                bindings,
            } => write!(
                f,
                "name table entry {entry} havok_index {index} out of range (bindings={bindings})"
            ),
            Self::NoAnimations => write!(f, "pap has no usable animations"),
        }
    }
}

impl std::error::Error for AnimationError {}

struct PapEntry {
    name: String,
    havok_index: i16,
}

struct PapHeader {
    entries: Vec<PapEntry>,
    havok_position: usize,
    tmb_offset: usize,
}

fn parse_pap_header(bytes: &[u8]) -> Result<PapHeader, AnimationError> {
    if bytes.len() < PAP_HEADER_LEN || &bytes[0..4] != b"pap " {
        return Err(AnimationError::BadMagic);
    }
    let num_animations = i16::from_le_bytes([bytes[8], bytes[9]]);
    if num_animations < 0 {
        return Err(AnimationError::NegativeAnimationCount {
            count: num_animations,
        });
    }
    let havok_position = i32::from_le_bytes([bytes[18], bytes[19], bytes[20], bytes[21]]);
    let tmb_offset = i32::from_le_bytes([bytes[22], bytes[23], bytes[24], bytes[25]]);
    if havok_position <= 0 {
        return Err(AnimationError::BadHavokPosition {
            position: havok_position,
        });
    }
    let entries = (0..num_animations as usize)
        .map(|index| {
            let base = PAP_HEADER_LEN + index * PAP_ENTRY_LEN;
            if base + PAP_ENTRY_LEN > bytes.len() {
                return Err(AnimationError::TableTruncated { entry: index });
            }
            let raw_name = &bytes[base..base + 32];
            let name_end = raw_name
                .iter()
                .position(|b| *b == 0)
                .unwrap_or(raw_name.len());
            Ok(PapEntry {
                name: String::from_utf8_lossy(&raw_name[..name_end]).into_owned(),
                havok_index: i16::from_le_bytes([bytes[base + 34], bytes[base + 35]]),
            })
        })
        .collect::<Result<Vec<_>, AnimationError>>()?;
    Ok(PapHeader {
        entries,
        havok_position: havok_position as usize,
        tmb_offset: tmb_offset as usize,
    })
}

/// 内嵌 havok 字节：取 `[havok_position, tmb_offset)`，tmb_offset 异常
/// （不大于 havok 起点或越出文件）时取到文件尾。
fn pap_havok_bytes<'a>(bytes: &'a [u8], header: &PapHeader) -> Result<&'a [u8], AnimationError> {
    if header.havok_position >= bytes.len() {
        return Err(AnimationError::HavokBytesOutOfRange {
            position: header.havok_position,
            len: bytes.len(),
        });
    }
    let end = if header.tmb_offset > header.havok_position && header.tmb_offset <= bytes.len() {
        header.tmb_offset
    } else {
        bytes.len()
    };
    let havok = &bytes[header.havok_position..end];
    if havok.len() < 8
        || u32::from_le_bytes([havok[0], havok[1], havok[2], havok[3]]) != TAGFILE_MAGIC
    {
        return Err(AnimationError::TagfileMagicMissing);
    }
    Ok(havok)
}

struct DecodedBinding {
    binding: HavokAnimationBinding,
    track_bone_map: Vec<Option<usize>>,
    dropped_tracks: usize,
}

/// 单条 binding 的构造 + 预检（独立 catch_unwind：单条失败只丢该动画）。
/// 预检项：spline 量化直方图（采样路径已验证的 panic 点）、duration 有限
/// 正值、tracks 与 bone 映射表等长、track→bone 越界统计。
fn decode_binding(
    object: std::sync::Arc<std::cell::RefCell<HavokObject>>,
    skeleton: &ModelSkeleton,
) -> Option<DecodedBinding> {
    let result = catch_unwind(AssertUnwindSafe(|| {
        // 用 binding 自己的 animation 成员做量化预检（与构造共享同一对象，
        // 预检实例随后丢弃，binding 内部重建一份）。
        let animation_object = object.borrow().get("animation").as_object();
        if &*animation_object.borrow().object_type.name != "hkaSplineCompressedAnimation" {
            return Err("unsupported animation type".to_string());
        }
        let spline = HavokSplineCompressedAnimation::new(animation_object);
        let (rotation, translation, scale) = spline.quantization_histogram();
        if rotation[3..].iter().any(|count| *count != 0)
            || translation[2..].iter().any(|count| *count != 0)
            || scale[2..].iter().any(|count| *count != 0)
        {
            return Err("unsupported spline quantization".to_string());
        }
        let duration_ms = spline.duration() * 1000.0;
        if !duration_ms.is_finite() || duration_ms <= 0.0 {
            return Err(format!("bad duration {duration_ms}ms"));
        }
        let binding = HavokAnimationBinding::new(object.clone());
        if binding.transform_track_to_bone_indices.len() != spline.number_of_transform_tracks() {
            return Err(format!(
                "track/bone mapping length {} != transform tracks {}",
                binding.transform_track_to_bone_indices.len(),
                spline.number_of_transform_tracks()
            ));
        }
        let mut dropped_tracks = 0usize;
        let track_bone_map = binding
            .transform_track_to_bone_indices
            .iter()
            .map(|&bone| {
                let bone = bone as usize;
                if bone < skeleton.bone_count() {
                    Some(bone)
                } else {
                    dropped_tracks += 1;
                    None
                }
            })
            .collect();
        Ok(DecodedBinding {
            binding,
            track_bone_map,
            dropped_tracks,
        })
    }));
    match result {
        Ok(Ok(decoded)) => {
            if decoded.dropped_tracks > 0 {
                eprintln!(
                    "animation: {} tracks dropped (bone index out of skeleton range)",
                    decoded.dropped_tracks
                );
            }
            Some(decoded)
        }
        Ok(Err(reason)) => {
            eprintln!("animation: binding skipped: {reason}");
            None
        }
        Err(panic) => {
            let message = panic
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| panic.downcast_ref::<&str>().map(|s| (*s).to_string()))
                .unwrap_or_else(|| "<non-string panic>".to_string());
            eprintln!("animation: binding skipped: {message}");
            None
        }
    }
}

/// 解析 pap 字节为动画集（按 `skeleton` 预解析 track→bone 映射）。
/// 结构错误（magic/表截断/无 tagfile/havok 解码 panic）返回 Err；
/// 单个 binding 失败只丢对应动画，全部不可用时返回 `NoAnimations`。
pub fn load_animation_set_from_pap_bytes(
    bytes: &[u8],
    skeleton: &ModelSkeleton,
) -> Result<ModelAnimationSet, AnimationError> {
    if bytes.len() < 4 {
        return Err(AnimationError::Truncated { len: bytes.len() });
    }
    let header = parse_pap_header(bytes)?;
    let havok = pap_havok_bytes(bytes, &header)?;
    let decoded = catch_unwind(AssertUnwindSafe(|| {
        let root = HavokBinaryTagFileReader::read(havok);
        let container_obj = root.find_object_by_type("hkaAnimationContainer");
        let raw_bindings: Vec<_> = container_obj
            .borrow()
            .get("bindings")
            .as_array()
            .iter()
            .map(|value| value.as_object())
            .collect();
        raw_bindings
            .iter()
            .map(|object| decode_binding(object.clone(), skeleton))
            .collect::<Vec<_>>()
    }))
    .map_err(|panic| {
        AnimationError::HavokPanic(
            panic
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| panic.downcast_ref::<&str>().map(|s| (*s).to_string()))
                .unwrap_or_else(|| "<non-string panic>".to_string()),
        )
    })?;

    let mut set = ModelAnimationSet::empty();
    set.bindings = Vec::with_capacity(decoded.len());
    set.track_bone_maps = Vec::with_capacity(decoded.len());
    for decoded_binding in decoded {
        match decoded_binding {
            Some(decoded_binding) => {
                set.bindings.push(Some(decoded_binding.binding));
                set.track_bone_maps.push(decoded_binding.track_bone_map);
            }
            None => {
                set.bindings.push(None);
                set.track_bone_maps.push(Vec::new());
            }
        }
    }

    for (entry_index, entry) in header.entries.iter().enumerate() {
        if entry.havok_index < 0 || entry.havok_index as usize >= set.bindings.len() {
            return Err(AnimationError::HavokIndexOutOfRange {
                entry: entry_index,
                index: entry.havok_index,
                bindings: set.bindings.len(),
            });
        }
        let binding_index = entry.havok_index as usize;
        if set.bindings[binding_index].is_none() {
            eprintln!(
                "animation: name table entry {entry_index} ({:?}) skipped (binding unusable)",
                entry.name
            );
            continue;
        }
        let duration_ms = set.bindings[binding_index]
            .as_ref()
            .map(|binding| binding.animation.duration() * 1000.0)
            .unwrap_or_default();
        set.animations.push(ModelAnimation {
            name: entry.name.clone(),
            duration_ms,
        });
        set.animation_bindings.push(binding_index);
    }

    if set.animations.is_empty() {
        return Err(AnimationError::NoAnimations);
    }
    Ok(set)
}

/// 采样动画 `index` 在 `time_ms` 的局部 TRS 覆盖（未覆盖骨骼保持 rest）。
/// 时间越界按 `[0, duration_ms]` 钳制；未知 index 返回 rest pose。
pub fn sample_animation_pose(
    set: &ModelAnimationSet,
    index: usize,
    time_ms: f32,
    skeleton: &ModelSkeleton,
) -> SkeletonPose {
    let mut pose = SkeletonPose::rest_pose(skeleton);
    let (Some(animation), Some(&binding_index)) = (
        set.animations.get(index),
        set.animation_bindings.get(index),
    ) else {
        return pose;
    };
    let (Some(Some(binding)), Some(map)) = (
        set.bindings.get(binding_index).map(Option::as_ref),
        set.track_bone_maps.get(binding_index),
    ) else {
        return pose;
    };
    let clamped = time_ms.clamp(0.0, animation.duration_ms.max(0.0));
    let sampled = binding.animation.sample(clamped);
    for (track, transform) in sampled.iter().enumerate() {
        let Some(&Some(bone)) = map.get(track) else {
            continue;
        };
        let _ = pose.set_transform(
            bone,
            BoneTransform {
                translation: [
                    transform.translation[0],
                    transform.translation[1],
                    transform.translation[2],
                ],
                rotation: [
                    transform.rotation[0],
                    transform.rotation[1],
                    transform.rotation[2],
                    transform.rotation[3],
                ],
                scale: [transform.scale[0], transform.scale[1], transform.scale[2]],
            },
        );
    }
    pose
}

/// `sample_animation_pose` + 关节矩阵直出（网页/测试播放驱动的便捷封装；
/// 高频调用方共享一个 `SkeletonInverseBindCache` 缓存 inverse bind）。
pub fn animation_joint_matrices(
    set: &ModelAnimationSet,
    index: usize,
    time_ms: f32,
    skeleton: &ModelSkeleton,
    joint_names: &[String],
    inverse_bind: &mut SkeletonInverseBindCache,
) -> Vec<[f32; 16]> {
    let pose = sample_animation_pose(set, index, time_ms, skeleton);
    inverse_bind.joint_matrices(skeleton, &pose, joint_names)
}

/// 按来源探测全部 pap 候选并合并为一个动画集。逐个候选读取，缺失/
/// 解析失败的候选跳过；全部落空返回空集（`animations` 为空），不报错——
/// 动画是 best-effort 增强，不动模型加载成败。
#[cfg(feature = "game-data")]
pub async fn load_animation_set_from_async_resource<R: crate::AsyncGameResource>(
    resource: &mut R,
    kind: AnimationSourceKind,
    skeleton: &ModelSkeleton,
) -> ModelAnimationSet {
    let mut merged = ModelAnimationSet::empty();
    for path in pap_path_candidates(kind) {
        let bytes = match resource.read(&path).await {
            Ok(bytes) => bytes,
            Err(_) => continue,
        };
        match load_animation_set_from_pap_bytes(&bytes, skeleton) {
            Ok(set) => merged.append(set),
            Err(error) => eprintln!("animation: {path}: {error}"),
        }
    }
    merged
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skeleton::{BoneTransform, IDENTITY_MAT4};

    fn test_skeleton() -> ModelSkeleton {
        ModelSkeleton {
            bone_names: vec!["root".to_string(), "spine".to_string()],
            parent_indices: vec![-1, 0],
            rest_pose: vec![
                BoneTransform {
                    translation: [0.0, 0.0, 0.0],
                    rotation: [0.0, 0.0, 0.0, 1.0],
                    scale: [1.0; 3],
                },
                BoneTransform {
                    translation: [0.0, 1.0, 0.0],
                    rotation: [0.0, 0.0, 0.0, 1.0],
                    scale: [1.0; 3],
                },
            ],
        }
    }

    /// 结构合法的最小 pap：正确 magic/头/havok_position 指向 tagfile magic
    /// 的垃圾（vendored reader 对其 panic → HavokPanic）。名表 1 条。
    fn structurally_valid_pap_with_garbage_havok() -> Vec<u8> {
        let mut bytes = b"pap ".to_vec();
        bytes.extend_from_slice(&1i32.to_le_bytes()); // version
        bytes.extend_from_slice(&1i16.to_le_bytes()); // num_animations
        bytes.extend_from_slice(&[0u8; 8]); // model_id/type/variant/info_offset
        bytes.extend_from_slice(&(26i32 + 40).to_le_bytes()); // havok_position（名表之后）
        bytes.extend_from_slice(&0i32.to_le_bytes()); // tmb_offset（异常 → 取到文件尾）
        let mut name = [0u8; 32];
        name[..3].copy_from_slice(b"joy");
        bytes.extend_from_slice(&name); // name[32]
        bytes.extend_from_slice(&0u16.to_le_bytes()); // type
        bytes.extend_from_slice(&0i16.to_le_bytes()); // havok_index
        bytes.extend_from_slice(&0i32.to_le_bytes()); // face
        bytes.extend_from_slice(&0xCAB0_0D1Eu32.to_le_bytes()); // tagfile magic
        bytes.extend_from_slice(&[0xFFu8; 128]); // 垃圾 tagfile 数据
        bytes
    }

    #[test]
    fn load_rejects_garbage_bytes_without_panic() {
        let skeleton = test_skeleton();
        assert!(matches!(
            load_animation_set_from_pap_bytes(&[], &skeleton),
            Err(AnimationError::Truncated { len: 0 })
        ));
        assert!(matches!(
            load_animation_set_from_pap_bytes(b"not a pap file....", &skeleton),
            Err(AnimationError::BadMagic)
        ));
        // 结构合法 + 垃圾 havok：vendored reader panic 必须包装为 HavokPanic。
        match load_animation_set_from_pap_bytes(
            &structurally_valid_pap_with_garbage_havok(),
            &skeleton,
        ) {
            Err(AnimationError::HavokPanic(_)) => {}
            other => panic!("expected HavokPanic, got {other:?}"),
        }
    }

    #[test]
    fn load_rejects_bad_header_fields() {
        let skeleton = test_skeleton();
        let mut bytes = structurally_valid_pap_with_garbage_havok();
        bytes[8] = 0xFF;
        bytes[9] = 0xFF; // num_animations = -1
        assert!(matches!(
            load_animation_set_from_pap_bytes(&bytes, &skeleton),
            Err(AnimationError::NegativeAnimationCount { count: -1 })
        ));

        let mut bytes = structurally_valid_pap_with_garbage_havok();
        bytes[18] = 0;
        bytes[19] = 0;
        bytes[20] = 0;
        bytes[21] = 0; // havok_position = 0
        assert!(matches!(
            load_animation_set_from_pap_bytes(&bytes, &skeleton),
            Err(AnimationError::BadHavokPosition { position: 0 })
        ));

        let mut bytes = structurally_valid_pap_with_garbage_havok();
        bytes[18] = 0xFF;
        bytes[19] = 0xFF;
        bytes[20] = 0xFF;
        bytes[21] = 0x7F; // havok_position = i32::MAX
        let len = bytes.len();
        let result = load_animation_set_from_pap_bytes(&bytes, &skeleton);
        assert!(
            matches!(
                result,
                Err(AnimationError::HavokBytesOutOfRange {
                    position,
                    len: range_len,
                }) if position == i32::MAX as usize && range_len == len
            ),
            "got {result:?}"
        );

        let mut bytes = structurally_valid_pap_with_garbage_havok();
        bytes.truncate(26 + 20); // 名表截断（40 字节项只留 20）
        assert!(matches!(
            load_animation_set_from_pap_bytes(&bytes, &skeleton),
            Err(AnimationError::TableTruncated { entry: 0 })
        ));
    }

    #[test]
    fn load_rejects_missing_tagfile_magic() {
        let skeleton = test_skeleton();
        let mut bytes = structurally_valid_pap_with_garbage_havok();
        let tagfile_at = 26 + 40;
        bytes[tagfile_at] = 0; // 抹掉 tagfile magic
        assert!(matches!(
            load_animation_set_from_pap_bytes(&bytes, &skeleton),
            Err(AnimationError::TagfileMagicMissing)
        ));
    }

    #[test]
    fn sample_unknown_index_returns_rest_pose() {
        let skeleton = test_skeleton();
        // 无动画集可用的场景：sample 对未知 index 返回 rest，不 panic。
        // （空集只能经内部构造；这里用未知 index 路径验证防御性回退。）
        let mut set = ModelAnimationSet::empty();
        set.animations.push(ModelAnimation {
            name: "dummy".to_string(),
            duration_ms: 1000.0,
        });
        // animation_bindings 为空 → index 0 也回退 rest。
        let pose = sample_animation_pose(&set, 0, 500.0, &skeleton);
        assert_eq!(pose, SkeletonPose::rest_pose(&skeleton));
        assert_eq!(pose.bone_count(), 2);
    }

    #[test]
    fn animation_joint_matrices_falls_back_to_rest() {
        let skeleton = test_skeleton();
        let mut set = ModelAnimationSet::empty();
        set.animations.push(ModelAnimation {
            name: "dummy".to_string(),
            duration_ms: 1000.0,
        });
        let joint_names = vec!["root".to_string(), "spine".to_string()];
        let mut cache = SkeletonInverseBindCache::new();
        let matrices = animation_joint_matrices(&set, 0, 500.0, &skeleton, &joint_names, &mut cache);
        assert_eq!(matrices.len(), 2);
        // rest pose ⇒ 恒等关节矩阵。
        for matrix in &matrices {
            assert!(
                matrix
                    .iter()
                    .zip(IDENTITY_MAT4.iter())
                    .all(|(a, b)| (a - b).abs() <= 1e-5),
                "rest joint matrix must be identity"
            );
        }
    }

    #[test]
    fn pap_path_candidates_follow_model_type() {
        let character = pap_path_candidates(AnimationSourceKind::Character { race_code: 101 });
        assert_eq!(
            character[0],
            "chara/human/c0101/animation/a0001/bt_common/resident/action.pap"
        );
        assert!(character
            .iter()
            .any(|path| path.ends_with("/emote/joy.pap")));
        assert!(character
            .iter()
            .any(|path| path.ends_with("/emote/wave.pap")));

        let monster = pap_path_candidates(AnimationSourceKind::Chara {
            model: PackedCharaModelId {
                model_id: 512,
                base_id: 1,
                variant_id: 1,
                chara_type: CharaModelType::Monster,
            },
        });
        assert_eq!(
            monster,
            [
                "chara/monster/m0512/animation/a0001/bt_common/resident/mount.pap",
                "chara/monster/m0512/animation/a0001/bt_common/resident/idle.pap",
            ]
        );

        let demihuman = pap_path_candidates(AnimationSourceKind::Chara {
            model: PackedCharaModelId {
                model_id: 1,
                base_id: 1,
                variant_id: 1,
                chara_type: CharaModelType::Demihuman,
            },
        });
        assert_eq!(
            demihuman[0],
            "chara/demihuman/d0001/animation/a0001/bt_common/resident/mount.pap"
        );

        assert!(pap_path_candidates(AnimationSourceKind::Chara {
            model: PackedCharaModelId {
                model_id: 0,
                base_id: 1,
                variant_id: 1,
                chara_type: CharaModelType::Monster,
            },
        })
        .is_empty());
    }

    #[cfg(feature = "game-data")]
    #[test]
    fn async_resource_probe_merges_candidates_and_tolerates_missing() {
        use std::collections::HashMap;
        use std::pin::Pin;

        struct MockResource {
            files: HashMap<String, Vec<u8>>,
            reads: Vec<String>,
        }

        impl crate::AsyncGameResource for MockResource {
            type Error = String;
            type ReadFuture<'a>
                = Pin<Box<dyn std::future::Future<Output = Result<Vec<u8>, String>> + 'a>>
            where
                Self: 'a;

            fn read<'a>(&'a mut self, path: &'a str) -> Self::ReadFuture<'a> {
                self.reads.push(path.to_string());
                let result = self.files.get(path).cloned().ok_or_else(|| "missing".to_string());
                Box::pin(async move { result })
            }

            fn platform(&self) -> physis::Platform {
                physis::Platform::Win32
            }
        }

        let skeleton = test_skeleton();
        // 只放 mount.pap（垃圾字节 → 解析失败但探测继续）；idle 缺失。
        let mut files = HashMap::new();
        files.insert(
            "chara/monster/m0512/animation/a0001/bt_common/resident/mount.pap".to_string(),
            b"garbage".to_vec(),
        );
        let mut resource = MockResource {
            files,
            reads: Vec::new(),
        };
        let set = futures_executor::block_on(load_animation_set_from_async_resource(
            &mut resource,
            AnimationSourceKind::Chara {
                model: PackedCharaModelId {
                    model_id: 512,
                    base_id: 1,
                    variant_id: 1,
                    chara_type: CharaModelType::Monster,
                },
            },
            &skeleton,
        ));
        assert!(set.is_empty(), "garbage pap 不应产生动画");
        assert_eq!(resource.reads.len(), 2, "两个候选都探测");

        // 全部缺失 → 空集、无 panic、零读取错误。
        let mut resource = MockResource {
            files: HashMap::new(),
            reads: Vec::new(),
        };
        let set = futures_executor::block_on(load_animation_set_from_async_resource(
            &mut resource,
            AnimationSourceKind::Character { race_code: 999 },
            &skeleton,
        ));
        assert!(set.is_empty());
        assert_eq!(resource.reads.len(), 1 + EMOTE_PAP_PROBES.len());
    }
}
