//! 武器、装备与饰品共用的 `.imc`（variant / material-set 表）读取器。
//!
//! 布局依据 xivModdingFramework `Variants/FileTypes/Imc.cs`：文件头为
//! `subset_count: i16 + kind: i16`（1 = NonSet（武器、怪物），31 = Set（装备）），
//! 之后是 1 个默认子集 + `subset_count` 个子集；每个子集 NonSet 含 1 条、
//! Set 含 5 条（met/top/glv/dwn/sho，饰品为 ear/nek/wrs/rir/ril）条目，
//! 每条 6 字节：MaterialSet(u8)、Decal(u8)、Mask(u16)、Vfx(u8)、Animation(u8)。
//! 子集 id 为 1 起（`index = subsetID - 1`）。

use std::fmt;

/// 一条 variant 条目；`vfx` 即武器 avfx 的 VfxId（0 = 无特效）。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImcEntry {
    pub material_set: u8,
    pub decal: u8,
    pub mask: u16,
    pub vfx: u8,
    pub animation: u8,
}

impl ImcEntry {
    /// MDL 本地 attribute 可见性位；高 6 位另存 sound id。
    pub fn attribute_mask(&self) -> u16 {
        self.mask & 0x3FF
    }
    pub fn sound_id(&self) -> u8 {
        (self.mask >> 10) as u8
    }
}

/// 装备/饰品 set 的 IMC 文件路径（`chara/equipment/e####/e####.imc` /
/// `chara/accessory/a####/a####.imc`）。
pub fn equipment_imc_path(set_id: u16, is_accessory: bool) -> String {
    if is_accessory {
        format!("chara/accessory/a{set_id:04}/a{set_id:04}.imc")
    } else {
        format!("chara/equipment/e{set_id:04}/e{set_id:04}.imc")
    }
}

/// Set 型 IMC 的槽位偏移（met/top/glv/dwn/sho → 0..4，xivModdingFramework
/// `Imc.SlotOffsetDictionary`）。
pub fn imc_slot_offset(abbreviation: &str) -> Option<usize> {
    match abbreviation {
        "met" | "ear" => Some(0),
        "top" | "nek" => Some(1),
        "glv" | "wrs" => Some(2),
        "dwn" | "rir" => Some(3),
        "sho" | "ril" => Some(4),
        _ => None,
    }
}

/// `.imc` 解析错误；本模块保持零依赖，不引入 anyhow。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImcParseError {
    TooShort { len: usize },
    UnknownKind { raw_kind: i16 },
    InvalidSubsetCount { count: i16 },
    Truncated { len: usize, expected: usize },
}

impl fmt::Display for ImcParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooShort { len } => {
                write!(
                    f,
                    "imc too short: {len} bytes (need at least the 4-byte header)"
                )
            }
            Self::InvalidSubsetCount { count } => write!(f, "negative imc subset count {count}"),
            Self::UnknownKind { raw_kind } => write!(
                f,
                "unknown imc kind {raw_kind} (expected 1 = NonSet or 31 = Set)"
            ),
            Self::Truncated { len, expected } => {
                write!(f, "imc truncated: {len} bytes, expected {expected}")
            }
        }
    }
}

impl std::error::Error for ImcParseError {}

/// IMC 文件种类（头部第二段 `i16`）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ImcKind {
    /// 武器、怪物等：每子集 1 条。
    NonSet,
    /// 装备/饰品：每子集 5 条（按槽位）。
    Set,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImcFile {
    pub kind: Option<ImcKind>,
    pub raw_kind: i16,
    pub subset_count: u16,
    /// 默认子集（variant 0 / 未命中任何子集时使用）。
    pub default_subset: Vec<ImcEntry>,
    /// 子集列表；`subsets[i]` 对应 variant `i + 1`。
    pub subsets: Vec<Vec<ImcEntry>>,
    /// 校验长度之后的尾部字节数（对齐填充等）。
    pub trailing_bytes: usize,
}

