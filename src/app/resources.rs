use dioxus::prelude::*;
use physis::ReadableFile;
use xiv_companion::{
    AsyncGameResource, BuiltinItemIconProvider, ItemIconResourceInfo, LocalItemIconImage,
    ProviderRequest, ResourceBlob, ResourceError, ResourceErrorKind, ResourceFuture, ResourceHub,
    ResourceProvider, ResourceSource, WeaponModelLoadRequest, item_icon_tex_path,
    load_weapon_model_from_async_resource, register_craft_data_resource,
    register_item_icon_resource, register_weapon_model_resources,
    resources::{
        craft_data::CraftDataKind,
        item_icon::ItemIconKind,
        weapon_model::WeaponCatalogKind,
    },
};

use crate::app::browser_sqpack::BrowserSqPack;
use crate::app::indexed_db_cache::{CachedResourceRecord, load_cached_resource, save_cached_resource};
use crate::app::log;

const BUNDLED_CRAFT_DATA_ASSET: Asset = asset!("/assets/craft-data.json");
const BUNDLED_WEAPON_CATALOG_ASSET: Asset = asset!("/assets/weapon-catalog.json");
const ITEM_ICON_READ_WINDOW: u64 = 2 * 1024 * 1024;

const CACHED_CRAFT_DATA_KEY: &str = "craft-data";
const CACHED_WEAPON_CATALOG_KEY: &str = "weapon-catalog";

pub fn default_web_resource_hub() -> ResourceHub {
    let mut hub = ResourceHub::new();
    register_craft_data_resource(&mut hub);
    register_item_icon_resource(&mut hub);
    register_weapon_model_resources(&mut hub);
    hub.add_provider(BundledProvider);
    hub.add_provider(IndexedDbCachedProvider);
    hub.add_provider(BuiltinItemIconProvider);
    hub.add_provider(BrowserSqPackProvider);
    hub
}

pub struct IndexedDbCachedProvider;

