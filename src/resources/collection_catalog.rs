use crate::CollectionCatalogPackage;

use super::{
    CachePolicy, DecodeContext, FallbackPolicy, ProviderRequest, ResourceDescriptor, ResourceError,
    ResourceErrorKind, ResourceHub, ResourceKindKey, ResourceKindLabel, ResourceSource,
    ResourceSpec, SourcePolicy,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct CollectionCatalogKind;

impl ResourceKindLabel for CollectionCatalogKind {
    fn id(&self) -> &'static str {
        "xiv_companion.resource.collection_catalog"
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CollectionCatalogId {
    Default,
}

pub struct CollectionCatalogResource;

impl ResourceSpec for CollectionCatalogResource {
    type Id = CollectionCatalogId;
    type Output = CollectionCatalogPackage;

    fn kind() -> ResourceKindKey {
        CollectionCatalogKind.into()
    }

    fn descriptor() -> ResourceDescriptor {
        ResourceDescriptor {
            kind: Self::kind(),
            default_policy: SourcePolicy::Fixed(ResourceSource::IndexedDb),
            fallback_policy: FallbackPolicy::default(),
            cache_policy: CachePolicy::ReadWrite,
            pipeline: "collection-catalog-json-v8",
        }
    }

    fn request(id: &Self::Id) -> ProviderRequest {
        let key = match id {
            CollectionCatalogId::Default => "default",
        };
        ProviderRequest {
            kind: Self::kind(),
            key: key.to_string(),
        }
    }

    fn decode(bytes: Vec<u8>, context: DecodeContext) -> Result<Self::Output, ResourceError> {
        serde_json::from_slice::<CollectionCatalogPackage>(&bytes).map_err(|error| {
            ResourceError::new(
                ResourceErrorKind::DecodeFailed,
                context.resource,
                Some(context.source),
                format!("failed to decode collection catalog JSON: {error}"),
            )
        })
    }
}

pub fn register_collection_catalog_resource(hub: &mut ResourceHub) {
    hub.register_resource::<CollectionCatalogResource>();
}
