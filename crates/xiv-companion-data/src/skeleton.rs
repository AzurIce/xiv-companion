//! 骨骼 rest pose 与 GPU 蒙皮的数据层基础。
//!
//! 骨骼来源是 sklb（Havok tagfile，`hkaAnimationContainer.skeletons[0]`，经
//! vendored `physis_havok` 解析）；蒙皮负载来自 MDL 顶点 blend weights/indices
//! 与 mesh bone_table（见 `crate::model`）。本模块只做纯数据与纯数学：路径模板、
//! 容器头解析、rest pose、世界/关节矩阵。刻意不实现 serde——骨架随用随加载，
//! 不进 IndexedDB/快照（体积与版本问题），渲染 API 以参数传入。
//!
//! 矩阵一律为列主序 `[f32; 16]`（下标 = col*4+row），与 wgpu/WGSL `mat4x4`
//! 一致。四元数为 xyzw 顺序（Havok `hkQuaternion` 布局）。

use std::collections::HashSet;
use std::fmt;
use std::panic::{AssertUnwindSafe, catch_unwind};

use crate::chara_models::{CharaModelType, PackedCharaModelId};
use crate::vendor::physis_havok::{HavokAnimationContainer, HavokBinaryTagFileReader};

const TAGFILE_MAGIC: u32 = 0xCAB0_0D1E;

/// 单位列主序 mat4。
pub const IDENTITY_MAT4: [f32; 16] = [
    1.0, 0.0, 0.0, 0.0, //
    0.0, 1.0, 0.0, 0.0, //
    0.0, 0.0, 1.0, 0.0, //
    0.0, 0.0, 0.0, 1.0,
];

/// 单骨骼局部变换（rest pose 或 pose 覆盖值），T/R/S 分解。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoneTransform {
    pub translation: [f32; 3],
    /// xyzw 四元数；非单位长度输入在矩阵化时按恒等旋转回退。
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
}

impl BoneTransform {
    pub const IDENTITY: Self = Self {
        translation: [0.0; 3],
        rotation: [0.0, 0.0, 0.0, 1.0],
        scale: [1.0; 3],
    };
}

/// 骨骼 rest pose：bone 名、父索引（-1=根）、每骨局部 TRS。
#[derive(Clone, Debug, PartialEq)]
pub struct ModelSkeleton {
    pub bone_names: Vec<String>,
    pub parent_indices: Vec<i32>,
    pub rest_pose: Vec<BoneTransform>,
    /// 体型缩放（RGSP，见 `crate::racial_scaling`）：角色装配加载时按
    /// human.cmp 缩放参数表 + 捏脸字节（身高/尾长/胸围）附着；None = 恒等。
    /// 只作用于**展示姿势**（[`ModelSkeleton::scaled_rest_pose`] 与
    /// 动画采样输出的合成），不参与 inverse bind / 种族骨变形烘焙
    /// （两者都以原始 rest pose 为准）。
    pub body_scaling: Option<crate::racial_scaling::BodyScaling>,
}

impl ModelSkeleton {
    pub fn bone_count(&self) -> usize {
        self.bone_names.len()
    }

    pub fn bone_index(&self, name: &str) -> Option<usize> {
        self.bone_names.iter().position(|bone| bone == name)
    }

    /// 展示用 rest pose：rest 合成体型缩放（无缩放时与原 rest 逐骨一致）。
    /// 绑定语义（inverse bind、race deform 烘焙）请用原始 rest pose
    /// （[`SkeletonPose::rest_pose`]）。
    pub fn scaled_rest_pose(&self) -> SkeletonPose {
        let mut pose = SkeletonPose::rest_pose(self);
        if let Some(scaling) = self.body_scaling {
            pose.apply_body_scaling(self, &scaling);
        }
        pose
    }
}

/// 每骨骼局部 TRS 覆盖集合。`rest_pose` 构造的 pose 即 rest；
/// `set_*` 修改单个骨骼。长度与骨架不一致时 `world_matrices` 对缺失骨骼
/// 回退 rest、忽略多余项（防御，不 panic）。
#[derive(Clone, Debug, PartialEq)]
pub struct SkeletonPose {
    transforms: Vec<BoneTransform>,
}

impl SkeletonPose {
    /// 全恒等局部变换的 pose（与任何具体骨架无关，按需再覆盖）。
    pub fn new(bone_count: usize) -> Self {
        Self {
            transforms: vec![BoneTransform::IDENTITY; bone_count],
        }
    }

    /// 拷贝骨架 rest pose 的 pose（默认=rest）。
    pub fn rest_pose(skeleton: &ModelSkeleton) -> Self {
        Self {
            transforms: skeleton.rest_pose.clone(),
        }
    }

    pub fn bone_count(&self) -> usize {
        self.transforms.len()
    }

    pub fn transform(&self, bone: usize) -> Option<BoneTransform> {
        self.transforms.get(bone).copied()
    }

    pub fn set_transform(
        &mut self,
        bone: usize,
        transform: BoneTransform,
    ) -> Result<(), SkeletonError> {
        let Some(slot) = self.transforms.get_mut(bone) else {
            return Err(SkeletonError::BoneIndexOutOfRange {
                bone,
                count: self.transforms.len(),
            });
        };
        *slot = transform;
        Ok(())
    }

    pub fn set_rotation(&mut self, bone: usize, rotation: [f32; 4]) -> Result<(), SkeletonError> {
        let Some(slot) = self.transforms.get_mut(bone) else {
            return Err(SkeletonError::BoneIndexOutOfRange {
                bone,
                count: self.transforms.len(),
            });
        };
        slot.rotation = rotation;
        Ok(())
    }

    pub fn set_translation(
        &mut self,
        bone: usize,
        translation: [f32; 3],
    ) -> Result<(), SkeletonError> {
        let Some(slot) = self.transforms.get_mut(bone) else {
            return Err(SkeletonError::BoneIndexOutOfRange {
                bone,
                count: self.transforms.len(),
            });
        };
        slot.translation = translation;
        Ok(())
    }

    fn local_transform(&self, bone: usize, rest: &BoneTransform) -> BoneTransform {
        self.transforms.get(bone).copied().unwrap_or(*rest)
    }

    /// 把体型缩放（RGSP）合成进姿势：身高 × `n_root` 局部 scale（全模型关于
    /// 原点均匀缩放）；尾长 × 尾链根局部 scale（`n_sippo_*` 链中父骨不属尾链
    /// 的第一根，其后代随层级继承）；胸围逐轴 × `j_mune_l`/`j_mune_r` 局部
    /// scale。对 rest 或动画采样得到的原始姿势调用一次；**不幂等**，重复
    /// 调用会重复累积。目标骨缺失时跳过该项（不报错）。
    pub fn apply_body_scaling(
        &mut self,
        skeleton: &ModelSkeleton,
        scaling: &crate::racial_scaling::BodyScaling,
    ) {
        use crate::racial_scaling::{BUST_BONES, HEIGHT_BONE, TAIL_BONE_PREFIX};
        if scaling.is_identity() {
            return;
        }
        let scale_bone = |transforms: &mut Vec<BoneTransform>, bone: usize, factor: [f32; 3]| {
            if let Some(slot) = transforms.get_mut(bone) {
                for (axis, value) in slot.scale.iter_mut().enumerate() {
                    *value *= factor[axis];
                }
            }
        };
        if scaling.height != 1.0
            && let Some(bone) = skeleton.bone_index(HEIGHT_BONE)
        {
            scale_bone(&mut self.transforms, bone, [scaling.height; 3]);
        }
        if scaling.tail != 1.0 {
            // 尾链根：n_sippo_* 中父骨不是尾骨的第一根（一般为 n_sippo_a）。
            let chain_root = (0..skeleton.bone_count()).find(|&bone| {
                if !skeleton.bone_names[bone].starts_with(TAIL_BONE_PREFIX) {
                    return false;
                }
                let parent = skeleton.parent_indices[bone];
                parent < 0 || !skeleton.bone_names[parent as usize].starts_with(TAIL_BONE_PREFIX)
            });
            if let Some(bone) = chain_root {
                scale_bone(&mut self.transforms, bone, [scaling.tail; 3]);
            }
        }
        if scaling.bust != [1.0; 3] {
            for name in BUST_BONES {
                if let Some(bone) = skeleton.bone_index(name) {
                    scale_bone(&mut self.transforms, bone, scaling.bust);
                }
            }
        }
    }
}

