use dioxus::prelude::*;
use js_sys::JsString;
use wasm_bindgen::JsValue;
use xiv_companion::{
    BuiltinItemIconProvider, ProviderRequest, ResourceBlob, ResourceError, ResourceErrorKind,
    ResourceFuture, ResourceHub, ResourceProvider, ResourceSource, register_craft_data_resource,
    register_item_icon_resource, resources::craft_data::CraftDataKind,
};

use crate::app::browser_sqpack::BrowserSqPack;
use crate::app::load_progress::{
    CraftDataCacheStatus, CraftDataLoadProgress, report_craft_data_cache_status,
    report_craft_data_progress,
};
use crate::app::log;

const BUNDLED_CRAFT_DATA_ASSET: Asset = asset!("/assets/craft-data.json");
const LOCAL_RESOURCE_CACHE_DB: &str = "xiv-companion-resource-cache";
const LOCAL_RESOURCE_CACHE_STORE: &str = "resources";
const LOCAL_CRAFT_DATA_CACHE_KEY: &str = "user-local-craft-data";

pub fn default_web_resource_hub() -> ResourceHub {
    let mut hub = ResourceHub::new();
    register_craft_data_resource(&mut hub);
    register_item_icon_resource(&mut hub);
    hub.add_provider(BundledProvider);
    hub.add_provider(BuiltinItemIconProvider);
    hub.add_provider(BrowserSqPackProvider);
    hub
}

pub struct BrowserSqPackProvider;

impl ResourceProvider for BrowserSqPackProvider {
    fn source(&self) -> ResourceSource {
        ResourceSource::UserLocal
    }

    fn supports(&self, request: &ProviderRequest) -> bool {
        browser_sqpack_handle_available()
            && request.kind == CraftDataKind.into()
            && request.key == "default"
    }

    fn read<'a>(
        &'a self,
        request: ProviderRequest,
    ) -> ResourceFuture<'a, Result<ResourceBlob, ResourceError>> {
        Box::pin(async move {
            let resource_kind = request.kind.clone();
            if !self.supports(&request) {
                return Err(ResourceError::new(
                    ResourceErrorKind::Unsupported,
                    resource_kind,
                    Some(self.source()),
                    "Local SqPack source is not configured for this resource",
                ));
            }

            log::info("resource", "loading CraftData from UserLocal SqPack");
            let start_ms = log::now_ms();
            let (bytes, fingerprint) = load_browser_sqpack_craft_data().await.map_err(|error| {
                report_craft_data_progress(Some(CraftDataLoadProgress {
                    stage: "本地 CraftData 失败".to_string(),
                    detail: error.clone(),
                    current: 1,
                    total: 1,
                    elapsed_ms: log::elapsed_ms(start_ms),
                    done: true,
                }));
                report_craft_data_cache_status(Some(CraftDataCacheStatus::Error {
                    message: error.clone(),
                }));
                ResourceError::new(
                    ResourceErrorKind::ProviderFailed,
                    resource_kind,
                    Some(self.source()),
                    error,
                )
            })?;
            log::info(
                "resource",
                format!(
                    "loaded CraftData from UserLocal in {}",
                    log::format_elapsed(log::elapsed_ms(start_ms))
                ),
            );
            Ok(ResourceBlob { bytes, fingerprint })
        })
    }
}

