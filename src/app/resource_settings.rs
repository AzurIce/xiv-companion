use serde::{Deserialize, Serialize};
use xiv_companion::{
    CraftDataKind, ItemIconKind, ResourceHub, ResourceSource, SourcePolicy, WeaponCatalogKind,
};
#[cfg(feature = "game-data")]
use xiv_companion::{LocalCraftDataProvider, LocalItemIconProvider};

use crate::app::resources::default_web_resource_hub;

const SETTINGS_KEY: &str = "xiv-companion-resource-settings";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SourcePreference {
    BuiltinFirst,
    UserLocalFirst,
    BuiltinOnly,
    UserLocalOnly,
}

impl SourcePreference {
    pub fn to_policy(self) -> SourcePolicy {
        match self {
            Self::BuiltinFirst => {
                SourcePolicy::Fallback(vec![ResourceSource::Builtin, ResourceSource::UserLocal])
            }
            Self::UserLocalFirst => {
                SourcePolicy::Fallback(vec![ResourceSource::UserLocal, ResourceSource::Builtin])
            }
            Self::BuiltinOnly => SourcePolicy::Fixed(ResourceSource::Builtin),
            Self::UserLocalOnly => SourcePolicy::Fixed(ResourceSource::UserLocal),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceSettings {
    #[serde(default)]
    pub user_local_path: String,
    #[serde(default)]
    pub user_local_display_name: String,
    pub global_preference: SourcePreference,
    pub craft_data_preference: Option<SourcePreference>,
    pub item_icon_preference: Option<SourcePreference>,
}

impl Default for ResourceSettings {
    fn default() -> Self {
        Self {
            user_local_path: String::new(),
            user_local_display_name: String::new(),
            global_preference: SourcePreference::BuiltinFirst,
            craft_data_preference: None,
            item_icon_preference: None,
        }
    }
}

impl ResourceSettings {
    pub fn craft_data_preference(&self) -> SourcePreference {
        self.craft_data_preference.unwrap_or(self.global_preference)
    }

    pub fn item_icon_preference(&self) -> SourcePreference {
        if cfg!(target_arch = "wasm32") {
            SourcePreference::BuiltinOnly
        } else {
            self.item_icon_preference.unwrap_or(self.global_preference)
        }
    }
}

pub fn load_resource_settings() -> ResourceSettings {
    let settings = local_storage_value(SETTINGS_KEY)
        .and_then(|value| serde_json::from_str::<ResourceSettings>(&value).ok())
        .unwrap_or_default();
    normalize_resource_settings_for_runtime(settings)
}

pub fn save_resource_settings(settings: &ResourceSettings) {
    let settings = normalize_resource_settings_for_runtime(settings.clone());
    if let Ok(value) = serde_json::to_string(&settings) {
        set_local_storage_value(SETTINGS_KEY, &value);
    }
}

pub fn is_user_local_path_usable(path: &str) -> bool {
    let path = path.trim();
    if path == "~"
        || path.starts_with("~/")
        || path.starts_with("~\\")
        || path.starts_with('/')
        || path.starts_with("\\\\")
    {
        return true;
    }

    let bytes = path.as_bytes();
    bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && matches!(bytes[2], b'\\' | b'/')
}

pub fn path_user_local_provider_available_for_runtime() -> bool {
    !cfg!(target_arch = "wasm32")
}

fn normalize_resource_settings_for_runtime(mut settings: ResourceSettings) -> ResourceSettings {
    if cfg!(target_arch = "wasm32") {
        settings.item_icon_preference = Some(SourcePreference::BuiltinOnly);
    }
    settings
}

pub fn configured_web_resource_hub() -> ResourceHub {
    configured_web_resource_hub_for(&load_resource_settings())
}

pub fn configured_web_resource_hub_for(settings: &ResourceSettings) -> ResourceHub {
    let mut hub = default_web_resource_hub();
    #[cfg(feature = "game-data")]
    {
        let user_local_path = settings.user_local_path.trim();
        if path_user_local_provider_available_for_runtime()
            && is_user_local_path_usable(user_local_path)
        {
            hub.add_provider(LocalCraftDataProvider::new(user_local_path));
            hub.add_provider(LocalItemIconProvider::new(user_local_path));
        }
    }

    if cfg!(target_arch = "wasm32") {
        hub.set_policy(
            CraftDataKind.into(),
            SourcePolicy::Fixed(ResourceSource::IndexedDb),
        );
        hub.set_policy(
            WeaponCatalogKind.into(),
            SourcePolicy::Fixed(ResourceSource::IndexedDb),
        );
        hub.set_policy(
            ItemIconKind.into(),
            SourcePolicy::Fixed(ResourceSource::Builtin),
        );
    } else {
        hub.set_policy(
            CraftDataKind.into(),
            settings.craft_data_preference().to_policy(),
        );
        hub.set_policy(
            WeaponCatalogKind.into(),
            SourcePolicy::Fixed(ResourceSource::IndexedDb),
        );
        hub.set_policy(
            ItemIconKind.into(),
            settings.item_icon_preference().to_policy(),
        );
    }
    hub
}

fn local_storage_value(key: &str) -> Option<String> {
    web_sys::window()
        .and_then(|window| window.local_storage().ok().flatten())
        .and_then(|storage| storage.get_item(key).ok().flatten())
}

fn set_local_storage_value(key: &str, value: &str) {
    if let Some(storage) =
        web_sys::window().and_then(|window| window.local_storage().ok().flatten())
    {
        let _ = storage.set_item(key, value);
    }
}