/// 单位轴角 → xyzw 四元数（pose 覆盖的构造辅助）。
pub fn quat_from_axis_angle(axis: [f32; 3], angle: f32) -> [f32; 4] {
    let length = (axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2]).sqrt();
    if !length.is_finite() || length <= f32::EPSILON {
        return [0.0, 0.0, 0.0, 1.0];
    }
    let half = angle * 0.5;
    let (sin, cos) = half.sin_cos();
    let scale = sin / length;
    [axis[0] * scale, axis[1] * scale, axis[2] * scale, cos]
}

/// TRS → 列主序 mat4（M = T·R·S）。
pub fn trs_to_mat4(translation: [f32; 3], rotation: [f32; 4], scale: [f32; 3]) -> [f32; 16] {
    let [x, y, z, w] = rotation;
    let norm = x * x + y * y + z * z + w * w;
    let (x, y, z, w) = if norm.is_finite() && norm > f32::EPSILON {
        let inv = norm.sqrt().recip();
        (x * inv, y * inv, z * inv, w * inv)
    } else {
        return [
            scale[0],
            0.0,
            0.0,
            0.0, //
            0.0,
            scale[1],
            0.0,
            0.0, //
            0.0,
            0.0,
            scale[2],
            0.0, //
            translation[0],
            translation[1],
            translation[2],
            1.0,
        ];
    };
    let xx = x * x;
    let yy = y * y;
    let zz = z * z;
    let xy = x * y;
    let xz = x * z;
    let yz = y * z;
    let wx = w * x;
    let wy = w * y;
    let wz = w * z;
    // R（行主序写出，按列填入）
    let r00 = 1.0 - 2.0 * (yy + zz);
    let r01 = 2.0 * (xy - wz);
    let r02 = 2.0 * (xz + wy);
    let r10 = 2.0 * (xy + wz);
    let r11 = 1.0 - 2.0 * (xx + zz);
    let r12 = 2.0 * (yz - wx);
    let r20 = 2.0 * (xz - wy);
    let r21 = 2.0 * (yz + wx);
    let r22 = 1.0 - 2.0 * (xx + yy);
    [
        r00 * scale[0],
        r10 * scale[0],
        r20 * scale[0],
        0.0,
        r01 * scale[1],
        r11 * scale[1],
        r21 * scale[1],
        0.0,
        r02 * scale[2],
        r12 * scale[2],
        r22 * scale[2],
        0.0,
        translation[0],
        translation[1],
        translation[2],
        1.0,
    ]
}

/// 列主序 mat4 乘法 C = A·B。
pub fn mat4_mul(a: [f32; 16], b: [f32; 16]) -> [f32; 16] {
    let mut out = [0.0f32; 16];
    for col in 0..4 {
        for row in 0..4 {
            let mut sum = 0.0f32;
            for k in 0..4 {
                sum += a[k * 4 + row] * b[col * 4 + k];
            }
            out[col * 4 + row] = sum;
        }
    }
    out
}

/// 一般 4x4 逆（伴随矩阵/行列式法）。仿射矩阵的数值误差 ~1e-7 相对量级。
pub fn mat4_inverse(m: [f32; 16]) -> [f32; 16] {
    let m00 = m[0];
    let m10 = m[1];
    let m20 = m[2];
    let m30 = m[3];
    let m01 = m[4];
    let m11 = m[5];
    let m21 = m[6];
    let m31 = m[7];
    let m02 = m[8];
    let m12 = m[9];
    let m22 = m[10];
    let m32 = m[11];
    let m03 = m[12];
    let m13 = m[13];
    let m23 = m[14];
    let m33 = m[15];

    let c00 =
        m11 * m22 * m33 - m11 * m23 * m32 - m21 * m12 * m33 + m21 * m13 * m32 + m31 * m12 * m23
            - m31 * m13 * m22;
    let c01 =
        -m01 * m22 * m33 + m01 * m23 * m32 + m21 * m02 * m33 - m21 * m03 * m32 - m31 * m02 * m23
            + m31 * m03 * m22;
    let c02 =
        m01 * m12 * m33 - m01 * m13 * m32 - m11 * m02 * m33 + m11 * m03 * m32 + m31 * m02 * m13
            - m31 * m03 * m12;
    let c03 =
        -m01 * m12 * m23 + m01 * m13 * m22 + m11 * m02 * m23 - m11 * m03 * m22 - m21 * m02 * m13
            + m21 * m03 * m12;
    let c10 =
        -m10 * m22 * m33 + m10 * m23 * m32 + m20 * m12 * m33 - m20 * m13 * m32 - m30 * m12 * m23
            + m30 * m13 * m22;
    let c11 =
        m00 * m22 * m33 - m00 * m23 * m32 - m20 * m02 * m33 + m20 * m03 * m32 + m30 * m02 * m23
            - m30 * m03 * m22;
    let c12 =
        -m00 * m12 * m33 + m00 * m13 * m32 + m10 * m02 * m33 - m10 * m03 * m32 - m30 * m02 * m13
            + m30 * m03 * m12;
    let c13 =
        m00 * m12 * m23 - m00 * m13 * m22 - m10 * m02 * m23 + m10 * m03 * m22 + m20 * m02 * m13
            - m20 * m03 * m12;
    let c20 =
        m10 * m21 * m33 - m10 * m23 * m31 - m20 * m11 * m33 + m20 * m13 * m31 + m30 * m11 * m23
            - m30 * m13 * m21;
    let c21 =
        -m00 * m21 * m33 + m00 * m23 * m31 + m20 * m01 * m33 - m20 * m03 * m31 - m30 * m01 * m23
            + m30 * m03 * m21;
    let c22 =
        m00 * m11 * m33 - m00 * m13 * m31 - m10 * m01 * m33 + m10 * m03 * m31 + m30 * m01 * m13
            - m30 * m03 * m11;
    let c23 =
        -m00 * m11 * m23 + m00 * m13 * m21 + m10 * m01 * m23 - m10 * m03 * m21 - m20 * m01 * m13
            + m20 * m03 * m11;
    let c30 =
        -m10 * m21 * m32 + m10 * m22 * m31 + m20 * m11 * m32 - m20 * m12 * m31 - m30 * m11 * m22
            + m30 * m12 * m21;
    let c31 =
        m00 * m21 * m32 - m00 * m22 * m31 - m20 * m01 * m32 + m20 * m02 * m31 + m30 * m01 * m22
            - m30 * m02 * m21;
    let c32 =
        -m00 * m11 * m32 + m00 * m12 * m31 + m10 * m01 * m32 - m10 * m02 * m31 - m30 * m01 * m12
            + m30 * m02 * m11;
    let c33 =
        m00 * m11 * m22 - m00 * m12 * m21 - m10 * m01 * m22 + m10 * m02 * m21 + m20 * m01 * m12
            - m20 * m02 * m11;

    let det = m00 * c00 + m10 * c01 + m20 * c02 + m30 * c03;
    if !det.is_finite() || det.abs() <= f32::EPSILON {
        return IDENTITY_MAT4;
    }
    let inv = 1.0 / det;
    [
        c00 * inv,
        c10 * inv,
        c20 * inv,
        c30 * inv, //
        c01 * inv,
        c11 * inv,
        c21 * inv,
        c31 * inv, //
        c02 * inv,
        c12 * inv,
        c22 * inv,
        c32 * inv, //
        c03 * inv,
        c13 * inv,
        c23 * inv,
        c33 * inv,
    ]
}

/// mat4 作用到点（w=1，忽略平移外投影项）。
pub fn mat4_transform_point(m: [f32; 16], point: [f32; 3]) -> [f32; 3] {
    [
        m[0] * point[0] + m[4] * point[1] + m[8] * point[2] + m[12],
        m[1] * point[0] + m[5] * point[1] + m[9] * point[2] + m[13],
        m[2] * point[0] + m[6] * point[1] + m[10] * point[2] + m[14],
    ]
}

