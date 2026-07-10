use dioxus::prelude::*;
use js_sys::JsString;
use physis::ReadableFile;
use wasm_bindgen::JsValue;
use xiv_companion::{
    AsyncGameResource, BuiltinItemIconProvider, ItemIconResourceInfo, LocalItemIconImage,
    ProviderRequest, ResourceBlob, ResourceError, ResourceErrorKind, ResourceFuture, ResourceHub,
    ResourceProvider, ResourceSource, WeaponModelLoadRequest, item_icon_tex_path,
    load_weapon_model_from_async_resource, register_craft_data_resource,
    register_item_icon_resource, register_weapon_model_resources,
    resources::{
        craft_data::CraftDataKind,
        item_icon::ItemIconKind,
        weapon_model::{WeaponCatalogKind, WeaponModelKind, parse_weapon_model_request_key},
    },
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
const LOCAL_WEAPON_CATALOG_CACHE_KEY: &str = "user-local-weapon-catalog-v2";
const ITEM_ICON_READ_WINDOW: u64 = 2 * 1024 * 1024;

pub fn default_web_resource_hub() -> ResourceHub {
    let mut hub = ResourceHub::new();
    register_craft_data_resource(&mut hub);
    register_item_icon_resource(&mut hub);
    register_weapon_model_resources(&mut hub);
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
        (request.kind == CraftDataKind.into() && request.key == "default")
            || (request.kind == ItemIconKind.into() && request.key.parse::<u32>().is_ok())
            || (request.kind == WeaponCatalogKind.into() && request.key == "default")
            || (request.kind == WeaponModelKind.into()
                && parse_weapon_model_request_key(&request.key).is_ok())
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

            if request.kind == CraftDataKind.into() {
                log::info("resource", "loading CraftData from UserLocal SqPack");
                let start_ms = log::now_ms();
                let (bytes, fingerprint) = load_from_browser_sqpack_direct("craft-data", "default")
                    .await
                    .map_err(|error| {
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
                            browser_sqpack_provider_error_kind(&error),
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
                return Ok(ResourceBlob { bytes, fingerprint });
            }

            if request.kind == WeaponCatalogKind.into() {
                log::info("resource", "loading WeaponCatalog from UserLocal SqPack");
                let start_ms = log::now_ms();
                let bytes = load_weapon_catalog_from_browser_sqpack()
                    .await
                    .map_err(|error| {
                        ResourceError::new(
                            browser_sqpack_provider_error_kind(&error),
                            resource_kind.clone(),
                            Some(self.source()),
                            error,
                        )
                    })?;
                log::info(
                    "resource",
                    format!(
                        "loaded WeaponCatalog from UserLocal in {}",
                        log::format_elapsed(log::elapsed_ms(start_ms)),
                    ),
                );
                return Ok(ResourceBlob {
                    bytes,
                    fingerprint: None,
                });
            }

            if request.kind == WeaponModelKind.into() {
                log::info("resource", format!("loading WeaponModel {}", request.key));
                let start_ms = log::now_ms();
                let id = parse_weapon_model_request_key(&request.key).map_err(|error| {
                    ResourceError::new(
                        ResourceErrorKind::Unsupported,
                        resource_kind.clone(),
                        Some(self.source()),
                        error,
                    )
                })?;
                let model = load_weapon_model_from_browser_sqpack(id)
                    .await
                    .map_err(|error| {
                        ResourceError::new(
                            browser_sqpack_provider_error_kind(&error),
                            resource_kind.clone(),
                            Some(self.source()),
                            error,
                        )
                    })?;
                log::info(
                    "resource",
                    format!(
                        "decoded WeaponModel in {}",
                        log::format_elapsed(log::elapsed_ms(start_ms)),
                    ),
                );
                let bytes = serde_json::to_vec(&model).map_err(|error| {
                    ResourceError::new(
                        ResourceErrorKind::ProviderFailed,
                        resource_kind,
                        Some(self.source()),
                        format!("failed to encode weapon model resource info: {error}"),
                    )
                })?;
                return Ok(ResourceBlob {
                    bytes,
                    fingerprint: None,
                });
            }

            log::info(
                "resource",
                format!("loading ItemIcon {} from UserLocal SqPack", request.key),
            );
            let start_ms = log::now_ms();
            let icon_id = request.key.parse::<u32>().map_err(|error| {
                ResourceError::new(
                    ResourceErrorKind::Unsupported,
                    resource_kind.clone(),
                    Some(self.source()),
                    format!("invalid item icon id {}: {error}", request.key),
                )
            })?;
            let path = item_icon_tex_path(icon_id);
            let (bytes, _) = load_from_browser_sqpack_direct("item-icon", &request.key)
                .await
                .map_err(|error| {
                    ResourceError::new(
                        browser_sqpack_not_found_or_access_error_kind(&error),
                        resource_kind.clone(),
                        Some(self.source()),
                        error,
                    )
                })?;
            let texture = physis::tex::Texture::from_existing(physis::Platform::Win32, &bytes)
                .ok_or_else(|| {
                    ResourceError::new(
                        ResourceErrorKind::DecodeFailed,
                        resource_kind.clone(),
                        Some(self.source()),
                        format!("failed to decode local icon texture {path}"),
                    )
                })?;
            let rgba = texture.to_rgba().ok_or_else(|| {
                ResourceError::new(
                    ResourceErrorKind::DecodeFailed,
                    resource_kind.clone(),
                    Some(self.source()),
                    format!("failed to convert local icon texture {path} to RGBA"),
                )
            })?;
            log::info(
                "resource",
                format!(
                    "decoded local ItemIcon {icon_id}: {}x{} in {}",
                    texture.width,
                    texture.height,
                    log::format_elapsed(log::elapsed_ms(start_ms)),
                ),
            );
            let info = ItemIconResourceInfo {
                icon_id,
                urls: Vec::new(),
                local_image: Some(LocalItemIconImage {
                    path,
                    width: texture.width,
                    height: texture.height,
                    rgba,
                }),
            };
            let bytes = serde_json::to_vec(&info).map_err(|error| {
                ResourceError::new(
                    ResourceErrorKind::ProviderFailed,
                    resource_kind,
                    Some(self.source()),
                    format!("failed to encode local item icon resource info: {error}"),
                )
            })?;
            Ok(ResourceBlob {
                bytes,
                fingerprint: None,
            })
        })
    }
}

