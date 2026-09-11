//! 宠物（minion）/坐骑（mount）模型与目录的数据层基础。
//!
//! 语义来源（均已核实）：
//! - EXD 链路：解锁物品 → ItemAction（Type 区分，Data[0]=目标行）→ Companion
//!   （宠物）/Mount（坐骑）行 → 第 8 列 ModelChara 链接 → ModelChara 行
//!   （Type/Model/Base/Variant）。Type 2=demihuman、3=monster，其余没有可用
//!   模型（xivModdingFramework `XivModelChara.GetModelInfo`）。
//! - 模型路径：xivModdingFramework `Mdl.GetMdlPath` 的 monster/demihuman 分支。
//! - 材质路径：`Mtrl.GetMtrlPath` 无 monster/demihuman 特例，走通用分支
//!   `{mdl_dir}/../material/v{materialSet}` + MDL 内嵌材质名，文件名形式
//!   `mt_m{model}b{base}_x` / `mt_d{model}e{base}_{slot}_x`。

use serde::{Deserialize, Serialize};

pub const CHARA_CATALOG_SCHEMA_VERSION: u32 = 1;

/// ItemAction Type：宠物解锁（853）与坐骑解锁（1322）。与
/// `collection_classification.rs` 的分类规则（Mount=1322 / Minion=853）一致。
pub const MINION_ITEM_ACTION_TYPE: u32 = 853;
pub const MOUNT_ITEM_ACTION_TYPE: u32 = 1_322;

/// 模型类别：宠物或坐骑。决定目录条目归属与展示文案。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CharaModelKind {
    Minion,
    Mount,
}

impl CharaModelKind {
    pub fn id(self) -> &'static str {
        match self {
            Self::Minion => "minion",
            Self::Mount => "mount",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Minion => "宠物",
            Self::Mount => "坐骑",
        }
    }

    /// 该类别对应的 ItemAction Type。
    pub fn item_action_type(self) -> u32 {
        match self {
            Self::Minion => MINION_ITEM_ACTION_TYPE,
            Self::Mount => MOUNT_ITEM_ACTION_TYPE,
        }
    }
}

/// ModelChara Type 映射出的模型种类（Type 2=demihuman，3=monster）。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CharaModelType {
    Demihuman,
    #[default]
    Monster,
}

/// 宠物/坐骑模型标识：ModelChara 的 Model/Base/Variant 三元组加模型种类。
/// monster 是单 MDL（m{model}b{base}），demihuman 按装备槽位有多 MDL
/// （d{model}e{base}_{slot}），`variant_id` 是材质版本 v#### 的首选猜测。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackedCharaModelId {
    pub model_id: u16,
    pub base_id: u16,
    pub variant_id: u16,
    pub chara_type: CharaModelType,
}

/// ModelChara EXD 行（CSV 解析在 xtask 侧）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CharaModelCharaRow {
    pub id: u32,
    pub type_id: u8,
    pub model_id: u16,
    pub base_id: u16,
    pub variant_id: u16,
}

impl CharaModelCharaRow {
    /// Type 2/3 映射为 demihuman/monster，其余 Type 没有可用模型（对齐
    /// xivModdingFramework `XivModelChara.GetModelInfo` 的 unknown 分支）。
    pub fn packed_model(&self) -> Option<PackedCharaModelId> {
        let chara_type = match self.type_id {
            2 => CharaModelType::Demihuman,
            3 => CharaModelType::Monster,
            _ => return None,
        };
        (self.model_id != 0).then_some(PackedCharaModelId {
            model_id: self.model_id,
            base_id: self.base_id,
            variant_id: self.variant_id,
            chara_type,
        })
    }
}

