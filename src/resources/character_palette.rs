use crate::CharacterPalettePackage;

use super::{
    CachePolicy, DecodeContext, FallbackPolicy, ProviderRequest, ResourceDescriptor, ResourceError,
    ResourceErrorKind, ResourceHub, ResourceKindKey, ResourceKindLabel, ResourceSource,
    ResourceSpec, SourcePolicy,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct CharacterPaletteKind;

impl ResourceKindLabel for CharacterPaletteKind {
    fn id(&self) -> &'static str {
        "xiv_companion.resource.character_palette"
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CharacterPaletteId {
    Default,
}

pub struct CharacterPaletteResource;

impl ResourceSpec for CharacterPaletteResource {
    type Id = CharacterPaletteId;
    type Output = CharacterPalettePackage;

    fn kind() -> ResourceKindKey {
        CharacterPaletteKind.into()
    }

    fn descriptor() -> ResourceDescriptor {
        ResourceDescriptor {
            kind: Self::kind(),
            default_policy: SourcePolicy::Fixed(ResourceSource::IndexedDb),
            fallback_policy: FallbackPolicy::default(),
            cache_policy: CachePolicy::ReadWrite,
            pipeline: "character-palette-json-v1",
        }
    }

    fn request(id: &Self::Id) -> ProviderRequest {
        let key = match id {
            CharacterPaletteId::Default => "default",
        };
        ProviderRequest {
            kind: Self::kind(),
            key: key.to_string(),
        }
    }

    fn decode(bytes: Vec<u8>, context: DecodeContext) -> Result<Self::Output, ResourceError> {
        serde_json::from_slice::<CharacterPalettePackage>(&bytes).map_err(|error| {
            ResourceError::new(
                ResourceErrorKind::DecodeFailed,
                context.resource,
                Some(context.source),
                format!("failed to decode character palette JSON: {error}"),
            )
        })
    }
}

pub fn register_character_palette_resource(hub: &mut ResourceHub) {
    hub.register_resource::<CharacterPaletteResource>();
}