fn browser_sqpack_provider_error_kind(error: &str) -> ResourceErrorKind {
    browser_sqpack_access_error_kind(error).unwrap_or(ResourceErrorKind::ProviderFailed)
}

fn browser_sqpack_not_found_or_access_error_kind(error: &str) -> ResourceErrorKind {
    browser_sqpack_access_error_kind(error).unwrap_or(ResourceErrorKind::NotFound)
}

fn browser_sqpack_access_error_kind(error: &str) -> Option<ResourceErrorKind> {
    if error.contains("尚未选择")
        || error.contains("权限")
        || error.contains("permission")
        || error.contains("denied")
        || error.contains("没有 window")
    {
        return Some(ResourceErrorKind::PermissionMissing);
    }

    if error.contains("没有 sqpack") {
        return Some(ResourceErrorKind::NotFound);
    }

    None
}

async fn load_weapon_catalog_from_browser_sqpack() -> Result<Vec<u8>, String> {
    let start_ms = log::now_ms();
    let open_start_ms = log::now_ms();
    let mut sqpack = BrowserSqPack::from_window_handle().await?;
    log::info(
        "resource",
        format!(
            "WeaponCatalog from_window_handle completed in {}",
            log::format_elapsed(log::elapsed_ms(open_start_ms)),
        ),
    );

    let fingerprint_start_ms = log::now_ms();
    let cache_fingerprint = match sqpack.weapon_catalog_cache_fingerprint().await {
        Ok(fingerprint) => {
            log::info(
                "resource",
                format!(
                    "WeaponCatalog cache fingerprint completed in {}",
                    log::format_elapsed(log::elapsed_ms(fingerprint_start_ms)),
                ),
            );
            Some(fingerprint)
        }
        Err(error) => {
            log::warn(
                "resource-cache",
                format!("WeaponCatalog cache fingerprint failed; skipping cache: {error}"),
            );
            None
        }
    };

    if let Some(cache_fingerprint) = cache_fingerprint.as_deref() {
        let cache_start_ms = log::now_ms();
        match load_cached_local_weapon_catalog(cache_fingerprint).await {
            Ok(Some((bytes, game_version))) => {
                let version_detail = game_version
                    .as_deref()
                    .map(|version| format!(" ({version})"))
                    .unwrap_or_default();
                log::info(
                    "resource",
                    format!(
                        "WeaponCatalog cache hit{version_detail}: {} bytes, cache lookup={}, total={}",
                        bytes.len(),
                        log::format_elapsed(log::elapsed_ms(cache_start_ms)),
                        log::format_elapsed(log::elapsed_ms(start_ms)),
                    ),
                );
                return Ok(bytes);
            }
            Ok(None) => {
                log::info(
                    "resource",
                    format!(
                        "WeaponCatalog cache miss checked in {}; exporting from SqPack",
                        log::format_elapsed(log::elapsed_ms(cache_start_ms)),
                    ),
                );
            }
            Err(error) => {
                log::warn(
                    "resource-cache",
                    format!("WeaponCatalog cache lookup failed; exporting from SqPack: {error}"),
                );
            }
        }
    }

    let preload_start_ms = log::now_ms();
    let resource = sqpack.preload_weapon_catalog_resource().await?;
    let preload_elapsed_ms = log::elapsed_ms(preload_start_ms);
    log::info(
        "resource",
        format!(
            "WeaponCatalog preload_weapon_catalog_resource completed in {}",
            log::format_elapsed(preload_elapsed_ms),
        ),
    );

    let generated_at = js_sys::Date::new_0()
        .to_iso_string()
        .as_string()
        .unwrap_or_else(|| "1970-01-01T00:00:00.000Z".to_string());
    let export_start_ms = log::now_ms();
    let data = xiv_companion::game_data::export_weapon_catalog_from_resource(
        resource,
        "Browser Local SqPack".to_string(),
        "Local SqPack".to_string(),
        generated_at,
    )
    .map_err(|error| format!("failed to export WeaponCatalog from local SqPack: {error:#}"))?;
    let export_elapsed_ms = log::elapsed_ms(export_start_ms);
    log::info(
        "resource",
        format!(
            "WeaponCatalog export_weapon_catalog_from_resource completed in {} ({} items, {} stains)",
            log::format_elapsed(export_elapsed_ms),
            data.counts.items,
            data.counts.stains,
        ),
    );

    let encode_start_ms = log::now_ms();
    let bytes = serde_json::to_vec(&data)
        .map_err(|error| format!("failed to encode WeaponCatalog: {error}"))?;
    let encode_elapsed_ms = log::elapsed_ms(encode_start_ms);
    log::info(
        "resource",
        format!(
            "WeaponCatalog serde_json encode completed in {} ({} bytes)",
            log::format_elapsed(encode_elapsed_ms),
            bytes.len(),
        ),
    );

    if let Some(cache_fingerprint) = cache_fingerprint.as_deref() {
        let save_start_ms = log::now_ms();
        match save_cached_local_weapon_catalog(cache_fingerprint, &data.game_version, &bytes).await
        {
            Ok(()) => {
                log::info(
                    "resource-cache",
                    format!(
                        "WeaponCatalog cache save completed in {}",
                        log::format_elapsed(log::elapsed_ms(save_start_ms)),
                    ),
                );
            }
            Err(error) => {
                log::warn(
                    "resource-cache",
                    format!("failed to save WeaponCatalog cache: {error}"),
                );
            }
        }
    }

    log::info(
        "resource",
        format!(
            "WeaponCatalog local pipeline completed: preload={}, export={}, encode={}, total={}",
            log::format_elapsed(preload_elapsed_ms),
            log::format_elapsed(export_elapsed_ms),
            log::format_elapsed(encode_elapsed_ms),
            log::format_elapsed(log::elapsed_ms(start_ms)),
        ),
    );
    Ok(bytes)
}