impl IndexedDbCachedProvider {
    fn cache_key(request: &ProviderRequest) -> Option<&'static str> {
        if request.kind == CraftDataKind.into() && request.key == "default" {
            return Some(CACHED_CRAFT_DATA_KEY);
        }
        if request.kind == WeaponCatalogKind.into() && request.key == "default" {
            return Some(CACHED_WEAPON_CATALOG_KEY);
        }
        None
    }

    fn bundled_asset(request: &ProviderRequest) -> Option<Asset> {
        if request.kind == CraftDataKind.into() && request.key == "default" {
            return Some(BUNDLED_CRAFT_DATA_ASSET);
        }
        if request.kind == WeaponCatalogKind.into() && request.key == "default" {
            return Some(BUNDLED_WEAPON_CATALOG_ASSET);
        }
        None
    }

    fn game_version_from_bytes(
        request: &ProviderRequest,
        bytes: &[u8],
    ) -> Result<String, ResourceError> {
        if request.kind == CraftDataKind.into() {
            return serde_json::from_slice::<xiv_companion::CraftDataPackage>(bytes)
                .map(|package| package.game_version)
                .map_err(|error| {
                    ResourceError::new(
                        ResourceErrorKind::DecodeFailed,
                        request.kind.clone(),
                        Some(ResourceSource::Builtin),
                        format!("failed to decode bundled CraftData for indexing: {error}"),
                    )
                });
        }
        if request.kind == WeaponCatalogKind.into() {
            return serde_json::from_slice::<xiv_companion::WeaponCatalogPackage>(bytes)
                .map(|package| package.game_version)
                .map_err(|error| {
                    ResourceError::new(
                        ResourceErrorKind::DecodeFailed,
                        request.kind.clone(),
                        Some(ResourceSource::Builtin),
                        format!("failed to decode bundled WeaponCatalog for indexing: {error}"),
                    )
                });
        }
        Err(ResourceError::new(
            ResourceErrorKind::Unsupported,
            request.kind.clone(),
            Some(ResourceSource::IndexedDb),
            "cannot extract game version for this resource kind",
        ))
    }

    pub async fn update_from_local(request: ProviderRequest) -> Result<(), ResourceError> {
        let resource_kind = request.kind.clone();
        let Some(cache_key) = Self::cache_key(&request) else {
            return Err(ResourceError::new(
                ResourceErrorKind::Unsupported,
                resource_kind,
                Some(ResourceSource::IndexedDb),
                "IndexedDb cache does not support this request",
            ));
        };

        let (bytes, game_version) = if request.kind == CraftDataKind.into() {
            log::info("resource", "updating CraftData in IndexedDB from local SqPack");
            let mut sqpack = BrowserSqPack::from_window_handle().await.map_err(|error| {
                ResourceError::new(
                    browser_sqpack_provider_error_kind(&error),
                    resource_kind.clone(),
                    Some(ResourceSource::UserLocal),
                    error,
                )
            })?;
            let resource = sqpack.preload_craft_data_resource().await.map_err(|error| {
                ResourceError::new(
                    ResourceErrorKind::ProviderFailed,
                    resource_kind.clone(),
                    Some(ResourceSource::UserLocal),
                    error,
                )
            })?;
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
            .map_err(|error| {
                ResourceError::new(
                    ResourceErrorKind::DecodeFailed,
                    resource_kind.clone(),
                    Some(ResourceSource::UserLocal),
                    format!("failed to export CraftData from local SqPack: {error:#}"),
                )
            })?;
            let game_version = data.game_version.clone();
            let bytes = serde_json::to_vec(&data).map_err(|error| {
                ResourceError::new(
                    ResourceErrorKind::ProviderFailed,
                    resource_kind.clone(),
                    Some(ResourceSource::UserLocal),
                    format!("failed to encode local CraftData: {error}"),
                )
            })?;
            (bytes, game_version)
        } else if request.kind == WeaponCatalogKind.into() {
            log::info(
                "resource",
                "updating WeaponCatalog in IndexedDB from local SqPack",
            );
            let bytes = load_weapon_catalog_from_browser_sqpack().await.map_err(|error| {
                ResourceError::new(
                    browser_sqpack_provider_error_kind(&error),
                    resource_kind.clone(),
                    Some(ResourceSource::UserLocal),
                    error,
                )
            })?;
            let game_version = serde_json::from_slice::<xiv_companion::WeaponCatalogPackage>(&bytes)
                .map(|package| package.game_version)
                .unwrap_or_default();
            (bytes, game_version)
        } else {
            return Err(ResourceError::new(
                ResourceErrorKind::Unsupported,
                resource_kind,
                Some(ResourceSource::IndexedDb),
                "IndexedDb cache does not support this request",
            ));
        };

        let record = CachedResourceRecord {
            fingerprint: game_version.clone(),
            source_tag: "local".to_string(),
            game_version,
            bytes,
        };
        save_cached_resource(cache_key, &record)
            .await
            .map_err(|error| {
                ResourceError::new(
                    ResourceErrorKind::ProviderFailed,
                    request.kind,
                    Some(ResourceSource::IndexedDb),
                    error,
                )
            })?;
        Ok(())
    }

    pub async fn reset_to_builtin(request: ProviderRequest) -> Result<(), ResourceError> {
        let Some(cache_key) = Self::cache_key(&request) else {
            return Err(ResourceError::new(
                ResourceErrorKind::Unsupported,
                request.kind.clone(),
                Some(ResourceSource::IndexedDb),
                "IndexedDb cache does not support this request",
            ));
        };
        crate::app::indexed_db_cache::delete_cached_resource(cache_key)
            .await
            .map_err(|error| {
                ResourceError::new(
                    ResourceErrorKind::ProviderFailed,
                    request.kind,
                    Some(ResourceSource::IndexedDb),
                    error,
                )
            })
    }

    /// Returns the cached source tag (`"builtin"` / `"local"`) and game version for the request,
    /// or `None` if the resource has never been cached.
    pub async fn current_cache_info(
        request: ProviderRequest,
    ) -> Result<Option<(String, String)>, ResourceError> {
        let resource_kind = request.kind.clone();
        let Some(cache_key) = Self::cache_key(&request) else {
            return Err(ResourceError::new(
                ResourceErrorKind::Unsupported,
                resource_kind,
                Some(ResourceSource::IndexedDb),
                "IndexedDb cache does not support this request",
            ));
        };
        load_cached_resource(cache_key)
            .await
            .map_err(|error| {
                ResourceError::new(
                    ResourceErrorKind::ProviderFailed,
                    request.kind,
                    Some(ResourceSource::IndexedDb),
                    error,
                )
            })
            .map(|record| record.map(|r| (r.source_tag, r.game_version)))
    }
}

impl ResourceProvider for IndexedDbCachedProvider {
    fn source(&self) -> ResourceSource {
        ResourceSource::IndexedDb
    }

    fn supports(&self, request: &ProviderRequest) -> bool {
        Self::cache_key(request).is_some()
    }

