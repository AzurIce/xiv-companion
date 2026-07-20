use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum InventoryContainerKind {
    Inventory,
    Equipped,
    Armoury,
    Currency,
    Crystals,
    KeyItems,
    Saddlebag,
    PremiumSaddlebag,
    RetainerInventory,
    RetainerEquipped,
    RetainerMarket,
    RetainerCrystals,
    Cabinet,
    GlamourDresser,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum InventoryContainerAvailability {
    Live,
    Cached,
    NotLoaded,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InventoryContainerDescriptor {
    pub container_id: String,
    pub kind: InventoryContainerKind,
    pub category: Option<String>,
    pub owner_id: Option<String>,
    pub index: Option<u32>,
    pub availability: InventoryContainerAvailability,
    pub capacity: Option<u32>,
    pub occupied_slots: u32,
    pub total_quantity: u32,
    pub revision: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InventoryContainerDirectory {
    pub revision: u64,
    pub containers: Vec<InventoryContainerDescriptor>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InventoryItemEntry {
    pub slot: u32,
    pub item_id: u32,
    pub quantity: u32,
    pub hq: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InventoryContainerSnapshot {
    pub container_id: String,
    pub kind: InventoryContainerKind,
    pub category: Option<String>,
    pub owner_id: Option<String>,
    pub index: Option<u32>,
    pub availability: InventoryContainerAvailability,
    pub capacity: Option<u32>,
    pub occupied_slots: u32,
    pub total_quantity: u32,
    pub revision: u64,
    pub items: Vec<InventoryItemEntry>,
}

impl InventoryContainerSnapshot {
    pub fn descriptor(&self) -> InventoryContainerDescriptor {
        InventoryContainerDescriptor {
            container_id: self.container_id.clone(),
            kind: self.kind,
            category: self.category.clone(),
            owner_id: self.owner_id.clone(),
            index: self.index,
            availability: self.availability,
            capacity: self.capacity,
            occupied_slots: self.occupied_slots,
            total_quantity: self.total_quantity,
            revision: self.revision,
        }
    }
}

pub fn should_apply_container(
    current: Option<&InventoryContainerSnapshot>,
    incoming: &InventoryContainerSnapshot,
) -> bool {
    current.is_none_or(|current| incoming.revision >= current.revision)
}

pub fn should_apply_directory(current_revision: u64, incoming_revision: u64) -> bool {
    incoming_revision >= current_revision
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParsedInventoryBridgeMessage {
    Directory(InventoryContainerDirectory),
    Container(InventoryContainerSnapshot),
    SessionLogin,
    SessionLogout,
    Error {
        id: Option<String>,
        code: String,
        message: String,
    },
}

pub fn parse_inventory_bridge_message(text: &str) -> Option<ParsedInventoryBridgeMessage> {
    let message = serde_json::from_str::<Value>(text).ok()?;
    match message.get("event").and_then(Value::as_str) {
        Some("inventory.containers.changed") => {
            return serde_json::from_value(message.get("data")?.clone())
                .ok()
                .map(ParsedInventoryBridgeMessage::Directory);
        }
        Some("inventory.container.changed") => {
            return serde_json::from_value(message.get("data")?.clone())
                .ok()
                .map(ParsedInventoryBridgeMessage::Container);
        }
        Some("session.login") => return Some(ParsedInventoryBridgeMessage::SessionLogin),
        Some("session.logout") => return Some(ParsedInventoryBridgeMessage::SessionLogout),
        Some(_) => return None,
        None => {}
    }

    let id = message
        .get("id")
        .and_then(Value::as_str)
        .map(str::to_string);
    if let Some(error) = message.get("error") {
        return Some(ParsedInventoryBridgeMessage::Error {
            id,
            code: error
                .get("code")
                .and_then(Value::as_str)
                .unwrap_or("unknown_error")
                .to_string(),
            message: error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("桥接请求失败")
                .to_string(),
        });
    }

    let id = id?;
    if id.starts_with("inventory-directory-") {
        return serde_json::from_value(message.get("result")?.clone())
            .ok()
            .map(ParsedInventoryBridgeMessage::Directory);
    }
    if id.starts_with("inventory-container-") {
        return serde_json::from_value(message.pointer("/result/container")?.clone())
            .ok()
            .map(ParsedInventoryBridgeMessage::Container);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_directory_response() {
        let parsed = parse_inventory_bridge_message(
            r#"{"id":"inventory-directory-1","result":{"revision":4,"containers":[{"containerId":"inventory:1","kind":"inventory","category":null,"ownerId":null,"index":1,"availability":"live","capacity":35,"occupiedSlots":2,"totalQuantity":8,"revision":4}]}}"#,
        );
        let Some(ParsedInventoryBridgeMessage::Directory(directory)) = parsed else {
            panic!("expected inventory directory");
        };
        assert_eq!(directory.revision, 4);
        assert_eq!(directory.containers[0].container_id, "inventory:1");
    }

    #[test]
    fn parses_container_change_event() {
        let parsed = parse_inventory_bridge_message(
            r#"{"event":"inventory.container.changed","data":{"containerId":"cabinet","kind":"cabinet","category":null,"ownerId":null,"index":null,"availability":"cached","capacity":800,"occupiedSlots":1,"totalQuantity":1,"revision":7,"items":[{"slot":2,"itemId":10,"quantity":1,"hq":false}]}}"#,
        );
        let Some(ParsedInventoryBridgeMessage::Container(container)) = parsed else {
            panic!("expected inventory container");
        };
        assert_eq!(
            container.availability,
            InventoryContainerAvailability::Cached
        );
        assert_eq!(container.items[0].item_id, 10);
    }

    #[test]
    fn parses_not_logged_in_error() {
        assert_eq!(
            parse_inventory_bridge_message(
                r#"{"id":"inventory-directory-1","error":{"code":"not_logged_in","message":"No character is currently logged in.","data":null}}"#,
            ),
            Some(ParsedInventoryBridgeMessage::Error {
                id: Some("inventory-directory-1".to_string()),
                code: "not_logged_in".to_string(),
                message: "No character is currently logged in.".to_string(),
            })
        );
    }

    #[test]
    fn rejects_stale_container_revision() {
        let current = InventoryContainerSnapshot {
            container_id: "inventory:1".to_string(),
            kind: InventoryContainerKind::Inventory,
            category: None,
            owner_id: None,
            index: Some(1),
            availability: InventoryContainerAvailability::Live,
            capacity: Some(35),
            occupied_slots: 1,
            total_quantity: 1,
            revision: 8,
            items: Vec::new(),
        };
        let mut incoming = current.clone();
        incoming.revision = 7;
        assert!(!should_apply_container(Some(&current), &incoming));
        incoming.revision = 8;
        assert!(should_apply_container(Some(&current), &incoming));
    }

    #[test]
    fn rejects_stale_directory_revision() {
        assert!(!should_apply_directory(12, 11));
        assert!(should_apply_directory(12, 12));
        assert!(should_apply_directory(12, 13));
    }
}
