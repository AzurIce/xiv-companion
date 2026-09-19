//! 装备参数文件（IMC/EQP）解析——着装角色装配的变体显隐与身体遮蔽数据源。
//!
//! 语义来源（xivModdingFramework 为权威，均经真实 SqPack 数据核对）：
//! - IMC（`Variants/FileTypes/Imc.cs` + `Variants/DataContainers/XivImc.cs`）：
//!   每件装备/饰品 set 一个 `.imc`，头部 `{ subset_count: i16, type: i16 }`
//!   （type 1 = NonSet 单槽，31 = Set 五槽），随后默认子集（NonSet 1 条 /
//!   Set 5 条，槽序 met/top/glv/dwn/sho）+ subset_count 个子集。条目 6 字节
//!   `{ material_set: u8, decal: u8, mask: u16, vfx: u8, animation: u8 }`；
//!   `mask` 低 10 位是 MDL 本地 attribute 可见性位（bit i = 该 MDL attribute
//!   表第 i 项），高 6 位是 sound id（忽略）。子集查找 1 基（= 物品
//!   Model{Main} 的 IMC 子集 id，即 [`crate::model::PackedEquipmentModelId`]
//!   的 variant_id），0/越界 → 默认子集。`material_set` 即材质版本 v####。
//! - EQP（`Models/FileTypes/Eqp.cs` + `Models/DataContainers/EquipmentParameter.cs`）：
//!   `chara/xls/equipmentparameter/equipmentparameter.eqp` 按 160 套一块稀疏
//!   存储：文件头 8 字节是块存在位表（64 块 = 10240 套上限；位表占用 set 0
//!   的条目槽，故 set 0 不可读，游戏硬编码其与 set 1 共享），块内每套 8 字节
//!   小端 u64 标志位（bit n = byte[n/8] 的 bit n%8）。标志位语义见
//!   [`EquipmentParameterEntry`] 的位常量（`EquipmentParameterFlag` 枚举）。

/// 装备/饰品 set 的 IMC 文件路径（`chara/equipment/e####/e####.imc` /
/// `chara/accessory/a####/a####.imc`）。
pub fn equipment_imc_path(set_id: u16, is_accessory: bool) -> String {
    if is_accessory {
        format!("chara/accessory/a{set_id:04}/a{set_id:04}.imc")
    } else {
        format!("chara/equipment/e{set_id:04}/e{set_id:04}.imc")
    }
}

/// EQP 表文件路径（全游戏唯一一份）。
pub const EQUIPMENT_PARAMETER_PATH: &str = "chara/xls/equipmentparameter/equipmentparameter.eqp";

/// IMC type 标识：NonSet（单槽，武器/饰品/怪物用）。
pub const IMC_TYPE_NON_SET: i16 = 1;
/// IMC type 标识：Set（五槽装备套装）。
pub const IMC_TYPE_SET: i16 = 31;

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

/// IMC 条目（6 字节）。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ImcEntry {
    /// 材质版本号（材质目录 v#### 的 ####）。
    pub material_set: u8,
    pub decal: u8,
    /// 低 10 位 attribute 可见性 + 高 6 位 sound id。
    pub mask: u16,
    pub vfx: u8,
    pub animation: u8,
}

impl ImcEntry {
    /// attribute 可见性位（`mask & 0x3FF`；bit i = 该 MDL 本地 attribute 表
    /// 第 i 项，跨 MDL 数值不可比）。
    pub fn attribute_mask(&self) -> u16 {
        self.mask & 0x3FF
    }

    /// sound id（mask 高 6 位；本实现不使用，仅解析留档）。
    pub fn sound_id(&self) -> u8 {
        (self.mask >> 10) as u8
    }
}

/// IMC 文件类型。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImcKind {
    /// 单槽（饰品/武器/怪物）：默认子集 1 条，每子集 1 条。
    NonSet,
    /// 五槽套装：默认子集 5 条（met/top/glv/dwn/sho），每子集 5 条。
    Set,
}

impl ImcKind {
    fn subset_size(self) -> usize {
        match self {
            Self::NonSet => 1,
            Self::Set => 5,
        }
    }
}

