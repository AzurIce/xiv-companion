use std::collections::HashSet;

use crate::{CollectionItem, CollectionKind};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InventoryCollectionSync {
    pub detected_equipment_ids: HashSet<u32>,
    pub added_ids: HashSet<u32>,
    pub removed_ids: HashSet<u32>,
    pub next_collection_ids: HashSet<u32>,
}

pub fn inventory_collection_item_ids(
    owned_item_ids: &HashSet<u32>,
    catalog: &[CollectionItem],
) -> HashSet<u32> {
    catalog
        .iter()
        .filter(|item| {
            item.kind == CollectionKind::Equipment
                && (owned_item_ids.contains(&item.id)
                    || item
                        .set_item_ids
                        .iter()
                        .any(|set_item_id| owned_item_ids.contains(set_item_id)))
        })
        .map(|item| item.id)
        .collect()
}

pub fn inventory_collection_sync(
    owned_item_ids: &HashSet<u32>,
    obtained_item_ids: &HashSet<u32>,
    catalog: &[CollectionItem],
) -> InventoryCollectionSync {
    let equipment_catalog_ids = catalog
        .iter()
        .filter(|item| item.kind == CollectionKind::Equipment)
        .map(|item| item.id)
        .collect::<HashSet<_>>();
    let detected_equipment_ids = inventory_collection_item_ids(owned_item_ids, catalog);
    let current_equipment_ids = obtained_item_ids
        .intersection(&equipment_catalog_ids)
        .copied()
        .collect::<HashSet<_>>();
    let added_ids = detected_equipment_ids
        .difference(&current_equipment_ids)
        .copied()
        .collect();
    let removed_ids = current_equipment_ids
        .difference(&detected_equipment_ids)
        .copied()
        .collect();
    let mut next_collection_ids = obtained_item_ids
        .difference(&equipment_catalog_ids)
        .copied()
        .collect::<HashSet<_>>();
    next_collection_ids.extend(detected_equipment_ids.iter().copied());

    InventoryCollectionSync {
        detected_equipment_ids,
        added_ids,
        removed_ids,
        next_collection_ids,
    }
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
            set_item_ids: Vec::new(),
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

    #[test]
    fn synchronizes_equipment_in_both_directions_without_touching_unlocks() {
        let catalog = vec![
            item(10, CollectionKind::Equipment),
            item(20, CollectionKind::Mount),
            item(30, CollectionKind::Equipment),
            item(40, CollectionKind::Equipment),
        ];
        let sync = inventory_collection_sync(
            &HashSet::from([30, 40]),
            &HashSet::from([10, 20, 30]),
            &catalog,
        );

        assert_eq!(sync.detected_equipment_ids, HashSet::from([30, 40]));
        assert_eq!(sync.added_ids, HashSet::from([40]));
        assert_eq!(sync.removed_ids, HashSet::from([10]));
        assert_eq!(sync.next_collection_ids, HashSet::from([20, 30, 40]));
    }

    #[test]
    fn owning_an_unopened_set_item_detects_every_equipment_piece() {
        let mut top = item(52_406, CollectionKind::Equipment);
        top.set_item_ids = vec![52_596];
        let mut bottoms = item(52_407, CollectionKind::Equipment);
        bottoms.set_item_ids = vec![52_596];
        let mut shoes = item(52_408, CollectionKind::Equipment);
        shoes.set_item_ids = vec![52_596];
        let catalog = vec![top, bottoms, shoes];

        assert_eq!(
            inventory_collection_item_ids(&HashSet::from([52_596]), &catalog),
            HashSet::from([52_406, 52_407, 52_408])
        );
    }
}
