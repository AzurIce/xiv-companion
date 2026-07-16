use crate::WeaponCatalogPackage;

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
            default_policy: SourcePolicy::Fixed(ResourceSource::IndexedDb),
            fallback_policy: FallbackPolicy::default(),
            cache_policy: CachePolicy::ReadWrite,
            pipeline: "weapon-catalog-json-v2",
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
        let package = serde_json::from_slice::<WeaponCatalogPackage>(&bytes).map_err(|error| {
            ResourceError::new(
                ResourceErrorKind::DecodeFailed,
                context.resource.clone(),
                Some(context.source),
                format!("failed to decode weapon catalog JSON: {error}"),
            )
        })?;
        if package.stains.is_empty() || package.counts.stains != package.stains.len() {
            return Err(ResourceError::new(
                ResourceErrorKind::DecodeFailed,
                context.resource,
                Some(context.source),
                format!(
                    "weapon catalog stain metadata is incomplete: counts.stains={}, stains.len()={}",
                    package.counts.stains,
                    package.stains.len()
                ),
            ));
        }
        Ok(package)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WeaponModelId {
    pub item_id: u32,
    pub item_name: String,
    pub model_main: u64,
    pub model_sub: u64,
    pub stain_ids: [u8; 2],
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
}

pub fn parse_weapon_model_request_key(key: &str) -> Result<WeaponModelId, String> {
    let mut parts = key.splitn(6, '|');
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
    let stain0 = parts
        .next()
        .ok_or_else(|| "missing stain0".to_string())?
        .parse::<u8>()
        .map_err(|error| format!("invalid stain0: {error}"))?;
    let stain1 = parts
        .next()
        .ok_or_else(|| "missing stain1".to_string())?
        .parse::<u8>()
        .map_err(|error| format!("invalid stain1: {error}"))?;
    let item_name = parts.next().unwrap_or_default().to_string();
    Ok(WeaponModelId {
        item_id,
        item_name,
        model_main,
        model_sub,
        stain_ids: [stain0, stain1],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weapon_catalog_defaults_missing_stain_metadata() {
        let catalog = serde_json::from_str::<WeaponCatalogPackage>(
            r#"{
                "generatedAt":"old",
                "gameVersion":"old",
                "source":"old",
                "counts":{"items":0},
                "items":[]
            }"#,
        )
        .expect("legacy weapon catalog");

        assert_eq!(catalog.counts.stains, 0);
        assert!(catalog.stains.is_empty());
    }

    #[test]
    fn weapon_catalog_resource_rejects_missing_stain_metadata() {
        let error = WeaponCatalogResource::decode(
            br#"{
                "generatedAt":"old",
                "gameVersion":"old",
                "source":"old",
                "counts":{"items":0},
                "items":[]
            }"#
            .to_vec(),
            DecodeContext {
                resource: WeaponCatalogResource::kind(),
                source: ResourceSource::Builtin,
                fingerprint: None,
            },
        )
        .expect_err("legacy catalog without stains must be rejected");

        assert!(error.to_string().contains("stain metadata is incomplete"));
    }

    #[test]
    fn weapon_model_request_key_round_trips_stain_ids() {
        let id = WeaponModelId {
            item_id: 42,
            item_name: "Test | Weapon".to_string(),
            model_main: 100,
            model_sub: 200,
            stain_ids: [17, 93],
        };

        assert_eq!(
            parse_weapon_model_request_key("42|100|200|17|93|Test   Weapon"),
            Ok(WeaponModelId {
                item_name: "Test   Weapon".to_string(),
                ..id
            })
        );
    }
}
