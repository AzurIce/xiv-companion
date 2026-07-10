use std::collections::HashMap;

use anyhow::{Context, Result, bail, ensure};
use half::f16;
use serde::{Deserialize, Serialize};

use crate::model::{
    ColorTableRowColors, ModelColorDyeTable, ModelDawntrailColorDyeTableRow,
    ModelLegacyColorDyeTableRow,
};

pub const LEGACY_STAINING_TEMPLATE_PATH: &str = "chara/base_material/stainingtemplate.stm";
pub const DAWNTRAIL_STAINING_TEMPLATE_PATH: &str = "chara/base_material/stainingtemplate_gud.stm";
pub const MAX_STAIN_ID: u8 = 254;

const STM_MAGIC: u16 = 0x534d;
const STM_VERSION_LEGACY: u16 = 0x0101;
const STM_VERSION_2_0: u16 = 0x0200;
const STM_VERSION_2_1: u16 = 0x0201;
const STAIN_COUNT: usize = MAX_STAIN_ID as usize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum StainingTemplateKind {
    Legacy,
    Dawntrail,
}

#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyStainingTemplateDye {
    pub diffuse: [f32; 3],
    pub specular: [f32; 3],
    pub emissive: [f32; 3],
    pub shininess: f32,
    pub specular_mask: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DawntrailStainingTemplateDye {
    pub diffuse: [f32; 3],
    pub specular: [f32; 3],
    pub emissive: [f32; 3],
    pub scalar3: f32,
    pub metalness: f32,
    pub roughness: f32,
    pub sheen_rate: f32,
    pub sheen_tint_rate: f32,
    pub sheen_aperture: f32,
    pub anisotropy: f32,
    pub sphere_map_index: u16,
    pub sphere_map_mask: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
#[serde(tag = "kind", content = "dye", rename_all = "camelCase")]
pub enum StainingTemplateDye {
    Legacy(LegacyStainingTemplateDye),
    Dawntrail(DawntrailStainingTemplateDye),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StainingApplicationReport {
    pub rows_considered: usize,
    pub rows_changed: usize,
    pub rows_skipped_no_stain: usize,
    pub rows_skipped_missing_template: usize,
    pub rows_unavailable: usize,
    pub template_kind_mismatch: bool,
}

#[derive(Clone, Debug)]
pub struct StainingTemplate {
    kind: StainingTemplateKind,
    version: u16,
    entries: HashMap<u32, StainingTemplateEntry>,
}

#[derive(Clone, Debug)]
struct StainingTemplateEntry {
    colors: Vec<Vec<[f32; 3]>>,
    scalars: Vec<Vec<f32>>,
}

impl StainingTemplate {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        ensure!(bytes.len() >= 8, "STM header is truncated");

        let magic = read_u16(bytes, 0, "STM magic")?;
        ensure!(magic == STM_MAGIC, "invalid STM magic 0x{magic:04X}");

        let version = read_u16(bytes, 2, "STM version")?;
        let entry_count = usize::from(read_u16(bytes, 4, "STM entry count")?);
        let raw_color_count = usize::from(bytes[6]);
        let raw_scalar_count = usize::from(bytes[7]);
        let (kind, color_count, scalar_count) = match version {
            STM_VERSION_LEGACY => {
                ensure!(
                    raw_color_count == 0 && raw_scalar_count == 0,
                    "legacy STM has unexpected column counts {raw_color_count}/{raw_scalar_count}"
                );
                (StainingTemplateKind::Legacy, 3, 2)
            }
            STM_VERSION_2_0 | STM_VERSION_2_1 => match (raw_color_count, raw_scalar_count) {
                (3, 2) => (StainingTemplateKind::Legacy, 3, 2),
                (3, 9) => (StainingTemplateKind::Dawntrail, 3, 9),
                _ => bail!(
                    "unsupported STM column counts {raw_color_count}/{raw_scalar_count} for version 0x{version:04X}"
                ),
            },
            _ => bail!("unsupported STM version 0x{version:04X}"),
        };

        if entry_count == 0 {
            return Ok(Self {
                kind,
                version,
                entries: HashMap::new(),
            });
        }

        let key_width = if version == STM_VERSION_LEGACY
            && bytes.len() > 0x0b
            && (bytes[0x0a] != 0 || bytes[0x0b] != 0)
        {
            2
        } else {
            4
        };
        let key_table_bytes = entry_count
            .checked_mul(key_width)
            .context("STM key table size overflow")?;
        let table_bytes = key_table_bytes
            .checked_mul(2)
            .context("STM key/offset table size overflow")?;
        let data_base = 8usize
            .checked_add(table_bytes)
            .context("STM data base overflow")?;
        ensure!(
            data_base <= bytes.len(),
            "STM key/offset tables are truncated"
        );

        let mut keys = Vec::with_capacity(entry_count);
        let mut offsets = Vec::with_capacity(entry_count);
        let offset_table = 8 + key_table_bytes;
        for index in 0..entry_count {
            keys.push(read_key(bytes, 8 + index * key_width, key_width)?);
            offsets.push(read_key(
                bytes,
                offset_table + index * key_width,
                key_width,
            )?);
        }

        let mut entries = HashMap::with_capacity(entry_count);
        for index in 0..entry_count {
            let relative_start = usize::try_from(offsets[index])
                .context("STM entry offset does not fit usize")?
                .checked_mul(2)
                .context("STM entry offset overflow")?;
            let entry_start = data_base
                .checked_add(relative_start)
                .context("STM entry start overflow")?;
            let entry_end = if let Some(next_offset) = offsets.get(index + 1) {
                let relative_end = usize::try_from(*next_offset)
                    .context("STM next entry offset does not fit usize")?
                    .checked_mul(2)
                    .context("STM next entry offset overflow")?;
                data_base
                    .checked_add(relative_end)
                    .context("STM entry end overflow")?
            } else {
                bytes.len()
            };
            ensure!(
                entry_start <= entry_end && entry_end <= bytes.len(),
                "STM entry {} has invalid range {entry_start}..{entry_end}",
                keys[index]
            );

            let entry = parse_entry(&bytes[entry_start..entry_end], color_count, scalar_count)
                .with_context(|| format!("failed to parse STM entry {}", keys[index]))?;
            ensure!(
                entries.insert(keys[index], entry).is_none(),
                "STM contains duplicate template key {}",
                keys[index]
            );
        }

        Ok(Self {
            kind,
            version,
            entries,
        })
    }

    pub fn kind(&self) -> StainingTemplateKind {
        self.kind
    }

    pub fn version(&self) -> u16 {
        self.version
    }

    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    pub fn contains_template(&self, template: u16) -> bool {
        self.entries
            .contains_key(&self.resolve_template_key(template))
    }

    pub fn dye(&self, template: u16, stain_id: u8) -> Option<StainingTemplateDye> {
        if stain_id == 0 || stain_id > MAX_STAIN_ID {
            return None;
        }
        let stain_index = usize::from(stain_id - 1);
        let entry = self.entries.get(&self.resolve_template_key(template))?;

        match self.kind {
            StainingTemplateKind::Legacy => {
                Some(StainingTemplateDye::Legacy(LegacyStainingTemplateDye {
                    diffuse: entry.color(0, stain_index)?,
                    specular: entry.color(1, stain_index)?,
                    emissive: entry.color(2, stain_index)?,
                    shininess: entry.scalar(0, stain_index)?,
                    specular_mask: entry.scalar(1, stain_index)?,
                }))
            }
            StainingTemplateKind::Dawntrail => Some(StainingTemplateDye::Dawntrail(
                DawntrailStainingTemplateDye {
                    diffuse: entry.color(0, stain_index)?,
                    specular: entry.color(1, stain_index)?,
                    emissive: entry.color(2, stain_index)?,
                    scalar3: entry.scalar(0, stain_index)?,
                    metalness: entry.scalar(1, stain_index)?,
                    roughness: entry.scalar(2, stain_index)?,
                    sheen_rate: entry.scalar(3, stain_index)?,
                    sheen_tint_rate: entry.scalar(4, stain_index)?,
                    sheen_aperture: entry.scalar(5, stain_index)?,
                    anisotropy: entry.scalar(6, stain_index)?,
                    sphere_map_index: entry.scalar(7, stain_index)? as u16,
                    sphere_map_mask: entry.scalar(8, stain_index)?,
                },
            )),
        }
    }

    fn resolve_template_key(&self, template: u16) -> u32 {
        if self.kind == StainingTemplateKind::Legacy && template >= 1000 {
            u32::from(template - 1000)
        } else {
            u32::from(template)
        }
    }
}

impl StainingTemplateEntry {
    fn color(&self, column: usize, stain_index: usize) -> Option<[f32; 3]> {
        self.colors.get(column)?.get(stain_index).copied()
    }

    fn scalar(&self, column: usize, stain_index: usize) -> Option<f32> {
        self.scalars.get(column)?.get(stain_index).copied()
    }
}

pub fn apply_staining_template_to_rows(
    rows: &mut [ColorTableRowColors],
    dye_table: &ModelColorDyeTable,
    stain_ids: &[u8],
    template: &StainingTemplate,
) -> StainingApplicationReport {
    match dye_table {
        ModelColorDyeTable::Legacy(dye_rows) => {
            if template.kind() != StainingTemplateKind::Legacy {
                return StainingApplicationReport {
                    template_kind_mismatch: true,
                    ..StainingApplicationReport::default()
                };
            }
            apply_legacy_rows(rows, dye_rows, stain_ids, template)
        }
        ModelColorDyeTable::Dawntrail(dye_rows) => {
            apply_dawntrail_rows(rows, dye_rows, stain_ids, template)
        }
        ModelColorDyeTable::Opaque => StainingApplicationReport::default(),
    }
}

fn apply_legacy_rows(
    rows: &mut [ColorTableRowColors],
    dye_rows: &[ModelLegacyColorDyeTableRow],
    stain_ids: &[u8],
    template: &StainingTemplate,
) -> StainingApplicationReport {
    let mut report = StainingApplicationReport {
        rows_unavailable: dye_rows.len().saturating_sub(rows.len()),
        ..StainingApplicationReport::default()
    };
    for (row, dye_row) in rows.iter_mut().zip(dye_rows) {
        report.rows_considered += 1;
        let Some(stain_id) = stain_ids
            .first()
            .copied()
            .filter(|stain_id| *stain_id != 0 && *stain_id <= MAX_STAIN_ID)
        else {
            report.rows_skipped_no_stain += 1;
            continue;
        };
        let Some(StainingTemplateDye::Legacy(dye)) = template.dye(dye_row.template, stain_id)
        else {
            report.rows_skipped_missing_template += 1;
            continue;
        };
        if apply_legacy_dye(row, dye_row, dye) {
            report.rows_changed += 1;
        }
    }
    report
}

fn apply_dawntrail_rows(
    rows: &mut [ColorTableRowColors],
    dye_rows: &[ModelDawntrailColorDyeTableRow],
    stain_ids: &[u8],
    template: &StainingTemplate,
) -> StainingApplicationReport {
    let mut report = StainingApplicationReport {
        rows_unavailable: dye_rows.len().saturating_sub(rows.len()),
        ..StainingApplicationReport::default()
    };
    for (row, dye_row) in rows.iter_mut().zip(dye_rows) {
        report.rows_considered += 1;
        let Some(stain_id) = stain_ids
            .get(usize::from(dye_row.channel))
            .copied()
            .filter(|stain_id| *stain_id != 0 && *stain_id <= MAX_STAIN_ID)
        else {
            report.rows_skipped_no_stain += 1;
            continue;
        };
        let Some(dye) = template.dye(dye_row.template, stain_id) else {
            report.rows_skipped_missing_template += 1;
            continue;
        };
        let changed = match dye {
            StainingTemplateDye::Legacy(dye) => {
                apply_legacy_dye_to_dawntrail_row(row, dye_row, dye)
            }
            StainingTemplateDye::Dawntrail(dye) => apply_dawntrail_dye(row, dye_row, dye),
        };
        if changed {
            report.rows_changed += 1;
        }
    }
    report
}

fn apply_legacy_dye(
    row: &mut ColorTableRowColors,
    dye_row: &ModelLegacyColorDyeTableRow,
    dye: LegacyStainingTemplateDye,
) -> bool {
    let mut changed = false;
    changed |= replace_if(&mut row.diffuse, dye.diffuse, dye_row.diffuse);
    changed |= replace_if(
        &mut row.specular,
        dye.specular,
        dye_row.specular && !is_black(dye.specular),
    );
    changed |= replace_if(&mut row.emissive, dye.emissive, dye_row.emissive);
    changed |= replace_if(&mut row.gloss_strength, dye.shininess, dye_row.gloss);
    changed |= replace_if(
        &mut row.specular_strength,
        dye.specular_mask,
        dye_row.specular_strength,
    );
    changed
}

fn apply_legacy_dye_to_dawntrail_row(
    row: &mut ColorTableRowColors,
    dye_row: &ModelDawntrailColorDyeTableRow,
    dye: LegacyStainingTemplateDye,
) -> bool {
    let mut changed = false;
    changed |= replace_if(&mut row.diffuse, dye.diffuse, dye_row.diffuse);
    changed |= replace_if(
        &mut row.specular,
        dye.specular,
        dye_row.specular && !is_black(dye.specular),
    );
    changed |= replace_if(&mut row.emissive, dye.emissive, dye_row.emissive);
    changed |= replace_if(&mut row.gloss_strength, dye.shininess, dye_row.scalar3);
    changed |= replace_if(
        &mut row.specular_strength,
        dye.specular_mask,
        dye_row.metalness,
    );
    changed
}

fn apply_dawntrail_dye(
    row: &mut ColorTableRowColors,
    dye_row: &ModelDawntrailColorDyeTableRow,
    dye: DawntrailStainingTemplateDye,
) -> bool {
    let mut changed = false;
    changed |= replace_if(&mut row.diffuse, dye.diffuse, dye_row.diffuse);
    changed |= replace_if(
        &mut row.specular,
        dye.specular,
        dye_row.specular && !is_black(dye.specular),
    );
    changed |= replace_if(&mut row.emissive, dye.emissive, dye_row.emissive);
    changed |= replace_if(&mut row.scalar3, dye.scalar3, dye_row.scalar3);
    changed |= replace_if(&mut row.metalness, dye.metalness, dye_row.metalness);
    changed |= replace_if(&mut row.roughness, dye.roughness, dye_row.roughness);
    changed |= replace_if(&mut row.sheen_rate, dye.sheen_rate, dye_row.sheen_rate);
    changed |= replace_if(
        &mut row.sheen_tint,
        dye.sheen_tint_rate,
        dye_row.sheen_tint_rate,
    );
    changed |= replace_if(
        &mut row.sheen_aperture,
        dye.sheen_aperture,
        dye_row.sheen_aperture,
    );
    changed |= replace_if(&mut row.anisotropy, dye.anisotropy, dye_row.anisotropy);
    changed |= replace_if(
        &mut row.sphere_index,
        f32::from(dye.sphere_map_index),
        dye_row.sphere_map_index,
    );
    changed |= replace_if(
        &mut row.sphere_mask,
        dye.sphere_map_mask,
        dye_row.sphere_map_mask,
    );
    changed
}

fn replace_if<T: Copy + PartialEq>(target: &mut T, value: T, enabled: bool) -> bool {
    if enabled && *target != value {
        *target = value;
        true
    } else {
        false
    }
}

fn is_black(color: [f32; 3]) -> bool {
    color == [0.0; 3]
}

fn parse_entry(
    bytes: &[u8],
    color_count: usize,
    scalar_count: usize,
) -> Result<StainingTemplateEntry> {
    let column_count = color_count + scalar_count;
    let column_table_size = column_count
        .checked_mul(2)
        .context("STM column table size overflow")?;
    ensure!(
        bytes.len() >= column_table_size,
        "STM entry column table is truncated"
    );

    let mut ranges = Vec::with_capacity(column_count);
    let mut previous_end = 0usize;
    for column in 0..column_count {
        let end = usize::from(read_u16(bytes, column * 2, "STM column end")?)
            .checked_mul(2)
            .context("STM column end overflow")?;
        ensure!(end >= previous_end, "STM column ends are not monotonic");
        ranges.push((previous_end, end));
        previous_end = end;
    }

    let data_end = column_table_size
        .checked_add(previous_end)
        .context("STM entry data end overflow")?;
    let data = bytes
        .get(column_table_size..data_end)
        .context("STM entry column data is truncated")?;
    let mut colors = Vec::with_capacity(color_count);
    let mut scalars = Vec::with_capacity(scalar_count);
    for (column, (start, end)) in ranges.into_iter().enumerate() {
        let column_bytes = &data[start..end];
        if column < color_count {
            colors.push(decode_array(column_bytes, 6, parse_half_color)?);
        } else {
            scalars.push(decode_array(column_bytes, 2, parse_half_scalar)?);
        }
    }

    Ok(StainingTemplateEntry { colors, scalars })
}

fn decode_array<T: Copy + Default>(
    bytes: &[u8],
    element_size: usize,
    parse: impl Fn(&[u8]) -> Result<T>,
) -> Result<Vec<T>> {
    if bytes.is_empty() {
        return Ok(vec![T::default(); STAIN_COUNT]);
    }
    if bytes.len() == element_size {
        return Ok(vec![parse(bytes)?; STAIN_COUNT]);
    }
    if bytes.len() == element_size * STAIN_COUNT {
        return bytes.chunks_exact(element_size).map(parse).collect();
    }

    ensure!(
        bytes.len() >= STAIN_COUNT,
        "STM indexed column is shorter than its index table"
    );
    let palette_bytes = bytes.len() - STAIN_COUNT;
    ensure!(
        palette_bytes.is_multiple_of(element_size),
        "STM indexed column palette is misaligned"
    );
    let palette_count = palette_bytes / element_size;
    ensure!(
        palette_count < STAIN_COUNT,
        "STM indexed column palette is too large"
    );

    let mut palette = Vec::with_capacity(palette_count + 1);
    palette.push(T::default());
    for value in bytes[..palette_bytes].chunks_exact(element_size) {
        palette.push(parse(value)?);
    }

    let indices = &bytes[palette_bytes..];
    ensure!(indices[0] == 0xff, "STM indexed column marker is not 0xFF");
    let mut values = Vec::with_capacity(STAIN_COUNT);
    for stain_index in 0..STAIN_COUNT {
        let palette_index = if stain_index + 1 < STAIN_COUNT {
            usize::from(indices[stain_index + 1])
        } else {
            0
        };
        values.push(palette.get(palette_index).copied().unwrap_or_default());
    }
    Ok(values)
}

fn parse_half_color(bytes: &[u8]) -> Result<[f32; 3]> {
    ensure!(bytes.len() == 6, "STM half color has invalid size");
    Ok([
        f16::from_bits(u16::from_le_bytes([bytes[0], bytes[1]])).to_f32(),
        f16::from_bits(u16::from_le_bytes([bytes[2], bytes[3]])).to_f32(),
        f16::from_bits(u16::from_le_bytes([bytes[4], bytes[5]])).to_f32(),
    ])
}

fn parse_half_scalar(bytes: &[u8]) -> Result<f32> {
    ensure!(bytes.len() == 2, "STM half scalar has invalid size");
    Ok(f16::from_bits(u16::from_le_bytes([bytes[0], bytes[1]])).to_f32())
}

fn read_key(bytes: &[u8], offset: usize, width: usize) -> Result<u32> {
    match width {
        2 => Ok(u32::from(read_u16(bytes, offset, "STM key/offset")?)),
        4 => {
            let bytes = bytes
                .get(offset..offset + 4)
                .context("STM key/offset is truncated")?;
            Ok(u32::from_le_bytes(bytes.try_into()?))
        }
        _ => bail!("unsupported STM key width {width}"),
    }
}

fn read_u16(bytes: &[u8], offset: usize, label: &str) -> Result<u16> {
    let bytes = bytes
        .get(offset..offset + 2)
        .with_context(|| format!("{label} is truncated"))?;
    Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_legacy_staining_template_singletons() {
        let columns = vec![
            half_color([0.25, 0.5, 1.0]),
            half_color([1.0, 0.5, 0.25]),
            half_color([0.0, 0.25, 0.5]),
            half_scalar(16.0),
            half_scalar(0.75),
        ];
        let bytes = staining_fixture(STM_VERSION_LEGACY, 3, 2, 42, 4, columns);
        let template = StainingTemplate::from_bytes(&bytes).expect("parse legacy STM");

        assert_eq!(template.kind(), StainingTemplateKind::Legacy);
        assert_eq!(template.version(), STM_VERSION_LEGACY);
        assert_eq!(template.entry_count(), 1);
        assert!(template.contains_template(42));
        assert_eq!(template.dye(42, 0), None);
        assert_eq!(template.dye(42, 255), None);
        assert_eq!(
            template.dye(42, 1),
            Some(StainingTemplateDye::Legacy(LegacyStainingTemplateDye {
                diffuse: [0.25, 0.5, 1.0],
                specular: [1.0, 0.5, 0.25],
                emissive: [0.0, 0.25, 0.5],
                shininess: 16.0,
                specular_mask: 0.75,
            }))
        );
        assert_eq!(template.dye(42, 254), template.dye(42, 1));
        assert_eq!(template.dye(1042, 1), template.dye(42, 1));
    }

    #[test]
    fn parses_legacy_staining_template_with_u16_keys() {
        let columns = vec![
            half_color([1.0, 1.0, 1.0]),
            half_color([0.5, 0.5, 0.5]),
            half_color([0.0, 0.0, 0.0]),
            half_scalar(8.0),
            half_scalar(1.0),
        ];
        let bytes = staining_fixture(STM_VERSION_LEGACY, 3, 2, 42, 2, columns);
        let template = StainingTemplate::from_bytes(&bytes).expect("parse u16 legacy STM");

        assert!(template.contains_template(42));
        assert!(template.dye(42, 1).is_some());
    }

    #[test]
    fn parses_dawntrail_staining_template_singletons() {
        let columns = vec![
            half_color([0.5, 0.25, 0.125]),
            half_color([1.0, 0.5, 0.25]),
            half_color([0.0, 0.125, 0.25]),
            half_scalar(0.75),
            half_scalar(0.5),
            half_scalar(0.25),
            half_scalar(0.125),
            half_scalar(0.375),
            half_scalar(2.0),
            half_scalar(-0.5),
            half_scalar(7.0),
            half_scalar(0.625),
        ];
        let bytes = staining_fixture(STM_VERSION_2_0, 3, 9, 1001, 4, columns);
        let template = StainingTemplate::from_bytes(&bytes).expect("parse GUD STM");

        assert_eq!(template.kind(), StainingTemplateKind::Dawntrail);
        assert_eq!(
            template.dye(1001, 1),
            Some(StainingTemplateDye::Dawntrail(
                DawntrailStainingTemplateDye {
                    diffuse: [0.5, 0.25, 0.125],
                    specular: [1.0, 0.5, 0.25],
                    emissive: [0.0, 0.125, 0.25],
                    scalar3: 0.75,
                    metalness: 0.5,
                    roughness: 0.25,
                    sheen_rate: 0.125,
                    sheen_tint_rate: 0.375,
                    sheen_aperture: 2.0,
                    anisotropy: -0.5,
                    sphere_map_index: 7,
                    sphere_map_mask: 0.625,
                }
            ))
        );
    }

    #[test]
    fn decodes_repeating_direct_and_indexed_columns() {
        assert_eq!(
            decode_array(&half_scalar(0.5), 2, parse_half_scalar).expect("repeat")[253],
            0.5
        );

        let direct = (0..STAIN_COUNT)
            .flat_map(|index| half_scalar(index as f32))
            .collect::<Vec<_>>();
        let decoded = decode_array(&direct, 2, parse_half_scalar).expect("direct");
        assert_eq!(decoded[0], 0.0);
        assert_eq!(decoded[32], 32.0);

        let mut indexed = Vec::new();
        indexed.extend_from_slice(&half_scalar(0.25));
        indexed.extend_from_slice(&half_scalar(0.75));
        let mut indices = vec![0u8; STAIN_COUNT];
        indices[0] = 0xff;
        indices[1] = 1;
        indices[2] = 2;
        indexed.extend_from_slice(&indices);
        let decoded = decode_array(&indexed, 2, parse_half_scalar).expect("indexed");
        assert_eq!(decoded[0], 0.25);
        assert_eq!(decoded[1], 0.75);
        assert_eq!(decoded[2], 0.0);
        assert_eq!(decoded[253], 0.0);
    }

    #[test]
    fn rejects_unknown_staining_template_version() {
        let mut bytes = vec![0u8; 8];
        bytes[0..2].copy_from_slice(&STM_MAGIC.to_le_bytes());
        bytes[2..4].copy_from_slice(&0x0300u16.to_le_bytes());

        assert!(StainingTemplate::from_bytes(&bytes).is_err());
    }

    #[test]
    fn applies_legacy_dye_flags_and_preserves_black_specular() {
        let template = StainingTemplate {
            kind: StainingTemplateKind::Legacy,
            version: STM_VERSION_LEGACY,
            entries: HashMap::from([(
                42,
                repeated_entry(
                    [[0.25, 0.5, 1.0], [0.0, 0.0, 0.0], [0.1, 0.2, 0.3]],
                    &[16.0, 0.75],
                ),
            )]),
        };
        let dye_table = ModelColorDyeTable::Legacy(vec![ModelLegacyColorDyeTableRow {
            template: 42,
            diffuse: true,
            specular: true,
            emissive: true,
            gloss: true,
            specular_strength: true,
        }]);
        let mut rows = vec![ColorTableRowColors {
            specular: [0.4, 0.5, 0.6],
            ..ColorTableRowColors::default()
        }];

        let report = apply_staining_template_to_rows(&mut rows, &dye_table, &[1], &template);

        assert_eq!(report.rows_changed, 1);
        assert_eq!(rows[0].diffuse, [0.25, 0.5, 1.0]);
        assert_eq!(rows[0].specular, [0.4, 0.5, 0.6]);
        assert_eq!(rows[0].emissive, [0.1, 0.2, 0.3]);
        assert_eq!(rows[0].gloss_strength, 16.0);
        assert_eq!(rows[0].specular_strength, 0.75);
    }

    #[test]
    fn applies_dawntrail_dye_by_channel_and_flag() {
        let template = StainingTemplate {
            kind: StainingTemplateKind::Dawntrail,
            version: STM_VERSION_2_1,
            entries: HashMap::from([
                (
                    1001,
                    repeated_entry(
                        [[0.25, 0.5, 1.0], [1.0, 1.0, 1.0], [0.0, 0.0, 0.0]],
                        &[0.1, 0.2, 0.3, 0.4, 0.5, 2.0, -0.25, 7.0, 0.75],
                    ),
                ),
                (
                    1002,
                    repeated_entry(
                        [[1.0, 0.5, 0.25], [0.5, 0.5, 0.5], [0.1, 0.2, 0.3]],
                        &[0.9, 0.8, 0.7, 0.6, 0.5, 4.0, 0.25, 9.0, 0.35],
                    ),
                ),
            ]),
        };
        let mut first = dawntrail_dye_row(1001, 0);
        first.diffuse = true;
        first.roughness = true;
        let mut second = dawntrail_dye_row(1002, 1);
        second.metalness = true;
        second.sphere_map_index = true;
        let dye_table = ModelColorDyeTable::Dawntrail(vec![first, second]);

        let mut rows = vec![ColorTableRowColors::default(); 2];
        let first_report =
            apply_staining_template_to_rows(&mut rows, &dye_table, &[1, 0], &template);
        assert_eq!(first_report.rows_changed, 1);
        assert_eq!(first_report.rows_skipped_no_stain, 1);
        assert_eq!(rows[0].diffuse, [0.25, 0.5, 1.0]);
        assert_eq!(rows[0].roughness, 0.3);
        assert_eq!(rows[1].metalness, 0.0);

        rows = vec![ColorTableRowColors::default(); 2];
        let second_report =
            apply_staining_template_to_rows(&mut rows, &dye_table, &[0, 1], &template);
        assert_eq!(second_report.rows_changed, 1);
        assert_eq!(second_report.rows_skipped_no_stain, 1);
        assert_eq!(rows[0].diffuse, [0.0; 3]);
        assert_eq!(rows[1].metalness, 0.8);
        assert_eq!(rows[1].sphere_index, 9.0);
    }

    #[test]
    fn reports_staining_template_kind_mismatch() {
        let template = StainingTemplate {
            kind: StainingTemplateKind::Dawntrail,
            version: STM_VERSION_2_1,
            entries: HashMap::new(),
        };
        let dye_table = ModelColorDyeTable::Legacy(vec![ModelLegacyColorDyeTableRow {
            template: 42,
            diffuse: true,
            specular: false,
            emissive: false,
            gloss: false,
            specular_strength: false,
        }]);
        let mut rows = vec![ColorTableRowColors::default()];

        let report = apply_staining_template_to_rows(&mut rows, &dye_table, &[1], &template);

        assert!(report.template_kind_mismatch);
        assert_eq!(report.rows_changed, 0);
    }

    #[test]
    #[ignore = "requires an installed FFXIV game directory"]
    fn parses_installed_game_staining_templates() {
        use std::path::PathBuf;

        use physis::resource::{Resource, SqPackResource};

        let game_dir = std::env::var_os("XIV_GAME_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(r"E:\_ff14\game"));
        let game_dir = game_dir.to_string_lossy();
        let mut resource = SqPackResource::from_existing(&game_dir);

        for (path, expected_kind) in [
            (LEGACY_STAINING_TEMPLATE_PATH, StainingTemplateKind::Legacy),
            (
                DAWNTRAIL_STAINING_TEMPLATE_PATH,
                StainingTemplateKind::Dawntrail,
            ),
        ] {
            let bytes = resource.read(path).expect("read staining template");
            let template = StainingTemplate::from_bytes(&bytes).expect("parse staining template");
            assert_eq!(template.kind(), expected_kind);
            assert!(template.entry_count() > 0);

            let key = *template.entries.keys().next().expect("template entry");
            let key = u16::try_from(key).expect("template key fits u16");
            assert!(template.dye(key, 1).is_some());
            let min_key = template.entries.keys().min().expect("minimum template key");
            let max_key = template.entries.keys().max().expect("maximum template key");
            eprintln!(
                "parsed {path}: version=0x{:04X}, entries={}, keys={min_key}..={max_key}",
                template.version(),
                template.entry_count()
            );
        }
    }

    fn staining_fixture(
        version: u16,
        color_count: u8,
        scalar_count: u8,
        key: u32,
        key_width: usize,
        columns: Vec<Vec<u8>>,
    ) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&STM_MAGIC.to_le_bytes());
        bytes.extend_from_slice(&version.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        if version == STM_VERSION_LEGACY {
            bytes.extend_from_slice(&[0, 0]);
        } else {
            bytes.extend_from_slice(&[color_count, scalar_count]);
        }
        match key_width {
            2 => {
                bytes.extend_from_slice(&(key as u16).to_le_bytes());
                bytes.extend_from_slice(&1u16.to_le_bytes());
            }
            4 => {
                bytes.extend_from_slice(&key.to_le_bytes());
                bytes.extend_from_slice(&0u32.to_le_bytes());
            }
            _ => panic!("unsupported fixture key width"),
        }
        if key_width == 2 {
            bytes.extend_from_slice(&[0, 0]);
        }

        let mut cumulative_bytes = 0usize;
        for column in &columns {
            cumulative_bytes += column.len();
            bytes.extend_from_slice(&u16::try_from(cumulative_bytes / 2).unwrap().to_le_bytes());
        }
        for column in columns {
            bytes.extend_from_slice(&column);
        }
        bytes
    }

    fn half_color(color: [f32; 3]) -> Vec<u8> {
        color
            .into_iter()
            .flat_map(|value| f16::from_f32(value).to_bits().to_le_bytes())
            .collect()
    }

    fn half_scalar(value: f32) -> Vec<u8> {
        f16::from_f32(value).to_bits().to_le_bytes().to_vec()
    }

    fn repeated_entry(colors: [[f32; 3]; 3], scalars: &[f32]) -> StainingTemplateEntry {
        StainingTemplateEntry {
            colors: colors
                .into_iter()
                .map(|color| vec![color; STAIN_COUNT])
                .collect(),
            scalars: scalars
                .iter()
                .map(|scalar| vec![*scalar; STAIN_COUNT])
                .collect(),
        }
    }

    fn dawntrail_dye_row(template: u16, channel: u8) -> ModelDawntrailColorDyeTableRow {
        ModelDawntrailColorDyeTableRow {
            template,
            channel,
            diffuse: false,
            specular: false,
            emissive: false,
            scalar3: false,
            metalness: false,
            roughness: false,
            sheen_rate: false,
            sheen_tint_rate: false,
            sheen_aperture: false,
            anisotropy: false,
            sphere_map_index: false,
            sphere_map_mask: false,
        }
    }
}
