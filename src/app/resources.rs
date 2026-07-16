use dioxus::prelude::*;
use physis::ReadableFile;
use serde::Deserialize;
use xiv_companion::{
    AsyncGameResource, BuiltinItemIconProvider, ItemIconResourceInfo, LocalItemIconImage,
    ProviderRequest, ResourceBlob, ResourceError, ResourceErrorKind, ResourceFuture, ResourceHub,
    ResourceMetadata, ResourceOrigin, ResourceProvider, ResourceSource, ResourceStatus,
    WeaponModelLoadRequest, WeaponStainingTemplates, compare_resource_versions, item_icon_tex_path,
    load_weapon_model_from_async_resource, register_collection_catalog_resource,
    register_craft_data_resource, register_item_icon_resource, register_weapon_model_resources,
    resources::{
        collection_catalog::CollectionCatalogKind, craft_data::CraftDataKind,
        item_icon::ItemIconKind, weapon_model::WeaponCatalogKind,
    },
};

use crate::app::browser_sqpack::BrowserSqPack;
use crate::app::indexed_db_cache::{
    CachedResourceRecord, load_cached_resource, save_cached_resource,
};
use crate::app::load_progress::{WeaponModelLoadProgress, report_weapon_model_progress};
use crate::app::log;

const BUNDLED_CRAFT_DATA_ASSET: Asset = asset!("/assets/craft-data.json");
const BUNDLED_WEAPON_CATALOG_ASSET: Asset = asset!("/assets/weapon-catalog.json");
const BUNDLED_COLLECTION_CATALOG_ASSET: Asset = asset!("/assets/collection-catalog.json");
const BUNDLED_RESOURCE_MANIFEST_ASSET: Asset = asset!("/assets/resource-manifest.json");
const ITEM_ICON_READ_WINDOW: u64 = 2 * 1024 * 1024;

const CACHED_CRAFT_DATA_KEY: &str = "craft-data";
const CACHED_WEAPON_CATALOG_KEY: &str = "weapon-catalog";
const CACHED_COLLECTION_CATALOG_KEY: &str = "collection-catalog";