impl ImcFile {
    pub fn parse(bytes: &[u8]) -> Result<Self, ImcParseError> {
        if bytes.len() < 4 {
            return Err(ImcParseError::TooShort { len: bytes.len() });
        }
        let count = i16::from_le_bytes([bytes[0], bytes[1]]);
        if count < 0 {
            return Err(ImcParseError::InvalidSubsetCount { count });
        }
        let subset_count = count as u16;
        let raw_kind = i16::from_le_bytes([bytes[2], bytes[3]]);
        let kind = match raw_kind {
            1 => Some(ImcKind::NonSet),
            31 => Some(ImcKind::Set),
            _ => None,
        };
        let kind = kind.ok_or(ImcParseError::UnknownKind { raw_kind })?;
        let entries_per_subset = match kind {
            ImcKind::NonSet => 1,
            ImcKind::Set => 5,
        };
        let subset_count_usize = subset_count as usize;
        let total_subsets = subset_count_usize + 1;
        let expected = 4 + entries_per_subset * total_subsets * 6;
        if bytes.len() < expected {
            return Err(ImcParseError::Truncated {
                len: bytes.len(),
                expected,
            });
        }

        let mut offset = 4;
        let read_subset = |offset: &mut usize| -> Vec<ImcEntry> {
            let mut entries = Vec::with_capacity(entries_per_subset);
            for _ in 0..entries_per_subset {
                let entry_bytes = &bytes[*offset..*offset + 6];
                entries.push(ImcEntry {
                    material_set: entry_bytes[0],
                    decal: entry_bytes[1],
                    mask: u16::from_le_bytes([entry_bytes[2], entry_bytes[3]]),
                    vfx: entry_bytes[4],
                    animation: entry_bytes[5],
                });
                *offset += 6;
            }
            entries
        };
        let default_subset = read_subset(&mut offset);
        let subsets = (0..subset_count_usize)
            .map(|_| read_subset(&mut offset))
            .collect();

        Ok(Self {
            kind: Some(kind),
            raw_kind,
            subset_count,
            default_subset,
            subsets,
            trailing_bytes: bytes.len() - expected,
        })
    }

    /// 子集按 variant 查询；非法槽位回退第一个条目，NonSet 恒为单条。
    pub fn entry(&self, variant: u16, slot_offset: usize) -> &ImcEntry {
        let subset = self.subset_for_variant(variant);
        subset.get(slot_offset).unwrap_or(&subset[0])
    }

    /// 取 variant 对应的子集（1 起）；0 或越界回落到默认子集。
    pub fn subset_for_variant(&self, variant: u16) -> &[ImcEntry] {
        if variant == 0 {
            return &self.default_subset;
        }
        self.subsets
            .get(variant as usize - 1)
            .map_or(&self.default_subset, |subset| subset.as_slice())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(material_set: u8, decal: u8, mask: u16, vfx: u8, animation: u8) -> [u8; 6] {
        let mask = mask.to_le_bytes();
        [material_set, decal, mask[0], mask[1], vfx, animation]
    }

    #[test]
    fn parses_non_set_weapon_table() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&2_i16.to_le_bytes());
        bytes.extend_from_slice(&1_i16.to_le_bytes());
        bytes.extend_from_slice(&entry(1, 0, 0, 0, 0));
        bytes.extend_from_slice(&entry(1, 0, 0x0304, 13, 2));
        bytes.extend_from_slice(&entry(2, 0, 0, 44, 0));

