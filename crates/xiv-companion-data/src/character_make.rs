//! 捏脸数据资产：human.cmp 调色板与 CharaMakeType 捏脸菜单的结构定义与纯解析。
//!
//! 语义来源（均已核实）：
//! - human.cmp：u32（4 字节 RGBA）颜色项数组；公共区 18 个 256 项块
//!   （Anamnesis ColorData：0=眼/特征、1=挑染、13=唇/面妆上下半各 96+32 跳
//!   项），其后 16 部族 × 2 性别 × 5 块（前 3 块未用，+3=肤色 192、+4=发色
//!   192/208）。Ktisis `CharaCmpReader` 交叉验证：肤色 (4608 + idx*1280 +
//!   3*256)*4 字节处。
//! - CharaMakeType：32 行 = 8 Race × 2 Tribe × 2 Gender；`Menu[i]` 是菜单号，
//!   `Customize[i]` 是该菜单写入的捏脸字节偏移（0=不写入，如声音 203），
//!   `InitVal[i]` 是默认值；SubMenuType 0=列表 1=图标列表 2=色板 3=发色板
//!   4=特征 bitmask 5=滑条。映射从 CSV 数据驱动提取，无硬编码菜单表。
//!
//! squared RGB 验证结论：cmp 颜色按原始 u8 RGBA 直接作显示色使用（Anamnesis
//! `ColorData` r/255 直接用；UI 显示正常），即非 squared；squared 仅出现在
//! 渲染 cbuf（Meddle 注释所指），与 cmp 资产无关。

use serde::{Deserialize, Serialize};

use crate::chara_assemble::{CharacterCustomize, RACE_HROTHGAR};

pub const CHARACTER_PALETTE_SCHEMA_VERSION: u32 = 1;
pub const CHARACTER_MAKE_SCHEMA_VERSION: u32 = 1;

/// human.cmp 单个块的色项数。
pub const CMP_BLOCK_LEN: usize = 256;
/// 公共区块数（18 × 256 色项 = 4608）。
pub const CMP_COMMON_BLOCK_COUNT: usize = 18;
/// 每（部族, 性别）占 5 个块（前 3 未用，第 4 肤色、第 5 发色）。
pub const CMP_TRIBE_BLOCK_COUNT: usize = 5;
/// (部族, 性别) 组数：16 × 2。
pub const CMP_TRIBE_GROUP_COUNT: usize = 32;
/// 公共区在色项数组中的起始偏移（部族区紧随其后）。
pub const CMP_TRIBE_BASE_INDEX: usize = CMP_COMMON_BLOCK_COUNT * CMP_BLOCK_LEN;
/// 部族区内肤色块序号。
pub const CMP_TRIBE_SKIN_BLOCK: usize = 3;
/// 部族区内发色块序号。
pub const CMP_TRIBE_HAIR_BLOCK: usize = 4;

/// 眼睛/特征/挑染/唇/面妆调色板的公共区块下标与长度（Anamnesis ColorData
/// 布局，已对真实 human.cmp 校验）。
pub const CMP_EYE_BLOCK: usize = 0;
pub const CMP_EYE_LEN: usize = 192;
pub const CMP_HIGHLIGHT_BLOCK: usize = 1;
pub const CMP_HIGHLIGHT_LEN: usize = 208;
/// 唇/面妆共用块：下半 0..96 为唇色、上半 128..224 为面妆色，各带 alpha
/// 通道（Anamnesis `IsPaletteSplit`）；中间 32 项为占位。
pub const CMP_LIP_FACEPAINT_BLOCK: usize = 13;
pub const CMP_LIP_FACEPAINT_HALF_LEN: usize = 96;
pub const CMP_LIP_FACEPAINT_SPLIT_SKIP: usize = 32;
/// 种族特征色与眼色共用块 0（Anamnesis FacialFeature PaletteIndex=0）。
pub const CMP_FEATURE_BLOCK: usize = CMP_EYE_BLOCK;
pub const CMP_FEATURE_LEN: usize = CMP_EYE_LEN;
/// 唇/面妆导出长度（导出含 alpha 的完整 128 项：96 色 + 32 占位置透明）。
pub const CMP_LIP_EXPORT_LEN: usize = 128;