async fn load_browser_sqpack_craft_data() -> Result<(Vec<u8>, Option<String>), String> {
    log::debug("resource", "BrowserSqPack request: craft-data/default");
    let start_ms = log::now_ms();
    let mut sqpack = BrowserSqPack::from_window_handle().await?;

    let cache_fingerprint = sqpack.craft_data_cache_fingerprint().await?;
    report_craft_data_cache_status(Some(CraftDataCacheStatus::Checking));
    report_craft_data_progress(Some(CraftDataLoadProgress {
        stage: "检查本地缓存".to_string(),
        detail: "CraftData JSON".to_string(),
        current: 0,
        total: 1,
        elapsed_ms: log::elapsed_ms(start_ms),
        done: false,
    }));
    if let Some((bytes, game_version)) = load_cached_local_craft_data(&cache_fingerprint).await? {
        report_craft_data_cache_status(Some(CraftDataCacheStatus::Hit { bytes: bytes.len() }));
        let elapsed_ms = log::elapsed_ms(start_ms);
        log::info(
            "resource",
            format!(
                "loaded CraftData from IndexedDB cache in {}",
                log::format_elapsed(elapsed_ms)
            ),
        );
        report_craft_data_progress(Some(CraftDataLoadProgress {
            stage: "使用本地缓存".to_string(),
            detail: game_version
                .clone()
                .unwrap_or_else(|| "CraftData JSON".to_string()),
            current: 1,
            total: 1,
            elapsed_ms,
            done: true,
        }));
        return Ok((bytes, game_version));
    }

    report_craft_data_cache_status(Some(CraftDataCacheStatus::Miss {
        reason: "fingerprint mismatch or empty".to_string(),
    }));
    log::info("resource", "local CraftData cache miss; exporting from SqPack");
    let resource = sqpack.preload_craft_data_resource().await?;
    let after_preload_ms = log::elapsed_ms(start_ms);
    report_craft_data_progress(Some(CraftDataLoadProgress {
        stage: "导出 CraftData".to_string(),
        detail: "转换本地 EXD 为应用数据".to_string(),
        current: 1,
        total: 1,
        elapsed_ms: after_preload_ms,
        done: false,
    }));
    let export_start_ms = log::now_ms();
    let generated_at = js_sys::Date::new_0()
        .to_iso_string()
        .as_string()
        .unwrap_or_else(|| "1970-01-01T00:00:00.000Z".to_string());
    let data = xiv_companion::game_data::export_craft_data_from_resource(
        resource,
        "Browser Local SqPack".to_string(),
        "Local SqPack".to_string(),
        generated_at,
    )
    .map_err(|error| format!("failed to export CraftData from local SqPack: {error:#}"))?;
    let fingerprint = Some(data.game_version.clone());
    let export_elapsed_ms = log::elapsed_ms(export_start_ms);
    let bytes = serde_json::to_vec(&data)
        .map_err(|error| format!("failed to encode local CraftData: {error}"))?;
    let total_elapsed_ms = log::elapsed_ms(start_ms);
    log::info(
        "resource",
        format!(
            "CraftData local pipeline completed: preload={}, export={}, total={}",
            log::format_elapsed(after_preload_ms),
            log::format_elapsed(export_elapsed_ms),
            log::format_elapsed(total_elapsed_ms),
        ),
    );
    report_craft_data_cache_status(Some(CraftDataCacheStatus::Saving { bytes: bytes.len() }));
    save_cached_local_craft_data(&cache_fingerprint, &data.game_version, &bytes).await?;
    report_craft_data_cache_status(Some(CraftDataCacheStatus::Saved { bytes: bytes.len() }));
    report_craft_data_progress(Some(CraftDataLoadProgress {
        stage: "本地 CraftData 就绪".to_string(),
        detail: format!(
            "{} / {} items / {} recipes",
            data.game_version, data.counts.items, data.counts.recipes
        ),
        current: 1,
        total: 1,
        elapsed_ms: total_elapsed_ms,
        done: true,
    }));
    Ok((bytes, fingerprint))
}

async fn local_resource_cache_db() -> Result<indexed_db::Database<String>, String> {
    let factory = indexed_db::Factory::get()
        .map_err(|error| format!("打开 IndexedDB 缓存失败: {error}"))?;
    factory
        .open(LOCAL_RESOURCE_CACHE_DB, 1, |event| async move {
            let db = event.database();
            db.build_object_store(LOCAL_RESOURCE_CACHE_STORE).create()?;
            Ok(())
        })
        .await
        .map_err(|error| format!("打开资源缓存数据库失败: {error}"))
}

async fn load_cached_local_craft_data(
    expected_fingerprint: &str,
) -> Result<Option<(Vec<u8>, Option<String>)>, String> {
    let db = local_resource_cache_db().await?;
    let record = db
        .transaction(&[LOCAL_RESOURCE_CACHE_STORE])
        .run(|transaction| async move {
            transaction
                .object_store(LOCAL_RESOURCE_CACHE_STORE)?
                .get(&JsString::from(LOCAL_CRAFT_DATA_CACHE_KEY))
                .await
        })
        .await
        .map_err(|error| format!("读取本地 CraftData 缓存失败: {error}"))?;

    let Some(record) = record else {
        log::info("resource-cache", "CraftData cache miss: empty");
        return Ok(None);
    };

    let fingerprint = js_string_field(&record, "fingerprint");
    if fingerprint.as_deref() != Some(expected_fingerprint) {
        log::info("resource-cache", "CraftData cache miss: fingerprint changed");
        return Ok(None);
    }

    let bytes = js_sys::Reflect::get(&record, &JsValue::from_str("bytes"))
        .map_err(format_js_error)?;
    if bytes.is_undefined() || bytes.is_null() {
        log::warn("resource-cache", "CraftData cache record has no bytes");
        return Ok(None);
    }

    let bytes = js_sys::Uint8Array::new(&bytes).to_vec();
    let game_version = js_string_field(&record, "gameVersion");
    log::info(
        "resource-cache",
        format!("CraftData cache hit: {} bytes", bytes.len()),
    );
    Ok(Some((bytes, game_version)))
}

