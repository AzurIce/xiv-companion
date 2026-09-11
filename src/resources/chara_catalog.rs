use crate::CharaCatalogPackage;

use super::{
    CachePolicy, DecodeContext, FallbackPolicy, ProviderRequest, ResourceDescriptor, ResourceError,
    ResourceErrorKind, ResourceHub, ResourceKindKey, ResourceKindLabel, ResourceSource,
    ResourceSpec, SourcePolicy,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct CharaCatalogKind;

impl ResourceKindLabel for CharaCatalogKind {
    fn id(&self) -> &'static str {
        "xiv_companion.resource.chara_catalog"
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CharaCatalogId {
    Default,
}

pub struct CharaCatalogResource;

impl ResourceSpec for CharaCatalogResource {
    type Id = CharaCatalogId;
    type Output = CharaCatalogPackage;

    fn kind() -> ResourceKindKey {
        CharaCatalogKind.into()
    }

    fn descriptor() -> ResourceDescriptor {
        ResourceDescriptor {
            kind: Self::kind(),
            default_policy: SourcePolicy::Fixed(ResourceSource::IndexedDb),
            fallback_policy: FallbackPolicy::default(),
            cache_policy: CachePolicy::ReadWrite,
            pipeline: "chara-catalog-json-v1",
        }
    }

    fn request(id: &Self::Id) -> ProviderRequest {
        let key = match id {
            CharaCatalogId::Default => "default",
        };
        ProviderRequest {
            kind: Self::kind(),
            key: key.to_string(),
        }
    }

    fn decode(bytes: Vec<u8>, context: DecodeContext) -> Result<Self::Output, ResourceError> {
        serde_json::from_slice::<CharaCatalogPackage>(&bytes).map_err(|error| {
            ResourceError::new(
                ResourceErrorKind::DecodeFailed,
                context.resource,
                Some(context.source),
                format!("failed to decode chara catalog JSON: {error}"),
            )
        })
    }
}

pub fn register_chara_catalog_resource(hub: &mut ResourceHub) {
    hub.register_resource::<CharaCatalogResource>();
}
