use crate::mdl_metadata::{MdlMeshMetadata, MdlMeshRangeMetadata, mdl_metadata_from_mdl_bytes};
use crate::model::{ModelVertex, WeaponModelVertex};

const MODEL_FILE_HEADER_SIZE: usize = 68;
const VERTEX_DECLARATION_SIZE: usize = 17 * 8;
const VERTEX_STREAM_END: u8 = 0xff;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct MdlGeometryMesh {
    pub mesh_index: usize,
    pub category: String,
    pub material_index: u16,
    pub material_name: String,
    pub vertices: Vec<WeaponModelVertex>,
    pub indices: Vec<u16>,
    pub submeshes: Vec<MdlGeometrySubmesh>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct MdlGeometrySubmesh {
    pub submesh_index: usize,
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
        meshes.push(MdlGeometryMesh {
            mesh_index,
            category,
            material_index: mesh.material_index,
            material_name,
            vertices,
            indices,
            submeshes: geometry_submeshes(mesh),
        });
    }

    Ok(meshes)
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
                submesh_index: index,
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
        3 => {
            if element.usage_index == 0 {
                vertex.normal = read_vec3(bytes, offset, element.vertex_type)?;
            }
        }
        4 => {
            let uv = read_vec4(bytes, offset, element.vertex_type)?;
            apply_texcoord(vertex, element.usage_index, uv);
        }
        6 => {
            if element.usage_index == 0 {
                vertex.bitangent = read_tangent(bytes, offset)?;
            }
        }
        7 => {
            if element.usage_index == 0 {
                vertex.color = read_vec4(bytes, offset, element.vertex_type)?;
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
        normal: [0.0; 3],
        uv0: [0.0; 2],
        uv1: [0.0; 2],
        uv2: [0.0; 2],
        uv3: [0.0; 2],
        bitangent: [0.0; 4],
        color: [1.0; 4],
    }
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
    fn missing_vertex_color_defaults_to_white() {
        assert_eq!(default_model_vertex().color, [1.0, 1.0, 1.0, 1.0]);
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