/// demihuman 模型按这些装备槽位后缀逐个探测，存在的 MDL 全部合并
/// （依据 xivModdingFramework `Imc.EquipmentSlotOffsetDictionary` 的槽位集，
/// 与 `Mdl.GetMdlPath` demihuman 分支使用的 SlotAbbreviationDictionary）。
pub const DEMIHUMAN_SLOT_ABBREVIATIONS: &[&str] = &["met", "top", "glv", "dwn", "sho"];

/// 模型候选路径：monster 单 MDL；demihuman 返回全部槽位候选（不存在的槽位
/// 由加载方探测跳过）。
pub fn chara_model_candidate_paths(model: PackedCharaModelId) -> Vec<String> {
    if model.model_id == 0 {
        return Vec::new();
    }
    match model.chara_type {
        CharaModelType::Monster => vec![format!(
            "chara/monster/m{model_id:04}/obj/body/b{base_id:04}/model/m{model_id:04}b{base_id:04}.mdl",
            model_id = model.model_id,
            base_id = model.base_id,
        )],
        CharaModelType::Demihuman => DEMIHUMAN_SLOT_ABBREVIATIONS
            .iter()
            .map(|slot| {
                format!(
                    "chara/demihuman/d{model_id:04}/obj/equipment/e{base_id:04}/model/d{model_id:04}e{base_id:04}_{slot}.mdl",
                    model_id = model.model_id,
                    base_id = model.base_id,
                )
            })
            .collect(),
    }
}

/// 材质候选路径。MDL 内嵌材质名优先原样尝试；随后按模型路径推导
/// `{obj_root}/material` 根，再从材质文件名反推 monster/demihuman 跨 id
/// 根目录（mt_m{id}b{base} / mt_d{id}e{base}），版本目录按
/// [variant, base, 1, 101, 201] 回退（对齐武器的多版本风格；真实版本由
/// 各模型 imc 的 MaterialSet 决定，这里不解析 imc）。
pub fn chara_material_candidate_paths(
    model: PackedCharaModelId,
    model_path: &str,
    material_name: &str,
) -> Vec<String> {
    let mut candidates = Vec::new();
    let normalized_name = normalize_chara_resource_path(material_name);
    if normalized_name.is_empty() {
        return candidates;
    }

    push_unique_path(&mut candidates, normalized_name.clone());
    if normalized_name.starts_with("chara/") {
        return candidates;
    }

    let normalized_model_path = normalize_chara_resource_path(model_path);
    let Some((object_root, _)) = normalized_model_path.split_once("/model/") else {
        return candidates;
    };
    let material_root = format!("{object_root}/material");

    if normalized_name.starts_with('v') {
        push_unique_path(
            &mut candidates,
            format!("{material_root}/{normalized_name}"),
        );
    }

    let material_file = normalized_name
        .rsplit('/')
        .next()
        .unwrap_or(normalized_name.as_str());
    let mut material_roots = vec![material_root];
    if let Some(chara_root) = chara_root_from_material_file(material_file) {
        push_unique_path(&mut material_roots, format!("{chara_root}/material"));
    }

    let mut versions = Vec::new();
    for version in [model.variant_id, model.base_id, 1, 101, 201] {
        if version != 0 && !versions.contains(&version) {
            versions.push(version);
        }
    }

    for material_root in material_roots {
        for version in &versions {
            push_unique_path(
                &mut candidates,
                format!("{material_root}/v{version:04}/{material_file}"),
            );
        }
        push_unique_path(&mut candidates, format!("{material_root}/{material_file}"));
    }
    candidates
}

