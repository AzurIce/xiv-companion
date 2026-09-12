use crate::CharacterMakePackage;

use super::{
    CachePolicy, DecodeContext, FallbackPolicy, ProviderRequest, ResourceDescriptor, ResourceError,
    ResourceErrorKind, ResourceHub, ResourceKindKey, ResourceKindLabel, ResourceSource,
    ResourceSpec, SourcePolicy,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct CharacterMakeKind;

impl ResourceKindLabel for CharacterMakeKind {
    fn id(&self) -> &'static str {
        "xiv_companion.resource.character_make"
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CharacterMakeId {
    Default,
}

pub struct CharacterMakeResource;

impl ResourceSpec for CharacterMakeResource {
    type Id = CharacterMakeId;
    type Output = CharacterMakePackage;

    fn kind() -> ResourceKindKey {
        CharacterMakeKind.into()
    }

    fn descriptor() -> ResourceDescriptor {
        ResourceDescriptor {
            kind: Self::kind(),
            default_policy: SourcePolicy::Fixed(ResourceSource::IndexedDb),
            fallback_policy: FallbackPolicy::default(),
            cache_policy: CachePolicy::ReadWrite,
            pipeline: "character-make-json-v1",
        }
    }

    fn request(id: &Self::Id) -> ProviderRequest {
        let key = match id {
            CharacterMakeId::Default => "default",
        };
        ProviderRequest {
            kind: Self::kind(),
            key: key.to_string(),
        }
    }

    fn decode(bytes: Vec<u8>, context: DecodeContext) -> Result<Self::Output, ResourceError> {
        serde_json::from_slice::<CharacterMakePackage>(&bytes).map_err(|error| {
            ResourceError::new(
                ResourceErrorKind::DecodeFailed,
                context.resource,
                Some(context.source),
                format!("failed to decode character make JSON: {error}"),
            )
        })
    }
}

pub fn register_character_make_resource(hub: &mut ResourceHub) {
    hub.register_resource::<CharacterMakeResource>();
}
