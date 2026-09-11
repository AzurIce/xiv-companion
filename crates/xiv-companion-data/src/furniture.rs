//! 家具/庭具（housing）模型与目录的数据层基础。
//!
//! 语义来源（均已核实）：
//! - EXD→物品映射：ffxiv-datamining-cn `HousingFurniture.csv`（室内家具，行 key
//!   从 196608 起，Item 在第 7 列）与 `HousingYardObject.csv`（庭具，行 key 从
//!   131072 起，Item 在第 6 列）；`ModelKey` 列即模型 id，Item 为 0 的行无物品。
//! - 模型路径与 SGB 字符串区提取：xivModdingFramework `Housing.cs`
//!   （`GetFurnitureAssets` / `GetAdditionalAssets`）。家具模型不是直接的 MDL，
//!   而是 SGB 资源，内部字符串区列出引用的 MDL、关联 SGB 与其他文件
//!   （.mtrl/.tex 等）。

use serde::{Deserialize, Serialize};

pub const FURNITURE_CATALOG_SCHEMA_VERSION: u32 = 1;

/// 家具模型类别：室内家具或庭具。决定 SGB 根路径与目录条目归属。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FurnitureModelKind {
    Indoor,
    Outdoor,
}

impl FurnitureModelKind {
    pub fn id(self) -> &'static str {
        match self {
            Self::Indoor => "indoor",
            Self::Outdoor => "outdoor",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Indoor => "室内家具",
            Self::Outdoor => "庭具",
        }
    }
}

/// 家具/庭具的 SGB 资源路径（依据 xivModdingFramework `Housing.GetFurnitureAssets`）。
/// `model_key` 是 HousingFurniture/HousingYardObject 的 ModelKey。
pub fn furniture_sgb_path(kind: FurnitureModelKind, model_key: u16) -> String {
    let (root, file_prefix) = match kind {
        FurnitureModelKind::Indoor => ("bgcommon/hou/indoor/general", "fun_b0_m"),
        FurnitureModelKind::Outdoor => ("bgcommon/hou/outdoor/general", "gar_b0_m"),
    };
    format!("{root}/{model_key:04}/asset/{file_prefix}{model_key:04}.sgb")
}

/// SGB 字符串区提取出的资源路径，按扩展名分类。路径统一为小写正斜杠。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SgbAssetPaths {
    /// 含 `.mdl` 的路径：家具本体与替换部件模型。
    pub models: Vec<String>,
    /// 含 `.sgb` 的路径：关联 SGB，需要递归提取。
    pub related_sgbs: Vec<String>,
    /// 其余含 `.` 的路径：.mtrl/.tex 等引用文件。
    pub others: Vec<String>,
}

/// 从 SGB 字节中提取资源路径（xivModdingFramework `Housing.GetAdditionalAssets`
/// 逐字逻辑）：`seek(20)` 读 i32 加 20 得 `skip`，`seek(skip+4)` 读 i32 得
/// `strings_offset`，`seek(skip+strings_offset)` 起循环读 NUL 结尾的 ASCII 字符串，
/// 遇到 0xFF 字节终止；空字符串跳过，不含 `.` 的字符串忽略。损坏/截断的输入
/// 返回已提取部分而不是报错。
pub fn extract_sgb_asset_paths(bytes: &[u8]) -> SgbAssetPaths {
    let mut paths = SgbAssetPaths::default();
    let Some(skip) = read_i32_le(bytes, 20) else {
        return paths;
    };
    let skip = skip as usize + 20;
    let Some(strings_offset) = read_i32_le(bytes, skip + 4) else {
        return paths;
    };
    let mut cursor = skip + strings_offset as usize;

    while cursor < bytes.len() {
        let string_start = cursor;
        let mut terminated = false;
        while cursor < bytes.len() {
            let byte = bytes[cursor];
            cursor += 1;
            if byte == 0 {
                terminated = true;
                break;
            }
            if byte == 0xFF {
                // 0xFF 终止整个字符串区，当前未完成的字符串一并丢弃。
                return paths;
            }
        }
        if !terminated {
            break;
        }

        let path = normalize_furniture_resource_path(&String::from_utf8_lossy(
            &bytes[string_start..cursor - 1],
        ));
        if path.is_empty() {
            continue;
        }
        if path.contains(".mdl") {
            push_unique_path(&mut paths.models, path);
        } else if path.contains(".sgb") {
            push_unique_path(&mut paths.related_sgbs, path);
        } else if path.contains('.') {
            push_unique_path(&mut paths.others, path);
        }
    }
    paths
}