/// 肤色/发色等 RGBA 颜色（u8 通道，顺序 r,g,b,a）。
pub type RgbaColor = [u8; 4];

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterPalette {
    pub common: CharacterPaletteCommon,
    /// 按 `(tribe - 1) * 2 + gender` 排序的 32 组。
    pub tribes: Vec<CharacterTribePalette>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterPaletteCommon {
    /// 眼色（同时也是种族特征色与胧眼色，192 项）。
    pub eye: Vec<RgbaColor>,
    /// 挑染色（208 项；h0101+ 发型可用）。
    pub highlight: Vec<RgbaColor>,
    /// 唇色（128 项含 alpha：96 色 + 32 透明占位）。
    pub lip: Vec<RgbaColor>,
    /// 种族特征色（与眼色同源，192 项）。
    pub feature: Vec<RgbaColor>,
    /// 面妆色（128 项含 alpha：96 色 + 32 透明占位）。
    pub face_paint: Vec<RgbaColor>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterTribePalette {
    pub tribe: u8,
    pub gender: u8,
    pub skin: Vec<RgbaColor>,
    /// 硌狮（Race 7）192 项，其余种族 208 项。
    pub hair: Vec<RgbaColor>,
}

/// human.cmp 调色板资产包（JSON schema 1）。
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterPalettePackage {
    pub schema_version: u32,
    pub generated_at: String,
    pub game_version: String,
    pub source: String,
    pub meta: CharacterPaletteMeta,
    pub palette: CharacterPalette,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterPaletteMeta {
    /// 颜色通道顺序与编码。
    pub channel_order: String,
    pub encoding: String,
    /// cmp 颜色是否为 squared RGB：已验证为否（见模块文档）。
    pub squared_rgb: bool,
    /// 布局说明（块结构摘要，供人工核对）。
    pub layout: String,
}

/// 解析 human.cmp 原始字节为调色板（纯函数，IO 在 xtask 侧）。
///
/// 布局：色项 = 连续 u32，文件内字节序为 r,g,b,a（Anamnesis 按字节读取）；
/// 色板区 4608 + 32 × 1280 = 45568 色项（182272 字节）。真实文件在色板区后
/// 还带 4480 字节尾部（32 × 140 字节的种族缩放参数记录，RGSP 同源的
/// CharaMakeParameter 数据，TexTools `CMP.cs` 读写；调色板用途忽略之）。
pub fn character_palette_from_cmp_bytes(bytes: &[u8]) -> Result<CharacterPalette, String> {
    const COLOR_BYTES: usize = 4;
    let expected = (CMP_TRIBE_BASE_INDEX
        + CMP_TRIBE_GROUP_COUNT * CMP_TRIBE_BLOCK_COUNT * CMP_BLOCK_LEN)
        * COLOR_BYTES;
    if bytes.len() < expected {
        return Err(format!(
            "human.cmp must be at least {expected} bytes ({} colors), got {} bytes",
            expected / COLOR_BYTES,
            bytes.len()
        ));
    }
    let bytes = &bytes[..expected];
    let color = |index: usize| -> RgbaColor {
        let at = index * COLOR_BYTES;
        [bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]
    };
    let block = |block_index: usize, len: usize, out_len: usize| -> Vec<RgbaColor> {
        let base = block_index * CMP_BLOCK_LEN;
        let mut colors: Vec<RgbaColor> = (0..len).map(|i| color(base + i)).collect();
        colors.resize(out_len, [0, 0, 0, 0]);
        colors
    };

    let common = CharacterPaletteCommon {
        eye: block(CMP_EYE_BLOCK, CMP_EYE_LEN, CMP_EYE_LEN),
        highlight: block(CMP_HIGHLIGHT_BLOCK, CMP_HIGHLIGHT_LEN, CMP_HIGHLIGHT_LEN),
        lip: split_palette(&color, CMP_LIP_FACEPAINT_BLOCK, false),
        feature: block(CMP_FEATURE_BLOCK, CMP_FEATURE_LEN, CMP_FEATURE_LEN),
        face_paint: split_palette(&color, CMP_LIP_FACEPAINT_BLOCK, true),
    };

    let mut tribes = Vec::with_capacity(CMP_TRIBE_GROUP_COUNT);
    for group in 0..CMP_TRIBE_GROUP_COUNT {
        let base = CMP_TRIBE_BASE_INDEX + group * CMP_TRIBE_BLOCK_COUNT * CMP_BLOCK_LEN;
        let tribe = (group / 2) as u8 + 1;
        let gender = (group % 2) as u8;
        let hair_len = if (tribe + 1) / 2 == RACE_HROTHGAR {
            CMP_EYE_LEN
        } else {
            CMP_HIGHLIGHT_LEN
        };
        tribes.push(CharacterTribePalette {
            tribe,
            gender,
            skin: (0..CMP_EYE_LEN)
                .map(|i| color(base + CMP_TRIBE_SKIN_BLOCK * CMP_BLOCK_LEN + i))
                .collect(),
            hair: (0..hair_len)
                .map(|i| color(base + CMP_TRIBE_HAIR_BLOCK * CMP_BLOCK_LEN + i))
                .collect(),
        });
    }

    Ok(CharacterPalette { common, tribes })
}

/// 唇/面妆半区导出：下半（`upper=false`）0..96 项，上半（`upper=true`）
/// 128..224 项；中间 32 项占位导为透明。
fn split_palette(
    color: &impl Fn(usize) -> RgbaColor,
    block_index: usize,
    upper: bool,
) -> Vec<RgbaColor> {
    let base = block_index * CMP_BLOCK_LEN;
    let mut colors = Vec::with_capacity(CMP_LIP_EXPORT_LEN);
    let start = if upper {
        CMP_LIP_FACEPAINT_HALF_LEN + CMP_LIP_FACEPAINT_SPLIT_SKIP
    } else {
        0
    };
    for i in 0..CMP_LIP_EXPORT_LEN {
        let index = start + i;
        if index < start + CMP_LIP_FACEPAINT_HALF_LEN {
            colors.push(color(base + index));
        } else {
            colors.push([0, 0, 0, 0]);
        }
    }
    colors
}

/// 发色长度：硌狮部族（13/14）192，其余 208。
pub fn tribe_hair_palette_len(tribe: u8) -> usize {
    if (13..=14).contains(&tribe) {
        CMP_EYE_LEN
    } else {
        CMP_HIGHLIGHT_LEN
    }
}

/// 角色渲染色：从捏脸 + human.cmp 调色板解析出的、材质合成阶段写入角色部件
/// 材质的颜色。RGB 为调色板显示值归一（预览管线直接作线性乘子；游戏 cbuf
/// 以 squared RGB 存储，差异原因见 [`appearance_color`]），W 通道为线性
/// 强度/不透明度。
///
/// - `skin`：肤色（SkinColor；调色板 alpha 原样保留，游戏 cbuf 该位是肌肉量，
///   渲染激活标记由渲染侧自行设置）。
/// - `lip`：唇色（LipColor，W = 唇釉不透明度；`lipstick` 关闭时渲染侧不消费）。
/// - `main`：发色（MainColor，头发/眉毛/尾/兔耳 hair.shpk 共用）。
/// - `mesh`：挑染色（MeshColor；`highlights` 关闭时渲染侧不消费）。
/// - `left_iris`/`right_iris`：左/右眼色（虹膜乘色；W = 角膜环强度，
///   26 字节捏脸无此数据，按 1.0 处理）。
/// - `option`：种族特征色（OptionColor，charactertattoo.shpk 面纹等）。
/// - `decal`：面妆色（DecalColor cbuffer，W = 不透明度）。
/// - `face_paint_reversed`：面妆镜像开关（FFXIVClientStructs `CustomizeData`
///   0x18 bit7；0..6 位为面妆号）。面妆贴图候选见
///   [`crate::chara_assemble::face_paint_decal_texture_candidates`]。
#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterAppearanceColors {
    pub skin: [f32; 4],
    pub lip: [f32; 4],
    pub main: [f32; 4],
    pub mesh: [f32; 4],
    pub left_iris: [f32; 4],
    pub right_iris: [f32; 4],
    pub option: [f32; 4],
    pub decal: [f32; 4],
    pub lipstick: bool,
    pub highlights: bool,
    pub face_paint_reversed: bool,
}

impl Default for CharacterAppearanceColors {
    fn default() -> Self {
        Self {
            skin: [1.0, 1.0, 1.0, 0.0],
            lip: [1.0, 1.0, 1.0, 0.0],
            main: [1.0, 1.0, 1.0, 0.0],
            mesh: [1.0, 1.0, 1.0, 0.0],
            left_iris: [1.0, 1.0, 1.0, 0.0],
            right_iris: [1.0, 1.0, 1.0, 0.0],
            option: [1.0, 1.0, 1.0, 0.0],
            decal: [1.0, 1.0, 1.0, 0.0],
            lipstick: false,
            highlights: false,
            face_paint_reversed: false,
        }
    }
}

/// cmp 色项 → 渲染色：RGB 按显示值直接归一（u8/255）作线性乘子，alpha 线性
/// （不透明度量）。
///
/// 游戏 cbuf 以 squared RGB 存储（FFXIVClientStructs `CustomizeParameter`
/// 注释），但预览管线的稳定预览光照（见 model.wgsl 顶部合约）与游戏光照
/// 不同：squared 值在本管线双重变暗。实测对照安装版默认角色（维埃拉女默认
/// 肤色/发色），显示值直接归一复刻游戏观感，squared 明显偏暗；因此预览用
/// 显示值，cbuf squared 语义仅作记录。
fn appearance_color(color: RgbaColor) -> [f32; 4] {
    [
        f32::from(color[0]) / 255.0,
        f32::from(color[1]) / 255.0,
        f32::from(color[2]) / 255.0,
        f32::from(color[3]) / 255.0,
    ]
}

fn palette_color_at(colors: &[RgbaColor], index: u8) -> [f32; 4] {
    let index = usize::from(index).min(colors.len().saturating_sub(1));
    appearance_color(colors.get(index).copied().unwrap_or([255, 255, 255, 255]))
}

/// 由捏脸 + 调色板解析角色渲染色（纯函数）。索引越界时钳到色板末位
/// （捏脸选项本来由菜单约束；调色板缺组时按白色不染色处理）。
pub fn appearance_colors_from_palette(
    customize: &CharacterCustomize,
    palette: &CharacterPalette,
) -> CharacterAppearanceColors {
    let tribe_palette = palette
        .tribes
        .get(customize.palette_group_index())
        .or_else(|| palette.tribes.first());
    let skin = tribe_palette
        .map(|tribe| palette_color_at(&tribe.skin, customize.skintone))
        .unwrap_or([1.0, 1.0, 1.0, 0.0]);
    let main = tribe_palette
        .map(|tribe| palette_color_at(&tribe.hair, customize.hair_tone))
        .unwrap_or([1.0, 1.0, 1.0, 0.0]);
    let mut left_iris = palette_color_at(&palette.common.eye, customize.l_eye_color);
    let mut right_iris = palette_color_at(&palette.common.eye, customize.r_eye_color);
    // 角膜环强度无离线来源（运行态 cbuffer），按满强度处理。
    left_iris[3] = 1.0;
    right_iris[3] = 1.0;
    CharacterAppearanceColors {
        skin,
        lip: palette_color_at(&palette.common.lip, customize.lips_tone),
        main,
        mesh: palette_color_at(&palette.common.highlight, customize.highlights),
        left_iris,
        right_iris,
        option: palette_color_at(&palette.common.feature, customize.facial_feature_color),
        decal: palette_color_at(&palette.common.face_paint, customize.face_paint_color),
        lipstick: customize.mouth & 0x80 != 0,
        highlights: customize.highlight_type & 0x80 != 0,
        face_paint_reversed: customize.face_paint & 0x80 != 0,
    }
}

/// CharaMakeType 原始行（CSV 列已由 xtask 切分）。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CharacterMakeCsvRow {
    pub key: u32,
    pub race: u8,
    pub tribe: u8,
    pub gender: u8,
    pub menus: Vec<CharacterMakeCsvMenu>,
    /// 发型选项 ID 列表（CharaMakeCustomize 槽位块序，即捏脸界面发型菜单的
    /// 0 基选项索引 → 发型 ID 映射）。
    pub hair_options: Vec<u16>,
    /// 面妆选项 ID 列表（同槽面妆块序；0 = 无面妆，不在列表中）。
    pub face_paint_options: Vec<u16>,
    /// FacialFeatureOption[i][j]（i 0..8 脸位、j 0..7 特征位），行优先展开。
    pub facial_feature_options: Vec<u16>,
}

/// CharaMakeCustomize 原始行。CSV 数据列实为 `key,FeatureID,Data,...`
/// （表头写的 Icon 列在实际数据中不存在）。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CharacterMakeCustomizeRow {
    pub key: u32,
    /// 发型/面妆 ID：发型即文件号（h#### 的 ####）。
    pub feature_id: u16,
    /// `prefix * 1000 + 组内 1 基序号`；发型 prefix = 130 + Race byte
    /// （131..=138），面妆 prefix = 250/251。
    pub data: u32,
}