/// 解析后的 IMC 文件。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImcFile {
    kind: ImcKind,
    default_subset: Vec<ImcEntry>,
    subsets: Vec<Vec<ImcEntry>>,
}

impl ImcFile {
    pub fn from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        use anyhow::anyhow;

        if bytes.len() < 4 {
            return Err(anyhow!(
                "IMC file too short for header: {} bytes",
                bytes.len()
            ));
        }
        let subset_count = i16::from_le_bytes([bytes[0], bytes[1]]);
        let type_id = i16::from_le_bytes([bytes[2], bytes[3]]);
        let kind = match type_id {
            IMC_TYPE_NON_SET => ImcKind::NonSet,
            IMC_TYPE_SET => ImcKind::Set,
            other => return Err(anyhow!("unknown IMC type identifier {other}")),
        };
        if subset_count < 0 {
            return Err(anyhow!("negative IMC subset count {subset_count}"));
        }
        let subset_size = kind.subset_size();
        let entry_count = subset_size * (1 + subset_count as usize);
        let expected_len = 4 + entry_count * 6;
        if bytes.len() < expected_len {
            return Err(anyhow!(
                "IMC file too short: {} bytes for {subset_count} subsets (expected {expected_len})",
                bytes.len()
            ));
        }

        let mut offset = 4;
        let mut read_entry = || {
            let entry = ImcEntry {
                material_set: bytes[offset],
                decal: bytes[offset + 1],
                mask: u16::from_le_bytes([bytes[offset + 2], bytes[offset + 3]]),
                vfx: bytes[offset + 4],
                animation: bytes[offset + 5],
            };
            offset += 6;
            entry
        };
        let default_subset = (0..subset_size).map(|_| read_entry()).collect();
        let subsets = (0..subset_count as usize)
            .map(|_| (0..subset_size).map(|_| read_entry()).collect())
            .collect();
        Ok(Self {
            kind,
            default_subset,
            subsets,
        })
    }

    pub fn kind(&self) -> ImcKind {
        self.kind
    }

    pub fn subset_count(&self) -> usize {
        self.subsets.len()
    }

    /// 子集条目查找：`subset_id` 1 基（物品 IMC 子集 id），0 或越界 → 默认
    /// 子集；`slot_offset`（[`imc_slot_offset`]）越界回退到 0（NonSet 恒 0）。
    pub fn entry(&self, subset_id: u16, slot_offset: usize) -> &ImcEntry {
        let subset = if subset_id >= 1 && usize::from(subset_id) <= self.subsets.len() {
            &self.subsets[usize::from(subset_id) - 1]
        } else {
            &self.default_subset
        };
        // 槽位偏移越界回退到 0（xivModdingFramework `FullImcInfo.GetEntry`）。
        &subset[if slot_offset < subset.len() {
            slot_offset
        } else {
            0
        }]
    }
}

/// EQP 块大小（每块 160 套）。
const EQP_BLOCK_SETS: usize = 160;
/// EQP 块存在位表长度（8 字节 = 64 块，占 set 0 条目槽）。
const EQP_BLOCK_TABLE_LEN: usize = 8;
/// EQP 条目字节数。
pub const EQUIPMENT_PARAMETER_ENTRY_LEN: usize = 8;

/// 解析后的 EQP 表（保留原始字节，按 set id 稀疏块寻址）。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EquipmentParameterTable {
    bytes: Vec<u8>,
}

impl EquipmentParameterTable {
    pub fn from_bytes(bytes: &[u8]) -> Self {
        Self {
            bytes: bytes.to_vec(),
        }
    }