/// 每骨世界矩阵：父链累乘（世界 = 父世界 · 局部 TRS）。列主序。
pub fn world_matrices(skeleton: &ModelSkeleton, pose: &SkeletonPose) -> Vec<[f32; 16]> {
    let count = skeleton.bone_count();
    let mut world = vec![IDENTITY_MAT4; count];
    let mut resolved = vec![false; count];
    fn resolve(
        bone: usize,
        skeleton: &ModelSkeleton,
        pose: &SkeletonPose,
        world: &mut [[f32; 16]],
        resolved: &mut [bool],
        visiting: &mut HashSet<usize>,
    ) {
        if resolved[bone] || !visiting.insert(bone) {
            return;
        }
        let local_transform = pose.local_transform(bone, &skeleton.rest_pose[bone]);
        let local = trs_to_mat4(
            local_transform.translation,
            local_transform.rotation,
            local_transform.scale,
        );
        let parent = skeleton.parent_indices[bone];
        world[bone] = if parent < 0 {
            local
        } else {
            let parent = parent as usize;
            if parent < skeleton.bone_count() && parent != bone {
                resolve(parent, skeleton, pose, world, resolved, visiting);
                mat4_mul(world[parent], local)
            } else {
                local
            }
        };
        visiting.remove(&bone);
        resolved[bone] = true;
    }
    let mut visiting = HashSet::new();
    for bone in 0..count {
        resolve(
            bone,
            skeleton,
            pose,
            &mut world,
            &mut resolved,
            &mut visiting,
        );
    }
    world
}

/// 关节矩阵缓存：`world × inverse(bind world)` 中的 inverse(bind world) 在
/// 首次使用时计算并缓存（一个缓存实例只服务一个骨架）。
#[derive(Clone, Debug, Default)]
pub struct SkeletonInverseBindCache {
    inverse_bind_world: Vec<[f32; 16]>,
}

impl SkeletonInverseBindCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// 实例 joint 名表 → 关节矩阵（world × inverse(bind world)）。
    /// 缺失名回退：脸部（`j_f_*`）与发件扩展骨（`j_ex_h*`）锚定头骨 `j_kao`
    /// 的关节矩阵（无 `j_kao` 时 `n_root`；这两族在游戏里挂在头骨链下，
    /// 其 sklb 不随装配加载——锚定使它们随身高缩放/头部姿势移动，跟随头骨
    /// 是固定的近似），其余缺失名回退单位阵（顶点等价于不蒙皮）。
    pub fn joint_matrices(
        &mut self,
        skeleton: &ModelSkeleton,
        pose: &SkeletonPose,
        joint_names: &[String],
    ) -> Vec<[f32; 16]> {
        if self.inverse_bind_world.len() != skeleton.bone_count() {
            let bind_world = world_matrices(skeleton, &SkeletonPose::rest_pose(skeleton));
            self.inverse_bind_world = bind_world.iter().map(|m| mat4_inverse(*m)).collect();
        }
        let world = world_matrices(skeleton, pose);
        joint_names
            .iter()
            .map(|name| match skeleton.bone_index(name) {
                Some(bone) => mat4_mul(world[bone], self.inverse_bind_world[bone]),
                None => match missing_joint_anchor(skeleton, name) {
                    Some(anchor) => mat4_mul(world[anchor], self.inverse_bind_world[anchor]),
                    None => IDENTITY_MAT4,
                },
            })
            .collect()
    }
}

/// 缺失 joint 名的锚定骨：脸部 Dawntrail 骨（`j_f_*`）与发件扩展骨
/// （`j_ex_h*`，真实数据里如 `j_ex_h0002_ke_a_l`）挂头骨 `j_kao`（游戏
/// 骨架里它们在脸部/发件 partial skeleton 中，根在头骨链下；锚定后其
/// 顶点随头骨 joint 移动——身高缩放与头部姿势下与头一致，代价是丢失
/// 脸部独立微动画，预览可接受）。其余缺失名不锚定（回退单位阵）。
fn missing_joint_anchor(skeleton: &ModelSkeleton, name: &str) -> Option<usize> {
    if !(name.starts_with("j_f_") || name.starts_with("j_ex_h")) {
        return None;
    }
    skeleton
        .bone_index("j_kao")
        .or_else(|| skeleton.bone_index("n_root"))
}

/// `joint_matrices` 的无缓存便捷版（每次重算 inverse bind；调用方高频更新
/// 姿势时应使用 [`SkeletonInverseBindCache`]）。
pub fn joint_matrices(
    skeleton: &ModelSkeleton,
    pose: &SkeletonPose,
    joint_names: &[String],
) -> Vec<[f32; 16]> {
    SkeletonInverseBindCache::new().joint_matrices(skeleton, pose, joint_names)
}

/// 武器挂接件的关节矩阵：joint 名即挂点骨名（人体骨架 `n_buki_r` 主手 /
/// `n_buki_l` 副手），矩阵 = 该骨姿势世界矩阵 × `correction`。武器顶点
/// 不在绑定空间而在挂点骨局部空间（原点在握把、轴向对齐骨局部轴），因此
/// **不乘 inverse(bind world)**；`correction` 是武器模型局部 → 骨局部的
/// 常量校正。骨缺失回退单位阵（顶点等价于停在原点；挂点骨恒在人体骨架内，
/// 不走 [`joint_matrices`] 的脸部/发件锚定回退）。
pub fn weapon_attach_joint_matrices(
    skeleton: &ModelSkeleton,
    pose: &SkeletonPose,
    joint_names: &[String],
    correction: [f32; 16],
) -> Vec<[f32; 16]> {
    let world = world_matrices(skeleton, pose);
    joint_names
        .iter()
        .map(|name| match skeleton.bone_index(name) {
            Some(bone) => mat4_mul(world[bone], correction),
            None => IDENTITY_MAT4,
        })
        .collect()
}

// ---------------------------------------------------------------------------
// 种族骨变形（race deform）烘焙
// ---------------------------------------------------------------------------

/// 种族骨变形矩阵表：源族（回退文件）骨架 rest → 目标族骨架 rest。
///
/// 游戏内跨族装备/身体复用经 PBD 骨变形运行态处理；离线近似为逐骨刚体替换
/// `deform[bone] = target_world × inverse(source_world)`（TexTools 种族转换的
/// 骨架替换同法）。骨骼按**名**映射；目标骨架缺失同名骨时回退该骨在源骨架
/// 中最近的存在于目标骨架的祖先的 deform，全无匹配回退单位阵。
pub struct RaceDeform {
    /// 源骨架 bone index → deform 矩阵（行序与源骨架一致）。
    matrices: Vec<[f32; 16]>,
}

impl RaceDeform {
    /// 由源/目标骨架 rest 构建 deform 表。目标骨架缺失同名骨时，该骨沿用其
    /// 源父链上最近的存在于目标骨架的祖先的 deform（顶点跟随父骨走，而不是
    /// 被拽到父骨原点）。
    pub fn new(source: &ModelSkeleton, target: &ModelSkeleton) -> Self {
        let source_world = world_matrices(source, &SkeletonPose::rest_pose(source));
        let target_world = world_matrices(target, &SkeletonPose::rest_pose(target));
        let matrices = (0..source.bone_count())
            .map(|bone| {
                let mut candidate = Some(bone);
                while let Some(current) = candidate {
                    let name = &source.bone_names[current];
                    if let Some(target_bone) = target.bone_index(name) {
                        return mat4_mul(
                            target_world[target_bone],
                            mat4_inverse(source_world[current]),
                        );
                    }
                    let parent = source.parent_indices[current];
                    candidate = (parent >= 0).then_some(parent as usize);
                }
                IDENTITY_MAT4
            })
            .collect();
        Self { matrices }
    }

    /// 源骨架 bone index 的 deform 矩阵（越界回退单位阵）。
    pub fn matrix(&self, source_bone: usize) -> [f32; 16] {
        self.matrices
            .get(source_bone)
            .copied()
            .unwrap_or(IDENTITY_MAT4)
    }

    /// 源骨架 bone **名**的 deform 矩阵（未知名回退单位阵）。
    pub fn matrix_for_name(&self, source: &ModelSkeleton, name: &str) -> [f32; 16] {
        source
            .bone_index(name)
            .map(|bone| self.matrix(bone))
            .unwrap_or(IDENTITY_MAT4)
    }
}

