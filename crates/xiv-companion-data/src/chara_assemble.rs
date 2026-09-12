//! 玩家角色拼装（chara/human + 小衣装备域）的数据层基础。
//!
//! 语义来源（均已核实，含真实 SqPack 探测验证）：
//! - 捏脸 26 字节布局：Anamnesis `ActorCustomizeMemory`（0x00 Race … 0x19
//!   FacePaintColor，声音不在其中）。Race byte：1 中原、2 精灵、3 拉拉菲尔、
//!   4 猫魅、5 鲁加、6 敖龙、7 硌狮、8 维埃拉；Tribe byte 1-16（中原 1、
//!   高地 2、…、维娜 16）。c 编码与 Race byte 序不同（拉拉 c11/12、猫魅
//!   c07/08），见 [`CharacterCustomize::race_code_from_parts`]。
//! - 部件路径：游戏裸装身体 = 小衣装备 e0001（xivModdingFramework
//!   `Mdl.GetMdlPath` equipment 分支 + 真实边界测量：top/dwn = 皮肤网格 +
//!   内衣网格，sho 仅中原男/女、鲁加男、拉拉男有）；硌狮/维埃拉无自身
//!   e0001 文件，按种族骨变形树回退（[`smallclothes_model_race_candidates`]）。
//!   `chara/human/c{race}/obj/body/b####` 的 b 文件不是玩家裸模（b0002 实测
//!   仅为颈部补丁等小件）。脸 `c{race}f{head+1}_fac.mdl`（高地无脸模，借
//!   中原同性别）；发 `c{race}h{hair}_hir.mdl`；尾 `c{race}t{tail+1}_til.mdl`
//!   （猫魅/敖龙）；兔耳 `c{race}z{ear+1}_zer.mdl`（维埃拉）。文件编号：
//!   脸/尾/兔耳 1 基而字节 0 基（+1），发 1 基直连。
//! - 发型选项：CharaMakeType 的 InitVal 是 0 基选项索引，ID 列表在
//!   CharaMakeCustomize 的 (Hair, tribe, gender) 块序里（character-make
//!   资产的 `hairOptions`）。
//! - 材质路径特例：`Mtrl.GetMtrlPath`/`GetHairMaterialRoot`——皮肤恒 v0001，
//!   肤族表逐族经 e0001 内嵌材质名核对（精灵/猫魅→中原、鲁加女→高地女、
//!   拉拉女→拉拉男，其余自用，见 [`skin_race_for`]）；脸/兔耳无版本目录；
//!   头发按硬编码共享表改写材质根与文件名（硌狮永不共享，h0101-h0115 五个
//!   种族独占，h0116-h0200 全部重定向中原，h0201+ 独占），共享段模型根分布
//!   不规则（候选回退）。
//! - attribute 显隐：submesh attribute 位是各 MDL 本地表序（跨模型不可比），
//!   按名启用（[`character_enabled_attribute_names`]）。
//! - 捏脸菜单/调色板：`character_make` 模块的 CharaMakeType 与 human.cmp 资产。

use serde::{Deserialize, Serialize};

use crate::model::{ModelAttributeOption, WeaponModelData, model_attribute_options};

/// 捏脸数据（ActorCustomize）字节长度。
pub const CHARACTER_CUSTOMIZE_LEN: usize = 26;

/// Race byte 1（中原人，与高地人同属 Race 1，部族区分）。
pub const RACE_HYUR: u8 = 1;
/// Race byte 2（精灵）。
pub const RACE_ELEZEN: u8 = 2;
/// Race byte 3（拉拉菲尔）。
pub const RACE_LALAFELL: u8 = 3;
/// Race byte 4（猫魅，有尾部模型）。
pub const RACE_MIQOTE: u8 = 4;
/// Race byte 5（鲁加）。
pub const RACE_ROEGADYN: u8 = 5;
/// Race byte 6（敖龙，有尾部模型）。
pub const RACE_AU_RA: u8 = 6;
/// Race byte 7（硌狮，头发永不共享）。
pub const RACE_HROTHGAR: u8 = 7;
/// Race byte 8（维埃拉，有兔耳模型）。
pub const RACE_VIERA: u8 = 8;

/// 身体 5 槽的槽位缩写（xivModdingFramework `Mdl.SlotAbbreviationDictionary`）。
pub const HUMAN_BODY_SLOT_ABBREVIATIONS: [&str; 5] = ["met", "top", "glv", "dwn", "sho"];

/// 小衣（皮肤身体）equipment id：游戏内裸装/捏脸界面的身体即小衣装备
/// （真实 SqPack 边界测量：e0001 top/dwn = 皮肤躯干/四肢网格 + 内衣网格）。
pub const SMALLCLOTHES_EQUIPMENT_ID: u16 = 1;

/// 皮肤种族表（`XivRaceTree.GetSkinRace` 可玩族段；真实 e0001 内嵌材质名
/// 逐族核对 + 硌狮/维埃拉自身皮肤材质文件存在性验证）：精灵/猫魅→中原同
/// 性别、鲁加女→高地女、拉拉女→拉拉男，其余（含硌狮/维埃拉）自用。
pub fn skin_race_for(race_code: u16) -> u16 {
    match race_code {
        501 => 101,
        601 => 201,
        701 => 101,
        801 => 201,
        1001 => 401,
        1201 => 1101,
        _ => race_code,
    }
}

/// 小衣模型文件的种族候选（真实 SqPack 普查：仅硌狮 1501/1601 与维埃拉
/// 1701/1801 无 `c{race}e0001` 文件，按种族骨变形树回退——硌狮→鲁加、
/// 维埃拉→猫魅；本实现不应用骨变形，比例为回退种族）。
pub fn smallclothes_model_race_candidates(race_code: u16) -> Vec<u16> {
    match race_code {
        1501 => vec![1501, 901],
        1601 => vec![1601, 1001],
        1701 => vec![1701, 701],
        1801 => vec![1801, 801],
        _ => vec![race_code],
    }
}

/// 裸肤手/足（e0000 glv/sho）的种族候选。真实 SqPack 普查：e0000 全槽仅
/// 中原男/女与拉拉男有文件，其余族按版型回退（拉拉女→拉拉男，其余→中原
/// 同性别；未应用骨变形，手部比例为回退种族）。
pub fn bare_limb_model_race_candidates(race_code: u16) -> Vec<u16> {
    let fit = match race_code {
        1201 => 1101,
        _ if (race_code / 100) % 2 == 0 => 201,
        _ => 101,
    };
    if fit == race_code {
        vec![race_code]
    } else {
        vec![race_code, fit]
    }
}

/// 面妆 decal 贴图目录（真实 SqPack 探测：`_decal_1.tex`..=`_decal_69.tex`
/// 存在，`_decal_0`/`_decal_70`+ 不存在；xivModdingFramework
/// `XivStrings.FacePaintFolder`/`FacePaintFile` 同规律）。
pub const FACE_PAINT_DECAL_FOLDER: &str = "chara/common/texture/decal_face";
/// 面妆贴图号上限（探测所得连续段 1..=69）。
pub const FACE_PAINT_DECAL_MAX_ID: u8 = 69;

