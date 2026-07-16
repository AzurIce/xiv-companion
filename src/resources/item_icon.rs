#[cfg(feature = "game-data")]
use std::path::PathBuf;

use super::{
    CachePolicy, DecodeContext, FallbackPolicy, ProviderRequest, ResourceBlob, ResourceDescriptor,
    ResourceError, ResourceErrorKind, ResourceFuture, ResourceKindKey, ResourceKindLabel,
    ResourceProvider, ResourceSource, ResourceSpec, SourcePolicy,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ItemIconKind;

impl ResourceKindLabel for ItemIconKind {
    fn id(&self) -> &'static str {
        "xiv_companion.resource.item_icon"
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ItemIconId {
    pub icon_id: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct ItemIconResourceInfo {
    pub icon_id: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub urls: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_image: Option<LocalItemIconImage>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct LocalItemIconImage {
    pub path: String,
    pub width: u16,
    pub height: u16,
    pub rgba: Vec<u8>,
}

pub struct ItemIconResource;

impl ResourceSpec for ItemIconResource {
    type Id = ItemIconId;
    type Output = ItemIconResourceInfo;

    fn kind() -> ResourceKindKey {
        ItemIconKind.into()
    }

    fn descriptor() -> ResourceDescriptor {
        ResourceDescriptor {
            kind: Self::kind(),
            default_policy: SourcePolicy::Fixed(ResourceSource::Builtin),
            fallback_policy: FallbackPolicy::default(),
            cache_policy: CachePolicy::None,
            pipeline: "item-icon-url-list-v1",
        }
    }

    fn request(id: &Self::Id) -> ProviderRequest {
        ProviderRequest {
            kind: Self::kind(),
            key: id.icon_id.to_string(),
        }
    }

    fn decode(bytes: Vec<u8>, context: DecodeContext) -> Result<Self::Output, ResourceError> {
        serde_json::from_slice::<ItemIconResourceInfo>(&bytes).map_err(|error| {
            ResourceError::new(
                ResourceErrorKind::DecodeFailed,
                context.resource,
                Some(context.source),
                format!("failed to decode item icon resource info: {error}"),
            )
        })
    }
}

pub fn register_item_icon_resource(hub: &mut super::ResourceHub) {
    hub.register_resource::<ItemIconResource>();
}

pub struct BuiltinItemIconProvider;

impl ResourceProvider for BuiltinItemIconProvider {
    fn source(&self) -> ResourceSource {
        ResourceSource::Builtin
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
            let icon_id = request.key.parse::<u32>().map_err(|error| {
                ResourceError::new(
                    ResourceErrorKind::Unsupported,
                    resource_kind.clone(),
                    Some(self.source()),
                    format!("invalid item icon id {}: {error}", request.key),
                )
            })?;
            let info = ItemIconResourceInfo {
                icon_id,
                urls: builtin_icon_urls(icon_id),
                local_image: None,
            };
            let bytes = serde_json::to_vec(&info).map_err(|error| {
                ResourceError::new(
                    ResourceErrorKind::ProviderFailed,
                    resource_kind,
                    Some(self.source()),
                    format!("failed to encode item icon resource info: {error}"),
                )
            })?;
            Ok(ResourceBlob {
                bytes,
                fingerprint: None,
                metadata: super::ResourceMetadata {
                    origin: Some(super::ResourceOrigin::Builtin),
                    ..Default::default()
                },
            })
        })
    }
}

#[cfg(feature = "game-data")]
#[derive(Clone, Debug)]
pub struct LocalItemIconProvider {
    game_dir: PathBuf,
}

#[cfg(feature = "game-data")]
impl LocalItemIconProvider {
    pub fn new(game_dir: impl Into<PathBuf>) -> Self {
        Self {
            game_dir: game_dir.into(),
        }
    }
}

#[cfg(feature = "game-data")]
impl ResourceProvider for LocalItemIconProvider {
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
            let icon_id = request.key.parse::<u32>().map_err(|error| {
                ResourceError::new(
                    ResourceErrorKind::Unsupported,
                    resource_kind.clone(),
                    Some(self.source()),
                    format!("invalid item icon id {}: {error}", request.key),
                )
            })?;

            let path = item_icon_tex_path(icon_id);
            let mut resource = physis::resource::SqPackResource::from_existing(
                self.game_dir.to_string_lossy().as_ref(),
            );
            let texture = resource
                .parsed::<physis::tex::Texture>(&path)
                .map_err(|error| {
                    ResourceError::new(
                        ResourceErrorKind::NotFound,
                        resource_kind.clone(),
                        Some(self.source()),
                        format!("failed to read local icon texture {path}: {error}"),
                    )
                })?;
            let rgba = texture.to_rgba().ok_or_else(|| {
                ResourceError::new(
                    ResourceErrorKind::DecodeFailed,
                    resource_kind.clone(),
                    Some(self.source()),
                    format!("failed to decode local icon texture {path} to RGBA"),
                )
            })?;
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
                metadata: super::ResourceMetadata {
                    origin: Some(super::ResourceOrigin::UserLocal),
                    ..Default::default()
                },
            })
        })
    }
}

pub fn builtin_icon_urls(icon_id: u32) -> Vec<String> {
    if icon_id == 0 {
        return Vec::new();
    }
    let folder = icon_id / 1000 * 1000;
    vec![
        format!("https://v2.xivapi.com/api/asset/ui/icon/{folder:06}/{icon_id:06}.tex?format=png"),
        format!("https://www.garlandtools.org/files/icons/item/t/{icon_id}.png"),
        format!("https://garlandtools.org/files/icons/item/t/{icon_id}.png"),
    ]
}

pub fn item_icon_tex_path(icon_id: u32) -> String {
    let folder = icon_id / 1000 * 1000;
    format!("ui/icon/{folder:06}/{icon_id:06}.tex")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_icon_urls_use_xivapi_v2_asset_path() {
        let urls = builtin_icon_urls(56_984);

        assert_eq!(
            urls.first().map(String::as_str),
            Some("https://v2.xivapi.com/api/asset/ui/icon/056000/056984.tex?format=png")
        );
    }
}