/// 把网格从源族骨架 rest 烘焙到目标族骨架 rest（就地改写顶点位置/法线）。
///
/// 顶点按 blend 权重对 deform 矩阵加权（权重按和归一，与 WGSL 蒙皮一致）；
/// 无 blend 数据或全部权重为零的顶点不动。法线用同一加权矩阵的 mat3 部分
/// 变换并归一（非均匀缩放的近似）。无 bone_table 的网格无法逐骨映射，返回
/// 跳过的顶点数（调用方记诊断）。
pub fn bake_race_deform(
    mesh: &mut crate::model::ModelMesh,
    source: &ModelSkeleton,
    deform: &RaceDeform,
) -> usize {
    let Some(bone_table) = &mesh.bone_table else {
        return mesh.vertices.len();
    };
    let mut skipped = 0usize;
    for vertex in &mut mesh.vertices {
        let (Some(weights), Some(indices)) = (vertex.blend_weights, vertex.blend_indices) else {
            skipped += 1;
            continue;
        };
        let count = usize::from(weights.count.min(indices.count).min(8));
        let weight_sum: f32 = weights.values[..count].iter().sum();
        if count == 0 || weight_sum <= f32::EPSILON {
            skipped += 1;
            continue;
        }
        let mut blended = [0.0f32; 16];
        for slot in 0..count {
            let bone = usize::from(indices.values[slot]);
            let deform = bone_table
                .bone_names
                .get(bone)
                .and_then(|name| name.as_deref())
                .map(|name| deform.matrix_for_name(source, name))
                .unwrap_or(IDENTITY_MAT4);
            let weight = weights.values[slot] / weight_sum;
            for lane in 0..16 {
                blended[lane] += weight * deform[lane];
            }
        }
        vertex.position = mat4_transform_point(blended, vertex.position);
        let normal = vertex.normal;
        let transformed = [
            blended[0] * normal[0] + blended[4] * normal[1] + blended[8] * normal[2],
            blended[1] * normal[0] + blended[5] * normal[1] + blended[9] * normal[2],
            blended[2] * normal[0] + blended[6] * normal[1] + blended[10] * normal[2],
        ];
        let length = (transformed[0] * transformed[0]
            + transformed[1] * transformed[1]
            + transformed[2] * transformed[2])
            .sqrt();
        if length > f32::EPSILON {
            vertex.normal = [
                transformed[0] / length,
                transformed[1] / length,
                transformed[2] / length,
            ];
        }
    }
    skipped
}

/// 角色骨架 sklb 路径（race code，如 101=中原人男）。
pub fn character_skeleton_path(race_code: u16) -> String {
    format!("chara/human/c{race_code:04}/skeleton/base/b0001/skl_c{race_code:04}b0001.sklb")
}

/// 宠物/坐骑骨架 sklb 路径（monster/demihuman 两套模板）。
pub fn skeleton_path_for_chara_model(model: PackedCharaModelId) -> String {
    match model.chara_type {
        CharaModelType::Monster => format!(
            "chara/monster/m{id:04}/skeleton/base/b0001/skl_m{id:04}b0001.sklb",
            id = model.model_id
        ),
        CharaModelType::Demihuman => format!(
            "chara/demihuman/d{id:04}/skeleton/base/b0001/skl_d{id:04}b0001.sklb",
            id = model.model_id
        ),
    }
}

/// sklb 加载/解析错误。vendored 解析器对不支持项直接 panic，对外统一经
/// `catch_unwind` 转成 `HavokPanic`（wasm 下 panic=abort，宁可加载失败不可炸页面）。
#[derive(Clone, Debug, PartialEq)]
pub enum SkeletonError {
    Truncated {
        len: usize,
    },
    BadMagic,
    UnknownVersion {
        version: u32,
    },
    TagfileMagicMissing {
        offset: usize,
    },
    HavokPanic(String),
    NoSkeleton,
    EmptySkeleton,
    LengthMismatch {
        names: usize,
        parents: usize,
        poses: usize,
    },
    InvalidParent {
        bone: usize,
        parent: i64,
    },
    ParentCycle {
        bone: usize,
    },
    EmptyBoneName {
        bone: usize,
    },
    NonFinite {
        bone: usize,
    },
    BoneIndexOutOfRange {
        bone: usize,
        count: usize,
    },
}

impl fmt::Display for SkeletonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated { len } => write!(f, "sklb file too short (len={len})"),
            Self::BadMagic => write!(f, "bad sklb magic (expected blks/sklb)"),
            Self::UnknownVersion { version } => {
                write!(f, "unknown sklb version {version:#010x}, no tagfile magic")
            }
            Self::TagfileMagicMissing { offset } => {
                write!(f, "havok tagfile magic missing at offset {offset}")
            }
            Self::HavokPanic(message) => write!(f, "havok decode panic: {message}"),
            Self::NoSkeleton => write!(f, "hkaAnimationContainer has no skeleton"),
            Self::EmptySkeleton => write!(f, "skeleton has no bones"),
            Self::LengthMismatch {
                names,
                parents,
                poses,
            } => write!(
                f,
                "array length mismatch: names={names} parents={parents} poses={poses}"
            ),
            Self::InvalidParent { bone, parent } => {
                write!(f, "bone {bone} parent {parent} out of range")
            }
            Self::ParentCycle { bone } => write!(f, "parent cycle involving bone {bone}"),
            Self::EmptyBoneName { bone } => write!(f, "bone {bone} has empty name"),
            Self::NonFinite { bone } => {
                write!(f, "bone {bone} rest pose has non-finite value")
            }
            Self::BoneIndexOutOfRange { bone, count } => {
                write!(f, "bone index {bone} out of range (count={count})")
            }
        }
    }
}

impl std::error::Error for SkeletonError {}

/// 解析 sklb 字节为 rest pose 骨架。magic 兼容 `blks`/`sklb`；版本 0021（u16
/// 偏移）、0030/0031（u32 偏移）走已知布局，未知版本在文件头 128 字节内回退
/// 扫描 tagfile magic。
pub fn load_skeleton_from_sklb_bytes(bytes: &[u8]) -> Result<ModelSkeleton, SkeletonError> {
    let havok_offset = sklb_havok_offset(bytes)?;
    let skeleton = parse_havok_skeleton(&bytes[havok_offset..])?;
    validate_skeleton(&skeleton)?;
    Ok(skeleton)
}

fn sklb_havok_offset(bytes: &[u8]) -> Result<usize, SkeletonError> {
    if bytes.len() < 16 {
        return Err(SkeletonError::Truncated { len: bytes.len() });
    }
    // 真实文件按字节是 "blks"，部分文档写作 "sklb"；两种都接受。
    if &bytes[0..4] != b"blks" && &bytes[0..4] != b"sklb" {
        return Err(SkeletonError::BadMagic);
    }
    let version = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
    let offset = match version {
        // physis SklbV1 / SklbV2 布局
        0x3132_3030 => u16::from_le_bytes([bytes[10], bytes[11]]) as usize,
        0x3133_3030 | 0x3133_3031 => {
            u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]) as usize
        }
        _ => bytes
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
            .ok_or(SkeletonError::UnknownVersion { version })?,
    };
    if offset + 4 > bytes.len()
        || u32::from_le_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
        ]) != TAGFILE_MAGIC
    {
        return Err(SkeletonError::TagfileMagicMissing { offset });
    }
    Ok(offset)
}

