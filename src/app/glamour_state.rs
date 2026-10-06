use std::collections::{BTreeMap, HashSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const GLAMOUR_STORAGE_KEY: &str = "xiv-companion-glamour-v1";
pub const GLAMOUR_EXPORT_SCHEMA_VERSION: u32 = 1;

/// 幻化套装的装备槽位，按游戏内投影台顺序排列。
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GlamourSlot {
    MainHand,
    OffHand,
    Head,
    Body,
    Hands,
    Legs,
    Feet,
    Earrings,
    Necklace,
    Bracelets,
    RingLeft,
    RingRight,
}

impl GlamourSlot {
    pub const ALL: [GlamourSlot; 12] = [
        GlamourSlot::MainHand,
        GlamourSlot::OffHand,
        GlamourSlot::Head,
        GlamourSlot::Body,
        GlamourSlot::Hands,
        GlamourSlot::Legs,
        GlamourSlot::Feet,
        GlamourSlot::Earrings,
        GlamourSlot::Necklace,
        GlamourSlot::Bracelets,
        GlamourSlot::RingLeft,
        GlamourSlot::RingRight,
    ];

    pub fn label(self) -> &'static str {
        match self {
            GlamourSlot::MainHand => "主手",
            GlamourSlot::OffHand => "副手",
            GlamourSlot::Head => "头部",
            GlamourSlot::Body => "身体",
            GlamourSlot::Hands => "手部",
            GlamourSlot::Legs => "腿部",
            GlamourSlot::Feet => "脚部",
            GlamourSlot::Earrings => "耳饰",
            GlamourSlot::Necklace => "项链",
            GlamourSlot::Bracelets => "手镯",
            GlamourSlot::RingLeft => "戒指（左）",
            GlamourSlot::RingRight => "戒指（右）",
        }
    }

    /// 判断某个 EquipSlotCategory 的装备能否放入该槽位。
    /// 1/13 = 主手武器（单手/双手），2 = 副手，12 = 戒指（左右通用）。
    /// 15-23 等复合部位不属于任何幻化槽位。
    pub fn accepts(self, equip_slot_category: u8) -> bool {
        match self {
            GlamourSlot::MainHand => matches!(equip_slot_category, 1 | 13),
            GlamourSlot::OffHand => equip_slot_category == 2,
            GlamourSlot::Head => equip_slot_category == 3,
            GlamourSlot::Body => equip_slot_category == 4,
            GlamourSlot::Hands => equip_slot_category == 5,
            GlamourSlot::Legs => equip_slot_category == 7,
            GlamourSlot::Feet => equip_slot_category == 8,
            GlamourSlot::Earrings => equip_slot_category == 9,
            GlamourSlot::Necklace => equip_slot_category == 10,
            GlamourSlot::Bracelets => equip_slot_category == 11,
            GlamourSlot::RingLeft | GlamourSlot::RingRight => equip_slot_category == 12,
        }
    }
}

/// 套装中单个槽位的装备记录。除 Item ID 外冗余保存名称、图标与模型快照，
/// 使套装在目录数据更新后仍可显示与渲染。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GlamourPiece {
    pub item_id: u32,
    pub name: String,
    #[serde(default)]
    pub icon: u32,
    #[serde(default)]
    pub equip_slot_category: u8,
    #[serde(default)]
    pub model_main: u64,
    #[serde(default)]
    pub model_sub: u64,
    /// 双通道染色，0 表示未染色。
    #[serde(default, skip_serializing_if = "stains_empty")]
    pub stains: [u8; 2],
}

fn stains_empty(stains: &[u8; 2]) -> bool {
    stains[0] == 0 && stains[1] == 0
}

