use crate::FurnitureCatalogPackage;

use super::{
    CachePolicy, DecodeContext, FallbackPolicy, ProviderRequest, ResourceDescriptor, ResourceError,
    ResourceErrorKind, ResourceHub, ResourceKindKey, ResourceKindLabel, ResourceSource,
    ResourceSpec, SourcePolicy,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct FurnitureCatalogKind;

impl ResourceKindLabel for FurnitureCatalogKind {
    fn id(&self) -> &'static str {
        "xiv_companion.resource.furniture_catalog"
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FurnitureCatalogId {
    Default,
}

pub struct FurnitureCatalogResource;

impl ResourceSpec for FurnitureCatalogResource {
    type Id = FurnitureCatalogId;
    type Output = FurnitureCatalogPackage;

    fn kind() -> ResourceKindKey {
        FurnitureCatalogKind.into()
    }

    fn descriptor() -> ResourceDescriptor {
        ResourceDescriptor {
            kind: Self::kind(),
            default_policy: SourcePolicy::Fixed(ResourceSource::IndexedDb),
            fallback_policy: FallbackPolicy::default(),
            cache_policy: CachePolicy::ReadWrite,
            pipeline: "furniture-catalog-json-v1",
        }
    }

    fn request(id: &Self::Id) -> ProviderRequest {
        let key = match id {
            FurnitureCatalogId::Default => "default",
        };
        ProviderRequest {
            kind: Self::kind(),
            key: key.to_string(),
        }
    }

    fn decode(bytes: Vec<u8>, context: DecodeContext) -> Result<Self::Output, ResourceError> {
        serde_json::from_slice::<FurnitureCatalogPackage>(&bytes).map_err(|error| {
            ResourceError::new(
                ResourceErrorKind::DecodeFailed,
                context.resource,
                Some(context.source),
                format!("failed to decode furniture catalog JSON: {error}"),
            )
        })
    }
}

pub fn register_furniture_catalog_resource(hub: &mut ResourceHub) {
    hub.register_resource::<FurnitureCatalogResource>();
}
