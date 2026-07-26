use std::collections::{BTreeMap, HashSet};
use std::rc::Rc;

use xiv_companion::{CollectionCatalogPackage, CollectionItem, CollectionKind};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EquipmentSetGroup {
    pub expansion: String,
    pub patch: String,
    pub set_id: String,
    pub set_name: String,
    pub item_indices: Vec<usize>,
    pub min_item_level: u16,
    pub max_item_level: u16,
    pub min_equip_level: u16,
    pub max_equip_level: u16,
    pub class_job_label: String,
    pub slot_count: usize,
    pub is_set: bool,
    pub search_text: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CollectionIndex {
    pub catalog: Rc<CollectionCatalogPackage>,
    pub items_by_kind_expansion: BTreeMap<(CollectionKind, String), Vec<usize>>,
    pub equipment_sets: Vec<EquipmentSetGroup>,
    pub collection_expansions: Vec<String>,
    pub equipment_slot_options: Vec<String>,
    pub equipment_level_bounds: (u16, u16),
    pub equipment_item_level_bounds: (u16, u16),
}

impl CollectionIndex {
    pub fn new(catalog: Rc<CollectionCatalogPackage>) -> Self {
        let mut items_by_kind_expansion: BTreeMap<(CollectionKind, String), Vec<usize>> =
            BTreeMap::new();
        let mut raw_sets: BTreeMap<(String, String, String), Vec<usize>> = BTreeMap::new();

        for (index, item) in catalog.items.iter().enumerate() {
            items_by_kind_expansion
                .entry((item.kind, item.expansion.clone()))
                .or_default()
                .push(index);
            if item.kind == CollectionKind::Equipment {
                raw_sets
                    .entry((
                        item.expansion.clone(),
                        item.patch.clone(),
                        effective_equipment_set_id(item),
                    ))
                    .or_default()
                    .push(index);
            }
        }

        let mut equipment_sets = raw_sets
            .into_iter()
            .map(|((expansion, patch, set_id), mut item_indices)| {
                item_indices.sort_by_key(|&index| {
                    let item = &catalog.items[index];
                    (item.slot_order, item.id)
                });
                let first = &catalog.items[item_indices[0]];
                let min_item_level = item_indices
                    .iter()
                    .map(|&index| catalog.items[index].level_item)
                    .min()
                    .unwrap_or_default();
                let max_item_level = item_indices
                    .iter()
                    .map(|&index| catalog.items[index].level_item)
                    .max()
                    .unwrap_or_default();
                let min_equip_level = item_indices
                    .iter()
                    .map(|&index| catalog.items[index].level_equip)
                    .min()
                    .unwrap_or_default();
                let max_equip_level = item_indices
                    .iter()
                    .map(|&index| catalog.items[index].level_equip)
                    .max()
                    .unwrap_or_default();
                let class_job_label = common_class_job_label(&catalog, &item_indices);
                let slot_count = item_indices
                    .iter()
                    .map(|&index| catalog.items[index].slot_order)
                    .collect::<HashSet<_>>()
                    .len();
                let search_text = item_indices
                    .iter()
                    .flat_map(|&index| {
                        let item = &catalog.items[index];
                        [item.name.as_str(), item.class_job_category_name.as_str()]
                    })
                    .chain(std::iter::once(first.set_name.as_str()))
                    .collect::<Vec<_>>()
                    .join(" ")
                    .to_lowercase();
                EquipmentSetGroup {
                    expansion,
                    patch,
                    set_id,
                    set_name: effective_equipment_set_name(first),
                    item_indices,
                    min_item_level,
                    max_item_level,
                    min_equip_level,
                    max_equip_level,
                    class_job_label,
                    slot_count,
                    is_set: slot_count >= 2,
                    search_text,
                }
            })
            .collect::<Vec<_>>();
        equipment_sets.sort_by(|left, right| {
            right
                .patch
                .cmp(&left.patch)
                .then(left.set_name.cmp(&right.set_name))
                .then(left.set_id.cmp(&right.set_id))
        });

        let mut collection_expansions = catalog
            .items
            .iter()
            .map(|item| item.expansion.clone())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        collection_expansions.sort_by(|left, right| {
            expansion_order(right)
                .cmp(&expansion_order(left))
                .then(left.cmp(right))
        });

        let equipment_slot_options = equipment_filter_options(&catalog, |item| &item.slot_name);
        let equipment_level_bounds =
            collect_equipment_level_bounds(&catalog, |item| item.level_equip);
        let equipment_item_level_bounds =
            collect_equipment_level_bounds(&catalog, |item| item.level_item);

        Self {
            catalog,
            items_by_kind_expansion,
            equipment_sets,
            collection_expansions,
            equipment_slot_options,
            equipment_level_bounds,
            equipment_item_level_bounds,
        }
    }

    pub fn items_for_kind_expansion(
        &self,
        kind: CollectionKind,
        expansion: &str,
    ) -> impl Iterator<Item = &CollectionItem> {
        self.items_by_kind_expansion
            .get(&(kind, expansion.to_string()))
            .into_iter()
            .flatten()
            .map(|&index| &self.catalog.items[index])
    }

    pub fn count_for_kind_expansion(&self, kind: CollectionKind, expansion: &str) -> usize {
        self.items_by_kind_expansion
            .get(&(kind, expansion.to_string()))
            .map(Vec::len)
            .unwrap_or_default()
    }
}

fn effective_equipment_set_id(item: &CollectionItem) -> String {
    if item.set_id.starts_with("model:") || item.set_name.starts_with("同模型套装") {
        format!("item:{}", item.id)
    } else {
        item.set_id.clone()
    }
}

fn effective_equipment_set_name(item: &CollectionItem) -> String {
    if item.set_id.starts_with("model:") || item.set_name.starts_with("同模型套装") {
        item.name.clone()
    } else {
        item.set_name.clone()
    }
}

fn expansion_order(name: &str) -> u8 {
    match name {
        "旧版遗留" => 0,
        "重生之境" => 1,
        "苍穹之禁城" => 2,
        "红莲之狂潮" => 3,
        "暗影之逆焰" => 4,
        "晓月之终途" => 5,
        "金曦之遗辉" => 6,
        "本地版本" => 7,
        _ => 0,
    }
}

fn equipment_filter_options(
    catalog: &CollectionCatalogPackage,
    value: impl Fn(&CollectionItem) -> &str,
) -> Vec<String> {
    let mut values = catalog
        .items
        .iter()
        .filter(|item| item.kind == CollectionKind::Equipment)
        .map(value)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    values.sort();
    values
}

fn collect_equipment_level_bounds(
    catalog: &CollectionCatalogPackage,
    value: impl Fn(&CollectionItem) -> u16,
) -> (u16, u16) {
    let mut values = catalog
        .items
        .iter()
        .filter(|item| item.kind == CollectionKind::Equipment)
        .map(value)
        .filter(|value| *value > 0);
    let Some(first) = values.next() else {
        return (0, 0);
    };
    values.fold((first, first), |(min, max), value| {
        (min.min(value), max.max(value))
    })
}

fn common_class_job_label(catalog: &CollectionCatalogPackage, indices: &[usize]) -> String {
    let labels = indices
        .iter()
        .map(|&index| catalog.items[index].class_job_category_name.as_str())
        .filter(|label| !label.is_empty())
        .collect::<HashSet<_>>();
    match labels.len() {
        0 => String::new(),
        1 => labels.into_iter().next().unwrap_or_default().to_string(),
        _ => "多职业".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use xiv_companion::CollectionCatalogCounts;

    fn equipment(id: u32, slot_order: u8) -> CollectionItem {
        CollectionItem {
            id,
            kind: CollectionKind::Equipment,
            name: format!("装备 {id}"),
            description: String::new(),
            icon: 0,
            item_ui_category: 0,
            item_search_category: 0,
            item_action: 0,
            equip_slot_category: 4,
            slot_name: "身体".to_string(),
            slot_order,
            level_item: 100,
            level_equip: 50,
            rarity: 1,
            class_job_category: 1,
            class_job_category_name: "所有职业".to_string(),
            item_series: 9,
            set_id: "series:9".to_string(),
            set_name: "测试套装".to_string(),
            set_item_ids: Vec::new(),
            expansion: "晓月之终途".to_string(),
            patch: "6.0".to_string(),
            model_main: 1,
            model_sub: 0,
            appearance_key: "equipment:1".to_string(),
        }
    }

    #[test]
    fn groups_equipment_by_patch_and_set_and_orders_slots() {
        let catalog = Rc::new(CollectionCatalogPackage {
            schema_version: xiv_companion::COLLECTION_CATALOG_SCHEMA_VERSION,
            generated_at: String::new(),
            game_version: String::new(),
            source: String::new(),
            counts: CollectionCatalogCounts::default(),
            items: vec![equipment(2, 7), equipment(1, 3)],
        });
        let index = CollectionIndex::new(catalog);
        assert_eq!(index.equipment_sets.len(), 1);
        assert_eq!(index.equipment_sets[0].item_indices, vec![1, 0]);
        assert_eq!(index.equipment_level_bounds, (50, 50));
        assert_eq!(index.equipment_item_level_bounds, (100, 100));
    }

    #[test]
    fn legacy_model_groups_are_exposed_as_single_items() {
        let mut first = equipment(1, 3);
        first.name = "骑士胸甲".to_string();
        first.set_id = "model:gear:14:0".to_string();
        first.set_name = "同模型套装 #0014".to_string();
        let mut second = equipment(2, 7);
        second.name = "法师长靴".to_string();
        second.set_id = first.set_id.clone();
        second.set_name = first.set_name.clone();
        let catalog = Rc::new(CollectionCatalogPackage {
            schema_version: 2,
            generated_at: String::new(),
            game_version: String::new(),
            source: String::new(),
            counts: CollectionCatalogCounts::default(),
            items: vec![first, second],
        });
        let index = CollectionIndex::new(catalog);
        assert_eq!(index.equipment_sets.len(), 2);
        assert_eq!(index.equipment_sets[0].item_indices.len(), 1);
        assert!(
            index
                .equipment_sets
                .iter()
                .any(|set| set.set_name == "骑士胸甲")
        );
        assert!(
            index
                .equipment_sets
                .iter()
                .any(|set| set.set_name == "法师长靴")
        );
    }

    #[test]
    fn expansion_tabs_are_sorted_latest_first() {
        let mut old = equipment(1, 3);
        old.expansion = "重生之境".to_string();
        old.set_id = "item:1".to_string();
        let mut latest = equipment(2, 3);
        latest.expansion = "金曦之遗辉".to_string();
        latest.set_id = "item:2".to_string();
        let catalog = Rc::new(CollectionCatalogPackage {
            schema_version: xiv_companion::COLLECTION_CATALOG_SCHEMA_VERSION,
            generated_at: String::new(),
            game_version: String::new(),
            source: String::new(),
            counts: CollectionCatalogCounts::default(),
            items: vec![old, latest],
        });
        let index = CollectionIndex::new(catalog);
        assert_eq!(index.collection_expansions[0], "金曦之遗辉");
        assert_eq!(index.collection_expansions[1], "重生之境");
    }
}