/// CharaMakeCustomize 槽位：一个（c 编码, 性别）组的发型/面妆选项块。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CharacterMakeCustomizeSlot {
    pub hair_options: Vec<u16>,
    pub face_paint_options: Vec<u16>,
}

/// 槽位首 key。
pub const CHARA_MAKE_CUSTOMIZE_FIRST_KEY: u32 = 1;
/// 槽位 key 步长：槽 i 占 keys `[1 + 130*i, 1 + 130*i + 129]`（发型区约
/// 99 key + 面妆区约 31 key，尾部存在发型/面妆行交错，须按 Data 前缀分区
/// 并按 Data 序号排序，不能按行序切）。
pub const CHARA_MAKE_CUSTOMIZE_SLOT_KEY_STRIDE: u32 = 130;
/// 槽位数 = 18 个 c 编码（中原/高地各占 2，其余种族每族 1 对）。
pub const CHARA_MAKE_CUSTOMIZE_SLOT_COUNT: usize = 18;

/// 槽 ↔ c 编码对应表：槽序即 c 编码序 0101..1801（已对真实 SqPack 发型
/// 文件集合全量验证：每槽 FeatureID 集合 ⊆ 对应种族发型文件集合）。
pub const CHARA_MAKE_CUSTOMIZE_SLOT_RACE_CODES: [u16; CHARA_MAKE_CUSTOMIZE_SLOT_COUNT] = [
    101, 201, 301, 401, 501, 601, 701, 801, 901, 1001, 1101, 1201, 1301, 1401, 1501, 1601, 1701,
    1801,
];

