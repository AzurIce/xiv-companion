use std::collections::HashSet;
use std::str::FromStr;

use js_sys::JsString;
use wasm_bindgen::JsValue;
use xiv_companion::CollectionEntryKey;

const STATE_DB_NAME: &str = "xiv-companion-collection-state";
const STATE_STORE_NAME: &str = "state";
const STATE_DB_VERSION: u32 = 1;
const STATE_KEY: &str = "default";

/// User-owned collection tracking state: which items are marked as obtained.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CollectionState {
    pub obtained: HashSet<CollectionEntryKey>,
    pub legacy_item_ids: HashSet<u32>,
    pub updated_at: String,
}

impl CollectionState {
    #[allow(dead_code)]
    pub fn is_obtained(&self, key: &CollectionEntryKey) -> bool {
        self.obtained.contains(key)
    }

    #[allow(dead_code)]
    pub fn set_obtained(&mut self, key: CollectionEntryKey, obtained: bool) {
        if obtained {
            self.obtained.insert(key);
        } else {
            self.obtained.remove(&key);
        }
        self.updated_at = js_sys::Date::new_0()
            .to_iso_string()
            .as_string()
            .unwrap_or_else(|| "1970-01-01T00:00:00.000Z".to_string());
    }

    #[allow(dead_code)]
    pub fn toggle_obtained(&mut self, key: CollectionEntryKey) -> bool {
        let obtained = !self.obtained.contains(&key);
        self.set_obtained(key, obtained);
        obtained
    }
}

pub async fn load_collection_state() -> Result<CollectionState, String> {
    let db = open_state_db().await?;
    let record = db
        .transaction(&[STATE_STORE_NAME])
        .run({
            move |transaction| async move {
                transaction
                    .object_store(STATE_STORE_NAME)?
                    .get(&JsString::from(STATE_KEY))
                    .await
            }
        })
        .await
        .map_err(|error| format!("读取收藏状态失败: {error}"))?;

    let Some(record) = record else {
        return Ok(CollectionState::default());
    };

    let obtained: HashSet<CollectionEntryKey> = js_array_of_strings(&record, "obtainedKeys")
        .unwrap_or_default()
        .into_iter()
        .filter_map(|key| CollectionEntryKey::from_str(&key).ok())
        .collect();
    let legacy_item_ids = js_array_of_u32(&record, "obtainedIds")
        .unwrap_or_default()
        .into_iter()
        .collect();
    let updated_at = js_string_field(&record, "updatedAt").unwrap_or_default();

    Ok(CollectionState {
        obtained,
        legacy_item_ids,
        updated_at,
    })
}

pub async fn save_collection_state(state: &CollectionState) -> Result<(), String> {
    let object = js_sys::Object::new();
    let mut keys = state
        .obtained
        .iter()
        .map(CollectionEntryKey::storage_key)
        .collect::<Vec<_>>();
    keys.sort();
    js_sys::Reflect::set(
        &object,
        &JsValue::from_str("obtainedKeys"),
        &serde_json::to_string(&keys)
            .map(|json| JsValue::from_str(&json))
            .unwrap_or_else(|_| JsValue::UNDEFINED),
    )
    .map_err(format_js_error)?;
    js_sys::Reflect::set(
        &object,
        &JsValue::from_str("updatedAt"),
        &JsValue::from_str(&state.updated_at),
    )
    .map_err(format_js_error)?;

    let db = open_state_db().await?;
    db.transaction(&[STATE_STORE_NAME])
        .rw()
        .run({
            move |transaction| async move {
                transaction
                    .object_store(STATE_STORE_NAME)?
                    .put_kv(&JsString::from(STATE_KEY), &object)
                    .await?;
                Ok(())
            }
        })
        .await
        .map_err(|error| format!("保存收藏状态失败: {error}"))
}

async fn open_state_db() -> Result<indexed_db::Database<String>, String> {
    let factory = indexed_db::Factory::get()
        .map_err(|error| format!("打开 IndexedDB 状态库失败: {error}"))?;
    factory
        .open(STATE_DB_NAME, STATE_DB_VERSION, |event| async move {
            let db = event.database();
            db.build_object_store(STATE_STORE_NAME).create()?;
            Ok(())
        })
        .await
        .map_err(|error| format!("打开收藏状态数据库失败: {error}"))
}

fn js_string_field(value: &JsValue, key: &str) -> Option<String> {
    js_sys::Reflect::get(value, &JsValue::from_str(key))
        .ok()
        .and_then(|value| value.as_string())
}

fn js_array_of_u32(value: &JsValue, key: &str) -> Option<Vec<u32>> {
    let json = js_string_field(value, key)?;
    serde_json::from_str(&json).ok()
}

fn js_array_of_strings(value: &JsValue, key: &str) -> Option<Vec<String>> {
    let json = js_string_field(value, key)?;
    serde_json::from_str(&json).ok()
}

fn format_js_error(error: JsValue) -> String {
    js_sys::Reflect::get(&error, &JsValue::from_str("message"))
        .ok()
        .and_then(|value| value.as_string())
        .or_else(|| error.as_string())
        .unwrap_or_else(|| "JavaScript 调用失败".to_string())
}