/// 面妆字节 → decal 贴图候选路径（降序尝试，首个存在者生效）。
///
/// 字节语义（FFXIVClientStructs `CustomizeData` 0x18）：bit7 = 镜像
/// （FacePaintReversed），bits 0..6 = 面妆号。候选规律为真实数据探测结论：
/// - 面妆号直连贴图号（维埃拉 24..29、61 验证）；
/// - 面妆号落在 100..=127（旧编号段，bit7 未置位）时补 `-100` 候选
///   （如 103 → `_decal_3`）；
/// - bit7 置位的旧编号（128..=195）经 `& 0x7F` 后落入 1..69 直连段
///   （如中原 193 → `_decal_65`），仅 128 需 `字节-100` 候选（→ `_decal_28`）。
///
/// 当前游戏数据（CharaMakeCustomize 全部 32 组面妆号）在上述候选下 100% 覆盖；
/// 无候选命中时调用方按缺失降级（不报错）。`face_paint = 0`（无面妆）返回空。
pub fn face_paint_decal_texture_candidates(face_paint: u8) -> Vec<String> {
    let mut candidates = Vec::new();
    let paint = face_paint & 0x7F;
    if paint == 0 && face_paint == 0 {
        return candidates;
    }
    let push = |id: u8, candidates: &mut Vec<String>| {
        if (1..=FACE_PAINT_DECAL_MAX_ID).contains(&id)
            && !candidates
                .iter()
                .any(|existing| existing.ends_with(&format!("_{id}.tex")))
        {
            candidates.push(format!("{FACE_PAINT_DECAL_FOLDER}/_decal_{id}.tex"));
        }
    };
    push(paint, &mut candidates);
    if (100..=127).contains(&paint) {
        push(paint - 100, &mut candidates);
    }
    if face_paint >= 100 {
        push(face_paint - 100, &mut candidates);
    }
    candidates
}

/// 玩家角色捏脸数据：游戏 ActorCustomize 缓冲的 26 个字节。
///
/// 布局（Anamnesis `ActorCustomizeMemory` 权威）：0x00 Race（1-8）、0x01
/// Gender（0 男 / 1 女）、0x02 Age、0x03 Height、0x04 Tribe（1-16）、0x05
/// Head（脸号）、0x06 Hair、0x07 HighlightType（bit7=开挑染）、0x08 Skintone、
/// 0x09 REyeColor、0x0A HairTone、0x0B Highlights、0x0C FacialFeatures
/// （bitmask）、0x0D FacialFeatureColor、0x0E Eyebrows、0x0F LEyeColor、
/// 0x10 Eyes（bit7=小虹膜）、0x11 Nose、0x12 Jaw、0x13 Mouth（bit7=唇色开关）、
/// 0x14 LipsTone、0x15 EarMuscleTailSize、0x16 TailEarsType、0x17 Bust、
/// 0x18 FacePaint、0x19 FacePaintColor。声音不在其中。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterCustomize {
    pub race: u8,
    pub gender: u8,
    pub age: u8,
    pub height: u8,
    pub tribe: u8,
    pub head: u8,
    pub hair: u8,
    pub highlight_type: u8,
    pub skintone: u8,
    pub r_eye_color: u8,
    pub hair_tone: u8,
    pub highlights: u8,
    pub facial_features: u8,
    pub facial_feature_color: u8,
    pub eyebrows: u8,
    pub l_eye_color: u8,
    pub eyes: u8,
    pub nose: u8,
    pub jaw: u8,
    pub mouth: u8,
    pub lips_tone: u8,
    pub ear_muscle_tail_size: u8,
    pub tail_ears_type: u8,
    pub bust: u8,
    pub face_paint: u8,
    pub face_paint_color: u8,
}

impl CharacterCustomize {
    /// 26 字节序列化，布局与游戏 ActorCustomize 缓冲一致。
    pub fn to_bytes(&self) -> [u8; CHARACTER_CUSTOMIZE_LEN] {
        [
            self.race,
            self.gender,
            self.age,
            self.height,
            self.tribe,
            self.head,
            self.hair,
            self.highlight_type,
            self.skintone,
            self.r_eye_color,
            self.hair_tone,
            self.highlights,
            self.facial_features,
            self.facial_feature_color,
            self.eyebrows,
            self.l_eye_color,
            self.eyes,
            self.nose,
            self.jaw,
            self.mouth,
            self.lips_tone,
            self.ear_muscle_tail_size,
            self.tail_ears_type,
            self.bust,
            self.face_paint,
            self.face_paint_color,
        ]
    }

    /// 从恰好 26 字节的缓冲解析（与 [`CharacterCustomize::to_bytes`] 往返）。
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() != CHARACTER_CUSTOMIZE_LEN {
            return Err(format!(
                "customize buffer must be exactly {CHARACTER_CUSTOMIZE_LEN} bytes, got {}",
                bytes.len()
            ));
        }
        Ok(Self {
            race: bytes[0],
            gender: bytes[1],
            age: bytes[2],
            height: bytes[3],
            tribe: bytes[4],
            head: bytes[5],
            hair: bytes[6],
            highlight_type: bytes[7],
            skintone: bytes[8],
            r_eye_color: bytes[9],
            hair_tone: bytes[10],
            highlights: bytes[11],
            facial_features: bytes[12],
            facial_feature_color: bytes[13],
            eyebrows: bytes[14],
            l_eye_color: bytes[15],
            eyes: bytes[16],
            nose: bytes[17],
            jaw: bytes[18],
            mouth: bytes[19],
            lips_tone: bytes[20],
            ear_muscle_tail_size: bytes[21],
            tail_ears_type: bytes[22],
            bust: bytes[23],
            face_paint: bytes[24],
            face_paint_color: bytes[25],
        })
    }

    /// 校验关键字段：Race 1-8、Gender 0/1、Tribe 1-16。返回所有问题的描述。
    pub fn validate(&self) -> Result<(), String> {
        let mut problems = Vec::new();
        if !(RACE_HYUR..=RACE_VIERA).contains(&self.race) {
            problems.push(format!("race {} out of range 1-8", self.race));
        }
        if self.gender > 1 {
            problems.push(format!("gender {} out of range 0-1", self.gender));
        }
        if !(1..=16).contains(&self.tribe) {
            problems.push(format!("tribe {} out of range 1-16", self.tribe));
        }
        if problems.is_empty() {
            Ok(())
        } else {
            Err(problems.join("; "))
        }
    }

    /// 角色种族编码（c 编码）：`(pair * 2 - 1 + gender) * 100 + 1`。pair 按
    /// （部族对）序：中原 01/02、高地 03/04、精灵 05/06、猫魅 07/08、
    /// 鲁加 09/10、拉拉 11/12、敖龙 13/14、硌狮 15/16、维埃拉 17/18。
    /// 注意 c 编码序 ≠ Race byte 序（拉拉 Race 3 在 c 编码第 6 对、猫魅
    /// Race 4 在第 4 对），由 `race_code_from_parts` 的表驱动映射。
    pub fn race_code(&self) -> u16 {
        Self::race_code_from_parts(self.race, self.tribe, self.gender)
    }

    /// 由 race/tribe/gender 计算 c 编码（xivModdingFramework `XivRace` +
    /// 真实 SqPack 验证）。pair 序：中原(T1) 01/02、高地(T2) 03/04、精灵
    /// 05/06、猫魅 07/08、鲁加 09/10、拉拉 11/12、敖龙 13/14、硌狮 15/16、
    /// 维埃拉 17/18；除中原/高地外同 Race 两部落共享一对编码。
    pub fn race_code_from_parts(race: u8, tribe: u8, gender: u8) -> u16 {
        let pair = match race {
            RACE_HYUR => u16::from(tribe.clamp(1, 2)),
            RACE_ELEZEN => 3,
            RACE_MIQOTE => 4,
            RACE_ROEGADYN => 5,
            RACE_LALAFELL => 6,
            RACE_AU_RA => 7,
            RACE_HROTHGAR => 8,
            RACE_VIERA => 9,
            _ => 1,
        };
        (pair * 2 - 1 + u16::from(gender.min(1))) * 100 + 1
    }

    /// 该角色是否有尾部模型（猫魅/敖龙）。
    pub fn has_tail(&self) -> bool {
        matches!(self.race, RACE_MIQOTE | RACE_AU_RA)
    }

    /// 该角色是否有兔耳模型（维埃拉）。
    pub fn has_zear(&self) -> bool {
        self.race == RACE_VIERA
    }

    /// 调色板 (tribe, gender) 区索引：`(tribe - 1) * 2 + gender`（human.cmp 布局）。
    pub fn palette_group_index(&self) -> usize {
        (usize::from(self.tribe.max(1)) - 1) * 2 + usize::from(self.gender.min(1))
    }
}

