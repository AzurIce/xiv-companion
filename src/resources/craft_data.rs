#[cfg(feature = "game-data")]
use std::path::PathBuf;

use crate::CraftDataPackage;

#[cfg(feature = "game-data")]
use crate::{LocalItemIconProvider, register_item_icon_resource};

use super::{
    CachePolicy, DecodeContext, FallbackPolicy, ProviderRequest, ResourceDescriptor, ResourceError,
    ResourceErrorKind, ResourceHub, ResourceKindKey, ResourceKindLabel, ResourceSource,
    ResourceSpec, SourcePolicy,
};
#[cfg(feature = "game-data")]
use super::{ResourceBlob, ResourceFuture, ResourceProvider};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct CraftDataKind;

impl ResourceKindLabel for CraftDataKind {
    fn id(&self) -> &'static str {
        "xiv_companion.resource.craft_data"
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CraftDataId {
    Default,
}

pub struct CraftDataResource;

impl ResourceSpec for CraftDataResource {
    type Id = CraftDataId;
    type Output = CraftDataPackage;

    fn kind() -> ResourceKindKey {
        CraftDataKind.into()
    }

    fn descriptor() -> ResourceDescriptor {
        ResourceDescriptor {
            kind: Self::kind(),
            default_policy: SourcePolicy::Fixed(ResourceSource::IndexedDb),
            fallback_policy: FallbackPolicy::default(),
            cache_policy: CachePolicy::ReadWrite,
            pipeline: "craft-data-json-v1",
        }
    }

    fn request(id: &Self::Id) -> ProviderRequest {
        let key = match id {
            CraftDataId::Default => "default",
        };
        ProviderRequest {
            kind: Self::kind(),
            key: key.to_string(),
        }
    }

    fn decode(bytes: Vec<u8>, context: DecodeContext) -> Result<Self::Output, ResourceError> {
        serde_json::from_slice::<CraftDataPackage>(&bytes).map_err(|error| {
            ResourceError::new(
                ResourceErrorKind::DecodeFailed,
                context.resource,
                Some(context.source),
                format!("failed to decode craft-data JSON: {error}"),
            )
        })
    }
}

pub fn register_craft_data_resource(hub: &mut ResourceHub) {
    hub.register_resource::<CraftDataResource>();
}

#[cfg(feature = "game-data")]
#[derive(Clone, Debug)]
pub struct LocalCraftDataProvider {
    game_dir: PathBuf,
}

#[cfg(feature = "game-data")]
impl LocalCraftDataProvider {
    pub fn new(game_dir: impl Into<PathBuf>) -> Self {
        Self {
            game_dir: game_dir.into(),
        }
    }
}

#[cfg(feature = "game-data")]
pub fn local_craft_data_hub(game_dir: impl Into<PathBuf>) -> ResourceHub {
    let game_dir = game_dir.into();
    let mut hub = ResourceHub::new();
    register_craft_data_resource(&mut hub);
    hub.add_provider(LocalCraftDataProvider::new(game_dir));
    hub.set_policy(
        CraftDataKind.into(),
        SourcePolicy::Fixed(ResourceSource::UserLocal),
    );
    hub
}

#[cfg(feature = "game-data")]
pub fn local_game_resource_hub(game_dir: impl Into<PathBuf>) -> ResourceHub {
    let game_dir = game_dir.into();
    let mut hub = ResourceHub::new();
    register_craft_data_resource(&mut hub);
    register_item_icon_resource(&mut hub);
    hub.add_provider(LocalCraftDataProvider::new(game_dir.clone()));
    hub.add_provider(LocalItemIconProvider::new(game_dir));
    hub
}

#[cfg(feature = "game-data")]
impl ResourceProvider for LocalCraftDataProvider {
    fn source(&self) -> ResourceSource {
        ResourceSource::UserLocal
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
                    format!(
                        "local craft-data provider has no resource key {}",
                        request.key
                    ),
                ));
            }

            let generated_at = local_generated_at();
            let data = crate::game_data::export_craft_data(&self.game_dir, generated_at).map_err(
                |error| {
                    ResourceError::new(
                        ResourceErrorKind::ProviderFailed,
                        resource_kind.clone(),
                        Some(self.source()),
                        format!("failed to export craft data from local game files: {error:#}"),
                    )
                },
            )?;
            let fingerprint = Some(data.game_version.clone());
            let bytes = serde_json::to_vec(&data).map_err(|error| {
                ResourceError::new(
                    ResourceErrorKind::ProviderFailed,
                    resource_kind,
                    Some(self.source()),
                    format!("failed to encode local craft data: {error}"),
                )
            })?;
            Ok(ResourceBlob {
                bytes,
                fingerprint,
                metadata: super::ResourceMetadata {
                    origin: Some(super::ResourceOrigin::UserLocal),
                    ..Default::default()
                },
            })
        })
    }
}

#[cfg(feature = "game-data")]
fn local_generated_at() -> String {
    std::process::Command::new("date")
        .args(["-u", "+%Y-%m-%dT%H:%M:%SZ"])
        .output()
        .ok()
        .and_then(|output| output.status.success().then_some(output.stdout))
        .and_then(|stdout| String::from_utf8(stdout).ok())
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "1970-01-01T00:00:00Z".to_string())
}