    /// 套装 set id 的 EQP 条目；块缺失（稀疏压缩）或越界返回 None（调用方
    /// 按"全部显示、不遮蔽"降级）。
    pub fn entry(&self, set_id: u16) -> Option<EquipmentParameterEntry> {
        let set_id = usize::from(set_id);
        if set_id == 0 {
            return None;
        }
        let table = self.bytes.get(..EQP_BLOCK_TABLE_LEN)?;
        let block = set_id / EQP_BLOCK_SETS;
        if block >= EQP_BLOCK_TABLE_LEN * 8 || table[block / 8] & (1 << (block % 8)) == 0 {
            return None;
        }
        let mut present_before = 0_usize;
        for (byte_index, byte) in table.iter().enumerate() {
            for bit in 0..8 {
                if byte_index * 8 + bit >= block {
                    break;
                }
                if byte & (1 << bit) != 0 {
                    present_before += 1;
                }
            }
        }
        let index = present_before * EQP_BLOCK_SETS + set_id % EQP_BLOCK_SETS;
        let offset = index * EQUIPMENT_PARAMETER_ENTRY_LEN;
        let raw: [u8; 8] = self
            .bytes
            .get(offset..offset + EQUIPMENT_PARAMETER_ENTRY_LEN)?
            .try_into()
            .ok()?;
        Some(EquipmentParameterEntry {
            raw: u64::from_le_bytes(raw),
        })
    }
}

/// 单套装备的 EQP 条目（8 字节小端标志位）。位常量见
/// [`EquipmentParameterEntry`] 的关联常量（xivModdingFramework
/// `EquipmentParameterFlag`；字节分组：byte0 top、byte1 身体显示、byte2
/// dwn、byte3 glv、byte4 sho、byte5-7 met）。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EquipmentParameterEntry {
    pub raw: u64,
}

impl EquipmentParameterEntry {
    pub const BODY_HIDE_WAIST: u8 = 1;
    pub const BODY_HIDE_SHORT_GLOVES: u8 = 3;
    pub const BODY_HIDE_MID_GLOVES: u8 = 5;
    pub const BODY_HIDE_LONG_GLOVES: u8 = 6;
    pub const BODY_HIDE_GORGET: u8 = 7;
    /// 关闭时腿/手/头的遮蔽数据改从本套（top）条目解析，而非对应槽位套装。
    pub const BODY_SHOW_LEG: u8 = 8;
    pub const BODY_SHOW_HAND: u8 = 9;
    pub const BODY_SHOW_HEAD: u8 = 10;
    pub const BODY_SHOW_NECKLACE: u8 = 11;
    pub const BODY_SHOW_BRACELET: u8 = 12;
    pub const BODY_SHOW_TAIL: u8 = 13;
    pub const BODY_DISABLE_BREAST_PHYSICS: u8 = 14;
    pub const LEG_HIDE_KNEE_PADS: u8 = 17;
    pub const LEG_HIDE_SHORT_BOOT: u8 = 18;
    pub const LEG_HIDE_HALF_BOOT: u8 = 19;
    pub const LEG_SHOW_FOOT: u8 = 21;
    pub const LEG_SHOW_TAIL: u8 = 22;
    /// 需同时置位 HAND_HIDE_FOREARM 才生效。
    pub const HAND_HIDE_ELBOW: u8 = 25;
    pub const HAND_HIDE_FOREARM: u8 = 26;
    pub const HAND_SHOW_BRACELET: u8 = 28;
    pub const HAND_SHOW_RING_L: u8 = 29;
    pub const HAND_SHOW_RING_R: u8 = 30;
    /// 需同时置位 FOOT_HIDE_CALF 才生效。
    pub const FOOT_HIDE_KNEE: u8 = 33;
    pub const FOOT_HIDE_CALF: u8 = 34;
    pub const FOOT_HIDE_ANKLE: u8 = 35;
    pub const HEAD_HIDE_SCALP: u8 = 41;
    pub const HEAD_HIDE_HAIR: u8 = 42;
    pub const HEAD_SHOW_HAIR_OVERRIDE: u8 = 43;
    pub const HEAD_HIDE_NECK: u8 = 44;
    pub const HEAD_SHOW_NECKLACE: u8 = 45;
    pub const HEAD_SHOW_EARRINGS: u8 = 47;
    pub const HEAD_SHOW_EARRINGS_HUMAN: u8 = 48;
    pub const HEAD_SHOW_EARRINGS_AURA: u8 = 49;
    pub const HEAD_SHOW_EAR_HUMAN: u8 = 50;
    pub const HEAD_SHOW_EAR_MIQO: u8 = 51;
    pub const HEAD_SHOW_EAR_AURA: u8 = 52;
    pub const HEAD_SHOW_EAR_VIERA: u8 = 53;
    pub const HEAD_SHOW_HROTHGAR_HAT: u8 = 56;
    pub const HEAD_SHOW_VIERA_HAT: u8 = 57;