async fn load_weapon_model_from_browser_sqpack(
    id: xiv_companion::WeaponModelId,
) -> Result<xiv_companion::WeaponModelData, String> {
    let mut sqpack = BrowserSqPack::from_window_handle().await?;
    let mut resource = BrowserSqPackGameResource {
        sqpack: &mut sqpack,
    };
    let request = WeaponModelLoadRequest {
        item_id: id.item_id,
        item_name: id.item_name,
        model_main: id.model_main,
        model_sub: id.model_sub,
        stain_ids: id.stain_ids,
    };

    load_weapon_model_from_async_resource(&mut resource, &request)
        .await
        .map_err(|error| format!("{error:#}"))
}

struct BrowserSqPackGameResource<'a> {
    sqpack: &'a mut BrowserSqPack,
}

impl AsyncGameResource for BrowserSqPackGameResource<'_> {
    type Error = String;
    type ReadFuture<'a>
        = std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<u8>, String>> + 'a>>
    where
        Self: 'a;

    fn read<'a>(&'a mut self, path: &'a str) -> Self::ReadFuture<'a> {
        Box::pin(async move { self.sqpack.try_read_game_file(path).await })
    }

    fn platform(&self) -> physis::Platform {
        physis::Platform::Win32
    }
}

async fn load_from_browser_sqpack_direct(
    kind: &str,
    key: &str,
) -> Result<(Vec<u8>, Option<String>), String> {
    log::debug("resource", format!("BrowserSqPack request: {kind}/{key}"));
    let start_ms = log::now_ms();
    let mut sqpack = BrowserSqPack::from_window_handle().await?;
    match kind {
        "craft-data" => {
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
            if let Some((bytes, game_version)) =
                load_cached_local_craft_data(&cache_fingerprint).await?
            {
                report_craft_data_cache_status(Some(CraftDataCacheStatus::Hit {
                    bytes: bytes.len(),
                }));
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
            log::info(
                "resource",
                "local CraftData cache miss; exporting from SqPack",
            );
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
            report_craft_data_cache_status(Some(CraftDataCacheStatus::Saving {
                bytes: bytes.len(),
            }));
            save_cached_local_craft_data(&cache_fingerprint, &data.game_version, &bytes).await?;
            report_craft_data_cache_status(Some(CraftDataCacheStatus::Saved {
                bytes: bytes.len(),
            }));
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
        "item-icon" => {
            let icon_id = key
                .parse::<u32>()
                .map_err(|error| format!("invalid icon id {key}: {error}"))?;
            let path = item_icon_tex_path(icon_id);
            let bytes = sqpack
                .read_game_file_with_window(&path, ITEM_ICON_READ_WINDOW)
                .await?;
            Ok((bytes, None))
        }
        other => Err(format!("unknown BrowserSqPack request {other}")),
    }
}

async fn local_resource_cache_db() -> Result<indexed_db::Database<String>, String> {
    let factory =
        indexed_db::Factory::get().map_err(|error| format!("打开 IndexedDB 缓存失败: {error}"))?;
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
        log::info(
            "resource-cache",
            "CraftData cache miss: fingerprint changed",
        );
        return Ok(None);
    }

    let bytes =
        js_sys::Reflect::get(&record, &JsValue::from_str("bytes")).map_err(format_js_error)?;
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

async fn load_cached_local_weapon_catalog(
    expected_fingerprint: &str,
) -> Result<Option<(Vec<u8>, Option<String>)>, String> {
    let db = local_resource_cache_db().await?;
    let record = db
        .transaction(&[LOCAL_RESOURCE_CACHE_STORE])
        .run(|transaction| async move {
            transaction
                .object_store(LOCAL_RESOURCE_CACHE_STORE)?
                .get(&JsString::from(LOCAL_WEAPON_CATALOG_CACHE_KEY))
                .await
        })
        .await
        .map_err(|error| format!("读取本地 WeaponCatalog 缓存失败: {error}"))?;

    let Some(record) = record else {
        log::info("resource-cache", "WeaponCatalog cache miss: empty");
        return Ok(None);
    };

    let fingerprint = js_string_field(&record, "fingerprint");
    if fingerprint.as_deref() != Some(expected_fingerprint) {
        log::info(
            "resource-cache",
            "WeaponCatalog cache miss: fingerprint changed",
        );
        return Ok(None);
    }

    let bytes =
        js_sys::Reflect::get(&record, &JsValue::from_str("bytes")).map_err(format_js_error)?;
    if bytes.is_undefined() || bytes.is_null() {
        log::warn("resource-cache", "WeaponCatalog cache record has no bytes");
        return Ok(None);
    }

    let bytes = js_sys::Uint8Array::new(&bytes).to_vec();
    let game_version = js_string_field(&record, "gameVersion");
    log::info(
        "resource-cache",
        format!("WeaponCatalog cache hit: {} bytes", bytes.len()),
    );
    Ok(Some((bytes, game_version)))
}

async fn save_cached_local_weapon_catalog(
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
                .put_kv(&JsString::from(LOCAL_WEAPON_CATALOG_CACHE_KEY), &object)
                .await?;
            Ok(())
        })
        .await
        .map_err(|error| format!("保存本地 WeaponCatalog 缓存失败: {error}"))?;
    log::info(
        "resource-cache",
        format!("saved WeaponCatalog cache: {} bytes", bytes.len()),
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