#[derive(Clone, Debug)]
struct CachedPackageInfo {
    game_version: String,
    revision: String,
    record_count: usize,
    schema_revision: Option<u32>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BundledResourceManifest {
    resources: std::collections::HashMap<String, BundledResourceManifestEntry>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BundledResourceManifestEntry {
    game_version: String,
    revision: String,
    schema_revision: u32,
    record_count: usize,
}

pub fn default_web_resource_hub() -> ResourceHub {
    let mut hub = ResourceHub::new();
    register_collection_catalog_resource(&mut hub);
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
    fn schema_revision(request: &ProviderRequest) -> u32 {
        if request.kind == CollectionCatalogKind.into() {
            xiv_companion::COLLECTION_CATALOG_SCHEMA_VERSION
        } else if request.kind == WeaponCatalogKind.into() {
            xiv_companion::WEAPON_CATALOG_SCHEMA_REVISION
        } else {
            1
        }
    }

    fn cache_key(request: &ProviderRequest) -> Option<&'static str> {
        if request.kind == CraftDataKind.into() && request.key == "default" {
            return Some(CACHED_CRAFT_DATA_KEY);
        }
        if request.kind == WeaponCatalogKind.into() && request.key == "default" {
            return Some(CACHED_WEAPON_CATALOG_KEY);
        }
        if request.kind == CollectionCatalogKind.into() && request.key == "default" {
            return Some(CACHED_COLLECTION_CATALOG_KEY);
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
        if request.kind == CollectionCatalogKind.into() && request.key == "default" {
            return Some(BUNDLED_COLLECTION_CATALOG_ASSET);
        }
        None
    }

    fn manifest_key(request: &ProviderRequest) -> Option<&'static str> {
        Self::cache_key(request)
    }

    async fn bundled_manifest_entry(
        request: &ProviderRequest,
    ) -> Result<BundledResourceManifestEntry, ResourceError> {
        let key = Self::manifest_key(request).ok_or_else(|| {
            ResourceError::new(
                ResourceErrorKind::Unsupported,
                request.kind.clone(),
                Some(ResourceSource::Builtin),
                "resource is not present in the bundled manifest",
            )
        })?;
        let bytes = dioxus::asset_resolver::read_asset_bytes(BUNDLED_RESOURCE_MANIFEST_ASSET)
            .await
            .map_err(|error| {
                ResourceError::new(
                    ResourceErrorKind::ProviderFailed,
                    request.kind.clone(),
                    Some(ResourceSource::Builtin),
                    format!("failed to read bundled resource manifest: {error}"),
                )
            })?;
        let manifest =
            serde_json::from_slice::<BundledResourceManifest>(&bytes).map_err(|error| {
                ResourceError::new(
                    ResourceErrorKind::DecodeFailed,
                    request.kind.clone(),
                    Some(ResourceSource::Builtin),
                    format!("failed to decode bundled resource manifest: {error}"),
                )
            })?;
        manifest.resources.get(key).cloned().ok_or_else(|| {
            ResourceError::new(
                ResourceErrorKind::NotFound,
                request.kind.clone(),
                Some(ResourceSource::Builtin),
                format!("bundled resource manifest has no entry for {key}"),
            )
        })
    }

    fn package_info_from_bytes(
        request: &ProviderRequest,
        bytes: &[u8],
    ) -> Result<CachedPackageInfo, ResourceError> {
        if request.kind == CraftDataKind.into() {
            return serde_json::from_slice::<xiv_companion::CraftDataPackage>(bytes)
                .map(|package| CachedPackageInfo {
                    game_version: package.game_version,
                    revision: package.generated_at,
                    record_count: package.counts.items,
                    schema_revision: None,
                })
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
                .map(|package| CachedPackageInfo {
                    game_version: package.game_version,
                    revision: package.generated_at,
                    record_count: package.counts.items,
                    schema_revision: None,
                })
                .map_err(|error| {
                    ResourceError::new(
                        ResourceErrorKind::DecodeFailed,
                        request.kind.clone(),
                        Some(ResourceSource::Builtin),
                        format!("failed to decode bundled WeaponCatalog for indexing: {error}"),
                    )
                });
        }
        if request.kind == CollectionCatalogKind.into() {
            return serde_json::from_slice::<xiv_companion::CollectionCatalogPackage>(bytes)
                .map(|package| CachedPackageInfo {
                    game_version: package.game_version,
                    revision: package.generated_at,
                    record_count: package.counts.items,
                    schema_revision: Some(package.schema_version),
                })
                .map_err(|error| {
                    ResourceError::new(
                        ResourceErrorKind::DecodeFailed,
                        request.kind.clone(),
                        Some(ResourceSource::Builtin),
                        format!("failed to decode bundled CollectionCatalog for indexing: {error}"),
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

    async fn bundled_record(
        request: &ProviderRequest,
    ) -> Result<CachedResourceRecord, ResourceError> {
        let asset = Self::bundled_asset(request).ok_or_else(|| {
            ResourceError::new(
                ResourceErrorKind::Unsupported,
                request.kind.clone(),
                Some(ResourceSource::Builtin),
                "no bundled asset for this request",
            )
        })?;
        let bytes = dioxus::asset_resolver::read_asset_bytes(asset)
            .await
            .map_err(|error| {
                ResourceError::new(
                    ResourceErrorKind::ProviderFailed,
                    request.kind.clone(),
                    Some(ResourceSource::Builtin),
                    format!("failed to read bundled asset: {error}"),
                )
            })?;
        let info = Self::package_info_from_bytes(request, &bytes)?;
        let schema_revision = Self::schema_revision(request);
        Ok(CachedResourceRecord {
            fingerprint: format!("{}:{}:{schema_revision}", info.game_version, info.revision),
            source_tag: ResourceOrigin::Builtin.id().to_string(),
            game_version: info.game_version,
            schema_revision,
            record_count: info.record_count,
            saved_at: now_iso_string(),
            bytes,
        })
    }

    async fn update_from_local(request: ProviderRequest) -> Result<ResourceStatus, ResourceError> {
        let resource_kind = request.kind.clone();
        let Some(cache_key) = Self::cache_key(&request) else {
            return Err(ResourceError::new(
                ResourceErrorKind::Unsupported,
                resource_kind,
                Some(ResourceSource::IndexedDb),
                "IndexedDb cache does not support this request",
            ));
        };

        let bytes = if request.kind == CraftDataKind.into() {
            log::info(
                "resource",
                "updating CraftData in IndexedDB from local SqPack",
            );
            let mut sqpack = BrowserSqPack::from_window_handle().await.map_err(|error| {
                ResourceError::new(
                    browser_sqpack_provider_error_kind(&error),
                    resource_kind.clone(),
                    Some(ResourceSource::UserLocal),
                    error,
                )
            })?;
            let resource = sqpack
                .preload_craft_data_resource()
                .await
                .map_err(|error| {
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
            let game_version = sqpack
                .game_version()
                .await
                .unwrap_or_else(|_| "未知本地版本".to_string());
            let data = xiv_companion::game_data::export_craft_data_from_resource(
                resource,
                "Browser Local SqPack".to_string(),
                game_version,
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
            serde_json::to_vec(&data).map_err(|error| {
                ResourceError::new(
                    ResourceErrorKind::ProviderFailed,
                    resource_kind.clone(),
                    Some(ResourceSource::UserLocal),
                    format!("failed to encode local CraftData: {error}"),
                )
            })?
        } else if request.kind == WeaponCatalogKind.into() {
            log::info(
                "resource",
                "updating WeaponCatalog in IndexedDB from local SqPack",
            );
            let bytes = load_weapon_catalog_from_browser_sqpack()
                .await
                .map_err(|error| {
                    ResourceError::new(
                        browser_sqpack_provider_error_kind(&error),
                        resource_kind.clone(),
                        Some(ResourceSource::UserLocal),
                        error,
                    )
                })?;
            bytes
        } else if request.kind == CollectionCatalogKind.into() {
            log::info(
                "resource",
                "updating CollectionCatalog in IndexedDB from local SqPack",
            );
            let bytes = load_collection_catalog_from_browser_sqpack()
                .await
                .map_err(|error| {
                    ResourceError::new(
                        browser_sqpack_provider_error_kind(&error),
                        resource_kind.clone(),
                        Some(ResourceSource::UserLocal),
                        error,
                    )
                })?;
            bytes
        } else {
            return Err(ResourceError::new(
                ResourceErrorKind::Unsupported,
                resource_kind,
                Some(ResourceSource::IndexedDb),
                "IndexedDb cache does not support this request",
            ));
        };

        let info = Self::package_info_from_bytes(&request, &bytes)?;
        let schema_revision = Self::schema_revision(&request);
        let record = CachedResourceRecord {
            fingerprint: format!("{}:{}:{schema_revision}", info.game_version, info.revision),
            source_tag: ResourceOrigin::UserLocal.id().to_string(),
            game_version: info.game_version,
            schema_revision,
            record_count: info.record_count,
            saved_at: now_iso_string(),
            bytes,
        };
        save_cached_resource(cache_key, &record)
            .await
            .map_err(|error| {
                ResourceError::new(
                    ResourceErrorKind::ProviderFailed,
                    request.kind.clone(),
                    Some(ResourceSource::IndexedDb),
                    error,
                )
            })?;
        Ok(resource_status_from_record(request.kind, &record))
    }

    async fn reset_to_builtin(request: ProviderRequest) -> Result<ResourceStatus, ResourceError> {
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
                    request.kind.clone(),
                    Some(ResourceSource::IndexedDb),
                    error,
                )
            })?;
        let record = Self::bundled_record(&request).await?;
        save_cached_resource(cache_key, &record)
            .await
            .map_err(|error| {
                ResourceError::new(
                    ResourceErrorKind::ProviderFailed,
                    request.kind.clone(),
                    Some(ResourceSource::IndexedDb),
                    error,
                )
            })?;
        Ok(resource_status_from_record(request.kind, &record))
    }

    async fn cache_status(request: ProviderRequest) -> Result<ResourceStatus, ResourceError> {
        let Some(cache_key) = Self::cache_key(&request) else {
            return Err(ResourceError::new(
                ResourceErrorKind::Unsupported,
                request.kind,
                Some(ResourceSource::IndexedDb),
                "IndexedDb cache does not support this request",
            ));
        };
        let record = load_cached_resource(cache_key).await.map_err(|error| {
            ResourceError::new(
                ResourceErrorKind::ProviderFailed,
                request.kind.clone(),
                Some(ResourceSource::IndexedDb),
                error,
            )
        })?;
        Ok(match record {
            Some(record) => resource_status_from_record(request.kind, &record),
            None => ResourceStatus {
                resource: request.kind,
                storage: ResourceSource::IndexedDb,
                available: false,
                metadata: ResourceMetadata::default(),
            },
        })
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
            let cached = load_cached_resource(cache_key).await.map_err(|error| {
                ResourceError::new(
                    ResourceErrorKind::ProviderFailed,
                    resource_kind.clone(),
                    Some(ResourceSource::IndexedDb),
                    error,
                )
            })?;

            if let Some(record) = cached {
                let mut incompatible_local = false;
                if record.source_tag == ResourceOrigin::UserLocal.id() {
                    let expected_schema = Self::schema_revision(&request);
                    if let Ok(info) = Self::package_info_from_bytes(&request, &record.bytes) {
                        let package_is_compatible = info
                            .schema_revision
                            .map(|schema| schema >= expected_schema)
                            .unwrap_or(record.schema_revision >= expected_schema);
                        if package_is_compatible {
                            let upgraded = CachedResourceRecord {
                                fingerprint: format!(
                                    "{}:{}:{expected_schema}",
                                    info.game_version, info.revision
                                ),
                                source_tag: record.source_tag,
                                game_version: info.game_version,
                                schema_revision: expected_schema,
                                record_count: info.record_count,
                                saved_at: record.saved_at,
                                bytes: record.bytes,
                            };
                            save_cached_resource(cache_key, &upgraded)
                                .await
                                .map_err(|error| {
                                    ResourceError::new(
                                        ResourceErrorKind::ProviderFailed,
                                        resource_kind.clone(),
                                        Some(ResourceSource::IndexedDb),
                                        error,
                                    )
                                })?;
                            let metadata = resource_metadata_from_record(&upgraded);
                            return Ok(ResourceBlob {
                                bytes: upgraded.bytes,
                                fingerprint: Some(upgraded.fingerprint),
                                metadata,
                            });
                        }
                    }
                    log::warn(
                        "resource",
                        format!(
                            "discarding incompatible local {cache_key} schema {} (expected {expected_schema})",
                            record.schema_revision
                        ),
                    );
                    incompatible_local = true;
                }

                let bundled_info = Self::bundled_manifest_entry(&request).await?;
                if !incompatible_local && !should_replace_builtin_cache(&record, &bundled_info) {
                    let metadata = resource_metadata_from_record(&record);
                    return Ok(ResourceBlob {
                        bytes: record.bytes,
                        fingerprint: Some(record.fingerprint),
                        metadata,
                    });
                }
                let bundled = Self::bundled_record(&request).await?;
                log::info(
                    "resource",
                    format!(
                        "upgrading {cache_key} builtin cache from {} to {}",
                        record.game_version, bundled.game_version
                    ),
                );
                save_cached_resource(cache_key, &bundled)
                    .await
                    .map_err(|error| {
                        ResourceError::new(
                            ResourceErrorKind::ProviderFailed,
                            resource_kind,
                            Some(ResourceSource::IndexedDb),
                            error,
                        )
                    })?;
                let metadata = resource_metadata_from_record(&bundled);
                return Ok(ResourceBlob {
                    bytes: bundled.bytes,
                    fingerprint: Some(bundled.fingerprint),
                    metadata,
                });
            }

            log::info(
                "resource",
                format!("{cache_key} not in IndexedDB; seeding from builtin asset"),
            );
            let record = Self::bundled_record(&request).await?;
            if let Err(error) = save_cached_resource(cache_key, &record).await {
                log::warn(
                    "resource",
                    format!("failed to seed {cache_key} in IndexedDB: {error}"),
                );
            }
            let metadata = resource_metadata_from_record(&record);
            Ok(ResourceBlob {
                bytes: record.bytes,
                fingerprint: Some(record.fingerprint),
                metadata,
            })
        })
    }

    fn status<'a>(
        &'a self,
        request: ProviderRequest,
    ) -> ResourceFuture<'a, Result<ResourceStatus, ResourceError>> {
        Box::pin(Self::cache_status(request))
    }

    fn refresh<'a>(
        &'a self,
        request: ProviderRequest,
        origin: ResourceOrigin,
    ) -> ResourceFuture<'a, Result<ResourceStatus, ResourceError>> {
        Box::pin(async move {
            match origin {
                ResourceOrigin::Builtin => Self::reset_to_builtin(request).await,
                ResourceOrigin::UserLocal => Self::update_from_local(request).await,
                ResourceOrigin::Network => Err(ResourceError::new(
                    ResourceErrorKind::Unsupported,
                    request.kind,
                    Some(ResourceSource::IndexedDb),
                    "network refresh is not supported for this resource",
                )),
            }
        })
    }