fn read_i32_le(bytes: &[u8], offset: usize) -> Option<usize> {
    let raw = bytes.get(offset..offset + 4)?;
    let value = i32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]);
    usize::try_from(value).ok()
}

/// 家具材质候选路径：SGB 给出的 MDL 路径是精确路径，材质优先按 SGB 引用文件
/// 列表（`sgb_files`，即 [`SgbAssetPaths::others`]）以文件名精确匹配；缺失时按
/// MDL 所在目录推导 `{object_root}/material/...` 兜底。
pub fn furniture_material_candidate_paths(
    model_path: &str,
    material_name: &str,
    sgb_files: &[String],
) -> Vec<String> {
    let mut candidates = Vec::new();
    let normalized_name = normalize_furniture_resource_path(material_name);
    if normalized_name.is_empty() {
        return candidates;
    }

    // 材质名本身可能已是完整路径（bg 模型常见），优先原样尝试。
    push_unique_path(&mut candidates, normalized_name.clone());
    let material_file = normalized_name
        .rsplit('/')
        .next()
        .unwrap_or(normalized_name.as_str());

    for sgb_file in sgb_files {
        let normalized = normalize_furniture_resource_path(sgb_file);
        if normalized.rsplit('/').next() == Some(material_file) {
            push_unique_path(&mut candidates, normalized);
        }
    }

    let normalized_model_path = normalize_furniture_resource_path(model_path);
    if let Some((object_root, _)) = normalized_model_path.split_once("/model/") {
        let material_root = format!("{object_root}/material");
        if normalized_name.starts_with('v') {
            push_unique_path(
                &mut candidates,
                format!("{material_root}/{normalized_name}"),
            );
        }
        push_unique_path(&mut candidates, format!("{material_root}/{material_file}"));
    }
    candidates
}

/// 解析后的 housing EXD 行（CSV 解析在 xtask 侧）。`item_id` 为 0 的行没有
/// 对应物品，构建目录时过滤。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FurnitureHousingRow {
    pub model_key: u16,
    pub item_id: u32,
}

