use crate::{
    CharaModelKind, CharaModelType, FurnitureModelKind, PackedCharaModelId, WeaponCatalogPackage,
};

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

/// 装备模型加载请求标识。与 [`WeaponModelId`] 平行，额外携带装备槽位与种族，
/// 请求 key 带 `equip|` 前缀，避免与武器 key 混淆。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EquipmentModelId {
    pub item_id: u32,
    pub item_name: String,
    pub model_main: u64,
    pub model_sub: u64,
    pub equip_slot_category: u32,
    pub race_id: u16,
    pub stain_ids: [u8; 2],
}

/// 家具/庭具模型加载请求标识。家具不可染色、无种族维度，key 带 `furniture|`
/// 前缀，类别取 [`FurnitureModelKind::id`]（indoor/outdoor）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FurnitureModelId {
    pub item_id: u32,
    pub item_name: String,
    pub kind: FurnitureModelKind,
    pub model_key: u16,
}

/// 宠物/坐骑模型加载请求标识。不可染色、无种族维度，key 带 `chara|` 前缀；
/// 类别取 [`CharaModelKind::id`]（minion/mount），模型种类取
/// [`CharaModelType`] 的 kebab-case 名（monster/demihuman）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CharaModelId {
    pub item_id: u32,
    pub item_name: String,
    pub kind: CharaModelKind,
    pub model: PackedCharaModelId,
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

pub fn parse_equipment_model_request_key(key: &str) -> Result<EquipmentModelId, String> {
    let mut parts = key.splitn(9, '|');
    if parts.next() != Some("equip") {
        return Err("missing equip prefix".to_string());
    }
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
    let equip_slot_category = parts
        .next()
        .ok_or_else(|| "missing equip slot category".to_string())?
        .parse::<u32>()
        .map_err(|error| format!("invalid equip slot category: {error}"))?;
    let race_id = parts
        .next()
        .ok_or_else(|| "missing race id".to_string())?
        .parse::<u16>()
        .map_err(|error| format!("invalid race id: {error}"))?;
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
    Ok(EquipmentModelId {
        item_id,
        item_name,
        model_main,
        model_sub,
        equip_slot_category,
        race_id,
        stain_ids: [stain0, stain1],
    })
}

pub fn parse_furniture_model_request_key(key: &str) -> Result<FurnitureModelId, String> {
    let mut parts = key.splitn(5, '|');
    if parts.next() != Some("furniture") {
        return Err("missing furniture prefix".to_string());
    }
    let item_id = parts
        .next()
        .ok_or_else(|| "missing item id".to_string())?
        .parse::<u32>()
        .map_err(|error| format!("invalid item id: {error}"))?;
    let kind = match parts.next() {
        Some("indoor") => FurnitureModelKind::Indoor,
        Some("outdoor") => FurnitureModelKind::Outdoor,
        Some(other) => return Err(format!("invalid furniture kind: {other}")),
        None => return Err("missing furniture kind".to_string()),
    };
    let model_key = parts
        .next()
        .ok_or_else(|| "missing model key".to_string())?
        .parse::<u16>()
        .map_err(|error| format!("invalid model key: {error}"))?;
    let item_name = parts.next().unwrap_or_default().to_string();
    Ok(FurnitureModelId {
        item_id,
        item_name,
        kind,
        model_key,
    })
}

pub fn parse_chara_model_request_key(key: &str) -> Result<CharaModelId, String> {
    let mut parts = key.splitn(8, '|');
    if parts.next() != Some("chara") {
        return Err("missing chara prefix".to_string());
    }
    let item_id = parts
        .next()
        .ok_or_else(|| "missing item id".to_string())?
        .parse::<u32>()
        .map_err(|error| format!("invalid item id: {error}"))?;
    let kind = match parts.next() {
        Some("minion") => CharaModelKind::Minion,
        Some("mount") => CharaModelKind::Mount,
        Some(other) => return Err(format!("invalid chara kind: {other}")),
        None => return Err("missing chara kind".to_string()),
    };
    let chara_type = match parts.next() {
        Some("monster") => CharaModelType::Monster,
        Some("demihuman") => CharaModelType::Demihuman,
        Some(other) => return Err(format!("invalid chara model type: {other}")),
        None => return Err("missing chara model type".to_string()),
    };
    let model_id = parts
        .next()
        .ok_or_else(|| "missing model id".to_string())?
        .parse::<u16>()
        .map_err(|error| format!("invalid model id: {error}"))?;
    let base_id = parts
        .next()
        .ok_or_else(|| "missing base id".to_string())?
        .parse::<u16>()
        .map_err(|error| format!("invalid base id: {error}"))?;
    let variant_id = parts
        .next()
        .ok_or_else(|| "missing variant id".to_string())?
        .parse::<u16>()
        .map_err(|error| format!("invalid variant id: {error}"))?;
    let item_name = parts.next().unwrap_or_default().to_string();
    Ok(CharaModelId {
        item_id,
        item_name,
        kind,
        model: PackedCharaModelId {
            model_id,
            base_id,
            variant_id,
            chara_type,
        },
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

    #[test]
    fn equipment_model_request_key_round_trips_slot_and_race() {
        let id = EquipmentModelId {
            item_id: 42,
            item_name: "Test | Armor".to_string(),
            model_main: 100,
            model_sub: 200,
            equip_slot_category: 4,
            race_id: 701,
            stain_ids: [17, 93],
        };

        assert_eq!(
            parse_equipment_model_request_key("equip|42|100|200|4|701|17|93|Test   Armor"),
            Ok(EquipmentModelId {
                item_name: "Test   Armor".to_string(),
                ..id
            })
        );
        assert!(parse_equipment_model_request_key("42|100|200|4|701|17|93|Test   Armor").is_err());
    }

    #[test]
    fn furniture_model_request_key_round_trips_kind_and_model_key() {
        let id = FurnitureModelId {
            item_id: 42,
            item_name: "Test | Furniture".to_string(),
            kind: FurnitureModelKind::Outdoor,
            model_key: 1234,
        };

        assert_eq!(
            parse_furniture_model_request_key("furniture|42|outdoor|1234|Test   Furniture"),
            Ok(FurnitureModelId {
                item_name: "Test   Furniture".to_string(),
                ..id
            })
        );
        assert!(parse_furniture_model_request_key("furniture|42|armor|1234|x").is_err());
        assert!(parse_furniture_model_request_key("42|outdoor|1234|x").is_err());
    }

    #[test]
    fn chara_model_request_key_round_trips_kind_and_model() {
        let id = CharaModelId {
            item_id: 42,
            item_name: "Test | Minion".to_string(),
            kind: CharaModelKind::Minion,
            model: PackedCharaModelId {
                model_id: 8003,
                base_id: 1,
                variant_id: 2,
                chara_type: CharaModelType::Monster,
            },
        };

        assert_eq!(
            parse_chara_model_request_key("chara|42|minion|monster|8003|1|2|Test   Minion"),
            Ok(CharaModelId {
                item_name: "Test   Minion".to_string(),
                ..id
            })
        );
        assert_eq!(
            parse_chara_model_request_key("chara|42|mount|demihuman|1|1|1|陆行鸟")
                .map(|id| id.model.chara_type),
            Ok(CharaModelType::Demihuman)
        );
        assert!(parse_chara_model_request_key("chara|42|furniture|monster|8003|1|2|x").is_err());
        assert!(parse_chara_model_request_key("42|minion|monster|8003|1|2|x").is_err());
    }
}
