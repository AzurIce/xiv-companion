use std::collections::{BTreeMap, HashMap, HashSet};
use std::rc::Rc;

use xiv_companion::{CollectionCatalogPackage, CollectionEntryKey, CollectionItem, CollectionKind};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EquipmentSetGroup {
    pub expansion: String,
    pub patch: String,
    pub set_id: String,
    pub set_name: String,
    pub item_indices: Vec<usize>,
    pub max_item_level: u16,
    pub class_job_label: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CollectionIndex {
    pub catalog: Rc<CollectionCatalogPackage>,
    pub items_by_kind: BTreeMap<CollectionKind, Vec<usize>>,
    pub equipment_sets: Vec<EquipmentSetGroup>,
    pub items_by_appearance: HashMap<String, Vec<usize>>,
}

impl CollectionIndex {
    pub fn new(catalog: Rc<CollectionCatalogPackage>) -> Self {
        let mut items_by_kind: BTreeMap<CollectionKind, Vec<usize>> = BTreeMap::new();
        let mut raw_sets: BTreeMap<(String, String, String), Vec<usize>> = BTreeMap::new();
        let mut items_by_appearance: HashMap<String, Vec<usize>> = HashMap::new();

        for (index, item) in catalog.items.iter().enumerate() {
            items_by_kind.entry(item.kind).or_default().push(index);
            if item.kind == CollectionKind::Equipment {
                raw_sets
                    .entry((
                        item.expansion.clone(),
                        item.patch.clone(),
                        item.set_id.clone(),
                    ))
                    .or_default()
                    .push(index);
                if !item.appearance_key.is_empty() {
                    items_by_appearance
                        .entry(item.appearance_key.clone())
                        .or_default()
                        .push(index);
                }
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
                let max_item_level = item_indices
                    .iter()
                    .map(|&index| catalog.items[index].level_item)
                    .max()
                    .unwrap_or_default();
                let class_job_label = common_class_job_label(&catalog, &item_indices);
                EquipmentSetGroup {
                    expansion,
                    patch,
                    set_id,
                    set_name: first.set_name.clone(),
                    item_indices,
                    max_item_level,
                    class_job_label,
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

        Self {
            catalog,
            items_by_kind,
            equipment_sets,
            items_by_appearance,
        }
    }

    pub fn items_for_kind(&self, kind: CollectionKind) -> impl Iterator<Item = &CollectionItem> {
        self.items_by_kind
            .get(&kind)
            .into_iter()
            .flatten()
            .map(|&index| &self.catalog.items[index])
    }

    pub fn has_obtained_sibling_model(
        &self,
        item: &CollectionItem,
        obtained: &HashSet<CollectionEntryKey>,
    ) -> bool {
        if item.appearance_key.is_empty() {
            return false;
        }
        self.items_by_appearance
            .get(&item.appearance_key)
            .into_iter()
            .flatten()
            .map(|&index| &self.catalog.items[index])
            .any(|sibling| sibling.id != item.id && obtained.contains(&sibling.key()))
    }
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
            schema_version: 2,
            generated_at: String::new(),
            game_version: String::new(),
            source: String::new(),
            counts: CollectionCatalogCounts::default(),
            items: vec![equipment(2, 7), equipment(1, 3)],
        });
        let index = CollectionIndex::new(catalog);
        assert_eq!(index.equipment_sets.len(), 1);
        assert_eq!(index.equipment_sets[0].item_indices, vec![1, 0]);
    }
}