fn parse_havok_skeleton(havok: &[u8]) -> Result<ModelSkeleton, SkeletonError> {
    let decoded = catch_unwind(AssertUnwindSafe(|| {
        let root = HavokBinaryTagFileReader::read(havok);
        let container_obj = root.find_object_by_type("hkaAnimationContainer");
        HavokAnimationContainer::new(container_obj)
    }))
    .map_err(|panic| {
        SkeletonError::HavokPanic(
            panic
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| panic.downcast_ref::<&str>().map(|s| (*s).to_string()))
                .unwrap_or_else(|| "<non-string panic>".to_string()),
        )
    })?;
    let Some(havok_skeleton) = decoded.skeletons.first() else {
        return Err(SkeletonError::NoSkeleton);
    };
    let bone_count = havok_skeleton.bone_names.len();
    let mut parent_indices = Vec::with_capacity(bone_count);
    for &parent in &havok_skeleton.parent_indices {
        // havok 无父骨骼标记为 -1（vendored 解析回转成 usize::MAX；wasm32 下是
        // 0xFFFFFFFF 而非 u64::MAX，不能与 u64::MAX 比较），其余应为合法下标。
        parent_indices.push(if parent == usize::MAX {
            -1
        } else if parent <= i32::MAX as usize {
            parent as i32
        } else {
            return Err(SkeletonError::InvalidParent {
                bone: parent_indices.len(),
                parent: parent as i64,
            });
        });
    }
    let rest_pose = havok_skeleton
        .reference_pose
        .iter()
        .map(|transform| BoneTransform {
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
        })
        .collect();
    Ok(ModelSkeleton {
        bone_names: havok_skeleton.bone_names.clone(),
        parent_indices,
        rest_pose,
        body_scaling: None,
    })
}

