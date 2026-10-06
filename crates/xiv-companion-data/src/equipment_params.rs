//! 装备参数文件（EQP）解析——着装角色装配的变体显隐与身体遮蔽数据源。
//!
//! 语义来源（xivModdingFramework 为权威，均经真实 SqPack 数据核对）：
//! - EQP（`Models/FileTypes/Eqp.cs` + `Models/DataContainers/EquipmentParameter.cs`）：
//!   `chara/xls/equipmentparameter/equipmentparameter.eqp` 按 160 套一块稀疏
//!   存储：文件头 8 字节是块存在位表（64 块 = 10240 套上限；位表占用 set 0
//!   的条目槽，故 set 0 不可读，游戏硬编码其与 set 1 共享），块内每套 8 字节
//!   小端 u64 标志位（bit n = byte[n/8] 的 bit n%8）。标志位语义见
//!   [`EquipmentParameterEntry`] 的位常量（`EquipmentParameterFlag` 枚举）。

/// EQP 表文件路径（全游戏唯一一份）。
pub const EQUIPMENT_PARAMETER_PATH: &str = "chara/xls/equipmentparameter/equipmentparameter.eqp";

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
    /// 耳饰显隐按种族分组，无通用位（Penumbra `EqpEntry`，对游戏运行时行为
    /// 核实；xivModdingFramework `EquipmentParameterFlag` 把 46 标为未用、47
    /// 标为通用，与真实数据矛盾——真实条目存在 46-49 任意组合的部分置位，
    /// 如耳饰仅对猫魅/硌狮/维埃拉组关闭的套装）。
    pub const HEAD_SHOW_EARRINGS_HYUR_ROE: u8 = 46;
    pub const HEAD_SHOW_EARRINGS_LALA_ELEZEN: u8 = 47;
    pub const HEAD_SHOW_EARRINGS_MIQO_HROTH_VIERA: u8 = 48;
    pub const HEAD_SHOW_EARRINGS_AURA: u8 = 49;
    /// 人族耳（中原/精灵/拉拉/鲁加脸部 `atr_mim` 子网格）。
    pub const HEAD_SHOW_EAR_HUMAN: u8 = 50;
    /// 猫魅耳（脸部基础网格内，无 attribute 隔离）。
    pub const HEAD_SHOW_EAR_MIQO: u8 = 51;
    /// 敖龙角（脸部 `atr_hrn` 子网格）。
    pub const HEAD_SHOW_EAR_AURA: u8 = 52;
    /// 维埃拉耳（独立 zear 部件）。
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
    #[cfg(feature = "game-data")]
    use crate::imc::{ImcFile, ImcKind, equipment_imc_path};

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
        // 项链+耳饰（中原/鲁加、精灵/拉拉组）显示；byte6=0x3f → 耳饰（猫魅/硌狮/
        // 维埃拉、敖龙组）+各族耳全显示；byte7=0x00 → 硌狮/维埃拉帽不显示。
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
        assert!(entry.flag(E::HEAD_SHOW_EARRINGS_HYUR_ROE));
        assert!(entry.flag(E::HEAD_SHOW_EARRINGS_LALA_ELEZEN));
        assert!(entry.flag(E::HEAD_SHOW_EARRINGS_MIQO_HROTH_VIERA));
        assert!(entry.flag(E::HEAD_SHOW_EARRINGS_AURA));
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
        let imc = ImcFile::parse(&bytes).expect("parse e0908.imc");
        assert_eq!(imc.kind, Some(ImcKind::Set));
        assert_eq!(imc.subset_count, 2);
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