/// 解析 CharaMakeCustomize 全部槽位。选项顺序 = 槽内 Data 序号升序。
pub fn parse_chara_make_customize_slots(
    rows: &[CharacterMakeCustomizeRow],
) -> Vec<CharacterMakeCustomizeSlot> {
    let mut entries: Vec<(Vec<(u32, u16)>, Vec<(u32, u16)>)> = (0..CHARA_MAKE_CUSTOMIZE_SLOT_COUNT)
        .map(|_| (Vec::new(), Vec::new()))
        .collect();
    for row in rows {
        if row.key < CHARA_MAKE_CUSTOMIZE_FIRST_KEY || (row.feature_id == 0 && row.data == 0) {
            continue;
        }
        let relative = row.key - CHARA_MAKE_CUSTOMIZE_FIRST_KEY;
        let slot_index = (relative / CHARA_MAKE_CUSTOMIZE_SLOT_KEY_STRIDE) as usize;
        if slot_index >= CHARA_MAKE_CUSTOMIZE_SLOT_COUNT {
            continue;
        }
        let prefix = row.data / 1000;
        let index = row.data % 1000;
        let entry = (index, row.feature_id);
        match prefix {
            131..=139 => entries[slot_index].0.push(entry),
            250..=251 => entries[slot_index].1.push(entry),
            _ => {}
        }
    }
    entries
        .into_iter()
        .map(|(mut hair, mut paint)| {
            hair.sort_unstable();
            paint.sort_unstable();
            CharacterMakeCustomizeSlot {
                hair_options: hair.into_iter().map(|(_, feature_id)| feature_id).collect(),
                face_paint_options: paint
                    .into_iter()
                    .map(|(_, feature_id)| feature_id)
                    .collect(),
            }
        })
        .collect()
}

