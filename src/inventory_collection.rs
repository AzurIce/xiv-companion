use std::collections::HashSet;

use crate::{CollectionItem, CollectionKind};

pub fn inventory_collection_item_ids(
    owned_item_ids: &HashSet<u32>,
    catalog: &[CollectionItem],
) -> HashSet<u32> {
    catalog
        .iter()
        .filter(|item| item.kind == CollectionKind::Equipment && owned_item_ids.contains(&item.id))
        .map(|item| item.id)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: u32, kind: CollectionKind) -> CollectionItem {
        CollectionItem {
            id,
            kind,
            name: String::new(),
            description: String::new(),
            icon: 0,
            item_ui_category: 0,
            item_search_category: 0,
            item_action: 0,
            equip_slot_category: 0,
            slot_name: String::new(),
            slot_order: 0,
            level_item: 0,
            level_equip: 0,
            rarity: 0,
            class_job_category: 0,
            class_job_category_name: String::new(),
            item_series: 0,
            set_id: String::new(),
            set_name: String::new(),
            expansion: String::new(),
            patch: String::new(),
            model_main: 0,
            model_sub: 0,
            appearance_key: String::new(),
        }
    }

    #[test]
    fn only_returns_owned_equipment() {
        let catalog = vec![
            item(10, CollectionKind::Equipment),
            item(20, CollectionKind::Mount),
            item(30, CollectionKind::Equipment),
        ];
        assert_eq!(
            inventory_collection_item_ids(&HashSet::from([10, 20, 40]), &catalog),
            HashSet::from([10])
        );
    }
}
