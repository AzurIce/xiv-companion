use std::collections::HashSet;

use js_sys::JsString;
use serde::{Deserialize, Serialize};
use wasm_bindgen::JsValue;

const COLLECTION_DB_NAME: &str = "xiv-companion-collection-state";
const COLLECTION_STORE_NAME: &str = "collections";
const COLLECTION_DB_VERSION: u32 = 3;
const LEGACY_ENTRY_STORE_NAME: &str = "entries";
const LEGACY_STATE_STORE_NAME: &str = "state";
const COLLECTION_EXPORT_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionExport {
    pub schema_version: u32,
    pub items: Vec<u32>,
}

pub async fn load_collection_ids() -> Result<HashSet<u32>, String> {
    let db = open_collection_db().await?;
    let keys = db
        .transaction(&[COLLECTION_STORE_NAME])
        .run(|transaction| async move {
            transaction
                .object_store(COLLECTION_STORE_NAME)?
                .get_all_keys(None)
                .await
        })
        .await
        .map_err(|error| format!("读取图鉴解锁状态失败: {error}"))?;

    Ok(keys.into_iter().filter_map(item_id_from_key).collect())
}

pub async fn save_collection_entry(item_id: u32, obtained: bool) -> Result<(), String> {
    let db = open_collection_db().await?;
    let key = JsString::from(item_id.to_string());
    db.transaction(&[COLLECTION_STORE_NAME])
        .rw()
        .run(move |transaction| async move {
            let collections = transaction.object_store(COLLECTION_STORE_NAME)?;
            if obtained {
                collections.put_kv(&key, &JsValue::TRUE).await?;
            } else {
                collections.delete(&key).await?;
            }
            Ok(())
        })
        .await
        .map_err(|error| format!("保存图鉴解锁状态失败: {error}"))
}

pub async fn replace_collection_ids(item_ids: &HashSet<u32>) -> Result<(), String> {
    let mut item_ids = item_ids.iter().copied().collect::<Vec<_>>();
    item_ids.sort_unstable();
    let db = open_collection_db().await?;
    db.transaction(&[COLLECTION_STORE_NAME])
        .rw()
        .run(move |transaction| async move {
            let collections = transaction.object_store(COLLECTION_STORE_NAME)?;
            collections.clear().await?;
            for item_id in item_ids {
                collections
                    .put_kv(&JsString::from(item_id.to_string()), &JsValue::TRUE)
                    .await?;
            }
            Ok(())
        })
        .await
        .map_err(|error| format!("替换图鉴解锁状态失败: {error}"))
}

pub fn export_collection_json(item_ids: &HashSet<u32>) -> Result<String, String> {
    let mut items = item_ids.iter().copied().collect::<Vec<_>>();
    items.sort_unstable();
    serde_json::to_string_pretty(&CollectionExport {
        schema_version: COLLECTION_EXPORT_SCHEMA_VERSION,
        items,
    })
    .map_err(|error| format!("序列化图鉴解锁状态失败: {error}"))
}

pub fn import_collection_json(json: &str) -> Result<HashSet<u32>, String> {
    let export = serde_json::from_str::<CollectionExport>(json)
        .map_err(|error| format!("图鉴 JSON 格式无效: {error}"))?;
    if export.schema_version != COLLECTION_EXPORT_SCHEMA_VERSION {
        return Err(format!(
            "不支持的图鉴 JSON schemaVersion {}，当前支持 {}",
            export.schema_version, COLLECTION_EXPORT_SCHEMA_VERSION
        ));
    }
    if export.items.contains(&0) {
        return Err("图鉴 JSON 包含无效的 Item ID 0".to_string());
    }
    Ok(export.items.into_iter().collect())
}

async fn open_collection_db() -> Result<indexed_db::Database<String>, String> {
    let factory = indexed_db::Factory::get()
        .map_err(|error| format!("打开 IndexedDB 图鉴数据库失败: {error}"))?;
    factory
        .open(
            COLLECTION_DB_NAME,
            COLLECTION_DB_VERSION,
            |event| async move {
                let db = event.database();
                let names = db.object_store_names();
                if !names.iter().any(|name| name == COLLECTION_STORE_NAME) {
                    db.build_object_store(COLLECTION_STORE_NAME).create()?;
                }

                if names.iter().any(|name| name == LEGACY_ENTRY_STORE_NAME) {
                    let legacy_keys = event
                        .transaction()
                        .object_store(LEGACY_ENTRY_STORE_NAME)?
                        .get_all_keys(None)
                        .await?;
                    let collections = event.transaction().object_store(COLLECTION_STORE_NAME)?;
                    for item_id in legacy_keys.into_iter().filter_map(item_id_from_key) {
                        collections
                            .put_kv(&JsString::from(item_id.to_string()), &JsValue::TRUE)
                            .await?;
                    }
                    db.delete_object_store(LEGACY_ENTRY_STORE_NAME)?;
                }
                if names.iter().any(|name| name == LEGACY_STATE_STORE_NAME) {
                    db.delete_object_store(LEGACY_STATE_STORE_NAME)?;
                }
                Ok(())
            },
        )
        .await
        .map_err(|error| format!("打开图鉴解锁数据库失败: {error}"))
}

fn item_id_from_key(key: JsValue) -> Option<u32> {
    if let Some(value) = key.as_string() {
        return value
            .rsplit_once(':')
            .map_or(value.as_str(), |(_, item_id)| item_id)
            .parse()
            .ok();
    }
    key.as_f64()
        .filter(|value| value.fract() == 0.0 && *value > 0.0 && *value <= u32::MAX as f64)
        .map(|value| value as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collection_json_is_sorted_and_round_trips() {
        let item_ids = HashSet::from([30, 10, 20]);
        let json = export_collection_json(&item_ids).unwrap();
        assert!(json.find("10").unwrap() < json.find("30").unwrap());
        assert_eq!(import_collection_json(&json), Ok(item_ids));
    }

    #[test]
    fn collection_json_rejects_unknown_schema_and_zero_ids() {
        assert!(import_collection_json(r#"{"schemaVersion":2,"items":[]}"#).is_err());
        assert!(import_collection_json(r#"{"schemaVersion":1,"items":[0]}"#).is_err());
    }
}
