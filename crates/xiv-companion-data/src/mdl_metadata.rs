use serde::Serialize;

const MODEL_FILE_HEADER_SIZE: usize = 68;
const VERTEX_DECLARATION_SIZE: usize = 17 * 8;
const MODEL_HEADER_SIZE: usize = 56;
const ELEMENT_ID_SIZE: usize = 32;
const LOD_SIZE: usize = 60;
const EXTRA_LOD_SIZE: usize = 40;
const MESH_SIZE: usize = 36;
const TERRAIN_SHADOW_MESH_SIZE: usize = 20;
const SUBMESH_SIZE: usize = 16;
const TERRAIN_SHADOW_SUBMESH_SIZE: usize = 12;
const SHAPE_SIZE: usize = 16;
const SHAPE_MESH_SIZE: usize = 12;
const SHAPE_VALUE_SIZE: usize = 4;
const MDL_VERSION_V5: u32 = 0x0100_0005;
const MDL_VERSION_V6: u32 = 0x0100_0006;
const V5_BONE_TABLE_SIZE: usize = 132;
const V5_BONE_TABLE_BONE_CAPACITY: usize = 64;

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MdlMetadata {
    pub path: String,
    pub file_header: MdlFileHeaderMetadata,
    pub string_count: u16,
    pub string_table_size: u32,
    pub model_header: MdlModelHeaderMetadata,
    pub lods: Vec<MdlLodMetadata>,
    pub extra_lods: Vec<MdlExtraLodMetadata>,
    pub meshes: Vec<MdlMeshMetadata>,
    pub terrain_shadow_meshes: Vec<MdlTerrainShadowMeshMetadata>,
    pub submeshes: Vec<MdlSubmeshMetadata>,
    pub terrain_shadow_submeshes: Vec<MdlTerrainShadowSubmeshMetadata>,
    pub attributes: Vec<MdlNamedOffset>,
    pub materials: Vec<MdlNamedOffset>,
    pub bones: Vec<MdlNamedOffset>,
    pub bone_tables: Vec<MdlBoneTableMetadata>,
    pub shapes: Vec<MdlShapeMetadata>,
    pub shape_meshes: Vec<MdlShapeMeshMetadata>,
    pub shape_values: Vec<MdlShapeValueMetadata>,
    pub submesh_bone_map_byte_size: u32,
    pub submesh_bone_map: Vec<u16>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MdlFileHeaderMetadata {
    pub version: u32,
    pub version_hex: String,
    pub stack_size: u32,
    pub runtime_size: u32,
    pub vertex_declaration_count: u16,
    pub material_count: u16,
    pub vertex_offsets: [u32; 3],
    pub index_offsets: [u32; 3],
    pub vertex_buffer_sizes: [u32; 3],
    pub index_buffer_sizes: [u32; 3],
    pub lod_count: u8,
    pub enable_index_buffer_streaming: bool,
    pub enable_edge_geometry: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MdlModelHeaderMetadata {
    pub radius: f32,
    pub mesh_count: u16,
    pub attribute_count: u16,
    pub submesh_count: u16,
    pub material_count: u16,
    pub bone_count: u16,
    pub bone_table_count: u16,
    pub shape_count: u16,
    pub shape_mesh_count: u16,
    pub shape_value_count: u16,
    pub lod_count: u8,
    pub flags1: u8,
    pub flags1_hex: String,
    pub element_id_count: u16,
    pub terrain_shadow_mesh_count: u8,
    pub flags2: u8,
    pub flags2_hex: String,
    pub has_extra_lods: bool,
    pub model_clip_out_distance: f32,
    pub shadow_clip_out_distance: f32,
    pub unknown4: u16,
    pub terrain_shadow_submesh_count: u16,
    pub flags3: u8,
    pub flags3_hex: String,
    pub bg_change_material_index: u8,
    pub bg_crest_change_material_index: u8,
    pub unknown6: u8,
    pub bone_table_array_count_total: u16,
    pub unknown8: u16,
    pub unknown9: u16,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MdlLodMetadata {
    pub index: usize,
    pub mesh_index: u16,
    pub mesh_count: u16,
    pub model_lod_range: f32,
    pub texture_lod_range: f32,
    pub water_mesh_index: u16,
    pub water_mesh_count: u16,
    pub shadow_mesh_index: u16,
    pub shadow_mesh_count: u16,
    pub terrain_shadow_mesh_index: u16,
    pub terrain_shadow_mesh_count: u16,
    pub vertical_fog_mesh_index: u16,
    pub vertical_fog_mesh_count: u16,
    pub edge_geometry_size: u32,
    pub edge_geometry_data_offset: u32,
    pub polygon_count: u32,
    pub unknown1: u32,
    pub vertex_buffer_size: u32,
    pub index_buffer_size: u32,
    pub vertex_data_offset: u32,
    pub index_data_offset: u32,
    pub mesh_ranges: Vec<MdlMeshRangeMetadata>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MdlExtraLodMetadata {
    pub index: usize,
    pub light_shaft_mesh_index: u16,
    pub light_shaft_mesh_count: u16,
    pub glass_mesh_index: u16,
    pub glass_mesh_count: u16,
    pub material_change_mesh_index: u16,
    pub material_change_mesh_count: u16,
    pub crest_change_mesh_index: u16,
    pub crest_change_mesh_count: u16,
    pub unknowns: [u16; 12],
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MdlMeshRangeMetadata {
    pub category: String,
    pub mesh_index: u16,
    pub mesh_count: u16,
    pub mesh_end: u16,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MdlMeshMetadata {
    pub index: usize,
    pub vertex_count: u16,
    pub padding: u16,
    pub index_count: u32,
    pub material_index: u16,
    pub material_name: Option<String>,
    pub submesh_index: u16,
    pub submesh_count: u16,
    pub submesh_indices: Vec<usize>,
    pub submeshes: Vec<MdlMeshSubmeshMetadata>,
    pub bone_table_index: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bone_table: Option<MdlBoneTableMetadata>,
    pub start_index: u32,
    pub vertex_buffer_offsets: [u32; 3],
    pub vertex_buffer_strides: [u8; 3],
    pub vertex_stream_count: u8,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MdlMeshSubmeshMetadata {
    pub table_index: usize,
    pub index_offset: u32,
    pub relative_index_offset: Option<i64>,
    pub index_count: u32,
    pub attribute_index_mask: u32,
    pub attribute_index_mask_hex: String,
    pub attribute_names: Vec<String>,
    pub bone_start_index: u16,
    pub bone_count: u16,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MdlSubmeshMetadata {
    pub index: usize,
    pub index_offset: u32,
    pub index_count: u32,
    pub attribute_index_mask: u32,
    pub attribute_index_mask_hex: String,
    pub attribute_names: Vec<String>,
    pub bone_start_index: u16,
    pub bone_count: u16,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MdlTerrainShadowMeshMetadata {
    pub index: usize,
    pub index_count: u32,
    pub start_index: u32,
    pub vertex_buffer_offset: u32,
    pub vertex_count: u16,
    pub submesh_index: u16,
    pub submesh_count: u16,
    pub vertex_buffer_stride: u8,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MdlTerrainShadowSubmeshMetadata {
    pub index: usize,
    pub index_offset: u32,
    pub index_count: u32,
    pub unknown1: u16,
    pub unknown2: u16,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MdlNamedOffset {
    pub index: usize,
    pub offset: u32,
    pub name: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MdlBoneTableMetadata {
    pub index: usize,
    pub bone_count: u32,
    pub bone_indices: Vec<u16>,
    pub bone_names: Vec<Option<String>>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MdlShapeMetadata {
    pub index: usize,
    pub string_offset: u32,
    pub name: Option<String>,
    pub shape_mesh_start_indices: [u16; 3],
    pub shape_mesh_counts: [u16; 3],
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MdlShapeMeshMetadata {
    pub index: usize,
    pub mesh_index_offset: u32,
    pub shape_value_count: u32,
    pub shape_value_offset: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MdlShapeValueMetadata {
    pub index: usize,
    pub base_indices_index: u16,
    pub replacing_vertex_index: u16,
}

pub fn mdl_metadata_from_mdl_bytes(path: &str, bytes: &[u8]) -> anyhow::Result<MdlMetadata> {
    let file_header = parse_file_header(bytes, 0)?;
    let mut offset = MODEL_FILE_HEADER_SIZE;
    offset = checked_advance(
        offset,
        usize::from(file_header.vertex_declaration_count) * VERTEX_DECLARATION_SIZE,
        bytes.len(),
        "vertex declarations",
    )?;

    let string_count = read_u16_le(bytes, offset, "string count")?;
    let string_table_size = read_u32_le(bytes, offset + 4, "string table size")?;
    offset = checked_advance(offset, 8, bytes.len(), "string table header")?;
    let string_table = read_bytes(
        bytes,
        offset,
        string_table_size as usize,
        "string table bytes",
    )?;
    offset = checked_advance(
        offset,
        string_table_size as usize,
        bytes.len(),
        "string table",
    )?;

    let model_header = parse_model_header(bytes, offset)?;
    offset = checked_advance(offset, MODEL_HEADER_SIZE, bytes.len(), "model header")?;
    offset = checked_advance(
        offset,
        usize::from(model_header.element_id_count) * ELEMENT_ID_SIZE,
        bytes.len(),
        "element ids",
    )?;

    let mut lods = Vec::with_capacity(3);
    for index in 0..3 {
        lods.push(parse_lod(bytes, offset, index)?);
        offset = checked_advance(offset, LOD_SIZE, bytes.len(), "lod table")?;
    }

    let mut extra_lods = Vec::new();
    if model_header.has_extra_lods {
        extra_lods.reserve(3);
        for index in 0..3 {
            extra_lods.push(parse_extra_lod(bytes, offset, index)?);
            offset = checked_advance(offset, EXTRA_LOD_SIZE, bytes.len(), "extra lod table")?;
        }
    }

    let raw_meshes = (0..usize::from(model_header.mesh_count))
        .map(|index| {
            let mesh = parse_raw_mesh(bytes, offset, index)?;
            offset = checked_advance(offset, MESH_SIZE, bytes.len(), "mesh table")?;
            Ok(mesh)
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    let attribute_offsets = read_u32_table(
        bytes,
        &mut offset,
        usize::from(model_header.attribute_count),
        "attribute name offsets",
    )?;
    let terrain_shadow_meshes = (0..usize::from(model_header.terrain_shadow_mesh_count))
        .map(|index| {
            let mesh = parse_terrain_shadow_mesh(bytes, offset, index)?;
            offset = checked_advance(
                offset,
                TERRAIN_SHADOW_MESH_SIZE,
                bytes.len(),
                "terrain shadow mesh table",
            )?;
            Ok(mesh)
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    let raw_submeshes = (0..usize::from(model_header.submesh_count))
        .map(|index| {
            let submesh = parse_raw_submesh(bytes, offset, index)?;
            offset = checked_advance(offset, SUBMESH_SIZE, bytes.len(), "submesh table")?;
            Ok(submesh)
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    let terrain_shadow_submeshes = (0..usize::from(model_header.terrain_shadow_submesh_count))
        .map(|index| {
            let submesh = parse_terrain_shadow_submesh(bytes, offset, index)?;
            offset = checked_advance(
                offset,
                TERRAIN_SHADOW_SUBMESH_SIZE,
                bytes.len(),
                "terrain shadow submesh table",
            )?;
            Ok(submesh)
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    let material_offsets = read_u32_table(
        bytes,
        &mut offset,
        usize::from(model_header.material_count),
        "material name offsets",
    )?;
    let bone_offsets = read_u32_table(
        bytes,
        &mut offset,
        usize::from(model_header.bone_count),
        "bone name offsets",
    )?;

    let attributes = named_offsets(&attribute_offsets, string_table);
    let materials = named_offsets(&material_offsets, string_table);
    let bones = named_offsets(&bone_offsets, string_table);
    let bone_tables = read_bone_tables(
        bytes,
        &mut offset,
        file_header.version,
        usize::from(model_header.bone_table_count),
        usize::from(model_header.bone_table_array_count_total),
        &bones,
    )?;
    let shapes = read_shapes(
        bytes,
        &mut offset,
        usize::from(model_header.shape_count),
        string_table,
    )?;
    let shape_meshes = read_shape_meshes(
        bytes,
        &mut offset,
        usize::from(model_header.shape_mesh_count),
    )?;
    let shape_values = read_shape_values(
        bytes,
        &mut offset,
        usize::from(model_header.shape_value_count),
    )?;
    let submesh_bone_map_byte_size = read_u32_le(bytes, offset, "submesh bone map byte size")?;
    offset = checked_advance(offset, 4, bytes.len(), "submesh bone map byte size")?;
    let submesh_bone_map_byte_count =
        usize::try_from(submesh_bone_map_byte_size).map_err(|_| {
            anyhow::anyhow!(
                "submesh bone map byte size does not fit usize: {submesh_bone_map_byte_size}"
            )
        })?;
    let submesh_bone_map = read_u16_table(
        bytes,
        offset,
        submesh_bone_map_byte_count / 2,
        "submesh bone map",
    )?;
    checked_advance(
        offset,
        submesh_bone_map_byte_count,
        bytes.len(),
        "submesh bone map",
    )?;
    let submeshes = raw_submeshes
        .into_iter()
        .map(|submesh| submesh.with_attribute_names(&attributes))
        .collect::<Vec<_>>();
    let meshes = raw_meshes
        .into_iter()
        .map(|mesh| mesh.with_tables(&materials, &submeshes, &bone_tables))
        .collect::<Vec<_>>();

    for (lod, extra_lod) in lods.iter_mut().zip(
        extra_lods
            .iter()
            .map(Some)
            .chain(std::iter::repeat(None))
            .take(3),
    ) {
        lod.mesh_ranges = mesh_ranges_for_lod(lod, extra_lod);
    }

    Ok(MdlMetadata {
        path: path.to_string(),
        file_header,
        string_count,
        string_table_size,
        model_header,
        lods,
        extra_lods,
        meshes,
        terrain_shadow_meshes,
        submeshes,
        terrain_shadow_submeshes,
        attributes,
        materials,
        bones,
        bone_tables,
        shapes,
        shape_meshes,
        shape_values,
        submesh_bone_map_byte_size,
        submesh_bone_map,
    })
}

#[derive(Clone, Debug, PartialEq)]
struct RawMeshMetadata {
    index: usize,
    vertex_count: u16,
    padding: u16,
    index_count: u32,
    material_index: u16,
    submesh_index: u16,
    submesh_count: u16,
    bone_table_index: u16,
    start_index: u32,
    vertex_buffer_offsets: [u32; 3],
    vertex_buffer_strides: [u8; 3],
    vertex_stream_count: u8,
}

impl RawMeshMetadata {
    fn with_tables(
        self,
        materials: &[MdlNamedOffset],
        submeshes: &[MdlSubmeshMetadata],
        bone_tables: &[MdlBoneTableMetadata],
    ) -> MdlMeshMetadata {
        let submesh_start = usize::from(self.submesh_index);
        let submesh_end = submesh_start.saturating_add(usize::from(self.submesh_count));
        let mesh_submeshes = submeshes
            .get(submesh_start..submesh_end)
            .unwrap_or_default()
            .iter()
            .map(|submesh| MdlMeshSubmeshMetadata {
                table_index: submesh.index,
                index_offset: submesh.index_offset,
                relative_index_offset: Some(
                    i64::from(submesh.index_offset) - i64::from(self.start_index),
                ),
                index_count: submesh.index_count,
                attribute_index_mask: submesh.attribute_index_mask,
                attribute_index_mask_hex: submesh.attribute_index_mask_hex.clone(),
                attribute_names: submesh.attribute_names.clone(),
                bone_start_index: submesh.bone_start_index,
                bone_count: submesh.bone_count,
            })
            .collect::<Vec<_>>();
        let bone_table = (self.bone_table_index != 255)
            .then(|| bone_tables.get(usize::from(self.bone_table_index)).cloned())
            .flatten();

        MdlMeshMetadata {
            index: self.index,
            vertex_count: self.vertex_count,
            padding: self.padding,
            index_count: self.index_count,
            material_index: self.material_index,
            material_name: materials
                .get(usize::from(self.material_index))
                .and_then(|material| material.name.clone()),
            submesh_index: self.submesh_index,
            submesh_count: self.submesh_count,
            submesh_indices: mesh_submeshes
                .iter()
                .map(|submesh| submesh.table_index)
                .collect(),
            submeshes: mesh_submeshes,
            bone_table_index: self.bone_table_index,
            bone_table,
            start_index: self.start_index,
            vertex_buffer_offsets: self.vertex_buffer_offsets,
            vertex_buffer_strides: self.vertex_buffer_strides,
            vertex_stream_count: self.vertex_stream_count,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct RawSubmeshMetadata {
    index: usize,
    index_offset: u32,
    index_count: u32,
    attribute_index_mask: u32,
    bone_start_index: u16,
    bone_count: u16,
}

impl RawSubmeshMetadata {
    fn with_attribute_names(self, attributes: &[MdlNamedOffset]) -> MdlSubmeshMetadata {
        MdlSubmeshMetadata {
            index: self.index,
            index_offset: self.index_offset,
            index_count: self.index_count,
            attribute_index_mask: self.attribute_index_mask,
            attribute_index_mask_hex: hex_u32(self.attribute_index_mask),
            attribute_names: attribute_names(self.attribute_index_mask, attributes),
            bone_start_index: self.bone_start_index,
            bone_count: self.bone_count,
        }
    }
}

fn parse_file_header(bytes: &[u8], offset: usize) -> anyhow::Result<MdlFileHeaderMetadata> {
    Ok(MdlFileHeaderMetadata {
        version: read_u32_le(bytes, offset, "file header version")?,
        version_hex: hex_u32(read_u32_le(bytes, offset, "file header version")?),
        stack_size: read_u32_le(bytes, offset + 4, "stack size")?,
        runtime_size: read_u32_le(bytes, offset + 8, "runtime size")?,
        vertex_declaration_count: read_u16_le(bytes, offset + 12, "vertex declaration count")?,
        material_count: read_u16_le(bytes, offset + 14, "file material count")?,
        vertex_offsets: read_u32x3(bytes, offset + 16, "vertex offsets")?,
        index_offsets: read_u32x3(bytes, offset + 28, "index offsets")?,
        vertex_buffer_sizes: read_u32x3(bytes, offset + 40, "vertex buffer sizes")?,
        index_buffer_sizes: read_u32x3(bytes, offset + 52, "index buffer sizes")?,
        lod_count: read_u8(bytes, offset + 64, "file lod count")?,
        enable_index_buffer_streaming: read_u8(bytes, offset + 65, "index buffer streaming")? != 0,
        enable_edge_geometry: read_u8(bytes, offset + 66, "edge geometry")? != 0,
    })
}

fn parse_model_header(bytes: &[u8], offset: usize) -> anyhow::Result<MdlModelHeaderMetadata> {
    let flags1 = read_u8(bytes, offset + 0x17, "flags1")?;
    let flags2 = read_u8(bytes, offset + 0x1b, "flags2")?;
    let flags3 = read_u8(bytes, offset + 0x28, "flags3")?;
    Ok(MdlModelHeaderMetadata {
        radius: read_f32_le(bytes, offset, "model radius")?,
        mesh_count: read_u16_le(bytes, offset + 0x04, "mesh count")?,
        attribute_count: read_u16_le(bytes, offset + 0x06, "attribute count")?,
        submesh_count: read_u16_le(bytes, offset + 0x08, "submesh count")?,
        material_count: read_u16_le(bytes, offset + 0x0a, "material count")?,
        bone_count: read_u16_le(bytes, offset + 0x0c, "bone count")?,
        bone_table_count: read_u16_le(bytes, offset + 0x0e, "bone table count")?,
        shape_count: read_u16_le(bytes, offset + 0x10, "shape count")?,
        shape_mesh_count: read_u16_le(bytes, offset + 0x12, "shape mesh count")?,
        shape_value_count: read_u16_le(bytes, offset + 0x14, "shape value count")?,
        lod_count: read_u8(bytes, offset + 0x16, "model lod count")?,
        flags1,
        flags1_hex: hex_u8(flags1),
        element_id_count: read_u16_le(bytes, offset + 0x18, "element id count")?,
        terrain_shadow_mesh_count: read_u8(bytes, offset + 0x1a, "terrain shadow mesh count")?,
        flags2,
        flags2_hex: hex_u8(flags2),
        has_extra_lods: (flags2 & 0x10) != 0,
        model_clip_out_distance: read_f32_le(bytes, offset + 0x1c, "model clip out distance")?,
        shadow_clip_out_distance: read_f32_le(bytes, offset + 0x20, "shadow clip out distance")?,
        unknown4: read_u16_le(bytes, offset + 0x24, "unknown4")?,
        terrain_shadow_submesh_count: read_u16_le(
            bytes,
            offset + 0x26,
            "terrain shadow submesh count",
        )?,
        flags3,
        flags3_hex: hex_u8(flags3),
        bg_change_material_index: read_u8(bytes, offset + 0x29, "bg change material index")?,
        bg_crest_change_material_index: read_u8(
            bytes,
            offset + 0x2a,
            "bg crest change material index",
        )?,
        unknown6: read_u8(bytes, offset + 0x2b, "unknown6")?,
        bone_table_array_count_total: read_u16_le(
            bytes,
            offset + 0x2c,
            "bone table array count total",
        )?,
        unknown8: read_u16_le(bytes, offset + 0x2e, "unknown8")?,
        unknown9: read_u16_le(bytes, offset + 0x30, "unknown9")?,
    })
}

fn parse_lod(bytes: &[u8], offset: usize, index: usize) -> anyhow::Result<MdlLodMetadata> {
    Ok(MdlLodMetadata {
        index,
        mesh_index: read_u16_le(bytes, offset, "lod mesh index")?,
        mesh_count: read_u16_le(bytes, offset + 2, "lod mesh count")?,
        model_lod_range: read_f32_le(bytes, offset + 4, "model lod range")?,
        texture_lod_range: read_f32_le(bytes, offset + 8, "texture lod range")?,
        water_mesh_index: read_u16_le(bytes, offset + 12, "water mesh index")?,
        water_mesh_count: read_u16_le(bytes, offset + 14, "water mesh count")?,
        shadow_mesh_index: read_u16_le(bytes, offset + 16, "shadow mesh index")?,
        shadow_mesh_count: read_u16_le(bytes, offset + 18, "shadow mesh count")?,
        terrain_shadow_mesh_index: read_u16_le(bytes, offset + 20, "terrain shadow mesh index")?,
        terrain_shadow_mesh_count: read_u16_le(bytes, offset + 22, "terrain shadow mesh count")?,
        vertical_fog_mesh_index: read_u16_le(bytes, offset + 24, "vertical fog mesh index")?,
        vertical_fog_mesh_count: read_u16_le(bytes, offset + 26, "vertical fog mesh count")?,
        edge_geometry_size: read_u32_le(bytes, offset + 28, "edge geometry size")?,
        edge_geometry_data_offset: read_u32_le(bytes, offset + 32, "edge geometry data offset")?,
        polygon_count: read_u32_le(bytes, offset + 36, "polygon count")?,
        unknown1: read_u32_le(bytes, offset + 40, "lod unknown1")?,
        vertex_buffer_size: read_u32_le(bytes, offset + 44, "lod vertex buffer size")?,
        index_buffer_size: read_u32_le(bytes, offset + 48, "lod index buffer size")?,
        vertex_data_offset: read_u32_le(bytes, offset + 52, "vertex data offset")?,
        index_data_offset: read_u32_le(bytes, offset + 56, "index data offset")?,
        mesh_ranges: Vec::new(),
    })
}

fn parse_extra_lod(
    bytes: &[u8],
    offset: usize,
    index: usize,
) -> anyhow::Result<MdlExtraLodMetadata> {
    let mut unknowns = [0_u16; 12];
    for (unknown_index, value) in unknowns.iter_mut().enumerate() {
        *value = read_u16_le(
            bytes,
            offset + 16 + unknown_index * 2,
            "extra lod unknown value",
        )?;
    }

    Ok(MdlExtraLodMetadata {
        index,
        light_shaft_mesh_index: read_u16_le(bytes, offset, "light shaft mesh index")?,
        light_shaft_mesh_count: read_u16_le(bytes, offset + 2, "light shaft mesh count")?,
        glass_mesh_index: read_u16_le(bytes, offset + 4, "glass mesh index")?,
        glass_mesh_count: read_u16_le(bytes, offset + 6, "glass mesh count")?,
        material_change_mesh_index: read_u16_le(bytes, offset + 8, "material change mesh index")?,
        material_change_mesh_count: read_u16_le(bytes, offset + 10, "material change mesh count")?,
        crest_change_mesh_index: read_u16_le(bytes, offset + 12, "crest change mesh index")?,
        crest_change_mesh_count: read_u16_le(bytes, offset + 14, "crest change mesh count")?,
        unknowns,
    })
}

fn parse_raw_mesh(bytes: &[u8], offset: usize, index: usize) -> anyhow::Result<RawMeshMetadata> {
    Ok(RawMeshMetadata {
        index,
        vertex_count: read_u16_le(bytes, offset, "mesh vertex count")?,
        padding: read_u16_le(bytes, offset + 2, "mesh padding")?,
        index_count: read_u32_le(bytes, offset + 4, "mesh index count")?,
        material_index: read_u16_le(bytes, offset + 8, "mesh material index")?,
        submesh_index: read_u16_le(bytes, offset + 10, "mesh submesh index")?,
        submesh_count: read_u16_le(bytes, offset + 12, "mesh submesh count")?,
        bone_table_index: read_u16_le(bytes, offset + 14, "mesh bone table index")?,
        start_index: read_u32_le(bytes, offset + 16, "mesh start index")?,
        vertex_buffer_offsets: read_u32x3(bytes, offset + 20, "mesh vertex buffer offsets")?,
        vertex_buffer_strides: read_u8x3(bytes, offset + 32, "mesh vertex buffer strides")?,
        vertex_stream_count: read_u8(bytes, offset + 35, "mesh vertex stream count")?,
    })
}

fn parse_terrain_shadow_mesh(
    bytes: &[u8],
    offset: usize,
    index: usize,
) -> anyhow::Result<MdlTerrainShadowMeshMetadata> {
    Ok(MdlTerrainShadowMeshMetadata {
        index,
        index_count: read_u32_le(bytes, offset, "terrain shadow mesh index count")?,
        start_index: read_u32_le(bytes, offset + 4, "terrain shadow mesh start index")?,
        vertex_buffer_offset: read_u32_le(
            bytes,
            offset + 8,
            "terrain shadow mesh vertex buffer offset",
        )?,
        vertex_count: read_u16_le(bytes, offset + 12, "terrain shadow mesh vertex count")?,
        submesh_index: read_u16_le(bytes, offset + 14, "terrain shadow mesh submesh index")?,
        submesh_count: read_u16_le(bytes, offset + 16, "terrain shadow mesh submesh count")?,
        vertex_buffer_stride: read_u8(bytes, offset + 18, "terrain shadow mesh vertex stride")?,
    })
}

fn parse_raw_submesh(
    bytes: &[u8],
    offset: usize,
    index: usize,
) -> anyhow::Result<RawSubmeshMetadata> {
    Ok(RawSubmeshMetadata {
        index,
        index_offset: read_u32_le(bytes, offset, "submesh index offset")?,
        index_count: read_u32_le(bytes, offset + 4, "submesh index count")?,
        attribute_index_mask: read_u32_le(bytes, offset + 8, "submesh attribute index mask")?,
        bone_start_index: read_u16_le(bytes, offset + 12, "submesh bone start index")?,
        bone_count: read_u16_le(bytes, offset + 14, "submesh bone count")?,
    })
}

fn parse_terrain_shadow_submesh(
    bytes: &[u8],
    offset: usize,
    index: usize,
) -> anyhow::Result<MdlTerrainShadowSubmeshMetadata> {
    Ok(MdlTerrainShadowSubmeshMetadata {
        index,
        index_offset: read_u32_le(bytes, offset, "terrain shadow submesh index offset")?,
        index_count: read_u32_le(bytes, offset + 4, "terrain shadow submesh index count")?,
        unknown1: read_u16_le(bytes, offset + 8, "terrain shadow submesh unknown1")?,
        unknown2: read_u16_le(bytes, offset + 10, "terrain shadow submesh unknown2")?,
    })
}

fn mesh_ranges_for_lod(
    lod: &MdlLodMetadata,
    extra_lod: Option<&MdlExtraLodMetadata>,
) -> Vec<MdlMeshRangeMetadata> {
    let mut ranges = vec![
        mesh_range("normal", lod.mesh_index, lod.mesh_count),
        mesh_range("water", lod.water_mesh_index, lod.water_mesh_count),
        mesh_range("shadow", lod.shadow_mesh_index, lod.shadow_mesh_count),
        mesh_range(
            "terrainShadow",
            lod.terrain_shadow_mesh_index,
            lod.terrain_shadow_mesh_count,
        ),
        mesh_range(
            "verticalFog",
            lod.vertical_fog_mesh_index,
            lod.vertical_fog_mesh_count,
        ),
    ];

    if let Some(extra_lod) = extra_lod {
        ranges.extend([
            mesh_range(
                "lightShaft",
                extra_lod.light_shaft_mesh_index,
                extra_lod.light_shaft_mesh_count,
            ),
            mesh_range(
                "glass",
                extra_lod.glass_mesh_index,
                extra_lod.glass_mesh_count,
            ),
            mesh_range(
                "materialChange",
                extra_lod.material_change_mesh_index,
                extra_lod.material_change_mesh_count,
            ),
            mesh_range(
                "crestChange",
                extra_lod.crest_change_mesh_index,
                extra_lod.crest_change_mesh_count,
            ),
        ]);
    }

    ranges
        .into_iter()
        .filter(|range| range.mesh_count != 0)
        .collect()
}

fn mesh_range(category: &str, mesh_index: u16, mesh_count: u16) -> MdlMeshRangeMetadata {
    MdlMeshRangeMetadata {
        category: category.to_string(),
        mesh_index,
        mesh_count,
        mesh_end: mesh_index.saturating_add(mesh_count),
    }
}

fn named_offsets(offsets: &[u32], string_table: &[u8]) -> Vec<MdlNamedOffset> {
    offsets
        .iter()
        .enumerate()
        .map(|(index, offset)| MdlNamedOffset {
            index,
            offset: *offset,
            name: read_string_at(string_table, *offset),
        })
        .collect()
}

fn attribute_names(mask: u32, attributes: &[MdlNamedOffset]) -> Vec<String> {
    attributes
        .iter()
        .filter(|attribute| {
            attribute.index < u32::BITS as usize && (mask & (1_u32 << attribute.index)) != 0
        })
        .filter_map(|attribute| attribute.name.clone())
        .collect()
}

fn read_bone_tables(
    bytes: &[u8],
    offset: &mut usize,
    version: u32,
    count: usize,
    array_count_total: usize,
    bones: &[MdlNamedOffset],
) -> anyhow::Result<Vec<MdlBoneTableMetadata>> {
    match version {
        MDL_VERSION_V5 => read_v5_bone_tables(bytes, offset, count, bones),
        MDL_VERSION_V6 => read_v6_bone_tables(bytes, offset, count, array_count_total, bones),
        _ if count == 0 => Ok(Vec::new()),
        _ => anyhow::bail!("unsupported mdl version {version:#010x} for bone tables"),
    }
}

fn read_v5_bone_tables(
    bytes: &[u8],
    offset: &mut usize,
    count: usize,
    bones: &[MdlNamedOffset],
) -> anyhow::Result<Vec<MdlBoneTableMetadata>> {
    let mut tables = Vec::with_capacity(count);
    for index in 0..count {
        let table_offset = *offset;
        let all_indices = read_u16_table(
            bytes,
            table_offset,
            V5_BONE_TABLE_BONE_CAPACITY,
            "v5 bone table indices",
        )?;
        let bone_count = read_u32_le(
            bytes,
            table_offset + V5_BONE_TABLE_BONE_CAPACITY * 2,
            "v5 bone table count",
        )?;
        let bone_indices = all_indices
            .into_iter()
            .take(bone_count as usize)
            .collect::<Vec<_>>();
        tables.push(bone_table_metadata(index, bone_indices, bones));
        *offset = checked_advance(*offset, V5_BONE_TABLE_SIZE, bytes.len(), "v5 bone table")?;
    }
    Ok(tables)
}

fn read_v6_bone_tables(
    bytes: &[u8],
    offset: &mut usize,
    count: usize,
    array_count_total: usize,
    bones: &[MdlNamedOffset],
) -> anyhow::Result<Vec<MdlBoneTableMetadata>> {
    let descriptor_base = *offset;
    let mut tables = Vec::with_capacity(count);
    for index in 0..count {
        let descriptor_offset = descriptor_base
            .checked_add(index.checked_mul(4).ok_or_else(|| {
                anyhow::anyhow!("v6 bone table descriptor offset overflow at index {index}")
            })?)
            .ok_or_else(|| anyhow::anyhow!("v6 bone table descriptor offset overflow"))?;
        let relative_offset = usize::from(read_u16_le(
            bytes,
            descriptor_offset,
            "v6 bone table relative offset",
        )?);
        let bone_count = usize::from(read_u16_le(
            bytes,
            descriptor_offset + 2,
            "v6 bone table count",
        )?);
        let table_offset = descriptor_offset
            .checked_add(relative_offset.checked_mul(4).ok_or_else(|| {
                anyhow::anyhow!("v6 bone table relative offset overflow at index {index}")
            })?)
            .ok_or_else(|| anyhow::anyhow!("v6 bone table offset overflow"))?;
        let bone_indices =
            read_u16_table(bytes, table_offset, bone_count, "v6 bone table indices")?;
        tables.push(bone_table_metadata(index, bone_indices, bones));
    }
    *offset = checked_advance(
        descriptor_base,
        count
            .checked_mul(4)
            .and_then(|descriptor_size| {
                descriptor_size.checked_add(array_count_total.checked_mul(2)?)
            })
            .ok_or_else(|| anyhow::anyhow!("v6 bone table block size overflow"))?,
        bytes.len(),
        "v6 bone tables",
    )?;
    Ok(tables)
}

fn bone_table_metadata(
    index: usize,
    bone_indices: Vec<u16>,
    bones: &[MdlNamedOffset],
) -> MdlBoneTableMetadata {
    let bone_names = bone_indices
        .iter()
        .map(|bone_index| {
            bones
                .get(usize::from(*bone_index))
                .and_then(|bone| bone.name.clone())
        })
        .collect::<Vec<_>>();
    MdlBoneTableMetadata {
        index,
        bone_count: bone_indices.len() as u32,
        bone_indices,
        bone_names,
    }
}

fn read_shapes(
    bytes: &[u8],
    offset: &mut usize,
    count: usize,
    string_table: &[u8],
) -> anyhow::Result<Vec<MdlShapeMetadata>> {
    let mut shapes = Vec::with_capacity(count);
    for index in 0..count {
        let shape_offset = *offset;
        let string_offset = read_u32_le(bytes, shape_offset, "shape string offset")?;
        shapes.push(MdlShapeMetadata {
            index,
            string_offset,
            name: read_string_at(string_table, string_offset),
            shape_mesh_start_indices: read_u16x3(
                bytes,
                shape_offset + 4,
                "shape mesh start indices",
            )?,
            shape_mesh_counts: read_u16x3(bytes, shape_offset + 10, "shape mesh counts")?,
        });
        *offset = checked_advance(*offset, SHAPE_SIZE, bytes.len(), "shape table")?;
    }
    Ok(shapes)
}

fn read_shape_meshes(
    bytes: &[u8],
    offset: &mut usize,
    count: usize,
) -> anyhow::Result<Vec<MdlShapeMeshMetadata>> {
    let mut shape_meshes = Vec::with_capacity(count);
    for index in 0..count {
        let shape_mesh_offset = *offset;
        shape_meshes.push(MdlShapeMeshMetadata {
            index,
            mesh_index_offset: read_u32_le(bytes, shape_mesh_offset, "shape mesh index offset")?,
            shape_value_count: read_u32_le(bytes, shape_mesh_offset + 4, "shape value count")?,
            shape_value_offset: read_u32_le(bytes, shape_mesh_offset + 8, "shape value offset")?,
        });
        *offset = checked_advance(*offset, SHAPE_MESH_SIZE, bytes.len(), "shape mesh table")?;
    }
    Ok(shape_meshes)
}

fn read_shape_values(
    bytes: &[u8],
    offset: &mut usize,
    count: usize,
) -> anyhow::Result<Vec<MdlShapeValueMetadata>> {
    let mut shape_values = Vec::with_capacity(count);
    for index in 0..count {
        let shape_value_offset = *offset;
        shape_values.push(MdlShapeValueMetadata {
            index,
            base_indices_index: read_u16_le(
                bytes,
                shape_value_offset,
                "shape value base indices index",
            )?,
            replacing_vertex_index: read_u16_le(
                bytes,
                shape_value_offset + 2,
                "shape value replacing vertex index",
            )?,
        });
        *offset = checked_advance(*offset, SHAPE_VALUE_SIZE, bytes.len(), "shape value table")?;
    }
    Ok(shape_values)
}

fn read_u32_table(
    bytes: &[u8],
    offset: &mut usize,
    count: usize,
    label: &str,
) -> anyhow::Result<Vec<u32>> {
    let mut values = Vec::with_capacity(count);
    for _ in 0..count {
        values.push(read_u32_le(bytes, *offset, label)?);
        *offset = checked_advance(*offset, 4, bytes.len(), label)?;
    }
    Ok(values)
}

fn read_u16_table(
    bytes: &[u8],
    offset: usize,
    count: usize,
    label: &str,
) -> anyhow::Result<Vec<u16>> {
    let mut values = Vec::with_capacity(count);
    for index in 0..count {
        values.push(read_u16_le(bytes, offset + index * 2, label)?);
    }
    Ok(values)
}

fn read_string_at(string_table: &[u8], offset: u32) -> Option<String> {
    let start = usize::try_from(offset).ok()?;
    let tail = string_table.get(start..)?;
    let end = tail
        .iter()
        .position(|value| *value == 0)
        .unwrap_or(tail.len());
    Some(String::from_utf8_lossy(&tail[..end]).to_string())
}

fn checked_advance(
    offset: usize,
    byte_count: usize,
    len: usize,
    label: &str,
) -> anyhow::Result<usize> {
    let next = offset
        .checked_add(byte_count)
        .ok_or_else(|| anyhow::anyhow!("{label} offset overflow at {offset} + {byte_count}"))?;
    if next <= len {
        Ok(next)
    } else {
        anyhow::bail!("{label} extends past end of file: {offset} + {byte_count} > {len}");
    }
}

fn read_bytes<'a>(
    bytes: &'a [u8],
    offset: usize,
    byte_count: usize,
    label: &str,
) -> anyhow::Result<&'a [u8]> {
    let end = checked_advance(offset, byte_count, bytes.len(), label)?;
    bytes
        .get(offset..end)
        .ok_or_else(|| anyhow::anyhow!("{label} is out of bounds at {offset}..{end}"))
}

fn read_u8(bytes: &[u8], offset: usize, label: &str) -> anyhow::Result<u8> {
    bytes
        .get(offset)
        .copied()
        .ok_or_else(|| anyhow::anyhow!("{label} is out of bounds at {offset}"))
}

fn read_u8x3(bytes: &[u8], offset: usize, label: &str) -> anyhow::Result<[u8; 3]> {
    let bytes = read_bytes(bytes, offset, 3, label)?;
    Ok([bytes[0], bytes[1], bytes[2]])
}

fn read_u16x3(bytes: &[u8], offset: usize, label: &str) -> anyhow::Result<[u16; 3]> {
    Ok([
        read_u16_le(bytes, offset, label)?,
        read_u16_le(bytes, offset + 2, label)?,
        read_u16_le(bytes, offset + 4, label)?,
    ])
}

fn read_u16_le(bytes: &[u8], offset: usize, label: &str) -> anyhow::Result<u16> {
    let bytes = read_bytes(bytes, offset, 2, label)?;
    Ok(u16::from_le_bytes(bytes.try_into()?))
}

fn read_u32_le(bytes: &[u8], offset: usize, label: &str) -> anyhow::Result<u32> {
    let bytes = read_bytes(bytes, offset, 4, label)?;
    Ok(u32::from_le_bytes(bytes.try_into()?))
}

fn read_u32x3(bytes: &[u8], offset: usize, label: &str) -> anyhow::Result<[u32; 3]> {
    Ok([
        read_u32_le(bytes, offset, label)?,
        read_u32_le(bytes, offset + 4, label)?,
        read_u32_le(bytes, offset + 8, label)?,
    ])
}

fn read_f32_le(bytes: &[u8], offset: usize, label: &str) -> anyhow::Result<f32> {
    let bytes = read_bytes(bytes, offset, 4, label)?;
    Ok(f32::from_le_bytes(bytes.try_into()?))
}

fn hex_u8(value: u8) -> String {
    format!("0x{value:02x}")
}

fn hex_u32(value: u32) -> String {
    format!("0x{value:08x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_meddle_aligned_mesh_and_extra_lod_tables() {
        let bytes = fixture_mdl_bytes();
        let metadata = mdl_metadata_from_mdl_bytes("test.mdl", &bytes).expect("metadata");

        assert_eq!(metadata.file_header.version_hex, "0x01000006");
        assert!(metadata.model_header.has_extra_lods);
        assert_eq!(metadata.attributes[1].name.as_deref(), Some("attr_b"));
        assert_eq!(metadata.materials[1].name.as_deref(), Some("/mt_b.mtrl"));
        assert_eq!(metadata.meshes.len(), 3);
        assert_eq!(
            metadata.meshes[2].material_name.as_deref(),
            Some("/mt_b.mtrl")
        );
        assert_eq!(metadata.meshes[2].submesh_indices, vec![1]);
        assert_eq!(
            metadata.meshes[2].submeshes[0].attribute_names,
            vec!["attr_b".to_string()]
        );
        assert_eq!(
            metadata.meshes[2].submeshes[0].relative_index_offset,
            Some(0)
        );
        assert_eq!(metadata.bones[1].name.as_deref(), Some("bone_b"));
        assert_eq!(metadata.bone_tables.len(), 1);
        assert_eq!(metadata.bone_tables[0].bone_indices, vec![1, 2]);
        assert_eq!(
            metadata.bone_tables[0].bone_names,
            vec![Some("bone_b".to_string()), Some("bone_c".to_string())]
        );
        assert_eq!(metadata.meshes[0].bone_table, None);
        assert_eq!(
            metadata.meshes[2]
                .bone_table
                .as_ref()
                .map(|table| table.bone_names.clone()),
            Some(vec![Some("bone_b".to_string()), Some("bone_c".to_string())])
        );
        assert!(metadata.lods[0].mesh_ranges.iter().any(|range| {
            range.category == "glass" && range.mesh_index == 2 && range.mesh_count == 1
        }));
        assert_eq!(metadata.shapes.len(), 1);
        assert_eq!(metadata.shapes[0].name.as_deref(), Some("shape_a"));
        assert_eq!(metadata.shapes[0].shape_mesh_start_indices, [0, 0, 0]);
        assert_eq!(metadata.shapes[0].shape_mesh_counts, [1, 0, 0]);
        assert_eq!(metadata.shape_meshes.len(), 1);
        assert_eq!(metadata.shape_meshes[0].mesh_index_offset, 6);
        assert_eq!(metadata.shape_meshes[0].shape_value_count, 2);
        assert_eq!(metadata.shape_meshes[0].shape_value_offset, 0);
        assert_eq!(metadata.shape_values.len(), 2);
        assert_eq!(metadata.shape_values[0].base_indices_index, 1);
        assert_eq!(metadata.shape_values[0].replacing_vertex_index, 4);
        assert_eq!(metadata.shape_values[1].base_indices_index, 2);
        assert_eq!(metadata.shape_values[1].replacing_vertex_index, 5);
        assert_eq!(metadata.submesh_bone_map_byte_size, 4);
        assert_eq!(metadata.submesh_bone_map, vec![1, 2]);
    }

    fn fixture_mdl_bytes() -> Vec<u8> {
        let mut bytes = vec![0; MODEL_FILE_HEADER_SIZE];
        write_u32(&mut bytes, 0, 0x0100_0006);
        write_u16(&mut bytes, 12, 1);
        write_u16(&mut bytes, 14, 2);
        write_u8(&mut bytes, 64, 1);

        bytes.extend_from_slice(&[0; VERTEX_DECLARATION_SIZE]);

        let string_table =
            b"attr_a\0attr_b\0/mt_a.mtrl\0/mt_b.mtrl\0bone_a\0bone_b\0bone_c\0shape_a\0";
        let string_header = bytes.len();
        bytes.extend_from_slice(&[0; 8]);
        write_u16(&mut bytes, string_header, 8);
        write_u32(&mut bytes, string_header + 4, string_table.len() as u32);
        bytes.extend_from_slice(string_table);

        let model_header = bytes.len();
        bytes.extend_from_slice(&[0; MODEL_HEADER_SIZE]);
        write_f32(&mut bytes, model_header, 1.0);
        write_u16(&mut bytes, model_header + 0x04, 3);
        write_u16(&mut bytes, model_header + 0x06, 2);
        write_u16(&mut bytes, model_header + 0x08, 2);
        write_u16(&mut bytes, model_header + 0x0a, 2);
        write_u16(&mut bytes, model_header + 0x0c, 3);
        write_u16(&mut bytes, model_header + 0x0e, 1);
        write_u16(&mut bytes, model_header + 0x10, 1);
        write_u16(&mut bytes, model_header + 0x12, 1);
        write_u16(&mut bytes, model_header + 0x14, 2);
        write_u8(&mut bytes, model_header + 0x16, 1);
        write_u8(&mut bytes, model_header + 0x1a, 1);
        write_u8(&mut bytes, model_header + 0x1b, 0x10);
        write_u16(&mut bytes, model_header + 0x26, 1);
        write_u16(&mut bytes, model_header + 0x2c, 2);

        let lod0 = bytes.len();
        bytes.extend_from_slice(&[0; LOD_SIZE * 3]);
        write_u16(&mut bytes, lod0, 0);
        write_u16(&mut bytes, lod0 + 2, 1);
        write_u16(&mut bytes, lod0 + 12, 1);
        write_u16(&mut bytes, lod0 + 14, 1);

        let extra_lod0 = bytes.len();
        bytes.extend_from_slice(&[0; EXTRA_LOD_SIZE * 3]);
        write_u16(&mut bytes, extra_lod0 + 4, 2);
        write_u16(&mut bytes, extra_lod0 + 6, 1);

        for index in 0..3 {
            let mesh = bytes.len();
            bytes.extend_from_slice(&[0; MESH_SIZE]);
            write_u16(&mut bytes, mesh, 4);
            write_u32(&mut bytes, mesh + 4, if index == 2 { 3 } else { 6 });
            write_u16(&mut bytes, mesh + 8, if index == 2 { 1 } else { 0 });
            write_u16(&mut bytes, mesh + 10, if index == 2 { 1 } else { 0 });
            write_u16(&mut bytes, mesh + 12, 1);
            write_u16(&mut bytes, mesh + 14, if index == 2 { 0 } else { 255 });
            write_u32(&mut bytes, mesh + 16, if index == 2 { 6 } else { 0 });
            write_u8(&mut bytes, mesh + 32, 12);
            write_u8(&mut bytes, mesh + 35, 1);
        }

        let attr_a_write_offset = bytes.len();
        write_u32(&mut bytes, attr_a_write_offset, 0);
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        let attr_b_offset = b"attr_a\0".len() as u32;
        let attr_b_write_offset = bytes.len() - 4;
        write_u32(&mut bytes, attr_b_write_offset, attr_b_offset);

        bytes.extend_from_slice(&[0; TERRAIN_SHADOW_MESH_SIZE]);

        let submesh0 = bytes.len();
        bytes.extend_from_slice(&[0; SUBMESH_SIZE]);
        write_u32(&mut bytes, submesh0, 0);
        write_u32(&mut bytes, submesh0 + 4, 6);
        write_u32(&mut bytes, submesh0 + 8, 1);
        let submesh1 = bytes.len();
        bytes.extend_from_slice(&[0; SUBMESH_SIZE]);
        write_u32(&mut bytes, submesh1, 6);
        write_u32(&mut bytes, submesh1 + 4, 3);
        write_u32(&mut bytes, submesh1 + 8, 2);

        bytes.extend_from_slice(&[0; TERRAIN_SHADOW_SUBMESH_SIZE]);

        let material_a_offset = b"attr_a\0attr_b\0".len() as u32;
        let material_b_offset = b"attr_a\0attr_b\0/mt_a.mtrl\0".len() as u32;
        bytes.extend_from_slice(&material_a_offset.to_le_bytes());
        bytes.extend_from_slice(&material_b_offset.to_le_bytes());

        let bone_a_offset = b"attr_a\0attr_b\0/mt_a.mtrl\0/mt_b.mtrl\0".len() as u32;
        let bone_b_offset = b"attr_a\0attr_b\0/mt_a.mtrl\0/mt_b.mtrl\0bone_a\0".len() as u32;
        let bone_c_offset =
            b"attr_a\0attr_b\0/mt_a.mtrl\0/mt_b.mtrl\0bone_a\0bone_b\0".len() as u32;
        bytes.extend_from_slice(&bone_a_offset.to_le_bytes());
        bytes.extend_from_slice(&bone_b_offset.to_le_bytes());
        bytes.extend_from_slice(&bone_c_offset.to_le_bytes());

        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&2_u16.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&2_u16.to_le_bytes());

        let shape_a_offset =
            b"attr_a\0attr_b\0/mt_a.mtrl\0/mt_b.mtrl\0bone_a\0bone_b\0bone_c\0".len() as u32;
        let shape = bytes.len();
        bytes.extend_from_slice(&[0; SHAPE_SIZE]);
        write_u32(&mut bytes, shape, shape_a_offset);
        write_u16(&mut bytes, shape + 10, 1);

        let shape_mesh = bytes.len();
        bytes.extend_from_slice(&[0; SHAPE_MESH_SIZE]);
        write_u32(&mut bytes, shape_mesh, 6);
        write_u32(&mut bytes, shape_mesh + 4, 2);

        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&4_u16.to_le_bytes());
        bytes.extend_from_slice(&2_u16.to_le_bytes());
        bytes.extend_from_slice(&5_u16.to_le_bytes());

        bytes.extend_from_slice(&4_u32.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&2_u16.to_le_bytes());

        bytes
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