    fn reset<'a>(
        &'a self,
        request: ProviderRequest,
    ) -> ResourceFuture<'a, Result<ResourceStatus, ResourceError>> {
        Box::pin(Self::reset_to_builtin(request))
    }
}

fn resource_metadata_from_record(record: &CachedResourceRecord) -> ResourceMetadata {
    ResourceMetadata {
        origin: match record.source_tag.as_str() {
            "builtin" => Some(ResourceOrigin::Builtin),
            "local" => Some(ResourceOrigin::UserLocal),
            "network" => Some(ResourceOrigin::Network),
            _ => None,
        },
        game_version: Some(record.game_version.clone()),
        revision: Some(record.fingerprint.clone()),
        saved_at: (!record.saved_at.is_empty()).then(|| record.saved_at.clone()),
        record_count: Some(record.record_count),
    }
}

fn resource_status_from_record(
    resource: xiv_companion::ResourceKindKey,
    record: &CachedResourceRecord,
) -> ResourceStatus {
    ResourceStatus {
        resource,
        storage: ResourceSource::IndexedDb,
        available: true,
        metadata: resource_metadata_from_record(record),
    }
}

fn should_replace_builtin_cache(
    cached: &CachedResourceRecord,
    bundled: &BundledResourceManifestEntry,
) -> bool {
    if cached.schema_revision < bundled.schema_revision {
        return true;
    }
    let version_order = compare_resource_versions(&bundled.game_version, &cached.game_version);
    let expected_fingerprint = format!(
        "{}:{}:{}",
        bundled.game_version, bundled.revision, bundled.schema_revision
    );
    version_order.is_gt()
        || (version_order.is_eq()
            && cached.schema_revision == bundled.schema_revision
            && (cached.record_count != bundled.record_count
                || cached.fingerprint != expected_fingerprint))
}