/// c 编码 → 槽索引。
pub fn chara_make_customize_slot_index(race_code: u16) -> Option<usize> {
    CHARA_MAKE_CUSTOMIZE_SLOT_RACE_CODES
        .iter()
        .position(|candidate| *candidate == race_code)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CharacterMakeCsvMenu {
    pub menu: u32,
    pub init_val: u16,
    pub sub_menu_type: u8,
    pub sub_menu_num: u16,
    /// `Customize[i]` 列：菜单写入的捏脸字节偏移（0 = 不写入，如声音）。
    pub byte_offset: usize,
}

/// 捏脸菜单（资产/展示用）：语义名、选项形态与默认值。
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterMakeMenu {
    pub slot: usize,
    pub menu: u32,
    /// 语义名（`character_make_menu_name`；未知为 null）。
    pub name: Option<String>,
    /// 写入的捏脸字节偏移（26 字节布局下标；0 表示不写入，如声音）。
    pub byte_offset: usize,
    /// 选项形态：list / icon-list / palette / hair-palette / feature-bitmask / slider。
    pub kind: CharacterMakeMenuKind,
    /// 选项数；list/icon-list 为 ID 上限（1 起），palette 为色板长度，
    /// slider 为最大值（最小值恒 0）。
    pub option_count: u16,
    pub default_value: u16,
    /// 色板类菜单对应的调色板键（`character_make_menu_palette`）。
    pub palette: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CharacterMakeMenuKind {
    List,
    IconList,
    Palette,
    HairPalette,
    FeatureBitmask,
    Slider,
}

impl CharacterMakeMenuKind {
    pub fn from_sub_menu_type(sub_menu_type: u8) -> Self {
        match sub_menu_type {
            1 => Self::IconList,
            2 => Self::Palette,
            3 => Self::HairPalette,
            4 => Self::FeatureBitmask,
            5 => Self::Slider,
            _ => Self::List,
        }
    }
}

/// 单（种族, 部族, 性别）捏脸组。
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterMakeGroup {
    pub key: u32,
    pub race: u8,
    pub tribe: u8,
    pub gender: u8,
    pub race_code: u16,
    pub menus: Vec<CharacterMakeMenu>,
    /// InitVal 推导的默认捏脸（26 字节；发型/面妆选项索引已换算为 ID）。
    pub default_customize: CharacterCustomize,
    /// 发型选项 ID 列表（0 基选项索引 → 发型 ID，同捏脸界面顺序）。
    pub hair_options: Vec<u16>,
    /// 面妆选项 ID 列表（0 = 无；菜单选项 InitVal 0 对应无面妆）。
    pub face_paint_options: Vec<u16>,
    pub facial_feature_options: Vec<u16>,
}

/// CharaMakeType 捏脸菜单资产包（JSON schema 1）。
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterMakePackage {
    pub schema_version: u32,
    pub generated_at: String,
    pub game_version: String,
    pub source: String,
    pub groups: Vec<CharacterMakeGroup>,
}

/// 已知菜单号的语义名（仅列确定语义的；种族专属滑条/列表按偏移泛化命名）。
pub fn character_make_menu_name(menu: u32) -> Option<&'static str> {
    Some(match menu {
        201 => "height",
        202 => "skintone",
        203 => "voice",
        234 => "hair",
        236 => "hairTone",
        238 => "face",
        241 => "jaw",
        242 => "eyebrows",
        243 => "eyes",
        244 => "eyePupil",
        245 => "eyeColorRight",
        246 => "nose",
        247 => "mouth",
        248 => "lipsTone",
        249 => "facePaint",
        250 => "facePaintColor",
        1013 | 1744 | 1748 | 1750 | 1756 => "facialFeatureColor",
        1045 | 1740 | 1741 | 1742 | 1743 | 1746 | 1747 | 1749 | 1751 | 1755 => "facialFeatures",
        _ => return None,
    })
}

/// 菜单写入字节偏移对应的调色板键（色板类菜单）。
pub fn character_make_menu_palette(byte_offset: usize) -> Option<&'static str> {
    Some(match byte_offset {
        8 => "skin",
        9 | 15 => "eye",
        10 => "hair",
        11 => "highlight",
        13 => "feature",
        20 => "lip",
        25 => "facepaint",
        _ => return None,
    })
}

/// 菜单偏移的泛化语义名（种族专属滑条/列表没有稳定菜单号时使用）。
fn character_make_offset_fallback_name(byte_offset: usize, kind: CharacterMakeMenuKind) -> String {
    match (byte_offset, kind) {
        (21, CharacterMakeMenuKind::Slider) => "muscleSize".to_string(),
        (21, _) => "earTailSize".to_string(),
        (22, _) => "tailEarsType".to_string(),
        (23, _) => "bustSize".to_string(),
        (5, _) => "face".to_string(),
        _ => format!("menuOffset{byte_offset}"),
    }
}

/// 由菜单行推导默认捏脸：Race/Gender/Tribe 取行值，Age 默认 1。多数菜单的
/// InitVal 直接写入对应字节；两个图标列表例外（已核实）：
/// - 发型（菜单 234）：InitVal 是 0 基选项索引，字节 = `hair_options[InitVal]`
///   （CharaMakeCustomize 块序，发型 ID 即文件号）；
/// - 脸型（菜单 238）：InitVal 是 0 基索引，字节即索引（脸文件号 = 字节 + 1）；
/// - 面妆（菜单 249）：InitVal 0 = 无面妆（字节 0），否则字节 =
///   `face_paint_options[InitVal - 1]`。
pub fn default_customize_from_menus(
    race: u8,
    tribe: u8,
    gender: u8,
    menus: &[CharacterMakeCsvMenu],
    hair_options: &[u16],
    face_paint_options: &[u16],
) -> CharacterCustomize {
    let mut customize = CharacterCustomize {
        race,
        tribe,
        gender,
        age: 1,
        height: 50,
        ..Default::default()
    };
    let mut bytes = customize.to_bytes();
    for menu in menus {
        if menu.byte_offset == 0 || menu.byte_offset >= bytes.len() {
            continue;
        }
        let value = match menu.menu {
            234 => hair_options
                .get(usize::from(menu.init_val))
                .copied()
                .unwrap_or(0),
            249 if menu.init_val > 0 => face_paint_options
                .get(usize::from(menu.init_val - 1))
                .copied()
                .unwrap_or(0),
            _ => menu.init_val,
        };
        bytes[menu.byte_offset] = value.min(u8::MAX as u16) as u8;
    }
    customize = CharacterCustomize::from_bytes(&bytes)
        .expect("default customize bytes built from menu rows");
    customize
}

