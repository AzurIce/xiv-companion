use js_sys::JsString;
use wasm_bindgen::JsValue;

const RESOURCE_DB_NAME: &str = "xiv-companion-resource-cache";
const RESOURCE_STORE_NAME: &str = "resources";
const RESOURCE_DB_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CachedResourceRecord {
    /// Version/fingerprint of the data itself (e.g. game version or build id).
    pub fingerprint: String,
    /// Where this cached copy came from: "builtin" or "local".
    pub source_tag: String,
    /// Human-readable game version string.
    pub game_version: String,
    /// Decoder/schema revision used to create the cached payload.
    pub schema_revision: u32,
    /// Number of logical records in the payload when known.
    pub record_count: usize,
    /// Timestamp at which this copy was stored.
    pub saved_at: String,
    /// Serialized resource bytes.
    pub bytes: Vec<u8>,
}

pub async fn open_resource_db() -> Result<indexed_db::Database<String>, String> {
    let factory =
        indexed_db::Factory::get().map_err(|error| format!("打开 IndexedDB 缓存失败: {error}"))?;
    factory
        .open(RESOURCE_DB_NAME, RESOURCE_DB_VERSION, |event| async move {
            let db = event.database();
            db.build_object_store(RESOURCE_STORE_NAME).create()?;
            Ok(())
        })
        .await
        .map_err(|error| format!("打开资源缓存数据库失败: {error}"))
}

pub async fn load_cached_resource(key: &str) -> Result<Option<CachedResourceRecord>, String> {
    let db = open_resource_db().await?;
    let key = key.to_string();
    let record = db
        .transaction(&[RESOURCE_STORE_NAME])
        .run({
            let key = key.clone();
            move |transaction| async move {
                transaction
                    .object_store(RESOURCE_STORE_NAME)?
                    .get(&JsString::from(key.as_str()))
                    .await
            }
        })
        .await
        .map_err(|error| format!("读取缓存资源 {key} 失败: {error}"))?;

    let Some(record) = record else {
        return Ok(None);
    };

    let bytes =
        js_sys::Reflect::get(&record, &JsValue::from_str("bytes")).map_err(format_js_error)?;
    if bytes.is_undefined() || bytes.is_null() {
        return Ok(None);
    }
    let bytes = js_sys::Uint8Array::new(&bytes).to_vec();

    Ok(Some(CachedResourceRecord {
        fingerprint: js_string_field(&record, "fingerprint").unwrap_or_default(),
        source_tag: js_string_field(&record, "sourceTag").unwrap_or_default(),
        game_version: js_string_field(&record, "gameVersion").unwrap_or_default(),
        schema_revision: js_number_field(&record, "schemaRevision").unwrap_or_default() as u32,
        record_count: js_number_field(&record, "recordCount").unwrap_or_default() as usize,
        saved_at: js_string_field(&record, "savedAt").unwrap_or_default(),
        bytes,
    }))
}

pub async fn save_cached_resource(key: &str, record: &CachedResourceRecord) -> Result<(), String> {
    let object = js_sys::Object::new();
    js_sys::Reflect::set(
        &object,
        &JsValue::from_str("fingerprint"),
        &JsValue::from_str(&record.fingerprint),
    )
    .map_err(format_js_error)?;
    js_sys::Reflect::set(
        &object,
        &JsValue::from_str("sourceTag"),
        &JsValue::from_str(&record.source_tag),
    )
    .map_err(format_js_error)?;
    js_sys::Reflect::set(
        &object,
        &JsValue::from_str("gameVersion"),
        &JsValue::from_str(&record.game_version),
    )
    .map_err(format_js_error)?;
    js_sys::Reflect::set(
        &object,
        &JsValue::from_str("schemaRevision"),
        &JsValue::from_f64(record.schema_revision as f64),
    )
    .map_err(format_js_error)?;
    js_sys::Reflect::set(
        &object,
        &JsValue::from_str("recordCount"),
        &JsValue::from_f64(record.record_count as f64),
    )
    .map_err(format_js_error)?;
    js_sys::Reflect::set(
        &object,
        &JsValue::from_str("savedAt"),
        &JsValue::from_str(&record.saved_at),
    )
    .map_err(format_js_error)?;
    let array = js_sys::Uint8Array::from(record.bytes.as_slice());
    js_sys::Reflect::set(&object, &JsValue::from_str("bytes"), &array).map_err(format_js_error)?;

    let db = open_resource_db().await?;
    let key = key.to_string();
    db.transaction(&[RESOURCE_STORE_NAME])
        .rw()
        .run({
            let key = key.clone();
            move |transaction| async move {
                transaction
                    .object_store(RESOURCE_STORE_NAME)?
                    .put_kv(&JsString::from(key.as_str()), &object)
                    .await?;
                Ok(())
            }
        })
        .await
        .map_err(|error| format!("保存缓存资源 {key} 失败: {error}"))
}

pub async fn delete_cached_resource(key: &str) -> Result<(), String> {
    let db = open_resource_db().await?;
    let key = key.to_string();
    db.transaction(&[RESOURCE_STORE_NAME])
        .rw()
        .run({
            let key = key.clone();
            move |transaction| async move {
                transaction
                    .object_store(RESOURCE_STORE_NAME)?
                    .delete(&JsString::from(key.as_str()))
                    .await?;
                Ok(())
            }
        })
        .await
        .map_err(|error| format!("删除缓存资源 {key} 失败: {error}"))
}

fn js_string_field(value: &JsValue, key: &str) -> Option<String> {
    js_sys::Reflect::get(value, &JsValue::from_str(key))
        .ok()
        .and_then(|value| value.as_string())
}

fn js_number_field(value: &JsValue, key: &str) -> Option<f64> {
    js_sys::Reflect::get(value, &JsValue::from_str(key))
        .ok()
        .and_then(|value| value.as_f64())
}

fn format_js_error(error: JsValue) -> String {
    js_sys::Reflect::get(&error, &JsValue::from_str("message"))
        .ok()
        .and_then(|value| value.as_string())
        .or_else(|| error.as_string())
        .unwrap_or_else(|| "JavaScript 调用失败".to_string())
}