    fn read<'a>(
        &'a self,
        request: ProviderRequest,
    ) -> ResourceFuture<'a, Result<ResourceBlob, ResourceError>> {
        Box::pin(async move {
            let resource_kind = request.kind.clone();
            let Some(cache_key) = Self::cache_key(&request) else {
                return Err(ResourceError::new(
                    ResourceErrorKind::Unsupported,
                    resource_kind,
                    Some(ResourceSource::IndexedDb),
                    "IndexedDb cache does not support this request",
                ));
            };
            let Some(asset) = Self::bundled_asset(&request) else {
                return Err(ResourceError::new(
                    ResourceErrorKind::Unsupported,
                    resource_kind,
                    Some(ResourceSource::IndexedDb),
                    "no bundled asset for this request",
                ));
            };

            if let Some(record) = load_cached_resource(cache_key).await.map_err(|error| {
                ResourceError::new(
                    ResourceErrorKind::ProviderFailed,
                    resource_kind.clone(),
                    Some(ResourceSource::IndexedDb),
                    error,
                )
            })? {
                log::info(
                    "resource",
                    format!(
                        "loaded {cache_key} from IndexedDB (source={}, version={})",
                        record.source_tag, record.game_version
                    ),
                );
                return Ok(ResourceBlob {
                    bytes: record.bytes,
                    fingerprint: Some(record.fingerprint),
                });
            }

            log::info(
                "resource",
                format!("{cache_key} not in IndexedDB; seeding from builtin asset"),
            );
            let bytes = dioxus::asset_resolver::read_asset_bytes(asset).await.map_err(|error| {
                ResourceError::new(
                    ResourceErrorKind::ProviderFailed,
                    resource_kind.clone(),
                    Some(ResourceSource::Builtin),
                    format!("failed to read bundled asset: {error}"),
                )
            })?;
            let game_version = Self::game_version_from_bytes(&request, &bytes)?;
            let record = CachedResourceRecord {
                fingerprint: game_version.clone(),
                source_tag: "builtin".to_string(),
                game_version: game_version.clone(),
                bytes: bytes.clone(),
            };
            if let Err(error) = save_cached_resource(cache_key, &record).await {
                log::warn("resource", format!("failed to seed {cache_key} in IndexedDB: {error}"));
            }
            Ok(ResourceBlob {
                bytes,
                fingerprint: Some(game_version),
            })
        })
    }
}

pub struct BrowserSqPackProvider;

impl ResourceProvider for BrowserSqPackProvider {
    fn source(&self) -> ResourceSource {
        ResourceSource::UserLocal
    }

    fn supports(&self, request: &ProviderRequest) -> bool {
        request.kind == ItemIconKind.into() && request.key.parse::<u32>().is_ok()
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
            let bytes = load_item_icon_from_browser_sqpack(icon_id)
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
    let mut sqpack = BrowserSqPack::from_window_handle().await?;

    let preload_start_ms = log::now_ms();
    let resource = sqpack.preload_weapon_catalog_resource().await?;
    let preload_elapsed_ms = log::elapsed_ms(preload_start_ms);
    log::info(
        "resource",
        format!(
            "WeaponCatalog preload completed in {}",
            log::format_elapsed(preload_elapsed_ms),
        ),
    );

    let generated_at = js_sys::Date::new_0()
        .to_iso_string()
        .as_string()
        .unwrap_or_else(|| "1970-01-01T00:00:00.000Z".to_string());
    let data = xiv_companion::game_data::export_weapon_catalog_from_resource(
        resource,
        "Browser Local SqPack".to_string(),
        "Local SqPack".to_string(),
        generated_at,
    )
    .map_err(|error| format!("failed to export WeaponCatalog from local SqPack: {error:#}"))?;

    let bytes = serde_json::to_vec(&data)
        .map_err(|error| format!("failed to encode WeaponCatalog: {error}"))?;
    log::info(
        "resource",
        format!(
            "WeaponCatalog exported in {} ({} items, {} bytes)",
            log::format_elapsed(log::elapsed_ms(start_ms)),
            data.counts.items,
            bytes.len(),
        ),
    );
    Ok(bytes)
}

pub async fn load_weapon_model_from_local(
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

async fn load_item_icon_from_browser_sqpack(icon_id: u32) -> Result<Vec<u8>, String> {
    log::debug("resource", format!("BrowserSqPack request: item-icon/{icon_id}"));
    let mut sqpack = BrowserSqPack::from_window_handle().await?;
    let path = item_icon_tex_path(icon_id);
    sqpack.read_game_file_with_window(&path, ITEM_ICON_READ_WINDOW).await
}

pub struct BundledProvider;

impl ResourceProvider for BundledProvider {
    fn source(&self) -> ResourceSource {
        ResourceSource::Builtin
    }

    fn supports(&self, request: &ProviderRequest) -> bool {
        (request.kind == CraftDataKind.into() && request.key == "default")
            || (request.kind == WeaponCatalogKind.into() && request.key == "default")
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

            let asset = if request.kind == CraftDataKind.into() {
                BUNDLED_CRAFT_DATA_ASSET
            } else {
                BUNDLED_WEAPON_CATALOG_ASSET
            };
            let bytes = dioxus::asset_resolver::read_asset_bytes(asset)
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
