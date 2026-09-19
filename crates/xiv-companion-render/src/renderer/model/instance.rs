use super::*;

#[cfg(test)]
pub(crate) fn flatten_model<M: ModelRenderData + ?Sized>(
    model: &M,
) -> (Vec<GpuVertex>, Vec<u32>, Vec<DrawBatch>) {
    flatten_model_with_options(model, PreparedModelOptions::default())
}

#[cfg(test)]
pub(crate) fn flatten_model_with_options<M: ModelRenderData + ?Sized>(
    model: &M,
    prepared_options: PreparedModelOptions,
) -> (Vec<GpuVertex>, Vec<u32>, Vec<DrawBatch>) {
    let (vertices, indices, draw_batches, _joint_names) =
        flatten_model_with_options_and_skeleton(model, prepared_options, None);
    (vertices, indices, draw_batches)
}

/// 展平几何 + 构建实例 joint 表。`skeleton` 为 Some 时，全部渲染 mesh 的
/// bone_table 名按首见顺序并入实例 joint 表，顶点 blend 索引（bone_table
/// 绝对下标）经 `bone_table 名 → joint 下标` 重映射；无骨架时 joint 表为空、
/// 顶点写默认槽位（joint 数 0 的旧 shader 分支）。
pub(crate) fn flatten_model_with_options_and_skeleton<M: ModelRenderData + ?Sized>(
    model: &M,
    prepared_options: PreparedModelOptions,
    skeleton: Option<&xiv_companion_data::ModelSkeleton>,
) -> (Vec<GpuVertex>, Vec<u32>, Vec<DrawBatch>, Vec<String>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let mut draw_batches = Vec::new();
    let mut joint_names: Vec<String> = Vec::new();

    let prepared_model = prepare_model_for_render_with_options(model, prepared_options.clone());
    let component_offsets =
        model_component_preview_offsets(model, &prepared_model, &prepared_options);
    for prepared_mesh in &prepared_model.meshes {
        if prepared_mesh.mesh_hidden {
            continue;
        }
        if !prepared_mesh.renders_in_main_pass
            && !prepared_mesh
                .prepared_material
                .render_pass
                .uses_additive_pipeline()
        {
            continue;
        }
        let Some(mesh) = model.meshes().get(prepared_mesh.mesh_index) else {
            continue;
        };

        let mesh_vertices =
            model_mesh_vertices_with_shape_mask(mesh, prepared_options.enabled_shape_mask);
        let component_offset = component_offsets
            .get(prepared_mesh.mesh_index)
            .copied()
            .unwrap_or([0.0; 3]);
        let base = vertices.len() as u32;

        let skinning = skeleton
            .is_some_and(|skeleton| register_mesh_joint_names(mesh, skeleton, &mut joint_names));
        let bone_table = skinning.then(|| mesh.bone_table.as_ref()).flatten();
        vertices.extend(mesh_vertices.iter().map(|vertex| {
            let mut gpu_vertex = GpuVertex::from_model_vertex(vertex);
            for (position, offset) in gpu_vertex.position.iter_mut().zip(component_offset) {
                *position += offset;
            }
            if let Some(bone_table) = bone_table {
                let (joints, weights) = remap_vertex_skinning(vertex, bone_table, &joint_names);
                gpu_vertex = gpu_vertex.with_skinning(joints, weights);
            }
            gpu_vertex
        }));
        let index_start = indices.len() as u32;
        indices.extend(mesh.indices.iter().map(|index| base + *index));
        let transparent_triangles = if prepared_mesh
            .prepared_material
            .render_pass
            .sorts_back_to_front()
        {
            transparent_triangles(&mesh_vertices, &mesh.indices, base, component_offset)
        } else {
            Vec::new()
        };
        draw_batches.push(DrawBatch {
            material_slot: prepared_mesh.material_slot,
            material_bind_group_index: draw_batches.len(),
            draw_role: prepared_mesh.draw_role,
            index_start,
            index_count: mesh.indices.len() as u32,
            prepared_material: prepared_mesh.prepared_material,
            transparent_triangles,
        });
    }

    (vertices, indices, draw_batches, joint_names)
}