/// 角色拼装部件类别。身体为小衣（e0001 top/dwn/sho）与裸肤（e0000 glv/sho）
/// 装备 MDL，特殊部位（脸/发/尾/兔耳）各 1 个。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CharacterPartKind {
    BodyMet,
    BodyTop,
    BodyGlv,
    BodyDwn,
    BodySho,
    Face,
    Hair,
    Tail,
    Zear,
}

impl CharacterPartKind {
    pub fn id(self) -> &'static str {
        match self {
            Self::BodyMet => "body-met",
            Self::BodyTop => "body-top",
            Self::BodyGlv => "body-glv",
            Self::BodyDwn => "body-dwn",
            Self::BodySho => "body-sho",
            Self::Face => "face",
            Self::Hair => "hair",
            Self::Tail => "tail",
            Self::Zear => "zear",
        }
    }

    /// 身体槽位部件。
    pub fn is_body_slot(self) -> bool {
        matches!(
            self,
            Self::BodyMet | Self::BodyTop | Self::BodyGlv | Self::BodyDwn | Self::BodySho
        )
    }
}

/// 单个部件的模型加载请求：`model_path` 是首选 MDL 路径（material 候选推导
/// 依赖 `/model/` 切分，见 [`character_material_candidate_paths`]）；
/// `alternate_model_paths` 是同部件的回退候选（共享发型的模型根不规则，
/// 真实数据里 h0116-h0200 部分只存中原男根，见模块文档）。
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterPartRequest {
    pub kind: CharacterPartKind,
    pub model_path: String,
    /// 备选模型路径（首选缺失时按序尝试）。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub alternate_model_paths: Vec<String>,
    /// 必需部件（小衣 top/dwn、裸肤 glv、脸、发）加载失败即整体失败；小衣
    /// sho/裸肤 sho（族别可选）、尾部/兔耳（种族可选）文件缺失时静默跳过。
    pub required: bool,
}

/// 角色拼装部件路径。游戏裸装身体 = 小衣装备 e0001（top/dwn = 皮肤网格 +
/// 内衣网格；sho 仅中原男/女、鲁加男、拉拉男有）+ 裸肤 e0000（glv 手部
/// 恒有，真实普查 e0000 全槽仅中原男/女/拉拉男有文件，其余族按
/// [`bare_limb_model_race_candidates`] 版型回退；无 e0001 sho 的族以 e0000
/// sho 补赤脚）。硌狮/维埃拉无自身 e0001 文件，经
/// [`smallclothes_model_race_candidates`] 回退（骨变形未应用）。皮肤材质恒
/// `mt_c{肤族}b0001_a`（内嵌名逐族核对，见 [`skin_race_for`]；跨族回退时
/// 游戏仍用角色自身肤族材质）。脸/尾/兔耳的文件编号规则：文件 1 基（硌狮
/// 女脸从 f0005 起，见 [`human_face_file_base`]）而捏脸字节 0 基
/// （Anamnesis 校验范围 + f0000/t0000/z0000 均不存在 + FFXIVClientStructs
/// `Human.FaceId`/`TailEarId` 存解析后文件号）；头发编号 1 基且字节直连
/// （h0000 不存在，CharaMakeCustomize FeatureID 即字节值）。材质共享（头发）
/// 不影响首选模型路径——共享只作用于材质根与备选模型根。
pub fn character_part_paths(customize: &CharacterCustomize) -> Vec<CharacterPartRequest> {
    let race_code = customize.race_code();
    let race = format!("c{race_code:04}");
    let mut parts = Vec::new();
    // 皮肤身体（小衣 e0001）：候选按种族回退表合并去重。
    let smallclothes_races = smallclothes_model_race_candidates(race_code);
    let equipment_path = |equipment: u16, race_code: u16, slot: &str| {
        format!(
            "chara/equipment/e{equipment:04}/model/c{race_code:04}e{equipment:04}_{slot}.mdl"
        )
    };
    for (kind, slot, required) in [
        (CharacterPartKind::BodyTop, "top", true),
        (CharacterPartKind::BodyDwn, "dwn", true),
        (CharacterPartKind::BodySho, "sho", false),
    ] {
        let model_path = equipment_path(SMALLCLOTHES_EQUIPMENT_ID, race_code, slot);
        let alternate_model_paths = smallclothes_races
            .iter()
            .skip(1)
            .map(|fallback| equipment_path(SMALLCLOTHES_EQUIPMENT_ID, *fallback, slot))
            .collect();
        parts.push(CharacterPartRequest {
            kind,
            model_path,
            alternate_model_paths,
            required,
        });
    }
    // 裸肤手（e0000 glv，恒有）与裸肤足（e0000 sho，无 e0001 sho 的族赤脚）。
    let bare_limb_races = bare_limb_model_race_candidates(race_code);
    parts.push(CharacterPartRequest {
        kind: CharacterPartKind::BodyGlv,
        model_path: equipment_path(0, race_code, "glv"),
        alternate_model_paths: bare_limb_races
            .iter()
            .skip(1)
            .map(|fallback| equipment_path(0, *fallback, "glv"))
            .collect(),
        required: true,
    });
    // e0001 sho 的存在集合（真实普查：中原男/女、鲁加男、拉拉男）——其余族
    // 赤脚，用 e0000 sho 补双足；有 e0001 sho 时不再叠加裸足（避免重叠）。
    let has_smallclothes_shoes = smallclothes_races
        .iter()
        .any(|candidate| matches!(candidate, 101 | 201 | 901 | 1101));
    if !has_smallclothes_shoes {
        parts.push(CharacterPartRequest {
            kind: CharacterPartKind::BodySho,
            model_path: equipment_path(0, race_code, "sho"),
            alternate_model_paths: bare_limb_races
                .iter()
                .skip(1)
                .map(|fallback| equipment_path(0, *fallback, "sho"))
                .collect(),
            required: false,
        });
    }
    // 脸字节 0 基索引 → 文件号：所有种族从 f0001 起，唯硌狮女（c1601）
    // 脸文件仅 f0005-f0008（4 个），索引从 5 起（真实 SqPack 验证）。
    let head = u16::from(customize.head) + human_face_file_base(race_code);
    // 高地人（Tribe 2）无自身脸模，游戏使用中原同性别脸（真实 SqPack 验证
    // c0301/c0401 均无 f#### 文件）。
    let face_alternates = (customize.race == RACE_HYUR && customize.tribe == 2)
        .then(|| {
            let midlander = if customize.gender == 1 { 201 } else { 101 };
            vec![format!(
                "chara/human/c{midlander:04}/obj/face/f{head:04}/model/c{midlander:04}f{head:04}_fac.mdl"
            )]
        })
        .unwrap_or_default();
    parts.push(CharacterPartRequest {
        kind: CharacterPartKind::Face,
        model_path: format!(
            "chara/human/{race}/obj/face/f{head:04}/model/{race}f{head:04}_fac.mdl"
        ),
        alternate_model_paths: face_alternates,
        required: true,
    });
    let hair = customize.hair;
    let mut hair_part = CharacterPartRequest {
        kind: CharacterPartKind::Hair,
        model_path: format!(
            "chara/human/{race}/obj/hair/h{hair:04}/model/{race}h{hair:04}_hir.mdl"
        ),
        alternate_model_paths: Vec::new(),
        required: true,
    };
    hair_part.alternate_model_paths = character_hair_model_alternates(
        customize.race_code(),
        u16::from(hair),
        &hair_part.model_path,
    );
    parts.push(hair_part);
    if customize.has_tail() {
        let tail = u16::from(customize.tail_ears_type) + 1;
        parts.push(CharacterPartRequest {
            kind: CharacterPartKind::Tail,
            model_path: format!(
                "chara/human/{race}/obj/tail/t{tail:04}/model/{race}t{tail:04}_til.mdl"
            ),
            alternate_model_paths: Vec::new(),
            required: false,
        });
    }
    if customize.has_zear() {
        let ear = u16::from(customize.tail_ears_type) + 1;
        parts.push(CharacterPartRequest {
            kind: CharacterPartKind::Zear,
            model_path: format!(
                "chara/human/{race}/obj/zear/z{ear:04}/model/{race}z{ear:04}_zer.mdl"
            ),
            alternate_model_paths: Vec::new(),
            required: false,
        });
    }
    parts
}

