use crate::{WeaponCatalogPackage, WeaponModelData};

use super::{
    CachePolicy, DecodeContext, FallbackPolicy, ProviderRequest, ResourceDescriptor, ResourceError,
    ResourceErrorKind, ResourceHub, ResourceKindKey, ResourceKindLabel, ResourceSource,
    ResourceSpec, SourcePolicy,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct WeaponCatalogKind;

impl ResourceKindLabel for WeaponCatalogKind {
    fn id(&self) -> &'static str {
        "xiv_companion.resource.weapon_catalog"
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WeaponCatalogId {
    Default,
}

pub struct WeaponCatalogResource;

impl ResourceSpec for WeaponCatalogResource {
    type Id = WeaponCatalogId;
    type Output = WeaponCatalogPackage;

    fn kind() -> ResourceKindKey {
        WeaponCatalogKind.into()
    }

    fn descriptor() -> ResourceDescriptor {
        ResourceDescriptor {
            kind: Self::kind(),
            default_policy: SourcePolicy::Fixed(ResourceSource::UserLocal),
            fallback_policy: FallbackPolicy::default(),
            cache_policy: CachePolicy::ReadWrite,
            pipeline: "weapon-catalog-json-v1",
        }
    }

    fn request(id: &Self::Id) -> ProviderRequest {
        let key = match id {
            WeaponCatalogId::Default => "default",
        };
        ProviderRequest {
            kind: Self::kind(),
            key: key.to_string(),
        }
    }

    fn decode(bytes: Vec<u8>, context: DecodeContext) -> Result<Self::Output, ResourceError> {
        serde_json::from_slice::<WeaponCatalogPackage>(&bytes).map_err(|error| {
            ResourceError::new(
                ResourceErrorKind::DecodeFailed,
                context.resource,
                Some(context.source),
                format!("failed to decode weapon catalog JSON: {error}"),
            )
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WeaponModelId {
    pub item_id: u32,
    pub item_name: String,
    pub model_main: u64,
    pub model_sub: u64,
}

pub struct WeaponModelResource;

impl ResourceSpec for WeaponModelResource {
    type Id = WeaponModelId;
    type Output = WeaponModelData;

    fn kind() -> ResourceKindKey {
        WeaponModelKind.into()
    }

    fn descriptor() -> ResourceDescriptor {
        ResourceDescriptor {
            kind: Self::kind(),
            default_policy: SourcePolicy::Fixed(ResourceSource::UserLocal),
            fallback_policy: FallbackPolicy::default(),
            cache_policy: CachePolicy::None,
            pipeline: "weapon-model-json-v1",
        }
    }

    fn request(id: &Self::Id) -> ProviderRequest {
        ProviderRequest {
            kind: Self::kind(),
            key: format!(
                "{}|{}|{}|{}",
                id.item_id,
                id.model_main,
                id.model_sub,
                id.item_name.replace('|', " ")
            ),
        }
    }

    fn decode(bytes: Vec<u8>, context: DecodeContext) -> Result<Self::Output, ResourceError> {
        serde_json::from_slice::<WeaponModelData>(&bytes).map_err(|error| {
            ResourceError::new(
                ResourceErrorKind::DecodeFailed,
                context.resource,
                Some(context.source),
                format!("failed to decode weapon model JSON: {error}"),
            )
        })
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct WeaponModelKind;

impl ResourceKindLabel for WeaponModelKind {
    fn id(&self) -> &'static str {
        "xiv_companion.resource.weapon_model"
    }
}

pub fn register_weapon_model_resources(hub: &mut ResourceHub) {
    hub.register_resource::<WeaponCatalogResource>();
    hub.register_resource::<WeaponModelResource>();
}

pub fn parse_weapon_model_request_key(key: &str) -> Result<WeaponModelId, String> {
    let mut parts = key.splitn(4, '|');
    let item_id = parts
        .next()
        .ok_or_else(|| "missing item id".to_string())?
        .parse::<u32>()
        .map_err(|error| format!("invalid item id: {error}"))?;
    let model_main = parts
        .next()
        .ok_or_else(|| "missing model_main".to_string())?
        .parse::<u64>()
        .map_err(|error| format!("invalid model_main: {error}"))?;
    let model_sub = parts
        .next()
        .ok_or_else(|| "missing model_sub".to_string())?
        .parse::<u64>()
        .map_err(|error| format!("invalid model_sub: {error}"))?;
    let item_name = parts.next().unwrap_or_default().to_string();
    Ok(WeaponModelId {
        item_id,
        item_name,
        model_main,
        model_sub,
    })
}
