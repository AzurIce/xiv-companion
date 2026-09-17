// Model shader module, part of the `package` WESL package
// (compiled to OUT_DIR/model.wgsl by the build script).

// 实例蒙皮关节矩阵（storage 头 + 运行时数组）。count=0（无骨架实例）时
// vs_main/vs_outline 跳过蒙皮分支，渲染与非蒙皮路径逐位一致。
struct JointData {
    count: u32,
    pad0: u32,
    pad1: u32,
    pad2: u32,
    matrices: array<mat4x4<f32>>,
};

@group(2) @binding(0)
var<storage, read> joint_data: JointData;

/// GPU 蒙皮：skinned_pos = Σ w·J·pos。MDL 原始权重是 u8/255 量化值，和只在
/// 量化误差范围内≈1，这里按 Σw 归一兜底。法线用 mat3(J)——骨骼无缩放时是
/// 精确的；FFXIV 骨骼为刚体（rest 含 uniform scale 时已并入 world 计算），
/// 非均匀缩放骨骼下只是近似（注释声明，不做逆转置）。
struct SkinnedPosition {
    position: vec4<f32>,
    normal: vec3<f32>,
};

fn skin_vertex(
    position: vec4<f32>,
    normal: vec3<f32>,
    joints: vec2<u32>,
    weights0: vec4<f32>,
    weights1: vec4<f32>,
) -> SkinnedPosition {
    var joint_indices = array<u32, 8>(
        joints.x & 0xFFu,
        (joints.x >> 8u) & 0xFFu,
        (joints.x >> 16u) & 0xFFu,
        joints.x >> 24u,
        joints.y & 0xFFu,
        (joints.y >> 8u) & 0xFFu,
        (joints.y >> 16u) & 0xFFu,
        joints.y >> 24u,
    );
    var weights = array<f32, 8>(
        weights0.x,
        weights0.y,
        weights0.z,
        weights0.w,
        weights1.x,
        weights1.y,
        weights1.z,
        weights1.w,
    );
    let weight_sum = weights0.x + weights0.y + weights0.z + weights0.w
        + weights1.x + weights1.y + weights1.z + weights1.w;
    let inv_weight_sum = 1.0 / max(weight_sum, 0.000001);
    var skinned = vec4<f32>(0.0);
    var skinned_normal = vec3<f32>(0.0);
    for (var slot = 0u; slot < 8u; slot++) {
        let index = joint_indices[slot];
        if index < joint_data.count {
            let joint = joint_data.matrices[index];
            let w = weights[slot] * inv_weight_sum;
            skinned += w * (joint * position);
            skinned_normal += w * (mat3x3<f32>(joint[0].xyz, joint[1].xyz, joint[2].xyz) * normal);
        }
    }
    var out: SkinnedPosition;
    out.position = skinned;
    out.normal = skinned_normal;
    return out;
}