/// 脸文件起始号：所有种族 f0001 起；硌狮女（c1601）脸文件仅 f0005-f0008
/// （4 个），捏脸字节 0 基索引从 5 起（真实 SqPack 验证）。
pub fn human_face_file_base(race_code: u16) -> u16 {
    match race_code {
        1601 => 5,
        _ => 1,
    }
}

/// 头发共享根（xivModdingFramework `Mtrl.GetHairMaterialRoot`）：
/// 硌狮（1501/1601）永不共享；h0001-h0100 与 h0201+ 种族独占；h0101-h0115
/// 由中原男/女、猫魅男/女、硌狮男/女共享（其余种族重定向中原对应性别）；
/// h0116-h0200 全部重定向中原男/女。
pub fn shared_hair_race_code(race_code: u16, hair_id: u16) -> u16 {
    const HAIR_SHARED_RANGE_OWNERS: [u16; 6] = [101, 201, 701, 801, 1501, 1601];
    if (1501..=1601).contains(&race_code) {
        return race_code;
    }
    let midlander = if (race_code / 100) % 2 == 0 { 201 } else { 101 };
    if hair_id < 101 {
        race_code
    } else if hair_id < 116 {
        if HAIR_SHARED_RANGE_OWNERS.contains(&race_code) {
            race_code
        } else {
            midlander
        }
    } else if hair_id < 201 {
        midlander
    } else {
        race_code
    }
}

/// 共享发型（h0101-h0200）的备选模型路径。真实 SqPack 里模型根的分布
/// 不规则：h0101-h0115 各共享族有自身（性别对应）副本，h0116-h0200 部分
/// 只存中原男根（如 h0116 仅 c0101，h0150 各共享族都有）。候选顺序：
/// 共享表根 → 中原男/女根，与首选（自身根）合并去重后交给加载方按序尝试。
pub fn character_hair_model_alternates(
    race_code: u16,
    hair_id: u16,
    primary_path: &str,
) -> Vec<String> {
    if !(101..=200).contains(&hair_id) {
        return Vec::new();
    }
    let path = |race: u16| {
        format!(
            "chara/human/c{race:04}/obj/hair/h{hair_id:04}/model/c{race:04}h{hair_id:04}_hir.mdl"
        )
    };
    let mut alternates = Vec::new();
    for race in [shared_hair_race_code(race_code, hair_id), 101, 201] {
        let candidate = path(race);
        if candidate != primary_path && !alternates.contains(&candidate) {
            alternates.push(candidate);
        }
    }
    alternates
}

/// 角色部件的材质候选路径。复用武器/装备的推导骨架（MDL 内嵌材质名优先
/// 原样尝试，再按 `/model/` 切分推导材质根），叠加 human 域特例：
/// - 身体（小衣 e0001）：皮肤材质 `mt_c{肤族}b0001_...` 自身肤族根优先
///   （跨族回退时游戏仍用角色自身肤族，见 [`skin_race_for`]），其次内嵌
///   文件名反推根；小衣材质走装备根 `chara/equipment/e0001/material/v0001`。
/// - 脸：无版本目录。
/// - 头发：按共享表改写材质根与文件名（SE 内部硬编码），再补自身根候选。
/// - 尾：`v0001` + 无版本兜底。
/// - 兔耳：与脸一致，无版本目录。
pub fn character_material_candidate_paths(
    customize: &CharacterCustomize,
    part: CharacterPartKind,
    model_path: &str,
    material_name: &str,
) -> Vec<String> {
    let mut candidates = Vec::new();
    let normalized_name = normalize_character_path(material_name);
    if normalized_name.is_empty() {
        return candidates;
    }
    push_unique(&mut candidates, normalized_name.clone());
    if normalized_name.starts_with("chara/") {
        return candidates;
    }

    let normalized_model_path = normalize_character_path(model_path);
    let Some((object_root, _)) = normalized_model_path.split_once("/model/") else {
        return candidates;
    };
    let material_file = normalized_name
        .rsplit('/')
        .next()
        .unwrap_or(normalized_name.as_str())
        .to_string();

    match part {
        CharacterPartKind::Face | CharacterPartKind::Zear => {
            // 脸/兔耳无版本目录：材质直接放在 obj 根下的 material/。
            push_unique(
                &mut candidates,
                format!("{object_root}/material/{material_file}"),
            );
        }
        CharacterPartKind::Hair => {
            let hair_id = u16::from(customize.hair);
            // 共享表按实际加载的模型路径种族解析（游戏内模型根可能已重定向，
            // 内嵌材质名随之指向该根，见 `Mdl.GetMdlPath` + `GetHairMaterialRoot`
            // 组合语义）；模型路径缺省时退回角色自身种族。
            let model_race = human_race_code_from_path(&normalized_model_path)
                .unwrap_or_else(|| customize.race_code());
            let shared_race = shared_hair_race_code(model_race, hair_id);
            let shared_file = rewrite_material_race(&material_file, shared_race);
            let shared_root = hair_material_root(shared_race, hair_id);
            // SE 硬编码共享：共享根 + 改写文件名优先。
            push_unique(
                &mut candidates,
                format!("{shared_root}/v0001/{shared_file}"),
            );
            push_unique(&mut candidates, format!("{shared_root}/{shared_file}"));
            // 角色自身根与模型根 + 原文件名（独占发型、共享族自身副本）。
            for root in [
                hair_material_root(customize.race_code(), hair_id),
                hair_material_root(model_race, hair_id),
            ] {
                push_unique(&mut candidates, format!("{root}/v0001/{material_file}"));
                push_unique(&mut candidates, format!("{root}/{material_file}"));
            }
        }
        CharacterPartKind::Tail => {
            push_unique(
                &mut candidates,
                format!("{object_root}/material/v0001/{material_file}"),
            );
            push_unique(
                &mut candidates,
                format!("{object_root}/material/{material_file}"),
            );
            if let Some(root) = human_material_root_from_file(&material_file) {
                push_unique(&mut candidates, format!("{root}/v0001/{material_file}"));
                push_unique(&mut candidates, format!("{root}/{material_file}"));
            }
        }
        body_slot => {
            // 小衣（e0001）两类材质：
            // - 皮肤（mt_c{肤族}b0001_...）：自身肤族根优先（跨族回退时游戏
            //   仍用角色自身肤族材质，见 [`skin_race_for`]），其次内嵌文件名
            //   反推根（自有文件族两者一致，天然消化精灵/猫魅→中原等共享）。
            // - 小衣（mt_c{race}e0001_...）：装备根 chara/equipment/e0001。
            let _ = body_slot;
            let skin_race = skin_race_for(customize.race_code());
            if human_material_root_from_file(&material_file).is_some() {
                let skin_file = rewrite_material_race(&material_file, skin_race);
                let skin_root =
                    format!("chara/human/c{skin_race:04}/obj/body/b0001/material");
                push_unique(&mut candidates, format!("{skin_root}/v0001/{skin_file}"));
                push_unique(&mut candidates, format!("{skin_root}/{skin_file}"));
            }
            if let Some(root) = human_material_root_from_file(&material_file) {
                push_unique(&mut candidates, format!("{root}/v0001/{material_file}"));
                push_unique(&mut candidates, format!("{root}/{material_file}"));
            }
            push_unique(
                &mut candidates,
                format!("{object_root}/material/v0001/{material_file}"),
            );
            push_unique(
                &mut candidates,
                format!("{object_root}/material/{material_file}"),
            );
        }
    }
    candidates
}