/// 目录构建时按物品 id 查询的展示信息（来自 craft-data）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FurnitureItemInfo {
    pub name: String,
    pub icon: u32,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FurnitureCatalogPackage {
    pub schema_version: u32,
    pub generated_at: String,
    pub game_version: String,
    pub source: String,
    pub counts: FurnitureCatalogCounts,
    pub items: Vec<FurnitureCatalogItem>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FurnitureCatalogCounts {
    pub items: usize,
    pub indoor: usize,
    pub outdoor: usize,
    /// 物品 id 在 craft-data 中查不到（无法补全名称/图标）而被跳过的行数。
    pub skipped_missing_items: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FurnitureCatalogItem {
    pub id: u32,
    pub kind: FurnitureModelKind,
    pub name: String,
    pub icon: u32,
    pub model_key: u16,
}

/// 由 housing EXD 行构建家具目录：过滤 Item==0 的行，物品名/图标经
/// `item_info` 按物品 id 查询，查不到的行跳过并计数。条目按
/// (kind, model_key) 排序。
pub fn build_furniture_catalog(
    indoor_rows: &[FurnitureHousingRow],
    outdoor_rows: &[FurnitureHousingRow],
    item_info: impl Fn(u32) -> Option<FurnitureItemInfo>,
    generated_at: String,
    game_version: String,
    source: String,
) -> FurnitureCatalogPackage {
    let mut items = Vec::new();
    let mut skipped_missing_items = 0;
    for (kind, rows) in [
        (FurnitureModelKind::Indoor, indoor_rows),
        (FurnitureModelKind::Outdoor, outdoor_rows),
    ] {
        for row in rows {
            if row.item_id == 0 {
                continue;
            }
            let Some(info) = item_info(row.item_id) else {
                skipped_missing_items += 1;
                continue;
            };
            items.push(FurnitureCatalogItem {
                id: row.item_id,
                kind,
                name: info.name,
                icon: info.icon,
                model_key: row.model_key,
            });
        }
    }
    items.sort_by_key(|item| (item.kind, item.model_key));

    let indoor = items
        .iter()
        .filter(|item| item.kind == FurnitureModelKind::Indoor)
        .count();
    let outdoor = items.len() - indoor;
    FurnitureCatalogPackage {
        schema_version: FURNITURE_CATALOG_SCHEMA_VERSION,
        generated_at,
        game_version,
        source,
        counts: FurnitureCatalogCounts {
            items: items.len(),
            indoor,
            outdoor,
            skipped_missing_items,
        },
        items,
    }
}

fn normalize_furniture_resource_path(path: &str) -> String {
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

/// 按提取算法构造最小合成 SGB 二进制，供本模块与加载链测试共用。
#[cfg(test)]
pub(crate) fn synthetic_sgb(entries: &[&str]) -> Vec<u8> {
    let mut strings = Vec::new();
    for entry in entries {
        strings.extend_from_slice(entry.as_bytes());
        strings.push(0);
    }
    strings.push(0xFF);

    // 布局：20 字节头 + i32(skip_value) + skip 区 + 4 字节未知 + i32(strings_offset)
    // + 字符串区。skip = skip_value + 20，字符串区起点 = skip + strings_offset。
    let skip_value = 12usize;
    let skip = skip_value + 20;
    let strings_offset = 8usize;
    let strings_start = skip + strings_offset;

    let mut bytes = vec![0u8; strings_start];
    bytes[20..24].copy_from_slice(&(skip_value as i32).to_le_bytes());
    bytes[skip + 4..skip + 8].copy_from_slice(&(strings_offset as i32).to_le_bytes());
    bytes.extend_from_slice(&strings);
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn furniture_sgb_paths_match_housing_layout() {
        assert_eq!(
            furniture_sgb_path(FurnitureModelKind::Indoor, 1),
            "bgcommon/hou/indoor/general/0001/asset/fun_b0_m0001.sgb"
        );
        assert_eq!(
            furniture_sgb_path(FurnitureModelKind::Outdoor, 1234),
            "bgcommon/hou/outdoor/general/1234/asset/gar_b0_m1234.sgb"
        );
    }

    #[test]
    fn sgb_extraction_classifies_strings_by_extension() {
        let bytes = synthetic_sgb(&[
            "bgcommon/hou/indoor/general/0001/model/fun_b0_m0001.mdl",
            "bgcommon/hou/indoor/general/0001/material/v0001/mt_fun_b0_m0001_a.mtrl",
            "",
            "bgcommon/hou/indoor/general/0002/asset/fun_b0_m0002.sgb",
            "bgcommon/hou/indoor/general/0001/asset",
            "bgcommon/hou/indoor/general/0001/texture/fun_b0_m0001_d.tex",
        ]);
        let paths = extract_sgb_asset_paths(&bytes);
        assert_eq!(
            paths.models,
            ["bgcommon/hou/indoor/general/0001/model/fun_b0_m0001.mdl"]
        );
        assert_eq!(
            paths.related_sgbs,
            ["bgcommon/hou/indoor/general/0002/asset/fun_b0_m0002.sgb"]
        );
        assert_eq!(
            paths.others,
            [
                "bgcommon/hou/indoor/general/0001/material/v0001/mt_fun_b0_m0001_a.mtrl",
                "bgcommon/hou/indoor/general/0001/texture/fun_b0_m0001_d.tex",
            ]
        );
    }

    #[test]
    fn sgb_extraction_normalizes_case_and_separators() {
        let bytes =
            synthetic_sgb(&["BGCOMMON\\HOU\\INDOOR\\GENERAL\\0001\\MODEL\\FUN_B0_M0001.MDL"]);
        let paths = extract_sgb_asset_paths(&bytes);
        assert_eq!(
            paths.models,
            ["bgcommon/hou/indoor/general/0001/model/fun_b0_m0001.mdl"]
        );
    }

    #[test]
    fn sgb_extraction_dedupes_and_ignores_bytes_after_terminator() {
        let bytes = synthetic_sgb(&[
            "bgcommon/hou/indoor/general/0001/model/fun_b0_m0001.mdl",
            "bgcommon/hou/indoor/general/0001/model/fun_b0_m0001.mdl",
        ]);
        let paths = extract_sgb_asset_paths(&bytes);
        assert_eq!(paths.models.len(), 1);
        // synthetic_sgb 在 0xFF 后没有更多内容，这里额外验证 0xFF 之后的字节不被读取。
        let mut bytes = synthetic_sgb(&["a/b/c.mdl"]);
        bytes.extend_from_slice(b"trailing/x.mdl\0");
        let paths = extract_sgb_asset_paths(&bytes);
        assert_eq!(paths.models, ["a/b/c.mdl"]);
    }

    #[test]
    fn sgb_extraction_tolerates_truncated_buffers() {
        assert_eq!(extract_sgb_asset_paths(&[]), SgbAssetPaths::default());
        assert_eq!(
            extract_sgb_asset_paths(&[0u8; 24]),
            SgbAssetPaths::default()
        );
        // skip/strings_offset 指向缓冲区外、字符串区无 NUL 终止都不应 panic。
        let mut bytes = vec![0u8; 64];
        bytes[20..24].copy_from_slice(&(-4i32).to_le_bytes());
        assert_eq!(extract_sgb_asset_paths(&bytes), SgbAssetPaths::default());
        let mut bytes = synthetic_sgb(&["a/b/c.mdl"]);
        bytes.truncate(bytes.len() - 4);
        let _ = extract_sgb_asset_paths(&bytes);
    }

    #[test]
    fn furniture_material_candidates_prefer_sgb_file_matches() {
        let sgb_files = vec![
            "bgcommon/hou/indoor/general/0001/material/v0001/mt_fun_b0_m0001_a.mtrl".to_string(),
            "bgcommon/hou/indoor/general/0001/texture/fun_b0_m0001_d.tex".to_string(),
        ];
        let candidates = furniture_material_candidate_paths(
            "bgcommon/hou/indoor/general/0001/model/fun_b0_m0001.mdl",
            "mt_fun_b0_m0001_a.mtrl",
            &sgb_files,
        );
        assert_eq!(
            candidates,
            [
                "mt_fun_b0_m0001_a.mtrl",
                "bgcommon/hou/indoor/general/0001/material/v0001/mt_fun_b0_m0001_a.mtrl",
                "bgcommon/hou/indoor/general/0001/material/mt_fun_b0_m0001_a.mtrl",
            ]
        );
    }

    #[test]
    fn furniture_material_candidates_keep_full_paths_and_version_fallback() {
        let candidates = furniture_material_candidate_paths(
            "bgcommon/hou/indoor/general/0001/model/fun_b0_m0001.mdl",
            "v0001/mt_fun_b0_m0001_a.mtrl",
            &[],
        );
        assert_eq!(
            candidates,
            [
                "v0001/mt_fun_b0_m0001_a.mtrl",
                "bgcommon/hou/indoor/general/0001/material/v0001/mt_fun_b0_m0001_a.mtrl",
                "bgcommon/hou/indoor/general/0001/material/mt_fun_b0_m0001_a.mtrl",
            ]
        );
        assert!(furniture_material_candidate_paths("bgcommon/x/model/y.mdl", "", &[]).is_empty());
    }

    fn test_item_info(
        items: &'static [(u32, &'static str, u32)],
    ) -> impl Fn(u32) -> Option<FurnitureItemInfo> {
        move |id| {
            items
                .iter()
                .find(|(item_id, _, _)| *item_id == id)
                .map(|(_, name, icon)| FurnitureItemInfo {
                    name: name.to_string(),
                    icon: *icon,
                })
        }
    }

    #[test]
    fn furniture_catalog_filters_empty_rows_and_maps_kinds() {
        let catalog = build_furniture_catalog(
            &[
                FurnitureHousingRow {
                    model_key: 0,
                    item_id: 0,
                },
                FurnitureHousingRow {
                    model_key: 42,
                    item_id: 19770,
                },
            ],
            &[FurnitureHousingRow {
                model_key: 7,
                item_id: 9710,
            }],
            test_item_info(&[(19770, "测试木桌", 59001), (9710, "测试庭具石灯", 59002)]),
            "2026-09-11T00:00:00Z".to_string(),
            "game-2026.06.18.0000.0000".to_string(),
            "test".to_string(),
        );

        assert_eq!(catalog.schema_version, FURNITURE_CATALOG_SCHEMA_VERSION);
        assert_eq!(
            catalog.counts,
            FurnitureCatalogCounts {
                items: 2,
                indoor: 1,
                outdoor: 1,
                skipped_missing_items: 0,
            }
        );
        assert_eq!(
            catalog.items,
            [
                FurnitureCatalogItem {
                    id: 19770,
                    kind: FurnitureModelKind::Indoor,
                    name: "测试木桌".to_string(),
                    icon: 59001,
                    model_key: 42,
                },
                FurnitureCatalogItem {
                    id: 9710,
                    kind: FurnitureModelKind::Outdoor,
                    name: "测试庭具石灯".to_string(),
                    icon: 59002,
                    model_key: 7,
                },
            ]
        );
    }

    #[test]
    fn furniture_catalog_skips_items_missing_from_lookup() {
        let catalog = build_furniture_catalog(
            &[
                FurnitureHousingRow {
                    model_key: 3,
                    item_id: 1,
                },
                FurnitureHousingRow {
                    model_key: 1,
                    item_id: 404,
                },
            ],
            &[],
            test_item_info(&[(1, "存在的家具", 100)]),
            String::new(),
            String::new(),
            String::new(),
        );

        assert_eq!(catalog.counts.items, 1);
        assert_eq!(catalog.counts.skipped_missing_items, 1);
        assert_eq!(catalog.items[0].name, "存在的家具");
    }
}
