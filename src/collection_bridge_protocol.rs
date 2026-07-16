use std::collections::HashSet;

use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParsedBridgeMessage {
    Response { id: String, item_ids: Vec<u32> },
    EventObtained(Vec<u32>),
    Error { id: Option<String>, message: String },
}

pub fn parse_bridge_message(
    text: &str,
    equipment_ids: &HashSet<u32>,
) -> Option<ParsedBridgeMessage> {
    let message = serde_json::from_str::<Value>(text).ok()?;

    if message.get("event").and_then(Value::as_str) == Some("collection.item.unlocked") {
        let item_id =
            u32::try_from(message.pointer("/data/itemId").and_then(Value::as_u64)?).ok()?;
        return Some(ParsedBridgeMessage::EventObtained(vec![item_id]));
    }

    if message.get("event").and_then(Value::as_str) == Some("collection.source.changed") {
        return Some(ParsedBridgeMessage::EventObtained(
            equipment_ids_from_source(message.get("data"), equipment_ids),
        ));
    }

    let id = message
        .get("id")
        .and_then(Value::as_str)
        .map(str::to_string);
    if let Some(error) = message.get("error") {
        let detail = error
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("桥接请求失败");
        return Some(ParsedBridgeMessage::Error {
            id,
            message: detail.to_string(),
        });
    }

    let id = id?;
    if id.starts_with("collection-unlocks-") {
        let Some(statuses) = message
            .pointer("/result/statuses")
            .and_then(Value::as_array)
        else {
            return Some(ParsedBridgeMessage::Error {
                id: Some(id),
                message: "桥接返回了无效的物品解锁响应".to_string(),
            });
        };
        let item_ids = statuses
            .iter()
            .filter(|status| status.get("status").and_then(Value::as_str) == Some("unlocked"))
            .filter_map(|status| status.get("itemId").and_then(Value::as_u64))
            .filter_map(|item_id| u32::try_from(item_id).ok())
            .collect();
        return Some(ParsedBridgeMessage::Response { id, item_ids });
    }

    if id == "collection-sources" {
        let Some(sources) = message.pointer("/result/sources").and_then(Value::as_array) else {
            return Some(ParsedBridgeMessage::Error {
                id: Some(id),
                message: "桥接返回了无效的物品来源响应".to_string(),
            });
        };
        let item_ids = sources
            .iter()
            .flat_map(|source| equipment_ids_from_source(Some(source), equipment_ids))
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        return Some(ParsedBridgeMessage::Response { id, item_ids });
    }

    None
}

fn equipment_ids_from_source(source: Option<&Value>, equipment_ids: &HashSet<u32>) -> Vec<u32> {
    ["itemIds", "observedItemIds"]
        .into_iter()
        .flat_map(|field| {
            source
                .and_then(|source| source.get(field))
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
        })
        .filter_map(Value::as_u64)
        .filter_map(|item_id| u32::try_from(item_id).ok())
        .filter(|item_id| equipment_ids.contains(item_id))
        .collect::<HashSet<_>>()
        .into_iter()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parser_correlates_unlock_responses_and_only_accepts_unlocked_items() {
        let parsed = parse_bridge_message(
            r#"{"id":"collection-unlocks-2","result":{"statuses":[{"itemId":10,"status":"unlocked"},{"itemId":20,"status":"locked"}]}}"#,
            &HashSet::new(),
        );

        assert_eq!(
            parsed,
            Some(ParsedBridgeMessage::Response {
                id: "collection-unlocks-2".to_string(),
                item_ids: vec![10],
            })
        );
    }

    #[test]
    fn parser_filters_source_snapshots_to_equipment_catalog_ids() {
        let parsed = parse_bridge_message(
            r#"{"id":"collection-sources","result":{"sources":[{"itemIds":[10,20],"observedItemIds":[20,30]}]}}"#,
            &HashSet::from([20, 30, 40]),
        );

        let Some(ParsedBridgeMessage::Response { id, mut item_ids }) = parsed else {
            panic!("expected collection source response");
        };
        item_ids.sort_unstable();
        assert_eq!(id, "collection-sources");
        assert_eq!(item_ids, vec![20, 30]);
    }

    #[test]
    fn parser_ignores_unknown_response_ids() {
        assert_eq!(
            parse_bridge_message(
                r#"{"id":"unrelated","result":{"statuses":[{"itemId":10,"status":"unlocked"}]}}"#,
                &HashSet::new(),
            ),
            None
        );
    }
}