/// `chara/human/c{race}/obj/{body|face|hair|tail|zear}/.../material` 根：
/// 从材质文件名 `mt_c{race}(b|f|h|t|z){id}_...` 反推。
fn human_material_root_from_file(material_file: &str) -> Option<String> {
    let tail = material_file.strip_prefix("mt_c")?;
    let (race, tail) = tail.split_at_checked(4)?;
    if !race.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let (kind, rest) = tail.split_at_checked(1)?;
    let (id, _) = rest.split_at_checked(4)?;
    if !id.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let obj = match kind {
        "b" => format!("body/b{id}"),
        "f" => format!("face/f{id}"),
        "h" => format!("hair/h{id}"),
        "t" => format!("tail/t{id}"),
        "z" => format!("zear/z{id}"),
        _ => return None,
    };
    Some(format!("chara/human/c{race}/obj/{obj}/material"))
}

fn hair_material_root(race_code: u16, hair_id: u16) -> String {
    format!("chara/human/c{race_code:04}/obj/hair/h{hair_id:04}/material")
}

/// 从 human 模型/材质路径解析 race code（`chara/human/c(\d{4})/`）。
fn human_race_code_from_path(path: &str) -> Option<u16> {
    let tail = path.strip_prefix("chara/human/c")?;
    let (race, _) = tail.split_at_checked(4)?;
    race.parse().ok()
}

/// 从角色相关模型路径解析 race code：`chara/human/c{race}/...`（human 域）
/// 或 `chara/equipment|accessory/{e|a}{id}/model/c{race}...mdl`（装备域）。
/// 即路径中第一个 `/c` 后恰跟 4 位数字的段。
pub fn race_code_from_character_model_path(path: &str) -> Option<u16> {
    let bytes = path.as_bytes();
    for (index, window) in bytes.windows(6).enumerate() {
        if window[0] != b'/' || window[1] != b'c' {
            continue;
        }
        let digits = &window[2..6];
        if !digits.iter().all(u8::is_ascii_digit) {
            continue;
        }
        if bytes.get(index + 6).is_some_and(u8::is_ascii_digit) {
            continue;
        }
        return std::str::from_utf8(digits).ok()?.parse().ok();
    }
    None
}

/// 改写材质文件名的种族段：`mt_c{old}h0116_...` → `mt_c{new}h0116_...`。
fn rewrite_material_race(material_file: &str, race_code: u16) -> String {
    let Some(tail) = material_file.strip_prefix("mt_c") else {
        return material_file.to_string();
    };
    let Some((race, rest)) = tail.split_at_checked(4) else {
        return material_file.to_string();
    };
    if !race.bytes().all(|byte| byte.is_ascii_digit()) {
        return material_file.to_string();
    }
    format!("mt_c{race_code:04}{rest}")
}

fn normalize_character_path(path: &str) -> String {
    let normalized = path.replace('\\', "/");
    let mut parts = Vec::new();
    for part in normalized.trim_start_matches('/').split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            value => parts.push(value),
        }
    }
    parts.join("/").to_ascii_lowercase()
}

fn push_unique(paths: &mut Vec<String>, path: String) {
    if !path.is_empty() && !paths.iter().any(|existing| existing == &path) {
        paths.push(path);
    }
}

/// 角色拼装的 attribute 名启用集合（裸装默认，按名判定）。
///
/// attribute 位是各 MDL **本地 attribute 表序**，跨 MDL 数值不可比（真实
/// SqPack 全 32 组名表核对：c0101 脸 bit0=atr_mim、c1401 脸 bit0=atr_kao、
/// c0801 身 bit0=atr_cn_neck），合并装配后全局数值掩码无意义，只能按名启用。
/// 规则（对齐游戏裸装行为）：
/// - 装配中出现的其余名全部启用：装备遮罩类（atr_nek/atr_hij/atr_ude/
///   atr_sne/atr_hiz/atr_cn_neck 等，游戏仅在装备覆盖时关闭）、脸部基件
///   （atr_kao/atr_mim/atr_hrn）、发件（atr_kam/atr_bak/atr_sta/atr_top/
///   **atr_lod**——名字有误导性，敖龙 h0001 刘海几何就在 atr_lod 子网格里，
///   游戏内可见）；
/// - `atr_fv_*` 脸部特征件按捏脸 FacialFeatures 字节（0x0C bitmask）逐位
///   开关：bit k ↔ atr_fv_{a+k}（bit0→fv_a … bit7→fv_h）。
pub fn character_enabled_attribute_names(
    customize: &CharacterCustomize,
    assembly: &WeaponModelData,
) -> Vec<String> {
    let mut names = Vec::new();
    for mesh in &assembly.meshes {
        let Some(submesh) = &mesh.submesh else {
            continue;
        };
        // `attribute_names` 与掩码置位按位升序 zip 对应。
        let mut submesh_names = submesh.attribute_names.iter();
        for bit in 0..u32::BITS {
            if submesh.attribute_index_mask & (1 << bit) == 0 {
                continue;
            }
            let Some(name) = submesh_names.next() else {
                break;
            };
            if let Some(suffix) = name.strip_prefix("atr_fv_") {
                let mut letters = suffix.chars();
                let Some(letter) = letters.next() else {
                    continue;
                };
                if letters.next().is_some() || !letter.is_ascii_lowercase() {
                    continue;
                }
                let feature_bit = u32::from(letter as u8 - b'a');
                if u32::from(customize.facial_features) & (1 << feature_bit) == 0 {
                    continue;
                }
            }
            if !names.iter().any(|existing| existing == name) {
                names.push(name.clone());
            }
        }
    }
    names
}