impl GlamourPiece {
    pub fn stained(self, channel: usize, stain_id: u8) -> Self {
        debug_assert!(channel < 2);
        let mut piece = self;
        piece.stains[channel] = stain_id;
        piece
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GlamourSet {
    pub id: String,
    pub name: String,
    /// 套装独立角色形象（52 字符 hex，同角色页 `?c=` 参数）。None = 跟随页面级
    /// 预览形象（`GlamourState::preview_customize`）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub customize: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub pieces: BTreeMap<GlamourSlot, GlamourPiece>,
    /// 毫秒时间戳，用于列表排序。
    #[serde(default)]
    pub updated_at: u64,
}

impl GlamourSet {
    pub fn piece_count(&self) -> usize {
        self.pieces.len()
    }

    /// 复制套装：换新 id、名称加「（副本）」后缀、刷新更新时间，装备与独立形象等
    /// 其余字段原样保留。
    pub fn duplicate(&self, id: String, updated_at: u64) -> GlamourSet {
        GlamourSet {
            id,
            name: format!("{}（副本）", self.name),
            updated_at,
            ..self.clone()
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GlamourState {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sets: Vec<GlamourSet>,
    /// 预览角色的捏脸（52 字符 hex，同角色页 `?c=` 参数），页面级默认值；
    /// 套装可用 `GlamourSet::customize` 单独绑定。None = 未自定义（预览用捏脸
    /// 菜单默认）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview_customize: Option<String>,
}

impl GlamourState {
    pub fn find_set(&self, set_id: &str) -> Option<&GlamourSet> {
        self.sets.iter().find(|set| set.id == set_id)
    }

    pub fn upsert_set(&mut self, set: GlamourSet) {
        match self.sets.iter_mut().find(|existing| existing.id == set.id) {
            Some(existing) => *existing = set,
            None => self.sets.push(set),
        }
    }

    pub fn remove_set(&mut self, set_id: &str) {
        self.sets.retain(|set| set.id != set_id);
    }
}

pub fn new_glamour_id() -> String {
    let now = js_sys::Date::now().round() as u64;
    let random = (js_sys::Math::random() * 1_000_000_000.0).round() as u32;
    format!("{now:x}-{random:x}")
}

pub fn now_timestamp() -> u64 {
    js_sys::Date::now().round() as u64
}

pub fn load_glamour_state() -> GlamourState {
    let Some(storage) = web_sys::window().and_then(|window| window.local_storage().ok().flatten())
    else {
        return GlamourState::default();
    };
    let Ok(Some(raw)) = storage.get_item(GLAMOUR_STORAGE_KEY) else {
        return GlamourState::default();
    };
    serde_json::from_str::<Value>(&raw)
        .map(normalize_state)
        .unwrap_or_default()
}

pub fn save_glamour_state(state: &GlamourState) {
    if let Some(storage) =
        web_sys::window().and_then(|window| window.local_storage().ok().flatten())
    {
        if let Ok(value) = serde_json::to_string(state) {
            let _ = storage.set_item(GLAMOUR_STORAGE_KEY, &value);
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GlamourExport {
    pub schema_version: u32,
    pub sets: Vec<GlamourSet>,
}

pub fn export_glamour_json(state: &GlamourState) -> Result<String, String> {
    serde_json::to_string_pretty(&GlamourExport {
        schema_version: GLAMOUR_EXPORT_SCHEMA_VERSION,
        sets: state.sets.clone(),
    })
    .map_err(|error| format!("序列化幻化套装失败: {error}"))
}

pub fn import_glamour_json(json: &str) -> Result<Vec<GlamourSet>, String> {
    let export = serde_json::from_str::<GlamourExport>(json)
        .map_err(|error| format!("幻化套装 JSON 格式无效: {error}"))?;
    if export.schema_version != GLAMOUR_EXPORT_SCHEMA_VERSION {
        return Err(format!(
            "不支持的幻化套装 schemaVersion {}，当前支持 {}",
            export.schema_version, GLAMOUR_EXPORT_SCHEMA_VERSION
        ));
    }
    Ok(normalize_sets(export.sets))
}

fn normalize_state(raw: Value) -> GlamourState {
    let parsed = raw
        .get("sets")
        .and_then(Value::as_array)
        .map(|sets| {
            sets.iter()
                .filter_map(|value| serde_json::from_value::<GlamourSet>(value.clone()).ok())
                .collect()
        })
        .unwrap_or_default();
    GlamourState {
        sets: normalize_sets(parsed),
        preview_customize: normalize_customize_hex(
            raw.get("previewCustomize").and_then(Value::as_str),
        ),
    }
}

/// 捏脸 hex 的轻量形状校验（52 字符 hex = 26 字节捏脸 × 2；逐字节语义校验在
/// 页面层 `customize_from_hex` 进行）。页面级 `preview_customize` 与套装级
/// `customize` 共用。
fn normalize_customize_hex(value: Option<&str>) -> Option<String> {
    const PREVIEW_CUSTOMIZE_HEX_LEN: usize = 52;
    let value = value?.trim();
    (value.len() == PREVIEW_CUSTOMIZE_HEX_LEN && value.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .then(|| value.to_string())
}

fn normalize_sets(sets: Vec<GlamourSet>) -> Vec<GlamourSet> {
    let mut seen = HashSet::new();
    let mut result = Vec::with_capacity(sets.len());
    for mut set in sets {
        if set.id.is_empty() || !seen.insert(set.id.clone()) {
            continue;
        }
        set.name = set.name.trim().to_string();
        if set.name.is_empty() {
            set.name = "未命名套装".to_string();
        }
        set.customize = normalize_customize_hex(set.customize.as_deref());
        set.pieces.retain(|_, piece| piece.item_id != 0);
        set.pieces
            .retain(|slot, piece| slot.accepts(piece.equip_slot_category));
        result.push(set);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn piece(item_id: u32, equip_slot_category: u8) -> GlamourPiece {
        GlamourPiece {
            item_id,
            name: format!("装备 {item_id}"),
            icon: 0,
            equip_slot_category,
            model_main: 0,
            model_sub: 0,
            stains: [0, 0],
        }
    }

    fn set(id: &str, name: &str) -> GlamourSet {
        GlamourSet {
            id: id.to_string(),
            name: name.to_string(),
            customize: None,
            pieces: BTreeMap::new(),
            updated_at: 0,
        }
    }

    #[test]
    fn slot_accepts_matching_equip_slot_category() {
        assert!(GlamourSlot::MainHand.accepts(1));
        assert!(GlamourSlot::MainHand.accepts(13));
        assert!(GlamourSlot::OffHand.accepts(2));
        assert!(GlamourSlot::Body.accepts(4));
        assert!(GlamourSlot::RingLeft.accepts(12));
        assert!(GlamourSlot::RingRight.accepts(12));
        assert!(!GlamourSlot::MainHand.accepts(2));
        assert!(!GlamourSlot::Head.accepts(5));
        // 复合部位（15-23）没有对应的幻化槽位
        assert!(!GlamourSlot::ALL.iter().any(|slot| slot.accepts(15)));
        assert!(!GlamourSlot::ALL.iter().any(|slot| slot.accepts(23)));
    }

    #[test]
    fn slot_keys_serialize_in_camel_case() {
        let mut set = set("a", "测试");
        set.pieces.insert(GlamourSlot::MainHand, piece(10, 1));
        let json = serde_json::to_string(&set).unwrap();
        assert!(json.contains("\"mainHand\""));
        let parsed: GlamourSet = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, set);
    }

    #[test]
    fn unstained_pieces_omit_stains_from_json() {
        let mut clean = piece(10, 4);
        let json = serde_json::to_string(&clean).unwrap();
        assert!(!json.contains("stains"));
        clean.stains = [3, 0];
        let json = serde_json::to_string(&clean).unwrap();
        assert!(json.contains("\"stains\":[3,0]"));
    }

    #[test]
    fn export_import_round_trips() {
        let mut state = GlamourState::default();
        let mut first = set("a", "套装 A");
        first
            .pieces
            .insert(GlamourSlot::Body, piece(20, 4).stained(0, 5));
        state.sets.push(first);
        state.sets.push(set("b", "套装 B"));

        let json = export_glamour_json(&state).unwrap();
        let imported = import_glamour_json(&json).unwrap();
        assert_eq!(imported, state.sets);
    }

    #[test]
    fn import_rejects_unknown_schema_version() {
        assert!(import_glamour_json(r#"{"schemaVersion":99,"sets":[]}"#).is_err());
    }

    #[test]
    fn preview_customize_round_trips_serde() {
        let mut state = GlamourState::default();
        let json = serde_json::to_string(&state).unwrap();
        assert!(!json.contains("previewCustomize"));
        state.preview_customize = Some("0".repeat(52));
        let json = serde_json::to_string(&state).unwrap();
        assert!(json.contains("\"previewCustomize\""));
        let parsed: GlamourState = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.preview_customize, Some("0".repeat(52)));
    }

    #[test]
    fn normalize_validates_preview_customize_hex_shape() {
        let valid = "a1".repeat(26);
        let state = normalize_state(serde_json::json!({
            "sets": [],
            "previewCustomize": valid,
        }));
        assert_eq!(state.preview_customize, Some(valid.clone()));
        // 大写 hex 保留原文（语义校验在页面层）。
        let upper = "AB".repeat(26);
        let state = normalize_state(serde_json::json!({"previewCustomize": upper}));
        assert_eq!(state.preview_customize, Some(upper));
        // 长度错误 / 非 hex / 缺键 → None；sets 缺键不再丢弃预览捏脸。
        let too_long = format!("{valid}ff");
        let non_hex = "zz".repeat(26);
        for bad in ["a1", too_long.as_str(), non_hex.as_str()] {
            let state = normalize_state(serde_json::json!({"previewCustomize": bad}));
            assert_eq!(state.preview_customize, None, "应拒绝: {bad}");
        }
        let state = normalize_state(serde_json::json!({}));
        assert_eq!(state.preview_customize, None);
    }

    #[test]
    fn set_customize_round_trips_serde() {
        let mut set = set("a", "测试");
        let json = serde_json::to_string(&set).unwrap();
        assert!(!json.contains("customize"));
        set.customize = Some("a1".repeat(26));
        let json = serde_json::to_string(&set).unwrap();
        assert!(json.contains("\"customize\""));
        let parsed: GlamourSet = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, set);
    }

    #[test]
    fn normalize_validates_set_customize_hex_shape() {
        let mut valid_set = set("a", "有效");
        valid_set.customize = Some("a1".repeat(26));
        let mut upper_set = set("b", "大写保留原文");
        upper_set.customize = Some("AB".repeat(26));
        let mut too_long = set("c", "长度错误");
        too_long.customize = Some("a1".repeat(27));
        let mut non_hex = set("d", "非 hex");
        non_hex.customize = Some("zz".repeat(26));

        let normalized = normalize_sets(vec![
            valid_set.clone(),
            upper_set.clone(),
            too_long,
            non_hex,
        ]);
        assert_eq!(normalized[0].customize, valid_set.customize);
        assert_eq!(normalized[1].customize, upper_set.customize);
        assert_eq!(normalized[2].customize, None);
        assert_eq!(normalized[3].customize, None);
    }

    #[test]
    fn import_v1_export_without_customize_field() {
        let imported = import_glamour_json(
            r#"{"schemaVersion":1,"sets":[{"id":"a","name":"套装","updatedAt":1}]}"#,
        )
        .unwrap();
        assert_eq!(imported.len(), 1);
        assert_eq!(imported[0].customize, None);
    }

    #[test]
    fn export_import_carries_set_customize() {
        let mut state = GlamourState::default();
        let mut with_customize = set("a", "绑定形象");
        with_customize.customize = Some("a1".repeat(26));
        state.sets.push(with_customize);
        state.sets.push(set("b", "跟随页面"));

        let json = export_glamour_json(&state).unwrap();
        let imported = import_glamour_json(&json).unwrap();
        assert_eq!(imported, state.sets);
        assert_eq!(imported[0].customize, Some("a1".repeat(26)));
        assert_eq!(imported[1].customize, None);
    }

    #[test]
    fn import_drops_invalid_set_customize() {
        let imported = import_glamour_json(
            r#"{"schemaVersion":1,"sets":[{"id":"a","name":"套装","customize":"zz","updatedAt":1}]}"#,
        )
        .unwrap();
        assert_eq!(imported[0].customize, None);
    }

    #[test]
    fn duplicate_keeps_customize_override() {
        let mut original = set("a", "套装");
        original.customize = Some("a1".repeat(26));
        original.pieces.insert(GlamourSlot::Body, piece(20, 4));
        let copy = original.duplicate("b".to_string(), 42);
        assert_eq!(copy.id, "b");
        assert_eq!(copy.name, "套装（副本）");
        assert_eq!(copy.updated_at, 42);
        assert_eq!(copy.customize, original.customize);
        assert_eq!(copy.pieces, original.pieces);
    }

    #[test]
    fn normalize_drops_invalid_entries() {
        let mut duplicate = set("a", "重复 id");
        duplicate.pieces.insert(GlamourSlot::Head, piece(30, 3));
        let mut mismatched = set("b", "槽位不匹配");
        mismatched.pieces.insert(GlamourSlot::Head, piece(31, 4));
        let mut zero_item = set("c", "无效物品");
        zero_item.pieces.insert(GlamourSlot::Feet, piece(0, 8));
        let empty_name = set("d", "  ");

        let normalized = normalize_sets(vec![
            duplicate.clone(),
            duplicate,
            mismatched,
            zero_item,
            empty_name,
        ]);
        assert_eq!(normalized.len(), 4);
        assert_eq!(normalized[0].pieces.len(), 1);
        assert_eq!(normalized[1].pieces.len(), 0);
        assert_eq!(normalized[2].pieces.len(), 0);
        assert_eq!(normalized[3].name, "未命名套装");
    }
}