async fn save_cached_local_craft_data(
    fingerprint: &str,
    game_version: &str,
    bytes: &[u8],
) -> Result<(), String> {
    let object = js_sys::Object::new();
    js_sys::Reflect::set(
        &object,
        &JsValue::from_str("fingerprint"),
        &JsValue::from_str(fingerprint),
    )
    .map_err(format_js_error)?;
    js_sys::Reflect::set(
        &object,
        &JsValue::from_str("gameVersion"),
        &JsValue::from_str(game_version),
    )
    .map_err(format_js_error)?;
    js_sys::Reflect::set(
        &object,
        &JsValue::from_str("savedAt"),
        &js_sys::Date::new_0().to_iso_string(),
    )
    .map_err(format_js_error)?;
    let array = js_sys::Uint8Array::from(bytes);
    js_sys::Reflect::set(&object, &JsValue::from_str("bytes"), &array).map_err(format_js_error)?;

    let db = local_resource_cache_db().await?;
    db.transaction(&[LOCAL_RESOURCE_CACHE_STORE])
        .rw()
        .run(move |transaction| async move {
            transaction
                .object_store(LOCAL_RESOURCE_CACHE_STORE)?
                .put_kv(&JsString::from(LOCAL_CRAFT_DATA_CACHE_KEY), &object)
                .await?;
            Ok(())
        })
        .await
        .map_err(|error| format!("保存本地 CraftData 缓存失败: {error}"))?;
    log::info(
        "resource-cache",
        format!("saved CraftData cache: {} bytes", bytes.len()),
    );
    Ok(())
}

fn js_string_field(value: &JsValue, key: &str) -> Option<String> {
    js_sys::Reflect::get(value, &JsValue::from_str(key))
        .ok()
        .and_then(|value| value.as_string())
}

fn format_js_error(error: JsValue) -> String {
    js_sys::Reflect::get(&error, &JsValue::from_str("message"))
        .ok()
        .and_then(|value| value.as_string())
        .or_else(|| error.as_string())
        .unwrap_or_else(|| "JavaScript 调用失败".to_string())
}

fn browser_sqpack_handle_available() -> bool {
    web_sys::window()
        .and_then(|window| {
            js_sys::Reflect::get(
                window.as_ref(),
                &JsValue::from_str("__xivCompanionUserLocalDirectory"),
            )
            .ok()
        })
        .is_some_and(|value| !value.is_undefined() && !value.is_null())
}

pub struct BundledProvider;

impl ResourceProvider for BundledProvider {
    fn source(&self) -> ResourceSource {
        ResourceSource::Builtin
    }

    fn supports(&self, request: &ProviderRequest) -> bool {
        request.kind == CraftDataKind.into() && request.key == "default"
    }

    fn read<'a>(
        &'a self,
        request: ProviderRequest,
    ) -> ResourceFuture<'a, Result<ResourceBlob, ResourceError>> {
        Box::pin(async move {
            let resource_kind = request.kind.clone();
            if !self.supports(&request) {
                return Err(ResourceError::new(
                    ResourceErrorKind::Unsupported,
                    resource_kind,
                    Some(self.source()),
                    format!("bundled provider has no resource key {}", request.key),
                ));
            }

            let bytes = dioxus::asset_resolver::read_asset_bytes(BUNDLED_CRAFT_DATA_ASSET)
                .await
                .map_err(|error| {
                    ResourceError::new(
                        ResourceErrorKind::ProviderFailed,
                        resource_kind,
                        Some(self.source()),
                        format!("failed to read bundled asset: {error}"),
                    )
                })?;
            Ok(ResourceBlob {
                bytes,
                fingerprint: None,
            })
        })
    }
}