/// 构建捏脸菜单资产包：行按 key 排序，组内菜单保持 Menu[i] 槽位顺序，
/// 默认值经 `default_customize_from_menus` 落回 26 字节结构。
pub fn build_character_make_package(
    rows: &[CharacterMakeCsvRow],
    generated_at: String,
    game_version: String,
    source: String,
) -> CharacterMakePackage {
    let mut groups: Vec<CharacterMakeGroup> = rows
        .iter()
        .map(|row| {
            let kind_of = |menu: &CharacterMakeCsvMenu| {
                CharacterMakeMenuKind::from_sub_menu_type(menu.sub_menu_type)
            };
            let menus = row
                .menus
                .iter()
                .enumerate()
                .map(|(slot, menu)| {
                    let kind = kind_of(menu);
                    CharacterMakeMenu {
                        slot,
                        menu: menu.menu,
                        name: character_make_menu_name(menu.menu)
                            .map(ToString::to_string)
                            .or_else(|| {
                                (menu.byte_offset != 0).then(|| {
                                    character_make_offset_fallback_name(menu.byte_offset, kind)
                                })
                            }),
                        byte_offset: menu.byte_offset,
                        kind,
                        option_count: menu.sub_menu_num,
                        default_value: menu.init_val,
                        palette: if matches!(
                            kind,
                            CharacterMakeMenuKind::Palette | CharacterMakeMenuKind::HairPalette
                        ) {
                            character_make_menu_palette(menu.byte_offset).map(ToString::to_string)
                        } else {
                            None
                        },
                    }
                })
                .collect::<Vec<_>>();
            CharacterMakeGroup {
                key: row.key,
                race: row.race,
                tribe: row.tribe,
                gender: row.gender,
                race_code: CharacterCustomize::race_code_from_parts(
                    row.race, row.tribe, row.gender,
                ),
                default_customize: default_customize_from_menus(
                    row.race,
                    row.tribe,
                    row.gender,
                    &row.menus,
                    &row.hair_options,
                    &row.face_paint_options,
                ),
                hair_options: row.hair_options.clone(),
                face_paint_options: row.face_paint_options.clone(),
                menus,
                facial_feature_options: row.facial_feature_options.clone(),
            }
        })
        .collect();
    groups.sort_by_key(|group| group.key);
    CharacterMakePackage {
        schema_version: CHARACTER_MAKE_SCHEMA_VERSION,
        generated_at,
        game_version,
        source,
        groups,
    }
}