fn validate_skeleton(skeleton: &ModelSkeleton) -> Result<(), SkeletonError> {
    let bone_count = skeleton.bone_count();
    if bone_count == 0 {
        return Err(SkeletonError::EmptySkeleton);
    }
    if skeleton.parent_indices.len() != bone_count || skeleton.rest_pose.len() != bone_count {
        return Err(SkeletonError::LengthMismatch {
            names: bone_count,
            parents: skeleton.parent_indices.len(),
            poses: skeleton.rest_pose.len(),
        });
    }
    for (index, name) in skeleton.bone_names.iter().enumerate() {
        if name.is_empty() {
            return Err(SkeletonError::EmptyBoneName { bone: index });
        }
    }
    for (index, &parent) in skeleton.parent_indices.iter().enumerate() {
        if parent == -1 {
            continue;
        }
        if parent < 0 || parent as usize >= bone_count {
            return Err(SkeletonError::InvalidParent {
                bone: index,
                parent: parent as i64,
            });
        }
        if parent as usize == index {
            return Err(SkeletonError::ParentCycle { bone: index });
        }
    }
    for start in 0..bone_count {
        let mut seen = HashSet::new();
        let mut cursor = start;
        loop {
            if !seen.insert(cursor) {
                return Err(SkeletonError::ParentCycle { bone: start });
            }
            let parent = skeleton.parent_indices[cursor];
            if parent == -1 {
                break;
            }
            cursor = parent as usize;
        }
    }
    for (index, transform) in skeleton.rest_pose.iter().enumerate() {
        let finite = transform
            .translation
            .iter()
            .chain(transform.rotation.iter())
            .chain(transform.scale.iter())
            .all(|value| value.is_finite());
        if !finite {
            return Err(SkeletonError::NonFinite { bone: index });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_mat4_approx(
        actual: &(impl AsRef<[f32]> + ?Sized),
        expected: &(impl AsRef<[f32]> + ?Sized),
        epsilon: f32,
        label: &str,
    ) {
        let (actual, expected) = (actual.as_ref(), expected.as_ref());
        assert_eq!(actual.len(), expected.len(), "{label}: length mismatch");
        for (index, (a, e)) in actual.iter().zip(expected.iter()).enumerate() {
            assert!(
                (a - e).abs() <= epsilon,
                "{label}[{index}]: actual {a} expected {e}"
            );
        }
    }

    fn two_bone_skeleton() -> ModelSkeleton {
        ModelSkeleton {
            bone_names: vec!["root".to_string(), "child".to_string()],
            parent_indices: vec![-1, 0],
            rest_pose: vec![
                BoneTransform {
                    translation: [1.0, 2.0, 3.0],
                    rotation: [0.0, 0.0, 0.0, 1.0],
                    scale: [1.0; 3],
                },
                BoneTransform {
                    translation: [0.0, 1.0, 0.0],
                    rotation: [0.0, 0.0, 0.0, 1.0],
                    scale: [1.0; 3],
                },
            ],
            body_scaling: None,
        }
    }

    #[test]
    fn trs_matrix_uses_translation_rotation_scale_order() {
        let rotation = quat_from_axis_angle([0.0, 0.0, 1.0], std::f32::consts::FRAC_PI_2);
        let matrix = trs_to_mat4([1.0, 0.0, 0.0], rotation, [2.0, 3.0, 4.0]);
        // 绕 Z 转 90°：+X → +Y；缩放先行（列 = R·S 的列）。
        let point = mat4_transform_point(matrix, [1.0, 0.0, 0.0]);
        assert_mat4_approx(&point, &[1.0, 2.0, 0.0], 1e-6, "point");
    }

    #[test]
    fn quaternion_axis_angle_layout_is_xyzw() {
        // 绕 X 180°：q = (1,0,0,0)。
        let rotation = quat_from_axis_angle([1.0, 0.0, 0.0], std::f32::consts::PI);
        assert_mat4_approx(&rotation, &[1.0, 0.0, 0.0, 0.0], 1e-6, "quat");
        let matrix = trs_to_mat4([0.0; 3], rotation, [1.0; 3]);
        let point = mat4_transform_point(matrix, [0.0, 1.0, 0.0]);
        assert_mat4_approx(&point, &[0.0, -1.0, 0.0], 1e-5, "point");
    }

    #[test]
    fn zero_length_quaternion_falls_back_to_identity_rotation() {
        let matrix = trs_to_mat4([5.0, 6.0, 7.0], [0.0; 4], [1.0; 3]);
        assert_mat4_approx(
            &matrix,
            &[
                1.0, 0.0, 0.0, 0.0, //
                0.0, 1.0, 0.0, 0.0, //
                0.0, 0.0, 1.0, 0.0, //
                5.0, 6.0, 7.0, 1.0,
            ],
            0.0,
            "matrix",
        );
    }

    #[test]
    fn mat4_inverse_of_rigid_transform_is_exact() {
        // 90° 绕 Z + 整数平移：det=±1 且所有中间值可精确表示，逆矩阵逐位精确。
        let rotation = quat_from_axis_angle([0.0, 0.0, 1.0], std::f32::consts::FRAC_PI_2);
        let matrix = trs_to_mat4([3.0, -2.0, 5.0], rotation, [1.0; 3]);
        let product = mat4_mul(matrix, mat4_inverse(matrix));
        assert_mat4_approx(&product, &IDENTITY_MAT4, 1e-6, "product");
    }

    #[test]
    fn world_matrices_accumulate_parent_chain() {
        let skeleton = two_bone_skeleton();
        let world = world_matrices(&skeleton, &SkeletonPose::rest_pose(&skeleton));
        assert_mat4_approx(
            &world[0],
            &[
                1.0, 0.0, 0.0, 0.0, //
                0.0, 1.0, 0.0, 0.0, //
                0.0, 0.0, 1.0, 0.0, //
                1.0, 2.0, 3.0, 1.0,
            ],
            1e-6,
            "root world",
        );
        // 子骨世界平移 = 父平移 + 父旋转(恒等) · 子平移 = (1,3,3)。
        assert_mat4_approx(
            &world[1],
            &[
                1.0, 0.0, 0.0, 0.0, //
                0.0, 1.0, 0.0, 0.0, //
                0.0, 0.0, 1.0, 0.0, //
                1.0, 3.0, 3.0, 1.0,
            ],
            1e-6,
            "child world",
        );
    }

    #[test]
    fn identity_pose_gives_identity_joint_matrices() {
        let skeleton = two_bone_skeleton();
        let joint_names = vec!["root".to_string(), "child".to_string()];
        let joints = joint_matrices(&skeleton, &SkeletonPose::rest_pose(&skeleton), &joint_names);
        assert_eq!(joints.len(), 2);
        assert_mat4_approx(&joints[0], &IDENTITY_MAT4, 1e-5, "root joint");
        assert_mat4_approx(&joints[1], &IDENTITY_MAT4, 1e-5, "child joint");
    }

    #[test]
    fn posed_parent_moves_child_joint_coherently() {
        // 父骨绕 Z 转 90°：绑定在子骨上的点随父旋转而转动。
        let skeleton = two_bone_skeleton();
        let mut pose = SkeletonPose::rest_pose(&skeleton);
        pose.set_rotation(
            0,
            quat_from_axis_angle([0.0, 0.0, 1.0], std::f32::consts::FRAC_PI_2),
        )
        .expect("set root rotation");
        let joints = joint_matrices(&skeleton, &pose, &["root".to_string(), "child".to_string()]);
        // 绑定于子骨的顶点（模型空间 (1,4,3)，即子骨世界原点 + (0,1,0)）：
        // 绑定于子骨的顶点（模型空间 (1,4,3)，即子骨世界原点 + (0,1,0)）：
        // 绕父骨原点 (1,2,3) 转 90°：相对 (0,2,0) → (-2,0,0) → posed (1-2, 2, 3)。
        let bind_point = [1.0, 4.0, 3.0];
        let skinned = mat4_transform_point(joints[1], bind_point);
        assert_mat4_approx(&skinned, &[-1.0, 2.0, 3.0], 1e-4, "skinned point");
        // 父骨自身上的绑定点绕父原点转动：(2,2,3) 相对 (1,0,0) → (0,1,0) → (1,3,3)。
        let root_skinned = mat4_transform_point(joints[0], [2.0, 2.0, 3.0]);
        assert_mat4_approx(&root_skinned, &[1.0, 3.0, 3.0], 1e-4, "root skinned");
    }

    #[test]
    fn joint_matrices_support_name_subset_and_missing_name_fallback() {
        let skeleton = two_bone_skeleton();
        let joint_names = vec!["child".to_string(), "missing".to_string()];
        let joints = joint_matrices(&skeleton, &SkeletonPose::rest_pose(&skeleton), &joint_names);
        assert_eq!(joints.len(), 2);
        // 子集内 rest ⇒ 恒等。
        assert_mat4_approx(&joints[0], &IDENTITY_MAT4, 1e-5, "child joint");
        // 缺失名回退单位阵。
        assert_mat4_approx(&joints[1], &IDENTITY_MAT4, 0.0, "missing joint");
    }

    #[test]
    fn inverse_bind_cache_reuses_inverses_across_poses() {
        let skeleton = two_bone_skeleton();
        let mut cache = SkeletonInverseBindCache::new();
        let names = ["root".to_string(), "child".to_string()];
        let first = cache.joint_matrices(&skeleton, &SkeletonPose::rest_pose(&skeleton), &names);
        let cached_len = cache.inverse_bind_world.len();
        assert_eq!(cached_len, skeleton.bone_count());
        let mut pose = SkeletonPose::rest_pose(&skeleton);
        pose.set_translation(0, [5.0, 2.0, 3.0])
            .expect("set translation");
        let second = cache.joint_matrices(&skeleton, &pose, &names);
        assert_eq!(
            cache.inverse_bind_world.len(),
            cached_len,
            "逆矩阵只计算一次"
        );
        // 平移不影响方向：子骨 joint 平移分量 = posed_world - bind_world 的平移差。
        let skinned = mat4_transform_point(second[1], [1.0, 4.0, 3.0]);
        assert_mat4_approx(&skinned, &[5.0, 4.0, 3.0], 1e-4, "translated skinned");
        assert_mat4_approx(&first[1], &IDENTITY_MAT4, 1e-5, "first child joint");
    }

    #[test]
    fn weapon_attach_joint_matrices_use_pose_world_times_correction() {
        // 武器挂接不做 inverse bind：joint 矩阵 = 挂点骨姿势世界 × 校正。
        // rest 下挂点骨世界矩阵平移即骨位（与 joint_matrices 的恒等输出不同）。
        let skeleton = two_bone_skeleton();
        let rest = SkeletonPose::rest_pose(&skeleton);
        let names = vec!["child".to_string(), "missing".to_string()];

        let identity_correction =
            weapon_attach_joint_matrices(&skeleton, &rest, &names, IDENTITY_MAT4);
        let world = world_matrices(&skeleton, &rest);
        assert_mat4_approx(
            &identity_correction[0],
            &world[1],
            1e-6,
            "rest attach joint",
        );
        assert_mat4_approx(&identity_correction[1], &IDENTITY_MAT4, 0.0, "missing bone");

        // 校正右乘生效：纯平移校正在挂接点基础上再平移（骨局部系）。
        let mut translated_correction = IDENTITY_MAT4;
        translated_correction[12] = 0.5;
        translated_correction[13] = 0.25;
        let corrected =
            weapon_attach_joint_matrices(&skeleton, &rest, &names[..1], translated_correction);
        let origin = mat4_transform_point(corrected[0], [0.0; 3]);
        assert_mat4_approx(&origin, &[1.5, 3.25, 3.0], 1e-6, "corrected attach origin");

        // 姿势驱动：父骨平移经父链传入挂点骨世界矩阵。
        let mut pose = SkeletonPose::rest_pose(&skeleton);
        pose.set_translation(0, [10.0, 0.0, 0.0])
            .expect("set translation");
        let posed = weapon_attach_joint_matrices(&skeleton, &pose, &names[..1], IDENTITY_MAT4);
        let origin = mat4_transform_point(posed[0], [0.0; 3]);
        assert_mat4_approx(&origin, &[10.0, 1.0, 0.0], 1e-6, "posed attach origin");
    }
    #[test]
    fn pose_index_out_of_range_is_an_error_not_panic() {
        let mut pose = SkeletonPose::new(2);
        assert!(pose.set_rotation(2, [0.0; 4]).is_err());
        assert!(pose.set_transform(9, BoneTransform::IDENTITY).is_err());
        assert!(pose.set_translation(1, [0.0; 3]).is_ok());
    }

    #[test]
    fn pose_length_mismatch_falls_back_to_rest_per_bone() {
        let skeleton = two_bone_skeleton();
        // 只覆盖 root 的 1 骨 pose：child 回退 rest。
        let mut pose = SkeletonPose::new(1);
        pose.set_translation(0, [10.0, 0.0, 0.0])
            .expect("set translation");
        let world = world_matrices(&skeleton, &pose);
        assert_mat4_approx(
            &world[0][12..16],
            &[10.0, 0.0, 0.0, 1.0],
            1e-6,
            "root translation",
        );
        assert_mat4_approx(
            &world[1][12..16],
            &[10.0, 1.0, 0.0, 1.0],
            1e-6,
            "child translation",
        );
    }

    #[test]
    fn skeleton_paths_follow_sklb_naming() {
        assert_eq!(
            character_skeleton_path(101),
            "chara/human/c0101/skeleton/base/b0001/skl_c0101b0001.sklb"
        );
        assert_eq!(
            skeleton_path_for_chara_model(PackedCharaModelId {
                model_id: 1,
                base_id: 1,
                variant_id: 1,
                chara_type: CharaModelType::Demihuman,
            }),
            "chara/demihuman/d0001/skeleton/base/b0001/skl_d0001b0001.sklb"
        );
        assert_eq!(
            skeleton_path_for_chara_model(PackedCharaModelId {
                model_id: 8003,
                base_id: 1,
                variant_id: 1,
                chara_type: CharaModelType::Monster,
            }),
            "chara/monster/m8003/skeleton/base/b0001/skl_m8003b0001.sklb"
        );
    }

    #[test]
    fn load_rejects_garbage_without_panic() {
        assert_eq!(
            load_skeleton_from_sklb_bytes(&[]),
            Err(SkeletonError::Truncated { len: 0 })
        );
        assert_eq!(
            load_skeleton_from_sklb_bytes(b"not a skeleton file at all...."),
            Err(SkeletonError::BadMagic)
        );
    }

    #[test]
    fn load_rejects_unknown_version_and_missing_tagfile() {
        let mut bytes = b"blks".to_vec();
        bytes.extend_from_slice(&0xDEAD_BEEFu32.to_le_bytes());
        bytes.extend_from_slice(&[0u8; 8]);
        // 前 128 字节内无 tagfile magic → UnknownVersion。
        assert_eq!(
            load_skeleton_from_sklb_bytes(&bytes),
            Err(SkeletonError::UnknownVersion {
                version: 0xDEAD_BEEF
            })
        );

        // 0031 布局：u32 偏移指向无 magic 的位置 → TagfileMagicMissing。
        let mut bytes = b"blks".to_vec();
        bytes.extend_from_slice(&0x3133_3031u32.to_le_bytes());
        bytes.extend_from_slice(&[0u8; 4]);
        bytes.extend_from_slice(&64u32.to_le_bytes());
        bytes.extend_from_slice(&[0u8; 60]);
        assert_eq!(
            load_skeleton_from_sklb_bytes(&bytes),
            Err(SkeletonError::TagfileMagicMissing { offset: 64 })
        );
    }

    #[test]
    fn load_wraps_havok_panics_into_error() {
        // 合法容器头 + 指向垃圾 tagfile：vendored reader 内部 panic，必须转为 Err。
        let mut bytes = b"blks".to_vec();
        bytes.extend_from_slice(&0x3133_3031u32.to_le_bytes());
        bytes.extend_from_slice(&[0u8; 4]);
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&0xCAB0_0D1Eu32.to_le_bytes());
        bytes.extend_from_slice(&[0xFFu8; 128]);
        match load_skeleton_from_sklb_bytes(&bytes) {
            Err(SkeletonError::HavokPanic(_)) => {}
            other => panic!("expected HavokPanic, got {other:?}"),
        }
    }

    #[test]
    fn validate_rejects_cycles_and_bad_parents() {
        let cyclic = ModelSkeleton {
            bone_names: vec!["a".to_string(), "b".to_string()],
            parent_indices: vec![1, 0],
            rest_pose: vec![BoneTransform::IDENTITY; 2],
            body_scaling: None,
        };
        assert_eq!(
            validate_skeleton(&cyclic),
            Err(SkeletonError::ParentCycle { bone: 0 })
        );
        let bad_parent = ModelSkeleton {
            parent_indices: vec![-1, 7],
            ..cyclic.clone()
        };
        assert_eq!(
            validate_skeleton(&bad_parent),
            Err(SkeletonError::InvalidParent { bone: 1, parent: 7 })
        );
        let ok = ModelSkeleton {
            parent_indices: vec![-1, 0],
            ..cyclic
        };
        assert!(validate_skeleton(&ok).is_ok());
    }

    fn deform_test_skeleton(child_translation: [f32; 3], child_scale: [f32; 3]) -> ModelSkeleton {
        ModelSkeleton {
            bone_names: vec!["root".to_string(), "limb".to_string()],
            parent_indices: vec![-1, 0],
            rest_pose: vec![
                BoneTransform::IDENTITY,
                BoneTransform {
                    translation: child_translation,
                    rotation: [0.0, 0.0, 0.0, 1.0],
                    scale: child_scale,
                },
            ],
            body_scaling: None,
        }
    }

    #[test]
    fn race_deform_maps_bone_names_with_ancestor_fallback() {
        let source = deform_test_skeleton([1.0, 0.0, 0.0], [1.0; 3]);
        let mut target = deform_test_skeleton([2.0, 0.0, 0.0], [2.0; 3]);
        // 目标缺 limb 时回退源父链（root）；全无匹配时为单位阵（root 也不在
        // 目标中比较的是同一名字——此处目标有 root）。
        let deform = RaceDeform::new(&source, &target);
        // limb：target_world(limb) × inverse(source_world(limb))。
        let source_world = world_matrices(&source, &SkeletonPose::rest_pose(&source));
        let target_world = world_matrices(&target, &SkeletonPose::rest_pose(&target));
        let expected = mat4_mul(target_world[1], mat4_inverse(source_world[1]));
        assert_mat4_approx(&deform.matrix(1), &expected, 1e-6, "limb deform");
        // 目标骨架删掉 limb：deform 回落 root 的（= target_root × inverse(source_root)）。
        target.bone_names[1] = "other".to_string();
        let deform = RaceDeform::new(&source, &target);
        let expected_root = mat4_mul(target_world[0], mat4_inverse(source_world[0]));
        assert_mat4_approx(&deform.matrix(1), &expected_root, 1e-6, "ancestor fallback");
    }

    #[test]
    fn bake_race_deform_moves_vertices_to_target_proportions() {
        use crate::model::{
            ModelBlendIndices, ModelBlendWeights, ModelBoneTable, ModelMesh, ModelVertex,
        };
        // 源族 limb 短（平移 1、缩放 1），目标族 limb 长（平移 1、缩放 2）：
        // limb 世界矩阵缩放差 2 倍 → limb 上的点距根加倍。
        let source = deform_test_skeleton([1.0, 0.0, 0.0], [1.0; 3]);
        let target = deform_test_skeleton([1.0, 0.0, 0.0], [2.0; 3]);
        let deform = RaceDeform::new(&source, &target);
        let vertex = |position: [f32; 3], weights: [f32; 2]| ModelVertex {
            position,
            blend_weights: Some(ModelBlendWeights {
                count: 2,
                values: [weights[0], weights[1], 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            }),
            blend_indices: Some(ModelBlendIndices {
                count: 2,
                values: [0, 1, 0, 0, 0, 0, 0, 0],
            }),
            normal: [1.0, 0.0, 0.0],
            uv0: [0.0; 2],
            uv1: [0.0; 2],
            uv2: [0.0; 2],
            uv3: [0.0; 2],
            bitangent: [0.0; 4],
            normal1: None,
            bitangent1: None,
            color: [0.0; 4],
            color1: None,
            flow0: None,
            flow1: None,
        };
        let mut mesh = ModelMesh {
            path: "chara/equipment/e0000/model/c0201e0000_glv.mdl".to_string(),
            part_index: 0,
            mesh_category: None,
            submesh: None,
            shape_influences: Vec::new(),
            shape_targets: Vec::new(),
            material_index: 0,
            material_slot: 0,
            material_name: String::new(),
            color: [0.0; 3],
            bone_table: Some(ModelBoneTable {
                index: 0,
                bone_count: 2,
                bone_indices: vec![0, 1],
                bone_names: vec![Some("root".to_string()), Some("limb".to_string())],
            }),
            vertices: vec![
                vertex([1.0, 1.0, 0.0], [0.0, 1.0]),
                vertex([0.5, 0.0, 0.0], [0.5, 0.5]),
                vertex([9.0, 9.0, 9.0], [0.0, 0.0]),
            ],
            indices: Vec::new(),
        };
        let skipped = bake_race_deform(&mut mesh, &source, &deform);
        assert_eq!(skipped, 1, "zero-weight vertex is skipped");
        // 纯 limb 顶点（源 limb 局部偏移 [0,1,0]）：目标 limb 缩放 2 → 偏移
        // [0,2,0] + limb 世界平移 [1,0,0] = [1,2,0]。
        assert_mat4_approx(
            &mesh.vertices[0].position,
            &[1.0, 2.0, 0.0],
            1e-5,
            "limb vertex",
        );
        // 半权顶点：deform(limb) 下 [0.5,0,0] → 2([0.5,0,0]-[1,0,0])+[1,0,0]
        // = [0,0,0]，与 root（恒等）0.5 混合 = [0.25,0,0]。
        assert_mat4_approx(
            &mesh.vertices[1].position,
            &[0.25, 0.0, 0.0],
            1e-5,
            "blended vertex",
        );
        // 零权顶点不动。
        assert_mat4_approx(
            &mesh.vertices[2].position,
            &[9.0, 9.0, 9.0],
            0.0,
            "skipped vertex",
        );
        // 法线保持单位长度。
        let normal = mesh.vertices[0].normal;
        let length = (normal[0] * normal[0] + normal[1] * normal[1] + normal[2] * normal[2]).sqrt();
        assert!((length - 1.0).abs() < 1e-5, "normal stays unit");
    }

    // -----------------------------------------------------------------------
    // 体型缩放（RGSP）姿势合成
    // -----------------------------------------------------------------------

    /// 人形小骨架：n_root → n_hara → (j_mune_l, n_sippo_a → n_sippo_b)。
    fn humanoid_scaling_skeleton(
        scaling: Option<crate::racial_scaling::BodyScaling>,
    ) -> ModelSkeleton {
        let bone = |translation: [f32; 3]| BoneTransform {
            translation,
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0; 3],
        };
        ModelSkeleton {
            bone_names: vec![
                "n_root".to_string(),
                "n_hara".to_string(),
                "j_mune_l".to_string(),
                "n_sippo_a".to_string(),
                "n_sippo_b".to_string(),
            ],
            parent_indices: vec![-1, 0, 1, 1, 3],
            rest_pose: vec![
                bone([0.0; 3]),
                bone([0.0, 0.5, 0.0]),
                bone([0.1, 0.3, 0.0]),
                bone([0.0, 0.1, -0.2]),
                bone([0.0, -0.1, -0.1]),
            ],
            body_scaling: scaling,
        }
    }

    #[test]
    fn scaled_rest_pose_applies_height_tail_bust_to_target_bones() {
        let scaling = crate::racial_scaling::BodyScaling {
            height: 2.0,
            tail: 3.0,
            bust: [1.1, 1.2, 1.3],
        };
        let skeleton = humanoid_scaling_skeleton(Some(scaling));
        let pose = skeleton.scaled_rest_pose();
        // 目标骨局部 scale 乘上系数，其余骨不变。
        assert_eq!(
            pose.transform(0).unwrap().scale,
            [2.0; 3],
            "n_root × height"
        );
        assert_eq!(pose.transform(1).unwrap().scale, [1.0; 3], "n_hara 不变");
        assert_eq!(
            pose.transform(2).unwrap().scale,
            [1.1, 1.2, 1.3],
            "j_mune_l × bust（逐轴）"
        );
        assert_eq!(
            pose.transform(3).unwrap().scale,
            [3.0; 3],
            "尾链根 n_sippo_a × tail"
        );
        assert_eq!(
            pose.transform(4).unwrap().scale,
            [1.0; 3],
            "尾链其余骨不直接缩放（随父链继承）"
        );
        // 原始 rest pose 不被污染（bind 语义保持未缩放）。
        assert_eq!(skeleton.rest_pose[0].scale, [1.0; 3]);
        assert_eq!(skeleton.rest_pose[3].scale, [1.0; 3]);

        // 世界矩阵：身高 2 倍沿根链传播（n_hara 世界 y = 0.5 × 2 = 1.0）；
        // n_sippo_b 世界平移 = 2×(hara + 3×sippo_a) 平移 + 6×sippo_b 平移。
        let world = world_matrices(&skeleton, &pose);
        assert_mat4_approx(&world[1][12..15], &[0.0, 1.0, 0.0], 1e-6, "n_hara world");
        assert_mat4_approx(
            &world[4][12..15],
            &[0.0, 0.6, -1.0],
            1e-6,
            "n_sippo_b world（身高 2 × 尾 3 复合）",
        );
        // 无缩放骨架：scaled_rest_pose 与 rest 逐骨一致。
        let plain = humanoid_scaling_skeleton(None);
        assert_eq!(
            plain.scaled_rest_pose(),
            SkeletonPose::rest_pose(&plain),
            "无体型缩放时展示 rest = 原始 rest"
        );
    }

    #[test]
    fn body_scaling_flows_through_skinning_and_weapon_attach() {
        // 蒙皮：joint = scaled_world × inverse(raw bind world) —— 身高 2 倍时
        // 任意骨 joint 都是关于原点的 2 倍缩放（顶点随体型缩放）。
        let skeleton = humanoid_scaling_skeleton(Some(crate::racial_scaling::BodyScaling {
            height: 2.0,
            ..crate::racial_scaling::BodyScaling::IDENTITY
        }));
        let pose = skeleton.scaled_rest_pose();
        let names: Vec<String> = skeleton.bone_names.clone();
        let joints = joint_matrices(&skeleton, &pose, &names);
        let skinned = mat4_transform_point(joints[1], [0.0, 0.5, 0.0]);
        assert_mat4_approx(&skinned, &[0.0, 1.0, 0.0], 1e-5, "蒙皮点随身高加倍");
        // rest（未缩放）下 joint = 恒等。
        let identity_joints =
            joint_matrices(&skeleton, &SkeletonPose::rest_pose(&skeleton), &names);
        assert_mat4_approx(&identity_joints[1], &IDENTITY_MAT4, 1e-5, "raw rest joint");

        // 武器挂接：挂点骨世界矩阵随身高缩放（武器随身体变大/变小）——挂到
        // n_sippo_b（类比 n_buki_l/r 的深层骨），attach 原点高度随身高加倍。
        let attach_names = vec!["n_sippo_b".to_string()];
        let unscaled = weapon_attach_joint_matrices(
            &skeleton,
            &SkeletonPose::rest_pose(&skeleton),
            &attach_names,
            IDENTITY_MAT4,
        );
        let scaled = weapon_attach_joint_matrices(&skeleton, &pose, &attach_names, IDENTITY_MAT4);
        let unscaled_origin = mat4_transform_point(unscaled[0], [0.0; 3]);
        let scaled_origin = mat4_transform_point(scaled[0], [0.0; 3]);
        assert_mat4_approx(
            &scaled_origin,
            &[
                unscaled_origin[0] * 2.0,
                unscaled_origin[1] * 2.0,
                unscaled_origin[2] * 2.0,
            ],
            1e-5,
            "挂接点随身高缩放",
        );
    }

    #[test]
    fn body_scaling_composes_on_top_of_sampled_scale_reset() {
        // 动画采样会用 track 值（一般 1.0）覆盖局部 scale：RGSP 必须在采样后
        // 重新叠加，而不是烘进 rest。模拟"采样把 n_root scale 覆盖为 1.0"的
        // 姿势，apply_body_scaling 后恢复身高系数。
        let scaling = crate::racial_scaling::BodyScaling {
            height: 1.5,
            ..crate::racial_scaling::BodyScaling::IDENTITY
        };
        let skeleton = humanoid_scaling_skeleton(Some(scaling));
        let mut pose = SkeletonPose::rest_pose(&skeleton);
        pose.set_transform(
            0,
            BoneTransform {
                translation: [0.0, 0.1, 0.0], // 动画同时改了平移
                rotation: [0.0, 0.0, 0.0, 1.0],
                scale: [1.0; 3], // track 覆盖 scale
            },
        )
        .expect("set sampled transform");
        pose.apply_body_scaling(&skeleton, &scaling);
        assert_eq!(
            pose.transform(0).unwrap().scale,
            [1.5; 3],
            "采样覆盖后身高系数重新叠加"
        );
        assert_eq!(
            pose.transform(0).unwrap().translation,
            [0.0, 0.1, 0.0],
            "采样的平移保留"
        );
    }

    #[test]
    fn missing_face_and_hair_bones_anchor_to_head_joint() {
        // 脸部 j_f_* / 发件 j_ex_h* 骨不在装配骨架里（真实人体 MDL 的骨表
        // 就引用这些 partial skeleton 骨）：关节矩阵锚定头骨（无 j_kao 时
        // n_root）——身高缩放时与头骨同步缩放，不再停在未缩放位置。
        let skeleton = humanoid_scaling_skeleton(Some(crate::racial_scaling::BodyScaling {
            height: 2.0,
            ..crate::racial_scaling::BodyScaling::IDENTITY
        }));
        let pose = skeleton.scaled_rest_pose();
        let names = vec![
            "j_f_hoho_l".to_string(),
            "j_ex_h0002_ke_a_l".to_string(),
            "n_truly_missing".to_string(),
        ];
        let joints = joint_matrices(&skeleton, &pose, &names);
        let root_joint = joint_matrices(&skeleton, &pose, &["n_root".to_string()])[0];
        assert_mat4_approx(&joints[0], &root_joint, 1e-6, "j_f_* 锚定头骨链 joint");
        assert_mat4_approx(&joints[1], &root_joint, 1e-6, "j_ex_h* 锚定头骨链 joint");
        assert_mat4_approx(&joints[2], &IDENTITY_MAT4, 0.0, "其余缺失名回退单位阵");
        // 锚定生效后顶点随身高缩放（本骨架无 j_kao → n_root：身高 2 倍）。
        let skinned = mat4_transform_point(joints[0], [0.0, 1.0, 0.0]);
        assert_mat4_approx(&skinned, &[0.0, 2.0, 0.0], 1e-5, "脸部顶点随身高缩放");
    }
}
