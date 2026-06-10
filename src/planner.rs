use std::collections::{BTreeMap, HashMap, HashSet};

use serde::{Deserialize, Serialize};
#[cfg(feature = "wasm")]
use tsify::Tsify;

use crate::{CraftDataPackage, CraftItem, CraftRecipe, ItemSource, SourceChoice};

#[derive(Clone, Debug)]
pub struct CraftDataIndex {
    pub recipes_by_result: HashMap<u32, Vec<CraftRecipe>>,
    pub craftable_by_type: HashMap<u32, Vec<CraftRecipe>>,
    pub craft_type_order: Vec<u32>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "wasm", derive(Tsify))]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct CraftTreeNode {
    pub item_id: u32,
    pub amount_needed: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recipe: Option<CraftRecipe>,
    pub children: Vec<CraftTreeNode>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "wasm", derive(Tsify))]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct MaterialSummary {
    pub item_id: u32,
    pub amount: u32,
}

pub fn create_craft_data_index(data: &CraftDataPackage) -> CraftDataIndex {
    let mut recipes_by_result: HashMap<u32, Vec<CraftRecipe>> = HashMap::new();
    let mut craftable_by_type: HashMap<u32, Vec<CraftRecipe>> = HashMap::new();
    let mut craft_type_order = Vec::new();

    for recipe in &data.recipes {
        recipes_by_result
            .entry(recipe.result_item_id)
            .or_default()
            .push(recipe.clone());

        let craft_type = recipe.craft_type.clamp(0, 7);
        if !craftable_by_type.contains_key(&craft_type) {
            craft_type_order.push(craft_type);
        }
        craftable_by_type
            .entry(craft_type)
            .or_default()
            .push(recipe.clone());
    }

    for recipes in craftable_by_type.values_mut() {
        recipes.sort_by_key(|recipe| recipe.result_item_id);
    }

    CraftDataIndex {
        recipes_by_result,
        craftable_by_type,
        craft_type_order,
    }
}

pub fn craftable_recipes(
    data: &CraftDataPackage,
    index: &CraftDataIndex,
    craft_type: Option<u32>,
    query: &str,
    limit: usize,
) -> Vec<CraftRecipe> {
    let text = query.trim().to_lowercase();
    let source: Vec<CraftRecipe> = match craft_type {
        Some(craft_type) => index
            .craftable_by_type
            .get(&craft_type)
            .cloned()
            .unwrap_or_default(),
        None => index
            .craft_type_order
            .iter()
            .filter_map(|craft_type| index.craftable_by_type.get(craft_type))
            .flatten()
            .cloned()
            .collect(),
    };

    source
        .into_iter()
        .filter(|recipe| {
            if text.is_empty() {
                return true;
            }
            let name = get_item_name(data, recipe.result_item_id).to_lowercase();
            name.contains(&text)
                || recipe.result_item_id.to_string().contains(&text)
                || recipe.id.to_string().contains(&text)
        })
        .take(limit)
        .collect()
}

pub fn get_item(data: &CraftDataPackage, item_id: u32) -> Option<&CraftItem> {
    data.items.get(&item_id.to_string())
}

pub fn get_item_name(data: &CraftDataPackage, item_id: u32) -> String {
    get_item(data, item_id)
        .map(|item| item.name.clone())
        .unwrap_or_else(|| format!("物品 #{item_id}"))
}

pub fn build_craft_tree(item_id: u32, amount: u32, index: &CraftDataIndex) -> CraftTreeNode {
    let mut visited = HashSet::new();
    build_craft_tree_inner(item_id, amount, index, &mut visited)
}

fn build_craft_tree_inner(
    item_id: u32,
    amount: u32,
    index: &CraftDataIndex,
    visited: &mut HashSet<u32>,
) -> CraftTreeNode {
    let recipe = if !visited.contains(&item_id) {
        index
            .recipes_by_result
            .get(&item_id)
            .and_then(|recipes| recipes.first())
            .cloned()
    } else {
        None
    };

    let mut children = Vec::new();
    if let Some(recipe) = &recipe {
        visited.insert(item_id);
        let craft_count = amount.div_ceil(recipe.result_amount.max(1));
        for ingredient in &recipe.ingredients {
            children.push(build_craft_tree_inner(
                ingredient.item_id,
                ingredient.amount * craft_count,
                index,
                visited,
            ));
        }
        visited.remove(&item_id);
    }

    CraftTreeNode {
        item_id,
        amount_needed: amount,
        recipe,
        children,
    }
}

pub fn summarize_materials(
    node: &CraftTreeNode,
    collapsed: &HashSet<String>,
) -> Vec<MaterialSummary> {
    let mut totals = BTreeMap::new();
    collect_leaves(node, 0, collapsed, &mut totals);
    totals
        .into_iter()
        .map(|(item_id, amount)| MaterialSummary { item_id, amount })
        .collect()
}

fn collect_leaves(
    node: &CraftTreeNode,
    depth: u32,
    collapsed: &HashSet<String>,
    totals: &mut BTreeMap<u32, u32>,
) {
    let key = collapse_key(node.item_id, depth);
    if node.children.is_empty() || collapsed.contains(&key) {
        *totals.entry(node.item_id).or_default() += node.amount_needed;
        return;
    }

    for child in &node.children {
        collect_leaves(child, depth + 1, collapsed, totals);
    }
}

pub fn collapse_key(item_id: u32, depth: u32) -> String {
    format!("{item_id}:{depth}")
}

pub fn default_source_index(sources: &[ItemSource]) -> Option<usize> {
    let (first, rest) = sources.split_first()?;
    let mut best_index = 0;
    let mut best_priority = source_priority(first);

    for (offset, source) in rest.iter().enumerate() {
        let priority = source_priority(source);
        if priority < best_priority {
            best_index = offset + 1;
            best_priority = priority;
        }
    }

    Some(best_index)
}

pub fn resolve_source<'a>(
    item_id: u32,
    sources: &'a [ItemSource],
    choices: &HashMap<u32, SourceChoice>,
) -> Option<&'a ItemSource> {
    match choices.get(&item_id) {
        Some(SourceChoice::Ignore | SourceChoice::Market) => None,
        Some(SourceChoice::Index { index }) => sources.get(*index),
        None => default_source_index(sources).and_then(|index| sources.get(index)),
    }
}

pub fn source_label(source: &ItemSource) -> &'static str {
    match source {
        ItemSource::GilShop { .. } => "金币商店",
        ItemSource::SpecialShop { .. } => "兑换",
        ItemSource::Fishing { .. } => "钓鱼",
        ItemSource::Gathering => "采集",
    }
}

pub fn source_priority(source: &ItemSource) -> u8 {
    match source {
        ItemSource::Gathering | ItemSource::Fishing { .. } => 1,
        ItemSource::GilShop { .. } => 2,
        ItemSource::SpecialShop { .. } => 3,
    }
}