        let imc = ImcFile::parse(&bytes).expect("non-set imc should parse");
        assert_eq!(imc.kind, Some(ImcKind::NonSet));
        assert_eq!(imc.subset_count, 2);
        assert_eq!(imc.default_subset.len(), 1);
        assert_eq!(imc.default_subset[0].vfx, 0);
        assert_eq!(
            imc.subset_for_variant(1),
            &[ImcEntry {
                material_set: 1,
                decal: 0,
                mask: 0x0304,
                vfx: 13,
                animation: 2
            }]
        );
        assert_eq!(imc.subset_for_variant(2)[0].vfx, 44);
        assert_eq!(imc.trailing_bytes, 0);
    }

    #[test]
    fn variant_lookup_falls_back_to_default_subset() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&1_i16.to_le_bytes());
        bytes.extend_from_slice(&1_i16.to_le_bytes());
        bytes.extend_from_slice(&entry(7, 0, 0, 0, 9));
        bytes.extend_from_slice(&entry(3, 0, 0, 21, 0));

        let imc = ImcFile::parse(&bytes).expect("imc should parse");
        assert_eq!(imc.subset_for_variant(0)[0].material_set, 7);
        assert_eq!(imc.subset_for_variant(1)[0].vfx, 21);
        assert_eq!(imc.subset_for_variant(2)[0].material_set, 7);
        assert_eq!(imc.subset_for_variant(99)[0].material_set, 7);
    }

    #[test]
    fn parses_set_equipment_table_with_five_entries_per_subset() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&1_i16.to_le_bytes());
        bytes.extend_from_slice(&31_i16.to_le_bytes());
        for material_set in [1, 2] {
            for slot in 0..5 {
                bytes.extend_from_slice(&entry(material_set, 0, 0, 0, slot as u8));
            }
        }

        let imc = ImcFile::parse(&bytes).expect("set imc should parse");
        assert_eq!(imc.kind, Some(ImcKind::Set));
        assert_eq!(imc.default_subset.len(), 5);
        assert_eq!(imc.subsets.len(), 1);
        assert_eq!(imc.subsets[0].len(), 5);
        assert_eq!(imc.subsets[0][0].material_set, 2);
        assert!(imc.subsets[0].iter().all(|entry| entry.vfx == 0));
    }

    #[test]
    fn rejects_unknown_kind_and_truncated_tables() {
        let mut unknown = Vec::new();
        unknown.extend_from_slice(&0_i16.to_le_bytes());
        unknown.extend_from_slice(&99_i16.to_le_bytes());
        let error = ImcFile::parse(&unknown).expect_err("unknown kind should fail");
        assert_eq!(error, ImcParseError::UnknownKind { raw_kind: 99 });

        let truncated = [1_u8, 0, 1, 0, 1, 0];
        assert_eq!(
            ImcFile::parse(&truncated),
            Err(ImcParseError::Truncated {
                len: 6,
                expected: 4 + 2 * 6
            })
        );
        assert_eq!(ImcFile::parse(&[]), Err(ImcParseError::TooShort { len: 0 }));
    }
    fn imc_entry_bytes(material_set: u8, mask: u16) -> [u8; 6] {
        let mut bytes = [0; 6];
        bytes[0] = material_set;
        bytes[2..4].copy_from_slice(&mask.to_le_bytes());
        bytes
    }

    /// Set 型 IMC：2 子集，默认子集全 0，子集 1 全 material_set=1，子集 2
    /// 全 material_set=2（mask 按槽位区分）。
    fn set_imc_fixture() -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&2_i16.to_le_bytes());
        bytes.extend_from_slice(&31_i16.to_le_bytes());
        for _ in 0..5 {
            bytes.extend_from_slice(&imc_entry_bytes(1, 0x3FF));
        }
        for slot in 0..5 {
            bytes.extend_from_slice(&imc_entry_bytes(1, 0x3FF & !(1 << slot)));
        }
        for slot in 0..5 {
            bytes.extend_from_slice(&imc_entry_bytes(2, 0x0001 << slot));
        }
        bytes
    }

    #[test]
    fn imc_set_lookup_is_one_based_with_default_fallback() {
        let imc = ImcFile::parse(&set_imc_fixture()).expect("parse Set IMC");
        assert_eq!(imc.kind, Some(ImcKind::Set));
        assert_eq!(imc.subset_count, 2);
        // 1 基：subset 1 → 第一子集（mask 清除对应槽位）。
        assert_eq!(imc.entry(1, 1).material_set, 1);
        assert_eq!(imc.entry(1, 1).mask, 0x3FF & !0b10);
        assert_eq!(imc.entry(2, 4).material_set, 2);
        assert_eq!(imc.entry(2, 4).mask, 0x0010);
        // 0 与越界子集 → 默认子集。
        assert_eq!(imc.entry(0, 0).mask, 0x3FF);
        assert_eq!(imc.entry(3, 2).mask, 0x3FF);
        // 槽位偏移越界钳到 0。
        assert_eq!(imc.entry(2, 9).mask, imc.entry(2, 0).mask);
        // attribute 位与 sound id 拆分。
        let entry = ImcEntry {
            mask: 0x47FE,
            ..ImcEntry::default()
        };
        assert_eq!(entry.attribute_mask(), 0x03FE);
        assert_eq!(entry.sound_id(), 0x11);
    }

    #[test]
    fn imc_non_set_lookup() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&2_i16.to_le_bytes());
        bytes.extend_from_slice(&1_i16.to_le_bytes());
        bytes.extend_from_slice(&imc_entry_bytes(1, 0x3FF));
        bytes.extend_from_slice(&imc_entry_bytes(3, 0x00FF));
        bytes.extend_from_slice(&imc_entry_bytes(4, 0x0FFF));
        let imc = ImcFile::parse(&bytes).expect("parse NonSet IMC");
        assert_eq!(imc.kind, Some(ImcKind::NonSet));
        assert_eq!(imc.subset_count, 2);
        assert_eq!(imc.entry(1, 0).material_set, 3);
        assert_eq!(imc.entry(2, 0).material_set, 4);
        assert_eq!(imc.entry(2, 0).attribute_mask(), 0x03FF);
        // 默认子集回退。
        assert_eq!(imc.entry(9, 0).material_set, 1);
        // 槽位偏移在 NonSet 下钳到唯一条目。
        assert_eq!(imc.entry(1, 3).material_set, 3);
    }

    #[test]
    fn imc_rejects_malformed_bytes() {
        assert!(ImcFile::parse(&[0; 3]).is_err());
        assert!(ImcFile::parse(&[1, 0, 2, 0]).is_err());
        // 声明 2 子集但字节不足。
        let mut bytes = set_imc_fixture();
        bytes.truncate(bytes.len() - 6);
        assert!(ImcFile::parse(&bytes).is_err());
    }

    #[test]
    fn rejects_negative_subset_count() {
        assert_eq!(
            ImcFile::parse(&[255, 255, 1, 0]),
            Err(ImcParseError::InvalidSubsetCount { count: -1 })
        );
    }
}
