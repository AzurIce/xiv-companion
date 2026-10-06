//! 种族体型缩放（RGSP：Racial Gender Scaling Parameters）——human.cmp 尾部
//! 缩放参数表的纯解析与捏脸字节 → 骨缩放系数的换算。
//!
//! 语义来源：
//! - 表布局：xivModdingFramework `CharaMakeParameter.cs`（`General/DataContainers`）
//!   与 `CMP.cs`（`HumanCmpPath = chara/xls/charamake/human.cmp`）。human.cmp =
//!   色板区（u32 RGBA 色项数组，SE 会在补丁间增补色板块）+ 尾部 4480 字节 =
//!   80 × 56 字节 `RacialScalingParameter` 记录；偏移从**文件尾**倒算。条目下标
//!   = `base_race * 10 + subrace`（TT `XivSubRace.GetBaseRace()`/`GetSubRaceId()`：
//!   base race 0-7 = Hyur/Elezen/Lalafell/Miqote/Roegadyn/AuRa/Hrothgar/Viera，
//!   与捏脸 Race byte-1 同序；subrace 0/1 = 部族对内序），每条目含 Male/Female
//!   两组 MinSize/MaxSize/MinTail/MaxTail 与 Female 专属 BustMin/Max XYZ
//!   （14 × f32 LE）。80 条中仅 16 条（base*10+sub）有效，其余为填充。
//! - 捏脸字节（Anamnesis `ActorCustomizeMemory` / Dalamud `CustomizeIndex`）：
//!   0x03 Height（0-100 滑条）、0x15 RaceFeatureSize（尾种族的尾长滑条，
//!   `CmToolLegacyAppearanceFile` 旧名 TailSize）、0x17 BustSize（女性）。
//! - 应用语义（xivmodding.com《Bone list and Bone Scaling notes》与
//!   CustomizePlus 实践：C+ 把 n_root 缩放等同全身身高）：身高 = `n_root`
//!   骨局部均匀缩放；尾长 = 尾链根 `n_sippo_a` 局部均匀缩放；胸围 =
//!   `j_mune_l`/`j_mune_r` 局部非均匀 XYZ 缩放（该文档给出中原女 0%=
//!   (0.92,0.816,0.80)、50%=(1,1,1)、100%=(1.08,1.184,1.20)，与 lerp
//!   (BustMin, BustMax, v/100) 一致）。滑条值 v 线性插值：
//!   `scale = min + (max - min) * v / 100`。

use serde::{Deserialize, Serialize};

use crate::chara_assemble::CharacterCustomize;

/// human.cmp 的 SqPack 路径（xivModdingFramework `CMP.HumanCmpPath`）。
pub const HUMAN_CMP_PATH: &str = "chara/xls/charamake/human.cmp";

/// 单条 `RacialScalingParameter` 的字节数（14 × f32，TT `TotalByteSize`）。
pub const RACIAL_SCALING_ENTRY_LEN: usize = 56;
/// 尾部记录条数：8 base race × 10 槽位（TT `8 * 10 * TotalByteSize`）。
pub const RACIAL_SCALING_ENTRY_COUNT: usize = 80;
/// human.cmp 尾部缩放参数区的总字节数。
pub const RACIAL_SCALING_TABLE_LEN: usize = RACIAL_SCALING_ENTRY_COUNT * RACIAL_SCALING_ENTRY_LEN;

/// 身高缩放作用的骨名（整骨架的根；缩放它 = 全模型关于原点均匀缩放）。
pub const HEIGHT_BONE: &str = "n_root";
/// 胸围缩放作用的骨名（左右胸骨）。
pub const BUST_BONES: [&str; 2] = ["j_mune_l", "j_mune_r"];
/// 尾链骨名前缀（`n_sippo_a`..`n_sippo_e`；缩放作用于链根——层级中第一个
/// 以该前缀命名且父骨不属尾链的骨）。
pub const TAIL_BONE_PREFIX: &str = "n_sippo";

/// human.cmp 尾部单条种族缩放参数（TT `RacialScalingParameter`，14 × f32 LE）。
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RacialScalingParameter {
    pub male_min_size: f32,
    pub male_max_size: f32,
    pub male_min_tail: f32,
    pub male_max_tail: f32,
    pub female_min_size: f32,
    pub female_max_size: f32,
    pub female_min_tail: f32,
    pub female_max_tail: f32,
    pub bust_min: [f32; 3],
    pub bust_max: [f32; 3],
}

