use crate::mdl_metadata::{MdlMeshMetadata, MdlMeshRangeMetadata, mdl_metadata_from_mdl_bytes};
use crate::model::{
    ModelBlendIndices, ModelBlendWeights, ModelBoneTable, ModelShapeInfo, ModelSubmeshInfo,
    ModelVertex, WeaponModelVertex,
};

const MODEL_FILE_HEADER_SIZE: usize = 68;
const VERTEX_DECLARATION_SIZE: usize = 17 * 8;
const VERTEX_STREAM_END: u8 = 0xff;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct MdlGeometryMesh {
    pub mesh_index: usize,
    pub category: String,
    pub material_index: u16,
    pub material_name: String,
    pub bone_table: Option<ModelBoneTable>,
    pub shape_targets: Vec<MdlGeometryShapeTarget>,
    pub vertices: Vec<WeaponModelVertex>,
    pub indices: Vec<u16>,
    pub submeshes: Vec<MdlGeometrySubmesh>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct MdlGeometryShapeTarget {
    pub info: ModelShapeInfo,
    pub replacements: Vec<MdlGeometryShapeReplacement>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct MdlGeometryShapeReplacement {
    pub base_indices_index: usize,
    pub replacing_vertex_index: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MdlGeometrySubmesh {
    pub info: ModelSubmeshInfo,
    pub index_offset: usize,
    pub index_count: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct VertexElement {
    stream: u8,
    offset: u8,
    vertex_type: u8,
    usage: u8,
    usage_index: u8,
}

pub(crate) fn extract_mdl_lod0_geometry(
    path: &str,
    bytes: &[u8],
) -> anyhow::Result<Vec<MdlGeometryMesh>> {
    let metadata = mdl_metadata_from_mdl_bytes(path, bytes)?;
    let lod = metadata
        .lods
        .first()
        .ok_or_else(|| anyhow::anyhow!("model {path} has no LOD0 metadata"))?;
    let declarations = parse_vertex_declarations(
        bytes,
        usize::from(metadata.file_header.vertex_declaration_count),
    )?;
    let mesh_indices = mesh_indices_from_ranges(&lod.mesh_ranges);
    let mut meshes = Vec::new();

    for (mesh_index, category) in mesh_indices {
        let Some(mesh) = metadata.meshes.get(mesh_index) else {
            continue;
        };
        if mesh.vertex_count == 0 || mesh.index_count == 0 {
            continue;
        }
        let Some(declaration) = declarations.get(mesh_index) else {
            continue;
        };

        let vertices = read_mesh_vertices(
            bytes,
            metadata.file_header.vertex_offsets[0] as usize,
            mesh,
            declaration,
        )?;
        let indices = read_mesh_indices(
            bytes,
            metadata.file_header.index_offsets[0] as usize,
            mesh.start_index as usize,
            mesh.index_count as usize,
        )?;
        if indices
            .iter()
            .any(|index| usize::from(*index) >= vertices.len())
        {
            anyhow::bail!(
                "mesh {mesh_index} in {path} contains an index outside its vertex buffer"
            );
        }

        let material_name = mesh
            .material_name
            .clone()
            .unwrap_or_else(|| format!("material-{}", mesh.material_index));
        let shape_targets = geometry_shape_targets(&metadata, mesh.start_index);
        meshes.push(MdlGeometryMesh {
            mesh_index,
            category,
            material_index: mesh.material_index,
            material_name,
            bone_table: mesh.bone_table.as_ref().map(model_bone_table_from_metadata),
            shape_targets,
            vertices,
            indices,
            submeshes: geometry_submeshes(mesh),
        });
    }

    Ok(meshes)
}

fn model_bone_table_from_metadata(
    table: &crate::mdl_metadata::MdlBoneTableMetadata,
) -> ModelBoneTable {
    ModelBoneTable {
        index: table.index,
        bone_count: table.bone_count,
        bone_indices: table.bone_indices.clone(),
        bone_names: table.bone_names.clone(),
    }
}

fn mesh_indices_from_ranges(ranges: &[MdlMeshRangeMetadata]) -> Vec<(usize, String)> {
    let mut indices = Vec::new();
    for range in ranges {
        for mesh_index in range.mesh_index..range.mesh_end {
            let mesh_index = usize::from(mesh_index);
            if !indices
                .iter()
                .any(|(existing_index, _)| *existing_index == mesh_index)
            {
                indices.push((mesh_index, range.category.clone()));
            }
        }
    }
    indices
}

fn geometry_shape_targets(
    metadata: &crate::mdl_metadata::MdlMetadata,
    mesh_start_index: u32,
) -> Vec<MdlGeometryShapeTarget> {
    let mut targets = Vec::new();
    for shape in &metadata.shapes {
        let start = usize::from(shape.shape_mesh_start_indices[0]);
        let count = usize::from(shape.shape_mesh_counts[0]);
        for shape_mesh_index in start..start.saturating_add(count) {
            let Some(shape_mesh) = metadata.shape_meshes.get(shape_mesh_index) else {
                continue;
            };
            if shape_mesh.mesh_index_offset != mesh_start_index {
                continue;
            }
            let shape_index_mask = shape_index_mask(shape.index);
            let info = ModelShapeInfo {
                index: shape.index,
                name: shape.name.clone(),
                shape_index_mask,
                shape_index_mask_hex: format!("0x{shape_index_mask:08X}"),
                shape_mesh_index,
                shape_value_count: shape_mesh.shape_value_count,
            };
            let value_start = shape_mesh.shape_value_offset as usize;
            let value_end = value_start.saturating_add(shape_mesh.shape_value_count as usize);
            let replacements = metadata.shape_values[value_start.min(metadata.shape_values.len())
                ..value_end.min(metadata.shape_values.len())]
                .iter()
                .map(|value| MdlGeometryShapeReplacement {
                    base_indices_index: usize::from(value.base_indices_index),
                    replacing_vertex_index: usize::from(value.replacing_vertex_index),
                })
                .collect();
            targets.push(MdlGeometryShapeTarget { info, replacements });
        }
    }

    targets.sort_by_key(|target| (target.info.index, target.info.shape_mesh_index));
    targets
}

fn shape_index_mask(shape_index: usize) -> u32 {
    1_u32.checked_shl(shape_index as u32).unwrap_or(0)
}

fn geometry_submeshes(mesh: &MdlMeshMetadata) -> Vec<MdlGeometrySubmesh> {
    mesh.submeshes
        .iter()
        .enumerate()
        .filter_map(|(index, submesh)| {
            let offset = submesh
                .relative_index_offset
                .and_then(|offset| usize::try_from(offset).ok())
                .or_else(|| {
                    submesh
                        .index_offset
                        .checked_sub(mesh.start_index)
                        .map(|offset| offset as usize)
                })?;
            Some(MdlGeometrySubmesh {
                info: ModelSubmeshInfo {
                    index,
                    table_index: submesh.table_index,
                    attribute_index_mask: submesh.attribute_index_mask,
                    attribute_index_mask_hex: submesh.attribute_index_mask_hex.clone(),
                    attribute_names: submesh.attribute_names.clone(),
                    bone_start_index: submesh.bone_start_index,
                    bone_count: submesh.bone_count,
                },
                index_offset: offset,
                index_count: submesh.index_count as usize,
            })
        })
        .collect()
}

fn parse_vertex_declarations(
    bytes: &[u8],
    declaration_count: usize,
) -> anyhow::Result<Vec<Vec<VertexElement>>> {
    let mut declarations = Vec::with_capacity(declaration_count);
    for declaration_index in 0..declaration_count {
        let declaration_offset =
            MODEL_FILE_HEADER_SIZE + declaration_index * VERTEX_DECLARATION_SIZE;
        let mut elements = Vec::new();
        for element_index in 0..17 {
            let offset = declaration_offset + element_index * 8;
            let element = read_vertex_element(bytes, offset)?;
            if element.stream == VERTEX_STREAM_END {
                break;
            }
            elements.push(element);
        }
        declarations.push(elements);
    }
    Ok(declarations)
}

fn read_vertex_element(bytes: &[u8], offset: usize) -> anyhow::Result<VertexElement> {
    let bytes = read_bytes(bytes, offset, 5, "vertex element")?;
    Ok(VertexElement {
        stream: bytes[0],
        offset: bytes[1],
        vertex_type: bytes[2],
        usage: bytes[3],
        usage_index: bytes[4],
    })
}

fn read_mesh_vertices(
    bytes: &[u8],
    vertex_buffer_offset: usize,
    mesh: &MdlMeshMetadata,
    declaration: &[VertexElement],
) -> anyhow::Result<Vec<WeaponModelVertex>> {
    let mut vertices = vec![default_model_vertex(); usize::from(mesh.vertex_count)];
    for vertex_index in 0..usize::from(mesh.vertex_count) {
        for element in declaration {
            let stream = usize::from(element.stream);
            if stream >= 3 || stream >= usize::from(mesh.vertex_stream_count) {
                continue;
            }
            let stride = usize::from(mesh.vertex_buffer_strides[stream]);
            if stride == 0 {
                continue;
            }
            let offset = vertex_buffer_offset
                .checked_add(mesh.vertex_buffer_offsets[stream] as usize)
                .and_then(|offset| offset.checked_add(vertex_index.checked_mul(stride)?))
                .and_then(|offset| offset.checked_add(usize::from(element.offset)))
                .ok_or_else(|| anyhow::anyhow!("mesh vertex offset overflow"))?;
            apply_vertex_element(&mut vertices[vertex_index], bytes, offset, *element)?;
        }
    }

    for vertex in &mut vertices {
        vertex.normal = normalized_or_fallback(vertex.normal);
        vertex.bitangent = sanitized_bitangent(vertex.bitangent);
        vertex.normal1 = vertex.normal1.map(normalized_or_fallback);
        vertex.bitangent1 = vertex.bitangent1.map(sanitized_bitangent);
    }
    Ok(vertices)
}

fn apply_vertex_element(
    vertex: &mut ModelVertex,
    bytes: &[u8],
    offset: usize,
    element: VertexElement,
) -> anyhow::Result<()> {
    match element.usage {
        0 => {
            vertex.position = read_vec3(bytes, offset, element.vertex_type)?;
        }
        1 => {
            vertex.blend_weights = Some(read_blend_weights(bytes, offset, element.vertex_type)?);
        }
        2 => {
            vertex.blend_indices = Some(read_blend_indices(bytes, offset, element.vertex_type)?);
        }
        3 => {
            let normal = read_vec3(bytes, offset, element.vertex_type)?;
            match element.usage_index {
                0 => {
                    vertex.normal = normal;
                }
                1 => {
                    vertex.normal1 = Some(normal);
                }
                _ => {}
            }
        }
        4 => {
            let uv = read_vec4(bytes, offset, element.vertex_type)?;
            apply_texcoord(vertex, element.usage_index, uv);
        }
        5 => {
            let flow = read_vec4(bytes, offset, element.vertex_type)?;
            match element.usage_index {
                0 => {
                    vertex.flow0 = Some(flow);
                }
                1 => {
                    vertex.flow1 = Some(flow);
                }
                _ => {}
            }
        }
        6 => {
            let bitangent = read_tangent(bytes, offset)?;
            match element.usage_index {
                0 => {
                    vertex.bitangent = bitangent;
                }
                1 => {
                    vertex.bitangent1 = Some(bitangent);
                }
                _ => {}
            }
        }
        7 => {
            let color = read_vec4(bytes, offset, element.vertex_type)?;
            match element.usage_index {
                0 => {
                    vertex.color = color;
                }
                1 => {
                    vertex.color1 = Some(color);
                }
                _ => {}
            }
        }
        _ => {}
    }
    Ok(())
}

fn read_mesh_indices(
    bytes: &[u8],
    index_buffer_offset: usize,
    start_index: usize,
    index_count: usize,
) -> anyhow::Result<Vec<u16>> {
    let offset = index_buffer_offset
        .checked_add(start_index.checked_mul(2).ok_or_else(|| {
            anyhow::anyhow!("mesh index buffer offset overflow at index {start_index}")
        })?)
        .ok_or_else(|| anyhow::anyhow!("mesh index buffer offset overflow"))?;
    let index_bytes = read_bytes(bytes, offset, index_count * 2, "mesh index buffer")?;
    Ok(index_bytes
        .chunks_exact(2)
        .map(|bytes| u16::from_le_bytes([bytes[0], bytes[1]]))
        .collect())
}

fn default_model_vertex() -> ModelVertex {
    ModelVertex {
        position: [0.0; 3],
        blend_weights: None,
        blend_indices: None,
        normal: [0.0; 3],
        uv0: [0.0; 2],
        uv1: [0.0; 2],
        uv2: [0.0; 2],
        uv3: [0.0; 2],
        bitangent: [0.0; 4],
        normal1: None,
        bitangent1: None,
        color: [1.0; 4],
        color1: None,
        flow0: None,
        flow1: None,
    }
}

fn read_blend_weights(
    bytes: &[u8],
    offset: usize,
    vertex_type: u8,
) -> anyhow::Result<ModelBlendWeights> {
    let raw = match vertex_type {
        5 | 8 => read_bytes(bytes, offset, 4, "blend weights vertex value")?,
        17 => read_bytes(bytes, offset, 8, "blend weights vertex value")?,
        _ => anyhow::bail!("unsupported blend weights vertex type {vertex_type}"),
    };
    let mut values = [0.0; 8];
    for (index, value) in raw.iter().enumerate() {
        values[index] = f32::from(*value) / 255.0;
    }
    Ok(ModelBlendWeights {
        count: raw.len() as u8,
        values,
    })
}

fn read_blend_indices(
    bytes: &[u8],
    offset: usize,
    vertex_type: u8,
) -> anyhow::Result<ModelBlendIndices> {
    let raw = match vertex_type {
        5 => read_bytes(bytes, offset, 4, "blend indices vertex value")?,
        17 => read_bytes(bytes, offset, 8, "blend indices vertex value")?,
        _ => anyhow::bail!("unsupported blend indices vertex type {vertex_type}"),
    };
    let mut values = [0; 8];
    values[..raw.len()].copy_from_slice(raw);
    Ok(ModelBlendIndices {
        count: raw.len() as u8,
        values,
    })
}

fn apply_texcoord(vertex: &mut ModelVertex, usage_index: u8, uv: [f32; 4]) {
    match usage_index {
        0 => {
            vertex.uv0 = [uv[0], uv[1]];
            vertex.uv1 = [uv[2], uv[3]];
        }
        1 => {
            vertex.uv2 = [uv[0], uv[1]];
            vertex.uv3 = [uv[2], uv[3]];
        }
        _ => {}
    }
}

fn read_vec3(bytes: &[u8], offset: usize, vertex_type: u8) -> anyhow::Result<[f32; 3]> {
    match vertex_type {
        0 => {
            let value = read_f32_le(bytes, offset, "single1 vertex value")?;
            Ok([value, value, value])
        }
        2 => Ok([
            read_f32_le(bytes, offset, "single3 vertex x")?,
            read_f32_le(bytes, offset + 4, "single3 vertex y")?,
            read_f32_le(bytes, offset + 8, "single3 vertex z")?,
        ]),
        3 => Ok([
            read_f32_le(bytes, offset, "single4 vertex x")?,
            read_f32_le(bytes, offset + 4, "single4 vertex y")?,
            read_f32_le(bytes, offset + 8, "single4 vertex z")?,
        ]),
        8 => {
            let values = read_byte_float4(bytes, offset)?;
            Ok([values[0], values[1], values[2]])
        }
        14 => {
            let values = read_half4(bytes, offset)?;
            Ok([values[0], values[1], values[2]])
        }
        _ => anyhow::bail!("unsupported vec3 vertex type {vertex_type}"),
    }
}

fn read_vec4(bytes: &[u8], offset: usize, vertex_type: u8) -> anyhow::Result<[f32; 4]> {
    match vertex_type {
        1 => Ok([
            read_f32_le(bytes, offset, "single2 vertex x")?,
            read_f32_le(bytes, offset + 4, "single2 vertex y")?,
            0.0,
            0.0,
        ]),
        3 => Ok([
            read_f32_le(bytes, offset, "single4 vertex x")?,
            read_f32_le(bytes, offset + 4, "single4 vertex y")?,
            read_f32_le(bytes, offset + 8, "single4 vertex z")?,
            read_f32_le(bytes, offset + 12, "single4 vertex w")?,
        ]),
        8 => read_byte_float4(bytes, offset),
        13 => {
            let values = read_half2(bytes, offset)?;
            Ok([values[0], values[1], 0.0, 0.0])
        }
        14 => read_half4(bytes, offset),
        _ => anyhow::bail!("unsupported vec4 vertex type {vertex_type}"),
    }
}

fn read_byte_float4(bytes: &[u8], offset: usize) -> anyhow::Result<[f32; 4]> {
    let bytes = read_bytes(bytes, offset, 4, "byte float4 vertex value")?;
    Ok([
        f32::from(bytes[0]) / 255.0,
        f32::from(bytes[1]) / 255.0,
        f32::from(bytes[2]) / 255.0,
        f32::from(bytes[3]) / 255.0,
    ])
}

fn read_tangent(bytes: &[u8], offset: usize) -> anyhow::Result<[f32; 4]> {
    let bytes = read_bytes(bytes, offset, 4, "tangent vertex value")?;
    let w = f32::from(bytes[3]) * 2.0 / 255.0 - 1.0;
    let sign = if w > 0.0 { 1.0 } else { -1.0 };
    Ok([
        f32::from(bytes[0]) * 2.0 / 255.0 - 1.0,
        f32::from(bytes[1]) * 2.0 / 255.0 - 1.0,
        f32::from(bytes[2]) * 2.0 / 255.0 - 1.0,
        sign,
    ])
}

fn read_half2(bytes: &[u8], offset: usize) -> anyhow::Result<[f32; 2]> {
    Ok([
        half_to_f32(read_u16_le(bytes, offset, "half2 vertex x")?),
        half_to_f32(read_u16_le(bytes, offset + 2, "half2 vertex y")?),
    ])
}

fn read_half4(bytes: &[u8], offset: usize) -> anyhow::Result<[f32; 4]> {
    Ok([
        half_to_f32(read_u16_le(bytes, offset, "half4 vertex x")?),
        half_to_f32(read_u16_le(bytes, offset + 2, "half4 vertex y")?),
        half_to_f32(read_u16_le(bytes, offset + 4, "half4 vertex z")?),
        half_to_f32(read_u16_le(bytes, offset + 6, "half4 vertex w")?),
    ])
}

fn half_to_f32(bits: u16) -> f32 {
    let sign = u32::from(bits & 0x8000) << 16;
    let exponent = (bits >> 10) & 0x1f;
    let mantissa = u32::from(bits & 0x03ff);
    let value = match exponent {
        0 => {
            if mantissa == 0 {
                sign
            } else {
                let mut mantissa = mantissa;
                let mut exponent = -14_i32;
                while (mantissa & 0x0400) == 0 {
                    mantissa <<= 1;
                    exponent -= 1;
                }
                mantissa &= 0x03ff;
                let exponent = u32::try_from(exponent + 127).unwrap_or(0);
                sign | (exponent << 23) | (mantissa << 13)
            }
        }
        0x1f => sign | 0x7f80_0000 | (mantissa << 13),
        _ => {
            let exponent = u32::from(exponent) + 112;
            sign | (exponent << 23) | (mantissa << 13)
        }
    };
    f32::from_bits(value)
}

fn normalized_or_fallback(normal: [f32; 3]) -> [f32; 3] {
    let length = (normal[0] * normal[0] + normal[1] * normal[1] + normal[2] * normal[2]).sqrt();
    if length.is_finite() && length > 0.0001 {
        [normal[0] / length, normal[1] / length, normal[2] / length]
    } else {
        [0.0, 1.0, 0.0]
    }
}

fn sanitized_bitangent(bitangent: [f32; 4]) -> [f32; 4] {
    let length =
        (bitangent[0] * bitangent[0] + bitangent[1] * bitangent[1] + bitangent[2] * bitangent[2])
            .sqrt();
    let xyz = if length.is_finite()
        && length > 0.0001
        && bitangent[..3].iter().all(|value| value.is_finite())
    {
        [
            bitangent[0] / length,
            bitangent[1] / length,
            bitangent[2] / length,
        ]
    } else {
        [1.0, 0.0, 0.0]
    };
    let sign = if bitangent[3].is_nan() || bitangent[3] > 0.0 {
        1.0
    } else {
        -1.0
    };
    [xyz[0], xyz[1], xyz[2], sign]
}

fn read_bytes<'a>(
    bytes: &'a [u8],
    offset: usize,
    byte_count: usize,
    label: &str,
) -> anyhow::Result<&'a [u8]> {
    let end = offset
        .checked_add(byte_count)
        .ok_or_else(|| anyhow::anyhow!("{label} offset overflow at {offset} + {byte_count}"))?;
    bytes
        .get(offset..end)
        .ok_or_else(|| anyhow::anyhow!("{label} is out of bounds at {offset}..{end}"))
}

fn read_u16_le(bytes: &[u8], offset: usize, label: &str) -> anyhow::Result<u16> {
    let bytes = read_bytes(bytes, offset, 2, label)?;
    Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
}

fn read_f32_le(bytes: &[u8], offset: usize, label: &str) -> anyhow::Result<f32> {
    let bytes = read_bytes(bytes, offset, 4, label)?;
    Ok(f32::from_le_bytes(bytes.try_into()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_extra_lod_meshes_from_raw_mdl() {
        let bytes = fixture_mdl_with_normal_and_glass_mesh();
        let meshes = extract_mdl_lod0_geometry("test.mdl", &bytes).expect("geometry");

        assert_eq!(meshes.len(), 2);
        assert_eq!(meshes[0].mesh_index, 0);
        assert_eq!(meshes[0].category, "normal");
        assert_eq!(meshes[0].material_index, 0);
        assert_eq!(meshes[0].vertices.len(), 3);
        assert_eq!(meshes[0].indices, vec![0, 1, 2]);
        assert_eq!(meshes[1].mesh_index, 1);
        assert_eq!(meshes[1].category, "glass");
        assert_eq!(meshes[1].material_index, 1);
        assert_eq!(meshes[1].material_name, "/mt_glass.mtrl");
        assert_eq!(meshes[1].submeshes[0].index_offset, 0);
        assert_eq!(meshes[1].submeshes[0].info.attribute_names, vec!["attr_a"]);
        assert_eq!(meshes[1].indices, vec![0, 1, 2]);
    }

    #[test]
    fn sanitized_bitangent_normalizes_xyz_and_sign() {
        let bitangent = sanitized_bitangent([0.0, 3.0, 4.0, 0.0]);

        assert_eq!(bitangent, [0.0, 0.6, 0.8, -1.0]);
    }

    #[test]
    fn sanitized_bitangent_falls_back_for_invalid_xyz() {
        let bitangent = sanitized_bitangent([0.0, 0.0, 0.0, f32::NAN]);

        assert_eq!(bitangent, [1.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn tangent_sign_uses_decoded_positive_w() {
        assert_eq!(read_tangent(&[128, 255, 128, 128], 0).unwrap()[3], 1.0);
        assert_eq!(read_tangent(&[128, 255, 128, 127], 0).unwrap()[3], -1.0);
    }

    #[test]
    fn blend_weights_and_indices_are_preserved() {
        let mut vertex = default_model_vertex();
        apply_vertex_element(
            &mut vertex,
            &[255, 128, 64, 0],
            0,
            VertexElement {
                stream: 0,
                offset: 0,
                vertex_type: 8,
                usage: 1,
                usage_index: 0,
            },
        )
        .expect("blend weights");
        apply_vertex_element(
            &mut vertex,
            &[3, 2, 1, 0],
            0,
            VertexElement {
                stream: 0,
                offset: 0,
                vertex_type: 5,
                usage: 2,
                usage_index: 0,
            },
        )
        .expect("blend indices");

        assert_eq!(
            vertex.blend_weights,
            Some(ModelBlendWeights {
                count: 4,
                values: [1.0, 128.0 / 255.0, 64.0 / 255.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            })
        );
        assert_eq!(
            vertex.blend_indices,
            Some(ModelBlendIndices {
                count: 4,
                values: [3, 2, 1, 0, 0, 0, 0, 0],
            })
        );
    }

    #[test]
    fn eight_slot_blend_weights_and_indices_are_preserved() {
        let mut vertex = default_model_vertex();
        apply_vertex_element(
            &mut vertex,
            &[255, 224, 192, 160, 128, 96, 64, 32],
            0,
            VertexElement {
                stream: 0,
                offset: 0,
                vertex_type: 17,
                usage: 1,
                usage_index: 0,
            },
        )
        .expect("eight-slot blend weights");
        apply_vertex_element(
            &mut vertex,
            &[10, 11, 12, 13, 14, 15, 16, 17],
            0,
            VertexElement {
                stream: 0,
                offset: 0,
                vertex_type: 17,
                usage: 2,
                usage_index: 0,
            },
        )
        .expect("eight-slot blend indices");

        assert_eq!(
            vertex.blend_weights,
            Some(ModelBlendWeights {
                count: 8,
                values: [
                    1.0,
                    224.0 / 255.0,
                    192.0 / 255.0,
                    160.0 / 255.0,
                    128.0 / 255.0,
                    96.0 / 255.0,
                    64.0 / 255.0,
                    32.0 / 255.0,
                ],
            })
        );
        assert_eq!(
            vertex.blend_indices,
            Some(ModelBlendIndices {
                count: 8,
                values: [10, 11, 12, 13, 14, 15, 16, 17],
            })
        );
    }

    #[test]
    fn extra_texcoord_usage_does_not_overwrite_primary_uvs() {
        let mut vertex = default_model_vertex();
        let primary = f32_bytes(&[0.1, 0.2, 0.3, 0.4]);
        apply_vertex_element(
            &mut vertex,
            &primary,
            0,
            VertexElement {
                stream: 0,
                offset: 0,
                vertex_type: 3,
                usage: 4,
                usage_index: 0,
            },
        )
        .expect("primary uv");

        let extra = f32_bytes(&[0.9, 0.8, 0.7, 0.6]);
        apply_vertex_element(
            &mut vertex,
            &extra,
            0,
            VertexElement {
                stream: 0,
                offset: 0,
                vertex_type: 3,
                usage: 4,
                usage_index: 1,
            },
        )
        .expect("extra uv");

        assert_eq!(vertex.uv0, [0.1, 0.2]);
        assert_eq!(vertex.uv1, [0.3, 0.4]);
        assert_eq!(vertex.uv2, [0.9, 0.8]);
        assert_eq!(vertex.uv3, [0.7, 0.6]);
    }

    #[test]
    fn explicit_black_vertex_color_is_preserved() {
        let mut vertex = default_model_vertex();
        apply_vertex_element(
            &mut vertex,
            &[0, 0, 0, 255],
            0,
            VertexElement {
                stream: 0,
                offset: 0,
                vertex_type: 8,
                usage: 7,
                usage_index: 0,
            },
        )
        .expect("vertex color");

        assert_eq!(vertex.color, [0.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn secondary_vertex_color_is_preserved() {
        let mut vertex = default_model_vertex();
        apply_vertex_element(
            &mut vertex,
            &[255, 128, 0, 64],
            0,
            VertexElement {
                stream: 0,
                offset: 0,
                vertex_type: 8,
                usage: 7,
                usage_index: 1,
            },
        )
        .expect("secondary vertex color");

        assert_eq!(vertex.color, [1.0, 1.0, 1.0, 1.0]);
        assert_eq!(vertex.color1, Some([1.0, 128.0 / 255.0, 0.0, 64.0 / 255.0]));
    }

    #[test]
    fn flow_vertex_attributes_are_preserved() {
        let mut vertex = default_model_vertex();
        apply_vertex_element(
            &mut vertex,
            &[0, 128, 255, 64],
            0,
            VertexElement {
                stream: 0,
                offset: 0,
                vertex_type: 8,
                usage: 5,
                usage_index: 0,
            },
        )
        .expect("primary flow");
        apply_vertex_element(
            &mut vertex,
            &[255, 0, 128, 32],
            0,
            VertexElement {
                stream: 0,
                offset: 0,
                vertex_type: 8,
                usage: 5,
                usage_index: 1,
            },
        )
        .expect("secondary flow");

        assert_eq!(vertex.flow0, Some([0.0, 128.0 / 255.0, 1.0, 64.0 / 255.0]));
        assert_eq!(vertex.flow1, Some([1.0, 0.0, 128.0 / 255.0, 32.0 / 255.0]));
    }

    #[test]
    fn secondary_normal_and_bitangent_are_preserved() {
        let mut vertex = default_model_vertex();
        apply_vertex_element(
            &mut vertex,
            &f32_bytes(&[0.0, 3.0, 4.0]),
            0,
            VertexElement {
                stream: 0,
                offset: 0,
                vertex_type: 2,
                usage: 3,
                usage_index: 1,
            },
        )
        .expect("secondary normal");
        apply_vertex_element(
            &mut vertex,
            &[128, 255, 128, 127],
            0,
            VertexElement {
                stream: 0,
                offset: 0,
                vertex_type: 8,
                usage: 6,
                usage_index: 1,
            },
        )
        .expect("secondary bitangent");

        assert_eq!(vertex.normal, [0.0, 0.0, 0.0]);
        assert_eq!(vertex.normal1, Some([0.0, 3.0, 4.0]));
        assert_eq!(vertex.bitangent, [0.0, 0.0, 0.0, 0.0]);
        assert_eq!(
            vertex.bitangent1,
            Some([
                128.0 * 2.0 / 255.0 - 1.0,
                1.0,
                128.0 * 2.0 / 255.0 - 1.0,
                -1.0,
            ])
        );
    }

    #[test]
    fn missing_vertex_color_defaults_to_white() {
        assert_eq!(default_model_vertex().color, [1.0, 1.0, 1.0, 1.0]);
        assert_eq!(default_model_vertex().blend_weights, None);
        assert_eq!(default_model_vertex().blend_indices, None);
        assert_eq!(default_model_vertex().normal1, None);
        assert_eq!(default_model_vertex().bitangent1, None);
        assert_eq!(default_model_vertex().color1, None);
        assert_eq!(default_model_vertex().flow0, None);
        assert_eq!(default_model_vertex().flow1, None);
    }

    fn fixture_mdl_with_normal_and_glass_mesh() -> Vec<u8> {
        const MODEL_HEADER_SIZE: usize = 56;
        const LOD_SIZE: usize = 60;
        const EXTRA_LOD_SIZE: usize = 40;
        const MESH_SIZE: usize = 36;
        const SUBMESH_SIZE: usize = 16;
        const STRIDE: usize = 40;

        let mut bytes = vec![0; MODEL_FILE_HEADER_SIZE];
        write_u32(&mut bytes, 0, 0x0100_0006);
        write_u16(&mut bytes, 12, 2);
        write_u16(&mut bytes, 14, 2);
        write_u8(&mut bytes, 64, 1);

        for _ in 0..2 {
            let declaration = bytes.len();
            bytes.extend_from_slice(&[0; VERTEX_DECLARATION_SIZE]);
            write_vertex_element(&mut bytes, declaration, 0, 0, 2, 0, 0);
            write_vertex_element(&mut bytes, declaration, 1, 12, 2, 3, 0);
            write_vertex_element(&mut bytes, declaration, 2, 24, 1, 4, 0);
            write_vertex_element(&mut bytes, declaration, 3, 32, 8, 6, 0);
            write_vertex_element(&mut bytes, declaration, 4, 36, 8, 7, 0);
            write_u8(&mut bytes, declaration + 5 * 8, VERTEX_STREAM_END);
        }

        let string_table = b"attr_a\0/mt_normal.mtrl\0/mt_glass.mtrl\0";
        let string_header = bytes.len();
        bytes.extend_from_slice(&[0; 8]);
        write_u16(&mut bytes, string_header, 3);
        write_u32(&mut bytes, string_header + 4, string_table.len() as u32);
        bytes.extend_from_slice(string_table);

        let model_header = bytes.len();
        bytes.extend_from_slice(&[0; MODEL_HEADER_SIZE]);
        write_f32(&mut bytes, model_header, 1.0);
        write_u16(&mut bytes, model_header + 0x04, 2);
        write_u16(&mut bytes, model_header + 0x06, 1);
        write_u16(&mut bytes, model_header + 0x08, 2);
        write_u16(&mut bytes, model_header + 0x0a, 2);
        write_u8(&mut bytes, model_header + 0x16, 1);
        write_u8(&mut bytes, model_header + 0x1b, 0x10);

        let lod0 = bytes.len();
        bytes.extend_from_slice(&[0; LOD_SIZE * 3]);
        write_u16(&mut bytes, lod0, 0);
        write_u16(&mut bytes, lod0 + 2, 1);

        let extra_lod0 = bytes.len();
        bytes.extend_from_slice(&[0; EXTRA_LOD_SIZE * 3]);
        write_u16(&mut bytes, extra_lod0 + 4, 1);
        write_u16(&mut bytes, extra_lod0 + 6, 1);

        for mesh_index in 0..2 {
            let mesh = bytes.len();
            bytes.extend_from_slice(&[0; MESH_SIZE]);
            write_u16(&mut bytes, mesh, 3);
            write_u32(&mut bytes, mesh + 4, 3);
            write_u16(&mut bytes, mesh + 8, mesh_index as u16);
            write_u16(&mut bytes, mesh + 10, mesh_index as u16);
            write_u16(&mut bytes, mesh + 12, 1);
            write_u32(&mut bytes, mesh + 16, (mesh_index * 3) as u32);
            write_u32(&mut bytes, mesh + 20, (mesh_index * 3 * STRIDE) as u32);
            write_u8(&mut bytes, mesh + 32, STRIDE as u8);
            write_u8(&mut bytes, mesh + 35, 1);
        }

        bytes.extend_from_slice(&0_u32.to_le_bytes());

        for submesh_index in 0..2 {
            let submesh = bytes.len();
            bytes.extend_from_slice(&[0; SUBMESH_SIZE]);
            write_u32(&mut bytes, submesh, (submesh_index * 3) as u32);
            write_u32(&mut bytes, submesh + 4, 3);
            write_u32(&mut bytes, submesh + 8, 1);
        }

        let material_a_offset = b"attr_a\0".len() as u32;
        let material_b_offset = b"attr_a\0/mt_normal.mtrl\0".len() as u32;
        bytes.extend_from_slice(&material_a_offset.to_le_bytes());
        bytes.extend_from_slice(&material_b_offset.to_le_bytes());

        let vertex_buffer_offset = bytes.len();
        write_u32(&mut bytes, 16, vertex_buffer_offset as u32);
        for mesh_index in 0..2 {
            for vertex_index in 0..3 {
                write_vertex(&mut bytes, mesh_index as f32, vertex_index as f32);
            }
        }

        let index_buffer_offset = bytes.len();
        write_u32(&mut bytes, 28, index_buffer_offset as u32);
        for index in [0_u16, 1, 2, 0, 1, 2] {
            bytes.extend_from_slice(&index.to_le_bytes());
        }

        bytes
    }

    fn write_vertex(bytes: &mut Vec<u8>, mesh: f32, vertex: f32) {
        bytes.extend_from_slice(
            &[mesh, vertex, 0.0]
                .into_iter()
                .flat_map(f32::to_le_bytes)
                .collect::<Vec<_>>(),
        );
        bytes.extend_from_slice(
            &[0.0_f32, 0.0, 1.0]
                .into_iter()
                .flat_map(f32::to_le_bytes)
                .collect::<Vec<_>>(),
        );
        bytes.extend_from_slice(
            &[0.0_f32, 0.0]
                .into_iter()
                .flat_map(f32::to_le_bytes)
                .collect::<Vec<_>>(),
        );
        bytes.extend_from_slice(&[255, 128, 128, 255]);
        bytes.extend_from_slice(&[255, 255, 255, 255]);
    }

    fn f32_bytes(values: &[f32]) -> Vec<u8> {
        values
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect()
    }

    fn write_vertex_element(
        bytes: &mut [u8],
        declaration: usize,
        index: usize,
        element_offset: u8,
        vertex_type: u8,
        usage: u8,
        usage_index: u8,
    ) {
        let write_offset = declaration + index * 8;
        bytes[write_offset] = 0;
        bytes[write_offset + 1] = element_offset;
        bytes[write_offset + 2] = vertex_type;
        bytes[write_offset + 3] = usage;
        bytes[write_offset + 4] = usage_index;
    }

    fn write_u8(bytes: &mut [u8], offset: usize, value: u8) {
        bytes[offset] = value;
    }

    fn write_u16(bytes: &mut [u8], offset: usize, value: u16) {
        bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
    }

    fn write_u32(bytes: &mut Vec<u8>, offset: usize, value: u32) {
        if offset == bytes.len() {
            bytes.extend_from_slice(&value.to_le_bytes());
        } else {
            bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
    }

    fn write_f32(bytes: &mut [u8], offset: usize, value: f32) {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
}