/// 从材质文件名反推模型根目录：`mt_m{model}b{base}_...` → monster，
/// `mt_d{model}e{base}_...` → demihuman（共享/跨引用的材质与模型不同 id 时
/// 需要，对齐武器材质候选的跨 id 回退）。
fn chara_root_from_material_file(material_file: &str) -> Option<String> {
    let tail = material_file.strip_prefix("mt_")?;
    let (root, obj_dir, tail) = match tail.split_at_checked(1)? {
        ("m", rest) => ("chara/monster/m", "body/b", rest),
        ("d", rest) => ("chara/demihuman/d", "equipment/e", rest),
        _ => return None,
    };
    let (model_id, tail) = tail.split_at_checked(4)?;
    let (separator, tail) = tail.split_at_checked(1)?;
    let (base_id, _) = tail.split_at_checked(4)?;
    let ids_are_digits = [model_id, base_id]
        .iter()
        .all(|id| id.bytes().all(|byte| byte.is_ascii_digit()));
    let separator_matches = (root == "chara/monster/m" && separator == "b")
        || (root == "chara/demihuman/d" && separator == "e");
    (ids_are_digits && separator_matches)
        .then(|| format!("{root}{model_id}/obj/{obj_dir}{base_id}"))
}

/// ItemAction EXD 行（CSV 解析在 xtask 侧）：key 是 **ItemAction 行 id**
/// （不是物品 id；物品经 Item.ItemAction 列链接到这里），Type 区分解锁
/// 类别，`target_id` 即 Data[0]（Companion/Mount 行 key，0=无目标）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CharaActionRow {
    pub action_id: u32,
    pub action_type: u32,
    pub target_id: u32,
}

/// 物品 → ItemAction 行的链接（来自 Item.ItemAction 列；xtask 侧从
/// collection-catalog.json 读取）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CharaUnlockItemRow {
    pub item_id: u32,
    pub action_id: u32,
}

/// Companion/Mount EXD 行：`id` 是行 key，`model_chara_id` 是第 8 列的
/// ModelChara 链接。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CharaModelLinkRow {
    pub id: u32,
    pub model_chara_id: u32,
}