impl RacialScalingParameter {
    /// 从恰好 56 字节解析（f32 LE × 14）。
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() != RACIAL_SCALING_ENTRY_LEN {
            return Err(format!(
                "racial scaling entry must be {RACIAL_SCALING_ENTRY_LEN} bytes, got {}",
                bytes.len()
            ));
        }
        let f32_at = |index: usize| -> f32 {
            f32::from_le_bytes([
                bytes[index * 4],
                bytes[index * 4 + 1],
                bytes[index * 4 + 2],
                bytes[index * 4 + 3],
            ])
        };
        Ok(Self {
            male_min_size: f32_at(0),
            male_max_size: f32_at(1),
            male_min_tail: f32_at(2),
            male_max_tail: f32_at(3),
            female_min_size: f32_at(4),
            female_max_size: f32_at(5),
            female_min_tail: f32_at(6),
            female_max_tail: f32_at(7),
            bust_min: [f32_at(8), f32_at(9), f32_at(10)],
            bust_max: [f32_at(11), f32_at(12), f32_at(13)],
        })
    }
}

/// human.cmp 尾部缩放参数表（80 条，仅 16 条有效位，其余为填充）。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RacialScalingTable {
    pub entries: Vec<RacialScalingParameter>,
}

impl RacialScalingTable {
    /// 从完整 human.cmp 字节解析尾部缩放参数表（偏移从文件尾倒算，
    /// 对齐 TT `CharaMakeParameterSet` 的布局假设）。
    pub fn from_cmp_bytes(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() < RACIAL_SCALING_TABLE_LEN {
            return Err(format!(
                "human.cmp must be at least {RACIAL_SCALING_TABLE_LEN} bytes, got {}",
                bytes.len()
            ));
        }
        let base = bytes.len() - RACIAL_SCALING_TABLE_LEN;
        let mut entries = Vec::with_capacity(RACIAL_SCALING_ENTRY_COUNT);
        for index in 0..RACIAL_SCALING_ENTRY_COUNT {
            let at = base + index * RACIAL_SCALING_ENTRY_LEN;
            entries.push(RacialScalingParameter::from_bytes(
                &bytes[at..at + RACIAL_SCALING_ENTRY_LEN],
            )?);
        }
        Ok(Self { entries })
    }

    /// 捏脸 (race, tribe) 对应的表条目下标（TT：`base_race * 10 + subrace`；
    /// base race = 捏脸 Race byte - 1，subrace = 部族对内 0/1 序）。非法
    /// race/tribe 钳制到合法域（与 `race_code_from_parts` 的保守回退一致）。
    pub fn entry_index(race: u8, tribe: u8) -> usize {
        let base_race = usize::from(race.clamp(1, 8) - 1);
        let subrace = usize::from(tribe.max(1) - 1) % 2;
        base_race * 10 + subrace
    }

    /// 查询条目（越界下标返回 None）。
    pub fn entry(&self, race: u8, tribe: u8) -> Option<&RacialScalingParameter> {
        self.entries.get(Self::entry_index(race, tribe))
    }

    /// 由捏脸数据换算最终骨缩放系数。滑条字节线性插值（0-100，越界钳制）；
    /// 男性无胸围（恒 1.0），无尾种族尾长恒 1.0。
    pub fn body_scaling(&self, customize: &CharacterCustomize) -> BodyScaling {
        let Some(entry) = self.entry(customize.race, customize.tribe) else {
            return BodyScaling::IDENTITY;
        };
        let lerp =
            |min: f32, max: f32, value: u8| min + (max - min) * f32::from(value.min(100)) / 100.0;
        let (min_size, max_size, min_tail, max_tail) = if customize.gender == 0 {
            (
                entry.male_min_size,
                entry.male_max_size,
                entry.male_min_tail,
                entry.male_max_tail,
            )
        } else {
            (
                entry.female_min_size,
                entry.female_max_size,
                entry.female_min_tail,
                entry.female_max_tail,
            )
        };
        let height = lerp(min_size, max_size, customize.height);
        let tail = if customize.has_tail() {
            lerp(min_tail, max_tail, customize.ear_muscle_tail_size)
        } else {
            1.0
        };
        let bust = if customize.gender == 1 {
            [
                lerp(entry.bust_min[0], entry.bust_max[0], customize.bust),
                lerp(entry.bust_min[1], entry.bust_max[1], customize.bust),
                lerp(entry.bust_min[2], entry.bust_max[2], customize.bust),
            ]
        } else {
            [1.0; 3]
        };
        BodyScaling { height, tail, bust }
    }
}