/// 角色拼装的 attribute 变体选项（眉/眼/鼻/嘴/轮廓等脸部 attribute submesh）：
/// 直接聚合加载结果的 submesh attribute 位。游戏默认全部关闭（attribute
/// mask = 0，无 attribute 的 submesh 不受影响），角色显隐按名判定见
/// [`character_enabled_attribute_names`]。
pub fn character_assembly_attribute_options(
    assembly: &WeaponModelData,
) -> Vec<ModelAttributeOption> {
    model_attribute_options(assembly)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hyur_male() -> CharacterCustomize {
        CharacterCustomize {
            race: 1,
            gender: 0,
            age: 1,
            height: 50,
            tribe: 1,
            head: 4,
            hair: 51,
            ..Default::default()
        }
    }

    #[test]
    fn face_paint_decal_candidates_follow_probe_conclusions() {
        // 无面妆：空候选。
        assert!(face_paint_decal_texture_candidates(0).is_empty());
        // 直连段（维埃拉默认 26、猫魅 61 的真实数据验证）。
        assert_eq!(
            face_paint_decal_texture_candidates(26),
            ["chara/common/texture/decal_face/_decal_26.tex"]
        );
        assert_eq!(
            face_paint_decal_texture_candidates(61),
            ["chara/common/texture/decal_face/_decal_61.tex"]
        );
        // 旧编号段（bit7 未置位，100..=127）：直连号超出贴图段，只留 -100 回退。
        assert_eq!(
            face_paint_decal_texture_candidates(103),
            ["chara/common/texture/decal_face/_decal_3.tex"]
        );
        // bit7 置位：&0x7F 直连（中原 193/194/195 → 65/66/67）。
        assert_eq!(
            face_paint_decal_texture_candidates(193),
            ["chara/common/texture/decal_face/_decal_65.tex"]
        );
        // 128 单独走 字节-100（&0x7F 为 0）。
        assert_eq!(
            face_paint_decal_texture_candidates(128),
            ["chara/common/texture/decal_face/_decal_28.tex"]
        );
        // 超出已知贴图段且无回退：空（调用方降级）。
        assert!(face_paint_decal_texture_candidates(98).is_empty());
    }

    #[test]
    fn enabled_attribute_names_follow_facial_features_with_lod_on() {
        use crate::model::{ModelMesh, ModelSubmeshInfo, WeaponModelData};
        use crate::PackedModelId;

        fn mesh(mask: u32, names: &[&str]) -> ModelMesh {
            ModelMesh {
                path: "test.mdl".to_string(),
                part_index: 0,
                mesh_category: None,
                submesh: Some(ModelSubmeshInfo {
                    index: 0,
                    table_index: 0,
                    attribute_index_mask: mask,
                    attribute_index_mask_hex: format!("0x{mask:08X}"),
                    attribute_names: names.iter().map(|name| name.to_string()).collect(),
                    bone_start_index: 0,
                    bone_count: 0,
                }),
                shape_influences: Vec::new(),
                shape_targets: Vec::new(),
                material_index: 0,
                material_slot: 0,
                material_name: String::new(),
                color: [0.0; 3],
                bone_table: None,
                vertices: Vec::new(),
                indices: Vec::new(),
            }
        }
        let assembly = WeaponModelData {
            item_id: 0,
            item_name: "test".to_string(),
            model_main: PackedModelId::from_raw(101),
            model_sub: None,
            stain_ids: [0, 0],
            load_diagnostics: Vec::new(),
            loaded_paths: Vec::new(),
            bounds: Default::default(),
            materials: Vec::new(),
            textures: Vec::new(),
            meshes: vec![
                // 敖龙脸本地表序（zip：置位升序对应名）：kao 恒开、fv_c 按特征位。
                mesh(0x1, &["atr_kao"]),
                mesh(0x3, &["atr_kao", "atr_fv_c"]),
                mesh(0x21, &["atr_kao", "atr_fv_d"]),
                mesh(0x4, &["atr_lod"]),
                mesh(0x10, &["atr_hrn"]),
            ],
        };
        // 敖龙女默认特征位 0b1100（bit2/bit3 → fv_c/fv_d）。
        let customize = CharacterCustomize {
            facial_features: 0b1100,
            ..Default::default()
        };
        let names = character_enabled_attribute_names(&customize, &assembly);
        assert_eq!(names, ["atr_kao", "atr_fv_c", "atr_fv_d", "atr_lod", "atr_hrn"]);
        // 特征位清空：fv 件全关，其余（含 atr_lod）不变。
        let names = character_enabled_attribute_names(
            &CharacterCustomize::default(),
            &assembly,
        );
        assert_eq!(names, ["atr_kao", "atr_lod", "atr_hrn"]);
    }

    #[test]
    fn customize_bytes_round_trip() {
        let customize = CharacterCustomize {
            race: 8,
            gender: 1,
            age: 1,
            height: 63,
            tribe: 16,
            head: 3,
            hair: 116,
            highlight_type: 128,
            skintone: 12,
            r_eye_color: 30,
            hair_tone: 103,
            highlights: 7,
            facial_features: 0b101,
            facial_feature_color: 2,
            eyebrows: 1,
            l_eye_color: 9,
            eyes: 0x83,
            nose: 5,
            jaw: 2,
            mouth: 0x80,
            lips_tone: 35,
            ear_muscle_tail_size: 25,
            tail_ears_type: 3,
            bust: 80,
            face_paint: 5,
            face_paint_color: 12,
        };
        let bytes = customize.to_bytes();
        assert_eq!(bytes.len(), 26);
        assert_eq!(bytes[0], 8);
        assert_eq!(bytes[7], 128);
        assert_eq!(bytes[16], 0x83);
        assert_eq!(bytes[19], 0x80);
        assert_eq!(bytes[25], 12);
        assert_eq!(
            CharacterCustomize::from_bytes(&bytes).expect("parse bytes"),
            customize
        );
        assert!(CharacterCustomize::from_bytes(&bytes[..25]).is_err());
        let mut long = bytes.to_vec();
        long.push(0);
        assert!(CharacterCustomize::from_bytes(&long).is_err());
    }

    #[test]
    fn race_code_combines_race_and_gender() {
        let mut customize = hyur_male();
        assert_eq!(customize.race_code(), 101);
        customize.gender = 1;
        assert_eq!(customize.race_code(), 201);
        customize.race = 8;
        customize.tribe = 16;
        customize.gender = 1;
        assert_eq!(customize.race_code(), 1801);
        assert_eq!(customize.palette_group_index(), (16 - 1) * 2 + 1);
    }

    #[test]
    fn validate_rejects_out_of_range_key_fields() {
        let mut customize = hyur_male();
        assert!(customize.validate().is_ok());
        customize.race = 0;
        customize.gender = 2;
        customize.tribe = 17;
        let error = customize.validate().expect_err("invalid customize");
        assert!(error.contains("race"));
        assert!(error.contains("gender"));
        assert!(error.contains("tribe"));
    }

    #[test]
    fn part_paths_cover_body_face_hair_for_all_races() {
        let customize = hyur_male();
        let parts = character_part_paths(&customize);
        // 中原男：小衣 top/dwn/sho + 裸肤 glv + 脸 + 发（有 e0001 sho，不补裸足）。
        assert_eq!(parts.len(), 6);
        assert_eq!(parts[0].kind, CharacterPartKind::BodyTop);
        assert_eq!(
            parts[0].model_path,
            "chara/equipment/e0001/model/c0101e0001_top.mdl"
        );
        assert!(parts[0].alternate_model_paths.is_empty());
        assert_eq!(parts[1].kind, CharacterPartKind::BodyDwn);
        assert_eq!(parts[2].kind, CharacterPartKind::BodySho);
        assert!(!parts[2].required);
        assert_eq!(parts[3].kind, CharacterPartKind::BodyGlv);
        assert_eq!(
            parts[3].model_path,
            "chara/equipment/e0000/model/c0101e0000_glv.mdl"
        );
        // 脸字节 0 基、文件 1 基：head=4 → f0005；发字节直连（1 基）：51 → h0051。
        assert_eq!(
            parts[4].model_path,
            "chara/human/c0101/obj/face/f0005/model/c0101f0005_fac.mdl"
        );
        assert_eq!(
            parts[5].model_path,
            "chara/human/c0101/obj/hair/h0051/model/c0101h0051_hir.mdl"
        );
        // 无尾/兔耳种族不出现可选部件。
        assert!(
            !parts
                .iter()
                .any(|part| part.kind == CharacterPartKind::Tail)
        );
        assert!(
            !parts
                .iter()
                .any(|part| part.kind == CharacterPartKind::Zear)
        );

        // 高地男（c0301）：无 e0001 sho，补 e0000 裸足（版型回退中原男）。
        let mut highlander = hyur_male();
        highlander.race = 1;
        highlander.tribe = 2;
        let hyur_parts = character_part_paths(&highlander);
        assert_eq!(
            hyur_parts[0].model_path,
            "chara/equipment/e0001/model/c0301e0001_top.mdl"
        );
        let bare_shoes = hyur_parts
            .iter()
            .filter(|part| part.kind == CharacterPartKind::BodySho)
            .collect::<Vec<_>>();
        assert_eq!(bare_shoes.len(), 2, "无 e0001 sho 的族补 e0000 裸足");
        assert_eq!(
            bare_shoes[1].model_path,
            "chara/equipment/e0000/model/c0301e0000_sho.mdl"
        );
        assert_eq!(
            bare_shoes[1].alternate_model_paths,
            ["chara/equipment/e0000/model/c0101e0000_sho.mdl"]
        );
        // 拉拉男（c1101）：小衣/裸肤均自有文件，无回退。
        let mut lala_male = hyur_male();
        lala_male.race = RACE_LALAFELL;
        lala_male.tribe = 9;
        let lala_parts = character_part_paths(&lala_male);
        assert_eq!(
            lala_parts[0].model_path,
            "chara/equipment/e0001/model/c1101e0001_top.mdl"
        );
        assert!(lala_parts[0].alternate_model_paths.is_empty());
        // 拉拉女（c1201）：小衣自有文件；裸肤手回退拉拉男。
        let mut lala_female = hyur_male();
        lala_female.race = RACE_LALAFELL;
        lala_female.tribe = 10;
        lala_female.gender = 1;
        let lala_female_parts = character_part_paths(&lala_female);
        assert_eq!(
            lala_female_parts[0].model_path,
            "chara/equipment/e0001/model/c1201e0001_top.mdl"
        );
        assert!(lala_female_parts[0].alternate_model_paths.is_empty());
        let lala_female_glv = lala_female_parts
            .iter()
            .find(|part| part.kind == CharacterPartKind::BodyGlv)
            .expect("glv");
        assert_eq!(
            lala_female_glv.model_path,
            "chara/equipment/e0000/model/c1201e0000_glv.mdl"
        );
        assert_eq!(
            lala_female_glv.alternate_model_paths,
            ["chara/equipment/e0000/model/c1101e0000_glv.mdl"]
        );
        // 硌狮女（c1601）：小衣回退鲁加女；裸肤手回退中原女。
        let mut hroth_female = hyur_male();
        hroth_female.race = RACE_HROTHGAR;
        hroth_female.tribe = 13;
        hroth_female.gender = 1;
        hroth_female.head = 3;
        let hroth_parts = character_part_paths(&hroth_female);
        assert_eq!(
            hroth_parts[0].alternate_model_paths,
            ["chara/equipment/e0001/model/c1001e0001_top.mdl"]
        );
        let hroth_glv = hroth_parts
            .iter()
            .find(|part| part.kind == CharacterPartKind::BodyGlv)
            .expect("glv");
        assert_eq!(
            hroth_glv.alternate_model_paths,
            ["chara/equipment/e0000/model/c0201e0000_glv.mdl"]
        );
        // 硌狮女（c1601）脸文件仅 f0005-f0008：字节 3 → f0008。
        let hroth_face = hroth_parts
            .iter()
            .find(|part| part.kind == CharacterPartKind::Face)
            .expect("face");
        assert_eq!(
            hroth_face.model_path,
            "chara/human/c1601/obj/face/f0008/model/c1601f0008_fac.mdl"
        );

        let mut miqo = hyur_male();
        miqo.race = RACE_MIQOTE;
        miqo.tribe = 7;
        miqo.tail_ears_type = 1;
        let miqo_parts = character_part_paths(&miqo);
        let tail = miqo_parts
            .iter()
            .find(|part| part.kind == CharacterPartKind::Tail)
            .expect("miqo'te has a tail");
        assert!(!tail.required);
        // 尾字节 0 基、文件 1 基：tail_ears_type=1 → t0002。
        assert_eq!(
            tail.model_path,
            "chara/human/c0701/obj/tail/t0002/model/c0701t0002_til.mdl"
        );

        let mut viera = hyur_male();
        viera.race = RACE_VIERA;
        viera.tribe = 16;
        viera.gender = 1;
        viera.tail_ears_type = 3;
        let viera_parts = character_part_paths(&viera);
        let zear = viera_parts
            .iter()
            .find(|part| part.kind == CharacterPartKind::Zear)
            .expect("viera has zear");
        assert_eq!(
            zear.model_path,
            "chara/human/c1801/obj/zear/z0004/model/c1801z0004_zer.mdl"
        );

        // 共享发型（h0116）的备选模型根：共享表根（中原女）+ 中原男/女根，
        // 去重且不含自身首选。
        let mut shared_hair = viera;
        shared_hair.hair = 116;
        let shared_parts = character_part_paths(&shared_hair);
        let hair_part = shared_parts
            .iter()
            .find(|part| part.kind == CharacterPartKind::Hair)
            .expect("hair part");
        assert_eq!(
            hair_part.model_path,
            "chara/human/c1801/obj/hair/h0116/model/c1801h0116_hir.mdl"
        );
        assert_eq!(
            hair_part.alternate_model_paths,
            [
                "chara/human/c0201/obj/hair/h0116/model/c0201h0116_hir.mdl",
                "chara/human/c0101/obj/hair/h0116/model/c0101h0116_hir.mdl",
            ]
        );
        // 独占发型无备选。
        let exclusive_parts = character_part_paths(&viera);
        let exclusive_hair = exclusive_parts
            .iter()
            .find(|part| part.kind == CharacterPartKind::Hair)
            .expect("hair part");
        assert!(exclusive_hair.alternate_model_paths.is_empty());
    }

    #[test]
    fn hair_sharing_hrothgar_and_unique_ranges_keep_own_root() {
        // 硌狮永不共享。
        for hair in [1, 50, 101, 116, 150, 201, 255] {
            assert_eq!(shared_hair_race_code(1501, hair), 1501, "hair {hair}");
            assert_eq!(shared_hair_race_code(1601, hair), 1601, "hair {hair}");
        }
        // 独占段原样。
        assert_eq!(shared_hair_race_code(101, 50), 101);
        assert_eq!(shared_hair_race_code(201, 50), 201);
        assert_eq!(shared_hair_race_code(801, 205), 801);
        assert_eq!(shared_hair_race_code(1801, 210), 1801);
        // h0101-h0115：五个共享族自身，其余按性别重定向中原。
        assert_eq!(shared_hair_race_code(101, 110), 101);
        assert_eq!(shared_hair_race_code(201, 110), 201);
        assert_eq!(shared_hair_race_code(701, 110), 701);
        assert_eq!(shared_hair_race_code(801, 110), 801);
        assert_eq!(shared_hair_race_code(301, 110), 101);
        assert_eq!(shared_hair_race_code(401, 110), 201);
        assert_eq!(shared_hair_race_code(1801, 110), 201);
        assert_eq!(shared_hair_race_code(1701, 110), 101);
        assert_eq!(shared_hair_race_code(1301, 115), 101);
        assert_eq!(shared_hair_race_code(1401, 115), 201);
    }

    #[test]
    fn hair_sharing_mid_range_redirects_to_midlander() {
        // h0116-h0200：全部共享到中原男/女（含猫魅与硌狮之外的种族）。
        assert_eq!(shared_hair_race_code(101, 116), 101);
        assert_eq!(shared_hair_race_code(101, 150), 101);
        assert_eq!(shared_hair_race_code(201, 116), 201);
        assert_eq!(shared_hair_race_code(301, 120), 101);
        assert_eq!(shared_hair_race_code(401, 120), 201);
        assert_eq!(shared_hair_race_code(701, 180), 101);
        assert_eq!(shared_hair_race_code(801, 180), 201);
        assert_eq!(shared_hair_race_code(1801, 200), 201);
        // h0201+ 回到独占。
        assert_eq!(shared_hair_race_code(301, 201), 301);
        assert_eq!(shared_hair_race_code(1401, 230), 1401);
    }

    #[test]
    fn hair_material_candidates_rewrite_shared_root_and_file() {
        // 维埃拉女 h0116：模型/材质根重定向中原女，文件名 c0201h0116。
        let mut customize = hyur_male();
        customize.race = RACE_VIERA;
        customize.gender = 1;
        customize.tribe = 15;
        customize.hair = 116;
        let candidates = character_material_candidate_paths(
            &customize,
            CharacterPartKind::Hair,
            "chara/human/c1801/obj/hair/h0116/model/c1801h0116_hir.mdl",
            "/mt_c1801h0116_hir_a.mtrl",
        );
        assert_eq!(
            candidates[1],
            "chara/human/c0201/obj/hair/h0116/material/v0001/mt_c0201h0116_hir_a.mtrl"
        );
        assert!(candidates.contains(
            &"chara/human/c1801/obj/hair/h0116/material/v0001/mt_c1801h0116_hir_a.mtrl".to_string()
        ));
        // 独占发型：共享根=自身根，候选去重后仍命中自身 v0001。
        customize.hair = 40;
        let own = character_material_candidate_paths(
            &customize,
            CharacterPartKind::Hair,
            "chara/human/c1801/obj/hair/h0040/model/c1801h0040_hir.mdl",
            "/mt_c1801h0040_hir_a.mtrl",
        );
        assert_eq!(
            own[1],
            "chara/human/c1801/obj/hair/h0040/material/v0001/mt_c1801h0040_hir_a.mtrl"
        );
        // 硌狮共享段也不重写。
        let mut hroth = hyur_male();
        hroth.race = RACE_HROTHGAR;
        hroth.tribe = 13;
        hroth.hair = 116;
        let hroth_candidates = character_material_candidate_paths(
            &hroth,
            CharacterPartKind::Hair,
            "chara/human/c1501/obj/hair/h0116/model/c1501h0116_hir.mdl",
            "/mt_c1501h0116_hir_a.mtrl",
        );
        assert_eq!(
            hroth_candidates[1],
            "chara/human/c1501/obj/hair/h0116/material/v0001/mt_c1501h0116_hir_a.mtrl"
        );
    }

    #[test]
    fn body_material_candidates_prefer_skin_file_race_and_v0001() {
        let customize = hyur_male();
        let candidates = character_material_candidate_paths(
            &customize,
            CharacterPartKind::BodyTop,
            "chara/equipment/e0001/model/c0101e0001_top.mdl",
            "/mt_c0101b0001_a.mtrl",
        );
        assert_eq!(
            candidates,
            [
                "mt_c0101b0001_a.mtrl",
                // 自身肤族根（中原男自用，与内嵌文件名反推一致，去重后次序如下）。
                "chara/human/c0101/obj/body/b0001/material/v0001/mt_c0101b0001_a.mtrl",
                "chara/human/c0101/obj/body/b0001/material/mt_c0101b0001_a.mtrl",
                // 装备根（小衣材质）。
                "chara/equipment/e0001/material/v0001/mt_c0101b0001_a.mtrl",
                "chara/equipment/e0001/material/mt_c0101b0001_a.mtrl",
            ]
        );
        // 鲁加女皮肤回退（肤族=高地女）：自身肤族根优先于内嵌文件名根。
        let mut roe = hyur_male();
        roe.race = RACE_ROEGADYN;
        roe.gender = 1;
        roe.tribe = 10;
        let roe_candidates = character_material_candidate_paths(
            &roe,
            CharacterPartKind::BodyTop,
            "chara/equipment/e0001/model/c1001e0001_top.mdl",
            "/mt_c1001b0001_a.mtrl",
        );
        assert_eq!(
            roe_candidates[1],
            "chara/human/c0401/obj/body/b0001/material/v0001/mt_c0401b0001_a.mtrl"
        );
        // 维埃拉女加载猫魅女文件（内嵌 mt_c0201b0001_a）：游戏仍用角色自身
        // 肤族（c1801）皮肤材质，自身肤族根优先于内嵌的 c0201 根。
        let mut viera = hyur_male();
        viera.race = RACE_VIERA;
        viera.gender = 1;
        viera.tribe = 15;
        let viera_candidates = character_material_candidate_paths(
            &viera,
            CharacterPartKind::BodyTop,
            "chara/equipment/e0001/model/c0801e0001_top.mdl",
            "/mt_c0201b0001_a.mtrl",
        );
        assert_eq!(
            viera_candidates[1],
            "chara/human/c1801/obj/body/b0001/material/v0001/mt_c1801b0001_a.mtrl"
        );
        assert!(viera_candidates.contains(
            &"chara/human/c0201/obj/body/b0001/material/v0001/mt_c0201b0001_a.mtrl".to_string()
        ));
    }

    #[test]
    fn face_and_zear_material_candidates_have_no_version_dir() {
        let customize = hyur_male();
        let face = character_material_candidate_paths(
            &customize,
            CharacterPartKind::Face,
            "chara/human/c0101/obj/face/f0004/model/c0101f0004_fac.mdl",
            "/mt_c0101f0004_fac_a.mtrl",
        );
        assert_eq!(
            face,
            [
                "mt_c0101f0004_fac_a.mtrl",
                "chara/human/c0101/obj/face/f0004/material/mt_c0101f0004_fac_a.mtrl",
            ]
        );
        let mut viera = hyur_male();
        viera.race = RACE_VIERA;
        viera.tribe = 16;
        let zear = character_material_candidate_paths(
            &viera,
            CharacterPartKind::Zear,
            "chara/human/c1801/obj/zear/z0003/model/c1801z0003_zer.mdl",
            "/mt_c1801z0003_zer_a.mtrl",
        );
        assert!(zear.iter().all(|candidate| !candidate.contains("/v0")));
        assert!(zear.contains(
            &"chara/human/c1801/obj/zear/z0003/material/mt_c1801z0003_zer_a.mtrl".to_string()
        ));
    }
}
