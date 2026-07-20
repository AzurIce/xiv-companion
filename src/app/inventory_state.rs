use std::collections::HashMap;

use js_sys::JsString;
use serde::{Deserialize, Serialize};
use wasm_bindgen::JsValue;
use xiv_companion::inventory_bridge_protocol::{
    InventoryContainerDescriptor, InventoryContainerSnapshot,
};

const INVENTORY_DB_NAME: &str = "xiv-companion-inventory-state";
const INVENTORY_STORE_NAME: &str = "snapshots";
const INVENTORY_DB_VERSION: u32 = 1;
const INVENTORY_STATE_KEY: &str = "latest";
const INVENTORY_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PersistedInventoryState {
    pub schema_version: u32,
    pub saved_at: String,
    pub directory_revision: u64,
    pub containers: Vec<InventoryContainerDescriptor>,
    pub snapshots: HashMap<String, InventoryContainerSnapshot>,
}

impl PersistedInventoryState {
    pub fn new(
        directory_revision: u64,
        containers: Vec<InventoryContainerDescriptor>,
        snapshots: HashMap<String, InventoryContainerSnapshot>,
    ) -> Self {
        Self {
            schema_version: INVENTORY_SCHEMA_VERSION,
            saved_at: js_sys::Date::new_0().to_iso_string().into(),
            directory_revision,
            containers,
            snapshots,
        }
    }

    pub fn item_ids(&self) -> std::collections::HashSet<u32> {
        self.snapshots
            .values()
            .flat_map(|snapshot| snapshot.items.iter().map(|item| item.item_id))
            .collect()
    }
}

pub async fn load_inventory_state() -> Result<Option<PersistedInventoryState>, String> {
    let db = open_inventory_db().await?;
    let value = db
        .transaction(&[INVENTORY_STORE_NAME])
        .run(|transaction| async move {
            transaction
                .object_store(INVENTORY_STORE_NAME)?
                .get(&JsString::from(INVENTORY_STATE_KEY))
                .await
        })
        .await
        .map_err(|error| format!("读取物品快照失败: {error}"))?;
    let Some(json) = value.and_then(|value| value.as_string()) else {
        return Ok(None);
    };
    let state = serde_json::from_str::<PersistedInventoryState>(&json)
        .map_err(|error| format!("解析物品快照失败: {error}"))?;
    if state.schema_version != INVENTORY_SCHEMA_VERSION {
        return Ok(None);
    }
    Ok(Some(state))
}

pub async fn save_inventory_state(state: &PersistedInventoryState) -> Result<(), String> {
    let json =
        serde_json::to_string(state).map_err(|error| format!("序列化物品快照失败: {error}"))?;
    let db = open_inventory_db().await?;
    db.transaction(&[INVENTORY_STORE_NAME])
        .rw()
        .run(move |transaction| async move {
            transaction
                .object_store(INVENTORY_STORE_NAME)?
                .put_kv(
                    &JsString::from(INVENTORY_STATE_KEY),
                    &JsValue::from_str(&json),
                )
                .await?;
            Ok(())
        })
        .await
        .map_err(|error| format!("保存物品快照失败: {error}"))
}

async fn open_inventory_db() -> Result<indexed_db::Database<String>, String> {
    let factory = indexed_db::Factory::get()
        .map_err(|error| format!("打开 IndexedDB 物品数据库失败: {error}"))?;
    factory
        .open(
            INVENTORY_DB_NAME,
            INVENTORY_DB_VERSION,
            |event| async move {
                let db = event.database();
                if !db
                    .object_store_names()
                    .iter()
                    .any(|name| name == INVENTORY_STORE_NAME)
                {
                    db.build_object_store(INVENTORY_STORE_NAME).create()?;
                }
                Ok(())
            },
        )
        .await
        .map_err(|error| format!("打开 IndexedDB 物品数据库失败: {error}"))
}