fn now_iso_string() -> String {
    js_sys::Date::new_0()
        .to_iso_string()
        .as_string()
        .unwrap_or_else(|| "1970-01-01T00:00:00.000Z".to_string())
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
                metadata: xiv_companion::ResourceMetadata {
                    origin: Some(xiv_companion::ResourceOrigin::UserLocal),
                    ..Default::default()
                },
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
    let game_version = sqpack
        .game_version()
        .await
        .unwrap_or_else(|_| "未知本地版本".to_string());

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
    let export_start_ms = log::now_ms();
    let data = xiv_companion::game_data::export_weapon_catalog_from_resource(
        resource,
        "Browser Local SqPack".to_string(),
        game_version,
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

async fn load_collection_catalog_from_browser_sqpack() -> Result<Vec<u8>, String> {
    let start_ms = log::now_ms();
    let mut sqpack = BrowserSqPack::from_window_handle().await?;
    let game_version = sqpack
        .game_version()
        .await
        .unwrap_or_else(|_| "未知本地版本".to_string());

    let preload_start_ms = log::now_ms();
    let resource = sqpack.preload_collection_catalog_resource().await?;
    let preload_elapsed_ms = log::elapsed_ms(preload_start_ms);
    log::info(
        "resource",
        format!(
            "CollectionCatalog preload completed in {}",
            log::format_elapsed(preload_elapsed_ms),
        ),
    );

    let generated_at = js_sys::Date::new_0()
        .to_iso_string()
        .as_string()
        .unwrap_or_else(|| "1970-01-01T00:00:00.000Z".to_string());
    let mut data = xiv_companion::game_data::export_collection_catalog_from_resource(
        resource,
        "Browser Local SqPack".to_string(),
        game_version.clone(),
        generated_at,
    )
    .map_err(|error| format!("failed to export CollectionCatalog from local SqPack: {error:#}"))?;
    if let Ok(bytes) =
        dioxus::asset_resolver::read_asset_bytes(BUNDLED_COLLECTION_CATALOG_ASSET).await
    {
        if let Ok(bundled) =
            serde_json::from_slice::<xiv_companion::CollectionCatalogPackage>(&bytes)
        {
            let release_by_id = bundled
                .items
                .into_iter()
                .map(|item| (item.id, (item.expansion, item.patch)))
                .collect::<std::collections::HashMap<_, _>>();
            for item in &mut data.items {
                if let Some((expansion, patch)) = release_by_id.get(&item.id) {
                    item.expansion.clone_from(expansion);
                    item.patch.clone_from(patch);
                } else {
                    if let Some((expansion, patch_series)) = local_release_bucket(&game_version) {
                        item.expansion = expansion.to_string();
                        item.patch = format!("{patch_series}（本地新增）");
                    } else {
                        item.expansion = "本地版本".to_string();
                        item.patch = game_version.clone();
                    }
                }
            }
        }
    }

    let bytes = serde_json::to_vec(&data)
        .map_err(|error| format!("failed to encode CollectionCatalog: {error}"))?;
    log::info(
        "resource",
        format!(
            "CollectionCatalog exported in {} ({} items, {} equipment, {} bytes)",
            log::format_elapsed(log::elapsed_ms(start_ms)),
            data.counts.items,
            data.counts.equipment,
            bytes.len(),
        ),
    );
    Ok(bytes)
}

fn local_release_bucket(game_version: &str) -> Option<(&'static str, &'static str)> {
    let year = game_version
        .split(|character: char| !character.is_ascii_digit())
        .find(|part| part.len() == 4)?
        .parse::<u16>()
        .ok()?;
    match year {
        2024.. => Some(("金曦之遗辉", "7.x")),
        2022..=2023 => Some(("晓月之终途", "6.x")),
        2019..=2021 => Some(("暗影之逆焰", "5.x")),
        2017..=2018 => Some(("红莲之狂潮", "4.x")),
        2015..=2016 => Some(("苍穹之禁城", "3.x")),
        2013..=2014 => Some(("重生之境", "2.x")),
        _ => None,
    }
}

#[cfg(test)]
mod local_release_tests {
    use super::{
        BundledResourceManifestEntry, CachedResourceRecord, local_release_bucket,
        should_replace_builtin_cache,
    };

    #[test]
    fn local_build_is_grouped_into_its_expansion() {
        assert_eq!(
            local_release_bucket("2026.06.18.0000.0000"),
            Some(("金曦之遗辉", "7.x"))
        );
        assert_eq!(
            local_release_bucket("game-2023.10.03.0000.0000"),
            Some(("晓月之终途", "6.x"))
        );
        assert_eq!(local_release_bucket("unknown"), None);
    }

    #[test]
    fn collection_schema_upgrade_replaces_older_local_cache() {
        let cached = CachedResourceRecord {
            fingerprint: "local:old:3".to_string(),
            source_tag: "local".to_string(),
            game_version: "2026.06.18.0000.0000".to_string(),
            schema_revision: 11,
            record_count: 30_000,
            saved_at: String::new(),
            bytes: Vec::new(),
        };
        let bundled = BundledResourceManifestEntry {
            game_version: "game-2025.10.23.0000.0000".to_string(),
            revision: "2026-07-11T18:26:09Z".to_string(),
            schema_revision: 12,
            record_count: 29_985,
        };
        assert!(should_replace_builtin_cache(&cached, &bundled));
    }
}

pub async fn load_weapon_model_from_local(
    id: xiv_companion::WeaponModelId,
) -> Result<xiv_companion::WeaponModelData, String> {
    let started_at_ms = log::now_ms();
    report_weapon_model_progress(Some(WeaponModelLoadProgress {
        item_id: id.item_id,
        stain_ids: id.stain_ids,
        stage: "连接本地游戏目录".to_string(),
        detail: id.item_name.clone(),
        checked_resources: 0,
        loaded_resources: 0,
        loaded_bytes: 0,
        elapsed_ms: 0.0,
        done: false,
    }));
    let mut sqpack = match BrowserSqPack::from_window_handle().await {
        Ok(sqpack) => sqpack,
        Err(error) => {
            report_weapon_model_progress(Some(WeaponModelLoadProgress {
                item_id: id.item_id,
                stain_ids: id.stain_ids,
                stage: "无法读取本地游戏目录".to_string(),
                detail: error.clone(),
                checked_resources: 0,
                loaded_resources: 0,
                loaded_bytes: 0,
                elapsed_ms: log::elapsed_ms(started_at_ms),
                done: true,
            }));
            return Err(error);
        }
    };
    let mut resource = BrowserSqPackGameResource {
        sqpack: &mut sqpack,
        item_id: id.item_id,
        stain_ids: id.stain_ids,
        started_at_ms,
        checked_resources: 0,
        loaded_resources: 0,
        loaded_bytes: 0,
    };
    let request = WeaponModelLoadRequest {
        item_id: id.item_id,
        item_name: id.item_name,
        model_main: id.model_main,
        model_sub: id.model_sub,
        stain_ids: id.stain_ids,
    };

    let result = load_weapon_model_from_async_resource(&mut resource, &request)
        .await
        .map_err(|error| format!("{error:#}"));
    let (stage, detail) = match &result {
        Ok(data) => (
            "模型资源已就绪",
            format!(
                "{} 个网格 · {} 个材质 · {} 张纹理",
                data.meshes.len(),
                data.materials.len(),
                data.textures.len()
            ),
        ),
        Err(error) => ("模型读取失败", error.clone()),
    };
    report_weapon_model_progress(Some(WeaponModelLoadProgress {
        item_id: id.item_id,
        stain_ids: id.stain_ids,
        stage: stage.to_string(),
        detail,
        checked_resources: resource.checked_resources,
        loaded_resources: resource.loaded_resources,
        loaded_bytes: resource.loaded_bytes,
        elapsed_ms: log::elapsed_ms(started_at_ms),
        done: true,
    }));
    result
}

pub async fn load_weapon_staining_templates_from_local() -> Result<WeaponStainingTemplates, String>
{
    let mut sqpack = BrowserSqPack::from_window_handle().await?;
    let legacy = sqpack
        .try_read_game_file(xiv_companion::LEGACY_STAINING_TEMPLATE_PATH)
        .await;
    let dawntrail = sqpack
        .try_read_game_file(xiv_companion::DAWNTRAIL_STAINING_TEMPLATE_PATH)
        .await;
    Ok(WeaponStainingTemplates::from_load_results(
        legacy, dawntrail,
    ))
}

struct BrowserSqPackGameResource<'a> {
    sqpack: &'a mut BrowserSqPack,
    item_id: u32,
    stain_ids: [u8; 2],
    started_at_ms: f64,
    checked_resources: u32,
    loaded_resources: u32,
    loaded_bytes: u64,
}

impl AsyncGameResource for BrowserSqPackGameResource<'_> {
    type Error = String;
    type ReadFuture<'a>
        = std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<u8>, String>> + 'a>>
    where
        Self: 'a;

    fn read<'a>(&'a mut self, path: &'a str) -> Self::ReadFuture<'a> {
        Box::pin(async move {
            report_weapon_model_progress(Some(WeaponModelLoadProgress {
                item_id: self.item_id,
                stain_ids: self.stain_ids,
                stage: weapon_model_resource_stage(path).to_string(),
                detail: compact_weapon_resource_path(path),
                checked_resources: self.checked_resources,
                loaded_resources: self.loaded_resources,
                loaded_bytes: self.loaded_bytes,
                elapsed_ms: log::elapsed_ms(self.started_at_ms),
                done: false,
            }));

            let result = self.sqpack.try_read_game_file(path).await;
            self.checked_resources = self.checked_resources.saturating_add(1);
            if let Ok(bytes) = &result {
                self.loaded_resources = self.loaded_resources.saturating_add(1);
                self.loaded_bytes = self.loaded_bytes.saturating_add(bytes.len() as u64);
            }
            report_weapon_model_progress(Some(WeaponModelLoadProgress {
                item_id: self.item_id,
                stain_ids: self.stain_ids,
                stage: weapon_model_resource_stage(path).to_string(),
                detail: compact_weapon_resource_path(path),
                checked_resources: self.checked_resources,
                loaded_resources: self.loaded_resources,
                loaded_bytes: self.loaded_bytes,
                elapsed_ms: log::elapsed_ms(self.started_at_ms),
                done: false,
            }));
            result
        })
    }

    fn platform(&self) -> physis::Platform {
        physis::Platform::Win32
    }
}