/// 目录构建时按物品 id 查询的展示信息（来自 craft-data）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CharaItemInfo {
    pub name: String,
    pub icon: u32,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CharaCatalogPackage {
    pub schema_version: u32,
    pub generated_at: String,
    pub game_version: String,
    pub source: String,
    pub counts: CharaCatalogCounts,
    pub items: Vec<CharaCatalogItem>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CharaCatalogCounts {
    pub items: usize,
    pub minions: usize,
    pub mounts: usize,
    /// 宠物/坐骑类 ItemAction 的 Data[0] 为 0（无目标行）而被跳过的行数。
    pub skipped_empty_targets: usize,
    /// ModelChara 链接缺失或 Type 非 2/3（无可用模型）而被跳过的行数。
    pub skipped_unsupported_models: usize,
    /// 物品 id 在 craft-data 中查不到（无法补全名称/图标）而被跳过的行数。
    pub skipped_missing_items: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CharaCatalogItem {
    pub id: u32,
    pub kind: CharaModelKind,
    pub name: String,
    pub icon: u32,
    pub model: PackedCharaModelId,
}

/// 由 EXD 行构建宠物/坐骑目录：物品经 ItemAction 链接行按 Type 853/1322
/// 分流到 Companion/Mount 链接表，ModelChara 行给出模型四元组；物品名/图标
/// 经 `item_info` 按物品 id 查询，查不到的行跳过并计数。条目按 (kind, id)
/// 排序。
pub fn build_chara_catalog(
    actions: &[CharaActionRow],
    unlock_items: &[CharaUnlockItemRow],
    companions: &[CharaModelLinkRow],
    mounts: &[CharaModelLinkRow],
    model_charas: &[CharaModelCharaRow],
    item_info: impl Fn(u32) -> Option<CharaItemInfo>,
    generated_at: String,
    game_version: String,
    source: String,
) -> CharaCatalogPackage {
    let action_map = actions
        .iter()
        .map(|row| (row.action_id, *row))
        .collect::<std::collections::HashMap<_, _>>();
    let model_chara_map = model_charas
        .iter()
        .map(|row| (row.id, *row))
        .collect::<std::collections::HashMap<_, _>>();
    let link_map = |rows: &[CharaModelLinkRow]| {
        rows.iter()
            .map(|row| (row.id, row.model_chara_id))
            .collect::<std::collections::HashMap<_, _>>()
    };
    let companion_map = link_map(companions);
    let mount_map = link_map(mounts);

    let mut counts = CharaCatalogCounts::default();
    let mut items = Vec::new();
    for unlock in unlock_items {
        let Some(action) = action_map.get(&unlock.action_id) else {
            continue;
        };
        let (kind, links) = match action.action_type {
            MINION_ITEM_ACTION_TYPE => (CharaModelKind::Minion, &companion_map),
            MOUNT_ITEM_ACTION_TYPE => (CharaModelKind::Mount, &mount_map),
            _ => continue,
        };
        if action.target_id == 0 {
            counts.skipped_empty_targets += 1;
            continue;
        }
        let model = links
            .get(&action.target_id)
            .and_then(|model_chara_id| model_chara_map.get(model_chara_id))
            .and_then(CharaModelCharaRow::packed_model);
        let Some(model) = model else {
            counts.skipped_unsupported_models += 1;
            continue;
        };
        let Some(info) = item_info(unlock.item_id) else {
            counts.skipped_missing_items += 1;
            continue;
        };
        items.push(CharaCatalogItem {
            id: unlock.item_id,
            kind,
            name: info.name,
            icon: info.icon,
            model,
        });
    }
    items.sort_by_key(|item| (item.kind, item.id));

    counts.items = items.len();
    counts.minions = items
        .iter()
        .filter(|item| item.kind == CharaModelKind::Minion)
        .count();
    counts.mounts = counts.items - counts.minions;
    CharaCatalogPackage {
        schema_version: CHARA_CATALOG_SCHEMA_VERSION,
        generated_at,
        game_version,
        source,
        counts,
        items,
    }
}

fn normalize_chara_resource_path(path: &str) -> String {
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

fn push_unique_path(paths: &mut Vec<String>, path: String) {
    if !path.is_empty() && !paths.iter().any(|existing| existing == &path) {
        paths.push(path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_chara_rows_map_only_types_2_and_3() {
        // 爆弹仔：ModelChara 行 427 → Type=3, Model=8003, Base=1, Variant=1。
        let row = CharaModelCharaRow {
            id: 427,
            type_id: 3,
            model_id: 8003,
            base_id: 1,
            variant_id: 1,
        };
        assert_eq!(
            row.packed_model(),
            Some(PackedCharaModelId {
                model_id: 8003,
                base_id: 1,
                variant_id: 1,
                chara_type: CharaModelType::Monster,
            })
        );
        let demihuman = CharaModelCharaRow {
            id: 1,
            type_id: 2,
            model_id: 1,
            base_id: 1,
            variant_id: 1,
        };
        assert_eq!(
            demihuman.packed_model().map(|model| model.chara_type),
            Some(CharaModelType::Demihuman)
        );
        for type_id in [0u8, 1, 4, 5] {
            assert_eq!(
                CharaModelCharaRow {
                    type_id,
                    ..demihuman
                }
                .packed_model(),
                None
            );
        }
        assert_eq!(
            CharaModelCharaRow {
                model_id: 0,
                ..demihuman
            }
            .packed_model(),
            None
        );
    }

    #[test]
    fn monster_model_path_is_a_single_mdl() {
        let model = PackedCharaModelId {
            model_id: 8003,
            base_id: 1,
            variant_id: 1,
            chara_type: CharaModelType::Monster,
        };
        assert_eq!(
            chara_model_candidate_paths(model),
            ["chara/monster/m8003/obj/body/b0001/model/m8003b0001.mdl"]
        );
    }

    #[test]
    fn demihuman_model_paths_cover_all_equipment_slots() {
        // 专属陆行鸟：ModelChara 行 1 → Type=2, Model=1, Base=1。
        let model = PackedCharaModelId {
            model_id: 1,
            base_id: 1,
            variant_id: 1,
            chara_type: CharaModelType::Demihuman,
        };
        assert_eq!(
            chara_model_candidate_paths(model),
            [
                "chara/demihuman/d0001/obj/equipment/e0001/model/d0001e0001_met.mdl",
                "chara/demihuman/d0001/obj/equipment/e0001/model/d0001e0001_top.mdl",
                "chara/demihuman/d0001/obj/equipment/e0001/model/d0001e0001_glv.mdl",
                "chara/demihuman/d0001/obj/equipment/e0001/model/d0001e0001_dwn.mdl",
                "chara/demihuman/d0001/obj/equipment/e0001/model/d0001e0001_sho.mdl",
            ]
        );
        assert!(
            chara_model_candidate_paths(PackedCharaModelId {
                model_id: 0,
                ..model
            })
            .is_empty()
        );
    }

    #[test]
    fn monster_material_candidates_use_variant_then_fallback_versions() {
        let model = PackedCharaModelId {
            model_id: 8003,
            base_id: 1,
            variant_id: 2,
            chara_type: CharaModelType::Monster,
        };
        let candidates = chara_material_candidate_paths(
            model,
            "chara/monster/m8003/obj/body/b0001/model/m8003b0001.mdl",
            "/mt_m8003b0001_a.mtrl",
        );
        assert_eq!(
            candidates,
            [
                "mt_m8003b0001_a.mtrl",
                "chara/monster/m8003/obj/body/b0001/material/v0002/mt_m8003b0001_a.mtrl",
                "chara/monster/m8003/obj/body/b0001/material/v0001/mt_m8003b0001_a.mtrl",
                "chara/monster/m8003/obj/body/b0001/material/v0101/mt_m8003b0001_a.mtrl",
                "chara/monster/m8003/obj/body/b0001/material/v0201/mt_m8003b0001_a.mtrl",
                "chara/monster/m8003/obj/body/b0001/material/mt_m8003b0001_a.mtrl",
            ]
        );
    }

    #[test]
    fn chara_material_candidates_follow_material_file_model_id() {
        // 材质名引用别的模型 id 时（共享材质），按文件名反推根目录。
        let model = PackedCharaModelId {
            model_id: 8003,
            base_id: 1,
            variant_id: 1,
            chara_type: CharaModelType::Monster,
        };
        let candidates = chara_material_candidate_paths(
            model,
            "chara/monster/m8003/obj/body/b0001/model/m8003b0001.mdl",
            "/mt_m8004b0001_a.mtrl",
        );
        assert!(candidates.contains(
            &"chara/monster/m8004/obj/body/b0001/material/v0001/mt_m8004b0001_a.mtrl".to_string()
        ));
    }

    #[test]
    fn chara_material_candidates_keep_absolute_paths() {
        let model = PackedCharaModelId {
            model_id: 1,
            base_id: 1,
            variant_id: 1,
            chara_type: CharaModelType::Demihuman,
        };
        let candidates = chara_material_candidate_paths(
            model,
            "chara/demihuman/d0001/obj/equipment/e0001/model/d0001e0001_top.mdl",
            "chara/common/texture/whatever.tex",
        );
        assert_eq!(candidates, ["chara/common/texture/whatever.tex"]);
        assert!(chara_material_candidate_paths(model, "chara/x/model/y.mdl", "").is_empty());
    }

    fn test_item_info(
        items: &'static [(u32, &'static str, u32)],
    ) -> impl Fn(u32) -> Option<CharaItemInfo> {
        move |id| {
            items
                .iter()
                .find(|(item_id, _, _)| *item_id == id)
                .map(|(_, name, icon)| CharaItemInfo {
                    name: name.to_string(),
                    icon: *icon,
                })
        }
    }

    #[test]
    fn chara_catalog_joins_actions_links_and_model_charas() {
        let catalog = build_chara_catalog(
            &[
                CharaActionRow {
                    action_id: 252,
                    action_type: MINION_ITEM_ACTION_TYPE,
                    target_id: 1,
                },
                CharaActionRow {
                    action_id: 324,
                    action_type: MOUNT_ITEM_ACTION_TYPE,
                    target_id: 4,
                },
                CharaActionRow {
                    action_id: 400,
                    action_type: MINION_ITEM_ACTION_TYPE,
                    target_id: 0,
                },
                CharaActionRow {
                    action_id: 401,
                    action_type: MINION_ITEM_ACTION_TYPE,
                    target_id: 2,
                },
                CharaActionRow {
                    action_id: 402,
                    action_type: MOUNT_ITEM_ACTION_TYPE,
                    target_id: 4,
                },
            ],
            &[
                CharaUnlockItemRow {
                    item_id: 100,
                    action_id: 252,
                },
                CharaUnlockItemRow {
                    item_id: 200,
                    action_id: 324,
                },
                // 其他 Type 的 ItemAction 直接过滤，不计数。
                CharaUnlockItemRow {
                    item_id: 300,
                    action_id: 999,
                },
                // 没有 ItemAction 链接（action_id=0）的物品同样过滤。
                CharaUnlockItemRow {
                    item_id: 301,
                    action_id: 0,
                },
                // Data[0]=0 跳过并计数。
                CharaUnlockItemRow {
                    item_id: 101,
                    action_id: 400,
                },
                // ModelChara Type 非 2/3 跳过并计数。
                CharaUnlockItemRow {
                    item_id: 102,
                    action_id: 401,
                },
                // 物品信息缺失跳过并计数。
                CharaUnlockItemRow {
                    item_id: 404,
                    action_id: 402,
                },
            ],
            &[
                CharaModelLinkRow {
                    id: 1,
                    model_chara_id: 427,
                },
                CharaModelLinkRow {
                    id: 2,
                    model_chara_id: 10,
                },
            ],
            &[CharaModelLinkRow {
                id: 4,
                model_chara_id: 200,
            }],
            &[
                CharaModelCharaRow {
                    id: 427,
                    type_id: 3,
                    model_id: 8003,
                    base_id: 1,
                    variant_id: 1,
                },
                CharaModelCharaRow {
                    id: 10,
                    type_id: 0,
                    model_id: 1,
                    base_id: 1,
                    variant_id: 1,
                },
                CharaModelCharaRow {
                    id: 200,
                    type_id: 2,
                    model_id: 4,
                    base_id: 1,
                    variant_id: 1,
                },
            ],
            test_item_info(&[(100, "爆弹仔", 2598), (200, "古菩猩猩", 3801)]),
            "2026-09-11T00:00:00Z".to_string(),
            "game-2026.06.18.0000.0000".to_string(),
            "test".to_string(),
        );

        assert_eq!(catalog.schema_version, CHARA_CATALOG_SCHEMA_VERSION);
        assert_eq!(
            catalog.counts,
            CharaCatalogCounts {
                items: 2,
                minions: 1,
                mounts: 1,
                skipped_empty_targets: 1,
                skipped_unsupported_models: 1,
                skipped_missing_items: 1,
            }
        );
        assert_eq!(
            catalog.items,
            [
                CharaCatalogItem {
                    id: 100,
                    kind: CharaModelKind::Minion,
                    name: "爆弹仔".to_string(),
                    icon: 2598,
                    model: PackedCharaModelId {
                        model_id: 8003,
                        base_id: 1,
                        variant_id: 1,
                        chara_type: CharaModelType::Monster,
                    },
                },
                CharaCatalogItem {
                    id: 200,
                    kind: CharaModelKind::Mount,
                    name: "古菩猩猩".to_string(),
                    icon: 3801,
                    model: PackedCharaModelId {
                        model_id: 4,
                        base_id: 1,
                        variant_id: 1,
                        chara_type: CharaModelType::Demihuman,
                    },
                },
            ]
        );
    }
}