/// 最终骨缩放系数（乘在对应骨局部 scale 上；1.0 = 不缩放）。
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BodyScaling {
    /// 身高缩放（作用于 `n_root`，全模型均匀缩放）。
    pub height: f32,
    /// 尾长缩放（作用于尾链根 `n_sippo_a`）。
    pub tail: f32,
    /// 胸围缩放（作用于 `j_mune_l`/`j_mune_r`，非均匀 XYZ）。
    pub bust: [f32; 3],
}

impl BodyScaling {
    /// 全部恒等（无缩放）。
    pub const IDENTITY: Self = Self {
        height: 1.0,
        tail: 1.0,
        bust: [1.0; 3],
    };

    /// 是否恒等（调用方据此跳过合成路径）。滑条中点的 lerp 结果与 1.0 有
    /// f32 噪声级偏差（如 0.96+0.04×0.5 = 1.0+2e-8），按 1e-6 容差判定。
    pub fn is_identity(&self) -> bool {
        (self.height - 1.0).abs() <= 1e-6
            && (self.tail - 1.0).abs() <= 1e-6
            && self.bust.iter().all(|value| (value - 1.0).abs() <= 1e-6)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chara_assemble::{RACE_AU_RA, RACE_LALAFELL};

    fn entry_fixture(seed: u8) -> RacialScalingParameter {
        let v = |k: u8| f32::from(seed) + f32::from(k) / 10.0;
        RacialScalingParameter {
            male_min_size: v(0),
            male_max_size: v(1),
            male_min_tail: v(2),
            male_max_tail: v(3),
            female_min_size: v(4),
            female_max_size: v(5),
            female_min_tail: v(6),
            female_max_tail: v(7),
            bust_min: [v(8), v(9), v(10)],
            bust_max: [v(11), v(12), v(13)],
        }
    }

    #[test]
    fn entry_parses_fourteen_le_f32() {
        let mut bytes = Vec::new();
        for index in 0..14u8 {
            bytes.extend_from_slice(&(f32::from(index) + 0.5).to_le_bytes());
        }
        let entry = RacialScalingParameter::from_bytes(&bytes).expect("parse entry");
        assert_eq!(entry.male_min_size, 0.5);
        assert_eq!(entry.male_max_tail, 3.5);
        assert_eq!(entry.female_min_size, 4.5);
        assert_eq!(entry.bust_min, [8.5, 9.5, 10.5]);
        assert_eq!(entry.bust_max, [11.5, 12.5, 13.5]);
        assert!(RacialScalingParameter::from_bytes(&bytes[..55]).is_err());
    }

    #[test]
    fn table_reads_from_file_tail() {
        // 尾部 80 × 56 字节；前面垫色板块（内容任意）。
        let mut bytes = vec![0xABu8; 1234];
        for index in 0..RACIAL_SCALING_ENTRY_COUNT {
            let entry = entry_fixture(index as u8);
            let fields = [
                entry.male_min_size,
                entry.male_max_size,
                entry.male_min_tail,
                entry.male_max_tail,
                entry.female_min_size,
                entry.female_max_size,
                entry.female_min_tail,
                entry.female_max_tail,
                entry.bust_min[0],
                entry.bust_min[1],
                entry.bust_min[2],
                entry.bust_max[0],
                entry.bust_max[1],
                entry.bust_max[2],
            ];
            for value in fields {
                bytes.extend_from_slice(&value.to_le_bytes());
            }
        }
        let table = RacialScalingTable::from_cmp_bytes(&bytes).expect("parse table");
        assert_eq!(table.entries.len(), RACIAL_SCALING_ENTRY_COUNT);
        assert_eq!(table.entries[0], entry_fixture(0));
        assert_eq!(table.entries[79], entry_fixture(79));
        assert!(
            RacialScalingTable::from_cmp_bytes(&bytes[..RACIAL_SCALING_TABLE_LEN - 1]).is_err()
        );
    }

    #[test]
    fn entry_index_follows_tt_base_race_times_ten_plus_subrace() {
        // 中原男（race 1, tribe 1）→ 0；高地（tribe 2）→ 1。
        assert_eq!(RacialScalingTable::entry_index(1, 1), 0);
        assert_eq!(RacialScalingTable::entry_index(1, 2), 1);
        // 精灵（race 2）→ 10/11；敖龙（race 6, tribe 11/12）→ 51/50+1。
        assert_eq!(RacialScalingTable::entry_index(2, 3), 10);
        assert_eq!(RacialScalingTable::entry_index(2, 4), 11);
        assert_eq!(RacialScalingTable::entry_index(6, 11), 50);
        assert_eq!(RacialScalingTable::entry_index(6, 12), 51);
        // 维埃拉（race 8, tribe 15/16）→ 70/71。
        assert_eq!(RacialScalingTable::entry_index(8, 15), 70);
        assert_eq!(RacialScalingTable::entry_index(8, 16), 71);
        // 非法 race/tribe 钳制不越界。
        assert_eq!(RacialScalingTable::entry_index(0, 0), 0);
        assert!(RacialScalingTable::entry_index(9, 2) < RACIAL_SCALING_ENTRY_COUNT);
    }

    #[test]
    fn body_scaling_lerps_sliders_per_gender() {
        // 构造敖龙（race 6）条目：index 50（Raen）。数值取可手算的整值。
        let mut table = RacialScalingTable {
            entries: vec![RacialScalingParameter::default(); RACIAL_SCALING_ENTRY_COUNT],
        };
        table.entries[50] = RacialScalingParameter {
            male_min_size: 0.9,
            male_max_size: 1.1,
            male_min_tail: 0.8,
            male_max_tail: 1.2,
            female_min_size: 0.85,
            female_max_size: 1.05,
            female_min_tail: 0.7,
            female_max_tail: 1.3,
            bust_min: [0.9, 0.8, 0.7],
            bust_max: [1.1, 1.2, 1.3],
        };
        let customize = CharacterCustomize {
            race: RACE_AU_RA,
            gender: 1,
            tribe: 11,
            height: 50,
            bust: 25,
            ear_muscle_tail_size: 75,
            ..Default::default()
        };
        let scaling = table.body_scaling(&customize);
        assert_eq!(scaling.height, 0.95, "female height mid");
        assert_eq!(scaling.tail, 1.15, "female tail at 75");
        for (actual, expected) in scaling.bust.iter().zip([0.95_f32, 0.9, 0.85]) {
            assert!(
                (actual - expected).abs() < 1e-6,
                "bust at 25: {actual} ≈ {expected}"
            );
        }
        // 男性：用 male 字段，无胸围。
        let scaling = table.body_scaling(&CharacterCustomize {
            gender: 0,
            height: 100,
            ear_muscle_tail_size: 0,
            ..customize
        });
        assert_eq!(scaling.height, 1.1);
        assert_eq!(scaling.tail, 0.8);
        assert_eq!(scaling.bust, [1.0; 3]);
        // 滑条越界钳制到 0-100。
        let scaling = table.body_scaling(&CharacterCustomize {
            gender: 1,
            height: 200,
            ..customize
        });
        assert_eq!(scaling.height, 1.05);
        // 无尾种族（拉拉菲尔）尾长恒 1.0。
        let scaling = table.body_scaling(&CharacterCustomize {
            race: RACE_LALAFELL,
            gender: 0,
            tribe: 9,
            ear_muscle_tail_size: 100,
            ..Default::default()
        });
        assert_eq!(scaling.tail, 1.0);
        // 越界 race/tribe 落不到有效条目时回退恒等。
        let empty = RacialScalingTable::default();
        assert_eq!(empty.body_scaling(&customize), BodyScaling::IDENTITY);
    }

    /// 真实 human.cmp 探针（需 XIV_GAME_DIR）：打印 16 个有效条目的全字段，
    /// 校验表规模与数值合理性（缩放系数均在 0.1..3.0，min ≤ max）；并核对
    /// 真实人体骨架（中原男/敖龙女）含有缩放目标骨（n_root/j_mune/n_sippo）。
    #[cfg(feature = "game-data")]
    #[test]
    #[ignore = "requires an installed FFXIV game directory"]
    fn installed_human_cmp_scaling_table_is_sane() {
        use physis::resource::Resource;

        let game_dir =
            std::env::var("XIV_GAME_DIR").unwrap_or_else(|_| r"E:\_ff14\game".to_string());
        let mut resource = physis::resource::SqPackResource::from_existing(&game_dir);
        let bytes = resource
            .read(HUMAN_CMP_PATH)
            .unwrap_or_else(|| panic!("{HUMAN_CMP_PATH} missing from game data"));
        eprintln!("human.cmp: {} bytes", bytes.len());
        let table = RacialScalingTable::from_cmp_bytes(&bytes).expect("parse human.cmp tail");
        assert_eq!(table.entries.len(), RACIAL_SCALING_ENTRY_COUNT);

        const RACE_NAMES: [&str; 8] = [
            "Hyur", "Elezen", "Lalafell", "Miqote", "Roegadyn", "AuRa", "Hrothgar", "Viera",
        ];
        for race in 1..=8u8 {
            for tribe in [race_tribe_pair(race).0, race_tribe_pair(race).1] {
                let entry = table.entry(race, tribe).expect("entry in range");
                eprintln!(
                    "{} tribe {tribe}: M size [{:.4}, {:.4}] tail [{:.4}, {:.4}] | F size [{:.4}, {:.4}] tail [{:.4}, {:.4}] bust [{:.3?}, {:.3?}]",
                    RACE_NAMES[usize::from(race - 1)],
                    entry.male_min_size,
                    entry.male_max_size,
                    entry.male_min_tail,
                    entry.male_max_tail,
                    entry.female_min_size,
                    entry.female_max_size,
                    entry.female_min_tail,
                    entry.female_max_tail,
                    entry.bust_min,
                    entry.bust_max,
                );
                for value in [
                    entry.male_min_size,
                    entry.male_max_size,
                    entry.male_min_tail,
                    entry.male_max_tail,
                    entry.female_min_size,
                    entry.female_max_size,
                    entry.female_min_tail,
                    entry.female_max_tail,
                ]
                .into_iter()
                .chain(entry.bust_min)
                .chain(entry.bust_max)
                {
                    assert!(
                        (0.1..=3.0).contains(&value),
                        "race {race} tribe {tribe}: implausible scaling value {value}"
                    );
                }
                assert!(entry.male_min_size <= entry.male_max_size);
                assert!(entry.female_min_size <= entry.female_max_size);
            }
        }

        // 缩放目标骨在真实人体骨架中存在：中原男（n_root/j_mune）与敖龙女
        // （+尾链 n_sippo_*，链根父骨非尾骨）。
        for race_code in [101u16, 1401] {
            let path = crate::skeleton::character_skeleton_path(race_code);
            let sklb = resource
                .read(&path)
                .unwrap_or_else(|| panic!("{path} missing from game data"));
            let skeleton = crate::skeleton::load_skeleton_from_sklb_bytes(&sklb)
                .unwrap_or_else(|error| panic!("{path}: {error}"));
            let has = |name: &str| skeleton.bone_index(name).is_some();
            eprintln!(
                "c{race_code:04}: bones={} n_root={} j_mune_l={} j_mune_r={} tail_bones={:?}",
                skeleton.bone_count(),
                has(HEIGHT_BONE),
                has(BUST_BONES[0]),
                has(BUST_BONES[1]),
                skeleton
                    .bone_names
                    .iter()
                    .filter(|name| name.starts_with(TAIL_BONE_PREFIX))
                    .collect::<Vec<_>>(),
            );
            assert!(has(HEIGHT_BONE), "c{race_code:04}: {HEIGHT_BONE} missing");
            assert!(has(BUST_BONES[0]), "c{race_code:04}: j_mune_l missing");
            assert!(has(BUST_BONES[1]), "c{race_code:04}: j_mune_r missing");
        }
        let aura = crate::skeleton::load_skeleton_from_sklb_bytes(
            &resource
                .read(&crate::skeleton::character_skeleton_path(1401))
                .expect("au-ra sklb"),
        )
        .expect("au-ra skeleton");
        assert!(
            aura.bone_names
                .iter()
                .any(|name| name.starts_with(TAIL_BONE_PREFIX)),
            "au-ra skeleton has a tail chain"
        );
    }

    /// 每个 race byte 的两个部族号（race 1: 1/2，其余 race r: 2r-1/2r... 唯
    /// 中原 1/2、高地同 race 1；按 chara_assemble 的部族对表）。
    #[cfg(feature = "game-data")]
    fn race_tribe_pair(race: u8) -> (u8, u8) {
        match race {
            1 => (1, 2),
            2 => (3, 4),
            3 => (9, 10),
            4 => (5, 6),
            5 => (7, 8),
            6 => (11, 12),
            7 => (13, 14),
            _ => (15, 16),
        }
    }
}