    pub fn flag(&self, bit: u8) -> bool {
        bit < 64 && self.raw & (1 << bit) != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        bytes.extend_from_slice(&IMC_TYPE_SET.to_le_bytes());
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
        let imc = ImcFile::from_bytes(&set_imc_fixture()).expect("parse Set IMC");
        assert_eq!(imc.kind(), ImcKind::Set);
        assert_eq!(imc.subset_count(), 2);
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
        bytes.extend_from_slice(&IMC_TYPE_NON_SET.to_le_bytes());
        bytes.extend_from_slice(&imc_entry_bytes(1, 0x3FF));
        bytes.extend_from_slice(&imc_entry_bytes(3, 0x00FF));
        bytes.extend_from_slice(&imc_entry_bytes(4, 0x0FFF));
        let imc = ImcFile::from_bytes(&bytes).expect("parse NonSet IMC");
        assert_eq!(imc.kind(), ImcKind::NonSet);
        assert_eq!(imc.subset_count(), 2);
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
        assert!(ImcFile::from_bytes(&[0; 3]).is_err());
        assert!(ImcFile::from_bytes(&[1, 0, 2, 0]).is_err());
        // 声明 2 子集但字节不足。
        let mut bytes = set_imc_fixture();
        bytes.truncate(bytes.len() - 6);
        assert!(ImcFile::from_bytes(&bytes).is_err());
    }

    /// EQP 夹具：块 0 存在，set 1 = 小衣已知字节 `01 3f 61 73 03 e0 3f 00`。
    fn eqp_fixture() -> Vec<u8> {
        let mut bytes = vec![0_u8; EQP_BLOCK_TABLE_LEN + EQP_BLOCK_SETS * 8];
        bytes[0] = 0x01;
        let set1 = 8_usize;
        bytes[set1..set1 + 8].copy_from_slice(&[0x01, 0x3f, 0x61, 0x73, 0x03, 0xe0, 0x3f, 0x00]);
        bytes
    }

    #[test]
    fn eqp_entry_resolves_sparse_block_offsets() {
        let table = EquipmentParameterTable::from_bytes(&eqp_fixture());
        // set 0 不可读（位表占其槽位，游戏硬编码与 set 1 共享）。
        assert!(table.entry(0).is_none());
        let smallclothes = table.entry(1).expect("set 1 entry");
        assert_eq!(smallclothes.raw, 0x003F_E003_7361_3F01);
        // 块 1 未存在 → None。
        assert!(table.entry(160).is_none());

        // 多块：块 0 + 块 2 存在时，set 321 落在新块第二条目。
        let mut multi = vec![0_u8; EQP_BLOCK_TABLE_LEN + EQP_BLOCK_SETS * 8 * 2];
        multi[0] = 0b0000_0101;
        multi[8..16].copy_from_slice(&[0x01, 0x3f, 0x61, 0x73, 0x03, 0xe0, 0x3f, 0x00]);
        let set321 = (EQP_BLOCK_SETS + 321 % EQP_BLOCK_SETS) * 8;
        multi[set321..set321 + 8].copy_from_slice(&[0xAA; 8]);
        let table = EquipmentParameterTable::from_bytes(&multi);
        assert_eq!(
            table.entry(1).map(|entry| entry.raw),
            Some(0x003F_E003_7361_3F01)
        );
        assert_eq!(
            table.entry(321).map(|entry| entry.raw),
            Some(u64::from_le_bytes([0xAA; 8]))
        );
        assert!(table.entry(160).is_none());
        // 越界 set（超出 64 块上限）与截断文件。
        assert!(table.entry(10240).is_none());
        assert!(
            EquipmentParameterTable::from_bytes(&[0xFF; 4])
                .entry(1)
                .is_none()
        );
        let mut truncated = eqp_fixture();
        truncated.truncate(16);
        assert!(
            EquipmentParameterTable::from_bytes(&truncated)
                .entry(159)
                .is_none()
        );
    }