fn weapon_model_resource_stage(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or_default() {
        "mdl" => "读取模型几何",
        "mtrl" => "解析材质",
        "tex" | "atex" => "解码纹理",
        "stm" => "应用染色模板",
        "shpk" => "读取着色器信息",
        _ => "读取本地资源",
    }
}

fn compact_weapon_resource_path(path: &str) -> String {
    let parts = path
        .split(['/', '\\'])
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    parts[parts.len().saturating_sub(3)..].join("/")
}

async fn load_item_icon_from_browser_sqpack(icon_id: u32) -> Result<Vec<u8>, String> {
    log::debug(
        "resource",
        format!("BrowserSqPack request: item-icon/{icon_id}"),
    );
    let mut sqpack = BrowserSqPack::from_window_handle().await?;
    let path = item_icon_tex_path(icon_id);
    sqpack
        .read_game_file_with_window(&path, ITEM_ICON_READ_WINDOW)
        .await
}

pub struct BundledProvider;

impl ResourceProvider for BundledProvider {
    fn source(&self) -> ResourceSource {
        ResourceSource::Builtin
    }

    fn supports(&self, request: &ProviderRequest) -> bool {
        (request.kind == CraftDataKind.into() && request.key == "default")
            || (request.kind == WeaponCatalogKind.into() && request.key == "default")
            || (request.kind == CollectionCatalogKind.into() && request.key == "default")
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
            } else if request.kind == WeaponCatalogKind.into() {
                BUNDLED_WEAPON_CATALOG_ASSET
            } else {
                BUNDLED_COLLECTION_CATALOG_ASSET
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
                metadata: xiv_companion::ResourceMetadata {
                    origin: Some(xiv_companion::ResourceOrigin::Builtin),
                    ..Default::default()
                },
            })
        })
    }
}