/// 26 字节默认捏脸表（32 组）的静态访问助手。
pub fn default_customize_for_race_code(
    package: &CharacterMakePackage,
    race_code: u16,
) -> Option<CharacterCustomize> {
    package
        .groups
        .iter()
        .find(|group| group.race_code == race_code)
        .map(|group| group.default_customize)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn customize_slots_parse_by_data_prefix_and_index() {
        // 构造两个槽：槽 0 尾部行交错（面妆行插在发型行之间），槽 1 有空槽
        // 位与未知 prefix；选项必须按 Data 序号而非行序。
        let mut rows = Vec::new();
        rows.push(CharacterMakeCustomizeRow {
            key: 0,
            feature_id: 0,
            data: 0,
        });
        // 槽 0：keys 1..=130。发型 131001..131003（key 3 为填充），面妆行
        // 交错在 key 4，尾部 key 5 的发型 Data 序号小于 key 4 之后的发型。
        for (key, fid, data) in [
            (1, 11, 131001),
            (2, 12, 131002),
            (3, 0, 0),       // 填充
            (4, 91, 250000), // 面妆交错在发型区
            (5, 13, 131003),
        ] {
            rows.push(CharacterMakeCustomizeRow {
                key,
                feature_id: fid,
                data,
            });
        }
        // 槽 1：keys 131..=260，仅两个发型（模拟真实块长度不齐）。
        for (key, fid, data) in [(131, 21, 132001), (132, 22, 132002)] {
            rows.push(CharacterMakeCustomizeRow {
                key,
                feature_id: fid,
                data,
            });
        }
        // 未知 prefix 与越界 key 必须忽略。
        rows.push(CharacterMakeCustomizeRow {
            key: 133,
            feature_id: 99,
            data: 999001,
        });
        rows.push(CharacterMakeCustomizeRow {
            key: 4000,
            feature_id: 99,
            data: 131001,
        });

        let slots = parse_chara_make_customize_slots(&rows);
        assert_eq!(slots.len(), CHARA_MAKE_CUSTOMIZE_SLOT_COUNT);
        assert_eq!(slots[0].hair_options, [11, 12, 13], "按 Data 序号排序");
        assert_eq!(slots[0].face_paint_options, [91]);
        assert_eq!(slots[1].hair_options, [21, 22]);
        assert!(slots[2..].iter().all(|slot| slot.hair_options.is_empty()));
        assert_eq!(chara_make_customize_slot_index(101), Some(0));
        assert_eq!(chara_make_customize_slot_index(1801), Some(17));
        assert_eq!(chara_make_customize_slot_index(102), None);
    }

    fn cmp_fixture() -> Vec<u8> {
        let colors =
            CMP_TRIBE_BASE_INDEX + CMP_TRIBE_GROUP_COUNT * CMP_TRIBE_BLOCK_COUNT * CMP_BLOCK_LEN;
        let mut bytes = vec![0u8; colors * 4];
        // eye[0] = (10, 20, 30, 255)；肤色 tribe1 male 首项 = (200, 150, 120, 255)。
        let put = |bytes: &mut [u8], index: usize, rgba: [u8; 4]| {
            bytes[index * 4..index * 4 + 4].copy_from_slice(&rgba);
        };
        put(&mut bytes, CMP_EYE_BLOCK * CMP_BLOCK_LEN, [10, 20, 30, 255]);
        put(
            &mut bytes,
            CMP_TRIBE_BASE_INDEX + CMP_TRIBE_SKIN_BLOCK * CMP_BLOCK_LEN,
            [200, 150, 120, 255],
        );
        put(
            &mut bytes,
            CMP_TRIBE_BASE_INDEX + CMP_TRIBE_HAIR_BLOCK * CMP_BLOCK_LEN,
            [11, 22, 33, 255],
        );
        bytes
    }

    #[test]
    fn cmp_parser_extracts_common_and_tribe_colors() {
        let palette = character_palette_from_cmp_bytes(&cmp_fixture()).expect("parse cmp");
        assert_eq!(palette.common.eye.len(), 192);
        assert_eq!(palette.common.eye[0], [10, 20, 30, 255]);
        assert_eq!(palette.common.highlight.len(), 208);
        assert_eq!(palette.common.lip.len(), 128);
        assert_eq!(palette.common.face_paint.len(), 128);
        assert_eq!(palette.tribes.len(), 32);
        assert_eq!(palette.tribes[0].tribe, 1);
        assert_eq!(palette.tribes[0].gender, 0);
        assert_eq!(palette.tribes[0].skin[0], [200, 150, 120, 255]);
        assert_eq!(palette.tribes[0].hair[0], [11, 22, 33, 255]);
        assert_eq!(palette.tribes[0].hair.len(), 208);
        // 硌狮部族（13/14）发色 192。
        assert_eq!(palette.tribes[(13 - 1) * 2].hair.len(), 192);
        assert_eq!(palette.tribes[(14 - 1) * 2 + 1].hair.len(), 192);
        assert_eq!(palette.tribes[15].tribe, 8);

        assert!(character_palette_from_cmp_bytes(&[0u8; 4]).is_err());
        // 尾组校验：全部零数据的合法文件不报错。
        let zeros = vec![0u8; cmp_fixture().len()];
        assert!(character_palette_from_cmp_bytes(&zeros).is_ok());
    }

    #[test]
    fn make_package_builds_default_customize_from_init_vals() {
        let rows = vec![
            CharacterMakeCsvRow {
                key: 0,
                race: 1,
                tribe: 1,
                gender: 0,
                menus: vec![
                    CharacterMakeCsvMenu {
                        menu: 201,
                        init_val: 42,
                        sub_menu_type: 5,
                        sub_menu_num: 100,
                        byte_offset: 3,
                    },
                    CharacterMakeCsvMenu {
                        menu: 238,
                        init_val: 4,
                        sub_menu_type: 1,
                        sub_menu_num: 7,
                        byte_offset: 5,
                    },
                    CharacterMakeCsvMenu {
                        menu: 234,
                        init_val: 51,
                        sub_menu_type: 1,
                        sub_menu_num: 53,
                        byte_offset: 6,
                    },
                    CharacterMakeCsvMenu {
                        menu: 249,
                        init_val: 0,
                        sub_menu_type: 1,
                        sub_menu_num: 27,
                        byte_offset: 24,
                    },
                    CharacterMakeCsvMenu {
                        menu: 203,
                        init_val: 0,
                        sub_menu_type: 0,
                        sub_menu_num: 12,
                        byte_offset: 0,
                    },
                ],
                hair_options: (1..=99).collect(),
                face_paint_options: vec![193, 194, 195],
                facial_feature_options: vec![7; 56],
            },
            CharacterMakeCsvRow {
                key: 1,
                race: 1,
                tribe: 1,
                gender: 1,
                menus: vec![CharacterMakeCsvMenu {
                    menu: 209,
                    init_val: 50,
                    sub_menu_type: 5,
                    sub_menu_num: 100,
                    byte_offset: 23,
                }],
                hair_options: Vec::new(),
                face_paint_options: Vec::new(),
                facial_feature_options: Vec::new(),
            },
        ];
        let package = build_character_make_package(
            &rows,
            "2026-09-11T00:00:00Z".to_string(),
            "game-2026.09.01.0000.0000".to_string(),
            "test".to_string(),
        );
        assert_eq!(package.schema_version, CHARACTER_MAKE_SCHEMA_VERSION);
        assert_eq!(package.groups.len(), 2);

        let male = &package.groups[0];
        assert_eq!(male.race_code, 101);
        let customize = male.default_customize;
        assert_eq!(customize.race, 1);
        assert_eq!(customize.gender, 0);
        assert_eq!(customize.age, 1);
        assert_eq!(customize.height, 42, "menu 201 InitVal writes byte 3");
        assert_eq!(
            customize.head, 4,
            "menu 238 InitVal is the 0-based face index"
        );
        assert_eq!(
            customize.hair, 52,
            "menu 234 InitVal is the 0-based hair option index"
        );
        assert_eq!(
            customize.face_paint, 0,
            "menu 249 InitVal 0 = no face paint"
        );
        // 声音菜单（byte_offset 0）不写入任何字节；默认捏脸与行数据推导一致。
        assert!(
            male.menus
                .iter()
                .any(|menu| menu.name.as_deref() == Some("voice"))
        );
        assert_eq!(
            customize,
            default_customize_from_menus(
                1,
                1,
                0,
                &rows[0].menus,
                &rows[0].hair_options,
                &rows[0].face_paint_options
            )
        );
        // 菜单语义名与形态。
        let face_menu = male
            .menus
            .iter()
            .find(|menu| menu.menu == 238)
            .expect("face menu");
        assert_eq!(face_menu.kind, CharacterMakeMenuKind::IconList);
        assert_eq!(face_menu.option_count, 7);
        assert_eq!(face_menu.name.as_deref(), Some("face"));
        let female = &package.groups[1];
        assert_eq!(female.default_customize.bust, 50, "menu 209 writes byte 23");
        assert!(
            female
                .menus
                .iter()
                .any(|menu| menu.name.as_deref() == Some("bustSize"))
        );

        assert_eq!(
            default_customize_for_race_code(&package, 201).map(|c| c.gender),
            Some(1)
        );
        assert!(default_customize_for_race_code(&package, 999).is_none());
    }

    #[test]
    fn face_paint_option_index_maps_to_feature_id() {
        let menus = vec![CharacterMakeCsvMenu {
            menu: 249,
            init_val: 2,
            sub_menu_type: 1,
            sub_menu_num: 27,
            byte_offset: 24,
        }];
        let customize = default_customize_from_menus(1, 1, 0, &menus, &[], &[193, 194, 195]);
        assert_eq!(customize.face_paint, 194, "option 2 = second face paint id");
        let none = default_customize_from_menus(1, 1, 0, &menus, &[], &[]);
        assert_eq!(none.face_paint, 0, "missing option list degrades to 0");
    }

    #[test]
    fn menu_palette_keys_follow_byte_offsets() {
        assert_eq!(character_make_menu_palette(8), Some("skin"));
        assert_eq!(character_make_menu_palette(15), Some("eye"));
        assert_eq!(character_make_menu_palette(10), Some("hair"));
        assert_eq!(character_make_menu_palette(20), Some("lip"));
        assert_eq!(character_make_menu_palette(25), Some("facepaint"));
        assert_eq!(character_make_menu_palette(3), None);
        assert_eq!(tribe_hair_palette_len(13), 192);
        assert_eq!(tribe_hair_palette_len(1), 208);
    }

    fn fixture_palette() -> CharacterPalette {
        let color = |base: u8| -> RgbaColor {
            [
                base,
                base.wrapping_add(1),
                base.wrapping_add(2),
                base.wrapping_add(3),
            ]
        };
        let colors = |base: u8, len: usize| -> Vec<RgbaColor> {
            (0..len)
                .map(|i| color(base.wrapping_add(i as u8)))
                .collect()
        };
        let mut tribes = Vec::new();
        for group in 0..CMP_TRIBE_GROUP_COUNT {
            tribes.push(CharacterTribePalette {
                tribe: (group / 2) as u8 + 1,
                gender: (group % 2) as u8,
                skin: colors(10 + group as u8, CMP_EYE_LEN),
                hair: colors(100 + group as u8, CMP_HIGHLIGHT_LEN),
            });
        }
        CharacterPalette {
            common: CharacterPaletteCommon {
                eye: colors(1, CMP_EYE_LEN),
                highlight: colors(50, CMP_HIGHLIGHT_LEN),
                lip: colors(70, CMP_LIP_EXPORT_LEN),
                feature: colors(1, CMP_FEATURE_LEN),
                face_paint: colors(200, CMP_LIP_EXPORT_LEN),
            },
            tribes,
        }
    }

    #[test]
    fn appearance_colors_resolve_each_channel_with_display_rgb() {
        let palette = fixture_palette();
        let customize = CharacterCustomize {
            race: 8,
            gender: 1,
            tribe: 16,
            skintone: 3,
            hair_tone: 7,
            highlights: 9,
            l_eye_color: 5,
            r_eye_color: 6,
            lips_tone: 11,
            face_paint_color: 13,
            facial_feature_color: 17,
            mouth: 0x80 | 2,
            highlight_type: 0x80,
            face_paint: 0x80 | 26,
            ..Default::default()
        };
        let colors = appearance_colors_from_palette(&customize, &palette);
        let group = customize.palette_group_index();
        // 显示值直接归一（u8/255），不做平方。
        let expect = |base: u8| {
            let channel = |value: u8| f32::from(value) / 255.0;
            [
                channel(base),
                channel(base.wrapping_add(1)),
                channel(base.wrapping_add(2)),
            ]
        };
        // 肤色取 (tribe-1)*2+gender 组的 skin[skintone]。
        let skin_base = 10 + group as u8 + 3;
        assert_eq!(colors.skin[..3], expect(skin_base));
        let hair_base = 100 + group as u8 + 7;
        assert_eq!(colors.main[..3], expect(hair_base));
        assert_eq!(colors.mesh[..3], expect(50 + 9));
        assert_eq!(colors.left_iris[..3], expect(1 + 5));
        assert_eq!(colors.right_iris[..3], expect(1 + 6));
        assert_eq!(colors.lip[..3], expect(70 + 11));
        assert_eq!(colors.option[..3], expect(1 + 17));
        assert_eq!(colors.decal[..3], expect(200 + 13));
        // alpha 通道线性不平方。
        assert_eq!(
            colors.lip[3],
            f32::from(70u8 + 11 + 3) / 255.0,
            "lip alpha keeps the palette alpha linear"
        );
        assert_eq!(
            colors.left_iris[3], 1.0,
            "cornea intensity defaults to full"
        );
        assert!(colors.lipstick, "mouth bit7 = lipstick");
        assert!(colors.highlights, "highlight_type bit7 = highlights");
        assert!(colors.face_paint_reversed, "face paint bit7 = reversed");
        assert_eq!(
            colors.skin[3],
            f32::from(skin_base.wrapping_add(3)) / 255.0,
            "skin W keeps the palette alpha; the renderer sets its own active flag"
        );
    }

    #[test]
    fn appearance_colors_clamp_out_of_range_palette_indices() {
        let palette = fixture_palette();
        let customize = CharacterCustomize {
            tribe: 16,
            gender: 1,
            skintone: 250,
            hair_tone: 250,
            highlights: 250,
            l_eye_color: 250,
            lips_tone: 250,
            face_paint_color: 250,
            ..Default::default()
        };
        let colors = appearance_colors_from_palette(&customize, &palette);
        // 末位色项（saturating 钳制）：肤色组 31 的最后一个色项。
        let group = customize.palette_group_index();
        let channel = |base: u8| f32::from(base) / 255.0;
        let skin_base = 10 + group as u8 + (CMP_EYE_LEN as u8 - 1);
        assert_eq!(
            colors.skin[..3],
            [
                channel(skin_base),
                channel(skin_base + 1),
                channel(skin_base + 2),
            ]
        );
        let hair_base = (100u8)
            .wrapping_add(group as u8)
            .wrapping_add(CMP_HIGHLIGHT_LEN as u8 - 1);
        assert_eq!(
            colors.main[..3],
            [
                channel(hair_base),
                channel(hair_base.wrapping_add(1)),
                channel(hair_base.wrapping_add(2)),
            ]
        );
        assert!(colors.lip[3] <= 1.0);
    }
}