    #[test]
    fn eqp_flag_bits_match_known_smallclothes_entry() {
        let table = EquipmentParameterTable::from_bytes(&eqp_fixture());
        let entry = table.entry(1).expect("set 1 entry");
        use EquipmentParameterEntry as E;
        // `01 3f 61 73 03 e0 3f 00`：byte1=0x3f → 腿/手/头/项链/手镯/尾全显示；
        // byte2=0x61 → 足/尾显示；byte3=0x73 → 隐藏肘（前臂位未置，按规则不
        // 生效）、手镯/左右戒显示；byte4=0x03 → FootHideKnee；byte5=0xe0 →
        // 项链+耳饰显示；byte6=0x3f → 各族耳饰/耳全显示；byte7=0x00 →
        // 硌狮/维埃拉帽不显示。
        assert!(entry.flag(E::BODY_SHOW_LEG));
        assert!(entry.flag(E::BODY_SHOW_HAND));
        assert!(entry.flag(E::BODY_SHOW_HEAD));
        assert!(entry.flag(E::BODY_SHOW_TAIL));
        assert!(entry.flag(E::LEG_SHOW_FOOT));
        assert!(entry.flag(E::LEG_SHOW_TAIL));
        assert!(entry.flag(E::HAND_HIDE_ELBOW));
        assert!(!entry.flag(E::HAND_HIDE_FOREARM));
        assert!(entry.flag(E::HAND_SHOW_RING_L));
        assert!(entry.flag(E::HAND_SHOW_BRACELET));
        assert!(entry.flag(E::FOOT_HIDE_KNEE));
        assert!(!entry.flag(E::FOOT_HIDE_CALF));
        assert!(!entry.flag(E::FOOT_HIDE_ANKLE));
        assert!(entry.flag(E::HEAD_SHOW_NECKLACE));
        assert!(entry.flag(E::HEAD_SHOW_EARRINGS));
        assert!(entry.flag(E::HEAD_SHOW_EAR_VIERA));
        assert!(!entry.flag(E::HEAD_SHOW_HROTHGAR_HAT));
        assert!(!entry.flag(E::HEAD_SHOW_VIERA_HAT));
        assert!(!entry.flag(63));
    }

    #[test]
    #[ignore = "requires an installed FFXIV game directory"]
    fn installed_imc_and_eqp_match_documented_layouts() {
        let game_dir =
            std::env::var("XIV_GAME_DIR").unwrap_or_else(|_| r"E:\_ff14\game".to_string());
        let mut resource = physis::resource::SqPackResource::from_existing(&game_dir);
        use physis::resource::Resource;

        // e0908（7.0 ilvl-790 套装）：2 子集 Set 型，子集 2 全槽 material_set=2，
        // mask 0x03FF（top 0x07FF、sho 0x0BFF，高位为 sound id）。
        let imc_path = equipment_imc_path(908, false);
        let bytes = resource.read(&imc_path).expect("read e0908.imc");
        assert_eq!(bytes.len(), 4 + (5 + 2 * 5) * 6);
        let imc = ImcFile::from_bytes(&bytes).expect("parse e0908.imc");
        assert_eq!(imc.kind(), ImcKind::Set);
        assert_eq!(imc.subset_count(), 2);
        for slot in 0..5 {
            assert_eq!(imc.entry(2, slot).material_set, 2, "slot {slot}");
        }
        let masks: Vec<u16> = (0..5).map(|slot| imc.entry(2, slot).mask).collect();
        assert_eq!(masks, [0x03FF, 0x07FF, 0x03FF, 0x03FF, 0x0BFF]);
        for slot in 0..5 {
            assert_eq!(imc.entry(2, slot).attribute_mask(), 0x03FF, "slot {slot}");
        }

        // EQP：set 1（小衣 e0001）= 已知字节 `01 3f 61 73 03 e0 3f 00`。
        let bytes = resource
            .read(EQUIPMENT_PARAMETER_PATH)
            .expect("read equipmentparameter.eqp");
        let table = EquipmentParameterTable::from_bytes(&bytes);
        let entry = table.entry(1).expect("set 1 entry");
        assert_eq!(entry.raw, 0x003F_E003_7361_3F01);
        // e0908 套装条目存在（真实值打印留档）。
        let set908 = table.entry(908).expect("set 908 entry");
        eprintln!("e0908 eqp raw = 0x{:016X}", set908.raw);
    }
}