/// mesh 的 bone_table 名按表序并入实例 joint 表（去重，首见顺序）。skeleton
/// 参数仅用于确认按名匹配可用（缺失名回退单位阵 joint，见 joint_matrices）。
pub(crate) fn register_mesh_joint_names(
    mesh: &crate::ModelMesh,
    _skeleton: &xiv_companion_data::ModelSkeleton,
    joint_names: &mut Vec<String>,
) -> bool {
    let Some(bone_table) = &mesh.bone_table else {
        return false;
    };
    for name in bone_table.bone_names.iter().flatten() {
        if joint_names.len() >= MAX_JOINTS {
            eprintln!(
                "model skinning: joint table truncated at {MAX_JOINTS} bones (mesh {})",
                mesh.path
            );
            break;
        }
        if !joint_names.iter().any(|existing| existing == name) {
            joint_names.push(name.clone());
        }
    }
    true
}

/// 顶点 blend 数据 → 8 槽实例 joint 索引 + 原始权重。blend 索引是 **MDL
/// bone_table 的绝对下标**（submesh 的 bone_start_index/bone_count 只是该
/// submesh 引用窗口的声明，不参与重映射——对 d0001e0001_dwn 实测：按窗口
/// 偏移重映射会把 submesh 网格撕碎，绝对下标边长守恒）。名缺失/越界回退
/// joint 0（该 joint 矩阵由数据层回退为单位阵语义由 joint_matrices 保证；
/// 真实数据不会触发）。
pub(crate) fn remap_vertex_skinning(
    vertex: &crate::ModelVertex,
    bone_table: &xiv_companion_data::ModelBoneTable,
    joint_names: &[String],
) -> ([u8; 8], [f32; 8]) {
    let default = ([0u8; 8], [1.0f32, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
    let (Some(blend_weights), Some(blend_indices)) = (vertex.blend_weights, vertex.blend_indices)
    else {
        return default;
    };
    let slots = usize::from(blend_weights.count).clamp(1, 8);
    let mut joints = [0u8; 8];
    let mut weights = [0.0f32; 8];
    for slot in 0..slots {
        let window = usize::from(blend_indices.values[slot]);
        joints[slot] = bone_table
            .bone_names
            .get(window)
            .and_then(|name| name.as_ref())
            .and_then(|name| joint_names.iter().position(|existing| existing == name))
            .map(|joint| joint.min(u8::MAX as usize) as u8)
            .unwrap_or(0);
        weights[slot] = blend_weights.values[slot];
    }
    (joints, weights)
}

pub(crate) fn model_component_preview_offsets<M: ModelRenderData + ?Sized>(
    model: &M,
    prepared_model: &crate::PreparedModel,
    prepared_options: &PreparedModelOptions,
) -> Vec<[f32; 3]> {
    let mut offsets = vec![[0.0; 3]; model.meshes().len()];
    // 角色拼装等原位布局：全零偏移，全部件按游戏坐标重叠。
    if !prepared_options.component_preview_layout {
        return offsets;
    }
    let Some(max_component) = prepared_model
        .meshes
        .iter()
        .map(|mesh| model.mesh_component_index(mesh.mesh_index) as usize)
        .max()
    else {
        return offsets;
    };
    if max_component == 0 {
        return offsets;
    }

    let mut minimums = vec![[f32::INFINITY; 3]; max_component + 1];
    let mut maximums = vec![[f32::NEG_INFINITY; 3]; max_component + 1];
    let mut present = vec![false; max_component + 1];
    for prepared_mesh in &prepared_model.meshes {
        if prepared_mesh.mesh_hidden {
            continue;
        }
        if !prepared_mesh.renders_in_main_pass
            && !prepared_mesh
                .prepared_material
                .render_pass
                .uses_additive_pipeline()
        {
            continue;
        }
        let Some(mesh) = model.meshes().get(prepared_mesh.mesh_index) else {
            continue;
        };
        let component = model.mesh_component_index(prepared_mesh.mesh_index) as usize;
        for vertex in
            model_mesh_vertices_with_shape_mask(mesh, prepared_options.enabled_shape_mask).iter()
        {
            present[component] = true;
            for axis in 0..3 {
                minimums[component][axis] = minimums[component][axis].min(vertex.position[axis]);
                maximums[component][axis] = maximums[component][axis].max(vertex.position[axis]);
            }
        }
    }

    let components = present
        .iter()
        .enumerate()
        .filter_map(|(component, present)| present.then_some(component))
        .collect::<Vec<_>>();
    if components.len() < 2 {
        return offsets;
    }

    let largest_extent = components
        .iter()
        .flat_map(|component| {
            (0..3).map(|axis| maximums[*component][axis] - minimums[*component][axis])
        })
        .fold(0.0_f32, f32::max);
    let gap = (largest_extent * 0.16).max(0.04);
    let total_width = components
        .iter()
        .map(|component| maximums[*component][0] - minimums[*component][0])
        .sum::<f32>()
        + gap * (components.len().saturating_sub(1) as f32);
    let mut cursor = -total_width * 0.5;
    let mut component_offsets = vec![0.0_f32; max_component + 1];
    for component in components {
        let width = maximums[component][0] - minimums[component][0];
        let source_center = (minimums[component][0] + maximums[component][0]) * 0.5;
        let target_center = cursor + width * 0.5;
        component_offsets[component] = target_center - source_center;
        cursor += width + gap;
    }

    for (mesh_index, offset) in offsets.iter_mut().enumerate() {
        let component = model.mesh_component_index(mesh_index) as usize;
        offset[0] = component_offsets
            .get(component)
            .copied()
            .unwrap_or_default();
    }
    offsets
}

pub(crate) fn gpu_vertices_bounds(vertices: &[GpuVertex]) -> Option<([f32; 3], f32)> {
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for vertex in vertices {
        for axis in 0..3 {
            min[axis] = min[axis].min(vertex.position[axis]);
            max[axis] = max[axis].max(vertex.position[axis]);
        }
    }
    if min.iter().any(|value| !value.is_finite()) || max.iter().any(|value| !value.is_finite()) {
        return None;
    }
    let center = [
        (min[0] + max[0]) * 0.5,
        (min[1] + max[1]) * 0.5,
        (min[2] + max[2]) * 0.5,
    ];
    let radius = vertices
        .iter()
        .map(|vertex| {
            let x = vertex.position[0] - center[0];
            let y = vertex.position[1] - center[1];
            let z = vertex.position[2] - center[2];
            (x * x + y * y + z * z).sqrt()
        })
        .fold(0.0_f32, f32::max)
        .max(0.0001);
    Some((center, radius))
}

pub(crate) fn transparent_triangles(
    vertices: &[crate::ModelVertex],
    indices: &[u32],
    vertex_base: u32,
    offset: [f32; 3],
) -> Vec<TransparentTriangle> {
    indices
        .chunks_exact(3)
        .map(|triangle| {
            let center = (|| {
                let positions = [
                    vertices.get(triangle[0] as usize)?.position,
                    vertices.get(triangle[1] as usize)?.position,
                    vertices.get(triangle[2] as usize)?.position,
                ];
                Some([
                    (positions[0][0] + positions[1][0] + positions[2][0]) / 3.0 + offset[0],
                    (positions[0][1] + positions[1][1] + positions[2][1]) / 3.0 + offset[1],
                    (positions[0][2] + positions[1][2] + positions[2][2]) / 3.0 + offset[2],
                ])
            })()
            .unwrap_or([0.0; 3]);
            TransparentTriangle {
                indices: [
                    vertex_base + triangle[0],
                    vertex_base + triangle[1],
                    vertex_base + triangle[2],
                ],
                center,
            }
        })
        .collect()
}

pub(crate) fn sorted_transparent_triangles(
    draw_batches: &[DrawBatch],
    yaw: f32,
    pitch: f32,
) -> SortedTransparentDraws {
    let sort_dir = transparent_sort_direction(yaw, pitch);
    let mut triangles = draw_batches
        .iter()
        .enumerate()
        .filter(|(_, batch)| batch.pass().sorts_back_to_front())
        .flat_map(|(batch_index, batch)| {
            batch
                .transparent_triangles
                .iter()
                .map(move |triangle| (batch_index, triangle))
        })
        .collect::<Vec<_>>();
    triangles.sort_by(|(_, left), (_, right)| {
        let left_depth = glam::Vec3::from(left.center).dot(sort_dir);
        let right_depth = glam::Vec3::from(right.center).dot(sort_dir);
        right_depth
            .partial_cmp(&left_depth)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut sorted = SortedTransparentDraws {
        indices: Vec::with_capacity(triangles.len() * 3),
        draws: Vec::new(),
    };
    for (batch_index, triangle) in triangles {
        let index_start = sorted.indices.len() as u32;
        sorted.indices.extend_from_slice(&triangle.indices);
        if let Some(draw) = sorted
            .draws
            .last_mut()
            .filter(|draw| draw.batch_index == batch_index)
        {
            draw.index_count += 3;
        } else {
            sorted.draws.push(SortedTransparentDraw {
                batch_index,
                index_start,
                index_count: 3,
            });
        }
    }
    sorted
}

/// 跨实例全局透明排序的批次表核心：收集场景内全部实例的透明三角形（模型
/// 空间中心 · 视线方向），单张深度表 back-to-front 全局排序。各实例的有序
/// 索引流分别落到自己的 transparent index buffer（索引是实例顶点缓冲局部
/// 的），`draws` 记录全局绘制顺序，渲染时按需切换实例缓冲逐段绘制。实例间
/// 坐标同源（角色拼装：全部件按游戏坐标原位重叠），模型空间深度可直接比
/// 较，与单实例排序同为静态 bind pose 中心的近似。
pub(crate) fn sorted_scene_transparent_batches(
    instance_batches: &[&[DrawBatch]],
    yaw: f32,
    pitch: f32,
) -> SortedSceneTransparentDraws {
    let sort_dir = transparent_sort_direction(yaw, pitch);
    let mut triangles = instance_batches
        .iter()
        .enumerate()
        .flat_map(|(instance_index, batches)| {
            batches
                .iter()
                .enumerate()
                .filter(|(_, batch)| batch.pass().sorts_back_to_front())
                .flat_map(move |(batch_index, batch)| {
                    batch
                        .transparent_triangles
                        .iter()
                        .map(move |triangle| (instance_index, batch_index, triangle))
                })
        })
        .collect::<Vec<_>>();
    triangles.sort_by(|(_, _, left), (_, _, right)| {
        let left_depth = glam::Vec3::from(left.center).dot(sort_dir);
        let right_depth = glam::Vec3::from(right.center).dot(sort_dir);
        right_depth
            .partial_cmp(&left_depth)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut sorted = SortedSceneTransparentDraws {
        indices: vec![Vec::new(); instance_batches.len()],
        draws: Vec::new(),
    };
    for (instance_index, batch_index, triangle) in triangles {
        let instance_indices = &mut sorted.indices[instance_index];
        let index_start = instance_indices.len() as u32;
        instance_indices.extend_from_slice(&triangle.indices);
        let merge_into_last = sorted
            .draws
            .last_mut()
            .filter(|draw| draw.instance_index == instance_index && draw.batch_index == batch_index)
            .map(|draw| draw.index_count += 3);
        if merge_into_last.is_none() {
            sorted.draws.push(SceneTransparentDraw {
                instance_index,
                batch_index,
                index_start,
                index_count: 3,
            });
        }
    }
    sorted
}

/// 包围球并（AABB 包住各球再取最小外接球的保守近似，覆盖全部输入球）。
pub(crate) fn scene_bounds_from_spheres(spheres: &[([f32; 3], f32)]) -> ([f32; 3], f32) {
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for (center, radius) in spheres {
        for axis in 0..3 {
            min[axis] = min[axis].min(center[axis] - radius);
            max[axis] = max[axis].max(center[axis] + radius);
        }
    }
    if min.iter().any(|value| !value.is_finite()) {
        return ([0.0; 3], 1.0);
    }
    let center = [
        (min[0] + max[0]) * 0.5,
        (min[1] + max[1]) * 0.5,
        (min[2] + max[2]) * 0.5,
    ];
    let radius = spheres
        .iter()
        .map(|(sphere_center, sphere_radius)| {
            let delta = glam::Vec3::from(*sphere_center) - glam::Vec3::from(center);
            delta.length() + sphere_radius
        })
        .fold(0.0_f32, f32::max)
        .max(0.0001);
    (center, radius)
}

pub(crate) fn transparent_sort_direction(yaw: f32, pitch: f32) -> glam::Vec3 {
    let pitch = pitch.clamp(-1.35, 1.35);
    glam::Vec3::new(
        -yaw.sin() * pitch.cos(),
        -pitch.sin(),
        -yaw.cos() * pitch.cos(),
    )
    .normalize_or_zero()
}

pub(crate) fn draw_model_batch<'a>(
    render_pass: &mut wgpu::RenderPass<'a>,
    material_bind_groups: &'a [wgpu::BindGroup],
    batch: &DrawBatch,
) {
    draw_model_batch_range(
        render_pass,
        material_bind_groups,
        batch,
        batch.index_start,
        batch.index_count,
    );
}

pub(crate) fn draw_model_batch_range<'a>(
    render_pass: &mut wgpu::RenderPass<'a>,
    material_bind_groups: &'a [wgpu::BindGroup],
    batch: &DrawBatch,
    index_start: u32,
    index_count: u32,
) {
    if let Some(bind_group) = material_bind_groups
        .get(batch.material_bind_group_index)
        .or_else(|| material_bind_groups.first())
    {
        render_pass.set_bind_group(1, bind_group, &[]);
        render_pass.draw_indexed(index_start..index_start + index_count, 0, 0..1);
    }
}

#[derive(Clone, Debug)]
pub(crate) struct DrawBatch {
    pub(crate) material_slot: usize,
    pub(crate) material_bind_group_index: usize,
    pub(crate) draw_role: ModelMeshDrawRole,
    pub(crate) index_start: u32,
    pub(crate) index_count: u32,
    pub(crate) prepared_material: PreparedMaterial,
    pub(crate) transparent_triangles: Vec<TransparentTriangle>,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) struct TransparentTriangle {
    pub(crate) indices: [u32; 3],
    pub(crate) center: [f32; 3],
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SortedTransparentDraws {
    pub(crate) indices: Vec<u32>,
    pub(crate) draws: Vec<SortedTransparentDraw>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) struct SortedTransparentDraw {
    pub(crate) batch_index: usize,
    pub(crate) index_start: u32,
    pub(crate) index_count: u32,
}

/// 跨实例全局排序的透明绘制计划：`indices[instance]` 是该实例按全局深度序
/// 排好的索引流（写入实例自己的 transparent index buffer），`draws` 是跨
/// 实例的全局绘制顺序（相邻同实例同批次段合并）。
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct SortedSceneTransparentDraws {
    pub(crate) indices: Vec<Vec<u32>>,
    pub(crate) draws: Vec<SceneTransparentDraw>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) struct SceneTransparentDraw {
    pub(crate) instance_index: usize,
    pub(crate) batch_index: usize,
    pub(crate) index_start: u32,
    pub(crate) index_count: u32,
}

impl DrawBatch {
    pub(crate) fn pass(&self) -> PreparedRenderPass {
        self.prepared_material.render_pass
    }

    pub(crate) fn render_backfaces(&self) -> bool {
        self.prepared_material.render_backfaces
    }

    pub(crate) fn uses_dither_depth_prepass(&self) -> bool {
        self.pass().sorts_back_to_front()
            && matches!(
                self.prepared_material.alpha_policy.draw_depth_mode,
                MaterialDrawDepthMode::Dither
            )
    }

    pub(crate) fn uses_additive_glass_pipeline(
        &self,
        glass_blend_mode: ModelGlassBlendMode,
    ) -> bool {
        self.pass() == PreparedRenderPass::Glass
            && matches!(glass_blend_mode, ModelGlassBlendMode::Additive)
    }

    pub(crate) fn uses_outline_pass(&self) -> bool {
        self.prepared_material.feature_flags.uses_outline
            && !self
                .prepared_material
                .unsupported_inputs
                .outline_composition
            && matches!(
                self.draw_role,
                ModelMeshDrawRole::Normal
                    | ModelMeshDrawRole::Glass
                    | ModelMeshDrawRole::MaterialChange
            )
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct GpuVertex {
    pub(crate) position: [f32; 3],
    pub(crate) normal: [f32; 3],
    pub(crate) uv0: [f32; 2],
    pub(crate) bitangent: [f32; 4],
    pub(crate) color: [f32; 4],
    pub(crate) uv1: [f32; 2],
    pub(crate) uv2: [f32; 2],
    pub(crate) uv3: [f32; 2],
    pub(crate) color1: [f32; 4],
    pub(crate) normal1: [f32; 3],
    pub(crate) bitangent1: [f32; 4],
    pub(crate) flow0: [f32; 4],
    pub(crate) flow1: [f32; 4],
    // 蒙皮槽位（0..3 在 joints.x 低字节，4..7 在 joints.y）。8 个 u8 索引打包
    // 成一个 Uint32x2 顶点属性：保守 limits 下限（downlevel_webgl2_defaults）
    // 的 max vertex attributes 只有 16，joints0/1 各占一个 location 会超限。
    // 无蒙皮数据时
    // joints=0、weights0=(1,0,0,0)，走 shader 的 joint 数 0 旧分支，渲染与
    // 非蒙皮路径逐位一致。u32 槽位排在 f32 前，保持 Pod 无填充对齐。
    pub(crate) joints: [u32; 2],
    pub(crate) weights0: [f32; 4],
    pub(crate) weights1: [f32; 4],
}

impl GpuVertex {
    pub(crate) const ATTRIBUTES: [wgpu::VertexAttribute; 16] = [
        wgpu::VertexAttribute {
            offset: std::mem::offset_of!(GpuVertex, position) as wgpu::BufferAddress,
            shader_location: 0,
            format: wgpu::VertexFormat::Float32x3,
        },
        wgpu::VertexAttribute {
            offset: std::mem::offset_of!(GpuVertex, normal) as wgpu::BufferAddress,
            shader_location: 1,
            format: wgpu::VertexFormat::Float32x3,
        },
        wgpu::VertexAttribute {
            offset: std::mem::offset_of!(GpuVertex, uv0) as wgpu::BufferAddress,
            shader_location: 2,
            format: wgpu::VertexFormat::Float32x2,
        },
        wgpu::VertexAttribute {
            offset: std::mem::offset_of!(GpuVertex, bitangent) as wgpu::BufferAddress,
            shader_location: 3,
            format: wgpu::VertexFormat::Float32x4,
        },
        wgpu::VertexAttribute {
            offset: std::mem::offset_of!(GpuVertex, color) as wgpu::BufferAddress,
            shader_location: 4,
            format: wgpu::VertexFormat::Float32x4,
        },
        wgpu::VertexAttribute {
            offset: std::mem::offset_of!(GpuVertex, uv1) as wgpu::BufferAddress,
            shader_location: 5,
            format: wgpu::VertexFormat::Float32x2,
        },
        wgpu::VertexAttribute {
            offset: std::mem::offset_of!(GpuVertex, uv2) as wgpu::BufferAddress,
            shader_location: 6,
            format: wgpu::VertexFormat::Float32x2,
        },
        wgpu::VertexAttribute {
            offset: std::mem::offset_of!(GpuVertex, uv3) as wgpu::BufferAddress,
            shader_location: 7,
            format: wgpu::VertexFormat::Float32x2,
        },
        wgpu::VertexAttribute {
            offset: std::mem::offset_of!(GpuVertex, color1) as wgpu::BufferAddress,
            shader_location: 8,
            format: wgpu::VertexFormat::Float32x4,
        },
        wgpu::VertexAttribute {
            offset: std::mem::offset_of!(GpuVertex, normal1) as wgpu::BufferAddress,
            shader_location: 9,
            format: wgpu::VertexFormat::Float32x3,
        },
        wgpu::VertexAttribute {
            offset: std::mem::offset_of!(GpuVertex, bitangent1) as wgpu::BufferAddress,
            shader_location: 10,
            format: wgpu::VertexFormat::Float32x4,
        },
        wgpu::VertexAttribute {
            offset: std::mem::offset_of!(GpuVertex, flow0) as wgpu::BufferAddress,
            shader_location: 11,
            format: wgpu::VertexFormat::Float32x4,
        },
        wgpu::VertexAttribute {
            offset: std::mem::offset_of!(GpuVertex, flow1) as wgpu::BufferAddress,
            shader_location: 12,
            format: wgpu::VertexFormat::Float32x4,
        },
        wgpu::VertexAttribute {
            offset: std::mem::offset_of!(GpuVertex, joints) as wgpu::BufferAddress,
            shader_location: 13,
            format: wgpu::VertexFormat::Uint32x2,
        },
        wgpu::VertexAttribute {
            offset: std::mem::offset_of!(GpuVertex, weights0) as wgpu::BufferAddress,
            shader_location: 14,
            format: wgpu::VertexFormat::Float32x4,
        },
        wgpu::VertexAttribute {
            offset: std::mem::offset_of!(GpuVertex, weights1) as wgpu::BufferAddress,
            shader_location: 15,
            format: wgpu::VertexFormat::Float32x4,
        },
    ];

    pub(crate) fn from_model_vertex(vertex: &crate::ModelVertex) -> Self {
        Self {
            position: vertex.position,
            normal: vertex.normal,
            uv0: vertex.uv0,
            bitangent: vertex.bitangent,
            color: vertex.color,
            uv1: vertex.uv1,
            uv2: vertex.uv2,
            uv3: vertex.uv3,
            color1: vertex.color1.unwrap_or([1.0, 1.0, 1.0, 1.0]),
            normal1: vertex.normal1.unwrap_or(vertex.normal),
            bitangent1: vertex.bitangent1.unwrap_or(vertex.bitangent),
            flow0: vertex.flow0.unwrap_or([0.0; 4]),
            flow1: vertex.flow1.unwrap_or([0.0; 4]),
            joints: [0; 2],
            weights0: [1.0, 0.0, 0.0, 0.0],
            weights1: [0.0; 4],
        }
    }

    /// 写入 8 槽 joint 索引（实例 joint 表下标，低字节在前的 u8×8 打包）与
    /// 原始权重（GPU 按和归一）。
    pub(crate) fn with_skinning(mut self, joints: [u8; 8], weights: [f32; 8]) -> Self {
        self.joints = [
            u32::from_le_bytes([joints[0], joints[1], joints[2], joints[3]]),
            u32::from_le_bytes([joints[4], joints[5], joints[6], joints[7]]),
        ];
        self.weights0 = [weights[0], weights[1], weights[2], weights[3]];
        self.weights1 = [weights[4], weights[5], weights[6], weights[7]];
        self
    }

    pub(crate) fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<GpuVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBUTES,
        }
    }
}
